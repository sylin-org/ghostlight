// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! MCP-edge multiplex and kill fan-out parity scenarios.

use std::io::{BufRead as _, BufReader, Write as _};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, ensure};
use serde_json::{json, Value};

use ghostlight_core::governance::audit::Recorder;
use ghostlight_core::governance::dispatch::Governance;
use ghostlight_core::governance::ports::AuditSink;
use ghostlight_core::hub::outbound::browser::Browser;

use crate::scenarios::Scenario;
use crate::support::{self, TempRoot};

type CreatorObservation = (i64, Value, Value);
type ExtensionTranscript = (Vec<CreatorObservation>, Vec<Value>);

pub(super) fn registry() -> Vec<Scenario> {
    vec![
        ("mcp-edge-two-client-multiplex", two_mcp_edge_multiplex),
        ("legacy-hub-kill-audit-fanout", kill_audit_fanout),
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

fn two_mcp_edge_multiplex() -> anyhow::Result<()> {
    let tmp = TempRoot::new("hub-two-mcp-edge")?;
    let endpoint = support::unique_endpoint("hub-two-mcp-edge");
    let log_dir = tmp.path().join("logs");
    let _service = support::spawn_service(&endpoint, &log_dir)?;
    let mut edge_a = support::spawn_mcp_edge(&endpoint, &log_dir)?;
    let mut edge_b = support::spawn_mcp_edge(&endpoint, &log_dir)?;
    let mut stdin_a = edge_a
        .stdin
        .take()
        .ok_or_else(|| anyhow!("MCP edge A stdin"))?;
    let mut stdin_b = edge_b
        .stdin
        .take()
        .ok_or_else(|| anyhow!("MCP edge B stdin"))?;
    let mut reader_a = BufReader::new(
        edge_a
            .stdout
            .take()
            .ok_or_else(|| anyhow!("MCP edge A stdout"))?,
    );
    let mut reader_b = BufReader::new(
        edge_b
            .stdout
            .take()
            .ok_or_else(|| anyhow!("MCP edge B stdout"))?,
    );
    write_line(&mut stdin_a, &support::initialize_2025(1, "lightbox-a"))?;
    write_line(&mut stdin_b, &support::initialize_2025(1, "lightbox-b"))?;
    ensure!(read_line(&mut reader_a)?["id"] == 1);
    ensure!(read_line(&mut reader_b)?["id"] == 1);
    write_line(&mut stdin_a, &support::initialized_2025())?;
    write_line(&mut stdin_b, &support::initialized_2025())?;

    let fake_endpoint = endpoint.clone();
    let extension = std::thread::spawn(move || -> anyhow::Result<ExtensionTranscript> {
        tokio::runtime::Runtime::new()?.block_on(async move {
            let stream = ghostlight_transport::ipc::connect(&fake_endpoint).await?;
            let (mut reader, mut writer) = tokio::io::split(stream);
            support::send_extension_attach_frames(&mut writer).await?;
            let mut tools = Vec::new();
            let mut creators: Vec<CreatorObservation> = Vec::new();
            while creators.len() < 2 || tools.len() < 2 {
                let bytes = ghostlight_transport::host::read_message(&mut reader)
                    .await?
                    .ok_or_else(|| anyhow!("extension link closed"))?;
                let value: Value = serde_json::from_slice(&bytes)?;
                match value["type"].as_str() {
                    Some("tab_url_request") => support::answer_tab_url(&mut writer, &value).await?,
                    Some("tool_request") => {
                        let reply = match value["tool"].as_str() {
                            Some("tabs_context_mcp") => {
                                let native = match creators.len() {
                                    0 => 101,
                                    1 => 202,
                                    _ => anyhow::bail!("unexpected third creator request"),
                                };
                                let guid = value["guid"].clone();
                                ensure!(guid.is_string());
                                let title = value["workspace"]["groupTitle"].clone();
                                ensure!(title.is_string());
                                creators.push((native, guid, title));
                                json!({
                                    "id": value["id"],
                                    "type": "tool_response",
                                    "result": support::creator_inventory_result(native),
                                })
                            }
                            Some("navigate") => {
                                let native = value["args"]["tabId"]
                                    .as_i64()
                                    .ok_or_else(|| anyhow!("navigate carried no native tab id"))?;
                                let creator_guid = creators
                                    .iter()
                                    .find(|(tab_id, _, _)| *tab_id == native)
                                    .map(|(_, guid, _)| guid)
                                    .ok_or_else(|| anyhow!("navigate targeted no creator tab"))?;
                                ensure!(&value["guid"] == creator_guid);
                                tools.push(value.clone());
                                json!({
                                    "id": value["id"],
                                    "type": "tool_response",
                                    "result": {"content":[{"type":"text","text":format!("navigated tabId={native}")}]},
                                })
                            }
                            other => anyhow::bail!("unexpected extension tool {other:?}: {value}"),
                        };
                        ghostlight_transport::host::write_message(
                            &mut writer,
                            &serde_json::to_vec(&reply)?,
                        )
                        .await?;
                    }
                    other => anyhow::bail!("unexpected extension frame {other:?}: {value}"),
                }
            }
            ghostlight_transport::host::write_message(
                &mut writer,
                &serde_json::to_vec(&json!({"type":"session_killed"}))?,
            )
            .await?;
            Ok((creators, tools))
        })
    });

    write_line(
        &mut stdin_a,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tabs_context_mcp","arguments":{}}}),
    )?;
    let context_a = read_line(&mut reader_a)?;
    ensure!(context_a["id"] == 2 && context_a["result"]["isError"] != true);
    let tab_a = support::creator_tab_id(&context_a)?;
    let (slot_a, native_a) = ghostlight_core::constants::tab_id::decode(tab_a);
    ensure!(slot_a != 0 && native_a == 101);

    write_line(
        &mut stdin_b,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tabs_context_mcp","arguments":{}}}),
    )?;
    let context_b = read_line(&mut reader_b)?;
    ensure!(context_b["id"] == 2 && context_b["result"]["isError"] != true);
    let tab_b = support::creator_tab_id(&context_b)?;
    let (slot_b, native_b) = ghostlight_core::constants::tab_id::decode(tab_b);
    ensure!(slot_b == slot_a && native_b == 202 && tab_b != tab_a);

    write_line(
        &mut stdin_a,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":tab_a,"url":"https://a.example.com"}}}),
    )?;
    write_line(
        &mut stdin_b,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":tab_b,"url":"https://b.example.com"}}}),
    )?;
    let reply_a = read_line(&mut reader_a)?;
    let reply_b = read_line(&mut reader_b)?;
    let text_a = reply_a["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let text_b = reply_b["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    ensure!(reply_a["id"] == 3 && text_a.contains("101") && !text_a.contains("202"));
    ensure!(reply_b["id"] == 3 && text_b.contains("202") && !text_b.contains("101"));

    let (creators, tools) = extension
        .join()
        .map_err(|_| anyhow!("fake extension panicked"))??;
    ensure!(creators.len() == 2 && tools.len() == 2);
    let mut tabs: Vec<i64> = tools
        .iter()
        .filter_map(|value| value["args"]["tabId"].as_i64())
        .collect();
    tabs.sort_unstable();
    ensure!(tabs == [101, 202]);
    ensure!(creators[0].1 != creators[1].1);
    let mut titles = creators
        .iter()
        .filter_map(|(_, _, title)| title.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    titles.sort();
    ensure!(titles == ["Ghostlight - lightbox-a", "Ghostlight - lightbox-b"]);

    write_line(
        &mut stdin_a,
        &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":tab_a,"url":"https://a.example.com"}}}),
    )?;
    write_line(
        &mut stdin_b,
        &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":tab_b,"url":"https://b.example.com"}}}),
    )?;
    for reply in [read_line(&mut reader_a)?, read_line(&mut reader_b)?] {
        ensure!(reply["id"] == 4 && reply["result"]["isError"] == true);
        ensure!(reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("ended the browser session"));
    }
    Ok(())
}

fn kill_audit_fanout() -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let tmp = TempRoot::new("hub-kill-fanout")?;
        let names = ["client-a", "client-b", "client-c"];
        let browser = Browser::new();
        let mut paths = Vec::new();
        let mut handles = Vec::new();
        for name in names {
            let path = tmp.path().join(format!("{name}.jsonl"));
            let governance = Arc::new(Governance::all_open(
                Arc::new(Recorder::to_file(path.clone())) as Arc<dyn AuditSink>,
            ));
            governance.set_client(name, "1.0.0");
            let handle = {
                let governance = Arc::clone(&governance);
                browser.register_session_kill_hook(move || governance.record_session_killed())
            };
            paths.push(path);
            handles.push(handle);
        }
        let (browser_side, mut extension) = tokio::io::duplex(64 * 1024);
        let attached = browser.clone();
        tokio::spawn(async move {
            let _ = attached.attach(browser_side).await;
        });
        let hello = ghostlight_transport::handshake::browser_hello_bytes(1, None);
        ghostlight_transport::host::write_message(&mut extension, &hello).await?;
        let identity = serde_json::to_vec(&json!({
            "type": ghostlight_transport::handshake::EXTENSION_IDENTITY_TYPE,
            ghostlight_transport::handshake::BROWSER_ID_FIELD: "lightbox-hub-kill",
        }))?;
        ghostlight_transport::host::write_message(&mut extension, &identity).await?;
        for _ in 0..200 {
            if browser.is_connected() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        ensure!(browser.is_connected());
        ghostlight_transport::host::write_message(
            &mut extension,
            &serde_json::to_vec(&json!({"type":"session_killed"}))?,
        )
        .await?;
        for _ in 0..200 {
            if browser.is_killed() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        ensure!(browser.is_killed());
        tokio::time::sleep(Duration::from_millis(50)).await;
        for (name, path) in names.iter().zip(paths) {
            let content = std::fs::read_to_string(path)?;
            ensure!(content.lines().count() == 1);
            let record: Value = serde_json::from_str(content.trim())?;
            let keys: Vec<&str> = record
                .as_object()
                .ok_or_else(|| anyhow!("audit record is not an object"))?
                .keys()
                .map(String::as_str)
                .collect();
            ensure!(keys == ["event_id", "ts", "identity", "client", "event", "manifest"]);
            ensure!(record["event"] == "session_killed");
            ensure!(record["client"]["name"] == *name);
        }
        drop(handles);
        Ok(())
    })
}
