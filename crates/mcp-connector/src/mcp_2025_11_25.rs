// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Exact MCP `2025-11-25` lifecycle and envelope rendering.
//!
//! This shore owns one implicit workspace. The service remains unaware of initialize,
//! notifications/initialized, JSON-RPC ids, and this protocol date.

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
use ghostlight_transport::operation::OperationEffect;
use serde_json::{json, Value};

/// The exact protocol revision implemented by this shore.
pub const PROTOCOL_VERSION: &str = "2025-11-25";
const SERVICE_UNAVAILABLE: i64 = -32001;
const LIFECYCLE_ERROR: i64 = -32002;
const OUTCOME_UNKNOWN: i64 = -32003;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lifecycle {
    OpeningWorkspace,
    LoadingInitializeCatalog,
    AwaitingInitialized,
    Operational,
    ReopeningWorkspace,
    RefreshingCatalog,
    InitializationFailed,
}

/// One selected MCP `2025-11-25` state machine.
pub struct Handler {
    lifecycle: Lifecycle,
    context: RequestContext,
    workspace: Option<WorkspaceId>,
    catalog: Option<CatalogProjection>,
    resume_lifecycle: Option<Lifecycle>,
}

impl Handler {
    /// Validate and begin the strict initialize lifecycle.
    pub fn select(
        request: &Request,
        correlation: &mut Correlation,
    ) -> Result<(Self, Effects), Value> {
        let (id, context) = parse_initialize(request)?;
        let mut handler = Self {
            lifecycle: Lifecycle::OpeningWorkspace,
            context: context.clone(),
            workspace: None,
            catalog: None,
            resume_lifecycle: None,
        };
        let effects = handler.open_workspace(id, context, None, correlation)?;
        Ok((handler, effects))
    }

    /// Handle one request or notification after this shore has been selected.
    pub fn handle(&mut self, request: &Request, correlation: &mut Correlation) -> Effects {
        if request.method == "notifications/cancelled" {
            if request.id.is_some() {
                return response_error(
                    request,
                    INVALID_REQUEST,
                    "notifications/cancelled must not carry an id",
                );
            }
            return cancellation(request, correlation);
        }
        if request
            .id
            .as_ref()
            .is_some_and(|id| correlation.contains_request(id))
        {
            return response_error(
                request,
                INVALID_REQUEST,
                "a request with this id is already active",
            );
        }
        if request.method == "ping" {
            return request.id.as_ref().map_or_else(Effects::default, |id| {
                Effects::output(success_response(id, json!({})))
            });
        }
        if request.method == "initialize" {
            if request.id.is_none() {
                return Effects::default();
            }
            if self.lifecycle == Lifecycle::InitializationFailed {
                return match parse_initialize(request) {
                    Ok((id, context)) => {
                        self.context = context.clone();
                        self.lifecycle = Lifecycle::OpeningWorkspace;
                        self.open_workspace(id, context, self.workspace.clone(), correlation)
                            .unwrap_or_else(Effects::output)
                    }
                    Err(response) => Effects::output(response),
                };
            }
            return response_error(
                request,
                INVALID_REQUEST,
                "initialize is only legal once for this MCP process",
            );
        }
        if request.method == "notifications/initialized" {
            if request.id.is_some() {
                return response_error(
                    request,
                    INVALID_REQUEST,
                    "notifications/initialized must not carry an id",
                );
            }
            if self.lifecycle == Lifecycle::AwaitingInitialized {
                self.lifecycle = Lifecycle::Operational;
            }
            return Effects::default();
        }
        if self.lifecycle != Lifecycle::Operational {
            return response_error(
                request,
                LIFECYCLE_ERROR,
                "Ghostlight is not operational until initialize completes and notifications/initialized arrives",
            );
        }

        match request.method.as_str() {
            "tools/list" => self.tools_list(request, correlation),
            "tools/call" => self.tools_call(request, correlation),
            _ if request.id.is_none() => Effects::default(),
            other => response_error(
                request,
                METHOD_NOT_FOUND,
                format!("Method not found: {other}"),
            ),
        }
    }

    /// Resume this handler after one correlated bridge operation resolves.
    pub fn on_resolution(
        &mut self,
        resolution: Resolution,
        correlation: &mut Correlation,
    ) -> Effects {
        match resolution {
            Resolution::WorkspaceOpened { pending, workspace }
                if pending.kind == PendingKind::OpenWorkspace2025 =>
            {
                self.workspace = Some(workspace.clone());
                if pending.suppressed {
                    self.lifecycle = Lifecycle::InitializationFailed;
                    return Effects::default();
                }
                self.lifecycle = Lifecycle::LoadingInitializeCatalog;
                let Some(id) = pending.request_id else {
                    self.lifecycle = Lifecycle::InitializationFailed;
                    return Effects::default();
                };
                self.catalog_request(
                    id,
                    workspace,
                    self.context.clone(),
                    PendingKind::InitializeCatalog2025,
                    correlation,
                )
            }
            Resolution::WorkspaceOpened { pending, workspace }
                if pending.kind == PendingKind::ReopenWorkspace2025 =>
            {
                self.workspace = Some(workspace.clone());
                self.lifecycle = Lifecycle::RefreshingCatalog;
                self.background_catalog(workspace, correlation)
            }
            Resolution::Catalog {
                pending,
                projection,
            } if pending.kind == PendingKind::InitializeCatalog2025 => {
                if pending.suppressed {
                    self.lifecycle = Lifecycle::InitializationFailed;
                    return Effects::default();
                }
                self.catalog = Some(projection.clone());
                self.lifecycle = Lifecycle::AwaitingInitialized;
                response_for_pending(
                    pending,
                    initialize_result(&projection),
                    Lifecycle::AwaitingInitialized,
                    self,
                )
            }
            Resolution::Catalog {
                pending,
                projection,
            } if pending.kind == PendingKind::ListTools2025 => {
                self.catalog = Some(projection.clone());
                response_for_pending(
                    pending,
                    list_tools_result(&projection),
                    Lifecycle::Operational,
                    self,
                )
            }
            Resolution::Catalog {
                pending,
                projection,
            } if pending.kind == PendingKind::ReconnectCatalog2025 => {
                self.catalog = Some(projection);
                let resumed = self
                    .resume_lifecycle
                    .take()
                    .unwrap_or(Lifecycle::Operational);
                self.lifecycle = resumed;
                if resumed == Lifecycle::Operational {
                    Effects::output(notification("notifications/tools/list_changed", None))
                } else {
                    Effects::default()
                }
            }
            Resolution::Completed { pending, outcome }
                if pending.kind == PendingKind::CallTool2025 =>
            {
                render_outcome(pending, outcome)
            }
            Resolution::Rejected { pending, error } => {
                if matches!(
                    pending.kind,
                    PendingKind::OpenWorkspace2025 | PendingKind::InitializeCatalog2025
                ) {
                    self.lifecycle = Lifecycle::InitializationFailed;
                }
                if matches!(
                    pending.kind,
                    PendingKind::ReopenWorkspace2025 | PendingKind::ReconnectCatalog2025
                ) {
                    self.lifecycle = Lifecycle::ReopeningWorkspace;
                }
                render_rejection(pending, error)
            }
            Resolution::WorkspaceReleased
            | Resolution::WorkspaceOpened { .. }
            | Resolution::Catalog { .. }
            | Resolution::Completed { .. } => Effects::default(),
        }
    }

    /// Invalidate the cached projection and emit the revision's list-changed notification.
    pub fn catalog_changed(&mut self, _generation: u64) -> Effects {
        self.catalog = None;
        if self.lifecycle == Lifecycle::Operational {
            Effects::output(notification("notifications/tools/list_changed", None))
        } else {
            Effects::default()
        }
    }

    /// Mark the retained implicit workspace for reattachment after bridge loss.
    pub fn bridge_disconnected(&mut self) {
        match self.lifecycle {
            Lifecycle::Operational | Lifecycle::AwaitingInitialized => {
                self.resume_lifecycle = Some(self.lifecycle);
                self.lifecycle = Lifecycle::ReopeningWorkspace;
            }
            Lifecycle::ReopeningWorkspace | Lifecycle::RefreshingCatalog => {
                self.lifecycle = Lifecycle::ReopeningWorkspace;
            }
            Lifecycle::OpeningWorkspace
            | Lifecycle::LoadingInitializeCatalog
            | Lifecycle::InitializationFailed => {}
        }
    }

    /// Reattach the retained workspace, falling back to a service-minted replacement.
    pub fn bridge_connected(&mut self, correlation: &mut Correlation) -> Effects {
        if self.lifecycle != Lifecycle::ReopeningWorkspace {
            return Effects::default();
        }
        let sequence = correlation
            .track(PendingRequest::background(PendingKind::ReopenWorkspace2025))
            .expect("background requests carry no client id");
        Effects::service(EdgeMessage::OpenWorkspace {
            sequence,
            workspace: self.workspace.clone(),
            context: self.context.clone(),
        })
    }

    /// Render a truthful failure for a pending operation retired on disconnect or write failure.
    pub fn bridge_failure(&mut self, disconnected: DisconnectedPending, reason: &str) -> Effects {
        let pending = disconnected.pending;
        if matches!(
            pending.kind,
            PendingKind::OpenWorkspace2025 | PendingKind::InitializeCatalog2025
        ) {
            self.lifecycle = Lifecycle::InitializationFailed;
        }
        if matches!(
            pending.kind,
            PendingKind::ReopenWorkspace2025 | PendingKind::ReconnectCatalog2025
        ) {
            self.lifecycle = Lifecycle::ReopeningWorkspace;
        }
        if pending.suppressed {
            return Effects::default();
        }
        let Some(id) = pending.request_id else {
            return Effects::default();
        };
        let (code, message) = if disconnected.may_have_started {
            (
                OUTCOME_UNKNOWN,
                "Ghostlight lost the service connection after this operation may have started; it was not replayed",
            )
        } else {
            (
                SERVICE_UNAVAILABLE,
                "Ghostlight could not deliver this operation to its local service",
            )
        };
        Effects::output(error_response(
            Some(&id),
            code,
            message,
            Some(json!({"detail": reason})),
        ))
    }

    /// Cleanly release the implicit workspace at stdio EOF or parent death.
    pub fn shutdown(&mut self, correlation: &mut Correlation) -> Effects {
        let Some(workspace) = self.workspace.take() else {
            return Effects::default();
        };
        let Ok(sequence) = correlation.track(PendingRequest::release()) else {
            return Effects::default();
        };
        Effects::service(EdgeMessage::ReleaseWorkspace {
            sequence,
            workspace,
        })
    }

    fn open_workspace(
        &mut self,
        id: RequestId,
        context: RequestContext,
        preferred: Option<WorkspaceId>,
        correlation: &mut Correlation,
    ) -> Result<Effects, Value> {
        let sequence = correlation
            .track(PendingRequest::request(
                id.clone(),
                PendingKind::OpenWorkspace2025,
            ))
            .map_err(|message| error_response(Some(&id), INVALID_REQUEST, message, None))?;
        Ok(Effects::service(EdgeMessage::OpenWorkspace {
            sequence,
            workspace: preferred,
            context,
        }))
    }

    fn background_catalog(&self, workspace: WorkspaceId, correlation: &mut Correlation) -> Effects {
        let sequence = correlation
            .track(PendingRequest::background(
                PendingKind::ReconnectCatalog2025,
            ))
            .expect("background requests carry no client id");
        Effects::service(EdgeMessage::Catalog {
            sequence,
            workspace: Some(workspace),
            context: self.context.clone(),
        })
    }

    fn catalog_request(
        &self,
        id: RequestId,
        workspace: WorkspaceId,
        context: RequestContext,
        kind: PendingKind,
        correlation: &mut Correlation,
    ) -> Effects {
        let sequence = match correlation.track(PendingRequest::request(id.clone(), kind)) {
            Ok(sequence) => sequence,
            Err(message) => {
                return Effects::output(error_response(Some(&id), INVALID_REQUEST, message, None));
            }
        };
        Effects::service(EdgeMessage::Catalog {
            sequence,
            workspace: Some(workspace),
            context,
        })
    }

    fn tools_list(&mut self, request: &Request, correlation: &mut Correlation) -> Effects {
        let Some(id) = request.id.clone() else {
            return Effects::default();
        };
        if request
            .params
            .as_object()
            .is_some_and(|params| params.contains_key("cursor"))
        {
            return Effects::output(error_response(
                Some(&id),
                INVALID_PARAMS,
                "Ghostlight did not issue this pagination cursor",
                None,
            ));
        }
        if let Some(projection) = &self.catalog {
            return Effects::output(success_response(&id, list_tools_result(projection)));
        }
        let Some(workspace) = self.workspace.clone() else {
            return Effects::output(error_response(
                Some(&id),
                LIFECYCLE_ERROR,
                "the implicit Ghostlight workspace is unavailable",
                None,
            ));
        };
        self.catalog_request(
            id,
            workspace,
            self.context.clone(),
            PendingKind::ListTools2025,
            correlation,
        )
    }

    fn tools_call(&self, request: &Request, correlation: &mut Correlation) -> Effects {
        let Some(id) = request.id.clone() else {
            return Effects::default();
        };
        let Some(params) = request.params.as_object() else {
            return Effects::output(error_response(
                Some(&id),
                INVALID_PARAMS,
                "tools/call params must be an object",
                None,
            ));
        };
        let Some(external_tool) = params.get("name").and_then(Value::as_str) else {
            return Effects::output(error_response(
                Some(&id),
                INVALID_PARAMS,
                "tools/call requires a string name",
                None,
            ));
        };
        let arguments = match params.get("arguments") {
            None => json!({}),
            Some(Value::Object(arguments)) => Value::Object(arguments.clone()),
            Some(_) => {
                return Effects::output(error_response(
                    Some(&id),
                    INVALID_PARAMS,
                    "tools/call arguments must be an object",
                    None,
                ));
            }
        };
        let operation = match surface::decode_call(
            McpRevision::Mcp2025_11_25,
            external_tool,
            arguments.clone(),
        ) {
            Ok(operation) => operation,
            Err(error) => {
                return Effects::output(success_response(
                    &id,
                    tool_error_result(None, &error.to_string()),
                ));
            }
        };
        let Some(workspace) = self.workspace.clone() else {
            return Effects::output(error_response(
                Some(&id),
                LIFECYCLE_ERROR,
                "the implicit Ghostlight workspace is unavailable",
                None,
            ));
        };
        let sequence = match correlation.track(PendingRequest::tool_request(
            id.clone(),
            PendingKind::CallTool2025,
            operation.kind(),
            Some(workspace.clone()),
        )) {
            Ok(sequence) => sequence,
            Err(message) => {
                return Effects::output(error_response(Some(&id), INVALID_REQUEST, message, None));
            }
        };
        Effects::service(EdgeMessage::Start {
            sequence,
            operation,
            workspace: Some(workspace),
            context: self.context.clone(),
        })
    }
}

fn parse_initialize(request: &Request) -> Result<(RequestId, RequestContext), Value> {
    let Some(id) = request.id.clone() else {
        return Err(error_response(
            None,
            INVALID_REQUEST,
            "initialize must be a request with an id",
            None,
        ));
    };
    let Some(params) = request.params.as_object() else {
        return Err(error_response(
            Some(&id),
            INVALID_PARAMS,
            "initialize params must be an object",
            None,
        ));
    };
    if params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .is_none()
    {
        return Err(error_response(
            Some(&id),
            INVALID_PARAMS,
            "initialize requires protocolVersion",
            None,
        ));
    }
    if !params.get("capabilities").is_some_and(Value::is_object) {
        return Err(error_response(
            Some(&id),
            INVALID_PARAMS,
            "initialize requires a capabilities object",
            None,
        ));
    }
    let Some(client_info) = params.get("clientInfo").and_then(Value::as_object) else {
        return Err(error_response(
            Some(&id),
            INVALID_PARAMS,
            "initialize requires a clientInfo object",
            None,
        ));
    };
    let Some(name) = client_info.get("name").and_then(Value::as_str) else {
        return Err(error_response(
            Some(&id),
            INVALID_PARAMS,
            "clientInfo.name must be a string",
            None,
        ));
    };
    let Some(version) = client_info.get("version").and_then(Value::as_str) else {
        return Err(error_response(
            Some(&id),
            INVALID_PARAMS,
            "clientInfo.version must be a string",
            None,
        ));
    };
    let restriction = restriction_from_meta(params.get("_meta"), Some(&id))?;
    Ok((
        id,
        RequestContext {
            client: Some(ClientPresentation {
                name: name.to_owned(),
                version: version.to_owned(),
            }),
            restriction,
        },
    ))
}

fn restriction_from_meta(
    meta: Option<&Value>,
    id: Option<&RequestId>,
) -> Result<Option<String>, Value> {
    let Some(meta) = meta else {
        return Ok(None);
    };
    let Some(meta) = meta.as_object() else {
        return Err(error_response(
            id,
            INVALID_PARAMS,
            "initialize _meta must be an object",
            None,
        ));
    };
    let restriction = meta
        .get("org.sylin/ghostlightSessionPolicy")
        .or_else(|| meta.get("ghostlightSessionPolicy"));
    match restriction {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(error_response(
            id,
            INVALID_PARAMS,
            "ghostlightSessionPolicy must be a string",
            None,
        )),
    }
}

fn cancellation(request: &Request, correlation: &mut Correlation) -> Effects {
    let request_id = request
        .params
        .as_object()
        .and_then(|params| params.get("requestId"))
        .and_then(RequestId::parse);
    request_id
        .as_ref()
        .and_then(|id| correlation.cancel(id))
        .map_or_else(Effects::default, Effects::service)
}

fn initialize_result(_projection: &CatalogProjection) -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {"tools": {"listChanged": true}},
        "serverInfo": {
            "name": Instance::resolve().mcp_server_name(),
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": format!(
            "{} {}",
            surface::agent_guide(),
            crate::TRANSPORT_CLOSED_RECOVERY_INSTRUCTIONS
        ),
    })
}

fn list_tools_result(projection: &CatalogProjection) -> Value {
    let tools = ghostlight::filtered_declarations(McpRevision::Mcp2025_11_25, projection);
    json!({
        "tools": tools
    })
}

fn response_for_pending(
    pending: PendingRequest,
    result: Value,
    lifecycle: Lifecycle,
    handler: &mut Handler,
) -> Effects {
    handler.lifecycle = lifecycle;
    if pending.suppressed {
        return Effects::default();
    }
    pending.request_id.map_or_else(Effects::default, |id| {
        Effects::output(success_response(&id, without_result_type(result)))
    })
}

fn render_outcome(pending: PendingRequest, outcome: TerminalOutcome) -> Effects {
    if pending.suppressed {
        return Effects::default();
    }
    let Some(id) = pending.request_id.clone() else {
        return Effects::default();
    };
    let result = outcome.result;
    if !pending.result_matches_expected_operation(&result) {
        return Effects::output(error_response(
            Some(&id),
            OUTCOME_UNKNOWN,
            "Ghostlight received a result for a different canonical operation. Do not retry automatically; reconnect the client before continuing.",
            Some(json!({"disposition": "outcome_unknown"})),
        ));
    }
    if let Err(message) = validated_result_workspace_2025(&pending, &result) {
        return Effects::output(error_response(
            Some(&id),
            OUTCOME_UNKNOWN,
            message,
            Some(json!({"disposition": "outcome_unknown"})),
        ));
    }
    let effect = result.effect;
    match surface::encode_result(surface::McpRevision::Mcp2025_11_25, *result) {
        Ok(result) => Effects::output(success_response(&id, without_result_type(result))),
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
            tool_error_result(None, &error.to_string()),
        )),
    }
}

fn validated_result_workspace_2025(
    pending: &PendingRequest,
    result: &ghostlight_transport::operation::BrowserResult,
) -> Result<(), &'static str> {
    let (Some(requested), Some(started), Some(returned)) = (
        pending.requested_workspace.as_ref(),
        pending.service_workspace.as_ref(),
        result.workspace.as_ref(),
    ) else {
        return Err(
            "Ghostlight received incomplete workspace facts after the operation. Do not retry automatically; reconnect the client before continuing.",
        );
    };
    if requested != started || started != returned {
        return Err(
            "Ghostlight received inconsistent workspace facts after the operation. Do not retry automatically; reconnect the client before continuing.",
        );
    }
    Ok(())
}

fn render_rejection(pending: PendingRequest, error: BridgeError) -> Effects {
    if pending.suppressed {
        return Effects::default();
    }
    let Some(id) = pending.request_id.clone() else {
        return Effects::default();
    };
    if pending.kind == PendingKind::CallTool2025 {
        let rendered = surface::encode_rejection(
            surface::McpRevision::Mcp2025_11_25,
            &error,
            pending.expected_operation,
            pending.requested_workspace.as_ref(),
        );
        return match rendered {
            Ok(result) => Effects::output(success_response(&id, without_result_type(result))),
            Err(message) => {
                Effects::output(success_response(&id, tool_error_result(None, &message)))
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

fn tool_error_result(result: Option<Value>, message: &str) -> Value {
    let mut object = result
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    object.remove("resultType");
    object.insert("isError".into(), Value::Bool(true));
    object
        .entry("content")
        .or_insert_with(|| json!([{"type": "text", "text": message}]));
    Value::Object(object)
}

fn without_result_type(mut result: Value) -> Value {
    if let Some(object) = result.as_object_mut() {
        object.remove("resultType");
    }
    result
}

fn response_error(request: &Request, code: i64, message: impl Into<String>) -> Effects {
    request.id.as_ref().map_or_else(Effects::default, |id| {
        Effects::output(error_response(Some(id), code, message, None))
    })
}
