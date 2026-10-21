use std::collections::BTreeMap;
use std::path::Path;

use crate::intake::session::{ServiceContent, ServiceRequest, ServiceResponse};
use crate::language::audit_health::Storage;
use crate::work::result::{Effect, Status};
use crate::work::InvocationId;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ghostlight_relay::protocol::{Failure, Templates};
use serde_json::{json, Value};

use super::{
    failure_templates, negotiate, McpSession, OwnedTab, ResumeState, Step, LATEST_REVISION,
    OLDEST_REVISION, RESUME_TOKEN_PREFIX, SUPPORTED_REVISIONS, TEXT_RESULT_LABEL,
};

fn initialize_line(id: Value, revision: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": revision,
            "clientInfo": {"name": "test client", "version": "1"},
            "capabilities": {}
        }
    })
    .to_string()
}

fn replies(steps: Vec<Step>) -> Vec<Value> {
    steps
        .into_iter()
        .filter_map(|step| match step {
            Step::Reply(value) => Some(value),
            Step::Forward(_) => None,
        })
        .collect()
}

fn forwards(steps: &[Step]) -> Vec<ServiceRequest> {
    steps
        .iter()
        .filter_map(|step| match step {
            Step::Forward(request) => Some(request.clone()),
            Step::Reply(_) => None,
        })
        .collect()
}

fn invoke_id(steps: &[Step]) -> InvocationId {
    match forwards(steps).as_slice() {
        [ServiceRequest::Invoke { id, .. }] => id.clone(),
        other => panic!("expected one invoke, got {other:?}"),
    }
}

/// The failure templates as the JSON an edge receives.
fn templates_json() -> Value {
    serde_json::to_value(failure_templates()).unwrap()
}

/// ACCEPTANCE C9: the service alone negotiates the MCP revision. `initialize` echoes every
/// revision this engine serves and counteroffers the latest for any other, as MCP requires, so a
/// client that asks for a revision this engine does not know still connects.
#[test]
fn initialize_echoes_a_served_revision_and_counteroffers_the_latest_otherwise() {
    for requested in SUPPORTED_REVISIONS {
        assert_eq!(negotiate(requested), requested);
        let mut session = McpSession::new();
        let reply = replies(session.accept(&initialize_line(json!(7), requested)));
        assert_eq!(reply[0]["result"]["protocolVersion"], requested);
    }
    let mut session = McpSession::new();
    let reply = replies(session.accept(&initialize_line(json!(1), "future-revision")));
    assert_eq!(reply[0]["result"]["protocolVersion"], LATEST_REVISION);
}

/// ACCEPTANCE C21 (ADR-0198 Decision 24): a client of a revision before 2025-06-18 cannot read
/// `structuredContent`, so its text block also carries the same lean JSON after the result label,
/// while a client of 2025-06-18 or later gets the summary alone. An image stays its own block. The
/// edge's own answers are written before any revision is negotiated, so they carry the text copy
/// for every client (defect S-D6).
#[test]
fn older_revisions_get_the_lean_json_in_the_text_block() {
    for revision in SUPPORTED_REVISIONS {
        let mut session = McpSession::new();
        session.accept(&initialize_line(json!(1), revision));
        session.accept(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        let id = invoke_id(&session.accept(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"browser_screenshot"}}"#,
        ));
        let lines = session.render(ServiceResponse::Result {
            id,
            text: "Captured the view.".into(),
            result: json!({"status": "succeeded", "readiness": "not_applicable", "facts": {"view": "view_1"}}),
            is_error: false,
            content: vec![ServiceContent::Image {
                mime_type: "image/jpeg".into(),
                data: "base64-image".into(),
            }],
        });
        let result = &lines[0]["result"];
        let text = result["content"][0]["text"].as_str().unwrap();
        let lean = &result["structuredContent"];
        assert_eq!(
            lean,
            &json!({"status": "succeeded", "facts": {"view": "view_1"}})
        );
        if revision < "2025-06-18" {
            let expected = format!("Captured the view.{TEXT_RESULT_LABEL}{lean}");
            assert_eq!(text, expected, "{revision}");
        } else {
            assert_eq!(text, "Captured the view.", "{revision}");
        }
        assert_eq!(result["content"][1]["type"], "image");
    }
    let templates = failure_templates();
    for failure in [Failure::CutOff, Failure::Expired] {
        let answer = templates
            .template(failure, "tools/call")
            .expect("a template");
        let result = &answer["result"];
        let text = result["content"][0]["text"].as_str().unwrap();
        let lean = &result["structuredContent"];
        assert!(
            text.ends_with(&format!("{TEXT_RESULT_LABEL}{lean}")),
            "{failure:?}: {text}"
        );
    }
}

/// ACCEPTANCE C10 and E4, and AH1 (ADR-0159): the answers the edge gives in the service's place
/// when it cannot reach it tell the truth, already in the model's lean view. A call cut off by a
/// restart may have taken effect, so its status and effect are unknown and repeating it is not
/// safe; a call the service never started had no effect and is safe to repeat; and neither says
/// its history was saved.
#[test]
fn a_cut_off_call_may_have_happened_and_an_expired_one_did_not() {
    let templates = templates_json();
    for failure in ["cut_off", "expired"] {
        let result = &templates[failure]["tools/call"]["result"];
        assert_eq!(result["isError"], true, "{failure}");
        let envelope = &result["structuredContent"];
        assert_eq!(
            envelope["history_storage"],
            json!(Storage::Unconfirmed),
            "{failure}"
        );
        assert_eq!(
            &super::lean::project(envelope),
            envelope,
            "{failure}: the edge hands it on as it is, so it is already lean"
        );
    }
    let cut_off = &templates["cut_off"]["tools/call"]["result"]["structuredContent"];
    assert_eq!(cut_off["status"], json!(Status::Unknown));
    assert_eq!(cut_off["effect"], json!(Effect::Unknown));
    assert_eq!(cut_off["repeat_safe"], false);
    let expired = &templates["expired"]["tools/call"]["result"]["structuredContent"];
    assert_eq!(expired["effect"], json!(Effect::None));
    assert_eq!(expired["repeat_safe"], true);
}

/// Contract: the failure templates the service hands every `mcp` stream have the shape of the
/// accepted-reply golden (tests/golden/relay4/reply.accepted.json), the templates the edge's own
/// tests answer with (crates/edge/src/forward/tests.rs): a result for `tools/call`, an error with
/// the golden's code for any other method, and the same template keys.
#[test]
fn failure_templates_have_the_relay_fixture_shape() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/relay4/reply.accepted.json");
    let text = std::fs::read_to_string(fixture).expect("relay 4 accepted reply fixture");
    let fixture: Value = serde_json::from_str(&text).unwrap();
    let pinned: Templates = serde_json::from_value(fixture["templates"].clone()).unwrap();
    let ours = failure_templates();
    for failure in [Failure::CutOff, Failure::Expired] {
        for method in ["tools/call", "tools/list"] {
            let ours = ours.template(failure, method).expect("a template");
            let pinned = pinned.template(failure, method).expect("a pinned template");
            assert_eq!(ours.get("result").is_some(), pinned.get("result").is_some());
            assert_eq!(ours["error"]["code"], pinned["error"]["code"]);
        }
    }
    let keys = |templates: &Templates| {
        let names = |entries: &BTreeMap<String, Value>| entries.keys().cloned().collect::<Vec<_>>();
        (names(&templates.cut_off), names(&templates.expired))
    };
    assert_eq!(keys(&ours), keys(&pinned));
}

/// A resume token an earlier build handed out. A published token keeps reading.
const EARLIER_RESUME_TOKEN: &str = "rs1.eyJyZXZpc2lvbiI6IjIwMjUtMDYtMTgifQ";

/// Defect ADR-0203 C13 (c78d4bd3, "the opener is named once"): a client that sends no name, or
/// only blank and direction text, claims nothing before and after a resume, so it is the unknown
/// agent and reuses no kept tab; a token's empty name is no name either, and a token written again
/// names no client.
#[test]
fn an_empty_name_and_no_name_stay_the_same_unknown_agent_after_a_resume() {
    use crate::agent::Agent;
    use crate::intake::session::IntakeChannel;
    use crate::provenance::{Attribution, ConnectionEvidence};

    let unnamed = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {"protocolVersion": LATEST_REVISION, "capabilities": {}}
    });
    let mut empty = unnamed.clone();
    empty["params"]["clientInfo"] = json!({"name": " \u{200f} ", "version": "1"});
    for line in [unnamed, empty] {
        let mut session = McpSession::new();
        replies(session.accept(&line.to_string()));
        let token = session
            .resume_state(Vec::new(), BTreeMap::new())
            .unwrap()
            .encode();
        let resumed = McpSession::resumed(ResumeState::decode(&token).unwrap());
        for claim in [session.claim(), resumed.claim()] {
            assert_eq!(claim, None, "{line}");
            let evidence =
                ConnectionEvidence::new(Attribution::accepted(IntakeChannel::Mcp), claim);
            assert_eq!(evidence.details().agent, Agent::unknown(), "{line}");
            assert_eq!(evidence.reuse_key(), None, "{line}");
        }
    }
    let written = json!({"revision": "2025-06-18", "client_label": ""});
    let token = format!(
        "{RESUME_TOKEN_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(written.to_string())
    );
    let resumed = McpSession::resumed(ResumeState::decode(&token).unwrap());
    assert_eq!(resumed.claim(), None, "a token's empty name is no name");
    let rewritten = resumed
        .resume_state(Vec::new(), BTreeMap::new())
        .unwrap()
        .encode();
    let body = URL_SAFE_NO_PAD
        .decode(rewritten.strip_prefix(RESUME_TOKEN_PREFIX).unwrap())
        .unwrap();
    let rewritten: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        rewritten,
        json!({"revision": "2025-06-18", "owned_tabs": []})
    );
}

/// Contract (a replace hands the next service the tokens the last one wrote, ACCEPTANCE B27) and
/// ADR-0203 C11 (8c216920): every token another build wrote continues an initialized session,
/// since its client will not initialize again. A token an earlier build handed out reads; one
/// with fields this build retired keeps its claim and its tabs and is written back without them;
/// one at a revision this engine does not serve keeps its claim and tabs and continues at the
/// oldest served revision; and one this build cannot read at all continues there with no tab.
#[test]
fn a_token_another_build_wrote_continues_an_initialized_session() {
    let serves = |session: &mut McpSession, case: &str| {
        assert!(session.is_initialized(), "{case}");
        let steps = session.accept(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
        assert_eq!(forwards(&steps), vec![ServiceRequest::Catalog], "{case}");
    };

    let earlier = ResumeState::decode(EARLIER_RESUME_TOKEN).expect("the token decodes");
    assert_eq!(earlier.revision, "2025-06-18");
    let mut session = McpSession::resumed(earlier);
    assert_eq!(session.claim(), None, "the token names no client");
    serves(&mut session, "an earlier build's token");

    let retired = json!({
        "revision": "2025-06-18",
        "client_label": "returning client",
        "channel": "cli",
        "session_key": "workspace_earlier",
        "owned_tabs": [{"browser_id": "browser_a", "tab_id": 42, "handle": "tab_1"}],
        "recording": "recording_1"
    });
    let token = format!(
        "{RESUME_TOKEN_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(retired.to_string())
    );
    let state = ResumeState::decode(&token).expect("the token decodes");
    assert_eq!(
        state.owned_tabs,
        vec![OwnedTab {
            browser_id: "browser_a".into(),
            tab_id: 42,
            handle: "tab_1".into(),
            url_digest: None,
        }]
    );
    let mut session = McpSession::resumed(state);
    assert_eq!(session.claim(), Some("returning client"));
    serves(&mut session, "a token with retired fields");
    let rewritten = session
        .resume_state(Vec::new(), BTreeMap::new())
        .unwrap()
        .encode();
    let body = URL_SAFE_NO_PAD
        .decode(rewritten.strip_prefix(RESUME_TOKEN_PREFIX).unwrap())
        .unwrap();
    let rewritten: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        rewritten,
        json!({"revision": "2025-06-18", "client_label": "returning client", "owned_tabs": []})
    );

    let unknown = ResumeState {
        revision: "1999-01-01".into(),
        claim: Some("returning client".into()),
        owned_tabs: vec![OwnedTab {
            browser_id: "browser_a".into(),
            tab_id: 42,
            handle: "tab_1".into(),
            url_digest: Some("digest".into()),
        }],
        lives: BTreeMap::new(),
        standby: false,
    };
    let decoded = ResumeState::decode(&unknown.encode()).expect("the token decodes");
    assert_eq!(decoded, unknown, "the claim and the tabs are kept");
    let mut session = McpSession::resumed(decoded);
    assert_eq!(session.claim(), Some("returning client"));
    assert_eq!(
        session
            .resume_state(Vec::new(), BTreeMap::new())
            .unwrap()
            .revision,
        OLDEST_REVISION
    );
    serves(&mut session, "a revision this engine does not serve");

    // What the service resumes for a token it cannot read (service/mod.rs).
    let too_long = format!("{RESUME_TOKEN_PREFIX}{}", "A".repeat(70_000));
    for unreadable in ["nope", "rs1.***", "rs1.W10", too_long.as_str()] {
        assert!(ResumeState::decode(unreadable).is_err(), "{unreadable:.20}");
        let mut session = McpSession::resumed(
            ResumeState::decode(unreadable).unwrap_or_else(|_| ResumeState::unreadable()),
        );
        assert_eq!(session.claim(), None);
        let state = session.resume_state(Vec::new(), BTreeMap::new()).unwrap();
        assert_eq!(state.revision, OLDEST_REVISION);
        assert!(state.owned_tabs.is_empty());
        serves(&mut session, "a token this build cannot read");
    }
}

/// Defect S-D5: a client that batches gets JSON-RPC's answer, every response in the batch's one
/// array, its tool results included, written once the last of them is answered. A tool result
/// sent outside the array can be dropped by a strict client, whose agent then waits for it.
#[test]
fn a_batch_is_answered_as_one_array_with_its_tool_results_in_it() {
    let mut session = McpSession::new();
    session.accept(&initialize_line(json!(0), "2025-03-26"));
    session.accept(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    let steps = session.accept(
        r#"[{"jsonrpc":"2.0","id":1,"method":"tools/list"},{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"browser_tabs","arguments":{"action":"list"}}},{"jsonrpc":"2.0","id":3,"method":"ping"}]"#,
    );
    assert!(
        steps.iter().all(|step| matches!(step, Step::Forward(_))),
        "nothing is answered before the batch is"
    );
    let invocation = forwards(&steps)
        .into_iter()
        .find_map(|request| match request {
            ServiceRequest::Invoke { id, .. } => Some(id),
            _ => None,
        })
        .expect("the call is forwarded");
    assert!(
        session
            .render(ServiceResponse::Catalog { tools: Vec::new() })
            .is_empty(),
        "the batch waits for its call"
    );
    let lines = session.render(ServiceResponse::Result {
        id: invocation,
        text: "Listed 0 tabs.".into(),
        result: json!({"status": "succeeded"}),
        is_error: false,
        content: Vec::new(),
    });
    assert_eq!(lines.len(), 1, "one line answers the batch");
    let answers = lines[0].as_array().expect("the batch's one array");
    let mut ids: Vec<i64> = answers
        .iter()
        .map(|answer| answer["id"].as_i64().unwrap())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, [1, 2, 3]);
    assert!(answers
        .iter()
        .any(|answer| answer["id"] == 2 && answer["result"]["content"].is_array()));
}
