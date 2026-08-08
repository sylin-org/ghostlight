// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Executable R4 evidence for negotiated service-to-extension mechanism wire skew.
//!
//! The new/new and new-service/old-extension scenarios run the production service and MCP edge
//! against a fake extension over the real owner and browser IPC boundaries. The
//! old-service/new-extension scenario runs the shipped extension adapter under Node because an
//! old production binary cannot be reconstructed honestly from the current source tree. That
//! narrower proof executes the same dual-reader function imported by the service worker.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, ensure};
use serde_json::{json, Value};

use crate::scenarios::Scenario;
use crate::support::{self, TempRoot};

const MECHANISM_REQUEST_V1: &str = "mechanismRequestV1";
const LEGACY_READER_NODE_FIXTURE: &str = r#"
const assert = require("node:assert/strict");
const fs = require("node:fs");
const wire = require("./extension/lib/mechanism-wire.js");
const worker = fs.readFileSync("./extension/service-worker.js", "utf8");
assert.match(worker, /GhostlightMechanismWire\.normalizeIncomingRequest\(msg\)/);
const frame = {
  id: "old-1",
  type: "tool_request",
  tool: "tabs_context_mcp",
  args: { createIfEmpty: true },
  guid: "workspace-1",
};
const normalized = wire.normalizeIncomingRequest(frame);
assert.strictEqual(normalized, frame);
assert.deepStrictEqual(normalized, frame);
process.stdout.write("legacy dual-reader passed\n");
"#;

/// Return the three required R4 skew scenarios under their stable batch names.
pub fn registry() -> Vec<Scenario> {
    vec![
        (
            "mechanism_wire_new_new",
            mechanism_wire_new_new as fn() -> anyhow::Result<()>,
        ),
        (
            "mechanism_wire_new_service_old_extension",
            mechanism_wire_new_service_old_extension,
        ),
        (
            "mechanism_wire_old_service_new_extension",
            mechanism_wire_old_service_new_extension,
        ),
    ]
}

fn write_line(stdin: &mut std::process::ChildStdin, value: &Value) -> anyhow::Result<()> {
    serde_json::to_writer(&mut *stdin, value)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn read_line(reader: &mut BufReader<std::process::ChildStdout>) -> anyhow::Result<Value> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    ensure!(!line.is_empty(), "MCP edge stdout closed");
    Ok(serde_json::from_str(line.trim_end())?)
}

/// A feature-bearing identity selects the semantic envelope and keeps canonical input intact.
fn mechanism_wire_new_new() -> anyhow::Result<()> {
    run_service_extension_skew(
        "mechanism-wire-new-new",
        &[MECHANISM_REQUEST_V1],
        ExpectedRequest::Mechanism,
    )
}

/// An identity without the feature selects the exact covered legacy request envelope.
fn mechanism_wire_new_service_old_extension() -> anyhow::Result<()> {
    run_service_extension_skew(
        "mechanism-wire-new-service-old-extension",
        &[],
        ExpectedRequest::LegacyTool,
    )
}

#[derive(Clone, Copy)]
enum ExpectedRequest {
    Mechanism,
    LegacyTool,
}

impl ExpectedRequest {
    const fn frame_type(self) -> &'static str {
        match self {
            Self::Mechanism => "mechanism_request",
            Self::LegacyTool => "tool_request",
        }
    }

    fn assert_frame(self, request: &Value) -> anyhow::Result<()> {
        ensure!(
            request["id"].is_string(),
            "request id is not a string: {request}"
        );
        ensure!(
            request["guid"].is_string(),
            "request guid is not a string: {request}"
        );
        match self {
            Self::Mechanism => {
                ensure!(
                    request["mechanism"] == "workspace.tabs.inspect",
                    "new extension received the wrong mechanism: {request}"
                );
                ensure!(
                    request["input"] == json!({}),
                    "canonical workspace.tabs.inspect input changed: {request}"
                );
                ensure!(
                    request.get("tool").is_none() && request.get("args").is_none(),
                    "typed mechanism frame leaked legacy aliases: {request}"
                );
            }
            Self::LegacyTool => {
                ensure!(
                    request["tool"] == "tabs_context_mcp",
                    "old extension received the wrong legacy alias: {request}"
                );
                ensure!(
                    request["args"] == json!({}),
                    "legacy tabs_context_mcp args changed: {request}"
                );
                ensure!(
                    request.get("mechanism").is_none() && request.get("input").is_none(),
                    "old extension fallback leaked typed mechanism fields: {request}"
                );
            }
        }
        Ok(())
    }
}

fn run_service_extension_skew(
    tag: &str,
    features: &'static [&'static str],
    expected: ExpectedRequest,
) -> anyhow::Result<()> {
    let tmp = TempRoot::new(tag)?;
    let endpoint = support::unique_endpoint(tag);
    let _service = support::spawn_service(&endpoint, tmp.path())?;
    let mut edge = support::spawn_mcp_edge(&endpoint, tmp.path())?;
    let mut stdin = edge.stdin.take().ok_or_else(|| anyhow!("MCP edge stdin"))?;
    let mut reader = BufReader::new(
        edge.stdout
            .take()
            .ok_or_else(|| anyhow!("MCP edge stdout"))?,
    );

    write_line(&mut stdin, &support::initialize_2025(1, "lightbox-wire"))?;
    ensure!(read_line(&mut reader)?["id"] == 1);
    write_line(&mut stdin, &support::initialized_2025())?;

    let extension_endpoint = endpoint.clone();
    let extension = std::thread::spawn(move || -> anyhow::Result<()> {
        tokio::runtime::Runtime::new()?.block_on(async move {
            let stream = ghostlight_transport::ipc::connect(&extension_endpoint).await?;
            let (mut extension_reader, mut extension_writer) = tokio::io::split(stream);
            if features.is_empty() {
                support::send_extension_attach_frames(&mut extension_writer).await?;
            } else {
                support::send_extension_attach_frames_with_features(
                    &mut extension_writer,
                    features,
                )
                .await?;
            }
            let request = support::read_frame_answering_tab_urls(
                &mut extension_reader,
                &mut extension_writer,
                expected.frame_type(),
            )
            .await?;
            expected.assert_frame(&request)?;
            let reply = json!({
                "id": request["id"],
                "type": "tool_response",
                "result": support::creator_inventory_result(1),
            });
            ghostlight_transport::host::write_message(
                &mut extension_writer,
                &serde_json::to_vec(&reply)?,
            )
            .await?;
            Ok(())
        })
    });

    write_line(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "tabs_context_mcp", "arguments": {} },
        }),
    )?;
    let response = read_line(&mut reader)?;
    ensure!(
        response["id"] == 2 && response["result"]["isError"] != true,
        "tabs_context_mcp failed through skew path: {response}"
    );
    let tab_id = support::creator_tab_id(&response)?;
    let (slot, native_tab) = ghostlight_core::constants::tab_id::decode(tab_id);
    ensure!(
        slot != 0 && native_tab == 1,
        "wrong encoded tab id: {tab_id}"
    );
    extension
        .join()
        .map_err(|_| anyhow!("fake extension panicked"))??;
    drop(stdin);
    Ok(())
}

/// The new adapter accepts an old service frame unchanged through its service-worker reader.
fn mechanism_wire_old_service_new_extension() -> anyhow::Result<()> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow!("Lightbox manifest has no workspace root"))?;
    let output = Command::new("node")
        .current_dir(repository)
        .args(["-e", LEGACY_READER_NODE_FIXTURE])
        .output()?;
    ensure!(
        output.status.success(),
        "extension legacy-reader fixture failed (status {}):\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.trim() == "legacy dual-reader passed",
        "Node did not execute the intended legacy-reader fixture:\n{stdout}"
    );
    Ok(())
}
