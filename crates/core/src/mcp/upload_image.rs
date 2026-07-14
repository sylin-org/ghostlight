// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `upload_image` tool (ADR-0050 Decision 4): upload a PREVIOUSLY CAPTURED screenshot -- resolved
//! from the per-session screenshot cache on [`Browser`] by `imageId` -- to a file input (`ref`) or a
//! drag-drop target (`coordinate`). The extension never reads the host filesystem: the bytes come
//! from the cache the `computer` screenshot action populated (ADR-0050 D4; see
//! `Browser::cache_screenshot`).
//!
//! Like `form_fill`, this is a `Handler::Local` that forwards to a dedicated EXTENSION-side command
//! (`upload_image_exec`, not an advertised tool). The parent `upload_image` call is governed +
//! audited once by the pipeline (requires Write); the internal forward carries the resolved bytes.

use crate::hub::outbound::browser::Browser;
use crate::hub::scheduling::ExecutionContext;
use crate::mcp::outcome::{CallOutcome, LocalCtx, LocalFuture};
use serde_json::{json, Value};

/// The ref/coordinate arg guard (ADR-0050 D4): exactly ONE of `ref` / `coordinate` must be present.
/// Pure, so it is unit-testable without a live [`Browser`]; returns the pinned error message on a
/// violation.
fn validate_target(args: &Value) -> Result<(), String> {
    let has_ref = args.get("ref").is_some_and(|v| !v.is_null());
    let has_coordinate = args.get("coordinate").is_some_and(|v| !v.is_null());
    match (has_ref, has_coordinate) {
        (true, true) => Err("Provide either ref or coordinate, not both.".to_string()),
        (false, false) => Err("Either ref or coordinate parameter is required.".to_string()),
        _ => Ok(()),
    }
}

/// A `CallOutcome::Failure` carrying a request error with `message`.
fn fail(message: impl Into<String>) -> CallOutcome {
    CallOutcome::Failure {
        error: crate::ToolError::invalid_request(message.into()),
    }
}

/// The `upload_image` `Handler::Local` entry point (ADR-0050 D4).
pub(crate) fn upload_image_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move { run(ctx.browser, ctx.guid, ctx.args, ctx.execution).await })
}

async fn run(
    browser: &Browser,
    guid: &str,
    args: &Value,
    execution: &ExecutionContext,
) -> CallOutcome {
    let Some(image_id) = args.get("imageId").and_then(Value::as_str) else {
        return fail("upload_image requires an imageId.");
    };
    if let Err(msg) = validate_target(args) {
        return fail(msg);
    }
    let Some(tab_id) = args.get("tabId").and_then(Value::as_i64) else {
        return fail("upload_image requires a numeric tabId.");
    };
    let filename = args
        .get("filename")
        .and_then(Value::as_str)
        .unwrap_or("image.png");

    let Some(cached) = browser.resolve_cached_image(guid, image_id) else {
        return fail(format!(
            "Image not found with ID: {image_id}. Capture it first with the computer screenshot action."
        ));
    };

    // Forward to the extension's dedicated `upload_image_exec` command (not an advertised tool) with
    // the resolved bytes; `ref` / `coordinate` (exactly one, guarded above) are passed through.
    let mut exec_args = json!({
        "tabId": tab_id,
        "filename": filename,
        "data": cached.base64,
        "mimeType": cached.media_type,
    });
    if let Some(r) = args.get("ref") {
        exec_args["ref"] = r.clone();
    }
    if let Some(c) = args.get("coordinate") {
        exec_args["coordinate"] = c.clone();
    }

    match browser
        .call_with_context(guid, "upload_image_exec", &exec_args, execution)
        .await
    {
        Ok(result) => CallOutcome::Success { result },
        Err(error) => CallOutcome::Failure { error },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_image_rejects_ref_and_coordinate_together() {
        let args = json!({"imageId": "img_x", "ref": "ref_1", "coordinate": [1, 2], "tabId": 0});
        assert_eq!(
            validate_target(&args).unwrap_err(),
            "Provide either ref or coordinate, not both."
        );
    }

    #[test]
    fn upload_image_requires_one_of_ref_or_coordinate() {
        let args = json!({"imageId": "img_x", "tabId": 0});
        assert_eq!(
            validate_target(&args).unwrap_err(),
            "Either ref or coordinate parameter is required."
        );
    }

    #[test]
    fn upload_image_accepts_exactly_one_target() {
        assert!(validate_target(&json!({"ref": "ref_1"})).is_ok());
        assert!(validate_target(&json!({"coordinate": [10, 20]})).is_ok());
    }
}
