//! One blocking client for the local service bridge, shared by every non-MCP edge.
//!
//! The bridge is a documented versioned contract (ADR-0105 Decision 4), so the handshake belongs in
//! one place rather than being reimplemented by each caller that wants to invoke a tool.

use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::path::Path;

use serde_json::Value;
use thiserror::Error;

use crate::framing::{read_json_line, write_json_line};
use crate::runtime::read_runtime;
use crate::service::{
    IntakeChannel, ServerProfile, ServiceContent, ServiceRequest, ServiceResponse, SessionMarker,
    ToolDefinition, SERVICE_BRIDGE_MAJOR,
};

/// A live authenticated session with the local service.
#[derive(Debug)]
pub struct ServiceClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    session: String,
    server: ServerProfile,
    next_id: u64,
}

impl ServiceClient {
    /// Connect to the service published by a runtime discovery file and complete the handshake.
    pub fn connect(
        runtime_file: &Path,
        client_label: &str,
        channel: IntakeChannel,
        session: Option<SessionMarker>,
    ) -> Result<Self, ClientError> {
        let endpoint = read_runtime(runtime_file).map_err(|_| ClientError::NoService)?;
        let stream = TcpStream::connect(("127.0.0.1", endpoint.service_port))
            .map_err(|_| ClientError::NoService)?;
        stream.set_nodelay(true).map_err(ClientError::Transport)?;
        let writer = stream.try_clone().map_err(ClientError::Transport)?;
        let mut client = Self {
            reader: BufReader::new(stream),
            writer,
            session: String::new(),
            server: ServerProfile::default(),
            next_id: 1,
        };
        let hello = ServiceRequest::Hello {
            major: SERVICE_BRIDGE_MAJOR,
            token: endpoint.token,
            client_label: client_label.into(),
            channel,
            session,
        };
        match client.exchange(&hello)? {
            ServiceResponse::HelloAccepted {
                session, server, ..
            } => {
                client.session = session;
                client.server = server;
                Ok(client)
            }
            ServiceResponse::Error { code, message, .. } => {
                Err(ClientError::Refused { code, message })
            }
            _ => Err(ClientError::Protocol),
        }
    }

    /// The opaque workspace handle this session was admitted as.
    #[must_use]
    pub fn session(&self) -> &str {
        &self.session
    }

    /// Product metadata the orchestrator published at hello.
    #[must_use]
    pub fn server(&self) -> &ServerProfile {
        &self.server
    }

    /// Retrieve the orchestrator-owned catalog.
    pub fn catalog(&mut self) -> Result<Vec<ToolDefinition>, ClientError> {
        match self.exchange(&ServiceRequest::Catalog)? {
            ServiceResponse::Catalog { tools } => Ok(tools),
            ServiceResponse::Error { code, message, .. } => {
                Err(ClientError::Refused { code, message })
            }
            _ => Err(ClientError::Protocol),
        }
    }

    /// Invoke one catalog tool and wait for its single terminal result.
    pub fn invoke(
        &mut self,
        tool: &str,
        input: Value,
        deadline_ms: Option<u64>,
    ) -> Result<Invocation, ClientError> {
        let id = format!("call_{}", self.next_id);
        self.next_id += 1;
        let request = ServiceRequest::Invoke {
            id: id.clone(),
            tool: tool.into(),
            input,
            deadline_ms,
        };
        match self.exchange(&request)? {
            ServiceResponse::Result {
                id: returned,
                text,
                result,
                is_error,
                content,
            } if returned == id => Ok(Invocation {
                text,
                result,
                is_error,
                content,
            }),
            ServiceResponse::Error { code, message, .. } => {
                Err(ClientError::Refused { code, message })
            }
            _ => Err(ClientError::Protocol),
        }
    }

    fn exchange(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ClientError> {
        write_json_line(&mut self.writer, request).map_err(|_| ClientError::Protocol)?;
        self.writer.flush().map_err(ClientError::Transport)?;
        read_json_line::<ServiceResponse>(&mut self.reader)
            .map_err(|_| ClientError::Protocol)?
            .ok_or(ClientError::Closed)
    }
}

/// One terminal result and any protocol-neutral content beside it.
#[derive(Clone, Debug)]
pub struct Invocation {
    /// Concise model-facing outcome authored by the orchestrator.
    pub text: String,
    /// The orchestrator's terminal product result.
    pub result: Value,
    /// Whether the product result reports an invocation failure.
    pub is_error: bool,
    /// Bounded content the edge renders itself.
    pub content: Vec<ServiceContent>,
}

/// A local service bridge failure, before or instead of a product result.
#[derive(Debug, Error)]
pub enum ClientError {
    /// No running service could be reached at the discovered endpoint.
    #[error("no running Ghostlight service")]
    NoService,
    /// The service refused the request before any invocation.
    #[error("{code}: {message}")]
    Refused {
        /// Stable bridge-level reason code.
        code: String,
        /// Bounded human-readable detail.
        message: String,
    },
    /// The service closed the session.
    #[error("the Ghostlight service closed the session")]
    Closed,
    /// The service answered with an incompatible frame.
    #[error("incompatible service bridge response")]
    Protocol,
    /// The local socket failed.
    #[error("local transport failed: {0}")]
    Transport(std::io::Error),
}
