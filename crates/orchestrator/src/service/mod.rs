//! Persistent service lifecycle and generic bridge session handling.

use std::collections::HashMap;
use std::env;
use std::io::{self, BufReader};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use ghostlight_bridge::browser::{BrowserCommand, BrowserEvent};
use ghostlight_bridge::framing::{read_json_line, read_native, write_json_line, write_native};
use ghostlight_bridge::lifecycle::ServiceLease;
use ghostlight_bridge::relay::{BrowserRelayRequest, BrowserRelayResponse, BROWSER_RELAY_MAJOR};
use ghostlight_bridge::runtime::{read_runtime, write_runtime, RuntimeEndpoint};
use ghostlight_bridge::service::{
    IntakeChannel, ServerProfile, ServiceRequest, ServiceResponse, SessionMarker, ToolDefinition,
    SERVICE_BRIDGE_MAJOR,
};
use uuid::Uuid;

use crate::browser::{BrowserEventSink, BrowserPort, RelayBrowserPort};
use crate::governance::{AuditRecord, AuditSink, Capability, GovernanceFacade, JsonlAuditSink};
use crate::language::{catalog_for, RequestRestrictions, SERVER_INSTRUCTIONS};
use crate::presentation::{BrowserPresentation, PresentationReactor};
use crate::work::{ActiveAuthorityRegistry, ApplicationExecutor, CancellationToken};
use crate::workbench::{
    ProjectingAuditSink, ReadinessSummary, WorkbenchFacade, WorkbenchProjection,
};
use crate::workspace::{ReleasedTabs, WorkspaceStore};

const DIAGNOSTIC_CLEAR_BATCH_SIZE: usize = 256;

/// A running local service host. Dropping it requests listener shutdown.
pub struct ServiceHost {
    /// Published authenticated endpoint.
    pub endpoint: RuntimeEndpoint,
    /// Typed in-process application boundary for the disposable desktop workbench.
    pub workbench: WorkbenchFacade,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    runtime_path: PathBuf,
    _lease: ServiceLease,
}

enum ServiceOpening {
    Workspace {
        client_label: String,
        channel: IntakeChannel,
        session: Option<SessionMarker>,
    },
    WorkbenchActivation,
    ReadinessInspection,
}

impl ServiceHost {
    /// Start both authenticated loopback listeners and publish runtime discovery.
    pub fn start(path: &Path) -> Result<Self> {
        let lease = ServiceLease::try_acquire(path)
            .context("open the orchestrator service lease")?
            .context("another Ghostlight orchestrator already owns this runtime")?;
        let service_listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .context("bind service bridge")?;
        let browser_listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .context("bind browser bridge")?;
        service_listener
            .set_nonblocking(true)
            .context("configure service listener")?;
        browser_listener
            .set_nonblocking(true)
            .context("configure browser listener")?;
        let token = format!("runtime_{}", Uuid::new_v4().simple());
        let endpoint = RuntimeEndpoint {
            service_port: service_listener.local_addr()?.port(),
            browser_port: browser_listener.local_addr()?.port(),
            token: token.clone(),
            service_bridge_major: SERVICE_BRIDGE_MAJOR,
            browser_relay_major: BROWSER_RELAY_MAJOR,
            service_version: env!("CARGO_PKG_VERSION").into(),
        };

        let workspaces = WorkspaceStore::default();
        let governance = GovernanceFacade::from_environment();
        let service_epoch = format!("service_{}", Uuid::new_v4().simple());
        let browser = Arc::new(RelayBrowserPort::new(service_epoch));
        let browser_port: Arc<dyn BrowserPort> = browser.clone();
        let _ = governance.runtime_decision();
        let _ = browser_port.publish_control_state(governance.runtime_state());
        let presentation = PresentationReactor::new(Arc::new(BrowserPresentation::new(
            browser_port.clone(),
            workspaces.clone(),
        )));
        let audit_path = env::var_os("GHOSTLIGHT_AUDIT_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| path.with_file_name("audit.jsonl"));
        let projection = WorkbenchProjection::default();
        projection
            .load_history(&audit_path)
            .context("restore content-minimized workbench history")?;
        let durable_audit =
            Arc::new(JsonlAuditSink::open(&audit_path).context("open content-minimized audit")?);
        let audit: Arc<dyn AuditSink> =
            Arc::new(ProjectingAuditSink::new(durable_audit, projection.clone()));
        let workbench = WorkbenchFacade::new(
            projection.clone(),
            workspaces.clone(),
            governance.clone(),
            browser.clone(),
        );
        let executor = Arc::new(ApplicationExecutor::new(
            governance.clone(),
            workspaces.clone(),
            browser_port.clone(),
            presentation.clone(),
            projection,
            audit.clone(),
        ));
        browser.set_event_sink(Arc::new(ServiceBrowserEvents {
            governance: governance.clone(),
            workspaces: workspaces.clone(),
            active: executor.active_authority(),
            audit,
            browser: browser_port.clone(),
        }));

        write_runtime(path, &endpoint).context("publish runtime endpoint")?;
        let stop = Arc::new(AtomicBool::new(false));
        let service_thread = spawn_service_listener(
            service_listener,
            Arc::clone(&stop),
            executor,
            workspaces,
            browser_port.clone(),
            workbench.clone(),
            governance.clone(),
            endpoint.token.clone(),
        );
        let browser_thread = spawn_browser_listener(
            browser_listener,
            Arc::clone(&stop),
            browser,
            endpoint.token.clone(),
        );
        Ok(Self {
            endpoint,
            workbench,
            stop,
            threads: vec![service_thread, browser_thread],
            runtime_path: path.into(),
            _lease: lease,
        })
    }

    fn join(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Drop for ServiceHost {
    fn drop(&mut self) {
        self.join();
        if ghostlight_bridge::runtime::read_runtime(&self.runtime_path)
            .is_ok_and(|current| current.token == self.endpoint.token)
        {
            let _ = std::fs::remove_file(&self.runtime_path);
        }
    }
}

/// Ask an already-running authenticated service to reveal its attached desktop workbench.
pub fn request_workbench_activation(path: &Path) -> Result<bool> {
    let (mut stream, endpoint) = connect_running_service(path)?;
    write_json_line(
        &mut stream,
        &ServiceRequest::ActivateWorkbench {
            major: SERVICE_BRIDGE_MAJOR,
            token: endpoint.token,
        },
    )
    .context("request workbench activation")?;
    let mut reader = BufReader::new(stream);
    match read_json_line::<ServiceResponse>(&mut reader).context("read workbench activation")? {
        Some(ServiceResponse::WorkbenchActivated { available }) => Ok(available),
        Some(ServiceResponse::Error { message, .. }) => bail!(message),
        _ => bail!("service returned an invalid workbench activation response"),
    }
}

/// Read readiness from an already-running authority without starting it or opening a session.
pub fn request_readiness(path: &Path) -> Result<ReadinessSummary> {
    let (mut stream, endpoint) = connect_running_service(path)?;
    write_json_line(
        &mut stream,
        &ServiceRequest::InspectReadiness {
            major: SERVICE_BRIDGE_MAJOR,
            token: endpoint.token,
        },
    )
    .context("request readiness inspection")?;
    let mut reader = BufReader::new(stream);
    match read_json_line::<ServiceResponse>(&mut reader).context("read readiness inspection")? {
        Some(ServiceResponse::Readiness { value }) => {
            serde_json::from_value(value).context("decode readiness inspection")
        }
        Some(ServiceResponse::Error { message, .. }) => bail!(message),
        _ => bail!("service returned an invalid readiness inspection response"),
    }
}

fn connect_running_service(path: &Path) -> Result<(TcpStream, RuntimeEndpoint)> {
    let endpoint = read_runtime(path).context("read current Ghostlight runtime")?;
    if endpoint.service_bridge_major != SERVICE_BRIDGE_MAJOR {
        bail!("running Ghostlight service bridge is incompatible");
    }
    let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, endpoint.service_port))
        .context("connect current Ghostlight service")?;
    stream.set_nodelay(true)?;
    Ok((stream, endpoint))
}

#[allow(clippy::too_many_arguments)]
fn spawn_service_listener(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    executor: Arc<ApplicationExecutor>,
    workspaces: WorkspaceStore,
    browser: Arc<dyn BrowserPort>,
    workbench: WorkbenchFacade,
    governance: GovernanceFacade,
    token: String,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("ghostlight-service-listener".into())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let executor = Arc::clone(&executor);
                        let workspaces = workspaces.clone();
                        let browser = Arc::clone(&browser);
                        let workbench = workbench.clone();
                        let governance = governance.clone();
                        let token = token.clone();
                        let _ = thread::Builder::new()
                            .name("ghostlight-mcp-session".into())
                            .spawn(move || {
                                if let Err(error) = serve_session(
                                    stream, executor, workspaces, browser, workbench, governance,
                                    &token,
                                ) {
                                    eprintln!("MCP service session ended: {error:#}");
                                }
                            });
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => thread::sleep(Duration::from_millis(50)),
                }
            }
        })
        .expect("service listener thread starts")
}

fn spawn_browser_listener(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    browser: Arc<RelayBrowserPort>,
    token: String,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("ghostlight-browser-listener".into())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let browser = Arc::clone(&browser);
                        let token = token.clone();
                        let _ = thread::Builder::new()
                            .name("ghostlight-browser-session".into())
                            .spawn(move || {
                                if let Err(error) = serve_browser_relay(stream, &browser, &token) {
                                    eprintln!("Browser bridge rejected connection: {error:#}");
                                }
                            });
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => thread::sleep(Duration::from_millis(50)),
                }
            }
        })
        .expect("browser listener thread starts")
}

fn serve_browser_relay(
    mut stream: TcpStream,
    browser: &Arc<RelayBrowserPort>,
    expected_token: &str,
) -> Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    let Some(request) = read_native::<BrowserRelayRequest>(&mut stream)? else {
        return Ok(());
    };
    let BrowserRelayRequest::Hello { major, token } = request;
    if token != expected_token {
        write_native(
            &mut stream,
            &BrowserRelayResponse::Rejected {
                code: "authentication_failed".into(),
                message: "Runtime authentication failed.".into(),
            },
        )?;
        return Ok(());
    }
    if major != BROWSER_RELAY_MAJOR {
        write_native(
            &mut stream,
            &BrowserRelayResponse::Rejected {
                code: "incompatible_relay".into(),
                message: format!(
                    "Browser relay major {major} is incompatible with required {BROWSER_RELAY_MAJOR}."
                ),
            },
        )?;
        return Ok(());
    }
    write_native(
        &mut stream,
        &BrowserRelayResponse::Accepted {
            major: BROWSER_RELAY_MAJOR,
        },
    )?;
    browser.attach(stream).map_err(anyhow::Error::msg)
}

#[allow(clippy::too_many_arguments)]
fn serve_session(
    stream: TcpStream,
    executor: Arc<ApplicationExecutor>,
    workspaces: WorkspaceStore,
    browser: Arc<dyn BrowserPort>,
    workbench: WorkbenchFacade,
    governance: GovernanceFacade,
    expected_token: &str,
) -> Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    // Capture the connection quadruple before the stream moves into the writer; the observed
    // peer is resolved after hello, where stage-2 attribution belongs.
    let observed_local = stream.local_addr().ok();
    let observed_peer = stream.peer_addr().ok();
    let reader_stream = stream.try_clone()?;
    let mut reader = BufReader::new(reader_stream);
    let writer = Arc::new(Mutex::new(stream));
    let Some(request) = read_json_line::<ServiceRequest>(&mut reader)? else {
        return Ok(());
    };
    let (major, token, opening) = match request {
        ServiceRequest::ActivateWorkbench { major, token } => {
            (major, token, ServiceOpening::WorkbenchActivation)
        }
        ServiceRequest::InspectReadiness { major, token } => {
            (major, token, ServiceOpening::ReadinessInspection)
        }
        ServiceRequest::Hello {
            major,
            token,
            client_label,
            channel,
            session,
        } => (
            major,
            token,
            ServiceOpening::Workspace {
                client_label,
                channel,
                session,
            },
        ),
        _ => {
            write_response(
                &writer,
                &ServiceResponse::Error {
                    id: None,
                    code: "hello_required".into(),
                    message: "Service hello must be the first message.".into(),
                },
            );
            return Ok(());
        }
    };
    if token != expected_token {
        write_response(
            &writer,
            &ServiceResponse::Error {
                id: None,
                code: "authentication_failed".into(),
                message: "Runtime authentication failed.".into(),
            },
        );
        return Ok(());
    }
    if major != SERVICE_BRIDGE_MAJOR {
        write_response(
            &writer,
            &ServiceResponse::Error {
                id: None,
                code: "incompatible_bridge".into(),
                message: format!(
                    "Bridge major {major} is incompatible with required {SERVICE_BRIDGE_MAJOR}."
                ),
            },
        );
        return Ok(());
    }
    let (client_label, channel, session) = match opening {
        ServiceOpening::WorkbenchActivation => {
            write_response(
                &writer,
                &ServiceResponse::WorkbenchActivated {
                    available: workbench.reveal().is_ok(),
                },
            );
            return Ok(());
        }
        ServiceOpening::ReadinessInspection => {
            let value = serde_json::to_value(workbench.snapshot().readiness)
                .context("serialize readiness inspection")?;
            write_response(&writer, &ServiceResponse::Readiness { value });
            return Ok(());
        }
        ServiceOpening::Workspace {
            client_label,
            channel,
            session,
        } => (client_label, channel, session),
    };
    // Admission, before any workspace exists: an authority layer may decline an intake entirely.
    let admission = governance.admits_channel(channel);
    if !admission.allowed {
        write_response(
            &writer,
            &ServiceResponse::Error {
                id: None,
                code: admission.reason.as_str().into(),
                message: format!(
                    "Configured authority does not admit the {} intake channel.",
                    channel.as_str()
                ),
            },
        );
        return Ok(());
    }
    // Before opening anything, release workspaces whose owner is gone and close the tabs they
    // still hold. Sweeping here rather than on a timer keeps the cost proportional to use.
    reap_finished_sessions(&workspaces, browser.as_ref());
    // Observed attribution (ADR-0105 stage 2): the kernel names the connection's owner; nothing
    // caller-asserted can forge it. Only the bounded lowercase image name is kept -- never the
    // path, and never as an authority input.
    let peer_image = match (observed_local, observed_peer) {
        (Some(local), Some(peer)) => {
            ghostlight_win_peer::identify_addresses(local, peer).map(|peer| peer.image_name)
        }
        _ => None,
    };
    let workspace = match session {
        Some(marker) => workspaces.resume_or_admit(client_label, channel, marker, peer_image),
        None => workspaces.admit(client_label, channel, peer_image),
    };
    write_response(
        &writer,
        &ServiceResponse::HelloAccepted {
            major: SERVICE_BRIDGE_MAJOR,
            session: workspace.as_str().into(),
            server: ServerProfile {
                name: "ghostlight".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                instructions: SERVER_INSTRUCTIONS.into(),
            },
        },
    );
    let active: Arc<Mutex<HashMap<String, CancellationToken>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let published_catalog: Arc<Mutex<Option<Vec<ToolDefinition>>>> = Arc::new(Mutex::new(None));
    let catalog_watch_stop = Arc::new(AtomicBool::new(false));
    let catalog_watch = (channel == IntakeChannel::Mcp).then(|| {
        let governance = governance.clone();
        let writer = Arc::clone(&writer);
        let published = Arc::clone(&published_catalog);
        let stop = Arc::clone(&catalog_watch_stop);
        thread::Builder::new()
            .name("ghostlight-policy-catalog".into())
            .spawn(move || {
                let mut generation = 1_u64;
                while !stop.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(250));
                    let Some(previous) = lock(&published).clone() else {
                        continue;
                    };
                    let snapshot = governance.snapshot(&RequestRestrictions::default());
                    let current = catalog_for(&snapshot);
                    if current == previous {
                        continue;
                    }
                    let changed = {
                        let mut published = lock(&published);
                        if published.as_ref() == Some(&current) {
                            false
                        } else {
                            *published = Some(current.clone());
                            true
                        }
                    };
                    if changed {
                        generation = generation.wrapping_add(1);
                        write_response(
                            &writer,
                            &ServiceResponse::CatalogChanged {
                                generation,
                                tools: current,
                            },
                        );
                    }
                }
            })
            .expect("policy catalog watcher starts")
    });

    // Every way this connection can end has to reach the teardown below. A reset socket, an
    // oversized frame, and one malformed line are all ordinary ways for a client to go away, and
    // each of them used to leave through `?` before the release ran. The workspace and every tab
    // it held then survived with nothing able to collect them: an unowned workspace has no owning
    // process to look up, so the reaper cannot see it either.
    let served = (|| -> Result<()> {
        while let Some(request) = read_json_line::<ServiceRequest>(&mut reader)? {
            match request {
                ServiceRequest::Catalog => {
                    let snapshot = governance.snapshot(&RequestRestrictions::default());
                    let tools = catalog_for(&snapshot);
                    write_response(
                        &writer,
                        &ServiceResponse::Catalog {
                            tools: tools.clone(),
                        },
                    );
                    *lock(&published_catalog) = Some(tools);
                }
                ServiceRequest::Invoke {
                    id,
                    tool,
                    input,
                    deadline_ms,
                } => {
                    let cancellation = CancellationToken::default();
                    if lock(&active)
                        .insert(id.clone(), cancellation.clone())
                        .is_some()
                    {
                        write_response(
                            &writer,
                            &ServiceResponse::Error {
                                id: Some(id),
                                code: "duplicate_request".into(),
                                message: "Request id is already active.".into(),
                            },
                        );
                        continue;
                    }
                    let executor = Arc::clone(&executor);
                    let writer = Arc::clone(&writer);
                    let active = Arc::clone(&active);
                    let workspace = workspace.clone();
                    let _ = thread::Builder::new().name("ghostlight-invocation".into()).spawn(move || {
                        let mut result = executor.execute(&workspace, &tool, input, deadline_ms, &cancellation);
                        lock(&active).remove(&id);
                        let text = result.model_text();
                        let is_error = result.is_error();
                        let content = std::mem::take(&mut result.content);
                        let value = serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({"status":"unknown","effect":"unknown","repeat_safe":false,"summary":"Result serialization failed.","facts":{},"next_steps":[]}));
                        write_response(&writer, &ServiceResponse::Result { id, text, result: value, is_error, content });
                    });
                }
                ServiceRequest::Cancel { id } => {
                    if let Some(token) = lock(&active).get(&id) {
                        token.cancel();
                    }
                }
                ServiceRequest::Hello { .. }
                | ServiceRequest::ActivateWorkbench { .. }
                | ServiceRequest::InspectReadiness { .. } => write_response(
                    &writer,
                    &ServiceResponse::Error {
                        id: None,
                        code: "duplicate_hello".into(),
                        message: "Session is already established.".into(),
                    },
                ),
            }
        }
        Ok(())
    })();

    catalog_watch_stop.store(true, Ordering::SeqCst);
    if let Some(watch) = catalog_watch {
        let _ = watch.join();
    }
    for cancellation in lock(&active).values() {
        cancellation.cancel();
    }
    // A workspace with an owner outlives this connection: the caller is still there and its next
    // call must reach the same tabs. It is released when its owner is gone, not when a socket is.
    if !workspaces.is_owned(&workspace) {
        let released = workspaces.release(&workspace);
        cleanup_released_tabs(workspace.as_str(), &released, browser.as_ref());
    }
    served
}

/// Close the tabs of every session whose owning process is gone.
fn reap_finished_sessions(workspaces: &WorkspaceStore, browser: &dyn BrowserPort) {
    for released in workspaces.reap(&owner_alive) {
        cleanup_released_tabs("reaped", &released, browser);
    }
}

/// Erase service-owned browser diagnostics before released tabs are offered to the close interlock.
///
/// Cleanup goes to the browser that holds the tabs and nowhere else. A workspace that never opened
/// a browser has nothing to clean up.
fn cleanup_released_tabs(workspace: &str, released: &ReleasedTabs, browser: &dyn BrowserPort) {
    let Some(target) = released.browser.as_deref() else {
        return;
    };
    let cancelled = AtomicBool::new(false);
    for tab_ids in released.physical_ids.chunks(DIAGNOSTIC_CLEAR_BATCH_SIZE) {
        let _ = browser.call(
            target,
            workspace,
            BrowserCommand::ClearDiagnostics {
                tab_ids: tab_ids.to_vec(),
            },
            Instant::now() + Duration::from_secs(2),
            &cancelled,
        );
    }
    for &tab_id in &released.physical_ids {
        let _ = browser.call(
            target,
            workspace,
            BrowserCommand::CloseTab {
                tab_id,
                released: true,
            },
            Instant::now() + Duration::from_secs(2),
            &cancelled,
        );
    }
}

/// Whether the process that owns a session is still running.
///
/// A declared key names no process, so it lives until the service does. That bound is the service's
/// own lifetime rather than an invented idle policy, and the workspace registry is in memory.
fn owner_alive(marker: &SessionMarker) -> bool {
    let SessionMarker::Process {
        pid, started_at, ..
    } = marker
    else {
        return true;
    };
    let Ok(pid) = usize::try_from(*pid).map(sysinfo::Pid::from) else {
        return false;
    };
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    // The start time is the half that matters: a recycled pid is a different owner.
    system
        .process(pid)
        .is_some_and(|process| process.start_time() == *started_at)
}

fn write_response(writer: &Mutex<TcpStream>, response: &ServiceResponse) {
    let _ = write_json_line(&mut *lock(writer), response);
}
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct ServiceBrowserEvents {
    governance: GovernanceFacade,
    workspaces: WorkspaceStore,
    active: ActiveAuthorityRegistry,
    audit: Arc<dyn AuditSink>,
    browser: Arc<dyn BrowserPort>,
}

impl BrowserEventSink for ServiceBrowserEvents {
    fn on_event(&self, browser: &str, event: BrowserEvent) {
        match event {
            BrowserEvent::DocumentCommitted {
                tab_id,
                url,
                correlation,
            } => {
                let Some(workspace) = self.workspaces.owner_of_physical(browser, tab_id) else {
                    return;
                };
                // The most recently started invocation still governing this workspace, when any
                // is; several may be active at once (recording status/stop/discard skip the
                // workspace lease), and each keeps its own entry until it finishes, so no
                // invocation's completion can clear a different one's still-active snapshot.
                let snapshot = lock(&self.active)
                    .get(workspace.as_str())
                    .and_then(|entries| entries.last())
                    .map(|(_, snapshot)| snapshot.clone())
                    .unwrap_or_else(|| self.governance.snapshot(&RequestRestrictions::default()));
                let runtime = self.governance.runtime_decision();
                let decision = if runtime.allowed {
                    snapshot.authorize_landing(Capability::Action, &url)
                } else {
                    runtime
                };
                let event_id = format!("browser_event_{}", Uuid::new_v4().simple());
                if correlation.is_none() || !decision.allowed {
                    let _ = self.workspaces.apply_browser_landing(
                        browser,
                        tab_id,
                        &url,
                        decision.allowed,
                    );
                }
                let record = AuditRecord::now(
                    &event_id,
                    workspace.as_str(),
                    "browser_landing",
                    Capability::Action,
                    snapshot.id(),
                    decision,
                    if decision.allowed {
                        "succeeded"
                    } else {
                        "blocked"
                    },
                    "applied",
                    if decision.allowed {
                        "The browser landed on a new page and its landing was governed."
                    } else {
                        "Authority blocked the page the browser landed on."
                    },
                    0,
                )
                .with_policy(&snapshot, decision);
                let _ = self.audit.record(&record);
            }
            BrowserEvent::ReadinessChanged { tab_id, readiness } => self
                .workspaces
                .apply_browser_readiness(browser, tab_id, readiness),
            BrowserEvent::ChildTabOpened { tab, opener_tab_id } => {
                let _ = self
                    .workspaces
                    .apply_browser_child(browser, opener_tab_id, &tab);
            }
            BrowserEvent::RuntimeControlRequested { intent } => {
                let state = self.governance.apply_runtime_intent(intent);
                let _ = self.browser.publish_control_state(state);
            }
            BrowserEvent::TabClosed { tab_id } => {
                self.workspaces.apply_browser_close(browser, tab_id)
            }
            // Attention never reaches this sink: it is browser routing state, and the port that
            // knows which connection reported it is the only thing that needs it.
            BrowserEvent::Attended | BrowserEvent::Disconnected => {}
            BrowserEvent::DialogChanged { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;
    use std::net::TcpStream;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use ghostlight_bridge::browser::BrowserCommand;
    use ghostlight_bridge::framing::{read_json_line, write_json_line};
    use ghostlight_bridge::service::{
        IntakeChannel, ServiceRequest, ServiceResponse, SERVICE_BRIDGE_MAJOR,
    };

    use crate::browser::testing::{FakeBrowser, FAKE_BROWSER};
    use crate::workbench::{
        WorkbenchNotification, WorkbenchPresentationError, WorkbenchPresentationPort,
    };
    use crate::workspace::ReleasedTabs;

    use super::{
        cleanup_released_tabs, request_readiness, request_workbench_activation, ServiceHost,
    };

    fn runtime_path(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let directory = std::env::temp_dir().join(format!(
            "ghostlight-service-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("ghostlight-runtime.json");
        (directory, path)
    }

    #[test]
    fn incompatible_service_bridge_fails_before_catalog() {
        let (directory, path) = runtime_path("incompatible");
        let host = ServiceHost::start(&path).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", host.endpoint.service_port)).unwrap();
        write_json_line(
            &mut stream,
            &ServiceRequest::Hello {
                major: SERVICE_BRIDGE_MAJOR + 1,
                token: host.endpoint.token.clone(),
                client_label: "test".into(),
                channel: IntakeChannel::Mcp,
                session: None,
            },
        )
        .unwrap();
        let mut reader = BufReader::new(stream);
        let response: ServiceResponse = read_json_line(&mut reader).unwrap().unwrap();
        assert!(
            matches!(response, ServiceResponse::Error { code, .. } if code == "incompatible_bridge")
        );
        drop(host);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn readiness_inspection_is_read_only_and_opens_no_session() {
        let (directory, path) = runtime_path("readiness-inspection");
        let host = ServiceHost::start(&path).unwrap();
        let before = host.workbench.snapshot();

        let observed = request_readiness(&path).unwrap();
        let after = host.workbench.snapshot();

        assert_eq!(observed, before.readiness);
        assert_eq!(after.readiness, before.readiness);
        assert!(after.sessions.is_empty());
        assert_eq!(after.history, before.history);

        drop(host);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn readiness_inspection_never_starts_an_absent_service() {
        let (directory, path) = runtime_path("readiness-no-start");

        assert!(request_readiness(&path).is_err());
        assert!(!path.exists());

        std::fs::remove_dir_all(directory).unwrap();
    }

    /// Open one MCP session, end the connection the given way, and report the sessions left.
    fn sessions_after(name: &str, goodbye: impl FnOnce(&mut TcpStream)) -> usize {
        let (directory, path) = runtime_path(name);
        let host = ServiceHost::start(&path).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", host.endpoint.service_port)).unwrap();
        write_json_line(
            &mut stream,
            &ServiceRequest::Hello {
                major: SERVICE_BRIDGE_MAJOR,
                token: host.endpoint.token.clone(),
                client_label: "leak-probe".into(),
                channel: IntakeChannel::Mcp,
                session: None,
            },
        )
        .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let accepted: ServiceResponse = read_json_line(&mut reader).unwrap().unwrap();
        assert!(matches!(accepted, ServiceResponse::HelloAccepted { .. }));
        assert_eq!(
            host.workbench.snapshot().sessions.len(),
            1,
            "the probe session must exist before the connection ends"
        );

        goodbye(&mut stream);
        drop(reader);
        drop(stream);

        // The session teardown runs on the connection's own thread, so give it a moment.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !host.workbench.snapshot().sessions.is_empty() {
            thread::sleep(Duration::from_millis(25));
        }
        let remaining = host.workbench.snapshot().sessions.len();
        drop(host);
        let _ = std::fs::remove_dir_all(directory);
        remaining
    }

    #[test]
    fn a_client_that_goes_away_badly_still_releases_its_session() {
        // The negative control: a tidy goodbye has always worked, so if this were the only case
        // the assertion below would prove nothing about the path that actually leaked.
        assert_eq!(sessions_after("goodbye-clean", |_| {}), 0);

        // One malformed line ends the connection through the error path. Before the teardown was
        // made unconditional this leaked the workspace and every tab it held, and nothing could
        // collect it afterwards: an unowned workspace has no owning process for the reaper.
        assert_eq!(
            sessions_after("goodbye-malformed", |stream| {
                use std::io::Write;
                let _ = stream.write_all(b"{ not json at all\n");
                let _ = stream.flush();
            }),
            0,
            "a session must not outlive the connection that opened it"
        );
    }

    #[test]
    fn service_lease_prevents_concurrent_authorities() {
        let (directory, path) = runtime_path("singleton");
        let first = ServiceHost::start(&path).unwrap();
        let second = ServiceHost::start(&path)
            .err()
            .expect("second authority is rejected");
        let detail = format!("{second:#}");
        assert!(
            detail.contains("another Ghostlight orchestrator already owns this runtime"),
            "{detail}"
        );
        drop(first);
        let replacement = ServiceHost::start(&path).unwrap();
        drop(replacement);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[derive(Default)]
    struct RevealCounter(AtomicUsize);

    impl WorkbenchPresentationPort for RevealCounter {
        fn reveal(&self) -> Result<(), WorkbenchPresentationError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn notify(
            &self,
            _notification: WorkbenchNotification,
        ) -> Result<(), WorkbenchPresentationError> {
            Ok(())
        }
    }

    #[test]
    fn authenticated_activation_reveals_the_existing_workbench() {
        let (directory, path) = runtime_path("activation");
        let host = ServiceHost::start(&path).unwrap();
        assert!(!request_workbench_activation(&path).unwrap());
        let reveals = Arc::new(RevealCounter::default());
        host.workbench.attach_presentation(reveals.clone());
        assert!(request_workbench_activation(&path).unwrap());
        assert_eq!(reveals.0.load(Ordering::SeqCst), 1);
        drop(host);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn released_tabs_clear_diagnostics_in_bounded_batches_before_close_attempts() {
        let browser = FakeBrowser::default();
        let released = ReleasedTabs {
            browser: Some(FAKE_BROWSER.into()),
            physical_ids: (1..=257).collect(),
        };

        cleanup_released_tabs("workspace-1", &released, &browser);

        let calls = browser.calls();
        assert_eq!(
            calls[0],
            BrowserCommand::ClearDiagnostics {
                tab_ids: (1..=256).collect()
            }
        );
        assert_eq!(
            calls[1],
            BrowserCommand::ClearDiagnostics { tab_ids: vec![257] }
        );
        assert_eq!(
            calls[2],
            BrowserCommand::CloseTab {
                tab_id: 1,
                released: true
            }
        );
        assert_eq!(
            calls.last(),
            Some(&BrowserCommand::CloseTab {
                tab_id: 257,
                released: true
            })
        );
    }
}
