// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Frozen `ghostlight-legacy/v1` declarations and canonical translation.

use super::schema;
use crate::bridge::{FlowRenderHints, FlowStepRenderHint};
use ghostlight_transport::bridge::{CatalogProjection, WorkspaceUse};
#[cfg(test)]
use ghostlight_transport::operation::FlowTermination;
use ghostlight_transport::operation::{
    BrowserOperation, BrowserResult, BrowserResultStatus, BrowserResultValidationError,
    FlowResultData, FlowStepResult, FlowStepStatus, FlowTerminationReason, IntentId,
    InvocationPresentation, OperationEffect, OperationId, OperationKey, PageProvenance, ResultPart,
    ResultPartError, MAX_PAGE_ORIGIN_BYTES,
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

pub(crate) const PROFILE_ID: &str = "ghostlight-legacy";
pub(crate) const PROFILE_VERSION: u32 = 1;

const DECLARATIONS_JSON: &str = include_str!("data/ghostlight-legacy-v1.json");
const AGENT_GUIDE: &str = include_str!("data/ghostlight-legacy-v1-agent-guide.txt");

/// One profile declaration paired with its protocol-neutral workspace behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedTool {
    pub(crate) declaration: Value,
    pub(crate) workspace_use: WorkspaceUse,
}

/// A legacy call could not be normalized into one canonical operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodeError {
    UnknownTool(String),
    ArgumentsNotObject { tool: String },
    SchemaViolation { tool: String, message: String },
    MissingAction { tool: &'static str },
    UnknownAction { tool: &'static str, action: String },
    InvalidShape(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTool(tool) => write!(formatter, "Unknown tool: {tool}"),
            Self::ArgumentsNotObject { tool } => {
                write!(formatter, "{tool} arguments must be an object")
            }
            Self::SchemaViolation { tool, message } => {
                write!(formatter, "invalid {tool} arguments: {message}")
            }
            Self::MissingAction { tool } => write!(formatter, "{tool} requires a string action"),
            Self::UnknownAction { tool, action } => {
                write!(formatter, "Unknown {tool} action: {action}")
            }
            Self::InvalidShape(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for DecodeError {}

/// A canonical result could not be rendered by this frozen profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EncodeError {
    WrongProfile,
    UnsupportedStatus(BrowserResultStatus),
    MalformedProvenance(&'static str),
    MissingFlowPresentation,
    UnsupportedFlowSurface(String),
    MissingFlowRenderHints,
    FlowRenderHintCount { expected: usize, actual: usize },
    FlowStepIdentityMismatch { step: u32 },
    MalformedFlowData(&'static str),
    InvalidResultPart(ResultPartError),
    InvalidResultDisposition(BrowserResultValidationError),
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongProfile => {
                formatter.write_str("result presentation does not belong to ghostlight-legacy/v1")
            }
            Self::UnsupportedStatus(status) => write!(
                formatter,
                "ghostlight-legacy/v1 cannot faithfully encode canonical status {}",
                status.as_str()
            ),
            Self::MalformedProvenance(reason) => write!(
                formatter,
                "ghostlight-legacy/v1 cannot faithfully encode canonical provenance: {reason}"
            ),
            Self::MissingFlowPresentation => formatter.write_str(
                "ghostlight-legacy/v1 flow rendering requires its root invocation presentation",
            ),
            Self::UnsupportedFlowSurface(tool) => write!(
                formatter,
                "ghostlight-legacy/v1 cannot render a browser flow as {tool}"
            ),
            Self::MissingFlowRenderHints => formatter.write_str(
                "ghostlight-legacy/v1 flow rendering requires edge-local step labels",
            ),
            Self::FlowRenderHintCount { expected, actual } => write!(
                formatter,
                "ghostlight-legacy/v1 flow has {expected} canonical steps but {actual} edge labels"
            ),
            Self::FlowStepIdentityMismatch { step } => write!(
                formatter,
                "ghostlight-legacy/v1 flow step {step} does not match its decoded canonical identity"
            ),
            Self::MalformedFlowData(reason) => write!(
                formatter,
                "ghostlight-legacy/v1 cannot faithfully encode canonical flow data: {reason}"
            ),
            Self::InvalidResultPart(error) => write!(
                formatter,
                "ghostlight-legacy/v1 cannot encode an invalid canonical result part: {error}"
            ),
            Self::InvalidResultDisposition(error) => write!(
                formatter,
                "ghostlight-legacy/v1 cannot encode an invalid canonical result: {error}"
            ),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Return the frozen ordered declaration object.
pub(crate) fn declarations() -> &'static Value {
    static VALUE: OnceLock<Value> = OnceLock::new();
    VALUE.get_or_init(|| {
        serde_json::from_str(DECLARATIONS_JSON)
            .expect("the embedded ghostlight-legacy/v1 catalog must remain valid JSON")
    })
}

fn declaration(external_tool: &str) -> Option<&'static Value> {
    declarations()["tools"].as_array().and_then(|tools| {
        tools
            .iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some(external_tool))
    })
}

fn declares_argument(external_tool: &str, field: &str) -> bool {
    declaration(external_tool)
        .and_then(|tool| tool.pointer("/inputSchema/properties"))
        .and_then(Value::as_object)
        .is_some_and(|properties| properties.contains_key(field))
}

/// Return the frozen legacy onboarding text.
pub(crate) const fn agent_guide() -> &'static str {
    AGENT_GUIDE
}

/// Build bounded presentation facts for one legacy invocation.
pub(crate) fn invocation_presentation(
    external_tool: &str,
    arguments: &Value,
) -> Result<InvocationPresentation, DecodeError> {
    if tool_keys(external_tool).is_none() {
        return Err(DecodeError::UnknownTool(external_tool.to_owned()));
    }
    let external_action = arguments
        .as_object()
        .and_then(|object| object.get("action"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    InvocationPresentation::new(PROFILE_ID, PROFILE_VERSION, external_tool, external_action)
        .map_err(|error| DecodeError::InvalidShape(error.to_string()))
}

/// Capture original nested flow labels in edge-local request correlation state.
///
/// These labels are presentation hints only. [`decode_call`] independently removes them while
/// producing canonical [`BrowserOperation`] steps for the service bridge.
pub(crate) fn flow_render_hints(
    external_tool: &str,
    arguments: &Value,
    operation: &BrowserOperation,
) -> Option<FlowRenderHints> {
    let (array_field, label_field) = match external_tool {
        "script" => ("steps", "tool"),
        "browser_batch" => ("actions", "name"),
        _ => return None,
    };
    let labels = arguments
        .get(array_field)?
        .as_array()?
        .iter()
        .map(|step| step.get(label_field)?.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    if operation.id != OperationId::BrowserFlow {
        return None;
    }
    let operations = operation
        .arguments
        .get("steps")?
        .as_array()?
        .iter()
        .cloned()
        .map(serde_json::from_value::<BrowserOperation>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if labels.len() != operations.len() {
        return None;
    }
    Some(FlowRenderHints {
        steps: labels
            .into_iter()
            .zip(operations)
            .map(|(label, operation)| FlowStepRenderHint {
                label,
                expected_operation: operation.key(),
            })
            .collect(),
    })
}

/// Filter the frozen declaration order through service-owned operation availability.
pub(crate) fn filtered_declarations(projection: &CatalogProjection) -> Vec<RenderedTool> {
    let available: HashMap<OperationKey, WorkspaceUse> = projection
        .operations
        .iter()
        .map(|operation| {
            (
                OperationKey::new(operation.id, operation.intent),
                operation.workspace_use,
            )
        })
        .collect();
    declarations()
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|declaration| {
            let name = declaration.get("name")?.as_str()?;
            let uses: Vec<WorkspaceUse> = tool_keys(name)?
                .into_iter()
                .filter_map(|key| available.get(&key).copied())
                .collect();
            let workspace_use = *uses.first()?;
            debug_assert!(uses.iter().all(|candidate| *candidate == workspace_use));
            Some(RenderedTool {
                declaration: declaration.clone(),
                workspace_use,
            })
        })
        .collect()
}

/// Convert a canonical result back into the frozen MCP tool-result shape.
pub(crate) fn encode_result(
    result: BrowserResult,
    presentation: Option<&InvocationPresentation>,
    flow_hints: Option<&FlowRenderHints>,
) -> Result<Value, EncodeError> {
    if presentation.is_some_and(|presentation| {
        presentation.profile_id() != PROFILE_ID || presentation.profile_version() != PROFILE_VERSION
    }) {
        return Err(EncodeError::WrongProfile);
    }
    for part in &result.parts {
        part.validate().map_err(EncodeError::InvalidResultPart)?;
    }
    result
        .validate_semantics()
        .map_err(EncodeError::InvalidResultDisposition)?;
    if result.operation == OperationId::BrowserFlow {
        return encode_flow_result(result, presentation, flow_hints);
    }
    let is_error = match result.status {
        BrowserResultStatus::Ok => false,
        BrowserResultStatus::Partial | BrowserResultStatus::Blocked => true,
        status => return Err(EncodeError::UnsupportedStatus(status)),
    };

    let mut data = result.data;
    if let Some(provenance) = result.provenance.as_ref() {
        reinsert_legacy_provenance(&mut data, provenance)?;
    }

    let content = result
        .parts
        .into_iter()
        .map(|part| match part {
            ResultPart::Text { text } => json!({"type": "text", "text": text}),
            ResultPart::Image { data, mime_type } => {
                json!({"type": "image", "data": data, "mimeType": mime_type})
            }
        })
        .collect::<Vec<_>>();
    let mut rendered = Map::new();
    rendered.insert("content".into(), Value::Array(content));
    if !data.is_null() {
        rendered.insert("structuredContent".into(), data);
    }
    if is_error {
        rendered.insert("isError".into(), Value::Bool(true));
    }
    Ok(Value::Object(rendered))
}

const STEP_TEXT_BUDGET: usize = 2_000;
const COMPACT_BUDGET: usize = 25_000;
const COMPACT_BATCH_ID_HEADROOM: usize = 64;

fn encode_flow_result(
    result: BrowserResult,
    presentation: Option<&InvocationPresentation>,
    hints: Option<&FlowRenderHints>,
) -> Result<Value, EncodeError> {
    if result.status == BrowserResultStatus::OutcomeUnknown
        || result.effect == OperationEffect::Unknown
    {
        return Err(EncodeError::UnsupportedStatus(result.status));
    }
    let presentation = presentation.ok_or(EncodeError::MissingFlowPresentation)?;
    let hints = hints.ok_or(EncodeError::MissingFlowRenderHints)?;
    let data = serde_json::from_value::<FlowResultData>(result.data)
        .map_err(|_| EncodeError::MalformedFlowData("data is not a typed flow result"))?;
    if data.steps.len() != hints.steps.len() {
        return Err(EncodeError::FlowRenderHintCount {
            expected: data.steps.len(),
            actual: hints.steps.len(),
        });
    }
    if data
        .steps
        .iter()
        .enumerate()
        .any(|(index, step)| step.step != (index + 1) as u32)
    {
        return Err(EncodeError::MalformedFlowData(
            "step positions are not contiguous and one-based",
        ));
    }
    if let Some((index, _)) =
        data.steps
            .iter()
            .zip(&hints.steps)
            .enumerate()
            .find(|(_, (step, hint))| {
                OperationKey::new(step.result.operation, step.result.intent)
                    != hint.expected_operation
            })
    {
        return Err(EncodeError::FlowStepIdentityMismatch {
            step: (index + 1) as u32,
        });
    }
    if let Some(error) = data
        .steps
        .iter()
        .find_map(|step| step.result.validate_semantics().err())
    {
        return Err(EncodeError::InvalidResultDisposition(error));
    }

    match presentation.external_tool() {
        "script" => encode_script_flow(data, hints),
        "browser_batch" if result.intent == IntentId::FlowExecute => {
            Ok(encode_batch_flow(data, hints))
        }
        "browser_batch" => Err(EncodeError::MalformedFlowData(
            "browser_batch cannot render a flow preflight",
        )),
        other => Err(EncodeError::UnsupportedFlowSurface(other.to_owned())),
    }
}

fn encode_script_flow(data: FlowResultData, hints: &FlowRenderHints) -> Result<Value, EncodeError> {
    let mut results = Vec::with_capacity(data.steps.len());
    let executed_cancel = (data.termination.reason == FlowTerminationReason::Cancelled)
        .then(|| {
            data.steps
                .iter()
                .find(|step| step.status == FlowStepStatus::Cancelled)
                .map(|step| step.step)
        })
        .flatten();
    for (step, hint) in data.steps.into_iter().zip(&hints.steps) {
        if executed_cancel
            .is_some_and(|cancelled| step.step > cancelled && step.status == FlowStepStatus::NotRun)
        {
            continue;
        }
        let mut entry = json!({
            "step": step.step,
            "tool": hint.label,
            "status": legacy_flow_status(step.status),
        });
        if let Some(text) = first_step_text(&step) {
            entry["result"] = Value::String(truncate_step_text(text));
        }
        let mut structured = step.result.data;
        if let Some(provenance) = step.result.provenance.as_ref() {
            reinsert_legacy_provenance(&mut structured, provenance)?;
        }
        if !structured.is_null() {
            entry["structured"] = structured;
        }
        results.push(entry);
    }
    let mut compact = json!({
        "results": results,
        "summary": data.summary,
        "duration_ms": data.duration_ms,
    });
    cap_compact(&mut compact)?;
    let text = serde_json::to_string_pretty(&compact)
        .map_err(|_| EncodeError::MalformedFlowData("compact result cannot be serialized"))?;
    Ok(json!({
        "content": [{"type": "text", "text": text}],
        "structuredContent": compact,
    }))
}

fn encode_batch_flow(data: FlowResultData, hints: &FlowRenderHints) -> Value {
    let mut content = Vec::new();
    for (step, hint) in data.steps.into_iter().zip(&hints.steps) {
        match step.status {
            FlowStepStatus::NotRun => {}
            FlowStepStatus::Ok => content.extend(step.result.parts.into_iter().map(render_part)),
            status => {
                let first_text = first_step_text(&step).unwrap_or("");
                content.push(json!({
                    "type": "text",
                    "text": format!(
                        "step {} ({}) {}: {first_text}",
                        step.step,
                        hint.label,
                        legacy_flow_status(status),
                    ),
                }));
            }
        }
    }
    content.push(json!({"type": "text", "text": data.summary}));
    json!({"content": content})
}

fn render_part(part: ResultPart) -> Value {
    match part {
        ResultPart::Text { text } => json!({"type": "text", "text": text}),
        ResultPart::Image { data, mime_type } => {
            json!({"type": "image", "data": data, "mimeType": mime_type})
        }
    }
}

fn first_step_text(step: &FlowStepResult) -> Option<&str> {
    step.result.parts.first().and_then(|part| match part {
        ResultPart::Text { text } => Some(text.as_str()),
        ResultPart::Image { .. } => None,
    })
}

fn legacy_flow_status(status: FlowStepStatus) -> &'static str {
    match status {
        FlowStepStatus::Ok => "ok",
        FlowStepStatus::Denied => "denied",
        FlowStepStatus::Held => "held",
        FlowStepStatus::AttentionRequired => "attention_required",
        FlowStepStatus::Cancelled => "cancelled",
        FlowStepStatus::NotRun => "not_run",
        FlowStepStatus::WouldAllow => "would_allow",
        FlowStepStatus::WouldDeny => "would_deny",
        FlowStepStatus::Partial
        | FlowStepStatus::NotMet
        | FlowStepStatus::Blocked
        | FlowStepStatus::NotDispatched
        | FlowStepStatus::OutcomeUnknown
        | FlowStepStatus::Unavailable => "error",
    }
}

fn truncate_step_text(text: &str) -> String {
    if text.chars().count() <= STEP_TEXT_BUDGET {
        return text.to_owned();
    }
    let head: String = text.chars().take(STEP_TEXT_BUDGET).collect();
    format!("{head}(truncated)")
}

fn cap_compact(compact: &mut Value) -> Result<(), EncodeError> {
    let target = COMPACT_BUDGET - COMPACT_BATCH_ID_HEADROOM;
    loop {
        let serialized = serde_json::to_string(compact).unwrap_or_default();
        if serialized.len() <= target {
            return Ok(());
        }
        let results = compact
            .get_mut("results")
            .and_then(Value::as_array_mut)
            .expect("compact flow has a results array");
        let mut longest: Option<(usize, usize)> = None;
        for (index, result) in results.iter().enumerate() {
            if let Some(text) = result.get("result").and_then(Value::as_str) {
                let len = text.len();
                if longest.is_none_or(|(longest_len, _)| len > longest_len) {
                    longest = Some((len, index));
                }
            }
        }
        if let Some((_, index)) = longest {
            let current = results[index]["result"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            if current.len() > 200 {
                let mut end = 160.min(current.len());
                while !current.is_char_boundary(end) {
                    end -= 1;
                }
                results[index]["result"] =
                    Value::String(format!("{}...(truncated)", &current[..end]));
                continue;
            }
        }

        let largest_structured = results
            .iter()
            .enumerate()
            .filter_map(|(index, result)| {
                result
                    .get("structured")
                    .map(|structured| (structured.to_string().len(), index))
            })
            .max_by_key(|(len, _)| *len);
        if let Some((_, index)) = largest_structured {
            if let Some(result) = results[index].as_object_mut() {
                result.remove("structured");
                result.insert("structured_truncated".into(), Value::Bool(true));
            }
            continue;
        }

        if results.len() > 1 {
            if let Some(index) = results.iter().rposition(|result| {
                matches!(
                    result.get("status").and_then(Value::as_str),
                    Some("ok" | "would_allow" | "not_run")
                )
            }) {
                results.remove(index);
                continue;
            }
        }

        return Err(EncodeError::MalformedFlowData(
            "compact result exceeds the 25000-byte legacy limit without a safe removable entry",
        ));
    }
}

fn reinsert_legacy_provenance(
    data: &mut Value,
    provenance: &PageProvenance,
) -> Result<(), EncodeError> {
    let top_origin = provenance
        .top_origin()
        .filter(|origin| is_valid_origin(origin))
        .ok_or(EncodeError::MalformedProvenance(
            "top_origin must be non-empty, control-free, and at most 240 UTF-8 bytes",
        ))?;
    let session_nonce = provenance
        .session_nonce()
        .filter(|nonce| is_valid_session_nonce(nonce))
        .ok_or(EncodeError::MalformedProvenance(
            "session_nonce must be bounded lowercase even-length hexadecimal with at least 96 bits",
        ))?;
    let mut marker = json!({
        "pageSourced": true,
        "untrusted": true,
        "topOrigin": top_origin,
        "sessionNonce": session_nonce
    });
    if let Some(frame_origin) = provenance.frame_origin() {
        marker["frameOrigin"] = Value::String(frame_origin.to_owned());
    }

    let root = data
        .as_object_mut()
        .ok_or(EncodeError::MalformedProvenance(
            "structured data must be an object",
        ))?;
    if root.contains_key("provenance") {
        return Err(EncodeError::MalformedProvenance(
            "structured data already contains a root provenance marker",
        ));
    }
    match root.get_mut("interactionReceipt") {
        Some(Value::Object(receipt)) => {
            if receipt.contains_key("provenance") {
                return Err(EncodeError::MalformedProvenance(
                    "interactionReceipt already contains a provenance marker",
                ));
            }
            receipt.insert("provenance".into(), marker);
        }
        Some(_) => {
            return Err(EncodeError::MalformedProvenance(
                "interactionReceipt must be an object when present",
            ));
        }
        None => {
            root.insert("provenance".into(), marker);
        }
    }
    Ok(())
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

/// Normalize one frozen legacy call into the canonical operation vocabulary.
pub(crate) fn decode_call(
    external_tool: &str,
    arguments: Value,
) -> Result<BrowserOperation, DecodeError> {
    decode_call_inner(external_tool, arguments, false)
}

fn decode_call_inner(
    external_tool: &str,
    arguments: Value,
    allow_deferred_references: bool,
) -> Result<BrowserOperation, DecodeError> {
    let declaration = declaration(external_tool)
        .ok_or_else(|| DecodeError::UnknownTool(external_tool.to_owned()))?;
    let validation_arguments = if allow_deferred_references {
        deferred_validation_view(&declaration["inputSchema"], &arguments)
    } else {
        arguments.clone()
    };
    schema::validate(&declaration["inputSchema"], &validation_arguments).map_err(|error| {
        DecodeError::SchemaViolation {
            tool: external_tool.to_owned(),
            message: error.to_string(),
        }
    })?;
    let args = arguments
        .as_object()
        .cloned()
        .ok_or_else(|| DecodeError::ArgumentsNotObject {
            tool: external_tool.to_owned(),
        })?;
    match external_tool {
        "tabs_context_mcp" => simple(
            OperationId::BrowserTabs,
            IntentId::TabsList,
            rename(args, &[("createIfEmpty", "create_if_empty")]),
        ),
        "tabs_create_mcp" => simple(OperationId::BrowserTabs, IntentId::TabsNew, args),
        "navigate" => decode_navigate(args),
        "computer" => decode_computer(args),
        "find" => simple(
            OperationId::BrowserFind,
            IntentId::FindQuery,
            with_tab(args),
        ),
        "form_input" => decode_form_input(args),
        "get_page_text" => simple(OperationId::BrowserRead, IntentId::ReadText, with_tab(args)),
        "javascript_tool" => decode_javascript(args),
        "read_console_messages" => decode_console(args),
        "read_network_requests" => decode_network(args),
        "read_page" => simple(
            OperationId::BrowserSnapshot,
            IntentId::SnapshotCapture,
            rename(with_tab(args), &[("ref_id", "scope_ref")]),
        ),
        "resize_window" => simple(
            OperationId::BrowserViewport,
            IntentId::ViewportResizeWindow,
            with_tab(args),
        ),
        "update_plan" => simple(OperationId::WorkflowPlan, IntentId::PlanUpdate, args),
        "narrate" => simple(
            OperationId::BrowserPresent,
            IntentId::PresentNarrate,
            with_tab(args),
        ),
        "wait_for" => decode_wait_for(args),
        "script" => decode_script(args),
        "form_fill" => decode_form_fill(args),
        "act_on" => decode_act_on(args),
        "dialog" => decode_dialog(args),
        "tab_control" => decode_tab_control(args),
        "file_upload" => decode_file_upload(args),
        "browser_batch" => decode_browser_batch(args),
        "upload_image" => decode_upload_image(args),
        "gif_creator" => decode_gif(args),
        "explain" => simple(OperationId::BrowserContext, IntentId::ContextDescribe, args),
        other => Err(DecodeError::UnknownTool(other.to_owned())),
    }
}

fn simple(
    id: OperationId,
    intent: IntentId,
    arguments: Map<String, Value>,
) -> Result<BrowserOperation, DecodeError> {
    Ok(BrowserOperation::new(id, intent, Value::Object(arguments)))
}

fn with_tab(args: Map<String, Value>) -> Map<String, Value> {
    rename(args, &[("tabId", "tab")])
}

fn rename(
    mut args: Map<String, Value>,
    fields: &[(&'static str, &'static str)],
) -> Map<String, Value> {
    for (old, new) in fields {
        if let Some(value) = args.remove(*old) {
            args.insert((*new).to_owned(), value);
        }
    }
    args
}

fn move_field(
    source: &mut Map<String, Value>,
    target: &mut Map<String, Value>,
    source_name: &str,
    target_name: &str,
) {
    if let Some(value) = source.remove(source_name) {
        target.insert(target_name.to_owned(), value);
    }
}

fn take_tab(args: &mut Map<String, Value>) -> Map<String, Value> {
    let mut canonical = Map::new();
    move_field(args, &mut canonical, "tabId", "tab");
    canonical
}

fn is_deferred_reference(value: &Value) -> bool {
    value.as_str().is_some_and(is_deferred_reference_string)
}

fn is_deferred_reference_string(value: &str) -> bool {
    let Some(body) = value.strip_prefix('$') else {
        return false;
    };
    if body.starts_with('$') {
        return false;
    }
    let rest = if let Some(rest) = body.strip_prefix("prev") {
        rest
    } else {
        let digit_count = body.bytes().take_while(u8::is_ascii_digit).count();
        if digit_count == 0 || body.as_bytes()[0] == b'0' {
            return false;
        }
        &body[digit_count..]
    };
    rest.is_empty()
        || rest
            .strip_prefix('.')
            .is_some_and(|path| !path.is_empty() && path.split('.').all(|part| !part.is_empty()))
}

fn deferred_validation_view(schema: &Value, instance: &Value) -> Value {
    if is_deferred_reference(instance) {
        return validation_placeholder(schema);
    }
    match instance {
        Value::Object(object) => {
            let properties = schema.get("properties").and_then(Value::as_object);
            let additional = schema.get("additionalProperties");
            Value::Object(
                object
                    .iter()
                    .map(|(field, value)| {
                        let field_schema = properties
                            .and_then(|properties| properties.get(field))
                            .or_else(|| additional.filter(|value| value.is_object()))
                            .unwrap_or(&Value::Null);
                        (field.clone(), deferred_validation_view(field_schema, value))
                    })
                    .collect(),
            )
        }
        Value::Array(items) => {
            let item_schema = schema.get("items").unwrap_or(&Value::Null);
            Value::Array(
                items
                    .iter()
                    .map(|item| deferred_validation_view(item_schema, item))
                    .collect(),
            )
        }
        _ => instance.clone(),
    }
}

fn validation_placeholder(schema: &Value) -> Value {
    if let Some(value) = schema.get("const") {
        return value.clone();
    }
    if let Some(value) = schema
        .get("enum")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
    {
        return value.clone();
    }
    if let Some(value) = schema.get("default") {
        return value.clone();
    }
    let type_name = match schema.get("type") {
        Some(Value::String(name)) => Some(name.as_str()),
        Some(Value::Array(names)) => names.first().and_then(Value::as_str),
        _ => None,
    };
    match type_name {
        Some("string") => {
            let length = schema.get("minLength").and_then(Value::as_u64).unwrap_or(1) as usize;
            Value::String("x".repeat(length))
        }
        Some("number") | Some("integer") => {
            schema.get("minimum").cloned().unwrap_or_else(|| json!(0))
        }
        Some("boolean") => Value::Bool(false),
        Some("array") => {
            let count = schema.get("minItems").and_then(Value::as_u64).unwrap_or(0) as usize;
            let item_schema = schema.get("items").unwrap_or(&Value::Null);
            Value::Array(
                (0..count)
                    .map(|_| validation_placeholder(item_schema))
                    .collect(),
            )
        }
        Some("object") => {
            let properties = schema.get("properties").and_then(Value::as_object);
            let mut object = Map::new();
            if let Some(required) = schema.get("required").and_then(Value::as_array) {
                for field in required.iter().filter_map(Value::as_str) {
                    let field_schema = properties
                        .and_then(|properties| properties.get(field))
                        .unwrap_or(&Value::Null);
                    object.insert(field.to_owned(), validation_placeholder(field_schema));
                }
            }
            let minimum = schema
                .get("minProperties")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            if object.len() < minimum {
                let additional = schema
                    .get("additionalProperties")
                    .filter(|value| value.is_object())
                    .unwrap_or(&Value::Null);
                object.insert("deferred".into(), validation_placeholder(additional));
            }
            Value::Object(object)
        }
        Some("null") | None => Value::Null,
        Some(_) => Value::Null,
    }
}

fn non_empty_string_or_reference(value: &Value) -> bool {
    is_deferred_reference(value) || value.as_str().is_some_and(|value| !value.trim().is_empty())
}

fn valid_point_or_reference(value: &Value) -> bool {
    is_deferred_reference(value)
        || value.as_array().is_some_and(|point| {
            point.len() == 2
                && point
                    .iter()
                    .all(|coordinate| coordinate.is_number() || is_deferred_reference(coordinate))
        })
}

fn reject_deferred_discriminant(
    tool: &'static str,
    field: &'static str,
    value: Option<&Value>,
) -> Result<(), DecodeError> {
    if value.is_some_and(is_deferred_reference) {
        return Err(DecodeError::InvalidShape(format!(
            "{tool}.{field} cannot be a deferred reference because it selects a canonical intent"
        )));
    }
    Ok(())
}

fn take_action(tool: &'static str, args: &mut Map<String, Value>) -> Result<String, DecodeError> {
    args.remove("action")
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(DecodeError::MissingAction { tool })
}

fn decode_navigate(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let url = args.get("url").and_then(Value::as_str);
    let intent = match url {
        Some("back") => {
            args.remove("url");
            IntentId::NavigateBack
        }
        Some("forward") => {
            args.remove("url");
            IntentId::NavigateForward
        }
        _ => IntentId::NavigateUrl,
    };
    simple(OperationId::BrowserNavigate, intent, with_tab(args))
}

fn decode_computer(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let action = take_action("computer", &mut args)?;
    let coordinate = args.remove("coordinate");
    let reference = args.remove("ref");
    let mut canonical = take_tab(&mut args);
    match action.as_str() {
        "left_click" | "right_click" | "double_click" | "triple_click" | "hover" => {
            let coordinate_intent = match action.as_str() {
                "left_click" => IntentId::InputPointerClick,
                "right_click" => IntentId::InputPointerRightClick,
                "double_click" => IntentId::InputPointerDoubleClick,
                "triple_click" => IntentId::InputPointerTripleClick,
                "hover" => IntentId::InputPointerHover,
                _ => unreachable!(),
            };
            let reference_intent = match action.as_str() {
                "left_click" => IntentId::ActClick,
                "right_click" => IntentId::ActRightClick,
                "double_click" => IntentId::ActDoubleClick,
                "triple_click" => IntentId::ActTripleClick,
                "hover" => IntentId::ActHover,
                _ => unreachable!(),
            };
            move_field(&mut args, &mut canonical, "modifiers", "modifiers");
            if let Some(point) = coordinate {
                canonical.insert("point".into(), point);
                simple(OperationId::BrowserInput, coordinate_intent, canonical)
            } else if let Some(reference) = reference.filter(non_empty_string_or_reference) {
                canonical.insert("target".into(), json!({"ref": reference}));
                simple(OperationId::BrowserAct, reference_intent, canonical)
            } else {
                Err(DecodeError::InvalidShape(format!(
                    "computer {action} requires coordinate or a non-empty ref"
                )))
            }
        }
        "screenshot" => simple(
            OperationId::BrowserScreenshot,
            IntentId::ScreenshotViewport,
            canonical,
        ),
        "zoom" => {
            let region = args
                .remove("region")
                .ok_or_else(|| DecodeError::InvalidShape("computer zoom requires region".into()))?;
            canonical.insert("region".into(), region);
            simple(
                OperationId::BrowserScreenshot,
                IntentId::ScreenshotRegion,
                canonical,
            )
        }
        "wait" => {
            let seconds = match args.remove("duration") {
                None => json!(1),
                Some(value) if value.as_f64() == Some(0.0) => json!(1),
                Some(value) => value,
            };
            canonical.insert("seconds".into(), seconds);
            simple(OperationId::BrowserWait, IntentId::WaitDelay, canonical)
        }
        "type" => {
            let text = args
                .remove("text")
                .filter(non_empty_string_or_reference)
                .ok_or_else(|| {
                    DecodeError::InvalidShape("computer type requires non-empty text".into())
                })?;
            canonical.insert("text".into(), text);
            simple(
                OperationId::BrowserInput,
                IntentId::InputTypeText,
                canonical,
            )
        }
        "key" => {
            let key = args
                .remove("text")
                .filter(non_empty_string_or_reference)
                .ok_or_else(|| {
                    DecodeError::InvalidShape("computer key requires non-empty text".into())
                })?;
            canonical.insert("key".into(), key);
            canonical.insert(
                "repeat".into(),
                args.remove("repeat").unwrap_or_else(|| json!(1)),
            );
            simple(
                OperationId::BrowserInput,
                IntentId::InputPressKey,
                canonical,
            )
        }
        "scroll" => {
            if let Some(point) = coordinate {
                canonical.insert("point".into(), point);
            } else if let Some(reference) = reference.filter(non_empty_string_or_reference) {
                canonical.insert("target".into(), json!({"ref": reference}));
            }
            canonical.insert(
                "direction".into(),
                args.remove("scroll_direction")
                    .unwrap_or_else(|| json!("down")),
            );
            canonical.insert(
                "amount".into(),
                args.remove("scroll_amount").unwrap_or_else(|| json!(3)),
            );
            move_field(&mut args, &mut canonical, "modifiers", "modifiers");
            simple(OperationId::BrowserInput, IntentId::InputWheel, canonical)
        }
        "scroll_to" => {
            if let Some(reference) = reference.filter(non_empty_string_or_reference) {
                canonical.insert("target".into(), json!({"ref": reference}));
                simple(
                    OperationId::BrowserAct,
                    IntentId::ActScrollIntoView,
                    canonical,
                )
            } else if let Some(point) = coordinate {
                canonical.insert("point".into(), point);
                simple(
                    OperationId::BrowserInput,
                    IntentId::InputScrollToOffset,
                    canonical,
                )
            } else {
                Err(DecodeError::InvalidShape(
                    "computer scroll_to requires ref or coordinate".into(),
                ))
            }
        }
        "left_click_drag" => {
            let from = args.remove("start_coordinate").ok_or_else(|| {
                DecodeError::InvalidShape(
                    "computer left_click_drag requires start_coordinate".into(),
                )
            })?;
            let to = coordinate.ok_or_else(|| {
                DecodeError::InvalidShape("computer left_click_drag requires coordinate".into())
            })?;
            canonical.insert("from".into(), from);
            canonical.insert("to".into(), to);
            move_field(&mut args, &mut canonical, "modifiers", "modifiers");
            simple(
                OperationId::BrowserInput,
                IntentId::InputPointerDrag,
                canonical,
            )
        }
        other => Err(DecodeError::UnknownAction {
            tool: "computer",
            action: other.to_owned(),
        }),
    }
}

fn decode_form_input(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let reference = args
        .remove("ref")
        .filter(non_empty_string_or_reference)
        .ok_or_else(|| DecodeError::InvalidShape("form_input requires a non-empty ref".into()))?;
    let mut canonical = with_tab(args);
    canonical.insert("target".into(), json!({"ref": reference}));
    simple(OperationId::BrowserFill, IntentId::FillField, canonical)
}

fn decode_javascript(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let action = take_action("javascript_tool", &mut args)?;
    if action != "javascript_exec" {
        return Err(DecodeError::UnknownAction {
            tool: "javascript_tool",
            action,
        });
    }
    let canonical = rename(with_tab(args), &[("text", "script")]);
    simple(
        OperationId::BrowserEvaluate,
        IntentId::EvaluateJavascript,
        canonical,
    )
}

fn decode_console(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    reject_deferred_discriminant("read_console_messages", "clear", args.get("clear"))?;
    let clear = args
        .remove("clear")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let intent = if clear {
        IntentId::ConsoleReadAndClear
    } else {
        IntentId::ConsoleRead
    };
    simple(
        OperationId::BrowserConsole,
        intent,
        rename(with_tab(args), &[("onlyErrors", "only_errors")]),
    )
}

fn decode_network(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    reject_deferred_discriminant("read_network_requests", "clear", args.get("clear"))?;
    let clear = args
        .remove("clear")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let intent = if clear {
        IntentId::NetworkReadAndClear
    } else {
        IntentId::NetworkRead
    };
    simple(
        OperationId::BrowserNetwork,
        intent,
        rename(with_tab(args), &[("urlPattern", "url_pattern")]),
    )
}

fn decode_wait_for(args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let selector = args.get("selector").filter(|value| {
        is_deferred_reference(value) || value.as_str().is_some_and(|value| !value.is_empty())
    });
    let text = args.get("text").filter(|value| {
        is_deferred_reference(value) || value.as_str().is_some_and(|value| !value.is_empty())
    });
    if selector.is_some() && text.is_some() {
        return Err(DecodeError::InvalidShape(
            "wait_for accepts at most one non-empty selector or text".into(),
        ));
    }
    if args.get("state").and_then(Value::as_str) == Some("settled")
        && (selector.is_some() || text.is_some())
    {
        return Err(DecodeError::InvalidShape(
            "wait_for state settled cannot be combined with selector or text".into(),
        ));
    }
    let timeout = args.get("timeout_ms").and_then(Value::as_f64);
    let minimum = args.get("min_ms").and_then(Value::as_f64);
    if timeout.is_some_and(|value| value > 30_000.0) {
        return Err(DecodeError::InvalidShape(
            "wait_for timeout_ms must not exceed 30000".into(),
        ));
    }
    if minimum
        .zip(timeout)
        .is_some_and(|(minimum, timeout)| minimum > timeout)
    {
        return Err(DecodeError::InvalidShape(
            "wait_for min_ms must not exceed timeout_ms".into(),
        ));
    }
    simple(
        OperationId::BrowserWait,
        IntentId::WaitUntil,
        with_tab(args),
    )
}

fn decode_script(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    reject_deferred_discriminant("script", "dry_run", args.get("dry_run"))?;
    let preflight = args
        .remove("dry_run")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let intent = if preflight {
        IntentId::FlowPreflight
    } else {
        IntentId::FlowExecute
    };
    let steps = args
        .remove("steps")
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| DecodeError::InvalidShape("script requires a steps array".into()))?;
    let mut canonical_steps = Vec::with_capacity(steps.len());
    let mut retained_tab = args.get("tabId").cloned();
    for step in steps {
        let object = step
            .as_object()
            .ok_or_else(|| DecodeError::InvalidShape("script step must be an object".into()))?;
        let tool = object
            .get("tool")
            .and_then(Value::as_str)
            .ok_or_else(|| DecodeError::InvalidShape("script step requires tool".into()))?;
        if matches!(tool, "script" | "browser_batch") {
            return Err(DecodeError::InvalidShape(
                "flows cannot contain another flow".into(),
            ));
        }
        let mut step_args = object.get("args").cloned().unwrap_or_else(|| json!({}));
        if declares_argument(tool, "tabId") {
            if let (Some(step_args), Some(tab)) = (step_args.as_object_mut(), retained_tab.as_ref())
            {
                step_args.entry("tabId").or_insert_with(|| tab.clone());
            }
        }
        let operation = decode_call_inner(tool, step_args, true)?;
        if let Some(tab) = operation.arguments.get("tab") {
            retained_tab = Some(tab.clone());
        }
        canonical_steps.push(serde_json::to_value(operation).expect("operation serializes"));
    }
    let mut canonical = rename(with_tab(args), &[("onError", "on_error")]);
    canonical.insert("steps".into(), Value::Array(canonical_steps));
    simple(OperationId::BrowserFlow, intent, canonical)
}

fn decode_browser_batch(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let actions = args
        .remove("actions")
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| {
            DecodeError::InvalidShape("browser_batch requires an actions array".into())
        })?;
    let mut steps = Vec::with_capacity(actions.len());
    for action in actions {
        let object = action.as_object().ok_or_else(|| {
            DecodeError::InvalidShape("browser_batch action must be an object".into())
        })?;
        let tool = object.get("name").and_then(Value::as_str).ok_or_else(|| {
            DecodeError::InvalidShape("browser_batch action requires name".into())
        })?;
        if matches!(tool, "script" | "browser_batch") {
            return Err(DecodeError::InvalidShape(
                "flows cannot contain another flow".into(),
            ));
        }
        let input = object.get("input").cloned().unwrap_or_else(|| json!({}));
        let operation = decode_call(tool, input)?;
        steps.push(serde_json::to_value(operation).expect("operation serializes"));
    }
    let mut canonical = rename(with_tab(args), &[("onError", "on_error")]);
    canonical.insert("steps".into(), Value::Array(steps));
    canonical.entry("on_error").or_insert_with(|| json!("stop"));
    simple(OperationId::BrowserFlow, IntentId::FlowExecute, canonical)
}

fn decode_form_fill(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    reject_deferred_discriminant("form_fill", "submit", args.get("submit"))?;
    let submit = args
        .remove("submit")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let fields = match args
        .remove("fields")
        .ok_or_else(|| DecodeError::InvalidShape("form_fill requires fields".into()))?
    {
        Value::Object(fields) => Value::Array(
            fields
                .into_iter()
                .map(|(query, value)| {
                    if query.trim().is_empty() {
                        return Err(DecodeError::InvalidShape(
                            "form_fill field queries must not be empty".into(),
                        ));
                    }
                    Ok(json!({"target": {"query": query}, "value": value}))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        reference if is_deferred_reference(&reference) => reference,
        _ => {
            return Err(DecodeError::InvalidShape(
                "form_fill fields must be an object or deferred reference".into(),
            ))
        }
    };
    let mut canonical = with_tab(args);
    canonical.insert("fields".into(), fields);
    simple(
        OperationId::BrowserFill,
        if submit {
            IntentId::FillFieldsAndSubmit
        } else {
            IntentId::FillFields
        },
        canonical,
    )
}

fn decode_act_on(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let action = take_action("act_on", &mut args)?;
    let intent = match action.as_str() {
        "left_click" => IntentId::ActClick,
        "right_click" => IntentId::ActRightClick,
        "double_click" => IntentId::ActDoubleClick,
        "hover" => IntentId::ActHover,
        "scroll_to" => IntentId::ActScrollIntoView,
        "set_value" => IntentId::ActSetValue,
        other => {
            return Err(DecodeError::UnknownAction {
                tool: "act_on",
                action: other.into(),
            })
        }
    };
    let target = args
        .get("target")
        .ok_or_else(|| DecodeError::InvalidShape("act_on requires target".into()))?;
    if !is_deferred_reference(target) {
        let target = target
            .as_object()
            .ok_or_else(|| DecodeError::InvalidShape("act_on target must be an object".into()))?;
        if target
            .keys()
            .any(|field| !matches!(field.as_str(), "ref" | "query" | "name" | "role"))
        {
            return Err(DecodeError::InvalidShape(
                "act_on target contains an unsupported field".into(),
            ));
        }
        let modes = ["ref", "query", "name"]
            .iter()
            .filter(|field| {
                target
                    .get(**field)
                    .is_some_and(non_empty_string_or_reference)
            })
            .count();
        if modes != 1 {
            return Err(DecodeError::InvalidShape(
                "act_on target requires exactly one non-empty ref, query, or name".into(),
            ));
        }
        if target.contains_key("role") && !target.contains_key("name") {
            return Err(DecodeError::InvalidShape(
                "act_on target.role is valid only with target.name".into(),
            ));
        }
    }
    let has_value = args.contains_key("value");
    if action == "set_value" && !has_value {
        return Err(DecodeError::InvalidShape(
            "act_on set_value requires value".into(),
        ));
    }
    if action != "set_value" && has_value {
        return Err(DecodeError::InvalidShape(
            "act_on value is valid only for set_value".into(),
        ));
    }
    if let Some(expect) = args.get("expect") {
        if !is_deferred_reference(expect) {
            let expect = expect.as_object().ok_or_else(|| {
                DecodeError::InvalidShape("act_on expect must be an object".into())
            })?;
            let modes = ["selector", "text"]
                .iter()
                .filter(|field| {
                    expect
                        .get(**field)
                        .is_some_and(non_empty_string_or_reference)
                })
                .count();
            if modes != 1 {
                return Err(DecodeError::InvalidShape(
                    "act_on expect requires exactly one non-empty selector or text".into(),
                ));
            }
            if expect.keys().any(|field| {
                !matches!(field.as_str(), "selector" | "text" | "state" | "timeout_ms")
            }) {
                return Err(DecodeError::InvalidShape(
                    "act_on expect contains an unsupported field".into(),
                ));
            }
        }
    }
    simple(OperationId::BrowserAct, intent, with_tab(args))
}

fn decode_dialog(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let action = take_action("dialog", &mut args)?;
    let intent = match action.as_str() {
        "status" => IntentId::DialogStatus,
        "accept" => IntentId::DialogAccept,
        "dismiss" => IntentId::DialogDismiss,
        "respond" => IntentId::DialogRespond,
        other => {
            return Err(DecodeError::UnknownAction {
                tool: "dialog",
                action: other.into(),
            })
        }
    };
    simple(OperationId::BrowserDialog, intent, with_tab(args))
}

fn decode_tab_control(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let action = take_action("tab_control", &mut args)?;
    let (id, intent) = match action.as_str() {
        "focus" => (OperationId::BrowserTabs, IntentId::TabsFocus),
        "reload" => (OperationId::BrowserNavigate, IntentId::NavigateReload),
        "close" => (OperationId::BrowserTabs, IntentId::TabsClose),
        other => {
            return Err(DecodeError::UnknownAction {
                tool: "tab_control",
                action: other.into(),
            })
        }
    };
    simple(id, intent, with_tab(args))
}

fn decode_file_upload(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let reference = args
        .remove("ref")
        .ok_or_else(|| DecodeError::InvalidShape("file_upload requires ref".into()))?;
    let mut canonical = with_tab(args);
    canonical.insert("target".into(), json!({"ref": reference}));
    if let Some(Value::Array(files)) = canonical.get_mut("files") {
        for file in files {
            if let Some(object) = file.as_object_mut() {
                if let Some(mime) = object.remove("mimeType") {
                    object.insert("mime_type".into(), mime);
                }
            }
        }
    }
    simple(
        OperationId::BrowserUpload,
        IntentId::UploadClientFiles,
        canonical,
    )
}

fn decode_upload_image(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let artifact = args
        .remove("imageId")
        .ok_or_else(|| DecodeError::InvalidShape("upload_image requires imageId".into()))?;
    let reference = args.remove("ref");
    let point = args.remove("coordinate");
    if usize::from(reference.is_some()) + usize::from(point.is_some()) != 1 {
        return Err(DecodeError::InvalidShape(
            "upload_image requires exactly one ref or coordinate".into(),
        ));
    }
    if point
        .as_ref()
        .is_some_and(|point| !valid_point_or_reference(point))
    {
        return Err(DecodeError::InvalidShape(
            "upload_image coordinate must contain exactly two numbers or deferred references"
                .into(),
        ));
    }
    let mut canonical = with_tab(args);
    canonical.insert("artifact".into(), artifact);
    if let Some(reference) = reference {
        canonical.insert("target".into(), json!({"ref": reference}));
    }
    if let Some(point) = point {
        canonical.insert("point".into(), point);
    }
    simple(
        OperationId::BrowserUpload,
        IntentId::UploadCapturedArtifact,
        canonical,
    )
}

fn decode_gif(mut args: Map<String, Value>) -> Result<BrowserOperation, DecodeError> {
    let action = take_action("gif_creator", &mut args)?;
    let intent = match action.as_str() {
        "start_recording" => IntentId::RecordStart,
        "stop_recording" => IntentId::RecordStop,
        "status" => IntentId::RecordStatus,
        "clear" => IntentId::RecordClear,
        "export" => IntentId::RecordExport,
        other => {
            return Err(DecodeError::UnknownAction {
                tool: "gif_creator",
                action: other.into(),
            })
        }
    };
    let mut canonical = take_tab(&mut args);
    if intent == IntentId::RecordExport {
        let reference = args.remove("ref");
        let point = args.remove("coordinate");
        let download = args.remove("download");
        let download_target = download
            .as_ref()
            .is_some_and(|value| value.as_bool() == Some(true) || is_deferred_reference(value));
        let delivery_targets = usize::from(reference.is_some())
            + usize::from(point.is_some())
            + usize::from(download_target);
        if delivery_targets != 1 {
            return Err(DecodeError::InvalidShape(
                "gif_creator export requires exactly one ref, coordinate, or download:true".into(),
            ));
        }
        if point
            .as_ref()
            .is_some_and(|point| !valid_point_or_reference(point))
        {
            return Err(DecodeError::InvalidShape(
                "gif_creator coordinate must contain exactly two numbers or deferred references"
                    .into(),
            ));
        }
        if let Some(reference) = reference {
            canonical.insert("target".into(), json!({"ref": reference}));
        }
        if let Some(point) = point {
            canonical.insert("point".into(), point);
        }
        if download_target {
            canonical.insert("download".into(), download.expect("download target exists"));
        }
        move_field(&mut args, &mut canonical, "filename", "filename");
        move_field(&mut args, &mut canonical, "options", "options");
    }
    simple(OperationId::BrowserRecord, intent, canonical)
}

fn tool_keys(tool: &str) -> Option<Vec<OperationKey>> {
    use IntentId::*;
    use OperationId::*;
    let keys = match tool {
        "tabs_context_mcp" => vec![OperationKey::new(BrowserTabs, TabsList)],
        "tabs_create_mcp" => vec![OperationKey::new(BrowserTabs, TabsNew)],
        "navigate" => vec![
            OperationKey::new(BrowserNavigate, NavigateUrl),
            OperationKey::new(BrowserNavigate, NavigateBack),
            OperationKey::new(BrowserNavigate, NavigateForward),
        ],
        "computer" => vec![
            OperationKey::new(BrowserAct, ActClick),
            OperationKey::new(BrowserAct, ActRightClick),
            OperationKey::new(BrowserAct, ActDoubleClick),
            OperationKey::new(BrowserAct, ActTripleClick),
            OperationKey::new(BrowserAct, ActHover),
            OperationKey::new(BrowserAct, ActScrollIntoView),
            OperationKey::new(BrowserInput, InputPointerClick),
            OperationKey::new(BrowserInput, InputPointerRightClick),
            OperationKey::new(BrowserInput, InputPointerDoubleClick),
            OperationKey::new(BrowserInput, InputPointerTripleClick),
            OperationKey::new(BrowserInput, InputPointerHover),
            OperationKey::new(BrowserInput, InputPointerDrag),
            OperationKey::new(BrowserInput, InputTypeText),
            OperationKey::new(BrowserInput, InputPressKey),
            OperationKey::new(BrowserInput, InputWheel),
            OperationKey::new(BrowserInput, InputScrollToOffset),
            OperationKey::new(BrowserScreenshot, ScreenshotViewport),
            OperationKey::new(BrowserScreenshot, ScreenshotRegion),
            OperationKey::new(BrowserWait, WaitDelay),
        ],
        "find" => vec![OperationKey::new(BrowserFind, FindQuery)],
        "form_input" => vec![OperationKey::new(BrowserFill, FillField)],
        "get_page_text" => vec![OperationKey::new(BrowserRead, ReadText)],
        "javascript_tool" => vec![OperationKey::new(BrowserEvaluate, EvaluateJavascript)],
        "read_console_messages" => vec![
            OperationKey::new(BrowserConsole, ConsoleRead),
            OperationKey::new(BrowserConsole, ConsoleReadAndClear),
        ],
        "read_network_requests" => vec![
            OperationKey::new(BrowserNetwork, NetworkRead),
            OperationKey::new(BrowserNetwork, NetworkReadAndClear),
        ],
        "read_page" => vec![OperationKey::new(BrowserSnapshot, SnapshotCapture)],
        "resize_window" => vec![OperationKey::new(BrowserViewport, ViewportResizeWindow)],
        "update_plan" => vec![OperationKey::new(WorkflowPlan, PlanUpdate)],
        "narrate" => vec![OperationKey::new(BrowserPresent, PresentNarrate)],
        "wait_for" => vec![OperationKey::new(BrowserWait, WaitUntil)],
        "script" => vec![
            OperationKey::new(BrowserFlow, FlowExecute),
            OperationKey::new(BrowserFlow, FlowPreflight),
        ],
        "browser_batch" => vec![OperationKey::new(BrowserFlow, FlowExecute)],
        "form_fill" => vec![
            OperationKey::new(BrowserFill, FillFields),
            OperationKey::new(BrowserFill, FillFieldsAndSubmit),
        ],
        "act_on" => vec![
            OperationKey::new(BrowserAct, ActClick),
            OperationKey::new(BrowserAct, ActRightClick),
            OperationKey::new(BrowserAct, ActDoubleClick),
            OperationKey::new(BrowserAct, ActHover),
            OperationKey::new(BrowserAct, ActScrollIntoView),
            OperationKey::new(BrowserAct, ActSetValue),
        ],
        "dialog" => vec![
            OperationKey::new(BrowserDialog, DialogStatus),
            OperationKey::new(BrowserDialog, DialogAccept),
            OperationKey::new(BrowserDialog, DialogDismiss),
            OperationKey::new(BrowserDialog, DialogRespond),
        ],
        "tab_control" => vec![
            OperationKey::new(BrowserTabs, TabsFocus),
            OperationKey::new(BrowserNavigate, NavigateReload),
            OperationKey::new(BrowserTabs, TabsClose),
        ],
        "file_upload" => vec![OperationKey::new(BrowserUpload, UploadClientFiles)],
        "upload_image" => vec![OperationKey::new(BrowserUpload, UploadCapturedArtifact)],
        "gif_creator" => vec![
            OperationKey::new(BrowserRecord, RecordStart),
            OperationKey::new(BrowserRecord, RecordStop),
            OperationKey::new(BrowserRecord, RecordStatus),
            OperationKey::new(BrowserRecord, RecordClear),
            OperationKey::new(BrowserRecord, RecordExport),
        ],
        "explain" => vec![OperationKey::new(BrowserContext, ContextDescribe)],
        _ => return None,
    };
    Some(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::bridge::OperationAvailability;

    #[test]
    fn embedded_assets_match_the_frozen_test_oracles() {
        let golden: Value = serde_json::from_str(include_str!(
            "../../../../tests/golden/surfaces/ghostlight-legacy-v1.json"
        ))
        .expect("golden catalog");
        assert_eq!(declarations(), &golden);
        assert_eq!(
            agent_guide(),
            include_str!("../../../../tests/golden/surfaces/ghostlight-legacy-v1-agent-guide.txt")
        );
        assert_eq!(declarations()["tools"].as_array().expect("tools").len(), 25);
    }

    #[test]
    fn every_declared_name_has_a_mapping_and_full_availability_preserves_order() {
        let operations = declarations()["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .flat_map(|tool| tool_keys(tool["name"].as_str().expect("name")).expect("mapping"))
            .map(|key| OperationAvailability {
                id: key.id,
                intent: key.intent,
                workspace_use: if matches!(
                    key.id,
                    OperationId::WorkflowPlan | OperationId::BrowserContext
                ) {
                    WorkspaceUse::Independent
                } else if matches!(key.intent, IntentId::TabsList | IntentId::TabsNew) {
                    WorkspaceUse::Creates
                } else {
                    WorkspaceUse::Uses
                },
            })
            .collect();
        let projection = CatalogProjection {
            generation: 1,
            operations,
            restricted: false,
        };
        let rendered = filtered_declarations(&projection);
        let names: Vec<&str> = rendered
            .iter()
            .map(|tool| tool.declaration["name"].as_str().expect("name"))
            .collect();
        let expected: Vec<&str> = declarations()["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .map(|tool| tool["name"].as_str().expect("name"))
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn argument_sensitive_calls_normalize_without_surface_discriminants() {
        let click = decode_call(
            "computer",
            json!({"action":"left_click","tabId":7,"ref":"ref_1"}),
        )
        .expect("ref click");
        assert_eq!(
            click.key(),
            OperationKey::new(OperationId::BrowserAct, IntentId::ActClick)
        );
        assert_eq!(click.arguments, json!({"tab":7,"target":{"ref":"ref_1"}}));

        let coordinate = decode_call(
            "computer",
            json!({"action":"left_click","tabId":7,"coordinate":[10,20],"ref":"ignored"}),
        )
        .expect("coordinate wins");
        assert_eq!(
            coordinate.key(),
            OperationKey::new(OperationId::BrowserInput, IntentId::InputPointerClick)
        );
        assert!(coordinate.arguments.get("action").is_none());
        assert!(coordinate.arguments.get("tabId").is_none());
        assert!(coordinate.arguments.get("ref").is_none());
        assert_eq!(coordinate.arguments["point"], json!([10, 20]));
    }

    fn assert_schema_rejected(tool: &str, arguments: Value) {
        let error = decode_call(tool, arguments).expect_err("invalid surface call must fail");
        assert!(
            matches!(error, DecodeError::SchemaViolation { .. }),
            "unexpected error for {tool}: {error}"
        );
    }

    #[test]
    fn wrong_typed_control_fields_never_choose_a_canonical_intent() {
        assert_schema_rejected(
            "script",
            json!({
                "dry_run": "true",
                "steps": [{
                    "tool": "computer",
                    "args": {"action":"left_click","tabId":7,"coordinate":[10,20]}
                }]
            }),
        );
        assert_schema_rejected(
            "form_fill",
            json!({"tabId":7,"fields":{"Email":"ada@example.com"},"submit":"true"}),
        );
        assert_schema_rejected("read_console_messages", json!({"tabId":7,"clear":"true"}));
        assert_schema_rejected("read_network_requests", json!({"tabId":7,"clear":1}));
    }

    #[test]
    fn frozen_schema_rejects_missing_unknown_and_nested_invalid_fields() {
        assert_schema_rejected("find", json!({"tabId":7}));
        assert_schema_rejected("find", json!({"tabId":7,"query":"Save","qurey":"typo"}));
        assert_schema_rejected(
            "act_on",
            json!({
                "tabId":7,
                "target":{"ref":"ref_1","unexpected":true},
                "action":"left_click"
            }),
        );
        assert_schema_rejected(
            "act_on",
            json!({
                "tabId":7,
                "target":{"ref":"ref_1"},
                "action":"left_click",
                "expect":{"text":"Saved","state":"later"}
            }),
        );
        assert_schema_rejected(
            "file_upload",
            json!({"tabId":7,"ref":"ref_1","files":[{"name":"a.txt","data":7}]}),
        );
        assert_schema_rejected("computer", json!({"tabId":7,"action":"fly"}));
    }

    #[test]
    fn frozen_declaration_examples_still_normalize() {
        for declaration in declarations()["tools"].as_array().expect("tools") {
            let name = declaration["name"].as_str().expect("tool name");
            let arguments = declaration
                .pointer("/example/call")
                .cloned()
                .unwrap_or_else(|| json!({}));
            decode_call(name, arguments)
                .unwrap_or_else(|error| panic!("valid example for {name} failed: {error}"));
        }
    }

    #[test]
    fn flows_contain_only_canonical_operations_and_reject_nesting() {
        let script_arguments = json!({
            "tabId": 7,
            "steps": [
                {"tool":"find","args":{"tabId":7,"query":"Save"}},
                {"tool":"act_on","args":{"tabId":7,"target":{"ref":"$prev.results.0.ref"},"action":"left_click"}}
            ]
        });
        let flow = decode_call("script", script_arguments.clone()).expect("flow");
        let hints = flow_render_hints("script", &script_arguments, &flow).expect("flow hints");
        assert_eq!(
            hints
                .steps
                .iter()
                .map(|hint| hint.label.as_str())
                .collect::<Vec<_>>(),
            vec!["find", "act_on"]
        );
        assert_eq!(
            hints.steps[0].expected_operation,
            OperationKey::new(OperationId::BrowserFind, IntentId::FindQuery)
        );
        let rendered = flow.arguments.to_string();
        assert!(!rendered.contains("\"tool\""));
        assert!(!rendered.contains("act_on"));
        assert!(rendered.contains("browser.find"));
        assert!(rendered.contains("browser.act"));

        let batch = decode_call(
            "browser_batch",
            json!({
                "actions": [{"name":"find","input":{"tabId":7,"query":"Save"}}]
            }),
        )
        .expect("browser_batch flow");
        assert_eq!(
            batch.key(),
            OperationKey::new(OperationId::BrowserFlow, IntentId::FlowExecute)
        );
        let batch_rendered = batch.arguments.to_string();
        assert!(!batch_rendered.contains("\"name\""));
        assert!(!batch_rendered.contains("\"tool\""));
        assert!(batch_rendered.contains("browser.find"));
        assert!(decode_call(
            "script",
            json!({"steps":[{"tool":"browser_batch","args":{"actions":[]}}]})
        )
        .is_err());
    }

    fn flow_step_tab(flow: &BrowserOperation, index: usize) -> Option<i64> {
        flow.arguments
            .pointer(&format!("/steps/{index}/arguments/tab"))
            .and_then(Value::as_i64)
    }

    #[test]
    fn script_root_tab_is_inherited_by_tab_scoped_steps() {
        let flow = decode_call(
            "script",
            json!({
                "tabId": 7,
                "steps": [
                    {"tool":"find","args":{"query":"Save"}},
                    {"tool":"get_page_text","args":{}}
                ]
            }),
        )
        .expect("root tab is inherited");
        assert_eq!(flow_step_tab(&flow, 0), Some(7));
        assert_eq!(flow_step_tab(&flow, 1), Some(7));
    }

    #[test]
    fn script_retains_the_first_concrete_nested_tab() {
        let flow = decode_call(
            "script",
            json!({
                "steps": [
                    {"tool":"find","args":{"tabId":9,"query":"Save"}},
                    {"tool":"get_page_text","args":{}}
                ]
            }),
        )
        .expect("first nested tab is retained");
        assert_eq!(flow_step_tab(&flow, 0), Some(9));
        assert_eq!(flow_step_tab(&flow, 1), Some(9));
    }

    #[test]
    fn script_explicit_step_tab_overrides_the_retained_tab() {
        let flow = decode_call(
            "script",
            json!({
                "tabId": 7,
                "steps": [
                    {"tool":"find","args":{"query":"one"}},
                    {"tool":"find","args":{"tabId":8,"query":"two"}},
                    {"tool":"get_page_text","args":{}}
                ]
            }),
        )
        .expect("explicit step tab overrides retained tab");
        assert_eq!(flow_step_tab(&flow, 0), Some(7));
        assert_eq!(flow_step_tab(&flow, 1), Some(8));
        assert_eq!(flow_step_tab(&flow, 2), Some(8));
    }

    #[test]
    fn script_without_any_concrete_tab_fails_before_normalization() {
        assert_schema_rejected(
            "script",
            json!({"steps":[{"tool":"find","args":{"query":"Save"}}]}),
        );
    }

    #[test]
    fn computer_projects_only_fields_used_by_each_physical_action() {
        let screenshot = decode_call(
            "computer",
            json!({
                "action":"screenshot", "tabId":7, "coordinate":[1,2], "duration":2,
                "modifiers":"ctrl", "ref":"ref_1", "region":[0,0,10,10], "repeat":4,
                "scroll_direction":"up", "scroll_amount":2, "start_coordinate":[3,4],
                "text":"ignored"
            }),
        )
        .expect("screenshot");
        assert_eq!(screenshot.arguments, json!({"tab":7}));

        let type_text = decode_call(
            "computer",
            json!({
                "action":"type", "tabId":7, "text":"hello", "coordinate":[1,2],
                "ref":"ref_1", "repeat":4, "modifiers":"ctrl"
            }),
        )
        .expect("type");
        assert_eq!(type_text.arguments, json!({"tab":7,"text":"hello"}));

        let key = decode_call(
            "computer",
            json!({
                "action":"key", "tabId":7, "text":"Enter", "repeat":4,
                "coordinate":[1,2], "ref":"ref_1", "modifiers":"ctrl"
            }),
        )
        .expect("key");
        assert_eq!(key.arguments, json!({"tab":7,"key":"Enter","repeat":4}));

        let scroll = decode_call(
            "computer",
            json!({
                "action":"scroll", "tabId":7, "coordinate":[1,2], "ref":"ignored",
                "scroll_direction":"left", "scroll_amount":2, "modifiers":"shift"
            }),
        )
        .expect("scroll coordinate precedence");
        assert_eq!(
            scroll.arguments,
            json!({"tab":7,"point":[1,2],"direction":"left","amount":2,"modifiers":"shift"})
        );

        let targeted_scroll = decode_call(
            "computer",
            json!({"action":"scroll","tabId":7,"ref":"ref_1"}),
        )
        .expect("targeted scroll defaults");
        assert_eq!(
            targeted_scroll.arguments,
            json!({"tab":7,"target":{"ref":"ref_1"},"direction":"down","amount":3})
        );

        let scroll_to = decode_call(
            "computer",
            json!({"action":"scroll_to","tabId":7,"ref":"ref_1","coordinate":[1,2]}),
        )
        .expect("scroll_to ref precedence");
        assert_eq!(
            scroll_to.arguments,
            json!({"tab":7,"target":{"ref":"ref_1"}})
        );

        for duration in [None, Some(0)] {
            let mut call = json!({"action":"wait","tabId":7});
            if let Some(duration) = duration {
                call["duration"] = json!(duration);
            }
            let wait = decode_call("computer", call).expect("wait default");
            assert_eq!(wait.arguments, json!({"tab":7,"seconds":1}));
        }

        for invalid in [
            json!({"action":"left_click","tabId":7}),
            json!({"action":"zoom","tabId":7}),
            json!({"action":"left_click_drag","tabId":7,"coordinate":[1,2]}),
        ] {
            assert!(decode_call("computer", invalid).is_err());
        }
    }

    #[test]
    fn gif_and_upload_delivery_modes_are_exact_and_action_specific() {
        for action in ["start_recording", "stop_recording", "status", "clear"] {
            let operation = decode_call(
                "gif_creator",
                json!({
                    "action":action, "tabId":7, "coordinate":[1,2], "ref":"ref_1",
                    "download":true, "filename":"ignored.gif", "options":{"speed":2}
                }),
            )
            .expect("non-export action");
            assert_eq!(operation.arguments, json!({"tab":7}));
        }

        let exported = decode_call(
            "gif_creator",
            json!({
                "action":"export", "tabId":7, "ref":"ref_1",
                "filename":"capture.gif", "options":{"speed":2}
            }),
        )
        .expect("ref export");
        assert_eq!(
            exported.arguments,
            json!({
                "tab":7, "target":{"ref":"ref_1"}, "filename":"capture.gif",
                "options":{"speed":2}
            })
        );

        for invalid in [
            json!({"action":"export","tabId":7}),
            json!({"action":"export","tabId":7,"download":false}),
            json!({"action":"export","tabId":7,"ref":"ref_1","coordinate":[1,2]}),
            json!({"action":"export","tabId":7,"coordinate":[1]}),
        ] {
            assert!(decode_call("gif_creator", invalid).is_err());
        }

        assert!(decode_call(
            "upload_image",
            json!({"tabId":7,"imageId":"img_1","ref":"ref_1","coordinate":[1,2]})
        )
        .is_err());
        assert!(decode_call(
            "upload_image",
            json!({"tabId":7,"imageId":"img_1","coordinate":[1]})
        )
        .is_err());
    }

    #[test]
    fn semantic_projection_rejects_schema_valid_but_ambiguous_calls() {
        for (tool, arguments) in [
            ("form_input", json!({"tabId":7,"ref":"","value":"x"})),
            ("form_fill", json!({"tabId":7,"fields":{"":"x"}})),
            (
                "act_on",
                json!({"tabId":7,"action":"left_click","target":{"ref":"ref_1"},"value":"x"}),
            ),
            (
                "act_on",
                json!({"tabId":7,"action":"left_click","target":{"ref":"ref_1","query":"Save"}}),
            ),
            (
                "wait_for",
                json!({"tabId":7,"selector":"#save","text":"Saved"}),
            ),
            (
                "wait_for",
                json!({"tabId":7,"state":"settled","selector":"#save"}),
            ),
            ("wait_for", json!({"tabId":7,"timeout_ms":100,"min_ms":101})),
        ] {
            assert!(
                decode_call(tool, arguments).is_err(),
                "{tool} semantic ambiguity must fail"
            );
        }
    }

    #[test]
    fn script_preserves_strict_typed_references_but_validates_concrete_siblings() {
        let flow = decode_call(
            "script",
            json!({
                "steps":[
                    {"tool":"get_page_text","args":{"tabId":"$prev.tabId","max_chars":"$1.limit"}},
                    {"tool":"form_fill","args":{"tabId":7,"fields":"$prev.fields"}}
                ]
            }),
        )
        .expect("typed references remain deferred");
        assert_eq!(
            flow.arguments.pointer("/steps/0/arguments/tab"),
            Some(&json!("$prev.tabId"))
        );
        assert_eq!(
            flow.arguments.pointer("/steps/0/arguments/max_chars"),
            Some(&json!("$1.limit"))
        );
        assert_eq!(
            flow.arguments.pointer("/steps/1/arguments/fields"),
            Some(&json!("$prev.fields"))
        );

        assert!(decode_call(
            "script",
            json!({"steps":[{"tool":"get_page_text","args":{"tabId":"$prev.tabId","max_chars":true}}]})
        )
        .is_err());
        assert!(decode_call(
            "script",
            json!({"steps":[{"tool":"get_page_text","args":{"tabId":"$0.tabId"}}]})
        )
        .is_err());
        // ADR-0101 deliberately tightens composition: unknown inner tools fail closed.
        assert!(decode_call(
            "script",
            json!({"steps":[{"tool":"future_tool","args":{}}]})
        )
        .is_err());
    }

    fn canonical_flow_result() -> BrowserResult {
        let mut partial = BrowserResult::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
            BrowserResultStatus::Partial,
            OperationEffect::Committed,
        );
        partial.parts = vec![ResultPart::Text {
            text: "clicked; expectation timed out".into(),
        }];
        partial.data = json!({"interactionReceipt": {"blockers": [{"kind":"expect_timeout"}]}});

        let mut screenshot = BrowserResult::new(
            OperationId::BrowserScreenshot,
            IntentId::ScreenshotViewport,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        screenshot.parts = vec![
            ResultPart::Text {
                text: "captured".into(),
            },
            ResultPart::Image {
                data: "AAAA".into(),
                mime_type: "image/jpeg".into(),
            },
        ];
        screenshot.data = json!({"imageId":"img_1"});
        screenshot.provenance = Some(
            PageProvenance::new(
                vec![
                    "/data".into(),
                    "/parts/0/text".into(),
                    "/parts/1/data".into(),
                ],
                Some("https://example.com".into()),
                Some("00112233445566778899aabbccddeeff".into()),
                None,
            )
            .expect("canonical provenance"),
        );

        let mut denied = BrowserResult::new(
            OperationId::BrowserFind,
            IntentId::FindQuery,
            BrowserResultStatus::Blocked,
            OperationEffect::None,
        );
        denied.parts = vec![ResultPart::Text {
            text: "denied by policy".into(),
        }];
        let not_run = BrowserResult::new(
            OperationId::BrowserNavigate,
            IntentId::NavigateUrl,
            BrowserResultStatus::NotDispatched,
            OperationEffect::None,
        );

        let mut root = BrowserResult::new(
            OperationId::BrowserFlow,
            IntentId::FlowExecute,
            BrowserResultStatus::Partial,
            OperationEffect::Committed,
        );
        root.data = serde_json::to_value(FlowResultData {
            steps: vec![
                FlowStepResult {
                    step: 1,
                    status: FlowStepStatus::Ok,
                    result: partial,
                },
                FlowStepResult {
                    step: 2,
                    status: FlowStepStatus::Ok,
                    result: screenshot,
                },
                FlowStepResult {
                    step: 3,
                    status: FlowStepStatus::Denied,
                    result: denied,
                },
                FlowStepResult {
                    step: 4,
                    status: FlowStepStatus::NotRun,
                    result: not_run,
                },
            ],
            summary: "2/4 steps completed; step 3 denied".into(),
            duration_ms: 17,
            termination: FlowTermination {
                reason: FlowTerminationReason::Denied,
                step: Some(3),
            },
        })
        .expect("flow data serializes");
        root
    }

    #[test]
    fn one_canonical_flow_renders_exact_script_and_batch_shapes() {
        let result = canonical_flow_result();
        let canonical_wire = serde_json::to_string(&result).expect("canonical result serializes");
        for external_label in ["act_on", "computer"] {
            assert!(!canonical_wire.contains(external_label));
        }
        let hints = FlowRenderHints {
            steps: vec![
                FlowStepRenderHint {
                    label: "act_on".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserAct,
                        IntentId::ActClick,
                    ),
                },
                FlowStepRenderHint {
                    label: "computer".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserScreenshot,
                        IntentId::ScreenshotViewport,
                    ),
                },
                FlowStepRenderHint {
                    label: "find".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserFind,
                        IntentId::FindQuery,
                    ),
                },
                FlowStepRenderHint {
                    label: "navigate".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserNavigate,
                        IntentId::NavigateUrl,
                    ),
                },
            ],
        };
        let script_presentation =
            InvocationPresentation::new(PROFILE_ID, PROFILE_VERSION, "script", None)
                .expect("script presentation");
        let batch_presentation =
            InvocationPresentation::new(PROFILE_ID, PROFILE_VERSION, "browser_batch", None)
                .expect("batch presentation");

        let script = encode_result(result.clone(), Some(&script_presentation), Some(&hints))
            .expect("script render");
        assert_eq!(
            script["structuredContent"]["results"]
                .as_array()
                .expect("results")
                .iter()
                .map(|entry| entry["status"].as_str().expect("status"))
                .collect::<Vec<_>>(),
            vec!["ok", "ok", "denied", "not_run"]
        );
        assert_eq!(
            script["structuredContent"]["results"][0]["result"],
            "clicked; expectation timed out"
        );
        assert_eq!(
            script.pointer("/structuredContent/results/1/structured/provenance/topOrigin"),
            Some(&json!("https://example.com"))
        );
        assert!(script.get("isError").is_none());

        let batch =
            encode_result(result, Some(&batch_presentation), Some(&hints)).expect("batch render");
        assert_eq!(
            batch["content"],
            json!([
                {"type":"text","text":"clicked; expectation timed out"},
                {"type":"text","text":"captured"},
                {"type":"image","data":"AAAA","mimeType":"image/jpeg"},
                {"type":"text","text":"step 3 (find) denied: denied by policy"},
                {"type":"text","text":"2/4 steps completed; step 3 denied"}
            ])
        );
        assert!(batch.get("structuredContent").is_none());
    }

    #[test]
    fn executed_cancellation_tail_is_canonical_but_omitted_from_legacy_script() {
        let mut cancelled = BrowserResult::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
            BrowserResultStatus::Cancelled,
            OperationEffect::None,
        );
        cancelled.parts = vec![ResultPart::Text {
            text: "cancelled before dispatch".into(),
        }];
        let not_run = BrowserResult::new(
            OperationId::BrowserFind,
            IntentId::FindQuery,
            BrowserResultStatus::NotDispatched,
            OperationEffect::None,
        );
        let mut root = BrowserResult::new(
            OperationId::BrowserFlow,
            IntentId::FlowExecute,
            BrowserResultStatus::Cancelled,
            OperationEffect::None,
        );
        root.data = serde_json::to_value(FlowResultData {
            steps: vec![
                FlowStepResult {
                    step: 1,
                    status: FlowStepStatus::Cancelled,
                    result: cancelled,
                },
                FlowStepResult {
                    step: 2,
                    status: FlowStepStatus::NotRun,
                    result: not_run,
                },
            ],
            summary: "0/2 steps completed; cancelled before step 1".into(),
            duration_ms: 1,
            termination: FlowTermination {
                reason: FlowTerminationReason::Cancelled,
                step: Some(1),
            },
        })
        .expect("flow data serializes");
        let hints = FlowRenderHints {
            steps: vec![
                FlowStepRenderHint {
                    label: "act_on".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserAct,
                        IntentId::ActClick,
                    ),
                },
                FlowStepRenderHint {
                    label: "find".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserFind,
                        IntentId::FindQuery,
                    ),
                },
            ],
        };
        let presentation = InvocationPresentation::new(PROFILE_ID, PROFILE_VERSION, "script", None)
            .expect("presentation");

        let rendered = encode_result(root, Some(&presentation), Some(&hints)).expect("render");
        let results = rendered["structuredContent"]["results"]
            .as_array()
            .expect("results");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["status"], "cancelled");
    }

    #[test]
    fn outcome_unknown_flow_never_reaches_an_ordinary_legacy_renderer() {
        let mut result = canonical_flow_result();
        result.status = BrowserResultStatus::OutcomeUnknown;
        result.effect = OperationEffect::Unknown;
        result.retry = Some(ghostlight_transport::operation::RetryDisposition::Unsafe);
        let presentation = InvocationPresentation::new(PROFILE_ID, PROFILE_VERSION, "script", None)
            .expect("presentation");
        let hints = FlowRenderHints {
            steps: vec![
                FlowStepRenderHint {
                    label: "act_on".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserAct,
                        IntentId::ActClick,
                    ),
                },
                FlowStepRenderHint {
                    label: "computer".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserScreenshot,
                        IntentId::ScreenshotViewport,
                    ),
                },
                FlowStepRenderHint {
                    label: "find".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserFind,
                        IntentId::FindQuery,
                    ),
                },
                FlowStepRenderHint {
                    label: "navigate".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserNavigate,
                        IntentId::NavigateUrl,
                    ),
                },
            ],
        };
        assert_eq!(
            encode_result(result, Some(&presentation), Some(&hints)),
            Err(EncodeError::UnsupportedStatus(
                BrowserResultStatus::OutcomeUnknown
            ))
        );
    }

    #[test]
    fn script_compact_result_retains_the_legacy_twenty_five_k_cap() {
        let mut steps = Vec::new();
        let mut hints = Vec::new();
        for index in 1..=20_u32 {
            let mut step_result = BrowserResult::new(
                OperationId::BrowserRead,
                IntentId::ReadText,
                BrowserResultStatus::Ok,
                OperationEffect::None,
            );
            step_result.parts = vec![ResultPart::Text {
                text: "x".repeat(3_000),
            }];
            steps.push(FlowStepResult {
                step: index,
                status: FlowStepStatus::Ok,
                result: step_result,
            });
            hints.push(FlowStepRenderHint {
                label: "get_page_text".into(),
                expected_operation: OperationKey::new(OperationId::BrowserRead, IntentId::ReadText),
            });
        }
        steps[0].result.data = json!({"large": "z".repeat(40_000)});
        steps[19].status = FlowStepStatus::Denied;
        steps[19].result.status = BrowserResultStatus::Blocked;
        let mut result = BrowserResult::new(
            OperationId::BrowserFlow,
            IntentId::FlowExecute,
            BrowserResultStatus::Blocked,
            OperationEffect::None,
        );
        result.data = serde_json::to_value(FlowResultData {
            steps,
            summary: "20/20 steps completed".into(),
            duration_ms: 1,
            termination: FlowTermination {
                reason: FlowTerminationReason::Denied,
                step: Some(20),
            },
        })
        .expect("flow data serializes");
        let hints = FlowRenderHints { steps: hints };
        let presentation = InvocationPresentation::new(PROFILE_ID, PROFILE_VERSION, "script", None)
            .expect("presentation");

        let encoded = encode_result(result, Some(&presentation), Some(&hints)).expect("render");
        assert!(
            serde_json::to_string(&encoded["structuredContent"])
                .expect("compact serializes")
                .len()
                <= COMPACT_BUDGET
        );
        assert!(encoded["structuredContent"]
            .to_string()
            .contains("truncated"));
        assert!(encoded["structuredContent"]["results"]
            .as_array()
            .expect("results")
            .iter()
            .any(|entry| entry["status"] == "denied"));
    }

    #[test]
    fn flow_renderer_fails_closed_without_matching_edge_labels() {
        let result = canonical_flow_result();
        let presentation = InvocationPresentation::new(PROFILE_ID, PROFILE_VERSION, "script", None)
            .expect("presentation");
        assert_eq!(
            encode_result(result.clone(), Some(&presentation), None),
            Err(EncodeError::MissingFlowRenderHints)
        );
        assert_eq!(
            encode_result(
                result.clone(),
                Some(&presentation),
                Some(&FlowRenderHints {
                    steps: vec![FlowStepRenderHint {
                        label: "find".into(),
                        expected_operation: OperationKey::new(
                            OperationId::BrowserFind,
                            IntentId::FindQuery,
                        ),
                    }]
                })
            ),
            Err(EncodeError::FlowRenderHintCount {
                expected: 4,
                actual: 1
            })
        );
        let wrong_identity = FlowRenderHints {
            steps: vec![
                FlowStepRenderHint {
                    label: "act_on".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserFind,
                        IntentId::FindQuery,
                    ),
                },
                FlowStepRenderHint {
                    label: "computer".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserScreenshot,
                        IntentId::ScreenshotViewport,
                    ),
                },
                FlowStepRenderHint {
                    label: "find".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserFind,
                        IntentId::FindQuery,
                    ),
                },
                FlowStepRenderHint {
                    label: "navigate".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserNavigate,
                        IntentId::NavigateUrl,
                    ),
                },
            ],
        };
        assert_eq!(
            encode_result(result, Some(&presentation), Some(&wrong_identity)),
            Err(EncodeError::FlowStepIdentityMismatch { step: 1 })
        );
    }

    #[test]
    fn flow_renderer_rejects_invalid_nested_images_for_both_legacy_surfaces() {
        let mut result = canonical_flow_result();
        *result
            .data
            .pointer_mut("/steps/1/result/parts/1/data")
            .expect("screenshot image data") = json!("AAAA=");
        let hints = FlowRenderHints {
            steps: vec![
                FlowStepRenderHint {
                    label: "act_on".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserAct,
                        IntentId::ActClick,
                    ),
                },
                FlowStepRenderHint {
                    label: "computer".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserScreenshot,
                        IntentId::ScreenshotViewport,
                    ),
                },
                FlowStepRenderHint {
                    label: "find".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserFind,
                        IntentId::FindQuery,
                    ),
                },
                FlowStepRenderHint {
                    label: "navigate".into(),
                    expected_operation: OperationKey::new(
                        OperationId::BrowserNavigate,
                        IntentId::NavigateUrl,
                    ),
                },
            ],
        };

        for external_tool in ["script", "browser_batch"] {
            let presentation =
                InvocationPresentation::new(PROFILE_ID, PROFILE_VERSION, external_tool, None)
                    .expect("flow presentation");
            assert_eq!(
                encode_result(result.clone(), Some(&presentation), Some(&hints)),
                Err(EncodeError::MalformedFlowData(
                    "data is not a typed flow result"
                ))
            );
        }
    }

    #[test]
    fn result_renderer_reconstructs_legacy_blocks_without_canonical_envelope_fields() {
        let presentation = InvocationPresentation::new(
            PROFILE_ID,
            PROFILE_VERSION,
            "computer",
            Some("screenshot".into()),
        )
        .expect("presentation");
        let mut result = BrowserResult::new(
            OperationId::BrowserScreenshot,
            IntentId::ScreenshotViewport,
            BrowserResultStatus::Ok,
            ghostlight_transport::operation::OperationEffect::None,
        );
        result.parts = vec![
            ResultPart::Text {
                text: "captured".into(),
            },
            ResultPart::Image {
                data: "AAAA".into(),
                mime_type: "image/jpeg".into(),
            },
        ];
        result.data = json!({"imageId":"img_1"});
        let encoded = encode_result(result, Some(&presentation), None).expect("encode");
        assert_eq!(encoded["content"][1]["mimeType"], "image/jpeg");
        assert_eq!(encoded["structuredContent"]["imageId"], "img_1");
        let text = encoded.to_string();
        assert!(!text.contains("ghostlight.browser.result"));
        assert!(!text.contains("operation"));
    }

    #[test]
    fn result_renderer_rejects_invalid_top_level_image_parts() {
        for (part, expected) in [
            (
                ResultPart::Image {
                    data: "AAAA=".into(),
                    mime_type: "image/png".into(),
                },
                ResultPartError::InvalidImageData,
            ),
            (
                ResultPart::Image {
                    data: "AAAA".into(),
                    mime_type: "image/*".into(),
                },
                ResultPartError::InvalidImageMimeType,
            ),
        ] {
            let mut result = BrowserResult::new(
                OperationId::BrowserScreenshot,
                IntentId::ScreenshotViewport,
                BrowserResultStatus::Ok,
                OperationEffect::None,
            );
            result.parts = vec![part];
            assert_eq!(
                encode_result(result, None, None),
                Err(EncodeError::InvalidResultPart(expected))
            );
        }
    }

    #[test]
    fn result_renderer_reconstructs_receipt_provenance_and_preserves_boundary_text() {
        let boundary = "--- GHOSTLIGHT PAGE CONTENT 00112233445566778899aabbccddeeff origin=https://example.com UNTRUSTED ---\nPrivate receipt facts\n--- END GHOSTLIGHT PAGE CONTENT 00112233445566778899aabbccddeeff ---";
        let mut result = BrowserResult::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
            BrowserResultStatus::Ok,
            ghostlight_transport::operation::OperationEffect::Committed,
        );
        result.parts = vec![ResultPart::Text {
            text: boundary.into(),
        }];
        result.data = json!({
            "interactionReceipt": {
                "action": "left_click",
                "target": {"frameOrigin": "https://frame.example"}
            },
            "serviceFact": "retained"
        });
        result.provenance = Some(
            PageProvenance::new(
                vec!["/data/interactionReceipt".into(), "/parts/0/text".into()],
                Some("https://example.com".into()),
                Some("00112233445566778899aabbccddeeff".into()),
                Some("https://frame.example".into()),
            )
            .expect("canonical provenance"),
        );
        let wire = serde_json::to_value(&result).expect("canonical wire");
        let result = serde_json::from_value(wire).expect("canonical round trip");

        let encoded = encode_result(result, None, None).expect("legacy render");
        assert_eq!(encoded["content"][0]["text"], boundary);
        assert_eq!(
            encoded.pointer("/structuredContent/interactionReceipt/provenance"),
            Some(&json!({
                "pageSourced": true,
                "untrusted": true,
                "topOrigin": "https://example.com",
                "frameOrigin": "https://frame.example",
                "sessionNonce": "00112233445566778899aabbccddeeff"
            }))
        );
        assert!(encoded.pointer("/structuredContent/provenance").is_none());
        assert_eq!(encoded["structuredContent"]["serviceFact"], "retained");
    }

    #[test]
    fn result_renderer_reconstructs_root_provenance_without_a_receipt() {
        let mut result = BrowserResult::new(
            OperationId::BrowserRead,
            IntentId::ReadText,
            BrowserResultStatus::Ok,
            ghostlight_transport::operation::OperationEffect::None,
        );
        result.data = json!({"url": "https://example.com/private"});
        result.provenance = Some(
            PageProvenance::new(
                vec!["/data".into()],
                Some("https://example.com".into()),
                Some("00112233445566778899aabb".into()),
                None,
            )
            .expect("canonical provenance"),
        );

        let encoded = encode_result(result, None, None).expect("legacy render");
        assert_eq!(
            encoded.pointer("/structuredContent/provenance/pageSourced"),
            Some(&Value::Bool(true))
        );
        assert!(encoded
            .pointer("/structuredContent/interactionReceipt/provenance")
            .is_none());
    }

    #[test]
    fn result_renderer_rejects_incomplete_or_conflicting_provenance() {
        let mut incomplete = BrowserResult::new(
            OperationId::BrowserRead,
            IntentId::ReadText,
            BrowserResultStatus::Ok,
            ghostlight_transport::operation::OperationEffect::None,
        );
        incomplete.data = json!({});
        incomplete.provenance = Some(
            PageProvenance::new(vec!["/data".into()], None, None, None)
                .expect("canonical provenance permits optional legacy facts"),
        );
        assert!(matches!(
            encode_result(incomplete, None, None),
            Err(EncodeError::MalformedProvenance(_))
        ));

        let mut conflicting = BrowserResult::new(
            OperationId::BrowserRead,
            IntentId::ReadText,
            BrowserResultStatus::Ok,
            ghostlight_transport::operation::OperationEffect::None,
        );
        conflicting.data = json!({"provenance": {"pageControlled": true}});
        conflicting.provenance = Some(
            PageProvenance::new(
                vec!["/data".into()],
                Some("https://example.com".into()),
                Some("00112233445566778899aabb".into()),
                None,
            )
            .expect("canonical provenance"),
        );
        assert!(matches!(
            encode_result(conflicting, None, None),
            Err(EncodeError::MalformedProvenance(_))
        ));
    }

    #[test]
    fn result_renderer_encodes_blocked_as_a_legacy_error_result() {
        let mut result = BrowserResult::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
            BrowserResultStatus::Blocked,
            ghostlight_transport::operation::OperationEffect::None,
        );
        result.parts = vec![ResultPart::Text {
            text: "blocked before dispatch".into(),
        }];

        let encoded =
            encode_result(result, None, None).expect("blocked has a faithful legacy shape");
        assert_eq!(encoded["isError"], true);
        assert_eq!(encoded["content"][0]["text"], "blocked before dispatch");
    }

    #[test]
    fn result_renderer_fails_closed_for_statuses_without_a_legacy_success_shape() {
        for status in [
            BrowserResultStatus::NotMet,
            BrowserResultStatus::Held,
            BrowserResultStatus::AttentionRequired,
            BrowserResultStatus::Cancelled,
            BrowserResultStatus::NotDispatched,
            BrowserResultStatus::OutcomeUnknown,
            BrowserResultStatus::Unavailable,
        ] {
            let result = BrowserResult::new(
                OperationId::BrowserWait,
                IntentId::WaitUntil,
                status,
                ghostlight_transport::operation::OperationEffect::None,
            );
            let expected = if status == BrowserResultStatus::OutcomeUnknown {
                Err(EncodeError::InvalidResultDisposition(
                    BrowserResultValidationError::InvalidOutcomeUnknown,
                ))
            } else {
                Err(EncodeError::UnsupportedStatus(status))
            };
            assert_eq!(encode_result(result, None, None), expected);
        }
    }
}
