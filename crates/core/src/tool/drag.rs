// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Semantic two-target drag composition.

use crate::browser::mechanism::{MechanismId, MechanismRequest};
use crate::governance::ports::Capability;
use crate::tool::outcome::{
    delivery_failure_outcome, tool_error_outcome, ExecutionOutcome as CallOutcome, LocalCtx,
    LocalFuture, OperationExecution, ResolvedTargets,
};
use crate::ToolError;
use ghostlight_transport::operation::{OperationEffect, OperationKind};
use serde_json::{json, Value};

/// Registry entry point for a semantic source-to-destination drag.
pub(crate) fn drag_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(run(ctx))
}

async fn run(ctx: LocalCtx<'_>) -> CallOutcome {
    let args = ctx.input;
    let Some(tab) = args.get("tab").and_then(Value::as_i64) else {
        return invalid("browser_drag requires a controlled tab");
    };
    let Some(from) = args.get("from") else {
        return invalid("browser_drag requires from");
    };
    let Some(to) = args.get("to") else {
        return invalid("browser_drag requires to");
    };
    let root = OperationKind::BrowserDrag;
    let batch = uuid::Uuid::new_v4().to_string();

    let (source, source_fact) = match resolve(&ctx, root, tab, from, &batch, 1).await {
        Ok(resolved) => resolved,
        Err(outcome) => return outcome,
    };
    let (destination, destination_fact) = match resolve(&ctx, root, tab, to, &batch, 2).await {
        Ok(resolved) => resolved,
        Err(outcome) => return outcome,
    };
    if ctx.cancellation.is_cancelled() {
        return CallOutcome::Cancelled {
            message: "The drag was cancelled before dispatch.".into(),
            effect: OperationEffect::None,
        };
    }

    let request = MechanismRequest::for_operation(
        OperationKind::BrowserDrag,
        MechanismId::PointerDrag,
        json!({"tab":tab,"from":source,"to":destination}),
    )
    .expect("browser_drag mechanism is declared by its dynamic plan");
    let mut audit = mechanism_audit(&ctx, root, &[Capability::Interact], &batch, 3);
    let result = ctx
        .browser
        .execute_mechanism_with_delivery_outcome(ctx.guid, &request, ctx.execution)
        .await;
    audit.dispatch_finished();
    audit.complete();
    match result {
        Ok(result) => {
            let mut result = OperationExecution::new(result);
            result.operation_tab = Some(tab);
            result.audit.batch_id = Some(batch);
            result.targets = ResolvedTargets::Drag {
                from: source_fact,
                to: destination_fact,
            };
            CallOutcome::Success {
                result: Box::new(result),
            }
        }
        Err(failure) => delivery_failure_outcome(failure),
    }
}

async fn resolve(
    ctx: &LocalCtx<'_>,
    root: OperationKind,
    tab: i64,
    target: &Value,
    batch: &str,
    step: u32,
) -> Result<(Value, Value), CallOutcome> {
    let request = MechanismRequest::for_operation(
        OperationKind::BrowserDrag,
        MechanismId::ElementResolve,
        json!({"tab":tab,"target":target}),
    )
    .expect("browser_drag resolution is declared by its dynamic plan");
    let mut audit = mechanism_audit(ctx, root, &[Capability::Read], batch, step);
    let response = ctx
        .browser
        .execute_mechanism(ctx.guid, &request, ctx.execution)
        .await;
    audit.dispatch_finished();
    audit.complete();
    let response = response.map_err(tool_error_outcome)?;
    let resolved = response
        .pointer("/content/0/text")
        .and_then(Value::as_str)
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .unwrap_or(Value::Null);
    if resolved.get("ambiguous").and_then(Value::as_bool) == Some(true) {
        return Err(blocked("The drag target matched more than one element."));
    }
    if resolved.get("covered").and_then(Value::as_bool) == Some(true) {
        return Err(blocked("The drag target is covered by another element."));
    }
    let Some(target) = resolved.get("target").and_then(Value::as_object) else {
        return Err(blocked(
            "The drag target was not found. Inspect the page and use a fresh target.",
        ));
    };
    let (Some(x), Some(y)) = (
        target.get("x").and_then(Value::as_f64),
        target.get("y").and_then(Value::as_f64),
    ) else {
        return Err(blocked("The drag target has no usable point."));
    };
    Ok((json!([x, y]), Value::Object(target.clone())))
}

fn mechanism_audit(
    ctx: &LocalCtx<'_>,
    operation: OperationKind,
    requires: &'static [Capability],
    batch: &str,
    step: u32,
) -> crate::governance::dispatch::CallAudit {
    let mut audit = ctx.governance.begin_with_client(
        operation.as_str(),
        None,
        Some(requires),
        ctx.work.client().cloned(),
    );
    audit.orchestrated("browser.drag", batch, Some(step));
    audit.mark_mechanism_phase();
    audit.attribute_grant(None);
    audit
}

fn blocked(message: &str) -> CallOutcome {
    CallOutcome::Success {
        result: json!({
            "content":[{"type":"text","text":message}],
            "structuredContent":{"blockers":[{"kind":"target_missing","summary":message}]},
            "isError":true
        })
        .into(),
    }
}

fn invalid(message: &str) -> CallOutcome {
    CallOutcome::Failure {
        error: ToolError::invalid_request(message),
    }
}
