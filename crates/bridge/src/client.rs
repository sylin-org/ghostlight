//! The one handshake home for the local service bridge, plus a blocking single-flight client.
//!
//! The bridge is a documented versioned contract (ADR-0105 Decision 4), so connection
//! establishment belongs in one place rather than being reimplemented by each edge. Every
//! caller -- the blocking CLI client and the MCP edge's reconnecting session -- negotiates
//! through [`connect_split`], which is also where the bridge major compatibility checks live.

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

/// Stable reason code shared with the service's own hello major refusal.
const INCOMPATIBLE_BRIDGE: &str = "incompatible_bridge";

/// One established service connection split for a concurrent reader and writer half.
///
/// Long-lived edges pump inbound frames from [`Connection::reader`] on their own thread while
/// writing requests through [`Connection::writer`]; the negotiation itself stays in this crate.
#[derive(Debug)]
pub struct Connection {
    /// Outbound half for request writes.
    pub writer: TcpStream,
    /// Inbound half buffered for response and event reads.
    pub reader: BufReader<TcpStream>,
    /// Opaque workspace session handle assigned at hello.
    pub session: String,
    /// Product metadata the orchestrator published at hello.
    pub server: ServerProfile,
    /// Orchestrator-owned catalog as of negotiation.
    pub catalog: Vec<ToolDefinition>,
}

/// Dial the endpoint published by a runtime discovery file, negotiate the versioned hello and
/// initial catalog, and return the split connection.
///
/// This is the single home of the service handshake (ADR-0105 Decision 4): it refuses an
/// incompatible runtime before dialing, refuses an incompatible `HelloAccepted` major, and maps
/// every other pre-invocation refusal to [`ClientError::Refused`].
pub fn connect_split(
    runtime_file: &Path,
    client_label: &str,
    channel: IntakeChannel,
    session: Option<SessionMarker>,
) -> Result<Connection, ClientError> {
    let endpoint = read_runtime(runtime_file).map_err(|_| ClientError::NoService)?;
    if endpoint.service_bridge_major != SERVICE_BRIDGE_MAJOR {
        return Err(ClientError::Refused {
            code: INCOMPATIBLE_BRIDGE.into(),
            message: "runtime service bridge major is incompatible".into(),
        });
    }
    let mut writer = TcpStream::connect(("127.0.0.1", endpoint.service_port))
        .map_err(|_| ClientError::NoService)?;
    writer.set_nodelay(true).map_err(ClientError::Transport)?;
    let hello = ServiceRequest::Hello {
        major: SERVICE_BRIDGE_MAJOR,
        token: endpoint.token,
        client_label: client_label.into(),
        channel,
        session,
    };
    write_json_line(&mut writer, &hello).map_err(|_| ClientError::Protocol)?;
    let mut reader = BufReader::new(writer.try_clone().map_err(ClientError::Transport)?);
    let accepted = match read_json_line::<ServiceResponse>(&mut reader) {
        Ok(Some(ServiceResponse::HelloAccepted {
            major,
            session,
            server,
        })) => {
            if major != SERVICE_BRIDGE_MAJOR {
                return Err(ClientError::Refused {
                    code: INCOMPATIBLE_BRIDGE.into(),
                    message: format!("service bridge major {major} is incompatible"),
                });
            }
            (session, server)
        }
        Ok(Some(ServiceResponse::Error { code, message, .. })) => {
            return Err(ClientError::Refused { code, message });
        }
        Ok(Some(_)) | Err(_) => return Err(ClientError::Protocol),
        Ok(None) => return Err(ClientError::Closed),
    };
    write_json_line(&mut writer, &ServiceRequest::Catalog).map_err(|_| ClientError::Protocol)?;
    let catalog = match read_json_line::<ServiceResponse>(&mut reader) {
        Ok(Some(ServiceResponse::Catalog { tools })) => tools,
        Ok(Some(ServiceResponse::Error { code, message, .. })) => {
            return Err(ClientError::Refused { code, message });
        }
        Ok(Some(_)) | Err(_) => return Err(ClientError::Protocol),
        Ok(None) => return Err(ClientError::Closed),
    };
    Ok(Connection {
        writer,
        reader,
        session: accepted.0,
        server: accepted.1,
        catalog,
    })
}

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
        let connection = connect_split(runtime_file, client_label, channel, session)?;
        Ok(Self {
            reader: connection.reader,
            writer: connection.writer,
            session: connection.session,
            server: connection.server,
            next_id: 1,
        })
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

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::thread;

    use serde_json::json;

    use super::{connect_split, ClientError, ServiceClient};
    use crate::framing::{read_json_line, write_json_line};
    use crate::runtime::{write_runtime, RuntimeEndpoint};
    use crate::service::{
        IntakeChannel, ServerProfile, ServiceRequest, ServiceResponse, ToolDefinition,
        SERVICE_BRIDGE_MAJOR,
    };

    fn unique_runtime_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ghostlight-client-{label}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn endpoint(port: u16) -> RuntimeEndpoint {
        RuntimeEndpoint {
            service_port: port,
            browser_port: port,
            token: "runtime_test".into(),
            service_bridge_major: SERVICE_BRIDGE_MAJOR,
            browser_relay_major: 1,
            service_version: "1.0.0".into(),
        }
    }

    fn one_tool() -> ToolDefinition {
        ToolDefinition {
            name: "browser_read".into(),
            description: "Read bounded page text.".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations: None,
        }
    }

    fn profile() -> ServerProfile {
        ServerProfile {
            name: "Ghostlight".into(),
            version: "1.0.0".into(),
            instructions: String::new(),
        }
    }

    /// Answer `connections` hellos and catalogs; each connection may also send one invoke.
    fn serve(
        listener: TcpListener,
        connections: usize,
        hello_reply: ServiceResponse,
        catalog_tools: Vec<ToolDefinition>,
    ) {
        for stream in listener.incoming().take(connections).flatten() {
            let mut writer = match stream.try_clone() {
                Ok(writer) => writer,
                Err(_) => continue,
            };
            let mut reader = std::io::BufReader::new(stream);
            let hello_ok = matches!(
                read_json_line::<ServiceRequest>(&mut reader),
                Ok(Some(ServiceRequest::Hello { .. }))
            );
            if !hello_ok || write_json_line(&mut writer, &hello_reply).is_err() {
                continue;
            }
            if matches!(hello_reply, ServiceResponse::Error { .. }) {
                continue;
            }
            if !matches!(
                read_json_line::<ServiceRequest>(&mut reader),
                Ok(Some(ServiceRequest::Catalog))
            ) {
                continue;
            }
            if write_json_line(
                &mut writer,
                &ServiceResponse::Catalog {
                    tools: catalog_tools.clone(),
                },
            )
            .is_err()
            {
                continue;
            }
            if let Some(ServiceRequest::Invoke { id, .. }) =
                read_json_line::<ServiceRequest>(&mut reader).ok().flatten()
            {
                let _ = write_json_line(
                    &mut writer,
                    &ServiceResponse::Result {
                        id,
                        text: "Read 12 words from example.com.".into(),
                        result: json!({"status": "succeeded"}),
                        is_error: false,
                        content: Vec::new(),
                    },
                );
                let _ = writer.flush();
            }
        }
    }

    #[test]
    fn negotiation_is_the_single_home_for_hello_catalog_and_invoke() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let runtime_path = unique_runtime_path("negotiation");
        write_runtime(
            &runtime_path,
            &endpoint(listener.local_addr().unwrap().port()),
        )
        .unwrap();
        let server = thread::spawn(move || {
            serve(
                listener,
                2,
                ServiceResponse::HelloAccepted {
                    major: SERVICE_BRIDGE_MAJOR,
                    session: "workspace_test".into(),
                    server: profile(),
                },
                vec![one_tool()],
            )
        });
        let connection =
            connect_split(&runtime_path, "test-client", IntakeChannel::Cli, None).unwrap();
        assert_eq!(connection.session, "workspace_test");
        assert_eq!(connection.server.name, "Ghostlight");
        assert_eq!(connection.catalog.len(), 1);
        drop(connection);

        let mut client =
            ServiceClient::connect(&runtime_path, "test-client", IntakeChannel::Cli, None).unwrap();
        let invocation = client.invoke("browser_read", json!({}), None).unwrap();
        assert!(!invocation.is_error);
        assert_eq!(invocation.text, "Read 12 words from example.com.");
        assert_eq!(client.session(), "workspace_test");

        server.join().unwrap();
        drop(client);
        let _ = std::fs::remove_file(&runtime_path);
    }

    #[test]
    fn incompatible_runtime_major_refuses_before_dialing() {
        let runtime_path = unique_runtime_path("major");
        let mut endpoint = endpoint(1);
        endpoint.service_bridge_major = SERVICE_BRIDGE_MAJOR + 1;
        write_runtime(&runtime_path, &endpoint).unwrap();
        let error = connect_split(&runtime_path, "test-client", IntakeChannel::Mcp, None)
            .expect_err("incompatible runtime must refuse");
        match error {
            ClientError::Refused { code, message } => {
                assert_eq!(code, "incompatible_bridge");
                assert!(message.contains("runtime"));
            }
            other => panic!("expected refusal, got {other:?}"),
        }
        let _ = std::fs::remove_file(&runtime_path);
    }

    #[test]
    fn hello_refusal_carries_the_service_code() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let runtime_path = unique_runtime_path("refusal");
        write_runtime(
            &runtime_path,
            &endpoint(listener.local_addr().unwrap().port()),
        )
        .unwrap();
        let server = thread::spawn(move || {
            serve(
                listener,
                1,
                ServiceResponse::Error {
                    id: None,
                    code: "channel_denied".into(),
                    message: "Configured authority does not admit the cli intake channel.".into(),
                },
                Vec::new(),
            )
        });
        let error = connect_split(&runtime_path, "test-client", IntakeChannel::Cli, None)
            .expect_err("refused hello must fail");
        match error {
            ClientError::Refused { code, message } => {
                assert_eq!(code, "channel_denied");
                assert!(message.contains("cli"));
            }
            other => panic!("expected refusal, got {other:?}"),
        }
        server.join().unwrap();
        let _ = std::fs::remove_file(&runtime_path);
    }
}
