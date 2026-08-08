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
use crate::surface::ghostlight_legacy;
use ghostlight_transport::bridge::{
    BridgeError, BridgeErrorKind, CatalogProjection, ClientPresentation, EdgeMessage,
    RequestContext, TerminalOutcome, WorkspaceId, WorkspaceUse,
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
        let mut arguments = match params.get("arguments") {
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
        let workspace = match arguments.remove("workspaceId") {
            None => None,
            Some(Value::String(raw)) => match WorkspaceId::parse(&raw) {
                Some(workspace) => Some(workspace),
                None => {
                    return Effects::output(error_response(
                        Some(&id),
                        INVALID_PARAMS,
                        "workspaceId is not a valid Ghostlight workspace handle; call a context-creating tab tool to obtain a new one",
                        None,
                    ));
                }
            },
            Some(_) => {
                return Effects::output(error_response(
                    Some(&id),
                    INVALID_PARAMS,
                    "workspaceId must be a string when supplied",
                    None,
                ));
            }
        };
        let arguments = Value::Object(arguments);
        let operation = match ghostlight_legacy::decode_call(external_tool, arguments.clone()) {
            Ok(operation) => operation,
            Err(error) => {
                return Effects::output(success_response(
                    &id,
                    complete_tool_error(None, &error.to_string()),
                ));
            }
        };
        let presentation =
            match ghostlight_legacy::invocation_presentation(external_tool, &arguments, &operation)
            {
                Ok(presentation) => presentation,
                Err(error) => {
                    return Effects::output(success_response(
                        &id,
                        complete_tool_error(None, &error.to_string()),
                    ));
                }
            };
        let flow_render_hints =
            ghostlight_legacy::flow_render_hints(external_tool, &arguments, &operation);
        let sequence = match correlation.track(PendingRequest::tool_request(
            id.clone(),
            PendingKind::CallTool2026 {
                context_creating: false,
            },
            presentation.clone(),
            operation.key(),
            flow_render_hints,
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
            presentation: Some(presentation),
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
    let tools = ghostlight_legacy::filtered_declarations(projection)
        .into_iter()
        .map(project_tool)
        .collect::<Vec<_>>();
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

fn project_tool(tool: ghostlight_legacy::RenderedTool) -> Value {
    let mut declaration = tool.declaration.as_object().cloned().unwrap_or_default();
    match tool.workspace_use {
        WorkspaceUse::Independent => {}
        WorkspaceUse::Creates => {
            let input_schema = object_field(&mut declaration, "inputSchema");
            insert_workspace_property(input_schema);
            let output_schema = object_field(&mut declaration, "outputSchema");
            insert_workspace_property(output_schema);
            require_schema_property(output_schema, "workspaceId");
        }
        WorkspaceUse::Uses => {
            let input_schema = object_field(&mut declaration, "inputSchema");
            insert_workspace_property(input_schema);
            require_schema_property(input_schema, "workspaceId");
        }
    }
    Value::Object(declaration)
}

fn require_schema_property(schema: &mut Map<String, Value>, property: &str) {
    let required = schema
        .entry("required")
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Some(required) = required.as_array_mut() {
        if !required.iter().any(|field| field == property) {
            required.push(Value::String(property.to_owned()));
        }
    }
}

fn object_field<'a>(object: &'a mut Map<String, Value>, field: &str) -> &'a mut Map<String, Value> {
    let value = object
        .entry(field.to_owned())
        .or_insert_with(|| json!({"type": "object", "properties": {}}));
    if !value.is_object() {
        *value = json!({"type": "object", "properties": {}});
    }
    let schema = value.as_object_mut().expect("object set above");
    schema
        .entry("type")
        .or_insert_with(|| Value::String("object".into()));
    schema
}

fn insert_workspace_property(schema: &mut Map<String, Value>) {
    let properties = schema
        .entry("properties")
        .or_insert_with(|| Value::Object(Map::new()));
    if !properties.is_object() {
        *properties = Value::Object(Map::new());
    }
    properties
        .as_object_mut()
        .expect("properties object set above")
        .entry("workspaceId")
        .or_insert_with(|| {
            json!({
                "type": "string",
                "description": "Opaque Ghostlight workspace handle returned by a context-creating tab tool."
            })
        });
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
    match outcome {
        TerminalOutcome::Success { result } => {
            if !pending.result_matches_expected_operation(&result) {
                return Effects::output(error_response(
                    Some(&id),
                    OUTCOME_UNKNOWN,
                    "Ghostlight received a result for a different canonical operation. Do not retry automatically; reconnect the client before continuing.",
                    Some(json!({"disposition": "outcome_unknown"})),
                ));
            }
            let workspace = match validated_result_workspace(
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
            match ghostlight_legacy::encode_result(
                *result,
                pending.presentation.as_ref(),
                pending.flow_render_hints.as_ref(),
            ) {
                Ok(result) => Effects::output(success_response(
                    &id,
                    complete_tool_result(result, workspace),
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
        TerminalOutcome::ToolFailure { result, message } => Effects::output(success_response(
            &id,
            complete_tool_error(Some(result), &message),
        )),
        TerminalOutcome::NotDispatched { message }
        | TerminalOutcome::Denied { message, .. }
        | TerminalOutcome::AttentionRequired { message } => {
            Effects::output(success_response(&id, complete_tool_error(None, &message)))
        }
        TerminalOutcome::Held { prolonged } => match ghostlight_legacy::encode_held(
            prolonged,
            pending.presentation.as_ref(),
            pending.expected_operation,
        ) {
            Ok(message) => {
                Effects::output(success_response(&id, complete_tool_error(None, &message)))
            }
            Err(_) => Effects::output(success_response(
                &id,
                complete_tool_error(None, &ghostlight_legacy::encode_held_fallback(prolonged)),
            )),
        },
        TerminalOutcome::Cancelled {
            message,
            effect: OperationEffect::None,
        } => Effects::output(success_response(&id, complete_tool_error(None, &message))),
        TerminalOutcome::Cancelled { message, effect } => Effects::output(error_response(
            Some(&id),
            OUTCOME_UNKNOWN,
            format!("{message} Do not retry automatically; inspect current browser state first."),
            Some(json!({
                "disposition": "outcome_unknown",
                "effect": effect.as_str(),
            })),
        )),
        TerminalOutcome::OutcomeUnknown { message } => Effects::output(error_response(
            Some(&id),
            OUTCOME_UNKNOWN,
            message,
            Some(json!({"disposition": "outcome_unknown"})),
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
    let Some(id) = pending.request_id else {
        return Effects::default();
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::bridge::{
        DenialSource, OperationAvailability, ServiceMessage, WorkId,
    };
    use ghostlight_transport::operation::{
        BrowserResult, BrowserResultStatus, IntentId, InvocationPresentation, OperationEffect,
        OperationId, OperationKey, ResultPart,
    };

    fn request(id: i64, method: &str, restriction: Option<&str>) -> Request {
        let mut meta = Map::new();
        meta.insert(PROTOCOL_VERSION_META.into(), json!(PROTOCOL_VERSION));
        meta.insert(CLIENT_CAPABILITIES_META.into(), json!({}));
        meta.insert(
            CLIENT_INFO_META.into(),
            json!({"name": format!("client-{id}"), "version": "1.0"}),
        );
        if let Some(restriction) = restriction {
            meta.insert(RESTRICTION_META.into(), json!(restriction));
        }
        Request {
            id: Some(RequestId::Number(id.into())),
            method: method.into(),
            params: json!({"_meta": meta}),
        }
    }

    fn tool_call(id: i64, name: &str, arguments: Value) -> Request {
        let mut request = request(id, "tools/call", None);
        request.params["name"] = json!(name);
        request.params["arguments"] = arguments;
        request
    }

    fn complete_tool_call(
        handler: &mut Handler,
        correlation: &mut Correlation,
        request: Request,
        work_id: WorkId,
        outcome: impl FnOnce(Option<WorkspaceId>) -> TerminalOutcome,
    ) -> Effects {
        let started = handler.handle(&request, correlation);
        let EdgeMessage::Start {
            sequence,
            workspace,
            ..
        } = started.service[0].clone()
        else {
            panic!("start expected");
        };
        let outcome = outcome(workspace.clone());
        assert!(matches!(
            correlation.observe(ServiceMessage::Started {
                sequence,
                work_id,
                workspace,
                context_creating: false,
            }),
            crate::bridge::Observation::None
        ));
        let crate::bridge::Observation::Resolved(resolution) =
            correlation.observe(ServiceMessage::Completed { work_id, outcome })
        else {
            panic!("completion resolution expected");
        };
        handler.on_resolution(resolution)
    }

    fn fnv1a64(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    #[test]
    fn full_legacy_profile_transcript_is_exact_through_the_2026_handler() {
        let projection = ghostlight_legacy::test_support::full_projection(23);
        let mut handler = Handler::new();
        let mut correlation = Correlation::default();
        let catalog = handler.handle(&request(1, "tools/list", None), &mut correlation);
        let EdgeMessage::Catalog { sequence, .. } = catalog.service[0].clone() else {
            panic!("catalog expected");
        };
        let crate::bridge::Observation::Resolved(resolution) =
            correlation.observe(ServiceMessage::Catalog {
                sequence,
                projection,
            })
        else {
            panic!("catalog resolution expected");
        };
        let listed = handler.on_resolution(resolution);
        assert_eq!(listed.output[0]["jsonrpc"], "2.0");
        assert_eq!(listed.output[0]["id"], 1);
        let tools = listed.output[0]["result"]["tools"]
            .as_array()
            .expect("full tools array");
        assert_eq!(tools.len(), 25);
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool["name"].as_str().expect("name"))
                .collect::<Vec<_>>(),
            ghostlight_legacy::declarations()["tools"]
                .as_array()
                .expect("frozen tools")
                .iter()
                .map(|tool| tool["name"].as_str().expect("name"))
                .collect::<Vec<_>>()
        );
        let encoded_tools = serde_json::to_vec(tools).expect("catalog JSON");
        assert_eq!(encoded_tools.len(), 77_598);
        assert_eq!(fnv1a64(&encoded_tools), 0x52da_9080_ec57_f8df);
        let mut catalog_result = listed.output[0]["result"]
            .as_object()
            .expect("catalog result")
            .clone();
        catalog_result.remove("tools");
        assert_eq!(
            Value::Object(catalog_result),
            json!({
                "ttlMs": UNRESTRICTED_CATALOG_TTL_MS,
                "cacheScope": "private",
                "resultType": "complete",
                "_meta": {
                    "io.modelcontextprotocol/serverInfo": {
                        "name": Instance::resolve().mcp_server_name(),
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "org.sylin/ghostlightCatalogGeneration": 23,
                },
            })
        );

        let explained = complete_tool_call(
            &mut handler,
            &mut correlation,
            tool_call(2, "explain", json!({})),
            WorkId(20),
            |workspace| TerminalOutcome::Success {
                result: Box::new(ghostlight_legacy::test_support::context_result(workspace)),
            },
        );
        assert_eq!(
            explained.output,
            vec![json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {
                    "content": [{
                        "type": "text",
                        "text": ghostlight_legacy::test_support::explain_text(),
                    }],
                    "resultType": "complete",
                    "_meta": {
                        "io.modelcontextprotocol/serverInfo": {
                            "name": Instance::resolve().mcp_server_name(),
                            "version": env!("CARGO_PKG_VERSION"),
                        },
                    },
                },
            })]
        );

        let workspace = WorkspaceId::mint();
        let succeeded = complete_tool_call(
            &mut handler,
            &mut correlation,
            tool_call(
                3,
                "get_page_text",
                json!({"workspaceId": workspace.as_str(), "tabId": 7}),
            ),
            WorkId(30),
            |workspace| {
                let mut result = BrowserResult::new(
                    OperationId::BrowserRead,
                    IntentId::ReadText,
                    BrowserResultStatus::Ok,
                    OperationEffect::None,
                );
                result.workspace = workspace;
                result.parts.push(ResultPart::Text {
                    text: "Page text".into(),
                });
                result.data = json!({"characters": 9});
                TerminalOutcome::Success {
                    result: Box::new(result),
                }
            },
        );
        assert_eq!(
            succeeded.output,
            vec![json!({
                "jsonrpc": "2.0",
                "id": 3,
                "result": {
                    "content": [{"type": "text", "text": "Page text"}],
                    "structuredContent": {"characters": 9},
                    "resultType": "complete",
                    "_meta": {
                        "io.modelcontextprotocol/serverInfo": {
                            "name": Instance::resolve().mcp_server_name(),
                            "version": env!("CARGO_PKG_VERSION"),
                        },
                    },
                },
            })]
        );

        let denied = complete_tool_call(
            &mut handler,
            &mut correlation,
            tool_call(
                4,
                "get_page_text",
                json!({"workspaceId": workspace.as_str(), "tabId": 7}),
            ),
            WorkId(40),
            |_| TerminalOutcome::Denied {
                message: "Blocked by test policy.".into(),
                source: DenialSource::Policy,
            },
        );
        assert_eq!(
            denied.output,
            vec![json!({
                "jsonrpc": "2.0",
                "id": 4,
                "result": {
                    "content": [{"type": "text", "text": "Blocked by test policy."}],
                    "isError": true,
                    "resultType": "complete",
                    "_meta": {
                        "io.modelcontextprotocol/serverInfo": {
                            "name": Instance::resolve().mcp_server_name(),
                            "version": env!("CARGO_PKG_VERSION"),
                        },
                    },
                },
            })]
        );
    }

    #[test]
    fn discovery_is_exact_unbinding_and_does_not_advertise_tasks() {
        let mut discovery = request(0, "server/discover", None);
        discovery.id = Some(RequestId::String("discover".into()));
        let response = discover(&discovery);
        let result = &response.output[0]["result"];
        assert_eq!(
            result["supportedVersions"],
            json!([PROTOCOL_VERSION, crate::mcp_2025_11_25::PROTOCOL_VERSION,])
        );
        assert_eq!(result["resultType"], "complete");
        assert_eq!(result["capabilities"]["tools"]["listChanged"], true);
        assert_eq!(
            result["_meta"][SERVER_INFO_META]["version"],
            env!("CARGO_PKG_VERSION")
        );
        assert!(!result.to_string().contains("tasks"));

        let missing_meta = discover(&Request {
            id: Some(RequestId::Number(1.into())),
            method: "server/discover".into(),
            params: json!({}),
        });
        assert_eq!(missing_meta.output[0]["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn discovery_appends_exact_transport_closed_recovery_instructions() {
        let mut discovery = request(0, "server/discover", None);
        discovery.id = Some(RequestId::String("discover".into()));
        let response = discover(&discovery);
        assert_eq!(
            response.output[0]["result"]["instructions"],
            format!(
                "Use Ghostlight tools to automate the user's governed Chromium workspace. {}",
                crate::TRANSPORT_CLOSED_RECOVERY_INSTRUCTIONS
            )
        );
    }

    #[test]
    fn every_request_reads_its_own_metadata_without_cross_stamping() {
        let mut correlation = Correlation::default();
        let mut handler = Handler::new();
        let first = handler.handle(&request(1, "tools/list", Some("one")), &mut correlation);
        let second = handler.handle(&request(2, "tools/list", Some("two")), &mut correlation);
        let EdgeMessage::Catalog { context, .. } = &first.service[0] else {
            panic!("catalog expected");
        };
        assert_eq!(context.restriction.as_deref(), Some("one"));
        assert_eq!(context.client.as_ref().unwrap().name, "client-1");
        let EdgeMessage::Catalog { context, .. } = &second.service[0] else {
            panic!("catalog expected");
        };
        assert_eq!(context.restriction.as_deref(), Some("two"));
        assert_eq!(context.client.as_ref().unwrap().name, "client-2");
    }

    #[test]
    fn tools_list_augments_only_the_workspace_shore_and_sets_private_zero_cache() {
        let projection = CatalogProjection {
            generation: 9,
            operations: vec![
                OperationAvailability {
                    id: OperationId::BrowserTabs,
                    intent: IntentId::TabsList,
                    workspace_use: WorkspaceUse::Creates,
                },
                OperationAvailability {
                    id: OperationId::BrowserInput,
                    intent: IntentId::InputPointerClick,
                    workspace_use: WorkspaceUse::Uses,
                },
                OperationAvailability {
                    id: OperationId::WorkflowPlan,
                    intent: IntentId::PlanUpdate,
                    workspace_use: WorkspaceUse::Independent,
                },
            ],
            restricted: true,
        };
        let result = list_tools_result(&projection);
        assert_eq!(
            result["tools"][0]["outputSchema"]["properties"]["workspaceId"]["type"],
            "string"
        );
        assert_eq!(
            result["tools"][0]["inputSchema"]["properties"]["workspaceId"]["type"],
            "string"
        );
        assert_eq!(
            result["tools"][1]["inputSchema"]["properties"]["workspaceId"]["type"],
            "string"
        );
        assert!(result["tools"][1]["inputSchema"]["required"]
            .as_array()
            .expect("uses schema required list")
            .iter()
            .any(|field| field == "workspaceId"));
        assert!(result["tools"][2]["inputSchema"]["properties"]
            .get("workspaceId")
            .is_none());
        assert_eq!(result["tools"][0]["name"], "tabs_context_mcp");
        assert_eq!(result["tools"][1]["name"], "computer");
        assert_eq!(result["tools"][2]["name"], "update_plan");
        assert_eq!(result["ttlMs"], 0);
        assert_eq!(result["cacheScope"], "private");
        assert_eq!(result["resultType"], "complete");
    }

    #[test]
    fn tool_call_removes_workspace_id_before_crossing_the_bridge() {
        let workspace = WorkspaceId::mint();
        let mut call = request(3, "tools/call", None);
        call.params["name"] = json!("computer");
        call.params["arguments"] = json!({
            "workspaceId": workspace.as_str(),
            "action": "left_click",
            "tabId": 7,
            "coordinate": [4, 8]
        });
        let mut correlation = Correlation::default();
        let effects = Handler::new().handle(&call, &mut correlation);
        let EdgeMessage::Start {
            operation,
            workspace: sent_workspace,
            ..
        } = &effects.service[0]
        else {
            panic!("start expected");
        };
        assert!(operation.arguments.get("workspaceId").is_none());
        assert!(operation.arguments.get("tabId").is_none());
        assert_eq!(operation.arguments["tab"], 7);
        assert_eq!(operation.arguments["point"], json!([4, 8]));
        assert_eq!(sent_workspace.as_ref(), Some(&workspace));
    }

    #[test]
    fn subscription_acknowledges_first_stays_live_and_closes_on_shutdown() {
        let mut handler = Handler::new();
        let mut correlation = Correlation::default();
        let mut listen = request(4, "subscriptions/listen", None);
        listen.params["notifications"] = json!({
            "toolsListChanged": true,
            "promptsListChanged": true,
        });
        let ack = handler.handle(&listen, &mut correlation);
        assert_eq!(
            ack.output[0]["method"],
            "notifications/subscriptions/acknowledged"
        );
        assert_eq!(ack.output[0]["params"]["_meta"][SUBSCRIPTION_ID_META], 4);
        assert_eq!(
            ack.output[0]["params"]["notifications"],
            json!({"toolsListChanged": true})
        );
        assert!(ack.output[0].get("id").is_none());
        let changed = handler.catalog_changed(2);
        assert_eq!(changed.output.len(), 1);
        assert_eq!(
            changed.output[0]["method"],
            "notifications/tools/list_changed"
        );
        assert_eq!(
            changed.output[0]["params"]["_meta"][SUBSCRIPTION_ID_META],
            4
        );

        let closed = handler.shutdown();
        assert_eq!(closed.output[0]["id"], 4);
        assert_eq!(closed.output[0]["result"]["resultType"], "complete");
        assert_eq!(closed.output[0]["result"]["_meta"][SUBSCRIPTION_ID_META], 4);
        assert!(handler.catalog_changed(3).output.is_empty());
    }

    #[test]
    fn cancelled_subscription_has_no_terminal_response_or_future_notifications() {
        let mut handler = Handler::new();
        let mut correlation = Correlation::default();
        let mut listen = request(4, "subscriptions/listen", None);
        listen.params["notifications"] = json!({"toolsListChanged": true});
        handler.handle(&listen, &mut correlation);
        handler.cancellation(
            &Request {
                id: None,
                method: "notifications/cancelled".into(),
                params: json!({"requestId": 4}),
            },
            &mut correlation,
        );
        assert!(handler.catalog_changed(3).output.is_empty());
        assert!(handler.shutdown().output.is_empty());
    }

    #[test]
    fn id_bearing_cancellation_errors_without_closing_the_subscription() {
        let mut handler = Handler::new();
        let mut correlation = Correlation::default();
        let mut listen = request(4, "subscriptions/listen", None);
        listen.params["notifications"] = json!({"toolsListChanged": true});
        handler.handle(&listen, &mut correlation);

        let effects = handler.handle(
            &Request {
                id: Some(RequestId::Number(5.into())),
                method: "notifications/cancelled".into(),
                params: json!({"requestId": 4}),
            },
            &mut correlation,
        );

        assert_eq!(effects.output[0]["error"]["code"], INVALID_REQUEST);
        assert_eq!(handler.catalog_changed(3).output.len(), 1);
        assert_eq!(handler.shutdown().output[0]["id"], 4);
    }

    #[test]
    fn subscription_filter_types_are_validated_even_when_unsupported() {
        let mut listen = request(5, "subscriptions/listen", None);
        listen.params["notifications"] = json!({"resourceSubscriptions": ["ok", 3]});
        let effects = Handler::new().handle(&listen, &mut Correlation::default());
        assert_eq!(effects.output[0]["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn live_subscription_ids_cannot_be_reused_by_other_requests() {
        let mut handler = Handler::new();
        let mut correlation = Correlation::default();
        let mut listen = request(6, "subscriptions/listen", None);
        listen.params["notifications"] = json!({});
        handler.handle(&listen, &mut correlation);
        let duplicate = handler.handle(&request(6, "tools/list", None), &mut correlation);
        assert_eq!(duplicate.output[0]["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn selected_handler_keeps_discovery_inside_active_id_checks() {
        let mut handler = Handler::new();
        let mut correlation = Correlation::default();
        handler.handle(&request(10, "tools/list", None), &mut correlation);

        let duplicate = handler.handle(&request(10, "server/discover", None), &mut correlation);
        assert_eq!(duplicate.output[0]["error"]["code"], INVALID_REQUEST);

        let discovery = handler.handle(&request(11, "server/discover", None), &mut correlation);
        assert_eq!(
            discovery.output[0]["result"]["supportedVersions"],
            json!([PROTOCOL_VERSION, crate::mcp_2025_11_25::PROTOCOL_VERSION])
        );
    }

    #[test]
    fn removed_ping_method_is_not_found() {
        let effects = Handler::new().handle(&request(7, "ping", None), &mut Correlation::default());
        assert_eq!(effects.output[0]["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn client_info_is_optional_but_checked_when_present() {
        let mut without_info = request(8, "tools/list", None);
        without_info.params["_meta"]
            .as_object_mut()
            .expect("metadata object")
            .remove(CLIENT_INFO_META);
        let effects = Handler::new().handle(&without_info, &mut Correlation::default());
        let EdgeMessage::Catalog { context, .. } = &effects.service[0] else {
            panic!("catalog expected");
        };
        assert!(context.client.is_none());

        let mut malformed = request(9, "tools/list", None);
        malformed.params["_meta"][CLIENT_INFO_META] = json!({"name": "missing-version"});
        let effects = Handler::new().handle(&malformed, &mut Correlation::default());
        assert_eq!(effects.output[0]["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn every_success_renderer_adds_result_type() {
        let success = complete_tool_result(json!({"content": []}), None);
        let failure = complete_tool_error(None, "failed");
        assert_eq!(success["resultType"], "complete");
        assert_eq!(failure["resultType"], "complete");
    }

    #[test]
    fn held_outcomes_render_the_exact_external_label_for_both_hold_boundaries() {
        let cases = [
            (
                40,
                "computer",
                "left_click",
                OperationKey::new(OperationId::BrowserInput, IntentId::InputPointerClick),
                false,
                "'computer (left_click)' call",
            ),
            (
                41,
                "computer",
                "left_click",
                OperationKey::new(OperationId::BrowserInput, IntentId::InputPointerClick),
                true,
                "'computer (left_click)' call",
            ),
            (
                42,
                "form_fill",
                "submit",
                OperationKey::new(OperationId::BrowserFill, IntentId::FillFieldsAndSubmit),
                true,
                "'form_fill (submit)' call",
            ),
        ];
        for (request_id, tool, action, expected, prolonged, expected_label) in cases {
            let presentation = InvocationPresentation::new(
                ghostlight_legacy::PROFILE_ID,
                ghostlight_legacy::PROFILE_VERSION,
                tool,
                Some(action.into()),
            )
            .expect("valid legacy presentation");
            let effects = render_outcome(
                PendingRequest::tool_request(
                    RequestId::Number(request_id.into()),
                    PendingKind::CallTool2026 {
                        context_creating: false,
                    },
                    presentation,
                    expected,
                    None,
                    None,
                ),
                TerminalOutcome::Held { prolonged },
            );

            let result = &effects.output[0]["result"];
            let text = result["content"][0]["text"].as_str().expect("hold text");
            assert!(text.contains(expected_label), "{text}");
            assert!(!text.contains("browser.input"), "{text}");
            assert!(!text.contains("pointer.click"), "{text}");
            assert_eq!(text.contains("more than 2 minutes"), prolonged);
            assert_eq!(result["isError"], true);
            assert_eq!(result["resultType"], "complete");
        }

        let fallback = render_outcome(
            PendingRequest {
                request_id: Some(RequestId::Number(43.into())),
                kind: PendingKind::CallTool2026 {
                    context_creating: false,
                },
                service_workspace: None,
                requested_workspace: None,
                presentation: None,
                expected_operation: Some(OperationKey::new(
                    OperationId::BrowserInput,
                    IntentId::InputPointerClick,
                )),
                flow_render_hints: None,
                suppressed: false,
                delivered: true,
            },
            TerminalOutcome::Held { prolonged: false },
        );
        assert!(fallback.output[0].get("error").is_none());
        assert!(fallback.output[0]["result"]["content"][0]["text"]
            .as_str()
            .expect("fallback")
            .contains("'browser tool' call"));
        assert_eq!(fallback.output[0]["result"]["resultType"], "complete");
    }

    #[test]
    fn unsupported_protocol_version_uses_the_revision_defined_error() {
        let mut unsupported = request(7, "ping", None);
        unsupported.params["_meta"][PROTOCOL_VERSION_META] = json!("2099-01-01");
        let effects = Handler::new().handle(&unsupported, &mut Correlation::default());
        assert_eq!(
            effects.output[0]["error"]["code"],
            UNSUPPORTED_PROTOCOL_VERSION
        );
        assert_eq!(
            effects.output[0]["error"]["data"]["supported"],
            json!([PROTOCOL_VERSION])
        );
    }

    #[test]
    fn product_local_errors_stay_outside_the_revision_reserved_range() {
        for code in [SERVICE_UNAVAILABLE, OUTCOME_UNKNOWN] {
            assert!(
                !(-32_768..=-32_000).contains(&code),
                "product-local error code {code} collides with the JSON-RPC/MCP reserved range"
            );
        }
    }

    #[test]
    fn context_creator_returns_the_service_minted_workspace_at_the_shore() {
        let workspace = WorkspaceId::mint();
        let raw = workspace.as_str().to_owned();
        let effects = render_outcome(
            PendingRequest {
                request_id: Some(RequestId::Number(8.into())),
                kind: PendingKind::CallTool2026 {
                    context_creating: true,
                },
                service_workspace: Some(workspace.clone()),
                requested_workspace: None,
                presentation: None,
                expected_operation: Some(OperationKey::new(
                    OperationId::BrowserTabs,
                    IntentId::TabsList,
                )),
                flow_render_hints: None,
                suppressed: false,
                delivered: true,
            },
            TerminalOutcome::Success {
                result: {
                    let mut result = BrowserResult::new(
                        OperationId::BrowserTabs,
                        IntentId::TabsList,
                        BrowserResultStatus::Ok,
                        OperationEffect::None,
                    );
                    result.workspace = Some(workspace);
                    result.data = json!({"tabId": 44});
                    Box::new(result)
                },
            },
        );
        assert_eq!(
            effects.output[0]["result"]["structuredContent"]["workspaceId"],
            raw
        );
        assert_eq!(effects.output[0]["result"]["resultType"], "complete");
    }

    #[test]
    fn inconsistent_result_workspace_fails_closed_without_leaking_either_handle() {
        let accepted = WorkspaceId::mint();
        let returned = WorkspaceId::mint();
        let accepted_raw = accepted.as_str().to_owned();
        let returned_raw = returned.as_str().to_owned();
        let mut result = BrowserResult::new(
            OperationId::BrowserTabs,
            IntentId::TabsList,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        result.workspace = Some(returned);
        let effects = render_outcome(
            PendingRequest {
                request_id: Some(RequestId::Number(9.into())),
                kind: PendingKind::CallTool2026 {
                    context_creating: true,
                },
                service_workspace: Some(accepted),
                requested_workspace: None,
                presentation: None,
                expected_operation: Some(OperationKey::new(
                    OperationId::BrowserTabs,
                    IntentId::TabsList,
                )),
                flow_render_hints: None,
                suppressed: false,
                delivered: true,
            },
            TerminalOutcome::Success {
                result: Box::new(result),
            },
        );
        assert_eq!(effects.output[0]["error"]["code"], OUTCOME_UNKNOWN);
        let rendered = effects.output[0].to_string();
        assert!(!rendered.contains(&accepted_raw));
        assert!(!rendered.contains(&returned_raw));
    }

    #[test]
    fn mismatched_result_operation_fails_closed_before_profile_rendering() {
        let pending = PendingRequest {
            request_id: Some(RequestId::Number(11.into())),
            kind: PendingKind::CallTool2026 {
                context_creating: false,
            },
            service_workspace: None,
            requested_workspace: None,
            presentation: None,
            expected_operation: Some(OperationKey::new(
                OperationId::BrowserTabs,
                IntentId::TabsList,
            )),
            flow_render_hints: None,
            suppressed: false,
            delivered: true,
        };
        let result = BrowserResult::new(
            OperationId::BrowserSnapshot,
            IntentId::SnapshotCapture,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        let effects = render_outcome(
            pending,
            TerminalOutcome::Success {
                result: Box::new(result),
            },
        );
        assert_eq!(effects.output[0]["error"]["code"], OUTCOME_UNKNOWN);
        assert_eq!(
            effects.output[0]["error"]["data"]["disposition"],
            "outcome_unknown"
        );
    }

    #[test]
    fn existing_workspace_is_verified_but_not_added_to_a_non_creator_result() {
        let workspace = WorkspaceId::mint();
        let raw = workspace.as_str().to_owned();
        let mut result = BrowserResult::new(
            OperationId::BrowserSnapshot,
            IntentId::SnapshotCapture,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        result.workspace = Some(workspace.clone());
        result.data = json!({"snapshot": "ok"});
        let effects = render_outcome(
            PendingRequest {
                request_id: Some(RequestId::Number(10.into())),
                kind: PendingKind::CallTool2026 {
                    context_creating: false,
                },
                service_workspace: Some(workspace.clone()),
                requested_workspace: Some(workspace),
                presentation: None,
                expected_operation: Some(OperationKey::new(
                    OperationId::BrowserSnapshot,
                    IntentId::SnapshotCapture,
                )),
                flow_render_hints: None,
                suppressed: false,
                delivered: true,
            },
            TerminalOutcome::Success {
                result: Box::new(result),
            },
        );
        assert_eq!(effects.output[0]["result"]["resultType"], "complete");
        assert!(!effects.output[0].to_string().contains(&raw));
    }

    #[test]
    fn substituted_non_creator_workspace_fails_closed_without_leaking_handles() {
        let requested = WorkspaceId::mint();
        let accepted = WorkspaceId::mint();
        let requested_raw = requested.as_str().to_owned();
        let accepted_raw = accepted.as_str().to_owned();
        let mut result = BrowserResult::new(
            OperationId::BrowserSnapshot,
            IntentId::SnapshotCapture,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        result.workspace = Some(accepted.clone());
        let effects = render_outcome(
            PendingRequest {
                request_id: Some(RequestId::Number(12.into())),
                kind: PendingKind::CallTool2026 {
                    context_creating: false,
                },
                service_workspace: Some(accepted),
                requested_workspace: Some(requested),
                presentation: None,
                expected_operation: Some(OperationKey::new(
                    OperationId::BrowserSnapshot,
                    IntentId::SnapshotCapture,
                )),
                flow_render_hints: None,
                suppressed: false,
                delivered: true,
            },
            TerminalOutcome::Success {
                result: Box::new(result),
            },
        );

        assert_eq!(effects.output[0]["error"]["code"], OUTCOME_UNKNOWN);
        let rendered = effects.output[0].to_string();
        assert!(!rendered.contains(&requested_raw));
        assert!(!rendered.contains(&accepted_raw));
    }
}
