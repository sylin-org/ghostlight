//! The persistent service: its lifetime, its one relay listener and the sessions it serves.
//!
//! [`ServiceHost`] holds the service lock, prepares the executor and the browser port, and binds
//! the relay protocol 4 endpoint once the desktop is ready (ADR-0166, ADR-0199 Decisions 5 and
//! 9). The workbench's loopback server starts first, on one port per user, and carries no relay
//! peer. Every connection then opens with one relay opening: an `mcp` stream is one MCP client
//! session served by [`crate::intake::mcp::McpSession`], a `browser` stream is one adapter
//! connection, and a `control` stream answers one operation.
//!
//! A start carries the reason it was made for (ADR-0199 Decision 10). The reason decides what a
//! held lock and the quit latch mean to it ([`lifetime`], [`latch`]). A person's Quit sets the
//! latch, and every quit says goodbye on each live stream before the service exits ([`Quit`]). The
//! end of the session is the fourth goodbye: the service says `session_end` on every stream and
//! leaves without the latch and without waiting for work in flight (ADR-0199 Decision 13).

mod admission;
pub mod latch;
pub mod lifetime;
pub mod lock;
#[cfg(test)]
pub(crate) mod test_home;

use std::collections::BTreeMap;
use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread::{self, JoinHandle, Thread};
use std::time::{Duration, Instant};

use crate::intake::session::{
    IntakeChannel, ServiceRequest, ServiceResponse, SessionMarker, ToolDefinition,
};
use anyhow::{Context, Result};
use ghostlight_relay::endpoint::{Connection, Endpoint, Listener};
use ghostlight_relay::protocol::{
    self, Control, GoodbyeReason, McpFrame, QuitReason, Reply, Role, StartReason, Templates,
};
use serde_json::Value;
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
use crate::governance::{AuthoritySnapshot, GovernanceFacade, JsonlAuditSink};
use crate::install;
use crate::intake::listener::{self, control, Judgement};
use crate::intake::mcp::{failure_templates, McpSession, OwnedTab, ResumeState, Step};
use crate::intake::MCP_CHANNEL;
use crate::language::catalog_for;
use crate::language::environment::{ELEVATED_SERVICE, QUIT_HELD};
use crate::language::intake::{OpeningRefusal, SHUTTING_DOWN_CODE, SHUTTING_DOWN_MESSAGE};
use crate::language::outcome::DialogKind;
use crate::presentation::{BrowserPresentation, PresentationReactor};
use crate::provenance::ConnectionEvidence;
use crate::work::{ApplicationExecutor, CancellationToken, PreparedInvocation};
use crate::workbench::web::{PendingWorkbench, WorkbenchAddress, WorkbenchServer};
use crate::workbench::{RuntimeSource, Watching, WorkbenchFacade, WorkbenchProjection};
use crate::workspace::{ReleasedTabs, WorkspaceId, WorkspaceStore};

const DIAGNOSTIC_CLEAR_BATCH_SIZE: usize = 256;
const QUEUED_METADATA_BYTES: usize = 512;

/// Prefix of the value that changes on every service start.
const INSTANCE_PREFIX: &str = "instance_";

/// Environment variable naming an audit log in place of the one in the state directory.
const AUDIT_FILE_VARIABLE: &str = "GHOSTLIGHT_AUDIT_FILE";

/// File name of the audit log in the state directory.
const AUDIT_FILE_NAME: &str = "audit.jsonl";

/// Longest a resumed session waits for one browser to return, and then for it to list its tabs.
const RESTORE_DEADLINE: Duration = Duration::from_secs(5);

/// How long the accept loop rests after a failed accept before it asks again.
const ACCEPT_RETRY: Duration = Duration::from_millis(50);

use admission::{Capacity, Job, QueueBudget, Refused, SessionQueue};
use lifetime::{Drain, ReadyWatch, QUIT_GRACE, READY_LIMIT};
use lock::ServiceLock;

/// What the runtime-published diagnostics line says: where the relay and the workbench listen.
///
/// It names the workbench by address only. The bearer is not an input, so it cannot appear.
fn published_detail(endpoint: Option<&str>, workbench: Option<&WorkbenchAddress>) -> String {
    format!(
        "desktop_ready=true endpoint={} relay={} workbench={}",
        endpoint.unwrap_or_default(),
        ghostlight_relay::RELAY,
        workbench.map_or("", WorkbenchAddress::url)
    )
}

/// The desktop's request to leave its event loop, installed once the desktop exists.
pub type QuitSignal = Arc<dyn Fn() + Send + Sync>;

/// How often a start that waits for the service lock asks for it again.
const LOCK_POLL: Duration = Duration::from_millis(50);

/// What a start asks of the service lock: who started it and how long it waits for a held lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartTerms {
    /// Why the service is being started.
    pub reason: StartReason,
    /// How long to keep asking for the lock while another process holds it.
    pub lock_wait: Duration,
}

impl StartTerms {
    /// The terms of a start for `reason`: an update waits for the previous service to leave, any
    /// other start asks once.
    #[must_use]
    pub fn for_reason(reason: StartReason) -> Self {
        Self {
            reason,
            lock_wait: lifetime::lock_wait(reason),
        }
    }
}

/// A running local service host. Dropping it stops the listener and releases the service lock.
pub struct ServiceHost {
    /// Typed in-process application boundary for the disposable desktop workbench.
    pub workbench: WorkbenchFacade,
    /// The process-diagnostics hub this authority owns.
    pub diagnostics: Arc<DiagnosticsHub>,
    browser: Arc<RelayBrowserPort>,
    audit: Arc<AuditRecorder>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    refresher: Thread,
    home: PathBuf,
    intake: Arc<Intake>,
    bound: Mutex<Option<Bound>>,
    quit: Quit,
    ready: Arc<ReadyWatch>,
    lock_taken: Instant,
    _lock: ServiceLock,
}

/// The endpoint once it is bound, and the thread that accepts on it.
struct Bound {
    listener: Arc<Listener>,
    thread: JoinHandle<()>,
}

/// Another authority owns this runtime; a competing explicit Open may await its presentation.
#[derive(Debug, thiserror::Error)]
#[error("another Ghostlight orchestrator already owns this runtime")]
pub struct AuthorityAlreadyRunning;

/// The person quit Ghostlight, and this start is one that Quit holds back.
#[derive(Debug, thiserror::Error)]
#[error("{}", QUIT_HELD)]
pub struct QuitLatched;

impl ServiceHost {
    /// Prepare the service in the Ghostlight home `home` under its lifetime lock, without
    /// binding the endpoint and without the elevation check a production start makes.
    #[cfg(test)]
    pub(crate) fn start(home: &Path) -> Result<Self> {
        Self::start_with_token(
            home,
            StartTerms {
                reason: StartReason::Agent,
                lock_wait: Duration::ZERO,
            },
            || Ok(false),
        )
    }

    /// Prepare the service in the Ghostlight home `home` for a start made for `reason`, under its
    /// lifetime lock, without binding the endpoint. The desktop must call `publish_ready` only
    /// after its native interaction route is ready.
    ///
    /// In order: a process whose token is elevated is refused; a browser or recover start is
    /// refused while the quit latch is set ([`QuitLatched`]); the lock is taken, and a start that
    /// finds it held gives way ([`AuthorityAlreadyRunning`]) except an update, which asks again
    /// for [`lifetime::UPDATE_LOCK_WAIT`] so the service it replaces can finish leaving; and only
    /// then does any other start clear the latch and open state.
    pub fn start_waiting(home: &Path, reason: StartReason) -> Result<Self> {
        Self::start_with_token(
            home,
            StartTerms::for_reason(reason),
            ghostlight_win_peer::is_elevated,
        )
    }

    /// Like [`ServiceHost::start_waiting`], with the terms spelled out and the answer to "is this
    /// process elevated" given by `elevated`.
    pub(crate) fn start_with_token(
        home: &Path,
        terms: StartTerms,
        elevated: impl FnOnce() -> io::Result<bool>,
    ) -> Result<Self> {
        let diagnostics = DiagnosticsHub::birth(home);
        Self::start_with_diagnostics(home, Arc::clone(&diagnostics), terms, elevated).inspect_err(
            |error| {
                // Meeting the running service is how a racing start ends, not a failure, and a
                // start Quit holds back is a decision, not a fault.
                if error.is::<QuitLatched>() {
                    diagnostics.sink().emit(
                        ghostlight_bridge::diagnostics::event::PROCESS_EXITED,
                        ghostlight_bridge::diagnostics::Level::Info,
                        None,
                        &format!("{}: {QUIT_HELD}", terms.reason.as_str()),
                    );
                } else if !error.is::<AuthorityAlreadyRunning>() {
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
        terms: StartTerms,
        elevated: impl FnOnce() -> io::Result<bool>,
    ) -> Result<Self> {
        refuse_elevated_token(elevated)?;
        let layout = install::layout::Layout::new(home);
        if lifetime::refused_by_quit_latch(terms.reason) && latch::is_set(&layout.quit_latch()) {
            return Err(QuitLatched.into());
        }
        let lock = acquire_lock(&layout.service_lock(), terms.lock_wait)?;
        let lock_taken = Instant::now();
        if !lifetime::refused_by_quit_latch(terms.reason) {
            latch::clear(&layout.quit_latch()).context("clear the quit latch")?;
        }
        if let Ok(executable) = env::current_exe() {
            remove_old_images_beside(&executable);
            register_missing_browsers(home, &executable);
        }

        let workspaces = WorkspaceStore::default();
        let governance = GovernanceFacade::from_environment();
        let service_epoch = format!("service_{}", Uuid::new_v4().simple());
        let browser = Arc::new(RelayBrowserPort::new(service_epoch, page_assets()));
        let browser_port: Arc<dyn BrowserPort> = browser.clone();
        browser
            .set_lifecycle_observer(Arc::clone(&diagnostics) as Arc<dyn AdapterLifecycleObserver>);
        governance.relay_runtime_state(browser_port.clone());
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
            workbench: workbench.clone(),
            workspaces: workspaces.clone(),
        }));

        let stop = Arc::new(AtomicBool::new(false));
        let quit = Quit::new(
            Arc::clone(&diagnostics),
            layout.quit_latch(),
            Arc::clone(&browser),
        );
        let refresher = spawn_refresher(governance.clone(), workbench.clone(), Arc::clone(&stop))?;
        let refresher_thread = refresher.thread().clone();
        let audit_stop = stop.clone();
        let audit_for_thread = Arc::clone(&audit);
        let audit_thread = std::thread::spawn(move || {
            while !audit_stop.load(Ordering::SeqCst) {
                audit_for_thread.wait_for_recovery(&audit_stop);
            }
        });
        let intake = Arc::new(Intake {
            instance: format!("{INSTANCE_PREFIX}{}", Uuid::new_v4().simple()),
            started_by: terms.reason,
            templates: failure_templates(),
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
            workbench_server: OnceLock::new(),
        });
        Ok(Self {
            workbench,
            diagnostics,
            browser,
            audit,
            stop,
            threads: vec![audit_thread, refresher],
            refresher: refresher_thread,
            home: home.into(),
            intake,
            bound: Mutex::new(None),
            quit,
            ready: Arc::default(),
            lock_taken,
            _lock: lock,
        })
    }

    /// Start the workbench's loopback server, then bind the relay endpoint after the desktop event
    /// loop has established an interaction route, and start accepting on it.
    ///
    /// The server binds first and agents are barred from its address before it answers, so no
    /// agent request can arrive before the rule holds. The relay has no port, token or discovery
    /// file; the workbench's one loopback port carries no relay peer. Until this runs nothing can
    /// connect. When the workbench cannot bind at all, the service serves on without it. A second
    /// call changes nothing.
    pub fn publish_ready(&self) -> Result<()> {
        let mut bound = lock(&self.bound);
        if bound.is_some() {
            return Ok(());
        }
        let workbench = self.start_workbench();
        // Edges that served an earlier service find this one from here on, so a call with no
        // browser waits for them from this moment, not from construction.
        self.browser.endpoint_published();
        let listener =
            Arc::new(Listener::bind_first(&self.home).context("bind the relay endpoint")?);
        let thread = spawn_listener(Arc::clone(&listener), Arc::clone(&self.intake));
        *bound = Some(Bound { listener, thread });
        self.ready.settle();
        let endpoint = Endpoint::for_home(&self.home).map(|endpoint| endpoint.to_string());
        self.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::RUNTIME_PUBLISHED,
            ghostlight_bridge::diagnostics::Level::Info,
            None,
            &published_detail(
                endpoint.as_ref().ok().map(String::as_str),
                workbench.as_ref(),
            ),
        );
        Ok(())
    }

    /// Bind the workbench's port, bar agents from it, and start answering.
    fn start_workbench(&self) -> Option<WorkbenchAddress> {
        let pending = match PendingWorkbench::bind(&self.home) {
            Ok(pending) => pending,
            Err(error) => {
                self.diagnostics.sink().emit(
                    ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
                    ghostlight_bridge::diagnostics::Level::Error,
                    None,
                    &format!("the workbench could not bind a loopback port: {error}"),
                );
                return None;
            }
        };
        self.intake
            .governance
            .protect_workbench_port(pending.address().port());
        let watching = Arc::new(Watching::load(
            install::layout::Layout::new(&self.home).watch_off_marker(),
        ));
        let server =
            Arc::new(pending.serve(&self.workbench, self.intake.instance.clone(), watching));
        let address = server.address().clone();
        let _ = self.intake.workbench_server.set(server);
        Some(address)
    }

    /// Where the workbench is served, once `publish_ready` has started its server.
    #[must_use]
    pub fn workbench_address(&self) -> Option<WorkbenchAddress> {
        self.intake
            .workbench_server
            .get()
            .map(|server| server.address().clone())
    }

    /// When this service must be accepting openings by: [`READY_LIMIT`] after it took the lock.
    #[must_use]
    pub fn ready_deadline(&self) -> Instant {
        self.lock_taken + READY_LIMIT
    }

    /// Stop this process by itself if the endpoint has not been bound by `deadline`.
    ///
    /// `publish_ready` cancels the deadline. On expiry the service records a diagnostics event and
    /// calls `on_expiry`, which ends the process. A start that stalls therefore never holds the
    /// service lock against the starts that follow it.
    pub fn expect_ready_by(
        &self,
        deadline: Instant,
        on_expiry: impl FnOnce() + Send + 'static,
    ) -> Result<()> {
        let diagnostics = Arc::clone(&self.diagnostics);
        self.ready
            .arm(deadline, move || {
                diagnostics.sink().emit(
                    ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
                    ghostlight_bridge::diagnostics::Level::Error,
                    None,
                    "the endpoint was not bound in time; the service stopped",
                );
                on_expiry();
            })
            .context("start the ready limit")
    }

    /// Install the request the desktop makes to leave its event loop with status 0.
    pub fn install_quit_signal(&self, signal: QuitSignal) {
        *lock(&self.quit.signal) = Some(signal);
    }

    /// Begin a quit for `reason`: refuse new work, let in-flight invocations finish for up to
    /// `grace`, say goodbye on every stream, then ask the desktop to exit. Returns at once; a
    /// second call while a quit is under way does nothing.
    pub fn begin_quit(&self, reason: QuitReason, grace: Duration) {
        self.quit.begin(reason, grace);
    }

    /// The handle the desktop uses for a person's Quit from the tray or the workbench.
    #[must_use]
    pub fn quit_handle(&self) -> Quit {
        self.quit.clone()
    }

    /// The process-diagnostics hub this authority owns.
    #[must_use]
    pub fn diagnostics(&self) -> Arc<DiagnosticsHub> {
        Arc::clone(&self.diagnostics)
    }

    fn join(&mut self) {
        self.ready.settle();
        lock(&self.quit.signal).take();
        self.stop.store(true, Ordering::SeqCst);
        self.audit.wake();
        self.refresher.unpark();
        if let Some(server) = self.intake.workbench_server.get() {
            server.stop();
        }
        if let Some(bound) = lock(&self.bound).take() {
            bound.listener.shutdown();
            let _ = bound.thread.join();
        }
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Drop for ServiceHost {
    fn drop(&mut self) {
        self.join();
    }
}

/// How the service ends on request: refuse new work, drain what runs, say goodbye, then leave
/// the desktop.
///
/// A person's Quit also sets the quit latch first, so a browser that restarts does not bring
/// Ghostlight back. A replacement or an uninstall leaves it alone.
#[derive(Clone)]
pub struct Quit {
    drain: Arc<Drain>,
    signal: Arc<Mutex<Option<QuitSignal>>>,
    diagnostics: Arc<DiagnosticsHub>,
    latch: PathBuf,
    farewell: Arc<Farewell>,
}

impl Quit {
    /// A quit that sets `latch` for a person's Quit and says goodbye on the MCP streams it is
    /// given and on the streams of `browser`.
    fn new(
        diagnostics: Arc<DiagnosticsHub>,
        latch: PathBuf,
        browser: Arc<RelayBrowserPort>,
    ) -> Self {
        Self {
            drain: Arc::new(Drain::default()),
            signal: Arc::default(),
            diagnostics,
            latch,
            farewell: Arc::new(Farewell::new(browser)),
        }
    }

    /// Begin a quit for `reason` unless one is under way: set the latch for a person's Quit,
    /// refuse new work, and once the work in flight has finished or `grace` has passed, say
    /// goodbye on every stream and ask the desktop to exit.
    pub fn begin(&self, reason: QuitReason, grace: Duration) {
        if !self.drain.begin_quit() {
            return;
        }
        if reason == QuitReason::Quit {
            if let Err(error) = latch::set(&self.latch) {
                self.diagnostics.sink().emit(
                    ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
                    ghostlight_bridge::diagnostics::Level::Error,
                    None,
                    &format!("the quit latch could not be written: {error}"),
                );
            }
        }
        let drain = Arc::clone(&self.drain);
        let signal = Arc::clone(&self.signal);
        let diagnostics = Arc::clone(&self.diagnostics);
        let farewell = Arc::clone(&self.farewell);
        let spawned = thread::Builder::new()
            .name("ghostlight-quit-drain".into())
            .spawn(move || {
                let drained = drain.wait_idle(grace);
                diagnostics.sink().emit(
                    ghostlight_bridge::diagnostics::event::PROCESS_EXITED,
                    ghostlight_bridge::diagnostics::Level::Info,
                    None,
                    &format!(
                        "quit: reason={} drained={drained} in_flight={}",
                        goodbye_for(reason).as_str(),
                        drain.in_flight()
                    ),
                );
                farewell.say(goodbye_for(reason));
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

    /// Leave because the session is ending: refuse new work, say `session_end` goodbye on every
    /// stream, then ask the desktop to exit. Returns at once, and neither waits for work in
    /// flight nor sets the latch, since a sign-out is not a person's Quit and the caller has
    /// already armed the fallback exit that bounds this stop.
    pub fn session_end(&self) {
        self.drain.begin_quit();
        let signal = Arc::clone(&self.signal);
        let farewell = Arc::clone(&self.farewell);
        let spawned = thread::Builder::new()
            .name("ghostlight-session-end".into())
            .spawn(move || {
                farewell.say(GoodbyeReason::SessionEnd);
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
                "the session end could not start",
            );
        }
    }

    /// Record `writer` as a live MCP stream until the returned guard is dropped, so a quit says
    /// goodbye on it.
    fn enroll(&self, writer: Arc<SocketWriter>) -> Enrolled {
        self.farewell.enroll(writer)
    }
}

/// The goodbye a quit for `reason` sends.
fn goodbye_for(reason: QuitReason) -> GoodbyeReason {
    match reason {
        QuitReason::Quit => GoodbyeReason::Quit,
        QuitReason::Replace => GoodbyeReason::Replace,
        QuitReason::Uninstall => GoodbyeReason::Uninstall,
        QuitReason::Unknown => GoodbyeReason::Unknown,
    }
}

/// The live streams a quit says goodbye on before the service closes them.
struct Farewell {
    mcp: Mutex<BTreeMap<u64, Arc<SocketWriter>>>,
    next: AtomicU64,
    browser: Arc<RelayBrowserPort>,
}

impl Farewell {
    fn new(browser: Arc<RelayBrowserPort>) -> Self {
        Self {
            mcp: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(0),
            browser,
        }
    }

    /// Record one MCP stream until the guard is dropped.
    fn enroll(self: &Arc<Self>, writer: Arc<SocketWriter>) -> Enrolled {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.mcp).insert(id, writer);
        Enrolled {
            farewell: Arc::clone(self),
            id,
        }
    }

    /// Say goodbye with `reason` on every MCP stream and every browser stream.
    fn say(&self, reason: GoodbyeReason) {
        let writers: Vec<Arc<SocketWriter>> = lock(&self.mcp).values().cloned().collect();
        for writer in writers {
            let _ = writer.native(&McpFrame::Goodbye { reason });
        }
        self.browser.say_goodbye(reason);
    }
}

/// One MCP stream recorded for the goodbye. Dropping it forgets the stream.
struct Enrolled {
    farewell: Arc<Farewell>,
    id: u64,
}

impl Drop for Enrolled {
    fn drop(&mut self) {
        lock(&self.farewell.mcp).remove(&self.id);
    }
}

/// Everything a relay connection needs from the service.
struct Intake {
    /// The value that changes on every start of this service.
    instance: String,
    /// Why this service was started.
    started_by: StartReason,
    /// The failure sentences every accepted `mcp` stream is handed.
    templates: Templates,
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
    /// The workbench's server, once `publish_ready` has started it. The desktop role reads its
    /// address and bearer from here.
    workbench_server: OnceLock<Arc<WorkbenchServer>>,
}

impl Intake {
    /// The reply that accepts an opening for `role`. Only an `mcp` stream is handed the failure
    /// templates.
    fn accepted(&self, role: Role) -> Reply {
        Reply::accepted(
            env!("CARGO_PKG_VERSION").into(),
            self.instance.clone(),
            (role == Role::Mcp).then(|| self.templates.clone()),
        )
    }
}

/// Accept relay connections until the service stops, each on its own thread.
///
/// A connection holds one of a few pre-judgement slots until its opening is judged, so local
/// processes that connect and say nothing cannot exhaust the service.
fn spawn_listener(listener: Arc<Listener>, intake: Arc<Intake>) -> JoinHandle<()> {
    let handshakes = Capacity::new(admission::HANDSHAKES);
    thread::Builder::new()
        .name("ghostlight-relay-listener".into())
        .spawn(move || {
            while !intake.stop.load(Ordering::SeqCst) {
                let connection = match listener.accept() {
                    Ok(connection) => connection,
                    Err(_) if intake.stop.load(Ordering::SeqCst) => break,
                    Err(_) => {
                        thread::sleep(ACCEPT_RETRY);
                        continue;
                    }
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
                        let judgement =
                            listener::judge(&connection, intake.quit.drain.is_quitting());
                        drop(handshake);
                        serve_connection(connection, judgement, &intake);
                    });
            }
        })
        .expect("relay listener thread starts")
}

/// Serve one judged connection to its end.
fn serve_connection(connection: Connection, judgement: Judgement, intake: &Arc<Intake>) {
    match judgement {
        Judgement::Drop => {}
        Judgement::Refuse(refusal) => listener::refuse(&connection, refusal),
        Judgement::Admit(opening) => match opening.role {
            Role::Browser => {
                if listener::accept(&connection, &intake.accepted(Role::Browser)).is_ok() {
                    // The first frame on the stream is the extension's own hello.
                    let _ = intake.browser.attach(connection);
                    intake.workbench.refresh_live_parts();
                }
            }
            Role::Control => serve_control(connection, opening.control, intake),
            Role::Mcp => {
                if serve_mcp(connection, intake, &opening).is_err() {
                    eprintln!("MCP session ended after an invalid or interrupted exchange.");
                }
            }
            Role::Desktop | Role::Command | Role::Unknown => {
                listener::refuse(&connection, OpeningRefusal::Unsupported);
            }
        },
    }
}

/// Answer one control operation and close.
fn serve_control(connection: Connection, operation: Option<Control>, intake: &Intake) {
    let value = match operation {
        Some(Control::Readiness) => {
            serde_json::to_value(intake.workbench.snapshot().readiness).unwrap_or(Value::Null)
        }
        Some(Control::Status) => serde_json::to_value(control::Status {
            service: env!("CARGO_PKG_VERSION").into(),
            instance: intake.instance.clone(),
            pid: std::process::id(),
            started_by: intake.started_by,
        })
        .unwrap_or(Value::Null),
        Some(Control::Show) => serde_json::to_value(control::Shown {
            shown: intake.workbench.reveal().is_ok(),
        })
        .unwrap_or(Value::Null),
        Some(Control::Quit { reason }) => {
            intake.quit.begin(reason, QUIT_GRACE);
            serde_json::to_value(control::Quitting { quitting: true }).unwrap_or(Value::Null)
        }
        Some(Control::Unknown) | None => {
            listener::refuse(&connection, OpeningRefusal::Unsupported);
            return;
        }
    };
    if listener::accept(&connection, &intake.accepted(Role::Control)).is_ok() {
        let _ = listener::answer_control(&connection, &value);
    }
}

/// Serve one MCP client session on an accepted `mcp` stream.
///
/// The opening names the channel and, for the command line, the process that owns the session.
/// A resume token continues an earlier session: its revision, client and owned tabs come back,
/// and tabs the browser no longer has stay stale. Tabs of a browser that has not returned yet
/// stay in the token and are bound when it does.
fn serve_mcp(
    connection: Connection,
    intake: &Arc<Intake>,
    requested: &protocol::Opening,
) -> Result<()> {
    let Some(_session_permit) = intake.sessions.acquire(1) else {
        listener::refuse(&connection, OpeningRefusal::Capacity);
        return Ok(());
    };
    let marker = match requested
        .session
        .clone()
        .map(serde_json::from_value::<SessionMarker>)
    {
        None => None,
        Some(Ok(marker)) => Some(marker),
        Some(Err(_)) => {
            listener::refuse(&connection, OpeningRefusal::Malformed);
            return Ok(());
        }
    };
    let channel = requested.channel.as_deref().unwrap_or(MCP_CHANNEL);
    let intake_channel = crate::intake::intake_channel(channel);
    let resumed = requested
        .resume
        .as_deref()
        .and_then(|token| ResumeState::decode(token).ok())
        .map(|mut state| {
            if state.channel.is_empty() {
                state.channel = channel.into();
            }
            state
        });
    listener::accept(&connection, &intake.accepted(Role::Mcp))?;
    let mut reader =
        SocketReader::new(connection.try_clone()?).interrupted_by(Arc::clone(&intake.stop));
    let writer = Arc::new(SocketWriter::new(connection)?);
    let _enrolled = intake.quit.enroll(Arc::clone(&writer));
    let opening = Opening {
        channel: intake_channel,
        marker,
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

/// How often the refresher checks the policy files and the managed authority.
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

/// Start the one thread that keeps published authority current until `stop` is set.
///
/// It is the only thread that reads policy files on a schedule or fetches the organization's
/// policy, so no tool call ever waits on either. After each check it tells the workbench, so
/// every screen hears of a changed policy or a changed saving state within a second.
fn spawn_refresher(
    governance: GovernanceFacade,
    workbench: WorkbenchFacade,
    stop: Arc<AtomicBool>,
) -> Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("ghostlight-policy-refresher".into())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                governance.refresh_tick();
                workbench.authority_refreshed();
                thread::park_timeout(REFRESH_INTERVAL);
            }
        })
        .context("start the policy refresher")
}

/// The tool catalog one session's client was last given, with the authority it was built from.
#[derive(Default)]
struct AdvertisedCatalog(Mutex<Option<Advertised>>);

struct Advertised {
    authority: String,
    tools: Vec<ToolDefinition>,
}

impl AdvertisedCatalog {
    /// Build the catalog `authority` admits and remember it as what the client now has.
    fn advertise(&self, authority: &AuthoritySnapshot) -> Vec<ToolDefinition> {
        let tools = catalog_for(authority);
        *lock(&self.0) = Some(Advertised {
            authority: authority.id().to_owned(),
            tools: tools.clone(),
        });
        tools
    }

    /// The catalog `authority` admits when it differs from the one given, and `None` otherwise.
    ///
    /// Nothing is compared until the client has asked for a catalog, and the catalog is rebuilt
    /// only when the authority itself changed.
    fn changed_by(&self, authority: &AuthoritySnapshot) -> Option<Vec<ToolDefinition>> {
        let mut guard = lock(&self.0);
        let advertised = guard.as_mut()?;
        if advertised.authority == authority.id() {
            return None;
        }
        advertised.authority = authority.id().to_owned();
        let tools = catalog_for(authority);
        if tools == advertised.tools {
            return None;
        }
        advertised.tools.clone_from(&tools);
        Some(tools)
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
    advertised: Arc<AdvertisedCatalog>,
}

impl LiveSession {
    /// Open the session's workspace and start its workers and its catalog watch.
    fn open(intake: &Arc<Intake>, wire: &Arc<McpWire>, opening: &Opening) -> Result<Self> {
        // Before opening anything, release workspaces whose owner is gone and close the tabs they
        // still hold. Sweeping here rather than on a timer keeps the cost proportional to use.
        reap_finished_sessions(&intake.workspaces, intake.browser_port.as_ref());
        // Evidence belongs to this connection. Workspace continuity never replaces it.
        let connection = Arc::new(ConnectionEvidence::new(
            &wire.client_label(),
            opening.channel,
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
        let advertised = Arc::new(AdvertisedCatalog::default());
        let watch_stop = Arc::new(AtomicBool::new(false));
        let mut live = Self {
            workspace,
            connection,
            queue,
            workers: Vec::new(),
            watch: None,
            watch_stop,
            advertised,
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
        intake.workbench.refresh_live_parts();
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
        let advertised = Arc::clone(&self.advertised);
        let stop = Arc::clone(&self.watch_stop);
        thread::Builder::new()
            .name("ghostlight-session-watch".into())
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
                    if let Some(tools) = advertised.changed_by(&governance.snapshot()) {
                        generation = generation.wrapping_add(1);
                        wire.render(ServiceResponse::CatalogChanged { generation, tools });
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
                let tools = self.advertised.advertise(&intake.governance.snapshot());
                wire.render(ServiceResponse::Catalog { tools });
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
        intake.workbench.refresh_live_parts();
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

/// Take the service lock at `path`, asking again until `wait` has passed while another process
/// holds it.
fn acquire_lock(path: &Path, wait: Duration) -> Result<ServiceLock> {
    let give_up = Instant::now() + wait;
    loop {
        if let Some(lock) = ServiceLock::try_acquire(path).context("open the service lock")? {
            return Ok(lock);
        }
        let remaining = give_up.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(AuthorityAlreadyRunning.into());
        }
        thread::sleep(LOCK_POLL.min(remaining));
    }
}

/// Refuse to serve, or to start a service, from an elevated token. `elevated` reads the token of
/// the calling process; a token that cannot be read is refused too.
pub(crate) fn refuse_elevated_token(elevated: impl FnOnce() -> io::Result<bool>) -> Result<()> {
    if elevated().context("read the process token")? {
        anyhow::bail!(ELEVATED_SERVICE);
    }
    Ok(())
}

/// Remove the images earlier installs moved aside in the directory `executable` runs from. The
/// lock is held, so no other service of this home is starting.
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

/// Applies what a connected browser reports: its tabs, its dialogs and its toolbar's pause, and
/// tells the workbench when who is connected may have changed.
struct ServiceBrowserEvents {
    workbench: WorkbenchFacade,
    workspaces: WorkspaceStore,
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
                self.workbench
                    .apply_runtime_control(intent, RuntimeSource::Browser);
            }
            BrowserEvent::TabClosed { tab_id } => {
                self.workspaces.apply_browser_close(browser, tab_id);
                self.workbench.refresh_live_parts();
            }
            // A browser that comes back may have restarted and reused its tab ids, so nothing it
            // left behind stays reusable.
            BrowserEvent::Disconnected => {
                self.workspaces.apply_browser_disconnect(browser);
                self.workbench.refresh_live_parts();
            }
            // Attention never reaches this sink: it is browser routing state, and the port that
            // knows which connection reported it is the only thing that needs it.
            BrowserEvent::Attended => {}
            BrowserEvent::DialogChanged {
                tab_id,
                present,
                dialog_type,
            } => self.workspaces.apply_browser_dialog(
                browser,
                tab_id,
                present.then(|| DialogKind::classify(&dialog_type)),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::intake::session::IntakeChannel;
    use ghostlight_relay::endpoint::{pair, Connection, Endpoint, Listener};
    use ghostlight_relay::frame;
    use ghostlight_relay::protocol::{
        self, Control, Edge, GoodbyeReason, McpFrame, Opening as RelayOpening, Program, QuitReason,
        RefusalCode, Reply, Role, StartReason,
    };
    use serde_json::{json, Value};

    use super::test_home::TestHome;
    use crate::browser::adapter::BrowserCommand;
    use crate::browser::testing::{FakeBrowser, FAKE_BROWSER};
    use crate::cli::client as cli_client;
    use crate::intake::listener::{self, control};
    use crate::intake::mcp::failure_templates;
    use crate::intake::mcp::ResumeState;
    use crate::language::intake::OpeningRefusal;
    use crate::workbench::{
        WorkbenchNotification, WorkbenchPresentationError, WorkbenchPresentationPort,
    };
    use crate::workspace::ReleasedTabs;

    use super::{
        cleanup_released_tabs, AuthorityAlreadyRunning, QuitLatched, ServiceHost, StartTerms,
    };

    /// The terms of an agent start, which asks for the lock once.
    fn agent() -> StartTerms {
        StartTerms::for_reason(StartReason::Agent)
    }

    /// A fresh directory to serve from, which the test removes.
    fn temp_home(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "ghostlight-service-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
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

        let directory = temp_home("passive-browsing");
        let policy = directory.join("policy.json");
        std::fs::write(&policy, r#"{"schema":4,"name":"test","version":"1","grants":[{"id":"read","hosts":{"allow":["allowed.example"]},"allowed":["read"]}]}"#).unwrap();
        let governance = GovernanceFacade::new(Some(policy), None);
        let workspaces = WorkspaceStore::default();
        let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp);
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
        let sink = super::ServiceBrowserEvents {
            workbench: view.clone(),
            workspaces: workspaces.clone(),
        };

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

        let workspace = workspaces.admit(RELEASED_CLIENT.into(), IntakeChannel::Mcp);
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
    fn the_browser_reporting_a_dialog_is_recorded_for_the_tab_that_holds_it() {
        use crate::browser::adapter::{BrowserEvent, BrowserReadiness, PhysicalTab};
        use crate::browser::BrowserEventSink;
        use crate::language::outcome::DialogKind;
        use crate::workspace::WorkspaceStore;

        let workspaces = WorkspaceStore::default();
        let workspace = workspaces.admit("dialogs".into(), IntakeChannel::Mcp);
        workspaces
            .pin_browser(workspace.as_str(), FAKE_BROWSER)
            .unwrap();
        let lease = workspaces.acquire(&workspace).unwrap();
        lease
            .add_tab(&PhysicalTab {
                tab_id: 4,
                url: "https://example.com/".into(),
                title: "Example".into(),
                active: true,
                readiness: BrowserReadiness::Complete,
            })
            .unwrap();
        drop(lease);
        let sink = super::ServiceBrowserEvents {
            workbench: test_workbench(&workspaces),
            workspaces: workspaces.clone(),
        };
        let dialog = |tab_id, present, dialog_type: &str| BrowserEvent::DialogChanged {
            tab_id,
            present,
            dialog_type: dialog_type.into(),
        };

        sink.on_event(FAKE_BROWSER, dialog(4, true, "confirm"));
        assert_eq!(
            workspaces.dialog_open(&workspace, 4),
            Some(DialogKind::Confirm)
        );
        // A tab this session does not control is not its dialog.
        sink.on_event(FAKE_BROWSER, dialog(9, true, "alert"));
        assert_eq!(workspaces.dialog_open(&workspace, 9), None);
        // The browser closing it, and a kind this build does not name.
        sink.on_event(FAKE_BROWSER, dialog(4, false, "unknown"));
        assert_eq!(workspaces.dialog_open(&workspace, 4), None);
        sink.on_event(FAKE_BROWSER, dialog(4, true, "a kind from the future"));
        assert_eq!(
            workspaces.dialog_open(&workspace, 4),
            Some(DialogKind::Other)
        );
    }

    #[test]
    fn browser_events_take_released_tabs_out_of_reuse() {
        use crate::browser::adapter::BrowserEvent;
        use crate::browser::BrowserEventSink;
        use crate::workspace::WorkspaceStore;

        let workspaces = WorkspaceStore::default();
        let sink = super::ServiceBrowserEvents {
            workbench: test_workbench(&workspaces),
            workspaces: workspaces.clone(),
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

    /// A workbench over `workspaces` and a fresh authority, with no browser connected.
    fn test_workbench(
        workspaces: &crate::workspace::WorkspaceStore,
    ) -> crate::workbench::WorkbenchFacade {
        crate::workbench::WorkbenchFacade::new(
            crate::workbench::WorkbenchProjection::default(),
            workspaces.clone(),
            crate::governance::GovernanceFacade::new(None, None),
            Arc::new(crate::browser::RelayBrowserPort::new(
                "events-test".into(),
                super::page_assets(),
            )),
            crate::diagnostics::DiagnosticsHub::for_tests(),
        )
    }

    /// Keeps the changes the workbench publishes, in order.
    #[derive(Default)]
    struct HeardChanges(std::sync::Mutex<Vec<crate::workbench::WorkbenchEvent>>);

    impl crate::workbench::WorkbenchEventSink for HeardChanges {
        fn publish(&self, event: crate::workbench::WorkbenchEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    impl HeardChanges {
        fn all(&self) -> Vec<crate::workbench::WorkbenchEvent> {
            self.0.lock().unwrap().clone()
        }

        /// The sessions the latest connections change named, if there has been one.
        fn sessions(&self) -> Option<usize> {
            self.all()
                .iter()
                .rev()
                .find_map(|event| match &event.change {
                    crate::workbench::WorkbenchChange::ConnectionsChanged { connections } => {
                        Some(connections.sessions.len())
                    }
                    _ => None,
                })
        }

        fn notes(&self) -> Vec<Option<String>> {
            self.all()
                .iter()
                .filter_map(|event| match &event.change {
                    crate::workbench::WorkbenchChange::StatusChanged { status } => {
                        Some(status.runtime_note.clone())
                    }
                    _ => None,
                })
                .collect()
        }
    }

    #[test]
    fn the_browser_toolbar_pauses_through_the_workbench_and_says_so() {
        use crate::browser::adapter::{BrowserEvent, RuntimeControlIntent};
        use crate::browser::BrowserEventSink;
        use crate::language::workbench::PAUSED_FROM_BROWSER;

        let workspaces = crate::workspace::WorkspaceStore::default();
        let workbench = test_workbench(&workspaces);
        let heard = Arc::new(HeardChanges::default());
        workbench.attach_events(heard.clone());
        let sink = super::ServiceBrowserEvents {
            workbench: workbench.clone(),
            workspaces,
        };
        let request = |intent| BrowserEvent::RuntimeControlRequested { intent };

        sink.on_event(FAKE_BROWSER, request(RuntimeControlIntent::ToggleHold));
        assert_eq!(heard.notes().pop(), Some(Some(PAUSED_FROM_BROWSER.into())));
        let statuses = heard.notes().len();
        let runtime_changes = |heard: &HeardChanges| {
            heard
                .all()
                .iter()
                .filter(|event| {
                    matches!(
                        event.change,
                        crate::workbench::WorkbenchChange::RuntimeChanged { .. }
                    )
                })
                .count()
        };
        assert_eq!(runtime_changes(&heard), 1);

        // The same pause again is a change of nothing a screen shows.
        sink.on_event(FAKE_BROWSER, request(RuntimeControlIntent::Hold));
        assert_eq!(runtime_changes(&heard), 2);
        assert_eq!(heard.notes().len(), statuses);

        sink.on_event(FAKE_BROWSER, request(RuntimeControlIntent::ToggleHold));
        assert_eq!(heard.notes().pop(), Some(None));
    }

    #[test]
    fn opening_and_closing_a_session_tells_every_screen() {
        let home = TestHome::new("connections-live");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let heard = Arc::new(HeardChanges::default());
        host.workbench.attach_events(heard.clone());

        let mut socket = open_mcp(None);
        initialize(&mut socket);
        let deadline = Instant::now() + PATIENCE;
        while heard.sessions() != Some(1) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(heard.sessions(), Some(1), "the session opened unannounced");

        drop(socket);
        let deadline = Instant::now() + PATIENCE;
        while heard.sessions() != Some(0) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(heard.sessions(), Some(0), "the session closed unannounced");
    }

    /// The instance every test service reports.
    const TEST_INSTANCE: &str = "instance_test";

    /// The MCP channel an edge names.
    const CONNECTOR_CHANNEL: &str = "npm";

    /// How long a test waits for the service to answer.
    const PATIENCE: Duration = Duration::from_secs(10);

    /// A relay listener over a fake browser, bound at the test home's endpoint.
    struct Served {
        intake: Arc<super::Intake>,
        browser: Arc<FakeBrowser>,
        relay: Arc<crate::browser::RelayBrowserPort>,
        listener: Arc<Listener>,
        thread: Option<thread::JoinHandle<()>>,
        quit_requested: std::sync::mpsc::Receiver<()>,
    }

    impl Drop for Served {
        fn drop(&mut self) {
            self.intake.stop.store(true, Ordering::SeqCst);
            self.listener.shutdown();
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    impl Served {
        /// A connection to the service that never touches the endpoint: the far end is served on
        /// its own thread, the way the listener serves an accepted one.
        fn over_a_pair(&self) -> Connection {
            let (client, server) = pair().unwrap();
            let intake = Arc::clone(&self.intake);
            thread::spawn(move || {
                let judgement = listener::judge(&server, intake.quit.drain.is_quitting());
                super::serve_connection(server, judgement, &intake);
            });
            client
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
        let quit = super::Quit::new(
            diagnostics.clone(),
            home.path().join("state").join("quit"),
            relay.clone(),
        );
        let (signal, quit_requested) = std::sync::mpsc::channel();
        *super::lock(&quit.signal) = Some(Arc::new(move || {
            let _ = signal.send(());
        }));
        let intake = Arc::new(super::Intake {
            instance: TEST_INSTANCE.into(),
            started_by: StartReason::Agent,
            templates: failure_templates(),
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
            workbench_server: std::sync::OnceLock::new(),
        });
        let listener = Arc::new(Listener::bind_first(home.path()).unwrap());
        let thread = super::spawn_listener(Arc::clone(&listener), intake.clone());
        Served {
            intake,
            browser,
            relay,
            listener,
            thread: Some(thread),
            quit_requested,
        }
    }

    /// An opening for `role` from an edge.
    fn opening(role: Role) -> RelayOpening {
        RelayOpening::new(role, Edge::new(Program::Edge, "2.0.0".into()))
    }

    /// An `mcp` opening the way the MCP edge presents it.
    fn mcp_opening(resume: Option<String>) -> RelayOpening {
        let mut opening = opening(Role::Mcp);
        opening.channel = Some(CONNECTOR_CHANNEL.into());
        opening.resume = resume;
        opening
    }

    /// Open `opening` on the service of the test home.
    fn dial(opening: &RelayOpening) -> (Connection, Reply) {
        let home = ghostlight_relay::home::home().unwrap();
        let (connection, reply) =
            protocol::connect(&home, opening, Instant::now() + PATIENCE).unwrap();
        connection.set_read_timeout(Some(PATIENCE)).unwrap();
        (connection, reply)
    }

    /// Open an `mcp` stream the way the MCP edge does.
    fn open_mcp(resume: Option<String>) -> Connection {
        let (connection, reply) = dial(&mcp_opening(resume));
        assert!(reply.accepted, "{reply:?}");
        connection
    }

    fn send_line(socket: &mut Connection, line: Value) {
        frame::write_frame(
            socket,
            &McpFrame::Message {
                line: line.to_string(),
            },
        )
        .unwrap();
    }

    /// Read frames until the response to `id`, collecting the resume tokens on the way.
    fn response(socket: &mut Connection, id: u64) -> (Value, Vec<String>) {
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
                McpFrame::Goodbye { .. } | McpFrame::Unknown => {}
            }
        }
    }

    fn initialize(socket: &mut Connection) -> (Value, Vec<String>) {
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
    fn an_mcp_opening_over_a_pair_is_accepted_with_the_templates_and_served() {
        let home = TestHome::new("mcp-pair");
        let served = serve_fake(&home);
        let mut client = served.over_a_pair();
        client.set_read_timeout(Some(PATIENCE)).unwrap();
        let reply = protocol::open(&client, &mcp_opening(None), Instant::now() + PATIENCE).unwrap();
        assert!(reply.accepted, "{reply:?}");
        assert_eq!(reply.service.as_deref(), Some(env!("CARGO_PKG_VERSION")));
        assert_eq!(reply.instance.as_deref(), Some(TEST_INSTANCE));
        assert_eq!(reply.templates, Some(failure_templates()));

        let (initialized, _) = initialize(&mut client);
        assert_eq!(initialized["result"]["serverInfo"]["name"], "ghostlight");
    }

    #[test]
    fn only_an_mcp_stream_is_handed_the_failure_templates() {
        let home = TestHome::new("templates-only-for-mcp");
        let served = serve_fake(&home);
        for role in [Role::Browser, Role::Control] {
            let client = served.over_a_pair();
            let mut opening = opening(role);
            opening.control = Some(Control::Status);
            let reply = protocol::open(&client, &opening, Instant::now() + PATIENCE).unwrap();
            assert!(reply.accepted, "{role:?}: {reply:?}");
            assert_eq!(reply.templates, None, "{role:?}");
            assert_eq!(reply.instance.as_deref(), Some(TEST_INSTANCE));
        }
    }

    #[test]
    fn control_status_answers_the_version_the_instance_and_the_process() {
        let home = TestHome::new("control-status");
        let served = serve_fake(&home);
        let status = cli_client::status().unwrap();
        assert_eq!(
            status,
            control::Status {
                service: env!("CARGO_PKG_VERSION").into(),
                instance: TEST_INSTANCE.into(),
                pid: std::process::id(),
                started_by: StartReason::Agent,
            }
        );
        drop(served);
    }

    #[test]
    fn the_desktop_and_command_roles_are_refused_as_unsupported_with_the_service_sentence() {
        let home = TestHome::new("unsupported-roles");
        let served = serve_fake(&home);
        for role in [Role::Desktop, Role::Command] {
            let client = served.over_a_pair();
            let reply = protocol::open(&client, &opening(role), Instant::now() + PATIENCE).unwrap();
            assert!(!reply.accepted, "{role:?}");
            assert_eq!(reply.code, Some(RefusalCode::Unsupported), "{role:?}");
            assert_eq!(
                reply.say,
                Some(OpeningRefusal::Unsupported.message()),
                "{role:?}"
            );
        }
        assert!(served.relay.connected_browsers().is_empty());
        assert!(served.intake.workspaces.summaries().is_empty());
    }

    #[test]
    fn an_opening_that_names_relay_3_is_refused_and_opens_nothing() {
        let home = TestHome::new("relay-3");
        let served = serve_fake(&home);
        let client = served.over_a_pair();
        let mut earlier = opening(Role::Browser);
        earlier.relay = 3;
        let reply = protocol::open(&client, &earlier, Instant::now() + PATIENCE).unwrap();
        assert_eq!(reply.code, Some(RefusalCode::Unsupported));
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

        let (service, extension) = pair().unwrap();
        let fake = FakeExtension::connect(
            extension,
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
        served.relay.attach(service).unwrap();
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
    fn publishing_starts_the_workbench_and_bars_agents_from_its_address() {
        let home = TestHome::new("workbench-published");
        let host = ServiceHost::start(home.path()).unwrap();
        assert!(host.workbench_address().is_none());
        host.publish_ready().unwrap();
        let address = host.workbench_address().expect("the workbench is served");
        assert!(address.url().starts_with("http://127.0.0.1:"));

        let snapshot = host.intake.governance.snapshot();
        for url in [address.url().to_owned(), format!("{}app.js", address.url())] {
            let decision = snapshot.authorize_landing(crate::governance::CapabilitySet::READ, &url);
            assert!(!decision.allowed, "{url} must be barred from agents");
        }
        let elsewhere = snapshot.authorize_landing(
            crate::governance::CapabilitySet::READ,
            "https://example.com/",
        );
        assert!(elsewhere.allowed);

        // The page answers on that address, and the facade knows where it is.
        assert_eq!(host.workbench.web_address(), Some(address));
        host.publish_ready().unwrap();
        drop(host);
    }

    #[test]
    fn the_published_line_names_the_address_and_never_the_bearer() {
        let home = TestHome::new("workbench-line");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let address = host.workbench_address().unwrap();
        let bearer = host
            .intake
            .workbench_server
            .get()
            .unwrap()
            .bearer()
            .to_owned();
        let line = super::published_detail(Some("endpoint"), Some(&address));
        assert!(line.contains(&format!("workbench={}", address.url())));
        assert!(!line.contains(&bearer));
        assert!(!format!("{:?}", host.intake.workbench_server.get().unwrap()).contains(&bearer));
        assert!(super::published_detail(None, None).ends_with("workbench="));
        drop(host);
    }

    #[test]
    fn a_stopped_host_frees_the_workbench_port() {
        let home = TestHome::new("workbench-freed");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let port = host.workbench_address().unwrap().port();
        drop(host);
        assert!(
            std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_err(),
            "nothing answers on the port after the service stops"
        );
    }

    #[test]
    fn readiness_never_starts_an_absent_service() {
        let home = TestHome::new("readiness-absent");
        assert!(cli_client::readiness().is_err());
        assert!(cli_client::status().is_err());
        assert!(!home.path().join("run").exists());
    }

    /// Open one MCP session, end the connection the given way, and report the sessions left.
    fn sessions_after(name: &str, goodbye: impl FnOnce(&mut Connection)) -> usize {
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
    fn a_prepared_service_binds_its_endpoint_only_at_desktop_ready() {
        let home = TestHome::new("desktop-readiness");
        let host = ServiceHost::start(home.path()).unwrap();
        assert!(cli_client::readiness().is_err());
        let contender = ServiceHost::start(home.path()).err().unwrap();
        assert!(contender.is::<super::AuthorityAlreadyRunning>());
        host.publish_ready().unwrap();
        assert!(cli_client::readiness().is_ok());
        let status = cli_client::status().unwrap();
        assert_eq!(status.instance, host.intake.instance);
        assert!(status.instance.starts_with("instance_"));
        assert_eq!(status.service, env!("CARGO_PKG_VERSION"));
        host.publish_ready().unwrap();
        assert!(cli_client::readiness().is_ok());
        drop(host);
        assert!(
            cli_client::readiness().is_err(),
            "a stopped service leaves nothing to find"
        );
        let unpublished = ServiceHost::start(home.path()).unwrap();
        assert!(
            cli_client::readiness().is_err(),
            "failed desktop setup binds no endpoint"
        );
        drop(unpublished);
    }

    #[test]
    fn two_homes_bind_two_endpoints_and_serve_two_instances() {
        let first = temp_home("endpoint-first");
        let second = temp_home("endpoint-second");
        assert_ne!(
            Endpoint::for_home(&first).unwrap(),
            Endpoint::for_home(&second).unwrap()
        );
        let one = ServiceHost::start(&first).unwrap();
        let two = ServiceHost::start(&second).unwrap();
        one.publish_ready().unwrap();
        two.publish_ready().unwrap();
        let seen_by_first = cli_client::status_at(&first).unwrap();
        let seen_by_second = cli_client::status_at(&second).unwrap();
        assert_eq!(seen_by_first.instance, one.intake.instance);
        assert_eq!(seen_by_second.instance, two.intake.instance);
        assert_ne!(seen_by_first.instance, seen_by_second.instance);
        drop(one);
        assert!(cli_client::status_at(&first).is_err());
        assert!(cli_client::status_at(&second).is_ok());
        drop(two);
        std::fs::remove_dir_all(first).unwrap();
        std::fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn a_second_service_on_one_home_cannot_take_the_service_lock() {
        let directory = temp_home("singleton");
        let first = ServiceHost::start(&directory).unwrap();
        let second = ServiceHost::start(&directory)
            .err()
            .expect("second authority is rejected");
        let detail = format!("{second:#}");
        assert!(
            detail.contains("another Ghostlight orchestrator already owns this runtime"),
            "{detail}"
        );
        assert!(!super::ServiceLock::is_free(
            &crate::install::layout::Layout::new(&directory).service_lock()
        ));
        drop(first);
        let replacement = ServiceHost::start(&directory).unwrap();
        drop(replacement);
        std::fs::remove_dir_all(directory).unwrap();
    }

    /// The terms of a start for `reason` that waits `lock_wait` for a held lock.
    fn waiting(reason: StartReason, lock_wait: Duration) -> StartTerms {
        StartTerms { reason, lock_wait }
    }

    /// Start for `terms` in `directory` with a token that is not elevated.
    fn start_for(directory: &std::path::Path, terms: StartTerms) -> anyhow::Result<ServiceHost> {
        ServiceHost::start_with_token(directory, terms, || Ok(false))
    }

    /// Every file and directory under `root`, as paths relative to it.
    fn files_under(root: &std::path::Path) -> std::collections::BTreeSet<std::path::PathBuf> {
        let mut found = std::collections::BTreeSet::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory)
                .into_iter()
                .flatten()
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path.clone());
                }
                found.insert(path.strip_prefix(root).unwrap().to_path_buf());
            }
        }
        found
    }

    #[test]
    fn a_start_that_finds_the_lock_held_gives_way_at_once_and_touches_no_state() {
        let directory = temp_home("lock-held-gives-way");
        let layout = crate::install::layout::Layout::new(&directory);
        let holder = start_for(&directory, agent()).unwrap();
        super::latch::set(&layout.quit_latch()).unwrap();
        let before = files_under(&directory);

        for reason in [
            StartReason::Agent,
            StartReason::Command,
            StartReason::Person,
            StartReason::Update,
        ] {
            let started = Instant::now();
            let terms = StartTerms {
                reason,
                lock_wait: Duration::ZERO,
            };
            let error = start_for(&directory, terms)
                .err()
                .expect("the lock is held");
            assert!(error.is::<AuthorityAlreadyRunning>(), "{reason:?}");
            assert!(started.elapsed() < Duration::from_secs(1), "{reason:?}");
            assert!(
                super::latch::is_set(&layout.quit_latch()),
                "{reason:?}: a start that gives way leaves the latch alone"
            );
        }
        assert_eq!(files_under(&directory), before, "no state was opened");
        drop(holder);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn an_update_start_takes_the_lock_when_the_old_service_releases_it_within_the_wait() {
        let directory = temp_home("update-waits");
        let holder = start_for(&directory, agent()).unwrap();
        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            drop(holder);
        });
        let started = Instant::now();
        let taken = start_for(&directory, waiting(StartReason::Update, PATIENCE)).unwrap();
        assert!(
            started.elapsed() >= Duration::from_millis(100),
            "it waited for the holder"
        );
        release.join().unwrap();
        drop(taken);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn an_update_start_gives_up_when_the_lock_is_still_held_after_its_wait() {
        let directory = temp_home("update-gives-up");
        let holder = start_for(&directory, agent()).unwrap();
        let started = Instant::now();
        let error = start_for(
            &directory,
            waiting(StartReason::Update, Duration::from_millis(200)),
        )
        .err()
        .expect("the lock stays held");
        assert!(error.is::<AuthorityAlreadyRunning>());
        assert!(started.elapsed() >= Duration::from_millis(200));
        drop(holder);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_browser_or_recover_start_is_refused_while_the_latch_is_set_and_never_takes_the_lock() {
        let directory = temp_home("latch-refuses");
        let layout = crate::install::layout::Layout::new(&directory);
        super::latch::set(&layout.quit_latch()).unwrap();
        for reason in [StartReason::Browser, StartReason::Recover] {
            let error = start_for(&directory, StartTerms::for_reason(reason))
                .err()
                .expect("Quit holds this start back");
            assert!(error.is::<QuitLatched>(), "{reason:?}");
            assert!(
                super::latch::is_set(&layout.quit_latch()),
                "{reason:?}: the latch stays"
            );
            assert!(
                !layout.run().exists(),
                "{reason:?}: the lock was never taken"
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_browser_start_serves_when_no_quit_is_held() {
        let directory = temp_home("latch-clear");
        let started = start_for(&directory, StartTerms::for_reason(StartReason::Browser));
        assert!(started.is_ok());
        drop(started);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn any_other_start_clears_the_latch_after_taking_the_lock_and_serves() {
        for reason in [
            StartReason::Agent,
            StartReason::Command,
            StartReason::Person,
            StartReason::Update,
        ] {
            let directory = temp_home("latch-cleared");
            let layout = crate::install::layout::Layout::new(&directory);
            super::latch::set(&layout.quit_latch()).unwrap();
            let started = start_for(&directory, StartTerms::for_reason(reason)).unwrap();
            assert!(!super::latch::is_set(&layout.quit_latch()), "{reason:?}");
            drop(started);
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn the_service_reports_the_reason_it_was_started_for() {
        let home = TestHome::new("started-by");
        let host = start_for(
            home.path(),
            StartTerms {
                reason: StartReason::Person,
                lock_wait: Duration::ZERO,
            },
        )
        .unwrap();
        host.publish_ready().unwrap();
        assert_eq!(
            cli_client::status().unwrap().started_by,
            StartReason::Person
        );
        drop(host);
    }

    #[test]
    fn a_service_started_with_an_elevated_token_refuses_before_it_takes_any_state() {
        let directory = temp_home("elevated");
        let refused = ServiceHost::start_with_token(&directory, agent(), || Ok(true))
            .err()
            .expect("an elevated start is refused");
        assert_eq!(format!("{refused:#}"), super::ELEVATED_SERVICE);
        assert!(
            !directory.join("run").exists(),
            "no lock, run directory or socket was made"
        );
        let unreadable = ServiceHost::start_with_token(&directory, agent(), || {
            Err(std::io::Error::other("no token"))
        })
        .err()
        .expect("a token that cannot be read is refused");
        assert!(format!("{unreadable:#}").contains("no token"));
        let started = ServiceHost::start_with_token(&directory, agent(), || Ok(false));
        assert!(started.is_ok());
        drop(started);
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
    fn show_reveals_the_existing_workbench() {
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
        // Stand in for the browser edge: carry the extension's bytes over a browser stream.
        let (native, extension) = pair().unwrap();
        let carrier = thread::spawn(move || {
            let (service, reply) = dial(&opening(Role::Browser));
            assert!(reply.accepted, "{reply:?}");
            service.set_read_timeout(None).unwrap();
            let mut to_service = (native.try_clone().unwrap(), service.try_clone().unwrap());
            let mut to_extension = (service, native);
            thread::spawn(move || {
                let _ = std::io::copy(&mut to_extension.0, &mut to_extension.1);
            });
            let _ = std::io::copy(&mut to_service.0, &mut to_service.1);
        });
        let fake = FakeExtension::connect(
            extension,
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

    /// Read frames from `socket` until a goodbye arrives and return its reason.
    fn goodbye_on(socket: &mut Connection) -> GoodbyeReason {
        loop {
            match frame::read_frame::<McpFrame>(socket).unwrap() {
                Some(McpFrame::Goodbye { reason }) => return reason,
                Some(_) => {}
                None => panic!("the stream closed with no goodbye"),
            }
        }
    }

    #[test]
    fn a_persons_quit_sets_the_latch_and_a_replacement_or_uninstall_does_not() {
        for (reason, latched) in [
            (QuitReason::Quit, true),
            (QuitReason::Replace, false),
            (QuitReason::Uninstall, false),
        ] {
            let (home, host, quit_requested) = quittable_host("quit-latch");
            let latch = home.path().join("state").join("quit");
            assert!(!super::latch::is_set(&latch));
            host.begin_quit(reason, Duration::from_secs(5));
            quit_requested
                .recv_timeout(Duration::from_secs(5))
                .expect("an idle service asks the desktop to exit");
            assert_eq!(super::latch::is_set(&latch), latched, "{reason:?}");
            drop(host);
        }
    }

    #[test]
    fn a_quit_over_control_with_reason_quit_sets_the_latch_before_it_answers() {
        let home = TestHome::new("quit-control-latch");
        let served = serve_fake(&home);
        cli_client::control(Control::Quit {
            reason: QuitReason::Quit,
        })
        .unwrap();
        assert!(super::latch::is_set(
            &home.path().join("state").join("quit")
        ));
        served
            .quit_requested
            .recv_timeout(Duration::from_secs(5))
            .expect("the desktop is asked to exit");
    }

    #[test]
    fn a_quit_says_goodbye_on_an_mcp_stream_before_the_service_closes_it() {
        for (reason, goodbye) in [
            (QuitReason::Quit, GoodbyeReason::Quit),
            (QuitReason::Replace, GoodbyeReason::Replace),
            (QuitReason::Uninstall, GoodbyeReason::Uninstall),
        ] {
            let home = TestHome::new("quit-goodbye");
            let served = serve_fake(&home);
            let mut socket = open_mcp(None);
            initialize(&mut socket);

            let answer = cli_client::control(Control::Quit { reason }).unwrap();
            assert_eq!(answer, json!({"quitting": true}));
            assert_eq!(goodbye_on(&mut socket), goodbye);
            served
                .quit_requested
                .recv_timeout(Duration::from_secs(5))
                .expect("the desktop is asked to exit after the goodbye");
        }
    }

    #[test]
    fn a_session_end_says_goodbye_on_an_mcp_stream_and_leaves_without_the_latch() {
        let (home, host, quit_requested) = quittable_host("session-end-goodbye");
        let mut socket = open_mcp(None);
        initialize(&mut socket);
        let running = host.intake.quit.drain.admit().unwrap();

        host.quit_handle().session_end();

        assert_eq!(goodbye_on(&mut socket), GoodbyeReason::SessionEnd);
        quit_requested
            .recv_timeout(Duration::from_secs(5))
            .expect("the desktop is asked to exit without waiting for work in flight");
        assert!(
            !super::latch::is_set(&home.path().join("state").join("quit")),
            "a sign-out is not a person's Quit"
        );
        assert!(
            host.intake.quit.drain.admit().is_err(),
            "no new work is admitted once the session is ending"
        );
        drop(running);
    }

    #[test]
    fn a_quit_says_goodbye_after_in_flight_work_and_before_the_desktop_exits() {
        let home = TestHome::new("quit-goodbye-order");
        let served = serve_fake(&home);
        let mut socket = open_mcp(None);
        initialize(&mut socket);
        let running = served.intake.quit.drain.admit().unwrap();

        cli_client::control(Control::Quit {
            reason: QuitReason::Quit,
        })
        .unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(300)))
            .unwrap();
        while let Ok(Some(frame)) = frame::read_frame::<McpFrame>(&mut socket) {
            assert!(
                !matches!(frame, McpFrame::Goodbye { .. }),
                "no goodbye while work is in flight"
            );
        }
        assert!(served.quit_requested.try_recv().is_err());

        socket.set_read_timeout(Some(PATIENCE)).unwrap();
        drop(running);
        assert_eq!(goodbye_on(&mut socket), GoodbyeReason::Quit);
    }

    #[test]
    fn a_quit_says_goodbye_on_a_browser_stream_with_the_fixed_bytes() {
        use crate::browser::adapter::chromium::protocol4::fake::FakeExtension;
        use crate::browser::adapter::chromium::protocol4::fake::Reply as FakeReply;

        let home = TestHome::new("quit-browser-goodbye");
        let served = serve_fake(&home);
        let (native, extension) = pair().unwrap();
        let carrier = thread::spawn(move || {
            let (service, reply) = dial(&opening(Role::Browser));
            assert!(reply.accepted, "{reply:?}");
            service.set_read_timeout(None).unwrap();
            let mut to_service = (native.try_clone().unwrap(), service.try_clone().unwrap());
            let mut to_extension = (service, native);
            thread::spawn(move || {
                let _ = std::io::copy(&mut to_extension.0, &mut to_extension.1);
            });
            let _ = std::io::copy(&mut to_service.0, &mut to_service.1);
        });
        let fake = FakeExtension::connect(
            extension,
            &FakeExtension::hello("browser_goodbye"),
            Box::new(|correlation, _| {
                FakeReply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))
            }),
        );
        drop(fake.wait(|seen| !seen.frames.is_empty()));
        eventually(|| served.relay.is_connected());

        served.relay.say_goodbye(GoodbyeReason::Uninstall);
        let seen = fake.wait(|seen| {
            seen.frames
                .iter()
                .any(|frame| frame.get("goodbye").is_some())
        });
        let goodbye = seen
            .frames
            .iter()
            .find(|frame| frame.get("goodbye").is_some())
            .cloned()
            .unwrap();
        assert_eq!(goodbye, json!({"goodbye": "uninstall"}));
        drop(seen);
        fake.close();
        drop(fake);
        let _ = carrier.join();
    }

    #[test]
    fn a_quit_answers_new_invocations_with_shutting_down() {
        use crate::language::intake::{SHUTTING_DOWN_CODE, SHUTTING_DOWN_MESSAGE};

        let (_home, host, quit_requested) = quittable_host("quit-refuses");
        let mut socket = open_mcp(None);
        initialize(&mut socket);

        host.begin_quit(QuitReason::Quit, Duration::from_secs(5));
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
        let mut readiness = opening(Role::Control);
        readiness.control = Some(Control::Readiness);
        let (_, reply) = dial(&readiness);
        assert!(!reply.accepted, "{reply:?}");
        assert_eq!(reply.code, Some(RefusalCode::ShuttingDown));
        drop(socket);
        drop(host);
    }

    #[test]
    fn a_quit_over_control_answers_and_drains() {
        let home = TestHome::new("quit-control");
        let served = serve_fake(&home);
        let running = served.intake.quit.drain.admit().unwrap();

        let answer = cli_client::control(Control::Quit {
            reason: QuitReason::Quit,
        })
        .unwrap();
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

        host.begin_quit(QuitReason::Quit, Duration::from_secs(30));
        assert!(
            quit_requested
                .recv_timeout(Duration::from_millis(300))
                .is_err(),
            "the desktop is not asked to exit while work is in flight"
        );
        // A second quit while one is under way changes nothing.
        host.begin_quit(QuitReason::Quit, Duration::ZERO);
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
        host.begin_quit(QuitReason::Quit, Duration::from_millis(200));
        quit_requested
            .recv_timeout(Duration::from_secs(5))
            .expect("the desktop is asked to exit when the grace ends");
        assert!(started.elapsed() >= Duration::from_millis(200));
        drop(host);
    }

    #[test]
    fn a_start_that_never_becomes_ready_stops_by_itself() {
        let directory = temp_home("ready-stalls");
        let host = ServiceHost::start(&directory).unwrap();
        let (expired, fired) = std::sync::mpsc::channel();
        host.expect_ready_by(Instant::now() + Duration::from_millis(100), move || {
            let _ = expired.send(());
        })
        .unwrap();
        assert!(fired.recv_timeout(Duration::from_secs(5)).is_ok());
        drop(host);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn becoming_ready_cancels_the_stop() {
        let directory = temp_home("ready-published");
        let host = ServiceHost::start(&directory).unwrap();
        let (expired, fired) = std::sync::mpsc::channel();
        host.expect_ready_by(Instant::now() + Duration::from_millis(200), move || {
            let _ = expired.send(());
        })
        .unwrap();
        host.publish_ready().unwrap();
        assert!(fired.recv_timeout(Duration::from_millis(600)).is_err());
        drop(host);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn the_session_watch_tells_the_client_only_when_a_policy_change_changes_the_catalog() {
        use crate::governance::GovernanceFacade;

        use super::AdvertisedCatalog;

        let directory = std::env::temp_dir().join(format!(
            "ghostlight-service-catalog-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("policy.json");
        let write = |name: &str, allowed: Value| {
            std::fs::write(
                &path,
                serde_json::to_vec(&json!({
                    "schema": 4,
                    "name": name,
                    "version": "1",
                    "grants": [{"id": "all", "hosts": {"allow": ["*"]}, "allowed": allowed}]
                }))
                .unwrap(),
            )
            .unwrap();
        };
        write("rules", json!(["read", "action", "write", "execute"]));
        let governance = GovernanceFacade::new(Some(path.clone()), None);
        let advertised = AdvertisedCatalog::default();
        assert!(
            advertised.changed_by(&governance.snapshot()).is_none(),
            "nothing is compared before the client has a catalog"
        );
        let given = advertised.advertise(&governance.snapshot());
        assert!(advertised.changed_by(&governance.snapshot()).is_none());

        write(
            "renamed rules",
            json!(["read", "action", "write", "execute"]),
        );
        governance.refresh_tick();
        assert!(
            advertised.changed_by(&governance.snapshot()).is_none(),
            "a new policy that admits the same tools changes nothing the client sees"
        );

        write("narrower", json!(["read"]));
        governance.refresh_tick();
        let narrowed = advertised
            .changed_by(&governance.snapshot())
            .expect("a narrower policy changes the catalog");
        assert!(narrowed.len() < given.len());
        assert!(
            advertised.changed_by(&governance.snapshot()).is_none(),
            "one change is told once"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
