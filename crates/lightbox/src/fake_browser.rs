// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! `lightbox fake-browser` (ADR-0059): an interactive, offline stand-in for the real
//! `ghostlight-relay` browser role + Chrome extension. Dials a REAL running service's extension
//! endpoint exactly as the real relay does (same endpoint candidates, same `ROLE_BROWSER`
//! hello), so wire-protocol changes -- routing, tabId encoding, focus, notifications -- can be
//! exercised and watched WITHOUT Chrome, an extension reload, or a real browser process at all.
//!
//! `--auto-reply` answers every `tool_request`/`tab_url_request` with a canned result.
//! `tabs_context_mcp`/`tabs_create_mcp` are answered with a DELIBERATELY billion-scale native
//! tab id (`2_000_000_000` and up), not a small one -- so a tabId-encoding regression like the
//! one ADR-0058 shipped with is caught by the FIRST offline round trip, before a real browser
//! (whose own tab ids can genuinely reach that magnitude) is ever involved.

use ghostlight_transport::instance::Selection;
use ghostlight_transport::ipc::{self, EndpointProbe};
use ghostlight_transport::{handshake, host, proc};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Options for `lightbox fake-browser`.
pub struct FakeBrowserOptions {
    /// `--instance <name>`: which instance's endpoints to try (default: unpinned, `[dev,
    /// default]`, matching how the real browser role resolves).
    pub instance: Option<String>,
    /// `--pid <u32>`: the fake browser identity this session's hello presents.
    pub pid: u32,
    /// `--auto-reply`: answer every `tool_request`/`tab_url_request` with a canned result
    /// instead of requiring a manual `reply <id> <json>` for each one.
    pub auto_reply: bool,
}

pub fn run(opts: FakeBrowserOptions) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_async(opts))
}

async fn run_async(opts: FakeBrowserOptions) -> anyhow::Result<()> {
    let selection = Selection::resolve_from(opts.instance.as_deref())
        .map_err(|e| anyhow::anyhow!("invalid --instance: {e}"))?;
    let endpoints = ipc::endpoint_candidates(&selection);
    let endpoint = pick_endpoint(&endpoints);

    println!(
        "lightbox fake-browser: connecting to {endpoint} as browser pid={}",
        opts.pid
    );
    let stream = ipc::connect(&endpoint)
        .await
        .map_err(|e| anyhow::anyhow!("connect to {endpoint} failed: {e}"))?;
    let (mut read_half, mut write_half) = tokio::io::split(stream);

    let hello = handshake::browser_hello_bytes(
        std::process::id(),
        Some(proc::ProcId {
            pid: opts.pid,
            created: 0,
        }),
    );
    host::write_message(&mut write_half, &hello).await?;
    println!(
        "hello sent; attached as pid={}. auto-reply={}",
        opts.pid, opts.auto_reply
    );
    println!("commands: focus | kill | reply <id> <json-result> | quit");

    let mut stdin_lines = BufReader::new(tokio::io::stdin()).lines();
    let mut next_native_tab: i64 = 2_000_000_000; // billion-scale on purpose (ADR-0059)

    loop {
        tokio::select! {
            frame = host::read_message(&mut read_half) => {
                match frame {
                    Ok(Some(bytes)) => {
                        let v: Value = serde_json::from_slice(&bytes)
                            .unwrap_or_else(|_| json!({"unparseable_bytes": bytes.len()}));
                        println!("<- {}", serde_json::to_string_pretty(&v).unwrap_or_default());
                        if opts.auto_reply {
                            if let Some(reply) = canned_reply(&v, &mut next_native_tab) {
                                send(&mut write_half, &reply).await?;
                                println!("-> {}", serde_json::to_string_pretty(&reply).unwrap_or_default());
                            }
                        }
                    }
                    Ok(None) => { println!("(service closed the connection)"); break; }
                    Err(e) => { println!("(read error: {e})"); break; }
                }
            }
            line = stdin_lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        match parse_command(&line) {
                            Some(ReplCommand::Focus) => {
                                send(&mut write_half, &json!({"type": "focus"})).await?;
                                println!("-> focus sent");
                            }
                            Some(ReplCommand::Kill) => {
                                send(&mut write_half, &json!({"type": "session_killed"})).await?;
                                println!("-> session_killed sent");
                            }
                            Some(ReplCommand::Reply { id, value }) => {
                                let reply = json!({ "id": id, "type": "tool_response", "result": value });
                                send(&mut write_half, &reply).await?;
                                println!("-> reply sent for id={id}");
                            }
                            Some(ReplCommand::Quit) => break,
                            None if line.trim().is_empty() => {}
                            None => println!("(unrecognized command; try: focus | kill | reply <id> <json-result> | quit)"),
                        }
                    }
                    Ok(None) => break, // stdin closed (piped input exhausted)
                    Err(e) => { println!("(stdin error: {e})"); break; }
                }
            }
        }
    }
    Ok(())
}

/// Mirrors `ghostlight_transport::ipc`'s own (private) `pick_native_host_endpoint`: the first
/// candidate whose endpoint exists right now wins; with everything absent, fall to the last
/// (the default instance in the unpinned candidate order), so this tool's own connect retries
/// naturally cover a service that is still starting up.
fn pick_endpoint(endpoints: &[String]) -> String {
    for ep in endpoints {
        if !matches!(ipc::probe_endpoint(ep), EndpointProbe::Absent) {
            return ep.clone();
        }
    }
    endpoints.last().cloned().unwrap_or_default()
}

async fn send<W: tokio::io::AsyncWrite + Unpin>(
    write_half: &mut W,
    msg: &Value,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(msg)?;
    host::write_message(write_half, &bytes).await?;
    write_half.flush().await?;
    Ok(())
}

/// The extension's real `tabs_context_mcp`/`tabs_create_mcp` result shape (a `content[].text`
/// JSON-stringified blob AND a `structuredContent` twin -- both carry `tabId`, exercising
/// `Browser::encode_tab_ids_in_value`'s walk over both).
fn canned_tab_context(native_tab_id: i64) -> Value {
    let structured = json!({
        "mcpGroupId": 1,
        "tabs": [{ "tabId": native_tab_id, "title": "New Tab", "url": "chrome://newtab/" }],
    });
    json!({
        "content": [{ "type": "text", "text": structured.to_string() }],
        "structuredContent": structured,
    })
}

/// The canned auto-reply for one incoming frame, or `None` for a frame this mode does not
/// answer (an id-less event like `group_request`/`notification`, which correctly gets no
/// reply). `next_native_tab` advances on every minted tab so repeated `tabs_create_mcp` calls
/// return distinct ids, the same way a real browser's own counter would.
fn canned_reply(frame: &Value, next_native_tab: &mut i64) -> Option<Value> {
    let id = frame.get("id")?.clone();
    match frame.get("type").and_then(Value::as_str) {
        Some("tool_request") => {
            let tool = frame.get("tool").and_then(Value::as_str).unwrap_or("");
            let result = if tool == "tabs_context_mcp" || tool == "tabs_create_mcp" {
                let native = *next_native_tab;
                *next_native_tab += 15; // a plausible "one more tab" increment, not always +1
                canned_tab_context(native)
            } else {
                json!({ "content": [{ "type": "text", "text": "lightbox fake-browser: ok" }] })
            };
            Some(json!({ "id": id, "type": "tool_response", "result": result }))
        }
        Some("tab_url_request") => Some(json!({
            "id": id,
            "type": "tab_url_response",
            "result": { "url": "https://example.org/" },
        })),
        _ => None,
    }
}

enum ReplCommand {
    Focus,
    Kill,
    Reply { id: String, value: Value },
    Quit,
}

fn parse_command(line: &str) -> Option<ReplCommand> {
    let line = line.trim();
    match line {
        "focus" => return Some(ReplCommand::Focus),
        "kill" => return Some(ReplCommand::Kill),
        "quit" | "exit" => return Some(ReplCommand::Quit),
        _ => {}
    }
    let rest = line.strip_prefix("reply ")?;
    let (id, json_str) = rest.split_once(' ')?;
    let value: Value = serde_json::from_str(json_str).ok()?;
    Some(ReplCommand::Reply {
        id: id.to_string(),
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_bare_commands() {
        assert!(matches!(parse_command("focus"), Some(ReplCommand::Focus)));
        assert!(matches!(parse_command("kill"), Some(ReplCommand::Kill)));
        assert!(matches!(parse_command("quit"), Some(ReplCommand::Quit)));
        assert!(matches!(parse_command("exit"), Some(ReplCommand::Quit)));
    }

    #[test]
    fn parses_a_reply_command() {
        let Some(ReplCommand::Reply { id, value }) = parse_command(r#"reply 5 {"ok":true}"#) else {
            panic!("expected a Reply command");
        };
        assert_eq!(id, "5");
        assert_eq!(value, json!({"ok": true}));
    }

    #[test]
    fn unrecognized_and_malformed_lines_are_none() {
        assert!(parse_command("").is_none());
        assert!(parse_command("bogus").is_none());
        assert!(parse_command("reply 5 not-json").is_none());
        assert!(parse_command("reply").is_none());
    }

    #[test]
    fn canned_reply_answers_tool_and_tab_url_requests_but_not_id_less_events() {
        let req = json!({"id": "1", "type": "tool_request", "tool": "navigate"});
        let mut next = 2_000_000_000i64;
        let reply = canned_reply(&req, &mut next).expect("tool_request gets a reply");
        assert_eq!(reply["type"], "tool_response");
        assert_eq!(reply["id"], "1");

        let tab_url = json!({"id": "2", "type": "tab_url_request", "tabId": 5});
        let reply = canned_reply(&tab_url, &mut next).expect("tab_url_request gets a reply");
        assert_eq!(reply["type"], "tab_url_response");

        let event = json!({"type": "group_request", "guid": "x", "tabIds": []});
        assert!(
            canned_reply(&event, &mut next).is_none(),
            "an id-less event gets no reply"
        );
    }

    #[test]
    fn canned_tab_context_uses_a_billion_scale_native_id_and_advances_it() {
        let mut next = 2_000_000_000i64;
        let req = json!({"id": "1", "type": "tool_request", "tool": "tabs_create_mcp"});
        let reply = canned_reply(&req, &mut next).unwrap();
        let structured = &reply["result"]["structuredContent"];
        let tab_id = structured["tabs"][0]["tabId"].as_i64().unwrap();
        assert!(
            tab_id >= 2_000_000_000,
            "deliberately billion-scale: {tab_id}"
        );
        assert!(
            next > 2_000_000_000,
            "the counter must advance so a second create differs"
        );

        // The content[].text blob carries the SAME tabId, JSON-stringified -- exercising
        // Browser::encode_tab_ids_in_value's text-block walk, not just structuredContent.
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains(&tab_id.to_string()));
    }
}
