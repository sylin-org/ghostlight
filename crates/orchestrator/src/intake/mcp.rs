//! The MCP session engine: one client session, from JSON-RPC lines to service requests and back.
//!
//! [`McpSession::accept`] reads one client line (a JSON-RPC message or a batch array) and returns
//! what to do now: a [`Step::Reply`] line to write to the client, or a [`Step::Forward`] request
//! for the session queue. [`McpSession::render`] turns the queue's answers back into MCP lines.
//! The engine negotiates the protocol revision, echoes every client id with its JSON type, maps
//! client ids to invocation ids for cancellation, and keeps the replies to one batch together.
//!
//! A tool result carries the envelope once, in `structuredContent`, as the lean model view with
//! fields at their default omitted (ADR-0198 Decision 21); its text block is the service's summary
//! and next steps, and tool definitions carry no `outputSchema` (ADR-0197 Decision 17). A session
//! negotiated at a revision before 2025-06-18 cannot read `structuredContent`, so its text block
//! also carries the same lean JSON after `Result: ` (ADR-0198 Decision 24). [`ResumeState`] is what lets a session continue across a service restart
//! (Decision 12), and [`failure_templates`] holds the answers an edge gives when it cannot
//! reach the service, or cannot forward a request too large for the relay (Decision 2).
//!
//! A session can stand by: its instructions then say that Ghostlight is already connected to this
//! agent, and the service answers its catalog with no tools until it is promoted. Whether it stands
//! by is the service's [`Seating`] decision, asked once with the name the client claims and, for a
//! session a resume token continues, whether its client was offered the tools (ADR-0203,
//! amendment of 2026-10-19).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

use crate::intake::session::{
    ServerProfile, ServiceContent, ServiceRequest, ServiceResponse, ToolDefinition,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ghostlight_relay::protocol::{Templates, DEFAULT_TEMPLATE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;

use super::server_profile;
use crate::language::audit_health::Storage;
use crate::language::intake::{EdgeFailure, ProtocolRefusal};
use crate::language::STANDBY_INSTRUCTIONS;
use crate::provenance::bounded_claim;
use crate::work::result::{Effect, Readiness, Status};
use crate::work::InvocationId;

mod lean;

/// The newest MCP revision this engine serves, offered when a client asks for an unknown one.
pub const LATEST_REVISION: &str = "2025-11-25";
/// The oldest MCP revision this engine serves. A resumed session whose revision this engine does
/// not serve, or whose token cannot be read, continues at it, because its results also carry the
/// lean JSON as text, which a client of any revision reads.
pub const OLDEST_REVISION: &str = "2024-11-05";
/// Every initialized MCP revision this engine serves, oldest first.
pub const SUPPORTED_REVISIONS: [&str; 4] = [
    OLDEST_REVISION,
    "2025-03-26",
    STRUCTURED_CONTENT_REVISION,
    LATEST_REVISION,
];
/// The first MCP revision whose clients read `structuredContent`; earlier ones get the lean JSON
/// in the text block as well (ADR-0198 Decision 24).
const STRUCTURED_CONTENT_REVISION: &str = "2025-06-18";
/// What introduces the lean JSON in the text block of a result for an older revision.
const TEXT_RESULT_LABEL: &str = "

Result: ";
/// The MCP revision whose stateless `server/discover` request this engine answers.
pub const DISCOVERY_REVISION: &str = "2026-07-28";

/// The JSON-RPC version every message names, the engine's and the command line's alike.
pub(crate) const JSONRPC_VERSION: &str = "2.0";
const RESUME_TOKEN_PREFIX: &str = "rs1.";
/// The longest resume token an opening may carry: [`ResumeState::decode`] reads none longer, and
/// [`ResumeState::encode`] writes none longer.
pub const MAX_RESUME_TOKEN_BYTES: usize = 64 * 1024;

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
pub(crate) mod content {
    pub(crate) const TEXT: &str = "text";
    pub(crate) const IMAGE: &str = "image";
}

/// The fields of a tool result the engine writes and the command line reads.
pub(crate) mod result_field {
    /// The content blocks: the text, and any images.
    pub(crate) const CONTENT: &str = "content";
    /// The lean JSON envelope.
    pub(crate) const STRUCTURED: &str = "structuredContent";
    /// Whether the call failed.
    pub(crate) const IS_ERROR: &str = "isError";
}

/// Reasons a failure template's structured content names.
mod reason {
    pub(super) const CUT_OFF: &str = "service_restarted";
    pub(super) const EXPIRED: &str = "service_not_started";
    pub(super) const TOO_LARGE: &str = "too_large";
}

/// The first relay revision whose MCP edge answers a request too large to forward from the
/// `too_large` template (ADR-0197 amendment of 2026-10-21). An older edge is handed revision 4's
/// templates, byte for byte.
const TOO_LARGE_RELAY: u16 = 5;

const _: () = assert!(
    TOO_LARGE_RELAY <= ghostlight_relay::RELAY,
    "the relay this service speaks carries the too_large template"
);

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

/// A client request waiting for the service's answer: a catalog listing or a tool call.
#[derive(Clone, Debug)]
struct Waiter {
    /// The client's request id.
    id: Value,
    /// The batch the request came in, when it came in one.
    batch: Option<u64>,
}

/// The replies to one batch, held until every catalog request and tool call it forwarded is
/// answered, so the batch is answered as JSON-RPC answers one: one array (S-D5).
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

/// The service's decision whether a session stands by, asked once with the bounded name its
/// client claims, or none, and, for a session a resume token continues, whether it stood by when
/// the token was written: true when the session must not offer its client the tools.
pub struct Seating(Box<StandsBy>);

/// The decision a [`Seating`] makes.
type StandsBy = dyn FnOnce(Option<&str>, Option<bool>) -> bool + Send;

impl Seating {
    /// The decision `stands_by` makes.
    pub fn new(
        stands_by: impl FnOnce(Option<&str>, Option<bool>) -> bool + Send + 'static,
    ) -> Self {
        Self(Box::new(stands_by))
    }
}

impl fmt::Debug for Seating {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Seating")
    }
}

/// One MCP client session, independent of the stream that carries it.
#[derive(Debug)]
pub struct McpSession {
    server: ServerProfile,
    phase: Phase,
    revision: Option<&'static str>,
    /// The name the client claimed for itself, bounded ([`bounded_claim`]); none when it sent none.
    claim: Option<String>,
    /// Whether the session stands by: another connection of the same agent offers the tools.
    standby: bool,
    /// The decision a fresh session asks when it initializes, until then.
    seating: Option<Seating>,
    /// Running tool calls: the invocation id to the client's request.
    calls: HashMap<InvocationId, Waiter>,
    /// Running tool calls: the client's id, as JSON text, to the invocation id.
    call_ids: HashMap<String, InvocationId>,
    catalog_waiters: VecDeque<Waiter>,
    batches: HashMap<u64, Batch>,
    next_batch: u64,
}

impl Default for McpSession {
    fn default() -> Self {
        Self::new()
    }
}

impl McpSession {
    /// A new session, waiting for its client's `initialize`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            server: server_profile(),
            phase: Phase::Fresh,
            revision: None,
            claim: None,
            standby: false,
            seating: None,
            calls: HashMap::new(),
            call_ids: HashMap::new(),
            catalog_waiters: VecDeque::new(),
            batches: HashMap::new(),
            next_batch: 0,
        }
    }

    /// A session continued from a resume token: initialized, with its revision, its client and
    /// whether it stood by, until its [`Seating`] decides.
    ///
    /// The client negotiated its revision with an earlier service, so it is never offered another
    /// one. A revision this engine does not serve continues at [`OLDEST_REVISION`].
    #[must_use]
    pub fn resumed(state: ResumeState) -> Self {
        let mut session = Self::new();
        session.phase = Phase::Ready;
        session.revision = Some(
            SUPPORTED_REVISIONS
                .into_iter()
                .find(|served| *served == state.revision)
                .unwrap_or(OLDEST_REVISION),
        );
        session.claim = state.claim.as_deref().and_then(bounded_claim);
        session.standby = state.standby;
        session
    }

    /// The name the client claimed for itself, bounded, for attribution and presentation only;
    /// none when it claimed none.
    #[must_use]
    pub fn claim(&self) -> Option<&str> {
        self.claim.as_deref()
    }

    /// Let `seating` decide whether this session stands by: now, with whether it stood by, for a
    /// session that is already initialized, which a resume token continued, and otherwise when its
    /// `initialize` arrives, before it is answered.
    pub fn seat(&mut self, seating: Seating) {
        if self.is_initialized() {
            self.standby = (seating.0)(self.claim(), Some(self.standby));
        } else {
            self.seating = Some(seating);
        }
    }

    /// Whether the session stands by, so its client is offered no tools.
    #[must_use]
    pub fn stands_by(&self) -> bool {
        self.standby
    }

    /// Stop standing by, because the connection that offered this agent the tools ended. Returns
    /// the line that tells the client its tool list changed, so it lists the tools again; none
    /// when the session did not stand by or its client has not finished initializing, since it
    /// then lists them anyway.
    pub fn promote(&mut self) -> Vec<Value> {
        let promoted = std::mem::take(&mut self.standby);
        if promoted && self.phase == Phase::Ready {
            vec![list_changed()]
        } else {
            Vec::new()
        }
    }

    /// The instructions this session's client is given.
    fn instructions(&self) -> &str {
        if self.standby {
            STANDBY_INSTRUCTIONS
        } else {
            &self.server.instructions
        }
    }

    /// Whether `initialize` has been answered, so requests are served.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.phase != Phase::Fresh
    }

    /// What a later connection needs to continue this session, once it is initialized: the tabs
    /// it owns, the extension life of each of their browsers that names one, and whether it
    /// stands by.
    #[must_use]
    pub fn resume_state(
        &self,
        owned_tabs: Vec<OwnedTab>,
        lives: BTreeMap<String, String>,
    ) -> Option<ResumeState> {
        Some(ResumeState {
            revision: self.revision?.into(),
            claim: self.claim.clone(),
            owned_tabs,
            lives,
            standby: self.standby,
        })
    }

    /// Read one client line and say what to do with it.
    ///
    /// A line that is not JSON is answered with a parse error and a null id. A batch array is
    /// handled element by element and answered as one array line, held until every catalog
    /// request and tool call in it is answered, its tool results included (S-D5).
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
            ServiceResponse::CatalogChanged if self.phase == Phase::Ready => {
                vec![list_changed()]
            }
            ServiceResponse::Result {
                id,
                text,
                result,
                is_error,
                content,
            } => {
                let Some(waiter) = self.finish_call(&id) else {
                    return Vec::new();
                };
                let revision = self.revision.unwrap_or(LATEST_REVISION);
                let reply = success(
                    waiter.id,
                    call_tool_result(revision, text, result, is_error, content),
                );
                self.answer(waiter.batch, reply)
            }
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
                if matches!(
                    forward,
                    ServiceRequest::Catalog | ServiceRequest::Invoke { .. }
                ) {
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
                .map(|invocation| {
                    Handled::forward(ServiceRequest::Cancel {
                        id: invocation.clone(),
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
                self.catalog_waiters.push_back(Waiter { id, batch });
                Handled::forward(ServiceRequest::Catalog)
            }
            method::TOOLS_CALL => self.call(id, message, batch),
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
                self.claim = bounded_claim(name);
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
                "instructions": self.instructions(),
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
            self.claim = bounded_claim(name);
        }
        if let Some(seating) = self.seating.take() {
            self.standby = (seating.0)(self.claim(), None);
        }
        self.revision = Some(revision);
        self.phase = Phase::Initialized;
        success(
            id,
            json!({
                "protocolVersion": revision,
                "capabilities": {"tools": {"listChanged": true}},
                "serverInfo": {"name": self.server.name, "version": self.server.version},
                "instructions": self.instructions()
            }),
        )
    }

    fn call(&mut self, id: Value, message: &Value, batch: Option<u64>) -> Handled {
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
        let invocation = InvocationId::mint();
        self.call_ids.insert(key, invocation.clone());
        self.calls.insert(invocation.clone(), Waiter { id, batch });
        Handled::forward(ServiceRequest::Invoke {
            id: invocation,
            tool: tool.into(),
            input,
        })
    }

    fn finish_call(&mut self, invocation: &InvocationId) -> Option<Waiter> {
        let waiter = self.calls.remove(invocation)?;
        self.call_ids.remove(&id_key(&waiter.id));
        Some(waiter)
    }

    fn render_catalog(&mut self, tools: Vec<ToolDefinition>) -> Vec<Value> {
        let Some(waiter) = self.catalog_waiters.pop_front() else {
            return Vec::new();
        };
        let reply = success(waiter.id, json!({"tools": tools}));
        self.answer(waiter.batch, reply)
    }

    /// The lines that carry `reply`: itself at once, or nothing while its batch still waits for
    /// another answer, and then the whole batch as one array.
    fn answer(&mut self, batch: Option<u64>, reply: Value) -> Vec<Value> {
        let Some(batch_id) = batch else {
            return vec![reply];
        };
        let Some(held) = self.batches.get_mut(&batch_id) else {
            return vec![reply];
        };
        held.replies.push(reply);
        held.waiting = held.waiting.saturating_sub(1);
        if held.waiting > 0 {
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
    /// The digest of the URL the session last knew for the tab, never the URL itself. The resumed
    /// binding waits until its browser lists a tab with this id at that URL. A token written
    /// before digests carries none, and its tab resumes stale.
    #[serde(default)]
    pub url_digest: Option<String>,
}

/// What a session needs to continue after a service restart (ADR-0197 Decision 12).
///
/// It travels as an opaque token: the MCP edge keeps it and hands it back on reconnect. The read is
/// tolerant: a field this build does not know, such as one an earlier build wrote, is ignored.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResumeState {
    /// The negotiated MCP revision.
    pub revision: String,
    /// The name the client claimed for itself, for attribution and presentation only; omitted
    /// when it claimed none. Read bounded, as the session reads it ([`bounded_claim`]).
    #[serde(
        default,
        rename = "client_label",
        skip_serializing_if = "Option::is_none"
    )]
    pub claim: Option<String>,
    /// The tabs the session owned.
    #[serde(default)]
    pub owned_tabs: Vec<OwnedTab>,
    /// The extension life each browser of `owned_tabs` was in when the token was written, by
    /// browser id (ADR-0203 Decision 1). A tab whose browser has no life here, as in a token an
    /// earlier build wrote, is confirmed by its URL digest.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lives: BTreeMap<String, String>,
    /// Whether the session stood by, so its client was offered no tools (ADR-0203, amendment of
    /// 2026-10-19). A resumed session keeps that role whatever order the edges reconnect in,
    /// since its client never drops a tool it was given. Omitted when it offered them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub standby: bool,
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
}

impl ResumeState {
    /// What an opening whose token cannot be read resumes (ADR-0203 Decision 1): the client
    /// initialized with an earlier service and will not initialize again, so its session continues
    /// at [`OLDEST_REVISION`], naming no client, owning no tab and offering the tools.
    #[must_use]
    pub fn unreadable() -> Self {
        Self {
            revision: OLDEST_REVISION.into(),
            claim: None,
            owned_tabs: Vec::new(),
            lives: BTreeMap::new(),
            standby: false,
        }
    }

    /// Encode the state as an opaque token: a version prefix and base64url JSON, never longer
    /// than [`MAX_RESUME_TOKEN_BYTES`].
    ///
    /// A state too large for that keeps its first tabs, as many as fit, and the lives of their
    /// browsers only. A tab left out is not restored after a restart, and its handle then reads as
    /// no longer open.
    #[must_use]
    pub fn encode(&self) -> String {
        let token = |tabs: usize| {
            let owned_tabs = self.owned_tabs[..tabs].to_vec();
            let lives = self
                .lives
                .iter()
                .filter(|(browser, _)| owned_tabs.iter().any(|tab| &tab.browser_id == *browser))
                .map(|(browser, life)| (browser.clone(), life.clone()))
                .collect();
            let kept = Self {
                owned_tabs,
                lives,
                ..self.clone()
            };
            let json = serde_json::to_vec(&kept).unwrap_or_default();
            format!("{RESUME_TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
        };
        let whole = token(self.owned_tabs.len());
        if whole.len() <= MAX_RESUME_TOKEN_BYTES {
            return whole;
        }
        // The most tabs that fit: `fits` always fits, since with no tab the state is a served
        // revision and a bounded claim with no life, and `over` never does.
        let (mut fits, mut over) = (0, self.owned_tabs.len());
        while over - fits > 1 {
            let middle = fits + (over - fits) / 2;
            if token(middle).len() <= MAX_RESUME_TOKEN_BYTES {
                fits = middle;
            } else {
                over = middle;
            }
        }
        token(fits)
    }

    /// Decode a token written by [`ResumeState::encode`]. A revision this engine does not serve
    /// still decodes; [`McpSession::resumed`] decides where it continues.
    ///
    /// # Errors
    ///
    /// Returns a [`ResumeError`] when the token is malformed or too long.
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
        serde_json::from_slice(&json).map_err(|_| ResumeError::Content)
    }
}

/// The notification that tells a client its tool list changed.
fn list_changed() -> Value {
    json!({"jsonrpc": JSONRPC_VERSION, "method": method::TOOLS_LIST_CHANGED})
}

/// Echo a served MCP revision, or counteroffer the latest.
#[must_use]
pub fn negotiate(requested: &str) -> &'static str {
    SUPPORTED_REVISIONS
        .into_iter()
        .find(|candidate| *candidate == requested)
        .unwrap_or(LATEST_REVISION)
}

/// The failure templates an edge of relay revision `relay` answers with when it cannot reach the
/// service, or cannot forward a request too large for the relay.
///
/// For each failure, a JSON-RPC response for `tools/call` and a `default` for every other request,
/// each with a null id the edge fills. The service hands them to every `mcp` stream in its
/// accepted reply. Only an edge of [`TOO_LARGE_RELAY`] or later reads `too_large`, so an older one
/// gets none, and its reply is revision 4's.
#[must_use]
pub fn failure_templates(relay: u16) -> Templates {
    Templates {
        cut_off: failure_entries(EdgeFailure::CutOff),
        expired: failure_entries(EdgeFailure::Expired),
        too_large: if relay >= TOO_LARGE_RELAY {
            failure_entries(EdgeFailure::TooLarge)
        } else {
            BTreeMap::new()
        },
    }
}

fn failure_entries(failure: EdgeFailure) -> BTreeMap<String, Value> {
    let mut entries = BTreeMap::new();
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
    entries
}

fn failure_call_result(failure: EdgeFailure) -> Value {
    let (status, effect, readiness, repeat_safe, cause) = match failure {
        EdgeFailure::CutOff => (
            Status::Unknown,
            Effect::Unknown,
            Readiness::Unknown,
            false,
            reason::CUT_OFF,
        ),
        EdgeFailure::Expired => (
            Status::Failed,
            Effect::None,
            Readiness::NotApplicable,
            true,
            reason::EXPIRED,
        ),
        EdgeFailure::TooLarge => (
            Status::Failed,
            Effect::None,
            Readiness::NotApplicable,
            true,
            reason::TOO_LARGE,
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
    // A template is built before any client negotiates a revision and the MCP edge fills only its
    // id, so it is written for the oldest revision: the lean JSON travels as structured content
    // and in the text block, which a client of every revision reads (S-D6).
    call_tool_result(
        OLDEST_REVISION,
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
    let mut result = serde_json::Map::new();
    result.insert(result_field::CONTENT.into(), Value::Array(blocks));
    result.insert(result_field::STRUCTURED.into(), lean);
    result.insert(result_field::IS_ERROR.into(), Value::Bool(is_error));
    Value::Object(result)
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

#[cfg(test)]
mod tests;
