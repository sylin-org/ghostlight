//! Thin Tauri 2 adapter for Ghostlight's orchestrator-owned workbench facade.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use anyhow::Result;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};
use tauri_plugin_notification::NotificationExt;

use crate::install::{HarnessAction, HarnessActionResult, HarnessSummary};
use crate::service::ServiceHost;
use crate::workbench::{
    SearchHit, WorkbenchEvent, WorkbenchEventSink, WorkbenchFacade, WorkbenchIntentResult,
    WorkbenchNotification, WorkbenchPresentationError, WorkbenchPresentationPort,
    WorkbenchRuntimeIntent, WorkbenchSnapshot,
};

const MAIN_WINDOW: &str = "main";
const SEARCH_QUERY_LIMIT: usize = 120;
/// Single channel the disposable workbench listens on for sequenced orchestrator changes.
const CHANGE_EVENT: &str = "ghostlight://change";

struct DesktopState {
    workbench: WorkbenchFacade,
}

struct NativePresentation {
    app: AppHandle,
}

impl WorkbenchPresentationPort for NativePresentation {
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

/// Start the orchestrator and its disposable desktop workbench in one process.
pub fn run() -> Result<()> {
    let host = ServiceHost::start(&ghostlight_bridge::runtime::runtime_file())?;
    eprintln!(
        "Ghostlight 1.0 ready on local ports {} and {}",
        host.endpoint.service_port, host.endpoint.browser_port
    );
    let workbench = host.workbench.clone();
    let setup_workbench = workbench.clone();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(DesktopState { workbench })
        .invoke_handler(tauri::generate_handler![
            workbench_snapshot,
            workbench_search,
            apply_runtime_intent,
            refresh_harnesses,
            manage_harness,
            test_notification,
            quit_ghostlight
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            setup_workbench.attach_presentation(Arc::new(NativePresentation {
                app: app.handle().clone(),
            }));
            setup_workbench.attach_events(Arc::new(NativeEvents {
                app: app.handle().clone(),
            }));
            if let Err(error) = build_tray(app) {
                eprintln!("Ghostlight tray is unavailable: {error}");
            }
            if std::env::args_os().any(|argument| argument == "--show") {
                show_workbench(app.handle());
            }
            Ok(())
        });

    let app = match catch_unwind(AssertUnwindSafe(|| {
        builder.build(tauri::generate_context!())
    })) {
        Ok(Ok(app)) => app,
        Ok(Err(error)) => {
            eprintln!("Ghostlight workbench is unavailable; continuing headless: {error}");
            host.wait();
            return Ok(());
        }
        Err(_) => {
            eprintln!("Ghostlight workbench failed during startup; continuing headless");
            host.wait();
            return Ok(());
        }
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| app.run_return(|_, _| {})));
    match outcome {
        Ok(0) => Ok(()),
        Ok(code) => {
            eprintln!("Ghostlight workbench exited with status {code}; continuing headless");
            host.wait();
            Ok(())
        }
        Err(_) => {
            eprintln!("Ghostlight workbench stopped unexpectedly; continuing headless");
            host.wait();
            Ok(())
        }
    }
}

fn build_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Ghostlight", true, None::<&str>)?;
    let hold = MenuItem::with_id(app, "hold", "Pause browser work", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Resume browser work", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Ghostlight", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &hold, &resume, &quit])?;
    TrayIconBuilder::with_id("ghostlight")
        .icon(tauri::include_image!("../../extension/icons/icon32.png"))
        .tooltip("Ghostlight")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_workbench(app),
            "hold" => apply_tray_intent(app, WorkbenchRuntimeIntent::Hold),
            "resume" => apply_tray_intent(app, WorkbenchRuntimeIntent::Resume),
            "quit" => app.exit(0),
            _ => {}
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
                show_workbench(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn apply_tray_intent(app: &AppHandle, intent: WorkbenchRuntimeIntent) {
    let state = app.state::<DesktopState>();
    let _ = state.workbench.apply_runtime_intent(intent);
}

fn show_workbench(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        show_window(&window);
    }
}

fn show_window(window: &WebviewWindow) {
    if let Err(error) = window.unminimize() {
        eprintln!("Ghostlight workbench could not restore its window: {error}");
    }
    if let Err(error) = window.show() {
        eprintln!("Ghostlight workbench could not show its window: {error}");
    }
    if let Err(error) = window.set_focus() {
        eprintln!("Ghostlight workbench could not focus its window: {error}");
    }
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
fn test_notification(state: State<'_, DesktopState>) -> Result<(), String> {
    state
        .workbench
        .test_notification()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn quit_ghostlight(app: AppHandle) {
    app.exit(0);
}

fn validate_search_query(query: &str) -> Result<(), String> {
    if query.chars().count() > SEARCH_QUERY_LIMIT {
        return Err(format!(
            "Search is limited to {SEARCH_QUERY_LIMIT} characters."
        ));
    }
    if query
        .chars()
        .any(|character| character.is_control() && !character.is_whitespace())
    {
        return Err("Search contains an unsupported control character.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_search_query;

    #[test]
    fn search_input_is_bounded_at_the_adapter() {
        assert!(validate_search_query("blocked browser").is_ok());
        assert!(validate_search_query(&"x".repeat(121)).is_err());
        assert!(validate_search_query("blocked\u{0007}").is_err());
    }

    #[test]
    fn workbench_uses_the_original_ghostlight_artwork_byte_for_byte() {
        assert_eq!(
            include_bytes!("../../ui/ghostlight.png"),
            include_bytes!("../../../../extension/icons/icon128.png")
        );
    }

    #[test]
    fn workbench_content_remains_scrollable_in_a_short_window() {
        fn declarations<'a>(styles: &'a str, selector: &str) -> &'a str {
            let marker = format!("{selector} {{");
            styles
                .split_once(&marker)
                .and_then(|(_, rest)| rest.split_once('}'))
                .map(|(rule, _)| rule)
                .unwrap_or_else(|| panic!("missing {selector} style rule"))
        }

        let styles = include_str!("../../ui/styles.css");
        let shell = declarations(styles, ".app-shell");
        assert!(shell.contains("min-height: 0"));
        assert!(shell.contains("overflow: hidden"));
        let content = declarations(styles, "#main-content");
        assert!(content.contains("min-height: 0"));
        assert!(content.contains("overflow-y: auto"));
    }

    #[test]
    fn the_workbench_listens_for_changes_without_gaining_the_right_to_emit_them() {
        let capability = include_str!("../../capabilities/main.json");
        assert!(capability.contains("core:event:allow-listen"));
        assert!(!capability.contains("core:event:allow-emit"));
    }

    #[test]
    fn the_surface_uses_the_published_ghostlight_palette() {
        let styles = include_str!("../../ui/styles.css");
        // Ghostlight's accent and the night-garden ground, as published on sylin.org.
        for value in ["#5eead4", "94, 234, 212", "#0f0e12"] {
            assert!(
                styles.contains(value),
                "the workbench dropped the published palette value {value}"
            );
        }
        // The site standard: no rule hard-codes the hue, so the accent stays swappable.
        assert_eq!(
            styles.matches("#5eead4").count(),
            2,
            "the accent belongs to --a and --al only; rules must use var(--a) or var(--argb)"
        );
    }

    #[test]
    fn the_surface_and_the_page_share_one_motion_curve() {
        let styles = include_str!("../../ui/styles.css");
        let renderer = include_str!("../../../../extension/lib/presentation.js");
        assert!(styles.contains("cubic-bezier(.22, 1, .36, 1)"));
        assert!(renderer.contains("cubic-bezier(.22,1,.36,1)"));
    }

    #[test]
    fn the_surface_handles_every_change_the_orchestrator_can_publish() {
        use ghostlight_bridge::browser::RuntimeControlState;

        use crate::governance::Capability;
        use crate::workbench::{HistoryItem, OperationPhase, OperationSummary, WorkbenchChange};

        let app = include_str!("../../ui/app.js");
        let operation = OperationSummary {
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            tool: "browser_read_page".into(),
            activity: "Reading page".into(),
            capability: Capability::Read,
            started_at_ms: Some(0),
            phase: OperationPhase::Running,
        };
        let record = HistoryItem {
            timestamp_ms: 0,
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            tool: "browser_read_page".into(),
            capability: "read".into(),
            allowed: true,
            reason: "permitted".into(),
            status: "succeeded".into(),
            effect: "none".into(),
        };

        for change in [
            WorkbenchChange::OperationStarted {
                operation: operation.clone(),
            },
            WorkbenchChange::OperationChanged { operation },
            WorkbenchChange::OperationSettled { record },
            WorkbenchChange::RuntimeChanged {
                runtime_state: RuntimeControlState::Active,
            },
        ] {
            let encoded = serde_json::to_value(&change).expect("changes serialize");
            let kind = encoded["kind"].as_str().expect("every change is tagged");
            assert!(
                app.contains(&format!("case \"{kind}\":")),
                "the workbench does not handle the {kind} change"
            );
        }
    }

    #[test]
    fn every_runtime_intent_stays_reachable_from_the_surface() {
        use crate::workbench::WorkbenchRuntimeIntent;

        let surface = concat!(
            include_str!("../../ui/index.html"),
            include_str!("../../ui/app.js")
        );
        // This match is exhaustive on purpose: a new runtime intent must not compile until
        // someone decides where the workbench offers it.
        let names = [
            WorkbenchRuntimeIntent::Hold,
            WorkbenchRuntimeIntent::Resume,
            WorkbenchRuntimeIntent::EndSession,
            WorkbenchRuntimeIntent::StartSession,
        ]
        .map(|intent| match intent {
            WorkbenchRuntimeIntent::Hold => "hold",
            WorkbenchRuntimeIntent::Resume => "resume",
            WorkbenchRuntimeIntent::EndSession => "end_session",
            WorkbenchRuntimeIntent::StartSession => "start_session",
        });

        for name in names {
            serde_json::from_value::<WorkbenchRuntimeIntent>(serde_json::Value::String(
                name.into(),
            ))
            .unwrap_or_else(|_| panic!("the adapter no longer accepts the {name} intent"));
            assert!(
                surface.contains(&format!("\"{name}\"")),
                "the workbench can no longer request the {name} intent"
            );
        }
    }

    #[test]
    fn every_capability_class_the_surface_can_receive_has_a_visual_treatment() {
        use crate::governance::Capability;

        let app = include_str!("../../ui/app.js");
        let styles = include_str!("../../ui/styles.css");
        for capability in [
            Capability::Read,
            Capability::Action,
            Capability::Write,
            Capability::Execute,
        ] {
            let name = serde_json::to_value(capability).expect("capabilities serialize");
            let name = name.as_str().expect("capabilities encode as strings");
            assert!(
                app.contains(&format!("{name}: \"cap-{name}\"")),
                "the workbench cannot classify the {name} capability"
            );
            assert!(
                styles.contains(&format!(".cap-{name} {{")),
                "the {name} capability has no visual treatment"
            );
        }
    }
}
