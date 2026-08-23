//! Governed result-aware flow composition over ordinary decoded operations.

use std::collections::HashMap;
use std::time::Instant;

use serde_json::{json, Map, Value};

use crate::governance::{Capability, CapabilitySet};
use crate::language::outcome::Outcome;
use crate::language::RunFlow;
use crate::workspace::WorkspaceLease;

use super::{
    result::Readiness, status_name, ApplicationExecutor, Effect, InvocationContext,
    InvocationResult, Status, Terminal,
};

const FLOW_RESULT_BUDGET_BYTES: usize = 100_000;

impl ApplicationExecutor {
    pub(super) fn flow(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &RunFlow,
    ) -> Terminal {
        let decision = self.authorize(context, CapabilitySet::EMPTY, None);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let total = value.steps.len();
        if value.dry_run {
            let mut rows = Vec::with_capacity(total);
            for step in &value.steps {
                let empty = HashMap::new();
                match substitute_references(&step.arguments, &empty) {
                    Ok(Some(arguments)) => {
                        let row = match crate::language::decode(&step.tool, arguments) {
                            Ok(operation) => {
                                let requirements =
                                    crate::language::capability_map::requirements(&operation);
                                json!({
                                    "id":step.id,
                                    "tool":operation.name(),
                                    "capabilities":capability_names(&requirements),
                                })
                            }
                            Err(error) => json!({"id":step.id,"decode_error":error.to_string()}),
                        };
                        rows.push(row);
                    }
                    Ok(None) => rows
                        .push(json!({"id":step.id,"error":"a result reference did not resolve"})),
                    Err(reason) => rows.push(json!({"id":step.id,"error":reason})),
                }
            }
            return Terminal {
                result: InvocationResult::new(
                    context.invocation,
                    Status::Succeeded,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    Outcome::FlowDecoded { steps: total }.summary().as_str(),
                    json!({"dry_run":true,"steps":rows}),
                    Outcome::FlowDecoded { steps: total }.next_steps(),
                ),
                decision,
                physical_id: None,
                observed: Outcome::FlowDecoded { steps: total }.observed(),
            };
        }
        let mut envelopes: HashMap<String, Value> = HashMap::new();
        let mut rows = Vec::with_capacity(total);
        let mut completed = 0usize;
        let mut budget_used = 0usize;
        let mut stopped = false;
        let mut saw_unknown = false;
        let mut saw_effect = false;
        for step in &value.steps {
            if context.cancellation.is_cancelled() || Instant::now() >= context.deadline {
                stopped = true;
                break;
            }
            let substituted = match substitute_references(&step.arguments, &envelopes) {
                Ok(Some(arguments)) => arguments,
                Ok(None) => {
                    rows.push(json!({"id":step.id,"error":"a result reference did not resolve"}));
                    if value.on_error == "stop" {
                        stopped = true;
                    }
                    continue;
                }
                Err(reason) => {
                    rows.push(json!({"id":step.id,"error":reason}));
                    if value.on_error == "stop" {
                        stopped = true;
                    }
                    continue;
                }
            };
            let decoded = match crate::language::decode(&step.tool, substituted) {
                Ok(operation) => operation,
                Err(error) => {
                    rows.push(json!({"id":step.id,"error":error.to_string()}));
                    if value.on_error == "stop" {
                        stopped = true;
                    }
                    continue;
                }
            };
            let terminal = self.run(context, lease, &decoded);
            completed += 1;
            match terminal.result.effect {
                Effect::Unknown => saw_unknown = true,
                Effect::Applied | Effect::Partial => saw_effect = true,
                Effect::None => {}
            }
            let envelope = serde_json::to_value(&terminal.result).unwrap_or(Value::Null);
            budget_used = budget_used
                .saturating_add(serde_json::to_string(&envelope).unwrap_or_default().len());
            let omitted = budget_used > FLOW_RESULT_BUDGET_BYTES;
            if omitted {
                rows.push(json!({
                    "id":step.id,
                    "status":status_name(terminal.result.status),
                    "omitted":true,
                }));
            } else {
                rows.push(json!({
                    "id":step.id,
                    "result":envelope,
                }));
            }
            envelopes.insert(step.id.clone(), envelope);
            if terminal.result.status != Status::Succeeded && value.on_error == "stop" {
                stopped = true;
            }
        }
        let effect = if saw_unknown {
            Effect::Unknown
        } else if completed < total && saw_effect {
            Effect::Partial
        } else if saw_effect {
            Effect::Applied
        } else {
            Effect::None
        };
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                if stopped || saw_unknown {
                    if saw_effect || saw_unknown {
                        Status::Unknown
                    } else {
                        Status::Failed
                    }
                } else {
                    Status::Succeeded
                },
                effect,
                Readiness::NotApplicable,
                false,
                Outcome::FlowRan {
                    completed,
                    total,
                    stopped,
                }
                .summary()
                .as_str(),
                json!({"completed":completed,"total":total,"stopped":stopped,"steps":rows}),
                Outcome::FlowRan {
                    completed,
                    total,
                    stopped,
                }
                .next_steps(),
            ),
            decision,
            physical_id: None,
            observed: Outcome::FlowRan {
                completed,
                total,
                stopped,
            }
            .observed(),
        }
    }
}

fn capability_names(requirements: &CapabilitySet) -> Vec<&'static str> {
    [
        (Capability::Read, "read"),
        (Capability::Action, "action"),
        (Capability::Write, "write"),
        (Capability::Execute, "execute"),
    ]
    .into_iter()
    .filter(|(capability, _)| requirements.contains(*capability))
    .map(|(_, name)| name)
    .collect()
}

/// Substitute every embedded `{"flow_ref":{...}}` with the referenced value.
/// Returns `Ok(false)` when a reference does not resolve.
fn substitute_references(
    input: &Value,
    envelopes: &HashMap<String, Value>,
) -> Result<Option<Value>, String> {
    match input {
        Value::Object(object) => {
            if let Some(reference) = object.get("flow_ref") {
                let parsed: crate::language::ResultReference =
                    serde_json::from_value(reference.clone())
                        .map_err(|_| "invalid flow_ref".to_string())?;
                let envelope = envelopes
                    .get(&parsed.step)
                    .ok_or_else(|| format!("reference to missing step `{}`", parsed.step))?;
                let resolved = envelope.pointer(&parsed.pointer).cloned().ok_or_else(|| {
                    format!(
                        "pointer `{}` did not resolve inside step `{}`",
                        parsed.pointer, parsed.step
                    )
                })?;
                return Ok(Some(resolved));
            }
            let mut replaced = Map::new();
            for (key, nested) in object {
                match substitute_references(nested, envelopes)? {
                    Some(value) => {
                        replaced.insert(key.clone(), value);
                    }
                    None => return Ok(None),
                }
            }
            Ok(Some(Value::Object(replaced)))
        }
        Value::Array(items) => {
            let mut replaced = Vec::with_capacity(items.len());
            for item in items {
                match substitute_references(item, envelopes)? {
                    Some(value) => replaced.push(value),
                    None => return Ok(None),
                }
            }
            Ok(Some(Value::Array(replaced)))
        }
        other => Ok(Some(other.clone())),
    }
}
