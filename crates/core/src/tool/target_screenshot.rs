// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Target-based screenshot composition for the canonical surface.

use crate::browser::mechanism::{MechanismId, MechanismRequest};
use crate::operation::registry::SuccessDisposition;
use crate::tool::outcome::{
    delivery_failure_outcome, tool_error_outcome, ExecutionDisposition,
    ExecutionOutcome as CallOutcome, LocalCtx, LocalFuture, OperationExecution, ResolvedTargets,
};
use crate::ToolError;
use ghostlight_transport::operation::{
    BrowserResultStatus, OperationEffect, OperationKind, RetryDisposition,
};
use serde_json::{json, Value};

/// Registry entry point for a screenshot cropped to one semantic target.
pub(crate) fn target_screenshot_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(run(ctx))
}

async fn run(ctx: LocalCtx<'_>) -> CallOutcome {
    let args = ctx.input;
    if args.get("target").is_none() {
        let request = MechanismRequest::for_operation(
            OperationKind::BrowserTakeScreenshot,
            MechanismId::ScreenshotViewport,
            args.clone(),
        )
        .expect("viewport screenshot is declared by its operation plan");
        return match ctx
            .browser
            .execute_mechanism_with_delivery_outcome(ctx.guid, &request, ctx.execution)
            .await
        {
            Ok(result) => CallOutcome::Success {
                result: result.into(),
            },
            Err(failure) => delivery_failure_outcome(failure),
        };
    }
    let (Some(tab), Some(target)) = (args.get("tab").and_then(Value::as_i64), args.get("target"))
    else {
        return invalid("browser_take_screenshot target capture requires tab and target");
    };
    let root = OperationKind::BrowserTakeScreenshot;
    let resolve = MechanismRequest::for_operation(
        root,
        MechanismId::ElementResolve,
        json!({"tab":tab,"target":target}),
    )
    .expect("target screenshot resolution is declared by its dynamic plan");
    let resolved = match ctx
        .browser
        .execute_mechanism(ctx.guid, &resolve, ctx.execution)
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
        return blocked(
            "The screenshot target matched more than one element.",
            &resolved,
        );
    }
    let Some(box_) = resolved.pointer("/target/box").and_then(Value::as_object) else {
        return blocked(
            "The screenshot target was not found or had no visible bounds.",
            &resolved,
        );
    };
    let (Some(x), Some(y), Some(width), Some(height)) = (
        box_.get("x").and_then(Value::as_f64),
        box_.get("y").and_then(Value::as_f64),
        box_.get("width").and_then(Value::as_f64),
        box_.get("height").and_then(Value::as_f64),
    ) else {
        return blocked("The screenshot target had invalid bounds.", &resolved);
    };
    if width <= 0.0 || height <= 0.0 {
        return blocked("The screenshot target is not visibly sized.", &resolved);
    }
    let capture = MechanismRequest::for_operation(
        root,
        MechanismId::ScreenshotRegion,
        json!({"tab":tab,"region":[x,y,x+width,y+height]}),
    )
    .expect("target screenshot capture is declared by its dynamic plan");
    match ctx
        .browser
        .execute_mechanism_with_delivery_outcome(ctx.guid, &capture, ctx.execution)
        .await
    {
        Ok(result) => {
            let mut result = OperationExecution::new(result);
            result.targets = resolved
                .get("target")
                .cloned()
                .map_or(ResolvedTargets::None, ResolvedTargets::One);
            result.operation_tab = Some(tab);
            CallOutcome::Success {
                result: Box::new(result),
            }
        }
        Err(failure) => delivery_failure_outcome(failure),
    }
}

fn blocked(message: &str, resolved: &Value) -> CallOutcome {
    let mut result = OperationExecution::new(json!({
        "content":[{"type":"text","text":message}],
        "structuredContent":{
            "candidates":resolved.get("candidates").cloned().unwrap_or_else(|| json!([]))
        },
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
