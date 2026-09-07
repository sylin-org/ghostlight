//! Governed result-aware flow composition over ordinary decoded operations.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::governance::{Capability, CapabilitySet};
use crate::language::composition::StepCause;
use crate::language::history::{CompositionKind, StepReceipt};
use crate::language::outcome::Outcome;
use crate::language::RunFlow;
use crate::workspace::WorkspaceLease;

use super::{
    result::Readiness, ApplicationExecutor, Effect, InvocationContext, InvocationResult, Status,
    Terminal,
};

use super::composition::{terminal_row, unexecuted_row, Composition, UnexecutedStatus};

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
                audit: Outcome::FlowDecoded { steps: total }.audit(),
            };
        }
        let mut envelopes: HashMap<String, Value> = HashMap::new();
        let mut rows = Vec::with_capacity(total);
        let mut progress = Composition::new(total, decision, Readiness::NotApplicable, None);
        let mut budget_used = 0usize;
        for (index, step) in value.steps.iter().enumerate() {
            let position = index + 1;
            if progress.stop_at_boundary(context, position) {
                break;
            }
            let prepared = substitute_references(&step.arguments, &envelopes)
                .and_then(|arguments| {
                    arguments.ok_or_else(|| "a result reference did not resolve".into())
                })
                .map_err(|error| (StepCause::MissingReference, error))
                .and_then(|arguments| {
                    crate::language::decode(&step.tool, arguments)
                        .map_err(|error| (StepCause::InvalidArguments, error.to_string()))
                });
            let decoded = match prepared {
                Ok(operation) => operation,
                Err((cause, error)) => {
                    progress.not_started(position, cause);
                    self.record_preparation(
                        context,
                        &step.tool,
                        StepReceipt {
                            parent: CompositionKind::Flow,
                            position,
                            total,
                            preparation_failed: true,
                        },
                    );
                    let mut row = unexecuted_row(position, UnexecutedStatus::NotStarted);
                    row["id"] = json!(step.id);
                    row["cause"] = json!(cause);
                    row["error"] = json!(error);
                    rows.push(row);
                    if value.on_error == "stop" {
                        progress.progress.stopped = true;
                        break;
                    }
                    continue;
                }
            };
            let terminal = self.run_child(
                context,
                lease,
                &decoded,
                StepReceipt {
                    parent: CompositionKind::Flow,
                    position,
                    total,
                    preparation_failed: false,
                },
            );
            let cause = progress.record(position, &terminal);
            let mut row = terminal_row(position, &terminal, cause);
            row["id"] = json!(step.id);
            let envelope = serde_json::to_value(&terminal.result).unwrap_or(Value::Null);
            budget_used = budget_used
                .saturating_add(serde_json::to_string(&envelope).unwrap_or_default().len());
            if budget_used > FLOW_RESULT_BUDGET_BYTES {
                row["omitted"] = json!(true);
            } else {
                row["result"] = envelope.clone();
            }
            rows.push(row);
            envelopes.insert(step.id.clone(), envelope);
            if cause.is_some_and(StepCause::stops_execution)
                || (terminal.result.status != Status::Succeeded && value.on_error == "stop")
            {
                progress.progress.stopped = true;
                break;
            }
        }
        for (index, step) in value.steps.iter().enumerate().skip(rows.len()) {
            let mut row = unexecuted_row(index + 1, UnexecutedStatus::NotRun);
            row["id"] = json!(step.id);
            rows.push(row);
        }
        let facts = json!({"completed":progress.progress.counts.succeeded,"total":total,
            "stopped":progress.progress.stopped,"steps":rows});
        let mut terminal = progress.finish(context, facts);
        terminal.audit = terminal
            .audit
            .with_tools(value.steps.iter().map(|step| &step.tool));
        terminal
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
/// Missing referenced steps or pointers return an error before child decoding.
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
