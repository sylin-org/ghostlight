// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Canonical full-page or target-scoped prose reading.

use crate::browser::mechanism::{MechanismId, MechanismRequest};
use crate::operation::registry::SuccessDisposition;
use crate::tool::outcome::{
    delivery_failure_outcome, tool_error_outcome, ExecutionDisposition,
    ExecutionOutcome as CallOutcome, LocalCtx, LocalFuture, OperationExecution,
};
use crate::ToolError;
use ghostlight_transport::operation::{
    BrowserResultStatus, OperationEffect, OperationKind, RetryDisposition,
};
use serde_json::{json, Value};

/// Registry entry point for bounded page text reads.
pub(crate) fn page_read_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(run(ctx))
}

async fn run(ctx: LocalCtx<'_>) -> CallOutcome {
    let Some(mut input) = ctx.input.as_object().cloned() else {
        return invalid("browser_read_page arguments must be an object");
    };
    let Some(tab) = input.get("tab").and_then(Value::as_i64) else {
        return invalid("browser_read_page requires a controlled tab");
    };
    let root = OperationKind::BrowserReadPage;
    if let Some(target) = input.remove("target") {
        let request = MechanismRequest::for_operation(
            root,
            MechanismId::ElementResolve,
            json!({"tab":tab,"target":target}),
        )
        .expect("target read resolution is declared by its dynamic plan");
        let resolved = match ctx
            .browser
            .execute_mechanism(ctx.guid, &request, ctx.execution)
            .await
        {
            Ok(result) => result,
            Err(error) => return tool_error_outcome(error),
        };
        let resolved = resolved
            .pointer("/content/0/text")
            .and_then(Value::as_str)
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .unwrap_or(Value::Null);
        if resolved.get("ambiguous").and_then(Value::as_bool) == Some(true) {
            return blocked("The read target matched more than one element.", &resolved);
        }
        let Some(reference) = resolved.pointer("/target/ref").and_then(Value::as_str) else {
            return blocked(
                "The read target was not found. Inspect the page and use a fresh target.",
                &resolved,
            );
        };
        input.insert("target".into(), json!({"ref":reference}));
    }
    let request =
        MechanismRequest::for_operation(root, MechanismId::PageReadText, Value::Object(input))
            .expect("page read is declared by its dynamic plan");
    match ctx
        .browser
        .execute_mechanism_with_delivery_outcome(ctx.guid, &request, ctx.execution)
        .await
    {
        Ok(result) => CallOutcome::Success {
            result: result.into(),
        },
        Err(failure) => delivery_failure_outcome(failure),
    }
}

fn blocked(message: &str, resolved: &Value) -> CallOutcome {
    let mut result = OperationExecution::new(json!({
        "content":[{"type":"text","text":message}],
        "structuredContent":{"blockers":[{"kind":"target_missing","summary":message}],"candidates":resolved.get("candidates").cloned().unwrap_or_else(|| json!([]))},
        "isError":true
    }));
    result.disposition = ExecutionDisposition::Override(SuccessDisposition::new(
        BrowserResultStatus::Blocked,
        OperationEffect::None,
        Some(RetryDisposition::AfterStateChange),
    ));
    CallOutcome::Success {
        result: Box::new(result),
    }
}

fn invalid(message: &str) -> CallOutcome {
    CallOutcome::Failure {
        error: ToolError::invalid_request(message),
    }
}
