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
use crate::tool::outcome::{
    delivery_failure_outcome, tool_error_outcome, ExecutionOutcome as CallOutcome, LocalCtx,
    LocalFuture, OperationExecution,
};
use crate::work::WorkContext;
use ghostlight_transport::operation::{OperationEffect, OperationKind};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::Instant;

/// The canonical fill operation's `Handler::Local` entry point (post-grant dispatch position,
/// PINS.md SS2). The parent's governance decision has already run by the time this is called.
pub(crate) fn form_fill_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(run(ctx))
}

fn execution_with_batch(result: Value, batch_id: &str) -> OperationExecution {
    let mut execution = OperationExecution::new(result);
    execution.audit.batch_id = Some(batch_id.to_owned());
    execution
}

/// Build a `Success` result carrying `isError: true` while retaining compound audit identity.
fn error_outcome(msg: impl Into<String>, batch_id: &str) -> CallOutcome {
    let mut result = crate::tool::result::text_content(msg.into());
    if let Some(obj) = result.as_object_mut() {
        obj.insert("isError".to_string(), json!(true));
    }
    CallOutcome::Success {
        result: Box::new(execution_with_batch(result, batch_id)),
    }
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

fn internal_audit(
    governance: &Governance,
    operation: OperationKind,
    requires: Option<&'static [Capability]>,
    batch_id: &str,
    step: u32,
    work: &WorkContext,
) -> CallAudit {
    let mut audit =
        governance.begin_with_client(operation.as_str(), None, requires, work.client().cloned());
    audit.orchestrated(operation.as_str(), batch_id, Some(step));
    audit.mark_mechanism_phase();
    audit.attribute_grant(None);
    audit
}

fn canonical_fields(args: &Value) -> Option<Vec<(String, Value)>> {
    let fields = args.get("fields")?.as_array()?;
    let mut values = Vec::with_capacity(fields.len());
    let mut queries = HashSet::with_capacity(fields.len());
    for field in fields {
        let query = field.pointer("/target/query")?.as_str()?;
        if query.is_empty() || !queries.insert(query.to_string()) {
            return None;
        }
        values.push((query.to_string(), field.get("value")?.clone()));
    }
    (!values.is_empty()).then_some(values)
}

fn requested_submit<'a>(
    args: &Value,
    outcome: &form_match::MatchOutcome,
    structure: &'a FormStructure,
) -> Option<&'a crate::browser::form_match::SubmitCandidate> {
    let target = args.get("submit_target")?.as_object()?;
    let form_index = outcome.form_index?;
    let candidates = &structure
        .forms
        .iter()
        .find(|form| form.form_index == form_index)?
        .submits;
    if let Some(reference) = target.get("ref").and_then(Value::as_str) {
        return candidates
            .iter()
            .filter(|candidate| candidate.ref_id == reference && !candidate.disabled)
            .exactly_one();
    }
    let query = target.get("query").and_then(Value::as_str)?;
    let normalized = normalize_submit_name(query);
    candidates
        .iter()
        .filter(|candidate| {
            !candidate.disabled
                && candidate
                    .label
                    .as_deref()
                    .is_some_and(|label| normalize_submit_name(label) == normalized)
        })
        .exactly_one()
}

fn normalize_submit_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

trait ExactlyOne: Iterator + Sized {
    fn exactly_one(mut self) -> Option<Self::Item> {
        let first = self.next()?;
        self.next().is_none().then_some(first)
    }
}

impl<I: Iterator> ExactlyOne for I {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FillInterruption {
    Paused,
    AttentionRequired,
    RevalidationFailed,
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
            Self::RevalidationFailed => "not_run_after_revalidation_failure",
        }
    }

    const fn kind(self) -> &'static str {
        match self {
            Self::Paused => "paused_after_partial_fill",
            Self::AttentionRequired => "interrupted_after_partial_fill",
            Self::RevalidationFailed => "target_revalidation_failed",
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
            Self::RevalidationFailed => format!(
                "A form target failed immediate revalidation after {committed} field(s) committed; remaining fields and submit were not attempted."
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
            Self::RevalidationFailed => {
                "Inspect the current form again before deciding whether to fill any remaining fields."
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StrictFillRefusal {
    UnresolvedTarget,
    CredentialTarget,
    SensitiveClassificationUnavailable,
}

impl StrictFillRefusal {
    const fn kind(self) -> &'static str {
        match self {
            Self::UnresolvedTarget => "unresolved_field",
            Self::CredentialTarget => "credential_target",
            Self::SensitiveClassificationUnavailable => "sensitive_classification_unavailable",
        }
    }

    const fn summary(self) -> &'static str {
        match self {
            Self::UnresolvedTarget => {
                "One or more fields were missing or ambiguous, so no field was changed."
            }
            Self::CredentialTarget => {
                "The form contains a credential-class target, so no field was changed."
            }
            Self::SensitiveClassificationUnavailable => {
                "The browser adapter could not prove that every target is non-sensitive, so no field was changed."
            }
        }
    }

    const fn next_step(self) -> &'static str {
        match self {
            Self::UnresolvedTarget => {
                "Inspect the form again and provide an exact query for every field, or explicitly allow partial progress."
            }
            Self::CredentialTarget => {
                "Ask the user to enter credentials directly in the browser; do not send them through browser_fill."
            }
            Self::SensitiveClassificationUnavailable => {
                "Update the browser adapter or ask the user to fill the form directly."
            }
        }
    }
}

fn strict_fill_refusal(
    outcome: &form_match::MatchOutcome,
    allow_partial: bool,
    reject_sensitive: bool,
) -> Option<StrictFillRefusal> {
    if !allow_partial && !outcome.unmatched.is_empty() {
        return Some(StrictFillRefusal::UnresolvedTarget);
    }
    if !reject_sensitive {
        return None;
    }
    if outcome
        .matched
        .iter()
        .any(|(_, control)| control.control_type == "password" || control.sensitive == Some(true))
    {
        return Some(StrictFillRefusal::CredentialTarget);
    }
    outcome
        .matched
        .iter()
        .any(|(_, control)| control.sensitive.is_none())
        .then_some(StrictFillRefusal::SensitiveClassificationUnavailable)
}

fn unmatched_receipt(outcome: &form_match::MatchOutcome) -> Vec<Value> {
    outcome
        .unmatched
        .iter()
        .map(|(key, candidates)| {
            let candidates = candidates
                .iter()
                .map(|candidate| {
                    json!({
                        "label": candidate.label,
                        "ref": candidate.ref_id,
                        "type": candidate.control_type,
                    })
                })
                .collect::<Vec<_>>();
            json!({ "key": key, "candidates": candidates })
        })
        .collect()
}

fn strict_refusal_outcome(
    refusal: StrictFillRefusal,
    outcome: &form_match::MatchOutcome,
    page: Option<Value>,
    batch_id: &str,
) -> CallOutcome {
    let skipped = outcome
        .matched
        .iter()
        .map(|(key, control)| {
            json!({
                "label": key,
                "ref": control.ref_id,
                "reason": refusal.kind(),
            })
        })
        .collect::<Vec<_>>();
    let mut structured = json!({
        "filled": [],
        "unmatched": unmatched_receipt(outcome),
        "skipped": skipped,
        "submitted": false,
        "submit_ref": null,
        "interruption": {
            "kind": refusal.kind(),
            "summary": refusal.summary(),
            "nextStep": refusal.next_step(),
        }
    });
    if let Some(page) = page {
        structured["page"] = page;
    }
    let mut result = crate::tool::result::text_content(refusal.summary());
    if let Some(object) = result.as_object_mut() {
        object.insert("structuredContent".to_string(), structured);
        object.insert("isError".to_string(), json!(true));
    }
    CallOutcome::Success {
        result: Box::new(execution_with_batch(result, batch_id)),
    }
}

fn strict_revalidation_succeeds(
    query: &str,
    expected: &ControlRef,
    structure: &FormStructure,
) -> bool {
    let outcome = form_match::match_fields(&[query.to_string()], structure);
    outcome.unmatched.is_empty()
        && matches!(
            outcome.matched.as_slice(),
            [(matched_query, control)]
                if matched_query == query
                    && control.ref_id == expected.ref_id
                    && control.control_type == expected.control_type
                    && control.sensitive == Some(false)
                    && skip_reason(control).is_none()
        )
}

fn strict_submit_revalidation_succeeds(
    form_index: usize,
    expected_ref: &str,
    structure: &FormStructure,
) -> bool {
    structure
        .forms
        .iter()
        .find(|form| form.form_index == form_index)
        .and_then(|form| {
            form.submits
                .iter()
                .find(|candidate| candidate.ref_id == expected_ref)
        })
        .is_some_and(|candidate| !candidate.disabled)
}

fn inspect_request(tab: i64) -> MechanismRequest {
    MechanismRequest::for_operation(
        OperationKind::BrowserFillForm,
        MechanismId::FormInspect,
        json!({ "tab": tab }),
    )
    .expect("browser.fill inspection must be declared by its dynamic plan")
}

fn fill_request(
    tab: i64,
    reference: &str,
    value: Value,
    reject_sensitive: bool,
    expected_type: &str,
) -> MechanismRequest {
    let mut input = json!({ "tab": tab, "target": { "ref": reference }, "value": value });
    if reject_sensitive {
        input["reject_sensitive"] = json!(true);
        input["expected_type"] = json!(expected_type);
    }
    MechanismRequest::for_operation(
        OperationKind::BrowserFillForm,
        MechanismId::FormSetValue,
        input,
    )
    .expect("browser.fill value assignment must be declared by its dynamic plan")
}

fn submit_request(tab: i64, reference: &str) -> MechanismRequest {
    MechanismRequest::for_operation(
        OperationKind::BrowserFillForm,
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
        input,
        execution,
        work,
        cancellation,
        ..
    } = ctx;
    let root_operation = operation.kind();
    let args = input;
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
    let submit_requested = args.get("submit_target").is_some();

    // Step 1: the dedicated form-structure internal read (C9), audited by physical mechanism.
    if cancellation.is_cancelled() {
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
        .execute_mechanism(guid, &inspect_request(tab_id), execution)
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

    let keys: Vec<String> = fields_obj.iter().map(|(query, _)| query.clone()).collect();
    let mut outcome = form_match::match_fields(&keys, &structure);
    outcome.matched.sort_by_key(|(query, _)| {
        keys.iter()
            .position(|key| key == query)
            .unwrap_or(usize::MAX)
    });
    outcome.unmatched.sort_by_key(|(query, _)| {
        keys.iter()
            .position(|key| key == query)
            .unwrap_or(usize::MAX)
    });

    let selected_submit = if submit_requested {
        match requested_submit(args, &outcome, &structure) {
            Some(candidate) => Some(candidate.clone()),
            None => {
                return strict_refusal_outcome(
                    StrictFillRefusal::UnresolvedTarget,
                    &outcome,
                    page,
                    &batch_id,
                )
            }
        }
    } else {
        None
    };

    let allow_partial = args.get("partial").and_then(Value::as_bool).unwrap_or(true);
    let reject_sensitive = args
        .get("reject_sensitive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if let Some(refusal) = strict_fill_refusal(&outcome, allow_partial, reject_sensitive) {
        return strict_refusal_outcome(refusal, &outcome, page, &batch_id);
    }

    let mut step: u32 = 2;
    let mut filled: Vec<Value> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    let mut interruption = None;

    for (index, (key, control)) in outcome.matched.iter().enumerate() {
        if cancellation.is_cancelled() {
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

        if reject_sensitive {
            let mut revalidation_audit = internal_audit(
                governance,
                root_operation,
                Some(&[Capability::Read]),
                &batch_id,
                step,
                work,
            );
            let revalidation = browser
                .execute_mechanism(guid, &inspect_request(tab_id), execution)
                .await;
            revalidation_audit.dispatch_finished();
            match revalidation.as_ref().err() {
                Some(crate::ToolError::Held { .. }) => revalidation_audit.held(),
                Some(crate::ToolError::AttentionRequired { .. }) => {
                    revalidation_audit.attention_required()
                }
                _ => revalidation_audit.complete(),
            }
            step += 1;
            let revalidation = match revalidation {
                Ok(result) => first_text(&result)
                    .and_then(|text| serde_json::from_str::<FormStructure>(text).ok()),
                Err(
                    error @ (crate::ToolError::Held { .. }
                    | crate::ToolError::AttentionRequired { .. }),
                ) => {
                    if filled.is_empty() {
                        return tool_error_outcome(error);
                    }
                    let stopped = FillInterruption::from_error(&error)
                        .expect("strict revalidation safety refusal");
                    skip_remaining_matches(&outcome.matched, index, &mut skipped, stopped);
                    interruption = Some(stopped);
                    break;
                }
                Err(_) => None,
            };
            if !revalidation
                .as_ref()
                .is_some_and(|structure| strict_revalidation_succeeds(key, control, structure))
            {
                let stopped = FillInterruption::RevalidationFailed;
                skip_remaining_matches(&outcome.matched, index, &mut skipped, stopped);
                interruption = Some(stopped);
                break;
            }
        }

        let value = fields_obj
            .iter()
            .find_map(|(query, value)| (query == key).then(|| value.clone()))
            .unwrap_or(Value::Null);
        let fill_audit = internal_audit(
            governance,
            root_operation,
            Some(&[Capability::Write]),
            &batch_id,
            step,
            work,
        );
        let request = fill_request(
            tab_id,
            &control.ref_id,
            value.clone(),
            reject_sensitive,
            &control.control_type,
        );
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
                if reject_sensitive {
                    let stopped = FillInterruption::RevalidationFailed;
                    skip_remaining_matches(&outcome.matched, index, &mut skipped, stopped);
                    interruption = Some(stopped);
                    break;
                }
                skipped.push(json!({
                    "label": key,
                    "ref": control.ref_id,
                    "reason": format!("error: {}", failure.error),
                }));
            }
        }
    }

    let unmatched = unmatched_receipt(&outcome);

    let mut submitted = false;
    let mut submit_ref: Option<String> = None;
    let mut observation: Option<String> = None;

    if submit_requested && interruption.is_none() && !filled.is_empty() {
        if cancellation.is_cancelled() {
            return CallOutcome::Cancelled {
                message: "form_fill stopped before submit after cancellation; completed field edits remain audited."
                    .to_string(),
                effect: OperationEffect::Committed,
            };
        }
        if let (Some(idx), Some(candidate)) = (outcome.form_index, selected_submit.as_ref()) {
            submit_ref = Some(candidate.ref_id.clone());
            if reject_sensitive {
                let mut revalidation_audit = internal_audit(
                    governance,
                    root_operation,
                    Some(&[Capability::Read]),
                    &batch_id,
                    step,
                    work,
                );
                let revalidation = browser
                    .execute_mechanism(guid, &inspect_request(tab_id), execution)
                    .await;
                revalidation_audit.dispatch_finished();
                match revalidation.as_ref().err() {
                    Some(crate::ToolError::Held { .. }) => revalidation_audit.held(),
                    Some(crate::ToolError::AttentionRequired { .. }) => {
                        revalidation_audit.attention_required()
                    }
                    _ => revalidation_audit.complete(),
                }
                step += 1;
                match revalidation {
                    Ok(result)
                        if first_text(&result)
                            .and_then(|text| serde_json::from_str::<FormStructure>(text).ok())
                            .as_ref()
                            .is_some_and(|structure| {
                                strict_submit_revalidation_succeeds(
                                    idx,
                                    &candidate.ref_id,
                                    structure,
                                )
                            }) => {}
                    Err(
                        error @ (crate::ToolError::Held { .. }
                        | crate::ToolError::AttentionRequired { .. }),
                    ) => {
                        interruption = FillInterruption::from_error(&error);
                    }
                    _ => {
                        interruption = Some(FillInterruption::RevalidationFailed);
                    }
                }
            }
            if interruption.is_none() {
                let submit_audit = internal_audit(
                    governance,
                    root_operation,
                    Some(&[Capability::Interact]),
                    &batch_id,
                    step,
                    work,
                );
                let request = submit_request(tab_id, &candidate.ref_id);
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
        if interruption.is_some() {
            obj.insert("isError".to_string(), json!(true));
        }
    }
    CallOutcome::Success {
        result: Box::new(execution_with_batch(result, &batch_id)),
    }
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
