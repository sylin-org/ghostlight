// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The captured-artifact upload operation (ADR-0050 Decision 4): resolve a previously captured
//! screenshot from the per-workspace cache on [`Browser`] by canonical artifact id, then deliver
//! it to a target ref or point. The extension never reads the host filesystem: the bytes come
//! from the cache the screenshot mechanism populated (ADR-0050 D4; see
//! `Browser::cache_screenshot`).
//!
//! This is a `Handler::Local`: the parent operation is governed and audited once by the pipeline
//! (requires Write), while the local handler resolves the artifact and emits one typed physical
//! upload mechanism.

use crate::browser::mechanism::{MechanismId, MechanismRequest};
use crate::hub::outbound::browser::Browser;
use crate::hub::scheduling::ExecutionContext;
use crate::tool::outcome::{delivery_failure_outcome, CallOutcome, LocalCtx, LocalFuture};
use ghostlight_transport::operation::OperationKey;
use serde_json::{json, Value};

/// The target guard (ADR-0050 D4): exactly one canonical target ref or point must be present.
/// Pure, so it is unit-testable without a live [`Browser`]; returns the pinned error message on a
/// violation.
fn validate_target(args: &Value) -> Result<(), String> {
    let has_ref = args
        .pointer("/target/ref")
        .and_then(Value::as_str)
        .is_some_and(|reference| !reference.is_empty());
    let has_point = args.get("point").is_some_and(|value| !value.is_null());
    match (has_ref, has_point) {
        (true, true) => Err("Provide either target.ref or point, not both.".to_string()),
        (false, false) => Err("Either target.ref or point is required.".to_string()),
        _ => Ok(()),
    }
}

fn upload_request(
    operation: OperationKey,
    tab: i64,
    filename: &str,
    data: String,
    mime_type: String,
    args: &Value,
) -> MechanismRequest {
    let mut input = json!({
        "tab": tab,
        "filename": filename,
        "data": data,
        "mime_type": mime_type,
    });
    if let Some(reference) = args.pointer("/target/ref") {
        input["target"] = json!({ "ref": reference });
    }
    if let Some(point) = args.get("point") {
        input["point"] = point.clone();
    }
    MechanismRequest::for_operation(operation, MechanismId::UploadImage, input)
        .expect("captured-artifact delivery must be declared by its dynamic plan")
}

/// A `CallOutcome::Failure` carrying a request error with `message`.
fn fail(message: impl Into<String>) -> CallOutcome {
    CallOutcome::Failure {
        error: crate::ToolError::invalid_request(message.into()),
    }
}

/// The `upload_image` `Handler::Local` entry point (ADR-0050 D4).
pub(crate) fn upload_image_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move {
        run(
            ctx.browser,
            ctx.guid,
            ctx.operation.key(),
            &ctx.operation.arguments,
            ctx.execution,
        )
        .await
    })
}

async fn run(
    browser: &Browser,
    guid: &str,
    operation: OperationKey,
    args: &Value,
    execution: &ExecutionContext,
) -> CallOutcome {
    let Some(artifact) = args.get("artifact").and_then(Value::as_str) else {
        return fail("captured-artifact upload requires an artifact id.");
    };
    if let Err(msg) = validate_target(args) {
        return fail(msg);
    }
    let Some(tab) = args.get("tab").and_then(Value::as_i64) else {
        return fail("captured-artifact upload requires a numeric tab.");
    };
    let filename = args
        .get("filename")
        .and_then(Value::as_str)
        .unwrap_or("image.png");

    let Some(cached) = browser.resolve_cached_image(guid, artifact) else {
        return fail(format!(
            "Artifact not found with ID: {artifact}. Capture a screenshot first."
        ));
    };

    let request = upload_request(
        operation,
        tab,
        filename,
        cached.base64,
        cached.media_type,
        args,
    );

    match browser
        .execute_mechanism_with_delivery_outcome(guid, &request, execution)
        .await
    {
        Ok(result) => CallOutcome::Success { result },
        Err(failure) => delivery_failure_outcome(failure),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::operation::{IntentId, OperationId};

    fn upload_operation() -> OperationKey {
        OperationKey::new(OperationId::BrowserUpload, IntentId::UploadCapturedArtifact)
    }

    #[test]
    fn upload_image_rejects_ref_and_point_together() {
        let args = json!({"artifact":"img_x","target":{"ref":"ref_1"},"point":[1,2],"tab":0});
        assert_eq!(
            validate_target(&args).unwrap_err(),
            "Provide either target.ref or point, not both."
        );
    }

    #[test]
    fn upload_image_requires_one_of_target_ref_or_point() {
        let args = json!({"artifact":"img_x","tab":0});
        assert_eq!(
            validate_target(&args).unwrap_err(),
            "Either target.ref or point is required."
        );
    }

    #[test]
    fn upload_image_accepts_exactly_one_target() {
        assert!(validate_target(&json!({"target":{"ref":"ref_1"}})).is_ok());
        assert!(validate_target(&json!({"point":[10,20]})).is_ok());
    }

    #[test]
    fn upload_image_builds_a_typed_canonical_request() {
        for args in [json!({"target":{"ref":"ref_1"}}), json!({"point":[10,20]})] {
            let request = upload_request(
                upload_operation(),
                4,
                "capture.png",
                "AAAA".to_string(),
                "image/png".to_string(),
                &args,
            );
            assert_eq!(request.id(), MechanismId::UploadImage);
            assert_eq!(request.input()["tab"], 4);
            assert_eq!(request.input()["mime_type"], "image/png");
            assert!(request.input().get("tabId").is_none());
            assert!(request.input().get("mimeType").is_none());
            assert!(request.input().get("ref").is_none());
            assert!(request.input().get("coordinate").is_none());
        }
    }

    #[test]
    fn legacy_only_arguments_cannot_drive_canonical_upload() {
        let legacy = json!({
            "imageId":"img_x",
            "tabId":1,
            "ref":"ref_1",
            "coordinate":[1,2]
        });
        assert!(validate_target(&legacy).is_err());
        assert!(legacy.get("artifact").is_none());
        assert!(legacy.get("tab").is_none());
    }
}
