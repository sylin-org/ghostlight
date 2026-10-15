//! The MCP session engine: one client session, from JSON-RPC lines to service requests and back.
//!
//! [`McpSession::accept`] reads one client line (a JSON-RPC message or a batch array) and returns
//! what to do now: a [`Step::Reply`] line to write to the client, or a [`Step::Forward`] request
//! for the session queue. [`McpSession::render`] turns the queue's answers back into MCP lines.
//! The engine negotiates the protocol revision, echoes every client id with its JSON type, maps
//! client ids to service ids for cancellation, and keeps the replies to one batch together.
//!
//! A tool result carries the envelope once, in `structuredContent`, as the lean model view with
//! fields at their default omitted (ADR-0198 Decision 21); its text block is the service's summary
//! and next steps, and tool definitions carry no `outputSchema` (ADR-0197 Decision 17). A session
//! negotiated at a revision before 2025-06-18 cannot read `structuredContent`, so its text block
//! also carries the same lean JSON after `Result: ` (ADR-0198 Decision 24). [`ResumeState`] is what lets a session continue across a service restart
//! (Decision 12), and [`failure_templates`] holds the answers a connector gives when it cannot
//! reach the service (Decision 2).

use std::collections::{HashMap, VecDeque};

use crate::intake::session::{
    ServerProfile, ServiceContent, ServiceRequest, ServiceResponse, ToolDefinition,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ghostlight_relay::templates::DEFAULT_TEMPLATE;
use ghostlight_relay::RELAY;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;
use uuid::Uuid;

use super::server_profile;
use crate::language::audit_health::Storage;
use crate::language::intake::{ConnectorFailure, ProtocolRefusal};
use crate::work::result::{Effect, Readiness, Status};

mod lean;

/// The newest MCP revision this engine serves, offered when a client asks for an unknown one.
pub const LATEST_REVISION: &str = "2025-11-25";
/// Every initialized MCP revision this engine serves, oldest first.
pub const SUPPORTED_REVISIONS: [&str; 4] =
    ["2024-11-05", "2025-03-26", "2025-06-18", LATEST_REVISION];
/// The first MCP revision whose clients read `structuredContent`; earlier ones get the lean JSON
/// in the text block as well (ADR-0198 Decision 24).
const STRUCTURED_CONTENT_REVISION: &str = "2025-06-18";
/// What introduces the lean JSON in the text block of a result for an older revision.
const TEXT_RESULT_LABEL: &str = "

Result: ";
/// The MCP revision whose stateless `server/discover` request this engine answers.
pub const DISCOVERY_REVISION: &str = "2026-07-28";

const JSONRPC_VERSION: &str = "2.0";
const SERVICE_ID_PREFIX: &str = "mcp_";
const DEFAULT_CLIENT_LABEL: &str = "MCP client";
const MAX_CLIENT_LABEL_CHARS: usize = 100;
const RESUME_TOKEN_PREFIX: &str = "rs1.";
const MAX_RESUME_TOKEN_BYTES: usize = 64 * 1024;

/// The JSON-RPC methods this engine knows.
pub mod method {
    /// MCP 2026-07-28 stateless discovery.
    pub const DISCOVER: &str = "server/discover";
    /// Session initialization.
    pub const INITIALIZE: &str = "initialize";
    /// The client's confirmation that initialization finished.
    pub const INITIALIZED: &str = "notifications/initialized";
    /// The client gave up on an earlier request.
    pub const CANCELLED: &str = "notifications/cancelled";
    /// Liveness check.
    pub const PING: &str = "ping";
    /// Catalog retrieval.
    pub const TOOLS_LIST: &str = "tools/list";
    /// Tool invocation.
    pub const TOOLS_CALL: &str = "tools/call";
    /// The server's notice that the catalog changed.
    pub const TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
}

/// The JSON-RPC error codes this engine answers with.
pub mod code {
    /// The line is not JSON.
    pub const PARSE_ERROR: i64 = -32700;
    /// The message is not a valid request.
    pub const INVALID_REQUEST: i64 = -32600;
    /// The method is unknown.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// The parameters are invalid.
    pub const INVALID_PARAMS: i64 = -32602;
    /// The service failed the request.
    pub const INTERNAL_ERROR: i64 = -32603;
    /// A request arrived before `initialize`.
    pub const NOT_INITIALIZED: i64 = -32002;
}

/// JSON pointers into the MCP 2026-07-28 discovery request.
mod discovery {
    pub(super) const VERSION: &str = "/params/_meta/io.modelcontextprotocol~1protocolVersion";
    pub(super) const CAPABILITIES: &str =
        "/params/_meta/io.modelcontextprotocol~1clientCapabilities";
    pub(super) const CLIENT_NAME: &str = "/params/_meta/io.modelcontextprotocol~1clientInfo/name";
    pub(super) const SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";
}

/// MCP content block kinds a tool result carries.
mod content {
    pub(super) const TEXT: &str = "text";
    pub(super) const IMAGE: &str = "image";
}

/// Reasons a failure template's structured content names.
mod reason {
    pub(super) const CUT_OFF: &str = "service_restarted";
    pub(super) const EXPIRED: &str = "service_not_started";
}

/// What the holder of the stream does with one result of [`McpSession::accept`].
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Write this JSON-RPC line to the client now.
    Reply(Value),
    /// Send this request to the session queue.
    Forward(ServiceRequest),
}

/// Where a session stands in the MCP lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    /// No `initialize` yet.
    Fresh,
    /// `initialize` answered; waiting for `notifications/initialized`.
    Initialized,
    /// The client confirmed initialization; catalog notices flow.
    Ready,
}

/// A client waiting for the catalog the service will answer next.
#[derive(Clone, Debug)]
struct CatalogWaiter {
    /// The client's request id.
    id: Value,
    /// The batch the request came in, when it came in one.
    batch: Option<u64>,
}

/// The replies to one batch, held until every forwarded catalog request in it is answered.
#[derive(Clone, Debug, Default)]
struct Batch {
    replies: Vec<Value>,
    waiting: usize,
}

/// What one message asks of the engine: at most one reply and at most one forward.
#[derive(Default)]
struct Handled {
    reply: Option<Value>,
    forward: Option<ServiceRequest>,
}

impl Handled {
    fn reply(value: Value) -> Self {
        Self {
            reply: Some(value),
            forward: None,
        }
    }

    fn forward(request: ServiceRequest) -> Self {
        Self {
            reply: None,
            forward: Some(request),
        }
    }
}

/// One MCP client session, independent of the stream that carries it.
#[derive(Debug)]
pub struct McpSession {
    server: ServerProfile,
    channel: String,
    phase: Phase,
    revision: Option<&'static str>,
    client_label: String,
    session_key: Option<String>,
    /// Running tool calls: service id to the client's id.
    calls: HashMap<String, Value>,
    /// Running tool calls: the client's id, as JSON text, to the service id.
    call_ids: HashMap<String, String>,
    catalog_waiters: VecDeque<CatalogWaiter>,
    batches: HashMap<u64, Batch>,
    next_batch: u64,
}

impl McpSession {
    /// A new session for a client that reached the connector through `channel`.
    #[must_use]
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            server: server_profile(),
            channel: channel.into(),
            phase: Phase::Fresh,
            revision: None,
            client_label: DEFAULT_CLIENT_LABEL.into(),
            session_key: None,
            calls: HashMap::new(),
            call_ids: HashMap::new(),
            catalog_waiters: VecDeque::new(),
            batches: HashMap::new(),
            next_batch: 0,
        }
    }

    /// A session continued from a resume token: initialized, with its revision, client and key.
    #[must_use]
    pub fn resumed(state: ResumeState) -> Self {
        let mut session = Self::new(state.channel);
        session.phase = Phase::Ready;
        session.revision = Some(negotiate(&state.revision));
        if !state.client_label.is_empty() {
            session.client_label = bounded_label(&state.client_label);
        }
        session.session_key = state.session_key;
        session
    }

    /// How the client reached the connector.
    #[must_use]
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// The client's own name, bounded, for attribution and presentation only.
    #[must_use]
    pub fn client_label(&self) -> &str {
        &self.client_label
    }

    /// The negotiated MCP revision, once `initialize` has been answered.
    #[must_use]
    pub const fn revision(&self) -> Option<&'static str> {
        self.revision
    }

    /// Whether `initialize` has been answered, so requests are served.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.phase != Phase::Fresh
    }

    /// The workspace session key this session is bound to, when it is bound.
    #[must_use]
    pub fn session_key(&self) -> Option<&str> {
        self.session_key.as_deref()
    }

    /// Bind the session to the workspace session the service opened for it.
    pub fn bind_session(&mut self, key: impl Into<String>) {
        self.session_key = Some(key.into());
    }

    /// What a later connection needs to continue this session, once it is initialized.
    #[must_use]
    pub fn resume_state(
        &self,
        owned_tabs: Vec<OwnedTab>,
        recording: Option<String>,
    ) -> Option<ResumeState> {
        Some(ResumeState {
            revision: self.revision?.into(),
            client_label: self.client_label.clone(),
            channel: self.channel.clone(),
            session_key: self.session_key.clone(),
            owned_tabs,
            recording,
        })
    }

    /// Read one client line and say what to do with it.
    ///
    /// A line that is not JSON is answered with a parse error and a null id. A batch array is
    /// handled element by element: its synchronous replies go out together as one array line,
    /// held until the catalog requests in it are answered, while its tool results go out one by
    /// one as they finish.
    pub fn accept(&mut self, line: &str) -> Vec<Step> {
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(error) => {
                return vec![Step::Reply(refusal(
                    Value::Null,
                    &ProtocolRefusal::Parse(error.to_string()),
                ))];
            }
        };
        match message {
            Value::Array(elements) => self.accept_batch(elements),
            message => {
                let handled = self.handle(message, None);
                let mut steps = Vec::new();
                steps.extend(handled.forward.map(Step::Forward));
                steps.extend(handled.reply.map(Step::Reply));
                steps
            }
        }
    }

    /// Turn one answer from the session queue into the lines to write to the client.
    pub fn render(&mut self, response: ServiceResponse) -> Vec<Value> {
        match response {
            ServiceResponse::Catalog { tools } => self.render_catalog(tools),
            ServiceResponse::CatalogChanged { .. } if self.phase == Phase::Ready => {
                vec![json!({"jsonrpc": JSONRPC_VERSION, "method": method::TOOLS_LIST_CHANGED})]
            }
            ServiceResponse::Result {
                id,
                text,
                result,
                is_error,
                content,
            } => self
                .finish_call(&id)
                .map(|client_id| {
                    let revision = self.revision.unwrap_or(LATEST_REVISION);
                    success(
                        client_id,
                        call_tool_result(revision, text, result, is_error, content),
                    )
                })
                .into_iter()
                .collect(),
            ServiceResponse::Error {
                id: Some(id),
                code: service_code,
                message,
            } => self
                .finish_call(&id)
                .map(|client_id| {
                    rpc_error(
                        client_id,
                        code::INTERNAL_ERROR,
                        &ProtocolRefusal::Service {
                            code: service_code,
                            message,
                        },
                    )
                })
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    fn accept_batch(&mut self, elements: Vec<Value>) -> Vec<Step> {
        if elements.is_empty() {
            return vec![Step::Reply(refusal(
                Value::Null,
                &ProtocolRefusal::EmptyBatch,
            ))];
        }
        let batch_id = self.next_batch;
        self.next_batch += 1;
        let mut batch = Batch::default();
        let mut steps = Vec::new();
        for element in elements {
            let handled = self.handle(element, Some(batch_id));
            if let Some(forward) = handled.forward {
                if forward == ServiceRequest::Catalog {
                    batch.waiting += 1;
                }
                steps.push(Step::Forward(forward));
            }
            batch.replies.extend(handled.reply);
        }
        if batch.waiting > 0 {
            self.batches.insert(batch_id, batch);
        } else if !batch.replies.is_empty() {
            steps.push(Step::Reply(Value::Array(batch.replies)));
        }
        steps
    }

    fn handle(&mut self, message: Value, batch: Option<u64>) -> Handled {
        let Value::Object(fields) = message else {
            return Handled::reply(refusal(Value::Null, &ProtocolRefusal::InvalidRequest));
        };
        let id = fields.get("id").cloned();
        if fields.get("jsonrpc").and_then(Value::as_str) != Some(JSONRPC_VERSION) {
            return Handled::reply(refusal(
                id.unwrap_or(Value::Null),
                &ProtocolRefusal::InvalidRequest,
            ));
        }
        let Some(name) = fields
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            if fields.contains_key("result") || fields.contains_key("error") {
                return Handled::default();
            }
            return Handled::reply(refusal(
                id.unwrap_or(Value::Null),
                &ProtocolRefusal::InvalidRequest,
            ));
        };
        let message = Value::Object(fields);
        match id {
            None => self.notify(&name, &message),
            Some(id) => self.request(&name, id, &message, batch),
        }
    }

    fn notify(&mut self, name: &str, message: &Value) -> Handled {
        match name {
            method::INITIALIZED => {
                if self.phase == Phase::Initialized {
                    self.phase = Phase::Ready;
                }
                Handled::default()
            }
            method::CANCELLED => message
                .pointer("/params/requestId")
                .and_then(|request_id| self.call_ids.get(&id_key(request_id)))
                .map(|service_id| {
                    Handled::forward(ServiceRequest::Cancel {
                        id: service_id.clone(),
                    })
                })
                .unwrap_or_default(),
            _ => Handled::default(),
        }
    }

    fn request(&mut self, name: &str, id: Value, message: &Value, batch: Option<u64>) -> Handled {
        match name {
            method::DISCOVER => Handled::reply(self.discover(id, message)),
            method::INITIALIZE => Handled::reply(self.initialize(id, message)),
            method::PING => Handled::reply(success(id, json!({}))),
            _ if self.phase == Phase::Fresh => {
                Handled::reply(refusal(id, &ProtocolRefusal::NotInitialized))
            }
            method::TOOLS_LIST => {
                self.catalog_waiters.push_back(CatalogWaiter { id, batch });
                Handled::forward(ServiceRequest::Catalog)
            }
            method::TOOLS_CALL => self.call(id, message),
            _ => Handled::reply(refusal(id, &ProtocolRefusal::MethodNotFound)),
        }
    }

    fn discover(&mut self, id: Value, message: &Value) -> Value {
        if id.is_null() {
            return refusal(id, &ProtocolRefusal::DiscoveryNullId);
        }
        if message.pointer(discovery::VERSION).and_then(Value::as_str) != Some(DISCOVERY_REVISION) {
            return refusal(id, &ProtocolRefusal::DiscoveryVersion(DISCOVERY_REVISION));
        }
        if !message
            .pointer(discovery::CAPABILITIES)
            .is_some_and(Value::is_object)
        {
            return refusal(id, &ProtocolRefusal::DiscoveryCapabilities);
        }
        if self.phase == Phase::Fresh {
            if let Some(name) = message
                .pointer(discovery::CLIENT_NAME)
                .and_then(Value::as_str)
            {
                self.client_label = bounded_label(name);
            }
        }
        let mut meta = Map::new();
        meta.insert(
            discovery::SERVER_INFO.into(),
            json!({"name": self.server.name, "version": self.server.version}),
        );
        success(
            id,
            json!({
                "resultType": "complete",
                "supportedVersions": SUPPORTED_REVISIONS,
                "capabilities": {"tools": {"listChanged": true}},
                "_meta": meta,
                "instructions": self.server.instructions,
                "ttlMs": 0,
                "cacheScope": "private"
            }),
        )
    }

    fn initialize(&mut self, id: Value, message: &Value) -> Value {
        if self.phase != Phase::Fresh {
            return refusal(id, &ProtocolRefusal::AlreadyInitialized);
        }
        let Some(requested) = message
            .pointer("/params/protocolVersion")
            .and_then(Value::as_str)
        else {
            return refusal(id, &ProtocolRefusal::MissingProtocolVersion);
        };
        let revision = negotiate(requested);
        if let Some(name) = message
            .pointer("/params/clientInfo/name")
            .and_then(Value::as_str)
        {
            self.client_label = bounded_label(name);
        }
        self.revision = Some(revision);
        self.phase = Phase::Initialized;
        success(
            id,
            json!({
                "protocolVersion": revision,
                "capabilities": {"tools": {"listChanged": true}},
                "serverInfo": {"name": self.server.name, "version": self.server.version},
                "instructions": self.server.instructions
            }),
        )
    }

    fn call(&mut self, id: Value, message: &Value) -> Handled {
        let Some(tool) = message.pointer("/params/name").and_then(Value::as_str) else {
            return Handled::reply(refusal(id, &ProtocolRefusal::MissingToolName));
        };
        let input = message
            .pointer("/params/arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !input.is_object() {
            return Handled::reply(refusal(id, &ProtocolRefusal::ArgumentsNotObject));
        }
        let key = id_key(&id);
        if self.call_ids.contains_key(&key) {
            return Handled::reply(refusal(id, &ProtocolRefusal::DuplicateId));
        }
        let service_id = format!("{SERVICE_ID_PREFIX}{}", Uuid::new_v4().simple());
        self.call_ids.insert(key, service_id.clone());
        self.calls.insert(service_id.clone(), id);
        Handled::forward(ServiceRequest::Invoke {
            id: service_id,
            tool: tool.into(),
            input,
            deadline_ms: None,
        })
    }

    fn finish_call(&mut self, service_id: &str) -> Option<Value> {
        let client_id = self.calls.remove(service_id)?;
        self.call_ids.remove(&id_key(&client_id));
        Some(client_id)
    }

    fn render_catalog(&mut self, tools: Vec<ToolDefinition>) -> Vec<Value> {
        let Some(waiter) = self.catalog_waiters.pop_front() else {
            return Vec::new();
        };
        let reply = success(waiter.id, json!({"tools": tools}));
        let Some(batch_id) = waiter.batch else {
            return vec![reply];
        };
        let Some(batch) = self.batches.get_mut(&batch_id) else {
            return vec![reply];
        };
        batch.replies.push(reply);
        batch.waiting = batch.waiting.saturating_sub(1);
        if batch.waiting > 0 {
            return Vec::new();
        }
        self.batches
            .remove(&batch_id)
            .map(|batch| vec![Value::Array(batch.replies)])
            .unwrap_or_default()
    }
}

/// One tab a session owns, as the resume token carries it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OwnedTab {
    /// The browser holding the tab.
    pub browser_id: String,
    /// The physical tab id inside that browser.
    pub tab_id: u64,
    /// The model-facing tab handle.
    pub handle: String,
}

/// What a session needs to continue after a service restart (ADR-0197 Decision 12).
///
/// It travels as an opaque token: the connector keeps it and hands it back on reconnect.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResumeState {
    /// The negotiated MCP revision.
    pub revision: String,
    /// The client's own name, for attribution and presentation only. A token may omit it.
    #[serde(default)]
    pub client_label: String,
    /// How the client reached the connector. A token may omit it; the opening names it too.
    #[serde(default)]
    pub channel: String,
    /// The workspace session key, when the session was bound to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
    /// The tabs the session owned.
    #[serde(default)]
    pub owned_tabs: Vec<OwnedTab>,
    /// The recording the session had running, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording: Option<String>,
}

/// Why a resume token was not accepted.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ResumeError {
    /// The token does not carry this engine's token prefix.
    #[error("resume token has an unknown format")]
    Format,
    /// The token is longer than any this engine writes.
    #[error("resume token is too long")]
    TooLong,
    /// The token is not base64url.
    #[error("resume token is not base64url")]
    Encoding,
    /// The token does not decode to a resume state.
    #[error("resume token does not hold a resume state")]
    Content,
    /// The token names an MCP revision this engine does not serve.
    #[error("resume token names an unsupported MCP revision")]
    Revision,
}

impl ResumeState {
    /// Encode the state as an opaque token: a version prefix and base64url JSON.
    #[must_use]
    pub fn encode(&self) -> String {
        let json = serde_json::to_vec(self).unwrap_or_default();
        format!("{RESUME_TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
    }

    /// Decode a token written by [`ResumeState::encode`].
    ///
    /// # Errors
    ///
    /// Returns a [`ResumeError`] when the token is malformed, too long, or names a revision this
    /// engine does not serve.
    pub fn decode(token: &str) -> Result<Self, ResumeError> {
        if token.len() > MAX_RESUME_TOKEN_BYTES {
            return Err(ResumeError::TooLong);
        }
        let body = token
            .strip_prefix(RESUME_TOKEN_PREFIX)
            .ok_or(ResumeError::Format)?;
        let json = URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| ResumeError::Encoding)?;
        let state: Self = serde_json::from_slice(&json).map_err(|_| ResumeError::Content)?;
        if !SUPPORTED_REVISIONS.contains(&state.revision.as_str()) {
            return Err(ResumeError::Revision);
        }
        Ok(state)
    }
}

/// Echo a served MCP revision, or counteroffer the latest.
#[must_use]
pub fn negotiate(requested: &str) -> &'static str {
    SUPPORTED_REVISIONS
        .into_iter()
        .find(|candidate| *candidate == requested)
        .unwrap_or(LATEST_REVISION)
}

/// The failure templates a connector answers with when it cannot reach the service.
///
/// Shaped as relay protocol 3's `relay-templates.json`: for each failure, a JSON-RPC response for
/// `tools/call` and a `default` for every other request, each with a null id the connector fills.
#[must_use]
pub fn failure_templates() -> Value {
    json!({
        "relay": RELAY,
        "cut_off": failure_entries(ConnectorFailure::CutOff),
        "expired": failure_entries(ConnectorFailure::Expired),
    })
}

fn failure_entries(failure: ConnectorFailure) -> Value {
    let mut entries = Map::new();
    entries.insert(
        method::TOOLS_CALL.into(),
        success(Value::Null, failure_call_result(failure)),
    );
    entries.insert(
        DEFAULT_TEMPLATE.into(),
        json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": Value::Null,
            "error": {"code": code::INTERNAL_ERROR, "message": failure.request_message()}
        }),
    );
    Value::Object(entries)
}

fn failure_call_result(failure: ConnectorFailure) -> Value {
    let (status, effect, readiness, repeat_safe, cause) = match failure {
        ConnectorFailure::CutOff => (
            Status::Unknown,
            Effect::Unknown,
            Readiness::Unknown,
            false,
            reason::CUT_OFF,
        ),
        ConnectorFailure::Expired => (
            Status::Failed,
            Effect::None,
            Readiness::NotApplicable,
            true,
            reason::EXPIRED,
        ),
    };
    let summary = failure.call_summary();
    let next_steps = failure.call_next_steps();
    let envelope = json!({
        "status": status,
        "effect": effect,
        "readiness": readiness,
        "repeat_safe": repeat_safe,
        "summary": summary,
        "history_storage": Storage::Unconfirmed,
        "facts": {"reason": cause},
        "next_steps": next_steps,
    });
    // A template is built before any client negotiates a revision and the connector fills only its
    // id, so it is written for the latest revision: structured content only, no text copy.
    call_tool_result(
        LATEST_REVISION,
        failure.call_text(),
        envelope,
        true,
        Vec::new(),
    )
}

/// Build a `tools/call` result. The envelope travels as lean `structuredContent`; for a revision
/// that predates it, the same JSON is also appended to the first text block.
fn call_tool_result(
    revision: &str,
    mut text: String,
    envelope: Value,
    is_error: bool,
    images: Vec<ServiceContent>,
) -> Value {
    let lean = lean::project(&envelope);
    if revision < STRUCTURED_CONTENT_REVISION {
        text.push_str(TEXT_RESULT_LABEL);
        text.push_str(&lean.to_string());
    }
    let mut blocks = vec![json!({"type": content::TEXT, "text": text})];
    for item in images {
        match item {
            ServiceContent::Image { mime_type, data } => {
                blocks.push(json!({"type": content::IMAGE, "mimeType": mime_type, "data": data}));
            }
        }
    }
    json!({"content": blocks, "structuredContent": lean, "isError": is_error})
}

fn success(id: Value, result: Value) -> Value {
    json!({"jsonrpc": JSONRPC_VERSION, "id": id, "result": result})
}

fn refusal(id: Value, why: &ProtocolRefusal) -> Value {
    let error_code = match why {
        ProtocolRefusal::Parse(_) => code::PARSE_ERROR,
        ProtocolRefusal::InvalidRequest
        | ProtocolRefusal::EmptyBatch
        | ProtocolRefusal::AlreadyInitialized
        | ProtocolRefusal::DuplicateId => code::INVALID_REQUEST,
        ProtocolRefusal::NotInitialized => code::NOT_INITIALIZED,
        ProtocolRefusal::MethodNotFound => code::METHOD_NOT_FOUND,
        ProtocolRefusal::MissingProtocolVersion
        | ProtocolRefusal::DiscoveryNullId
        | ProtocolRefusal::DiscoveryVersion(_)
        | ProtocolRefusal::DiscoveryCapabilities
        | ProtocolRefusal::MissingToolName
        | ProtocolRefusal::ArgumentsNotObject => code::INVALID_PARAMS,
        ProtocolRefusal::Service { .. } => code::INTERNAL_ERROR,
    };
    rpc_error(id, error_code, why)
}

fn rpc_error(id: Value, error_code: i64, why: &ProtocolRefusal) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {"code": error_code, "message": why.message()}
    })
}

/// A client id as JSON text, so `1` and `"1"` stay different ids.
fn id_key(id: &Value) -> String {
    serde_json::to_string(id).unwrap_or_default()
}

fn bounded_label(label: &str) -> String {
    label.chars().take(MAX_CLIENT_LABEL_CHARS).collect()
}

#[cfg(test)]
mod tests;
