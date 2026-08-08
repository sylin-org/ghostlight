// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Pure conversion from the current internal success shape to canonical browser results.
//!
//! The browser adapter and legacy local handlers currently return an MCP-like object containing
//! `content`, optional `structuredContent`, and optional `isError`. This module consumes that
//! temporary internal shape at the operation boundary. Protocol wrappers, vendor envelopes, and
//! unknown fields are rejected rather than retained in canonical data.

use super::registry::SuccessDisposition;
use ghostlight_transport::operation::{
    BrowserResult, OperationKey, PageProvenance, ResultPart, MAX_PAGE_ORIGIN_BYTES,
};
use ghostlight_transport::workspace_id::WorkspaceId;
use serde_json::{Map, Value};

/// A current internal success value cannot be represented by the canonical result vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResultConversionError {
    /// The current success boundary requires an object result.
    #[error("successful browser result must be an object")]
    RootNotObject,
    /// A top-level field is not part of the temporary internal success contract.
    #[error("successful browser result contains unsupported top-level field: {field}")]
    UnsupportedTopLevelField {
        /// Unsupported field name. Its value is never retained or rendered.
        field: String,
    },
    /// The optional content field was not an array.
    #[error("successful browser result content must be an array")]
    ContentNotArray,
    /// The optional error marker was not a boolean.
    #[error("successful browser result isError must be a boolean")]
    ErrorMarkerNotBoolean,
    /// One content item was not an object.
    #[error("successful browser result content block {index} must be an object")]
    ContentBlockNotObject {
        /// Zero-based content-block index.
        index: usize,
    },
    /// One content item had no string type discriminator.
    #[error("successful browser result content block {index} must have a string type")]
    ContentBlockTypeMissing {
        /// Zero-based content-block index.
        index: usize,
    },
    /// The current canonical result vocabulary does not support this block type.
    #[error("successful browser result content block {index} has unsupported type: {block_type}")]
    UnsupportedContentBlock {
        /// Zero-based content-block index.
        index: usize,
        /// Unsupported type discriminator. No block payload is retained.
        block_type: String,
    },
    /// A text block did not have the exact supported shape.
    #[error("successful browser result text block {index} must contain only type and string text")]
    InvalidTextBlock {
        /// Zero-based content-block index.
        index: usize,
    },
    /// An image block did not have one exact supported base64 shape.
    #[error(
        "successful browser result image block {index} must contain base64 data and a media type"
    )]
    InvalidImageBlock {
        /// Zero-based content-block index.
        index: usize,
    },
    /// Both reserved legacy provenance locations were populated, so placement is ambiguous.
    #[error("successful browser result contains conflicting provenance markers")]
    ConflictingProvenanceMarkers,
    /// A reserved legacy provenance marker did not have the exact service-authored shape.
    #[error("successful browser result has malformed provenance at {location}: {reason}")]
    MalformedProvenanceMarker {
        /// Reserved legacy marker location.
        location: &'static str,
        /// Stable validation reason without retaining marker payload.
        reason: &'static str,
    },
}

/// Convert one current internal successful result into the canonical result vocabulary.
///
/// Accepted top-level fields are `content`, `structuredContent`, and `isError`. Text blocks and
/// base64 image blocks become typed [`ResultPart`] values. `structuredContent` becomes canonical
/// structured data. The registry-derived [`SuccessDisposition`] supplies canonical status,
/// effect, and retry semantics. The temporary `isError` marker is validated and removed, but it
/// cannot independently weaken or strengthen that disposition. Unknown fields and unsupported
/// content shapes return [`ResultConversionError`] instead of crossing the bridge through a
/// fallback payload.
pub fn canonicalize_success(
    key: OperationKey,
    disposition: SuccessDisposition,
    workspace: Option<WorkspaceId>,
    value: Value,
) -> Result<BrowserResult, ResultConversionError> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or(ResultConversionError::RootNotObject)?;

    if let Some(field) = object
        .keys()
        .find(|field| !matches!(field.as_str(), "content" | "structuredContent" | "isError"))
        .cloned()
    {
        return Err(ResultConversionError::UnsupportedTopLevelField { field });
    }

    match object.remove("isError") {
        None | Some(Value::Bool(_)) => {}
        Some(_) => return Err(ResultConversionError::ErrorMarkerNotBoolean),
    }
    let parts = parse_content(object.remove("content"))?;
    let mut data = object.remove("structuredContent").unwrap_or(Value::Null);
    let provenance = lift_legacy_provenance(&mut data, &parts)?;

    let mut result = BrowserResult::new(key.id, key.intent, disposition.status, disposition.effect);
    result.retry = disposition.retry;
    result.workspace = workspace;
    result.parts = parts;
    result.data = data;
    result.provenance = provenance;
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyProvenancePlacement {
    Root,
    InteractionReceipt,
}

impl LegacyProvenancePlacement {
    const fn location(self) -> &'static str {
        match self {
            Self::Root => "structuredContent.provenance",
            Self::InteractionReceipt => "structuredContent.interactionReceipt.provenance",
        }
    }

    const fn data_pointer(self) -> &'static str {
        match self {
            Self::Root => "/data",
            Self::InteractionReceipt => "/data/interactionReceipt",
        }
    }
}

fn lift_legacy_provenance(
    data: &mut Value,
    parts: &[ResultPart],
) -> Result<Option<PageProvenance>, ResultConversionError> {
    let Some(root) = data.as_object() else {
        return Ok(None);
    };
    let root_marker = root.contains_key("provenance");
    let receipt_marker = root
        .get("interactionReceipt")
        .and_then(Value::as_object)
        .is_some_and(|receipt| receipt.contains_key("provenance"));
    if root_marker && receipt_marker {
        return Err(ResultConversionError::ConflictingProvenanceMarkers);
    }
    if root_marker && root.contains_key("interactionReceipt") {
        return Err(ResultConversionError::MalformedProvenanceMarker {
            location: LegacyProvenancePlacement::Root.location(),
            reason: "root marker cannot accompany interactionReceipt",
        });
    }
    let placement = match (root_marker, receipt_marker) {
        (false, false) => return Ok(None),
        (true, true) => unreachable!("conflicting markers were rejected above"),
        (true, false) => LegacyProvenancePlacement::Root,
        (false, true) => LegacyProvenancePlacement::InteractionReceipt,
    };

    let marker = match placement {
        LegacyProvenancePlacement::Root => data
            .as_object_mut()
            .and_then(|root| root.remove("provenance")),
        LegacyProvenancePlacement::InteractionReceipt => data
            .get_mut("interactionReceipt")
            .and_then(Value::as_object_mut)
            .and_then(|receipt| receipt.remove("provenance")),
    }
    .expect("the selected legacy provenance marker was observed above");

    let (top_origin, session_nonce, frame_origin) =
        parse_legacy_provenance_marker(marker, placement.location())?;
    let mut untrusted_fields = vec![placement.data_pointer().to_owned()];
    for (index, part) in parts.iter().enumerate() {
        let field = match part {
            ResultPart::Text { .. } => "text",
            ResultPart::Image { .. } => "data",
        };
        untrusted_fields.push(format!("/parts/{index}/{field}"));
    }

    PageProvenance::new(
        untrusted_fields,
        Some(top_origin),
        Some(session_nonce),
        frame_origin,
    )
    .map(Some)
    .map_err(|_| ResultConversionError::MalformedProvenanceMarker {
        location: placement.location(),
        reason: "frameOrigin is empty, contains a control character, or exceeds 240 UTF-8 bytes",
    })
}

fn parse_legacy_provenance_marker(
    marker: Value,
    location: &'static str,
) -> Result<(String, String, Option<String>), ResultConversionError> {
    let Value::Object(marker) = marker else {
        return malformed_provenance(location, "marker must be an object");
    };
    if marker.keys().any(|field| {
        !matches!(
            field.as_str(),
            "pageSourced" | "untrusted" | "topOrigin" | "frameOrigin" | "sessionNonce"
        )
    }) {
        return malformed_provenance(location, "marker contains an unsupported field");
    }
    if marker.get("pageSourced") != Some(&Value::Bool(true)) {
        return malformed_provenance(location, "pageSourced must be true");
    }
    if marker.get("untrusted") != Some(&Value::Bool(true)) {
        return malformed_provenance(location, "untrusted must be true");
    }
    let top_origin = marker
        .get("topOrigin")
        .and_then(Value::as_str)
        .filter(|origin| is_valid_origin(origin))
        .ok_or(ResultConversionError::MalformedProvenanceMarker {
            location,
            reason: "topOrigin must be non-empty, control-free, and at most 240 UTF-8 bytes",
        })?
        .to_owned();
    let session_nonce = marker
        .get("sessionNonce")
        .and_then(Value::as_str)
        .filter(|nonce| is_valid_session_nonce(nonce))
        .ok_or(ResultConversionError::MalformedProvenanceMarker {
            location,
            reason: "sessionNonce must be bounded lowercase even-length hexadecimal with at least 96 bits",
        })?
        .to_owned();
    let frame_origin = match marker.get("frameOrigin") {
        None => None,
        Some(Value::String(origin)) => Some(origin.clone()),
        Some(_) => return malformed_provenance(location, "frameOrigin must be a string"),
    };
    Ok((top_origin, session_nonce, frame_origin))
}

fn malformed_provenance<T>(
    location: &'static str,
    reason: &'static str,
) -> Result<T, ResultConversionError> {
    Err(ResultConversionError::MalformedProvenanceMarker { location, reason })
}

fn is_valid_origin(origin: &str) -> bool {
    !origin.is_empty()
        && origin.len() <= MAX_PAGE_ORIGIN_BYTES
        && !origin.chars().any(char::is_control)
}

fn is_valid_session_nonce(nonce: &str) -> bool {
    const MIN_NONCE_BYTES: usize = 12;
    const MAX_NONCE_BYTES: usize = 64;

    nonce.len() >= MIN_NONCE_BYTES * 2
        && nonce.len() <= MAX_NONCE_BYTES * 2
        && nonce.len().is_multiple_of(2)
        && nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn parse_content(value: Option<Value>) -> Result<Vec<ResultPart>, ResultConversionError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(blocks) = value else {
        return Err(ResultConversionError::ContentNotArray);
    };

    blocks
        .into_iter()
        .enumerate()
        .map(|(index, block)| parse_content_block(index, block))
        .collect()
}

fn parse_content_block(index: usize, block: Value) -> Result<ResultPart, ResultConversionError> {
    let Value::Object(block) = block else {
        return Err(ResultConversionError::ContentBlockNotObject { index });
    };
    let block_type = block
        .get("type")
        .and_then(Value::as_str)
        .ok_or(ResultConversionError::ContentBlockTypeMissing { index })?;

    match block_type {
        "text" => parse_text_block(index, block),
        "image" => parse_image_block(index, block),
        other => Err(ResultConversionError::UnsupportedContentBlock {
            index,
            block_type: other.to_owned(),
        }),
    }
}

fn parse_text_block(
    index: usize,
    block: Map<String, Value>,
) -> Result<ResultPart, ResultConversionError> {
    if block.len() != 2 {
        return Err(ResultConversionError::InvalidTextBlock { index });
    }
    let text = block
        .get("text")
        .and_then(Value::as_str)
        .ok_or(ResultConversionError::InvalidTextBlock { index })?;
    Ok(ResultPart::Text {
        text: text.to_owned(),
    })
}

fn parse_image_block(
    index: usize,
    block: Map<String, Value>,
) -> Result<ResultPart, ResultConversionError> {
    let parsed = if block.contains_key("source") {
        parse_source_image(&block)
    } else {
        parse_direct_image(&block)
    };
    let Some((data, mime_type)) = parsed else {
        return Err(ResultConversionError::InvalidImageBlock { index });
    };
    ResultPart::image(data, mime_type)
        .map_err(|_| ResultConversionError::InvalidImageBlock { index })
}

fn parse_direct_image(block: &Map<String, Value>) -> Option<(&str, &str)> {
    if block.len() != 3 {
        return None;
    }
    Some((
        block.get("data")?.as_str()?,
        block.get("mimeType")?.as_str()?,
    ))
}

fn parse_source_image(block: &Map<String, Value>) -> Option<(&str, &str)> {
    if !matches!(block.len(), 2 | 3) {
        return None;
    }
    if block
        .keys()
        .any(|field| !matches!(field.as_str(), "type" | "source" | "mimeType"))
    {
        return None;
    }
    let source = block.get("source")?.as_object()?;
    if source.get("type")?.as_str()? != "base64" {
        return None;
    }
    let data = source.get("data")?.as_str()?;

    let outer_mime = block.get("mimeType").and_then(Value::as_str);
    let source_snake_mime = source.get("media_type").and_then(Value::as_str);
    let source_camel_mime = source.get("mimeType").and_then(Value::as_str);
    let mime_count = usize::from(outer_mime.is_some())
        + usize::from(source_snake_mime.is_some())
        + usize::from(source_camel_mime.is_some());
    if mime_count != 1 {
        return None;
    }
    let mime_type = outer_mime.or(source_snake_mime).or(source_camel_mime)?;

    let expected_source_len = if source_snake_mime.is_some() || source_camel_mime.is_some() {
        3
    } else {
        2
    };
    if source.len() != expected_source_len {
        return None;
    }
    Some((data, mime_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::operation::{
        BrowserResultStatus, IntentId, OperationEffect, OperationId, RetryDisposition,
    };
    use serde_json::json;

    const KEY: OperationKey = OperationKey::new(OperationId::BrowserAct, IntentId::ActClick);
    const OK_COMMITTED: SuccessDisposition =
        SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::Committed, None);
    const OK_NONE: SuccessDisposition =
        SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::None, None);

    #[test]
    fn text_and_direct_image_convert_without_protocol_wrappers() {
        let workspace = WorkspaceId::mint();
        let result = canonicalize_success(
            KEY,
            OK_COMMITTED,
            Some(workspace.clone()),
            json!({
                "content": [
                    {"type": "text", "text": "clicked"},
                    {"type": "image", "data": "aW1hZ2U=", "mimeType": "image/jpeg"}
                ],
                "structuredContent": {"receipt": {"action": "click"}}
            }),
        )
        .expect("supported result converts");

        assert_eq!(result.operation, OperationId::BrowserAct);
        assert_eq!(result.intent, IntentId::ActClick);
        assert_eq!(result.status, BrowserResultStatus::Ok);
        assert_eq!(result.effect, OperationEffect::Committed);
        assert_eq!(result.workspace.as_ref(), Some(&workspace));
        assert_eq!(
            result.parts,
            vec![
                ResultPart::Text {
                    text: "clicked".into()
                },
                ResultPart::Image {
                    data: "aW1hZ2U=".into(),
                    mime_type: "image/jpeg".into()
                }
            ]
        );
        assert_eq!(result.data, json!({"receipt": {"action": "click"}}));

        let wire = serde_json::to_value(result).expect("canonical result serializes");
        assert!(wire.get("content").is_none());
        assert!(wire.get("structuredContent").is_none());
        assert!(wire.get("isError").is_none());
        assert!(wire.get("legacy_payload").is_none());
    }

    #[test]
    fn source_base64_images_accept_one_explicit_media_type_location() {
        for block in [
            json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "data": "R0lGODlh",
                    "media_type": "image/gif"
                }
            }),
            json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "data": "R0lGODlh",
                    "mimeType": "image/gif"
                }
            }),
            json!({
                "type": "image",
                "mimeType": "image/gif",
                "source": {"type": "base64", "data": "R0lGODlh"}
            }),
        ] {
            let result = canonicalize_success(KEY, OK_COMMITTED, None, json!({"content": [block]}))
                .expect("supported source image converts");
            assert_eq!(
                result.parts,
                vec![ResultPart::Image {
                    data: "R0lGODlh".into(),
                    mime_type: "image/gif".into()
                }]
            );
        }
    }

    #[test]
    fn direct_and_source_images_reject_invalid_base64_or_media_types() {
        for block in [
            json!({"type": "image", "data": "AAAA=", "mimeType": "image/png"}),
            json!({"type": "image", "data": "AAAA", "mimeType": "image/*"}),
            json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "data": "AA=",
                    "media_type": "image/png"
                }
            }),
            json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "data": "AAAA",
                    "media_type": "text/plain"
                }
            }),
        ] {
            assert_eq!(
                canonicalize_success(KEY, OK_NONE, None, json!({"content": [block]})),
                Err(ResultConversionError::InvalidImageBlock { index: 0 })
            );
        }
    }

    #[test]
    fn explicit_disposition_controls_error_marked_success() {
        let result = canonicalize_success(
            KEY,
            SuccessDisposition::new(
                BrowserResultStatus::Blocked,
                OperationEffect::None,
                Some(RetryDisposition::AfterStateChange),
            ),
            None,
            json!({
                "isError": true,
                "content": [{"type": "text", "text": "some fields changed before failure"}]
            }),
        )
        .expect("partial result converts");

        assert_eq!(result.status, BrowserResultStatus::Blocked);
        assert_eq!(result.effect, OperationEffect::None);
        assert_eq!(result.retry, Some(RetryDisposition::AfterStateChange));
        assert_eq!(
            result.parts,
            vec![ResultPart::Text {
                text: "some fields changed before failure".into()
            }]
        );
        assert!(serde_json::to_value(result)
            .expect("canonical result serializes")
            .get("isError")
            .is_none());
    }

    #[test]
    fn absent_content_and_structured_data_are_valid_empty_success() {
        let result = canonicalize_success(
            KEY,
            OK_NONE,
            None,
            json!({"structuredContent": {"available": true}}),
        )
        .expect("structured-only success converts");
        assert!(result.parts.is_empty());
        assert_eq!(result.data, json!({"available": true}));

        let empty =
            canonicalize_success(KEY, OK_NONE, None, json!({})).expect("empty success converts");
        assert!(empty.parts.is_empty());
        assert!(empty.data.is_null());
    }

    #[test]
    fn protocol_wrappers_and_unknown_payloads_fail_instead_of_crossing() {
        for (value, field) in [
            (json!({"content": [], "jsonrpc": "2.0"}), "jsonrpc"),
            (
                json!({"content": [], "resultType": "complete"}),
                "resultType",
            ),
            (
                json!({"content": [], "legacy_payload": {}}),
                "legacy_payload",
            ),
            (json!({"content": [], "vendor": {"raw": true}}), "vendor"),
        ] {
            assert_eq!(
                canonicalize_success(KEY, OK_NONE, None, value),
                Err(ResultConversionError::UnsupportedTopLevelField {
                    field: field.into()
                })
            );
        }
    }

    #[test]
    fn malformed_or_unsupported_blocks_fail_honestly() {
        let cases = [
            json!({"content": "not-an-array"}),
            json!({"content": ["not-an-object"]}),
            json!({"content": [{"text": "missing type"}]}),
            json!({"content": [{"type": "audio", "data": "AAAA"}]}),
            json!({"content": [{"type": "text", "text": 7}]}),
            json!({"content": [{"type": "text", "text": "ok", "extra": true}]}),
            json!({"content": [{"type": "image", "data": "AAAA"}]}),
            json!({
                "content": [{
                    "type": "image",
                    "source": {"type": "url", "data": "AAAA", "media_type": "image/png"}
                }]
            }),
            json!({
                "content": [{
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "data": "AAAA",
                        "media_type": "image/png",
                        "extra": true
                    }
                }]
            }),
            json!({
                "content": [{
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "data": "AAAA",
                        "media_type": "image/png"
                    },
                    "extra": true
                }]
            }),
        ];

        for value in cases {
            assert!(canonicalize_success(KEY, OK_NONE, None, value).is_err());
        }
    }

    #[test]
    fn malformed_error_marker_is_rejected() {
        assert_eq!(
            canonicalize_success(
                KEY,
                OK_NONE,
                None,
                json!({"isError": "true", "content": []})
            ),
            Err(ResultConversionError::ErrorMarkerNotBoolean)
        );
    }

    #[test]
    fn root_legacy_provenance_is_lifted_without_changing_boundary_text() {
        let boundary = "--- GHOSTLIGHT PAGE CONTENT 00112233445566778899aabb origin=https://example.com UNTRUSTED ---\nPrivate page text\n--- END GHOSTLIGHT PAGE CONTENT 00112233445566778899aabb ---";
        let result = canonicalize_success(
            KEY,
            OK_NONE,
            None,
            json!({
                "content": [{"type": "text", "text": boundary}],
                "structuredContent": {
                    "url": "https://example.com/private",
                    "provenance": {
                        "pageSourced": true,
                        "untrusted": true,
                        "topOrigin": "https://example.com",
                        "sessionNonce": "00112233445566778899aabb"
                    }
                }
            }),
        )
        .expect("root provenance lifts");

        assert_eq!(
            result.parts,
            vec![ResultPart::Text {
                text: boundary.into()
            }]
        );
        assert_eq!(result.data, json!({"url": "https://example.com/private"}));
        let provenance = result.provenance.expect("canonical provenance");
        assert_eq!(
            provenance.untrusted_fields(),
            &["/data".to_owned(), "/parts/0/text".to_owned()]
        );
        assert_eq!(provenance.top_origin(), Some("https://example.com"));
        assert_eq!(provenance.session_nonce(), Some("00112233445566778899aabb"));
        assert_eq!(provenance.frame_origin(), None);
    }

    #[test]
    fn receipt_legacy_provenance_is_lifted_with_frame_origin() {
        let result = canonicalize_success(
            KEY,
            OK_COMMITTED,
            None,
            json!({
                "content": [{"type": "text", "text": "receipt boundary remains byte exact"}],
                "structuredContent": {
                    "interactionReceipt": {
                        "action": "left_click",
                        "target": {"frameOrigin": "https://frame.example"},
                        "provenance": {
                            "pageSourced": true,
                            "untrusted": true,
                            "topOrigin": "https://example.com",
                            "frameOrigin": "https://frame.example",
                            "sessionNonce": "00112233445566778899aabbccddeeff"
                        }
                    },
                    "serviceFact": "retained"
                }
            }),
        )
        .expect("receipt provenance lifts");

        assert_eq!(
            result.data,
            json!({
                "interactionReceipt": {
                    "action": "left_click",
                    "target": {"frameOrigin": "https://frame.example"}
                },
                "serviceFact": "retained"
            })
        );
        let provenance = result.provenance.expect("canonical provenance");
        assert_eq!(
            provenance.untrusted_fields(),
            &[
                "/data/interactionReceipt".to_owned(),
                "/parts/0/text".to_owned()
            ]
        );
        assert_eq!(provenance.frame_origin(), Some("https://frame.example"));
    }

    #[test]
    fn malformed_or_conflicting_legacy_provenance_fails_closed() {
        let marker = json!({
            "pageSourced": true,
            "untrusted": true,
            "topOrigin": "https://example.com",
            "sessionNonce": "00112233445566778899aabb"
        });
        let conflicting = json!({
            "structuredContent": {
                "provenance": marker.clone(),
                "interactionReceipt": {"provenance": marker.clone()}
            }
        });
        assert_eq!(
            canonicalize_success(KEY, OK_NONE, None, conflicting),
            Err(ResultConversionError::ConflictingProvenanceMarkers)
        );
        assert!(matches!(
            canonicalize_success(
                KEY,
                OK_NONE,
                None,
                json!({
                    "structuredContent": {
                        "provenance": marker.clone(),
                        "interactionReceipt": {"action": "left_click"}
                    }
                })
            ),
            Err(ResultConversionError::MalformedProvenanceMarker { .. })
        ));

        for invalid in [
            json!("not-an-object"),
            json!({
                "pageSourced": false,
                "untrusted": true,
                "topOrigin": "https://example.com",
                "sessionNonce": "00112233445566778899aabb"
            }),
            json!({
                "pageSourced": true,
                "untrusted": true,
                "topOrigin": "https://example.com",
                "sessionNonce": "00112233"
            }),
            json!({
                "pageSourced": true,
                "untrusted": true,
                "topOrigin": "https://example.com",
                "frameOrigin": "x".repeat(MAX_PAGE_ORIGIN_BYTES + 1),
                "sessionNonce": "00112233445566778899aabb"
            }),
            json!({
                "pageSourced": true,
                "untrusted": true,
                "topOrigin": "https://example.com",
                "sessionNonce": "00112233445566778899aabb",
                "unexpected": true
            }),
        ] {
            assert!(matches!(
                canonicalize_success(
                    KEY,
                    OK_NONE,
                    None,
                    json!({"structuredContent": {"provenance": invalid}})
                ),
                Err(ResultConversionError::MalformedProvenanceMarker { .. })
            ));
        }
    }

    #[test]
    fn data_without_a_reserved_marker_is_unchanged() {
        let boundary_like_text = "--- GHOSTLIGHT PAGE CONTENT page-controlled text ---";
        let result = canonicalize_success(
            KEY,
            OK_NONE,
            None,
            json!({
                "content": [{"type": "text", "text": boundary_like_text}],
                "structuredContent": {
                    "receipt": {"provenanceLabel": "page-controlled"}
                }
            }),
        )
        .expect("marker-free result remains valid");

        assert_eq!(
            result.parts,
            vec![ResultPart::Text {
                text: boundary_like_text.into()
            }]
        );
        assert_eq!(
            result.data,
            json!({"receipt": {"provenanceLabel": "page-controlled"}})
        );
        assert!(result.provenance.is_none());
    }
}
