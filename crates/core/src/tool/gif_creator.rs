// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The workspace-owned `gif_creator` orchestrator (ADR-0073, amended by ADR-0096).
//!
//! The extension is a thin CDP capture executor. This handler coordinates transactional start,
//! final-frame stop, memory-only state, immutable export, and truthful structured results.

use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::b64;
use crate::browser::mechanism::{MechanismId, MechanismRequest};
use crate::hub::outbound::browser::DeliveryFailure;
use crate::recording::{CommitStartError, RecordingState, RecordingSummary, StopReason, SurfaceId};
use crate::tool::outcome::{
    delivery_failure_outcome, tool_error_outcome, CallOutcome, LocalCtx, LocalFuture,
};
use ghostlight_transport::operation::{IntentId, OperationKey};

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

fn recording_start_request(
    operation: OperationKey,
    tab: i64,
    recording_id: &str,
    generation: u64,
) -> MechanismRequest {
    MechanismRequest::for_operation(
        operation,
        MechanismId::RecordingStart,
        json!({
            "tab": tab,
            "recording_id": recording_id,
            "generation": generation,
            "quality": SCREENCAST_QUALITY,
            "max_side": SCREENCAST_MAX_SIDE,
            "min_interval_ms": SCREENCAST_MIN_INTERVAL_MS,
            "lease_ms": crate::recording::HEALTH_LEASE.as_millis() as u64,
            "hard_timeout_ms": crate::recording::HARD_TIMEOUT.as_millis() as u64,
        }),
    )
    .expect("recording start must be declared by its dynamic plan")
}

fn gif_upload_request(
    operation: OperationKey,
    tab: i64,
    data: String,
    filename: &str,
    args: &Value,
) -> MechanismRequest {
    let mut input = json!({
        "tab": tab,
        "data": data,
        "filename": filename,
        "mime_type": "image/gif",
    });
    if let Some(reference) = args.pointer("/target/ref") {
        input["target"] = json!({ "ref": reference });
    }
    if let Some(point) = args.get("point") {
        input["point"] = point.clone();
    }
    MechanismRequest::for_operation(operation, MechanismId::UploadImage, input)
        .expect("recording export upload must be declared by its dynamic plan")
}

fn recording_tab(args: &Value) -> Option<i64> {
    args.get("tab").and_then(Value::as_i64)
}

fn delivery_target_count(args: &Value) -> usize {
    usize::from(args.get("point").is_some_and(|value| !value.is_null()))
        + usize::from(
            args.pointer("/target/ref")
                .and_then(Value::as_str)
                .is_some_and(|reference| !reference.is_empty()),
        )
        + usize::from(args.get("download").and_then(Value::as_bool) == Some(true))
}

enum FinalizeOutcome {
    Missing,
    Ready {
        summary: RecordingSummary,
        facts: FinalizationFacts,
    },
    Partial(FinalizationPartial),
    Failed(DeliveryFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FinalizationFacts {
    stop_acknowledged: bool,
    recording_state_changed: bool,
    cancel_enqueued: bool,
}

impl FinalizationFacts {
    const NO_OP: Self = Self {
        stop_acknowledged: false,
        recording_state_changed: false,
        cancel_enqueued: false,
    };

    const fn effect(self) -> ghostlight_transport::operation::OperationEffect {
        if self.stop_acknowledged || self.recording_state_changed {
            ghostlight_transport::operation::OperationEffect::Committed
        } else if self.cancel_enqueued {
            ghostlight_transport::operation::OperationEffect::Dispatched
        } else {
            ghostlight_transport::operation::OperationEffect::None
        }
    }

    const fn has_effect(self) -> bool {
        !matches!(
            self.effect(),
            ghostlight_transport::operation::OperationEffect::None
        )
    }

    const fn stop_committed(self) -> bool {
        self.stop_acknowledged || self.cancel_enqueued
    }

    fn apply(self, structured: &mut Value) {
        structured["changed"] = json!(self.has_effect());
        structured["stop_committed"] = json!(self.stop_committed());
        structured["stop_acknowledged"] = json!(self.stop_acknowledged);
        structured["recording_state_changed"] = json!(self.recording_state_changed);
        structured["cancel_enqueued"] = json!(self.cancel_enqueued);
        structured["finalization_effect"] = json!(self.effect());
    }
}

struct FinalizationPartial {
    summary: Option<RecordingSummary>,
    facts: FinalizationFacts,
    kind: &'static str,
    detail: String,
}

fn outcome_with_structured(
    text: impl Into<String>,
    structured: Value,
    is_error: bool,
) -> CallOutcome {
    let mut result = json!({
        "content": [{"type":"text", "text": text.into()}],
        "structuredContent": structured,
    });
    if is_error {
        result["isError"] = json!(true);
    }
    CallOutcome::Success { result }
}

fn finalization_structured(summary: Option<&RecordingSummary>, facts: FinalizationFacts) -> Value {
    let mut structured = summary.map(summary_value).unwrap_or_else(|| json!({}));
    facts.apply(&mut structured);
    structured
}

fn stop_result(
    text: impl Into<String>,
    summary: Option<&RecordingSummary>,
    facts: FinalizationFacts,
    is_error: bool,
) -> CallOutcome {
    outcome_with_structured(text, finalization_structured(summary, facts), is_error)
}

fn start_result(
    text: impl Into<String>,
    summary: Option<&RecordingSummary>,
    changed: bool,
    is_error: bool,
    cancel_enqueued: bool,
) -> CallOutcome {
    let text = text.into();
    let mut structured = summary.map(summary_value).unwrap_or_else(|| json!({}));
    structured["changed"] = json!(changed);
    structured["start_acknowledged"] = json!(changed);
    structured["start_committed"] = json!(changed);
    structured["cancel_enqueued"] = json!(cancel_enqueued);
    if is_error {
        structured["retry_safe"] = json!(false);
        structured["blocker"] = json!({
            "kind":"start_acknowledged_after_state_change",
            "summary":text.clone(),
            "nextStep":"Inspect recording status before deciding whether to start again; do not retry automatically."
        });
    }
    outcome_with_structured(text, structured, is_error)
}

fn export_result(
    text: impl Into<String>,
    summary: Option<&RecordingSummary>,
    facts: FinalizationFacts,
    export_completed: bool,
    delivery: &str,
    is_error: bool,
) -> CallOutcome {
    let mut structured = finalization_structured(summary, facts);
    structured["changed"] = json!(facts.has_effect() || export_completed);
    structured["export_completed"] = json!(export_completed);
    structured["delivery"] = json!(delivery);
    outcome_with_structured(text, structured, is_error)
}

fn partial_export_result(
    mut structured: Value,
    facts: FinalizationFacts,
    kind: &str,
    summary: impl Into<String>,
    next_step: &str,
) -> CallOutcome {
    let summary = summary.into();
    facts.apply(&mut structured);
    structured["export_completed"] = json!(false);
    structured["delivery"] = json!("not_completed");
    structured["blocker"] = json!({
        "kind": kind,
        "summary": summary,
        "nextStep": next_step,
    });
    CallOutcome::Success {
        result: json!({
            "content": [{"type":"text", "text": summary}],
            "structuredContent": structured,
            "isError": true,
        }),
    }
}

fn partial_after_stop_error(
    summary: &RecordingSummary,
    facts: FinalizationFacts,
    kind: &str,
    detail: impl std::fmt::Display,
) -> CallOutcome {
    partial_export_result(
        summary_value(summary),
        facts,
        kind,
        format!(
            "The recording was stopped and kept, but export did not complete: {detail}"
        ),
        "Inspect the frozen recording before choosing whether to export it again; do not restart the recording automatically.",
    )
}

fn export_delivery_failure(
    summary: &RecordingSummary,
    facts: FinalizationFacts,
    failure: DeliveryFailure,
) -> CallOutcome {
    if failure.outcome_unknown || !facts.has_effect() {
        return delivery_failure_outcome(failure);
    }
    let kind = match &failure.error {
        crate::ToolError::Held { .. } => "delivery_paused_after_stop",
        crate::ToolError::AttentionRequired { .. } => "delivery_interrupted_after_stop",
        _ => "delivery_failed_after_stop",
    };
    partial_after_stop_error(summary, facts, kind, failure.error)
}

fn finish_after_acknowledgement(
    recordings: &crate::recording::RecordingCoordinator,
    ticket: &crate::recording::RecordingTicket,
    reason: StopReason,
) -> FinalizeOutcome {
    match recordings.finish_finalizing(ticket, true, reason) {
        Some(summary) => FinalizeOutcome::Ready {
            summary,
            facts: FinalizationFacts {
                stop_acknowledged: true,
                recording_state_changed: true,
                cancel_enqueued: false,
            },
        },
        None => FinalizeOutcome::Partial(FinalizationPartial {
            summary: None,
            facts: FinalizationFacts {
                stop_acknowledged: true,
                recording_state_changed: false,
                cancel_enqueued: false,
            },
            kind: "stop_acknowledged_state_race",
            detail: "The browser acknowledged the recording stop, but local recording state changed before finalization could be recorded."
                .to_string(),
        }),
    }
}

fn finish_after_failure(
    recordings: &crate::recording::RecordingCoordinator,
    ticket: &crate::recording::RecordingTicket,
    failure: DeliveryFailure,
    cancel_enqueued: bool,
) -> FinalizeOutcome {
    let summary = recordings.finish_finalizing(ticket, false, StopReason::FinalizeFailed);
    if failure.outcome_unknown {
        return FinalizeOutcome::Failed(failure);
    }
    let facts = FinalizationFacts {
        stop_acknowledged: false,
        recording_state_changed: summary.is_some(),
        cancel_enqueued,
    };
    if !facts.has_effect() {
        return FinalizeOutcome::Failed(failure);
    }
    let (kind, prefix) = match &failure.error {
        crate::ToolError::Held { .. } => (
            "finalization_paused_after_state_change",
            "The browser session paused before the stop acknowledgement",
        ),
        crate::ToolError::AttentionRequired { .. } => (
            "finalization_interrupted_after_state_change",
            "Ghostlight required user attention before the stop acknowledgement",
        ),
        _ => (
            "finalization_failed_after_state_change",
            "The browser stop failed before acknowledgement",
        ),
    };
    FinalizeOutcome::Partial(FinalizationPartial {
        summary,
        facts,
        kind,
        detail: format!(
            "{prefix}; recording state was committed as interrupted and cleanup cancel queued={cancel_enqueued}. ({})",
            failure.error
        ),
    })
}

fn partial_finalization_result(partial: FinalizationPartial, export: bool) -> CallOutcome {
    let mut structured = finalization_structured(partial.summary.as_ref(), partial.facts);
    if export {
        structured["export_completed"] = json!(false);
        structured["delivery"] = json!("not_completed");
    }
    structured["blocker"] = json!({
        "kind": partial.kind,
        "summary": partial.detail.clone(),
        "nextStep": "Inspect the current recording state before retrying; do not restart or replay the operation automatically.",
    });
    outcome_with_structured(partial.detail, structured, true)
}

async fn finalize(ctx: &LocalCtx<'_>, surface: SurfaceId, reason: StopReason) -> FinalizeOutcome {
    let recordings = ctx.browser.recordings();
    match recordings.begin_finalizing(ctx.guid, surface) {
        Ok(ticket) => match ctx
            .browser
            .stop_recording_capture(ctx.operation.key(), ctx.guid, &ticket, ctx.execution)
            .await
        {
            Ok(_) => finish_after_acknowledgement(recordings, &ticket, reason),
            Err(failure) => {
                let cancel_enqueued = ctx
                    .browser
                    .cancel_recording_capture(ctx.operation.key(), &ticket)
                    .is_ok();
                finish_after_failure(recordings, &ticket, failure, cancel_enqueued)
            }
        },
        Err(Some(summary)) => FinalizeOutcome::Ready {
            summary,
            facts: FinalizationFacts::NO_OP,
        },
        Err(None) => FinalizeOutcome::Missing,
    }
}

async fn run(ctx: LocalCtx<'_>) -> CallOutcome {
    let intent = ctx.operation.intent;
    let args = &ctx.operation.arguments;
    let Some(tab) = recording_tab(args) else {
        return CallOutcome::Failure {
            error: crate::ToolError::binary("browser.record requires a numeric tab"),
        };
    };
    let Some(surface) = ctx.browser.recording_surface(tab) else {
        return CallOutcome::Failure {
            error: crate::ToolError::extension("Browser extension not connected"),
        };
    };
    let recordings = ctx.browser.recordings();

    match intent {
        IntentId::RecordStart => {
            ctx.browser.ensure_recording_supervisor();
            let ticket = match recordings.begin_start(ctx.guid, surface) {
                Ok(ticket) => ticket,
                Err(summary) => {
                    return start_result(
                        "A recording is already active for this tab; it was not replaced.",
                        Some(&summary),
                        false,
                        false,
                        false,
                    )
                }
            };
            let request = recording_start_request(
                ctx.operation.key(),
                tab,
                ticket.id.as_str(),
                ticket.generation,
            );
            match ctx
                .browser
                .execute_mechanism_with_delivery_outcome(ctx.guid, &request, ctx.execution)
                .await
            {
                Err(failure) => {
                    recordings.fail_start(&ticket);
                    if failure.outcome_unknown {
                        let _ = ctx
                            .browser
                            .cancel_recording_capture(ctx.operation.key(), &ticket);
                    }
                    crate::tool::outcome::delivery_failure_outcome(failure)
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
                    match recordings.commit_start(&ticket, vp_w) {
                        Ok(summary) => start_result(
                            format!(
                                "Recording started ({seeded} seed frame(s)). Continue browser work, then export; export will stop recording automatically."
                            ),
                            Some(&summary),
                            true,
                            false,
                            false,
                        ),
                        Err(error) => {
                            let cancel_enqueued = ctx
                                .browser
                                .cancel_recording_capture(ctx.operation.key(), &ticket)
                                .is_ok();
                            let (summary, detail) = match error {
                                CommitStartError::Interrupted(summary) => (
                                    Some(summary),
                                    "The browser acknowledged recording start after the staged recording had already been interrupted."
                                ),
                                CommitStartError::Stale => (
                                    None,
                                    "The browser acknowledged recording start after its local generation had already changed."
                                ),
                            };
                            start_result(
                                format!(
                                    "{detail} Cleanup cancel queued={cancel_enqueued}; do not retry automatically."
                                ),
                                summary.as_ref(),
                                true,
                                true,
                                cancel_enqueued,
                            )
                        }
                    }
                }
            }
        }
        IntentId::RecordStop => match finalize(&ctx, surface, StopReason::Explicit).await {
            FinalizeOutcome::Ready { summary, facts } if facts.has_effect() => stop_result(
                format!(
                    "Recording stopped with {} frame(s) kept.",
                    summary.frame_count
                ),
                Some(&summary),
                facts,
                false,
            ),
            FinalizeOutcome::Ready { summary, facts } => stop_result(
                format!(
                    "Recording is already {}; no stop was sent.",
                    summary.state.as_str()
                ),
                Some(&summary),
                facts,
                false,
            ),
            FinalizeOutcome::Partial(partial) => partial_finalization_result(partial, false),
            FinalizeOutcome::Missing => stop_result(
                "No recording for this tab; no stop was sent.",
                None,
                FinalizationFacts::NO_OP,
                false,
            ),
            FinalizeOutcome::Failed(failure) => delivery_failure_outcome(failure),
        },
        IntentId::RecordStatus => match recordings.status(ctx.guid, surface) {
            Some(summary) => outcome(
                format!("Recording state: {}.", summary.state.as_str()),
                Some(&summary),
            ),
            None => outcome("No recording for this tab.", None),
        },
        IntentId::RecordClear => {
            let changed = match ctx.browser.clear_recording_capture(
                ctx.operation.key(),
                ctx.guid,
                surface,
                ctx.execution,
            ) {
                Ok(changed) => changed,
                Err(error) => return tool_error_outcome(error),
            };
            let summary = recordings.status(ctx.guid, surface);
            let mut structured = summary
                .as_ref()
                .map(summary_value)
                .unwrap_or_else(|| json!({}));
            structured["changed"] = json!(changed);
            structured["clear_committed"] = json!(changed);
            outcome_with_structured(
                if changed {
                    "Recording erased from memory."
                } else {
                    "No recording to clear."
                },
                structured,
                false,
            )
        }
        IntentId::RecordExport => {
            let point = args.get("point").filter(|value| !value.is_null());
            let element_ref = args
                .pointer("/target/ref")
                .and_then(Value::as_str)
                .filter(|reference| !reference.is_empty());
            let download = args.get("download").and_then(Value::as_bool) == Some(true);
            let delivery_count = delivery_target_count(args);
            if delivery_count != 1 {
                return CallOutcome::Failure {
                    error: crate::ToolError::invalid_request(
                        "record.export requires exactly one delivery target: download:true, point, or target.ref",
                    ),
                };
            }
            let (summary, facts) = match finalize(&ctx, surface, StopReason::Explicit).await {
                FinalizeOutcome::Ready { summary, facts } => (summary, facts),
                FinalizeOutcome::Partial(partial) => {
                    return partial_finalization_result(partial, true)
                }
                FinalizeOutcome::Missing => {
                    return export_result(
                        "No recording to export. Start one with action=start_recording.",
                        None,
                        FinalizationFacts::NO_OP,
                        false,
                        "not_started",
                        false,
                    )
                }
                FinalizeOutcome::Failed(failure) => return delivery_failure_outcome(failure),
            };
            if !matches!(
                summary.state,
                RecordingState::Frozen | RecordingState::Interrupted
            ) {
                return export_result(
                    format!(
                        "Recording is {}; wait for finalization before exporting.",
                        summary.state.as_str()
                    ),
                    Some(&summary),
                    facts,
                    false,
                    "not_started",
                    facts.has_effect(),
                );
            }
            let frames = recordings.frames(ctx.guid, surface);
            if frames.is_empty() {
                return export_result(
                    "The recording contains no exportable frames.",
                    Some(&summary),
                    facts,
                    false,
                    "not_started",
                    facts.has_effect(),
                );
            }
            let count = frames.len();
            let options = args.get("options").cloned().unwrap_or(Value::Null);
            let encoded = tokio::task::spawn_blocking(move || {
                crate::gif::encode_recording(&frames, &options)
            })
            .await;
            let gif = match encoded {
                Ok(Ok(gif)) => Zeroizing::new(gif),
                Ok(Err(error)) => {
                    let error = crate::ToolError::binary(format!("GIF encoding failed: {error}"));
                    return if facts.has_effect() {
                        partial_after_stop_error(
                            &summary,
                            facts,
                            "encoding_failed_after_stop",
                            error,
                        )
                    } else {
                        CallOutcome::Failure { error }
                    };
                }
                Err(error) => {
                    let error =
                        crate::ToolError::binary(format!("GIF encoding task failed: {error}"));
                    return if facts.has_effect() {
                        partial_after_stop_error(
                            &summary,
                            facts,
                            "encoding_failed_after_stop",
                            error,
                        )
                    } else {
                        CallOutcome::Failure { error }
                    };
                }
            };
            if !recordings.delivery_allowed(ctx.guid, surface, &summary.id) {
                let error = crate::ToolError::binary(
                    "Recording export was revoked before delivery; captured bytes were erased",
                );
                return if facts.has_effect() {
                    partial_after_stop_error(&summary, facts, "delivery_revoked_after_stop", error)
                } else {
                    CallOutcome::Failure { error }
                };
            }
            let bytes = gif.len();
            let data = b64::encode(&gif);
            let filename = ctx
                .operation
                .arguments
                .get("filename")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .unwrap_or("recording.gif");

            if point.is_some() || element_ref.is_some() {
                let request = gif_upload_request(ctx.operation.key(), tab, data, filename, args);
                return match ctx
                    .browser
                    .execute_mechanism_with_delivery_outcome(ctx.guid, &request, ctx.execution)
                    .await
                {
                    Err(failure) => export_delivery_failure(&summary, facts, failure),
                    Ok(reply) => {
                        let detail = first_text(&reply).unwrap_or("Drop event dispatched.");
                        let mut delivered = summary_value(&summary);
                        facts.apply(&mut delivered);
                        delivered["changed"] = json!(true);
                        delivered["export_completed"] = json!(true);
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
                facts.apply(&mut prepared);
                prepared["changed"] = json!(true);
                prepared["export_completed"] = json!(true);
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
        other => outcome(format!("Unsupported browser.record intent: {other}."), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::operation::{OperationId, OperationKey};

    fn record_operation(intent: IntentId) -> OperationKey {
        OperationKey::new(OperationId::BrowserRecord, intent)
    }

    #[test]
    fn first_text_reads_the_text_block() {
        let reply = json!({ "content": [
            { "type": "image", "data": "x" },
            { "type": "text", "text": "hello" },
        ]});
        assert_eq!(first_text(&reply), Some("hello"));
        assert_eq!(first_text(&json!({})), None);
    }

    #[test]
    fn recording_start_uses_a_typed_canonical_mechanism() {
        let request =
            recording_start_request(record_operation(IntentId::RecordStart), 7, "rec_1", 3);
        assert_eq!(request.id(), MechanismId::RecordingStart);
        assert_eq!(request.input()["tab"], 7);
        assert_eq!(request.input()["recording_id"], "rec_1");
        assert_eq!(request.input()["generation"], 3);
        assert_eq!(request.input()["max_side"], SCREENCAST_MAX_SIDE);
        assert_eq!(
            request.input()["min_interval_ms"],
            SCREENCAST_MIN_INTERVAL_MS
        );
        for legacy in [
            "action",
            "tabId",
            "recordingId",
            "maxSide",
            "minIntervalMs",
            "leaseMs",
            "hardTimeoutMs",
        ] {
            assert!(request.input().get(legacy).is_none());
        }
    }

    #[test]
    fn gif_delivery_uses_typed_canonical_uploads() {
        for args in [json!({"target":{"ref":"ref_1"}}), json!({"point":[10,20]})] {
            let request = gif_upload_request(
                record_operation(IntentId::RecordExport),
                7,
                "AAAA".to_string(),
                "clip.gif",
                &args,
            );
            assert_eq!(request.id(), MechanismId::UploadImage);
            assert_eq!(request.input()["tab"], 7);
            assert_eq!(request.input()["mime_type"], "image/gif");
            assert!(request.input().get("tabId").is_none());
            assert!(request.input().get("mimeType").is_none());
            assert!(request.input().get("coordinate").is_none());
            assert!(request.input().get("ref").is_none());
        }
    }

    #[test]
    fn canonical_delivery_count_is_exact_and_legacy_fields_are_poison() {
        assert_eq!(delivery_target_count(&json!({"download":true})), 1);
        assert_eq!(delivery_target_count(&json!({"point":[1,2]})), 1);
        assert_eq!(delivery_target_count(&json!({"target":{"ref":"ref_1"}})), 1);
        assert_eq!(
            delivery_target_count(&json!({"point":[1,2],"download":true})),
            2
        );

        let legacy = json!({
            "action":"export",
            "tabId":7,
            "coordinate":[1,2],
            "ref":"ref_1"
        });
        assert_eq!(recording_tab(&legacy), None);
        assert_eq!(delivery_target_count(&legacy), 0);
    }

    #[test]
    fn failed_delivery_after_stop_has_a_partial_committed_receipt() {
        let facts = FinalizationFacts {
            stop_acknowledged: true,
            recording_state_changed: true,
            cancel_enqueued: false,
        };
        let outcome = partial_export_result(
            json!({"state":"frozen", "frame_count":3}),
            facts,
            "delivery_paused_after_stop",
            "The recording was stopped, but delivery paused.",
            "Resume and inspect before exporting again.",
        );
        let CallOutcome::Success { result } = outcome else {
            panic!("post-stop failure must stay a structured success receipt");
        };
        assert_eq!(result["isError"], true);
        assert_eq!(result["structuredContent"]["stop_committed"], true);
        assert_eq!(result["structuredContent"]["delivery"], "not_completed");
        assert_eq!(
            result["structuredContent"]["blocker"]["kind"],
            "delivery_paused_after_stop"
        );
        assert_eq!(result["structuredContent"]["export_completed"], false);
    }

    fn surface(tab: i64) -> SurfaceId {
        SurfaceId {
            slot: 1,
            native_tab: tab,
        }
    }

    #[test]
    fn acknowledged_stop_state_race_is_partial_and_committed() {
        let recordings = crate::recording::RecordingCoordinator::new();
        let ticket = recordings.begin_start("g1", surface(7)).unwrap();
        recordings.commit_start(&ticket, None).unwrap();
        let finishing = recordings.begin_finalizing("g1", surface(7)).unwrap();
        assert!(recordings.clear_ticket("g1", &finishing, StopReason::Cleared));

        let FinalizeOutcome::Partial(partial) =
            finish_after_acknowledgement(&recordings, &finishing, StopReason::Explicit)
        else {
            panic!("acknowledged state race must be partial");
        };
        assert!(partial.facts.stop_acknowledged);
        assert_eq!(
            partial.facts.effect(),
            ghostlight_transport::operation::OperationEffect::Committed
        );
        assert_eq!(partial.kind, "stop_acknowledged_state_race");
        let CallOutcome::Success { result } = partial_finalization_result(partial, false) else {
            panic!("acknowledged state race must retain a result receipt");
        };
        assert_eq!(result["isError"], true);
        assert_eq!(result["structuredContent"]["changed"], true);
        assert_eq!(result["structuredContent"]["stop_committed"], true);
    }

    #[test]
    fn conclusive_pause_after_begin_finalizing_records_interruption_and_cancel() {
        let recordings = crate::recording::RecordingCoordinator::new();
        let ticket = recordings.begin_start("g1", surface(8)).unwrap();
        recordings.commit_start(&ticket, None).unwrap();
        let finishing = recordings.begin_finalizing("g1", surface(8)).unwrap();
        let failure = DeliveryFailure {
            error: crate::ToolError::held(false),
            outcome_unknown: false,
        };

        let FinalizeOutcome::Partial(partial) =
            finish_after_failure(&recordings, &finishing, failure, true)
        else {
            panic!("post-begin pause must not become a semantic no-effect hold");
        };
        assert!(partial.facts.recording_state_changed);
        assert!(partial.facts.cancel_enqueued);
        assert_eq!(
            partial.facts.effect(),
            ghostlight_transport::operation::OperationEffect::Committed
        );
        assert_eq!(
            recordings.status("g1", surface(8)).unwrap().state,
            RecordingState::Interrupted
        );
        let CallOutcome::Success { result } = partial_finalization_result(partial, true) else {
            panic!("interrupted export finalization must retain a result receipt");
        };
        assert_eq!(result["isError"], true);
        assert_eq!(result["structuredContent"]["changed"], true);
        assert_eq!(result["structuredContent"]["cancel_enqueued"], true);
        assert_eq!(result["structuredContent"]["export_completed"], false);
    }

    #[test]
    fn acknowledged_start_after_hold_is_partial_and_unsafe() {
        let recordings = crate::recording::RecordingCoordinator::new();
        let ticket = recordings.begin_start("g1", surface(9)).unwrap();
        recordings.interrupt_all(StopReason::UserHold);
        let CommitStartError::Interrupted(summary) =
            recordings.commit_start(&ticket, None).unwrap_err()
        else {
            panic!("held staged start must stay interrupted");
        };
        let outcome = start_result(
            "The browser acknowledged start after hold; cleanup was queued.",
            Some(&summary),
            true,
            true,
            true,
        );
        let CallOutcome::Success { result } = outcome else {
            panic!("acknowledged interrupted start needs a partial receipt");
        };
        assert_eq!(result["isError"], true);
        assert_eq!(result["structuredContent"]["changed"], true);
        assert_eq!(result["structuredContent"]["start_acknowledged"], true);
        assert_eq!(result["structuredContent"]["start_committed"], true);
        assert_eq!(result["structuredContent"]["retry_safe"], false);
        assert_eq!(result["structuredContent"]["state"], "interrupted");
    }
}
