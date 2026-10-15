//! Tests of the relay protocol 4 vocabulary: the opening exchange over a connected pair, the
//! browser goodbye, and the templates.

use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use super::*;
use crate::endpoint::pair;
use crate::frame::{read_frame, write_frame};

/// How long a test waits for something that should happen at once.
const PATIENCE: Duration = Duration::from_secs(10);

fn edge() -> Edge {
    Edge::new(Program::Edge, "2.0.0".to_owned())
}

fn accepted() -> Reply {
    Reply::accepted(
        "2.0.0".to_owned(),
        "i1".to_owned(),
        Some(Templates::default()),
    )
}

#[test]
fn a_new_opening_carries_this_revision_and_writes_only_what_is_set() {
    let opening = Opening::new(Role::Browser, edge());
    assert_eq!(opening.relay, crate::RELAY);
    assert_eq!(
        serde_json::to_value(&opening).unwrap(),
        json!({
            "relay": 4,
            "role": "browser",
            "edge": {"program": "edge", "revision": 4, "build": "2.0.0"},
        })
    );
}

#[test]
fn a_control_opening_names_its_operation_and_the_quit_reason() {
    let mut opening = Opening::new(Role::Control, edge());
    opening.control = Some(Control::Quit {
        reason: QuitReason::Replace,
    });
    assert_eq!(
        serde_json::to_value(&opening).unwrap()["control"],
        json!({"op": "quit", "reason": "replace"})
    );
    let decoded: Opening = serde_json::from_value(serde_json::to_value(&opening).unwrap()).unwrap();
    assert_eq!(decoded, opening);
}

#[test]
fn the_two_reply_shapes_write_only_their_own_fields() {
    assert_eq!(
        serde_json::to_value(accepted()).unwrap(),
        json!({
            "accepted": true,
            "service": "2.0.0",
            "instance": "i1",
            "templates": {"cut_off": {}, "expired": {}},
        })
    );
    assert_eq!(
        serde_json::to_value(Reply::refused(RefusalCode::Capacity, "Busy.".to_owned())).unwrap(),
        json!({"accepted": false, "code": "capacity", "say": "Busy."})
    );
}

#[test]
fn unknown_words_decode_to_unknown_and_unknown_fields_are_ignored() {
    let role: Role = serde_json::from_value(json!("telemetry")).unwrap();
    assert_eq!(role, Role::Unknown);
    let reason: StartReason = serde_json::from_value(json!("scheduled")).unwrap();
    assert_eq!(reason, StartReason::Unknown);
    let goodbye: GoodbyeReason = serde_json::from_value(json!("maintenance")).unwrap();
    assert_eq!(goodbye, GoodbyeReason::Unknown);
    let quit: QuitReason = serde_json::from_value(json!("crash")).unwrap();
    assert_eq!(quit, QuitReason::Unknown);
    let program: Program = serde_json::from_value(json!("browser")).unwrap();
    assert_eq!(program, Program::Unknown);
    let code: RefusalCode = serde_json::from_value(json!("installing")).unwrap();
    assert_eq!(code, RefusalCode::Unknown);
    let control: Control = serde_json::from_value(json!({"op": "reboot", "x": 1})).unwrap();
    assert_eq!(control, Control::Unknown);
    let frame: McpFrame = serde_json::from_value(json!({"kind": "ping"})).unwrap();
    assert_eq!(frame, McpFrame::Unknown);
    let reply: Reply =
        serde_json::from_value(json!({"accepted": true, "service": "9", "later": [1]})).unwrap();
    assert!(reply.accepted);
}

#[test]
fn a_words_wire_name_is_its_as_str() {
    for reason in [
        StartReason::Agent,
        StartReason::Browser,
        StartReason::Command,
        StartReason::Person,
        StartReason::Recover,
        StartReason::Update,
    ] {
        assert_eq!(
            serde_json::to_value(reason).unwrap(),
            json!(reason.as_str())
        );
    }
    for reason in [
        GoodbyeReason::Quit,
        GoodbyeReason::Replace,
        GoodbyeReason::Uninstall,
        GoodbyeReason::SessionEnd,
    ] {
        assert_eq!(
            serde_json::to_value(reason).unwrap(),
            json!(reason.as_str())
        );
    }
}

#[test]
fn only_the_six_start_reasons_parse() {
    assert_eq!(StartReason::parse("recover"), Some(StartReason::Recover));
    assert_eq!(StartReason::parse("unknown"), None);
    assert_eq!(StartReason::parse("Agent"), None);
    assert_eq!(StartReason::parse(""), None);
}

#[test]
fn a_goodbye_frame_is_the_fixed_text_and_reads_back() {
    assert_eq!(
        goodbye_frame(GoodbyeReason::SessionEnd),
        br#"{"goodbye":"session_end"}"#
    );
    assert_eq!(
        goodbye_in(br#"{"goodbye":"uninstall"}"#),
        Some(GoodbyeReason::Uninstall)
    );
}

#[test]
fn goodbye_in_looks_only_at_frames_with_the_fixed_prefix() {
    assert_eq!(goodbye_in(b""), None);
    assert_eq!(goodbye_in(b"{}"), None);
    assert_eq!(goodbye_in(br#"{"kind":"heartbeat"}"#), None);
    assert_eq!(goodbye_in(br#"{"kind":"notice","goodbye":"quit"}"#), None);
    assert_eq!(goodbye_in(br#"{ "goodbye": "quit" }"#), None);
    assert_eq!(goodbye_in(br#"{"goodbye":"quit"#), None);
    assert_eq!(goodbye_in(br#"{"goodbye":"quit" "#), None);
    assert_eq!(goodbye_in(b"{\"goodbye\":\"\xff\"}"), None);
    assert_eq!(
        goodbye_in(br#"{"goodbye":"quit","later":1}"#),
        Some(GoodbyeReason::Quit)
    );
    assert_eq!(
        goodbye_in(br#"{"goodbye":"someday"}"#),
        Some(GoodbyeReason::Unknown)
    );
}

#[test]
fn open_writes_the_opening_and_returns_the_reply() {
    let (client, mut service) = pair().unwrap();
    let mut opening = Opening::new(Role::Mcp, edge());
    opening.channel = Some("mcp".to_owned());
    let expected = opening.clone();
    let server = thread::spawn(move || {
        let received: Opening = read_frame(&mut service).unwrap().unwrap();
        write_frame(&mut service, &accepted()).unwrap();
        (received, service)
    });
    let reply = open(&client, &opening, Instant::now() + PATIENCE).unwrap();
    assert_eq!(reply, accepted());
    let (received, mut service) = server.join().unwrap();
    assert_eq!(received, expected);

    // The connection is left with no timeouts: a later frame arrives however long it takes.
    let mut client = client;
    write_frame(&mut service, &json!({"later": true})).unwrap();
    let later: Value = read_frame(&mut client).unwrap().unwrap();
    assert_eq!(later, json!({"later": true}));
}

#[test]
fn open_returns_a_refusal_like_any_other_reply() {
    let (client, mut service) = pair().unwrap();
    let server = thread::spawn(move || {
        let _: Opening = read_frame(&mut service).unwrap().unwrap();
        let refusal = Reply::refused(RefusalCode::Unsupported, "Update Ghostlight.".to_owned());
        write_frame(&mut service, &refusal).unwrap();
    });
    let opening = Opening::new(Role::Desktop, edge());
    let reply = open(&client, &opening, Instant::now() + PATIENCE).unwrap();
    server.join().unwrap();
    assert!(!reply.accepted);
    assert_eq!(reply.code, Some(RefusalCode::Unsupported));
    assert_eq!(reply.say.as_deref(), Some("Update Ghostlight."));
}

#[test]
fn open_reports_a_service_that_never_answers_as_the_deadline() {
    let (client, _service) = pair().unwrap();
    let opening = Opening::new(Role::Browser, edge());
    let started = Instant::now();
    let failure = open(
        &client,
        &opening,
        Instant::now() + Duration::from_millis(150),
    )
    .unwrap_err();
    assert!(matches!(failure, ConnectError::Deadline), "{failure:?}");
    assert!(started.elapsed() < PATIENCE);
}

#[test]
fn open_with_a_deadline_already_past_is_the_deadline() {
    let (client, _service) = pair().unwrap();
    let opening = Opening::new(Role::Browser, edge());
    let failure = open(&client, &opening, Instant::now()).unwrap_err();
    assert!(matches!(failure, ConnectError::Deadline), "{failure:?}");
}

#[test]
fn open_reports_a_service_that_closes_without_replying() {
    let (client, mut service) = pair().unwrap();
    let server = thread::spawn(move || {
        let _: Opening = read_frame(&mut service).unwrap().unwrap();
        service.shutdown();
    });
    let opening = Opening::new(Role::Mcp, edge());
    let failure = open(&client, &opening, Instant::now() + PATIENCE).unwrap_err();
    server.join().unwrap();
    assert!(matches!(failure, ConnectError::Closed), "{failure:?}");
}

#[test]
fn open_reports_an_unreadable_reply_as_invalid_data() {
    let (client, mut service) = pair().unwrap();
    let server = thread::spawn(move || {
        let _: Opening = read_frame(&mut service).unwrap().unwrap();
        crate::frame::write_frame_raw(&mut service, b"not json").unwrap();
        service
    });
    let opening = Opening::new(Role::Mcp, edge());
    let failure = open(&client, &opening, Instant::now() + PATIENCE).unwrap_err();
    let _service = server.join().unwrap();
    match failure {
        ConnectError::Io(error) => assert_eq!(error.kind(), io::ErrorKind::InvalidData),
        other => panic!("{other:?}"),
    }
}

fn sample_templates() -> Templates {
    serde_json::from_value(json!({
        "cut_off": {
            "tools/call": {"jsonrpc": "2.0", "id": null, "result": {"isError": true}},
            "default": {"jsonrpc": "2.0", "id": null, "error": {"code": -32603, "message": "cut"}},
        },
        "expired": {
            "default": {"jsonrpc": "2.0", "id": null, "error": {"code": -32603, "message": "late"}},
        },
    }))
    .unwrap()
}

#[test]
fn a_method_template_wins_over_the_default() {
    let templates = sample_templates();
    let call = templates.template(Failure::CutOff, "tools/call").unwrap();
    assert_eq!(call["result"], json!({"isError": true}));
    let other = templates.template(Failure::CutOff, "ping").unwrap();
    assert_eq!(other["error"]["message"], json!("cut"));
    let late = templates.template(Failure::Expired, "tools/call").unwrap();
    assert_eq!(late["error"]["message"], json!("late"));
}

#[test]
fn no_template_for_a_failure_is_none() {
    assert_eq!(Templates::default().template(Failure::CutOff, "ping"), None);
}

#[test]
fn fill_keeps_the_json_type_of_the_id_and_the_rest_of_the_template() {
    let template = json!({"jsonrpc": "2.0", "id": null, "result": {"isError": true}});
    assert_eq!(fill(&template, &json!(7))["id"], json!(7));
    assert_eq!(fill(&template, &json!("7"))["id"], json!("7"));
    assert_eq!(fill(&template, &json!(null))["id"], json!(null));
    let filled = fill(&template, &json!(1));
    assert_eq!(filled["result"], json!({"isError": true}));
    assert_eq!(filled["jsonrpc"], json!("2.0"));
}

#[test]
fn a_non_object_template_fills_as_the_bare_error() {
    assert_eq!(fill(&json!("text"), &json!(1)), bare_error(&json!(1)));
    assert_eq!(
        bare_error(&json!(3)),
        json!({"jsonrpc": "2.0", "id": 3, "error": {"code": -32603, "message": ""}})
    );
}

#[test]
fn a_failure_answers_with_the_filled_template_or_the_bare_error() {
    let templates = sample_templates();
    let response = failure_response(Some(&templates), Failure::CutOff, "tools/call", &json!(5));
    assert_eq!(response["result"], json!({"isError": true}));
    assert_eq!(response["id"], json!(5));
    let response = failure_response(Some(&templates), Failure::Expired, "ping", &json!(6));
    assert_eq!(response["error"]["message"], json!("late"));
    let empty = Templates::default();
    assert_eq!(
        failure_response(Some(&empty), Failure::CutOff, "ping", &json!(7)),
        bare_error(&json!(7))
    );
    assert_eq!(
        failure_response(None, Failure::CutOff, "ping", &json!(8)),
        bare_error(&json!(8))
    );
}

#[test]
fn connect_dials_the_home_and_opens_in_one_step() {
    let home = std::env::temp_dir().join(format!(
        "ghostlight-protocol-connect-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&home).unwrap();
    let listener = crate::endpoint::Listener::bind_first(&home).unwrap();
    let server = thread::spawn(move || {
        let mut service = listener.accept().unwrap();
        let received: Opening = read_frame(&mut service).unwrap().unwrap();
        write_frame(&mut service, &accepted()).unwrap();
        (received, service)
    });
    let opening = Opening::new(Role::Control, edge());
    let (_connection, reply) = connect(&home, &opening, Instant::now() + PATIENCE).unwrap();
    let (received, _service) = server.join().unwrap();
    assert_eq!(reply, accepted());
    assert_eq!(received, opening);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn connect_reports_no_service_when_nothing_listens() {
    let home = std::env::temp_dir().join(format!(
        "ghostlight-protocol-connect-none-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&home).unwrap();
    let opening = Opening::new(Role::Control, edge());
    let failure = connect(&home, &opening, Instant::now() + PATIENCE).unwrap_err();
    assert!(matches!(failure, ConnectError::NoService), "{failure:?}");
    let _ = std::fs::remove_dir_all(&home);
}
