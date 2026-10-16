//! Thin Tauri 2 adapter for Ghostlight's orchestrator-owned workbench facade.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
use ghostlight_relay::os::session;
use ghostlight_relay::protocol::{QuitReason, StartReason};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, RunEvent, State, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

use crate::governance::effective::EffectiveAuthority;
use crate::install::{
    HarnessAction, HarnessActionResult, HarnessCopyKind, HarnessSetupResult, HarnessSummary,
};
use crate::intake::desktop::TrayItemId;
use crate::language::environment::SESSION_LISTENER_FAILED;
use crate::language::workbench::{COPIED_COMMAND, COPIED_SETUP, TRAY_TOOLTIP};
use crate::service::lifetime::QUIT_GRACE;
use crate::service::{Quit, ServiceHost};
use crate::workbench::context::WebContext;
use crate::workbench::{
    validate_policy_document, validate_search_query, PolicyPreview, RuntimeSource, SearchHit,
    WorkbenchDestination, WorkbenchEvent, WorkbenchEventSink, WorkbenchFacade,
    WorkbenchIntentResult, WorkbenchNativeActs, WorkbenchNotification, WorkbenchPresentationError,
    WorkbenchPresentationPort, WorkbenchRuntimeIntent, WorkbenchSnapshot,
};

const MAIN_WINDOW: &str = "main";
/// Single channel the disposable workbench listens on for sequenced orchestrator changes.
const CHANGE_EVENT: &str = "ghostlight://change";
/// The status a `serve` start exits with when it never bound its endpoint in time.
pub const READY_STALLED_EXIT: i32 = 3;

struct DesktopState {
    workbench: WorkbenchFacade,
    /// A person's Quit from the tray or the workbench goes through the service, which sets the
    /// quit latch and says goodbye before the desktop exits.
    quit: Quit,
    window_lifecycle: Mutex<WindowLifecycle>,
}

#[derive(Default)]
struct WindowLifecycle {
    #[cfg(target_os = "linux")]
    replacement_pending: bool,
}

struct NativePresentation {
    app: AppHandle,
}

impl WorkbenchPresentationPort for NativePresentation {
    fn reveal(&self) -> Result<(), WorkbenchPresentationError> {
        show_workbench(&self.app)
    }

    fn notify(
        &self,
        notification: WorkbenchNotification,
    ) -> Result<(), WorkbenchPresentationError> {
        self.app
            .notification()
            .builder()
            .title(notification.title)
            .body(notification.body)
            .show()
            .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))
    }
}

/// The operating-system acts the service asks of this process: open a link, open a folder,
/// choose a file.
struct NativeActs {
    app: AppHandle,
}

impl WorkbenchNativeActs for NativeActs {
    fn open_url(&self, url: &str) -> Result<(), WorkbenchPresentationError> {
        self.app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|error| WorkbenchPresentationError::ActFailed(error.to_string()))
    }

    fn open_folder(&self, path: &Path) -> Result<(), WorkbenchPresentationError> {
        self.app
            .opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|error| WorkbenchPresentationError::ActFailed(error.to_string()))
    }

    fn pick_file(&self, title: &str) -> Result<Option<PathBuf>, WorkbenchPresentationError> {
        self.app
            .dialog()
            .file()
            .set_title(title)
            .blocking_pick_file()
            .map(|selected| {
                selected
                    .into_path()
                    .map_err(|error| WorkbenchPresentationError::ActFailed(error.to_string()))
            })
            .transpose()
    }
}

/// Relays sequenced orchestrator changes to the disposable workbench WebView.
struct NativeEvents {
    app: AppHandle,
}

impl WorkbenchEventSink for NativeEvents {
    fn publish(&self, event: WorkbenchEvent) {
        // A closed or reloading WebView is an ordinary presentation outcome, never a domain
        // failure. The surface resynchronizes from a snapshot when it next opens.
        let _ = self.app.emit(CHANGE_EVENT, event);
    }
}

/// Start the service and its backgrounded desktop in this process, for a start made for `reason`.
///
/// The process stops by itself, with [`READY_STALLED_EXIT`], when it is not accepting openings
/// within the ready limit of taking the service lock.
pub fn run(reason: StartReason) -> Result<()> {
    #[cfg(target_os = "linux")]
    configure_linux_webkit();

    // Heard from the first moment: a start that is still waiting for the service lock leaves at
    // once, and once the host exists the session end goes through its quit path.
    let ending = Arc::new(OnceLock::<Quit>::new());
    let _session_listener = listen_for_session_end(Arc::clone(&ending))
        .map_err(|error| anyhow::anyhow!("{SESSION_LISTENER_FAILED} {error}"))?;

    let host = Arc::new(ServiceHost::start_waiting(
        &ghostlight_relay::home::home()?,
        reason,
    )?);
    let _ = ending.set(host.quit_handle());
    host.expect_ready_by(host.ready_deadline(), || {
        std::process::exit(READY_STALLED_EXIT)
    })?;
    let ready_host = Arc::clone(&host);
    let workbench = host.workbench.clone();
    let setup_workbench = workbench.clone();
    let tray_available = Arc::new(AtomicBool::new(false));
    let setup_tray_available = Arc::clone(&tray_available);
    let setup_host = Arc::clone(&host);
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        // Registered in Rust only. The capability file grants the webview neither plugin's generic
        // commands; it reaches them only through the closed harness commands below.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        // Registered in Rust only. The capability file grants the webview no opener permission,
        // so the surface cannot reach this except through the closed command below.
        .plugin(tauri_plugin_opener::init())
        .manage(DesktopState {
            workbench,
            quit: host.quit_handle(),
            window_lifecycle: Mutex::new(WindowLifecycle::default()),
        })
        .invoke_handler(tauri::generate_handler![
            workbench_context,
            workbench_snapshot,
            workbench_search,
            workbench_policy,
            preview_user_policy,
            apply_user_policy,
            remove_user_policy,
            apply_runtime_intent,
            set_diagnostics,
            set_watching,
            reveal_diagnostics,
            refresh_harnesses,
            setup_detected_harnesses,
            manage_harness,
            locate_harness,
            copy_harness_text,
            open_harness_download,
            test_notification,
            open_destination,
            continue_in_window,
            quit_ghostlight
        ])
        .on_window_event(|_window, event| {
            if let WindowEvent::Destroyed = event {
                eprintln!("Ghostlight workbench window ended; the tray can create a replacement");
                #[cfg(target_os = "linux")]
                continue_linux_replacement(_window.app_handle());
            }
        })
        .setup(move |app| {
            setup_workbench.attach_events(Arc::new(NativeEvents {
                app: app.handle().clone(),
            }));
            match build_tray(app) {
                Ok(()) => {
                    setup_tray_available.store(true, Ordering::SeqCst);
                    setup_host.native_tray_present();
                }
                Err(error) => {
                    eprintln!(
                        "Ghostlight could not create a tray icon in this desktop session: {error}; use the Applications entry or 'ghostlight open'"
                    );
                }
            }
            Ok(())
        });

    let built = catch_unwind(AssertUnwindSafe(|| {
        builder.build(tauri::generate_context!())
    }));
    let app = match built {
        Ok(Ok(app)) => app,
        Ok(Err(error)) => {
            record_desktop_failure(&host, "native desktop construction failed");
            return Err(error.into());
        }
        Err(_) => {
            record_desktop_failure(&host, "native desktop initialization panicked; check the caller's desktop session environment");
            anyhow::bail!("Ghostlight desktop authority failed during startup");
        }
    };
    // A quit from the service asks for the same exit the tray Quit takes.
    let quit_handle = app.handle().clone();
    host.install_quit_signal(Arc::new(move || quit_handle.exit(0)));
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        app.run_return(move |app, event| match event {
            RunEvent::Ready => {
                // Keep the configured workbench as metadata until the native event loop is live.
                // This is the same disposable construction seam that tray activation uses after
                // Close, and it prevents Windows from losing its startup window.
                match build_workbench(app) {
                    Ok(_) => {
                        if let Err(error) = background_workbench(app) {
                            eprintln!("Ghostlight could not background its workbench: {error}");
                        }
                    }
                    Err(error) => {
                        eprintln!("Ghostlight workbench is unavailable: {error}");
                        if !tray_available.load(Ordering::SeqCst) {
                            record_desktop_failure(
                                &ready_host,
                                "neither tray nor workbench could be constructed",
                            );
                            eprintln!("Ghostlight has no desktop interaction route and will stop");
                            app.exit(1);
                            return;
                        }
                    }
                }
                // Publish activation only after startup has finished constructing and registering
                // its window. Otherwise a service-thread Open can build a second native window
                // before Tauri registers the first under MAIN_WINDOW. Early callers already retry
                // unavailable presentation; a recoverable construction failure still permits Open.
                app.state::<DesktopState>()
                    .workbench
                    .attach_presentation(Arc::new(NativePresentation { app: app.clone() }));
                app.state::<DesktopState>()
                    .workbench
                    .attach_native_acts(Arc::new(NativeActs { app: app.clone() }));
                if let Err(error) = ready_host.publish_ready() {
                    record_desktop_failure(&ready_host, "desktop-ready runtime publication failed");
                    eprintln!("Ghostlight could not publish desktop readiness: {error}");
                    app.exit(1);
                    return;
                }
                eprintln!("Ghostlight {} desktop ready", env!("CARGO_PKG_VERSION"));
            }
            RunEvent::ExitRequested { code, api, .. }
                if should_prevent_desktop_exit(code, session::session_ending()) =>
            {
                api.prevent_exit();
            }
            _ => {}
        })
    }));
    match outcome {
        Ok(0) => Ok(()),
        Ok(code) => {
            record_desktop_failure(&host, "desktop event loop exited unsuccessfully");
            anyhow::bail!("Ghostlight desktop authority exited with status {code}");
        }
        Err(_) => {
            record_desktop_failure(&host, "desktop event loop panicked");
            anyhow::bail!("Ghostlight desktop authority stopped unexpectedly");
        }
    }
}

fn record_desktop_failure(host: &ServiceHost, detail: &str) {
    host.diagnostics.sink().emit(
        ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
        ghostlight_bridge::diagnostics::Level::Error,
        None,
        detail,
    );
}

fn build_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let item = |id: TrayItemId| MenuItem::with_id(app, id.as_str(), id.label(), true, None::<&str>);
    let open = item(TrayItemId::Open)?;
    let hold = item(TrayItemId::Hold)?;
    let resume = item(TrayItemId::Resume)?;
    let quit = item(TrayItemId::Quit)?;
    let menu = Menu::with_items(app, &[&open, &hold, &resume, &quit])?;
    TrayIconBuilder::with_id("ghostlight")
        .icon(tauri::include_image!("../../extension/icons/icon32.png"))
        .tooltip(TRAY_TOOLTIP)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match TrayItemId::parse(event.id().as_ref()) {
            Some(TrayItemId::Open) => reveal_from_tray(app),
            Some(TrayItemId::Hold) => apply_tray_intent(app, WorkbenchRuntimeIntent::Hold),
            Some(TrayItemId::Resume) => apply_tray_intent(app, WorkbenchRuntimeIntent::Resume),
            Some(TrayItemId::Quit) => quit_by_person(app),
            None => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                reveal_from_tray(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn reveal_from_tray(app: &AppHandle) {
    if let Err(error) = show_workbench(app) {
        eprintln!("Ghostlight could not open its workbench from the tray: {error}");
    }
}

fn apply_tray_intent(app: &AppHandle, intent: WorkbenchRuntimeIntent) {
    let state = app.state::<DesktopState>();
    let _ = state
        .workbench
        .apply_runtime_control(intent.into(), RuntimeSource::Tray);
}

fn show_workbench(app: &AppHandle) -> Result<(), WorkbenchPresentationError> {
    #[cfg(target_os = "linux")]
    {
        show_linux_workbench(app)
    }

    #[cfg(target_os = "windows")]
    {
        show_windows_workbench(app)
    }
}

#[cfg(target_os = "linux")]
fn show_linux_workbench(app: &AppHandle) -> Result<(), WorkbenchPresentationError> {
    let state = app.state::<DesktopState>();
    let mut lifecycle = state
        .window_lifecycle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if lifecycle.replacement_pending {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        // xdg-shell deliberately gives a client neither its minimized state nor a request to
        // undo minimization. Rebuild the disposable view so Open has one deterministic meaning.
        lifecycle.replacement_pending = true;
        if let Err(error) = window.destroy() {
            lifecycle.replacement_pending = false;
            return Err(WorkbenchPresentationError::Native(error.to_string()));
        }
        return Ok(());
    }
    let window = build_workbench(app)?;
    show_window(&window)
}

#[cfg(target_os = "windows")]
fn show_windows_workbench(app: &AppHandle) -> Result<(), WorkbenchPresentationError> {
    let state = app.state::<DesktopState>();
    let _lifecycle = state
        .window_lifecycle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let window = match app.get_webview_window(MAIN_WINDOW) {
        Some(window) => window,
        None => build_workbench(app)?,
    };
    show_window(&window)
}

#[cfg(target_os = "linux")]
fn continue_linux_replacement(app: &AppHandle) {
    let state = app.state::<DesktopState>();
    let replacement_pending = state
        .window_lifecycle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .replacement_pending;
    if !replacement_pending {
        return;
    }

    let app = app.clone();
    glib::idle_add_local_once(move || {
        let result = build_workbench(&app).and_then(|window| show_window(&window));
        let state = app.state::<DesktopState>();
        state
            .window_lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .replacement_pending = false;
        if let Err(error) = result {
            eprintln!("Ghostlight could not replace its Linux workbench: {error}");
        }
    });
}

fn build_workbench(app: &AppHandle) -> Result<WebviewWindow, WorkbenchPresentationError> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == MAIN_WINDOW)
        .ok_or_else(|| {
            WorkbenchPresentationError::Native(
                "Ghostlight workbench configuration is unavailable".into(),
            )
        })?;
    let window = WebviewWindowBuilder::from_config(app, config)
        .and_then(WebviewWindowBuilder::build)
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))?;
    monitor_workbench(&window)?;
    Ok(window)
}

/// Listen for the end of the session from this process's start.
///
/// On the final signal the fallback exit is armed first. A service that exists then says goodbye
/// with `session_end` on every stream and leaves through the desktop's exit; a start that has no
/// service yet holds nothing and leaves at once.
fn listen_for_session_end(
    ending: Arc<OnceLock<Quit>>,
) -> std::io::Result<session::SessionListener> {
    session::listen(move |event| session::stop_on_final(event, || end_session(ending.get())))
}

/// End this process because the session is ending: through the quit path when the service
/// exists, at once when it does not.
fn end_session(quit: Option<&Quit>) {
    match quit {
        Some(quit) => quit.session_end(),
        None => std::process::exit(0),
    }
}

/// Whether the desktop contains an exit request instead of leaving: only the implicit loss of
/// the last window is contained, and never while the session is ending.
fn should_prevent_desktop_exit(code: Option<i32>, session_ending: bool) -> bool {
    !session_ending && code.is_none()
}

#[cfg(target_os = "linux")]
fn monitor_workbench(window: &WebviewWindow) -> Result<(), WorkbenchPresentationError> {
    let failed_window = window.clone();
    window
        .with_webview(move |webview| {
            use webkit2gtk::WebViewExt;

            webview
                .inner()
                .connect_web_process_terminated(move |_, reason| {
                    eprintln!(
                        "Ghostlight workbench renderer ended ({reason:?}); discarding its window"
                    );
                    let window = failed_window.clone();
                    glib::idle_add_once(move || {
                        if let Err(error) = window.destroy() {
                            eprintln!(
                                "Ghostlight could not discard its failed workbench window: {error}"
                            );
                        }
                    });
                });
        })
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))
}

#[cfg(target_os = "linux")]
fn configure_linux_webkit() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some() {
        return;
    }

    // WebKitGTK's DMA-BUF path remains unreliable with the proprietary NVIDIA stack. Apply the
    // compatibility policy before GTK/WebKit starts, while preserving an explicit user override.
    if Path::new("/sys/module/nvidia").exists() || Path::new("/proc/driver/nvidia/version").exists()
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[cfg(target_os = "windows")]
fn monitor_workbench(_: &WebviewWindow) -> Result<(), WorkbenchPresentationError> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn background_workbench(app: &AppHandle) -> Result<(), WorkbenchPresentationError> {
    app.get_webview_window(MAIN_WINDOW).ok_or_else(|| {
        WorkbenchPresentationError::Native("Ghostlight workbench window is unavailable".into())
    })?;
    // The configured Linux window begins hidden. GTK3 cannot reliably deiconify a window that
    // started minimized under Wayland, while its native presentation operation maps a hidden
    // window correctly when the tray or activation endpoint asks for it.
    Ok(())
}

#[cfg(target_os = "windows")]
fn background_workbench(app: &AppHandle) -> Result<(), WorkbenchPresentationError> {
    let window = app.get_webview_window(MAIN_WINDOW).ok_or_else(|| {
        WorkbenchPresentationError::Native("Ghostlight workbench window is unavailable".into())
    })?;
    window
        .show()
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))?;
    window
        .minimize()
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))
}

#[cfg(target_os = "linux")]
fn show_window(window: &WebviewWindow) -> Result<(), WorkbenchPresentationError> {
    window
        .show()
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))?;
    window
        .unminimize()
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))?;
    let focus_window = window.clone();
    window
        .run_on_main_thread(move || {
            glib::idle_add_local_once(move || {
                if let Err(error) = focus_window.set_focus() {
                    eprintln!("Ghostlight could not focus its workbench: {error}");
                }
            });
        })
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))
}

#[cfg(target_os = "windows")]
fn show_window(window: &WebviewWindow) -> Result<(), WorkbenchPresentationError> {
    window
        .unminimize()
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))?;
    window
        .show()
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))?;
    window
        .set_focus()
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))
}

/// What this window may do: the Control context, with every act available.
#[tauri::command]
fn workbench_context(state: State<'_, DesktopState>) -> serde_json::Value {
    state.workbench.context_document(WebContext::Control)
}

#[tauri::command]
fn workbench_snapshot(state: State<'_, DesktopState>) -> WorkbenchSnapshot {
    state.workbench.snapshot()
}

#[tauri::command]
fn workbench_search(
    query: String,
    state: State<'_, DesktopState>,
) -> Result<Vec<SearchHit>, String> {
    validate_search_query(&query)?;
    Ok(state.workbench.search(&query))
}

/// The compiled policy for the Policy destination.
#[tauri::command]
fn workbench_policy(state: State<'_, DesktopState>) -> EffectiveAuthority {
    state.workbench.policy()
}

/// Decide recorded work against a candidate policy without applying or recording anything.
#[tauri::command]
fn preview_user_policy(
    document: String,
    state: State<'_, DesktopState>,
) -> Result<PolicyPreview, String> {
    validate_policy_document(&document)?;
    state.workbench.preview_user_policy(&document)
}

/// Apply one locally authored user policy.
///
/// The surface hands over a complete document and the orchestrator decides everything about it:
/// whether authoring is permitted here, whether the document is valid, and where it may be written.
/// There is no path, no file handle, and no partial write on this seam (ADR-0122 Decision 4).
#[tauri::command]
fn apply_user_policy(
    document: String,
    state: State<'_, DesktopState>,
) -> Result<WorkbenchIntentResult, String> {
    validate_policy_document(&document)?;
    state
        .workbench
        .apply_user_policy(&document)
        .map_err(|error| error.to_string())
}

/// Remove this machine's user policy.
#[tauri::command]
fn remove_user_policy(state: State<'_, DesktopState>) -> Result<WorkbenchIntentResult, String> {
    state
        .workbench
        .remove_user_policy()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_runtime_intent(
    intent: WorkbenchRuntimeIntent,
    state: State<'_, DesktopState>,
) -> WorkbenchIntentResult {
    state.workbench.apply_runtime_intent(intent)
}

#[tauri::command]
async fn refresh_harnesses(state: State<'_, DesktopState>) -> Result<Vec<HarnessSummary>, String> {
    let workbench = state.workbench.clone();
    tauri::async_runtime::spawn_blocking(move || {
        workbench
            .refresh_harnesses()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn setup_detected_harnesses(
    state: State<'_, DesktopState>,
) -> Result<HarnessSetupResult, String> {
    let workbench = state.workbench.clone();
    tauri::async_runtime::spawn_blocking(move || {
        workbench
            .setup_detected_harnesses()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn manage_harness(
    id: String,
    action: HarnessAction,
    state: State<'_, DesktopState>,
) -> Result<HarnessActionResult, String> {
    let workbench = state.workbench.clone();
    tauri::async_runtime::spawn_blocking(move || {
        workbench
            .manage_harness(&id, action)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn locate_harness(
    id: String,
    state: State<'_, DesktopState>,
) -> Result<Option<HarnessActionResult>, String> {
    let workbench = state.workbench.clone();
    // The dialog blocks until the person answers, so it never runs on the main thread.
    tauri::async_runtime::spawn_blocking(move || {
        workbench
            .locate_harness_by_picking(&id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn copy_harness_text(
    id: String,
    kind: HarnessCopyKind,
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<String, String> {
    let text = state
        .workbench
        .harness_copy_text(&id, kind)
        .map_err(|error| error.to_string())?;
    app.clipboard()
        .write_text(text)
        .map_err(|error| error.to_string())?;
    Ok(match kind {
        HarnessCopyKind::Command => COPIED_COMMAND.into(),
        HarnessCopyKind::Setup => COPIED_SETUP.into(),
    })
}

#[tauri::command]
fn open_harness_download(product_id: String, state: State<'_, DesktopState>) -> Result<(), String> {
    state
        .workbench
        .open_harness_download(&product_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn test_notification(state: State<'_, DesktopState>) -> Result<(), String> {
    state
        .workbench
        .test_notification()
        .map_err(|error| error.to_string())
}

/// Turn the shared process-diagnostics marker on or off and report the resulting state.
#[tauri::command]
fn set_diagnostics(on: bool, state: State<'_, DesktopState>) -> Result<serde_json::Value, String> {
    serde_json::to_value(state.workbench.set_diagnostics(on)).map_err(|error| error.to_string())
}

/// Turn "Others can watch" on or off and report the Controls part the window now shows.
#[tauri::command]
fn set_watching(watch: bool, state: State<'_, DesktopState>) -> Result<serde_json::Value, String> {
    let controls = state
        .workbench
        .set_watching(watch)
        .map_err(|error| error.to_string())?;
    serde_json::to_value(controls).map_err(|error| error.to_string())
}

/// Open the diagnostics folder in the operating system's file manager. The folder is the one
/// the diagnostics state names; this actuation lives in the native process, and the WebView
/// gains no opener permission.
#[tauri::command]
fn reveal_diagnostics(state: State<'_, DesktopState>) -> Result<(), String> {
    state
        .workbench
        .reveal_diagnostics()
        .map_err(|error| error.to_string())
}

/// Open one of the destinations Ghostlight is willing to point at.
///
/// The surface sends a name from a closed vocabulary, never an address, so this cannot be talked
/// into opening something the product did not choose.
#[tauri::command]
fn open_destination(destination: WorkbenchDestination, app: AppHandle) -> Result<(), String> {
    app.state::<DesktopState>()
        .workbench
        .open_destination(destination)
        .map_err(|error| error.to_string())
}

/// Bring the Ghostlight window forward. This window already is the one, so it only shows it.
#[tauri::command]
fn continue_in_window(state: State<'_, DesktopState>) -> Result<(), String> {
    state.workbench.reveal().map_err(|error| error.to_string())
}

/// A person's Quit: the service sets the latch, finishes what is in flight, says goodbye on every
/// stream and then asks this desktop to exit.
fn quit_by_person(app: &AppHandle) {
    app.state::<DesktopState>()
        .quit
        .begin(QuitReason::Quit, QUIT_GRACE);
}

/// The page's Quit. It answers what the loopback route answers, so the page says one thing.
#[tauri::command]
fn quit_ghostlight(app: AppHandle) -> serde_json::Value {
    quit_by_person(&app);
    serde_json::json!({ "message": crate::language::workbench::QUITTING })
}

#[cfg(test)]
mod tests {
    use super::should_prevent_desktop_exit;

    #[test]
    fn only_implicit_window_loss_is_contained() {
        assert!(should_prevent_desktop_exit(None, false));
        assert!(!should_prevent_desktop_exit(Some(0), false));
        assert!(!should_prevent_desktop_exit(Some(1), false));
    }

    #[test]
    fn nothing_is_contained_while_the_session_is_ending() {
        assert!(!should_prevent_desktop_exit(None, true));
        assert!(!should_prevent_desktop_exit(Some(0), true));
        assert!(!should_prevent_desktop_exit(Some(1), true));
    }

    #[test]
    fn the_workbench_listens_for_changes_without_gaining_the_right_to_emit_them() {
        let capability = include_str!("../../capabilities/main.json");
        assert!(capability.contains("core:event:allow-listen"));
        assert!(!capability.contains("core:event:allow-emit"));
    }

    #[test]
    fn the_notification_plugin_can_bootstrap_without_exposing_notification_actions() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/main.json"))
                .expect("the workbench capability must be valid JSON");
        let notification_permissions = capability["permissions"]
            .as_array()
            .expect("the workbench capability must list permissions")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter(|permission| permission.starts_with("notification:"))
            .collect::<Vec<_>>();

        assert_eq!(
            notification_permissions,
            ["notification:allow-is-permission-granted"],
            "the plugin bootstrap needs permission-state access, not notification authority"
        );
    }

    #[test]
    fn clearing_the_monitor_is_disposable_view_state_not_an_audit_mutation() {
        let desktop = include_str!("mod.rs");

        let commands = desktop
            .split_once(".invoke_handler(tauri::generate_handler![")
            .and_then(|(_, rest)| rest.split_once("])"))
            .map(|(commands, _)| commands)
            .expect("the desktop keeps an explicit Tauri command allowlist");
        assert!(
            !commands.contains("clear"),
            "clearing visible rows must not become a Tauri command"
        );
    }

    #[test]
    fn the_page_can_ask_what_it_may_do_and_carry_on_in_the_window() {
        let desktop = include_str!("mod.rs");
        let commands = desktop
            .split_once(".invoke_handler(tauri::generate_handler![")
            .and_then(|(_, rest)| rest.split_once("])"))
            .map(|(commands, _)| commands)
            .expect("the desktop keeps an explicit Tauri command allowlist");
        let transport = include_str!("../../ui/lib/transport.js");
        // Each command the shared transport names for the shell is registered, and each one the
        // shell registers is named by the transport, so neither side can drift alone.
        for command in [
            "workbench_context",
            "set_diagnostics",
            "set_watching",
            "continue_in_window",
            "quit_ghostlight",
        ] {
            assert!(commands.contains(command), "{command} is not registered");
            assert!(
                transport.contains(&format!("\"{command}\"")),
                "the transport never calls {command}"
            );
        }
        assert!(
            !commands.contains("toggle_diagnostics"),
            "the page sets diagnostics now; a toggle would leave two ways to say it"
        );
    }

    #[test]
    fn the_workbench_grants_its_surface_no_permission_to_open_anything() {
        let capability = include_str!("../../capabilities/main.json");
        // The opener plugin is registered in Rust so the closed command can use it. The webview
        // must not be handed it directly, or the vocabulary above stops being the only way out.
        assert!(
            !capability.contains("opener:"),
            "the surface was granted the opener directly, bypassing the closed destinations"
        );
        let desktop = include_str!("mod.rs");
        assert!(desktop.contains("tauri_plugin_opener::init()"));
        assert!(desktop.contains("fn open_destination(destination: WorkbenchDestination"));
    }

    #[test]
    fn harness_native_plugins_are_available_only_through_closed_commands() {
        let capability = include_str!("../../capabilities/main.json");
        for permission in ["dialog:", "clipboard-manager:", "opener:", "shell:", "fs:"] {
            assert!(
                !capability.contains(permission),
                "the workbench bypasses the closed harness command through {permission}"
            );
        }
        let desktop = include_str!("mod.rs");
        for command in [
            "fn locate_harness(",
            "fn copy_harness_text(",
            "fn open_harness_download(",
        ] {
            assert!(
                desktop.contains(command),
                "missing closed command {command}"
            );
        }
    }
}
