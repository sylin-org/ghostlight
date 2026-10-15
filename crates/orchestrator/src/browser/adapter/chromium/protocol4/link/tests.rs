//! The Link and the browser-native commands against a scripted protocol-4 extension.

use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::fake::{FakeExtension, Reply, Script};
use super::super::scope::PlannedCall;
use super::super::translate::tabs::group_key;
use super::super::{wire, world};
use super::{Answers, FocusTab, Frames, Line, Link, Spec, View, WorldCalls};
use crate::browser::adapter::{
    BrowserCommand, BrowserOutcome, BrowserReadiness, ClickShape, PhysicalTab,
    PresentationActivity, PresentationKind, PresentationSignal, RecordingDestination,
};
use crate::browser::{lock, BrowserDispatch, BrowserError, BrowserPort, RelayBrowserPort};
use crate::governance::ReasonCode;

const FOUR: &str = "browser_four";
const WORKSPACE: &str = "workspace_link";
const GROUP_TITLE: &str = "Ghostlight - Claude Code";
const PATIENCE: Duration = Duration::from_secs(30);

fn port(script: Script) -> (RelayBrowserPort, FakeExtension) {
    let port = RelayBrowserPort::new(
        "service_link".into(),
        Arc::new(wire::Assets {
            id: "0".repeat(64),
            source: String::new(),
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let fake = FakeExtension::connect(
        listener.local_addr().unwrap(),
        &FakeExtension::hello(FOUR),
        script,
    );
    port.attach(listener.accept().unwrap().0).unwrap();
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
    port.call(
        FOUR,
        WORKSPACE,
        command,
        Instant::now() + PATIENCE,
        &cancelled,
    )
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

fn summary() -> Value {
    json!({"recording_id": "recording_1", "tab_id": 12, "state": "frozen", "frame_count": 3,
           "bytes_held": 100, "duration_ms": 900, "source_urls": ["https://example.com/"]})
}

fn line(port: &RelayBrowserPort) -> Line {
    lock(&port.adapters).connections[FOUR].line.clone()
}

fn recordings(port: &RelayBrowserPort) -> Arc<super::RecordingIndex> {
    line(port).recordings
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
                reuse: "never".into(),
                focus_window: true,
            },
            primitive: json!({"name": "open_tab", "args": {"url": example,
                "group_key": "g_0308773300b3c6a8", "group_title": GROUP_TITLE,
                "focus_window": true, "ready_timeout_ms": 8000}}),
            receipt: effect(json!({"tab": tab(example), "adopted": false}), &[example]),
            outcome: BrowserOutcome::TabOpened {
                tab: physical_tab(example),
                committed_urls: vec![example.into()],
                reused: false,
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
                direction: "back".into(),
            },
            primitive: navigated(json!({"to": "back"}), false),
            receipt: effect(json!({"tab": tab(example)}), &[example]),
            outcome: landed(),
        },
        Row {
            command: BrowserCommand::TraverseHistory {
                tab_id: 12,
                direction: "forward".into(),
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
    ]
}

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
        let request = seen.of_kind("request")[index];
        assert_eq!(request["primitive"], row.primitive, "row {index} request");
        let correlation = request["correlation"].as_str().unwrap();
        assert!(correlation.starts_with("p4_") && correlation.len() == 35);
    }
}

#[test]
fn the_group_key_is_the_digest_prefix_of_the_group_title() {
    assert_eq!(group_key(GROUP_TITLE), "g_0308773300b3c6a8");
}

#[test]
fn a_history_direction_other_than_back_or_forward_sends_nothing() {
    let next = Arc::new(Mutex::new(answer(json!({"tabs": []}))));
    let (port, fake) = port(scripted(&next));
    let result = call(
        &port,
        BrowserCommand::TraverseHistory {
            tab_id: 12,
            direction: "sideways".into(),
        },
    );
    assert!(matches!(result, Err(BrowserError::Primitive(_))));
    assert_eq!(
        call(&port, BrowserCommand::ListTabs),
        Ok(BrowserOutcome::Tabs { tabs: vec![] })
    );
    let seen = fake.wait(|seen| !seen.requests.is_empty());
    assert_eq!(seen.requests.len(), 1, "only the listing was sent");
}

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
            BrowserError::Primitive("browser tab is missing".into()),
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
            BrowserError::Primitive("Cannot attach to this page.".into()),
        ),
        (
            wire::Failure::Other,
            None,
            BrowserError::Primitive("other".into()),
        ),
        (
            wire::Failure::Unsupported,
            None,
            BrowserError::Unsupported {
                primitive: "navigate".into(),
            },
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

#[test]
fn a_failure_after_the_primitive_acted_is_an_unknown_effect() {
    let next = Arc::new(Mutex::new(wire::Receipt {
        dispatched: true,
        message: Some("The debugger detached.".into()),
        ..FakeExtension::failed("", wire::Failure::SessionLost)
    }));
    let (port, _fake) = port(scripted(&next));
    assert_eq!(
        call(
            &port,
            BrowserCommand::Navigate {
                tab_id: 12,
                url: "https://example.com/".into()
            }
        ),
        Err(BrowserError::EffectUnknown("The debugger detached.".into()))
    );
    *next.lock().unwrap() = wire::Receipt {
        dispatched: true,
        ..FakeExtension::failed("", wire::Failure::Timeout)
    };
    assert_eq!(
        call(&port, BrowserCommand::FocusTab { tab_id: 12 }),
        Err(BrowserError::DeadlineAfterDispatch)
    );
}

#[test]
fn a_receipt_with_both_an_outcome_and_a_failure_is_a_protocol_failure() {
    let next = Arc::new(Mutex::new(wire::Receipt {
        ok: Some(json!({"tabs": []})),
        ..FakeExtension::failed("", wire::Failure::Other)
    }));
    let (port, _fake) = port(scripted(&next));
    assert!(matches!(
        call(&port, BrowserCommand::ListTabs),
        Err(BrowserError::Protocol(_))
    ));
    *next.lock().unwrap() = FakeExtension::ok("", Value::Null);
    next.lock().unwrap().ok = None;
    assert!(matches!(
        call(&port, BrowserCommand::ListTabs),
        Err(BrowserError::Protocol(_))
    ));
}

#[test]
fn an_unreadable_receipt_counts_an_effect_primitive_as_dispatched() {
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
            WORKSPACE,
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
}

#[test]
fn the_deadline_forwards_a_cancel_and_returns_at_once() {
    let (port, fake) = port(Box::new(|_, _| Reply::Never));
    let cancelled = AtomicBool::new(false);
    for (command, expected) in [
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
    ] {
        let deadline = Instant::now() + Duration::from_millis(300);
        let result = port.call(FOUR, WORKSPACE, command, deadline, &cancelled);
        let late = Instant::now().saturating_duration_since(deadline);
        assert_eq!(result, Err(expected));
        assert!(late < Duration::from_millis(200), "returned {late:?} late");
    }
    let seen = fake.wait(|seen| seen.cancels.len() == 2);
    let requested: Vec<_> = seen.requests.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(seen.cancels, requested);
}

#[test]
fn runtime_control_refusing_an_effect_writes_nothing() {
    let next = Arc::new(Mutex::new(answer(json!({"tabs": []}))));
    let (port, fake) = port(scripted(&next));
    let cancelled = AtomicBool::new(false);
    let refuse = || Err(BrowserError::RuntimeControl(ReasonCode::RuntimeHold));
    let dispatch = || BrowserDispatch {
        deadline: Instant::now() + PATIENCE,
        cancelled: &cancelled,
        admit: &refuse,
    };
    assert_eq!(
        port.call_guarded(
            FOUR,
            WORKSPACE,
            BrowserCommand::Navigate {
                tab_id: 12,
                url: "https://example.com/".into()
            },
            dispatch()
        ),
        Err(BrowserError::RuntimeControl(ReasonCode::RuntimeHold))
    );
    assert_eq!(
        port.call_guarded(FOUR, WORKSPACE, BrowserCommand::ListTabs, dispatch()),
        Ok(BrowserOutcome::Tabs { tabs: vec![] }),
        "a read does not ask runtime control"
    );
    let seen = fake.wait(|seen| !seen.requests.is_empty());
    assert_eq!(seen.requests.len(), 1);
    assert!(matches!(seen.requests[0].1, wire::Primitive::ListTabs(_)));
}

#[test]
fn a_workspace_reaches_only_its_one_recording_without_naming_it() {
    let next = Arc::new(Mutex::new(answer(
        json!({"result": "status", "summary": summary()}),
    )));
    let (port, fake) = port(scripted(&next));
    let index = recordings(&port);

    assert_eq!(
        call(&port, BrowserCommand::StatusRecording),
        Ok(BrowserOutcome::RecordingNotFound)
    );
    index.learn("workspace_other", "recording_9");
    assert_eq!(
        call(&port, BrowserCommand::StopRecording),
        Ok(BrowserOutcome::RecordingNotFound),
        "another workspace's recording is not this workspace's to reach"
    );

    index.learn(WORKSPACE, "recording_1");
    assert!(matches!(
        call(&port, BrowserCommand::StatusRecording),
        Ok(BrowserOutcome::RecordingStatus { .. })
    ));
    *next.lock().unwrap() = answer(json!({"result": "exported", "summary": summary(),
        "encoded": {"frame_count": 3, "captured_frame_count": 3, "duration_ms": 900,
                    "width": 640, "height": 400, "byte_count": 2048},
        "delivery": {"delivery": "downloaded"}}));
    assert!(matches!(
        call(
            &port,
            BrowserCommand::ExportRecording {
                destination: RecordingDestination::Download {
                    file_name: "flow.gif".into()
                },
                max_output_bytes: 4096,
            }
        ),
        Ok(BrowserOutcome::RecordingExported { .. })
    ));
    *next.lock().unwrap() =
        answer(json!({"result": "discarded", "recording_id": "recording_1", "released_bytes": 10}));
    assert_eq!(
        call(&port, BrowserCommand::DiscardRecording),
        Ok(BrowserOutcome::RecordingDiscarded { released_bytes: 10 })
    );
    assert_eq!(index.id(WORKSPACE), None);
    assert_eq!(index.id("workspace_other").as_deref(), Some("recording_9"));

    let seen = fake.wait(|seen| seen.requests.len() == 3);
    let args: Vec<Value> = seen
        .of_kind("request")
        .iter()
        .map(|request| request["primitive"].clone())
        .collect();
    assert_eq!(
        args,
        [
            json!({"name": "recording", "args": {"op": "status", "recording_id": "recording_1"}}),
            json!({"name": "recording", "args": {"op": "export", "recording_id": "recording_1",
                "destination": {"destination": "download", "file_name": "flow.gif"},
                "max_output_bytes": 4096}}),
            json!({"name": "recording", "args": {"op": "discard", "recording_id": "recording_1"}}),
        ]
    );
}

/// The ops a scripted extension saw, in order, for the recording tests below.
fn recording_ops(fake: &FakeExtension, count: usize) -> Vec<String> {
    fake.wait(|seen| seen.requests.len() == count)
        .of_kind("request")
        .iter()
        .map(|request| {
            request["primitive"]["args"]["op"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
fn a_second_recording_is_held_until_the_first_is_finished() {
    let next = Arc::new(Mutex::new(answer(json!({"result": "status", "summary": {
        "recording_id": "recording_1", "tab_id": 12, "state": "recording", "frame_count": 3,
        "bytes_held": 100, "duration_ms": 900, "source_urls": ["https://example.com/"]}}))));
    let (port, fake) = port(scripted(&next));
    let index = recordings(&port);
    index.learn(WORKSPACE, "recording_1");

    // The tab it is capturing answers with the recording it already has.
    assert!(matches!(
        call(&port, BrowserCommand::StartRecording { tab_id: 12 }),
        Ok(BrowserOutcome::RecordingStarted { existing: true, .. })
    ));
    // Another tab is held: nothing is started.
    assert_eq!(
        call(&port, BrowserCommand::StartRecording { tab_id: 13 }),
        Ok(BrowserOutcome::RecordingHeld)
    );
    // A stopped recording is held too, until it is saved or discarded.
    *next.lock().unwrap() = answer(json!({"result": "status", "summary": summary()}));
    assert_eq!(
        call(&port, BrowserCommand::StartRecording { tab_id: 12 }),
        Ok(BrowserOutcome::RecordingHeld)
    );
    assert_eq!(index.id(WORKSPACE).as_deref(), Some("recording_1"));
    assert_eq!(recording_ops(&fake, 3), ["status", "status", "status"]);
}

#[test]
fn a_saved_recording_is_replaced_by_the_next_start_and_an_unsaved_one_is_not() {
    let script: Script = Box::new(|correlation, primitive| {
        let facts = match primitive {
            wire::Primitive::Recording(wire::RecordingOp::Status { .. }) => {
                json!({"result": "status", "summary": summary()})
            }
            wire::Primitive::Recording(wire::RecordingOp::Export { .. }) => {
                json!({"result": "exported", "summary": summary(),
                    "encoded": {"frame_count": 3, "captured_frame_count": 3, "duration_ms": 900,
                                "width": 640, "height": 400, "byte_count": 2048},
                    "delivery": {"delivery": "downloaded"}})
            }
            wire::Primitive::Recording(wire::RecordingOp::Discard { .. }) => {
                json!({"result": "discarded", "recording_id": "recording_1", "released_bytes": 10})
            }
            _ => json!({"result": "started", "summary": {
                "recording_id": "recording_2", "tab_id": 13, "state": "recording",
                "frame_count": 0, "bytes_held": 0, "duration_ms": 0, "source_urls": []},
                "existing": false}),
        };
        Reply::Now(wire::Receipt {
            correlation: correlation.into(),
            ..answer(facts)
        })
    });
    let (port, fake) = port(script);
    let index = recordings(&port);
    index.learn(WORKSPACE, "recording_1");

    // Stopped and never saved: held.
    assert_eq!(
        call(&port, BrowserCommand::StartRecording { tab_id: 13 }),
        Ok(BrowserOutcome::RecordingHeld)
    );
    // A save delivers it; it can be saved again.
    for _ in 0..2 {
        assert!(matches!(
            call(
                &port,
                BrowserCommand::ExportRecording {
                    destination: RecordingDestination::Download {
                        file_name: "flow.gif".into()
                    },
                    max_output_bytes: 4096,
                }
            ),
            Ok(BrowserOutcome::RecordingExported { .. })
        ));
    }
    // The next start, on another tab, discards the saved recording and begins a new one.
    assert!(matches!(
        call(&port, BrowserCommand::StartRecording { tab_id: 13 }),
        Ok(BrowserOutcome::RecordingStarted {
            existing: false,
            ..
        })
    ));
    assert_eq!(index.id(WORKSPACE).as_deref(), Some("recording_2"));
    assert!(!index.is_saved(WORKSPACE), "the new recording is unsaved");
    assert_eq!(
        recording_ops(&fake, 6),
        ["status", "export", "export", "status", "discard", "start"]
    );
}

#[test]
fn a_held_recording_the_extension_lost_is_forgotten_and_the_start_goes_ahead() {
    let script: Script = Box::new(|correlation, primitive| {
        let facts = match primitive {
            wire::Primitive::Recording(wire::RecordingOp::Status { .. }) => {
                json!({"result": "not_found"})
            }
            _ => json!({"result": "started", "summary": {
                "recording_id": "recording_2", "tab_id": 12, "state": "recording",
                "frame_count": 0, "bytes_held": 0, "duration_ms": 0, "source_urls": []},
                "existing": false}),
        };
        Reply::Now(wire::Receipt {
            correlation: correlation.into(),
            ..answer(facts)
        })
    });
    let (port, fake) = port(script);
    let index = recordings(&port);
    index.learn(WORKSPACE, "recording_1");

    assert!(matches!(
        call(&port, BrowserCommand::StartRecording { tab_id: 12 }),
        Ok(BrowserOutcome::RecordingStarted {
            existing: false,
            ..
        })
    ));
    assert_eq!(index.id(WORKSPACE).as_deref(), Some("recording_2"));
    assert_eq!(recording_ops(&fake, 2), ["status", "start"]);
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

#[test]
fn a_notice_is_written_once_and_never_awaited() {
    let (port, fake) = port(Box::new(|_, _| Reply::Never));
    for (kind, locator) in [
        (
            PresentationKind::Denial,
            Some("7F3A:9C21:locator_a1b2c3d4_12"),
        ),
        (PresentationKind::Target, None),
    ] {
        assert_eq!(
            call(
                &port,
                BrowserCommand::Present {
                    signal: signal(kind, Some(12), locator)
                }
            ),
            Ok(BrowserOutcome::Presented { rendered: false })
        );
    }
    assert_eq!(
        call(
            &port,
            BrowserCommand::Present {
                signal: signal(PresentationKind::Denial, None, None)
            }
        ),
        Ok(BrowserOutcome::Presented { rendered: false }),
        "a signal with no tab is dropped"
    );
    assert!(matches!(
        call(
            &port,
            BrowserCommand::Present {
                signal: signal(
                    PresentationKind::Target,
                    Some(12),
                    Some("locator_a1b2c3d4_12")
                )
            }
        ),
        Err(BrowserError::Primitive(_))
    ));
    let seen = fake.wait(|seen| seen.notices.len() == 2);
    assert_eq!(
        seen.of_kind("notice")[0],
        &json!({"kind": "notice", "tab_id": 12,
            "frame": {"frame_id": "7F3A", "loader_id": "9C21"},
            "signal": {"invocation": "invocation_link", "signal": "denial", "activity": "click",
                "phase": "Ghostlight blocked this action", "label": "Clicking",
                "detail": "A configured guardrail prevented it.",
                "locator": "locator_a1b2c3d4_12", "click": {"clicks": 1, "button": "left"}},
            "defer_until_visible": true})
    );
    assert!(!seen.notices[1].defer_until_visible);
    assert_eq!(seen.notices[1].frame, None);
    assert!(seen.requests.is_empty(), "a notice is not a request");
}

#[test]
fn every_spec_names_the_primitive_it_sends() {
    fn name_of(primitive: wire::Primitive) -> Value {
        serde_json::to_value(primitive).unwrap()["name"].clone()
    }
    fn check<P: Spec>(primitive: P) {
        assert_eq!(name_of(primitive.into_primitive()), P::NAME);
    }
    let guard = None;
    check(wire::ListTabs { tab_ids: None });
    check(FocusTab(1));
    check(Frames(1));
    check(View(1));
    check(wire::CloseTab {
        tab_id: 1,
        released: false,
    });
    check(wire::SetZoom {
        tab_id: 1,
        zoom: 1.0,
    });
    check(wire::ResizeWindow {
        tab_id: 1,
        width: 800,
        height: 600,
    });
    check(WorldCalls {
        call: wire::Call {
            tab_id: 1,
            mode: wire::CallMode::Sequence,
            calls: vec![],
            guard: guard.clone(),
            settle_ms: 0,
            timeout_ms: 1,
        },
        effectful: false,
    });
    check(wire::EvaluateMain {
        tab_id: 1,
        script: String::new(),
        max_result_chars: 1,
        guard,
        settle_ms: 0,
        timeout_ms: 1,
    });
    check(wire::Dialog {
        tab_id: 1,
        op: wire::DialogOp::Status,
    });
    check(wire::DiagnosticsOp::Clear { tab_ids: vec![] });
    check(wire::RecordingOp::Discard {
        recording_id: String::new(),
    });
}

#[test]
fn an_effect_call_acts_only_when_an_answer_shows_it_ran() {
    let next = Arc::new(Mutex::new(answer(Value::Null)));
    let (port, _fake) = port(scripted(&next));
    let cancelled = AtomicBool::new(false);
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
            WORKSPACE,
            BrowserDispatch {
                deadline: Instant::now() + PATIENCE,
                cancelled: &cancelled,
                admit: &admit,
            },
        );
        let answers = link
            .call(12, &[plan], wire::CallMode::Sequence, None, 0, 1_000, None)
            .unwrap();
        assert_eq!(answers.len(), 1);
        assert_eq!(link.effect_started(), acted, "{answers:?}");
    }

    for (plan, dispatched, unpaired) in [(read(), false, true), (scroll(), true, false)] {
        *next.lock().unwrap() = wire::Receipt {
            dispatched,
            ..FakeExtension::ok("", json!({"answers": []}))
        };
        let mut link = Link::new(
            line(&port),
            WORKSPACE,
            BrowserDispatch {
                deadline: Instant::now() + PATIENCE,
                cancelled: &cancelled,
                admit: &admit,
            },
        );
        let failed = link
            .call(12, &[plan], wire::CallMode::Parallel, None, 0, 1_000, None)
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
    let admit = || Ok(());
    let mut link = Link::new(
        line(&port),
        WORKSPACE,
        BrowserDispatch {
            deadline: Instant::now() + PATIENCE,
            cancelled: &cancelled,
            admit: &admit,
        },
    );
    thread::scope(|scope| {
        scope.spawn(|| {
            drop(fake.wait(|seen| !seen.requests.is_empty()));
            cancelled.store(true, Ordering::SeqCst);
        });
        link.call(
            12,
            &[PlannedCall::top(world::scroll(None, Some("down"), None))],
            wire::CallMode::Sequence,
            None,
            0,
            1_000,
            None,
        )
    })
}

#[test]
fn an_ok_call_receipt_during_the_cancel_grace_keeps_its_effect() {
    assert_eq!(
        cancelled_call(true),
        Err(BrowserError::CancelledAfterDispatch)
    );
    assert_eq!(
        cancelled_call(false),
        Err(BrowserError::CancelledBeforeDispatch)
    );
}

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
    let admit = || Ok(());
    let mut link = Link::new(
        line(&port),
        WORKSPACE,
        BrowserDispatch {
            deadline: Instant::now() + PATIENCE,
            cancelled: &cancelled,
            admit: &admit,
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

#[test]
fn the_tab_comes_from_the_last_receipt_or_one_listing() {
    let next = Arc::new(Mutex::new(wire::Receipt {
        tab: Some(serde_json::from_value(tab("https://example.com/")).unwrap()),
        ..effect(json!({"tab": tab("https://example.com/")}), &[])
    }));
    let (port, fake) = port(scripted(&next));
    let cancelled = AtomicBool::new(false);
    let admit = || Ok(());
    let mut link = Link::new(
        line(&port),
        WORKSPACE,
        BrowserDispatch {
            deadline: Instant::now() + PATIENCE,
            cancelled: &cancelled,
            admit: &admit,
        },
    );
    link.send(wire::Navigate {
        tab_id: 12,
        to: wire::NavigationTarget::Back,
        discard_beforeunload: false,
        ready_timeout_ms: 1_000,
    })
    .unwrap();
    assert!(link.effect_started());
    assert_eq!(link.tab(12).unwrap().tab_id, 12);
    *next.lock().unwrap() = answer(json!({"tabs": []}));
    assert_eq!(
        link.tab(13),
        Err(BrowserError::EffectUnknown("browser tab is missing".into())),
        "a missing tab after an effect leaves the effect unknown"
    );
    let seen = fake.wait(|seen| seen.requests.len() == 2);
    assert_eq!(
        seen.of_kind("request")[1]["primitive"],
        json!({"name": "list_tabs", "args": {"tab_ids": [13]}})
    );
}
