// SPDX-License-Identifier: Apache-2.0 OR MIT
//! URL navigation inside one admitted Ghostlight operation.
//!
//! This handler owns only physical mechanism sequencing. Governance, scheduling admission,
//! landing authorization, cancellation, audit, and result projection stay in the operation
//! pipeline.

use crate::browser::mechanism::compile_navigation_transaction;
use crate::tool::outcome::{
    delivery_failure_outcome, ExecutionOutcome as CallOutcome, LocalCtx, LocalFuture,
    OperationExecution,
};
use crate::ToolError;
use ghostlight_transport::operation::OperationKind;
use serde_json::Value;

/// Registry entry point for URL navigation that may create the workspace's first tab.
pub(crate) fn tab_navigation_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(run(ctx))
}

async fn run(ctx: LocalCtx<'_>) -> CallOutcome {
    if ctx.work.workspace().is_none() {
        return invalid("tab navigation requires a workspace");
    }
    if ctx.operation.kind() != OperationKind::BrowserNavigate {
        return invalid("unsupported tab navigation operation");
    }
    let request = match compile_navigation_transaction(ctx.input) {
        Ok(request) => request,
        Err(error) => return CallOutcome::Failure { error },
    };
    match ctx
        .browser
        .execute_mechanism_with_delivery_outcome(ctx.guid, &request, ctx.execution)
        .await
    {
        Ok(result) => {
            let mut result = OperationExecution::new(result);
            result.operation_tab = result
                .operation_tab
                .or_else(|| ctx.input.get("tab").and_then(Value::as_i64));
            CallOutcome::Success {
                result: Box::new(result),
            }
        }
        Err(failure) => delivery_failure_outcome(failure),
    }
}

fn invalid(message: &str) -> CallOutcome {
    CallOutcome::Failure {
        error: ToolError::invalid_request(message),
    }
}
