//! Reconnecting protocol-neutral session to the local Ghostlight orchestrator.

use std::io::BufReader;
use std::net::TcpStream;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use ghostlight_bridge::diagnostics::{event, Level, Sink};
use ghostlight_bridge::framing::{read_json_line, write_json_line};
use ghostlight_bridge::lifecycle::{request_orchestrator_start, StartDisposition};
use ghostlight_bridge::runtime::runtime_file;
use ghostlight_bridge::service::{
    IntakeChannel, ServerProfile, ServiceRequest, ServiceResponse, ToolDefinition,
};

/// Protocol-neutral events emitted by the reconnecting service session.
pub enum ServiceEvent {
    /// A compatible service is ready with a current profile and catalog.
    Connected { catalog_changed: bool },
    /// The current service changed its policy-projected catalog.
    CatalogChanged,
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
        diagnostics: Arc<Sink>,
        event_handler: Arc<dyn Fn(ServiceEvent) + Send + Sync>,
    ) -> Result<Self> {
        let state = Arc::new((Mutex::new(SessionState::default()), Condvar::new()));
        let worker_state = Arc::clone(&state);
        thread::Builder::new()
            .name("ghostlight-service-session".into())
            .spawn(move || reconnect_loop(worker_state, client_label, diagnostics, event_handler))
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
    diagnostics: Arc<Sink>,
    event_handler: Arc<dyn Fn(ServiceEvent) + Send + Sync>,
) {
    let mut startup_error_reported = false;
    let mut reported_disposition: Option<String> = None;
    loop {
        let connection = connect(&client_label);
        let Ok((stream, reader, server, catalog)) = connection else {
            match request_orchestrator_start() {
                Ok(disposition) => {
                    let note = match &disposition {
                        StartDisposition::Spawned { process_id } => (
                            event::DEMAND_START_SPAWNED,
                            format!("orchestrator pid {process_id}"),
                        ),
                        StartDisposition::AlreadyRunning => (
                            event::DEMAND_START_ALREADY_RUNNING,
                            "lease held; retrying connection".into(),
                        ),
                        StartDisposition::DeploymentInProgress => (
                            event::DEMAND_START_DEPLOYMENT_IN_PROGRESS,
                            "deploy lock present; startup quiesced".into(),
                        ),
                    };
                    let marker = format!("{}|{}", note.0, note.1);
                    if reported_disposition.as_deref() != Some(marker.as_str()) {
                        reported_disposition = Some(marker);
                        diagnostics.emit(note.0, Level::Info, None, &note.1);
                    }
                }
                Err(error) => {
                    if !startup_error_reported {
                        eprintln!("Ghostlight could not start its local orchestrator: {error}");
                        startup_error_reported = true;
                        diagnostics.emit(
                            event::DEMAND_START_FAILED,
                            Level::Warn,
                            None,
                            &format!("demand-start failed: {error}"),
                        );
                    }
                }
            }
            thread::sleep(Duration::from_millis(500));
            continue;
        };
        startup_error_reported = false;
        reported_disposition = None;
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
        diagnostics.emit(
            event::SERVICE_CONNECTED,
            Level::Info,
            None,
            &format!("{client_label} connected to the orchestrator"),
        );
        event_handler(ServiceEvent::Connected { catalog_changed });
        read_until_disconnected(reader, &state, generation, &event_handler);
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
            diagnostics.emit(
                event::SERVICE_DISCONNECTED,
                Level::Warn,
                None,
                &format!("{client_label} lost the orchestrator connection"),
            );
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
    // Negotiation lives once in the bridge (ADR-0105 Decision 4); the edge keeps only its
    // reconnect loop, event pump, and concurrent-request plumbing.
    let connection = ghostlight_bridge::client::connect_split(
        &runtime_file(),
        client_label,
        IntakeChannel::Mcp,
        // The MCP edge keeps its workspace bound to the connection: a client that goes away
        // has no later call to gather.
        None,
    )
    .context("connect Ghostlight service")?;
    Ok((
        connection.writer,
        connection.reader,
        connection.server,
        connection.catalog,
    ))
}

fn read_until_disconnected(
    mut reader: BufReader<TcpStream>,
    state: &Arc<(Mutex<SessionState>, Condvar)>,
    generation: u64,
    event_handler: &Arc<dyn Fn(ServiceEvent) + Send + Sync>,
) {
    loop {
        match read_json_line::<ServiceResponse>(&mut reader) {
            Ok(Some(ServiceResponse::CatalogChanged { tools, .. })) => {
                let changed = {
                    let mut locked = lock(&state.0);
                    let Some(session) = locked.connected.as_mut() else {
                        continue;
                    };
                    if session.generation != generation || session.catalog == tools {
                        false
                    } else {
                        session.catalog = tools.clone();
                        locked.previous_catalog = Some(tools);
                        true
                    }
                };
                if changed {
                    event_handler(ServiceEvent::CatalogChanged);
                }
            }
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
