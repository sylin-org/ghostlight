// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `ghostlight demo-brief`: a short, visible launch-brief story for README hero capture.
//!
//! The story drives the public Sylin stage as an ordinary MCP client through the installed relay.
//! It reads one stable interactive surface, writes five exact refs as separately paced visual
//! moments, then uses one visible click for submission. The page owns only its quiet completion
//! transition; Ghostlight owns every blue control and activity cue.

use crate::demo_client::{parse_tab_id, Client, RefInventory, DEMO_POLICY};
use anyhow::{anyhow, Context, Result};
use serde_json::json;
use std::time::Duration;

const BRIEF_ROUTE: &str = "brief/";
const READ_SCAN_DURATION: Duration = Duration::from_millis(1600);

/// Operator-tunable timing for the short capture story.
#[derive(Debug, Clone, Copy)]
pub struct Pacing {
    /// Seconds between the fill, selections, and submit action.
    pub step_secs: f64,
    /// Seconds to hold the loaded stage before reading begins.
    pub setup_secs: f64,
    /// Seconds to hold the completed brief before the command exits.
    pub success_secs: f64,
}

/// Run the public launch-brief story through the ordinary Ghostlight MCP path.
pub fn run(base_url: &str, pacing: Pacing) -> Result<()> {
    let stage = stage_url(base_url);
    let runtime = tokio::runtime::Runtime::new().context("build the demo-brief tokio runtime")?;
    runtime.block_on(drive(stage, pacing))
}

fn stage_url(base_url: &str) -> String {
    format!("{}/{BRIEF_ROUTE}", base_url.trim_end_matches('/'))
}

async fn drive(stage: String, pacing: Pacing) -> Result<()> {
    println!("Ghostlight demo-brief");
    println!("  stage : {stage}");
    println!("  story : read, fill, choose, and complete one local launch brief");
    println!(
        "  note  : keep Chrome visible; the active story is designed for a 1024 x 800 recording."
    );

    let mut client = Client::spawn(Duration::from_secs_f64(pacing.step_secs.max(0.0))).await?;

    step("Connect through the ordinary MCP relay");
    client
        .request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "ghostlight-demo-brief",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "_meta": { "ghostlightSessionPolicy": DEMO_POLICY }
            }),
        )
        .await
        .context(
            "initialize (is a Ghostlight service running with the extension attached? run `ghostlight doctor`)",
        )?;
    client
        .notify("notifications/initialized", json!({}))
        .await?;

    step("Open the public launch-brief stage");
    let created = client.call_tool("tabs_create_mcp", json!({})).await?;
    let tab_id = parse_tab_id(&created)
        .ok_or_else(|| anyhow!("could not read the new tab id from: {created}"))?;
    client
        .call_tool("navigate", json!({ "tabId": tab_id, "url": stage }))
        .await?;
    client
        .call_tool(
            "wait_for",
            json!({ "tabId": tab_id, "text": "Create a launch brief" }),
        )
        .await?;

    let setup = pacing.setup_secs.max(0.0);
    if setup > 0.0 {
        println!("   (holding {setup:.1}s before the story begins)");
        tokio::time::sleep(Duration::from_secs_f64(setup)).await;
    }

    step("Read the form and keep its stable controls");
    let refs = RefInventory::read(
        &mut client,
        tab_id,
        &[
            "Project",
            "Owner",
            "Summary",
            "Include screenshots",
            "Keep data local",
            "Create brief",
        ],
    )
    .await?;
    tokio::time::sleep(READ_SCAN_DURATION).await;

    step("Fill the brief and choose its two constraints");
    for (name, value) in [
        ("Project", json!("Moonlight Notes")),
        ("Owner", json!("Maya Chen")),
        (
            "Summary",
            json!("Turn field observations into a shared release brief."),
        ),
        ("Include screenshots", json!(true)),
        ("Keep data local", json!(true)),
    ] {
        client
            .call_tool(
                "form_input",
                json!({ "tabId": tab_id, "ref": refs.require(name)?, "value": value }),
            )
            .await?;
        client.pause().await;
    }

    step("Create the brief and hold the completed state");
    click_ref(&mut client, tab_id, refs.require("Create brief")?).await?;
    client
        .call_tool(
            "wait_for",
            json!({
                "tabId": tab_id,
                "text": "Moonlight Notes is ready for review.",
                "settle": false
            }),
        )
        .await?;
    tokio::time::sleep(Duration::from_secs_f64(pacing.success_secs.max(0.0))).await;

    println!("\nDemo brief complete. The tab remains on the final frame.");
    Ok(())
}

async fn click_ref(client: &mut Client, tab_id: i64, element_ref: &str) -> Result<()> {
    client
        .call_tool(
            "computer",
            json!({ "action": "left_click", "tabId": tab_id, "ref": element_ref }),
        )
        .await?;
    Ok(())
}

fn step(message: &str) {
    println!("\n>> {message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_public_and_preview_stage_urls() {
        assert_eq!(
            stage_url("https://sylin.org/ghostlight/demo"),
            "https://sylin.org/ghostlight/demo/brief/"
        );
        assert_eq!(
            stage_url("http://localhost:8080/ghostlight/demo/"),
            "http://localhost:8080/ghostlight/demo/brief/"
        );
    }
}
