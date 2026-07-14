// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The session-owned `gif_creator` orchestrator (ADR-0073).
//!
//! The extension is a thin CDP capture executor. This handler coordinates transactional start,
//! final-frame stop, memory-only state, immutable export, and truthful structured results.

use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::b64;
use crate::mcp::outcome::{CallOutcome, LocalCtx, LocalFuture};
use crate::recording::action;
use crate::recording::{RecordingState, RecordingSummary, StopReason, SurfaceId};

/// Service-chosen CDP capture mechanics (ADR-0053 D2).
const SCREENCAST_QUALITY: u32 = 70;
const SCREENCAST_MAX_SIDE: u32 = 1568;
const SCREENCAST_MIN_INTERVAL_MS: u32 = 200;

pub(crate) fn gif_creator_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move { run(ctx).await })
}

fn outcome(text: impl Into<String>, summary: Option<&RecordingSummary>) -> CallOutcome {
    let mut result = json!({ "content": [{ "type": "text", "text": text.into() }] });
    if let (Some(object), Some(summary)) = (result.as_object_mut(), summary) {
        object.insert("structuredContent".to_string(), summary_value(summary));
    }
    CallOutcome::Success { result }
}

fn summary_value(summary: &RecordingSummary) -> Value {
    let next = match summary.state {
        RecordingState::Starting => json!(["status"]),
        RecordingState::Recording => json!(["export", "stop_recording", "clear"]),
        RecordingState::Finalizing => json!(["status"]),
        RecordingState::Frozen | RecordingState::Interrupted => json!(["export", "clear"]),
        RecordingState::Erased | RecordingState::Expired => json!(["start_recording"]),
    };
    json!({
        "recording_id": summary.id.as_str(),
        "state": summary.state.as_str(),
        "storage": "memory_only",
        "browser_slot": summary.surface.slot,
        "native_tab_id": summary.surface.native_tab,
        "frame_count": summary.frame_count,
        "bytes_held": summary.bytes_held,
        "duration_ms": summary.duration_ms,
        "idle_remaining_ms": summary.idle_remaining_ms,
        "hard_remaining_ms": summary.hard_remaining_ms,
        "expires_at_ms": summary.expires_at_ms,
        "stop_reason": summary.stop_reason.map(StopReason::as_str),
        "auto_stopped": matches!(
            summary.stop_reason,
            Some(StopReason::IdleTimeout | StopReason::HardTimeout | StopReason::LeaseExpired)
        ),
        "content_logged": false,
        "next": next,
    })
}

/// The first text content block of an extension reply (internal ops answer in `text(...)` shape).
fn first_text(reply: &Value) -> Option<&str> {
    reply
        .get("content")?
        .as_array()?
        .iter()
        .find(|block| block.get("type").and_then(Value::as_str) == Some("text"))?
        .get("text")?
        .as_str()
}

async fn finalize(
    ctx: &LocalCtx<'_>,
    surface: SurfaceId,
    reason: StopReason,
) -> Option<RecordingSummary> {
    let recordings = ctx.browser.recordings();
    match recordings.begin_finalizing(ctx.guid, surface) {
        Ok(ticket) => {
            let stopped = ctx
                .browser
                .stop_recording_capture(ctx.guid, &ticket)
                .await
                .is_ok();
            recordings.finish_finalizing(
                &ticket,
                stopped,
                if stopped {
                    reason
                } else {
                    StopReason::FinalizeFailed
                },
            )
        }
        Err(summary) => summary,
    }
}

async fn run(ctx: LocalCtx<'_>) -> CallOutcome {
    let action = ctx.args.get("action").and_then(Value::as_str).unwrap_or("");
    let Some(tab) = ctx.args.get("tabId").and_then(Value::as_i64) else {
        return CallOutcome::Failure {
            error: crate::ToolError::binary("gif_creator requires a numeric tabId"),
        };
    };
    let Some(surface) = ctx.browser.recording_surface(tab) else {
        return CallOutcome::Failure {
            error: crate::ToolError::extension("Browser extension not connected"),
        };
    };
    let recordings = ctx.browser.recordings();

    match action {
        action::START => {
            ctx.browser.ensure_recording_supervisor();
            let ticket = match recordings.begin_start(ctx.guid, surface) {
                Ok(ticket) => ticket,
                Err(summary) => {
                    return outcome(
                        "A recording is already active for this tab; it was not replaced.",
                        Some(&summary),
                    )
                }
            };
            let args = json!({
                "tabId": tab,
                "recordingId": ticket.id.as_str(),
                "generation": ticket.generation,
                "quality": SCREENCAST_QUALITY,
                "maxSide": SCREENCAST_MAX_SIDE,
                "minIntervalMs": SCREENCAST_MIN_INTERVAL_MS,
                "leaseMs": crate::recording::HEALTH_LEASE.as_millis() as u64,
                "hardTimeoutMs": crate::recording::HARD_TIMEOUT.as_millis() as u64,
            });
            match ctx.browser.call(ctx.guid, "gif_capture_start", &args).await {
                Err(error) => {
                    recordings.fail_start(&ticket);
                    CallOutcome::Failure { error }
                }
                Ok(reply) => {
                    let parsed = first_text(&reply)
                        .and_then(|text| serde_json::from_str::<Value>(text).ok());
                    let seeded = parsed
                        .as_ref()
                        .and_then(|value| value.get("seeded"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let vp_w = parsed
                        .as_ref()
                        .and_then(|value| value.get("vpW"))
                        .and_then(Value::as_f64);
                    let Some(summary) = recordings.commit_start(&ticket, vp_w) else {
                        return CallOutcome::Failure {
                            error: crate::ToolError::binary(
                                "Recording start reply no longer matched its generation",
                            ),
                        };
                    };
                    outcome(
                        format!(
                            "Recording started ({seeded} seed frame(s)). Continue browser work, then export; export will stop recording automatically."
                        ),
                        Some(&summary),
                    )
                }
            }
        }
        action::STOP => match finalize(&ctx, surface, StopReason::Explicit).await {
            Some(summary) => outcome(
                format!(
                    "Recording {} with {} frame(s) kept.",
                    if summary.state == RecordingState::Frozen {
                        "stopped"
                    } else {
                        summary.state.as_str()
                    },
                    summary.frame_count
                ),
                Some(&summary),
            ),
            None => outcome("No recording for this tab.", None),
        },
        action::STATUS => match recordings.status(ctx.guid, surface) {
            Some(summary) => outcome(
                format!("Recording state: {}.", summary.state.as_str()),
                Some(&summary),
            ),
            None => outcome("No recording for this tab.", None),
        },
        action::CLEAR => {
            let ticket = recordings.ticket(ctx.guid, surface);
            recordings.clear(ctx.guid, surface, StopReason::Cleared);
            if let Some(ticket) = ticket {
                ctx.browser.cancel_recording_capture(&ticket);
            }
            let summary = recordings.status(ctx.guid, surface);
            outcome("Recording erased from memory.", summary.as_ref())
        }
        action::EXPORT => {
            let coordinate = ctx.args.get("coordinate").filter(|value| !value.is_null());
            let element_ref = ctx.args.get("ref").filter(|value| !value.is_null());
            let download = ctx.args.get("download").and_then(Value::as_bool) == Some(true);
            let delivery_count = usize::from(coordinate.is_some())
                + usize::from(element_ref.is_some())
                + usize::from(download);
            if delivery_count != 1 {
                return CallOutcome::Failure {
                    error: crate::ToolError::invalid_request(
                        "export requires exactly one delivery target: download:true, coordinate, or ref",
                    ),
                };
            }
            let Some(summary) = finalize(&ctx, surface, StopReason::Explicit).await else {
                return outcome(
                    "No recording to export. Start one with action=start_recording.",
                    None,
                );
            };
            if !matches!(
                summary.state,
                RecordingState::Frozen | RecordingState::Interrupted
            ) {
                return outcome(
                    format!(
                        "Recording is {}; wait for finalization before exporting.",
                        summary.state.as_str()
                    ),
                    Some(&summary),
                );
            }
            let frames = recordings.frames(ctx.guid, surface);
            if frames.is_empty() {
                return outcome(
                    "The recording contains no exportable frames.",
                    Some(&summary),
                );
            }
            let count = frames.len();
            let options = ctx.args.get("options").cloned().unwrap_or(Value::Null);
            let encoded = tokio::task::spawn_blocking(move || {
                crate::gif::encode_recording(&frames, &options)
            })
            .await;
            let gif = match encoded {
                Ok(Ok(gif)) => Zeroizing::new(gif),
                Ok(Err(error)) => {
                    return CallOutcome::Failure {
                        error: crate::ToolError::binary(format!("GIF encoding failed: {error}")),
                    }
                }
                Err(error) => {
                    return CallOutcome::Failure {
                        error: crate::ToolError::binary(format!(
                            "GIF encoding task failed: {error}"
                        )),
                    }
                }
            };
            if !recordings.delivery_allowed(ctx.guid, surface, &summary.id) {
                return CallOutcome::Failure {
                    error: crate::ToolError::binary(
                        "Recording export was revoked before delivery; captured bytes were erased",
                    ),
                };
            }
            let bytes = gif.len();
            let data = b64::encode(&gif);
            let filename = ctx
                .args
                .get("filename")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .unwrap_or("recording.gif");

            if coordinate.is_some() || element_ref.is_some() {
                let mut args = json!({
                    "tabId": tab,
                    "data": data,
                    "filename": filename,
                    "mimeType": "image/gif",
                });
                if let Some(coordinate) = coordinate {
                    args["coordinate"] = coordinate.clone();
                }
                if let Some(element_ref) = element_ref {
                    args["ref"] = element_ref.clone();
                }
                return match ctx
                    .browser
                    .call_with_delivery_outcome(ctx.guid, "upload_image_exec", &args)
                    .await
                {
                    Err(failure) if failure.outcome_unknown => {
                        let mut uncertain = summary_value(&summary);
                        uncertain["delivery"] = json!("outcome_unknown");
                        uncertain["acceptance"] = json!("unknown");
                        uncertain["retry_safe"] = json!(false);
                        uncertain["gif_bytes"] = json!(bytes);
                        let mut result = json!({ "content": [{
                            "type": "text",
                            "text": format!(
                                "GIF delivery may have reached the page, but no conclusive reply arrived. Do not retry automatically; inspect the page first. ({count} frame(s), {bytes} bytes.)"
                            )
                        }] });
                        result["structuredContent"] = uncertain;
                        CallOutcome::Success { result }
                    }
                    Err(failure) => CallOutcome::Failure {
                        error: failure.error,
                    },
                    Ok(reply) => {
                        let detail = first_text(&reply).unwrap_or("Drop event dispatched.");
                        let mut delivered = summary_value(&summary);
                        delivered["delivery"] = json!("dispatched");
                        delivered["acceptance"] = json!("unverified");
                        delivered["retry_safe"] = json!(false);
                        delivered["gif_bytes"] = json!(bytes);
                        let mut result = json!({ "content": [{
                            "type": "text",
                            "text": format!(
                                "{detail} The page's acceptance is unverified ({count} frame(s), {bytes} bytes)."
                            )
                        }] });
                        result["structuredContent"] = delivered;
                        CallOutcome::Success { result }
                    }
                };
            }

            if download {
                let mut prepared = summary_value(&summary);
                prepared["delivery"] = json!("prepared_for_client");
                prepared["gif_bytes"] = json!(bytes);
                return CallOutcome::Success {
                    result: json!({
                        "content": [
                            { "type": "text", "text": format!(
                                "Prepared an animated GIF for the client: {count} frame(s), {bytes} bytes."
                            ) },
                            { "type": "image", "data": data, "mimeType": "image/gif" }
                        ],
                        "structuredContent": prepared,
                    }),
                };
            }

            unreachable!("delivery target count was validated before encoding")
        }
        other => outcome(format!("Unknown gif_creator action: {other}."), None),
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
