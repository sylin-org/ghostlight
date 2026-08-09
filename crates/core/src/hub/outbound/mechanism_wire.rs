// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Serializer for the feature-negotiated semantic browser-mechanism request wire.
//!
//! The browser identity advertises [`MECHANISM_REQUEST_V1`] when it accepts this envelope. The
//! service binds that fact to one exact browser-session generation before choosing this serializer;
//! covered older sessions continue through `adapter_wire_v0` instead.

use crate::browser::mechanism::{MechanismId, MechanismRequest};
use crate::ToolError;
use serde_json::{Map, Value};

/// Exact browser-identity feature that enables semantic mechanism request envelopes.
pub(super) const MECHANISM_REQUEST_V1: &str = "mechanismRequestV1";
/// Exact browser-identity feature that enables the one-deadline navigation transaction.
pub(super) const NAVIGATION_READINESS_V1: &str = "navigationReadinessV1";
/// Exact browser-identity feature that enables one-step observed tab creation at a URL.
pub(super) const ATOMIC_TAB_OPEN_V1: &str = "atomicTabOpenV1";
/// Exact browser-identity feature that proves value mutations enforce sensitive-target rejection.
pub(super) const STRICT_SENSITIVE_WRITES_V1: &str = "strictSensitiveWritesV1";

/// Serialize one ordinary request/reply mechanism to the semantic envelope.
pub(super) fn serialize_tool_request(
    request_id: &str,
    guid: &str,
    request: &MechanismRequest,
    result_feature: &str,
    execution: &Value,
    workspace_group_title: Option<&str>,
    navigation_readiness_v1: bool,
) -> Result<Vec<u8>, ToolError> {
    if request.id() == MechanismId::TabUrlQuery {
        return Err(ToolError::binary(
            "tab.url_query requires the auxiliary mechanism envelope",
        ));
    }

    let mut envelope = base_envelope(request_id, request)?;
    if !navigation_readiness_v1
        && matches!(
            request.id(),
            MechanismId::WorkspaceTabOpen
                | MechanismId::NavigateUrl
                | MechanismId::NavigateBack
                | MechanismId::NavigateForward
                | MechanismId::NavigateReload
        )
    {
        envelope
            .get_mut("input")
            .and_then(Value::as_object_mut)
            .expect("mechanism input was validated as an object")
            .remove("readiness");
    }
    envelope.insert("guid".into(), Value::String(guid.to_owned()));
    envelope.insert(
        "resultFeatures".into(),
        Value::Array(vec![Value::String(result_feature.to_owned())]),
    );
    envelope.insert("execution".into(), execution.clone());
    if let Some(group_title) = workspace_group_title {
        envelope.insert(
            crate::constants::workspace::REQUEST.into(),
            Value::Object(Map::from_iter([(
                crate::constants::workspace::GROUP_TITLE.into(),
                Value::String(group_title.to_owned()),
            )])),
        );
    }
    encode(envelope)
}

/// Serialize the typed tab URL query while retaining its distinct correlated reply class.
pub(super) fn serialize_tab_url_request(
    request_id: &str,
    request: &MechanismRequest,
    execution: &Value,
) -> Result<Vec<u8>, ToolError> {
    if request.id() != MechanismId::TabUrlQuery {
        return Err(ToolError::binary(format!(
            "mechanism {} is not a tab URL query",
            request.id()
        )));
    }
    let mut envelope = base_envelope(request_id, request)?;
    envelope.insert("execution".into(), execution.clone());
    encode(envelope)
}

fn base_envelope(
    request_id: &str,
    request: &MechanismRequest,
) -> Result<Map<String, Value>, ToolError> {
    if !request.input().is_object() {
        return Err(ToolError::invalid_request(format!(
            "mechanism {} input must be an object",
            request.id()
        )));
    }
    let mut envelope = Map::new();
    envelope.insert("id".into(), Value::String(request_id.to_owned()));
    envelope.insert("type".into(), Value::String("mechanism_request".into()));
    envelope.insert(
        "mechanism".into(),
        Value::String(request.id().as_str().into()),
    );
    envelope.insert("input".into(), request.input().clone());
    Ok(envelope)
}

fn encode(envelope: Map<String, Value>) -> Result<Vec<u8>, ToolError> {
    serde_json::to_vec(&Value::Object(envelope)).map_err(|error| {
        ToolError::binary(format!(
            "failed to encode the semantic mechanism request: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(id: MechanismId, input: Value) -> MechanismRequest {
        MechanismRequest::test_only(id, input)
    }

    #[test]
    fn ordinary_envelope_is_byte_exact_with_optional_workspace_last() {
        let bytes = serialize_tool_request(
            "7",
            "workspace-1",
            &request(
                MechanismId::NavigateUrl,
                json!({"url":"https://example.com","tab":9}),
            ),
            "tabDeltaV1",
            &json!({"class":"scheduled"}),
            Some("Ghostlight - Example"),
            true,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"id":"7","type":"mechanism_request","mechanism":"navigate.url","input":{"url":"https://example.com","tab":9},"guid":"workspace-1","resultFeatures":["tabDeltaV1"],"execution":{"class":"scheduled"},"workspace":{"groupTitle":"Ghostlight - Example"}}"#
        );
    }

    #[test]
    fn tab_url_envelope_is_byte_exact_and_keeps_canonical_input() {
        let bytes = serialize_tab_url_request(
            "8",
            &request(MechanismId::TabUrlQuery, json!({"tab":4})),
            &json!({"class":"safety_protocol"}),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"id":"8","type":"mechanism_request","mechanism":"tab.url_query","input":{"tab":4},"execution":{"class":"safety_protocol"}}"#
        );
    }

    #[test]
    fn all_typed_mechanisms_have_one_semantic_serialization_class() {
        for id in MechanismId::ALL {
            let request = request(*id, json!({}));
            let bytes = if *id == MechanismId::TabUrlQuery {
                serialize_tab_url_request("1", &request, &json!({})).unwrap()
            } else {
                serialize_tool_request(
                    "1",
                    "workspace",
                    &request,
                    "tabDeltaV1",
                    &json!({}),
                    None,
                    true,
                )
                .unwrap()
            };
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["type"], "mechanism_request", "{id}");
            assert_eq!(value["mechanism"], id.as_str(), "{id}");
            assert_eq!(value["input"], json!({}), "{id}");
        }
    }

    #[test]
    fn ordinary_and_auxiliary_classes_fail_closed_when_crossed() {
        let tab_url = request(MechanismId::TabUrlQuery, json!({"tab":4}));
        assert!(serialize_tool_request(
            "1",
            "workspace",
            &tab_url,
            "tabDeltaV1",
            &json!({}),
            None,
            true,
        )
        .is_err());
        let navigate = request(MechanismId::NavigateUrl, json!({}));
        assert!(serialize_tab_url_request("1", &navigate, &json!({})).is_err());
    }

    #[test]
    fn covered_typed_adapter_does_not_receive_readiness_input() {
        let request = request(
            MechanismId::NavigateUrl,
            json!({
                "tab": 9,
                "url": "https://example.com",
                "readiness": {"settle":true,"timeout_ms":10000,"min_ms":0}
            }),
        );
        let bytes = serialize_tool_request(
            "1",
            "workspace",
            &request,
            "tabDeltaV1",
            &json!({}),
            None,
            false,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(value.pointer("/input/readiness").is_none());
    }
}
