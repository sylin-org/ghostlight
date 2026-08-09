// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Exact MCP `2026-07-28` request-stateless behavior and envelope rendering.
//!
//! Protocol metadata is parsed independently from every request. Workspace continuity is explicit
//! in tool arguments and is removed at this shore before a protocol-neutral operation crosses the
//! service bridge.

use crate::bridge::{
    Correlation, DisconnectedPending, Effects, PendingKind, PendingRequest, Resolution,
};
use crate::jsonrpc::{
    error_response, notification, success_response, Request, RequestId, INVALID_PARAMS,
    INVALID_REQUEST, METHOD_NOT_FOUND,
};
use crate::surface::{self, ghostlight, McpRevision};
use ghostlight_transport::bridge::{
    BridgeError, BridgeErrorKind, CatalogProjection, ClientPresentation, EdgeMessage,
    RequestContext, TerminalOutcome, WorkspaceId,
};
use ghostlight_transport::instance::Instance;
use ghostlight_transport::operation::{BrowserResult, OperationEffect};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// The exact protocol revision implemented by this shore.
pub const PROTOCOL_VERSION: &str = "2026-07-28";
/// Required per-request protocol metadata key.
pub const PROTOCOL_VERSION_META: &str = "io.modelcontextprotocol/protocolVersion";
/// Required per-request client-capabilities metadata key.
pub const CLIENT_CAPABILITIES_META: &str = "io.modelcontextprotocol/clientCapabilities";
/// Optional per-request client presentation metadata key.
pub const CLIENT_INFO_META: &str = "io.modelcontextprotocol/clientInfo";
/// Optional per-request log-level metadata key.
pub const LOG_LEVEL_META: &str = "io.modelcontextprotocol/logLevel";
/// Result metadata key identifying the responding server.
pub const SERVER_INFO_META: &str = "io.modelcontextprotocol/serverInfo";
/// Subscription stream correlation metadata key.
pub const SUBSCRIPTION_ID_META: &str = "io.modelcontextprotocol/subscriptionId";
const RESTRICTION_META: &str = "org.sylin/ghostlightSessionPolicy";
const COMPAT_RESTRICTION_META: &str = "ghostlightSessionPolicy";
const CATALOG_GENERATION_META: &str = "org.sylin/ghostlightCatalogGeneration";
const UNRESTRICTED_CATALOG_TTL_MS: u64 = 30_000;
const DISCOVERY_TTL_MS: u64 = 300_000;
const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;
// MCP 2026-07-28 reserves -32000 through -32099. Product-local errors live outside that range.
const SERVICE_UNAVAILABLE: i64 = -33001;
const OUTCOME_UNKNOWN: i64 = -33003;

#[derive(Clone, Copy)]
struct Subscription {
    tools_list_changed: bool,
}

/// One selected MCP `2026-07-28` state machine.
#[derive(Default)]
pub struct Handler {
    subscriptions: HashMap<RequestId, Subscription>,
}

impl Handler {
    /// Construct the request-stateless handler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle one request or cancellation notification.
    pub fn handle(&mut self, request: &Request, correlation: &mut Correlation) -> Effects {
        if request.method == "notifications/cancelled" {
            if request.id.is_some() {
                return Effects::output(error_response(
                    request.id.as_ref(),
                    INVALID_REQUEST,
                    "notifications/cancelled must not carry an id",
                    None,
                ));
            }
            return self.cancellation(request, correlation);
        }
        if request.id.is_none() {
            return Effects::default();
        }
        let id = request.id.as_ref().expect("checked above");
        if !id.is_2026_07_28_legal() {
            return Effects::output(error_response(
                Some(id),
                INVALID_REQUEST,
                "MCP 2026-07-28 request ids must be non-null strings or integers",
                None,
            ));
        }
        let context = match request_context(request) {
            Ok(context) => context,
            Err(response) => return Effects::output(response),
        };
        if self.subscriptions.contains_key(id) || correlation.contains_request(id) {
            return Effects::output(error_response(
                Some(id),
                INVALID_REQUEST,
                "a request with this id is already active",
                None,
            ));
        }

        match request.method.as_str() {
            "server/discover" => discover(request),
            "tools/list" => self.tools_list(request, context, correlation),
            "tools/call" => self.tools_call(request, context, correlation),
            "subscriptions/listen" => self.listen(request),
            other => Effects::output(error_response(
                Some(id),
                METHOD_NOT_FOUND,
                format!("Method not found: {other}"),
                None,
            )),
        }
    }

    /// Resume this handler after one correlated service operation resolves.
    pub fn on_resolution(&mut self, resolution: Resolution) -> Effects {
        match resolution {
            Resolution::Catalog {
                pending,
                projection,
            } if pending.kind == PendingKind::ListTools2026 => {
                if pending.suppressed {
                    return Effects::default();
                }
                pending.request_id.map_or_else(Effects::default, |id| {
                    Effects::output(success_response(&id, list_tools_result(&projection)))
                })
            }
            Resolution::Completed { pending, outcome }
                if matches!(pending.kind, PendingKind::CallTool2026 { .. }) =>
            {
                render_outcome(pending, outcome)
            }
            Resolution::Rejected { pending, error } => render_rejection(pending, error),
            Resolution::WorkspaceOpened { .. }
            | Resolution::WorkspaceReleased
            | Resolution::Catalog { .. }
            | Resolution::Completed { .. } => Effects::default(),
        }
    }

    /// Deliver catalog changes only to listen streams that explicitly subscribed.
    pub fn catalog_changed(&self, _generation: u64) -> Effects {
        let mut effects = Effects::default();
        for (subscription_id, subscription) in &self.subscriptions {
            if !subscription.tools_list_changed {
                continue;
            }
            effects.output.push(notification(
                "notifications/tools/list_changed",
                Some(json!({
                    "_meta": {
                        SUBSCRIPTION_ID_META: subscription_id.to_value(),
                    }
                })),
            ));
        }
        effects
    }

    /// Render a truthful failure for an operation retired on disconnect or write failure.
    pub fn bridge_failure(&self, disconnected: DisconnectedPending, reason: &str) -> Effects {
        let pending = disconnected.pending;
        if pending.suppressed {
            return Effects::default();
        }
        let Some(id) = pending.request_id else {
            return Effects::default();
        };
        let (code, message, disposition) = if disconnected.may_have_started {
            (
                OUTCOME_UNKNOWN,
                "Ghostlight lost the service connection after this operation may have started; it was not replayed",
                "outcome_unknown",
            )
        } else {
            (
                SERVICE_UNAVAILABLE,
                "Ghostlight could not deliver this operation to its local service",
                "not_dispatched",
            )
        };
        Effects::output(error_response(
            Some(&id),
            code,
            message,
            Some(json!({"detail": reason, "disposition": disposition})),
        ))
    }

    /// Gracefully close every long-lived subscription at stdio shutdown.
    pub fn shutdown(&mut self) -> Effects {
        let mut effects = Effects::default();
        for (id, _) in self.subscriptions.drain() {
            effects.output.push(success_response(
                &id,
                complete_result_with_meta(json!({}), Some((SUBSCRIPTION_ID_META, id.to_value()))),
            ));
        }
        effects
    }

    fn tools_list(
        &self,
        request: &Request,
        context: RequestContext,
        correlation: &mut Correlation,
    ) -> Effects {
        let id = request.id.clone().expect("validated request id");
        if request
            .params
            .as_object()
            .and_then(|params| params.get("cursor"))
            .is_some()
        {
            return Effects::output(error_response(
                Some(&id),
                INVALID_PARAMS,
                "Ghostlight did not issue this pagination cursor",
                None,
            ));
        }
        let sequence = match correlation.track(PendingRequest::request(
            id.clone(),
            PendingKind::ListTools2026,
        )) {
            Ok(sequence) => sequence,
            Err(message) => {
                return Effects::output(error_response(Some(&id), INVALID_REQUEST, message, None));
            }
        };
        Effects::service(EdgeMessage::Catalog {
            sequence,
            workspace: None,
            context,
        })
    }

    fn tools_call(
        &self,
        request: &Request,
        context: RequestContext,
        correlation: &mut Correlation,
    ) -> Effects {
        let id = request.id.clone().expect("validated request id");
        let Some(params) = request.params.as_object() else {
            return Effects::output(error_response(
                Some(&id),
                INVALID_PARAMS,
                "tools/call params must be an object",
                None,
            ));
        };
        if params.contains_key("requestState") || params.contains_key("inputResponses") {
            return Effects::output(error_response(
                Some(&id),
                INVALID_PARAMS,
                "Ghostlight did not issue a multi-round-trip continuation for this tool call",
                None,
            ));
        }
        let Some(external_tool) = params.get("name").and_then(Value::as_str) else {
            return Effects::output(error_response(
                Some(&id),
                INVALID_PARAMS,
                "tools/call requires a string name",
                None,
            ));
        };
        let arguments = match params.get("arguments") {
            None => Map::new(),
            Some(Value::Object(arguments)) => arguments.clone(),
            Some(_) => {
                return Effects::output(error_response(
                    Some(&id),
                    INVALID_PARAMS,
                    "tools/call arguments must be an object",
                    None,
                ));
            }
        };
        let workspace_field = "workspace";
        let workspace_value = arguments.get(workspace_field).cloned();
        let workspace = match workspace_value {
            None => None,
            Some(Value::String(raw)) => match WorkspaceId::parse(&raw) {
                Some(workspace) => Some(workspace),
                None => {
                    return Effects::output(error_response(
                        Some(&id),
                        INVALID_PARAMS,
                        format!("{workspace_field} is not a valid Ghostlight workspace handle; call a context-creating tab tool to obtain a new one"),
                        None,
                    ));
                }
            },
            Some(_) => {
                return Effects::output(error_response(
                    Some(&id),
                    INVALID_PARAMS,
                    format!("{workspace_field} must be a string when supplied"),
                    None,
                ));
            }
        };
        let arguments = Value::Object(arguments);
        let operation = match surface::decode_call(
            McpRevision::Mcp2026_07_28,
            external_tool,
            arguments.clone(),
        ) {
            Ok(operation) => operation,
            Err(error) => {
                return Effects::output(success_response(
                    &id,
                    complete_tool_error(None, &error.to_string()),
                ));
            }
        };
        let sequence = match correlation.track(PendingRequest::tool_request(
            id.clone(),
            PendingKind::CallTool2026 {
                context_creating: false,
            },
            operation.kind(),
            workspace.clone(),
        )) {
            Ok(sequence) => sequence,
            Err(message) => {
                return Effects::output(error_response(Some(&id), INVALID_REQUEST, message, None));
            }
        };
        Effects::service(EdgeMessage::Start {
            sequence,
            operation,
            workspace,
            context,
        })
    }

    fn listen(&mut self, request: &Request) -> Effects {
        let id = request.id.clone().expect("validated request id");
        let Some(notifications) = request
            .params
            .as_object()
            .and_then(|params| params.get("notifications"))
            .and_then(Value::as_object)
        else {
            return Effects::output(error_response(
                Some(&id),
                INVALID_PARAMS,
                "subscriptions/listen requires a notifications object",
                None,
            ));
        };
        let tools_list_changed = match notifications.get("toolsListChanged") {
            None | Some(Value::Bool(false)) => false,
            Some(Value::Bool(true)) => true,
            Some(_) => {
                return Effects::output(error_response(
                    Some(&id),
                    INVALID_PARAMS,
                    "notifications.toolsListChanged must be a boolean",
                    None,
                ));
            }
        };
        for field in ["promptsListChanged", "resourcesListChanged"] {
            if notifications
                .get(field)
                .is_some_and(|value| !value.is_boolean())
            {
                return Effects::output(error_response(
                    Some(&id),
                    INVALID_PARAMS,
                    format!("notifications.{field} must be a boolean"),
                    None,
                ));
            }
        }
        if let Some(resource_subscriptions) = notifications.get("resourceSubscriptions") {
            let legal = resource_subscriptions
                .as_array()
                .is_some_and(|values| values.iter().all(Value::is_string));
            if !legal {
                return Effects::output(error_response(
                    Some(&id),
                    INVALID_PARAMS,
                    "notifications.resourceSubscriptions must be an array of strings",
                    None,
                ));
            }
        }
        self.subscriptions
            .insert(id.clone(), Subscription { tools_list_changed });

        let mut accepted = Map::new();
        if tools_list_changed {
            accepted.insert("toolsListChanged".into(), Value::Bool(true));
        }
        Effects::output(notification(
            "notifications/subscriptions/acknowledged",
            Some(json!({
                "_meta": {
                    SUBSCRIPTION_ID_META: id.to_value(),
                },
                "notifications": accepted,
            })),
        ))
    }

    fn cancellation(&mut self, request: &Request, correlation: &mut Correlation) -> Effects {
        let request_id = request
            .params
            .as_object()
            .and_then(|params| params.get("requestId"))
            .and_then(RequestId::parse);
        let Some(request_id) = request_id else {
            return Effects::default();
        };
        if self.subscriptions.remove(&request_id).is_some() {
            return Effects::default();
        }
        correlation
            .cancel(&request_id)
            .map_or_else(Effects::default, Effects::service)
    }
}

/// Whether a request is a legal first non-discovery request for selecting this handler.
pub fn valid_selector(request: &Request) -> bool {
    request
        .id
        .as_ref()
        .is_some_and(RequestId::is_2026_07_28_legal)
        && request_context(request).is_ok()
}

/// Render the exact metadata error for a failed 2026 selection attempt.
pub fn selector_error(request: &Request) -> Effects {
    match request_context(request) {
        Err(response) => Effects::output(response),
        Ok(_) => Effects::output(error_response(
            request.id.as_ref(),
            INVALID_REQUEST,
            "this request cannot select MCP 2026-07-28",
            None,
        )),
    }
}

/// Render the unbinding `server/discover` compatibility probe.
pub fn discover(request: &Request) -> Effects {
    let Some(id) = request.id.as_ref() else {
        return Effects::default();
    };
    if !id.is_2026_07_28_legal() {
        return Effects::output(error_response(
            Some(id),
            INVALID_REQUEST,
            "server/discover requires a non-null string or integer id",
            None,
        ));
    }
    if let Err(response) = request_context(request) {
        return Effects::output(response);
    }
    Effects::output(success_response(
        id,
        complete_result_with_meta(
            json!({
                "supportedVersions": [
                    PROTOCOL_VERSION,
                    crate::mcp_2025_11_25::PROTOCOL_VERSION,
                ],
                "capabilities": {"tools": {"listChanged": true}},
                "instructions": format!(
                    "Use Ghostlight tools to automate the user's governed Chromium workspace. {}",
                    crate::TRANSPORT_CLOSED_RECOVERY_INSTRUCTIONS
                ),
                "ttlMs": DISCOVERY_TTL_MS,
                "cacheScope": "public",
            }),
            None,
        ),
    ))
}

fn request_context(request: &Request) -> Result<RequestContext, Value> {
    let id = request.id.as_ref();
    let Some(params) = request.params.as_object() else {
        return Err(error_response(
            id,
            INVALID_PARAMS,
            "MCP 2026-07-28 params must be an object",
            None,
        ));
    };
    let Some(meta) = params.get("_meta").and_then(Value::as_object) else {
        return Err(error_response(
            id,
            INVALID_PARAMS,
            "MCP 2026-07-28 requires a params._meta object on every request",
            None,
        ));
    };
    match meta.get(PROTOCOL_VERSION_META) {
        Some(Value::String(version)) if version == PROTOCOL_VERSION => {}
        Some(Value::String(version)) => {
            return Err(error_response(
                id,
                UNSUPPORTED_PROTOCOL_VERSION,
                "Unsupported protocol version",
                Some(json!({
                    "requested": version,
                    "supported": [PROTOCOL_VERSION],
                })),
            ));
        }
        Some(_) => {
            return Err(error_response(
                id,
                INVALID_PARAMS,
                format!("{PROTOCOL_VERSION_META} must be a string"),
                None,
            ));
        }
        None => {
            return Err(error_response(
                id,
                INVALID_PARAMS,
                format!("{PROTOCOL_VERSION_META} is required"),
                None,
            ));
        }
    }
    if !meta
        .get(CLIENT_CAPABILITIES_META)
        .is_some_and(Value::is_object)
    {
        return Err(error_response(
            id,
            INVALID_PARAMS,
            format!("{CLIENT_CAPABILITIES_META} must be an object"),
            None,
        ));
    }
    if let Some(log_level) = meta.get(LOG_LEVEL_META) {
        let legal = matches!(
            log_level.as_str(),
            Some(
                "debug"
                    | "info"
                    | "notice"
                    | "warning"
                    | "error"
                    | "critical"
                    | "alert"
                    | "emergency"
            )
        );
        if !legal {
            return Err(error_response(
                id,
                INVALID_PARAMS,
                format!("{LOG_LEVEL_META} is not a valid MCP logging level"),
                None,
            ));
        }
    }
    let client = match meta.get(CLIENT_INFO_META) {
        None => None,
        Some(Value::Object(info)) => {
            let Some(name) = info.get("name").and_then(Value::as_str) else {
                return Err(error_response(
                    id,
                    INVALID_PARAMS,
                    format!("{CLIENT_INFO_META}.name must be a string"),
                    None,
                ));
            };
            let Some(version) = info.get("version").and_then(Value::as_str) else {
                return Err(error_response(
                    id,
                    INVALID_PARAMS,
                    format!("{CLIENT_INFO_META}.version must be a string"),
                    None,
                ));
            };
            Some(ClientPresentation {
                name: name.to_owned(),
                version: version.to_owned(),
            })
        }
        Some(_) => {
            return Err(error_response(
                id,
                INVALID_PARAMS,
                format!("{CLIENT_INFO_META} must be an object"),
                None,
            ));
        }
    };
    let restriction = match meta
        .get(RESTRICTION_META)
        .or_else(|| meta.get(COMPAT_RESTRICTION_META))
    {
        None => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => {
            return Err(error_response(
                id,
                INVALID_PARAMS,
                "ghostlightSessionPolicy must be a string",
                None,
            ));
        }
    };
    Ok(RequestContext {
        client,
        restriction,
    })
}

fn list_tools_result(projection: &CatalogProjection) -> Value {
    let tools = ghostlight::filtered_declarations(McpRevision::Mcp2026_07_28, projection);
    let ttl_ms = if projection.restricted {
        0
    } else {
        UNRESTRICTED_CATALOG_TTL_MS
    };
    complete_result_with_meta(
        json!({
            "tools": tools,
            "ttlMs": ttl_ms,
            // Catalogs can reflect local authority even without an explicit per-request overlay.
            "cacheScope": "private",
        }),
        Some((CATALOG_GENERATION_META, json!(projection.generation))),
    )
}

fn render_outcome(pending: PendingRequest, outcome: TerminalOutcome) -> Effects {
    if pending.suppressed {
        return Effects::default();
    }
    let Some(id) = pending.request_id.clone() else {
        return Effects::default();
    };
    let context_creating = matches!(
        pending.kind,
        PendingKind::CallTool2026 {
            context_creating: true
        }
    );
    let result = outcome.result;
    if !pending.result_matches_expected_operation(&result) {
        return Effects::output(error_response(
            Some(&id),
            OUTCOME_UNKNOWN,
            "Ghostlight received a result for a different canonical operation. Do not retry automatically; reconnect the client before continuing.",
            Some(json!({"disposition": "outcome_unknown"})),
        ));
    }
    let _workspace = match validated_result_workspace(
        pending.requested_workspace.as_ref(),
        pending.service_workspace.as_ref(),
        &result,
        context_creating,
    ) {
        Ok(workspace) => workspace,
        Err(message) => {
            return Effects::output(error_response(
                Some(&id),
                OUTCOME_UNKNOWN,
                message,
                Some(json!({"disposition": "outcome_unknown"})),
            ));
        }
    };
    let effect = result.effect;
    match surface::encode_result(surface::McpRevision::Mcp2026_07_28, *result) {
        Ok(result) => Effects::output(success_response(
            &id,
            complete_tool_result(result, None),
        )),
        Err(error) if effect != OperationEffect::None => Effects::output(error_response(
            Some(&id),
            OUTCOME_UNKNOWN,
            "Ghostlight could not render the canonical result after an effect may have occurred. Do not retry automatically; inspect current browser state first.",
            Some(json!({
                "disposition": "outcome_unknown",
                "effect": effect.as_str(),
                "renderError": error.to_string(),
            })),
        )),
        Err(error) => Effects::output(success_response(
            &id,
            complete_tool_error(None, &error.to_string()),
        )),
    }
}

fn validated_result_workspace(
    requested: Option<&WorkspaceId>,
    accepted: Option<&WorkspaceId>,
    result: &BrowserResult,
    context_creating: bool,
) -> Result<Option<WorkspaceId>, String> {
    if requested != accepted && (!context_creating || requested.is_some()) {
        return Err(
            "Ghostlight accepted work for a different workspace than the request. Do not retry automatically; reconnect the client before continuing."
                .to_string(),
        );
    }
    if result.workspace.as_ref() != accepted {
        return Err(
            "Ghostlight received inconsistent workspace facts after the operation. Do not retry automatically; reconnect the client before continuing."
                .to_string(),
        );
    }
    if context_creating {
        return result
            .workspace
            .clone()
            .map(Some)
            .ok_or_else(|| {
                "Ghostlight did not receive the workspace created for this operation. Do not retry automatically; reconnect the client before continuing."
                    .to_string()
            });
    }
    Ok(None)
}

fn render_rejection(pending: PendingRequest, error: BridgeError) -> Effects {
    if pending.suppressed {
        return Effects::default();
    }
    let Some(id) = pending.request_id.clone() else {
        return Effects::default();
    };
    if matches!(pending.kind, PendingKind::CallTool2026 { .. }) {
        let rendered = surface::encode_rejection(
            surface::McpRevision::Mcp2026_07_28,
            &error,
            pending.expected_operation,
            pending.requested_workspace.as_ref(),
        );
        return match rendered {
            Ok(result) => {
                Effects::output(success_response(&id, complete_tool_result(result, None)))
            }
            Err(message) => {
                Effects::output(success_response(&id, complete_tool_error(None, &message)))
            }
        };
    }
    let code = match error.kind {
        BridgeErrorKind::InvalidRequest
        | BridgeErrorKind::InvalidWorkspace
        | BridgeErrorKind::Restriction => INVALID_PARAMS,
        BridgeErrorKind::Busy | BridgeErrorKind::Transport | BridgeErrorKind::UnsupportedBridge => {
            SERVICE_UNAVAILABLE
        }
    };
    Effects::output(error_response(
        Some(&id),
        code,
        error.message,
        Some(json!({
            "kind": serde_json::to_value(error.kind).unwrap_or(Value::Null),
            "nextStep": error.next_step,
        })),
    ))
}

fn complete_tool_result(result: Value, workspace: Option<WorkspaceId>) -> Value {
    let mut object = result.as_object().cloned().unwrap_or_else(|| {
        let mut object = Map::new();
        object.insert("structuredContent".into(), result);
        object
    });
    object
        .entry("content")
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Some(workspace) = workspace {
        let raw = workspace.as_str().to_owned();
        match object.remove("structuredContent") {
            Some(Value::Object(mut structured)) => {
                structured.insert("workspaceId".into(), Value::String(raw));
                object.insert("structuredContent".into(), Value::Object(structured));
            }
            Some(other) => {
                object.insert(
                    "structuredContent".into(),
                    json!({"value": other, "workspaceId": raw}),
                );
            }
            None => {
                object.insert("structuredContent".into(), json!({"workspaceId": raw}));
            }
        }
    }
    complete_result(Value::Object(object))
}

fn complete_tool_error(result: Option<Value>, message: &str) -> Value {
    let mut object = result
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    object.insert("isError".into(), Value::Bool(true));
    object
        .entry("content")
        .or_insert_with(|| json!([{"type": "text", "text": message}]));
    complete_result(Value::Object(object))
}

fn complete_result(result: Value) -> Value {
    complete_result_with_meta(result, None)
}

fn complete_result_with_meta(result: Value, extra_meta: Option<(&str, Value)>) -> Value {
    let mut object = result.as_object().cloned().unwrap_or_else(|| {
        let mut object = Map::new();
        object.insert("value".into(), result);
        object
    });
    object.insert("resultType".into(), Value::String("complete".into()));
    let meta = object
        .entry("_meta")
        .or_insert_with(|| Value::Object(Map::new()));
    if !meta.is_object() {
        *meta = Value::Object(Map::new());
    }
    let meta = meta.as_object_mut().expect("meta object set above");
    meta.insert(
        SERVER_INFO_META.into(),
        json!({
            "name": Instance::resolve().mcp_server_name(),
            "version": env!("CARGO_PKG_VERSION"),
        }),
    );
    if let Some((key, value)) = extra_meta {
        meta.insert(key.into(), value);
    }
    Value::Object(object)
}
