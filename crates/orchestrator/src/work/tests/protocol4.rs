//! What the model is told when protocol-4 work fails, end to end through the executor.
//!
//! Each case runs the executor over a real `RelayBrowserPort` and a scripted protocol-4
//! extension, and checks the status, the effect and the repeat-safety the model reads: a failure
//! that provably changed nothing is a plain failure, and one that may have changed the page is an
//! unknown effect.

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

/// A scripted reply: how the browser answers a request, or `None` to answer it as the page would.
type Script = Box<dyn Fn(&str, &wire::Primitive) -> Option<Reply> + Send>;

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
pub(super) fn ordinary(correlation: &str, primitive: &wire::Primitive) -> wire::Receipt {
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
    opened_under(fault, GovernanceFacade::new(None))
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
    opened_replying(
        Box::new(move |correlation, primitive| fault(correlation, primitive).map(Reply::Now)),
        governance,
    )
}

/// Like [`opened_audited`], with a script that may also hold a reply back or never give one.
fn opened_replying(
    script: Script,
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
    let (service, extension) = ghostlight_relay::endpoint::pair().unwrap();
    let fake = FakeExtension::connect(
        extension,
        &FakeExtension::hello(BROWSER),
        Box::new(move |correlation, primitive| {
            script(correlation, primitive)
                .unwrap_or_else(|| Reply::Now(ordinary(correlation, primitive)))
        }),
    );
    port.attach(service).unwrap();
    let workspaces = WorkspaceStore::default();
    let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp);
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
    executor.execute(workspace, tool, arguments, &CancellationToken::default())
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
fn a_click_the_page_refuses_says_why_that_nothing_was_clicked_and_what_works() {
    for (code, summary, tool) in [
        (
            "not_visible",
            "That target is not visible on the page. Nothing was clicked.",
            "browser_find",
        ),
        (
            "disabled",
            "That target is disabled. Nothing was clicked.",
            "browser_inspect",
        ),
    ] {
        let (executor, _fake, workspace) = opened(page_fault("activate", refused(code)));
        let target = control(&executor, &workspace);
        let result = execute(
            &executor,
            &workspace,
            "browser_click",
            json!({"target": target}),
        );
        assert_plain(&result, Status::Failed, summary, code);
        assert!(result.facts.get("detail").is_none(), "{code}");
        assert_eq!(result.next_steps.len(), 1, "{code}");
        assert!(result.next_steps[0].contains(tool), "{code}");
    }
}

#[test]
fn typing_into_a_field_the_page_refuses_says_nothing_was_typed() {
    for (code, summary) in [
        ("read_only", "That field is read-only. Nothing was typed."),
        (
            "not_editable",
            "That target does not take typed text. Nothing was typed.",
        ),
    ] {
        let (executor, _fake, workspace) = opened(page_fault("type_text", refused(code)));
        let target = control(&executor, &workspace);
        let result = execute(
            &executor,
            &workspace,
            "browser_type_text",
            json!({"target": target, "text": "Ada"}),
        );
        assert_plain(&result, Status::Failed, summary, code);
        assert!(
            result.next_steps[0].contains("browser_find with scope control"),
            "{code}"
        );
    }
}

#[test]
fn a_flow_step_the_page_refuses_names_that_steps_own_gesture() {
    let (executor, _fake, workspace) = opened(page_fault("activate", refused("disabled")));
    let target = control(&executor, &workspace);
    let result = execute(
        &executor,
        &workspace,
        "browser_flow",
        json!({"steps": [{"tool": "browser_click", "arguments": {"target": target}}]}),
    );
    assert_eq!(result.status, Status::Failed, "{}", result.summary);
    assert_eq!(result.effect, Effect::None);
    let step = &result.facts["steps"][0]["result"];
    assert_eq!(
        step["summary"],
        "That target is disabled. Nothing was clicked."
    );
    assert_eq!(step["facts"]["reason"], "disabled");
}

#[test]
fn a_document_that_changed_before_input_is_a_stale_target_not_unknown() {
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
    assert_eq!(
        result.summary,
        "That target belongs to an older version of the page."
    );
    assert_eq!(result.effect, Effect::None, "{}", result.summary);
    assert!(result.repeat_safe, "{}", result.summary);
    assert_eq!(result.facts["stale"], "navigated", "{}", result.summary);
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
        "The browser never confirmed what the sent command did."
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
        serde_json::to_vec(&json!({"schema": 4, "name": "dialogs", "version": "1",
            "grants": [{"id": "main", "hosts": {"allow": ["example.com"]},
                "allowed": ["read", "action", "write", "execute"]}],
            "config": []}))
        .unwrap(),
    )
    .unwrap();
    let (executor, fake, workspace, audit) = opened_audited(
        dialog_fault(Some(DIALOG_FRAME)),
        GovernanceFacade::new(Some(policy.to_path_buf())),
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

/// The two controls the fill tests inspect: a dropdown and a checkbox, in document order.
const DROPDOWN: &str = "locator_a1_1";
const CHECKBOX: &str = "locator_a1_2";

/// A page with a dropdown named Role and a checkbox named Agree. `special` answers a world call
/// first, when it has an answer; the form answers the controls' inspection and description, and
/// the ordinary page answers the rest.
fn form_page(special: impl Fn(&wire::WorldCall) -> Option<Value> + Send + 'static) -> Fault {
    Box::new(move |correlation, primitive| {
        let wire::Primitive::Call(call) = primitive else {
            return None;
        };
        let controls = [
            json!({"locator": DROPDOWN, "role": "combobox", "name": "Role", "state": [],
                "credential_class": false}),
            json!({"locator": CHECKBOX, "role": "checkbox", "name": "Agree", "state": [],
                "credential_class": false}),
        ];
        let answers: Vec<Value> = call
            .calls
            .iter()
            .map(|world| {
                special(world).unwrap_or_else(|| {
                    match world.message["kind"].as_str().unwrap_or_default() {
                        "inspect" => returned(json!({"targets": controls, "truncated": false})),
                        "describe" => returned(json!({"targets": world.message["locators"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|locator| controls
                                .iter()
                                .find(|control| control["locator"] == *locator)
                                .cloned()
                                .unwrap())
                            .collect::<Vec<_>>()})),
                        _ => page(world),
                    }
                })
            })
            .collect();
        Some(FakeExtension::ok(correlation, json!({"answers": answers})))
    })
}

/// A page whose fill the page refuses with `detail`, as one refused `prepare_fill` envelope.
fn refusing_form(detail: Value) -> Fault {
    form_page(move |world| {
        (world.message["kind"] == "prepare_fill").then(|| {
            json!({"status": "returned", "envelope": {"ok": false,
                "error": {"code": "option_missing", "message": "select option not found",
                    "detail": detail}}})
        })
    })
}

/// A page that takes every field as a page-set control and answers the verification with
/// `verify`.
fn verifying_form(verify: Value) -> Fault {
    form_page(move |world| match world.message["kind"].as_str() {
        Some("prepare_fill") => Some(returned(json!({"prepared": true,
            "field_kinds": world.message["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|_| "local")
                .collect::<Vec<_>>()}))),
        Some("fill_local") => Some(returned(json!({"filled": true}))),
        Some("verify_fill_retained") => Some(verify.clone()),
        _ => None,
    })
}

/// The handles of the form's two controls, from an inspection.
fn form_handles(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
) -> (String, String) {
    let inspected = execute(executor, workspace, "browser_inspect", json!({}));
    assert_eq!(inspected.status, Status::Succeeded, "{}", inspected.summary);
    let handle = |index: usize| {
        inspected.facts["items"][index]["target"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    (handle(0), handle(1))
}

#[test]
fn a_refused_fill_lists_what_would_succeed_and_names_each_handle() {
    let detail = json!({"problems": [
        {"field": 0, "problem": "option_missing", "options": ["Developer", "Administrator", "Other"]},
        {"field": 1, "problem": "not_boolean"}
    ]});
    let (executor, _fake, workspace, audit) =
        opened_audited(refusing_form(detail), GovernanceFacade::new(None));
    let (role, agree) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "Engineer"}, {"target": agree, "value": "maybe"}]}),
    );

    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.effect, Effect::None);
    assert!(result.repeat_safe);
    assert_eq!(
        result.summary,
        "Nothing was changed: 2 of 2 fields cannot take their values. The \"Role\" dropdown has no option \"Engineer\". It offers: Developer, Administrator, Other. The \"Agree\" checkbox takes true or false, not \"maybe\"."
    );
    assert_eq!(
        result.next_steps,
        vec![
            format!("Repeat browser_fill_form with {role} set to one of the offered options."),
            format!("Repeat with true or false for {agree}.")
        ]
    );
    assert_eq!(result.facts["reason"], "field_value_rejected");
    assert_eq!(
        result.facts["problems"],
        json!([
            {"field": 0, "target": role, "problem": "option_missing",
             "options": ["Developer", "Administrator", "Other"]},
            {"field": 1, "target": agree, "problem": "not_boolean"}
        ])
    );
    let recorded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(recorded
        .contains("Refused a form fill before any edit: 2 fields could not take their values."));
    for private in ["Developer", "Engineer", "maybe", "Role", "Agree"] {
        assert!(!recorded.contains(private), "{private} reached the audit");
    }
}

#[test]
fn a_refused_fill_without_target_names_speaks_in_unnamed_nouns() {
    let policy = temporary_policy("fill-refusal-names");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let detail = json!({"problems": [
        {"field": 0, "problem": "option_missing", "options": ["PRIVATE_OPTION"]}
    ]});
    let (executor, _fake, workspace) = opened_under(
        refusing_form(detail),
        GovernanceFacade::new(Some(policy.to_path_buf())),
    );
    let (role, _) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "PRIVATE_VALUE"}]}),
    );

    assert_eq!(
        result.summary,
        "Nothing was changed. A dropdown has no matching option."
    );
    assert_eq!(result.facts["problems"][0]["target"], role);
    assert_eq!(
        result.facts["problems"][0]["options"],
        json!(["PRIVATE_OPTION"])
    );
    assert!(!result.facts.to_string().contains("PRIVATE_VALUE"));
    assert!(!result.model_text().contains("PRIVATE"));
}

#[test]
fn a_refusal_the_service_cannot_read_stays_the_browsers_own_sentence() {
    let detail = json!({"problems": [{"field": 0, "problem": "option_vanished"}]});
    let (executor, _fake, workspace) = opened(refusing_form(detail));
    let (role, _) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "Engineer"}]}),
    );

    assert_plain(
        &result,
        Status::Failed,
        "The browser refused this job: select option not found.",
        "browser_primitive_failed",
    );
}

#[test]
fn a_problem_for_a_field_the_call_does_not_have_is_an_invalid_plan() {
    let detail = json!({"problems": [{"field": 7, "problem": "option_missing"}]});
    let (executor, _fake, workspace) = opened(refusing_form(detail));
    let (role, _) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "Engineer"}]}),
    );

    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.effect, Effect::None);
    assert_eq!(result.facts["reason"], "browser_primitive_failed");
    assert_eq!(
        result.facts["detail"],
        "content primitive returned an invalid fill plan"
    );
}

#[test]
fn a_fill_says_what_the_page_now_holds() {
    let verify = returned(json!({"retained": true, "fields": [
        {"field": 0, "now": "Administrator"},
        {"field": 1, "now": "checked"}
    ]}));
    let (executor, _fake, workspace, audit) =
        opened_audited(verifying_form(verify), GovernanceFacade::new(None));
    let (role, agree) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "Administrator"}, {"target": agree, "value": true}]}),
    );

    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    assert_eq!(
        result.summary,
        "Filled 2 fields on example.com: Role = Administrator, Agree = checked."
    );
    assert_eq!(
        result.facts["fields"],
        json!([
            {"target": role, "now": "Administrator"},
            {"target": agree, "now": "checked"}
        ])
    );
    let recorded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(recorded.contains("Filled 2 fields on example.com."));
    for private in ["Administrator", "Role", "Agree"] {
        assert!(!recorded.contains(private), "{private} reached the audit");
    }
}

#[test]
fn a_fill_without_target_names_says_only_how_many_fields_it_filled() {
    let policy = temporary_policy("fill-echo-names");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let verify =
        returned(json!({"retained": true, "fields": [{"field": 0, "now": "Administrator"}]}));
    let (executor, _fake, workspace) = opened_under(
        verifying_form(verify),
        GovernanceFacade::new(Some(policy.to_path_buf())),
    );
    let (role, _) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "Administrator"}]}),
    );

    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    assert_eq!(result.summary, "Filled 1 field on example.com.");
    assert_eq!(
        result.facts["fields"],
        json!([{"target": role, "now": "Administrator"}])
    );
}

#[test]
fn a_normalized_match_is_echoed_with_the_value_the_caller_gave() {
    let verify = returned(json!({"retained": true, "fields": [
        {"field": 0, "now": "Administrator", "matched": "normalized"}
    ]}));
    let (executor, _fake, workspace, audit) =
        opened_audited(verifying_form(verify), GovernanceFacade::new(None));
    let (role, _) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "administrator"}]}),
    );

    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    assert_eq!(
        result.summary,
        "Filled 1 field on example.com: Role = Administrator (matched \"administrator\")."
    );
    assert_eq!(
        result.facts["fields"],
        json!([{"target": role, "now": "Administrator", "match": "normalized"}])
    );
    let recorded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    for private in ["Administrator", "administrator", "normalized", "Role"] {
        assert!(!recorded.contains(private), "{private} reached the audit");
    }
}

#[test]
fn a_normalized_match_without_target_names_keeps_the_given_value_out_of_the_sentence() {
    let policy = temporary_policy("fill-match-names");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let verify = returned(json!({"retained": true, "fields": [
        {"field": 0, "now": "Administrator", "matched": "normalized"}
    ]}));
    let (executor, _fake, workspace) = opened_under(
        verifying_form(verify),
        GovernanceFacade::new(Some(policy.to_path_buf())),
    );
    let (role, _) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "PRIVATE_GIVEN"}]}),
    );

    assert_eq!(result.summary, "Filled 1 field on example.com.");
    assert_eq!(result.facts["fields"][0]["match"], "normalized");
    assert!(!result.model_text().contains("PRIVATE_GIVEN"));
    assert!(!result.facts.to_string().contains("PRIVATE_GIVEN"));
}

#[test]
fn a_list_reaches_the_page_as_a_list_and_a_boolean_stays_a_boolean() {
    let prepared: Arc<std::sync::Mutex<Vec<Value>>> = Arc::default();
    let record = prepared.clone();
    let verify = returned(json!({"retained": true, "fields": []}));
    let (executor, _fake, workspace) = opened(form_page(move |world| {
        match world.message["kind"].as_str() {
            Some("prepare_fill") => {
                record.lock().unwrap().push(world.message["fields"].clone());
                Some(returned(
                    json!({"prepared": true, "field_kinds": ["local", "local"]}),
                ))
            }
            Some("fill_local") => Some(returned(json!({"filled": true}))),
            Some("verify_fill_retained") => Some(verify.clone()),
            _ => None,
        }
    }));
    let (role, agree) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": ["Alpha", "beta"]}, {"target": agree, "value": true}]}),
    );

    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    let seen = prepared.lock().unwrap();
    assert_eq!(seen[0][0]["value"], json!(["Alpha", "beta"]));
    assert_eq!(seen[0][1]["value"], json!(true));
}

#[test]
fn a_refused_entry_of_a_list_quotes_that_entry_and_not_the_whole_list() {
    let detail = json!({"problems": [
        {"field": 0, "problem": "option_missing", "value_index": 1, "options": ["Alpha", "Beta"]}
    ]});
    let (executor, _fake, workspace) = opened(refusing_form(detail));
    let (role, _) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": ["Beta", "Delta"]}]}),
    );

    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.effect, Effect::None);
    assert_eq!(
        result.summary,
        "Nothing was changed. The \"Role\" dropdown has no option \"Delta\". It offers: Alpha, Beta."
    );
    assert_eq!(result.facts["problems"][0]["value_index"], 1);
    assert!(!result.facts.to_string().contains("Delta"));
}

#[test]
fn a_fill_the_page_partly_undid_is_a_partial_failure_with_no_dialog_advice() {
    let verify = json!({"status": "returned", "envelope": {"ok": false, "error": {
        "code": "not_retained",
        "message": "target did not retain filled value",
        "detail": {"problems": [{"field": 0, "problem": "reverted", "now": "Choose one..."}]}
    }}});
    let (executor, _fake, workspace, audit) =
        opened_audited(verifying_form(verify), GovernanceFacade::new(None));
    let (role, agree) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "Administrator"}, {"target": agree, "value": true}]}),
    );

    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.effect, Effect::Partial);
    assert!(!result.repeat_safe);
    assert_eq!(
        result.summary,
        "Set 1 of 2 fields; the \"Role\" dropdown went back to \"Choose one...\" after the page reacted."
    );
    assert_eq!(
        result.next_steps,
        vec![format!(
            "Read {role} with browser_read to see its current value."
        )]
    );
    assert_eq!(result.facts["reason"], "fields_reverted");
    assert_eq!(result.facts["set"], 1);
    assert_eq!(
        result.facts["reverted"],
        json!([{"field": 0, "target": role, "problem": "reverted", "now": "Choose one..."}])
    );
    assert!(!result.model_text().to_lowercase().contains("dialog"));
    let recorded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(recorded.contains("A form fill was partly applied: 1 field held its value."));
    for private in ["Choose one", "Role", "Administrator"] {
        assert!(!recorded.contains(private), "{private} reached the audit");
    }
}

#[test]
fn a_page_that_never_settled_is_still_an_unknown_effect_with_dialog_advice() {
    let (executor, _fake, workspace) = opened(verifying_form(refused("not_retained")));
    let (role, _) = form_handles(&executor, &workspace);

    let result = execute(
        &executor,
        &workspace,
        "browser_fill_form",
        json!({"fields": [{"target": role, "value": "Administrator"}]}),
    );

    assert_eq!(result.status, Status::Unknown);
    assert_eq!(result.effect, Effect::Unknown);
    assert!(result.model_text().contains("browser_dialog"));
}

/// A page with two controls named Save. Its semantic query keeps the maximum it is sent and says
/// when more matched, as the page asset does.
fn two_saves() -> Fault {
    let semantic = |world: &wire::WorldCall| {
        let maximum = world.message["max_results"].as_u64().unwrap_or(0);
        let targets: Vec<Value> = (1..=std::cmp::min(2, maximum))
            .map(|index| {
                json!({"locator": format!("locator_a1_{index}"), "role": "button",
                    "name": "Save", "state": [], "credential_class": false})
            })
            .collect();
        returned(json!({"targets": targets, "truncated": maximum < 2}))
    };
    Box::new(move |correlation, primitive| {
        let wire::Primitive::Call(call) = primitive else {
            return None;
        };
        let answers: Vec<Value> = call
            .calls
            .iter()
            .map(|world| {
                if world.message["kind"] == "query_semantic" {
                    semantic(world)
                } else {
                    page(world)
                }
            })
            .collect();
        Some(FakeExtension::ok(correlation, json!({"answers": answers})))
    })
}

#[test]
fn a_selector_present_wait_over_two_matches_reports_no_size_limit() {
    let (executor, _fake, workspace) = opened(two_saves());

    let waited = execute(
        &executor,
        &workspace,
        "browser_wait",
        json!({"condition": "selector_present", "selector": {"name": "Save"}}),
    );

    assert_eq!(waited.status, Status::Succeeded, "{}", waited.summary);
    assert_ne!(
        waited.facts["coverage"]["limited_by_size"], true,
        "{}",
        waited.facts
    );
    assert!(
        !waited.model_text().contains("size limit"),
        "{}",
        waited.model_text()
    );
}

/// An address embedded in the page, on a host of its own.
const EMBEDDED: &str = "https://embedded.example/inner";

/// A page that embeds [`EMBEDDED`]: the frames list both documents, the embedded document holds a
/// control named Inside, and a read there answers with the embedded document's own title and URL.
fn embedding_page() -> Fault {
    Box::new(|correlation, primitive| match primitive {
        wire::Primitive::Frames(_) => Some(FakeExtension::ok(
            correlation,
            json!({"frames": [
                {"frame_id": "T", "loader_id": "t1", "parent_frame_id": null, "url": URL},
                {"frame_id": "C", "loader_id": "c1", "parent_frame_id": "T", "url": EMBEDDED}],
                "complete": true, "unseen": []}),
        )),
        wire::Primitive::Call(call) => {
            let answers: Vec<Value> = call
                .calls
                .iter()
                .map(|world| {
                    let embedded = world
                        .frame
                        .as_ref()
                        .is_some_and(|frame| frame.frame_id == "C");
                    match (embedded, world.message["kind"].as_str()) {
                        (true, Some("inspect" | "find" | "describe" | "query_semantic")) => {
                            returned(json!({"targets": [{"locator": "locator_c1_1",
                                "role": "button", "name": "Inside", "state": [],
                                "credential_class": false}], "truncated": false}))
                        }
                        (true, Some("read_text")) => returned(json!({"text": "Inside",
                            "truncated": false, "title": "Embedded", "url": EMBEDDED})),
                        _ => page(world),
                    }
                })
                .collect();
            Some(FakeExtension::ok(correlation, json!({"answers": answers})))
        }
        _ => None,
    })
}

/// A read aimed at a control inside an embedded document is the tab's read: the landing check,
/// the host the agent reads and the durable permission trace all name the tab's own host.
#[test]
fn no_embedded_host_reaches_the_permission_trace_from_a_targeted_read() {
    let (executor, _fake, workspace, audit) =
        opened_audited(embedding_page(), GovernanceFacade::new(None));
    let inspected = execute(&executor, &workspace, "browser_inspect", json!({}));
    assert_eq!(inspected.status, Status::Succeeded, "{}", inspected.summary);
    let target = inspected.facts["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "Inside")
        .map(|item| item["target"].clone())
        .expect("the embedded control is listed");

    let read = execute(
        &executor,
        &workspace,
        "browser_read",
        json!({"target": target}),
    );

    assert_eq!(read.status, Status::Succeeded, "{}", read.summary);
    assert_eq!(
        read.facts["text"], "Inside",
        "the read ran in the embedded document"
    );
    let answered = serde_json::to_string(&read).unwrap();
    assert!(!answered.contains("embedded.example"), "{answered}");
    let traced = serde_json::to_string(
        &audit
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|record| record.permissions.clone())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(traced.contains("example.com"), "{traced}");
    assert!(
        !traced.contains("embedded.example"),
        "an embedded document's host never enters the durable audit: {traced}"
    );
}

/// The state of the scripted recording, as the extension reports it.
fn recording(state: &str) -> Value {
    json!({"recording_id": "recording_1", "tab_id": TAB, "state": state, "frame_count": 3,
        "bytes_held": 100, "duration_ms": 900, "source_urls": [URL],
        "source_urls_complete": true})
}

/// A save to Downloads writes a file in the browser, so when its receipt never comes before the
/// deadline the file may exist: the agent reads an unknown effect and is told to look first.
#[test]
fn a_download_save_that_outlives_its_deadline_is_unknown_and_not_repeat_safe() {
    let (executor, fake, workspace, _audit) = opened_replying(
        Box::new(|correlation, primitive| {
            let wire::Primitive::Recording(op) = primitive else {
                return None;
            };
            let effect = |value: Value| wire::Receipt {
                dispatched: true,
                ..FakeExtension::ok(correlation, value)
            };
            Some(match op {
                wire::RecordingOp::Start { .. } => Reply::Now(effect(json!({"result": "started",
                    "summary": recording("recording"), "existing": false}))),
                wire::RecordingOp::Stop { .. } => Reply::Now(effect(json!({"result": "stopped",
                    "summary": recording("frozen"), "changed": true}))),
                wire::RecordingOp::Export {
                    destination: wire::RecordingTarget::Download { .. },
                    ..
                } => Reply::Never,
                _ => return None,
            })
        }),
        GovernanceFacade::new(None),
    );
    let started = execute(
        &executor,
        &workspace,
        "browser_record",
        json!({"action": "start"}),
    );
    assert_eq!(started.status, Status::Succeeded, "{}", started.summary);

    let saved = execute(
        &executor,
        &workspace,
        "browser_record",
        json!({"action": "save", "download": true, "timeout_ms": 300}),
    );

    assert_eq!(saved.status, Status::Unknown, "{}", saved.summary);
    assert_eq!(saved.effect, Effect::Unknown, "{}", saved.summary);
    assert!(!saved.repeat_safe, "{}", saved.summary);
    assert_eq!(saved.facts["phase"], "after_dispatch", "{}", saved.facts);
    assert!(
        saved
            .summary
            .contains("before the browser confirmed what the sent command did"),
        "{}",
        saved.summary
    );
    assert!(fake
        .wait(|seen| !seen.cancels.is_empty())
        .requests
        .iter()
        .any(|(_, primitive)| matches!(
            primitive,
            wire::Primitive::Recording(wire::RecordingOp::Export {
                destination: wire::RecordingTarget::Download { .. },
                ..
            })
        )));
}
