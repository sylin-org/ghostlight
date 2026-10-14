//! What the model is told when protocol-4 work fails, end to end through the executor.
//!
//! Each case runs the executor over a real `RelayBrowserPort` and a scripted protocol-4
//! extension, and checks the status, the effect and the repeat-safety the model reads: a failure
//! that provably changed nothing is a plain failure, and one that may have changed the page is an
//! unknown effect.

use std::net::TcpListener;

use serde_json::Value;

use super::*;
use crate::browser::adapter::chromium::protocol4::fake::{FakeExtension, Reply};
use crate::browser::adapter::chromium::protocol4::wire;
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
        "inspect" | "find" | "describe" | "query_semantic" => {
            returned(json!({"targets": [{"locator": CONTROL,
            "role": "button", "name": "Save", "state": [], "credential_class": false}],
            "truncated": false}))
        }
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
    opened_under(fault, GovernanceFacade::new(None, None))
}

/// [`opened`] under the given governance.
fn opened_under(
    fault: Fault,
    governance: GovernanceFacade,
) -> (
    ApplicationExecutor,
    FakeExtension,
    crate::workspace::WorkspaceId,
) {
    let (executor, fake, workspace, _) = opened_audited(fault, governance);
    (executor, fake, workspace)
}

/// Like [`opened_under`], and also returns the audit the executor records into.
fn opened_audited(
    fault: Fault,
    governance: GovernanceFacade,
) -> (
    ApplicationExecutor,
    FakeExtension,
    crate::workspace::WorkspaceId,
    Arc<MemoryAudit>,
) {
    let audit = Arc::new(MemoryAudit::default());
    let port = Arc::new(RelayBrowserPort::new(
        "service_protocol4".into(),
        Arc::new(wire::Assets {
            id: "0".repeat(64),
            source: String::new(),
        }),
    ));
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
        governance,
        workspaces,
        port,
        PresentationReactor::new(Arc::new(NoPresentation)),
        WorkbenchProjection::default(),
        Arc::new(crate::audit::AuditRecorder::new(
            audit.clone(),
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
    (executor, fake, workspace, audit)
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
            "The job ran out of time before anything changed in the browser.",
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

/// The message the scripted dialog carries.
const DIALOG_MESSAGE: &str = "Your name?";
/// The document inside the page that raised the scripted dialog.
const DIALOG_FRAME: &str = "https://private-frame.invalid/inner";

/// A fault that has a prompt open: `status` reports it as raised by `url`, `handle` answers it.
fn dialog_fault(url: Option<&'static str>) -> Fault {
    Box::new(move |correlation, primitive| {
        let wire::Primitive::Dialog(dialog) = primitive else {
            return None;
        };
        let mut facts =
            json!({"present": true, "dialog_type": "prompt", "message": DIALOG_MESSAGE});
        if let Some(url) = url {
            facts["url"] = json!(url);
        }
        Some(match dialog.op {
            wire::DialogOp::Status => FakeExtension::ok(correlation, facts),
            wire::DialogOp::Handle { .. } => {
                facts["handled"] = json!(true);
                facts["accepted"] = json!(true);
                wire::Receipt {
                    dispatched: true,
                    ..FakeExtension::ok(correlation, facts)
                }
            }
        })
    })
}

/// The primitives the service sent after the first `before` requests, as `name` or
/// `dialog:status` and `dialog:handle`.
fn sent_since(fake: &FakeExtension, before: usize) -> Vec<String> {
    fake.wait(|_| true).requests[before..]
        .iter()
        .map(|(_, primitive)| {
            let encoded = serde_json::to_value(primitive).unwrap();
            let name = encoded["name"].as_str().unwrap().to_owned();
            match encoded["args"]["op"]["op"].as_str() {
                Some(op) if name == "dialog" => format!("{name}:{op}"),
                _ => name,
            }
        })
        .collect()
}

fn requests_so_far(fake: &FakeExtension) -> usize {
    fake.wait(|_| true).requests.len()
}

#[test]
fn dialog_status_is_one_dialog_primitive_and_never_reads_the_frames() {
    let (executor, fake, workspace) = opened(dialog_fault(Some(URL)));
    let before = requests_so_far(&fake);
    let result = execute(
        &executor,
        &workspace,
        "browser_dialog",
        json!({"action": "status"}),
    );
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    assert!(
        result.summary.contains(DIALOG_MESSAGE),
        "{}",
        result.summary
    );
    assert_eq!(sent_since(&fake, before), ["dialog:status"]);
}

#[test]
fn responding_to_a_dialog_looks_once_then_handles_and_never_reads_the_frames() {
    let (executor, fake, workspace) = opened(dialog_fault(Some(URL)));
    let before = requests_so_far(&fake);
    let result = execute(
        &executor,
        &workspace,
        "browser_dialog",
        json!({"action": "respond", "text": "Ada"}),
    );
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    assert_eq!(result.effect, Effect::Applied);
    assert_eq!(
        sent_since(&fake, before),
        ["dialog:status", "dialog:handle"]
    );
}

#[test]
fn a_dialog_raised_by_a_denied_document_is_refused_and_its_message_stays_out() {
    let policy = temporary_policy("dialog-denied");
    fs::write(
        &policy,
        serde_json::to_vec(&json!({"schema": 3, "name": "dialogs", "version": "1",
            "grants": [{"id": "main", "hosts": {"allow": ["example.com"]},
                "allowed": ["read", "action", "write", "execute"]}],
            "config": []}))
        .unwrap(),
    )
    .unwrap();
    let (executor, fake, workspace, audit) = opened_audited(
        dialog_fault(Some(DIALOG_FRAME)),
        GovernanceFacade::new(Some(policy.clone()), None),
    );
    let before = requests_so_far(&fake);
    for arguments in [json!({"action": "status"}), json!({"action": "dismiss"})] {
        let result = execute(&executor, &workspace, "browser_dialog", arguments);
        assert_eq!(result.status, Status::Blocked, "{}", result.summary);
        assert_eq!(result.effect, Effect::None);
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains(DIALOG_MESSAGE),
            "the dialog's message never reaches a refused result"
        );
    }
    assert!(
        !sent_since(&fake, before).contains(&"dialog:handle".to_owned()),
        "a refused dialog is never handled"
    );
    let audited = serde_json::to_string(
        &audit
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|record| record.permissions.clone())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(
        !audited.contains("private-frame.invalid"),
        "an embedded document's host never enters the durable audit: {audited}"
    );
    fs::remove_file(policy).unwrap();
}

#[test]
fn a_dialog_that_names_no_document_cannot_be_authorized() {
    let (executor, fake, workspace) = opened(dialog_fault(None));
    let before = requests_so_far(&fake);
    for arguments in [json!({"action": "status"}), json!({"action": "accept"})] {
        let result = execute(&executor, &workspace, "browser_dialog", arguments);
        assert_plain(
            &result,
            Status::Failed,
            "Document access could not be verified.",
            "document_unavailable",
        );
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains(DIALOG_MESSAGE));
    }
    assert!(!sent_since(&fake, before).contains(&"dialog:handle".to_owned()));
}

#[test]
fn page_work_blocked_by_a_dialog_says_so_and_points_at_the_dialog() {
    let blocked: [Fault; 3] = [
        primitive_fault(
            |primitive| matches!(primitive, wire::Primitive::Frames(_)),
            wire::Failure::DialogOpen,
        ),
        primitive_fault(
            |primitive| matches!(primitive, wire::Primitive::Call(_)),
            wire::Failure::DialogOpen,
        ),
        Box::new(|correlation, primitive| {
            let wire::Primitive::Call(call) = primitive else {
                return None;
            };
            let answer = json!({"status": "failed", "code": "dialog_open",
                "message": "A JavaScript dialog blocks the page.", "started": false});
            Some(FakeExtension::ok(
                correlation,
                json!({"answers": call.calls.iter().map(|_| answer.clone()).collect::<Vec<_>>()}),
            ))
        }),
    ];
    for fault in blocked {
        let (executor, _fake, workspace) = opened(fault);
        let result = execute(&executor, &workspace, "browser_read", json!({}));
        assert_plain(
            &result,
            Status::Failed,
            "A JavaScript dialog is open on this page, so nothing was sent.",
            "dialog_open",
        );
        assert_eq!(
            result.next_steps,
            crate::language::outcome::Refusal::DialogOpen.next_steps()
        );
        assert!(result.next_steps[0].contains("browser_dialog"));
    }
}

#[test]
fn a_fill_after_a_read_spent_the_budget_still_asks_the_page_to_settle() {
    let seen: Arc<std::sync::Mutex<Vec<Value>>> = Arc::default();
    let record = seen.clone();
    let (executor, _fake, workspace) = opened(Box::new(move |correlation, primitive| {
        let wire::Primitive::Call(call) = primitive else {
            return None;
        };
        let mut messages = record.lock().unwrap();
        messages.extend(call.calls.iter().map(|world| world.message.clone()));
        call.calls
            .iter()
            .any(|world| world.message["kind"] == "prepare_fill")
            .then(|| {
                let answers: Vec<Value> = call
                    .calls
                    .iter()
                    .map(|world| {
                        if world.message["kind"] == "prepare_fill" {
                            refused("not_editable")
                        } else {
                            page(world)
                        }
                    })
                    .collect();
                FakeExtension::ok(correlation, json!({"answers": answers}))
            })
    }));
    seen.lock().unwrap().clear();

    let _ = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"selector": {"name": "Save"}, "value": "x"}]}),
    );

    let messages = seen.lock().unwrap();
    let of = |kind: &str| {
        messages
            .iter()
            .find(|message| message["kind"] == kind)
            .unwrap_or_else(|| panic!("no {kind} message was sent"))
    };
    let semantic = of("query_semantic");
    assert!(semantic["settle_ms"].as_u64().unwrap() > 0);
    assert_eq!(semantic["visual_settle"], true);
    let prepare = of("prepare_fill");
    assert_eq!(prepare["settle_ms"], 0, "the read spent the budget");
    assert_eq!(
        prepare["visual_settle"], true,
        "a spent budget is not an opt-out"
    );

    drop(messages);
    seen.lock().unwrap().clear();
    let _ = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"selector": {"name": "Save"}, "value": "x"}], "visual_settle": false}),
    );
    let messages = seen.lock().unwrap();
    let prepare = messages
        .iter()
        .find(|message| message["kind"] == "prepare_fill")
        .expect("a declined settle still prepares the fill");
    assert_eq!(prepare["visual_settle"], false);
}
