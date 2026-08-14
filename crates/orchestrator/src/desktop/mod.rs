//! Thin Tauri 2 adapter for Ghostlight's orchestrator-owned workbench facade.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use anyhow::Result;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

use crate::install::{HarnessAction, HarnessActionResult, HarnessSummary};
use crate::service::ServiceHost;
use crate::workbench::{
    SearchHit, WorkbenchDestination, WorkbenchEvent, WorkbenchEventSink, WorkbenchFacade,
    WorkbenchIntentResult, WorkbenchNotification, WorkbenchPresentationError,
    WorkbenchPresentationPort, WorkbenchRuntimeIntent, WorkbenchSnapshot,
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

/// Start the orchestrator and its initially minimized desktop workbench in one process.
pub fn run() -> Result<()> {
    match crate::install::native_host::NativeHostRegistry::discover().reconcile_packaged_launch() {
        Ok(Some(result)) => {
            if result.changed {
                eprintln!("Ghostlight updated the packaged browser connector registration");
            }
            let migration = crate::install::migration::retire_obsolete_supervisor();
            for warning in migration.warnings {
                eprintln!("Ghostlight package migration warning: {warning}");
            }
        }
        Ok(None) => {}
        Err(error) => {
            eprintln!("Ghostlight could not reconcile the packaged browser connector: {error}");
        }
    }
    let host = ServiceHost::start(&ghostlight_bridge::runtime::runtime_file())?;
    eprintln!(
        "Ghostlight 1.0 ready on local ports {} and {}",
        host.endpoint.service_port, host.endpoint.browser_port
    );
    let workbench = host.workbench.clone();
    let setup_workbench = workbench.clone();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        // Registered in Rust only. The capability file grants the webview no opener permission,
        // so the surface cannot reach this except through the closed command below.
        .plugin(tauri_plugin_opener::init())
        .manage(DesktopState { workbench })
        .invoke_handler(tauri::generate_handler![
            workbench_snapshot,
            workbench_search,
            apply_runtime_intent,
            refresh_harnesses,
            manage_harness,
            test_notification,
            open_destination,
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
            minimize_workbench(app.handle())?;
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
            "open" => reveal_from_tray(app),
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
    let _ = state.workbench.apply_runtime_intent(intent);
}

fn show_workbench(app: &AppHandle) -> Result<(), WorkbenchPresentationError> {
    let window = app.get_webview_window(MAIN_WINDOW).ok_or_else(|| {
        WorkbenchPresentationError::Native("Ghostlight workbench window is unavailable".into())
    })?;
    show_window(&window)
}

fn minimize_workbench(app: &AppHandle) -> Result<(), WorkbenchPresentationError> {
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

fn show_window(window: &WebviewWindow) -> Result<(), WorkbenchPresentationError> {
    window
        .show()
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))?;
    // Some Linux window managers reject unminimize for a hidden window even after show has made
    // it visible. The reveal still succeeds there, so de-minimization is a best-effort refinement.
    let _ = window.unminimize();
    window
        .set_focus()
        .map_err(|error| WorkbenchPresentationError::Native(error.to_string()))?;
    Ok(())
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

/// Open one of the destinations Ghostlight is willing to point at.
///
/// The surface sends a name from a closed vocabulary, never an address, so this cannot be talked
/// into opening something the product did not choose.
#[tauri::command]
fn open_destination(destination: WorkbenchDestination, app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(destination.url(), None::<&str>)
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
    fn clearing_the_monitor_is_disposable_view_state_not_an_audit_mutation() {
        let html = include_str!("../../ui/index.html");
        let app = &surface_source();
        let desktop = include_str!("mod.rs");

        assert!(html.contains("id=\"clear-monitor\""));
        assert!(html.contains("Clear view"));
        assert!(app.contains("hidden: new Set()"));
        assert!(app.contains("state.hidden.add(entry.invocation)"));
        assert!(app.contains("state.feed = state.feed.filter(isRunning)"));
        assert!(app.contains("state.hidden.has(record.invocation)"));
        assert!(app.contains("Audit history is unchanged."));
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
    fn the_surface_handles_every_change_the_orchestrator_can_publish() {
        use ghostlight_bridge::browser::RuntimeControlState;

        use crate::governance::Capability;
        use crate::workbench::{HistoryItem, OperationPhase, OperationSummary, WorkbenchChange};

        let app = &surface_source();
        let operation = OperationSummary {
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            tool: "browser_read".into(),
            activity: "Reading page".into(),
            capability: Capability::Read,
            started_at_ms: Some(0),
            phase: OperationPhase::Running,
        };
        let record = HistoryItem {
            timestamp_ms: 0,
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            tool: "browser_read".into(),
            capability: "read".into(),
            allowed: true,
            reason: "permitted".into(),
            status: "succeeded".into(),
            effect: "none".into(),
            summary: "Read 1,240 words from example.com.".into(),
            duration_ms: 1200,
            observed: sample_observation(),
            channel: Some(ghostlight_bridge::service::IntakeChannel::Mcp),
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

    /// One observation with every field populated, so a guard sees the whole vocabulary.
    fn sample_observation() -> crate::language::outcome::Observed {
        crate::language::outcome::Observed {
            host: Some("example.com".into()),
            readiness: Some("complete".into()),
            count: Some(1240),
            width: Some(1280),
            height: Some(720),
        }
    }

    #[test]
    fn the_band_latches_working_instead_of_tracking_each_operation() {
        let app = &surface_source();
        // Most calls settle in well under a second. A word tied to them strobes, so it latches
        // and every new action pushes the deadline back.
        assert!(app.contains("const WORKING_LATCH_MS = 10_000;"));
        assert!(
            app.contains("now() - state.interactionAt < WORKING_LATCH_MS"),
            "the band no longer latches on recent interaction"
        );
        assert!(
            !app.contains(r#"state.feed.some(isRunning) ? "runtime-working""#),
            "the band still reads its word straight off the live operations"
        );
        // Nothing else wakes the band once the last operation settles, so the latch has to
        // schedule its own expiry or the word stays lit until an unrelated repaint.
        assert!(
            app.contains("state.latchTimer = setTimer(() => emit(CHANGE.Band), WORKING_LATCH_MS"),
            "the latch never expires on its own"
        );
        // The negative control: every operation event still refreshes it, so the latch is fed by
        // real work rather than by one arbitrary moment at startup.
        for event in [
            "case \"operation_started\": touch();",
            "case \"operation_changed\": touch();",
            "case \"operation_settled\": touch();",
        ] {
            assert!(
                app.contains(event),
                "an operation event does not refresh the latch: {event}"
            );
        }
    }

    #[test]
    fn about_wears_the_published_card_and_the_products_own_artwork() {
        let markup = include_str!("../../ui/index.html");
        let styles = include_str!("../../ui/styles.css");

        assert!(
            markup.contains(r#"data-view="about""#),
            "About is not reachable from the band"
        );
        assert!(markup.contains(r#"id="view-about""#), "About has no view");

        // The card is the sylin.org card: same anatomy, same holo layers.
        for part in [
            "card-art",
            "card-divider",
            "card-disc",
            "card-notch",
            "card-pane",
            "card-title",
            "card-rules",
            "card-foot",
            "holo2",
        ] {
            assert!(markup.contains(part), "the About card is missing {part}");
        }
        assert!(
            styles.contains("mix-blend-mode: color-dodge"),
            "the foil layer lost its blend"
        );
        // The surface already owns .card for its diagnostic panels. The ported card keeps its own
        // root so restyling one never restyles the other.
        assert!(
            markup.contains(r#"<button class="tcg" id="about-card""#),
            "the About card is not scoped away from the diagnostic cards"
        );
        assert!(
            styles.contains(".card { padding: 16px;"),
            "the diagnostic card rule was overwritten by the About card"
        );

        // One artwork. The card wears the same bytes the extension ships as its icon, so the
        // character cannot drift between the two places a person meets it.
        assert!(
            markup.contains(r#"class="px mascot" src="ghostlight.png""#),
            "the card does not wear the product's own artwork"
        );
        // The band no longer does: a 100px sprite resampled to 30px read as a smudge.
        assert!(
            !markup.contains("lamp-ghost"),
            "the crunched mascot is still in the band"
        );
        assert!(markup.contains("lamp-core"), "the band lost its lamp");
    }

    /// Every script the window loads, in the order the page loads it.
    ///
    /// A guard should care what the surface does, not which module a line happens to live in.
    /// Reading one file made every assertion quietly depend on the file layout, so splitting the
    /// surface broke guards that were still perfectly true.
    fn surface_source() -> String {
        [
            include_str!("../../ui/lib/words.js"),
            include_str!("../../ui/lib/entries.js"),
            include_str!("../../ui/lib/transport.js"),
            include_str!("../../ui/lib/store.js"),
            include_str!("../../ui/lib/view.js"),
            include_str!("../../ui/app.js"),
        ]
        .join(
            "
",
        )
    }

    #[test]
    fn the_pure_layers_of_the_surface_hold_no_state_and_no_document() {
        let words = include_str!("../../ui/lib/words.js");
        let entries = include_str!("../../ui/lib/entries.js");
        let store = include_str!("../../ui/lib/store.js");
        let transport = include_str!("../../ui/lib/transport.js");
        let markup = include_str!("../../ui/index.html");

        // These two exist to be readable and testable without a browser. The moment either
        // reaches for the document or keeps state of its own, that stops being true and the
        // seam has quietly moved back.
        // The store keeps the cache and the transport talks to the orchestrator. Neither may
        // touch the document: that is what stops a rendering fault from corrupting what the
        // window believes, and what lets both be exercised with no browser present.
        for (name, module) in [
            ("words", words),
            ("entries", entries),
            ("store", store),
            ("transport", transport),
        ] {
            for forbidden in ["document.", "window.", "el["] {
                assert!(
                    !module.contains(forbidden),
                    "{name} reaches for {forbidden}, so it is no longer a pure layer"
                );
            }
        }

        // The negative control: the composition root is where the document belongs, so a rule
        // that held everywhere would be measuring nothing.
        let view = include_str!("../../ui/lib/view.js");
        assert!(
            view.contains("document.querySelectorAll"),
            "the view is where the document belongs, so a rule that held everywhere would be              measuring nothing"
        );

        // Load order is the page's, and the surface test reads it from here rather than
        // repeating it, so both move together.
        let order: Vec<&str> = markup
            .match_indices("<script src=\"")
            .filter_map(|(at, _)| {
                let rest = &markup[at + 13..];
                rest.find('"').map(|end| &rest[..end])
            })
            .collect();
        assert_eq!(
            order,
            vec![
                "lib/words.js",
                "lib/entries.js",
                "lib/transport.js",
                "lib/store.js",
                "lib/view.js",
                "app.js"
            ],
            "the composition root must load after everything it composes"
        );
    }

    #[test]
    fn every_element_the_surface_reaches_for_exists_in_the_markup() {
        let app = &surface_source();
        let markup = include_str!("../../ui/index.html");

        let ids: Vec<&str> = markup
            .match_indices("id=\"")
            .filter_map(|(at, _)| {
                let rest = &markup[at + 4..];
                rest.find('"').map(|end| &rest[..end])
            })
            .collect();
        // The negative control: a scrape that finds nothing would let every assertion below pass
        // while proving nothing at all.
        assert!(ids.len() > 20, "the id scrape found only {} ids", ids.len());

        let mut referenced = Vec::new();
        for (at, _) in app.match_indices("el[\"") {
            let rest = &app[at + 4..];
            if let Some(end) = rest.find('"') {
                referenced.push(&rest[..end]);
            }
        }
        assert!(
            referenced.len() > 10,
            "the reference scrape found only {} lookups",
            referenced.len()
        );

        // A node the surface reaches for but the markup never defines reads as undefined and
        // throws on first use. One of those took the whole window down: the boot sequence
        // abandoned the snapshot, the change subscription, and the heartbeat behind it.
        for name in referenced {
            assert!(
                ids.contains(&name),
                "the surface reaches for #{name}, which the markup does not define"
            );
        }
    }

    #[test]
    fn the_live_surface_boots_before_anything_decorative() {
        let app = include_str!("../../ui/app.js");
        let boot = app
            .split_once("function boot() {")
            .expect("the surface has no boot sequence")
            .1
            .split_once(
                "
}",
            )
            .expect("the boot sequence never closes")
            .0;
        let heartbeat = boot
            .find("HEARTBEAT_MS")
            .expect("boot never starts the heartbeat");
        let subscribe = boot.find("subscribe(").expect("boot never subscribes");
        let resync = boot
            .find("resync({ rebuildFeed: true })")
            .expect("boot never resyncs");
        let decoration = boot
            .find("armCard")
            .expect("boot never arms the About card");

        // The heartbeat is the surface's own recovery. Installed first, a bad subscription or a
        // bad first snapshot costs one cycle; installed last, it costs the window.
        assert!(
            heartbeat < subscribe && heartbeat < resync,
            "recovery is installed after the things it exists to recover from"
        );
        assert!(
            resync < decoration,
            "a decorative step runs before the window is connected to the truth"
        );
        // Every fallible boot step is isolated, or one of them takes the rest with it.
        for step in ["subscribing to changes", "first snapshot", "about card"] {
            assert!(
                boot.contains(&format!("attempt(\"{step}\"")),
                "the {step} step is not isolated"
            );
        }
    }

    #[test]
    fn a_surface_that_cannot_draw_is_not_reported_as_a_lost_connection() {
        let app = &surface_source();
        // One catch around both the fetch and the render said "Not connected" for either, which
        // sends whoever is reading it to look at the orchestrator when the fault is in here.
        assert!(
            app.contains(r#"attempt("rendering the snapshot", () =>"#),
            "a render failure is still indistinguishable from a lost connection"
        );
        // Every wiring statement was at module scope ahead of boot, where one failure to attach
        // took the snapshot and the heartbeat with it.
        assert!(app.contains("function wire() {"));
        assert!(
            !app.contains("document.getElementById(\"refresh-status\")"),
            "a listener still reaches around the derived table at module scope"
        );
    }

    #[test]
    fn a_failed_paint_is_retried_rather_than_remembered_as_done() {
        let app = &surface_source();
        // The signature must be recorded only after the paint succeeded. Recording first, with a
        // memo that is never cleared, remembers a panel that threw as finished and leaves it
        // blank for the life of the window.
        assert!(
            app.contains("if (attempt(`painting ${key}`, paint)) painted[key] = signature;"),
            "a failed paint is still memoised as a completed one"
        );
        // And nothing may fail where only a console would notice.
        assert!(app.contains(r#"window.addEventListener("error""#));
        assert!(app.contains(r#"window.addEventListener("unhandledrejection""#));
        assert!(
            app.contains("if (view?.el.toast) view.toast(detail, true);"),
            "failures never reach the person using the window"
        );
    }

    #[test]
    fn the_about_page_can_reach_only_the_destinations_the_product_chose() {
        use crate::workbench::WorkbenchDestination;

        let markup = include_str!("../../ui/index.html");
        let app = &surface_source();

        // The surface holds no addresses at all. Without this, "closed vocabulary" would be a
        // convention rather than a property, and one hand-written anchor would quietly undo it.
        assert!(
            !markup.contains("href=\"http"),
            "the About markup still carries a raw URL"
        );
        assert!(
            !app.contains("https://"),
            "the surface script still carries a raw URL"
        );
        assert!(app.contains(r#"call("open_destination", { destination })"#));

        // Every destination the orchestrator will open is offered, and every name the surface
        // offers is one the orchestrator knows. Either half alone lets the two drift.
        for destination in WorkbenchDestination::all() {
            let key = destination.key();
            assert!(
                app.contains(&format!("[\"{key}\", \""))
                    || markup.contains(&format!(r#"data-destination="{key}""#)),
                "{key} is a destination nobody can reach"
            );
            let url = destination.url();
            assert!(
                url.starts_with("https://sylin.org/")
                    || url.starts_with("https://github.com/sylin-org/"),
                "{key} points outside the product's own surfaces: {url}"
            );
        }

        // Documentation deliberately points at dev: main still carries the 0.8 line, so a main
        // link would answer questions about a product this window is not running.
        for destination in WorkbenchDestination::all() {
            let url = destination.url();
            assert!(
                !url.contains("/blob/main/") && !url.contains("/tree/main/"),
                "{} points at main, which is still the 0.8 line: {url}",
                destination.key()
            );
        }
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
    fn the_card_mascot_renders_at_a_whole_multiple_of_its_sprite() {
        // The expectation is read off the artwork, not written down beside it. Swap the sprite
        // for one of a different size and this fails until the card is resized to match.
        let sprite = include_bytes!("../../ui/ghostlight.png");
        assert_eq!(&sprite[1..4], b"PNG", "the mascot is not a PNG");
        let native = u32::from_be_bytes([sprite[16], sprite[17], sprite[18], sprite[19]]);
        let tall = u32::from_be_bytes([sprite[20], sprite[21], sprite[22], sprite[23]]);
        assert_eq!(native, tall, "a square sprite is assumed by the card art");

        let styles = include_str!("../../ui/styles.css");
        let rule = styles
            .split_once(".tcg .card-art .mascot {")
            .expect("the card has no mascot rule")
            .1
            .split_once('}')
            .expect("the mascot rule never closes")
            .0;
        let rendered: u32 = rule
            .split_once("width:")
            .expect("the mascot has no width")
            .1
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .expect("the mascot width is not a plain pixel count");

        assert!(
            rendered >= native,
            "the sprite is rendered below its native size"
        );
        assert_eq!(
            rendered % native,
            0,
            "the mascot renders at {rendered}px from a {native}px sprite, so source pixels split              unevenly across screen pixels and the art aliases however it is filtered"
        );
        assert!(
            styles.contains(".tcg img.px { image-rendering: pixelated; }"),
            "the sprite is being smoothed"
        );
        // The negative control: the echo behind the sprite is deliberately blurred and scaled off
        // the grid, so "everything must be pixel-exact" would be the wrong rule to write here.
        assert!(
            styles.contains("filter: blur(7px) saturate(1.3)"),
            "the blurred echo lost its treatment"
        );
    }

    #[test]
    fn the_connections_bar_groups_by_client_rather_than_by_connection() {
        let app = &surface_source();
        // One chip per client. A client that opens a session per request otherwise fills the bar
        // with identical names that tell the user nothing.
        assert!(
            app.contains("function connectionGroups(sessions)"),
            "the connections bar does not group its sessions"
        );
        assert!(
            app.contains("connectionGroups(snapshot.sessions).map"),
            "the connections bar is not painted from the grouped sessions"
        );
        assert!(
            !app.contains("snapshot.sessions.map(session =>"),
            "the connections bar still paints one chip per connection"
        );
        // The negative control: grouping is a rendering choice for one bar, not a change to the
        // data. History attribution still resolves a single workspace to the client that owns it,
        // and that lookup breaks the moment someone collapses the sessions array itself.
        assert!(
            app.contains("state.snapshot?.sessions.find((item) => item.id === workspace)"),
            "per-session attribution was lost when the bar was grouped"
        );
    }

    #[test]
    fn surface_renders_seam_facts_and_trusts_outcome_language_for_measurements() {
        let app = &surface_source();
        // Readiness is the only observed fact no sentence states, so it is the only one the
        // surface must read structurally. The host is guarded where it is collected instead: an
        // assertion that the surface renders it separately would only pin a second rendering of
        // what the sentence already says.
        assert!(
            app.contains("observed.readiness"),
            "the workbench never renders seam-owned readiness evidence"
        );
        assert!(app.contains("const body = sentence(entry);"));
        assert!(
            !app.contains("measured("),
            "the surface is still guessing which outcome register to render"
        );
        assert!(
            !app.contains("observed.host"),
            "the hero says the host twice: the sentence already names it"
        );
    }

    #[test]
    fn every_observed_fact_is_documented_where_it_is_collected() {
        // The audit record's consumer is a person configuring a collector, so the guide is where
        // the vocabulary has to stay honest. A new field that nobody documents fails here.
        let guide = include_str!("../../../../docs/guides/siem-integration.md");
        let encoded = serde_json::to_value(sample_observation()).expect("observations serialize");
        let fields = encoded.as_object().expect("an observation is an object");
        assert_eq!(fields.len(), 5, "the observation vocabulary changed");
        for field in fields.keys() {
            assert!(
                guide.contains(&format!("`{field}`")),
                "siem-integration.md documents no {field} field, so collectors cannot expect it"
            );
        }
    }

    #[test]
    fn every_row_cell_has_a_grid_track_at_every_width() {
        // A row is a CSS grid, so adding a cell without adding a track silently shifts every
        // column after it. Both numbers are derived here rather than pinned, and the narrow
        // layouts are checked against what they hide.
        let app = &surface_source();
        let styles = include_str!("../../ui/styles.css");

        let markup = app
            .split_once("function rowMarkup(entry) {")
            .and_then(|(_, rest)| {
                rest.split_once(
                    "
    }",
                )
            })
            .map(|(body, _)| body)
            .expect("the surface still builds rows");
        let cells = markup.matches("<div class=\"").count();
        assert!(cells >= 6, "expected a row of cells, saw {cells}");

        fn tracks(block: &str) -> usize {
            let value = block
                .split_once("grid-template-columns:")
                .and_then(|(_, rest)| rest.split_once(';'))
                .map(|(value, _)| value)
                .expect("a row grid declares its columns");
            // minmax(0, 1fr) is one track that contains a space; collapse it before counting.
            let mut collapsed = String::new();
            let mut depth = 0_usize;
            for character in value.chars() {
                match character {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    c if c.is_whitespace() && depth > 0 => continue,
                    _ => {}
                }
                collapsed.push(character);
            }
            collapsed.split_whitespace().count()
        }

        // Narrower media queries stack on wider ones, so what a width hides is everything hidden
        // up to and including its own section.
        let mut hidden = 0_usize;
        for (index, section) in styles.split("@media").enumerate() {
            hidden += section
                .lines()
                .filter(|line| line.contains("display: none"))
                .flat_map(|line| line.split(','))
                .filter(|selector| selector.trim_start().starts_with(".row-"))
                .count();
            let Some((_, rest)) = section.split_once(".row {") else {
                continue;
            };
            let Some((block, _)) = rest.split_once('}') else {
                continue;
            };
            if !block.contains("grid-template-columns") {
                continue;
            }
            assert_eq!(
                tracks(block) + hidden,
                cells,
                "row layout {index} declares {} tracks and hides {hidden} of {cells} cells",
                tracks(block)
            );
        }
    }

    #[test]
    fn every_readiness_the_surface_can_receive_has_a_note() {
        use crate::work::result::Readiness;

        let app = &surface_source();
        // Read the table itself rather than the whole file: several of these words appear in the
        // effect story too, and a guard that matches anywhere would pass without the table.
        let notes = app
            .split_once("const READINESS_NOTE = {")
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(block, _)| block)
            .expect("the surface keeps a readiness note table");
        // Exhaustive on purpose: a new readiness must not compile until the surface decides what
        // it says about a settled row.
        for readiness in [
            Readiness::NotApplicable,
            Readiness::Loading,
            Readiness::Interactive,
            Readiness::Complete,
            Readiness::Unknown,
        ] {
            let name = serde_json::to_value(readiness).expect("readiness serializes");
            let name = name.as_str().expect("readiness encodes as a string");
            assert!(
                notes.contains(&format!("{name}:")),
                "the surface has no note for {name} readiness"
            );
        }
    }

    #[test]
    fn every_catalog_tool_has_a_medallion() {
        let app = &surface_source();
        let tools = crate::language::catalog();
        assert_eq!(tools.len(), 22, "expected the complete catalog");
        for tool in tools {
            assert!(
                app.contains(&format!("{}: \"", tool.name)),
                "{} has no medallion, so its row would fall back to a generic glyph",
                tool.name
            );
        }
    }

    #[test]
    fn every_runtime_intent_stays_reachable_from_the_surface() {
        use crate::workbench::WorkbenchRuntimeIntent;

        let surface = format!(
            "{}{}",
            include_str!("../../ui/index.html"),
            surface_source()
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

        let app = &surface_source();
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
