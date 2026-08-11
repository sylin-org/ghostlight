//! The command-line intake: one local edge for scripts and programs.
//!
//! This is an edge in the same shape as the MCP connector (ADR-0105 Decision 1). It owns argument
//! parsing, rendering, and exit codes, and makes no product decision: every call crosses the same
//! service bridge, executor, governance facade, and completion path a model's call does.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ghostlight_bridge::client::{ClientError, ServiceClient};
use ghostlight_bridge::service::{IntakeChannel, ServiceContent};
use serde_json::Value;

/// The label controlled tabs are grouped under for scripted work.
///
/// One stable label per channel, not one per calling program: the tab strip has a fixed budget of
/// attention, so a script's tabs group together instead of multiplying groups (ADR-0105).
const CLI_CLIENT_LABEL: &str = "ghostlight call";

/// Terminal status to process exit code.
///
/// Zero means the browser did what was asked. Every other outcome is distinguishable, and an
/// uncertain effect is never zero, because a script must not treat "cannot be determined" as done.
const EXIT_SUCCEEDED: i32 = 0;
const EXIT_USAGE: i32 = 1;
const EXIT_BLOCKED: i32 = 2;
const EXIT_ATTENTION: i32 = 3;
const EXIT_FAILED: i32 = 4;
const EXIT_CANCELLED: i32 = 5;
const EXIT_UNKNOWN: i32 = 6;

/// How a caller wants results rendered, and where bounded content should land.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Rendering {
    /// Print the whole terminal result rather than its sentence.
    pub json: bool,
    /// Where to write bounded content, such as a screenshot's image bytes.
    ///
    /// Without this an image is reported as omitted: a script asking for a capture wants a file,
    /// not a megabyte of base64 in its terminal.
    pub output: Option<PathBuf>,
}

/// What the caller asked the command line to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// Invoke one tool with one input.
    Call {
        /// Exact catalog tool name.
        tool: String,
        /// Opaque JSON input, validated by the orchestrator.
        input: String,
        /// How to render the result.
        rendering: Rendering,
    },
    /// Read `<tool> <json>` lines from standard input over one session.
    ///
    /// Handles belong to a session, so multi-step scripted work needs one process. Reading a line
    /// at a time lets a caller use a handle the previous line returned, and a tool that takes an
    /// optional tab resolves the session's only tab without one.
    Batch {
        /// How to render each result.
        rendering: Rendering,
    },
    /// List the catalog this service offers.
    Catalog,
}

/// Parse the `call` subcommand's arguments.
pub fn parse(arguments: &[String]) -> Result<Command> {
    let mut rendering = Rendering::default();
    let mut positional: Vec<String> = Vec::new();
    let mut catalog = false;
    let mut batch = false;
    let mut remaining = arguments.iter();
    while let Some(argument) = remaining.next() {
        match argument.as_str() {
            "--json" => rendering.json = true,
            "--stdin" => batch = true,
            "--catalog" => catalog = true,
            "--output" => {
                let path = remaining
                    .next()
                    .ok_or_else(|| anyhow!("--output needs a file path"))?;
                rendering.output = Some(PathBuf::from(path));
            }
            other if other.starts_with("--output=") => {
                rendering.output = Some(PathBuf::from(&other["--output=".len()..]));
            }
            other if other.starts_with("--") => {
                return Err(anyhow!("unknown option {other}"));
            }
            other => positional.push(other.to_owned()),
        }
    }
    if catalog {
        return Ok(Command::Catalog);
    }
    if batch {
        return Ok(Command::Batch { rendering });
    }
    match positional.as_slice() {
        [tool] => Ok(Command::Call {
            tool: tool.clone(),
            input: "{}".into(),
            rendering,
        }),
        [tool, input] => Ok(Command::Call {
            tool: tool.clone(),
            input: input.clone(),
            rendering,
        }),
        [] => Err(anyhow!(
            "ghostlight call <tool> [json] [--json] [--output <file>]"
        )),
        _ => Err(anyhow!("one tool and at most one JSON input per call")),
    }
}

/// Run one command against the local service and return the process exit code.
pub fn run(command: Command, runtime_file: &Path, out: &mut impl Write) -> i32 {
    let mut client =
        match ServiceClient::connect(runtime_file, CLI_CLIENT_LABEL, IntakeChannel::Cli) {
            Ok(client) => client,
            Err(error) => return report_transport(&error),
        };
    match command {
        Command::Catalog => match client.catalog() {
            Ok(tools) => {
                for tool in tools {
                    let _ = writeln!(out, "{}", tool.name);
                }
                EXIT_SUCCEEDED
            }
            Err(error) => report_transport(&error),
        },
        Command::Call {
            tool,
            input,
            rendering,
        } => {
            let Ok(input) = serde_json::from_str::<Value>(&input) else {
                eprintln!("the input is not valid JSON");
                return EXIT_USAGE;
            };
            invoke_once(
                &mut client,
                &tool,
                input,
                &rendering,
                &mut Captures::default(),
                out,
            )
        }
        Command::Batch { rendering } => {
            let stdin = std::io::stdin();
            let mut worst = EXIT_SUCCEEDED;
            let mut captures = Captures::default();
            for line in stdin.lock().lines() {
                let Ok(line) = line else { return EXIT_USAGE };
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let (tool, rest) = line.split_once(char::is_whitespace).unwrap_or((line, "{}"));
                let Ok(input) = serde_json::from_str::<Value>(rest.trim()) else {
                    eprintln!("the input is not valid JSON");
                    return EXIT_USAGE;
                };
                let code = invoke_once(&mut client, tool, input, &rendering, &mut captures, out);
                if code != EXIT_SUCCEEDED {
                    worst = code;
                }
            }
            worst
        }
    }
}

/// How many content items a session has already written, so a batch does not overwrite itself.
#[derive(Debug, Default)]
struct Captures(usize);

fn invoke_once(
    client: &mut ServiceClient,
    tool: &str,
    input: Value,
    rendering: &Rendering,
    captures: &mut Captures,
    out: &mut impl Write,
) -> i32 {
    match client.invoke(tool, input, None) {
        Ok(invocation) => {
            for item in &invocation.content {
                match rendering.output.as_deref() {
                    Some(path) => {
                        let path = numbered(path, captures.0);
                        captures.0 += 1;
                        if let Err(error) = write_content(item, &path) {
                            eprintln!("could not write {}: {error}", path.display());
                            return EXIT_USAGE;
                        }
                        eprintln!("wrote {}", path.display());
                    }
                    // The facts still carry the view handle that names the capture.
                    None => eprintln!("content omitted; pass --output <file> to keep it"),
                }
            }
            if rendering.json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string(&invocation.result).unwrap_or_default()
                );
            } else {
                let _ = writeln!(out, "{}", summary_of(&invocation.result));
            }
            exit_code(&invocation.result)
        }
        Err(error) => report_transport(&error),
    }
}

/// Decode one bounded content item to disk.
fn write_content(item: &ServiceContent, path: &Path) -> std::io::Result<()> {
    let ServiceContent::Image { data, .. } = item;
    let bytes = BASE64
        .decode(data)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, bytes)
}

/// The first capture keeps the requested name; later ones in the same session gain an index.
fn numbered(path: &Path, index: usize) -> PathBuf {
    if index == 0 {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "capture".into());
    let extension = path
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    path.with_file_name(format!("{stem}-{}{extension}", index + 1))
}

fn summary_of(result: &Value) -> String {
    result
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("the service returned no summary")
        .to_owned()
}

/// Map the terminal status onto a process exit code.
fn exit_code(result: &Value) -> i32 {
    match result.get("status").and_then(Value::as_str) {
        Some("succeeded") => EXIT_SUCCEEDED,
        Some("blocked") => EXIT_BLOCKED,
        Some("attention_required") => EXIT_ATTENTION,
        Some("failed") => EXIT_FAILED,
        Some("cancelled") => EXIT_CANCELLED,
        _ => EXIT_UNKNOWN,
    }
}

fn report_transport(error: &ClientError) -> i32 {
    eprintln!("{error}");
    EXIT_USAGE
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{exit_code, numbered, parse, Command, Rendering, EXIT_SUCCEEDED, EXIT_UNKNOWN};
    use serde_json::json;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn a_call_takes_one_tool_and_an_optional_input() {
        assert_eq!(
            parse(&arguments(&["browser_list_tabs"])).unwrap(),
            Command::Call {
                tool: "browser_list_tabs".into(),
                input: "{}".into(),
                rendering: Rendering::default()
            }
        );
        assert_eq!(
            parse(&arguments(&[
                "browser_open_page",
                "{\"url\":\"x\"}",
                "--json"
            ]))
            .unwrap(),
            Command::Call {
                tool: "browser_open_page".into(),
                input: "{\"url\":\"x\"}".into(),
                rendering: Rendering {
                    json: true,
                    output: None
                }
            }
        );
        assert_eq!(parse(&arguments(&["--catalog"])).unwrap(), Command::Catalog);
        assert!(parse(&arguments(&[])).is_err());
        assert!(parse(&arguments(&["a", "b", "c"])).is_err());
        assert!(parse(&arguments(&["browser_list_tabs", "--nonsense"])).is_err());
    }

    #[test]
    fn an_output_path_is_taken_as_a_value_not_a_tool() {
        // Without consuming its value, "shot.jpg" would look like a second positional and the
        // call would be rejected as ambiguous.
        assert_eq!(
            parse(&arguments(&["--stdin", "--json", "--output", "shot.jpg"])).unwrap(),
            Command::Batch {
                rendering: Rendering {
                    json: true,
                    output: Some(PathBuf::from("shot.jpg"))
                }
            }
        );
        assert_eq!(
            parse(&arguments(&[
                "browser_take_screenshot",
                "--output=shot.jpg"
            ]))
            .unwrap(),
            Command::Call {
                tool: "browser_take_screenshot".into(),
                input: "{}".into(),
                rendering: Rendering {
                    json: false,
                    output: Some(PathBuf::from("shot.jpg"))
                }
            }
        );
        assert!(parse(&arguments(&["browser_take_screenshot", "--output"])).is_err());
    }

    #[test]
    fn a_batch_does_not_overwrite_its_own_captures() {
        let path = PathBuf::from("shots/page.jpg");
        assert_eq!(numbered(&path, 0), path);
        assert_eq!(numbered(&path, 1), PathBuf::from("shots/page-2.jpg"));
        assert_eq!(numbered(&path, 2), PathBuf::from("shots/page-3.jpg"));
    }

    #[test]
    fn an_uncertain_effect_never_exits_zero() {
        // A script that treats "cannot be determined" as success will replay an effect that may
        // already have happened. Every terminal status is distinguishable for that reason.
        assert_eq!(exit_code(&json!({"status":"succeeded"})), EXIT_SUCCEEDED);
        assert_eq!(exit_code(&json!({"status":"unknown"})), EXIT_UNKNOWN);
        assert_eq!(exit_code(&json!({})), EXIT_UNKNOWN);
        for status in ["blocked", "attention_required", "failed", "cancelled"] {
            assert_ne!(
                exit_code(&json!({ "status": status })),
                EXIT_SUCCEEDED,
                "{status} must not look like success to a shell"
            );
        }
    }
}
