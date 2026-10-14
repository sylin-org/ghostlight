//! The persistent service: its lifetime, its one relay listener and the sessions it serves.
//!
//! [`ServiceHost`] holds the lease, prepares the executor and the browser port, and publishes the
//! relay protocol 3 runtime file once the desktop is ready (ADR-0166, ADR-0197 Decision 3). Every
//! connection then opens with one relay opening: an `mcp` stream is one MCP client session served
//! by [`crate::intake::mcp::McpSession`], a `browser` stream is one adapter connection, and a
//! `control` stream answers one operation.

mod admission;
pub mod lifetime;
#[cfg(test)]
pub(crate) mod test_home;

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::intake::session::{
    IntakeChannel, ServiceRequest, ServiceResponse, SessionMarker, ToolDefinition,
};
use anyhow::{Context, Result};
use ghostlight_relay::home::{LEASE_FILE_NAME, RUNTIME_FILE_NAME, TEMPLATES_FILE_NAME};
use ghostlight_relay::opening::{ControlOp, McpFrame, Stream};
use ghostlight_relay::runtime::RuntimeFile;
use ghostlight_relay::server::Lease;
use ghostlight_relay::templates::FailureTemplates;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::audit::AuditRecorder;
use crate::browser::adapter::chromium::protocol4::assets::page_assets;
use crate::browser::adapter::{BrowserCommand, BrowserEvent, BrowserOutcome};
use crate::browser::transport::{SocketReader, SocketWriter};
use crate::browser::{
    AdapterLifecycleObserver, Availability, BrowserError, BrowserEventSink, BrowserPort,
    RelayBrowserPort,
};
use crate::diagnostics::DiagnosticsHub;
use crate::governance::{GovernanceFacade, JsonlAuditSink};
use crate::install;
use crate::intake::listener::{self, Judgement};
use crate::intake::mcp::{failure_templates, McpSession, OwnedTab, ResumeState, Step};
use crate::language::catalog_for;
use crate::language::intake::{OpeningRefusal, SHUTTING_DOWN_CODE, SHUTTING_DOWN_MESSAGE};
use crate::presentation::{BrowserPresentation, PresentationReactor};
use crate::provenance::ConnectionEvidence;
use crate::work::{ApplicationExecutor, CancellationToken, PreparedInvocation};
use crate::workbench::{WorkbenchFacade, WorkbenchProjection};
use crate::workspace::{ReleasedTabs, WorkspaceId, WorkspaceStore};

const DIAGNOSTIC_CLEAR_BATCH_SIZE: usize = 256;
const QUEUED_METADATA_BYTES: usize = 512;

/// Prefix of the per-service secret every opening presents.
const TOKEN_PREFIX: &str = "runtime_";

/// Environment variable naming an audit log in place of the one in the state directory.
const AUDIT_FILE_VARIABLE: &str = "GHOSTLIGHT_AUDIT_FILE";

/// File name of the audit log in the state directory.
const AUDIT_FILE_NAME: &str = "audit.jsonl";

/// Suffix of the sibling file the failure templates are written through.
const TEMPORARY_SUFFIX: &str = ".tmp";

/// Longest a resumed session waits for one browser to return, and then for it to list its tabs.
const RESTORE_DEADLINE: Duration = Duration::from_secs(5);

use admission::{Capacity, Job, QueueBudget, Refused, SessionQueue};
use lifetime::{DiscoveryWatch, Drain, QUIT_GRACE};

/// The desktop's request to leave its event loop, installed once the desktop exists.
pub type QuitSignal = Arc<dyn Fn() + Send + Sync>;

/// How often a start that waits for the service lease asks for it again.
const LEASE_POLL: Duration = Duration::from_millis(50);

/// A running local service host. Dropping it stops the listener and withdraws discovery.
pub struct ServiceHost {
    /// Typed in-process application boundary for the disposable desktop workbench.
    pub workbench: WorkbenchFacade,
    /// The process-diagnostics hub this authority owns.
    pub diagnostics: Arc<DiagnosticsHub>,
    browser: Arc<RelayBrowserPort>,
    audit: Arc<AuditRecorder>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    home: PathBuf,
    port: u16,
    token: String,
    quit: Quit,
    discovery: Arc<DiscoveryWatch>,
    _lease: Lease,
}

/// Another authority owns this runtime; a competing explicit Open may await its presentation.
#[derive(Debug, thiserror::Error)]
#[error("another Ghostlight orchestrator already owns this runtime")]
pub struct AuthorityAlreadyRunning;

impl ServiceHost {
    /// Prepare the service in the Ghostlight home `home` under its lifetime lease, without
    /// publishing discovery. The desktop must call `publish_ready` only after its native
    /// interaction route is ready.
    pub fn start(home: &Path) -> Result<Self> {
        Self::start_waiting(home, Duration::ZERO)
    }

    /// Like [`ServiceHost::start`], but a held lease is asked for again until `lease_wait` has
    /// passed. A `serve` start uses this so a service that is about to exit does not make its
    /// successor give up.
    pub fn start_waiting(home: &Path, lease_wait: Duration) -> Result<Self> {
        let runtime_path = home.join(RUNTIME_FILE_NAME);
        let diagnostics = DiagnosticsHub::birth(&runtime_path);
        Self::start_with_diagnostics(home, Arc::clone(&diagnostics), lease_wait).inspect_err(
            |error| {
                // Meeting the running service is how a racing start ends, not a failure.
                if !error.is::<AuthorityAlreadyRunning>() {
                    diagnostics.sink().emit(
                        ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
                        ghostlight_bridge::diagnostics::Level::Error,
                        None,
                        &format!("service startup: {error:#}"),
                    );
                }
            },
        )
    }

    fn start_with_diagnostics(
        home: &Path,
        diagnostics: Arc<DiagnosticsHub>,
        lease_wait: Duration,
    ) -> Result<Self> {
        let lease = acquire_lease(&home.join(LEASE_FILE_NAME), lease_wait)?;
        if let Ok(executable) = env::current_exe() {
            remove_old_images_beside(&executable);
            register_missing_browsers(home, &executable);
        }
        let relay_listener = listener::bind().context("bind the relay listener")?;
        let port = relay_listener.local_addr()?.port();
        let token = format!("{TOKEN_PREFIX}{}", Uuid::new_v4().simple());

        let workspaces = WorkspaceStore::default();
        let governance = GovernanceFacade::from_environment();
        let service_epoch = format!("service_{}", Uuid::new_v4().simple());
        let browser = Arc::new(RelayBrowserPort::new(service_epoch, page_assets()));
        let browser_port: Arc<dyn BrowserPort> = browser.clone();
        browser
            .set_lifecycle_observer(Arc::clone(&diagnostics) as Arc<dyn AdapterLifecycleObserver>);
        let _ = governance.runtime_decision();
        let _ = browser_port.publish_control_state(governance.runtime_state());
        let presentation = PresentationReactor::new(Arc::new(BrowserPresentation::new(
            browser_port.clone(),
            workspaces.clone(),
        )));
        let audit_path = env::var_os(AUDIT_FILE_VARIABLE)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                install::layout::Layout::new(home)
                    .state()
                    .join(AUDIT_FILE_NAME)
            });
        let projection = WorkbenchProjection::default();
        let _ = projection.load_history(&audit_path);
        let durable_audit = Arc::new(JsonlAuditSink::new(&audit_path));
        let audit = Arc::new(AuditRecorder::new(durable_audit, projection.clone()));
        let workbench = WorkbenchFacade::new(
            projection.clone(),
            workspaces.clone(),
            governance.clone(),
            browser.clone(),
            Arc::clone(&diagnostics),
        );
        let executor = Arc::new(ApplicationExecutor::new(
            governance.clone(),
            workspaces.clone(),
            browser_port.clone(),
            presentation.clone(),
            projection,
            audit.clone(),
            Arc::clone(&diagnostics),
        ));
        browser.set_event_sink(Arc::new(ServiceBrowserEvents {
            governance: governance.clone(),
            workspaces: workspaces.clone(),
            browser: browser_port.clone(),
        }));

        let stop = Arc::new(AtomicBool::new(false));
        let quit = Quit {
            drain: Arc::new(Drain::default()),
            signal: Arc::default(),
            diagnostics: Arc::clone(&diagnostics),
        };
        let audit_stop = stop.clone();
        let audit_for_thread = Arc::clone(&audit);
        let audit_thread = std::thread::spawn(move || {
            while !audit_stop.load(Ordering::SeqCst) {
                audit_for_thread.wait_for_recovery(&audit_stop);
            }
        });
        let intake = Arc::new(Intake {
            token: token.clone(),
            executor,
            workspaces,
            browser_port,
            browser: Arc::clone(&browser),
            workbench: workbench.clone(),
            governance,
            diagnostics: Arc::clone(&diagnostics),
            quit: quit.clone(),
            sessions: Capacity::new(admission::SESSIONS),
            budget: QueueBudget::default(),
            stop: Arc::clone(&stop),
        });
        let listener_thread = spawn_listener(relay_listener, intake);
        Ok(Self {
            workbench,
            diagnostics,
            browser,
            audit,
            stop,
            threads: vec![listener_thread, audit_thread],
            home: home.into(),
            port,
            token,
            quit,
            discovery: Arc::default(),
            _lease: lease,
        })
    }

    /// Publish discovery after the desktop event loop has established an interaction route.
    ///
    /// The failure templates go first, so a connector that finds this service can already answer
    /// for it when it goes away; then the runtime file names the listener and its token.
    pub fn publish_ready(&self) -> Result<()> {
        // Relays that served an earlier service find this one from here on, so a call with no
        // browser waits for them from this moment, not from construction.
        self.browser.endpoint_published();
        write_templates(&self.home.join(TEMPLATES_FILE_NAME))
            .context("publish the failure templates")?;
        RuntimeFile::new(
            self.port,
            self.token.clone(),
            env!("CARGO_PKG_VERSION").into(),
        )
        .write(&self.home.join(RUNTIME_FILE_NAME))
        .context("publish desktop-ready discovery")?;
        self.discovery.settle();
        self.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::RUNTIME_PUBLISHED,
            ghostlight_bridge::diagnostics::Level::Info,
            None,
            &format!(
                "desktop_ready=true port={} relay={}",
                self.port,
                ghostlight_relay::RELAY
            ),
        );
        Ok(())
    }

    /// Stop this process by itself if discovery has not been published by `deadline`.
    ///
    /// `publish_ready` cancels the deadline. On expiry the service records a diagnostics event and
    /// calls `on_expiry`, which ends the process. A start that stalls therefore never holds the
    /// lease against the starts that follow it.
    pub fn expect_discovery_by(
        &self,
        deadline: Instant,
        on_expiry: impl FnOnce() + Send + 'static,
    ) -> Result<()> {
        let diagnostics = Arc::clone(&self.diagnostics);
        self.discovery
            .arm(deadline, move || {
                diagnostics.sink().emit(
                    ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
                    ghostlight_bridge::diagnostics::Level::Error,
                    None,
                    "discovery was not published in time; the service stopped",
                );
                on_expiry();
            })
            .context("start the discovery deadline")
    }

    /// Install the request the desktop makes to leave its event loop with status 0.
    pub fn install_quit_signal(&self, signal: QuitSignal) {
        *lock(&self.quit.signal) = Some(signal);
    }

    /// Begin a quit: refuse new work, let in-flight invocations finish for up to `grace`, then ask
    /// the desktop to exit. Returns at once; a second call while a quit is under way does nothing.
    pub fn begin_quit(&self, grace: Duration) {
        self.quit.begin(grace);
    }

    /// The process-diagnostics hub this authority owns.
    #[must_use]
    pub fn diagnostics(&self) -> Arc<DiagnosticsHub> {
        Arc::clone(&self.diagnostics)
    }

    fn join(&mut self) {
        self.discovery.settle();
        self.stop.store(true, Ordering::SeqCst);
        self.audit.wake();
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Drop for ServiceHost {
    fn drop(&mut self) {
        self.join();
        let runtime_path = self.home.join(RUNTIME_FILE_NAME);
        if RuntimeFile::read(&runtime_path)
            .ok()
            .flatten()
            .is_some_and(|current| current.token == self.token)
        {
            let _ = fs::remove_file(&runtime_path);
        }
    }
}

/// How the service ends on request: refuse new work, drain what runs, then leave the desktop.
#[derive(Clone)]
struct Quit {
    drain: Arc<Drain>,
    signal: Arc<Mutex<Option<QuitSignal>>>,
    diagnostics: Arc<DiagnosticsHub>,
}

impl Quit {
    /// Begin a quit unless one is under way, and ask the desktop to exit once the work in flight
    /// has finished or `grace` has passed.
    fn begin(&self, grace: Duration) {
        if !self.drain.begin_quit() {
            return;
        }
        let drain = Arc::clone(&self.drain);
        let signal = Arc::clone(&self.signal);
        let diagnostics = Arc::clone(&self.diagnostics);
        let spawned = thread::Builder::new()
            .name("ghostlight-quit-drain".into())
            .spawn(move || {
                let drained = drain.wait_idle(grace);
                diagnostics.sink().emit(
                    ghostlight_bridge::diagnostics::event::PROCESS_EXITED,
                    ghostlight_bridge::diagnostics::Level::Info,
                    None,
                    &format!("quit: drained={drained} in_flight={}", drain.in_flight()),
                );
                let signal = lock(&signal).clone();
                if let Some(signal) = signal {
                    signal();
                }
            });
        if spawned.is_err() {
            self.diagnostics.sink().emit(
                ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
                ghostlight_bridge::diagnostics::Level::Error,
                None,
                "the quit drain could not start",
            );
        }
    }
}

/// Write the failure templates connectors answer with while the service is away.
fn write_templates(path: &Path) -> Result<()> {
    let templates: FailureTemplates =
        serde_json::from_value(failure_templates()).context("shape the failure templates")?;
    let bytes = serde_json::to_vec(&templates)?;
    let mut temporary = OsString::from(path.as_os_str());
    temporary.push(TEMPORARY_SUFFIX);
    let temporary = PathBuf::from(temporary);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })?;
    Ok(())
}

/// Everything a relay connection needs from the service.
struct Intake {
    token: String,
    executor: Arc<ApplicationExecutor>,
    workspaces: WorkspaceStore,
    browser_port: Arc<dyn BrowserPort>,
    browser: Arc<RelayBrowserPort>,
    workbench: WorkbenchFacade,
    governance: GovernanceFacade,
    diagnostics: Arc<DiagnosticsHub>,
    quit: Quit,
    sessions: Capacity,
    budget: QueueBudget,
    stop: Arc<AtomicBool>,
}

/// Accept relay connections until the service stops, each on its own thread.
///
/// A connection holds one of a few pre-authentication slots until its opening is judged, so
/// local processes that connect and say nothing cannot exhaust the service.
fn spawn_listener(listener: TcpListener, intake: Arc<Intake>) -> JoinHandle<()> {
    let handshakes = Capacity::new(admission::HANDSHAKES);
    thread::Builder::new()
        .name("ghostlight-relay-listener".into())
        .spawn(move || {
            while !intake.stop.load(Ordering::SeqCst) {
                let Ok((connection, _)) = listener.accept() else {
                    continue;
                };
                if intake.stop.load(Ordering::SeqCst) {
                    break;
                }
                let Some(handshake) = handshakes.acquire(1) else {
                    continue;
                };
                let intake = Arc::clone(&intake);
                let _ = thread::Builder::new()
                    .name("ghostlight-relay-connection".into())
                    .spawn(move || {
                        let mut connection = connection;
                        let _ = connection.set_nodelay(true);
                        let judgement = listener::judge(
                            &mut connection,
                            &intake.token,
                            intake.quit.drain.is_quitting(),
                        );
                        drop(handshake);
                        serve_connection(connection, judgement, &intake);
                    });
            }
        })
        .expect("relay listener thread starts")
}

/// Serve one judged connection to its end.
fn serve_connection(mut connection: TcpStream, judgement: Judgement, intake: &Arc<Intake>) {
    match judgement {
        Judgement::Drop => {}
        Judgement::Refuse(refusal) => listener::refuse(&mut connection, refusal),
        Judgement::Admit(Stream::Browser) => {
            if listener::accept(&mut connection).is_ok() {
                // The first frame on the stream is the extension's own hello.
                let _ = intake.browser.attach(connection);
            }
        }
        Judgement::Admit(Stream::Control { op }) => serve_control(connection, op, intake),
        Judgement::Admit(Stream::Mcp {
            channel,
            session,
            resume,
        }) => {
            if serve_mcp(connection, intake, &channel, session, resume.as_deref()).is_err() {
                eprintln!("MCP session ended after an invalid or interrupted exchange.");
            }
        }
        Judgement::Admit(Stream::Unknown) => {
            listener::refuse(&mut connection, OpeningRefusal::Unsupported)
        }
    }
}

/// Answer one control operation and close.
fn serve_control(mut connection: TcpStream, op: ControlOp, intake: &Intake) {
    let value = match op {
        ControlOp::Readiness => {
            serde_json::to_value(intake.workbench.snapshot().readiness).unwrap_or(Value::Null)
        }
        ControlOp::ActivateWorkbench => {
            json!({ listener::control::AVAILABLE: intake.workbench.reveal().is_ok() })
        }
        ControlOp::Quit => {
            intake.quit.begin(QUIT_GRACE);
            json!({ listener::control::QUITTING: true })
        }
        ControlOp::Unknown => {
            listener::refuse(&mut connection, OpeningRefusal::Unsupported);
            return;
        }
    };
    if listener::accept(&mut connection).is_ok() {
        let _ = listener::answer_control(&mut connection, value);
    }
}

/// Serve one MCP client session on an accepted `mcp` stream.
///
/// The opening names the channel and, for the command line, the process that owns the session.
/// A resume token continues an earlier session: its revision, client and owned tabs come back,
/// and tabs the browser no longer has stay stale. Tabs of a browser that has not returned yet
/// stay in the token and are bound when it does.
fn serve_mcp(
    mut connection: TcpStream,
    intake: &Arc<Intake>,
    channel: &str,
    session: Option<Value>,
    resume: Option<&str>,
) -> Result<()> {
    let Some(_session_permit) = intake.sessions.acquire(1) else {
        listener::refuse(&mut connection, OpeningRefusal::Capacity);
        return Ok(());
    };
    let marker = match session.map(serde_json::from_value::<SessionMarker>) {
        None => None,
        Some(Ok(marker)) => Some(marker),
        Some(Err(_)) => {
            listener::refuse(&mut connection, OpeningRefusal::Malformed);
            return Ok(());
        }
    };
    let intake_channel = crate::intake::intake_channel(channel);
    // Admission, before any workspace exists: an authority layer may decline an intake entirely.
    if !intake.governance.admits_channel(intake_channel).allowed {
        listener::refuse(
            &mut connection,
            OpeningRefusal::ChannelNotAdmitted(intake_channel.as_str()),
        );
        return Ok(());
    }
    let resumed = resume
        .and_then(|token| ResumeState::decode(token).ok())
        .map(|mut state| {
            if state.channel.is_empty() {
                state.channel = channel.into();
            }
            state
        });
    listener::accept(&mut connection)?;
    connection.set_nonblocking(false)?;
    let observed_local = connection.local_addr().ok();
    let observed_peer = connection.peer_addr().ok();
    let mut reader =
        SocketReader::new(connection.try_clone()?).interrupted_by(Arc::clone(&intake.stop));
    let writer = Arc::new(SocketWriter::new(connection)?);
    let opening = Opening {
        channel: intake_channel,
        marker,
        observed_local,
        observed_peer,
    };

    let (wire, mut live) = match resumed {
        Some(state) => {
            let owned_tabs = state.owned_tabs.clone();
            let wire = Arc::new(McpWire::new(
                Arc::clone(&writer),
                McpSession::resumed(state),
            ));
            let live = LiveSession::open(intake, &wire, &opening)?;
            let unrestored = restore_tabs(
                intake,
                &live.workspace,
                &owned_tabs,
                Instant::now() + RESTORE_DEADLINE,
            );
            *lock(&wire.unrestored) = unrestored;
            wire.publish_resume(&intake.workspaces, &live.workspace);
            (wire, Some(live))
        }
        None => (
            Arc::new(McpWire::new(Arc::clone(&writer), McpSession::new(channel))),
            None,
        ),
    };

    // Every way this connection can end has to reach the teardown below. A reset socket, an
    // oversized frame and one malformed frame are all ordinary ways for a client to go away.
    let served = (|| -> Result<()> {
        while let Some(frame) = reader.native::<McpFrame>(None)? {
            let McpFrame::Message { line } = frame else {
                continue;
            };
            let steps = wire.accept(&line);
            let opened = live.is_none() && wire.is_initialized();
            if opened {
                live = Some(LiveSession::open(intake, &wire, &opening)?);
            }
            for step in steps {
                match step {
                    Step::Reply(reply) => wire.send(&reply),
                    Step::Forward(request) => {
                        if let Some(live) = &live {
                            if !live.handle(intake, &wire, request) {
                                return Ok(());
                            }
                        }
                    }
                }
            }
            if let (true, Some(live)) = (opened, &live) {
                wire.publish_resume(&intake.workspaces, &live.workspace);
            }
        }
        Ok(())
    })();

    writer.close();
    if let Some(live) = live {
        live.close(intake);
    }
    served
}

/// What an `mcp` opening said about its session.
struct Opening {
    channel: IntakeChannel,
    marker: Option<SessionMarker>,
    observed_local: Option<std::net::SocketAddr>,
    observed_peer: Option<std::net::SocketAddr>,
}

/// One session's engine and the stream its lines go out on.
struct McpWire {
    writer: Arc<SocketWriter>,
    session: Mutex<McpSession>,
    resume: Mutex<Option<String>>,
    /// Resumed tabs whose browser has not listed its tabs yet. They stay in every resume token
    /// until that browser answers.
    unrestored: Mutex<Vec<OwnedTab>>,
}

impl McpWire {
    fn new(writer: Arc<SocketWriter>, session: McpSession) -> Self {
        Self {
            writer,
            session: Mutex::new(session),
            resume: Mutex::new(None),
            unrestored: Mutex::new(Vec::new()),
        }
    }

    fn accept(&self, line: &str) -> Vec<Step> {
        lock(&self.session).accept(line)
    }

    fn is_initialized(&self) -> bool {
        lock(&self.session).is_initialized()
    }

    fn client_label(&self) -> String {
        lock(&self.session).client_label().to_owned()
    }

    fn bind_session(&self, workspace: &WorkspaceId) {
        lock(&self.session).bind_session(workspace.as_str());
    }

    /// Write one MCP line to the client.
    fn send(&self, line: &Value) {
        let _ = self.writer.native(&McpFrame::Message {
            line: line.to_string(),
        });
    }

    /// Render one answer from the session queue and write its lines.
    fn render(&self, response: ServiceResponse) {
        let lines = lock(&self.session).render(response);
        for line in &lines {
            self.send(line);
        }
    }

    /// Hand the connector a fresh resume token when what it would restore has changed.
    fn publish_resume(&self, workspaces: &WorkspaceStore, workspace: &WorkspaceId) {
        let browser = workspaces.browser_of(workspace.as_str());
        let mut owned_tabs: Vec<OwnedTab> = browser
            .map(|browser_id| {
                workspaces
                    .tab_bindings(workspace.as_str())
                    .into_iter()
                    .map(|(handle, tab_id)| OwnedTab {
                        browser_id: browser_id.clone(),
                        tab_id,
                        handle,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let unrestored: Vec<OwnedTab> = lock(&self.unrestored)
            .iter()
            .filter(|tab| !owned_tabs.iter().any(|bound| bound.handle == tab.handle))
            .cloned()
            .collect();
        owned_tabs.extend(unrestored);
        let Some(state) = lock(&self.session).resume_state(owned_tabs, None) else {
            return;
        };
        let token = state.encode();
        let mut last = lock(&self.resume);
        if last.as_deref() == Some(token.as_str()) {
            return;
        }
        if self
            .writer
            .native(&McpFrame::Resume {
                token: token.clone(),
            })
            .is_ok()
        {
            *last = Some(token);
        }
    }
}

/// The workspace, queue and workers of a session that has been initialized.
struct LiveSession {
    workspace: WorkspaceId,
    connection: Arc<ConnectionEvidence>,
    queue: Arc<SessionQueue>,
    workers: Vec<JoinHandle<()>>,
    watch: Option<JoinHandle<()>>,
    watch_stop: Arc<AtomicBool>,
    published_catalog: Arc<Mutex<Option<Vec<ToolDefinition>>>>,
}

impl LiveSession {
    /// Open the session's workspace and start its workers and its catalog watch.
    fn open(intake: &Arc<Intake>, wire: &Arc<McpWire>, opening: &Opening) -> Result<Self> {
        // Before opening anything, release workspaces whose owner is gone and close the tabs they
        // still hold. Sweeping here rather than on a timer keeps the cost proportional to use.
        reap_finished_sessions(&intake.workspaces, intake.browser_port.as_ref());
        // Evidence belongs to this connection. Workspace continuity never replaces it.
        let connection = Arc::new(ConnectionEvidence::observe(
            &wire.client_label(),
            opening.channel,
            opening.observed_local,
            opening.observed_peer,
        ));
        let workspace = intake
            .workspaces
            .connect(connection.clone(), opening.marker.clone());
        wire.bind_session(&workspace);
        intake.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::HARNESS_ATTACHED,
            ghostlight_bridge::diagnostics::Level::Info,
            None,
            &format!(
                "{} via {}",
                connection
                    .attribution()
                    .connection_id
                    .as_deref()
                    .unwrap_or_default(),
                opening.channel.as_str()
            ),
        );
        let queue = Arc::new(SessionQueue::new(
            intake.budget.clone(),
            Arc::clone(&intake.quit.drain),
        ));
        let published_catalog: Arc<Mutex<Option<Vec<ToolDefinition>>>> = Arc::new(Mutex::new(None));
        let watch_stop = Arc::new(AtomicBool::new(false));
        let mut live = Self {
            workspace,
            connection,
            queue,
            workers: Vec::new(),
            watch: None,
            watch_stop,
            published_catalog,
        };
        live.watch = Some(live.spawn_watch(intake, wire)?);
        for independent in [false, true] {
            let worker = live.spawn_worker(intake, wire, independent);
            match worker {
                Ok(worker) => live.workers.push(worker),
                Err(error) => {
                    live.close(intake);
                    return Err(error);
                }
            }
        }
        Ok(live)
    }

    /// The once-a-second thread that advances cancelled work, keeps waiting work visible, binds
    /// resumed tabs whose browser has returned, and tells the client when policy changes the
    /// catalog it was given.
    fn spawn_watch(&self, intake: &Arc<Intake>, wire: &Arc<McpWire>) -> Result<JoinHandle<()>> {
        let intake = Arc::clone(intake);
        let queue = Arc::clone(&self.queue);
        let executor = Arc::clone(&intake.executor);
        let workspace = self.workspace.clone();
        let governance = intake.governance.clone();
        let wire = Arc::clone(wire);
        let published = Arc::clone(&self.published_catalog);
        let stop = Arc::clone(&self.watch_stop);
        thread::Builder::new()
            .name("ghostlight-policy-catalog".into())
            .spawn(move || {
                let mut generation = 1_u64;
                while !stop.load(Ordering::SeqCst) {
                    if !queue.wait_idle_or_timeout(Duration::from_secs(1)) {
                        break;
                    }
                    queue.advance_cancelled();
                    for job in queue.jobs() {
                        executor.show_waiting(&workspace, &job.prepared);
                    }
                    restore_returned_tabs(&intake, &wire, &workspace);
                    let Some(previous) = lock(&published).clone() else {
                        continue;
                    };
                    let current = catalog_for(&governance.snapshot());
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
                        wire.render(ServiceResponse::CatalogChanged {
                            generation,
                            tools: current,
                        });
                    }
                }
            })
            .context("start the session watch")
    }

    /// One bounded worker: ordinary work, or the independent lane that keeps controls moving.
    fn spawn_worker(
        &self,
        intake: &Arc<Intake>,
        wire: &Arc<McpWire>,
        independent: bool,
    ) -> Result<JoinHandle<()>> {
        let queue = Arc::clone(&self.queue);
        let executor = Arc::clone(&intake.executor);
        let workspace = self.workspace.clone();
        let workspaces = intake.workspaces.clone();
        let wire = Arc::clone(wire);
        thread::Builder::new()
            .name("ghostlight-session-work".into())
            .spawn(move || {
                while let Some(job) = queue.next(independent) {
                    let result = executor.execute_prepared(
                        &workspace,
                        &job.prepared,
                        &job.cancellation,
                        false,
                    );
                    deliver_result(&wire, &job.id, result);
                    wire.publish_resume(&workspaces, &workspace);
                    queue.complete(&job.id);
                }
            })
            .context("start bounded session worker")
    }

    /// Serve one request the engine forwarded. Returns false when the session must end.
    fn handle(&self, intake: &Arc<Intake>, wire: &Arc<McpWire>, request: ServiceRequest) -> bool {
        match request {
            ServiceRequest::Catalog => {
                let tools = catalog_for(&intake.governance.snapshot());
                wire.render(ServiceResponse::Catalog {
                    tools: tools.clone(),
                });
                *lock(&self.published_catalog) = Some(tools);
            }
            ServiceRequest::Invoke {
                id,
                tool,
                input,
                deadline_ms,
            } => {
                // Bound retained payloads as well as queue count. Decoding occurs once at intake.
                let bytes = serde_json::to_vec(&input).map_or(0, |input| input.len())
                    + id.len()
                    + tool.len()
                    + QUEUED_METADATA_BYTES;
                let job = Job {
                    id,
                    prepared: Arc::new(PreparedInvocation::new(
                        &tool,
                        input,
                        deadline_ms,
                        Some(self.connection.clone()),
                    )),
                    cancellation: CancellationToken::default(),
                };
                match self.queue.admit(job.clone(), bytes) {
                    Ok(()) => {}
                    Err(Refused::Duplicate) => wire.render(ServiceResponse::Error {
                        id: Some(job.id),
                        code: "duplicate_request".into(),
                        message: "Request id is already active.".into(),
                    }),
                    Err(Refused::Full) => match self.queue.begin_unqueued() {
                        Ok(_flight) => {
                            let result = intake.executor.execute_prepared(
                                &self.workspace,
                                &job.prepared,
                                &job.cancellation,
                                true,
                            );
                            deliver_result(wire, &job.id, result);
                            wire.publish_resume(&intake.workspaces, &self.workspace);
                        }
                        Err(_) => refuse_while_quitting(wire, job.id),
                    },
                    Err(Refused::ShuttingDown) => refuse_while_quitting(wire, job.id),
                    Err(Refused::Closed) => return false,
                }
            }
            ServiceRequest::Cancel { id } => self.queue.cancel(&id),
        }
        true
    }

    /// Stop the workers, detach the connection, and release a workspace nobody owns.
    fn close(self, intake: &Intake) {
        self.watch_stop.store(true, Ordering::SeqCst);
        self.queue.stop();
        if let Some(watch) = self.watch {
            let _ = watch.join();
        }
        for worker in self.workers {
            let _ = worker.join();
        }
        intake.workspaces.disconnect(
            &self.workspace,
            self.connection
                .attribution()
                .connection_id
                .as_deref()
                .unwrap_or_default(),
        );
        // A workspace with an owner outlives this connection: the caller is still there and its
        // next call must reach the same tabs. It is released when its owner is gone, not when a
        // socket is.
        if !intake.workspaces.is_owned(&self.workspace) {
            let released = intake.workspaces.release(&self.workspace);
            cleanup_released_tabs(
                self.workspace.as_str(),
                &released,
                intake.browser_port.as_ref(),
                &intake.workspaces,
            );
        }
        intake.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::HARNESS_DETACHED,
            ghostlight_bridge::diagnostics::Level::Info,
            None,
            self.workspace.as_str(),
        );
    }
}

/// Bind a resumed session's tabs again, checking each browser once for the tabs it still has.
///
/// Each browser is waited for until `until`, since after a service restart a browser returns
/// some time after the client does. Only the browser's own tab list stales a tab: a tab it no
/// longer lists, or one another session now controls, is not bound, and its handle answers as
/// stale, which is the truth after a restart. The tabs of a browser that could not be asked come
/// back unrestored, to be kept in the resume token and bound when that browser returns.
fn restore_tabs(
    intake: &Intake,
    workspace: &WorkspaceId,
    owned: &[OwnedTab],
    until: Instant,
) -> Vec<OwnedTab> {
    let mut by_browser: BTreeMap<&str, Vec<&OwnedTab>> = BTreeMap::new();
    for tab in owned {
        by_browser.entry(&tab.browser_id).or_default().push(tab);
    }
    let mut unrestored = Vec::new();
    for (browser, tabs) in by_browser {
        let cancelled = AtomicBool::new(false);
        let returned = intake
            .browser_port
            .await_available(browser, until, &cancelled)
            == Availability::Available;
        let listed = returned.then(|| {
            intake.browser_port.call(
                browser,
                workspace.as_str(),
                BrowserCommand::ListTabs,
                Instant::now() + RESTORE_DEADLINE,
                &cancelled,
            )
        });
        let Some(Ok(BrowserOutcome::Tabs { tabs: live })) = listed else {
            unrestored.extend(tabs.into_iter().cloned());
            continue;
        };
        // A session already working in another browser cannot hold these tabs: they are stale.
        if intake
            .workspaces
            .pin_browser(workspace.as_str(), browser)
            .is_err()
        {
            continue;
        }
        let Ok(lease) = intake.workspaces.acquire(workspace) else {
            unrestored.extend(tabs.into_iter().cloned());
            continue;
        };
        for tab in tabs {
            if let Some(physical) = live.iter().find(|live| live.tab_id == tab.tab_id) {
                let _ = lease.restore_tab(&tab.handle, physical);
            }
        }
    }
    unrestored
}

/// Bind the resumed tabs of browsers that have returned since the session resumed, without
/// waiting for any browser, and hand the client a token that reflects them.
fn restore_returned_tabs(intake: &Intake, wire: &McpWire, workspace: &WorkspaceId) {
    let pending = lock(&wire.unrestored).clone();
    if pending.is_empty() {
        return;
    }
    let still = restore_tabs(intake, workspace, &pending, Instant::now());
    if still.len() == pending.len() {
        return;
    }
    *lock(&wire.unrestored) = still;
    wire.publish_resume(&intake.workspaces, workspace);
}

/// Answer an invocation that arrived after a quit began.
fn refuse_while_quitting(wire: &McpWire, id: String) {
    wire.render(ServiceResponse::Error {
        id: Some(id),
        code: SHUTTING_DOWN_CODE.into(),
        message: SHUTTING_DOWN_MESSAGE.into(),
    });
}

/// Take the service lease at `path`, asking again until `wait` has passed while another process
/// holds it.
fn acquire_lease(path: &Path, wait: Duration) -> Result<Lease> {
    let give_up = Instant::now() + wait;
    loop {
        if let Some(lease) = Lease::try_acquire(path).context("open the service lease")? {
            return Ok(lease);
        }
        let remaining = give_up.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(AuthorityAlreadyRunning.into());
        }
        thread::sleep(LEASE_POLL.min(remaining));
    }
}

/// Remove the images earlier installs moved aside in the directory `executable` runs from. The
/// lease is held, so no other service of this home is starting.
fn remove_old_images_beside(executable: &Path) {
    if let Some(directory) = executable.parent() {
        install::swap::remove_old_images(directory);
    }
}

/// Register every browser that has no Ghostlight registration, when `executable` serves the
/// installation in `home` (the installed service or the marked development build). A person who
/// installed through a route that never registered a browser gets one on the first start; a
/// registration that exists is the installer's to reclaim, never this start's.
fn register_missing_browsers(home: &Path, executable: &Path) {
    if !install::reconcile::native_hosts_allowed() {
        return;
    }
    let layout = install::layout::Layout::new(home);
    let executable = install::entry::canonical_path(executable);
    let marked = install::marker::DevelopmentMarker::read(&layout.development_marker())
        .ok()
        .flatten()
        .map(|marker| marker.executable);
    let serving = std::iter::once(layout.service_executable())
        .chain(marked)
        .any(|candidate| {
            install::entry::same_path(&install::entry::canonical_path(&candidate), &executable)
        });
    if serving {
        let _ = install::native_host::NativeHostRegistry::discover().register_missing();
    }
}

/// Close the tabs of every session whose owning process is gone.
fn reap_finished_sessions(workspaces: &WorkspaceStore, browser: &dyn BrowserPort) {
    for released in workspaces.reap(&owner_alive) {
        cleanup_released_tabs("reaped", &released, browser, workspaces);
    }
}

/// Erase service-owned browser diagnostics before released tabs are offered to the close interlock,
/// then settle each tab's place in the released pool from its close answer.
///
/// Cleanup goes to the browser that holds the tabs and nowhere else. A workspace that never opened
/// a browser has nothing to clean up. A tab becomes reusable only when the person's preserve-tabs
/// setting refused its close: the browser then already dropped the dead workspace's binding and
/// debugger hold and keeps the tab open. Until its close answers, no other session can claim it,
/// so this close can never act on a tab another session took (ADR-0196 Decisions 8 and 9).
fn cleanup_released_tabs(
    workspace: &str,
    released: &ReleasedTabs,
    browser: &dyn BrowserPort,
    workspaces: &WorkspaceStore,
) {
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
        let closed = browser.call(
            target,
            workspace,
            BrowserCommand::CloseTab {
                tab_id,
                released: true,
            },
            Instant::now() + Duration::from_secs(2),
            &cancelled,
        );
        let kept_open = matches!(closed, Err(BrowserError::LocalInterlock(_)));
        workspaces.settle_released(target, tab_id, kept_open);
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

/// Hand one finished invocation to the session as a result, or end the stream when the result
/// cannot be encoded.
fn deliver_result(wire: &McpWire, id: &str, mut result: crate::work::result::InvocationResult) {
    let text = result.model_text();
    let is_error = result.is_error();
    let content = std::mem::take(&mut result.content);
    match serde_json::to_value(&result) {
        Ok(value) => wire.render(ServiceResponse::Result {
            id: id.into(),
            text,
            result: value,
            is_error,
            content,
        }),
        Err(_) => wire.writer.close(),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct ServiceBrowserEvents {
    governance: GovernanceFacade,
    workspaces: WorkspaceStore,
    browser: Arc<dyn BrowserPort>,
}

impl BrowserEventSink for ServiceBrowserEvents {
    fn on_event(&self, browser: &str, event: BrowserEvent) {
        match event {
            BrowserEvent::DocumentCommitted { tab_id, url, .. } => {
                // A commit is browser state, not proof of a Ghostlight action. Never authorize,
                // audit, or react to it as work. Only stale our own references; the executor
                // checks the current destination when an agent actually requests an operation.
                self.workspaces.note_browser_landing(browser, tab_id, &url);
            }
            BrowserEvent::ReadinessChanged { tab_id, readiness } => self
                .workspaces
                .apply_browser_readiness(browser, tab_id, readiness),
            BrowserEvent::RuntimeControlRequested { intent } => {
                let state = self.governance.apply_runtime_intent(intent);
                let _ = self.browser.publish_control_state(state);
            }
            BrowserEvent::TabClosed { tab_id } => {
                self.workspaces.apply_browser_close(browser, tab_id)
            }
            // A browser that comes back may have restarted and reused its tab ids, so nothing it
            // left behind stays reusable.
            BrowserEvent::Disconnected => self.workspaces.apply_browser_disconnect(browser),
            // Attention never reaches this sink: it is browser routing state, and the port that
            // knows which connection reported it is the only thing that needs it.
            BrowserEvent::Attended => {}
            BrowserEvent::DialogChanged { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpStream;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::intake::session::IntakeChannel;
    use ghostlight_relay::client::{self, REPLY_DEADLINE};
    use ghostlight_relay::frame;
    use ghostlight_relay::home::{RUNTIME_FILE_NAME, TEMPLATES_FILE_NAME};
    use ghostlight_relay::opening::{ControlOp, McpFrame, RefusalCode, Reply, Stream};
    use ghostlight_relay::runtime::RuntimeFile;
    use ghostlight_relay::templates::FailureTemplates;
    use serde_json::{json, Value};

    use super::test_home::TestHome;
    use crate::browser::adapter::BrowserCommand;
    use crate::browser::testing::{FakeBrowser, FAKE_BROWSER};
    use crate::cli::client as cli_client;
    use crate::intake::mcp::ResumeState;
    use crate::workbench::{
        WorkbenchNotification, WorkbenchPresentationError, WorkbenchPresentationPort,
    };
    use crate::workspace::ReleasedTabs;

    use super::{cleanup_released_tabs, ServiceHost};

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
    fn passive_browsing_never_becomes_work_or_claims_a_child_tab() {
        use crate::audit::{AuditRecorder, JsonlAuditSink};
        use crate::browser::adapter::{BrowserEvent, BrowserReadiness, PhysicalTab};
        use crate::browser::BrowserEventSink;
        use crate::governance::GovernanceFacade;
        use crate::presentation::{BrowserPresentation, PresentationReactor};
        use crate::work::result::Status;
        use crate::work::{ApplicationExecutor, CancellationToken};
        use crate::workbench::{WorkbenchFacade, WorkbenchProjection};
        use crate::workspace::WorkspaceStore;

        let (directory, _) = runtime_path("passive-browsing");
        let policy = directory.join("policy.json");
        std::fs::write(&policy, r#"{"schema":3,"name":"test","version":"1","grants":[{"id":"read","hosts":{"allow":["allowed.example"]},"allowed":["read"]}]}"#).unwrap();
        let governance = GovernanceFacade::new(Some(policy), None);
        let workspaces = WorkspaceStore::default();
        let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp, None);
        workspaces
            .pin_browser(workspace.as_str(), FAKE_BROWSER)
            .unwrap();
        let tab = PhysicalTab {
            tab_id: 7,
            url: "https://allowed.example/".into(),
            title: "test".into(),
            active: true,
            readiness: BrowserReadiness::Complete,
        };
        let lease = workspaces.acquire(&workspace).unwrap();
        let selected = lease.add_tab(&tab).unwrap();
        let selected = lease.apply_landing(&selected.handle, &tab).unwrap();
        drop(lease);
        let browser = Arc::new(FakeBrowser::default());
        let diagnostics = crate::diagnostics::DiagnosticsHub::for_tests();
        let projection = WorkbenchProjection::default();
        let audit_path = directory.join("audit.jsonl");
        let executor = ApplicationExecutor::new(
            governance.clone(),
            workspaces.clone(),
            browser.clone(),
            PresentationReactor::new(Arc::new(BrowserPresentation::new(
                browser.clone(),
                workspaces.clone(),
            ))),
            projection.clone(),
            Arc::new(AuditRecorder::new(
                Arc::new(JsonlAuditSink::new(&audit_path)),
                projection.clone(),
            )),
            diagnostics.clone(),
        );
        let sink = super::ServiceBrowserEvents {
            governance: governance.clone(),
            workspaces: workspaces.clone(),
            browser: browser.clone(),
        };
        let view = WorkbenchFacade::new(
            projection,
            workspaces.clone(),
            governance,
            Arc::new(crate::browser::RelayBrowserPort::new(
                "passive-test".into(),
                super::page_assets(),
            )),
            diagnostics,
        );

        let initial_audit = std::fs::read_to_string(&audit_path).unwrap_or_default();

        // Correlation only means a command was watching. It cannot prove who navigated.
        for correlation in [None, Some("in-flight-command".into())] {
            for url in [
                "chrome://version",
                "https://denied.example/private?secret=1",
                "https://allowed.example/",
            ] {
                sink.on_event(
                    FAKE_BROWSER,
                    BrowserEvent::DocumentCommitted {
                        tab_id: 7,
                        url: url.into(),
                        correlation: correlation.clone(),
                    },
                );
                let current = workspaces
                    .acquire(&workspace)
                    .unwrap()
                    .select_tab(Some(selected.handle.as_str()))
                    .unwrap();
                assert_eq!(current.url, url);
                assert!(current.generation > selected.generation);
                assert_eq!(workspaces.summaries()[0].held_tab_count, 0);
                assert!(view.snapshot().history.is_empty());
                assert!(view.snapshot().operations.is_empty());
                assert!(browser.calls().is_empty());
                assert!(browser.control_states().is_empty());
                assert_eq!(
                    std::fs::read_to_string(&audit_path).unwrap_or_default(),
                    initial_audit
                );
            }
        }
        // A tab Ghostlight never opened stays entirely unowned when it commits.
        sink.on_event(
            FAKE_BROWSER,
            BrowserEvent::DocumentCommitted {
                tab_id: 8,
                url: "chrome://version".into(),
                correlation: None,
            },
        );
        assert!(workspaces.owner_of_physical(FAKE_BROWSER, 8).is_none());
        assert!(browser.calls().is_empty());
        assert_eq!(
            std::fs::read_to_string(&audit_path).unwrap_or_default(),
            initial_audit
        );

        // It is the next agent request, not the person's visit, that receives a refusal.
        sink.on_event(
            FAKE_BROWSER,
            BrowserEvent::DocumentCommitted {
                tab_id: 7,
                url: "chrome://version".into(),
                correlation: None,
            },
        );
        let result = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":selected.handle.as_str()}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert!(browser
            .calls()
            .iter()
            .all(|call| matches!(call, BrowserCommand::Present { .. })));
        let snapshot = view.snapshot();
        assert_eq!(snapshot.history.len(), 1);
        assert_eq!(snapshot.history[0].tool, "browser_read");
        let bytes = std::fs::read_to_string(&audit_path).unwrap();
        assert!(!bytes.contains("browser_landing"));
        assert!(!bytes.contains("secret"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    /// The client label of the session whose tabs the release cleanup tests leave behind.
    const RELEASED_CLIENT: &str = "released";

    /// Admit a workspace in the fake browser, land one example.com tab per id, and release it.
    fn release_example_tabs(
        workspaces: &crate::workspace::WorkspaceStore,
        ids: &[u64],
    ) -> ReleasedTabs {
        use crate::browser::adapter::{BrowserReadiness, PhysicalTab};

        let workspace = workspaces.admit(RELEASED_CLIENT.into(), IntakeChannel::Mcp, None);
        workspaces
            .pin_browser(workspace.as_str(), FAKE_BROWSER)
            .unwrap();
        let lease = workspaces.acquire(&workspace).unwrap();
        for &tab_id in ids {
            let tab = PhysicalTab {
                tab_id,
                url: "https://example.com/".into(),
                title: "Example".into(),
                active: true,
                readiness: BrowserReadiness::Complete,
            };
            let added = lease.add_tab(&tab).unwrap();
            lease.apply_landing(&added.handle, &tab).unwrap();
        }
        drop(lease);
        workspaces.release(&workspace)
    }

    /// Run release cleanup against a browser whose preserve-tabs setting keeps every tab open.
    fn clean_up_kept_open(workspaces: &crate::workspace::WorkspaceStore, released: &ReleasedTabs) {
        use crate::browser::adapter::BrowserOutcome;
        use crate::browser::BrowserError;

        let browser = FakeBrowser::default();
        browser.push(Ok(BrowserOutcome::DiagnosticsCleared {
            cleared_count: released.physical_ids.len(),
        }));
        for _ in &released.physical_ids {
            browser.push(Err(BrowserError::LocalInterlock("preserved".into())));
        }
        cleanup_released_tabs("released", released, &browser, workspaces);
    }

    /// The tab a later session of the same client opening example.com would reuse, if any.
    fn reusable_example_tab(workspaces: &crate::workspace::WorkspaceStore) -> Option<u64> {
        workspaces
            .claim_released(FAKE_BROWSER, RELEASED_CLIENT, "https://example.com/")
            .map(|released| released.physical_id)
    }

    #[test]
    fn only_a_tab_the_browser_kept_open_after_release_cleanup_is_reusable() {
        use crate::browser::adapter::BrowserOutcome;
        use crate::browser::BrowserError;
        use crate::workspace::WorkspaceStore;

        let workspaces = WorkspaceStore::default();
        let mut released = release_example_tabs(&workspaces, &[1, 2, 3]);
        // The fake answers in call order, so close the tabs in a known order.
        released.physical_ids.sort_unstable();
        // Until its close answers, the browser may still bind each tab to the dead session, and
        // that close could still act on whoever took it, so nothing is claimable yet.
        assert_eq!(reusable_example_tab(&workspaces), None);

        let browser = FakeBrowser::default();
        browser.push(Ok(BrowserOutcome::DiagnosticsCleared { cleared_count: 3 }));
        // The browser closes tab 1, keeps tab 2 open by the person's preserve-tabs setting, and
        // never answers for tab 3 in time.
        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 1 }));
        browser.push(Err(BrowserError::LocalInterlock("preserved".into())));
        browser.push(Err(BrowserError::DeadlineAfterDispatch));
        cleanup_released_tabs("released", &released, &browser, &workspaces);

        assert_eq!(
            browser.calls(),
            vec![
                BrowserCommand::ClearDiagnostics {
                    tab_ids: vec![1, 2, 3]
                },
                BrowserCommand::CloseTab {
                    tab_id: 1,
                    released: true
                },
                BrowserCommand::CloseTab {
                    tab_id: 2,
                    released: true
                },
                BrowserCommand::CloseTab {
                    tab_id: 3,
                    released: true
                },
            ]
        );
        assert_eq!(reusable_example_tab(&workspaces), Some(2));
        assert_eq!(reusable_example_tab(&workspaces), None);
    }

    #[test]
    fn browser_events_take_released_tabs_out_of_reuse() {
        use crate::browser::adapter::BrowserEvent;
        use crate::browser::BrowserEventSink;
        use crate::governance::GovernanceFacade;
        use crate::workspace::WorkspaceStore;

        let workspaces = WorkspaceStore::default();
        let sink = super::ServiceBrowserEvents {
            governance: GovernanceFacade::new(None, None),
            workspaces: workspaces.clone(),
            browser: Arc::new(FakeBrowser::default()),
        };
        let closed = |tab_id| BrowserEvent::TabClosed { tab_id };
        // The person loaded a page in a tab Ghostlight left open: it is theirs again.
        let committed = |tab_id| BrowserEvent::DocumentCommitted {
            tab_id,
            url: "https://example.com/mine".into(),
            correlation: None,
        };

        let released = release_example_tabs(&workspaces, &[1, 2, 3]);
        clean_up_kept_open(&workspaces, &released);
        sink.on_event(FAKE_BROWSER, closed(1));
        sink.on_event(FAKE_BROWSER, committed(2));
        assert_eq!(reusable_example_tab(&workspaces), Some(3));
        assert_eq!(reusable_example_tab(&workspaces), None);

        // A close or a commit while cleanup is still running counts the same way: the cleanup
        // answer that follows does not bring the tab back.
        let released = release_example_tabs(&workspaces, &[5, 6]);
        sink.on_event(FAKE_BROWSER, closed(5));
        sink.on_event(FAKE_BROWSER, committed(6));
        clean_up_kept_open(&workspaces, &released);
        assert_eq!(reusable_example_tab(&workspaces), None);

        // A browser that reconnects may have restarted and handed its tab ids out again.
        let released = release_example_tabs(&workspaces, &[4]);
        clean_up_kept_open(&workspaces, &released);
        sink.on_event(FAKE_BROWSER, BrowserEvent::Disconnected);
        assert_eq!(reusable_example_tab(&workspaces), None);
    }

    /// The token every test service presents in its runtime file.
    const TEST_TOKEN: &str = "runtime_test";

    /// The MCP channel a connector names.
    const CONNECTOR_CHANNEL: &str = "npm";

    /// How long a test waits for the service to answer.
    const PATIENCE: Duration = Duration::from_secs(10);

    /// A relay listener over a fake browser, found through the test home's runtime file.
    struct Served {
        intake: Arc<super::Intake>,
        browser: Arc<FakeBrowser>,
        relay: Arc<crate::browser::RelayBrowserPort>,
        port: u16,
        listener: Option<thread::JoinHandle<()>>,
        quit_requested: std::sync::mpsc::Receiver<()>,
    }

    impl Drop for Served {
        fn drop(&mut self) {
            self.intake.stop.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(("127.0.0.1", self.port));
            if let Some(listener) = self.listener.take() {
                let _ = listener.join();
            }
        }
    }

    /// Page assets a protocol-4 fake can be accepted with.
    fn test_assets() -> Arc<crate::browser::adapter::chromium::protocol4::wire::Assets> {
        use sha2::{Digest, Sha256};
        use std::fmt::Write as _;

        let source = "\"use strict\";\n".to_owned();
        let mut id = String::new();
        for byte in Sha256::digest(source.as_bytes()) {
            write!(&mut id, "{byte:02x}").unwrap();
        }
        Arc::new(crate::browser::adapter::chromium::protocol4::wire::Assets { id, source })
    }

    /// Serve the relay listener over a fake browser and publish it in `home`.
    fn serve_fake(home: &TestHome) -> Served {
        serve(home, false)
    }

    /// Serve the relay listener with the relay port as the session's browser port, so sessions
    /// meet browsers only as protocol-4 extensions attach.
    fn serve_relayed(home: &TestHome) -> Served {
        serve(home, true)
    }

    /// Serve the relay listener in `home`; `relayed` puts the relay port behind the sessions.
    fn serve(home: &TestHome, relayed: bool) -> Served {
        use crate::audit::{AuditRecorder, JsonlAuditSink};
        use crate::governance::GovernanceFacade;
        use crate::presentation::{BrowserPresentation, PresentationReactor};
        use crate::work::ApplicationExecutor;
        use crate::workbench::{WorkbenchFacade, WorkbenchProjection};
        use crate::workspace::WorkspaceStore;

        let browser = Arc::new(FakeBrowser::default());
        let relay = Arc::new(crate::browser::RelayBrowserPort::new(
            "service_test".into(),
            test_assets(),
        ));
        let workspaces = WorkspaceStore::default();
        let governance = GovernanceFacade::new(None, None);
        let diagnostics = crate::diagnostics::DiagnosticsHub::for_tests();
        let projection = WorkbenchProjection::default();
        let audit = Arc::new(AuditRecorder::new(
            Arc::new(JsonlAuditSink::new(&home.path().join("audit.jsonl"))),
            projection.clone(),
        ));
        let executor = Arc::new(ApplicationExecutor::new(
            governance.clone(),
            workspaces.clone(),
            browser.clone(),
            PresentationReactor::new(Arc::new(BrowserPresentation::new(
                browser.clone(),
                workspaces.clone(),
            ))),
            projection.clone(),
            audit,
            diagnostics.clone(),
        ));
        let workbench = WorkbenchFacade::new(
            projection,
            workspaces.clone(),
            governance.clone(),
            relay.clone(),
            diagnostics.clone(),
        );
        let quit = super::Quit {
            drain: Arc::default(),
            signal: Arc::default(),
            diagnostics: diagnostics.clone(),
        };
        let (signal, quit_requested) = std::sync::mpsc::channel();
        *super::lock(&quit.signal) = Some(Arc::new(move || {
            let _ = signal.send(());
        }));
        let intake = Arc::new(super::Intake {
            token: TEST_TOKEN.into(),
            executor,
            workspaces,
            browser_port: if relayed {
                relay.clone()
            } else {
                browser.clone()
            },
            browser: relay.clone(),
            workbench,
            governance,
            diagnostics,
            quit,
            sessions: super::Capacity::new(super::admission::SESSIONS),
            budget: super::QueueBudget::default(),
            stop: Arc::default(),
        });
        let listener = crate::intake::listener::bind().unwrap();
        let port = listener.local_addr().unwrap().port();
        RuntimeFile::new(port, TEST_TOKEN.into(), "2.0.0".into())
            .write(&home.path().join(RUNTIME_FILE_NAME))
            .unwrap();
        let thread = super::spawn_listener(listener, intake.clone());
        Served {
            intake,
            browser,
            relay,
            port,
            listener: Some(thread),
            quit_requested,
        }
    }

    /// Open an `mcp` stream the way the MCP connector does.
    fn open_mcp(resume: Option<String>) -> TcpStream {
        let (socket, reply) = client::connect(
            Stream::Mcp {
                channel: CONNECTOR_CHANNEL.into(),
                session: None,
                resume,
            },
            REPLY_DEADLINE,
        )
        .unwrap();
        assert!(matches!(reply, Reply::Accepted { .. }), "{reply:?}");
        socket.set_read_timeout(Some(PATIENCE)).unwrap();
        socket
    }

    fn send_line(socket: &mut TcpStream, line: Value) {
        frame::write_frame(
            socket,
            &McpFrame::Message {
                line: line.to_string(),
            },
        )
        .unwrap();
    }

    /// Read frames until the response to `id`, collecting the resume tokens on the way.
    fn response(socket: &mut TcpStream, id: u64) -> (Value, Vec<String>) {
        let mut tokens = Vec::new();
        loop {
            match frame::read_frame::<McpFrame>(socket).unwrap().unwrap() {
                McpFrame::Message { line } => {
                    let message: Value = serde_json::from_str(&line).unwrap();
                    if message["id"] == json!(id) {
                        return (message, tokens);
                    }
                }
                McpFrame::Resume { token } => tokens.push(token),
                McpFrame::Unknown => {}
            }
        }
    }

    fn initialize(socket: &mut TcpStream) -> (Value, Vec<String>) {
        send_line(
            socket,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "relay test", "version": "1"}
                }
            }),
        );
        let answered = response(socket, 1);
        send_line(
            socket,
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        );
        answered
    }

    /// Wait until `condition` holds or the patience runs out.
    fn eventually(condition: impl Fn() -> bool) {
        let until = Instant::now() + PATIENCE;
        while !condition() {
            assert!(Instant::now() < until, "the condition never held");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn an_mcp_session_initializes_lists_tools_and_calls_one() {
        let home = TestHome::new("mcp-session");
        let served = serve_fake(&home);
        let mut socket = open_mcp(None);

        let (initialized, tokens) = initialize(&mut socket);
        assert_eq!(initialized["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(initialized["result"]["serverInfo"]["name"], "ghostlight");

        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        );
        let (listed, later) = response(&mut socket, 2);
        let tools = listed["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|tool| tool["name"] == "browser_tabs"));
        assert!(tools.iter().all(|tool| tool.get("outputSchema").is_none()));
        let resumed: Vec<String> = tokens.into_iter().chain(later).collect();
        let state =
            ResumeState::decode(resumed.first().expect("a resume token after initialize")).unwrap();
        assert_eq!(state.revision, "2025-06-18");
        assert_eq!(state.client_label, "relay test");

        send_line(
            &mut socket,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "browser_tabs", "arguments": {"action": "list"}}
            }),
        );
        let (called, _) = response(&mut socket, 3);
        let result = &called["result"];
        assert!(
            result["structuredContent"]["status"].is_string(),
            "{called}"
        );
        assert_eq!(result["content"][0]["type"], "text");
        assert_eq!(served.intake.workspaces.summaries().len(), 1);

        drop(socket);
        eventually(|| served.intake.workspaces.summaries().is_empty());
    }

    #[test]
    fn a_wrong_token_is_refused_and_opens_nothing() {
        let home = TestHome::new("bad-token");
        let served = serve_fake(&home);
        RuntimeFile::new(served.port, "runtime_wrong".into(), "2.0.0".into())
            .write(&home.path().join(RUNTIME_FILE_NAME))
            .unwrap();

        let (_, reply) = client::connect(Stream::Browser, REPLY_DEADLINE).unwrap();
        assert!(
            matches!(
                reply,
                Reply::Refused {
                    code: RefusalCode::AuthenticationFailed,
                    ..
                }
            ),
            "{reply:?}"
        );
        assert!(served.relay.connected_browsers().is_empty());
        assert!(served.intake.workspaces.summaries().is_empty());
    }

    #[test]
    fn a_resume_token_restores_the_session_and_stales_a_missing_tab() {
        use crate::browser::adapter::{BrowserOutcome, BrowserReadiness, PhysicalTab};
        use crate::intake::mcp::OwnedTab;

        let home = TestHome::new("resume");
        let served = serve_fake(&home);
        // The browser still has tab 7; tab 8 closed while the service was away.
        served.browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![PhysicalTab {
                tab_id: 7,
                url: "https://example.com/".into(),
                title: "Example".into(),
                active: true,
                readiness: BrowserReadiness::Complete,
            }],
        }));
        let token = ResumeState {
            revision: "2025-06-18".into(),
            client_label: "returning client".into(),
            channel: String::new(),
            session_key: Some("workspace_earlier".into()),
            owned_tabs: vec![
                OwnedTab {
                    browser_id: FAKE_BROWSER.into(),
                    tab_id: 7,
                    handle: "tab_kept".into(),
                },
                OwnedTab {
                    browser_id: FAKE_BROWSER.into(),
                    tab_id: 8,
                    handle: "tab_gone".into(),
                },
            ],
            recording: None,
        }
        .encode();

        let mut socket = open_mcp(Some(token));
        let McpFrame::Resume { token } =
            frame::read_frame::<McpFrame>(&mut socket).unwrap().unwrap()
        else {
            panic!("a resumed session hands back a fresh token first");
        };
        let state = ResumeState::decode(&token).unwrap();
        assert_eq!(state.revision, "2025-06-18");
        assert_eq!(state.client_label, "returning client");
        assert_eq!(state.channel, CONNECTOR_CHANNEL);
        assert_eq!(
            state.owned_tabs,
            vec![OwnedTab {
                browser_id: FAKE_BROWSER.into(),
                tab_id: 7,
                handle: "tab_kept".into(),
            }]
        );
        assert_eq!(served.browser.calls(), vec![BrowserCommand::ListTabs]);
        let summaries = served.intake.workspaces.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].client_label, "returning client");

        // A resumed session is initialized already: a tools call is served without initialize.
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 9, "method": "tools/list"}),
        );
        let (listed, _) = response(&mut socket, 9);
        assert!(listed["result"]["tools"].is_array(), "{listed}");
    }

    /// The browser identity of an extension that returns after its session resumed.
    const RETURNING_BROWSER: &str = "browser_returning";

    /// The tab the returning browser still holds.
    const RETURNING_TAB: u64 = 12;

    /// A resume token for a session that owned the returning browser's tab.
    fn returning_token() -> String {
        ResumeState {
            revision: "2025-06-18".into(),
            client_label: "returning client".into(),
            channel: String::new(),
            session_key: Some("workspace_earlier".into()),
            owned_tabs: vec![returning_tab()],
            recording: None,
        }
        .encode()
    }

    fn returning_tab() -> crate::intake::mcp::OwnedTab {
        crate::intake::mcp::OwnedTab {
            browser_id: RETURNING_BROWSER.into(),
            tab_id: RETURNING_TAB,
            handle: "tab_kept".into(),
        }
    }

    /// Attach a protocol-4 extension for the returning browser that still lists its tab.
    fn attach_returning_browser(
        served: &Served,
    ) -> crate::browser::adapter::chromium::protocol4::fake::FakeExtension {
        use crate::browser::adapter::chromium::protocol4::fake::{
            FakeExtension, Reply as FakeReply,
        };
        use crate::browser::adapter::chromium::protocol4::wire::Primitive;

        let native = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let fake = FakeExtension::connect(
            native.local_addr().unwrap(),
            &FakeExtension::hello(RETURNING_BROWSER),
            Box::new(|correlation, primitive| {
                let outcome = match primitive {
                    Primitive::ListTabs(_) => json!({"tabs": [{"tab_id": RETURNING_TAB,
                        "title": "Example", "url": "https://example.com/", "active": true,
                        "readiness": "complete"}]}),
                    _ => Value::Null,
                };
                FakeReply::Now(FakeExtension::ok(correlation, outcome))
            }),
        );
        served.relay.attach(native.accept().unwrap().0).unwrap();
        fake
    }

    /// The tabs the one session holds in the workspace store.
    fn bound_tabs(served: &Served) -> Vec<(String, u64)> {
        let summaries = served.intake.workspaces.summaries();
        assert_eq!(summaries.len(), 1);
        served.intake.workspaces.tab_bindings(&summaries[0].id)
    }

    #[test]
    fn a_resumed_session_waits_for_its_browser_to_return() {
        let home = TestHome::new("resume-returning");
        let served = serve_relayed(&home);
        let token = returning_token();
        let resumed = thread::spawn(move || {
            let mut socket = open_mcp(Some(token));
            let McpFrame::Resume { token } =
                frame::read_frame::<McpFrame>(&mut socket).unwrap().unwrap()
            else {
                panic!("a resumed session hands back a fresh token first");
            };
            (socket, token)
        });
        // The client reconnects first; the browser returns after the session opened.
        eventually(|| !served.intake.workspaces.summaries().is_empty());
        thread::sleep(Duration::from_millis(200));
        let _browser = attach_returning_browser(&served);
        let (_socket, token) = resumed.join().unwrap();
        assert_eq!(
            ResumeState::decode(&token).unwrap().owned_tabs,
            vec![returning_tab()]
        );
        assert_eq!(
            bound_tabs(&served),
            vec![("tab_kept".to_owned(), RETURNING_TAB)]
        );
    }

    #[test]
    fn a_browser_that_returns_late_keeps_and_then_binds_its_tabs() {
        let home = TestHome::new("resume-late");
        let served = serve_relayed(&home);
        let mut socket = open_mcp(Some(returning_token()));
        let McpFrame::Resume { token } =
            frame::read_frame::<McpFrame>(&mut socket).unwrap().unwrap()
        else {
            panic!("a resumed session hands back a fresh token first");
        };
        // The browser did not return in time: nothing is bound, and nothing is forgotten.
        assert_eq!(
            ResumeState::decode(&token).unwrap().owned_tabs,
            vec![returning_tab()]
        );
        assert!(bound_tabs(&served).is_empty());

        let _browser = attach_returning_browser(&served);
        eventually(|| bound_tabs(&served) == vec![("tab_kept".to_owned(), RETURNING_TAB)]);
    }

    #[test]
    fn readiness_is_read_only_and_opens_no_session() {
        let home = TestHome::new("readiness");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let before = host.workbench.snapshot();

        let observed = cli_client::readiness().unwrap();
        let after = host.workbench.snapshot();

        assert_eq!(observed, before.readiness);
        assert_eq!(after.readiness, before.readiness);
        assert!(after.sessions.is_empty());
        assert_eq!(after.history, before.history);
        drop(host);
    }

    #[test]
    fn starting_clears_the_images_installs_moved_aside() {
        let home = TestHome::new("old-images");
        let executable = std::env::current_exe().unwrap();
        let old = executable.with_file_name(format!(
            "ghostlight-start-test-{}.old-1",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&old, b"image").unwrap();
        let host = ServiceHost::start(home.path()).unwrap();
        assert!(!old.exists(), "the numbered old image was left behind");
        drop(host);
        let _ = std::fs::remove_file(&old);
    }

    #[test]
    fn readiness_never_starts_an_absent_service() {
        let home = TestHome::new("readiness-absent");
        assert!(cli_client::readiness().is_err());
        assert!(!home.path().join(RUNTIME_FILE_NAME).exists());
    }

    /// Open one MCP session, end the connection the given way, and report the sessions left.
    fn sessions_after(name: &str, goodbye: impl FnOnce(&mut TcpStream)) -> usize {
        let home = TestHome::new(name);
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut socket = open_mcp(None);
        initialize(&mut socket);
        assert_eq!(
            host.workbench.snapshot().sessions.len(),
            1,
            "the probe session must exist before the connection ends"
        );

        goodbye(&mut socket);
        drop(socket);

        // The session teardown runs on the connection's own thread, so give it a moment.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !host.workbench.snapshot().sessions.is_empty() {
            thread::sleep(Duration::from_millis(25));
        }
        host.workbench.snapshot().sessions.len()
    }

    #[test]
    fn a_client_that_goes_away_badly_still_releases_its_session() {
        // The negative control: a tidy goodbye has always worked, so if this were the only case
        // the assertion below would prove nothing about the error path.
        assert_eq!(sessions_after("goodbye-clean", |_| {}), 0);

        // One frame that is not JSON ends the connection through the error path. The workspace
        // and every tab it held must still be released: an unowned workspace has no owning
        // process for the reaper.
        assert_eq!(
            sessions_after("goodbye-malformed", |socket| {
                frame::write_frame_raw(socket, b"{ not json at all").unwrap();
            }),
            0,
            "a session must not outlive the connection that opened it"
        );
    }

    #[test]
    fn prepared_service_keeps_discovery_private_until_desktop_ready() {
        let home = TestHome::new("desktop-readiness");
        let runtime = home.path().join(RUNTIME_FILE_NAME);
        let host = ServiceHost::start(home.path()).unwrap();
        assert!(!runtime.exists());
        assert!(cli_client::readiness().is_err());
        let contender = ServiceHost::start(home.path()).err().unwrap();
        assert!(contender.is::<super::AuthorityAlreadyRunning>());
        host.publish_ready().unwrap();
        let published = RuntimeFile::read(&runtime).unwrap().unwrap();
        assert_eq!(published.token, host.token);
        assert_eq!(published.port, host.port);
        assert_eq!(published.relay, ghostlight_relay::RELAY);
        assert!(published.token.starts_with("runtime_"));
        let templates = FailureTemplates::read(&home.path().join(TEMPLATES_FILE_NAME))
            .unwrap()
            .expect("failure templates are published with discovery");
        assert_eq!(templates.relay, ghostlight_relay::RELAY);
        assert!(cli_client::readiness().is_ok());
        drop(host);
        assert!(!runtime.exists());
        let unpublished = ServiceHost::start(home.path()).unwrap();
        drop(unpublished);
        assert!(
            !runtime.exists(),
            "failed desktop setup leaves no discovery behind"
        );
    }

    #[test]
    fn service_lease_prevents_concurrent_authorities() {
        let (directory, _) = runtime_path("singleton");
        let first = ServiceHost::start(&directory).unwrap();
        let second = ServiceHost::start(&directory)
            .err()
            .expect("second authority is rejected");
        let detail = format!("{second:#}");
        assert!(
            detail.contains("another Ghostlight orchestrator already owns this runtime"),
            "{detail}"
        );
        drop(first);
        let replacement = ServiceHost::start(&directory).unwrap();
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
        let home = TestHome::new("activation");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        assert!(!cli_client::activate_workbench().unwrap());
        let reveals = Arc::new(RevealCounter::default());
        host.workbench.attach_presentation(reveals.clone());
        assert!(cli_client::activate_workbench().unwrap());
        assert_eq!(reveals.0.load(Ordering::SeqCst), 1);
        drop(host);
    }

    #[test]
    fn a_browser_stream_carries_an_extension_to_the_browser_port() {
        use crate::browser::adapter::chromium::protocol4::fake::FakeExtension;
        use crate::browser::adapter::chromium::protocol4::fake::Reply as FakeReply;

        let home = TestHome::new("browser-stream");
        let served = serve_fake(&home);
        // Stand in for the browser connector: carry the extension's bytes over a browser stream.
        let native = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = native.local_addr().unwrap();
        let carrier = thread::spawn(move || {
            let (extension, _) = native.accept().unwrap();
            let (service, reply) = client::connect(Stream::Browser, REPLY_DEADLINE).unwrap();
            assert!(matches!(reply, Reply::Accepted { .. }), "{reply:?}");
            let mut to_service = (extension.try_clone().unwrap(), service.try_clone().unwrap());
            let mut to_extension = (service, extension);
            thread::spawn(move || {
                let _ = std::io::copy(&mut to_extension.0, &mut to_extension.1);
            });
            let _ = std::io::copy(&mut to_service.0, &mut to_service.1);
        });
        let fake = FakeExtension::connect(
            address,
            &FakeExtension::hello("browser_relayed"),
            Box::new(|correlation, _| {
                FakeReply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))
            }),
        );
        let accepted = fake.wait(|seen| !seen.frames.is_empty());
        assert_eq!(accepted.frames[0]["kind"], "hello_accepted");
        drop(accepted);
        eventually(|| {
            served
                .relay
                .connected_browsers()
                .iter()
                .any(|browser| browser.id == "browser_relayed")
        });
        fake.close();
        drop(fake);
        let _ = carrier.join();
    }

    #[test]
    fn released_tabs_clear_diagnostics_in_bounded_batches_before_close_attempts() {
        let browser = FakeBrowser::default();
        let released = ReleasedTabs {
            browser: Some(FAKE_BROWSER.into()),
            physical_ids: (1..=257).collect(),
        };

        cleanup_released_tabs(
            "workspace-1",
            &released,
            &browser,
            &crate::workspace::WorkspaceStore::default(),
        );

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

    /// A host that signals through a channel instead of a desktop.
    fn quittable_host(name: &str) -> (TestHome, ServiceHost, std::sync::mpsc::Receiver<()>) {
        let home = TestHome::new(name);
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let (quit, quit_requested) = std::sync::mpsc::channel();
        host.install_quit_signal(Arc::new(move || {
            let _ = quit.send(());
        }));
        (home, host, quit_requested)
    }

    #[test]
    fn a_quit_answers_new_invocations_with_shutting_down() {
        use crate::language::intake::{SHUTTING_DOWN_CODE, SHUTTING_DOWN_MESSAGE};

        let (_home, host, quit_requested) = quittable_host("quit-refuses");
        let mut socket = open_mcp(None);
        initialize(&mut socket);

        host.begin_quit(Duration::from_secs(5));
        quit_requested
            .recv_timeout(Duration::from_secs(5))
            .expect("an idle service asks the desktop to exit at once");

        send_line(
            &mut socket,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {"name": "browser_tabs", "arguments": {"action": "list"}}
            }),
        );
        let (refused, _) = response(&mut socket, 2);
        assert_eq!(
            refused["error"]["message"],
            format!("{SHUTTING_DOWN_CODE}: {SHUTTING_DOWN_MESSAGE}")
        );

        // A quitting service takes no new connection either.
        let (_, reply) = client::connect(
            Stream::Control {
                op: ControlOp::Readiness,
            },
            REPLY_DEADLINE,
        )
        .unwrap();
        assert!(
            matches!(
                reply,
                Reply::Refused {
                    code: RefusalCode::ShuttingDown,
                    ..
                }
            ),
            "{reply:?}"
        );
        drop(socket);
        drop(host);
    }

    #[test]
    fn a_quit_over_control_answers_and_drains() {
        let home = TestHome::new("quit-control");
        let served = serve_fake(&home);
        let running = served.intake.quit.drain.admit().unwrap();

        let answer = cli_client::control(ControlOp::Quit).unwrap();
        assert_eq!(answer, json!({"quitting": true}));
        assert!(
            served
                .quit_requested
                .recv_timeout(Duration::from_millis(300))
                .is_err(),
            "the desktop is not asked to exit while work is in flight"
        );
        drop(running);
        served
            .quit_requested
            .recv_timeout(Duration::from_secs(5))
            .expect("the desktop is asked to exit once the work finishes");
    }

    #[test]
    fn a_quit_waits_for_work_in_flight_and_then_asks_the_desktop_to_exit() {
        let (_home, host, quit_requested) = quittable_host("quit-drains");
        // The fake executor: an invocation the service has admitted and not yet finished.
        let running = host.quit.drain.admit().unwrap();

        host.begin_quit(Duration::from_secs(30));
        assert!(
            quit_requested
                .recv_timeout(Duration::from_millis(300))
                .is_err(),
            "the desktop is not asked to exit while work is in flight"
        );
        // A second quit while one is under way changes nothing.
        host.begin_quit(Duration::ZERO);
        assert!(quit_requested
            .recv_timeout(Duration::from_millis(100))
            .is_err());

        drop(running);
        quit_requested
            .recv_timeout(Duration::from_secs(5))
            .expect("the desktop is asked to exit once the work finishes");
        drop(host);
    }

    #[test]
    fn a_quit_stops_waiting_for_work_after_the_grace() {
        let (_home, host, quit_requested) = quittable_host("quit-bound");
        let _stuck = host.quit.drain.admit().unwrap();

        let started = Instant::now();
        host.begin_quit(Duration::from_millis(200));
        quit_requested
            .recv_timeout(Duration::from_secs(5))
            .expect("the desktop is asked to exit when the grace ends");
        assert!(started.elapsed() >= Duration::from_millis(200));
        drop(host);
    }

    #[test]
    fn a_start_that_never_publishes_discovery_stops_by_itself() {
        let (directory, _) = runtime_path("discovery-stalls");
        let host = ServiceHost::start(&directory).unwrap();
        let (expired, fired) = std::sync::mpsc::channel();
        host.expect_discovery_by(Instant::now() + Duration::from_millis(100), move || {
            let _ = expired.send(());
        })
        .unwrap();
        assert!(fired.recv_timeout(Duration::from_secs(5)).is_ok());
        drop(host);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn publishing_discovery_cancels_the_stop() {
        let (directory, _) = runtime_path("discovery-published");
        let host = ServiceHost::start(&directory).unwrap();
        let (expired, fired) = std::sync::mpsc::channel();
        host.expect_discovery_by(Instant::now() + Duration::from_millis(200), move || {
            let _ = expired.send(());
        })
        .unwrap();
        host.publish_ready().unwrap();
        assert!(fired.recv_timeout(Duration::from_millis(600)).is_err());
        drop(host);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
