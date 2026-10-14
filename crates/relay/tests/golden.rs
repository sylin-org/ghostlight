//! Binds the relay crate to the shared fixtures under `tests/golden/relay3`.
//!
//! The service and both connectors read the same fixtures, so these tests are what keeps the
//! three sides of relay protocol 3 saying the same thing.

use std::fs;
use std::path::PathBuf;

use ghostlight_relay::opening::{
    ControlOp, ControlResult, McpFrame, Opening, RefusalCode, Reply, Stream,
};
use ghostlight_relay::runtime::RuntimeFile;
use ghostlight_relay::templates::{Failure, FailureTemplates, DEFAULT_TEMPLATE};
use ghostlight_relay::{
    home, BACKOFF_FIRST, BACKOFF_MAX, HOLD_LIMIT, HOLD_LIMIT_BYTES, MAX_FRAME_BYTES,
    OPENING_DEADLINE, RELAY,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
        .join("relay3")
        .join(name);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The fixture decodes as `T` and encodes back to an equal value.
fn assert_round_trip<T: Serialize + DeserializeOwned>(name: &str) -> T {
    let original = fixture(name);
    let typed: T = serde_json::from_value(original.clone())
        .unwrap_or_else(|error| panic!("{name} does not decode: {error}"));
    let encoded = serde_json::to_value(&typed).unwrap();
    assert_eq!(encoded, original, "{name} does not re-encode to itself");
    typed
}

/// The fixture decodes as `T`.
fn assert_decodes<T: DeserializeOwned>(name: &str) -> T {
    serde_json::from_value(fixture(name))
        .unwrap_or_else(|error| panic!("{name} does not decode: {error}"))
}

fn names(vocabulary: &Value, key: &str) -> Vec<String> {
    vocabulary[key]
        .as_array()
        .unwrap_or_else(|| panic!("vocabulary.json has no list {key}"))
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn runtime_fixtures_round_trip() {
    let runtime: RuntimeFile = assert_round_trip("runtime.json");
    assert_eq!(runtime.relay, RELAY);
}

#[test]
fn runtime_extra_fields_decode_to_the_same_file() {
    let plain: RuntimeFile = assert_decodes("runtime.json");
    let extra: RuntimeFile = assert_decodes("runtime.extra_fields.json");
    assert_eq!(extra, plain);
}

#[test]
fn opening_fixtures_round_trip() {
    let mcp: Opening = assert_round_trip("opening.mcp.json");
    assert!(matches!(
        mcp.stream,
        Stream::Mcp {
            session: None,
            resume: None,
            ..
        }
    ));
    let resume: Opening = assert_round_trip("opening.mcp.resume.json");
    assert!(matches!(
        resume.stream,
        Stream::Mcp {
            resume: Some(_),
            ..
        }
    ));
    let cli: Opening = assert_round_trip("opening.cli.json");
    match cli.stream {
        Stream::Mcp {
            channel, session, ..
        } => {
            assert_eq!(channel, "cli");
            assert_eq!(session.unwrap()["kind"], json!("process"));
        }
        other => panic!("cli opening decoded as {other:?}"),
    }
    let browser: Opening = assert_round_trip("opening.browser.json");
    assert_eq!(browser.stream, Stream::Browser);
    for (file, op) in [
        (
            "opening.control.activate_workbench.json",
            ControlOp::ActivateWorkbench,
        ),
        ("opening.control.readiness.json", ControlOp::Readiness),
        ("opening.control.quit.json", ControlOp::Quit),
    ] {
        let control: Opening = assert_round_trip(file);
        assert_eq!(control.stream, Stream::Control { op });
    }
}

#[test]
fn reply_fixtures_round_trip() {
    let accepted: Reply = assert_round_trip("reply.accepted.json");
    assert!(matches!(accepted, Reply::Accepted { .. }));
    let refused: Reply = assert_round_trip("reply.refused.json");
    assert!(matches!(
        refused,
        Reply::Refused {
            code: RefusalCode::AuthenticationFailed,
            ..
        }
    ));
}

#[test]
fn mcp_and_control_fixtures_round_trip() {
    let message: McpFrame = assert_round_trip("mcp.message.json");
    assert!(matches!(message, McpFrame::Message { .. }));
    let resume: McpFrame = assert_round_trip("mcp.resume.json");
    assert!(matches!(resume, McpFrame::Resume { .. }));
    let result: ControlResult = assert_round_trip("control.result.readiness.json");
    assert_eq!(
        result,
        ControlResult::Result {
            value: json!({"ready": true})
        }
    );
}

#[test]
fn template_fixture_round_trips_and_resolves() {
    let templates: FailureTemplates = assert_round_trip("templates.json");
    for failure in [Failure::CutOff, Failure::Expired] {
        let call = templates.template(failure, "tools/call").unwrap();
        let fallback = templates.template(failure, "resources/list").unwrap();
        assert_eq!(call["id"], Value::Null);
        assert_eq!(fallback["id"], Value::Null);
        assert_ne!(call, fallback);
    }
}

#[test]
fn tolerance_fixtures_decode() {
    let expected: Opening = assert_decodes("opening.mcp.json");
    let extra: Opening = assert_decodes("tolerance/opening.extra_fields.json");
    assert_eq!(extra, expected);

    let unknown_stream: Opening = assert_decodes("tolerance/opening.unknown_stream_kind.json");
    assert_eq!(unknown_stream.stream, Stream::Unknown);
    let unknown_op: Opening = assert_decodes("tolerance/opening.unknown_control_op.json");
    assert_eq!(
        unknown_op.stream,
        Stream::Control {
            op: ControlOp::Unknown
        }
    );

    let accepted: Reply = assert_decodes("tolerance/reply.accepted.extra_fields.json");
    assert!(matches!(accepted, Reply::Accepted { .. }));
    let refused: Reply = assert_decodes("tolerance/reply.refused.extra_fields.json");
    assert!(matches!(
        refused,
        Reply::Refused {
            code: RefusalCode::Capacity,
            ..
        }
    ));

    let unknown_reply: Reply = assert_decodes("tolerance/reply.unknown_kind.json");
    assert_eq!(unknown_reply, Reply::Unknown);
    let unknown_code: Reply = assert_decodes("tolerance/reply.refused.unknown_code.json");
    assert!(matches!(
        unknown_code,
        Reply::Refused {
            code: RefusalCode::Unknown,
            ..
        }
    ));

    let unknown_frame: McpFrame = assert_decodes("tolerance/mcp.unknown_kind.json");
    assert_eq!(unknown_frame, McpFrame::Unknown);
    let message: McpFrame = assert_decodes("tolerance/mcp.message.extra_fields.json");
    assert!(matches!(message, McpFrame::Message { .. }));

    let result: ControlResult = assert_decodes("tolerance/control.result.extra_fields.json");
    assert_eq!(
        result,
        ControlResult::Result {
            value: json!({"ready": true})
        }
    );

    let templates: FailureTemplates = assert_decodes("tolerance/templates.extra_fields.json");
    assert!(templates.template(Failure::CutOff, "tools/call").is_some());
    assert!(templates.template(Failure::Expired, "tools/call").is_some());
}

#[test]
fn constants_equal_the_vocabulary() {
    let vocabulary = fixture("vocabulary.json");
    assert_eq!(vocabulary["relay"], json!(RELAY));
    assert_eq!(vocabulary["frame"]["max_bytes"], json!(MAX_FRAME_BYTES));
    assert_eq!(vocabulary["hold_limit_bytes"], json!(HOLD_LIMIT_BYTES));
    assert_eq!(vocabulary["hold_limit_ms"], json!(HOLD_LIMIT.as_millis()));
    assert_eq!(
        vocabulary["backoff_first_ms"],
        json!(BACKOFF_FIRST.as_millis())
    );
    assert_eq!(vocabulary["backoff_max_ms"], json!(BACKOFF_MAX.as_millis()));
    assert_eq!(
        vocabulary["opening_deadline_ms"],
        json!(OPENING_DEADLINE.as_millis())
    );
    assert_eq!(
        vocabulary["failure_template_default"],
        json!(DEFAULT_TEMPLATE)
    );
    assert_eq!(
        vocabulary["test_home_variable"],
        json!(home::TEST_HOME_VARIABLE)
    );

    let files = &vocabulary["files"];
    assert_eq!(files["home_directory"], json!(home::HOME_DIRECTORY_NAME));
    assert_eq!(files["runtime"], json!(home::RUNTIME_FILE_NAME));
    assert_eq!(files["lease"], json!(home::LEASE_FILE_NAME));
    assert_eq!(files["templates"], json!(home::TEMPLATES_FILE_NAME));
    assert_eq!(files["bin"], json!(home::BIN_DIRECTORY_NAME));
    assert_eq!(
        files["diagnostics_marker"],
        json!(home::DIAGNOSTICS_MARKER_NAME)
    );
    assert_eq!(
        files["diagnostics_directory"],
        json!(home::DIAGNOSTICS_DIRECTORY_NAME)
    );
    let platform = if cfg!(windows) { "windows" } else { "other" };
    assert_eq!(
        vocabulary["service_executable"][platform],
        json!(home::SERVICE_EXECUTABLE_NAME)
    );
}

#[test]
fn closed_name_lists_equal_the_vocabulary() {
    let vocabulary = fixture("vocabulary.json");

    let streams = [
        Stream::Mcp {
            channel: String::new(),
            session: None,
            resume: None,
        },
        Stream::Browser,
        Stream::Control {
            op: ControlOp::Readiness,
        },
    ];
    let stream_names: Vec<String> = streams
        .iter()
        .map(|stream| {
            serde_json::to_value(stream).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(stream_names, names(&vocabulary, "streams"));

    let ops = [
        ControlOp::ActivateWorkbench,
        ControlOp::Readiness,
        ControlOp::Quit,
    ];
    let op_names: Vec<String> = ops
        .iter()
        .map(|op| {
            serde_json::to_value(op)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(op_names, names(&vocabulary, "control_ops"));

    let replies = [
        Reply::Accepted {
            service_version: String::new(),
        },
        Reply::Refused {
            code: RefusalCode::Malformed,
            message: String::new(),
        },
    ];
    let reply_names: Vec<String> = replies
        .iter()
        .map(|reply| {
            serde_json::to_value(reply).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(reply_names, names(&vocabulary, "reply_kinds"));

    let codes = [
        RefusalCode::AuthenticationFailed,
        RefusalCode::Unsupported,
        RefusalCode::Capacity,
        RefusalCode::ShuttingDown,
        RefusalCode::Malformed,
    ];
    let code_names: Vec<String> = codes
        .iter()
        .map(|code| {
            serde_json::to_value(code)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(code_names, names(&vocabulary, "refusal_codes"));

    let frames = [
        McpFrame::Message {
            line: String::new(),
        },
        McpFrame::Resume {
            token: String::new(),
        },
    ];
    let frame_names: Vec<String> = frames
        .iter()
        .map(|frame| {
            serde_json::to_value(frame).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(frame_names, names(&vocabulary, "mcp_frame_kinds"));

    let result = serde_json::to_value(ControlResult::Result { value: Value::Null }).unwrap();
    assert_eq!(result["kind"], vocabulary["control_result_kind"]);
}
