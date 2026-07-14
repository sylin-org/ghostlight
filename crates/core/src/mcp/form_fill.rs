// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `form_fill` tool's orchestration (ADR-0036, PINS.md SS13): one Write-class governance
//! decision at the parent, then a dedicated `formStructure` internal read (C9), the matcher
//! (`browser::form_match`), and pre-authorized internal fills/submit -- each still individually
//! audited and correlated by `batch_id` (ADR-0036 Decision 7), symmetric with `script` (ADR-0035).
//!
//! Unlike `script`, `form_fill`'s internals do NOT re-enter [`pipeline::run_tool_call`]: the
//! parent's own governance decision already covers the whole interaction (ADR-0036 Decision 4),
//! so each internal dispatch goes straight to [`Browser::call`], with its own `CallAudit` scope
//! stamped `orchestrated("form_fill", batch_id, step)` and attributed to the parent's grant.
//!
//! No idempotency wrap (SS8 supersession note, C8/C10): `form_fill` fires once; a re-fire is the
//! caller's explicit choice.

use crate::browser::directory;
use crate::browser::form_match::{self, ControlRef, FormStructure};
use crate::governance::dispatch::Governance;
use crate::governance::ports::Capability;
use crate::hub::outbound::browser::Browser;
use crate::hub::scheduling::ExecutionContext;
use crate::mcp::outcome::{CallOutcome, LocalCtx, LocalFuture};
use serde_json::{json, Value};
use std::time::Instant;

/// The `form_fill` tool's `Handler::Local` entry point (post-grant dispatch position, PINS.md
/// SS2): the parent's governance decision has already run by the time this is called.
pub(crate) fn form_fill_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
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

/// Build a `Success` result carrying `isError: true` -- byte-identical to what
/// `pipeline::error_result` renders for a `CallOutcome::Failure`, but as a `Success` so the
/// `_batch_id` side channel (which only `take_batch_id` extracts from a `Success`, PINS.md SS7)
/// survives to stamp the parent audit record even when the call itself failed.
fn error_outcome(msg: impl Into<String>, batch_id: &str) -> CallOutcome {
    let mut result = crate::mcp::types::text_content(msg.into());
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

async fn run(
    browser: &Browser,
    governance: &Governance,
    guid: &str,
    args: &Value,
    execution: &ExecutionContext,
) -> CallOutcome {
    let started = Instant::now();
    let batch_id = uuid::Uuid::new_v4().to_string();

    let Some(tab_id) = args.get("tabId").and_then(Value::as_i64) else {
        return error_outcome("form_fill requires a numeric tabId", &batch_id);
    };
    let Some(fields_obj) = args.get("fields").and_then(Value::as_object) else {
        return error_outcome("form_fill requires a non-empty 'fields' object", &batch_id);
    };
    if fields_obj.is_empty() {
        return error_outcome(
            "form_fill requires at least one field in 'fields'",
            &batch_id,
        );
    }
    let submit_requested = args.get("submit").and_then(Value::as_bool).unwrap_or(false);

    // Step 1: the dedicated form-structure internal read (C9), audited as tool "form_structure"
    // (not "form_structure_internal": that name is the extension message only, PINS.md SS13).
    let mut structure_audit = governance.begin("form_structure", None, Some(&[Capability::Read]));
    structure_audit.orchestrated("form_fill", &batch_id, Some(1));
    // D-grant (C10 STOP note): the parent's resolved grant id lives only inside its own
    // `CallAudit` (a private field), which this handler has no way to reach -- `Gate::Proceed`
    // carries nothing. Internals attribute `None` rather than re-resolving a second grant lookup.
    structure_audit.attribute_grant(None);
    let structure_result = browser
        .call_with_context(
            guid,
            "form_structure_internal",
            &json!({ "tabId": tab_id }),
            execution,
        )
        .await;
    structure_audit.complete();

    let structure_value = match structure_result {
        Ok(v) => v,
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
    let mut held_abort = false;

    for (key, control) in &outcome.matched {
        if held_abort || browser.held_for().is_some() {
            held_abort = true;
            skipped.push(json!({ "label": key, "ref": control.ref_id, "reason": "held" }));
            continue;
        }
        if let Some(reason) = skip_reason(control) {
            skipped.push(json!({ "label": key, "ref": control.ref_id, "reason": reason }));
            continue;
        }

        let value = fields_obj.get(key).cloned().unwrap_or(Value::Null);
        let mut fill_audit =
            governance.begin("form_input", None, directory::requires("form_input", None));
        fill_audit.orchestrated("form_fill", &batch_id, Some(step));
        fill_audit.attribute_grant(None);
        let dispatch = browser
            .call_with_context(
                guid,
                "form_input",
                &json!({ "tabId": tab_id, "ref": control.ref_id, "value": value.clone() }),
                execution,
            )
            .await;
        fill_audit.complete();
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
            Err(e) => {
                skipped.push(json!({
                    "label": key,
                    "ref": control.ref_id,
                    "reason": format!("error: {e}"),
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

    if submit_requested && !held_abort && !filled.is_empty() {
        if let Some(idx) = outcome.form_index {
            if let Some(form) = structure.forms.iter().find(|f| f.form_index == idx) {
                if let Some(candidate) = form.submits.first() {
                    submit_ref = Some(candidate.ref_id.clone());
                    let mut submit_audit = governance.begin(
                        "computer",
                        Some("left_click"),
                        directory::requires("computer", Some("left_click")),
                    );
                    submit_audit.orchestrated("form_fill", &batch_id, Some(step));
                    submit_audit.attribute_grant(None);
                    let dispatch = browser
                        .call_with_context(
                            guid,
                            "computer",
                            &json!({ "action": "left_click", "tabId": tab_id, "ref": candidate.ref_id }),
                            execution,
                        )
                        .await;
                    submit_audit.complete();
                    if let Ok(result) = dispatch {
                        submitted = true;
                        observation = extract_observation(&result);
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

    let mut result = crate::mcp::types::text_content(text);
    if let Some(obj) = result.as_object_mut() {
        obj.insert("structuredContent".to_string(), structured);
        obj.insert("_batch_id".to_string(), json!(batch_id));
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
