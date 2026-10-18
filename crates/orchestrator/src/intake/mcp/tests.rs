use std::collections::BTreeMap;
use std::path::Path;

use crate::intake::session::{
    ServiceContent, ServiceRequest, ServiceResponse, ToolAnnotations, ToolDefinition,
};
use crate::work::InvocationId;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ghostlight_relay::protocol::{Failure, Templates};
use serde_json::{json, Value};

use super::{
    failure_templates, negotiate, McpSession, OwnedTab, ResumeError, ResumeState, Step,
    LATEST_REVISION, RESUME_TOKEN_PREFIX, SUPPORTED_REVISIONS,
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

fn ready_session() -> McpSession {
    let mut session = McpSession::new();
    session.accept(&initialize_line(json!(1), LATEST_REVISION));
    session.accept(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    session
}

fn invoke_id(steps: &[Step]) -> InvocationId {
    match forwards(steps).as_slice() {
        [ServiceRequest::Invoke { id, .. }] => id.clone(),
        other => panic!("expected one invoke, got {other:?}"),
    }
}

fn tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: "A tool.".into(),
        input_schema: json!({"type": "object"}),
        annotations: Some(ToolAnnotations {
            read_only_hint: Some(true),
            ..ToolAnnotations::default()
        }),
    }
}

#[test]
fn negotiation_counteroffers_the_latest_revision_for_an_unknown_one() {
    let mut session = McpSession::new();
    let reply = replies(session.accept(&initialize_line(json!(1), "future-revision")));
    let result = &reply[0]["result"];
    assert_eq!(result["protocolVersion"], LATEST_REVISION);
    assert_eq!(result["capabilities"]["tools"]["listChanged"], true);
    assert_eq!(result["serverInfo"]["name"], "ghostlight");
    assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["instructions"], crate::language::SERVER_INSTRUCTIONS);
    assert_eq!(session.client_label(), "test client");
}

#[test]
fn negotiation_echoes_every_served_revision() {
    for requested in SUPPORTED_REVISIONS {
        assert_eq!(negotiate(requested), requested);
        let mut session = McpSession::new();
        let reply = replies(session.accept(&initialize_line(json!(7), requested)));
        assert_eq!(reply[0]["result"]["protocolVersion"], requested);
    }
}

#[test]
fn discovery_then_initialize_serves_one_session() {
    let mut session = McpSession::new();
    let discovery = json!({
        "jsonrpc": "2.0",
        "id": "d",
        "method": "server/discover",
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {"name": "Antigravity", "version": "1"},
            "io.modelcontextprotocol/clientCapabilities": {}
        }}
    });
    let reply = replies(session.accept(&discovery.to_string()));
    let result = &reply[0]["result"];
    assert_eq!(reply[0]["id"], "d");
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["supportedVersions"], json!(SUPPORTED_REVISIONS));
    assert_eq!(result["cacheScope"], "private");
    assert_eq!(result["ttlMs"], 0);
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "ghostlight"
    );
    assert_eq!(session.client_label(), "Antigravity");
    assert!(!session.is_initialized());

    let reply = replies(session.accept(&initialize_line(json!(2), "2025-06-18")));
    assert_eq!(reply[0]["result"]["protocolVersion"], "2025-06-18");
    assert!(session.is_initialized());
}

#[test]
fn discovery_for_another_revision_is_invalid_params() {
    let mut session = McpSession::new();
    let reply = replies(session.accept(
        r#"{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2025-11-25","io.modelcontextprotocol/clientCapabilities":{}}}}"#,
    ));
    assert_eq!(reply[0]["error"]["code"], -32602);
}

#[test]
fn a_request_before_initialize_is_refused_and_ping_is_answered() {
    let mut session = McpSession::new();
    let steps = session.accept(r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#);
    assert!(forwards(&steps).is_empty());
    let reply = replies(steps);
    assert_eq!(reply[0]["error"]["code"], -32002);
    assert_eq!(reply[0]["id"], 3);

    let reply = replies(session.accept(r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#));
    assert_eq!(reply[0], json!({"jsonrpc": "2.0", "id": 4, "result": {}}));
}

#[test]
fn a_line_that_is_not_json_is_a_parse_error_with_a_null_id() {
    let mut session = ready_session();
    let reply = replies(session.accept("{not json"));
    assert_eq!(reply[0]["error"]["code"], -32700);
    assert_eq!(reply[0]["id"], Value::Null);
    assert!(reply[0]["error"]["message"]
        .as_str()
        .unwrap()
        .starts_with("Parse error: "));
}

#[test]
fn an_unknown_method_is_not_found_and_an_unknown_notification_is_ignored() {
    let mut session = ready_session();
    let reply = replies(session.accept(r#"{"jsonrpc":"2.0","id":9,"method":"resources/list"}"#));
    assert_eq!(reply[0]["error"]["code"], -32601);
    assert!(session
        .accept(r#"{"jsonrpc":"2.0","method":"notifications/roots/list_changed"}"#)
        .is_empty());
}

#[test]
fn a_batch_of_tools_list_and_ping_returns_one_array() {
    let mut session = ready_session();
    let steps = session.accept(
        r#"[{"jsonrpc":"2.0","id":1,"method":"tools/list"},{"jsonrpc":"2.0","id":"p","method":"ping"}]"#,
    );
    assert_eq!(forwards(&steps), vec![ServiceRequest::Catalog]);
    assert!(replies(steps).is_empty());

    let lines = session.render(ServiceResponse::Catalog {
        tools: vec![tool("browser_read")],
    });
    assert_eq!(lines.len(), 1);
    let batch = lines[0].as_array().expect("one array line");
    assert_eq!(batch.len(), 2);
    let ids: Vec<&Value> = batch.iter().map(|reply| &reply["id"]).collect();
    assert!(ids.contains(&&json!(1)));
    assert!(ids.contains(&&json!("p")));
}

#[test]
fn a_batch_without_forwards_replies_at_once_and_an_empty_batch_is_invalid() {
    let mut session = ready_session();
    let reply = replies(session.accept(
        r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"}]"#,
    ));
    assert_eq!(reply.len(), 1);
    assert_eq!(reply[0].as_array().unwrap().len(), 1);

    let reply = replies(session.accept("[]"));
    assert_eq!(reply[0]["error"]["code"], -32600);
    assert_eq!(reply[0]["id"], Value::Null);
}

#[test]
fn tool_results_in_a_batch_go_out_one_by_one() {
    let mut session = ready_session();
    let steps = session.accept(
        r#"[{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"browser_read"}},{"jsonrpc":"2.0","id":2,"method":"ping"}]"#,
    );
    let invocation = invoke_id(&steps);
    let reply = replies(steps);
    assert_eq!(reply.len(), 1);
    assert_eq!(reply[0].as_array().unwrap()[0]["id"], 2);

    let lines = session.render(ServiceResponse::Result {
        id: invocation,
        text: "Read the page.".into(),
        result: json!({"status": "succeeded"}),
        is_error: false,
        content: Vec::new(),
    });
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["id"], 1);
}

#[test]
fn string_and_numeric_ids_echo_with_their_type() {
    let mut session = ready_session();
    let numeric = invoke_id(&session.accept(
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"browser_read","arguments":{}}}"#,
    ));
    let textual = invoke_id(&session.accept(
        r#"{"jsonrpc":"2.0","id":"5","method":"tools/call","params":{"name":"browser_read","arguments":{}}}"#,
    ));
    assert_ne!(numeric, textual);

    let render = |session: &mut McpSession, id: InvocationId| {
        session.render(ServiceResponse::Result {
            id,
            text: "Done.".into(),
            result: json!({}),
            is_error: false,
            content: Vec::new(),
        })
    };
    assert_eq!(render(&mut session, textual)[0]["id"], json!("5"));
    assert_eq!(render(&mut session, numeric)[0]["id"], json!(5));
}

#[test]
fn tools_call_without_a_name_or_with_non_object_arguments_is_invalid_params() {
    let mut session = ready_session();
    let reply = replies(
        session
            .accept(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"arguments":{}}}"#),
    );
    assert_eq!(reply[0]["error"]["code"], -32602);
    let reply = replies(session.accept(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"browser_read","arguments":[1]}}"#,
    ));
    assert_eq!(reply[0]["error"]["code"], -32602);
}

#[test]
fn cancelled_maps_to_cancel_of_the_right_invocation() {
    let mut session = ready_session();
    let first = invoke_id(&session.accept(
        r#"{"jsonrpc":"2.0","id":"a","method":"tools/call","params":{"name":"browser_read"}}"#,
    ));
    let second = invoke_id(&session.accept(
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"browser_click"}}"#,
    ));
    let steps = session
        .accept(r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":7}}"#);
    assert_ne!(first, second);
    assert_eq!(
        forwards(&steps),
        vec![ServiceRequest::Cancel { id: second }]
    );

    let unknown = session.accept(
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"7"}}"#,
    );
    assert!(unknown.is_empty());
}

#[test]
fn a_running_client_id_is_refused_and_is_free_again_once_answered() {
    let mut session = ready_session();
    let line = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"browser_read"}}"#;
    let first = invoke_id(&session.accept(line));
    let steps = session.accept(line);
    assert!(forwards(&steps).is_empty());
    assert_eq!(replies(steps)[0]["error"]["code"], -32600);

    session.render(ServiceResponse::Result {
        id: first.clone(),
        text: "Read the page.".into(),
        result: json!({}),
        is_error: false,
        content: Vec::new(),
    });
    assert_ne!(invoke_id(&session.accept(line)), first);
}

#[test]
fn a_tool_result_carries_the_envelope_once_with_images_and_the_error_flag() {
    let mut session = ready_session();
    let id = invoke_id(&session.accept(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"browser_screenshot"}}"#,
    ));
    let envelope = json!({"status": "refused", "facts": {"view": "view_1"}});
    let lines = session.render(ServiceResponse::Result {
        id,
        text: "Refused the capture. Next: Read the page.".into(),
        result: envelope.clone(),
        is_error: true,
        content: vec![ServiceContent::Image {
            mime_type: "image/jpeg".into(),
            data: "base64-image".into(),
        }],
    });
    let result = &lines[0]["result"];
    assert_eq!(result["content"][0]["type"], "text");
    assert_eq!(
        result["content"][0]["text"],
        "Refused the capture. Next: Read the page."
    );
    assert!(!result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("view_1"));
    assert_eq!(result["content"][1]["type"], "image");
    assert_eq!(result["content"][1]["mimeType"], "image/jpeg");
    assert_eq!(result["content"][1]["data"], "base64-image");
    assert_eq!(result["structuredContent"], envelope);
    assert_eq!(result["isError"], true);
}

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
            let expected = format!(
                "Captured the view.

Result: {lean}"
            );
            assert_eq!(text, expected, "{revision}");
        } else {
            assert_eq!(text, "Captured the view.", "{revision}");
        }
        assert_eq!(result["content"][1]["type"], "image");
    }
}

#[test]
fn a_service_result_loses_its_defaults_in_structured_content() {
    let mut session = ready_session();
    let id = invoke_id(&session.accept(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"browser_dialog"}}"#,
    ));
    let lines = session.render(ServiceResponse::Result {
        id,
        text: "Accepted the browser dialog.".into(),
        result: json!({
            "status": "succeeded", "effect": "applied", "repeat_safe": false,
            "summary": "Accepted the browser dialog.", "facts": {"tab": "tab_q3k7"},
            "history_storage": "saved", "readiness": "not_applicable", "next_steps": [],
        }),
        is_error: false,
        content: Vec::new(),
    });
    assert_eq!(
        lines[0]["result"]["structuredContent"],
        json!({
            "status": "succeeded", "effect": "applied", "repeat_safe": false,
            "summary": "Accepted the browser dialog.", "facts": {"tab": "tab_q3k7"},
        })
    );
}

/// The failure templates as the JSON an edge receives.
fn templates_json() -> Value {
    serde_json::to_value(failure_templates()).unwrap()
}

#[test]
fn every_failure_template_says_history_was_not_confirmed_and_is_already_lean() {
    let templates = templates_json();
    for failure in ["cut_off", "expired"] {
        let envelope = &templates[failure]["tools/call"]["result"]["structuredContent"];
        assert_eq!(envelope["history_storage"], "unconfirmed");
        assert_eq!(&super::lean::project(envelope), envelope);
    }
}

#[test]
fn a_service_error_for_a_call_is_an_internal_error_naming_its_code() {
    let mut session = ready_session();
    let id = invoke_id(&session.accept(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"browser_read"}}"#,
    ));
    let lines = session.render(ServiceResponse::Error {
        id: Some(id.clone()),
        code: "duplicate_request".into(),
        message: "Request id is already active.".into(),
    });
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[0]["error"]["code"], -32603);
    assert_eq!(
        lines[0]["error"]["message"],
        "duplicate_request: Request id is already active."
    );
    assert!(session
        .render(ServiceResponse::Error {
            id: Some(id),
            code: "late".into(),
            message: "Again.".into(),
        })
        .is_empty());
}

#[test]
fn tools_list_carries_no_output_schema() {
    let mut session = ready_session();
    let steps = session.accept(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    assert_eq!(forwards(&steps), vec![ServiceRequest::Catalog]);
    let lines = session.render(ServiceResponse::Catalog {
        tools: vec![tool("browser_read"), tool("browser_click")],
    });
    let tools = lines[0]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    for listed in tools {
        assert!(listed.get("outputSchema").is_none());
        assert_eq!(listed["inputSchema"]["type"], "object");
        assert_eq!(listed["annotations"]["readOnlyHint"], true);
    }
}

#[test]
fn a_catalog_change_is_announced_only_once_initialized() {
    let changed = || ServiceResponse::CatalogChanged {
        generation: 1,
        tools: Vec::new(),
    };
    let mut session = McpSession::new();
    session.accept(&initialize_line(json!(1), LATEST_REVISION));
    assert!(session.render(changed()).is_empty());
    session.accept(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    assert_eq!(
        session.render(changed()),
        vec![json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"})]
    );
}

#[test]
fn a_second_initialize_is_refused() {
    let mut session = ready_session();
    let reply = replies(session.accept(&initialize_line(json!(2), LATEST_REVISION)));
    assert_eq!(reply[0]["error"]["code"], -32600);
}

#[test]
fn failure_templates_are_json_rpc_responses_with_null_ids_and_ascii_text() {
    let templates = templates_json();
    for failure in ["cut_off", "expired"] {
        let call = &templates[failure]["tools/call"];
        assert_eq!(call["jsonrpc"], "2.0");
        assert_eq!(call["id"], Value::Null);
        assert_eq!(call["result"]["isError"], true);
        let text = call["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.is_ascii());
        assert!(call["result"]["structuredContent"]["repeat_safe"].is_boolean());

        let default = &templates[failure]["default"];
        assert_eq!(default["jsonrpc"], "2.0");
        assert_eq!(default["id"], Value::Null);
        assert_eq!(default["error"]["code"], -32603);
        assert!(default["error"]["message"].as_str().unwrap().is_ascii());
    }
    let cut_off = &templates["cut_off"]["tools/call"]["result"]["structuredContent"];
    assert_eq!(cut_off["status"], "unknown");
    assert_eq!(cut_off["effect"], "unknown");
    assert_eq!(cut_off["repeat_safe"], false);
    let expired = &templates["expired"]["tools/call"]["result"]["structuredContent"];
    assert_eq!(expired["effect"], "none");
}

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

#[test]
fn resume_state_round_trips_through_the_token() {
    let state = ready_session()
        .resume_state(vec![OwnedTab {
            browser_id: "browser_a".into(),
            tab_id: 42,
            handle: "tab_1".into(),
        }])
        .unwrap();
    let token = state.encode();
    assert!(token.is_ascii());
    let decoded = ResumeState::decode(&token).unwrap();
    assert_eq!(decoded, state);

    let resumed = McpSession::resumed(decoded);
    assert!(resumed.is_initialized());
    assert_eq!(resumed.client_label(), "test client");
    assert_eq!(
        resumed.resume_state(Vec::new()).unwrap().revision,
        LATEST_REVISION
    );
}

#[test]
fn a_resumed_session_serves_requests_and_catalog_notices_at_once() {
    let state = ready_session().resume_state(Vec::new()).unwrap();
    let mut resumed = McpSession::resumed(state);
    let steps = resumed.accept(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
    assert_eq!(forwards(&steps), vec![ServiceRequest::Catalog]);
    assert_eq!(
        resumed
            .render(ServiceResponse::CatalogChanged {
                generation: 2,
                tools: Vec::new(),
            })
            .len(),
        1
    );
}

#[test]
fn a_session_before_initialize_has_no_resume_state() {
    assert!(McpSession::new().resume_state(Vec::new()).is_none());
}

/// A resume token an earlier build handed out. A published token keeps reading.
const EARLIER_RESUME_TOKEN: &str = "rs1.eyJyZXZpc2lvbiI6IjIwMjUtMDYtMTgifQ";

#[test]
fn a_resume_token_an_earlier_build_handed_out_decodes() {
    let state = ResumeState::decode(EARLIER_RESUME_TOKEN).expect("the token decodes");
    assert_eq!(state.revision, "2025-06-18");
    let session = McpSession::resumed(state);
    assert!(session.is_initialized());
    assert!(!session.client_label().is_empty());
}

#[test]
fn a_resume_token_written_with_the_retired_fields_still_resumes() {
    let written = json!({
        "revision": "2025-06-18",
        "client_label": "returning client",
        "channel": "cli",
        "session_key": "workspace_earlier",
        "owned_tabs": [{"browser_id": "browser_a", "tab_id": 42, "handle": "tab_1"}],
        "recording": "recording_1"
    });
    let token = format!(
        "{RESUME_TOKEN_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(written.to_string())
    );
    let state = ResumeState::decode(&token).expect("the token decodes");
    assert_eq!(
        state.owned_tabs,
        vec![OwnedTab {
            browser_id: "browser_a".into(),
            tab_id: 42,
            handle: "tab_1".into(),
        }]
    );

    let mut resumed = McpSession::resumed(state);
    assert!(resumed.is_initialized());
    assert_eq!(resumed.client_label(), "returning client");
    let steps = resumed.accept(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
    assert_eq!(forwards(&steps), vec![ServiceRequest::Catalog]);

    let rewritten = resumed.resume_state(Vec::new()).unwrap().encode();
    let body = URL_SAFE_NO_PAD
        .decode(rewritten.strip_prefix(RESUME_TOKEN_PREFIX).unwrap())
        .unwrap();
    let rewritten: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        rewritten,
        json!({"revision": "2025-06-18", "client_label": "returning client", "owned_tabs": []})
    );
}

#[test]
fn a_malformed_token_is_rejected() {
    assert_eq!(ResumeState::decode("nope"), Err(ResumeError::Format));
    assert_eq!(ResumeState::decode("rs1.***"), Err(ResumeError::Encoding));
    assert_eq!(ResumeState::decode("rs1.W10"), Err(ResumeError::Content));
    let unsupported = ResumeState {
        revision: "1999-01-01".into(),
        client_label: "c".into(),
        owned_tabs: Vec::new(),
    };
    assert_eq!(
        ResumeState::decode(&unsupported.encode()),
        Err(ResumeError::Revision)
    );
    let long = format!("rs1.{}", "A".repeat(70_000));
    assert_eq!(ResumeState::decode(&long), Err(ResumeError::TooLong));
}
