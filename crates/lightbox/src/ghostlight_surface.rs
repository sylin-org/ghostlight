// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Executable evidence for Ghostlight's sole model-facing surface.
//!
//! These scenarios run the production service and MCP edge. The navigation journey connects a
//! fake extension at the browser IPC boundary, where it proves that opening a URL is one physical
//! browser transaction, that the MCP-visible opaque tab handle is minted only after workspace
//! admission, and that committed navigation readiness retains one transaction token and deadline.

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{ChildStdin, ChildStdout};
use std::time::Duration;

use anyhow::{anyhow, ensure};
use ghostlight_transport::operation::{ResultTab, MAX_RESULT_TABS};
use serde_json::{json, Value};

use crate::scenarios::Scenario;
use crate::support::{self, ChildGuard, TempRoot};

const GHOSTLIGHT_SURFACE: &str =
    include_str!("../../mcp-connector/src/surface/data/ghostlight-v1.json");
const MECHANISM_REQUEST_V1: &str = "mechanismRequestV1";
const NAVIGATION_READINESS_V1: &str = "navigationReadinessV1";
const ATOMIC_TAB_OPEN_V1: &str = "atomicTabOpenV1";
const NATIVE_TAB_ID: i64 = 41;
const NAVIGATION_URL: &str = "https://example.com/";
const NAVIGATION_TOKEN: &str = "n_lightbox";
const DOCUMENT_HANDLE: &str = "d_lightbox";
const READINESS_DEADLINE_MS: u64 = 10_000;
const FORBIDDEN_NATIVE_IDENTITY_FIELDS: [&str; 5] =
    ["tabId", "tab_id", "mcpGroupId", "groupId", "group_id"];

const GHOSTLIGHT_TOOLS: [&str; 24] = [
    "browser_get_status",
    "browser_open_tab",
    "browser_list_tabs",
    "browser_focus_tab",
    "browser_close_tab",
    "browser_navigate",
    "browser_go_back",
    "browser_go_forward",
    "browser_reload_page",
    "browser_inspect_page",
    "browser_read_page",
    "browser_take_screenshot",
    "browser_click",
    "browser_hover",
    "browser_scroll_to_target",
    "browser_scroll_page",
    "browser_press_key",
    "browser_press_escape",
    "browser_drag",
    "browser_fill_form",
    "browser_wait_for",
    "browser_run_sequence",
    "browser_get_dialog",
    "browser_handle_dialog",
];

/// Return the sole-surface process scenarios.
pub fn registry() -> Vec<Scenario> {
    vec![
        (
            "ghostlight_surface_catalog",
            ghostlight_surface_catalog as fn() -> anyhow::Result<()>,
        ),
        (
            "ghostlight_surface_navigation_readiness",
            ghostlight_surface_navigation_readiness,
        ),
    ]
}

struct LiveEdge {
    endpoint: String,
    log_dir: std::path::PathBuf,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    _edge: ChildGuard,
    _service: ChildGuard,
    _temp: TempRoot,
}

impl LiveEdge {
    fn start(tag: &str) -> anyhow::Result<Self> {
        let temp = TempRoot::new(tag)?;
        let endpoint = support::unique_endpoint(tag);
        let log_dir = temp.path().to_path_buf();
        let service = support::spawn_service(&endpoint, &log_dir)?;
        let mut edge = support::spawn_mcp_edge(&endpoint, &log_dir)?;
        let stdin = edge.stdin.take().ok_or_else(|| anyhow!("MCP edge stdin"))?;
        let stdout = edge
            .stdout
            .take()
            .ok_or_else(|| anyhow!("MCP edge stdout"))?;
        let mut live = Self {
            endpoint,
            log_dir,
            stdin,
            reader: BufReader::new(stdout),
            _edge: edge,
            _service: service,
            _temp: temp,
        };
        live.send(&support::initialize_2025(1, "lightbox-ghostlight"))?;
        let initialized = live.receive()?;
        ensure!(
            initialized["id"] == 1 && initialized["result"]["protocolVersion"] == "2025-11-25",
            "Ghostlight surface initialize failed: {initialized}"
        );
        live.send(&support::initialized_2025())?;
        Ok(live)
    }

    fn send(&mut self, value: &Value) -> anyhow::Result<()> {
        serde_json::to_writer(&mut self.stdin, value)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn receive(&mut self) -> anyhow::Result<Value> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        ensure!(!line.is_empty(), "MCP edge stdout closed");
        Ok(serde_json::from_str(line.trim_end())?)
    }

    fn tools(&mut self, id: i64) -> anyhow::Result<Value> {
        self.send(&json!({
            "jsonrpc":"2.0",
            "id":id,
            "method":"tools/list",
            "params":{}
        }))?;
        let response = self.receive()?;
        ensure!(
            response["id"] == id,
            "unexpected tools/list response: {response}"
        );
        response
            .pointer("/result/tools")
            .cloned()
            .ok_or_else(|| anyhow!("tools/list returned no tool array: {response}"))
    }

    fn call(&mut self, id: i64, name: &str, arguments: Value) -> anyhow::Result<Value> {
        self.send(&json!({
            "jsonrpc":"2.0",
            "id":id,
            "method":"tools/call",
            "params":{"name":name,"arguments":arguments}
        }))?;
        let response = self.receive()?;
        ensure!(
            response["id"] == id,
            "unexpected tools/call response: {response}"
        );
        Ok(response)
    }
}

/// The sole edge surface exposes exactly the canonical declarations and rendering.
fn ghostlight_surface_catalog() -> anyhow::Result<()> {
    let mut edge = LiveEdge::start("ghostlight-surface-catalog")?;
    let actual = edge.tools(2)?;
    let actual_tools = actual
        .as_array()
        .ok_or_else(|| anyhow!("Ghostlight tools/list result is not an array"))?;
    ensure!(
        actual_tools.len() == GHOSTLIGHT_TOOLS.len(),
        "Ghostlight catalog has the wrong size: {}",
        actual_tools.len()
    );

    let expected: Value = serde_json::from_str(GHOSTLIGHT_SURFACE)?;
    let expected_tools = expected["tools"]
        .as_array()
        .ok_or_else(|| anyhow!("embedded Ghostlight catalog is not an array"))?;

    for ((declaration, expected), name) in actual_tools
        .iter()
        .zip(expected_tools)
        .zip(GHOSTLIGHT_TOOLS)
    {
        ensure!(declaration["name"] == name, "Ghostlight tool order changed");
        ensure!(
            declaration["description"] == expected["description"]
                && declaration["annotations"] == expected["annotations"],
            "Ghostlight declaration copy changed for {name}"
        );
        ensure!(
            declaration["inputSchema"]["additionalProperties"] == false,
            "Ghostlight input schema for {name} is not typo-closed"
        );
        ensure!(
            declaration.get("pack").is_none(),
            "Ghostlight core advertised an undeclared pack member: {name}"
        );
    }

    let context = edge.call(3, "browser_get_status", json!({}))?;
    ensure!(
        context["result"]["isError"] != true
            && context["result"]["structuredContent"]["status"] == "ok"
            && context["result"]["structuredContent"]["effect"] == "none"
            && context["result"]["structuredContent"]["result"]["operations"]
                .as_array()
                .map(Vec::len)
                == Some(GHOSTLIGHT_TOOLS.len()),
        "browser_get_status did not use the canonical result path: {context}"
    );
    Ok(())
}

/// Opening a URL is one observed physical transaction with one truthful readiness journey.
fn ghostlight_surface_navigation_readiness() -> anyhow::Result<()> {
    let mut edge = LiveEdge::start("ghostlight-navigation-readiness")?;
    let extension_endpoint = edge.endpoint.clone();
    let extension = std::thread::spawn(move || -> anyhow::Result<()> {
        tokio::runtime::Runtime::new()?.block_on(run_native_extension(extension_endpoint))
    });
    support::wait_extension_connected(&edge.log_dir, Duration::from_secs(15))?;

    let created = edge.call(2, "browser_open_tab", json!({"url":NAVIGATION_URL}))?;
    ensure!(
        created["result"]["isError"] != true
            && created["result"]["structuredContent"]["status"] == "ok"
            && created["result"]["structuredContent"]["effect"] == "committed"
            && created["result"]["structuredContent"]["result"]["created"] == true
            && created["result"]["structuredContent"]["result"]["navigated"] == true
            && created["result"]["structuredContent"]["readiness"]["status"] == "ready",
        "Ghostlight atomic tab opening failed: {created}"
    );
    let workspace = created
        .pointer("/result/structuredContent/workspace")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("native creator returned no opaque workspace: {created}"))?
        .to_owned();
    let tab = created
        .pointer("/result/structuredContent/tab/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("native creator returned no opaque tab: {created}"))?
        .to_owned();
    ensure!(
        tab.starts_with("t_") && tab.len() > 2,
        "creator returned a malformed opaque tab handle: {tab}"
    );
    let inventory: Vec<ResultTab> = serde_json::from_value(
        created
            .pointer("/result/structuredContent/tabs")
            .cloned()
            .ok_or_else(|| anyhow!("native creator returned no typed tab inventory: {created}"))?,
    )?;
    ensure!(
        !inventory.is_empty() && inventory.len() <= MAX_RESULT_TABS,
        "native creator returned an unbounded tab inventory: {created}"
    );
    ensure!(
        inventory
            .iter()
            .all(|entry| entry.id.as_str().starts_with("t_") && entry.id.as_str().len() > 2),
        "native creator inventory contained a malformed opaque tab handle: {created}"
    );
    ensure!(
        inventory
            .iter()
            .map(|entry| entry.id.as_str())
            .eq([tab.as_str()]),
        "native creator inventory lost primary opaque tab correlation: {created}"
    );
    ensure!(
        !has_forbidden_native_identity(&created),
        "native creator leaked browser-owned tab or group identity: {created}"
    );

    ensure!(
        created["result"]["structuredContent"]["workspace"] == workspace
            && created["result"]["structuredContent"]["tab"]["id"] == tab,
        "Ghostlight atomic tab opening lost opaque workspace or tab continuity: {created}"
    );
    ensure!(
        created["result"]["structuredContent"]["tab"]["url"] == NAVIGATION_URL,
        "Ghostlight atomic tab opening did not report the verified landing: {created}"
    );

    extension
        .join()
        .map_err(|_| anyhow!("fake Ghostlight extension panicked"))??;
    Ok(())
}

async fn run_native_extension(endpoint: String) -> anyhow::Result<()> {
    let stream = ghostlight_transport::ipc::connect(&endpoint).await?;
    let (mut reader, mut writer) = tokio::io::split(stream);
    support::send_extension_attach_frames_with_features(
        &mut writer,
        &[
            MECHANISM_REQUEST_V1,
            NAVIGATION_READINESS_V1,
            ATOMIC_TAB_OPEN_V1,
        ],
    )
    .await?;

    let opened =
        support::read_frame_answering_tab_urls(&mut reader, &mut writer, "mechanism_request")
            .await?;
    ensure!(
        opened["mechanism"] == "workspace.tab.open"
            && opened["input"]["url"] == NAVIGATION_URL
            && opened["input"]["readiness"] == json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        "atomic tab opening reached the wrong browser mechanism: {opened}"
    );
    let route = opened["guid"].clone();
    ensure!(
        route.is_string(),
        "atomic tab opening carried no workspace route"
    );
    reply(
        &mut writer,
        &opened,
        atomic_open_result(navigation_evidence("committed", 1)),
    )
    .await?;

    let awaited =
        support::read_frame_answering_tab_urls(&mut reader, &mut writer, "mechanism_request")
            .await?;
    assert_readiness_follow_up(&awaited, "navigation.await_readiness", &route)?;
    reply(
        &mut writer,
        &awaited,
        json!({"structuredContent":{"navigation":navigation_evidence("ready", 250)}}),
    )
    .await?;

    let verified =
        support::read_frame_answering_tab_urls(&mut reader, &mut writer, "mechanism_request")
            .await?;
    assert_readiness_follow_up(&verified, "navigation.verify_document", &route)?;
    reply(
        &mut writer,
        &verified,
        json!({"structuredContent":{"navigation":navigation_evidence("same", 251)}}),
    )
    .await?;
    Ok(())
}

fn assert_readiness_follow_up(
    request: &Value,
    expected_mechanism: &str,
    route: &Value,
) -> anyhow::Result<()> {
    ensure!(
        request["mechanism"] == expected_mechanism
            && request["guid"] == *route
            && request["input"]["tab"] == NATIVE_TAB_ID
            && request["input"]["navigation_token"] == NAVIGATION_TOKEN
            && request["input"]["document_handle"] == DOCUMENT_HANDLE,
        "readiness follow-up changed transaction or routing identity: {request}"
    );
    Ok(())
}

fn atomic_open_result(evidence: Value) -> Value {
    json!({
        "content":[{"type":"text","text":"Opened https://example.com/."}],
        "structuredContent":{
            "tabId":NATIVE_TAB_ID,
            "mcpGroupId":1,
            "tabs":[{
                "tabId":NATIVE_TAB_ID,
                "title":"Example",
                "url":NAVIGATION_URL
            }],
            "url":NAVIGATION_URL,
            "title":"Example",
            "created":true,
            "navigated":true,
            "navigation":evidence
        }
    })
}

fn navigation_evidence(state: &str, elapsed_ms: u64) -> Value {
    json!({
        "state":state,
        "navigation_token":NAVIGATION_TOKEN,
        "document_handle":DOCUMENT_HANDLE,
        "url":NAVIGATION_URL,
        "deadline_at_ms":READINESS_DEADLINE_MS,
        "elapsed_ms":elapsed_ms
    })
}

async fn reply<W>(writer: &mut W, request: &Value, result: Value) -> anyhow::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let response = json!({
        "id":request["id"],
        "type":"tool_response",
        "result":result
    });
    ghostlight_transport::host::write_message(writer, &serde_json::to_vec(&response)?).await?;
    Ok(())
}

fn has_forbidden_native_identity(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(field, value)| {
            FORBIDDEN_NATIVE_IDENTITY_FIELDS.contains(&field.as_str())
                || has_forbidden_native_identity(value)
        }),
        Value::Array(values) => values.iter().any(has_forbidden_native_identity),
        Value::String(text) => {
            FORBIDDEN_NATIVE_IDENTITY_FIELDS
                .iter()
                .any(|field| text.contains(field))
                || contains_standalone_native_tab_id(text)
        }
        _ => false,
    }
}

fn contains_standalone_native_tab_id(text: &str) -> bool {
    let native_id = NATIVE_TAB_ID.to_string();
    text.match_indices(&native_id).any(|(start, matched)| {
        let before = text[..start].chars().next_back();
        let after = text[start + matched.len()..].chars().next();
        !before.is_some_and(is_opaque_identity_character)
            && !after.is_some_and(is_opaque_identity_character)
    })
}

fn is_opaque_identity_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_native_identity_scan_covers_keys_strings_and_raw_fixture_id_text() {
        let fixture = support::creator_inventory_result(NATIVE_TAB_ID);
        assert!(has_forbidden_native_identity(
            &fixture["content"][0]["text"]
        ));
        assert!(has_forbidden_native_identity(
            &json!({"tabId":NATIVE_TAB_ID})
        ));
        assert!(has_forbidden_native_identity(&json!(
            "legacy inventory contains mcpGroupId"
        )));
        assert!(has_forbidden_native_identity(&json!(format!(
            "Created tab {NATIVE_TAB_ID}."
        ))));
        assert!(!has_forbidden_native_identity(&json!({
            "tab":{"id":format!("t_workspace_{NATIVE_TAB_ID}")}
        })));
    }
}
