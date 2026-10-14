//! Documents, reading and observation against a scripted protocol-4 extension.

use std::net::TcpListener;
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
    BrowserCommand, BrowserOutcome, BrowserReadiness, DiagnosticDetail, DiagnosticSource,
    PhysicalPoint,
};
use crate::browser::{BrowserError, BrowserPort, RelayBrowserPort};

const FOUR: &str = "browser_four";
const WORKSPACE: &str = "workspace_translate";
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
    let port = RelayBrowserPort::new("service_translate".into());
    port.set_protocol4_assets(Arc::new(wire::Assets {
        id: "0".repeat(64),
        source: String::new(),
    }));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let fake = FakeExtension::connect(
        listener.local_addr().unwrap(),
        &FakeExtension::hello(FOUR),
        script,
    );
    port.attach(listener.accept().unwrap().0).unwrap();
    (port, fake)
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
        mask: None,
        watch_changes: true,
        strict_tree: strict,
        settle_ms: 0,
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

fn wait(condition: &str) -> BrowserCommand {
    BrowserCommand::Observe {
        tab_id: 12,
        condition: condition.into(),
        value: Some("Saved".into()),
        locator: None,
        timeout_ms: 2_000,
    }
}

#[test]
fn text_present_races_the_documents_and_completes_at_the_first_satisfied_one() {
    let (port, fake) = port(
        inventory(),
        Box::new(|call| match call.mode {
            wire::CallMode::Race => vec![json!({"status": "skipped"}), observed(true, 40)],
            _ => vec![observed(true, 0)],
        }),
    );
    let (observation, result) = unwrap_scoped(
        call(
            &port,
            scoped(scope(&["T.t1", "C.c1"], false), wait("text_present")),
        )
        .unwrap(),
    );
    assert_eq!(
        result,
        BrowserOutcome::Observed {
            tab_id: 12,
            satisfied: true,
            elapsed_ms: 40,
            readiness: BrowserReadiness::Complete,
        }
    );
    let calls = calls_seen(&fake);
    assert_eq!(calls[0].mode, wire::CallMode::Race);
    assert_eq!(frames_of(&calls[0]), ["T", "C"]);
    assert!(calls[0].guard.is_some());
    assert!(calls[0]
        .calls
        .iter()
        .all(|world| world.message["until_satisfied"] == true));
    assert!(calls[0].timeout_ms >= 2_000, "the page owns the timeout");
    assert_eq!(calls[1].calls[0].message["condition"], "load_ready");
    assert_eq!(
        observation.visited,
        ["C.c1"],
        "a skipped loser records nothing"
    );
    assert!(observation.unavailable.is_empty());
}

#[test]
fn text_absent_needs_every_document_and_an_unavailable_one_counts_as_absent() {
    for (child, satisfied, unavailable) in [
        (observed(true, 5), true, false),
        (observed(false, 2_000), false, false),
        (
            json!({"status": "failed", "code": "frame_missing", "started": false}),
            true,
            true,
        ),
    ] {
        let child = child.clone();
        let (port, fake) = port(
            inventory(),
            Box::new(move |call| match call.mode {
                wire::CallMode::Parallel => vec![observed(true, 10), child.clone()],
                _ => vec![observed(true, 0)],
            }),
        );
        let (observation, result) = unwrap_scoped(
            call(
                &port,
                scoped(scope(&["T.t1", "C.c1"], false), wait("text_absent")),
            )
            .unwrap(),
        );
        let BrowserOutcome::Observed {
            satisfied: answered,
            ..
        } = result
        else {
            panic!("an observation");
        };
        assert_eq!(answered, satisfied);
        assert_eq!(calls_seen(&fake)[0].mode, wire::CallMode::Parallel);
        assert_eq!(observation.unavailable == ["C.c1"], unavailable);
    }
}

#[test]
fn an_unseen_frame_is_an_unsupported_document_that_a_strict_guard_pins() {
    let (port, fake) = port(
        json!({"frames": [frame("T", "t1", None, TOP)], "complete": true,
               "unseen": ["https://ads.example/"]}),
        Box::new(|call| vec![returned(json!({"focused": false})); call.calls.len()]),
    );
    let described = call(
        &port,
        BrowserCommand::DescribeDocuments {
            tab_id: 12,
            locators: vec![],
            points: vec![],
            focused: false,
            settle_ms: 0,
        },
    )
    .unwrap();
    let BrowserOutcome::Documents { inventory, .. } = described else {
        panic!("documents");
    };
    assert_eq!(
        inventory.documents[1],
        PhysicalDocument {
            id: "unseen.0".into(),
            url: "https://ads.example/".into(),
            parent: Some("T.t1".into()),
            supported: false,
        }
    );
    let strict = DocumentScope {
        documents: inventory.documents,
        allowed: vec!["T.t1".into()],
        subjects: vec![],
        mask: None,
        watch_changes: true,
        strict_tree: true,
        settle_ms: 0,
    };
    call(
        &port,
        scoped(
            strict,
            BrowserCommand::ReadDiagnostics {
                tab_id: 12,
                source: DiagnosticSource::Both,
                detail: DiagnosticDetail::Problems,
                match_text: None,
                after: None,
                limit: 20,
            },
        ),
    )
    .unwrap();
    let seen = fake.wait(|_| true);
    let Some((_, wire::Primitive::Diagnostics(wire::DiagnosticsOp::Read { guard, .. }))) =
        seen.requests.last()
    else {
        panic!("a diagnostics read");
    };
    let guard = guard.as_ref().expect("a guard");
    assert!(guard.exact);
    assert_eq!(guard.unseen, 1);
}

#[test]
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

#[test]
fn only_admitted_documents_are_asked_and_visited_stays_inside_them() {
    let target = |locator: &str| json!({"locator": locator, "role": "button", "name": "Save", "credential_class": false});
    let (port, fake) = port(
        inventory(),
        Box::new(move |call| {
            frames_of(call)
                .iter()
                .map(|frame| match frame.as_str() {
                    "T" => returned(json!({"targets": [target("locator_a1_1")]})),
                    _ => json!({"status": "failed", "code": "frame_missing", "started": false}),
                })
                .collect()
        }),
    );
    let (observation, result) = unwrap_scoped(
        call(
            &port,
            scoped(
                scope(&["T.t1", "D.d1"], false),
                BrowserCommand::Find {
                    tab_id: 12,
                    text: "Save".into(),
                    kind: "any".into(),
                    max_results: 10,
                },
            ),
        )
        .unwrap(),
    );
    assert_eq!(frames_of(&calls_seen(&fake)[0]), ["T", "D"]);
    assert_eq!(observation.visited, ["T.t1"]);
    assert_eq!(observation.unavailable, ["D.d1"]);
    let BrowserOutcome::Targets { targets, .. } = result else {
        panic!("targets");
    };
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].locator, "T:t1:locator_a1_1");
}

/// Read a document over the top and the documents it embeds, with `read` answering each world
/// call from its frame id, mode and character bound.
fn read_document(
    mode: &str,
    max_chars: usize,
    read: fn(&str, &str, u64) -> Value,
) -> (BrowserOutcome, Vec<(String, String, u64)>) {
    let (port, fake) = port(
        inventory(),
        Box::new(move |call| {
            call.calls
                .iter()
                .map(|world| {
                    read(
                        &world.frame.as_ref().unwrap().frame_id,
                        world.message["mode"].as_str().unwrap_or_default(),
                        world.message["max_chars"].as_u64().unwrap_or_default(),
                    )
                })
                .collect()
        }),
    );
    let (_, result) = unwrap_scoped(
        call(
            &port,
            scoped(
                scope(&["T.t1", "C.c1", "D.d1"], false),
                BrowserCommand::ReadDocument {
                    tab_id: 12,
                    mode: mode.into(),
                    max_chars,
                },
            ),
        )
        .unwrap(),
    );
    let asked = calls_seen(&fake)
        .iter()
        .flat_map(|call| call.calls.clone())
        .map(|world| {
            (
                world.frame.unwrap().frame_id,
                world.message["mode"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                world.message["max_chars"].as_u64().unwrap_or_default(),
            )
        })
        .collect();
    (result, asked)
}

fn text(frame_id: &str, body: &str, maximum: u64) -> Value {
    let kept: String = body
        .chars()
        .take(usize::try_from(maximum).unwrap())
        .collect();
    let (title, url) = if frame_id == "T" {
        ("Top", "https://top.test/")
    } else {
        ("Child", "https://child.test/")
    };
    returned(
        json!({"text": kept, "truncated": body.chars().count() as u64 > maximum,
                    "title": title, "url": url}),
    )
}

#[test]
fn a_document_read_merges_like_the_frames_cases() {
    let (visible, asked) = read_document("visible", 15, |frame, _, maximum| match frame {
        "T" => text(frame, "1234567890", maximum),
        "C" => text(frame, "abcdef", maximum),
        _ => text(frame, "never reached", maximum),
    });
    assert_eq!(
        visible,
        BrowserOutcome::Text {
            tab_id: 12,
            text: "1234567890\n\nabc".into(),
            truncated: true,
            title: "Top".into(),
            url: "https://top.test/".into(),
        }
    );
    assert_eq!(asked[0], ("T".into(), "visible".into(), 15));
    assert_eq!(asked[1], ("C".into(), "visible".into(), 3));

    let (article, asked) = read_document("article", 500, |_, _, _| {
        returned(json!({"text": "Useful article prose", "truncated": false,
                        "article_found": true, "title": "Article",
                        "url": "https://article.test/"}))
    });
    assert_eq!(
        article,
        BrowserOutcome::Text {
            tab_id: 12,
            text: "Useful article prose".into(),
            truncated: false,
            title: "Article".into(),
            url: "https://article.test/".into(),
        }
    );
    assert_eq!(asked, [("T".into(), "article".into(), 500)]);

    let (fallback, asked) = read_document("article", 100, |frame, mode, _| match (frame, mode) {
        (_, "article") => returned(json!({"text": "", "truncated": false,
                "article_found": false, "title": "Shell", "url": "https://shell.test/"})),
        ("T", _) => refused("page_exception"),
        ("C", _) => returned(json!({"text": "Embedded application", "truncated": false,
                "title": "Embed", "url": "https://embed.test/"})),
        _ => returned(json!({"text": "", "truncated": false, "title": "", "url": ""})),
    });
    assert_eq!(
        fallback,
        BrowserOutcome::Text {
            tab_id: 12,
            text: "Embedded application".into(),
            truncated: false,
            title: "Shell".into(),
            url: "https://shell.test/".into(),
        }
    );
    assert_eq!(
        asked
            .iter()
            .map(|(frame, mode, _)| (frame.as_str(), mode.as_str()))
            .collect::<Vec<_>>(),
        [
            ("T", "article"),
            ("T", "visible"),
            ("C", "visible"),
            ("D", "visible")
        ]
    );
}

/// Inspect the whole page's tree at `max_depth`, with each document answering its own tree.
fn inspect_tree(max_depth: usize) -> (Value, bool, Vec<(String, u64, u64)>) {
    let tree = |label: &str, child: Option<&str>| {
        let children: Vec<Value> = child
            .map(|child| json!({"kind": "heading", "label": child, "children": []}))
            .into_iter()
            .collect();
        json!({"kind": "container", "label": label, "children": children})
    };
    let (port, fake) = port(
        inventory(),
        Box::new(move |call| {
            frames_of(call)
                .iter()
                .map(|frame| match frame.as_str() {
                    "T" => returned(json!({"tree": tree("frame 0", Some("top")), "nodes": 2})),
                    "C" => returned(json!({"tree": tree("frame 3", Some("a")), "nodes": 2})),
                    _ => returned(json!({"tree": tree("frame 8", Some("b")), "nodes": 2})),
                })
                .collect()
        }),
    );
    let (_, result) = unwrap_scoped(
        call(
            &port,
            scoped(
                scope(&["T.t1", "C.c1", "D.d1"], false),
                BrowserCommand::InspectTree {
                    tab_id: 12,
                    locator: None,
                    max_depth,
                },
            ),
        )
        .unwrap(),
    );
    let BrowserOutcome::DocumentTree {
        tree, truncated, ..
    } = result
    else {
        panic!("a document tree");
    };
    let asked = calls_seen(&fake)
        .iter()
        .flat_map(|call| call.calls.clone())
        .map(|world| {
            (
                world.frame.unwrap().frame_id,
                world.message["max_depth"].as_u64().unwrap(),
                world.message["max_nodes"].as_u64().unwrap(),
            )
        })
        .collect();
    (serde_json::from_str(&tree).unwrap(), truncated, asked)
}

#[test]
fn a_page_tree_merges_like_the_frames_cases() {
    let (tree, truncated, asked) = inspect_tree(5);
    assert_eq!(
        asked,
        [
            ("T".into(), 5, 400),
            ("C".into(), 4, 398),
            ("D".into(), 4, 398)
        ]
    );
    let labels: Vec<&str> = tree["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["top", "frame 3", "frame 8"]);
    assert!(!truncated);

    let (tree, truncated, asked) = inspect_tree(1);
    assert_eq!(asked, [("T".into(), 1, 400)]);
    assert_eq!(tree["label"], "frame 0");
    assert!(
        truncated,
        "a depth of one leaves the embedded documents unread"
    );
}

#[test]
fn a_recording_started_under_a_watching_scope_watches_the_admitted_documents() {
    for watch in [true, false] {
        let (port, fake) = port(inventory(), Box::new(|_| vec![]));
        let mut admitted = scope(&["T.t1", "C.c1"], true);
        admitted.watch_changes = watch;
        let (_, result) = unwrap_scoped(
            call(
                &port,
                scoped(admitted, BrowserCommand::StartRecording { tab_id: 12 }),
            )
            .unwrap(),
        );
        assert!(matches!(result, BrowserOutcome::RecordingStarted { .. }));
        let seen = fake.wait(|_| true);
        let Some((_, wire::Primitive::Recording(wire::RecordingOp::Start { watch: sent, .. }))) =
            seen.requests.last()
        else {
            panic!("a recording start");
        };
        let expected = watch.then(|| {
            [("T", "t1", TOP), ("C", "c1", CHILD), ("D", "d1", SECOND)]
                .iter()
                .map(|(frame_id, loader_id, url)| wire::GuardDoc {
                    frame_id: (*frame_id).into(),
                    loader_id: (*loader_id).into(),
                    url: (*url).into(),
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(sent, &expected);
    }
}

#[test]
fn the_diagnostics_binding_follows_the_admitted_set() {
    let (port, fake) = port(inventory(), Box::new(|_| vec![]));
    let read = || BrowserCommand::ReadDiagnostics {
        tab_id: 12,
        source: DiagnosticSource::Console,
        detail: DiagnosticDetail::All,
        match_text: None,
        after: None,
        limit: 50,
    };
    for allowed in [
        &["T.t1"][..],
        &["T.t1"][..],
        &["C.c1", "T.t1"][..],
        &["T.t1", "C.c1"][..],
    ] {
        let (_, result) =
            unwrap_scoped(call(&port, scoped(scope(allowed, false), read())).unwrap());
        assert!(matches!(
            result,
            BrowserOutcome::DiagnosticsRead {
                capture_started: true,
                ..
            }
        ));
    }
    let seen = fake.wait(|_| true);
    let bindings: Vec<String> = seen
        .requests
        .iter()
        .filter_map(|(_, primitive)| match primitive {
            wire::Primitive::Diagnostics(wire::DiagnosticsOp::Read { binding, guard, .. }) => {
                assert!(guard.is_some());
                binding.clone()
            }
            _ => None,
        })
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

#[test]
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

#[test]
fn describing_documents_binds_locators_points_and_focus_to_current_documents() {
    let (port, fake) = port(
        inventory(),
        Box::new(|call| {
            call.calls
                .iter()
                .map(|world| {
                    let frame = world.frame.as_ref().map(|frame| frame.frame_id.as_str());
                    match (world.message["kind"].as_str().unwrap(), frame) {
                        ("observe", _) => observed(true, 0),
                        ("hit_point", Some("T")) => {
                            returned(json!({"x": 110, "y": 120, "embed": true, "subject": null}))
                        }
                        ("hit_point", _) => returned(json!({"x": 10, "y": 20, "embed": false,
                            "subject": {"role": "button", "name": "Go"}})),
                        ("owner_probe", _) => returned(json!({
                            "hit": world.owner_of.as_deref() == Some("C"),
                            "left": 100, "top": 100})),
                        ("focus_probe", Some(frame)) => returned(json!({"focused": frame == "D"})),
                        _ => refused("unknown_kind"),
                    }
                })
                .collect()
        }),
    );
    let described = call(
        &port,
        BrowserCommand::DescribeDocuments {
            tab_id: 12,
            locators: vec!["T:t1:locator_a1_1".into(), "C:c0:locator_a1_2".into()],
            points: vec![PhysicalPoint { x: 110.0, y: 620.0 }],
            focused: true,
            settle_ms: 300,
        },
    )
    .unwrap();
    assert_eq!(
        described,
        BrowserOutcome::Documents {
            tab_id: 12,
            inventory: DocumentInventory {
                documents: documents(),
                subjects: vec!["T.t1".into(), "C.c1".into(), "D.d1".into()],
                unresolved: true,
                incomplete: false,
            },
        }
    );
    let calls = calls_seen(&fake);
    assert_eq!(calls[0].calls[0].message["condition"], "visual_settle");
    assert_eq!(calls[0].calls[0].message["timeout_ms"], 300);
    assert_eq!(calls[1].calls[0].message["page"], true);
    assert_eq!(calls[2].calls[0].message["x"], 110);
    assert_eq!(calls[3].calls[0].message["x"], 10);
    assert_eq!(calls[3].calls[0].message["page"], false);
    assert_eq!(frames_of(&calls[4]), ["T", "C", "D"]);
}

mod interaction;
