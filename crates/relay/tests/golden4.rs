//! Binds the relay crate to the shared fixtures under `tests/golden/relay4`.
//!
//! These fixtures are the executable form of `docs/design/relay-protocol-4.md`. The service and
//! every edge read the same files, so these tests are what keeps them saying the same thing. They
//! record revision 4, what a 2.0.0 edge sends and the service still admits. Revision 5 sends the
//! same frames under its own number; `golden5.rs` holds what it adds.

use std::fs;
use std::path::{Path, PathBuf};

use ghostlight_relay::endpoint::{fnv1a32, home_hash, Endpoint};
use ghostlight_relay::protocol::{
    goodbye_frame, goodbye_in, Control, ControlResult, Edge, Failure, GoodbyeReason, McpFrame,
    Opening, Program, QuitReason, RefusalCode, Reply, Role, StartReason, CLI_CHANNEL,
    DEFAULT_TEMPLATE, MCP_CHANNEL,
};
use ghostlight_relay::{
    home, BACKOFF_FIRST, BACKOFF_MAX, HOLD_LIMIT, HOLD_LIMIT_BYTES, MAX_FRAME_BYTES,
    OPENING_DEADLINE,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

/// The revision these fixtures record.
const FIXTURE_REVISION: u16 = 4;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = golden_dir().join("relay4").join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn fixture(name: &str) -> Value {
    serde_json::from_slice(&fixture_bytes(name))
        .unwrap_or_else(|error| panic!("{name} is not JSON: {error}"))
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

/// The wire names of closed words that serialize as strings.
fn wire_names<T: Serialize>(words: &[T]) -> Vec<String> {
    words
        .iter()
        .map(|word| {
            serde_json::to_value(word)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

/// The `kind` or `op` tag of tagged values.
fn tags<T: Serialize>(values: &[T], tag: &str) -> Vec<String> {
    values
        .iter()
        .map(|value| {
            serde_json::to_value(value).unwrap()[tag]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn edge(program: Program) -> Edge {
    Edge {
        program,
        revision: FIXTURE_REVISION,
        build: "2.0.0".to_owned(),
    }
}

#[test]
fn opening_fixtures_round_trip() {
    let mcp: Opening = assert_round_trip("opening.mcp.json");
    assert_eq!(mcp.relay, FIXTURE_REVISION);
    assert_eq!(mcp.role, Role::Mcp);
    assert_eq!(mcp.edge, edge(Program::Edge));
    assert_eq!(mcp.reason, Some(StartReason::Agent));
    assert_eq!(mcp.channel.as_deref(), Some(MCP_CHANNEL));
    assert!(mcp.resume.is_some());

    let cli: Opening = assert_round_trip("opening.cli.json");
    assert_eq!(cli.role, Role::Mcp);
    assert_eq!(cli.channel.as_deref(), Some(CLI_CHANNEL));
    assert_eq!(cli.session.unwrap()["kind"], json!("process"));
    assert_eq!(cli.resume, None);

    let browser: Opening = assert_round_trip("opening.browser.json");
    assert_eq!(browser.role, Role::Browser);
    assert_eq!(browser.reason, Some(StartReason::Browser));
    assert!(browser.origin.unwrap().starts_with("chrome-extension://"));

    for (file, control) in [
        ("opening.control.readiness.json", Control::Readiness),
        ("opening.control.status.json", Control::Status),
        ("opening.control.show.json", Control::Show),
        (
            "opening.control.quit.quit.json",
            Control::Quit {
                reason: QuitReason::Quit,
            },
        ),
        (
            "opening.control.quit.replace.json",
            Control::Quit {
                reason: QuitReason::Replace,
            },
        ),
        (
            "opening.control.quit.uninstall.json",
            Control::Quit {
                reason: QuitReason::Uninstall,
            },
        ),
    ] {
        let opening: Opening = assert_round_trip(file);
        assert_eq!(opening.role, Role::Control, "{file}");
        assert_eq!(opening.control, Some(control), "{file}");
    }

    let desktop: Opening = assert_round_trip("opening.desktop.json");
    assert_eq!(desktop.role, Role::Desktop);
    assert_eq!(desktop.edge.program, Program::Desktop);
    let tray: Opening = assert_round_trip("opening.desktop.tray.json");
    assert_eq!(tray.role, Role::Desktop);
    assert_eq!(tray.channel.as_deref(), Some("tray"));
    let command: Opening = assert_round_trip("opening.command.json");
    assert_eq!(command.role, Role::Command);
    assert_eq!(command.reason, Some(StartReason::Command));
}

#[test]
fn reply_fixtures_round_trip() {
    let accepted: Reply = assert_round_trip("reply.accepted.json");
    assert!(accepted.accepted);
    assert_eq!(accepted.service.as_deref(), Some("2.0.0"));
    assert!(accepted.instance.is_some());
    assert_eq!(accepted.code, None);
    let templates = accepted.templates.unwrap();
    for failure in [Failure::CutOff, Failure::Expired] {
        let call = templates.template(failure, "tools/call").unwrap();
        let fallback = templates.template(failure, "resources/list").unwrap();
        assert_eq!(call["id"], Value::Null);
        assert_eq!(fallback["id"], Value::Null);
        assert_ne!(call, fallback);
    }

    let refused: Reply = assert_round_trip("reply.refused.json");
    assert!(!refused.accepted);
    assert_eq!(refused.code, Some(RefusalCode::Unsupported));
    assert!(refused.say.is_some());
    assert_eq!(refused.templates, None);
}

#[test]
fn mcp_and_control_fixtures_round_trip() {
    let message: McpFrame = assert_round_trip("mcp.message.json");
    assert!(matches!(message, McpFrame::Message { .. }));
    let resume: McpFrame = assert_round_trip("mcp.resume.json");
    assert!(matches!(resume, McpFrame::Resume { .. }));
    let goodbye: McpFrame = assert_round_trip("mcp.goodbye.json");
    assert_eq!(
        goodbye,
        McpFrame::Goodbye {
            reason: GoodbyeReason::Replace
        }
    );
    let result: ControlResult = assert_round_trip("control.result.status.json");
    assert_eq!(
        result,
        ControlResult::Result {
            value: json!({
                "service": "2.0.0",
                "instance": "9f3a61c2d0b84e17",
                "pid": 18244,
                "started_by": "agent"
            })
        }
    );
}

#[test]
fn the_browser_goodbye_is_the_exact_frame_text() {
    let text = fixture_bytes("browser.goodbye.txt");
    assert_eq!(goodbye_frame(GoodbyeReason::Replace), text);
    assert_eq!(goodbye_in(&text), Some(GoodbyeReason::Replace));
    for reason in [
        GoodbyeReason::Quit,
        GoodbyeReason::Replace,
        GoodbyeReason::Uninstall,
        GoodbyeReason::SessionEnd,
    ] {
        assert_eq!(goodbye_in(&goodbye_frame(reason)), Some(reason));
    }
    assert_eq!(
        goodbye_in(&fixture_bytes(
            "tolerance/browser.goodbye.unknown_reason.txt"
        )),
        Some(GoodbyeReason::Unknown)
    );
}

#[test]
fn no_adapter_frame_is_a_goodbye() {
    let frames = golden_dir().join("protocol4").join("frames");
    let mut seen = 0;
    for entry in fs::read_dir(&frames).unwrap() {
        let path = entry.unwrap().path();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(goodbye_in(&bytes), None, "{}", path.display());
        seen += 1;
    }
    assert!(seen > 0, "no adapter frames found");
}

#[test]
fn tolerance_fixtures_decode() {
    let expected: Opening = assert_decodes("opening.mcp.json");
    let extra: Opening = assert_decodes("tolerance/opening.extra_fields.json");
    assert_eq!(extra, expected);

    let role: Opening = assert_decodes("tolerance/opening.unknown_role.json");
    assert_eq!(role.role, Role::Unknown);
    let reason: Opening = assert_decodes("tolerance/opening.unknown_reason.json");
    assert_eq!(reason.reason, Some(StartReason::Unknown));
    let program: Opening = assert_decodes("tolerance/opening.unknown_program.json");
    assert_eq!(program.edge.program, Program::Unknown);
    let op: Opening = assert_decodes("tolerance/opening.unknown_control_op.json");
    assert_eq!(op.control, Some(Control::Unknown));
    let quit: Opening = assert_decodes("tolerance/opening.unknown_quit_reason.json");
    assert_eq!(
        quit.control,
        Some(Control::Quit {
            reason: QuitReason::Unknown
        })
    );

    let accepted: Reply = assert_decodes("tolerance/reply.accepted.extra_fields.json");
    assert!(accepted.accepted);
    assert!(accepted.templates.is_some());
    let refused: Reply = assert_decodes("tolerance/reply.refused.unknown_code.json");
    assert_eq!(refused.code, Some(RefusalCode::Unknown));

    let frame: McpFrame = assert_decodes("tolerance/mcp.unknown_kind.json");
    assert_eq!(frame, McpFrame::Unknown);
    let message: McpFrame = assert_decodes("tolerance/mcp.message.extra_fields.json");
    assert!(matches!(message, McpFrame::Message { .. }));
    let goodbye: McpFrame = assert_decodes("tolerance/mcp.goodbye.unknown_reason.json");
    assert_eq!(
        goodbye,
        McpFrame::Goodbye {
            reason: GoodbyeReason::Unknown
        }
    );
    let result: ControlResult = assert_decodes("tolerance/control.result.extra_fields.json");
    assert_eq!(
        result,
        ControlResult::Result {
            value: json!({"ready": true})
        }
    );
}

#[test]
fn constants_equal_the_vocabulary() {
    let vocabulary = fixture("vocabulary.json");
    assert_eq!(vocabulary["relay"], json!(FIXTURE_REVISION));
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
    assert_eq!(files["run_directory"], json!(home::RUN_DIRECTORY_NAME));
    assert_eq!(files["relay_socket"], json!(home::RELAY_SOCKET_NAME));
    assert_eq!(files["service_lock"], json!(home::SERVICE_LOCK_NAME));
}

#[test]
fn closed_name_lists_equal_the_vocabulary() {
    let vocabulary = fixture("vocabulary.json");

    let roles = [
        Role::Mcp,
        Role::Browser,
        Role::Control,
        Role::Desktop,
        Role::Command,
    ];
    assert_eq!(wire_names(&roles), names(&vocabulary, "roles"));

    let starts = [
        StartReason::Agent,
        StartReason::Browser,
        StartReason::Command,
        StartReason::Person,
        StartReason::Recover,
        StartReason::Update,
    ];
    assert_eq!(wire_names(&starts), names(&vocabulary, "start_reasons"));
    for reason in starts {
        assert_eq!(StartReason::parse(reason.as_str()), Some(reason));
    }
    assert_eq!(StartReason::parse("scheduled"), None);

    let goodbyes = [
        GoodbyeReason::Quit,
        GoodbyeReason::Replace,
        GoodbyeReason::Uninstall,
        GoodbyeReason::SessionEnd,
    ];
    assert_eq!(wire_names(&goodbyes), names(&vocabulary, "goodbye_reasons"));
    for reason in goodbyes {
        assert_eq!(GoodbyeReason::parse(reason.as_str()), reason);
    }

    let quits = [QuitReason::Quit, QuitReason::Replace, QuitReason::Uninstall];
    assert_eq!(wire_names(&quits), names(&vocabulary, "quit_reasons"));

    let programs = [Program::Edge, Program::Service, Program::Desktop];
    assert_eq!(wire_names(&programs), names(&vocabulary, "edge_programs"));

    let ops = [
        Control::Readiness,
        Control::Status,
        Control::Show,
        Control::Quit {
            reason: QuitReason::Quit,
        },
    ];
    assert_eq!(tags(&ops, "op"), names(&vocabulary, "control_ops"));

    let codes = [
        RefusalCode::Unsupported,
        RefusalCode::Capacity,
        RefusalCode::ShuttingDown,
        RefusalCode::Malformed,
    ];
    assert_eq!(wire_names(&codes), names(&vocabulary, "refusal_codes"));

    let frames = [
        McpFrame::Message {
            line: String::new(),
        },
        McpFrame::Resume {
            token: String::new(),
        },
        McpFrame::Goodbye {
            reason: GoodbyeReason::Quit,
        },
    ];
    assert_eq!(tags(&frames, "kind"), names(&vocabulary, "mcp_frame_kinds"));

    let result = serde_json::to_value(ControlResult::Result { value: Value::Null }).unwrap();
    assert_eq!(result["kind"], vocabulary["control_result_kind"]);

    let goodbye = String::from_utf8(goodbye_frame(GoodbyeReason::Quit)).unwrap();
    let wire = &vocabulary["browser_goodbye"];
    assert_eq!(
        goodbye,
        format!(
            "{}quit{}",
            wire["prefix"].as_str().unwrap(),
            wire["suffix"].as_str().unwrap()
        )
    );
}

#[test]
fn the_worked_endpoint_name_is_the_one_the_code_derives() {
    let vocabulary = fixture("vocabulary.json");
    let windows = &vocabulary["endpoint"]["windows"];
    let example = &windows["example"];
    let home = Path::new(example["home"].as_str().unwrap());

    assert_eq!(
        example["lowercased_home"],
        json!(home.to_str().unwrap().to_ascii_lowercase())
    );
    let hash = fnv1a32(example["lowercased_home"].as_str().unwrap().as_bytes());
    assert_eq!(example["fnv1a32"], json!(hash));
    assert_eq!(example["home_hash"], json!(format!("{hash:08x}")));
    assert_eq!(home_hash(home), example["home_hash"].as_str().unwrap());

    let sid = example["user_sid"].as_str().unwrap();
    let name = example["name"].as_str().unwrap();
    assert_eq!(
        name,
        format!(
            "{}{sid}-{}",
            windows["pipe_prefix"].as_str().unwrap(),
            example["home_hash"].as_str().unwrap()
        )
    );
    assert_eq!(
        Endpoint::named_pipe(sid, home),
        Endpoint::NamedPipe(name.to_owned())
    );
}

#[test]
fn the_unix_endpoint_is_the_socket_under_the_run_directory() {
    let vocabulary = fixture("vocabulary.json");
    let files = &vocabulary["files"];
    let home = Path::new("/home/ada/.ghostlight");
    let expected = home
        .join(files["run_directory"].as_str().unwrap())
        .join(files["relay_socket"].as_str().unwrap());
    assert_eq!(Endpoint::unix_socket(home), Endpoint::UnixSocket(expected));
}
