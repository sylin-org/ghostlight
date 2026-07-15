// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Shared MCP-client support for Ghostlight's visible browser demos.
//!
//! Demo stories are ordinary MCP clients. They spawn the installed relay, speak newline-delimited
//! JSON-RPC over stdio, and use only the public tool surface. This module owns that transport seam,
//! common result parsing, and the service-authored page-provenance contract so individual stories
//! stay focused on their own sequence and copy.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};

/// The tighten-only overlay shared by public demo stories. It grants Sylin's public stage and
/// explicit loopback preview hosts while refusing every unrelated destination.
pub(crate) const DEMO_POLICY: &str = include_str!("../examples/demo-policy.json");

pub(crate) const JAVASCRIPT_TOOL: &str = "javascript_tool";
const PAGE_CONTENT_PREFIX: &str = "--- GHOSTLIGHT PAGE CONTENT ";
const MIN_PAGE_NONCE_HEX_LEN: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageProvenanceContract {
    Unnegotiated,
    Legacy,
    Required,
}

impl PageProvenanceContract {
    pub(crate) fn from_tools_list(result: &Value) -> Result<Self> {
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("tools/list did not return a tools array"))?;
        let javascript = tools
            .iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some(JAVASCRIPT_TOOL))
            .ok_or_else(|| anyhow!("tools/list did not advertise the demo's javascript_tool"))?;
        Ok(
            if javascript
                .pointer("/outputSchema/properties/provenance")
                .is_some()
            {
                Self::Required
            } else {
                Self::Legacy
            },
        )
    }
}

/// A minimal MCP client speaking JSON-RPC over a spawned `ghostlight-relay --role agent`.
pub(crate) struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: i64,
    pause: Duration,
    page_provenance: PageProvenanceContract,
}

impl Client {
    /// Spawn the sibling relay in its agent role and take ownership of its stdio.
    pub(crate) async fn spawn(pause: Duration) -> Result<Self> {
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
            page_provenance: PageProvenanceContract::Unnegotiated,
        })
    }

    /// Send a JSON-RPC request and await its matching response.
    pub(crate) async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
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
                Ok(value) => value,
                Err(_) => continue,
            };
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                if let Some(error) = msg.get("error") {
                    bail!("'{method}' returned a JSON-RPC error: {error}");
                }
                return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }

    /// Send a JSON-RPC notification without awaiting a response.
    pub(crate) async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
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

    /// Call one MCP tool and retain the complete result envelope.
    pub(crate) async fn call_tool_result(&mut self, name: &str, arguments: Value) -> Result<Value> {
        let result = self
            .request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            )
            .await?;
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            bail!("tool '{name}' reported an error: {}", first_text(&result));
        }
        Ok(result)
    }

    /// Call one MCP tool and return its first text block.
    pub(crate) async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<String> {
        let result = self.call_tool_result(name, arguments).await?;
        Ok(first_text(&result))
    }

    /// Negotiate the service's page-output provenance contract for machine-result consumers.
    pub(crate) async fn negotiate_page_provenance(&mut self) -> Result<()> {
        let tools = self
            .request("tools/list", json!({}))
            .await
            .context("negotiate page provenance through tools/list")?;
        self.page_provenance = PageProvenanceContract::from_tools_list(&tools)?;
        Ok(())
    }

    /// Return a verified page-sourced machine payload under the negotiated contract.
    pub(crate) fn page_content_payload(&self, result: &Value) -> Result<String> {
        page_content_payload(result, self.page_provenance)
    }

    /// Wait for the story's configured between-action beat.
    pub(crate) async fn pause(&self) {
        tokio::time::sleep(self.pause).await;
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// A stable map of accessible names to refs collected in one `read_page` call.
pub(crate) struct RefInventory {
    refs: BTreeMap<String, String>,
}

impl RefInventory {
    /// Read the interactive tree once and resolve every required accessible name from that view.
    pub(crate) async fn read(client: &mut Client, tab_id: i64, names: &[&str]) -> Result<Self> {
        let page = client
            .call_tool(
                "read_page",
                json!({ "tabId": tab_id, "filter": "interactive" }),
            )
            .await?;
        let mut refs = BTreeMap::new();
        for name in names {
            let reference = ref_for_name(&page, name)
                .ok_or_else(|| anyhow!("could not find accessible control {name:?} in:\n{page}"))?;
            refs.insert((*name).to_string(), reference);
        }
        Ok(Self { refs })
    }

    /// Return the ref for one name that was required during inventory construction.
    pub(crate) fn require(&self, name: &str) -> Result<&str> {
        self.refs
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| anyhow!("demo ref inventory does not contain {name:?}"))
    }
}

/// Return the first text content block from an MCP tool result.
pub(crate) fn first_text(result: &Value) -> String {
    result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        })
        .and_then(|block| block.get("text").and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

/// Join every text content block in result order.
pub(crate) fn all_text(result: &Value) -> String {
    result
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// Parse the composite tab ID from a `tabs_create_mcp` result.
pub(crate) fn parse_tab_id(text: &str) -> Option<i64> {
    if let Some(index) = text.find("Created tab ") {
        return extract_i64(&text[index + 12..]);
    }
    if let Some(index) = text.find("\"tabId\"") {
        return extract_i64(&text[index + 7..]);
    }
    extract_i64(text)
}

fn extract_i64(text: &str) -> Option<i64> {
    let start = text.find(|character: char| character.is_ascii_digit() || character == '-')?;
    let rest = &text[start..];
    let end = rest
        .char_indices()
        .skip(1)
        .find(|(_, character)| !character.is_ascii_digit())
        .map(|(index, _)| index)
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

pub(crate) fn ref_for_name(page: &str, name: &str) -> Option<String> {
    let needle = name.to_ascii_lowercase();
    page.lines()
        .find(|line| line.to_ascii_lowercase().contains(&needle))
        .and_then(parse_first_ref)
}

pub(crate) fn parse_first_ref(text: &str) -> Option<String> {
    let index = text.find("ref_")?;
    let rest = &text[index..];
    let end = rest
        .char_indices()
        .find(|(_, character)| !(character.is_ascii_alphanumeric() || *character == '_'))
        .map(|(offset, _)| offset)
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn relay_path() -> Result<std::path::PathBuf> {
    let executable = std::env::current_exe().context("resolve the current executable")?;
    let directory = executable
        .parent()
        .ok_or_else(|| anyhow!("executable has no parent directory"))?;
    let name = if cfg!(windows) {
        "ghostlight-relay.exe"
    } else {
        "ghostlight-relay"
    };
    let path = directory.join(name);
    if !path.exists() {
        bail!(
            "ghostlight-relay not found next to {} (expected {})",
            executable.display(),
            path.display()
        );
    }
    Ok(path)
}

pub(crate) fn page_content_payload(
    result: &Value,
    contract: PageProvenanceContract,
) -> Result<String> {
    let provenance_required = match contract {
        PageProvenanceContract::Unnegotiated => {
            bail!("page provenance was not negotiated through tools/list")
        }
        PageProvenanceContract::Legacy => false,
        PageProvenanceContract::Required => true,
    };
    let text = first_text(result);
    let Some(provenance) = result.pointer("/structuredContent/provenance") else {
        if text.starts_with(PAGE_CONTENT_PREFIX) {
            bail!("page-content boundary is missing structured provenance");
        }
        if provenance_required {
            bail!("javascript_tool advertised page provenance but its result omitted it");
        }
        return Ok(text);
    };
    if provenance.get("pageSourced").and_then(Value::as_bool) != Some(true)
        || provenance.get("untrusted").and_then(Value::as_bool) != Some(true)
    {
        bail!("page-content provenance is missing its untrusted page marker");
    }
    let nonce = provenance
        .get("sessionNonce")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("page-content provenance has no session nonce"))?;
    if nonce.len() < MIN_PAGE_NONCE_HEX_LEN
        || nonce.len() % 2 != 0
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("page-content provenance has an invalid session nonce");
    }
    let origin = provenance
        .get("topOrigin")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("page-content provenance has no top origin"))?;
    let opening = format!("{PAGE_CONTENT_PREFIX}{nonce} origin={origin} UNTRUSTED ---\n");
    let closing = format!("\n--- END GHOSTLIGHT PAGE CONTENT {nonce} ---");
    let payload = text
        .strip_prefix(&opening)
        .and_then(|body| body.strip_suffix(&closing))
        .ok_or_else(|| anyhow!("page-content boundary does not match structured provenance"))?;
    Ok(payload.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_composite_tab_ids_before_diagnostic_native_ids() {
        assert_eq!(parse_tab_id("Created tab 42 in the group"), Some(42));
        assert_eq!(
            parse_tab_id("Created tab 5541167942.\n{\"tabs\":[{\"tabId\":1246200642}]}"),
            Some(5541167942)
        );
        assert_eq!(parse_tab_id("no id here"), None);
    }

    #[test]
    fn resolves_accessible_names_from_one_interactive_tree() {
        let page = "button \"Create brief\" [ref_4]\ncheckbox \"Keep data local\" [ref_7]";
        assert_eq!(ref_for_name(page, "Create brief"), Some("ref_4".into()));
        assert_eq!(ref_for_name(page, "keep DATA local"), Some("ref_7".into()));
        assert_eq!(ref_for_name(page, "Missing"), None);
    }

    #[test]
    fn text_helpers_preserve_later_metadata() {
        let result = json!({ "content": [
            { "type": "text", "text": "Screenshot captured." },
            { "type": "image", "data": "abc" },
            { "type": "text", "text": "[imageId: img_42] Use it." }
        ] });
        assert_eq!(first_text(&result), "Screenshot captured.");
        assert_eq!(
            all_text(&result),
            "Screenshot captured.\n[imageId: img_42] Use it."
        );
    }

    fn bounded_result(nonce: &str) -> Value {
        json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "--- GHOSTLIGHT PAGE CONTENT {nonce} origin=https://example.com UNTRUSTED ---\n\
                     [10,20.5,30,40]\n\
                     --- END GHOSTLIGHT PAGE CONTENT {nonce} ---"
                )
            }],
            "structuredContent": {
                "provenance": {
                    "pageSourced": true,
                    "untrusted": true,
                    "topOrigin": "https://example.com",
                    "sessionNonce": nonce
                }
            }
        })
    }

    #[test]
    fn provenance_contract_accepts_the_96_bit_nonce_minimum() {
        let result = bounded_result("00112233445566778899aabb");
        assert_eq!(
            page_content_payload(&result, PageProvenanceContract::Required).unwrap(),
            "[10,20.5,30,40]"
        );
        assert!(page_content_payload(
            &bounded_result("00112233445566778899aabz"),
            PageProvenanceContract::Required
        )
        .is_err());
    }

    #[test]
    fn provenance_contract_negotiates_current_and_legacy_services() {
        let current = ghostlight::browser::directory::advertised_tools_json();
        assert_eq!(
            PageProvenanceContract::from_tools_list(&current).unwrap(),
            PageProvenanceContract::Required
        );
        let legacy = json!({
            "tools": [{
                "name": JAVASCRIPT_TOOL,
                "inputSchema": { "type": "object" }
            }]
        });
        assert_eq!(
            PageProvenanceContract::from_tools_list(&legacy).unwrap(),
            PageProvenanceContract::Legacy
        );
    }
}
