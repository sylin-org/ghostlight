// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The canonical target-bound semantic interaction (ADR-0078 D3): resolve one target, refuse
//! ambiguity, dispatch one pre-authorized browser mechanism, and optionally observe a
//! postcondition in one operation.
//!
//! The parent pipeline performs the complete RAWX decision before this handler runs. Internal
//! resolution, cue, action, and wait calls go directly to the browser and receive correlated audit
//! records; they never trigger a second policy prompt or use page content as authorization input.

use crate::browser::mechanism::{MechanismId, MechanismRequest};
use crate::governance::dispatch::Governance;
use crate::governance::ports::Capability;
use crate::operation::registry as operation_registry;
use crate::tool::outcome::{
    delivery_failure_outcome, tool_error_outcome, CallOutcome, LocalCtx, LocalFuture,
};
use crate::work::{CancellationToken, WorkContext};
use ghostlight_transport::operation::{IntentId, OperationEffect, OperationId, OperationKey};
use serde_json::{json, Map, Value};

const EXPECT_STATES: &[&str] = &["visible", "present", "gone"];

/// Canonical registry entry point. The parent grant decision has completed before this runs.
pub(crate) fn act_on_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(run(ctx))
}

fn invalid(message: impl Into<String>) -> CallOutcome {
    CallOutcome::Failure {
        error: crate::ToolError::invalid_request(message.into()).next_step(
            r#"use {"tabId":1,"target":{"name":"Save","role":"button"},"action":"left_click"}"#,
        ),
    }
}

fn validate(intent: IntentId, args: &Value) -> Result<(), String> {
    let profile = action_profile(intent)
        .ok_or_else(|| format!("unsupported browser.act intent: {intent}"))?;
    if args.get("tab").and_then(Value::as_i64).is_none() {
        return Err("act_on requires a numeric tabId".to_string());
    }
    let target = args
        .get("target")
        .and_then(Value::as_object)
        .ok_or_else(|| "act_on requires a target object".to_string())?;
    let modes = ["ref", "query", "name"]
        .iter()
        .filter(|key| {
            target
                .get(**key)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty())
        })
        .count();
    if modes != 1 {
        return Err("target must contain exactly one non-empty ref, query, or name".to_string());
    }
    if target.contains_key("role") && !target.contains_key("name") {
        return Err("target.role is valid only with target.name".to_string());
    }
    if target.contains_key("role") && target.get("role").and_then(Value::as_str).is_none() {
        return Err("target.role must be a string".to_string());
    }
    if target
        .keys()
        .any(|key| !matches!(key.as_str(), "ref" | "query" | "name" | "role"))
    {
        return Err("target contains an unsupported field".to_string());
    }
    let has_value = args.get("value").is_some();
    if profile.mechanism == MechanismId::FormSetValue && !has_value {
        return Err("value is required for set_value".to_string());
    }
    if profile.mechanism == MechanismId::FormSetValue
        && args.get("value").and_then(Value::as_str).is_none()
    {
        return Err("value for set_value must be a string".to_string());
    }
    if profile.mechanism != MechanismId::FormSetValue && has_value {
        return Err("value is valid only for set_value".to_string());
    }
    if let Some(expect) = args.get("expect") {
        let object = expect
            .as_object()
            .ok_or_else(|| "expect must be an object".to_string())?;
        let modes = ["selector", "text"]
            .iter()
            .filter(|key| {
                object
                    .get(**key)
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty())
            })
            .count();
        if modes != 1 {
            return Err("expect must contain exactly one non-empty selector or text".to_string());
        }
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "selector" | "text" | "state" | "timeout_ms"))
        {
            return Err("expect contains an unsupported field".to_string());
        }
        if let Some(state) = object.get("state") {
            let state = state
                .as_str()
                .ok_or_else(|| "expect.state must be a string".to_string())?;
            if !EXPECT_STATES.contains(&state) {
                return Err("expect.state must be visible, present, or gone".to_string());
            }
        }
        if let Some(timeout) = object.get("timeout_ms") {
            let timeout = timeout
                .as_f64()
                .ok_or_else(|| "expect.timeout_ms must be a number".to_string())?;
            if !(0.0..=30_000.0).contains(&timeout) {
                return Err("expect.timeout_ms must be from 0 through 30000".to_string());
            }
        }
    }
    Ok(())
}

fn first_text(result: &Value) -> Option<&str> {
    result
        .get("content")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()
}

fn stamp(result: &mut Value, batch_id: &str, assurance: &str, outcome: &str) {
    if let Some(object) = result.as_object_mut() {
        object.insert("_batch_id".to_string(), json!(batch_id));
        object.insert("_target_assurance".to_string(), json!(assurance));
        object.insert("_outcome_category".to_string(), json!(outcome));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PostActionRefusal {
    Paused,
    Interrupted,
}

impl PostActionRefusal {
    fn from_error(error: &crate::ToolError) -> Option<Self> {
        match error {
            crate::ToolError::Held { .. } => Some(Self::Paused),
            crate::ToolError::AttentionRequired { .. } => Some(Self::Interrupted),
            _ => None,
        }
    }

    fn blocker(self, expected: bool) -> Value {
        let observation = if expected {
            "the requested postcondition"
        } else {
            "post-action settlement"
        };
        match self {
            Self::Paused => json!({
                "kind": "postcondition_paused",
                "summary": format!(
                    "The action committed, but {observation} was not observed because the user paused the browser session."
                ),
                "nextStep": "Ask the user to resume, then inspect the current page state before choosing another action."
            }),
            Self::Interrupted => json!({
                "kind": "postcondition_interrupted",
                "summary": format!(
                    "The action committed, but {observation} was not observed because Ghostlight requires user attention."
                ),
                "nextStep": "Ask the user to review and resume Ghostlight, then inspect the current page state before choosing another action."
            }),
        }
    }

    const fn category(self) -> &'static str {
        match self {
            Self::Paused => "postcondition_paused",
            Self::Interrupted => "postcondition_interrupted",
        }
    }
}

fn append_post_action_refusal(result: &mut Value, refusal: PostActionRefusal, expected: bool) {
    if let Some(blockers) = result
        .pointer_mut("/structuredContent/interactionReceipt/blockers")
        .and_then(Value::as_array_mut)
    {
        blockers.push(refusal.blocker(expected));
    }
    if let Some(first) = result
        .get_mut("content")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.first_mut())
        .and_then(Value::as_object_mut)
    {
        let text = first.get("text").and_then(Value::as_str).unwrap_or("");
        let note = match refusal {
            PostActionRefusal::Paused => {
                "The action committed, but observation paused when the user held the browser session."
            }
            PostActionRefusal::Interrupted => {
                "The action committed, but observation was interrupted when Ghostlight required user attention."
            }
        };
        first.insert("text".to_string(), json!(format!("{text}\n{note}")));
    }
    if let Some(object) = result.as_object_mut() {
        object.insert("isError".to_string(), json!(true));
    }
}

fn recovery_result(
    message: String,
    batch_id: &str,
    assurance: &str,
    page: Value,
    kind: &str,
    candidates: Vec<Value>,
    more: bool,
) -> CallOutcome {
    let next = match kind {
        "ambiguous_target" => "Use a ref from candidates or add an exact role/name.",
        "covered_target" => "Dismiss or move the covering element, then retry the same target.",
        "frame_unsupported" => {
            "Interact with the top document or wait for a separately governed frame capability."
        }
        _ => "Read the target again and retry with a fresh ref or more specific name.",
    };
    let receipt = json!({
        "targetAssurance": assurance,
        "action": "resolve",
        "observedAfter": {},
        "blockers": [{ "kind": kind, "summary": message, "nextStep": next }],
        "page": page,
        "more": more
    });
    let mut result = crate::tool::result::text_content(format!(
        "interaction receipt: blocked before action: {kind}. {next}"
    ));
    if let Some(object) = result.as_object_mut() {
        object.insert("isError".to_string(), json!(true));
        object.insert(
            "structuredContent".to_string(),
            json!({ "interactionReceipt": receipt, "candidates": candidates }),
        );
    }
    stamp(&mut result, batch_id, assurance, "blocked");
    CallOutcome::Success { result }
}

fn internal_audit(
    governance: &Governance,
    operation: OperationKey,
    requires: Option<&'static [Capability]>,
    batch_id: &str,
    step: u32,
    work: Option<&WorkContext>,
) -> crate::governance::dispatch::CallAudit {
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

fn canonical_requirements(key: OperationKey) -> &'static [Capability] {
    operation_registry::descriptor(key)
        .expect("act_on internal operation key must exist")
        .requires
}

#[derive(Clone, Copy)]
struct ActionProfile {
    mechanism: MechanismId,
    cue_kind: &'static str,
    operation: OperationKey,
    button: Option<&'static str>,
    count: Option<u64>,
}

fn action_profile(intent: IntentId) -> Option<ActionProfile> {
    let click = |cue_kind, button, count, intent| ActionProfile {
        mechanism: MechanismId::PointerClick,
        cue_kind,
        operation: OperationKey::new(OperationId::BrowserAct, intent),
        button: Some(button),
        count: Some(count),
    };
    Some(match intent {
        IntentId::ActClick => click("click", "left", 1, intent),
        IntentId::ActRightClick => click("right_click", "right", 1, intent),
        IntentId::ActDoubleClick => click("double_click", "left", 2, intent),
        IntentId::ActTripleClick => click("triple_click", "left", 3, intent),
        IntentId::ActHover => ActionProfile {
            mechanism: MechanismId::PointerHover,
            cue_kind: "hover",
            operation: OperationKey::new(OperationId::BrowserAct, intent),
            button: None,
            count: None,
        },
        IntentId::ActScrollIntoView => ActionProfile {
            mechanism: MechanismId::ScrollTargetIntoView,
            cue_kind: "scroll_into_view",
            operation: OperationKey::new(OperationId::BrowserAct, intent),
            button: None,
            count: None,
        },
        IntentId::ActSetValue => ActionProfile {
            mechanism: MechanismId::FormSetValue,
            cue_kind: "set_value",
            operation: OperationKey::new(OperationId::BrowserFill, IntentId::FillField),
            button: None,
            count: None,
        },
        _ => return None,
    })
}

fn action_request(
    operation: OperationKey,
    profile: ActionProfile,
    tab: i64,
    reference: &str,
    args: &Value,
) -> MechanismRequest {
    let mut input = json!({ "tab": tab, "target": { "ref": reference } });
    if let Some(button) = profile.button {
        input["button"] = json!(button);
    }
    if let Some(count) = profile.count {
        input["count"] = json!(count);
    }
    if profile.mechanism == MechanismId::FormSetValue {
        input["value"] = args["value"].clone();
    }
    if let Some(modifiers) = args.get("modifiers") {
        input["modifiers"] = modifiers.clone();
    }
    MechanismRequest::for_operation(operation, profile.mechanism, input)
        .expect("browser.act mechanism must be declared by its dynamic plan")
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
    let args = &operation.arguments;
    if let Err(message) = validate(intent, args) {
        return invalid(message);
    }
    let profile = action_profile(intent).expect("validated browser.act intent");
    let root_operation = OperationKey::new(OperationId::BrowserAct, intent);
    let batch_id = uuid::Uuid::new_v4().to_string();
    let tab_id = args["tab"].as_i64().expect("validated tab");
    let target = args["target"].clone();
    let assurance = if target.get("ref").is_some() {
        "ref"
    } else {
        "semantic"
    };
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return CallOutcome::Cancelled {
            message: "act_on was cancelled before target resolution.".to_string(),
            effect: OperationEffect::None,
        };
    }

    let mut resolve_audit = internal_audit(
        governance,
        root_operation,
        Some(&[Capability::Read]),
        &batch_id,
        1,
        work,
    );
    let resolved = browser
        .execute_mechanism(
            guid,
            &MechanismRequest::for_operation(
                root_operation,
                MechanismId::ElementResolve,
                json!({ "tab": tab_id, "target": target }),
            )
            .expect("browser.act resolution must be declared by its dynamic plan"),
            execution,
        )
        .await;
    resolve_audit.dispatch_finished();
    match resolved.as_ref().err() {
        Some(crate::ToolError::Held { .. }) => resolve_audit.held(),
        Some(crate::ToolError::AttentionRequired { .. }) => resolve_audit.attention_required(),
        _ => resolve_audit.complete(),
    }
    let resolved = match resolved {
        Ok(result) => first_text(&result)
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .unwrap_or_else(|| json!({ "target": null, "candidates": [], "page": {} })),
        Err(error) => return tool_error_outcome(error),
    };
    let page = resolved.get("page").cloned().unwrap_or_else(|| json!({}));
    if let Some(error) = resolved.get("error").and_then(Value::as_str) {
        return recovery_result(
            error.to_string(),
            &batch_id,
            assurance,
            page,
            "stale_ref",
            Vec::new(),
            false,
        );
    }
    let candidates = resolved
        .get("candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if resolved.get("ambiguous").and_then(Value::as_bool) == Some(true) {
        return recovery_result(
            "The strongest semantic rank contains more than one target.".to_string(),
            &batch_id,
            assurance,
            page,
            "ambiguous_target",
            candidates,
            resolved
                .get("more")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        );
    }
    if resolved.get("covered").and_then(Value::as_bool) == Some(true) {
        let candidates = resolved.get("target").cloned().into_iter().collect();
        return recovery_result(
            "Another visible element covers the resolved target point.".to_string(),
            &batch_id,
            assurance,
            page,
            "covered_target",
            candidates,
            false,
        );
    }
    let Some(resolved_target) = resolved.get("target").filter(|value| !value.is_null()) else {
        let frame_unsupported =
            resolved.get("frameUnsupported").and_then(Value::as_bool) == Some(true);
        return recovery_result(
            if frame_unsupported {
                "No visible target matched in the top document; embedded frame content is outside the current automation surface."
                    .to_string()
            } else {
                "No visible target matched the request.".to_string()
            },
            &batch_id,
            assurance,
            page,
            if frame_unsupported {
                "frame_unsupported"
            } else {
                "target_missing"
            },
            Vec::new(),
            false,
        );
    };
    let Some(reference) = resolved_target.get("ref").and_then(Value::as_str) else {
        return invalid("resolved target did not carry a ref");
    };

    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return CallOutcome::Cancelled {
            message: "act_on stopped after target resolution and before the action; resolved state was not reused."
                .to_string(),
            effect: OperationEffect::None,
        };
    }

    let mut cue_audit = internal_audit(governance, root_operation, Some(&[]), &batch_id, 2, work);
    let cue = browser
        .execute_mechanism(
            guid,
            &MechanismRequest::for_operation(
                root_operation,
                MechanismId::TargetCue,
                json!({
                    "tab": tab_id,
                    "point": [
                        resolved_target.get("x").and_then(Value::as_f64).unwrap_or(0.0),
                        resolved_target.get("y").and_then(Value::as_f64).unwrap_or(0.0)
                    ],
                    "cue_kind": profile.cue_kind
                }),
            )
            .expect("browser.act cue must be declared by its dynamic plan"),
            execution,
        )
        .await;
    cue_audit.dispatch_finished();
    match cue.as_ref().err() {
        Some(crate::ToolError::Held { .. }) => cue_audit.held(),
        Some(crate::ToolError::AttentionRequired { .. }) => cue_audit.attention_required(),
        _ => cue_audit.complete(),
    }
    if let Err(
        error @ (crate::ToolError::Held { .. } | crate::ToolError::AttentionRequired { .. }),
    ) = cue
    {
        return tool_error_outcome(error);
    }

    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return CallOutcome::Cancelled {
            message: "act_on stopped after its presentation cue and before the browser action."
                .to_string(),
            effect: OperationEffect::None,
        };
    }

    let request = action_request(root_operation, profile, tab_id, reference, args);
    let mut action_audit = internal_audit(
        governance,
        root_operation,
        Some(canonical_requirements(profile.operation)),
        &batch_id,
        3,
        work,
    );
    let dispatched = browser
        .execute_mechanism_with_delivery_outcome(guid, &request, execution)
        .await;
    action_audit.dispatch_finished();
    match dispatched.as_ref().err().map(|failure| &failure.error) {
        Some(crate::ToolError::Held { .. }) => action_audit.held(),
        Some(crate::ToolError::AttentionRequired { .. }) => action_audit.attention_required(),
        _ => action_audit.complete(),
    }
    let mut result = match dispatched {
        Ok(result) => result,
        Err(failure) => return delivery_failure_outcome(failure),
    };

    let mutations = result
        .pointer("/structuredContent/interactionReceipt/observedAfter/mutations")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let expect = args.get("expect").and_then(Value::as_object);
    if expect.is_some() || mutations > 0 {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return CallOutcome::Cancelled {
                message: "act_on cancellation arrived after the atomic action; the action completed and was audited, and no postcondition wait was started."
                    .to_string(),
                effect: OperationEffect::Committed,
            };
        }
        let mut wait_args = Map::new();
        wait_args.insert("tab".to_string(), json!(tab_id));
        wait_args.insert("settle".to_string(), json!(true));
        if let Some(expect) = expect {
            for key in ["selector", "text", "state", "timeout_ms"] {
                if let Some(value) = expect.get(key) {
                    wait_args.insert(key.to_string(), value.clone());
                }
            }
        } else {
            wait_args.insert("state".to_string(), json!("settled"));
            wait_args.insert("timeout_ms".to_string(), json!(5000));
        }
        let mut wait_audit = internal_audit(
            governance,
            root_operation,
            Some(canonical_requirements(OperationKey::new(
                OperationId::BrowserWait,
                IntentId::WaitUntil,
            ))),
            &batch_id,
            4,
            work,
        );
        let waited = browser
            .execute_mechanism(
                guid,
                &MechanismRequest::for_operation(
                    root_operation,
                    MechanismId::WaitUntil,
                    Value::Object(wait_args),
                )
                .expect("browser.act wait must be declared by its dynamic plan"),
                execution,
            )
            .await;
        wait_audit.dispatch_finished();
        match waited.as_ref().err() {
            Some(crate::ToolError::Held { .. }) => wait_audit.held(),
            Some(crate::ToolError::AttentionRequired { .. }) => wait_audit.attention_required(),
            _ => wait_audit.complete(),
        }
        match waited {
            Ok(wait_result) => {
                if let Some(observed) = result
                    .pointer_mut("/structuredContent/interactionReceipt/observedAfter")
                    .and_then(Value::as_object_mut)
                {
                    observed.insert(
                        if expect.is_some() {
                            "expectMet"
                        } else {
                            "settled"
                        }
                        .to_string(),
                        json!(true),
                    );
                }
                if let Some(structured) = result
                    .get_mut("structuredContent")
                    .and_then(Value::as_object_mut)
                {
                    structured.insert(
                        "wait".to_string(),
                        wait_result
                            .get("structuredContent")
                            .cloned()
                            .unwrap_or_else(|| json!({})),
                    );
                }
            }
            Err(error) if PostActionRefusal::from_error(&error).is_some() => {
                let refusal = PostActionRefusal::from_error(&error)
                    .expect("guard proved a post-action safety refusal");
                append_post_action_refusal(&mut result, refusal, expect.is_some());
                stamp(&mut result, &batch_id, assurance, refusal.category());
                return CallOutcome::Success { result };
            }
            Err(error) if expect.is_some() => {
                if let Some(blockers) = result
                    .pointer_mut("/structuredContent/interactionReceipt/blockers")
                    .and_then(Value::as_array_mut)
                {
                    blockers.push(json!({
                        "kind": "expect_timeout",
                        "summary": "The requested postcondition was not observed within its timeout.",
                        "nextStep": "Inspect the current receipt and retry only after narrowing the expected state."
                    }));
                }
                if let Some(first) = result
                    .get_mut("content")
                    .and_then(Value::as_array_mut)
                    .and_then(|items| items.first_mut())
                    .and_then(Value::as_object_mut)
                {
                    let text = first.get("text").and_then(Value::as_str).unwrap_or("");
                    first.insert(
                        "text".to_string(),
                        json!(format!("{text}\nexpectation not observed: {error}")),
                    );
                }
                if let Some(object) = result.as_object_mut() {
                    object.insert("isError".to_string(), json!(true));
                }
                stamp(&mut result, &batch_id, assurance, "expect_timeout");
                return CallOutcome::Success { result };
            }
            Err(_) => {
                // Settlement is opportunistic. The receipt already truthfully reports the first
                // observation and must not claim settled when this wait fails.
            }
        }
    }

    let category = if result
        .pointer("/structuredContent/interactionReceipt/observedAfter/expectMet")
        .and_then(Value::as_bool)
        == Some(true)
    {
        "expect_met"
    } else if mutations > 0 {
        "changed"
    } else {
        "unchanged"
    };
    stamp(&mut result, &batch_id, assurance, category);
    CallOutcome::Success { result }
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
    fn internal_dispatches_use_semantic_operation_keys() {
        for intent in [
            IntentId::ActClick,
            IntentId::ActRightClick,
            IntentId::ActDoubleClick,
            IntentId::ActTripleClick,
            IntentId::ActHover,
            IntentId::ActScrollIntoView,
        ] {
            assert_eq!(
                action_profile(intent).unwrap().operation,
                OperationKey::new(OperationId::BrowserAct, intent)
            );
        }
        assert_eq!(
            action_profile(IntentId::ActSetValue).unwrap().operation,
            OperationKey::new(OperationId::BrowserFill, IntentId::FillField)
        );
        assert_eq!(
            canonical_requirements(action_profile(IntentId::ActClick).unwrap().operation),
            &[Capability::Action]
        );
        assert_eq!(
            canonical_requirements(action_profile(IntentId::ActHover).unwrap().operation),
            &[Capability::Read]
        );
        assert_eq!(
            canonical_requirements(action_profile(IntentId::ActSetValue).unwrap().operation),
            &[Capability::Write]
        );
    }

    #[test]
    fn internal_physical_steps_are_marked_without_changing_canonical_identity() {
        let sink = Arc::new(Capture::default());
        let governance = Governance::all_open(sink.clone() as Arc<dyn AuditSink>);
        let root = OperationKey::new(OperationId::BrowserAct, IntentId::ActClick);
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
    fn validates_target_value_and_expect_shapes() {
        assert!(validate(
            IntentId::ActClick,
            &json!({
                "tab": 1,
                "target": { "name": "Save", "role": "button" },
                "expect": { "text": "Saved", "state": "visible", "timeout_ms": 5000 }
            })
        )
        .is_ok());
        assert!(validate(
            IntentId::ActClick,
            &json!({
                "tab": 1,
                "target": { "ref": "ref_1", "query": "Save" }
            })
        )
        .unwrap_err()
        .contains("exactly one"));
        assert!(validate(
            IntentId::ActSetValue,
            &json!({
                "tab": 1, "target": { "ref": "ref_1" }
            })
        )
        .unwrap_err()
        .contains("value is required"));
        assert!(validate(
            IntentId::ActClick,
            &json!({
                "tab": 1,
                "target": { "name": "Save" },
                "expect": { "text": "A", "selector": "#a" }
            })
        )
        .unwrap_err()
        .contains("exactly one"));
        assert!(validate(
            IntentId::ActSetValue,
            &json!({
                "tab": 1, "target": { "ref": "ref_1" }, "value": true
            })
        )
        .unwrap_err()
        .contains("must be a string"));
        assert!(validate(
            IntentId::ActClick,
            &json!({
                "tab": 1, "target": { "name": "Save", "role": 7 }
            })
        )
        .unwrap_err()
        .contains("role must be a string"));
        assert!(validate(
            IntentId::ActClick,
            &json!({
                "tab": 1,
                "target": { "name": "Save" },
                "expect": { "text": "A", "timeout_ms": 30001 }
            })
        )
        .unwrap_err()
        .contains("0 through 30000"));
    }

    #[test]
    fn canonical_intent_builds_typed_effect_requests() {
        for (intent, mechanism, button, count) in [
            (
                IntentId::ActClick,
                MechanismId::PointerClick,
                Some("left"),
                Some(1),
            ),
            (
                IntentId::ActRightClick,
                MechanismId::PointerClick,
                Some("right"),
                Some(1),
            ),
            (
                IntentId::ActDoubleClick,
                MechanismId::PointerClick,
                Some("left"),
                Some(2),
            ),
            (
                IntentId::ActTripleClick,
                MechanismId::PointerClick,
                Some("left"),
                Some(3),
            ),
            (IntentId::ActHover, MechanismId::PointerHover, None, None),
            (
                IntentId::ActScrollIntoView,
                MechanismId::ScrollTargetIntoView,
                None,
                None,
            ),
        ] {
            let profile = action_profile(intent).unwrap();
            let request = action_request(
                OperationKey::new(OperationId::BrowserAct, intent),
                profile,
                7,
                "ref_1",
                &json!({"modifiers":"SHIFT"}),
            );
            assert_eq!(request.id(), mechanism);
            assert_eq!(request.input()["tab"], 7);
            assert_eq!(
                request.input().pointer("/target/ref"),
                Some(&json!("ref_1"))
            );
            assert_eq!(
                request.input().get("button").and_then(Value::as_str),
                button
            );
            assert_eq!(request.input().get("count").and_then(Value::as_u64), count);
            assert!(request.input().get("action").is_none());
            assert!(request.input().get("tabId").is_none());
        }

        let request = action_request(
            OperationKey::new(OperationId::BrowserAct, IntentId::ActSetValue),
            action_profile(IntentId::ActSetValue).unwrap(),
            7,
            "ref_2",
            &json!({"value":"hello"}),
        );
        assert_eq!(request.id(), MechanismId::FormSetValue);
        assert_eq!(request.input()["value"], "hello");
    }

    #[test]
    fn legacy_only_arguments_cannot_drive_the_canonical_handler() {
        let error = validate(
            IntentId::ActClick,
            &json!({
                "tabId": 1,
                "target": {"ref":"ref_1"},
                "action": "left_click"
            }),
        )
        .unwrap_err();
        assert!(error.contains("numeric tabId"));
    }

    #[test]
    fn post_action_safety_refusals_have_distinct_committed_receipts() {
        for (refusal, expected_kind, expected_copy) in [
            (
                PostActionRefusal::Paused,
                "postcondition_paused",
                "observation paused",
            ),
            (
                PostActionRefusal::Interrupted,
                "postcondition_interrupted",
                "observation was interrupted",
            ),
        ] {
            let mut result = json!({
                "content": [{"type":"text","text":"interaction receipt: action committed"}],
                "structuredContent": {
                    "interactionReceipt": {
                        "observedAfter": {},
                        "blockers": []
                    }
                }
            });
            append_post_action_refusal(&mut result, refusal, true);
            assert_eq!(result["isError"], true);
            assert_eq!(
                result.pointer("/structuredContent/interactionReceipt/blockers/0/kind"),
                Some(&json!(expected_kind))
            );
            assert!(result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(expected_copy));
            assert_ne!(expected_kind, "expect_timeout");
        }
    }
}
