//! The command-line intake: one local edge for scripts and programs.
//!
//! This is an edge in the same shape as the MCP connector (ADR-0105 Decision 1). It owns argument
//! parsing, rendering, and exit codes, and makes no product decision: every call crosses the same
//! service bridge, executor, governance facade, and completion path a model's call does.

use std::io::{BufRead, Write};
use std::path::Path;

use anyhow::{anyhow, Result};
use ghostlight_bridge::client::{ClientError, ServiceClient};
use ghostlight_bridge::service::IntakeChannel;
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

/// What the caller asked the command line to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// Invoke one tool with one input.
    Call {
        /// Exact catalog tool name.
        tool: String,
        /// Opaque JSON input, validated by the orchestrator.
        input: String,
        /// Print the whole terminal result rather than its sentence.
        json: bool,
    },
    /// Read `<tool> <json>` lines from standard input over one session.
    ///
    /// Handles belong to a session, so multi-step scripted work needs one process. Reading a line
    /// at a time lets a caller use a handle the previous line returned.
    Batch {
        /// Print whole terminal results rather than their sentences.
        json: bool,
    },
    /// List the catalog this service offers.
    Catalog,
}

/// Parse the `call` subcommand's arguments.
pub fn parse(arguments: &[String]) -> Result<Command> {
    let json = arguments.iter().any(|argument| argument == "--json");
    let positional: Vec<&String> = arguments
        .iter()
        .filter(|argument| !argument.starts_with("--"))
        .collect();
    if arguments.iter().any(|argument| argument == "--catalog") {
        return Ok(Command::Catalog);
    }
    if arguments.iter().any(|argument| argument == "--stdin") {
        return Ok(Command::Batch { json });
    }
    match positional.as_slice() {
        [tool] => Ok(Command::Call {
            tool: (*tool).clone(),
            input: "{}".into(),
            json,
        }),
        [tool, input] => Ok(Command::Call {
            tool: (*tool).clone(),
            input: (*input).clone(),
            json,
        }),
        [] => Err(anyhow!("ghostlight call <tool> [json] [--json]")),
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
        Command::Call { tool, input, json } => {
            let Ok(input) = serde_json::from_str::<Value>(&input) else {
                eprintln!("the input is not valid JSON");
                return EXIT_USAGE;
            };
            invoke_once(&mut client, &tool, input, json, out)
        }
        Command::Batch { json } => {
            let stdin = std::io::stdin();
            let mut worst = EXIT_SUCCEEDED;
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
                let code = invoke_once(&mut client, tool, input, json, out);
                if code != EXIT_SUCCEEDED {
                    worst = code;
                }
            }
            worst
        }
    }
}

fn invoke_once(
    client: &mut ServiceClient,
    tool: &str,
    input: Value,
    json: bool,
    out: &mut impl Write,
) -> i32 {
    match client.invoke(tool, input, None) {
        Ok(invocation) => {
            if !invocation.content.is_empty() {
                // Bounded rich content is not rendered here yet; the result's facts carry the
                // handle that names it.
                eprintln!("{} content item(s) omitted", invocation.content.len());
            }
            if json {
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
    use super::{exit_code, parse, Command, EXIT_SUCCEEDED, EXIT_UNKNOWN};
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
                json: false
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
                json: true
            }
        );
        assert_eq!(
            parse(&arguments(&["--stdin"])).unwrap(),
            Command::Batch { json: false }
        );
        assert_eq!(parse(&arguments(&["--catalog"])).unwrap(), Command::Catalog);
        assert!(parse(&arguments(&[])).is_err());
        assert!(parse(&arguments(&["a", "b", "c"])).is_err());
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
