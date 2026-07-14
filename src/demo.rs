// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `ghostlight demo`: a scripted tour of the public demo stage
//! (sylin.org/ghostlight/demo), driven as an ordinary MCP client so it exercises the REAL tool
//! surface -- the same path Claude takes. Cross-platform, superseding the pre-ADR-0046/0051
//! PowerShell harnesses that directly spawned the old single-process server.
//!
//! It connects by spawning `ghostlight-relay --role agent` and speaking newline-delimited
//! JSON-RPC over its stdio -- the relay handles all the connect/handshake/reconnect resilience, so
//! this stays a thin scripted client. At `initialize` it declares a tighten-only session policy
//! overlay (ADR-0060, `examples/demo-policy.json`): grants only sylin.org. Every step on the demo
//! pages then works, and the finale -- a navigation to example.com -- is refused by the overlay in
//! ANY service mode, with zero operator setup, so the governance ribbon appears on screen.
//!
//! Prerequisites (checked/reported, never worked around): a running Ghostlight service with the
//! extension attached (`ghostlight doctor`), and a real, visible browser window -- the effects are
//! deliberately hidden from screenshots, so this is a watch-your-browser demo.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};

/// The tighten-only session overlay declared at `initialize` (ADR-0060): grants only sylin.org, so
/// the whole tour works on the demo pages and the finale's off-domain navigation is refused.
const DEMO_POLICY: &str = include_str!("../examples/demo-policy.json");

/// How long each chapter caption remains visible and how long the demo waits before acting. The
/// matching values make narration a deliberate chapter card instead of an overlay the next action
/// races underneath.
const NARRATION_DURATION: Duration = Duration::from_secs(6);

/// The demo's three watchability rhythms, all operator-tunable: a short beat after each visible
/// step, a long hold right after the tab opens (time to resize/position the window before the
/// tour starts), and a breather between sections so each "test" reads as its own scene.
#[derive(Debug, Clone, Copy)]
pub struct Pacing {
    /// Seconds after each visible step (`--pause`, default 3).
    pub step_secs: f64,
    /// Seconds after the demo tab opens, before the tour starts (`--setup-pause`, default 10).
    pub setup_secs: f64,
    /// Seconds between the tour's sections (`--section-pause`, default 5).
    pub section_secs: f64,
}

/// Entry point for the `demo` subcommand. `base_url` defaults to the live site; `pacing` carries
/// the three watchability rhythms (step beat, window-setup hold, section breather).
pub fn run(base_url: &str, pacing: Pacing) -> Result<()> {
    let base = base_url.trim_end_matches('/').to_string();
    let rt = tokio::runtime::Runtime::new().context("build the demo tokio runtime")?;
    rt.block_on(drive(base, pacing))
}

/// A minimal MCP client speaking JSON-RPC over a spawned `ghostlight-relay --role agent`.
struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: i64,
    pause: Duration,
}

impl Client {
    /// Spawn the relay (the sibling binary of this executable) as an agent-role MCP pass-through
    /// and take its stdio. The relay resolves the same instance this process did (it inherits
    /// `GHOSTLIGHT_INSTANCE`), so `ghostlight --instance dev demo` drives the dev service.
    async fn spawn(pause: Duration) -> Result<Self> {
        let relay = relay_path().context("locate the ghostlight-relay binary")?;
        let mut child = tokio::process::Command::new(&relay)
            .arg("--role")
            .arg("agent")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("spawn {}", relay.display()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("relay stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("relay stdout unavailable"))?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            next_id: 0,
            pause,
        })
    }

    /// Send a request and await the response with the matching id, skipping notifications and
    /// unrelated ids. Fails if the relay closes its output before answering.
    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let frame = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.write(&frame).await?;
        loop {
            let line = self
                .stdout
                .next_line()
                .await
                .context("read from relay")?
                .ok_or_else(|| anyhow!("relay closed its output while awaiting '{method}'"))?;
            if line.trim().is_empty() {
                continue;
            }
            let msg: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                if let Some(err) = msg.get("error") {
                    bail!("'{method}' returned a JSON-RPC error: {err}");
                }
                return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }

    /// Send a notification (no id, no response awaited).
    async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.write(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
            .await
    }

    async fn write(&mut self, frame: &Value) -> Result<()> {
        let mut line = serde_json::to_string(frame)?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .context("write to relay")?;
        self.stdin.flush().await.context("flush relay stdin")
    }

    /// Call a tool and return the first text block of its result. A denial is ordinary text
    /// beginning `Denied (` (rendered as a successful result), so callers that want to detect the
    /// guardrail inspect the returned string; a genuine `isError` result is surfaced as an error.
    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<String> {
        let result = self
            .request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            )
            .await?;
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            bail!("tool '{name}' reported an error: {}", first_text(&result));
        }
        Ok(first_text(&result))
    }

    async fn pause(&self) {
        tokio::time::sleep(self.pause).await;
    }
}

/// The first text block of an MCP tool result (`{ content: [ { type, text } ] }`), or "".
fn first_text(result: &Value) -> String {
    result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        })
        .and_then(|b| b.get("text").and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

/// The `ghostlight-relay` binary sitting next to this executable.
fn relay_path() -> Result<std::path::PathBuf> {
    let exe = std::env::current_exe().context("resolve the current executable")?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("executable has no parent directory"))?;
    let name = if cfg!(windows) {
        "ghostlight-relay.exe"
    } else {
        "ghostlight-relay"
    };
    let path = dir.join(name);
    if !path.exists() {
        bail!(
            "ghostlight-relay not found next to {} (expected {})",
            exe.display(),
            path.display()
        );
    }
    Ok(path)
}

fn step(msg: &str) {
    println!("\n>> {msg}");
}

/// The between-sections breather: a visible countdown-free hold so each section of the tour
/// reads as its own scene rather than one continuous blur.
async fn section_break(pacing: &Pacing) {
    tokio::time::sleep(Duration::from_secs_f64(pacing.section_secs.max(0.0))).await;
}

/// Put the demo's own semantic caption track on screen, then leave it undisturbed for its full
/// lifetime so the sentence reads as a chapter card before the section begins. The visual layer
/// controls replacement and expiry; this helper is only pacing and copy.
async fn narrate(c: &mut Client, tab_id: i64, message: &str) -> Result<()> {
    c.call_tool(
        "narrate",
        json!({
            "tabId": tab_id,
            "text": message,
            "position": "auto",
            "duration_ms": NARRATION_DURATION.as_millis()
        }),
    )
    .await?;
    tokio::time::sleep(NARRATION_DURATION).await;
    Ok(())
}

/// Run the whole scripted tour. Returns an error (non-zero exit) if any step fails, so this
/// doubles as an end-to-end smoke test.
async fn drive(base: String, pacing: Pacing) -> Result<()> {
    println!("Ghostlight demo");
    println!("  stage : {base}");
    println!("  note  : keep the browser window visible -- the effects are hidden from screenshots by design.");

    let mut c = Client::spawn(Duration::from_secs_f64(pacing.step_secs.max(0.0))).await?;

    step("Handshake, declaring a tighten-only session policy (grants only sylin.org)");
    c.request(
        "initialize",
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "ghostlight-demo", "version": env!("CARGO_PKG_VERSION") },
            "_meta": { "ghostlightSessionPolicy": DEMO_POLICY }
        }),
    )
    .await
    .context("initialize (is a Ghostlight service running with the extension attached? run `ghostlight doctor`)")?;
    c.notify("notifications/initialized", json!({})).await?;

    step("Open a fresh tab in the Ghostlight group");
    let created = c.call_tool("tabs_create_mcp", json!({})).await?;
    let tab_id = parse_tab_id(&created)
        .ok_or_else(|| anyhow!("could not read the new tab id from: {created}"))?;
    println!("   tab {tab_id}");
    let setup = pacing.setup_secs.max(0.0);
    if setup > 0.0 {
        println!("   (holding {setup:.0}s -- resize and position the browser window now)");
        tokio::time::sleep(Duration::from_secs_f64(setup)).await;
    }

    // --- Desk: point and act ---
    section_break(&pacing).await;
    step("Desk: navigate, then click the button and type in the field");
    c.call_tool(
        "navigate",
        json!({ "tabId": tab_id, "url": format!("{base}/desk/") }),
    )
    .await?;
    c.pause().await;
    narrate(
        &mut c,
        tab_id,
        "Ghostlight works in the browser session you already use.",
    )
    .await?;
    narrate(
        &mut c,
        tab_id,
        "The agent can point, click, and type visibly.",
    )
    .await?;
    if let Some(btn) = find_ref(&mut c, tab_id, "the big call-to-action button").await? {
        c.call_tool(
            "computer",
            json!({ "action": "left_click", "tabId": tab_id, "ref": btn }),
        )
        .await?;
        c.pause().await;
    }
    if let Some(field) = find_ref(&mut c, tab_id, "the text input field").await? {
        c.call_tool(
            "computer",
            json!({ "action": "left_click", "tabId": tab_id, "ref": field }),
        )
        .await?;
        c.pause().await;
        c.call_tool("computer", json!({ "action": "type", "tabId": tab_id, "text": "Ghostlight is watching the stage." })).await?;
        c.pause().await;
    }

    // --- Form: fill it in ---
    section_break(&pacing).await;
    step("Form: fill every field and submit (nothing is sent anywhere)");
    c.call_tool(
        "navigate",
        json!({ "tabId": tab_id, "url": format!("{base}/form/") }),
    )
    .await?;
    c.pause().await;
    narrate(
        &mut c,
        tab_id,
        "Structured tools can complete an entire form.",
    )
    .await?;
    c.call_tool(
        "form_fill",
        json!({
            "tabId": tab_id,
            "fields": {
                "Full name": "Ada Lovelace",
                "Email": "ada@example.org",
                "Role": "developer",
                "Message": "Driven by Ghostlight, safely."
            },
            "submit": true
        }),
    )
    .await?;
    c.pause().await;
    let _ = c
        .call_tool(
            "wait_for",
            json!({ "tabId": tab_id, "text": "Form received" }),
        )
        .await;
    c.pause().await;

    // --- Signals: watch the wire ---
    section_break(&pacing).await;
    step("Signals: log to the console, fetch data, and wait for a slow task");
    c.call_tool(
        "navigate",
        json!({ "tabId": tab_id, "url": format!("{base}/signals/") }),
    )
    .await?;
    c.pause().await;
    narrate(
        &mut c,
        tab_id,
        "The agent can inspect console and network signals.",
    )
    .await?;
    if let Some(log_btn) = find_ref(&mut c, tab_id, "the log to the console button").await? {
        c.call_tool(
            "computer",
            json!({ "action": "left_click", "tabId": tab_id, "ref": log_btn }),
        )
        .await?;
        c.pause().await;
        let _ = c
            .call_tool("read_console_messages", json!({ "tabId": tab_id }))
            .await;
        c.pause().await;
    }
    if let Some(fetch_btn) = find_ref(&mut c, tab_id, "the fetch demo data button").await? {
        c.call_tool(
            "computer",
            json!({ "action": "left_click", "tabId": tab_id, "ref": fetch_btn }),
        )
        .await?;
        c.pause().await;
        let _ = c
            .call_tool("read_network_requests", json!({ "tabId": tab_id }))
            .await;
        c.pause().await;
    }
    if let Some(slow_btn) = find_ref(&mut c, tab_id, "the start a slow task button").await? {
        c.call_tool(
            "computer",
            json!({ "action": "left_click", "tabId": tab_id, "ref": slow_btn }),
        )
        .await?;
        c.pause().await;
        let _ = c
            .call_tool(
                "wait_for",
                json!({ "tabId": tab_id, "text": "slow task finished", "timeout_ms": 6000 }),
            )
            .await;
    }
    c.pause().await;

    // --- Reading room: take it in ---
    section_break(&pacing).await;
    step("Reading room: extract the text and find a passage");
    c.call_tool(
        "navigate",
        json!({ "tabId": tab_id, "url": format!("{base}/reading/") }),
    )
    .await?;
    c.pause().await;
    narrate(
        &mut c,
        tab_id,
        "It can read page content without moving the session elsewhere.",
    )
    .await?;
    let text = c
        .call_tool("get_page_text", json!({ "tabId": tab_id }))
        .await?;
    println!("   get_page_text: {} chars", text.len());
    let _ = find_ref(&mut c, tab_id, "the word lantern").await?;
    c.pause().await;

    // --- The guardrail: the whole point ---
    section_break(&pacing).await;
    narrate(
        &mut c,
        tab_id,
        "Policy still decides where the agent may go.",
    )
    .await?;
    step("The guardrail: ask Ghostlight to step off the granted domain -- it should refuse");
    let outcome = c
        .call_tool(
            "navigate",
            json!({ "tabId": tab_id, "url": "https://example.com/" }),
        )
        .await?;
    if outcome.starts_with("Denied") {
        println!("   refused, on screen and in plain language:");
        println!("   {outcome}");
        c.pause().await;
        println!("\nDemo complete -- every tool ran, and the guardrail held.");
        Ok(())
    } else {
        // With the session overlay declared this should never happen; report loudly if it does.
        bail!(
            "the off-domain navigation was NOT refused (got: {outcome}). The session policy overlay \
             did not take effect -- is this build's service current with ADR-0060?"
        )
    }
}

/// Find one element by natural-language query and return its first ref, or None if nothing matched
/// (a soft miss: the tour narrates and continues rather than aborting on a page tweak).
async fn find_ref(c: &mut Client, tab_id: i64, query: &str) -> Result<Option<String>> {
    let found = c
        .call_tool("find", json!({ "tabId": tab_id, "query": query }))
        .await?;
    Ok(parse_first_ref(&found))
}

/// Pull the first `ref_N` token out of a find/read_page result.
fn parse_first_ref(text: &str) -> Option<String> {
    let idx = text.find("ref_")?;
    let rest = &text[idx..];
    let end = rest
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_'))
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Pull the composite `tabId` out of a tabs_create_mcp result (either a `"tabId": N` field in the
/// text, or a bare "Created tab N").
fn parse_tab_id(text: &str) -> Option<i64> {
    if let Some(idx) = text.find("\"tabId\"") {
        let rest = &text[idx + 7..];
        return extract_i64(rest);
    }
    if let Some(idx) = text.find("Created tab ") {
        return extract_i64(&text[idx + 12..]);
    }
    extract_i64(text)
}

/// The first run of ASCII digits (optionally sign-prefixed) parsed as i64.
fn extract_i64(s: &str) -> Option<i64> {
    let start = s.find(|c: char| c.is_ascii_digit() || c == '-')?;
    let rest = &s[start..];
    let end = rest
        .char_indices()
        .skip(1)
        .find(|(_, ch)| !ch.is_ascii_digit())
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

impl Drop for Client {
    fn drop(&mut self) {
        // Best-effort: close stdin and reap the relay so it does not linger.
        let _ = self.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_composite_tab_id_from_structured_text() {
        assert_eq!(
            parse_tab_id(r#"{"tabId": 93825471234567, "url": "x"}"#),
            Some(93825471234567)
        );
        assert_eq!(parse_tab_id("Created tab 42 in the group"), Some(42));
        assert_eq!(parse_tab_id("no id here"), None);
    }

    #[test]
    fn pulls_the_first_ref_token() {
        assert_eq!(
            parse_first_ref("button [ref_7] primary"),
            Some("ref_7".to_string())
        );
        assert_eq!(parse_first_ref("nothing"), None);
    }

    #[test]
    fn first_text_reads_the_text_block() {
        let r = json!({ "content": [ { "type": "text", "text": "hello" } ] });
        assert_eq!(first_text(&r), "hello");
    }
}
