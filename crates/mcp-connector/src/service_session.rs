//! Reconnecting protocol-neutral session to the local Ghostlight orchestrator.

use std::io::BufReader;
use std::net::TcpStream;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use ghostlight_bridge::framing::{read_json_line, write_json_line};
use ghostlight_bridge::lifecycle::request_orchestrator_start;
use ghostlight_bridge::runtime::{read_runtime, runtime_file};
use ghostlight_bridge::service::{
    IntakeChannel, ServerProfile, ServiceRequest, ServiceResponse, ToolDefinition,
    SERVICE_BRIDGE_MAJOR,
};

/// Protocol-neutral events emitted by the reconnecting service session.
pub enum ServiceEvent {
    /// A compatible service is ready with a current profile and catalog.
    Connected { catalog_changed: bool },
    /// The current service connection ended.
    Disconnected,
    /// An opaque service response arrived after negotiation.
    Response(ServiceResponse),
}

struct ConnectedSession {
    generation: u64,
    writer: Arc<Mutex<TcpStream>>,
    server: ServerProfile,
    catalog: Vec<ToolDefinition>,
}

#[derive(Default)]
struct SessionState {
    generation: u64,
    connected: Option<ConnectedSession>,
    previous_catalog: Option<Vec<ToolDefinition>>,
}

/// Reconnecting authenticated session to the orchestrator's generic edge bridge.
pub struct ServiceSession {
    state: Arc<(Mutex<SessionState>, Condvar)>,
}

impl ServiceSession {
    /// Start one reconnect loop for the lifetime of the MCP stdio process.
    pub fn start(
        client_label: String,
        event_handler: Arc<dyn Fn(ServiceEvent) + Send + Sync>,
    ) -> Result<Self> {
        let state = Arc::new((Mutex::new(SessionState::default()), Condvar::new()));
        let worker_state = Arc::clone(&state);
        thread::Builder::new()
            .name("ghostlight-service-session".into())
            .spawn(move || reconnect_loop(worker_state, client_label, event_handler))
            .context("spawn service session")?;
        Ok(Self { state })
    }

    /// Block initial MCP negotiation until orchestrator product metadata is available.
    pub fn wait_until_connected(&self) -> ServerProfile {
        let (state, connected) = &*self.state;
        let mut state = lock(state);
        loop {
            if let Some(session) = &state.connected {
                return session.server.clone();
            }
            state = connected
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Snapshot the current orchestrator-owned catalog, if the service is connected.
    pub fn catalog(&self) -> Option<Vec<ToolDefinition>> {
        lock(&self.state.0)
            .connected
            .as_ref()
            .map(|session| session.catalog.clone())
    }

    /// Send one generic bridge request without interpreting product payloads.
    pub fn send(&self, request: &ServiceRequest) -> Result<()> {
        let writer = lock(&self.state.0)
            .connected
            .as_ref()
            .map(|session| Arc::clone(&session.writer))
            .ok_or_else(|| anyhow::anyhow!("service is reconnecting"))?;
        let result = write_json_line(&mut *lock(&writer), request).context("write service request");
        result
    }
}

fn reconnect_loop(
    state: Arc<(Mutex<SessionState>, Condvar)>,
    client_label: String,
    event_handler: Arc<dyn Fn(ServiceEvent) + Send + Sync>,
) {
    let mut startup_error_reported = false;
    loop {
        let connection = connect(&client_label);
        let Ok((stream, reader, server, catalog)) = connection else {
            if let Err(error) = request_orchestrator_start() {
                if !startup_error_reported {
                    eprintln!("Ghostlight could not start its local orchestrator: {error}");
                    startup_error_reported = true;
                }
            }
            thread::sleep(Duration::from_millis(500));
            continue;
        };
        startup_error_reported = false;
        let writer = Arc::new(Mutex::new(stream));
        let (generation, catalog_changed) = {
            let mut locked = lock(&state.0);
            locked.generation = locked.generation.wrapping_add(1);
            let generation = locked.generation;
            let catalog_changed = locked
                .previous_catalog
                .as_ref()
                .is_some_and(|previous| previous != &catalog);
            locked.previous_catalog = Some(catalog.clone());
            locked.connected = Some(ConnectedSession {
                generation,
                writer,
                server,
                catalog,
            });
            state.1.notify_all();
            (generation, catalog_changed)
        };
        event_handler(ServiceEvent::Connected { catalog_changed });
        read_until_disconnected(reader, &event_handler);
        let was_current = {
            let mut locked = lock(&state.0);
            if locked
                .connected
                .as_ref()
                .is_some_and(|session| session.generation == generation)
            {
                locked.connected = None;
                true
            } else {
                false
            }
        };
        if was_current {
            event_handler(ServiceEvent::Disconnected);
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn connect(
    client_label: &str,
) -> Result<(
    TcpStream,
    BufReader<TcpStream>,
    ServerProfile,
    Vec<ToolDefinition>,
)> {
    let endpoint = read_runtime(&runtime_file()).context("read runtime endpoint")?;
    if endpoint.service_bridge_major != SERVICE_BRIDGE_MAJOR {
        bail!("runtime service bridge major is incompatible");
    }
    let mut stream = TcpStream::connect(("127.0.0.1", endpoint.service_port))
        .context("connect Ghostlight service")?;
    stream.set_nodelay(true)?;
    write_json_line(
        &mut stream,
        &ServiceRequest::Hello {
            major: SERVICE_BRIDGE_MAJOR,
            token: endpoint.token,
            client_label: client_label.into(),
            channel: IntakeChannel::Mcp,
        },
    )
    .context("send service hello")?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let server = match read_json_line::<ServiceResponse>(&mut reader)
        .context("read service hello")?
    {
        Some(ServiceResponse::HelloAccepted { major, server, .. })
            if major == SERVICE_BRIDGE_MAJOR =>
        {
            server
        }
        Some(ServiceResponse::Error { message, .. }) => bail!("service rejected edge: {message}"),
        _ => bail!("service returned an invalid hello response"),
    };
    write_json_line(&mut stream, &ServiceRequest::Catalog).context("request catalog")?;
    let catalog = match read_json_line::<ServiceResponse>(&mut reader).context("read catalog")? {
        Some(ServiceResponse::Catalog { tools }) => tools,
        Some(ServiceResponse::Error { message, .. }) => bail!("catalog failed: {message}"),
        _ => bail!("service returned an invalid catalog response"),
    };
    Ok((stream, reader, server, catalog))
}

fn read_until_disconnected(
    mut reader: BufReader<TcpStream>,
    event_handler: &Arc<dyn Fn(ServiceEvent) + Send + Sync>,
) {
    loop {
        match read_json_line::<ServiceResponse>(&mut reader) {
            Ok(Some(response)) => event_handler(ServiceEvent::Response(response)),
            Ok(None) | Err(_) => return,
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
