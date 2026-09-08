//! Governed result-aware flow composition over ordinary decoded operations.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::governance::CapabilitySet;
use crate::language::composition::StepCause;
use crate::language::history::{CompositionKind, StepReceipt};
use crate::language::RunFlow;
use crate::workspace::WorkspaceLease;

use super::{result::Readiness, ApplicationExecutor, Effect, InvocationContext, Status, Terminal};

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
                    crate::language::decode_flow_step(&step.tool, arguments, value.tab.as_deref())
                        .map_err(|error| (StepCause::InvalidArguments, error.to_string()))
                });
            let decoded = match prepared {
                Ok(operation) => operation,
                Err((cause, error)) => {
                    progress.not_started(position, cause);
                    let storage = self.record_preparation(
                        context,
                        &step.tool,
                        StepReceipt {
                            parent: CompositionKind::Flow,
                            position,
                            total,
                            preparation_failed: true,
                        },
                    );
                    progress.record_storage(storage);
                    let mut row = unexecuted_row(position, UnexecutedStatus::NotStarted);
                    row["history_storage"] = json!(storage);
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
