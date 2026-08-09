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
use crate::tool::outcome::{
    delivery_failure_outcome, tool_error_outcome, ExecutionAuditFacts,
    ExecutionOutcome as CallOutcome, LocalCtx, LocalFuture, OperationExecution, ResolvedTargets,
};
use crate::work::WorkContext;
use ghostlight_transport::operation::{OperationEffect, OperationKind};
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

fn validate(profile: ActionProfile, args: &Value) -> Result<(), String> {
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
    if profile.mechanism == MechanismId::KeyPress
        && args
            .get("key")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err("key is required for browser_press_key".to_string());
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

fn operation_execution(
    result: Value,
    batch_id: &str,
    assurance: &str,
    outcome: &str,
    target: Option<Value>,
) -> OperationExecution {
    let mut execution = OperationExecution::new(result);
    execution.audit = ExecutionAuditFacts {
        batch_id: Some(batch_id.to_owned()),
        target_assurance: Some(assurance.to_owned()),
        outcome_category: Some(outcome.to_owned()),
    };
    execution.targets = target.map_or(ResolvedTargets::None, ResolvedTargets::One);
    execution
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
        "credential_target" => {
            "Ask the user to enter credentials directly in the browser; do not send them through browser_act."
        }
        "sensitive_classification_unavailable" => {
            "Update the browser adapter or ask the user to enter the value directly."
        }
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
    CallOutcome::Success {
        result: Box::new(operation_execution(
            result, batch_id, assurance, "blocked", None,
        )),
    }
}

fn internal_audit(
    governance: &Governance,
    operation: OperationKind,
    requires: Option<&'static [Capability]>,
    batch_id: &str,
    step: u32,
    work: &WorkContext,
) -> crate::governance::dispatch::CallAudit {
    let mut audit =
        governance.begin_with_client(operation.as_str(), None, requires, work.client().cloned());
    audit.orchestrated(operation.as_str(), batch_id, Some(step));
    audit.mark_mechanism_phase();
    audit.attribute_grant(None);
    audit
}

#[derive(Clone, Copy)]
struct ActionProfile {
    mechanism: MechanismId,
    cue_kind: &'static str,
    kind: OperationKind,
    button: Option<&'static str>,
    count: Option<u64>,
}

fn action_profile(kind: OperationKind, args: &Value) -> Option<ActionProfile> {
    let click = |cue_kind, button, count| ActionProfile {
        mechanism: MechanismId::PointerClick,
        cue_kind,
        kind: OperationKind::BrowserClick,
        button: Some(button),
        count: Some(count),
    };
    Some(match kind {
        OperationKind::BrowserClick => {
            let button = match args.get("button").and_then(Value::as_str) {
                Some("right") => "right",
                Some("middle") => "middle",
                _ => "left",
            };
            click(
                "click",
                button,
                args.get("clicks").and_then(Value::as_u64).unwrap_or(1),
            )
        }
        OperationKind::BrowserHover => ActionProfile {
            mechanism: MechanismId::PointerHover,
            cue_kind: "hover",
            kind: OperationKind::BrowserHover,
            button: None,
            count: None,
        },
        OperationKind::BrowserScrollToTarget => ActionProfile {
            mechanism: MechanismId::ScrollTargetIntoView,
            cue_kind: "scroll_into_view",
            kind: OperationKind::BrowserScrollToTarget,
            button: None,
            count: None,
        },
        OperationKind::BrowserPressKey => ActionProfile {
            mechanism: MechanismId::KeyPress,
            cue_kind: "press_key",
            kind: OperationKind::BrowserPressKey,
            button: None,
            count: None,
        },
        _ => return None,
    })
}

fn action_request(
    profile: ActionProfile,
    tab: i64,
    reference: &str,
    args: &Value,
) -> MechanismRequest {
    let mut input = json!({ "tab": tab, "target": { "ref": reference } });
    if let Some(button) = profile.button {
        input["button"] = json!(button);
    }
    if let Some(button) = args.get("button").and_then(Value::as_str) {
        input["button"] = json!(button);
    }
    if let Some(count) = profile.count {
        input["count"] = json!(count);
    }
    if profile.mechanism == MechanismId::FormSetValue {
        input["value"] = args["value"].clone();
        if args.get("reject_sensitive").and_then(Value::as_bool) == Some(true) {
            input["reject_sensitive"] = json!(true);
        }
    }
    if profile.mechanism == MechanismId::KeyPress {
        input["key"] = args["key"].clone();
        input["repeat"] = json!(1);
    }
    if let Some(modifiers) = args.get("modifiers") {
        input["modifiers"] = modifiers.clone();
    }
    MechanismRequest::for_operation(profile.kind, profile.mechanism, input)
        .expect("browser.act mechanism must be declared by its dynamic plan")
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
    let kind = operation.kind();
    let args = input;
    let Some(profile) = action_profile(kind, args) else {
        return invalid(format!("unsupported target interaction: {}", kind.as_str()));
    };
    if let Err(message) = validate(profile, args) {
        return invalid(message);
    }
    let root_operation = kind;
    let batch_id = uuid::Uuid::new_v4().to_string();
    let tab_id = args["tab"].as_i64().expect("validated tab");
    let target = args["target"].clone();
    let assurance = if target.get("ref").is_some() {
        "ref"
    } else {
        "semantic"
    };
    if cancellation.is_cancelled() {
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
                profile.kind,
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
    if profile.mechanism == MechanismId::FormSetValue
        && args.get("reject_sensitive").and_then(Value::as_bool) == Some(true)
    {
        let sensitive = resolved_target
            .get("sensitive")
            .and_then(Value::as_bool)
            .or_else(|| {
                (resolved_target.get("secret").and_then(Value::as_bool) == Some(true))
                    .then_some(true)
            });
        if sensitive != Some(false) {
            let (kind, message) = if sensitive == Some(true) {
                (
                    "credential_target",
                    "The resolved field is credential-class, so no value was sent to the page.",
                )
            } else {
                (
                    "sensitive_classification_unavailable",
                    "The browser adapter could not prove that the resolved field is non-sensitive, so no value was sent to the page.",
                )
            };
            return recovery_result(
                message.to_string(),
                &batch_id,
                assurance,
                page,
                kind,
                Vec::new(),
                false,
            );
        }
    }
    let Some(reference) = resolved_target.get("ref").and_then(Value::as_str) else {
        return invalid("resolved target did not carry a ref");
    };

    if cancellation.is_cancelled() {
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
                profile.kind,
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

    if cancellation.is_cancelled() {
        return CallOutcome::Cancelled {
            message: "act_on stopped after its presentation cue and before the browser action."
                .to_string(),
            effect: OperationEffect::None,
        };
    }

    let request = action_request(profile, tab_id, reference, args);
    let mut action_audit = internal_audit(
        governance,
        root_operation,
        Some(crate::operation::registry::descriptor(profile.kind).requires),
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
        Err(failure)
            if profile.mechanism == MechanismId::FormSetValue
                && args.get("reject_sensitive").and_then(Value::as_bool) == Some(true)
                && !failure.outcome_unknown
                && !failure.stops_composition() =>
        {
            return recovery_result(
                "The resolved field failed immediate non-sensitive target revalidation, so no value was sent to the page."
                    .to_string(),
                &batch_id,
                assurance,
                page,
                "sensitive_classification_unavailable",
                Vec::new(),
                false,
            );
        }
        Err(failure) => return delivery_failure_outcome(failure),
    };

    let mutations = result
        .pointer("/structuredContent/interactionReceipt/observedAfter/mutations")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let expect = args.get("expect").and_then(Value::as_object);
    if expect.is_some() || mutations > 0 {
        if cancellation.is_cancelled() {
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
            Some(&[Capability::Read]),
            &batch_id,
            4,
            work,
        );
        let waited = browser
            .execute_mechanism(
                guid,
                &MechanismRequest::for_operation(
                    profile.kind,
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
                return CallOutcome::Success {
                    result: Box::new(operation_execution(
                        result,
                        &batch_id,
                        assurance,
                        refusal.category(),
                        Some(resolved_target.clone()),
                    )),
                };
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
                return CallOutcome::Success {
                    result: Box::new(operation_execution(
                        result,
                        &batch_id,
                        assurance,
                        "expect_timeout",
                        Some(resolved_target.clone()),
                    )),
                };
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
    CallOutcome::Success {
        result: Box::new(operation_execution(
            result,
            &batch_id,
            assurance,
            category,
            Some(resolved_target.clone()),
        )),
    }
}
