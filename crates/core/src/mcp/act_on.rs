// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `act_on` semantic interaction (ADR-0078 D3): resolve one target, refuse ambiguity, dispatch
//! one pre-authorized browser mechanism, and optionally observe a postcondition in one MCP call.
//!
//! The parent pipeline performs the complete RAWX decision before this handler runs. Internal
//! resolution, cue, action, and wait calls go directly to the browser and receive correlated audit
//! records; they never trigger a second policy prompt or use page content as authorization input.

use crate::browser::directory;
use crate::governance::dispatch::Governance;
use crate::governance::ports::Capability;
use crate::hub::outbound::browser::Browser;
use crate::hub::scheduling::ExecutionContext;
use crate::mcp::outcome::{CallOutcome, LocalCtx, LocalFuture};
use serde_json::{json, Map, Value};

const ACTIONS: &[&str] = &[
    "left_click",
    "right_click",
    "double_click",
    "hover",
    "scroll_to",
    "set_value",
];
const EXPECT_STATES: &[&str] = &["visible", "present", "gone"];

/// Registry entry point. The parent grant decision has completed before this runs.
pub(crate) fn act_on_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move {
        run(
            ctx.browser,
            ctx.governance,
            ctx.guid,
            ctx.args,
            ctx.execution,
        )
        .await
    })
}

fn invalid(message: impl Into<String>) -> CallOutcome {
    CallOutcome::Failure {
        error: crate::ToolError::invalid_request(message.into()).next_step(
            r#"use {"tabId":1,"target":{"name":"Save","role":"button"},"action":"left_click"}"#,
        ),
    }
}

fn validate(args: &Value) -> Result<(), String> {
    if args.get("tabId").and_then(Value::as_i64).is_none() {
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
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| "act_on requires an action".to_string())?;
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported act_on action: {action}"));
    }
    let has_value = args.get("value").is_some();
    if action == "set_value" && !has_value {
        return Err("value is required for set_value".to_string());
    }
    if action == "set_value" && args.get("value").and_then(Value::as_str).is_none() {
        return Err("value for set_value must be a string".to_string());
    }
    if action != "set_value" && has_value {
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
    let mut result = crate::mcp::types::text_content(format!(
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
    tool: &str,
    action: Option<&str>,
    requires: Option<&'static [Capability]>,
    batch_id: &str,
    step: u32,
) -> crate::governance::dispatch::CallAudit {
    let mut audit = governance.begin(tool, action, requires);
    audit.orchestrated("act_on", batch_id, Some(step));
    audit.attribute_grant(None);
    audit
}

async fn run(
    browser: &Browser,
    governance: &Governance,
    guid: &str,
    args: &Value,
    execution: &ExecutionContext,
) -> CallOutcome {
    if let Err(message) = validate(args) {
        return invalid(message);
    }
    let batch_id = uuid::Uuid::new_v4().to_string();
    let tab_id = args["tabId"].as_i64().expect("validated tabId");
    let target = args["target"].clone();
    let assurance = if target.get("ref").is_some() {
        "ref"
    } else {
        "semantic"
    };

    let mut resolve_audit = internal_audit(
        governance,
        "resolve_actionable",
        None,
        Some(&[Capability::Read]),
        &batch_id,
        1,
    );
    let resolved = browser
        .call_with_context(
            guid,
            "resolve_actionable_internal",
            &json!({ "tabId": tab_id, "target": target }),
            execution,
        )
        .await;
    resolve_audit.dispatch_finished();
    resolve_audit.complete();
    let resolved = match resolved {
        Ok(result) => first_text(&result)
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .unwrap_or_else(|| json!({ "target": null, "candidates": [], "page": {} })),
        Err(error) => return CallOutcome::Failure { error },
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

    let mut cue_audit = internal_audit(governance, "target_cue", None, Some(&[]), &batch_id, 2);
    let cue = browser
        .call_with_context(
            guid,
            "target_cue_internal",
            &json!({
                "tabId": tab_id,
                "x": resolved_target.get("x").and_then(Value::as_f64).unwrap_or(0.0),
                "y": resolved_target.get("y").and_then(Value::as_f64).unwrap_or(0.0),
                "action": args["action"]
            }),
            execution,
        )
        .await;
    cue_audit.dispatch_finished();
    cue_audit.complete();
    let _ = cue;

    let action = args["action"].as_str().expect("validated action");
    let (tool, dispatch_args, requirements) = if action == "set_value" {
        (
            "form_input",
            json!({ "tabId": tab_id, "ref": reference, "value": args["value"] }),
            directory::requires("form_input", None),
        )
    } else {
        (
            "computer",
            json!({ "tabId": tab_id, "ref": reference, "action": action }),
            directory::requires("computer", Some(action)),
        )
    };
    let mut action_audit = internal_audit(
        governance,
        tool,
        if tool == "computer" {
            Some(action)
        } else {
            None
        },
        requirements,
        &batch_id,
        3,
    );
    let dispatched = browser
        .call_with_context(guid, tool, &dispatch_args, execution)
        .await;
    action_audit.dispatch_finished();
    action_audit.complete();
    let mut result = match dispatched {
        Ok(result) => result,
        Err(error) => return CallOutcome::Failure { error },
    };

    let mutations = result
        .pointer("/structuredContent/interactionReceipt/observedAfter/mutations")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let expect = args.get("expect").and_then(Value::as_object);
    if expect.is_some() || mutations > 0 {
        let mut wait_args = Map::new();
        wait_args.insert("tabId".to_string(), json!(tab_id));
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
            "wait_for",
            None,
            directory::requires("wait_for", None),
            &batch_id,
            4,
        );
        let waited = browser
            .call_with_context(guid, "wait_for", &Value::Object(wait_args), execution)
            .await;
        wait_audit.dispatch_finished();
        wait_audit.complete();
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

    #[test]
    fn validates_target_value_and_expect_shapes() {
        assert!(validate(&json!({
            "tabId": 1,
            "target": { "name": "Save", "role": "button" },
            "action": "left_click",
            "expect": { "text": "Saved", "state": "visible", "timeout_ms": 5000 }
        }))
        .is_ok());
        assert!(validate(&json!({
            "tabId": 1,
            "target": { "ref": "ref_1", "query": "Save" },
            "action": "left_click"
        }))
        .unwrap_err()
        .contains("exactly one"));
        assert!(validate(&json!({
            "tabId": 1, "target": { "ref": "ref_1" }, "action": "set_value"
        }))
        .unwrap_err()
        .contains("value is required"));
        assert!(validate(&json!({
            "tabId": 1,
            "target": { "name": "Save" },
            "action": "left_click",
            "expect": { "text": "A", "selector": "#a" }
        }))
        .unwrap_err()
        .contains("exactly one"));
        assert!(validate(&json!({
            "tabId": 1, "target": { "ref": "ref_1" }, "action": "set_value", "value": true
        }))
        .unwrap_err()
        .contains("must be a string"));
        assert!(validate(&json!({
            "tabId": 1, "target": { "name": "Save", "role": 7 }, "action": "left_click"
        }))
        .unwrap_err()
        .contains("role must be a string"));
        assert!(validate(&json!({
            "tabId": 1,
            "target": { "name": "Save" },
            "action": "left_click",
            "expect": { "text": "A", "timeout_ms": 30001 }
        }))
        .unwrap_err()
        .contains("0 through 30000"));
    }
}
