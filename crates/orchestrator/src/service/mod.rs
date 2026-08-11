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

use anyhow::{Context, Result};
use ghostlight_bridge::browser::{BrowserCommand, BrowserEvent};
use ghostlight_bridge::framing::{read_json_line, read_native, write_json_line, write_native};
use ghostlight_bridge::relay::{BrowserRelayRequest, BrowserRelayResponse, BROWSER_RELAY_MAJOR};
use ghostlight_bridge::runtime::{runtime_file, write_runtime, RuntimeEndpoint};
use ghostlight_bridge::service::{
    ServerProfile, ServiceRequest, ServiceResponse, SERVICE_BRIDGE_MAJOR,
};
use uuid::Uuid;

use crate::browser::{BrowserEventSink, BrowserPort, RelayBrowserPort};
use crate::governance::{AuditRecord, AuditSink, Capability, GovernanceFacade, JsonlAuditSink};
use crate::language::{catalog, RequestRestrictions, SERVER_INSTRUCTIONS};
use crate::presentation::{BrowserPresentation, PresentationReactor};
use crate::work::{ActiveAuthorityRegistry, ApplicationExecutor, CancellationToken};
use crate::workbench::{ProjectingAuditSink, WorkbenchFacade, WorkbenchProjection};
use crate::workspace::WorkspaceStore;

/// A running local service host. Dropping it requests listener shutdown.
pub struct ServiceHost {
    /// Published authenticated endpoint.
    pub endpoint: RuntimeEndpoint,
    /// Typed in-process application boundary for the disposable desktop workbench.
    pub workbench: WorkbenchFacade,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    runtime_path: PathBuf,
}

impl ServiceHost {
    /// Start both authenticated loopback listeners and publish runtime discovery.
    pub fn start(path: &Path) -> Result<Self> {
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
        let presentation =
            PresentationReactor::new(Arc::new(BrowserPresentation::new(browser_port.clone())));
        let audit_path = env::var_os("GHOSTLIGHT_AUDIT_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| path.with_file_name("audit.jsonl"));
        let projection = WorkbenchProjection::default();
        projection
            .load_history(&audit_path)
            .context("restore payload-free workbench history")?;
        let durable_audit =
            Arc::new(JsonlAuditSink::open(&audit_path).context("open payload-free audit")?);
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
            governance,
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
            browser_port,
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
        })
    }

    /// Run until the host is dropped or an external caller requests shutdown.
    pub fn wait(mut self) {
        while !self.stop.load(Ordering::SeqCst) {
            thread::park_timeout(Duration::from_secs(1));
        }
        self.join();
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

/// Run the persistent service using default runtime discovery.
pub fn run_forever() -> Result<()> {
    let host = ServiceHost::start(&runtime_file())?;
    eprintln!(
        "Ghostlight 1.0 service ready on local ports {} and {}",
        host.endpoint.service_port, host.endpoint.browser_port
    );
    host.wait();
    Ok(())
}

fn spawn_service_listener(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    executor: Arc<ApplicationExecutor>,
    workspaces: WorkspaceStore,
    browser: Arc<dyn BrowserPort>,
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
                        let token = token.clone();
                        let _ = thread::Builder::new()
                            .name("ghostlight-mcp-session".into())
                            .spawn(move || {
                                if let Err(error) =
                                    serve_session(stream, executor, workspaces, browser, &token)
                                {
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

fn serve_session(
    stream: TcpStream,
    executor: Arc<ApplicationExecutor>,
    workspaces: WorkspaceStore,
    browser: Arc<dyn BrowserPort>,
    expected_token: &str,
) -> Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    let reader_stream = stream.try_clone()?;
    let mut reader = BufReader::new(reader_stream);
    let writer = Arc::new(Mutex::new(stream));
    let Some(request) = read_json_line::<ServiceRequest>(&mut reader)? else {
        return Ok(());
    };
    let (major, token, client_label) = match request {
        ServiceRequest::Hello {
            major,
            token,
            client_label,
        } => (major, token, client_label),
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
    let workspace = workspaces.admit(client_label);
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

    while let Some(request) = read_json_line::<ServiceRequest>(&mut reader)? {
        match request {
            ServiceRequest::Catalog => {
                write_response(&writer, &ServiceResponse::Catalog { tools: catalog() })
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
                    let content = std::mem::take(&mut result.content);
                    let value = serde_json::to_value(result).unwrap_or_else(|_| serde_json::json!({"status":"unknown","effect":"unknown","repeat_safe":false,"summary":"Result serialization failed.","facts":{},"next_steps":[]}));
                    write_response(&writer, &ServiceResponse::Result { id, result: value, content });
                });
            }
            ServiceRequest::Cancel { id } => {
                if let Some(token) = lock(&active).get(&id) {
                    token.cancel();
                }
            }
            ServiceRequest::Hello { .. } => write_response(
                &writer,
                &ServiceResponse::Error {
                    id: None,
                    code: "duplicate_hello".into(),
                    message: "Session is already established.".into(),
                },
            ),
        }
    }

    for cancellation in lock(&active).values() {
        cancellation.cancel();
    }
    let physical_tabs = workspaces.release(&workspace);
    let cancelled = AtomicBool::new(false);
    for tab_id in physical_tabs {
        let _ = browser.call(
            workspace.as_str(),
            BrowserCommand::CloseTab { tab_id },
            Instant::now() + Duration::from_secs(2),
            &cancelled,
        );
    }
    Ok(())
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
    fn on_event(&self, event: BrowserEvent) {
        match event {
            BrowserEvent::DocumentCommitted {
                tab_id,
                url,
                correlation,
            } => {
                let Some(workspace) = self.workspaces.owner_of_physical(tab_id) else {
                    return;
                };
                let snapshot = lock(&self.active)
                    .get(workspace.as_str())
                    .cloned()
                    .unwrap_or_else(|| self.governance.snapshot(&RequestRestrictions::default()));
                let runtime = self.governance.runtime_decision();
                let decision = if runtime.allowed {
                    snapshot.authorize_landing(Capability::Action, &url)
                } else {
                    runtime
                };
                let event_id = format!("browser_event_{}", Uuid::new_v4().simple());
                if correlation.is_none() || !decision.allowed {
                    let _ = self
                        .workspaces
                        .apply_browser_landing(tab_id, &url, decision.allowed);
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
                );
                let _ = self.audit.record(&record);
            }
            BrowserEvent::ReadinessChanged { tab_id, readiness } => {
                self.workspaces.apply_browser_readiness(tab_id, readiness)
            }
            BrowserEvent::ChildTabOpened { tab, opener_tab_id } => {
                let _ = self.workspaces.apply_browser_child(opener_tab_id, &tab);
            }
            BrowserEvent::RuntimeControlRequested { intent } => {
                let state = self.governance.apply_runtime_intent(intent);
                let _ = self.browser.publish_control_state(state);
            }
            BrowserEvent::TabClosed { tab_id } => self.workspaces.apply_browser_close(tab_id),
            BrowserEvent::DialogChanged { .. } | BrowserEvent::Disconnected => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;
    use std::net::TcpStream;

    use ghostlight_bridge::framing::{read_json_line, write_json_line};
    use ghostlight_bridge::service::{ServiceRequest, ServiceResponse, SERVICE_BRIDGE_MAJOR};

    use super::ServiceHost;

    #[test]
    fn incompatible_service_bridge_fails_before_catalog() {
        let path =
            std::env::temp_dir().join(format!("ghostlight-runtime-{}.json", uuid::Uuid::new_v4()));
        std::env::set_var("GHOSTLIGHT_RUNTIME_FILE", &path);
        let host = ServiceHost::start(&path).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", host.endpoint.service_port)).unwrap();
        write_json_line(
            &mut stream,
            &ServiceRequest::Hello {
                major: SERVICE_BRIDGE_MAJOR + 1,
                token: host.endpoint.token.clone(),
                client_label: "test".into(),
            },
        )
        .unwrap();
        let mut reader = BufReader::new(stream);
        let response: ServiceResponse = read_json_line(&mut reader).unwrap().unwrap();
        assert!(
            matches!(response, ServiceResponse::Error { code, .. } if code == "incompatible_bridge")
        );
        drop(host);
        std::env::remove_var("GHOSTLIGHT_RUNTIME_FILE");
    }
}
