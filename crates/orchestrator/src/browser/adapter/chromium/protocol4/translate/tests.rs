//! Documents, reading and observation against a scripted protocol-4 extension.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::fake::{FakeExtension, Reply};
use super::super::wire;
use crate::browser::adapter::documents::{
    DocumentInventory, DocumentObservation, DocumentScope, PhysicalDocument,
};
use crate::browser::adapter::{
    BrowserCommand, BrowserOutcome, DiagnosticDetail, DiagnosticSource, PhysicalPoint,
    RecordingDestination,
};
use crate::browser::adapter::{
    PhysicalClick, PhysicalDiagnosticsRead, PhysicalFill, PhysicalTypeText, PhysicalWait,
};
use crate::browser::{BrowserError, BrowserPort, RelayBrowserPort};
use crate::language::PageCondition;
use crate::language::{FindScope, PointerButton};
use crate::workspace::StaleReason;

const FOUR: &str = "browser_four";
const PATIENCE: Duration = Duration::from_secs(30);
const TOP: &str = "https://example.com/";
const CHILD: &str = "https://child.example/";
const SECOND: &str = "https://second.example/";

/// What the scripted extension answers to each `call`: one answer per world call.
type Calls = Box<dyn Fn(&wire::Call) -> Vec<Value> + Send>;

/// A port with a protocol-4 extension attached that answers `frames` with `inventory`, every
/// `call` through `calls`, and diagnostics and recordings with fixed facts.
fn port(inventory: Value, calls: Calls) -> (RelayBrowserPort, FakeExtension) {
    port_with(Box::new(move |correlation, primitive| {
        Reply::Now(FakeExtension::ok(
            correlation,
            match primitive {
                wire::Primitive::Frames(_) => inventory.clone(),
                wire::Primitive::Call(call) => json!({"answers": calls(call)}),
                wire::Primitive::Diagnostics(_) => json!({"entries": [], "truncated": false,
                    "evicted": false, "capture_started": true, "omitted_count": 0}),
                wire::Primitive::Recording(_) => json!({"result": "started", "existing": false,
                    "summary": {"recording_id": "recording_1", "tab_id": 12, "state": "recording",
                    "frame_count": 0, "bytes_held": 0, "duration_ms": 0, "source_urls": [TOP]}}),
                _ => Value::Null,
            },
        ))
    }))
}

fn port_with(script: super::super::fake::Script) -> (RelayBrowserPort, FakeExtension) {
    let port = RelayBrowserPort::new(
        "service_translate".into(),
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

fn call(port: &RelayBrowserPort, command: BrowserCommand) -> Result<BrowserOutcome, BrowserError> {
    let cancelled = AtomicBool::new(false);
    port.call(FOUR, command, Instant::now() + PATIENCE, &cancelled)
}

fn frame(frame_id: &str, loader_id: &str, parent: Option<&str>, url: &str) -> Value {
    json!({"frame_id": frame_id, "loader_id": loader_id, "parent_frame_id": parent, "url": url})
}

/// A top document embedding two documents, as the extension reports it.
fn inventory() -> Value {
    json!({"frames": [frame("T", "t1", None, TOP), frame("C", "c1", Some("T"), CHILD),
                      frame("D", "d1", Some("T"), SECOND)],
           "complete": true, "unseen": []})
}

fn physical(id: &str, parent: Option<&str>, url: &str) -> PhysicalDocument {
    PhysicalDocument {
        id: id.into(),
        url: url.into(),
        parent: parent.map(Into::into),
        supported: true,
    }
}

/// The same three documents, as the authority names them.
fn documents() -> Vec<PhysicalDocument> {
    vec![
        physical("T.t1", None, TOP),
        physical("C.c1", Some("T.t1"), CHILD),
        physical("D.d1", Some("T.t1"), SECOND),
    ]
}

fn scope(allowed: &[&str], strict: bool) -> DocumentScope {
    DocumentScope {
        documents: documents(),
        allowed: allowed.iter().map(|id| (*id).into()).collect(),
        subjects: vec![],
        watch_changes: true,
        strict_tree: strict,
        settle_ms: 0,
        settle_requested: true,
    }
}

fn scoped(scope: DocumentScope, command: BrowserCommand) -> BrowserCommand {
    BrowserCommand::InDocuments {
        scope,
        primitive: Box::new(command),
    }
}

/// The answer of a world call that returned `value`.
fn returned(value: Value) -> Value {
    json!({"status": "returned", "envelope": {"ok": true, "value": value}})
}

/// The answer of a world call whose asset failed with `code`.
fn refused(code: &str) -> Value {
    json!({"status": "returned", "envelope": {"ok": false, "error": {"code": code, "message": ""}}})
}

/// The frame id each world call of a `call` names, or `top` for the current top.
fn frames_of(call: &wire::Call) -> Vec<String> {
    call.calls
        .iter()
        .map(|world| {
            world
                .frame
                .as_ref()
                .map_or_else(|| "top".into(), |frame| frame.frame_id.clone())
        })
        .collect()
}

/// Every `call` the extension received, in order.
fn calls_seen(fake: &FakeExtension) -> Vec<wire::Call> {
    fake.wait(|_| true)
        .requests
        .iter()
        .filter_map(|(_, primitive)| match primitive {
            wire::Primitive::Call(call) => Some(call.clone()),
            _ => None,
        })
        .collect()
}

fn unwrap_scoped(outcome: BrowserOutcome) -> (DocumentObservation, BrowserOutcome) {
    match outcome {
        BrowserOutcome::InDocuments {
            observation,
            result,
        } => (observation, *result),
        other => panic!("a document-scoped outcome, not {other:?}"),
    }
}

fn observed(satisfied: bool, elapsed_ms: u64) -> Value {
    returned(json!({"satisfied": satisfied, "elapsed_ms": elapsed_ms, "readiness": "complete"}))
}

fn wait(condition: PageCondition) -> BrowserCommand {
    BrowserCommand::Observe(PhysicalWait {
        tab_id: 12,
        condition,
        value: Some("Saved".into()),
        locator: None,
        timeout_ms: 2_000,
    })
}

fn a_strict_guard_mismatch_before_any_effect_is_an_unavailable_document() {
    let (port, _fake) = port_with(Box::new(|correlation, _| {
        Reply::Now(FakeExtension::failed(
            correlation,
            wire::Failure::DocumentChanged,
        ))
    }));
    assert_eq!(
        call(
            &port,
            scoped(
                scope(&["T.t1"], true),
                BrowserCommand::ReadText {
                    tab_id: 12,
                    locator: None,
                    max_chars: 100,
                },
            ),
        ),
        Err(BrowserError::DocumentUnavailable)
    );
}

fn read_console() -> BrowserCommand {
    BrowserCommand::ReadDiagnostics(PhysicalDiagnosticsRead {
        tab_id: 12,
        source: DiagnosticSource::Console,
        detail: DiagnosticDetail::All,
        match_text: None,
        after: None,
        limit: 50,
    })
}

/// The binding and guard of every diagnostics read the extension received.
fn diagnostics_reads(fake: &FakeExtension) -> Vec<(Option<String>, bool)> {
    fake.wait(|_| true)
        .requests
        .iter()
        .filter_map(|(_, primitive)| match primitive {
            wire::Primitive::Diagnostics(wire::DiagnosticsOp::Read { binding, guard, .. }) => {
                Some((binding.clone(), guard.is_some()))
            }
            _ => None,
        })
        .collect()
}

/// A diagnostics read binds its records to documents only when the scope watches documents, and
/// always sends its guard (commit a0dfc56b; ACCEPTANCE, browser job journeys 20).
#[test]
fn an_unrestricted_diagnostics_read_sends_no_binding() {
    let (port, fake) = port(inventory(), Box::new(|_| vec![]));
    let mut admitted = scope(&["T.t1", "C.c1", "D.d1"], false);
    admitted.watch_changes = false;
    let (_, result) = unwrap_scoped(call(&port, scoped(admitted, read_console())).unwrap());
    assert!(matches!(result, BrowserOutcome::DiagnosticsRead { .. }));
    assert_eq!(diagnostics_reads(&fake), [(None, true)]);

    a_restricted_read_still_binds_its_documents();
}

fn a_restricted_read_still_binds_its_documents() {
    let (port, fake) = port(inventory(), Box::new(|_| vec![]));
    for allowed in [
        &["T.t1"][..],
        &["T.t1"][..],
        &["C.c1", "T.t1"][..],
        &["T.t1", "C.c1"][..],
    ] {
        let (_, result) =
            unwrap_scoped(call(&port, scoped(scope(allowed, false), read_console())).unwrap());
        assert!(matches!(
            result,
            BrowserOutcome::DiagnosticsRead {
                capture_started: true,
                ..
            }
        ));
    }
    let reads = diagnostics_reads(&fake);
    assert!(reads.iter().all(|(_, guarded)| *guarded));
    let bindings: Vec<String> = reads
        .into_iter()
        .filter_map(|(binding, _)| binding)
        .collect();
    assert_eq!(bindings.len(), 4);
    assert_eq!(bindings[0], bindings[1]);
    assert_ne!(bindings[1], bindings[2]);
    assert_eq!(
        bindings[2], bindings[3],
        "the binding ignores the order of the set"
    );
    assert_eq!(bindings[0].len(), 64);
}

fn nested_document_execution_is_a_protocol_failure_and_sends_nothing() {
    let (port, fake) = port(inventory(), Box::new(|_| vec![]));
    let inner = scoped(
        scope(&["T.t1"], false),
        BrowserCommand::ReadText {
            tab_id: 12,
            locator: None,
            max_chars: 10,
        },
    );
    let describe = BrowserCommand::DescribeDocuments {
        tab_id: 12,
        locators: vec![],
        points: vec![],
        focused: false,
        settle_ms: 0,
    };
    for nested in [inner, describe] {
        assert_eq!(
            call(&port, scoped(scope(&["T.t1"], false), nested)),
            Err(BrowserError::Protocol(
                "nested document execution is invalid".into()
            ))
        );
    }
    assert!(fake.wait(|_| true).requests.is_empty());
}

/// No content reaches a page outside an admitted scope: a content command without one, or one
/// that asks to execute nested documents, is a protocol failure that sends nothing (ADR-0203
/// Decision 6.6).
#[test]
fn a_content_command_without_a_scope_is_a_protocol_error() {
    let (port, fake) = port(inventory(), Box::new(|_| vec![]));
    let locator = "C:c1:locator_a1_1";
    for bare in [
        BrowserCommand::ReadText {
            tab_id: 12,
            locator: None,
            max_chars: 10,
        },
        BrowserCommand::Find {
            tab_id: 12,
            text: "Save".into(),
            kind: FindScope::Any,
            max_results: 5,
        },
        wait(PageCondition::LoadReady),
        BrowserCommand::Activate(PhysicalClick {
            tab_id: 12,
            locator: locator.into(),
            button: PointerButton::Primary,
            click_count: 1,
            modifiers: vec![],
        }),
        BrowserCommand::TypeText(PhysicalTypeText {
            tab_id: 12,
            locator: locator.into(),
            text: "hello".into(),
            clear_first: false,
            allow_credentials: false,
        }),
        BrowserCommand::Fill(PhysicalFill {
            tab_id: 12,
            fields: vec![],
            submit_locator: None,
            timeout_ms: 1_000,
            allow_credentials: false,
        }),
        BrowserCommand::EvaluateScript {
            tab_id: 12,
            script: "1".into(),
            max_result_chars: 10,
        },
        read_console(),
        BrowserCommand::StartRecording { tab_id: 12 },
        BrowserCommand::ExportRecording {
            recording_id: "recording_1".into(),
            destination: RecordingDestination::Target {
                tab_id: 12,
                locator: locator.into(),
                file_name: "flow.gif".into(),
            },
            max_output_bytes: 4096,
        },
    ] {
        assert_eq!(
            call(&port, bare),
            Err(BrowserError::Protocol(super::UNSCOPED_CONTENT.into()))
        );
    }
    assert!(fake.wait(|_| true).requests.is_empty());

    nested_document_execution_is_a_protocol_failure_and_sends_nothing();
}

/// Describe the documents of tab 12 for one locator, with no settle.
fn describe_locator(
    port: &RelayBrowserPort,
    locator: &str,
) -> Result<BrowserOutcome, BrowserError> {
    call(
        port,
        BrowserCommand::DescribeDocuments {
            tab_id: 12,
            locators: vec![locator.into()],
            points: vec![],
            focused: false,
            settle_ms: 0,
        },
    )
}

/// A handle whose document changed is stale as navigated, whether the frame reloaded, left an
/// incomplete inventory, or changed under a call that names one handle, including a recording
/// exported into a page; a change before any effect with no single handle is an unavailable
/// document (commit fa3bcb02, finding G1; ADR-0203 Decision 5.4).
#[test]
fn a_target_in_a_reloaded_iframe_is_refused_as_navigated() {
    for locator in [
        "C:c0:locator_a1_2",
        "T:t0:locator_a1_1",
        "R:r1:locator_a1_3",
    ] {
        let (port, fake) = port(inventory(), Box::new(|_| vec![]));
        assert_eq!(
            describe_locator(&port, locator),
            Err(BrowserError::stale(StaleReason::Navigated)),
            "{locator}"
        );
        assert!(
            calls_seen(&fake).is_empty(),
            "nothing is read for {locator}"
        );
    }
    let (port, _fake) = port(inventory(), Box::new(|_| vec![]));
    assert_eq!(
        describe_locator(&port, "not a locator"),
        Err(BrowserError::stale(StaleReason::Unknown))
    );
    points::a_document_change_during_a_page_export_is_a_navigated_target();

    a_strict_guard_mismatch_before_any_effect_is_an_unavailable_document();
    an_incomplete_inventory_keeps_document_unavailable();
    a_document_change_on_a_one_locator_call_is_a_navigated_target();
}

fn an_incomplete_inventory_keeps_document_unavailable() {
    let incomplete = json!({"frames": [frame("T", "t1", None, TOP)], "complete": false,
                            "unseen": []});
    let (port, _fake) = port(incomplete, Box::new(|_| vec![]));
    assert_eq!(
        describe_locator(&port, "C:c1:locator_a1_2"),
        Ok(BrowserOutcome::Documents {
            tab_id: 12,
            inventory: DocumentInventory {
                documents: vec![physical("T.t1", None, TOP)],
                subjects: vec![],
                unresolved: false,
                incomplete: true,
            },
        }),
        "a frame the extension could not show may still hold the locator's document"
    );
    assert_eq!(
        describe_locator(&port, "T:t0:locator_a1_1"),
        Err(BrowserError::stale(StaleReason::Navigated)),
        "a frame that holds another loader is stale however complete the inventory"
    );
}

/// A read of a target in an embedded document reports the tab's url and host, so an embedded
/// page's host and title never reach the agent or the record (commit fa3bcb02; ADR-0203
/// Decision 6.1).
#[test]
fn a_targeted_read_in_an_iframe_reports_the_tabs_url_and_host() {
    let (port, _fake) = port(
        inventory(),
        Box::new(|call| {
            call.calls
                .iter()
                .map(|world| {
                    let child = world
                        .frame
                        .as_ref()
                        .is_some_and(|frame| frame.frame_id == "C");
                    returned(if child {
                        json!({"text": "Inside", "truncated": false, "title": "Embedded",
                               "url": CHILD})
                    } else {
                        json!({"text": "Outside", "truncated": false, "title": "Top page",
                               "url": "https://example.com/moved"})
                    })
                })
                .collect()
        }),
    );
    let read = |locator: &str| {
        unwrap_scoped(
            call(
                &port,
                scoped(
                    scope(&["T.t1", "C.c1"], false),
                    BrowserCommand::ReadText {
                        tab_id: 12,
                        locator: Some(locator.into()),
                        max_chars: 100,
                    },
                ),
            )
            .unwrap(),
        )
        .1
    };
    assert_eq!(
        read("C:c1:locator_a1_2"),
        BrowserOutcome::Text {
            tab_id: 12,
            text: "Inside".into(),
            truncated: false,
            title: String::new(),
            url: TOP.into(),
        },
        "the embedded document's host and title stay out"
    );
    assert_eq!(
        read("T:t1:locator_a1_1"),
        BrowserOutcome::Text {
            tab_id: 12,
            text: "Outside".into(),
            truncated: false,
            title: "Top page".into(),
            url: "https://example.com/moved".into(),
        },
        "the top document reports its own title and URL"
    );
}

fn a_document_change_on_a_one_locator_call_is_a_navigated_target() {
    let (port, _fake) = port_with(Box::new(|correlation, _| {
        Reply::Now(FakeExtension::failed(
            correlation,
            wire::Failure::DocumentChanged,
        ))
    }));
    let read = |command| call(&port, scoped(scope(&["T.t1", "C.c1"], false), command));
    assert_eq!(
        read(BrowserCommand::ReadText {
            tab_id: 12,
            locator: Some("C:c1:locator_a1_2".into()),
            max_chars: 100,
        }),
        Err(BrowserError::stale(StaleReason::Navigated))
    );
    assert_eq!(
        read(BrowserCommand::DescribeTargets {
            tab_id: 12,
            locators: vec!["T:t1:locator_a1_1".into(), "C:c1:locator_a1_2".into()],
        }),
        Err(BrowserError::DocumentUnavailable),
        "a change among several locators' documents names none of them"
    );
}

mod forms;
mod interaction;
mod points;
