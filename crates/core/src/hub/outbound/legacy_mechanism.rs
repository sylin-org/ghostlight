// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Isolated serializer for the covered pre-mechanism browser-adapter wire.
//!
//! Service code dispatches typed [`MechanismRequest`] values. This module is the sole compatibility
//! boundary that knows the older adapter's tool aliases, argument spellings, and envelope shape.

use crate::browser::mechanism::{
    BrowserControl, BrowserControlId, BrowserEvent, BrowserEventId, MechanismId, MechanismRequest,
    RecordingEndReason,
};
use crate::ToolError;
use serde_json::{Map, Value};

pub(super) const TAB_DELTA_V1: &str = "tabDeltaV1";

const LEGACY_TOP_LEVEL_FIELDS: &[&str] = &[
    "action",
    "tabId",
    "createIfEmpty",
    "ref",
    "ref_id",
    "coordinate",
    "start_coordinate",
    "scroll_direction",
    "scroll_amount",
    "onlyErrors",
    "urlPattern",
    "imageId",
    "mimeType",
    "recordingId",
    "maxSide",
    "minIntervalMs",
    "leaseMs",
    "hardTimeoutMs",
];

/// Every covered inbound message kind on the pre-mechanism adapter wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InboundKind {
    ToolAccepted,
    ToolTerminal,
    SurfaceDestroyed,
    SessionKilled,
    FocusChanged,
    DebugEvent,
    RecordingFrame,
    RecordingEnded,
    HoldGet,
    HoldSet,
    HoldToggle,
    AttentionGet,
    AttentionAction,
    ToolResponse,
    ToolError,
    TabUrlResponse,
}

/// Parse one exact covered inbound type. Unknown types remain outside this compatibility seam.
pub(super) fn parse_inbound_kind(message: &Value) -> Option<InboundKind> {
    let kind = match message.get("type").and_then(Value::as_str)? {
        "tool_accepted" => InboundKind::ToolAccepted,
        "tool_terminal" => InboundKind::ToolTerminal,
        "surface_destroyed" => InboundKind::SurfaceDestroyed,
        "session_killed" => InboundKind::SessionKilled,
        "focus" => InboundKind::FocusChanged,
        "debug_event" => InboundKind::DebugEvent,
        "gif_frame" => InboundKind::RecordingFrame,
        "gif_capture_ended" => InboundKind::RecordingEnded,
        "get_hold" => InboundKind::HoldGet,
        "set_hold" => InboundKind::HoldSet,
        "toggle_hold" => InboundKind::HoldToggle,
        "get_attention" => InboundKind::AttentionGet,
        "attention_action" => InboundKind::AttentionAction,
        "tool_response" => InboundKind::ToolResponse,
        "tool_error" => InboundKind::ToolError,
        "tab_url_response" => InboundKind::TabUrlResponse,
        _ => return None,
    };
    Some(kind)
}

/// Exact correlated reply kinds accepted from the covered adapter wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CorrelatedReplyKind {
    /// Result of a legacy tool request.
    ToolResponse,
    /// Failure of either correlated request class.
    ToolError,
    /// Result of the auxiliary tab URL query.
    TabUrlResponse,
}

/// Narrow a covered inbound kind to the correlated reply vocabulary.
pub(super) fn correlated_reply_kind(kind: InboundKind) -> Option<CorrelatedReplyKind> {
    match kind {
        InboundKind::ToolResponse => Some(CorrelatedReplyKind::ToolResponse),
        InboundKind::ToolError => Some(CorrelatedReplyKind::ToolError),
        InboundKind::TabUrlResponse => Some(CorrelatedReplyKind::TabUrlResponse),
        _ => None,
    }
}

/// Service-owned replies to covered extension control requests.
pub(super) enum ServiceReply<'a> {
    HoldState { id: &'a str, held: bool },
    HoldError { id: &'a str, error: &'a str },
    AttentionState { id: &'a str, result: &'a Value },
    AttentionError { id: &'a str, error: &'a str },
}

/// Serialize one service-owned control reply to the unchanged covered adapter wire.
pub(super) fn serialize_service_reply(reply: &ServiceReply<'_>) -> Result<Vec<u8>, ToolError> {
    let mut message = Map::new();
    match reply {
        ServiceReply::HoldState { id, held } => {
            message.insert("id".into(), Value::String((*id).to_owned()));
            message.insert("type".into(), Value::String("hold_state".into()));
            message.insert("result".into(), serde_json::json!({ "held": held }));
        }
        ServiceReply::HoldError { id, error } => {
            message.insert("id".into(), Value::String((*id).to_owned()));
            message.insert("type".into(), Value::String("hold_error".into()));
            message.insert("error".into(), Value::String((*error).to_owned()));
        }
        ServiceReply::AttentionState { id, result } => {
            message.insert("id".into(), Value::String((*id).to_owned()));
            message.insert("type".into(), Value::String("attention_state".into()));
            message.insert("result".into(), (*result).clone());
        }
        ServiceReply::AttentionError { id, error } => {
            message.insert("id".into(), Value::String((*id).to_owned()));
            message.insert("type".into(), Value::String("attention_error".into()));
            message.insert("error".into(), Value::String((*error).to_owned()));
        }
    }
    serde_json::to_vec(&Value::Object(message)).map_err(|error| {
        ToolError::binary(format!("failed to encode browser control reply: {error}"))
    })
}

/// Serialize one typed one-way control to the unchanged covered adapter message.
pub(super) fn serialize_control(control: &BrowserControl) -> Result<Vec<u8>, ToolError> {
    use BrowserControlId::*;

    let input = control.input().as_object().ok_or_else(|| {
        ToolError::invalid_request(format!(
            "browser control {} input must be an object",
            control.id()
        ))
    })?;
    let mut message = Map::new();
    match control.id() {
        RecordingLeaseRenew => {
            reject_control_fields(control, &["tab", "recording_id", "generation", "lease_ms"])?;
            message.insert("type".into(), Value::String("gif_lease_renew".into()));
            copy_required(input, &mut message, "tab", "tabId", control.id())?;
            copy_required(
                input,
                &mut message,
                "recording_id",
                "recordingId",
                control.id(),
            )?;
            copy_required(
                input,
                &mut message,
                "generation",
                "generation",
                control.id(),
            )?;
            copy_required(input, &mut message, "lease_ms", "leaseMs", control.id())?;
        }
        RecordingCancel => {
            reject_control_fields(control, &["tab", "recording_id", "generation"])?;
            message.insert("type".into(), Value::String("gif_capture_cancel".into()));
            copy_required(input, &mut message, "tab", "tabId", control.id())?;
            copy_required(
                input,
                &mut message,
                "recording_id",
                "recordingId",
                control.id(),
            )?;
            copy_required(
                input,
                &mut message,
                "generation",
                "generation",
                control.id(),
            )?;
        }
        NarrationClear => {
            reject_control_fields(control, &["tab"])?;
            message.insert("type".into(), Value::String("narration_clear".into()));
            copy_required(input, &mut message, "tab", "tabId", control.id())?;
        }
        NotificationShow => {
            reject_control_fields(
                control,
                &[
                    "tab",
                    "class",
                    "title",
                    "mode",
                    "duration_ms",
                    "icon",
                    "description",
                    "reference",
                ],
            )?;
            message.insert("type".into(), Value::String("notification".into()));
            copy_required(input, &mut message, "tab", "tabId", control.id())?;
            copy_required(input, &mut message, "class", "class", control.id())?;
            copy_required(input, &mut message, "title", "title", control.id())?;
            copy_required(input, &mut message, "mode", "mode", control.id())?;
            copy_required(
                input,
                &mut message,
                "duration_ms",
                "durationMs",
                control.id(),
            )?;
            copy_optional(input, &mut message, "icon", "icon");
            copy_optional(input, &mut message, "description", "description");
            copy_optional(input, &mut message, "reference", "ref");
        }
        AttentionRequired => {
            reject_control_fields(
                control,
                &[
                    "tab",
                    "guid",
                    "label",
                    "category",
                    "origin",
                    "threshold",
                    "count",
                    "title",
                    "description",
                    "controls",
                ],
            )?;
            message.insert("type".into(), Value::String("attention_required".into()));
            for field in [
                ("tab", "tabId"),
                ("guid", "guid"),
                ("label", "label"),
                ("category", "category"),
                ("origin", "origin"),
                ("threshold", "threshold"),
                ("count", "count"),
                ("title", "title"),
                ("description", "description"),
                ("controls", "controls"),
            ] {
                copy_required(input, &mut message, field.0, field.1, control.id())?;
            }
        }
        AttentionResolved => {
            reject_control_fields(control, &["tab", "guid"])?;
            message.insert("type".into(), Value::String("attention_resolved".into()));
            copy_required(input, &mut message, "tab", "tabId", control.id())?;
            copy_required(input, &mut message, "guid", "guid", control.id())?;
        }
    }
    serde_json::to_vec(&Value::Object(message)).map_err(|error| {
        ToolError::binary(format!("failed to encode one-way browser control: {error}"))
    })
}

/// Parse one covered unsolicited adapter event into the closed canonical event vocabulary.
///
/// Unknown message types are not browser events owned by this seam. A known event with a missing
/// or invalid field fails closed and is never allowed to mutate recording state.
pub(super) fn parse_event(message: &Value) -> Result<Option<BrowserEvent>, ToolError> {
    if message.get("id").is_some() {
        return Ok(None);
    }
    let Some(kind) = parse_inbound_kind(message) else {
        return Ok(None);
    };
    let Some(object) = message.as_object() else {
        return Ok(None);
    };

    let event = match kind {
        InboundKind::RecordingFrame => {
            let mut input = Map::new();
            input.insert("tab".into(), Value::from(required_i64(object, "tabId")?));
            input.insert(
                "recording_id".into(),
                Value::String(required_string(object, "recordingId")?.to_owned()),
            );
            input.insert(
                "generation".into(),
                Value::from(required_u64(object, "generation")?),
            );
            input.insert(
                "sequence".into(),
                Value::from(required_u64(object, "sequence")?),
            );
            input.insert(
                "data".into(),
                Value::String(required_string(object, "data")?.to_owned()),
            );
            if let Some(ts) = object.get("ts").and_then(Value::as_i64) {
                input.insert("ts".into(), Value::from(ts));
            }
            if let Some(device_width) = object.get("deviceWidth").and_then(Value::as_f64) {
                input.insert("device_width".into(), Value::from(device_width));
            }
            if object.get("final").and_then(Value::as_bool) == Some(true) {
                input.insert("final_frame".into(), Value::Bool(true));
            }
            BrowserEvent::object(BrowserEventId::RecordingFrame, Value::Object(input))
        }
        InboundKind::RecordingEnded => {
            let reason = required_string(object, "reason")?;
            let reason = RecordingEndReason::parse(reason).ok_or_else(|| {
                ToolError::invalid_request(format!(
                    "unknown browser recording end reason: {reason}"
                ))
            })?;
            BrowserEvent::object(
                BrowserEventId::RecordingEnded,
                Value::Object(Map::from_iter([
                    ("tab".into(), Value::from(required_i64(object, "tabId")?)),
                    (
                        "recording_id".into(),
                        Value::String(required_string(object, "recordingId")?.to_owned()),
                    ),
                    (
                        "generation".into(),
                        Value::from(required_u64(object, "generation")?),
                    ),
                    ("reason".into(), Value::String(reason.as_str().into())),
                ])),
            )
        }
        _ => return Ok(None),
    };
    Ok(Some(event))
}

/// Serialize one typed mechanism to the unchanged covered adapter tool-request envelope.
pub(super) fn serialize_tool_request(
    request_id: &str,
    guid: &str,
    request: &MechanismRequest,
    execution: &Value,
    workspace_group_title: Option<&str>,
) -> Result<Vec<u8>, ToolError> {
    let (tool, args) = legacy_tool(request)?;
    let mut envelope = Map::new();
    envelope.insert("id".into(), Value::String(request_id.to_owned()));
    envelope.insert("type".into(), Value::String("tool_request".into()));
    envelope.insert("tool".into(), Value::String(tool.into()));
    envelope.insert("args".into(), args);
    envelope.insert("guid".into(), Value::String(guid.to_owned()));
    envelope.insert(
        "resultFeatures".into(),
        Value::Array(vec![Value::String(TAB_DELTA_V1.into())]),
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
    serde_json::to_vec(&Value::Object(envelope))
        .map_err(|error| ToolError::binary(format!("failed to encode the tool request: {error}")))
}

/// Serialize the one auxiliary typed mechanism that predates tool-request envelopes.
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
    reject_legacy_input(request)?;
    let tab = request
        .input()
        .get("tab")
        .and_then(Value::as_i64)
        .ok_or_else(|| ToolError::invalid_request("tab.url_query requires a numeric tab"))?;
    let mut envelope = Map::new();
    envelope.insert("id".into(), Value::String(request_id.to_owned()));
    envelope.insert("type".into(), Value::String("tab_url_request".into()));
    envelope.insert("tabId".into(), Value::from(tab));
    envelope.insert("execution".into(), execution.clone());
    serde_json::to_vec(&Value::Object(envelope)).map_err(|error| {
        ToolError::binary(format!("failed to encode the tab url request: {error}"))
    })
}

fn reject_control_fields(control: &BrowserControl, allowed: &[&str]) -> Result<(), ToolError> {
    let input = control.input().as_object().ok_or_else(|| {
        ToolError::invalid_request(format!(
            "browser control {} input must be an object",
            control.id()
        ))
    })?;
    if let Some(field) = input
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(ToolError::invalid_request(format!(
            "browser control {} input uses unknown field {field}",
            control.id()
        )));
    }
    Ok(())
}

fn copy_required(
    input: &Map<String, Value>,
    output: &mut Map<String, Value>,
    canonical: &str,
    legacy: &str,
    id: BrowserControlId,
) -> Result<(), ToolError> {
    let value = input.get(canonical).cloned().ok_or_else(|| {
        ToolError::invalid_request(format!("browser control {id} requires {canonical}"))
    })?;
    output.insert(legacy.to_owned(), value);
    Ok(())
}

fn copy_optional(
    input: &Map<String, Value>,
    output: &mut Map<String, Value>,
    canonical: &str,
    legacy: &str,
) {
    if let Some(value) = input.get(canonical) {
        output.insert(legacy.to_owned(), value.clone());
    }
}

fn required_i64(object: &Map<String, Value>, field: &str) -> Result<i64, ToolError> {
    object.get(field).and_then(Value::as_i64).ok_or_else(|| {
        ToolError::invalid_request(format!("covered browser event requires numeric {field}"))
    })
}

fn required_u64(object: &Map<String, Value>, field: &str) -> Result<u64, ToolError> {
    object.get(field).and_then(Value::as_u64).ok_or_else(|| {
        ToolError::invalid_request(format!(
            "covered browser event requires non-negative integer {field}"
        ))
    })
}

fn required_string<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, ToolError> {
    object.get(field).and_then(Value::as_str).ok_or_else(|| {
        ToolError::invalid_request(format!("covered browser event requires string {field}"))
    })
}

fn legacy_tool(request: &MechanismRequest) -> Result<(&'static str, Value), ToolError> {
    use MechanismId::*;

    reject_legacy_input(request)?;
    let mut args = request
        .input()
        .as_object()
        .cloned()
        .ok_or_else(|| ToolError::invalid_request("mechanism input must be an object"))?;
    rename(&mut args, "tab", "tabId");

    let tool = match request.id() {
        WorkspaceTabsInspect => {
            rename(&mut args, "create_if_empty", "createIfEmpty");
            "tabs_context_mcp"
        }
        WorkspaceTabsEnsure => {
            args.remove("create_if_empty");
            args.insert("createIfEmpty".into(), Value::Bool(true));
            "tabs_context_mcp"
        }
        WorkspaceTabCreate => "tabs_create_mcp",
        TabFocus => action(&mut args, "focus", "tab_control"),
        TabClose => action(&mut args, "close", "tab_control"),
        NavigateUrl => "navigate",
        NavigateBack => field(&mut args, "url", "back", "navigate"),
        NavigateForward => field(&mut args, "url", "forward", "navigate"),
        NavigateReload => action(&mut args, "reload", "tab_control"),
        PageSnapshot => {
            rename(&mut args, "scope_ref", "ref_id");
            "read_page"
        }
        PageReadText => "get_page_text",
        PageFind => "find",
        ScreenshotViewport => action(&mut args, "screenshot", "computer"),
        ScreenshotRegion => action(&mut args, "zoom", "computer"),
        ElementResolve => "resolve_actionable_internal",
        TargetCue => {
            let cue_kind = args
                .remove("cue_kind")
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| ToolError::invalid_request("target.cue requires cue_kind"))?;
            let legacy_action = match cue_kind.as_str() {
                "click" => "left_click",
                "right_click" => "right_click",
                "double_click" => "double_click",
                "triple_click" => "triple_click",
                "hover" => "hover",
                "scroll_into_view" => "scroll_to",
                "set_value" => "set_value",
                _ => {
                    return Err(ToolError::invalid_request(format!(
                        "unknown target.cue kind: {cue_kind}"
                    )))
                }
            };
            let point = args
                .remove("point")
                .and_then(|value| value.as_array().cloned())
                .filter(|point| point.len() == 2)
                .ok_or_else(|| {
                    ToolError::invalid_request("target.cue requires a two-item point")
                })?;
            args.insert("x".into(), point[0].clone());
            args.insert("y".into(), point[1].clone());
            action(&mut args, legacy_action, "target_cue_internal")
        }
        PointerClick => {
            let button = args
                .remove("button")
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| ToolError::invalid_request("pointer.click requires button"))?;
            let count = args
                .remove("count")
                .and_then(|value| value.as_u64())
                .ok_or_else(|| ToolError::invalid_request("pointer.click requires count"))?;
            rename(&mut args, "point", "coordinate");
            flatten_target_reference(&mut args)?;
            let legacy_action = match (button.as_str(), count) {
                ("left", 1) => "left_click",
                ("right", 1) => "right_click",
                ("left", 2) => "double_click",
                ("left", 3) => "triple_click",
                _ => {
                    return Err(ToolError::invalid_request(format!(
                        "unsupported pointer.click button/count pair: {button}/{count}"
                    )))
                }
            };
            action(&mut args, legacy_action, "computer")
        }
        PointerHover => {
            rename(&mut args, "point", "coordinate");
            flatten_target_reference(&mut args)?;
            action(&mut args, "hover", "computer")
        }
        PointerDrag => {
            rename(&mut args, "from", "start_coordinate");
            rename(&mut args, "to", "coordinate");
            action(&mut args, "left_click_drag", "computer")
        }
        TextType => action(&mut args, "type", "computer"),
        KeyPress => {
            rename(&mut args, "key", "text");
            action(&mut args, "key", "computer")
        }
        WheelScroll => {
            rename(&mut args, "point", "coordinate");
            rename(&mut args, "direction", "scroll_direction");
            rename(&mut args, "amount", "scroll_amount");
            flatten_target_reference(&mut args)?;
            action(&mut args, "scroll", "computer")
        }
        ScrollTargetIntoView => {
            flatten_target_reference(&mut args)?;
            action(&mut args, "scroll_to", "computer")
        }
        ScrollViewportToOffset => {
            rename(&mut args, "point", "coordinate");
            action(&mut args, "scroll_to", "computer")
        }
        FormInspect => "form_structure_internal",
        FormSetValue => {
            flatten_target_reference(&mut args)?;
            "form_input"
        }
        WaitDelay => {
            rename(&mut args, "seconds", "duration");
            action(&mut args, "wait", "computer")
        }
        WaitUntil => "wait_for",
        DialogInspect => action(&mut args, "status", "dialog"),
        DialogAccept => action(&mut args, "accept", "dialog"),
        DialogDismiss => action(&mut args, "dismiss", "dialog"),
        DialogRespond => action(&mut args, "respond", "dialog"),
        ViewportResize => "resize_window",
        UploadFiles => {
            flatten_target_reference(&mut args)?;
            rename_file_media_types(&mut args)?;
            "file_upload"
        }
        UploadImage => {
            flatten_target_reference(&mut args)?;
            rename(&mut args, "point", "coordinate");
            rename(&mut args, "mime_type", "mimeType");
            "upload_image_exec"
        }
        ConsoleRead => {
            rename(&mut args, "only_errors", "onlyErrors");
            "read_console_messages"
        }
        NetworkRead => {
            rename(&mut args, "url_pattern", "urlPattern");
            "read_network_requests"
        }
        PageEvaluate => {
            rename(&mut args, "script", "text");
            action(&mut args, "javascript_exec", "javascript_tool")
        }
        RecordingStart => {
            rename(&mut args, "recording_id", "recordingId");
            rename(&mut args, "max_side", "maxSide");
            rename(&mut args, "min_interval_ms", "minIntervalMs");
            rename(&mut args, "lease_ms", "leaseMs");
            rename(&mut args, "hard_timeout_ms", "hardTimeoutMs");
            "gif_capture_start"
        }
        RecordingStop => {
            rename(&mut args, "recording_id", "recordingId");
            "gif_capture_stop"
        }
        PointsRescale => "rescale_coords",
        NarrationShow => "narrate",
        TabUrlQuery => {
            return Err(ToolError::binary(
                "tab.url_query uses the auxiliary browser-adapter wire",
            ))
        }
    };
    Ok((tool, Value::Object(args)))
}

fn reject_legacy_input(request: &MechanismRequest) -> Result<(), ToolError> {
    let Some(input) = request.input().as_object() else {
        return Err(ToolError::invalid_request(format!(
            "mechanism {} input must be an object",
            request.id()
        )));
    };
    if let Some(field) = LEGACY_TOP_LEVEL_FIELDS
        .iter()
        .find(|field| input.contains_key(**field))
    {
        return Err(ToolError::invalid_request(format!(
            "mechanism {} input uses legacy field {field}",
            request.id()
        )));
    }
    if input
        .get("files")
        .and_then(Value::as_array)
        .is_some_and(|files| files.iter().any(|file| file.get("mimeType").is_some()))
    {
        return Err(ToolError::invalid_request(format!(
            "mechanism {} input uses legacy nested field mimeType",
            request.id()
        )));
    }
    Ok(())
}

fn rename(args: &mut Map<String, Value>, canonical: &str, legacy: &str) {
    if let Some(value) = args.remove(canonical) {
        args.insert(legacy.to_owned(), value);
    }
}

fn action(args: &mut Map<String, Value>, name: &'static str, tool: &'static str) -> &'static str {
    args.insert("action".into(), Value::String(name.into()));
    tool
}

fn field(
    args: &mut Map<String, Value>,
    key: &'static str,
    value: &'static str,
    tool: &'static str,
) -> &'static str {
    args.insert(key.into(), Value::String(value.into()));
    tool
}

fn flatten_target_reference(args: &mut Map<String, Value>) -> Result<(), ToolError> {
    let Some(target) = args.remove("target") else {
        return Ok(());
    };
    let reference = target
        .get("ref")
        .cloned()
        .ok_or_else(|| ToolError::invalid_request("physical target requires target.ref"))?;
    args.insert("ref".into(), reference);
    Ok(())
}

fn rename_file_media_types(args: &mut Map<String, Value>) -> Result<(), ToolError> {
    let Some(files) = args.get_mut("files") else {
        return Ok(());
    };
    let files = files
        .as_array_mut()
        .ok_or_else(|| ToolError::invalid_request("upload.files files must be an array"))?;
    for file in files {
        let file = file
            .as_object_mut()
            .ok_or_else(|| ToolError::invalid_request("upload.files entries must be objects"))?;
        rename(file, "mime_type", "mimeType");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(id: MechanismId, input: Value) -> MechanismRequest {
        MechanismRequest::test_only(id, input)
    }

    fn control(id: BrowserControlId, input: Value) -> BrowserControl {
        BrowserControl::test_only(id, input)
    }

    #[test]
    fn every_one_way_control_has_one_byte_exact_legacy_alias() {
        let cases = [
            (
                BrowserControlId::RecordingLeaseRenew,
                json!({
                    "tab": 7,
                    "recording_id": "rec-1",
                    "generation": 2,
                    "lease_ms": 15000
                }),
                r#"{"type":"gif_lease_renew","tabId":7,"recordingId":"rec-1","generation":2,"leaseMs":15000}"#,
            ),
            (
                BrowserControlId::RecordingCancel,
                json!({"tab":7,"recording_id":"rec-1","generation":2}),
                r#"{"type":"gif_capture_cancel","tabId":7,"recordingId":"rec-1","generation":2}"#,
            ),
            (
                BrowserControlId::NarrationClear,
                json!({"tab":7}),
                r#"{"type":"narration_clear","tabId":7}"#,
            ),
            (
                BrowserControlId::NotificationShow,
                json!({
                    "tab":7,
                    "class":"warn",
                    "title":"Blocked",
                    "mode":"sticker",
                    "duration_ms":3000,
                    "icon":"shield",
                    "description":"Denied",
                    "reference":"denial-1"
                }),
                r#"{"type":"notification","tabId":7,"class":"warn","title":"Blocked","mode":"sticker","durationMs":3000,"icon":"shield","description":"Denied","ref":"denial-1"}"#,
            ),
            (
                BrowserControlId::AttentionRequired,
                json!({
                    "tab":7,
                    "guid":"workspace-1",
                    "label":"MCP client",
                    "category":"policy",
                    "origin":null,
                    "threshold":"repeated",
                    "count":3,
                    "title":"Agent browsing paused",
                    "description":"Repeated blocked actions need your attention before this client can continue.",
                    "controls":["keep_paused","resume","resume_quiet","end_session"]
                }),
                r#"{"type":"attention_required","tabId":7,"guid":"workspace-1","label":"MCP client","category":"policy","origin":null,"threshold":"repeated","count":3,"title":"Agent browsing paused","description":"Repeated blocked actions need your attention before this client can continue.","controls":["keep_paused","resume","resume_quiet","end_session"]}"#,
            ),
            (
                BrowserControlId::AttentionResolved,
                json!({"tab":7,"guid":"workspace-1"}),
                r#"{"type":"attention_resolved","tabId":7,"guid":"workspace-1"}"#,
            ),
        ];
        assert_eq!(cases.len(), BrowserControlId::ALL.len());
        for ((id, input, expected), declared) in cases.into_iter().zip(BrowserControlId::ALL) {
            assert_eq!(&id, declared);
            assert_eq!(
                String::from_utf8(serialize_control(&control(id, input)).unwrap()).unwrap(),
                expected,
                "{id}"
            );
        }
    }

    #[test]
    fn one_way_controls_reject_unknown_or_legacy_fields() {
        for control in [
            control(BrowserControlId::NarrationClear, json!({"tabId":7})),
            control(
                BrowserControlId::RecordingCancel,
                json!({"tab":7,"recording_id":"rec-1","generation":2,"extra":true}),
            ),
        ] {
            assert!(serialize_control(&control).is_err());
        }
    }

    #[test]
    fn covered_recording_events_parse_to_canonical_fields() {
        let frame = parse_event(&json!({
            "type":"gif_frame",
            "tabId":7,
            "recordingId":"rec-1",
            "generation":2,
            "sequence":3,
            "data":"YQ==",
            "ts":4,
            "deviceWidth":1280.5,
            "final":true
        }))
        .unwrap()
        .unwrap();
        assert_eq!(frame.id, BrowserEventId::RecordingFrame);
        assert_eq!(
            frame.input,
            json!({
                "tab":7,
                "recording_id":"rec-1",
                "generation":2,
                "sequence":3,
                "data":"YQ==",
                "ts":4,
                "device_width":1280.5,
                "final_frame":true
            })
        );

        for (wire, expected) in [
            ("hard_timeout", RecordingEndReason::HardTimeout),
            ("browser_detached", RecordingEndReason::BrowserDetached),
            ("lease_expired", RecordingEndReason::LeaseExpired),
        ] {
            let ended = parse_event(&json!({
                "type":"gif_capture_ended",
                "tabId":7,
                "recordingId":"rec-1",
                "generation":2,
                "reason":wire
            }))
            .unwrap()
            .unwrap();
            assert_eq!(ended.id, BrowserEventId::RecordingEnded);
            assert_eq!(ended.input["reason"], expected.as_str());
            assert_eq!(ended.input["tab"], 7);
        }
    }

    #[test]
    fn recording_event_aliases_are_exact_and_unknowns_fail_closed() {
        assert_eq!(parse_event(&json!({"type":"invented"})).unwrap(), None);
        assert_eq!(
            parse_event(&json!({"id":"1","type":"gif_frame"})).unwrap(),
            None
        );
        assert!(parse_event(&json!({
            "type":"gif_frame",
            "tabId":7,
            "recordingId":"rec-1",
            "generation":2,
            "sequence":"bad",
            "data":"YQ=="
        }))
        .is_err());
        assert!(parse_event(&json!({
            "type":"gif_capture_ended",
            "tabId":7,
            "recordingId":"rec-1",
            "generation":2,
            "reason":"invented"
        }))
        .is_err());
    }

    #[test]
    fn inbound_aliases_are_exhaustive_exact_and_unknowns_fail_closed() {
        for (wire, expected) in [
            ("tool_accepted", InboundKind::ToolAccepted),
            ("tool_terminal", InboundKind::ToolTerminal),
            ("surface_destroyed", InboundKind::SurfaceDestroyed),
            ("session_killed", InboundKind::SessionKilled),
            ("focus", InboundKind::FocusChanged),
            ("debug_event", InboundKind::DebugEvent),
            ("gif_frame", InboundKind::RecordingFrame),
            ("gif_capture_ended", InboundKind::RecordingEnded),
            ("get_hold", InboundKind::HoldGet),
            ("set_hold", InboundKind::HoldSet),
            ("toggle_hold", InboundKind::HoldToggle),
            ("get_attention", InboundKind::AttentionGet),
            ("attention_action", InboundKind::AttentionAction),
            ("tool_response", InboundKind::ToolResponse),
            ("tool_error", InboundKind::ToolError),
            ("tab_url_response", InboundKind::TabUrlResponse),
        ] {
            assert_eq!(parse_inbound_kind(&json!({"type":wire})), Some(expected));
        }
        assert_eq!(parse_inbound_kind(&json!({})), None);
        assert_eq!(
            parse_inbound_kind(&json!({"type":"invented_response"})),
            None
        );

        assert_eq!(
            correlated_reply_kind(InboundKind::ToolResponse),
            Some(CorrelatedReplyKind::ToolResponse)
        );
        assert_eq!(
            correlated_reply_kind(InboundKind::ToolError),
            Some(CorrelatedReplyKind::ToolError)
        );
        assert_eq!(
            correlated_reply_kind(InboundKind::TabUrlResponse),
            Some(CorrelatedReplyKind::TabUrlResponse)
        );
        assert_eq!(correlated_reply_kind(InboundKind::FocusChanged), None);
    }

    #[test]
    fn service_control_replies_are_byte_exact() {
        let attention = json!({"sessions":[],"endSession":false});
        for (reply, expected) in [
            (
                ServiceReply::HoldState {
                    id: "1",
                    held: true,
                },
                r#"{"id":"1","type":"hold_state","result":{"held":true}}"#,
            ),
            (
                ServiceReply::HoldError {
                    id: "2",
                    error: "bad hold",
                },
                r#"{"id":"2","type":"hold_error","error":"bad hold"}"#,
            ),
            (
                ServiceReply::AttentionState {
                    id: "3",
                    result: &attention,
                },
                r#"{"id":"3","type":"attention_state","result":{"sessions":[],"endSession":false}}"#,
            ),
            (
                ServiceReply::AttentionError {
                    id: "4",
                    error: "bad attention",
                },
                r#"{"id":"4","type":"attention_error","error":"bad attention"}"#,
            ),
        ] {
            assert_eq!(
                String::from_utf8(serialize_service_reply(&reply).unwrap()).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn tool_envelope_is_byte_exact_with_optional_workspace_last() {
        let bytes = serialize_tool_request(
            "7",
            "workspace-1",
            &request(
                MechanismId::NavigateUrl,
                json!({"url":"https://example.com","tab":9}),
            ),
            &json!({"class":"scheduled"}),
            Some("Ghostlight - Example"),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"id":"7","type":"tool_request","tool":"navigate","args":{"url":"https://example.com","tabId":9},"guid":"workspace-1","resultFeatures":["tabDeltaV1"],"execution":{"class":"scheduled"},"workspace":{"groupTitle":"Ghostlight - Example"}}"#
        );
    }

    #[test]
    fn every_typed_mechanism_has_an_exhaustive_legacy_serialization_decision() {
        for id in MechanismId::ALL {
            let input = match id {
                MechanismId::PointerClick => json!({"button":"left","count":1}),
                MechanismId::TargetCue => json!({"cue_kind":"click","point":[1,2]}),
                MechanismId::UploadImage => json!({
                    "tab": 1,
                    "target": {"ref":"ref_1"},
                    "data": "YQ==",
                    "filename": "a.png",
                    "mime_type": "image/png"
                }),
                MechanismId::RecordingStart => json!({
                    "tab": 1,
                    "recording_id": "recording-1",
                    "generation": 1,
                    "quality": 70,
                    "max_side": 1568,
                    "min_interval_ms": 200,
                    "lease_ms": 15000,
                    "hard_timeout_ms": 120000
                }),
                MechanismId::RecordingStop => {
                    json!({"tab":1,"recording_id":"recording-1","generation":1})
                }
                MechanismId::TabUrlQuery => json!({"tab":4}),
                _ => json!({}),
            };
            let request = request(*id, input);
            if *id == MechanismId::TabUrlQuery {
                let bytes = serialize_tab_url_request("1", &request, &json!({})).unwrap();
                assert_eq!(
                    String::from_utf8(bytes).unwrap(),
                    r#"{"id":"1","type":"tab_url_request","tabId":4,"execution":{}}"#
                );
            } else {
                let expected_tool = match id {
                    MechanismId::WorkspaceTabsInspect | MechanismId::WorkspaceTabsEnsure => {
                        "tabs_context_mcp"
                    }
                    MechanismId::WorkspaceTabCreate => "tabs_create_mcp",
                    MechanismId::TabFocus | MechanismId::TabClose | MechanismId::NavigateReload => {
                        "tab_control"
                    }
                    MechanismId::NavigateUrl
                    | MechanismId::NavigateBack
                    | MechanismId::NavigateForward => "navigate",
                    MechanismId::PageSnapshot => "read_page",
                    MechanismId::PageReadText => "get_page_text",
                    MechanismId::PageFind => "find",
                    MechanismId::ScreenshotViewport
                    | MechanismId::ScreenshotRegion
                    | MechanismId::PointerClick
                    | MechanismId::PointerHover
                    | MechanismId::PointerDrag
                    | MechanismId::TextType
                    | MechanismId::KeyPress
                    | MechanismId::WheelScroll
                    | MechanismId::ScrollTargetIntoView
                    | MechanismId::ScrollViewportToOffset
                    | MechanismId::WaitDelay => "computer",
                    MechanismId::ElementResolve => "resolve_actionable_internal",
                    MechanismId::TargetCue => "target_cue_internal",
                    MechanismId::FormInspect => "form_structure_internal",
                    MechanismId::FormSetValue => "form_input",
                    MechanismId::WaitUntil => "wait_for",
                    MechanismId::DialogInspect
                    | MechanismId::DialogAccept
                    | MechanismId::DialogDismiss
                    | MechanismId::DialogRespond => "dialog",
                    MechanismId::ViewportResize => "resize_window",
                    MechanismId::UploadFiles => "file_upload",
                    MechanismId::UploadImage => "upload_image_exec",
                    MechanismId::ConsoleRead => "read_console_messages",
                    MechanismId::NetworkRead => "read_network_requests",
                    MechanismId::PageEvaluate => "javascript_tool",
                    MechanismId::RecordingStart => "gif_capture_start",
                    MechanismId::RecordingStop => "gif_capture_stop",
                    MechanismId::PointsRescale => "rescale_coords",
                    MechanismId::NarrationShow => "narrate",
                    MechanismId::TabUrlQuery => unreachable!(),
                };
                assert_eq!(legacy_tool(&request).unwrap().0, expected_tool, "{id}");
                serialize_tool_request("1", "w", &request, &json!({}), None)
                    .unwrap_or_else(|error| panic!("{id}: {error}"));
            }
        }
    }

    #[test]
    fn pointer_metadata_selects_the_covered_action_without_an_input_alias() {
        for (button, count, expected) in [
            ("left", 1, "left_click"),
            ("right", 1, "right_click"),
            ("left", 2, "double_click"),
            ("left", 3, "triple_click"),
        ] {
            let request = request(
                MechanismId::PointerClick,
                json!({"tab":3,"point":[10,20],"button":button,"count":count}),
            );
            let (_, args) = legacy_tool(&request).unwrap();
            assert_eq!(args["action"], expected);
            assert_eq!(args["tabId"], 3);
            assert_eq!(args["coordinate"], json!([10, 20]));
            assert!(args.get("button").is_none());
            assert!(args.get("count").is_none());
        }
    }

    #[test]
    fn every_action_bearing_mechanism_emits_its_exact_covered_action() {
        for (id, input, expected) in [
            (MechanismId::TabFocus, json!({}), "focus"),
            (MechanismId::TabClose, json!({}), "close"),
            (MechanismId::NavigateReload, json!({}), "reload"),
            (MechanismId::ScreenshotViewport, json!({}), "screenshot"),
            (MechanismId::ScreenshotRegion, json!({}), "zoom"),
            (MechanismId::PointerHover, json!({}), "hover"),
            (MechanismId::PointerDrag, json!({}), "left_click_drag"),
            (MechanismId::TextType, json!({}), "type"),
            (MechanismId::KeyPress, json!({}), "key"),
            (MechanismId::WheelScroll, json!({}), "scroll"),
            (MechanismId::ScrollTargetIntoView, json!({}), "scroll_to"),
            (MechanismId::ScrollViewportToOffset, json!({}), "scroll_to"),
            (MechanismId::WaitDelay, json!({}), "wait"),
            (MechanismId::DialogInspect, json!({}), "status"),
            (MechanismId::DialogAccept, json!({}), "accept"),
            (MechanismId::DialogDismiss, json!({}), "dismiss"),
            (MechanismId::DialogRespond, json!({}), "respond"),
            (MechanismId::PageEvaluate, json!({}), "javascript_exec"),
        ] {
            let (_, args) = legacy_tool(&request(id, input)).unwrap();
            assert_eq!(args["action"], expected, "{id}");
        }
    }

    #[test]
    fn renamed_argument_families_have_exact_representative_shapes() {
        for (id, input, expected) in [
            (
                MechanismId::WorkspaceTabsEnsure,
                json!({}),
                json!({"createIfEmpty":true}),
            ),
            (
                MechanismId::NavigateBack,
                json!({"tab":1}),
                json!({"tabId":1,"url":"back"}),
            ),
            (
                MechanismId::PageSnapshot,
                json!({"tab":1,"scope_ref":"ref_1"}),
                json!({"tabId":1,"ref_id":"ref_1"}),
            ),
            (
                MechanismId::PointerDrag,
                json!({"tab":1,"from":[1,2],"to":[3,4]}),
                json!({
                    "tabId":1,
                    "start_coordinate":[1,2],
                    "coordinate":[3,4],
                    "action":"left_click_drag"
                }),
            ),
            (
                MechanismId::KeyPress,
                json!({"tab":1,"key":"Enter","repeat":2}),
                json!({"repeat":2,"tabId":1,"text":"Enter","action":"key"}),
            ),
            (
                MechanismId::WheelScroll,
                json!({
                    "tab":1,
                    "point":[5,6],
                    "direction":"down",
                    "amount":3
                }),
                json!({
                    "tabId":1,
                    "coordinate":[5,6],
                    "scroll_direction":"down",
                    "scroll_amount":3,
                    "action":"scroll"
                }),
            ),
            (
                MechanismId::WheelScroll,
                json!({"tab":1,"target":{"ref":"ref_2"},"direction":"up","amount":1}),
                json!({
                    "tabId":1,
                    "scroll_direction":"up",
                    "scroll_amount":1,
                    "ref":"ref_2",
                    "action":"scroll"
                }),
            ),
            (
                MechanismId::FormSetValue,
                json!({"tab":1,"target":{"ref":"ref_3"},"value":"x"}),
                json!({"value":"x","tabId":1,"ref":"ref_3"}),
            ),
            (
                MechanismId::WaitDelay,
                json!({"tab":1,"seconds":2}),
                json!({"tabId":1,"duration":2,"action":"wait"}),
            ),
            (
                MechanismId::UploadImage,
                json!({
                    "tab":1,
                    "point":[7,8],
                    "mime_type":"image/png"
                }),
                json!({
                    "tabId":1,
                    "coordinate":[7,8],
                    "mimeType":"image/png"
                }),
            ),
            (
                MechanismId::ConsoleRead,
                json!({"tab":1,"only_errors":true,"clear":true}),
                json!({"clear":true,"tabId":1,"onlyErrors":true}),
            ),
            (
                MechanismId::NetworkRead,
                json!({"tab":1,"url_pattern":"api","clear":true}),
                json!({"clear":true,"tabId":1,"urlPattern":"api"}),
            ),
            (
                MechanismId::PageEvaluate,
                json!({"tab":1,"script":"return 1"}),
                json!({"tabId":1,"text":"return 1","action":"javascript_exec"}),
            ),
            (
                MechanismId::RecordingStart,
                json!({
                    "tab":1,
                    "recording_id":"rec_1",
                    "generation":2,
                    "max_side":1568,
                    "min_interval_ms":200,
                    "lease_ms":15000,
                    "hard_timeout_ms":120000
                }),
                json!({
                    "generation":2,
                    "tabId":1,
                    "recordingId":"rec_1",
                    "maxSide":1568,
                    "minIntervalMs":200,
                    "leaseMs":15000,
                    "hardTimeoutMs":120000
                }),
            ),
            (
                MechanismId::RecordingStop,
                json!({"tab":1,"recording_id":"rec_1","generation":2}),
                json!({"generation":2,"tabId":1,"recordingId":"rec_1"}),
            ),
        ] {
            assert_eq!(
                legacy_tool(&request(id, input)).unwrap().1,
                expected,
                "{id}"
            );
        }
    }

    #[test]
    fn presence_sensitive_fields_preserve_the_exact_covered_wire() {
        for (id, input, expected) in [
            (MechanismId::WorkspaceTabsInspect, json!({}), json!({})),
            (
                MechanismId::WorkspaceTabsInspect,
                json!({"create_if_empty":false}),
                json!({"createIfEmpty":false}),
            ),
            (
                MechanismId::WorkspaceTabsEnsure,
                json!({}),
                json!({"createIfEmpty":true}),
            ),
            (MechanismId::ConsoleRead, json!({}), json!({})),
            (
                MechanismId::ConsoleRead,
                json!({"clear":true}),
                json!({"clear":true}),
            ),
            (MechanismId::NetworkRead, json!({}), json!({})),
            (
                MechanismId::NetworkRead,
                json!({"clear":true}),
                json!({"clear":true}),
            ),
        ] {
            assert_eq!(
                legacy_tool(&request(id, input)).unwrap().1,
                expected,
                "{id}"
            );
        }
    }

    #[test]
    fn target_cue_maps_typed_kind_and_point_and_rejects_unknown_kinds() {
        let (_, args) = legacy_tool(&request(
            MechanismId::TargetCue,
            json!({"tab":7,"cue_kind":"scroll_into_view","point":[12,34]}),
        ))
        .unwrap();
        assert_eq!(args, json!({"tabId":7,"x":12,"y":34,"action":"scroll_to"}));
        assert!(legacy_tool(&request(
            MechanismId::TargetCue,
            json!({"cue_kind":"invented","point":[1,2]}),
        ))
        .is_err());
    }

    #[test]
    fn native_frame_is_the_exact_little_endian_prefix_plus_payload() {
        let payload = serialize_tool_request(
            "3",
            "workspace-1",
            &request(MechanismId::PageFind, json!({"tab":5,"query":"Save"})),
            &json!({"class":"scheduled"}),
            None,
        )
        .unwrap();
        let frame = ghostlight_transport::host::encode(&payload).unwrap();
        assert_eq!(
            &frame[..4],
            &(u32::try_from(payload.len()).unwrap()).to_le_bytes()
        );
        assert_eq!(&frame[4..], payload.as_slice());
    }

    #[test]
    fn canonical_fields_translate_only_at_the_compatibility_boundary() {
        let request = request(
            MechanismId::UploadFiles,
            json!({
                "tab": 8,
                "target": {"ref":"ref_2"},
                "files": [{"name":"a.txt","mime_type":"text/plain","data":"YQ=="}]
            }),
        );
        let (_, args) = legacy_tool(&request).unwrap();
        assert_eq!(args["tabId"], 8);
        assert_eq!(args["ref"], "ref_2");
        assert_eq!(args["files"][0]["mimeType"], "text/plain");
        assert!(args.get("target").is_none());
    }

    #[test]
    fn legacy_input_spellings_fail_closed() {
        for (id, input) in [
            (
                MechanismId::NavigateUrl,
                json!({"tabId":1,"url":"https://example.com"}),
            ),
            (
                MechanismId::PointerHover,
                json!({"action":"hover","point":[1,2]}),
            ),
            (MechanismId::PointerHover, json!({"coordinate":[1,2]})),
            (
                MechanismId::FormSetValue,
                json!({"ref":"ref_1","value":"x"}),
            ),
            (MechanismId::UploadImage, json!({"mimeType":"image/png"})),
            (
                MechanismId::UploadFiles,
                json!({"files":[{"name":"a","mimeType":"text/plain","data":"YQ=="}]}),
            ),
        ] {
            assert!(legacy_tool(&request(id, input)).is_err());
        }
    }
}
