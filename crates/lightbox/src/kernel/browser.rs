// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Browser handshake, output redaction, and parent-audit parity scenarios.

use std::io::{BufRead as _, BufReader, Write as _};
use std::time::Duration;

use anyhow::{anyhow, ensure};
use serde_json::{json, Value};

use crate::scenarios::Scenario;
use crate::support::{self, TempRoot};

pub(super) fn registry() -> Vec<Scenario> {
    vec![
        ("kernel-read-page-redaction", read_page_redaction),
        ("kernel-late-extension-wait", late_extension_wait),
        ("kernel-form-fill-parent-audit", form_fill_parent_audit),
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

fn start_pair(
    tag: &str,
) -> anyhow::Result<(TempRoot, String, support::ChildGuard, support::ChildGuard)> {
    let tmp = TempRoot::new(tag)?;
    let endpoint = support::unique_endpoint(tag);
    let service = support::spawn_service(&endpoint, tmp.path())?;
    let edge = support::spawn_mcp_edge(&endpoint, tmp.path())?;
    Ok((tmp, endpoint, service, edge))
}

fn read_page_redaction() -> anyhow::Result<()> {
    let (_tmp, endpoint, _service, mut edge) = start_pair("read-page-redaction")?;
    let mut stdin = edge.stdin.take().ok_or_else(|| anyhow!("MCP edge stdin"))?;
    let mut reader = BufReader::new(
        edge.stdout
            .take()
            .ok_or_else(|| anyhow!("MCP edge stdout"))?,
    );
    write_line(&mut stdin, &support::initialize_2025(1, "lightbox-browser"))?;
    ensure!(read_line(&mut reader)?["id"] == 1);
    write_line(&mut stdin, &support::initialized_2025())?;

    let extension_endpoint = endpoint.clone();
    let extension = std::thread::spawn(move || -> anyhow::Result<()> {
        tokio::runtime::Runtime::new()?.block_on(async move {
            let stream = ghostlight_transport::ipc::connect(&extension_endpoint).await?;
            let (mut ext_reader, mut ext_writer) = tokio::io::split(stream);
            support::send_extension_attach_frames(&mut ext_writer).await?;
            let creator = support::read_frame_answering_tab_urls(
                &mut ext_reader,
                &mut ext_writer,
                "tool_request",
            )
            .await?;
            ensure!(creator["tool"] == "tabs_create_mcp");
            let creator_guid = creator["guid"].clone();
            ensure!(creator_guid.is_string());
            let creator_reply = json!({
                "id": creator["id"],
                "type": "tool_response",
                "result": support::creator_inventory_result(1),
            });
            ghostlight_transport::host::write_message(
                &mut ext_writer,
                &serde_json::to_vec(&creator_reply)?,
            )
            .await?;
            let request = support::read_frame_answering_tab_urls(
                &mut ext_reader,
                &mut ext_writer,
                "tool_request",
            )
            .await?;
            ensure!(request["tool"] == "get_page_text");
            ensure!(request["args"]["tabId"] == 1);
            ensure!(request["guid"] == creator_guid);
            let reply = json!({
                "id": request["id"],
                "type": "tool_response",
                "result": {"content":[{
                    "type":"text",
                    "text":"textbox \"Password\" [ref_3] secret_value=\"hunter2\" type=\"password\""
                }]},
            });
            ghostlight_transport::host::write_message(
                &mut ext_writer,
                &serde_json::to_vec(&reply)?,
            )
            .await?;
            Ok(())
        })
    });

    write_line(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"browser_open_tab","arguments":{}}}),
    )?;
    let context = read_line(&mut reader)?;
    ensure!(context["id"] == 2 && context["result"]["isError"] != true);
    let tab = support::creator_tab_handle(&context)?;
    write_line(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"browser_read_page","arguments":{"tab":tab}}}),
    )?;
    let response = read_line(&mut reader)?;
    ensure!(
        response["id"] == 3 && response["result"]["isError"] != true,
        "browser_read_page failed: {response}"
    );
    let rendered = response.to_string();
    ensure!(rendered.contains("[value redacted]"), "{rendered}");
    ensure!(!rendered.contains("secret_value="), "{rendered}");
    ensure!(!rendered.contains("hunter2"), "{rendered}");
    extension
        .join()
        .map_err(|_| anyhow!("fake extension panicked"))??;
    drop(stdin);
    Ok(())
}

fn late_extension_wait() -> anyhow::Result<()> {
    let (tmp, endpoint, _service, mut edge) = start_pair("late-extension")?;
    let mut stdin = edge.stdin.take().ok_or_else(|| anyhow!("MCP edge stdin"))?;
    let mut reader = BufReader::new(
        edge.stdout
            .take()
            .ok_or_else(|| anyhow!("MCP edge stdout"))?,
    );
    write_line(&mut stdin, &support::initialize_2025(1, "lightbox-browser"))?;
    ensure!(read_line(&mut reader)?["id"] == 1);
    write_line(&mut stdin, &support::initialized_2025())?;

    let creator_endpoint = endpoint.clone();
    let creator_extension = std::thread::spawn(move || -> anyhow::Result<String> {
        tokio::runtime::Runtime::new()?.block_on(async move {
            let stream = ghostlight_transport::ipc::connect(&creator_endpoint).await?;
            let (mut ext_reader, mut ext_writer) = tokio::io::split(stream);
            support::send_extension_attach_frames(&mut ext_writer).await?;
            let creator = support::read_frame_answering_tab_urls(
                &mut ext_reader,
                &mut ext_writer,
                "tool_request",
            )
            .await?;
            ensure!(creator["tool"] == "tabs_create_mcp");
            let creator_guid = creator["guid"]
                .as_str()
                .ok_or_else(|| anyhow!("creator request carried no workspace guid"))?
                .to_string();
            let reply = json!({
                "id": creator["id"],
                "type": "tool_response",
                "result": support::creator_inventory_result(1),
            });
            ghostlight_transport::host::write_message(
                &mut ext_writer,
                &serde_json::to_vec(&reply)?,
            )
            .await?;
            Ok(creator_guid)
        })
    });

    write_line(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"browser_open_tab","arguments":{}}}),
    )?;
    let context = read_line(&mut reader)?;
    ensure!(context["id"] == 2 && context["result"]["isError"] != true);
    let tab = support::creator_tab_handle(&context)?;
    let creator_guid = creator_extension
        .join()
        .map_err(|_| anyhow!("creator extension panicked"))??;
    support::wait_extension_disconnected(tmp.path(), Duration::from_secs(5))?;

    let extension = std::thread::spawn(move || -> anyhow::Result<()> {
        tokio::runtime::Runtime::new()?.block_on(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let stream = ghostlight_transport::ipc::connect(&endpoint).await?;
            let (mut ext_reader, mut ext_writer) = tokio::io::split(stream);
            support::send_extension_attach_frames(&mut ext_writer).await?;
            let request = support::read_frame_answering_tab_urls(
                &mut ext_reader,
                &mut ext_writer,
                "tool_request",
            )
            .await?;
            ensure!(request["tool"] == "navigate");
            ensure!(request["args"]["tabId"] == 1);
            ensure!(request["guid"] == creator_guid);
            let reply = json!({
                "id": request["id"],
                "type":"tool_response",
                "result":{"content":[{"type":"text","text":"navigated"}]},
            });
            ghostlight_transport::host::write_message(
                &mut ext_writer,
                &serde_json::to_vec(&reply)?,
            )
            .await?;
            Ok(())
        })
    });

    write_line(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"browser_navigate","arguments":{"url":"https://example.com","tab":tab}}}),
    )?;
    let response = read_line(&mut reader)?;
    ensure!(response["id"] == 3 && response["result"]["isError"] != true);
    ensure!(response["result"]["structuredContent"]["status"] == "ok");
    ensure!(response["result"]["structuredContent"]["effect"] == "committed");
    extension
        .join()
        .map_err(|_| anyhow!("fake extension panicked"))??;
    drop(stdin);
    Ok(())
}

fn form_fill_parent_audit() -> anyhow::Result<()> {
    let tmp = TempRoot::new("form-fill-parent-audit")?;
    let endpoint = support::unique_endpoint("form-fill-parent-audit");
    let config_root = tmp.path().join("config");
    let config_dir = config_root.join("ghostlight");
    let audit_path = tmp.path().join("audit.jsonl");
    std::fs::create_dir_all(&config_dir)?;
    std::fs::write(
        config_dir.join("config.json"),
        serde_json::to_vec(&json!({"config":{
            "audit.enabled":true,
            "audit.destination":"file",
            "audit.file.path":audit_path.to_string_lossy(),
        }}))?,
    )?;
    let (_service, _port) =
        support::spawn_service_with_manage_web(&endpoint, tmp.path(), Some(&config_root))?;
    let mut edge = support::spawn_mcp_edge(&endpoint, tmp.path())?;
    let mut stdin = edge.stdin.take().ok_or_else(|| anyhow!("MCP edge stdin"))?;
    let mut reader = BufReader::new(
        edge.stdout
            .take()
            .ok_or_else(|| anyhow!("MCP edge stdout"))?,
    );
    write_line(&mut stdin, &support::initialize_2025(1, "lightbox-browser"))?;
    ensure!(read_line(&mut reader)?["id"] == 1);
    write_line(&mut stdin, &support::initialized_2025())?;

    let creator_endpoint = endpoint.clone();
    let creator_extension = std::thread::spawn(move || -> anyhow::Result<()> {
        tokio::runtime::Runtime::new()?.block_on(async move {
            let stream = ghostlight_transport::ipc::connect(&creator_endpoint).await?;
            let (mut ext_reader, mut ext_writer) = tokio::io::split(stream);
            support::send_extension_attach_frames(&mut ext_writer).await?;
            let creator = support::read_frame_answering_tab_urls(
                &mut ext_reader,
                &mut ext_writer,
                "tool_request",
            )
            .await?;
            ensure!(creator["tool"] == "tabs_create_mcp");
            let creator_guid = creator["guid"].clone();
            ensure!(creator_guid.is_string());
            let reply = json!({
                "id": creator["id"],
                "type": "tool_response",
                "result": support::creator_inventory_result(1),
            });
            ghostlight_transport::host::write_message(
                &mut ext_writer,
                &serde_json::to_vec(&reply)?,
            )
            .await?;
            Ok(())
        })
    });

    write_line(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"browser_open_tab","arguments":{}}}),
    )?;
    let context = read_line(&mut reader)?;
    ensure!(context["id"] == 2 && context["result"]["isError"] != true);
    let tab = support::creator_tab_handle(&context)?;
    creator_extension
        .join()
        .map_err(|_| anyhow!("creator extension panicked"))??;
    support::wait_extension_disconnected(tmp.path(), Duration::from_secs(5))?;

    write_line(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"browser_fill_form","arguments":{"tab":tab,"fields":[{"field":"Email","value":"a@b.c"}]}}}),
    )?;
    let response = read_line(&mut reader)?;
    ensure!(response["id"] == 3 && response["result"]["isError"] == true);
    ensure!(
        matches!(
            response["result"]["structuredContent"]["status"].as_str(),
            Some("blocked" | "not_dispatched" | "unavailable" | "outcome_unknown")
        ),
        "browser_fill_form returned the wrong disconnected-browser outcome: {response}"
    );
    drop(stdin);
    let _ = edge.wait();

    let audit: Vec<Value> = std::fs::read_to_string(&audit_path)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let parent = audit
        .iter()
        .find(|record| {
            record["tool"] == "browser_fill_form" && record.get("role").is_none_or(Value::is_null)
        })
        .ok_or_else(|| anyhow!("no browser_fill_form parent record: {audit:?}"))?;
    ensure!(parent["batch_id"].is_string());
    ensure!(parent["action"].is_null());
    ensure!(
        parent["required_capabilities"] == json!(["write"]),
        "unexpected fill parent audit record: {parent}"
    );
    let inspect_phase = audit
        .iter()
        .find(|record| record["tool"] == "browser_fill_form" && record["role"] == "mechanism_phase")
        .ok_or_else(|| anyhow!("no form-inspection mechanism phase: {audit:?}"))?;
    ensure!(inspect_phase["orchestrator"] == "browser_fill_form");
    ensure!(inspect_phase["required_capabilities"] == json!(["read"]));
    ensure!(inspect_phase["batch_id"] == parent["batch_id"]);
    ensure!(inspect_phase["step"] == 1);
    ensure!(inspect_phase["duration_ms"].is_u64());
    Ok(())
}
