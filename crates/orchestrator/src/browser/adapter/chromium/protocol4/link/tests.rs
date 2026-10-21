//! The Link and the browser-native commands against a scripted protocol-4 extension.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::fake::{FakeExtension, Reply, Script};
use super::super::scope::PlannedCall;
use super::super::{wire, world};
use super::{Answers, CallRequest, Line, Link};
use crate::browser::adapter::{
    BrowserCommand, BrowserOutcome, BrowserReadiness, ClickShape, PhysicalTab,
    PresentationActivity, PresentationKind, PresentationSignal, RecordingDestination,
};
use crate::browser::{BrowserDispatch, BrowserError, BrowserPort, RelayBrowserPort};
use crate::governance::ReasonCode;
use crate::language::outcome::PrimitiveCause;
use crate::language::HistoryDirection;
use crate::sync::lock;
use crate::workspace::StaleReason;

const FOUR: &str = "browser_four";
const GROUP_TITLE: &str = "\u{1F47B} Claude Code";
const PATIENCE: Duration = Duration::from_secs(30);

fn port(script: Script) -> (RelayBrowserPort, FakeExtension) {
    let port = RelayBrowserPort::new(
        "service_link".into(),
        Arc::new(wire::Assets {
            id: "0".repeat(64),
            source: String::new(),
        }),
    );
    let (service, extension) = ghostlight_relay::endpoint::pair().unwrap();
    let fake = FakeExtension::connect(extension, &FakeExtension::hello(FOUR), script);
    port.attach(service).unwrap();
    (port, fake)
}

/// A script that answers every request with the receipt `next` holds, re-addressed.
fn scripted(next: &Arc<Mutex<wire::Receipt>>) -> Script {
    let next = Arc::clone(next);
    Box::new(move |correlation, _| {
        Reply::Now(wire::Receipt {
            correlation: correlation.into(),
            ..next.lock().unwrap().clone()
        })
    })
}

fn call(port: &RelayBrowserPort, command: BrowserCommand) -> Result<BrowserOutcome, BrowserError> {
    let cancelled = AtomicBool::new(false);
    port.call(FOUR, command, Instant::now() + PATIENCE, &cancelled)
}

fn tab(url: &str) -> Value {
    json!({"tab_id": 12, "title": "Example", "url": url, "active": true, "readiness": "complete"})
}

fn physical_tab(url: &str) -> PhysicalTab {
    PhysicalTab {
        tab_id: 12,
        title: "Example".into(),
        url: url.into(),
        active: true,
        readiness: BrowserReadiness::Complete,
    }
}

fn line(port: &RelayBrowserPort) -> Line {
    lock(&port.adapters).connections[FOUR].line.clone()
}

/// One browser-native row: the work, the primitive it must send, the receipt the extension
/// answers, and the outcome the service must report.
struct Row {
    command: BrowserCommand,
    primitive: Value,
    receipt: wire::Receipt,
    outcome: BrowserOutcome,
}

fn answer(ok: Value) -> wire::Receipt {
    FakeExtension::ok("", ok)
}

fn effect(ok: Value, committed: &[&str]) -> wire::Receipt {
    wire::Receipt {
        dispatched: true,
        committed_urls: committed.iter().map(|url| (*url).to_owned()).collect(),
        ..answer(ok)
    }
}

fn rows() -> Vec<Row> {
    let example = "https://example.com/";
    let navigated = |to: Value, discard: bool| {
        json!({"name": "navigate", "args": {"tab_id": 12, "to": to,
               "discard_beforeunload": discard, "ready_timeout_ms": 8000}})
    };
    let save = || BrowserCommand::ExportRecording {
        recording_id: "recording_1".into(),
        destination: RecordingDestination::Download {
            file_name: "flow.gif".into(),
        },
        max_output_bytes: 4096,
    };
    let saved = json!({"name": "recording", "args": {"op": "export", "recording_id": "recording_1",
        "destination": {"destination": "download", "file_name": "flow.gif"},
        "max_output_bytes": 4096}});
    let landed = || BrowserOutcome::Navigated {
        tab: physical_tab(example),
        committed_urls: vec![example.into()],
    };
    vec![
        Row {
            command: BrowserCommand::ListTabs,
            primitive: json!({"name": "list_tabs", "args": {}}),
            receipt: answer(json!({"tabs": [tab(example)]})),
            outcome: BrowserOutcome::Tabs {
                tabs: vec![physical_tab(example)],
            },
        },
        Row {
            command: BrowserCommand::FocusTab { tab_id: 12 },
            primitive: json!({"name": "focus_tab", "args": {"tab_id": 12}}),
            receipt: effect(
                json!({"tab_id": 12, "active": true, "window_focused": false}),
                &[],
            ),
            outcome: BrowserOutcome::TabFocused {
                tab_id: 12,
                active: true,
                window_focused: false,
            },
        },
        Row {
            command: BrowserCommand::OpenTab {
                url: example.into(),
                group_title: GROUP_TITLE.into(),
                focus_window: true,
            },
            primitive: json!({"name": "open_tab", "args": {"url": example,
                "group_key": "g_ac9879c3b8d67871", "group_title": GROUP_TITLE,
                "focus_window": true, "ready_timeout_ms": 8000}}),
            receipt: effect(json!({"tab": tab(example), "adopted": false}), &[example]),
            outcome: BrowserOutcome::TabOpened {
                tab: physical_tab(example),
                committed_urls: vec![example.into()],
            },
        },
        Row {
            command: BrowserCommand::AdoptTab {
                tab_id: 12,
                url: example.into(),
                group_title: GROUP_TITLE.into(),
                focus_window: false,
            },
            primitive: json!({"name": "open_tab", "args": {"url": example,
                "group_key": "g_ac9879c3b8d67871", "group_title": GROUP_TITLE,
                "adopt_tab_id": 12, "focus_window": false, "ready_timeout_ms": 8000}}),
            receipt: effect(json!({"tab": tab(example), "adopted": true}), &[example]),
            outcome: BrowserOutcome::TabOpened {
                tab: physical_tab(example),
                committed_urls: vec![example.into()],
            },
        },
        Row {
            command: BrowserCommand::Navigate {
                tab_id: 12,
                url: example.into(),
            },
            primitive: navigated(json!({"to": "url", "url": example}), false),
            receipt: effect(json!({"tab": tab(example)}), &[example, example]),
            outcome: landed(),
        },
        Row {
            command: BrowserCommand::TraverseHistory {
                tab_id: 12,
                direction: HistoryDirection::Back,
            },
            primitive: navigated(json!({"to": "back"}), false),
            receipt: effect(json!({"tab": tab(example)}), &[example]),
            outcome: landed(),
        },
        Row {
            command: BrowserCommand::TraverseHistory {
                tab_id: 12,
                direction: HistoryDirection::Forward,
            },
            primitive: navigated(json!({"to": "forward"}), false),
            receipt: effect(json!({"tab": tab(example)}), &[example]),
            outcome: landed(),
        },
        Row {
            command: BrowserCommand::Reload {
                tab_id: 12,
                bypass_cache: true,
            },
            primitive: navigated(json!({"to": "reload", "bypass_cache": true}), false),
            receipt: effect(json!({"tab": tab(example)}), &[example]),
            outcome: landed(),
        },
        Row {
            command: BrowserCommand::NavigateDiscardingBeforeUnload {
                tab_id: 12,
                url: example.into(),
            },
            primitive: navigated(json!({"to": "url", "url": example}), true),
            receipt: effect(json!({"tab": tab(example)}), &[example]),
            outcome: landed(),
        },
        Row {
            command: BrowserCommand::CloseTab {
                tab_id: 12,
                released: true,
            },
            primitive: json!({"name": "close_tab", "args": {"tab_id": 12, "released": true}}),
            receipt: effect(json!({"tab_id": 12}), &[]),
            outcome: BrowserOutcome::TabClosed { tab_id: 12 },
        },
        Row {
            command: BrowserCommand::SetZoom {
                tab_id: 12,
                zoom: 1.5,
            },
            primitive: json!({"name": "set_zoom", "args": {"tab_id": 12, "zoom": 1.5}}),
            receipt: effect(json!({"tab_id": 12, "zoom": 1.5}), &[]),
            outcome: BrowserOutcome::Zoomed {
                tab_id: 12,
                zoom: 1.5,
            },
        },
        Row {
            command: BrowserCommand::ResizeWindow {
                tab_id: 12,
                width: 1280,
                height: 800,
            },
            primitive: json!({"name": "resize_window",
                "args": {"tab_id": 12, "width": 1280, "height": 800}}),
            receipt: effect(
                json!({"tab_id": 12, "width": 1280, "height": 800, "affected_tab_ids": [12, 13]}),
                &[],
            ),
            outcome: BrowserOutcome::WindowResized {
                tab_id: 12,
                width: 1280,
                height: 800,
                affected_tab_ids: vec![12, 13],
            },
        },
        Row {
            command: BrowserCommand::InspectDialog { tab_id: 12 },
            primitive: json!({"name": "dialog", "args": {"tab_id": 12, "op": {"op": "status"}}}),
            receipt: answer(
                json!({"present": true, "dialog_type": "confirm", "message": "Leave?",
                    "url": "https://example.com/form"}),
            ),
            outcome: BrowserOutcome::Dialog {
                tab_id: 12,
                present: true,
                dialog_type: "confirm".into(),
                message: Some("Leave?".into()),
                default_prompt: None,
                url: Some("https://example.com/form".into()),
            },
        },
        Row {
            command: BrowserCommand::InspectDialog { tab_id: 12 },
            primitive: json!({"name": "dialog", "args": {"tab_id": 12, "op": {"op": "status"}}}),
            receipt: answer(
                json!({"present": true, "dialog_type": "prompt", "message": "Name?",
                    "default_prompt": "Ada", "url": "https://example.com/form"}),
            ),
            outcome: BrowserOutcome::Dialog {
                tab_id: 12,
                present: true,
                dialog_type: "prompt".into(),
                message: Some("Name?".into()),
                default_prompt: Some("Ada".into()),
                url: Some("https://example.com/form".into()),
            },
        },
        Row {
            command: BrowserCommand::HandleDialog {
                tab_id: 12,
                accept: true,
                text: Some("yes".into()),
            },
            primitive: json!({"name": "dialog",
                "args": {"tab_id": 12, "op": {"op": "handle", "accept": true, "text": "yes"}}}),
            receipt: effect(
                json!({"present": true, "dialog_type": "prompt", "handled": true, "accepted": true,
                    "url": "https://example.com/form"}),
                &[],
            ),
            outcome: BrowserOutcome::DialogHandled {
                tab_id: 12,
                dialog_type: "prompt".into(),
                accepted: true,
                url: Some("https://example.com/form".into()),
            },
        },
        Row {
            command: BrowserCommand::HandleDialog {
                tab_id: 12,
                accept: false,
                text: None,
            },
            primitive: json!({"name": "dialog",
                "args": {"tab_id": 12, "op": {"op": "handle", "accept": false, "text": null}}}),
            receipt: answer(json!({"present": false, "handled": false})),
            outcome: BrowserOutcome::DialogAbsent { tab_id: 12 },
        },
        Row {
            command: BrowserCommand::ClearDiagnostics {
                tab_ids: vec![12, 13],
            },
            primitive: json!({"name": "diagnostics",
                "args": {"op": "clear", "tab_ids": [12, 13]}}),
            receipt: answer(json!({"cleared_count": 4})),
            outcome: BrowserOutcome::DiagnosticsCleared { cleared_count: 4 },
        },
        Row {
            command: save(),
            primitive: saved.clone(),
            receipt: answer(
                json!({"result": "export_failed", "reason": wire::PAGE_REFUSED_REPLAY}),
            ),
            outcome: BrowserOutcome::RecordingExportFailed {
                reason: wire::PAGE_REFUSED_REPLAY.into(),
                page_refused: true,
            },
        },
        Row {
            command: save(),
            primitive: saved,
            receipt: answer(json!({"result": "export_failed", "reason": "encoder failed"})),
            outcome: BrowserOutcome::RecordingExportFailed {
                reason: "encoder failed".into(),
                page_refused: false,
            },
        },
    ]
}

/// Each browser-native command becomes the primitive the extension reads, and its receipt maps to
/// the outcome the executor reports, including an export the page refused told apart from one
/// the encoder could not make (commit 6734f99b).
#[test]
fn each_browser_native_row_sends_its_primitive_and_maps_its_outcome() {
    let next = Arc::new(Mutex::new(answer(Value::Null)));
    let (port, fake) = port(scripted(&next));
    for (index, row) in rows().into_iter().enumerate() {
        *next.lock().unwrap() = row.receipt;
        assert_eq!(
            call(&port, row.command),
            Ok(row.outcome),
            "row {index} maps its outcome"
        );
        let seen = fake.wait(|seen| seen.of_kind("request").len() == index + 1);
        assert_eq!(
            seen.of_kind("request")[index]["primitive"],
            row.primitive,
            "row {index} request"
        );
    }
}

/// Each closed receipt code becomes its typed error: before any effect a plain or local failure
/// the agent can act on, with a debugger refusal and a lost session keeping their own cause
/// (Decision 5.2 of ADR-0203), and after dispatch an unknown effect or a deadline after dispatch.
#[test]
fn receipt_failures_map_to_plain_or_local_failures_before_any_effect() {
    let next = Arc::new(Mutex::new(answer(Value::Null)));
    let (port, _fake) = port(scripted(&next));
    let navigate = || BrowserCommand::Navigate {
        tab_id: 99,
        url: "https://example.com/".into(),
    };
    for (failure, message, expected) in [
        (
            wire::Failure::TabMissing,
            Some("No tab with id: 99."),
            BrowserError::primitive("browser tab is missing"),
        ),
        (
            wire::Failure::DocumentChanged,
            None,
            BrowserError::DocumentUnavailable,
        ),
        (
            wire::Failure::Timeout,
            None,
            BrowserError::DeadlineBeforeDispatch,
        ),
        (
            wire::Failure::InvalidRequest,
            Some("tab_id is required"),
            BrowserError::Protocol("tab_id is required".into()),
        ),
        (
            wire::Failure::DebuggerUnavailable,
            Some("Cannot attach to this page."),
            BrowserError::Primitive {
                detail: "Cannot attach to this page.".into(),
                cause: PrimitiveCause::DebuggerUnavailable,
            },
        ),
        (
            wire::Failure::SessionLost,
            Some("Debugger is not attached to the tab with id: 99."),
            BrowserError::Primitive {
                detail: "Debugger is not attached to the tab with id: 99.".into(),
                cause: PrimitiveCause::SessionLost,
            },
        ),
        (
            wire::Failure::CaptureTooLarge,
            Some("No quality in the budget fits the size limit."),
            BrowserError::CaptureTooLarge {
                detail: "No quality in the budget fits the size limit.".into(),
            },
        ),
        (
            wire::Failure::BrowserRefused,
            Some("Zoom level is out of range."),
            BrowserError::primitive("Zoom level is out of range."),
        ),
        (wire::Failure::Other, None, BrowserError::primitive("other")),
        (
            wire::Failure::Unsupported,
            None,
            BrowserError::Unsupported {
                primitive: "navigate".into(),
            },
        ),
        (
            wire::Failure::NoHistoryEntry,
            Some("The tab has no earlier page in its history."),
            BrowserError::NoHistoryEntry,
        ),
    ] {
        *next.lock().unwrap() = wire::Receipt {
            message: message.map(str::to_owned),
            ..FakeExtension::failed("", failure)
        };
        let result = call(&port, navigate());
        assert_eq!(result, Err(expected.clone()), "{failure:?}");
        assert!(!expected.effect_unknown());
    }

    for (failure, expected) in [
        (
            wire::Failure::SessionLost,
            BrowserError::EffectUnknown("The debugger detached.".into()),
        ),
        (wire::Failure::Timeout, BrowserError::DeadlineAfterDispatch),
    ] {
        *next.lock().unwrap() = wire::Receipt {
            dispatched: true,
            message: Some("The debugger detached.".into()),
            ..FakeExtension::failed("", failure)
        };
        assert_eq!(
            call(&port, navigate()),
            Err(expected),
            "{failure:?} after dispatch"
        );
    }

    *next.lock().unwrap() = wire::Receipt {
        message: Some("Kept open by the preserve-tabs setting.".into()),
        ..FakeExtension::failed("", wire::Failure::LocalInterlock)
    };
    assert_eq!(
        call(
            &port,
            BrowserCommand::CloseTab {
                tab_id: 12,
                released: false
            }
        ),
        Err(BrowserError::LocalInterlock(
            "Kept open by the preserve-tabs setting.".into()
        ))
    );
}

/// A receipt the service cannot trust is never read as success: one with both an outcome and a
/// failure, or with neither, is a protocol failure, and an unreadable one leaves an effect
/// primitive's effect unknown and fails a read as a protocol failure (ACCEPTANCE, executor and
/// truth gates 4).
#[test]
fn an_unreadable_receipt_counts_an_effect_primitive_as_dispatched() {
    let next = Arc::new(Mutex::new(wire::Receipt {
        ok: Some(json!({"tabs": []})),
        ..FakeExtension::failed("", wire::Failure::Other)
    }));
    let (scripted_port, _scripted_fake) = port(scripted(&next));
    assert!(matches!(
        call(&scripted_port, BrowserCommand::ListTabs),
        Err(BrowserError::Protocol(_))
    ));
    *next.lock().unwrap() = FakeExtension::ok("", Value::Null);
    next.lock().unwrap().ok = None;
    assert!(matches!(
        call(&scripted_port, BrowserCommand::ListTabs),
        Err(BrowserError::Protocol(_))
    ));

    let (port, fake) = port(Box::new(|_, _| Reply::Never));
    for (command, effectful) in [
        (
            BrowserCommand::Navigate {
                tab_id: 12,
                url: "https://example.com/".into(),
            },
            true,
        ),
        (BrowserCommand::ListTabs, false),
    ] {
        let asked = fake.wait(|_| true).requests.len();
        let result = thread::scope(|scope| {
            let waiting = scope.spawn(|| call(&port, command));
            let correlation = fake
                .wait(|seen| seen.requests.len() > asked)
                .requests
                .last()
                .map(|(id, _)| id.clone())
                .unwrap();
            fake.send(&json!({"kind": "receipt", "correlation": correlation,
                              "ok": {}, "dispatched": true, "committed_urls": 7}));
            waiting.join().unwrap()
        });
        let error = result.unwrap_err();
        assert_eq!(
            matches!(error, BrowserError::EffectUnknown(_)),
            effectful,
            "{error:?}"
        );
        assert_eq!(
            matches!(error, BrowserError::Protocol(_)),
            !effectful,
            "{error:?}"
        );
    }
}

/// Navigate under a cancellation raised once the fake holds the request, with the fake
/// answering the cancel with `dispatched`.
fn cancelled_navigate(dispatched: bool) -> (Result<BrowserOutcome, BrowserError>, Vec<String>) {
    let (port, fake) = port(Box::new(move |correlation, _| {
        Reply::OnCancel(wire::Receipt {
            dispatched,
            ..FakeExtension::failed(correlation, wire::Failure::Cancelled)
        })
    }));
    let cancelled = AtomicBool::new(false);
    let result = thread::scope(|scope| {
        scope.spawn(|| {
            drop(fake.wait(|seen| !seen.requests.is_empty()));
            cancelled.store(true, Ordering::SeqCst);
        });
        port.call(
            FOUR,
            BrowserCommand::Navigate {
                tab_id: 12,
                url: "https://example.com/".into(),
            },
            Instant::now() + PATIENCE,
            &cancelled,
        )
    });
    let seen = fake.wait(|seen| !seen.cancels.is_empty());
    assert_eq!(seen.cancels, [seen.requests[0].0.clone()]);
    (result, seen.cancels.clone())
}

/// An interrupted call forwards a cancel to the extension and says whether its effect may have
/// happened: a cancel or a deadline before dispatch is safe to repeat, after it is not, a world
/// call the extension answers during the cancel grace keeps what it says about dispatch, and a
/// save to Downloads whose receipt never comes may have written its file (commit a0dfc56b).
#[test]
fn a_cancel_during_navigate_is_forwarded_and_the_receipt_decides_before_or_after() {
    assert_eq!(
        cancelled_navigate(false).0,
        Err(BrowserError::CancelledBeforeDispatch)
    );
    assert_eq!(
        cancelled_navigate(true).0,
        Err(BrowserError::CancelledAfterDispatch)
    );
    assert_eq!(
        cancelled_call(true),
        Err(BrowserError::CancelledAfterDispatch)
    );
    assert_eq!(
        cancelled_call(false),
        Err(BrowserError::CancelledBeforeDispatch)
    );

    let (port, fake) = port(Box::new(|_, _| Reply::Never));
    let cancelled = AtomicBool::new(false);
    let save = |destination| BrowserCommand::ExportRecording {
        recording_id: "recording_1".into(),
        destination,
        max_output_bytes: 4096,
    };
    let cases = [
        (
            BrowserCommand::Navigate {
                tab_id: 12,
                url: "https://example.com/".into(),
            },
            BrowserError::DeadlineAfterDispatch,
        ),
        (
            BrowserCommand::ListTabs,
            BrowserError::DeadlineBeforeDispatch,
        ),
        (
            save(RecordingDestination::Download {
                file_name: "flow.gif".into(),
            }),
            BrowserError::DeadlineAfterDispatch,
        ),
        (
            save(RecordingDestination::Client),
            BrowserError::DeadlineBeforeDispatch,
        ),
    ];
    let count = cases.len();
    for (command, expected) in cases {
        let deadline = Instant::now() + Duration::from_millis(300);
        assert_eq!(
            port.call(FOUR, command, deadline, &cancelled),
            Err(expected)
        );
    }
    let seen = fake.wait(|seen| seen.cancels.len() == count);
    let requested: Vec<_> = seen.requests.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(
        seen.cancels, requested,
        "every deadline forwarded its cancel"
    );
}

/// A command reached the browser once its frame was written, whatever its failure says about
/// effects: a read whose deadline runs out while the browser holds it, and a navigation whose
/// receipt reports the document changed, both fail before any effect and both reached it (commit
/// b0436754). A browser with no connection is never written to, and neither is an effect a
/// runtime hold refuses.
#[test]
fn a_written_command_reached_the_browser_whatever_its_failure() {
    let (port, fake) = port(Box::new(|correlation, primitive| match primitive {
        wire::Primitive::ListTabs(_) => Reply::Never,
        _ => Reply::Now(FakeExtension::failed(
            correlation,
            wire::Failure::DocumentChanged,
        )),
    }));
    let cancelled = AtomicBool::new(false);
    let admit = || Ok(());
    let run = |browser: &str, command: BrowserCommand, patience: Duration| {
        let written = AtomicBool::new(false);
        let outcome = port.call_guarded(
            browser,
            command,
            BrowserDispatch {
                deadline: Instant::now() + patience,
                cancelled: &cancelled,
                admit: &admit,
                written: &written,
            },
        );
        (outcome, written.load(Ordering::SeqCst))
    };
    let navigate = || BrowserCommand::Navigate {
        tab_id: 12,
        url: "https://example.com/".into(),
    };

    assert_eq!(
        run(FOUR, BrowserCommand::ListTabs, Duration::from_millis(300)),
        (Err(BrowserError::DeadlineBeforeDispatch), true)
    );
    assert_eq!(
        run(FOUR, navigate(), PATIENCE),
        (Err(BrowserError::DocumentUnavailable), true)
    );
    assert_eq!(
        run("browser_gone", navigate(), PATIENCE),
        (Err(BrowserError::DisconnectedBeforeDispatch), false)
    );
    assert_eq!(fake.wait(|seen| seen.requests.len() == 2).requests.len(), 2);

    let held = AtomicBool::new(false);
    let refuse = || Err(BrowserError::RuntimeControl(ReasonCode::RuntimeHold));
    assert_eq!(
        port.call_guarded(
            FOUR,
            navigate(),
            BrowserDispatch {
                deadline: Instant::now() + PATIENCE,
                cancelled: &cancelled,
                admit: &refuse,
                written: &held,
            }
        ),
        Err(BrowserError::RuntimeControl(ReasonCode::RuntimeHold))
    );
    assert!(
        !held.load(Ordering::SeqCst),
        "a held effect is never written"
    );
    assert_eq!(fake.wait(|seen| seen.requests.len() == 2).requests.len(), 2);
}

fn signal(
    kind: PresentationKind,
    tab_id: Option<u64>,
    locator: Option<&str>,
) -> PresentationSignal {
    PresentationSignal {
        invocation: "invocation_link".into(),
        signal: kind,
        activity: PresentationActivity::Click,
        phase: "Ghostlight blocked this action".into(),
        label: "Clicking".into(),
        detail: Some("A configured guardrail prevented it.".into()),
        tab_id,
        locator: locator.map(str::to_owned),
        click: Some(ClickShape {
            clicks: 1,
            button: "left".into(),
        }),
    }
}

/// A notice is a frame, never a request, and is never awaited: it names the frame of its handle,
/// and a denial waits until its tab is visible (ACCEPTANCE, presentation 3 and 6). A signal with no
/// tab is dropped.
#[test]
fn a_notice_is_written_once_and_never_awaited() {
    let (port, fake) = port(Box::new(|_, _| Reply::Never));
    for signal in [
        signal(
            PresentationKind::Denial,
            Some(12),
            Some("7F3A:9C21:locator_a1b2c3d4_12"),
        ),
        signal(PresentationKind::Target, Some(12), None),
        signal(PresentationKind::Denial, None, None),
    ] {
        assert_eq!(
            call(&port, BrowserCommand::Present { signal }),
            Ok(BrowserOutcome::Presented { rendered: false })
        );
    }
    let seen = fake.wait(|seen| seen.notices.len() == 2);
    let denial = seen.of_kind("notice")[0];
    assert_eq!(
        denial["frame"],
        json!({"frame_id": "7F3A", "loader_id": "9C21"})
    );
    assert_eq!(denial["signal"]["locator"], "locator_a1b2c3d4_12");
    assert!(seen.notices[0].defer_until_visible);
    assert!(!seen.notices[1].defer_until_visible);
    assert_eq!(seen.notices[1].frame, None);
    assert!(seen.requests.is_empty(), "a notice is not a request");
}

/// An effect counts as started only when an answer shows it ran, and from then on a later failure,
/// or answers the extension cannot pair with the calls, leave the effect unknown (ACCEPTANCE,
/// executor and truth gates 5).
#[test]
fn an_effect_call_acts_only_when_an_answer_shows_it_ran() {
    let next = Arc::new(Mutex::new(answer(Value::Null)));
    let (port, _fake) = port(scripted(&next));
    let cancelled = AtomicBool::new(false);
    let written = AtomicBool::new(false);
    let admit = || Ok(());
    let scroll = || PlannedCall::top(world::scroll(None, Some("down"), None));
    let read = || PlannedCall::top(world::read_text(None, None, 100));
    let returned = |envelope: Value| json!({"status": "returned", "envelope": envelope});
    for (plan, answer, acted) in [
        (scroll(), returned(json!({"ok": true, "value": null})), true),
        (
            scroll(),
            returned(json!({"ok": false, "error": {"code": "stale_locator", "message": "gone"}})),
            false,
        ),
        (
            scroll(),
            returned(json!({"ok": false, "error": {"code": "page_exception", "message": "threw"}})),
            true,
        ),
        (
            scroll(),
            json!({"status": "failed", "code": "timeout", "started": true}),
            true,
        ),
        (
            scroll(),
            json!({"status": "failed", "code": "frame_missing", "started": false}),
            false,
        ),
        (scroll(), json!({"status": "skipped"}), false),
        (read(), returned(json!({"ok": true, "value": null})), false),
    ] {
        *next.lock().unwrap() = wire::Receipt {
            dispatched: true,
            ..FakeExtension::ok("", json!({"answers": [answer]}))
        };
        let mut link = Link::new(
            line(&port),
            BrowserDispatch {
                deadline: Instant::now() + PATIENCE,
                cancelled: &cancelled,
                admit: &admit,
                written: &written,
            },
        );
        let answers = link
            .call(CallRequest::sequence(12, &[plan]).budget(1_000), None)
            .unwrap();
        assert_eq!(answers.len(), 1);
        assert_eq!(link.effect_started(), acted, "{answers:?}");
        let stale = BrowserError::stale(StaleReason::Removed);
        assert_eq!(
            matches!(link.settle(stale), BrowserError::EffectUnknown(_)),
            acted,
            "a later failure is unknown only once an effect ran"
        );
    }

    for (plan, dispatched, unpaired) in [(read(), false, true), (scroll(), true, false)] {
        *next.lock().unwrap() = wire::Receipt {
            dispatched,
            ..FakeExtension::ok("", json!({"answers": []}))
        };
        let mut link = Link::new(
            line(&port),
            BrowserDispatch {
                deadline: Instant::now() + PATIENCE,
                cancelled: &cancelled,
                admit: &admit,
                written: &written,
            },
        );
        let failed = link
            .call(CallRequest::parallel(12, &[plan]).budget(1_000), None)
            .unwrap_err();
        assert_eq!(matches!(failed, BrowserError::Protocol(_)), unpaired);
        assert_eq!(
            matches!(failed, BrowserError::EffectUnknown(_)),
            !unpaired,
            "unpaired answers after a dispatched effect leave the effect unknown"
        );
    }
}

/// Run one scroll `call` under a cancellation raised once the fake holds it, with the fake
/// answering the cancel with an ok receipt whose scroll returned and whose `dispatched` flag
/// is `dispatched`.
fn cancelled_call(dispatched: bool) -> Result<Answers, BrowserError> {
    let (port, fake) = port(Box::new(move |correlation, _| {
        Reply::OnCancel(wire::Receipt {
            dispatched,
            ..FakeExtension::ok(
                correlation,
                json!({"answers": [{"status": "returned",
                                    "envelope": {"ok": true, "value": null}}]}),
            )
        })
    }));
    let cancelled = AtomicBool::new(false);
    let written = AtomicBool::new(false);
    let admit = || Ok(());
    let mut link = Link::new(
        line(&port),
        BrowserDispatch {
            deadline: Instant::now() + PATIENCE,
            cancelled: &cancelled,
            admit: &admit,
            written: &written,
        },
    );
    thread::scope(|scope| {
        scope.spawn(|| {
            drop(fake.wait(|seen| !seen.requests.is_empty()));
            cancelled.store(true, Ordering::SeqCst);
        });
        let scroll = [PlannedCall::top(world::scroll(None, Some("down"), None))];
        link.call(CallRequest::sequence(12, &scroll).budget(1_000), None)
    })
}

/// Requests route by their correlation, and one larger than a direct frame travels as chunks of
/// that correlation that the extension reassembles.
#[test]
fn requests_route_by_correlation_and_large_ones_arrive_in_chunks() {
    let (port, fake) = port(Box::new(|correlation, primitive| {
        let outcome = match primitive {
            wire::Primitive::EvaluateMain(evaluate) => {
                json!({"value": evaluate.script.len().to_string()})
            }
            _ => json!({"tabs": []}),
        };
        Reply::Now(FakeExtension::ok(correlation, outcome))
    }));
    let cancelled = AtomicBool::new(false);
    let written = AtomicBool::new(false);
    let admit = || Ok(());
    let mut link = Link::new(
        line(&port),
        BrowserDispatch {
            deadline: Instant::now() + PATIENCE,
            cancelled: &cancelled,
            admit: &admit,
            written: &written,
        },
    );
    let listed = link.send(wire::ListTabs { tab_ids: None }).unwrap();
    assert!(listed.tabs.is_empty());

    let script = "x".repeat(wire::DIRECT_FRAME_BYTES + 1);
    let evaluated = link
        .send(wire::EvaluateMain {
            tab_id: 7,
            script: script.clone(),
            max_result_chars: 10,
            guard: None,
            settle_ms: 0,
            timeout_ms: 1_000,
        })
        .unwrap();
    assert_eq!(evaluated.value, script.len().to_string());
    let seen = fake.wait(|seen| seen.requests.len() == 2);
    let chunks = seen.of_kind("command_chunk");
    assert!(chunks.len() > 1);
    assert!(chunks
        .iter()
        .all(|chunk| chunk["correlation"] == seen.requests[1].0.as_str()));
    assert_ne!(seen.requests[0].0, seen.requests[1].0);
}
