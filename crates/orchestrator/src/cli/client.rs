//! The command line as a relay protocol 3 peer of the service.
//!
//! `ghostlight call` is an MCP client like any other (ADR-0197 Decision 17): it opens an `mcp`
//! stream on the `cli` channel, names the process that owns its session, initializes, and then
//! lists or calls tools. `status`, `doctor` and `open` use `control` streams, which read readiness
//! or bring the workbench forward without opening a session. Nothing here starts a service.

use std::net::TcpStream;

use ghostlight_bridge::service::SessionMarker;
use ghostlight_relay::client::{self, ConnectError, REPLY_DEADLINE};
use ghostlight_relay::frame::{self, FrameError};
use ghostlight_relay::opening::{ControlOp, ControlResult, McpFrame, Reply, Stream};
use serde_json::{json, Value};
use thiserror::Error;

use crate::intake::listener::control;
use crate::intake::mcp::{method, LATEST_REVISION};
use crate::intake::CLI_CHANNEL;
use crate::workbench::ReadinessSummary;

/// The label controlled tabs are grouped under for scripted work.
///
/// One stable label per channel, not one per calling program: the tab strip has a fixed budget of
/// attention, so a script's tabs group together instead of multiplying groups (ADR-0105).
pub const CLI_CLIENT_LABEL: &str = "ghostlight call";

const JSONRPC_VERSION: &str = "2.0";

/// The MCP content block kind that carries an image.
const IMAGE_CONTENT: &str = "image";

/// Why the command line could not finish an exchange with the service.
#[derive(Debug, Error)]
pub enum ClientError {
    /// No service is running, or it could not be reached.
    #[error("Ghostlight is not running: {0}")]
    NoService(#[from] ConnectError),
    /// The service refused the connection, in its own words.
    #[error("{0}")]
    Refused(String),
    /// The service answered the request with an error, in its own words.
    #[error("{0}")]
    Rejected(String),
    /// The service closed the connection before it answered.
    #[error("the Ghostlight service closed the connection")]
    Closed,
    /// The service sent something the command line could not read.
    #[error("the Ghostlight service sent an unreadable answer: {0}")]
    Unreadable(String),
}

impl From<FrameError> for ClientError {
    fn from(error: FrameError) -> Self {
        Self::Unreadable(error.to_string())
    }
}

/// Open a stream, insisting that the service accepts it.
fn open(stream: Stream) -> Result<TcpStream, ClientError> {
    match client::connect(stream, REPLY_DEADLINE)? {
        (socket, Reply::Accepted { .. }) => Ok(socket),
        (_, Reply::Refused { message, .. }) => Err(ClientError::Refused(message)),
        (_, Reply::Unknown) => Err(ClientError::Unreadable("an unknown reply".into())),
    }
}

/// Run one control operation and return its result value.
///
/// # Errors
///
/// Returns a [`ClientError`] when no service answers or the exchange fails.
pub fn control(op: ControlOp) -> Result<Value, ClientError> {
    let mut socket = open(Stream::Control { op })?;
    socket
        .set_read_timeout(Some(REPLY_DEADLINE))
        .map_err(|error| ClientError::Unreadable(error.to_string()))?;
    match frame::read_frame::<ControlResult>(&mut socket)? {
        Some(ControlResult::Result { value }) => Ok(value),
        None => Err(ClientError::Closed),
    }
}

/// Read the running service's readiness without starting it or opening a session.
///
/// # Errors
///
/// Returns a [`ClientError`] when no service answers or the answer is not a readiness summary.
pub fn readiness() -> Result<ReadinessSummary, ClientError> {
    serde_json::from_value(control(ControlOp::Readiness)?)
        .map_err(|error| ClientError::Unreadable(error.to_string()))
}

/// Ask the running service to bring its workbench forward. Returns whether a workbench was there.
///
/// # Errors
///
/// Returns a [`ClientError`] when no service answers.
pub fn activate_workbench() -> Result<bool, ClientError> {
    Ok(control(ControlOp::ActivateWorkbench)?
        .get(control::AVAILABLE)
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

/// One finished tool call: the envelope and any images that came with it.
#[derive(Clone, Debug, PartialEq)]
pub struct Invocation {
    /// The structured result envelope.
    pub result: Value,
    /// Base64 image data, in the order the service sent it.
    pub images: Vec<String>,
}

/// One initialized MCP session with the service, used one request at a time.
#[derive(Debug)]
pub struct McpClient {
    socket: TcpStream,
    next_id: u64,
}

impl McpClient {
    /// Open an `mcp` stream on the `cli` channel and initialize the session.
    ///
    /// # Errors
    ///
    /// Returns a [`ClientError`] when no service answers, it refuses the session, or the
    /// initialization fails.
    pub fn connect(session: Option<SessionMarker>) -> Result<Self, ClientError> {
        let session = session
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| ClientError::Unreadable(error.to_string()))?;
        let socket = open(Stream::Mcp {
            channel: CLI_CHANNEL.into(),
            session,
            resume: None,
        })?;
        let mut client = Self { socket, next_id: 1 };
        client.request(
            method::INITIALIZE,
            json!({
                "protocolVersion": LATEST_REVISION,
                "capabilities": {},
                "clientInfo": {"name": CLI_CLIENT_LABEL, "version": env!("CARGO_PKG_VERSION")},
            }),
        )?;
        client.send(&json!({"jsonrpc": JSONRPC_VERSION, "method": method::INITIALIZED}))?;
        Ok(client)
    }

    /// The names of the tools the service offers this session.
    ///
    /// # Errors
    ///
    /// Returns a [`ClientError`] when the request fails.
    pub fn tool_names(&mut self) -> Result<Vec<String>, ClientError> {
        let listed = self.request(method::TOOLS_LIST, json!({}))?;
        Ok(listed
            .get("tools")
            .and_then(Value::as_array)
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(|tool| tool.get("name").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Call one tool and wait for its result.
    ///
    /// # Errors
    ///
    /// Returns a [`ClientError`] when the request fails or the service rejects it.
    pub fn call(&mut self, tool: &str, input: Value) -> Result<Invocation, ClientError> {
        let called = self.request(
            method::TOOLS_CALL,
            json!({"name": tool, "arguments": input}),
        )?;
        let images = called
            .get("content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|block| {
                        block.get("type").and_then(Value::as_str) == Some(IMAGE_CONTENT)
                    })
                    .filter_map(|block| block.get("data").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Ok(Invocation {
            result: called
                .get("structuredContent")
                .cloned()
                .unwrap_or(Value::Null),
            images,
        })
    }

    /// Send one request and return its result, skipping notices and resume tokens on the way.
    fn request(&mut self, name: &str, params: Value) -> Result<Value, ClientError> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(
            &json!({"jsonrpc": JSONRPC_VERSION, "id": id, "method": name, "params": params}),
        )?;
        loop {
            let Some(frame) = frame::read_frame::<McpFrame>(&mut self.socket)? else {
                return Err(ClientError::Closed);
            };
            let McpFrame::Message { line } = frame else {
                continue;
            };
            let message: Value = serde_json::from_str(&line)
                .map_err(|error| ClientError::Unreadable(error.to_string()))?;
            if message.get("id") != Some(&json!(id)) {
                continue;
            }
            if let Some(result) = message.get("result") {
                return Ok(result.clone());
            }
            let detail = message
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or_default();
            return Err(ClientError::Rejected(detail.into()));
        }
    }

    /// Write one MCP line to the service.
    fn send(&mut self, line: &Value) -> Result<(), ClientError> {
        frame::write_frame(
            &mut self.socket,
            &McpFrame::Message {
                line: line.to_string(),
            },
        )?;
        Ok(())
    }
}
