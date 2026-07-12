// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `gif_creator` orchestrator (ADR-0053 Decision 6): the tool's brain lives in the binary.
//!
//! start/stop/clear command the extension's thin screencast relay through two internal operations
//! (`gif_capture_start` / `gif_capture_stop`); frames flow back as unsolicited `gif_frame` events
//! into the [`crate::hub::outbound::recording::RecordingStore`]; `export` reads the recorded
//! frames from disk and runs the pure Rust pipeline ([`crate::gif::encode_recording`]) under
//! `spawn_blocking`, returning the GIF as MCP image content (`download: true`) or handing the
//! bytes to the existing `upload_image_exec` drag-drop path (`coordinate`). The advertised schema,
//! per-action classification, and audit surface are unchanged -- this is the form_fill precedent
//! (ADR-0036 Decision 4) applied to an action_key tool: the pipeline enforces each action's grant
//! BEFORE dispatching here (gif_creator has no `action: None` variant, so it is never free-local).

use serde_json::{json, Value};

use crate::b64;
use crate::mcp::outcome::{CallOutcome, LocalCtx, LocalFuture};

/// Screencast parameters the service chooses and the relay applies (ADR-0053 D2): JPEG quality at
/// the source, the screenshot token-budget dimension cap, and the frame-thinning interval.
const SCREENCAST_QUALITY: u32 = 70;
const SCREENCAST_MAX_SIDE: u32 = 1568;
const SCREENCAST_MIN_INTERVAL_MS: u32 = 200;

pub(crate) fn gif_creator_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move { run(ctx).await })
}

fn text_outcome(text: impl Into<String>) -> CallOutcome {
    CallOutcome::Success {
        result: json!({ "content": [{ "type": "text", "text": text.into() }] }),
    }
}

/// The first text content block of an extension reply (internal ops answer in `text(...)` shape).
fn first_text(reply: &Value) -> Option<&str> {
    reply
        .get("content")?
        .as_array()?
        .iter()
        .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))?
        .get("text")?
        .as_str()
}

async fn run(ctx: LocalCtx<'_>) -> CallOutcome {
    let action = ctx.args.get("action").and_then(Value::as_str).unwrap_or("");
    let Some(tab) = ctx.args.get("tabId").and_then(Value::as_i64) else {
        return CallOutcome::Failure {
            error: crate::ToolError::binary("gif_creator requires a numeric tabId"),
        };
    };
    // ADR-0058: `tab` (from the client-facing args) stays COMPOSITE everywhere it is forwarded
    // to `ctx.browser.call(...)` below -- `Browser::call` decodes and routes it internally, the
    // same as every other tool. `RecordingStore`, though, is keyed by the extension's OWN native
    // tab id (fed by `handle_gif_frame`'s `gif_frame` events, which the extension reports in its
    // own native numbering); every LOCAL `recordings.*()` call here uses this decoded value
    // instead, so both sides agree on the same key.
    let native_tab = crate::constants::tab_id::decode(tab).1;
    let recordings = ctx.browser.recordings();

    match action {
        "start_recording" => {
            // A fresh recording discards any prior frames for the tab.
            recordings.start(native_tab);
            let args = json!({
                "tabId": tab,
                "quality": SCREENCAST_QUALITY,
                "maxSide": SCREENCAST_MAX_SIDE,
                "minIntervalMs": SCREENCAST_MIN_INTERVAL_MS,
            });
            match ctx.browser.call(ctx.guid, "gif_capture_start", &args).await {
                Err(error) => {
                    recordings.clear(native_tab);
                    CallOutcome::Failure { error }
                }
                Ok(reply) => {
                    // The relay reports `{"seeded": n, "vpW": w}` as JSON text; the seed frame
                    // itself arrived as a gif_frame event ahead of this reply.
                    let mut seeded = 0u64;
                    if let Some(text) = first_text(&reply) {
                        if let Ok(v) = serde_json::from_str::<Value>(text) {
                            seeded = v.get("seeded").and_then(Value::as_u64).unwrap_or(0);
                            if let Some(vp_w) = v.get("vpW").and_then(Value::as_f64) {
                                recordings.set_vp_w(native_tab, vp_w);
                            }
                        }
                    }
                    text_outcome(format!(
                        "Recording started ({seeded} frame(s) captured). Perform actions, then \
                         stop_recording and export with download:true."
                    ))
                }
            }
        }
        "stop_recording" => match recordings.stop(native_tab) {
            None => text_outcome("No active recording for this tab."),
            Some(count) => {
                // Best-effort: the screencast may already be gone with the tab or the debugger.
                let _ = ctx
                    .browser
                    .call(ctx.guid, "gif_capture_stop", &json!({ "tabId": tab }))
                    .await;
                text_outcome(format!(
                    "Recording stopped; {count} frame(s) kept. Use export with download:true to \
                     get the GIF."
                ))
            }
        },
        "clear" => {
            let _ = ctx
                .browser
                .call(ctx.guid, "gif_capture_stop", &json!({ "tabId": tab }))
                .await;
            recordings.clear(native_tab);
            text_outcome("Recording cleared.")
        }
        "export" => {
            let frames = recordings.frames(native_tab);
            if frames.is_empty() {
                return text_outcome(
                    "No frames to export. Start a recording first with action=start_recording.",
                );
            }
            let count = frames.len();
            let options = ctx.args.get("options").cloned().unwrap_or(Value::Null);
            // The pipeline is pure CPU work over all frames: off the async runtime it goes.
            let encoded = tokio::task::spawn_blocking(move || {
                crate::gif::encode_recording(&frames, &options)
            })
            .await;
            let gif = match encoded {
                Ok(Ok(gif)) => gif,
                Ok(Err(e)) => {
                    return CallOutcome::Failure {
                        error: crate::ToolError::binary(format!("GIF encoding failed: {e}")),
                    }
                }
                Err(e) => {
                    return CallOutcome::Failure {
                        error: crate::ToolError::binary(format!("GIF encoding task failed: {e}")),
                    }
                }
            };
            let kb = (gif.len() as f64 / 1024.0).round() as u64;
            let data = b64::encode(&gif);

            if let Some(coordinate) = ctx.args.get("coordinate").filter(|c| !c.is_null()) {
                // Drag-drop the encoded GIF onto the page at the coordinate, reusing the
                // upload_image drag-drop machinery (ADR-0050 D4 / ADR-0053 D6).
                let filename = ctx
                    .args
                    .get("filename")
                    .and_then(Value::as_str)
                    .filter(|f| !f.is_empty())
                    .unwrap_or("recording.gif");
                let args = json!({
                    "tabId": tab,
                    "coordinate": coordinate,
                    "data": data,
                    "filename": filename,
                    "mimeType": "image/gif",
                });
                return match ctx.browser.call(ctx.guid, "upload_image_exec", &args).await {
                    Err(error) => CallOutcome::Failure { error },
                    Ok(reply) => {
                        let output = first_text(&reply).unwrap_or("Dropped the GIF.");
                        text_outcome(format!("{output} ({count} frame(s), {kb} KB)."))
                    }
                };
            }

            if ctx.args.get("download").and_then(Value::as_bool) == Some(true) {
                return CallOutcome::Success {
                    result: json!({ "content": [
                        { "type": "text",
                          "text": format!("Exported an animated GIF: {count} frame(s), {kb} KB.") },
                        { "type": "image", "data": data, "mimeType": "image/gif" },
                    ]}),
                };
            }

            text_outcome(
                "export requires either download:true (return/download the GIF) or a coordinate \
                 [x, y] (drag-drop it onto a page element).",
            )
        }
        other => text_outcome(format!("Unknown gif_creator action: {other}.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_text_reads_the_text_block() {
        let reply = json!({ "content": [
            { "type": "image", "data": "x" },
            { "type": "text", "text": "hello" },
        ]});
        assert_eq!(first_text(&reply), Some("hello"));
        assert_eq!(first_text(&json!({})), None);
    }
}
