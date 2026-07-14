// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Hub multiplex, kill fan-out, and two-phase adapter-wire parity scenarios.

use std::io::{BufRead as _, BufReader, Write as _};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, ensure};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _};

use ghostlight_core::governance::audit::Recorder;
use ghostlight_core::governance::dispatch::Governance;
use ghostlight_core::governance::ports::AuditSink;
use ghostlight_core::hub::outbound::browser::Browser;

use crate::scenarios::Scenario;
use crate::support::{self, TempRoot};

pub(super) fn registry() -> Vec<Scenario> {
    vec![
        ("legacy-hub-two-adapter-multiplex", two_adapter_multiplex),
        ("legacy-hub-kill-audit-fanout", kill_audit_fanout),
        ("legacy-hub-two-phase-wire", two_phase_wire),
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
    ensure!(!line.is_empty(), "adapter stdout closed");
    Ok(serde_json::from_str(line.trim_end())?)
}

fn two_adapter_multiplex() -> anyhow::Result<()> {
    let tmp = TempRoot::new("hub-two-adapter")?;
    let endpoint = support::unique_endpoint("hub-two-adapter");
    let log_dir = tmp.path().join("logs");
    let _service = support::spawn_service(&endpoint, &log_dir)?;
    let mut adapter_a = support::spawn_adapter(&endpoint, &log_dir)?;
    let mut adapter_b = support::spawn_adapter(&endpoint, &log_dir)?;
    let mut stdin_a = adapter_a
        .stdin
        .take()
        .ok_or_else(|| anyhow!("adapter A stdin"))?;
    let mut stdin_b = adapter_b
        .stdin
        .take()
        .ok_or_else(|| anyhow!("adapter B stdin"))?;
    let mut reader_a = BufReader::new(
        adapter_a
            .stdout
            .take()
            .ok_or_else(|| anyhow!("adapter A stdout"))?,
    );
    let mut reader_b = BufReader::new(
        adapter_b
            .stdout
            .take()
            .ok_or_else(|| anyhow!("adapter B stdout"))?,
    );
    write_line(
        &mut stdin_a,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    )?;
    write_line(
        &mut stdin_b,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    )?;
    ensure!(read_line(&mut reader_a)?["id"] == 1);
    ensure!(read_line(&mut reader_b)?["id"] == 1);

    let fake_endpoint = endpoint.clone();
    let extension = std::thread::spawn(move || -> anyhow::Result<(Vec<Value>, Vec<Value>)> {
        tokio::runtime::Runtime::new()?.block_on(async move {
            let stream = ghostlight_transport::ipc::connect(&fake_endpoint).await?;
            let (mut reader, mut writer) = tokio::io::split(stream);
            support::send_extension_attach_frames(&mut writer).await?;
            let mut groups = Vec::new();
            let mut tools = Vec::new();
            while groups.len() < 2 || tools.len() < 2 {
                let bytes = ghostlight_transport::host::read_message(&mut reader)
                    .await?
                    .ok_or_else(|| anyhow!("extension link closed"))?;
                let value: Value = serde_json::from_slice(&bytes)?;
                match value["type"].as_str() {
                    Some("tab_url_request") => support::answer_tab_url(&mut writer, &value).await?,
                    Some("group_request") => groups.push(value),
                    Some("tool_request") => {
                        let reply = json!({
                            "id": value["id"],
                            "type": "tool_response",
                            "result": {"content":[{"type":"text","text":format!("navigated tabId={}", value["args"]["tabId"])}]},
                        });
                        ghostlight_transport::host::write_message(
                            &mut writer,
                            &serde_json::to_vec(&reply)?,
                        )
                        .await?;
                        tools.push(value);
                    }
                    other => anyhow::bail!("unexpected extension frame {other:?}: {value}"),
                }
            }
            ghostlight_transport::host::write_message(
                &mut writer,
                &serde_json::to_vec(&json!({"type":"session_killed"}))?,
            )
            .await?;
            Ok((groups, tools))
        })
    });
    std::thread::sleep(Duration::from_millis(200));
    write_line(
        &mut stdin_a,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":101,"url":"https://a.example.com"}}}),
    )?;
    write_line(
        &mut stdin_b,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":202,"url":"https://b.example.com"}}}),
    )?;
    let reply_a = read_line(&mut reader_a)?;
    let reply_b = read_line(&mut reader_b)?;
    let text_a = reply_a["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let text_b = reply_b["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    ensure!(reply_a["id"] == 2 && text_a.contains("101") && !text_a.contains("202"));
    ensure!(reply_b["id"] == 2 && text_b.contains("202") && !text_b.contains("101"));

    let (groups, tools) = extension
        .join()
        .map_err(|_| anyhow!("fake extension panicked"))??;
    ensure!(groups.len() == 2 && tools.len() == 2);
    let mut tabs: Vec<i64> = tools
        .iter()
        .filter_map(|value| value["args"]["tabId"].as_i64())
        .collect();
    tabs.sort_unstable();
    ensure!(tabs == [101, 202]);
    ensure!(groups[0]["guid"] != groups[1]["guid"]);
    let mut group_tabs: Vec<Vec<i64>> = groups
        .iter()
        .map(|value| {
            value["tabIds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_i64)
                .collect()
        })
        .collect();
    group_tabs.sort();
    ensure!(group_tabs == [vec![101], vec![202]]);
    ensure!(groups.iter().all(|value| value["title"]
        .as_str()
        .is_some_and(|title| title.starts_with('\u{1F47B}'))));

    write_line(
        &mut stdin_a,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":101,"url":"https://a.example.com"}}}),
    )?;
    write_line(
        &mut stdin_b,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":202,"url":"https://b.example.com"}}}),
    )?;
    for reply in [read_line(&mut reader_a)?, read_line(&mut reader_b)?] {
        ensure!(reply["id"] == 3 && reply["result"]["isError"] == true);
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

fn two_phase_wire() -> anyhow::Result<()> {
    let tmp = TempRoot::new("hub-two-phase-wire")?;
    let endpoint = support::unique_endpoint("hub-two-phase-wire");
    let _service = support::spawn_service(&endpoint, tmp.path())?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let mut stream = ghostlight_transport::ipc::connect(&format!("{endpoint}-adapter")).await?;
        let hello = json!({"hub":1,"role":"adapter","guid":"00000000-0000-4000-8000-000000000000"});
        ghostlight_transport::host::write_message(&mut stream, &serde_json::to_vec(&hello)?)
            .await?;
        let proof = ghostlight_transport::host::read_message(&mut stream)
            .await?
            .ok_or_else(|| anyhow!("service sent no proof"))?;
        let proof: Value = serde_json::from_slice(&proof)?;
        ensure!(proof["role"] == "service-proof");
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
            .await?;
        let mut reader = tokio::io::BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        ensure!(serde_json::from_str::<Value>(line.trim_end())?["id"] == 1);
        Ok(())
    })
}
