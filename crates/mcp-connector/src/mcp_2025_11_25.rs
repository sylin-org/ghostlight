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
use crate::surface::ghostlight_legacy;
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
        let presentation =
            match ghostlight_legacy::invocation_presentation(external_tool, &arguments) {
                Ok(presentation) => presentation,
                Err(error) => {
                    return Effects::output(success_response(
                        &id,
                        tool_error_result(None, &error.to_string()),
                    ));
                }
            };
        let operation = match ghostlight_legacy::decode_call(external_tool, arguments.clone()) {
            Ok(operation) => operation,
            Err(error) => {
                return Effects::output(success_response(
                    &id,
                    tool_error_result(None, &error.to_string()),
                ));
            }
        };
        let flow_render_hints =
            ghostlight_legacy::flow_render_hints(external_tool, &arguments, &operation);
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
            presentation.clone(),
            operation.key(),
            flow_render_hints,
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
            presentation: Some(presentation),
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
            ghostlight_legacy::agent_guide(),
            crate::TRANSPORT_CLOSED_RECOVERY_INSTRUCTIONS
        ),
    })
}

fn list_tools_result(projection: &CatalogProjection) -> Value {
    json!({
        "tools": ghostlight_legacy::filtered_declarations(projection)
            .iter()
            .map(|tool| tool.declaration.clone())
            .collect::<Vec<_>>()
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
            if let Err(message) = validated_result_workspace_2025(&pending, &result) {
                return Effects::output(error_response(
                    Some(&id),
                    OUTCOME_UNKNOWN,
                    message,
                    Some(json!({"disposition": "outcome_unknown"})),
                ));
            }
            let effect = result.effect;
            match ghostlight_legacy::encode_result(
                *result,
                pending.presentation.as_ref(),
                pending.flow_render_hints.as_ref(),
            ) {
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
        TerminalOutcome::ToolFailure { result, message } => Effects::output(success_response(
            &id,
            tool_error_result(Some(result), &message),
        )),
        TerminalOutcome::NotDispatched { message }
        | TerminalOutcome::Denied { message, .. }
        | TerminalOutcome::Held { message }
        | TerminalOutcome::AttentionRequired { message } => {
            Effects::output(success_response(&id, tool_error_result(None, &message)))
        }
        TerminalOutcome::Cancelled {
            message,
            effect: OperationEffect::None,
        } => Effects::output(success_response(&id, tool_error_result(None, &message))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::bridge::{
        DenialSource, OperationAvailability, ServiceMessage, WorkId, WorkspaceUse,
    };
    use ghostlight_transport::operation::{
        BrowserResult, BrowserResultStatus, IntentId, OperationEffect, OperationId, OperationKey,
        ResultPart,
    };

    fn initialize(id: i64) -> Request {
        Request {
            id: Some(RequestId::Number(id.into())),
            method: "initialize".into(),
            params: json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "1.0"}
            }),
        }
    }

    fn projection() -> CatalogProjection {
        CatalogProjection {
            generation: 1,
            operations: vec![OperationAvailability {
                id: OperationId::BrowserTabs,
                intent: IntentId::TabsList,
                workspace_use: WorkspaceUse::Creates,
            }],
            restricted: false,
        }
    }

    fn empty_result() -> BrowserResult {
        BrowserResult::new(
            OperationId::BrowserTabs,
            IntentId::TabsList,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        )
    }

    fn operational_handler() -> Handler {
        Handler {
            lifecycle: Lifecycle::Operational,
            context: RequestContext {
                client: None,
                restriction: None,
            },
            workspace: Some(WorkspaceId::mint()),
            catalog: Some(projection()),
            resume_lifecycle: None,
        }
    }

    fn tool_call(id: i64, name: &str, arguments: Value) -> Request {
        Request {
            id: Some(RequestId::Number(id.into())),
            method: "tools/call".into(),
            params: json!({"name": name, "arguments": arguments}),
        }
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
        handler.on_resolution(resolution, correlation)
    }

    #[test]
    fn full_legacy_profile_transcript_is_exact_through_the_2025_handler() {
        let projection = ghostlight_legacy::test_support::full_projection(17);
        let workspace = WorkspaceId::mint();
        let mut correlation = Correlation::default();
        let (mut handler, opened) = Handler::select(&initialize(1), &mut correlation).unwrap();
        let EdgeMessage::OpenWorkspace { sequence, .. } = opened.service[0].clone() else {
            panic!("open workspace expected");
        };
        let crate::bridge::Observation::Resolved(resolution) =
            correlation.observe(ServiceMessage::WorkspaceOpened {
                sequence,
                workspace: workspace.clone(),
            })
        else {
            panic!("workspace resolution expected");
        };
        let catalog = handler.on_resolution(resolution, &mut correlation);
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
        let initialized = handler.on_resolution(resolution, &mut correlation);
        assert_eq!(initialized.output[0]["id"], 1);
        handler.handle(
            &Request {
                id: None,
                method: "notifications/initialized".into(),
                params: json!({}),
            },
            &mut correlation,
        );

        let listed = handler.handle(
            &Request {
                id: Some(RequestId::Number(2.into())),
                method: "tools/list".into(),
                params: json!({}),
            },
            &mut correlation,
        );
        assert_eq!(
            listed.output,
            vec![json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {"tools": ghostlight_legacy::declarations()["tools"].clone()},
            })]
        );

        let explained = complete_tool_call(
            &mut handler,
            &mut correlation,
            tool_call(3, "explain", json!({})),
            WorkId(30),
            |workspace| TerminalOutcome::Success {
                result: Box::new(ghostlight_legacy::test_support::context_result(workspace)),
            },
        );
        assert_eq!(
            explained.output,
            vec![json!({
                "jsonrpc": "2.0",
                "id": 3,
                "result": {
                    "content": [{
                        "type": "text",
                        "text": ghostlight_legacy::test_support::explain_text(),
                    }],
                },
            })]
        );

        let succeeded = complete_tool_call(
            &mut handler,
            &mut correlation,
            tool_call(4, "get_page_text", json!({"tabId": 7})),
            WorkId(40),
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
                "id": 4,
                "result": {
                    "content": [{"type": "text", "text": "Page text"}],
                    "structuredContent": {"characters": 9},
                },
            })]
        );

        let denied = complete_tool_call(
            &mut handler,
            &mut correlation,
            tool_call(5, "get_page_text", json!({"tabId": 7})),
            WorkId(50),
            |_| TerminalOutcome::Denied {
                message: "Blocked by test policy.".into(),
                source: DenialSource::Policy,
            },
        );
        assert_eq!(
            denied.output,
            vec![json!({
                "jsonrpc": "2.0",
                "id": 5,
                "result": {
                    "content": [{"type": "text", "text": "Blocked by test policy."}],
                    "isError": true,
                },
            })]
        );
    }

    #[test]
    fn strict_initialize_opens_workspace_then_catalog_then_waits_for_initialized() {
        let mut correlation = Correlation::default();
        let (mut handler, first) = Handler::select(&initialize(1), &mut correlation).unwrap();
        let EdgeMessage::OpenWorkspace { sequence, .. } = &first.service[0] else {
            panic!("open workspace expected");
        };
        let opened = correlation.observe(ServiceMessage::WorkspaceOpened {
            sequence: *sequence,
            workspace: WorkspaceId::mint(),
        });
        let crate::bridge::Observation::Resolved(resolution) = opened else {
            panic!("resolution expected");
        };
        let catalog = handler.on_resolution(resolution, &mut correlation);
        let EdgeMessage::Catalog { sequence, .. } = &catalog.service[0] else {
            panic!("catalog expected");
        };
        let projected = correlation.observe(ServiceMessage::Catalog {
            sequence: *sequence,
            projection: projection(),
        });
        let crate::bridge::Observation::Resolved(resolution) = projected else {
            panic!("resolution expected");
        };
        let initialized = handler.on_resolution(resolution, &mut correlation);
        assert_eq!(
            initialized.output[0]["result"]["protocolVersion"],
            PROTOCOL_VERSION
        );
        assert_eq!(
            initialized.output[0]["result"]["capabilities"],
            json!({"tools": {"listChanged": true}})
        );
        assert!(initialized.output[0]["result"].get("resultType").is_none());

        let before_notification = handler.handle(
            &Request {
                id: Some(RequestId::Number(2.into())),
                method: "tools/list".into(),
                params: json!({}),
            },
            &mut correlation,
        );
        assert_eq!(
            before_notification.output[0]["error"]["code"],
            LIFECYCLE_ERROR
        );

        handler.handle(
            &Request {
                id: None,
                method: "notifications/initialized".into(),
                params: json!({}),
            },
            &mut correlation,
        );
        let listed = handler.handle(
            &Request {
                id: Some(RequestId::Number(3.into())),
                method: "tools/list".into(),
                params: json!({}),
            },
            &mut correlation,
        );
        assert_eq!(
            listed.output[0]["result"]["tools"][0],
            ghostlight_legacy::declarations()["tools"][0],
            "the 2025 tools/list shore must preserve the canonical declaration exactly"
        );
        assert!(listed.output[0]["result"].get("resultType").is_none());
    }

    #[test]
    fn initialize_appends_exact_transport_closed_recovery_instructions() {
        let result = initialize_result(&projection());
        assert_eq!(
            result["instructions"],
            format!(
                "{} {}",
                ghostlight_legacy::agent_guide(),
                crate::TRANSPORT_CLOSED_RECOVERY_INSTRUCTIONS
            )
        );
    }

    #[test]
    fn ping_is_the_only_pre_operational_request_exception() {
        let mut correlation = Correlation::default();
        let (mut handler, _) = Handler::select(&initialize(1), &mut correlation).unwrap();
        let ping = handler.handle(
            &Request {
                id: Some(RequestId::String("p".into())),
                method: "ping".into(),
                params: json!({}),
            },
            &mut correlation,
        );
        assert_eq!(ping.output[0]["result"], json!({}));
    }

    #[test]
    fn cancellation_suppresses_the_terminal_response() {
        let pending = PendingRequest {
            request_id: Some(RequestId::Number(4.into())),
            kind: PendingKind::CallTool2025,
            service_workspace: None,
            requested_workspace: None,
            presentation: None,
            expected_operation: Some(OperationKey::new(
                OperationId::BrowserTabs,
                IntentId::TabsList,
            )),
            flow_render_hints: None,
            suppressed: true,
            delivered: true,
        };
        let effects = render_outcome(
            pending,
            TerminalOutcome::Success {
                result: Box::new(empty_result()),
            },
        );
        assert!(effects.output.is_empty());
    }

    #[test]
    fn mismatched_result_operation_fails_closed_before_profile_rendering() {
        let pending = PendingRequest {
            request_id: Some(RequestId::Number(5.into())),
            kind: PendingKind::CallTool2025,
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
    fn result_workspace_must_match_requested_and_started_workspace() {
        let requested = WorkspaceId::mint();
        let substituted = WorkspaceId::mint();
        let requested_raw = requested.as_str().to_owned();
        let substituted_raw = substituted.as_str().to_owned();
        let mut result = BrowserResult::new(
            OperationId::BrowserTabs,
            IntentId::TabsList,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        result.workspace = Some(substituted.clone());
        let effects = render_outcome(
            PendingRequest {
                request_id: Some(RequestId::Number(6.into())),
                kind: PendingKind::CallTool2025,
                service_workspace: Some(substituted),
                requested_workspace: Some(requested),
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
        assert!(!rendered.contains(&requested_raw));
        assert!(!rendered.contains(&substituted_raw));
    }

    #[test]
    fn cancellation_covers_the_full_multistage_initialize_attempt() {
        let mut correlation = Correlation::default();
        let (mut handler, first) = Handler::select(&initialize(1), &mut correlation).unwrap();
        let EdgeMessage::OpenWorkspace { sequence, .. } = first.service[0].clone() else {
            panic!("open workspace expected");
        };
        handler.handle(
            &Request {
                id: None,
                method: "notifications/cancelled".into(),
                params: json!({"requestId": 1}),
            },
            &mut correlation,
        );

        let opened = correlation.observe(ServiceMessage::WorkspaceOpened {
            sequence,
            workspace: WorkspaceId::mint(),
        });
        let crate::bridge::Observation::Resolved(opened) = opened else {
            panic!("workspace resolution expected");
        };
        let cancelled_before_catalog = handler.on_resolution(opened, &mut correlation);
        assert!(cancelled_before_catalog.output.is_empty());
        assert!(cancelled_before_catalog.service.is_empty());
        assert_eq!(handler.lifecycle, Lifecycle::InitializationFailed);

        let retry = handler.handle(&initialize(2), &mut correlation);
        assert!(matches!(
            retry.service.first(),
            Some(EdgeMessage::OpenWorkspace { .. })
        ));
    }

    #[test]
    fn cancellation_during_initialize_catalog_does_not_emit_initialize_result() {
        let mut correlation = Correlation::default();
        let (mut handler, first) = Handler::select(&initialize(1), &mut correlation).unwrap();
        let EdgeMessage::OpenWorkspace { sequence, .. } = first.service[0].clone() else {
            panic!("open workspace expected");
        };
        let opened = correlation.observe(ServiceMessage::WorkspaceOpened {
            sequence,
            workspace: WorkspaceId::mint(),
        });
        let crate::bridge::Observation::Resolved(opened) = opened else {
            panic!("workspace resolution expected");
        };
        let catalog = handler.on_resolution(opened, &mut correlation);
        let EdgeMessage::Catalog { sequence, .. } = catalog.service[0].clone() else {
            panic!("catalog expected");
        };

        handler.handle(
            &Request {
                id: None,
                method: "notifications/cancelled".into(),
                params: json!({"requestId": 1}),
            },
            &mut correlation,
        );
        let resolved = correlation.observe(ServiceMessage::Catalog {
            sequence,
            projection: projection(),
        });
        let crate::bridge::Observation::Resolved(resolved) = resolved else {
            panic!("catalog resolution expected");
        };
        let cancelled = handler.on_resolution(resolved, &mut correlation);
        assert!(cancelled.output.is_empty());
        assert!(cancelled.service.is_empty());
        assert_eq!(handler.lifecycle, Lifecycle::InitializationFailed);
    }

    #[test]
    fn id_bearing_cancellation_errors_without_cancelling() {
        let mut correlation = Correlation::default();
        let active_id = RequestId::Number(4.into());
        correlation
            .track(PendingRequest::request(
                active_id.clone(),
                PendingKind::CallTool2025,
            ))
            .unwrap();
        let effects = operational_handler().handle(
            &Request {
                id: Some(RequestId::Number(5.into())),
                method: "notifications/cancelled".into(),
                params: json!({"requestId": 4}),
            },
            &mut correlation,
        );

        assert_eq!(effects.output[0]["error"]["code"], INVALID_REQUEST);
        assert!(effects.service.is_empty());
        assert!(correlation.contains_request(&active_id));
    }

    #[test]
    fn tools_list_rejects_every_cursor_because_none_are_issued() {
        let effects = operational_handler().handle(
            &Request {
                id: Some(RequestId::Number(6.into())),
                method: "tools/list".into(),
                params: json!({"cursor": "not-issued"}),
            },
            &mut Correlation::default(),
        );

        assert_eq!(effects.output[0]["error"]["code"], INVALID_PARAMS);
        assert!(effects.service.is_empty());
    }

    #[test]
    fn unsupported_task_augmentation_is_ignored_and_call_starts_normally() {
        let mut handler = operational_handler();
        let expected_workspace = handler.workspace.clone();
        let effects = handler.handle(
            &Request {
                id: Some(RequestId::Number(7.into())),
                method: "tools/call".into(),
                params: json!({
                    "name": "computer",
                    "arguments": {"action":"left_click","tabId":7,"coordinate": [4, 8]},
                    "task": {"ttl": 60_000}
                }),
            },
            &mut Correlation::default(),
        );

        let EdgeMessage::Start {
            operation,
            workspace,
            ..
        } = &effects.service[0]
        else {
            panic!("start expected");
        };
        assert_eq!(operation.id, OperationId::BrowserInput);
        assert_eq!(operation.intent, IntentId::InputPointerClick);
        assert_eq!(workspace, &expected_workspace);
        assert_eq!(operation.arguments["point"], json!([4, 8]));
    }

    #[test]
    fn reconnect_reuses_or_replaces_workspace_then_refreshes_catalog() {
        let (_, context) = parse_initialize(&initialize(1)).unwrap();
        let retained = WorkspaceId::mint();
        let replacement = WorkspaceId::mint();
        let mut handler = Handler {
            lifecycle: Lifecycle::Operational,
            context,
            workspace: Some(retained.clone()),
            catalog: Some(projection()),
            resume_lifecycle: None,
        };
        let mut correlation = Correlation::default();

        handler.bridge_disconnected();
        let reopened = handler.bridge_connected(&mut correlation);
        let EdgeMessage::OpenWorkspace {
            sequence,
            workspace,
            ..
        } = &reopened.service[0]
        else {
            panic!("workspace reopen expected");
        };
        assert_eq!(workspace.as_ref(), Some(&retained));
        let resolved = correlation.observe(ServiceMessage::WorkspaceOpened {
            sequence: *sequence,
            workspace: replacement.clone(),
        });
        let crate::bridge::Observation::Resolved(resolved) = resolved else {
            panic!("reopen resolution expected");
        };
        let refresh = handler.on_resolution(resolved, &mut correlation);
        let EdgeMessage::Catalog { sequence, .. } = &refresh.service[0] else {
            panic!("catalog refresh expected");
        };
        let resolved = correlation.observe(ServiceMessage::Catalog {
            sequence: *sequence,
            projection: projection(),
        });
        let crate::bridge::Observation::Resolved(resolved) = resolved else {
            panic!("catalog resolution expected");
        };
        let notification = handler.on_resolution(resolved, &mut correlation);

        assert_eq!(handler.workspace.as_ref(), Some(&replacement));
        assert_eq!(handler.lifecycle, Lifecycle::Operational);
        assert_eq!(
            notification.output[0]["method"],
            "notifications/tools/list_changed"
        );
    }

    #[test]
    fn initialization_retry_reattaches_or_replaces_a_retained_workspace_first() {
        let (_, context) = parse_initialize(&initialize(1)).unwrap();
        let retained = WorkspaceId::mint();
        let replacement = WorkspaceId::mint();
        let mut handler = Handler {
            lifecycle: Lifecycle::InitializationFailed,
            context,
            workspace: Some(retained.clone()),
            catalog: None,
            resume_lifecycle: None,
        };
        let mut correlation = Correlation::default();

        let retry = handler.handle(&initialize(2), &mut correlation);
        let EdgeMessage::OpenWorkspace {
            sequence,
            workspace,
            ..
        } = &retry.service[0]
        else {
            panic!("workspace reopen expected");
        };
        assert_eq!(workspace.as_ref(), Some(&retained));

        let resolution = correlation.observe(ServiceMessage::WorkspaceOpened {
            sequence: *sequence,
            workspace: replacement.clone(),
        });
        let crate::bridge::Observation::Resolved(resolution) = resolution else {
            panic!("workspace resolution expected");
        };
        let catalog = handler.on_resolution(resolution, &mut correlation);
        let EdgeMessage::Catalog {
            workspace: projected,
            ..
        } = &catalog.service[0]
        else {
            panic!("catalog request expected");
        };
        assert_eq!(projected.as_ref(), Some(&replacement));
        assert_eq!(handler.lifecycle, Lifecycle::LoadingInitializeCatalog);
    }
}
