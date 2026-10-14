//! What the model is told when protocol-4 work fails, end to end through the executor.
//!
//! Each case runs the executor over a real `RelayBrowserPort` and a scripted protocol-4
//! extension, and checks the status, the effect and the repeat-safety the model reads. These are
//! the places where protocol 4 tells the truth more precisely than the level-3 extension did: a
//! failure that provably changed nothing is a plain failure, and one that may have changed the
//! page is an unknown effect.

use std::net::TcpListener;

use serde_json::Value;

use super::*;
use crate::browser::adapter::chromium::level4::fake::{FakeExtension, Reply};
use crate::browser::adapter::chromium::level4::wire;
use crate::browser::RelayBrowserPort;

/// The browser identity the scripted extension announces.
const BROWSER: &str = "browser_protocol4";
/// The one tab the scripted browser holds.
const TAB: u64 = 12;
/// The address of the tab's only document.
const URL: &str = "https://example.com/";
/// The document-local locator of the one control the page reports.
const CONTROL: &str = "locator_a1_1";

/// A scripted failure: the receipt for a request, or `None` to answer it as the page would.
type Fault = Box<dyn Fn(&str, &wire::Primitive) -> Option<wire::Receipt> + Send>;

fn tab() -> Value {
    json!({"tab_id": TAB, "title": "Example", "url": URL, "active": true,
        "readiness": "complete"})
}

fn returned(value: Value) -> Value {
    json!({"status": "returned", "envelope": {"ok": true, "value": value}})
}

fn refused(code: &str) -> Value {
    json!({"status": "returned", "envelope": {"ok": false,
        "error": {"code": code, "message": ""}}})
}

/// The page's answer to one world call when nothing goes wrong.
fn page(world: &wire::WorldCall) -> Value {
    let control = json!({"role": "button", "name": "Save"});
    match world.message["kind"].as_str().unwrap_or_default() {
        "observe" => returned(json!({"satisfied": true, "elapsed_ms": 0,
            "readiness": "complete"})),
        "inspect" | "find" | "describe" => returned(json!({"targets": [{"locator": CONTROL,
            "role": "button", "name": "Save", "state": [], "credential_class": false}],
            "truncated": false})),
        "activate" => returned(json!({"activated": true, "subject": control,
            "rectangle": {"left": 10, "top": 20, "width": 40, "height": 10}})),
        "read_text" => returned(json!({"text": "Hello world", "truncated": false,
            "title": "Example", "url": URL})),
        "upload_files" => returned(json!({"uploaded_count": 1, "uploaded_bytes": 1,
            "subject": control})),
        _ => refused("unknown_kind"),
    }
}

/// The browser's answer to one request when nothing goes wrong.
fn ordinary(correlation: &str, primitive: &wire::Primitive) -> wire::Receipt {
    let effect = |value: Value| wire::Receipt {
        dispatched: true,
        ..FakeExtension::ok(correlation, value)
    };
    match primitive {
        wire::Primitive::Frames(_) => FakeExtension::ok(
            correlation,
            json!({"frames": [{"frame_id": "T", "loader_id": "t1", "parent_frame_id": null,
                "url": URL}], "complete": true, "unseen": []}),
        ),
        wire::Primitive::ListTabs(_) => FakeExtension::ok(correlation, json!({"tabs": [tab()]})),
        wire::Primitive::OpenTab(_) => effect(json!({"tab": tab(), "adopted": false})),
        wire::Primitive::Navigate(_) => effect(json!({"tab": tab()})),
        wire::Primitive::Call(call) => FakeExtension::ok(
            correlation,
            json!({"answers": call.calls.iter().map(page).collect::<Vec<_>>()}),
        ),
        wire::Primitive::Input(input) => effect(json!({"completed_steps": input.steps.len()})),
        _ => FakeExtension::ok(correlation, Value::Null),
    }
}

/// A fault that answers every `call` naming `kind` with `answer` in that call's place.
fn page_fault(kind: &'static str, answer: Value) -> Fault {
    Box::new(move |correlation, primitive| {
        let wire::Primitive::Call(call) = primitive else {
            return None;
        };
        call.calls
            .iter()
            .any(|world| world.message["kind"] == kind)
            .then(|| {
                let answers: Vec<Value> = call
                    .calls
                    .iter()
                    .map(|world| {
                        if world.message["kind"] == kind {
                            answer.clone()
                        } else {
                            page(world)
                        }
                    })
                    .collect();
                FakeExtension::ok(correlation, json!({"answers": answers}))
            })
    })
}

/// A fault that answers every request `matches` picks with a failure that dispatched nothing.
fn primitive_fault(matches: fn(&wire::Primitive) -> bool, failure: wire::Failure) -> Fault {
    Box::new(move |correlation, primitive| {
        matches(primitive).then(|| FakeExtension::failed(correlation, failure))
    })
}

/// An executor over a relay port with a scripted protocol-4 extension attached, and a session
/// that already opened the browser's one tab.
fn opened(
    fault: Fault,
) -> (
    ApplicationExecutor,
    FakeExtension,
    crate::workspace::WorkspaceId,
) {
    let port = Arc::new(RelayBrowserPort::new("service_protocol4".into()));
    port.set_protocol4_assets(Arc::new(wire::Assets {
        id: "0".repeat(64),
        source: String::new(),
    }));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let fake = FakeExtension::connect(
        listener.local_addr().unwrap(),
        &FakeExtension::hello(BROWSER),
        Box::new(move |correlation, primitive| {
            Reply::Now(
                fault(correlation, primitive).unwrap_or_else(|| ordinary(correlation, primitive)),
            )
        }),
    );
    port.attach(listener.accept().unwrap().0).unwrap();
    let workspaces = WorkspaceStore::default();
    let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp, None);
    let mut executor = ApplicationExecutor::new(
        GovernanceFacade::new(None, None),
        workspaces,
        port,
        PresentationReactor::new(Arc::new(NoPresentation)),
        WorkbenchProjection::default(),
        Arc::new(crate::audit::AuditRecorder::new(
            Arc::new(MemoryAudit::default()),
            WorkbenchProjection::default(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    executor
        .recovery
        .set_test_candidates(vec![RecoveryCandidate {
            id: "chromium".into(),
            name: "Chromium".into(),
            package: BrowserPackage::Native,
            package_detail: "Chromium native package".into(),
            registration: NativeHostState::Current,
            ordinary_executable: Some(PathBuf::from("chromium")),
        }]);
    let opened = execute(
        &executor,
        &workspace,
        "browser_navigate",
        json!({"url": URL, "new_tab": true}),
    );
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    (executor, fake, workspace)
}

fn execute(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
    tool: &str,
    arguments: Value,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        tool,
        arguments,
        None,
        &CancellationToken::default(),
    )
}

/// The target handle of the page's one control, from an inspection.
fn control(executor: &ApplicationExecutor, workspace: &crate::workspace::WorkspaceId) -> Value {
    let inspected = execute(executor, workspace, "browser_inspect", json!({}));
    assert_eq!(inspected.status, Status::Succeeded, "{}", inspected.summary);
    inspected.facts["items"][0]["target"].clone()
}

/// Assert a failure that changed nothing: safe to repeat, said with `summary`, with `reason` in
/// its facts.
fn assert_plain(
    result: &crate::work::result::InvocationResult,
    status: Status,
    summary: &str,
    reason: &str,
) {
    assert_eq!(result.summary, summary);
    assert_eq!(result.status, status, "{}", result.summary);
    assert_eq!(result.effect, Effect::None, "{}", result.summary);
    assert!(result.repeat_safe, "{}", result.summary);
    assert_eq!(result.facts["reason"], reason, "{}", result.summary);
}

#[test]
fn navigating_a_tab_the_browser_no_longer_has_is_a_plain_failure() {
    let (executor, _fake, workspace) = opened(primitive_fault(
        |primitive| matches!(primitive, wire::Primitive::Navigate(_)),
        wire::Failure::TabMissing,
    ));
    let result = execute(
        &executor,
        &workspace,
        "browser_navigate",
        json!({"url": URL}),
    );
    assert_plain(
        &result,
        Status::Failed,
        "The browser refused this job: browser tab is missing.",
        "browser_primitive_failed",
    );
    assert_eq!(result.facts["detail"], "browser tab is missing");
}

#[test]
fn a_click_whose_target_cannot_be_prepared_is_a_plain_failure() {
    let (executor, _fake, workspace) = opened(page_fault("activate", refused("not_visible")));
    let target = control(&executor, &workspace);
    let result = execute(
        &executor,
        &workspace,
        "browser_click",
        json!({"target": target}),
    );
    assert_plain(
        &result,
        Status::Failed,
        "The browser refused this job: not_visible.",
        "browser_primitive_failed",
    );
    assert_eq!(result.facts["detail"], "not_visible");
}

#[test]
fn a_document_that_changed_before_input_is_unavailable_not_unknown() {
    let (executor, _fake, workspace) = opened(primitive_fault(
        |primitive| matches!(primitive, wire::Primitive::Input(_)),
        wire::Failure::DocumentChanged,
    ));
    let target = control(&executor, &workspace);
    let result = execute(
        &executor,
        &workspace,
        "browser_click",
        json!({"target": target}),
    );
    assert_plain(
        &result,
        Status::Failed,
        "Document access could not be verified.",
        "document_unavailable",
    );
}

#[test]
fn an_upload_that_failed_after_the_files_were_attached_has_an_unknown_effect() {
    let (executor, _fake, workspace) =
        opened(page_fault("upload_files", refused("page_exception")));
    let target = control(&executor, &workspace);
    let result = execute(
        &executor,
        &workspace,
        "browser_upload",
        json!({"target": target, "files": [{"name": "note.txt", "data_base64": "eA=="}]}),
    );
    assert_eq!(
        result.summary,
        "Sent, but the browser never confirmed what happened."
    );
    assert_eq!(result.effect, Effect::Unknown, "{}", result.summary);
    assert!(!result.repeat_safe, "{}", result.summary);
    assert_eq!(result.facts["reason"], "browser_effect_unknown");
}

#[test]
fn a_read_that_was_cancelled_or_ran_out_of_time_changed_nothing() {
    for (failure, status, summary, reason) in [
        (
            wire::Failure::Cancelled,
            Status::Cancelled,
            "Cancelled before anything reached the browser.",
            "cancelled",
        ),
        (
            wire::Failure::Timeout,
            Status::Failed,
            "The job ran out of time before reaching the browser.",
            "deadline",
        ),
    ] {
        let (executor, _fake, workspace) = opened(Box::new(move |correlation, primitive| {
            let wire::Primitive::Call(call) = primitive else {
                return None;
            };
            call.calls
                .iter()
                .any(|world| world.message["kind"] == "read_text")
                .then(|| FakeExtension::failed(correlation, failure))
        }));
        let result = execute(&executor, &workspace, "browser_read", json!({}));
        assert_plain(&result, status, summary, reason);
    }
}
