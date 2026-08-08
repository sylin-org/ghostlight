// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The canonical multi-field fill orchestration (ADR-0036, PINS.md SS13): one parent governance
//! decision, then a dedicated `formStructure` internal read (C9), the matcher
//! (`browser::form_match`), and pre-authorized internal fills/submit. Each internal step remains
//! individually audited and correlated by `batch_id` (ADR-0036 Decision 7).
//!
//! Unlike `browser.flow`, `browser.fill`'s internals do not re-enter the operation pipeline: the
//! parent's own governance decision already covers the whole interaction (ADR-0036 Decision 4),
//! so each internal dispatch goes straight to the browser mechanism adapter, with its own
//! `CallAudit` scope stamped with the canonical root operation and attributed to the parent's
//! grant.
//!
//! No idempotency wrap (SS8 supersession note, C8/C10): `form_fill` fires once; a re-fire is the
//! caller's explicit choice.

use crate::browser::form_match::{self, ControlRef, FormStructure};
use crate::browser::mechanism::{MechanismId, MechanismRequest};
use crate::governance::dispatch::{CallAudit, Governance};
use crate::governance::ports::Capability;
use crate::operation::registry as operation_registry;
use crate::tool::outcome::{
    delivery_failure_outcome, tool_error_outcome, CallOutcome, LocalCtx, LocalFuture,
};
use crate::work::{CancellationToken, WorkContext};
use ghostlight_transport::operation::{IntentId, OperationEffect, OperationId, OperationKey};
use serde_json::{json, Value};
use std::time::Instant;

/// The canonical fill operation's `Handler::Local` entry point (post-grant dispatch position,
/// PINS.md SS2). The parent's governance decision has already run by the time this is called.
pub(crate) fn form_fill_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(run(ctx))
}

/// Build a `Success` result carrying `isError: true` -- byte-identical to what
/// `pipeline::error_result` renders for a `CallOutcome::Failure`, but as a `Success` so the
/// `_batch_id` side channel (which only `take_batch_id` extracts from a `Success`, PINS.md SS7)
/// survives to stamp the parent audit record even when the call itself failed.
fn error_outcome(msg: impl Into<String>, batch_id: &str) -> CallOutcome {
    let mut result = crate::tool::result::text_content(msg.into());
    if let Some(obj) = result.as_object_mut() {
        obj.insert("isError".to_string(), json!(true));
        obj.insert("_batch_id".to_string(), json!(batch_id));
    }
    CallOutcome::Success { result }
}

/// Pull the trailing ADR-0078 interaction receipt or legacy `observation: ...` digest line off a
/// dispatched action's rendered text. `None` when no digest is present or it reports no change.
fn extract_observation(result: &Value) -> Option<String> {
    let text = result
        .get("content")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?;
    text.lines()
        .find_map(|line| {
            line.strip_prefix("interaction receipt: ")
                .or_else(|| line.strip_prefix("observation: "))
                .map(str::to_string)
        })
        .filter(|s| {
            s.as_str() != "no observable change" && !s.ends_with(": no meaningful page change")
        })
}

/// The first text content block of an MCP result object, if any (used to parse the
/// `form_structure_internal` internal read's raw JSON payload back out).
fn first_text(result: &Value) -> Option<&str> {
    result
        .get("content")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()
}

fn canonical_requirements(key: OperationKey) -> &'static [Capability] {
    operation_registry::descriptor(key)
        .expect("form_fill internal operation key must exist")
        .requires
}

fn internal_audit(
    governance: &Governance,
    operation: OperationKey,
    requires: Option<&'static [Capability]>,
    batch_id: &str,
    step: u32,
    work: Option<&WorkContext>,
) -> CallAudit {
    let mut audit = governance.begin_with_client(
        operation.id.as_str(),
        Some(operation.intent.as_str()),
        requires,
        work.and_then(WorkContext::client).cloned(),
    );
    audit.orchestrated(operation.id.as_str(), batch_id, Some(step));
    audit.mark_mechanism_phase();
    audit.attribute_grant(None);
    audit
}

fn canonical_fields(args: &Value) -> Option<serde_json::Map<String, Value>> {
    let fields = args.get("fields")?.as_array()?;
    let mut values = serde_json::Map::new();
    for field in fields {
        let query = field.pointer("/target/query")?.as_str()?;
        if query.is_empty() {
            return None;
        }
        values.insert(query.to_string(), field.get("value")?.clone());
    }
    (!values.is_empty()).then_some(values)
}

fn submit_requested(intent: IntentId) -> Result<bool, String> {
    match intent {
        IntentId::FillFields => Ok(false),
        IntentId::FillFieldsAndSubmit => Ok(true),
        _ => Err(format!("unsupported browser.fill intent: {intent}")),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FillInterruption {
    Paused,
    AttentionRequired,
}

impl FillInterruption {
    fn from_error(error: &crate::ToolError) -> Option<Self> {
        match error {
            crate::ToolError::Held { .. } => Some(Self::Paused),
            crate::ToolError::AttentionRequired { .. } => Some(Self::AttentionRequired),
            _ => None,
        }
    }

    const fn skipped_reason(self) -> &'static str {
        match self {
            Self::Paused => "not_run_after_pause",
            Self::AttentionRequired => "not_run_after_attention",
        }
    }

    const fn kind(self) -> &'static str {
        match self {
            Self::Paused => "paused_after_partial_fill",
            Self::AttentionRequired => "interrupted_after_partial_fill",
        }
    }

    fn summary(self, committed: usize) -> String {
        match self {
            Self::Paused => format!(
                "The browser session was paused after {committed} field(s) committed; remaining fields and submit were not attempted."
            ),
            Self::AttentionRequired => format!(
                "Ghostlight required user attention after {committed} field(s) committed; remaining fields and submit were not attempted."
            ),
        }
    }

    fn next_step(self) -> &'static str {
        match self {
            Self::Paused => {
                "Ask the user to resume, then inspect the form before deciding whether to fill the remaining fields."
            }
            Self::AttentionRequired => {
                "Ask the user to review and resume Ghostlight, then inspect the form before deciding whether to fill the remaining fields."
            }
        }
    }
}

fn skip_remaining_matches(
    matches: &[(String, ControlRef)],
    start: usize,
    skipped: &mut Vec<Value>,
    interruption: FillInterruption,
) {
    for (key, control) in &matches[start..] {
        skipped.push(json!({
            "label": key,
            "ref": control.ref_id,
            "reason": interruption.skipped_reason(),
        }));
    }
}

fn inspect_request(operation: OperationKey, tab: i64) -> MechanismRequest {
    MechanismRequest::for_operation(operation, MechanismId::FormInspect, json!({ "tab": tab }))
        .expect("browser.fill inspection must be declared by its dynamic plan")
}

fn fill_request(
    operation: OperationKey,
    tab: i64,
    reference: &str,
    value: Value,
) -> MechanismRequest {
    MechanismRequest::for_operation(
        operation,
        MechanismId::FormSetValue,
        json!({ "tab": tab, "target": { "ref": reference }, "value": value }),
    )
    .expect("browser.fill value assignment must be declared by its dynamic plan")
}

fn submit_request(operation: OperationKey, tab: i64, reference: &str) -> MechanismRequest {
    MechanismRequest::for_operation(
        operation,
        MechanismId::PointerClick,
        json!({
            "tab": tab,
            "target": { "ref": reference },
            "button": "left",
            "count": 1
        }),
    )
    .expect("browser.fill submit must be declared by its dynamic plan")
}

async fn run(ctx: LocalCtx<'_>) -> CallOutcome {
    let LocalCtx {
        browser,
        governance,
        guid,
        operation,
        execution,
        work,
        cancellation,
        ..
    } = ctx;
    let intent = operation.intent;
    let root_operation = OperationKey::new(OperationId::BrowserFill, intent);
    let args = &operation.arguments;
    let started = Instant::now();
    let batch_id = uuid::Uuid::new_v4().to_string();
    let Some(tab_id) = args.get("tab").and_then(Value::as_i64) else {
        return error_outcome("form_fill requires a numeric tabId", &batch_id);
    };
    let Some(fields_obj) = canonical_fields(args) else {
        return error_outcome(
            "form_fill requires non-empty canonical field rows",
            &batch_id,
        );
    };
    let submit_requested = match submit_requested(intent) {
        Ok(value) => value,
        Err(message) => return error_outcome(message, &batch_id),
    };

    // Step 1: the dedicated form-structure internal read (C9), audited by physical mechanism.
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return CallOutcome::Cancelled {
            message: "form_fill was cancelled before its first browser step.".to_string(),
            effect: OperationEffect::None,
        };
    }
    let mut structure_audit = internal_audit(
        governance,
        root_operation,
        Some(&[Capability::Read]),
        &batch_id,
        1,
        work,
    );
    // D-grant (C10 STOP note): the parent's resolved grant id lives only inside its own
    // `CallAudit` (a private field), which this handler has no way to reach -- `Gate::Proceed`
    // carries nothing. Internals attribute `None` rather than re-resolving a second grant lookup.
    let structure_result = browser
        .execute_mechanism(guid, &inspect_request(root_operation, tab_id), execution)
        .await;
    structure_audit.dispatch_finished();
    match structure_result.as_ref().err() {
        Some(crate::ToolError::Held { .. }) => structure_audit.held(),
        Some(crate::ToolError::AttentionRequired { .. }) => structure_audit.attention_required(),
        _ => structure_audit.complete(),
    }

    let structure_value = match structure_result {
        Ok(v) => v,
        Err(
            error @ (crate::ToolError::Held { .. } | crate::ToolError::AttentionRequired { .. }),
        ) => return tool_error_outcome(error),
        Err(e) => return error_outcome(format!("form_fill failed: {e}"), &batch_id),
    };
    let structure_json: Value = first_text(&structure_value)
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_else(|| json!({}));
    let page = structure_json.get("page").cloned();
    let structure: FormStructure = serde_json::from_value(structure_json).unwrap_or_default();

    let keys: Vec<String> = fields_obj.keys().cloned().collect();
    let outcome = form_match::match_fields(&keys, &structure);

    let mut step: u32 = 2;
    let mut filled: Vec<Value> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    let mut interruption = None;

    for (index, (key, control)) in outcome.matched.iter().enumerate() {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return CallOutcome::Cancelled {
                message: "form_fill stopped between fields after cancellation; completed fields remain audited and were not replayed."
                    .to_string(),
                effect: if filled.is_empty() {
                    OperationEffect::None
                } else {
                    OperationEffect::Committed
                },
            };
        }
        if let Some(held_for) = browser.held_for() {
            if filled.is_empty() {
                return CallOutcome::Held {
                    prolonged: held_for >= crate::governance::dispatch::HOLD_HINT_AFTER,
                };
            }
            let stopped = FillInterruption::Paused;
            skip_remaining_matches(&outcome.matched, index, &mut skipped, stopped);
            interruption = Some(stopped);
            break;
        }
        if let Some(reason) = skip_reason(control) {
            skipped.push(json!({ "label": key, "ref": control.ref_id, "reason": reason }));
            continue;
        }

        let value = fields_obj.get(key).cloned().unwrap_or(Value::Null);
        let fill_audit = internal_audit(
            governance,
            root_operation,
            Some(canonical_requirements(OperationKey::new(
                OperationId::BrowserFill,
                IntentId::FillField,
            ))),
            &batch_id,
            step,
            work,
        );
        let request = fill_request(root_operation, tab_id, &control.ref_id, value.clone());
        let dispatch = browser
            .execute_mechanism_with_delivery_outcome(guid, &request, execution)
            .await;
        match dispatch.as_ref().err().map(|failure| &failure.error) {
            Some(crate::ToolError::Held { .. }) => fill_audit.held(),
            Some(crate::ToolError::AttentionRequired { .. }) => fill_audit.attention_required(),
            _ => fill_audit.complete(),
        }
        step += 1;

        match dispatch {
            Ok(_) => {
                let display_value = if control.control_type == "password" {
                    json!("********")
                } else {
                    value.clone()
                };
                filled.push(json!({
                    "label": key,
                    "ref": control.ref_id,
                    "value": display_value,
                    "type": control.control_type,
                }));
            }
            Err(failure) if failure.stops_composition() => {
                if failure.outcome_unknown {
                    return delivery_failure_outcome(failure);
                }
                if filled.is_empty() {
                    return delivery_failure_outcome(failure);
                }
                let stopped = FillInterruption::from_error(&failure.error)
                    .expect("guard proved a fill safety interruption");
                skip_remaining_matches(&outcome.matched, index, &mut skipped, stopped);
                interruption = Some(stopped);
                break;
            }
            Err(failure) => {
                skipped.push(json!({
                    "label": key,
                    "ref": control.ref_id,
                    "reason": format!("error: {}", failure.error),
                }));
            }
        }
    }

    let unmatched: Vec<Value> = outcome
        .unmatched
        .iter()
        .map(|(key, candidates)| {
            let cands: Vec<Value> = candidates
                .iter()
                .map(|c| json!({ "label": c.label, "ref": c.ref_id, "type": c.control_type }))
                .collect();
            json!({ "key": key, "candidates": cands })
        })
        .collect();

    let mut submitted = false;
    let mut submit_ref: Option<String> = None;
    let mut observation: Option<String> = None;

    if submit_requested && interruption.is_none() && !filled.is_empty() {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return CallOutcome::Cancelled {
                message: "form_fill stopped before submit after cancellation; completed field edits remain audited."
                    .to_string(),
                effect: OperationEffect::Committed,
            };
        }
        if let Some(idx) = outcome.form_index {
            if let Some(form) = structure.forms.iter().find(|f| f.form_index == idx) {
                if let Some(candidate) = form.submits.first() {
                    submit_ref = Some(candidate.ref_id.clone());
                    let submit_audit = internal_audit(
                        governance,
                        root_operation,
                        Some(canonical_requirements(OperationKey::new(
                            OperationId::BrowserAct,
                            IntentId::ActClick,
                        ))),
                        &batch_id,
                        step,
                        work,
                    );
                    let request = submit_request(root_operation, tab_id, &candidate.ref_id);
                    let dispatch = browser
                        .execute_mechanism_with_delivery_outcome(guid, &request, execution)
                        .await;
                    match dispatch.as_ref().err().map(|failure| &failure.error) {
                        Some(crate::ToolError::Held { .. }) => submit_audit.held(),
                        Some(crate::ToolError::AttentionRequired { .. }) => {
                            submit_audit.attention_required()
                        }
                        _ => submit_audit.complete(),
                    }
                    match dispatch {
                        Ok(result) => {
                            submitted = true;
                            observation = extract_observation(&result);
                        }
                        Err(failure) if failure.stops_composition() => {
                            if failure.outcome_unknown {
                                return delivery_failure_outcome(failure);
                            }
                            interruption = Some(
                                FillInterruption::from_error(&failure.error)
                                    .expect("conclusive composition stop is a safety refusal"),
                            );
                        }
                        Err(_) => {}
                    }
                }
            }
        }
    }

    let total_fields = keys.len();
    let mut lines = vec![format!("Filled {}/{} fields.", filled.len(), total_fields)];
    for f in &filled {
        lines.push(format!(
            "{} -> {}",
            f["label"].as_str().unwrap_or(""),
            f["type"].as_str().unwrap_or("")
        ));
    }
    if !unmatched.is_empty() {
        let keys_str: Vec<&str> = unmatched
            .iter()
            .map(|u| u["key"].as_str().unwrap_or(""))
            .collect();
        lines.push(format!("unmatched: {}", keys_str.join(", ")));
    }
    lines.push(format!("submitted: {submitted}"));
    if let Some(stopped) = interruption {
        lines.push(stopped.summary(filled.len()));
    }
    let text = lines.join("\n");

    let mut structured = json!({
        "filled": filled,
        "unmatched": unmatched,
        "skipped": skipped,
        "submitted": submitted,
        "submit_ref": submit_ref,
        "duration_ms": started.elapsed().as_millis() as u64,
    });
    if let Some(obs) = observation {
        if let Some(obj) = structured.as_object_mut() {
            obj.insert("observation".to_string(), json!(obs));
        }
    }
    if let Some(page) = page {
        if let Some(obj) = structured.as_object_mut() {
            obj.insert("page".to_string(), page);
        }
    }
    if let Some(stopped) = interruption {
        structured["interruption"] = json!({
            "kind": stopped.kind(),
            "summary": stopped.summary(filled.len()),
            "nextStep": stopped.next_step(),
        });
    }

    let mut result = crate::tool::result::text_content(text);
    if let Some(obj) = result.as_object_mut() {
        obj.insert("structuredContent".to_string(), structured);
        obj.insert("_batch_id".to_string(), json!(batch_id));
        if interruption.is_some() {
            obj.insert("isError".to_string(), json!(true));
        }
    }
    CallOutcome::Success { result }
}

/// Why a matched control is never filled (ADR-0036 Decision 6): a file input is permanently out
/// of scope; disabled/readonly controls cannot accept a value. `None` means "fill it".
fn skip_reason(control: &ControlRef) -> Option<&'static str> {
    if control.control_type == "file" {
        Some("file input (out of scope)")
    } else if control.disabled {
        Some("disabled")
    } else if control.readonly {
        Some("readonly")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::ports::{
        AttentionEventRecord, AuditRecord, AuditRole, AuditSink, SessionEventRecord,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Capture {
        records: Mutex<Vec<AuditRecord>>,
    }

    impl AuditSink for Capture {
        fn record(&self, record: &AuditRecord) {
            self.records.lock().unwrap().push(record.clone());
        }

        fn record_session_event(&self, _record: &SessionEventRecord) {}

        fn record_attention_event(&self, _record: &AttentionEventRecord) {}
    }

    #[test]
    fn internal_audits_use_canonical_operation_requirements() {
        assert_eq!(
            canonical_requirements(OperationKey::new(
                OperationId::BrowserFill,
                IntentId::FillField,
            )),
            &[Capability::Write]
        );
        assert_eq!(
            canonical_requirements(OperationKey::new(
                OperationId::BrowserAct,
                IntentId::ActClick,
            )),
            &[Capability::Action]
        );
    }

    #[test]
    fn canonical_field_rows_preserve_values_and_reject_legacy_objects() {
        let fields = canonical_fields(&json!({
            "tab": 1,
            "fields": [
                {"target":{"query":"Email"},"value":"a@example.com"},
                {"target":{"query":"Remember"},"value":true}
            ]
        }))
        .unwrap();
        assert_eq!(fields["Email"], "a@example.com");
        assert_eq!(fields["Remember"], true);
        assert!(canonical_fields(&json!({
            "tabId": 1,
            "fields": {"Email":"a@example.com"},
            "submit": true
        }))
        .is_none());
    }

    #[test]
    fn canonical_intent_alone_controls_submit() {
        assert!(!submit_requested(IntentId::FillFields).unwrap());
        assert!(submit_requested(IntentId::FillFieldsAndSubmit).unwrap());
        assert!(submit_requested(IntentId::FillField).is_err());
    }

    #[test]
    fn form_sequence_uses_typed_canonical_mechanisms() {
        let operation = OperationKey::new(OperationId::BrowserFill, IntentId::FillFieldsAndSubmit);
        let inspect = inspect_request(operation, 4);
        let fill = fill_request(operation, 4, "ref_1", json!("secret"));
        let submit = submit_request(operation, 4, "ref_2");

        assert_eq!(inspect.id(), MechanismId::FormInspect);
        assert_eq!(inspect.input(), &json!({"tab":4}));
        assert_eq!(fill.id(), MechanismId::FormSetValue);
        assert_eq!(fill.input().pointer("/target/ref"), Some(&json!("ref_1")));
        assert_eq!(fill.input()["value"], "secret");
        assert_eq!(submit.id(), MechanismId::PointerClick);
        assert_eq!(submit.input()["button"], "left");
        assert_eq!(submit.input()["count"], 1);
        for request in [&inspect, &fill, &submit] {
            assert!(request.input().get("tabId").is_none());
            assert!(request.input().get("action").is_none());
            assert!(request.input().get("ref").is_none());
        }
    }

    #[test]
    fn internal_physical_steps_keep_the_parent_canonical_audit_identity() {
        let sink = Arc::new(Capture::default());
        let governance = Governance::all_open(sink.clone() as Arc<dyn AuditSink>);
        let root = OperationKey::new(OperationId::BrowserFill, IntentId::FillFieldsAndSubmit);
        let audit = internal_audit(
            &governance,
            root,
            Some(&[Capability::Read]),
            "00000000-0000-4000-8000-000000000001",
            1,
            None,
        );
        audit.complete();

        let records = sink.records.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool, root.id.as_str());
        assert_eq!(records[0].action.as_deref(), Some(root.intent.as_str()));
        assert_eq!(records[0].orchestrator, Some(root.id.as_str()));
        assert_eq!(records[0].step, Some(1));
        assert_eq!(records[0].role, Some(AuditRole::MechanismPhase));
    }

    #[test]
    fn partial_fill_interruption_marks_every_remaining_field_not_run() {
        let matches = vec![
            (
                "Email".to_string(),
                ControlRef {
                    ref_id: "ref_1".to_string(),
                    control_type: "text".to_string(),
                    disabled: false,
                    readonly: false,
                },
            ),
            (
                "Name".to_string(),
                ControlRef {
                    ref_id: "ref_2".to_string(),
                    control_type: "text".to_string(),
                    disabled: false,
                    readonly: false,
                },
            ),
        ];
        let mut skipped = Vec::new();
        skip_remaining_matches(
            &matches,
            0,
            &mut skipped,
            FillInterruption::AttentionRequired,
        );
        assert_eq!(skipped.len(), 2);
        assert!(skipped
            .iter()
            .all(|field| { field["reason"] == "not_run_after_attention" }));
        assert_eq!(FillInterruption::Paused.kind(), "paused_after_partial_fill");
        assert_eq!(
            FillInterruption::AttentionRequired.kind(),
            "interrupted_after_partial_fill"
        );
    }
}
