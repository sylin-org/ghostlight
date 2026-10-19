//! The persistent service: its lifetime, its one relay listener and the sessions it serves.
//!
//! [`ServiceHost`] holds the service lock, prepares the executor and the browser port, and binds
//! the relay protocol 4 endpoint (ADR-0199 Decisions 5 and 9). The workbench's loopback server starts first, on one port per user, and carries no relay
//! peer. Every connection then opens with one relay opening: an `mcp` stream is one MCP client
//! session served by [`crate::intake::mcp::McpSession`], a `browser` stream is one adapter
//! connection, a `control` stream answers one operation, and a `desktop` stream hands the shell of
//! the same build the workbench address and its secret (ADR-0199 Decision 7).
//!
//! The service links no UI toolkit. [`presence`] keeps the shell running whenever the desktop can
//! show it, and the service describes the tray the shell draws. It binds its endpoint as soon as
//! it starts (ADR-0199 Decision 5).
//!
//! A start carries the reason it was made for (ADR-0199 Decision 10). The reason decides what a
//! held lock and the quit latch mean to it ([`lifetime`], [`latch`]). A person's Quit sets the
//! latch. Every quit decides its sessions' tabs once, after the drain and under one deadline
//! ([`Quit`], ADR-0203 Decision 3): each MCP stream is handed its last resume token, the MCP
//! goodbyes go out, the workspaces that end are released, and the browser goodbye goes last. A
//! replacement keeps each live MCP session bound for its edge's resume; any other quit releases
//! every session. The end of the session is the fourth goodbye: the service forgets every tab
//! without a browser command, says `session_end` on every stream and leaves without the latch and
//! without waiting for work in flight (ADR-0199 Decision 13). The last way out is the idle exit:
//! after [`idle::IDLE_EXIT`] with no stream of any role open the service forgets every tab and
//! leaves with no goodbye and no latch (ADR-0199 Decision 12).

mod admission;
pub mod hint;
pub mod idle;
pub mod latch;
pub mod lifetime;
pub mod lock;
pub mod presence;
pub mod shell;
#[cfg(test)]
pub(crate) mod test_home;

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
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
use crate::browser::adapter::{BrowserCommand, BrowserEvent};
use crate::browser::transport::{SocketReader, SocketWriter};
use crate::browser::{
    AdapterLifecycleObserver, BrowserError, BrowserEventSink, BrowserPort, RelayBrowserPort,
};
use crate::diagnostics::DiagnosticsHub;
use crate::governance::{AuthoritySnapshot, GovernanceFacade, JsonlAuditSink};
use crate::install;
use crate::install::icons::{self, IconCache, IconPlaces};
use crate::intake::desktop::{DesktopFrame, ShellView, TrayItemId, TrayModel};
use crate::intake::listener::{self, control, Judgement};
use crate::intake::mcp::{failure_templates, McpSession, OwnedTab, ResumeState, Step};
use crate::language::catalog_for;
use crate::language::environment::{ELEVATED_SERVICE, QUIT_HELD};
use crate::language::intake::{OpeningRefusal, SHUTTING_DOWN_CODE, SHUTTING_DOWN_MESSAGE};
use crate::language::outcome::{CancelCause, DialogKind};
use crate::language::workbench::{TRAY_HINT_BODY, TRAY_HINT_TITLE};
use crate::presentation::{BrowserPresentation, PresentationReactor};
use crate::provenance::{Attribution, ConnectionEvidence, SourceReader};
use crate::work::{Admission, ApplicationExecutor, InvocationId, PreparedInvocation};
use crate::workbench::setup::SetupProgress;
use crate::workbench::web::{PendingWorkbench, WorkbenchAddress, WorkbenchServer};
use crate::workbench::{
    PageAddress, RuntimeSource, Watching, WorkbenchFacade, WorkbenchProjection, WorkbenchQuitPort,
    WorkbenchTrayPort,
};
use crate::workspace::{ReleasedTabs, UrlDigest, WorkspaceId, WorkspaceStore};
use presence::Presence;
use shell::{ShellRoute, Shells};

const DIAGNOSTIC_CLEAR_BATCH_SIZE: usize = 256;

/// The longest one release command may take: a diagnostics clear or a tab close.
const RELEASE_CALL_LIMIT: Duration = Duration::from_secs(2);
const QUEUED_METADATA_BYTES: usize = 512;

/// Prefix of the value that changes on every service start.
const INSTANCE_PREFIX: &str = "instance_";

/// Environment variable naming an audit log in place of the one in the state directory.
const AUDIT_FILE_VARIABLE: &str = "GHOSTLIGHT_AUDIT_FILE";

/// File name of the audit log in the state directory.
const AUDIT_FILE_NAME: &str = "audit.jsonl";

/// How long the accept loop rests after a failed accept before it asks again.
const ACCEPT_RETRY: Duration = Duration::from_millis(50);

use admission::{Capacity, QueueBudget, Refused, SessionQueue};
use idle::Streams;
use lifetime::{Clock, Drain, ReadyWatch, SystemClock, END_LIMIT, QUIT_GRACE, READY_LIMIT};
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

/// Record in diagnostics that this start failed, with the whole chain of `error`: a started
/// service's own output goes nowhere, so this is the line a person reads.
fn startup_failed(diagnostics: &DiagnosticsHub, error: &anyhow::Error) {
    diagnostics.sink().emit(
        ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
        ghostlight_bridge::diagnostics::Level::Error,
        None,
        &format!("service startup: {error:#}"),
    );
}

/// The request that ends the service's wait for a quit, installed by the process that serves.
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
    /// How long the service may hold no stream of any role before it exits.
    pub idle_exit: Duration,
}

impl StartTerms {
    /// The terms of a start for `reason`: an update waits for the previous service to leave, any
    /// other start asks once.
    #[must_use]
    pub fn for_reason(reason: StartReason) -> Self {
        Self {
            reason,
            lock_wait: lifetime::lock_wait(reason),
            idle_exit: idle::IDLE_EXIT,
        }
    }

    /// The terms of a start for `reason` that asks for the lock once, so a test that expects a
    /// held lock learns it at once.
    #[cfg(test)]
    fn asking_once(reason: StartReason) -> Self {
        Self {
            lock_wait: Duration::ZERO,
            ..Self::for_reason(reason)
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
        Self::start_with_token(home, StartTerms::asking_once(StartReason::Agent), || {
            Ok(false)
        })
    }

    /// Like [`ServiceHost::start`], with presence acting on `world` and so starting no process.
    #[cfg(test)]
    pub(crate) fn start_in_world(
        home: &Path,
        world: impl FnOnce(install::layout::Layout, presence::Ending) -> Box<dyn presence::Outside>,
    ) -> Result<Self> {
        Self::start_with_world(
            home,
            StartTerms::asking_once(StartReason::Agent),
            || Ok(false),
            world,
            Arc::new(SystemClock),
        )
    }

    /// The route the workbench facade is given, for tests that drive it.
    #[cfg(test)]
    pub(crate) fn route(&self) -> Arc<ShellRoute> {
        Arc::new(ShellRoute::new(
            self.quit.shells(),
            Arc::clone(&self.intake.presence),
        ))
    }

    /// Prepare the service in the Ghostlight home `home` for a start made for `reason`, under its
    /// lifetime lock, without binding the endpoint. `publish_ready` binds it.
    ///
    /// In order: a process whose token is elevated is refused; a browser or recover start is
    /// refused while the quit latch is set ([`QuitLatched`]); the lock is taken, and a start that
    /// finds it held asks again, an update for [`lifetime::UPDATE_LOCK_WAIT`] so the service it
    /// replaces can finish leaving and any other start for [`lifetime::LEAVING_LOCK_WAIT`] so it
    /// outlasts a service leaving on idle, and gives way ([`AuthorityAlreadyRunning`]) when the
    /// wait ends; a browser or recover start that waited looks at the latch again once it holds
    /// the lock; and only then does any other start clear the latch and open state.
    ///
    /// The service then shows the installed agents' own icons: it publishes the icon cache in the
    /// home's state directory, which every agent's symbol reads, and looks for the icons in the
    /// background. Only this start does, so a test service never reads this computer's apps.
    pub fn start_waiting(home: &Path, reason: StartReason) -> Result<Self> {
        let host = Self::start_with_token(
            home,
            StartTerms::for_reason(reason),
            ghostlight_win_peer::is_elevated,
        )?;
        let cache = IconCache::open(
            install::layout::Layout::new(home).icons(),
            IconPlaces::system(),
            Some(host.diagnostics.sink()),
        );
        icons::publish(&cache);
        host.workbench.attach_icons(cache);
        Ok(host)
    }

    /// Like [`ServiceHost::start_waiting`], with the terms spelled out and the answer to "is this
    /// process elevated" given by `elevated`.
    pub(crate) fn start_with_token(
        home: &Path,
        terms: StartTerms,
        elevated: impl FnOnce() -> io::Result<bool>,
    ) -> Result<Self> {
        Self::start_with_world(
            home,
            terms,
            elevated,
            presence::System::boxed,
            Arc::new(SystemClock),
        )
    }

    /// Like [`ServiceHost::start_with_token`], with the world presence acts on made by `world`
    /// from the installation and the question "has a quit begun", and the idle limit judged by
    /// `idle_clock`. A test passes a world that starts no process and a clock it moves by hand.
    pub(crate) fn start_with_world(
        home: &Path,
        terms: StartTerms,
        elevated: impl FnOnce() -> io::Result<bool>,
        world: impl FnOnce(install::layout::Layout, presence::Ending) -> Box<dyn presence::Outside>,
        idle_clock: Arc<dyn Clock>,
    ) -> Result<Self> {
        let diagnostics = DiagnosticsHub::birth(home);
        Self::start_with_diagnostics(
            home,
            Arc::clone(&diagnostics),
            terms,
            elevated,
            world,
            idle_clock,
        )
        .inspect_err(|error| {
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
                startup_failed(&diagnostics, error);
            }
        })
    }

    fn start_with_diagnostics(
        home: &Path,
        diagnostics: Arc<DiagnosticsHub>,
        terms: StartTerms,
        elevated: impl FnOnce() -> io::Result<bool>,
        world: impl FnOnce(install::layout::Layout, presence::Ending) -> Box<dyn presence::Outside>,
        idle_clock: Arc<dyn Clock>,
    ) -> Result<Self> {
        refuse_elevated_token(elevated)?;
        let layout = install::layout::Layout::new(home);
        if lifetime::refused_by_quit_latch(terms.reason) && latch::is_set(&layout.quit_latch()) {
            return Err(QuitLatched.into());
        }
        let lock = acquire_lock(&layout.service_lock(), terms.lock_wait)?;
        let lock_taken = Instant::now();
        // A Quit that began while this start waited for the lock is the old service's own, and
        // this start must not bring back what the person just quit.
        if lifetime::refused_by_quit_latch(terms.reason) && latch::is_set(&layout.quit_latch()) {
            return Err(QuitLatched.into());
        }
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
        browser.remember_browsers_in(layout.last_browser_record());
        let browser_port: Arc<dyn BrowserPort> = browser.clone();
        let streams = Streams::new(idle_clock, terms.idle_exit);
        browser.set_lifecycle_observer(Arc::new(idle::CountBrowsers::new(
            Arc::clone(&streams),
            Arc::clone(&diagnostics) as Arc<dyn AdapterLifecycleObserver>,
        )));
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
        browser.set_event_sink(Arc::new(ServiceBrowserEvents::new(
            workbench.clone(),
            workspaces.clone(),
        )));

        let stop = Arc::new(AtomicBool::new(false));
        let quit = Quit::new(
            Arc::clone(&diagnostics),
            layout.quit_latch(),
            Arc::clone(&browser),
            workspaces.clone(),
            browser_port.clone(),
        );
        let ending: presence::Ending = {
            let quit = quit.clone();
            Arc::new(move || quit.drain.is_quitting())
        };
        let presence = Arc::new(Presence::new(world(layout.clone(), ending)));
        let route = Arc::new(ShellRoute::new(quit.shells(), Arc::clone(&presence)));
        workbench.attach_presentation(route.clone());
        workbench.attach_native_acts(route);
        workbench.attach_setup(SetupProgress::load(&layout.setup_record()));
        let tray_relay = TrayRelay::spawn(quit.shells(), Arc::clone(&stop))?;
        workbench.attach_tray(tray_relay.0);
        let presence_thread = presence.spawn(Arc::clone(&stop))?;
        let idle_thread = {
            let held = Arc::clone(&quit.drain);
            let leaving = quit.clone();
            let limit = terms.idle_exit;
            idle::watch(
                Arc::clone(&streams),
                Arc::clone(&stop),
                move || held.in_flight() > 0,
                move || leaving.idle_exit(limit),
            )?
        };
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
            desktops: Capacity::new(admission::DESKTOPS),
            budget: QueueBudget::default(),
            stop: Arc::clone(&stop),
            workbench_server: OnceLock::new(),
            presence,
            streams,
            layout,
            sources: SourceReader::system(),
        });
        Ok(Self {
            workbench,
            diagnostics,
            browser,
            audit,
            stop,
            threads: vec![
                audit_thread,
                refresher,
                tray_relay.1,
                presence_thread,
                idle_thread,
            ],
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

    /// Start the workbench's loopback server, then bind the relay endpoint and start accepting on
    /// it. The shell is started two seconds later when none has attached (see [`presence`]).
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
        let listener = match Listener::bind_first(&self.home).context("bind the relay endpoint") {
            Ok(listener) => Arc::new(listener),
            Err(error) => {
                startup_failed(&self.diagnostics, &error);
                return Err(error);
            }
        };
        let thread = spawn_listener(Arc::clone(&listener), Arc::clone(&self.intake));
        *bound = Some(Bound { listener, thread });
        self.ready.settle();
        self.intake.streams.published();
        self.intake.presence.published();
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
        self.workbench
            .attach_quit(Arc::new(self.intake.quit.clone()));
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
    #[cfg(test)]
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

    /// Install the request that ends the service's wait for a quit, made once the quit has said
    /// goodbye on every stream.
    pub fn install_quit_signal(&self, signal: QuitSignal) {
        *lock(&self.quit.signal) = Some(signal);
    }

    /// Begin a quit for `reason`: refuse new work, let in-flight invocations finish for up to
    /// `grace`, say goodbye on every stream, then make the quit signal. Returns at once; a
    /// second call while a quit is under way does nothing.
    pub fn begin_quit(&self, reason: QuitReason, grace: Duration) {
        self.quit.begin(reason, grace);
    }

    /// The handle for a person's Quit and for the end of the session.
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
        for handle in &self.threads {
            handle.thread().unpark();
        }
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

/// How the service ends on request: refuse new work, drain what runs, decide every session's tabs
/// and say goodbye ([`Farewell`]), then make the quit signal that lets the process leave.
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
    /// A quit that sets `latch` for a person's Quit, decides the tabs of the workspaces in
    /// `workspaces` and closes released ones through `port`, and says goodbye on the MCP streams
    /// it is given and on the streams of `browser`.
    fn new(
        diagnostics: Arc<DiagnosticsHub>,
        latch: PathBuf,
        browser: Arc<RelayBrowserPort>,
        workspaces: WorkspaceStore,
        port: Arc<dyn BrowserPort>,
    ) -> Self {
        Self {
            drain: Arc::new(Drain::default()),
            signal: Arc::default(),
            diagnostics,
            latch,
            farewell: Arc::new(Farewell::new(browser, workspaces, port)),
        }
    }

    /// Begin a quit for `reason` unless one is under way: set the latch for a person's Quit,
    /// refuse new work, and once the work in flight has finished or `grace` has passed, end every
    /// session ([`Farewell::end`]) and make the quit signal.
    pub fn begin(&self, reason: QuitReason, grace: Duration) {
        if !self.drain.begin_quit() {
            return;
        }
        self.farewell.shells.close();
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
                farewell.end(reason);
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

    /// Leave because the session is ending: refuse new work, forget every tab without a browser
    /// command, say `session_end` goodbye on every stream, then make the quit signal. Returns at
    /// once, and neither waits for work in flight nor sets the latch, since a sign-out is not a
    /// person's Quit and the caller has already armed the fallback exit that bounds this stop.
    pub fn session_end(&self) {
        self.drain.begin_quit();
        self.farewell.shells.close();
        let signal = Arc::clone(&self.signal);
        let farewell = Arc::clone(&self.farewell);
        let spawned = thread::Builder::new()
            .name("ghostlight-session-end".into())
            .spawn(move || {
                farewell.end_session();
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

    /// Leave because no stream of any role was open for the idle limit: refuse new work, forget
    /// every tab without a browser command (no browser stream is open) and make the quit signal.
    /// Returns at once, and neither says goodbye (no stream is open) nor sets the latch, so the
    /// next edge, command or person starts the service again for its own reason. A quit or a
    /// session end already under way is left to finish alone.
    fn idle_exit(&self, idle: Duration) {
        if !self.drain.begin_quit() {
            return;
        }
        self.farewell.shells.close();
        self.farewell.forget_every_session();
        self.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::IDLE_EXIT,
            ghostlight_bridge::diagnostics::Level::Info,
            None,
            &format!("no stream of any role for {} s", idle.as_secs()),
        );
        if let Some(signal) = lock(&self.signal).clone() {
            signal();
        }
    }

    /// Record `wire` as a live MCP stream until the returned guard is dropped, so a quit hands it
    /// its last token and says goodbye on it.
    fn enroll(&self, wire: Arc<McpWire>) -> Enrolled {
        self.farewell.enroll(wire)
    }

    /// Release the workspace of a session whose stream closed, unless a service end has taken
    /// over every session; that end then decides the workspace.
    fn release_on_close(&self, workspace: &WorkspaceId) {
        self.farewell.release_on_close(workspace);
    }

    /// The Ghostlight shells attached to this service, which a quit says goodbye to and whose
    /// waiting acts it releases.
    fn shells(&self) -> Arc<Shells> {
        Arc::clone(&self.farewell.shells)
    }
}

impl WorkbenchQuitPort for Quit {
    fn quit(&self) {
        self.begin(QuitReason::Quit, QUIT_GRACE);
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

/// How every service end decides its sessions' tabs, once (ADR-0203 Decision 3), and the live
/// streams it says goodbye on before the service closes them.
///
/// Until an end takes over, a session whose stream closes releases its own workspace. An end takes
/// over before its first goodbye, and from then on it alone decides every workspace, so no release
/// races the goodbye or the process exit.
struct Farewell {
    state: Mutex<FarewellState>,
    /// Told when a release that a closing session began has finished.
    closed: Condvar,
    next: AtomicU64,
    browser: Arc<RelayBrowserPort>,
    shells: Arc<Shells>,
    workspaces: WorkspaceStore,
    /// The port released tabs are closed through.
    port: Arc<dyn BrowserPort>,
}

/// What a service end reads under one lock: the live MCP streams, whether an end took over, and
/// how many releases closing sessions began before it did.
#[derive(Default)]
struct FarewellState {
    streams: BTreeMap<u64, Arc<McpWire>>,
    taken_over: bool,
    releasing: usize,
}

impl Farewell {
    fn new(
        browser: Arc<RelayBrowserPort>,
        workspaces: WorkspaceStore,
        port: Arc<dyn BrowserPort>,
    ) -> Self {
        Self {
            state: Mutex::default(),
            closed: Condvar::new(),
            next: AtomicU64::new(0),
            browser,
            shells: Arc::default(),
            workspaces,
            port,
        }
    }

    /// Record one MCP stream until the guard is dropped.
    fn enroll(self: &Arc<Self>, wire: Arc<McpWire>) -> Enrolled {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.state).streams.insert(id, wire);
        Enrolled {
            farewell: Arc::clone(self),
            id,
        }
    }

    /// Take every session over for this end and return the live MCP streams. From now on a
    /// closing session releases nothing.
    fn take_over(&self) -> Vec<Arc<McpWire>> {
        let mut state = lock(&self.state);
        state.taken_over = true;
        state.streams.values().cloned().collect()
    }

    /// Release `workspace` and close its tabs, unless an end has taken over.
    fn release_on_close(&self, workspace: &WorkspaceId) {
        {
            let mut state = lock(&self.state);
            if state.taken_over {
                return;
            }
            state.releasing += 1;
        }
        let released = self.workspaces.release(workspace);
        cleanup_released_tabs(
            workspace.as_str(),
            &released,
            self.port.as_ref(),
            &self.workspaces,
            None,
        );
        lock(&self.state).releasing -= 1;
        self.closed.notify_all();
    }

    /// End every session for a quit with `reason`, in this order and under one [`END_LIMIT`]:
    ///
    /// 1. each MCP stream is handed its last resume token, which carries the session's tabs only
    ///    when its workspace stays bound;
    /// 2. the MCP and shell goodbyes;
    /// 3. the release of every workspace that ends: a command-line workspace at every end, and an
    ///    MCP workspace unless a replacement keeps it bound for its live edge's resume. The work of
    ///    each session that ends is cancelled first, and a workspace a call still holds at the
    ///    deadline is left as it is;
    /// 4. the browser goodbye.
    fn end(&self, reason: QuitReason) {
        let until = Instant::now() + END_LIMIT;
        let goodbye = goodbye_for(reason);
        let streams = self.take_over();
        let kept: HashSet<WorkspaceId> = if reason == QuitReason::Replace {
            streams
                .iter()
                .filter_map(|wire| wire.workspace())
                .filter(|workspace| !self.workspaces.is_owned(workspace))
                .collect()
        } else {
            HashSet::new()
        };
        for wire in &streams {
            let keeps_tabs = wire
                .workspace()
                .is_some_and(|workspace| kept.contains(&workspace));
            wire.hand_last_resume(&self.workspaces, keeps_tabs);
        }
        for wire in &streams {
            wire.goodbye(goodbye);
        }
        self.shells.say_goodbye(goodbye);
        self.release_all_but(&kept, &streams, until);
        self.browser.say_goodbye(goodbye);
    }

    /// End every session because the session is ending: forget every tab without a browser
    /// command, then say `session_end` on every stream.
    fn end_session(&self) {
        let streams = self.take_over();
        self.workspaces.forget_all();
        for wire in &streams {
            wire.goodbye(GoodbyeReason::SessionEnd);
        }
        self.shells.say_goodbye(GoodbyeReason::SessionEnd);
        self.browser.say_goodbye(GoodbyeReason::SessionEnd);
    }

    /// End every session when no stream is open to say goodbye on: forget every tab without a
    /// browser command.
    fn forget_every_session(&self) {
        self.take_over();
        self.workspaces.forget_all();
    }

    /// Release every workspace but `kept` and close its confirmed tabs before `until`, then wait
    /// until then for the releases that closing sessions began before the end took over.
    ///
    /// The work of each session that ends is cancelled first, so a running call can finish and
    /// give its workspace up. Workspaces no call holds are released before any is waited for.
    fn release_all_but(
        &self,
        kept: &HashSet<WorkspaceId>,
        streams: &[Arc<McpWire>],
        until: Instant,
    ) {
        let ending: Vec<WorkspaceId> = self
            .workspaces
            .ids()
            .into_iter()
            .filter(|workspace| !kept.contains(workspace))
            .collect();
        for live in streams.iter().filter_map(|wire| wire.live.get()) {
            if ending.contains(&live.workspace) {
                live.queue.stop(CancelCause::Quit);
            }
        }
        let mut held = Vec::new();
        for workspace in ending {
            match self.workspaces.release_unleased(&workspace, Instant::now()) {
                Some(released) => self.close_released(&workspace, &released, until),
                None => held.push(workspace),
            }
        }
        for workspace in held {
            if let Some(released) = self.workspaces.release_unleased(&workspace, until) {
                self.close_released(&workspace, &released, until);
            }
        }
        let state = lock(&self.state);
        let wait = until.saturating_duration_since(Instant::now());
        let _ = self
            .closed
            .wait_timeout_while(state, wait, |state| state.releasing > 0)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }

    /// Close the tabs `released` names before `until`.
    fn close_released(&self, workspace: &WorkspaceId, released: &ReleasedTabs, until: Instant) {
        cleanup_released_tabs(
            workspace.as_str(),
            released,
            self.port.as_ref(),
            &self.workspaces,
            Some(until),
        );
    }
}

/// One MCP stream recorded for the goodbye. Dropping it forgets the stream.
struct Enrolled {
    farewell: Arc<Farewell>,
    id: u64,
}

impl Drop for Enrolled {
    fn drop(&mut self) {
        lock(&self.farewell.state).streams.remove(&self.id);
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
    /// The places for `desktop` streams.
    desktops: Capacity,
    budget: QueueBudget,
    stop: Arc<AtomicBool>,
    /// The workbench's server, once `publish_ready` has started it. The desktop role reads its
    /// address and bearer from here.
    workbench_server: OnceLock<Arc<WorkbenchServer>>,
    /// Keeps the shell running whenever the desktop can show it.
    presence: Arc<Presence>,
    /// The open streams of every role, which the idle exit counts.
    streams: Arc<Streams>,
    /// Where the installation keeps its files.
    layout: install::layout::Layout,
    /// How each accepted `mcp` stream's source is read from its peer process.
    sources: SourceReader,
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
                // Only the opening is held here; the port counts the stream while it lives.
                let Some(_opening) = intake.streams.admit() else {
                    return listener::refuse(&connection, OpeningRefusal::ShuttingDown);
                };
                if listener::accept(&connection, &intake.accepted(Role::Browser)).is_ok() {
                    // The first frame on the stream is the extension's own hello.
                    let _ = intake.browser.attach(connection);
                    intake.workbench.refresh_live_parts();
                }
            }
            Role::Control => {
                let Some(_stream) = intake.streams.admit() else {
                    return listener::refuse(&connection, OpeningRefusal::ShuttingDown);
                };
                serve_control(connection, opening.control, intake);
            }
            Role::Mcp => {
                let Some(_stream) = intake.streams.admit() else {
                    return listener::refuse(&connection, OpeningRefusal::ShuttingDown);
                };
                if serve_mcp(connection, intake, &opening).is_err() {
                    eprintln!("MCP session ended after an invalid or interrupted exchange.");
                }
            }
            Role::Desktop => {
                let Some(_stream) = intake.streams.admit() else {
                    return listener::refuse(&connection, OpeningRefusal::ShuttingDown);
                };
                // A shell that goes away abruptly or sends a frame this build cannot read ends
                // its own stream and nothing else.
                let view = ShellView::from_channel(opening.channel.as_deref());
                let _ = serve_desktop(connection, intake, view);
            }
            Role::Command | Role::Unknown => {
                listener::refuse(&connection, OpeningRefusal::Unsupported);
            }
        },
    }
}

/// Serve one shell of this build on an accepted `desktop` stream.
///
/// The service has one desktop place. A shell that finds it taken is told `already_open` instead of
/// the workbench and the stream ends. The shell that holds the place is asked to show its window,
/// unless the newcomer said it shows only the tray: that is presence's own launch, made while the
/// shell it would have replaced was coming back, and nothing is there to show.
/// Otherwise the shell is handed the workbench address and its secret and the tray to draw. It is
/// then attached, so the window, notices and native acts of the workbench go to it, a quit says
/// goodbye to it, and presence knows a shell is there. If a request to show the window was waiting
/// for a shell, the shell is asked to show it. What the shell sends is a click on a tray entry, the
/// result of each act, that it cannot show a window, and that the person closed the window; the
/// first close that stays is explained once. The shell opens its own window, so an `open` click
/// only tells presence that a person has it open; a frame kind this build does not know is
/// skipped. A frame that does not decode ends the stream, and the shell is detached.
fn serve_desktop(
    connection: Connection,
    intake: &Arc<Intake>,
    view: Option<ShellView>,
) -> Result<()> {
    let Some(server) = intake.workbench_server.get() else {
        listener::refuse(&connection, OpeningRefusal::Unsupported);
        return Ok(());
    };
    let Some(_place) = intake.desktops.acquire(1) else {
        listener::refuse(&connection, OpeningRefusal::Capacity);
        return Ok(());
    };
    listener::accept(&connection, &intake.accepted(Role::Desktop))?;
    let mut reader =
        SocketReader::new(connection.try_clone()?).interrupted_by(Arc::clone(&intake.stop));
    let writer = Arc::new(SocketWriter::new(connection)?);
    let claim = match intake.quit.shells().claim() {
        Ok(claim) => claim,
        Err(occupied) => {
            if view != Some(ShellView::Tray) && occupied.show() {
                intake.presence.person_acted();
                intake.presence.window_shown();
            }
            writer.native(&DesktopFrame::AlreadyOpen)?;
            return Ok(());
        }
    };
    let tray = intake.workbench.tray_model();
    writer.native(&DesktopFrame::workbench(
        server.address().url(),
        server.bearer(),
        tray.clone(),
    ))?;
    let attached = claim.attach(writer, tray);
    let _present = ShellPresent::begin(&intake.presence);
    // Whether a reveal for the tray's Open is in flight: clicks that arrive meanwhile join it.
    let revealing = Arc::new(AtomicBool::new(false));
    if let Some(wanted) = intake.presence.attached(view) {
        attached.show(wanted.address);
    }
    // A change between the tray handed over and the shell being attached would have gone nowhere.
    attached.draw_tray(intake.workbench.tray_model());
    while let Some(frame) = reader.native::<DesktopFrame>(None)? {
        let item = match frame {
            DesktopFrame::Tray { item } => item,
            DesktopFrame::ActResult { id, outcome } => {
                attached.answer(id, outcome);
                continue;
            }
            DesktopFrame::Unavailable { reason } => {
                intake.presence.unavailable(reason.window_reason());
                continue;
            }
            DesktopFrame::Closed { stays } => {
                intake.presence.closed(stays);
                if stays {
                    explain_close(intake, &attached);
                }
                continue;
            }
            _ => continue,
        };
        match item {
            TrayItemId::Hold | TrayItemId::Resume => {
                if let Some(intent) = item.intent(intake.workbench.runtime_state()) {
                    intake
                        .workbench
                        .apply_runtime_control(intent, RuntimeSource::Tray);
                }
            }
            TrayItemId::Quit => intake.quit.begin(QuitReason::Quit, QUIT_GRACE),
            TrayItemId::Open => {
                intake.presence.person_acted();
                intake.presence.window_shown();
                // The shell shows its own window, where it last was. While setup is incomplete
                // the person is brought back to it, as a window opened fresh would be.
                if let Some(address) = intake.workbench.show_address() {
                    reveal_once(&intake.workbench, Some(address), &revealing);
                }
            }
        }
    }
    Ok(())
}

/// Tells presence that a shell is gone when it is dropped, however the stream ended.
struct ShellPresent<'a>(&'a Presence);

impl<'a> ShellPresent<'a> {
    fn begin(presence: &'a Presence) -> Self {
        Self(presence)
    }
}

impl Drop for ShellPresent<'_> {
    fn drop(&mut self) {
        self.0.detached();
    }
}

/// Say, once per home, that closing the window left Ghostlight running and where its icon is.
fn explain_close(intake: &Intake, shell: &shell::Attached) {
    match hint::first_close(&intake.layout.tray_hint()) {
        Ok(true) => shell.notice(TRAY_HINT_TITLE, TRAY_HINT_BODY),
        Ok(false) => {}
        Err(error) => intake.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::PROCESS_FAILED,
            ghostlight_bridge::diagnostics::Level::Warn,
            None,
            &format!("the tray hint marker could not be written: {error}"),
        ),
    }
}

/// Carries the tray the workbench describes to the attached shell on a thread of its own, so a
/// refresh of the live parts never waits on a stream.
struct TrayRelay {
    latest: Mutex<Option<TrayModel>>,
    thread: OnceLock<Thread>,
}

impl TrayRelay {
    /// Start the thread that writes each tray the workbench describes to the attached shell, until
    /// `stop` is set. The thread is unparked by whoever sets `stop`.
    fn spawn(
        shells: Arc<Shells>,
        stop: Arc<AtomicBool>,
    ) -> io::Result<(Arc<Self>, JoinHandle<()>)> {
        let relay = Arc::new(Self {
            latest: Mutex::new(None),
            thread: OnceLock::new(),
        });
        let carrying = Arc::clone(&relay);
        let handle = thread::Builder::new()
            .name("ghostlight-tray".into())
            .spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    let model = lock(&carrying.latest).take();
                    match model {
                        Some(model) => shells.send_tray(model),
                        None => thread::park(),
                    }
                }
            })?;
        let _ = relay.thread.set(handle.thread().clone());
        Ok((relay, handle))
    }
}

impl WorkbenchTrayPort for TrayRelay {
    fn tray_changed(&self, model: TrayModel) {
        *lock(&self.latest) = Some(model);
        if let Some(thread) = self.thread.get() {
            thread.unpark();
        }
    }
}

/// The answer to a control `status`: who this service is, what is connected and where the
/// workbench is. The workbench's bearer is never part of it.
fn control_status(intake: &Intake) -> control::Status {
    control::Status {
        service: env!("CARGO_PKG_VERSION").into(),
        instance: intake.instance.clone(),
        pid: std::process::id(),
        started_by: intake.started_by,
        connections: Some(intake.workbench.connection_counts()),
        workbench: intake
            .workbench_server
            .get()
            .map(|server| control::WorkbenchStatus {
                url: server.address().url().to_owned(),
                usual_port: server.address().usual(),
                watching: intake.workbench.watching(),
            }),
        window: intake.presence.window_reason(),
    }
}

/// Answer one control operation and close.
fn serve_control(connection: Connection, operation: Option<Control>, intake: &Intake) {
    let value = match operation {
        Some(Control::Readiness) => {
            serde_json::to_value(intake.workbench.readiness()).unwrap_or(Value::Null)
        }
        Some(Control::Status) => {
            serde_json::to_value(control_status(intake)).unwrap_or(Value::Null)
        }
        Some(Control::Show) => serde_json::to_value(control::Shown {
            shown: intake
                .workbench
                .reveal(intake.workbench.show_address())
                .is_ok(),
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
/// Once the opening is answered, the connection's attribution is minted with the accept time. The
/// app that started the session is read once from the stream's peer process when the session
/// opens ([`LiveSession::open`]): right after the accept for a resumed session, and after the
/// `initialize` answer is written for a fresh one. Neither answer waits on that read, and nothing
/// the client sends takes part in it.
/// A resume token continues an earlier session: its revision, client and owned tabs come back at
/// once, without waiting for any browser. Each tab is pending until its own browser lists a tab
/// with its id at the URL the session last knew, which the session's first call asks for
/// (ADR-0203 Decision 1). Until then the tab stays in every token the session hands out.
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
    let resumed = requested
        .resume
        .as_deref()
        .and_then(|token| ResumeState::decode(token).ok());
    listener::accept(&connection, &intake.accepted(Role::Mcp))?;
    let accepted = Accepted {
        attribution: Attribution::accepted(IntakeChannel::of_opening(requested.channel.as_deref())),
        peer: connection.peer_process_id(),
        marker,
    };
    let mut reader =
        SocketReader::new(connection.try_clone()?).interrupted_by(Arc::clone(&intake.stop));
    let writer = Arc::new(SocketWriter::new(connection)?);
    let (session, resumed_tabs) = match resumed {
        Some(state) => {
            let owned_tabs = state.owned_tabs.clone();
            (McpSession::resumed(state), Some(owned_tabs))
        }
        None => (McpSession::new(), None),
    };
    let wire = Arc::new(McpWire::new(Arc::clone(&writer), session));
    let _enrolled = intake.quit.enroll(Arc::clone(&wire));

    let mut live = match resumed_tabs {
        Some(owned_tabs) => {
            let live = LiveSession::open(intake, &wire, &accepted)?;
            remember_resumed_tabs(&intake.workspaces, &live.workspace, &owned_tabs);
            wire.publish_resume(&intake.workspaces, &live.workspace);
            Some(live)
        }
        None => None,
    };

    // Every way this connection can end has to reach the teardown below. A reset socket, an
    // oversized frame and one malformed frame are all ordinary ways for a client to go away.
    let served = (|| -> Result<()> {
        while let Some(frame) = reader.native::<McpFrame>(None)? {
            let McpFrame::Message { line } = frame else {
                continue;
            };
            let mut opened = false;
            // A fresh session opens once it is initialized and only after the lines written so
            // far, so its `initialize` answer never waits on the session's source read.
            let mut open_when_initialized = |live: &mut Option<LiveSession>| -> Result<()> {
                if live.is_none() && wire.is_initialized() {
                    *live = Some(LiveSession::open(intake, &wire, &accepted)?);
                    opened = true;
                }
                Ok(())
            };
            for step in wire.accept(&line) {
                match step {
                    Step::Reply(reply) => wire.send(&reply),
                    Step::Forward(request) => {
                        open_when_initialized(&mut live)?;
                        if let Some(live) = &live {
                            if !live.handle(intake, &wire, request) {
                                return Ok(());
                            }
                        }
                    }
                }
            }
            open_when_initialized(&mut live)?;
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

/// An accepted `mcp` stream: what the service knew of it when it answered the opening.
struct Accepted {
    /// The connection's identity, accept time and declared channel, still without its source.
    attribution: Attribution,
    /// The peer's process id, which the source is read from when the session opens.
    peer: Option<u32>,
    /// The session owner the opening declared, for the command line.
    marker: Option<SessionMarker>,
}

/// One session's engine and the stream its lines go out on.
struct McpWire {
    writer: Arc<SocketWriter>,
    session: Mutex<McpSession>,
    resume: Mutex<Resume>,
    /// The session's workspace and queue once it opened, which a service end decides.
    live: OnceLock<LiveEnd>,
}

/// The resume token a stream was last handed, and whether a service end handed it its last.
#[derive(Default)]
struct Resume {
    last: Option<String>,
    /// Nothing is published once a service end has handed the stream its last token.
    sealed: bool,
}

/// What a service end needs of an opened session: its workspace, and its queue to cancel.
struct LiveEnd {
    workspace: WorkspaceId,
    queue: Arc<SessionQueue>,
}

impl McpWire {
    fn new(writer: Arc<SocketWriter>, session: McpSession) -> Self {
        Self {
            writer,
            session: Mutex::new(session),
            resume: Mutex::default(),
            live: OnceLock::new(),
        }
    }

    fn accept(&self, line: &str) -> Vec<Step> {
        lock(&self.session).accept(line)
    }

    fn is_initialized(&self) -> bool {
        lock(&self.session).is_initialized()
    }

    /// The name the session's client claimed, if any.
    fn claim(&self) -> Option<String> {
        lock(&self.session).claim().map(str::to_owned)
    }

    /// The workspace of the session, once it opened.
    fn workspace(&self) -> Option<WorkspaceId> {
        self.live.get().map(|live| live.workspace.clone())
    }

    /// Write one MCP line to the client.
    fn send(&self, line: &Value) {
        let _ = self.writer.native(&McpFrame::Message {
            line: line.to_string(),
        });
    }

    /// Say goodbye with `reason` on this stream.
    fn goodbye(&self, reason: GoodbyeReason) {
        let _ = self.writer.native(&McpFrame::Goodbye { reason });
    }

    /// Render one answer from the session queue and write its lines.
    fn render(&self, response: ServiceResponse) {
        let lines = lock(&self.session).render(response);
        for line in &lines {
            self.send(line);
        }
    }

    /// Hand the edge a fresh resume token when what it would restore has changed: a binding was
    /// confirmed or forgotten, or its URL moved. Once a service end handed the stream its last
    /// token, nothing more is published.
    ///
    /// Each tab is written with its own binding's browser, a pending one included, so a call that
    /// completes before that browser returns keeps the tab in the token. A tab carries the digest
    /// of its URL, never the URL, so the edge holds no page address.
    fn publish_resume(&self, workspaces: &WorkspaceStore, workspace: &WorkspaceId) {
        let mut resume = lock(&self.resume);
        if !resume.sealed {
            self.write_resume(&mut resume, owned_tabs(workspaces, workspace));
        }
    }

    /// Hand the edge the last resume token of this service, with the session's tabs only when
    /// `keeps_tabs` says its workspace stays bound, and publish nothing after it.
    ///
    /// A session that ends at a quit resumes later with no tab, so its handles read as no longer
    /// open instead of rebinding tabs the release left behind.
    fn hand_last_resume(&self, workspaces: &WorkspaceStore, keeps_tabs: bool) {
        let mut resume = lock(&self.resume);
        if resume.sealed {
            return;
        }
        let tabs = match self.workspace() {
            Some(workspace) if keeps_tabs => owned_tabs(workspaces, &workspace),
            _ => Vec::new(),
        };
        self.write_resume(&mut resume, tabs);
        resume.sealed = true;
    }

    /// Write the token that restores `owned_tabs`, unless it is the one last written or the
    /// session is not initialized yet.
    fn write_resume(&self, resume: &mut Resume, owned_tabs: Vec<OwnedTab>) {
        let Some(state) = lock(&self.session).resume_state(owned_tabs) else {
            return;
        };
        let token = state.encode();
        if resume.last.as_deref() == Some(token.as_str()) {
            return;
        }
        if self
            .writer
            .native(&McpFrame::Resume {
                token: token.clone(),
            })
            .is_ok()
        {
            resume.last = Some(token);
        }
    }
}

/// The tabs a resume token restores for `workspace`: every binding, a pending one included, by
/// its own browser and the digest of its URL.
fn owned_tabs(workspaces: &WorkspaceStore, workspace: &WorkspaceId) -> Vec<OwnedTab> {
    workspaces
        .tab_bindings(workspace.as_str())
        .into_iter()
        .map(|binding| OwnedTab {
            browser_id: binding.browser,
            tab_id: binding.physical_id,
            handle: binding.handle,
            url_digest: Some(binding.url.as_str().to_owned()),
        })
        .collect()
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

/// The diagnostics detail of a session that attached: its connection id, its channel and the file
/// name of the app that started it, when one was seen. Never the client's claim (ADR-0161).
fn attached(attribution: &Attribution) -> String {
    let connection = attribution.connection_id.as_deref().unwrap_or_default();
    let channel = attribution.channel.as_str();
    match &attribution.source {
        Some(source) => format!("{connection} via {channel} from {}", source.image),
        None => format!("{connection} via {channel}"),
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
    ///
    /// This is where the connection's source is read, once, and its one evidence is built.
    fn open(intake: &Arc<Intake>, wire: &Arc<McpWire>, accepted: &Accepted) -> Result<Self> {
        // Before opening anything, release workspaces whose owner is gone and close the tabs they
        // still hold. Sweeping here rather than on a timer keeps the cost proportional to use.
        reap_finished_sessions(&intake.workspaces, intake.browser_port.as_ref());
        // Evidence belongs to this connection. Workspace continuity never replaces it.
        let attribution = accepted
            .attribution
            .clone()
            .observed(accepted.peer, &intake.sources);
        let connection = Arc::new(ConnectionEvidence::new(
            attribution,
            wire.claim().as_deref(),
        ));
        let workspace = intake
            .workspaces
            .connect(connection.clone(), accepted.marker.clone());
        intake.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::HARNESS_ATTACHED,
            ghostlight_bridge::diagnostics::Level::Info,
            None,
            &attached(connection.attribution()),
        );
        let queue = Arc::new(SessionQueue::new(
            intake.budget.clone(),
            Arc::clone(&intake.quit.drain),
        ));
        let advertised = Arc::new(AdvertisedCatalog::default());
        let watch_stop = Arc::new(AtomicBool::new(false));
        let _ = wire.live.set(LiveEnd {
            workspace: workspace.clone(),
            queue: Arc::clone(&queue),
        });
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

    /// The once-a-second thread that advances cancelled work, keeps waiting work visible, hands
    /// the client a fresh resume token when a tab changed outside its calls, and tells the client
    /// when policy changes the catalog it was given.
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
                    for request in queue.admitted() {
                        executor.show_waiting(&request);
                    }
                    wire.publish_resume(&intake.workspaces, &workspace);
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
                while let Some(request) = queue.next(independent) {
                    let result = executor.execute_prepared(&request, Admission::Queued);
                    deliver_result(&wire, result);
                    wire.publish_resume(&workspaces, &workspace);
                    queue.complete(&request.id);
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
            ServiceRequest::Invoke { id, tool, input } => {
                // Bound retained payloads as well as queue count. Decoding occurs once at intake.
                let bytes = serde_json::to_vec(&input).map_or(0, |input| input.len())
                    + id.as_str().len()
                    + tool.len()
                    + QUEUED_METADATA_BYTES;
                let request = Arc::new(PreparedInvocation::new(
                    id,
                    &tool,
                    input,
                    self.workspace.clone(),
                    Arc::clone(&self.connection),
                ));
                match self.queue.admit(Arc::clone(&request), bytes) {
                    Ok(()) => {}
                    Err(Refused::Full) => match self.queue.begin_unqueued() {
                        Ok(_flight) => {
                            let result = intake
                                .executor
                                .execute_prepared(&request, Admission::OverCapacity);
                            deliver_result(wire, result);
                            wire.publish_resume(&intake.workspaces, &self.workspace);
                        }
                        Err(_) => refuse_while_quitting(wire, request.id.clone()),
                    },
                    Err(Refused::ShuttingDown) => refuse_while_quitting(wire, request.id.clone()),
                    Err(Refused::Closed) => return false,
                }
            }
            ServiceRequest::Cancel { id } => self.queue.cancel(&id),
        }
        true
    }

    /// Stop the workers, detach the connection, and release a workspace nobody owns unless a
    /// service end has taken over, which then decides it.
    ///
    /// The work still admitted is cancelled because Ghostlight quit, while a quit drains, and
    /// because the session ended otherwise.
    fn close(self, intake: &Intake) {
        self.watch_stop.store(true, Ordering::SeqCst);
        self.queue.stop(if intake.quit.drain.is_quitting() {
            CancelCause::Quit
        } else {
            CancelCause::SessionEnded
        });
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
        // next call must reach the same tabs. It is released when its owner is gone or the
        // service ends, not when a socket is.
        if !intake.workspaces.is_owned(&self.workspace) {
            intake.quit.release_on_close(&self.workspace);
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

/// Bring a resumed session's tabs back as pending bindings, each in its own browser.
///
/// No browser is asked here: the session's first call confirms each tab against its browser's
/// own list (ADR-0203 Decision 1). A tab written without a URL digest, by a build before digests,
/// cannot be confirmed and is not brought back, so its handle names no open tab.
fn remember_resumed_tabs(workspaces: &WorkspaceStore, workspace: &WorkspaceId, owned: &[OwnedTab]) {
    for tab in owned {
        if let Some(url) = tab.url_digest.as_deref().and_then(UrlDigest::parse) {
            workspaces.remember_tab(workspace, &tab.handle, &tab.browser_id, tab.tab_id, url);
        }
    }
}

/// Answer an invocation that arrived after a quit began.
fn refuse_while_quitting(wire: &McpWire, id: InvocationId) {
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

/// The environment variable a debug build reads so a CI runner whose job token is elevated can
/// run the journeys. The value must be exactly `1`. Release builds never read it.
pub const ELEVATION_WAIVER_VARIABLE: &str = "GHOSTLIGHT_TEST_ALLOW_ELEVATED";

/// Refuse to serve, or to start a service, from an elevated token. `elevated` reads the token of
/// the calling process; a token that cannot be read is refused too. A debug build with
/// [`ELEVATION_WAIVER_VARIABLE`] set to `1` lets an elevated token through.
pub(crate) fn refuse_elevated_token(elevated: impl FnOnce() -> io::Result<bool>) -> Result<()> {
    decide_elevation(elevated(), elevation_waived())
}

/// Whether this process may run with an elevated token: true only in a debug build whose
/// environment sets [`ELEVATION_WAIVER_VARIABLE`] to exactly `1`.
fn elevation_waived() -> bool {
    cfg!(debug_assertions)
        && std::env::var(ELEVATION_WAIVER_VARIABLE).is_ok_and(|value| value == "1")
}

/// The refusal as a pure decision over the token answer and the waiver. Only an elevated answer
/// can be waived; a token that cannot be read is refused either way.
fn decide_elevation(token: io::Result<bool>, waived: bool) -> Result<()> {
    if token.context("read the process token")? && !waived {
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

/// Register every browser that has no Ghostlight registration, when `executable` is the installed
/// service of `home`. A person who installed through a route that never registered a browser gets
/// one on the first start; a registration that exists is the installer's to reclaim, never this
/// start's.
fn register_missing_browsers(home: &Path, executable: &Path) {
    if !install::reconcile::native_hosts_allowed() {
        return;
    }
    let layout = install::layout::Layout::new(home);
    let executable = install::entry::canonical_path(executable);
    let serving = install::entry::same_path(
        &install::entry::canonical_path(&layout.service_executable()),
        &executable,
    );
    if serving {
        let _ = install::native_host::NativeHostRegistry::discover().register_missing();
    }
}

/// Close the tabs of every session whose owning process is gone.
fn reap_finished_sessions(workspaces: &WorkspaceStore, browser: &dyn BrowserPort) {
    for released in workspaces.reap(&owner_alive) {
        cleanup_released_tabs("reaped", &released, browser, workspaces, None);
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
///
/// Each command has [`RELEASE_CALL_LIMIT`]. A service end also bounds the whole cleanup by
/// `until`: a command that would start after it is not sent, and its tab leaves the pool as one
/// whose close did not answer.
fn cleanup_released_tabs(
    workspace: &str,
    released: &ReleasedTabs,
    browser: &dyn BrowserPort,
    workspaces: &WorkspaceStore,
    until: Option<Instant>,
) {
    let Some(target) = released.browser.as_deref() else {
        return;
    };
    let call_deadline = || {
        let now = Instant::now();
        let deadline = now + RELEASE_CALL_LIMIT;
        match until {
            Some(until) if until <= now => None,
            Some(until) => Some(deadline.min(until)),
            None => Some(deadline),
        }
    };
    let cancelled = AtomicBool::new(false);
    for tab_ids in released.physical_ids.chunks(DIAGNOSTIC_CLEAR_BATCH_SIZE) {
        let Some(deadline) = call_deadline() else {
            break;
        };
        let _ = browser.call(
            target,
            workspace,
            BrowserCommand::ClearDiagnostics {
                tab_ids: tab_ids.to_vec(),
            },
            deadline,
            &cancelled,
        );
    }
    for &tab_id in &released.physical_ids {
        let kept_open = call_deadline().is_some_and(|deadline| {
            let closed = browser.call(
                target,
                workspace,
                BrowserCommand::CloseTab {
                    tab_id,
                    released: true,
                },
                deadline,
                &cancelled,
            );
            matches!(closed, Err(BrowserError::LocalInterlock(_)))
        });
        workspaces.settle_released(target, tab_id, kept_open);
    }
}

/// Whether the process that owns a session is still running.
///
/// A declared key names no process, so it lives until the service exits, by a quit or by the idle
/// exit ([`idle`]). The workspace registry is in memory, so the key goes with the service.
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

/// Hand one finished invocation to the session as a result for the request it names, or end the
/// stream when the result cannot be encoded.
fn deliver_result(wire: &McpWire, mut result: crate::work::result::InvocationResult) {
    let text = result.model_text();
    let is_error = result.is_error();
    let content = std::mem::take(&mut result.content);
    match serde_json::to_value(&result) {
        Ok(value) => wire.render(ServiceResponse::Result {
            id: result.invocation,
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

/// The name of the short-lived thread that carries out a request to show the window.
const SHOW_REQUEST_THREAD: &str = "ghostlight-show-request";

/// Applies what a connected browser reports: its tabs, its dialogs and its toolbar's pause, and
/// tells the workbench when who is connected may have changed.
struct ServiceBrowserEvents {
    workbench: WorkbenchFacade,
    workspaces: WorkspaceStore,
    /// Whether a show request is being carried out. One request in flight absorbs the presses
    /// that arrive meanwhile.
    showing: Arc<AtomicBool>,
}

impl ServiceBrowserEvents {
    fn new(workbench: WorkbenchFacade, workspaces: WorkspaceStore) -> Self {
        Self {
            workbench,
            workspaces,
            showing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Carry out the browser's request to be shown without holding the browser's reader.
    ///
    /// Revealing can wait for a shell to start and attach, and the reader that delivered the
    /// request also delivers the receipts of work in flight on that browser. The answer is not
    /// read: with nothing to show, or nothing to show it, the person is no worse off. While setup
    /// is incomplete the window opens at the setup view.
    fn request_show(&self) {
        reveal_once(
            &self.workbench,
            self.workbench.show_address(),
            &self.showing,
        );
    }
}

/// Reveal the workbench at `address` off the calling thread, unless a reveal guarded by
/// `in_flight` is already under way. One reveal in flight absorbs the requests that arrive
/// meanwhile, so repeated presses do not stack threads or windows.
fn reveal_once(
    workbench: &WorkbenchFacade,
    address: Option<PageAddress>,
    in_flight: &Arc<AtomicBool>,
) {
    if in_flight.swap(true, Ordering::SeqCst) {
        return;
    }
    let done = Arc::clone(in_flight);
    let spawned = reveal_off_thread(workbench.clone(), address, move || {
        done.store(false, Ordering::SeqCst);
    });
    if spawned.is_err() {
        in_flight.store(false, Ordering::SeqCst);
    }
}

/// Reveal the workbench at `address` on a short-lived thread, then run `finished`.
///
/// Revealing can wait for a shell to start and attach, so a reader that must keep reading never
/// does it inline. The answer is not read: with nothing to show, or nothing to show it, the
/// person is no worse off. `finished` does not run when the thread cannot be started.
fn reveal_off_thread(
    workbench: WorkbenchFacade,
    address: Option<PageAddress>,
    finished: impl FnOnce() + Send + 'static,
) -> std::io::Result<()> {
    thread::Builder::new()
        .name(SHOW_REQUEST_THREAD.into())
        .spawn(move || {
            let _ = workbench.reveal(address);
            finished();
        })
        .map(|_| ())
}

impl BrowserEventSink for ServiceBrowserEvents {
    fn on_event(&self, browser: &str, event: BrowserEvent) {
        match event {
            BrowserEvent::DocumentCommitted { tab_id, url } => {
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
            BrowserEvent::ShowRequested => self.request_show(),
            BrowserEvent::TabClosed { tab_id } => {
                self.workspaces.apply_browser_close(browser, tab_id);
                self.workbench.refresh_live_parts();
            }
            // A browser that comes back may have restarted and reused its tab ids, so every tab
            // binding it held waits for it to confirm it.
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
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::intake::session::{IntakeChannel, SessionMarker};
    use crate::provenance::{Attribution, ConnectionEvidence, Source, SourceReader};
    use ghostlight_relay::endpoint::{pair, Connection, Endpoint, Listener};
    use ghostlight_relay::frame;
    use ghostlight_relay::protocol::{
        self, Control, Edge, GoodbyeReason, McpFrame, Opening as RelayOpening, Program, QuitReason,
        RefusalCode, Reply, Role, StartReason,
    };
    use serde_json::{json, Value};

    use super::lifetime::HandClock;
    use super::presence::testing::Fake;
    use super::test_home::TestHome;
    use crate::browser::adapter::BrowserCommand;
    use crate::browser::adapter::RuntimeControlIntent;
    use crate::browser::adapter::RuntimeControlState;
    use crate::browser::testing::{FakeBrowser, FAKE_BROWSER};
    use crate::cli::client as cli_client;
    use crate::intake::desktop::{
        ActOutcome, DesktopFrame, NativeAct, ShellView, TrayItemId, TrayModel,
    };
    use crate::intake::listener::{self, control};
    use crate::intake::mcp::failure_templates;
    use crate::intake::mcp::ResumeState;
    use crate::language::intake::OpeningRefusal;
    use crate::scratch::Scratch;
    use crate::workbench::{
        PageAddress, RuntimeSource, WorkbenchNativeActs, WorkbenchNotification,
        WorkbenchPresentationError, WorkbenchPresentationPort,
    };
    use crate::workspace::ReleasedTabs;

    use super::{
        cleanup_released_tabs, AuthorityAlreadyRunning, QuitLatched, ServiceHost, StartTerms,
        QUIT_GRACE,
    };

    /// The terms of an agent start, which asks for the lock once.
    fn agent() -> StartTerms {
        StartTerms::asking_once(StartReason::Agent)
    }

    /// A fresh directory to serve from, removed when the returned guard drops.
    fn temp_home(name: &str) -> Scratch {
        Scratch::named(&format!("service-{name}"))
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
        let governance = GovernanceFacade::new(Some(policy));
        let workspaces = WorkspaceStore::default();
        let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp);
        let tab = PhysicalTab {
            tab_id: 7,
            url: "https://allowed.example/".into(),
            title: "test".into(),
            active: true,
            readiness: BrowserReadiness::Complete,
        };
        let lease = workspaces.acquire(&workspace).unwrap();
        let selected = lease.add_tab(FAKE_BROWSER, &tab).unwrap();
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
        let sink = super::ServiceBrowserEvents::new(view.clone(), workspaces.clone());

        let initial_audit = std::fs::read_to_string(&audit_path).unwrap_or_default();

        // A commit never authorizes, audits or reaches the executor, whoever caused it.
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
                },
            );
            let current = workspaces
                .acquire(&workspace)
                .unwrap()
                .select_tab(Some(selected.handle.as_str()))
                .unwrap();
            assert_eq!(current.url, url);
            assert!(current.generation > selected.generation);
            assert!(view.snapshot().history.is_empty());
            assert!(view.snapshot().operations.is_empty());
            assert!(browser.calls().is_empty());
            assert!(browser.control_states().is_empty());
            assert_eq!(
                std::fs::read_to_string(&audit_path).unwrap_or_default(),
                initial_audit
            );
        }
        // A tab Ghostlight never opened stays entirely unowned when it commits.
        sink.on_event(
            FAKE_BROWSER,
            BrowserEvent::DocumentCommitted {
                tab_id: 8,
                url: "chrome://version".into(),
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
            },
        );
        let result = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":selected.handle.as_str()}),
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
    }

    /// The name the client of the session whose tabs the release cleanup tests leave behind claims.
    const RELEASED_CLIENT: &str = "released";

    /// Admit a workspace in the fake browser, land one example.com tab per id, and release it.
    fn release_example_tabs(
        workspaces: &crate::workspace::WorkspaceStore,
        ids: &[u64],
    ) -> ReleasedTabs {
        use crate::browser::adapter::{BrowserReadiness, PhysicalTab};

        let workspace = workspaces.admit(RELEASED_CLIENT.into(), IntakeChannel::Mcp);
        let lease = workspaces.acquire(&workspace).unwrap();
        for &tab_id in ids {
            let tab = PhysicalTab {
                tab_id,
                url: "https://example.com/".into(),
                title: "Example".into(),
                active: true,
                readiness: BrowserReadiness::Complete,
            };
            let added = lease.add_tab(FAKE_BROWSER, &tab).unwrap();
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
        cleanup_released_tabs("released", released, &browser, workspaces, None);
    }

    /// The tab a later session of the same client opening example.com would reuse, if any.
    fn reusable_example_tab(workspaces: &crate::workspace::WorkspaceStore) -> Option<u64> {
        let key = ConnectionEvidence::claimed(RELEASED_CLIENT, IntakeChannel::Mcp)
            .reuse_key()
            .expect("the claim names an agent");
        workspaces
            .claim_released(FAKE_BROWSER, &key, "https://example.com/")
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
        cleanup_released_tabs("released", &released, &browser, &workspaces, None);

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
        let lease = workspaces.acquire(&workspace).unwrap();
        lease
            .add_tab(
                FAKE_BROWSER,
                &PhysicalTab {
                    tab_id: 4,
                    url: "https://example.com/".into(),
                    title: "Example".into(),
                    active: true,
                    readiness: BrowserReadiness::Complete,
                },
            )
            .unwrap();
        drop(lease);
        let sink =
            super::ServiceBrowserEvents::new(test_workbench(&workspaces), workspaces.clone());
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
        let sink =
            super::ServiceBrowserEvents::new(test_workbench(&workspaces), workspaces.clone());
        let closed = |tab_id| BrowserEvent::TabClosed { tab_id };
        // The person loaded a page in a tab Ghostlight left open: it is theirs again.
        let committed = |tab_id| BrowserEvent::DocumentCommitted {
            tab_id,
            url: "https://example.com/mine".into(),
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
    }

    /// A browser that restarted may have handed the agent's tab id to the person's tab. With
    /// preserve-tabs off the extension closes any id it is sent, so a release after the gap sends
    /// no close for a binding the browser has not confirmed.
    #[test]
    fn a_release_after_a_browser_restart_never_closes_the_persons_tab() {
        use crate::browser::adapter::{BrowserEvent, BrowserReadiness, PhysicalTab};
        use crate::browser::BrowserEventSink;
        use crate::workspace::WorkspaceStore;

        let workspaces = WorkspaceStore::default();
        let sink =
            super::ServiceBrowserEvents::new(test_workbench(&workspaces), workspaces.clone());
        let workspace = workspaces.admit(RELEASED_CLIENT.into(), IntakeChannel::Mcp);
        let lease = workspaces.acquire(&workspace).unwrap();
        let tab = PhysicalTab {
            tab_id: 4,
            url: "https://example.com/".into(),
            title: "Example".into(),
            active: true,
            readiness: BrowserReadiness::Complete,
        };
        let added = lease.add_tab(FAKE_BROWSER, &tab).unwrap();
        lease.apply_landing(&added.handle, &tab).unwrap();
        drop(lease);

        sink.on_event(FAKE_BROWSER, BrowserEvent::Disconnected);
        let released = workspaces.release(&workspace);
        let browser = FakeBrowser::default();
        cleanup_released_tabs("released", &released, &browser, &workspaces, None);
        assert!(browser.calls().is_empty(), "{:?}", browser.calls());
        assert_eq!(reusable_example_tab(&workspaces), None);
    }

    /// A hello that registers a browser id again replaces its connection, whether or not the old
    /// stream's end was seen, so the bindings in that browser wait for it to confirm them.
    #[test]
    fn a_registration_that_replaces_a_connection_marks_its_bindings_pending() {
        use crate::browser::adapter::{BrowserReadiness, PhysicalTab};

        let home = TestHome::new("registration-pending");
        let served = serve_relayed(&home);
        let workspaces = served.intake.workspaces.clone();
        served
            .relay
            .set_event_sink(Arc::new(super::ServiceBrowserEvents::new(
                test_workbench(&workspaces),
                workspaces.clone(),
            )));
        let _first = attach_returning_browser(&served);
        let workspace = workspaces.admit(RELEASED_CLIENT.into(), IntakeChannel::Mcp);
        let lease = workspaces.acquire(&workspace).unwrap();
        let tab = PhysicalTab {
            tab_id: RETURNING_TAB,
            url: RETURNING_URL.into(),
            title: "Example".into(),
            active: true,
            readiness: BrowserReadiness::Complete,
        };
        let added = lease.add_tab(RETURNING_BROWSER, &tab).unwrap();
        lease.apply_landing(&added.handle, &tab).unwrap();
        drop(lease);
        assert!(!workspaces.tab_bindings(workspace.as_str())[0].pending);

        let _second = attach_returning_browser(&served);
        let bindings = workspaces.tab_bindings(workspace.as_str());
        assert_eq!(bindings.len(), 1);
        assert!(bindings[0].pending, "{bindings:?}");
    }

    /// A workbench over `workspaces` and a fresh authority, with no browser connected.
    fn test_workbench(
        workspaces: &crate::workspace::WorkspaceStore,
    ) -> crate::workbench::WorkbenchFacade {
        crate::workbench::WorkbenchFacade::new(
            crate::workbench::WorkbenchProjection::default(),
            workspaces.clone(),
            crate::governance::GovernanceFacade::new(None),
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
        let sink = super::ServiceBrowserEvents::new(workbench.clone(), workspaces);
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
    fn a_show_request_from_the_browser_reveals_the_workbench_once() {
        use crate::browser::adapter::BrowserEvent;
        use crate::browser::BrowserEventSink;

        let workspaces = crate::workspace::WorkspaceStore::default();
        let workbench = test_workbench(&workspaces);
        let sink = super::ServiceBrowserEvents::new(workbench.clone(), workspaces);

        // Nothing is attached yet, and that is not an error.
        sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
        eventually(|| !sink.showing.load(Ordering::SeqCst));

        let reveals = Arc::new(RevealCounter::default());
        workbench.attach_presentation(reveals.clone());
        sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
        eventually(|| reveals.0.load(Ordering::SeqCst) == 1);
    }

    #[test]
    fn a_show_request_opens_the_window_at_setup_until_setup_is_complete() {
        use crate::browser::adapter::BrowserEvent;
        use crate::browser::BrowserEventSink;
        use crate::install::setup_state::{self, SetupRecord};
        use crate::workbench::setup::SetupProgress;

        let home = TestHome::new("show-address");
        let record = home.path().join("setup.json");
        let workspaces = crate::workspace::WorkspaceStore::default();
        let workbench = test_workbench(&workspaces);
        workbench.attach_setup(SetupProgress::load(&record));
        let revealed = Arc::new(RevealedAt::default());
        workbench.attach_presentation(revealed.clone());
        let sink = super::ServiceBrowserEvents::new(workbench.clone(), workspaces);

        sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
        eventually(|| revealed.addresses().len() == 1);
        assert_eq!(revealed.addresses(), [Some(PageAddress::Setup)]);

        // A home whose setup is complete shows the window where it last was.
        assert!(setup_state::record_if_absent(&record, &SetupRecord::now()).unwrap());
        workbench.attach_setup(SetupProgress::load(&record));
        eventually(|| !sink.showing.load(Ordering::SeqCst));
        sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
        eventually(|| revealed.addresses().len() == 2);
        assert_eq!(revealed.addresses(), [Some(PageAddress::Setup), None]);
    }

    #[test]
    fn a_show_request_that_waits_does_not_hold_the_browser_link() {
        use crate::browser::adapter::{BrowserEvent, RuntimeControlIntent};
        use crate::browser::BrowserEventSink;
        use crate::language::workbench::PAUSED_FROM_BROWSER;

        let workspaces = crate::workspace::WorkspaceStore::default();
        let workbench = test_workbench(&workspaces);
        let heard = Arc::new(HeardChanges::default());
        workbench.attach_events(heard.clone());
        let held = Arc::new(HeldReveal::default());
        workbench.attach_presentation(held.clone());
        let sink = super::ServiceBrowserEvents::new(workbench, workspaces);

        // The reveal waits, and the sink still returns and applies the next event.
        sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
        eventually(|| held.started.load(Ordering::SeqCst) == 1);
        sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
        sink.on_event(
            FAKE_BROWSER,
            BrowserEvent::RuntimeControlRequested {
                intent: RuntimeControlIntent::Hold,
            },
        );
        assert_eq!(heard.notes().pop(), Some(Some(PAUSED_FROM_BROWSER.into())));

        // The press that arrived while one was waiting was absorbed by it.
        held.release.store(true, Ordering::SeqCst);
        eventually(|| !sink.showing.load(Ordering::SeqCst));
        assert_eq!(held.started.load(Ordering::SeqCst), 1);
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
        serve(home, false, SourceReader::system())
    }

    /// Serve the relay listener over a fake browser, reading each session's source with
    /// `sources`.
    fn serve_reading(home: &TestHome, sources: SourceReader) -> Served {
        serve(home, false, sources)
    }

    /// Serve the relay listener with the relay port as the session's browser port, so sessions
    /// meet browsers only as protocol-4 extensions attach, for their calls as well as their
    /// resumed tabs.
    fn serve_relayed(home: &TestHome) -> Served {
        serve(home, true, SourceReader::system())
    }

    /// Serve the relay listener in `home`; `relayed` puts the relay port behind the sessions, and
    /// `sources` reads each session's source.
    fn serve(home: &TestHome, relayed: bool, sources: SourceReader) -> Served {
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
        let port: Arc<dyn crate::browser::BrowserPort> = if relayed {
            relay.clone()
        } else {
            browser.clone()
        };
        let workspaces = WorkspaceStore::default();
        let governance = GovernanceFacade::new(None);
        let diagnostics = crate::diagnostics::DiagnosticsHub::for_tests();
        let projection = WorkbenchProjection::default();
        let audit = Arc::new(AuditRecorder::new(
            Arc::new(JsonlAuditSink::new(&home.path().join("audit.jsonl"))),
            projection.clone(),
        ));
        let executor = Arc::new(ApplicationExecutor::new(
            governance.clone(),
            workspaces.clone(),
            port.clone(),
            PresentationReactor::new(Arc::new(BrowserPresentation::new(
                port.clone(),
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
            workspaces.clone(),
            port.clone(),
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
            browser_port: port,
            browser: relay.clone(),
            workbench,
            governance,
            diagnostics,
            quit,
            sessions: super::Capacity::new(super::admission::SESSIONS),
            desktops: super::Capacity::new(super::admission::DESKTOPS),
            budget: super::QueueBudget::default(),
            stop: Arc::default(),
            workbench_server: std::sync::OnceLock::new(),
            presence: Arc::new(super::Presence::new(Arc::new(Fake::default()).world())),
            streams: super::Streams::new(Arc::new(super::SystemClock), super::idle::IDLE_EXIT),
            layout: crate::install::layout::Layout::new(home.path()),
            sources,
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
        opening.channel = Some(protocol::MCP_CHANNEL.into());
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
        assert_eq!(state.claim.as_deref(), Some("relay test"));

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
    fn the_result_a_client_receives_names_the_invocation_its_receipt_names() {
        let home = TestHome::new("mcp-invocation");
        let _served = serve_fake(&home);
        let mut socket = open_mcp(None);
        initialize(&mut socket);

        let mut invocations = Vec::new();
        for id in [2, 3] {
            send_line(
                &mut socket,
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "tools/call",
                    "params": {"name": "browser_tabs", "arguments": {"action": "list"}}
                }),
            );
            let (called, _) = response(&mut socket, id);
            let invocation = called["result"]["structuredContent"]["invocation"].clone();
            assert!(invocation.is_string(), "{called}");
            invocations.push(invocation);
        }
        assert_ne!(invocations[0], invocations[1]);

        let audit = home.path().join("audit.jsonl");
        let recorded = || -> Vec<Value> {
            std::fs::read_to_string(&audit)
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .map(|record| record["invocation"].clone())
                .collect()
        };
        eventually(|| recorded().len() == invocations.len());
        assert_eq!(recorded(), invocations);
    }

    /// Join an `mcp` stream to a declared session whose workspace the test holds leased, so its
    /// calls wait in the session queue. The lease is the second value.
    fn open_held_session(
        served: &Served,
        key: &str,
    ) -> (Connection, crate::workspace::WorkspaceLease) {
        let marker = SessionMarker::Declared { key: key.into() };
        let workspace = served.intake.workspaces.connect(
            Arc::new(ConnectionEvidence::claimed("holder", IntakeChannel::Cli)),
            Some(marker.clone()),
        );
        let lease = served.intake.workspaces.acquire(&workspace).unwrap();
        let mut opening = mcp_opening(None);
        opening.session = Some(serde_json::to_value(marker).unwrap());
        let (mut socket, reply) = dial(&opening);
        assert!(reply.accepted, "{reply:?}");
        initialize(&mut socket);
        (socket, lease)
    }

    /// Send a `browser_tabs` list under `id` that may wait as long as a call can.
    fn send_list(socket: &mut Connection, id: u64) {
        send_line(
            socket,
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": "browser_tabs",
                    "arguments": {"action": "list", "timeout_ms": 30_000}
                }
            }),
        );
    }

    /// The audit lines of the test home, oldest first.
    fn audit_lines(home: &TestHome) -> Vec<Value> {
        std::fs::read_to_string(home.path().join("audit.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect()
    }

    #[test]
    fn a_full_queue_refuses_through_the_completion_path() {
        let home = TestHome::new("mcp-full-queue");
        let served = serve_fake(&home);
        let (mut socket, lease) = open_held_session(&served, "full-queue");
        // One call waits for the lease and the rest fill the ordinary lane behind it.
        let queued = 2..2 + super::admission::SESSION_WORK as u64;
        for id in queued.clone() {
            send_list(&mut socket, id);
        }
        let refused = queued.end;
        send_list(&mut socket, refused);
        let (called, _) = response(&mut socket, refused);
        let result = &called["result"]["structuredContent"];
        assert_eq!(result["status"], "failed", "{called}");
        assert_eq!(result["facts"]["reason"], "capacity", "{called}");
        let invocation = result["invocation"].clone();
        eventually(|| {
            audit_lines(&home)
                .iter()
                .any(|record| record["invocation"] == invocation)
        });
        let records: Vec<Value> = audit_lines(&home)
            .into_iter()
            .filter(|record| record["invocation"] == invocation)
            .collect();
        assert_eq!(records.len(), 1, "{records:?}");
        assert_eq!(records[0]["status"], "failed");
        drop(lease);
        for id in queued {
            let (called, _) = response(&mut socket, id);
            assert_ne!(
                called["result"]["structuredContent"]["facts"]["reason"],
                "capacity"
            );
        }
    }

    /// A declared session whose workspace the test holds, with the calls `ids` waiting for its
    /// lease.
    fn work_waiting(
        served: &Served,
        key: &str,
        ids: std::ops::Range<u64>,
    ) -> (Connection, crate::workspace::WorkspaceLease) {
        let (mut socket, lease) = open_held_session(served, key);
        for id in ids {
            send_list(&mut socket, id);
        }
        eventually(|| {
            served
                .intake
                .workbench
                .snapshot()
                .operations
                .iter()
                .any(|operation| operation.phase == crate::workbench::OperationPhase::Waiting)
        });
        (socket, lease)
    }

    /// Every audit line's summary, once `count` lines are written.
    fn summaries_of(home: &TestHome, count: usize) -> Vec<Value> {
        eventually(|| audit_lines(home).len() == count);
        audit_lines(home)
            .iter()
            .map(|record| {
                assert_eq!(record["status"], "cancelled", "{record}");
                record["summary"].clone()
            })
            .collect()
    }

    #[test]
    fn a_client_cancel_is_recorded_as_cancelled_by_the_agent() {
        let home = TestHome::new("mcp-client-cancel");
        let served = serve_fake(&home);
        let (mut socket, _lease) = work_waiting(&served, "client-cancel", 2..3);
        send_line(
            &mut socket,
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/cancelled",
                "params": {"requestId": 2}
            }),
        );
        assert_eq!(
            summaries_of(&home, 1),
            ["The agent cancelled the browser job before it started."]
        );
    }

    #[test]
    fn work_waiting_when_its_session_ends_says_the_session_ended() {
        let home = TestHome::new("mcp-session-ends");
        let served = serve_fake(&home);
        let (socket, _lease) = work_waiting(&served, "session-ends", 2..4);
        drop(socket);
        assert_eq!(
            summaries_of(&home, 2),
            ["The agent's session ended before the browser job started."; 2]
        );
    }

    #[test]
    fn work_waiting_at_quit_says_ghostlight_quit() {
        let home = TestHome::new("mcp-quit-waiting");
        let served = serve_fake(&home);
        let (socket, _lease) = work_waiting(&served, "quit-waiting", 2..4);
        served.intake.quit.begin(QuitReason::Quit, Duration::ZERO);
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the quit finishes its drain");
        drop(socket);
        assert_eq!(
            summaries_of(&home, 2),
            ["Ghostlight quit before the browser job started."; 2]
        );
    }

    #[test]
    fn a_disconnect_cancels_queued_work_and_records_it_once() {
        let home = TestHome::new("mcp-disconnect-queue");
        let served = serve_fake(&home);
        let (socket, lease) = work_waiting(&served, "disconnect-queue", 2..5);
        drop(socket);
        eventually(|| audit_lines(&home).len() == 3);
        drop(lease);
        eventually(|| served.intake.workbench.snapshot().operations.is_empty());
        let records = audit_lines(&home);
        assert_eq!(records.len(), 3, "{records:?}");
        let invocations: std::collections::BTreeSet<String> = records
            .iter()
            .map(|record| record["invocation"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(invocations.len(), 3, "each request is recorded once");
        for record in &records {
            assert_eq!(record["status"], "cancelled", "{record}");
        }
    }

    #[test]
    fn a_real_relay_connection_carries_the_app_that_started_its_peer_and_a_pair_carries_none() {
        let home = TestHome::new("mcp-source");
        let served = serve_fake(&home);
        let observed = Source::observe(Some(std::process::id()));
        assert!(
            observed.is_some(),
            "the test runner was started by a program that is not a wrapper"
        );
        let expected = ConnectionEvidence::new(
            Attribution {
                source: observed,
                ..Attribution::accepted(IntakeChannel::Mcp)
            },
            Some("relay test"),
        )
        .details()
        .explanation;

        let mut socket = open_mcp(None);
        initialize(&mut socket);
        eventually(|| served.intake.workspaces.summaries().len() == 1);
        let summary = &served.intake.workspaces.summaries()[0];
        assert_eq!(summary.connections[0].explanation, expected);

        let mut paired = served.over_a_pair();
        paired.set_read_timeout(Some(PATIENCE)).unwrap();
        let reply = protocol::open(&paired, &mcp_opening(None), Instant::now() + PATIENCE).unwrap();
        assert!(reply.accepted, "{reply:?}");
        initialize(&mut paired);
        eventually(|| served.intake.workspaces.summaries().len() == 2);
        let explanations: Vec<String> = served
            .intake
            .workspaces
            .summaries()
            .iter()
            .map(|summary| summary.connections[0].explanation.clone())
            .collect();
        let unseen = ConnectionEvidence::new(
            Attribution::accepted(IntakeChannel::Mcp),
            Some("relay test"),
        )
        .details()
        .explanation;
        assert!(
            explanations.contains(&unseen),
            "a stream with no peer process keeps the caveat: {explanations:?}"
        );
    }

    /// The executable a held source read names.
    const HELD_SOURCE: &str = "held.exe";

    /// How long a test waits between an accept and what follows it.
    const PAUSE: Duration = Duration::from_millis(300);

    /// A source read that waits until the test releases it, and counts the reads that began.
    #[derive(Default)]
    struct HeldRead {
        released: std::sync::Mutex<bool>,
        changed: std::sync::Condvar,
        began: AtomicUsize,
    }

    impl HeldRead {
        /// A reader that waits on this hold, then names [`HELD_SOURCE`].
        fn reader(self: &Arc<Self>) -> SourceReader {
            let held = Arc::clone(self);
            SourceReader::with(move |_| {
                held.began.fetch_add(1, Ordering::SeqCst);
                let mut released = held.released.lock().unwrap();
                while !*released {
                    released = held.changed.wait(released).unwrap();
                }
                Some(Source {
                    agent: None,
                    image: HELD_SOURCE.into(),
                })
            })
        }

        /// How many reads have begun.
        fn began(&self) -> usize {
            self.began.load(Ordering::SeqCst)
        }

        /// Let every read finish.
        fn release(&self) {
            *self.released.lock().unwrap() = true;
            self.changed.notify_all();
        }
    }

    /// Releases a hold when a test ends, however it ends, so no session thread stays blocked.
    struct Release(Arc<HeldRead>);

    impl Drop for Release {
        fn drop(&mut self) {
            self.0.release();
        }
    }

    /// Now, in milliseconds since the Unix epoch, as attribution is dated.
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .try_into()
            .unwrap()
    }

    /// A resume token for a session that owned no tabs.
    fn tabless_token() -> String {
        ResumeState {
            revision: "2025-06-18".into(),
            claim: Some("returning client".into()),
            owned_tabs: Vec::new(),
        }
        .encode()
    }

    /// Call a tool on `socket` and return the accept time its audit line in `home` records.
    fn recorded_accept_time(home: &TestHome, socket: &mut Connection) -> u64 {
        send_line(
            socket,
            json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "tools/call",
                "params": {"name": "browser_tabs", "arguments": {"action": "list"}}
            }),
        );
        response(socket, 7);
        let audit = home.path().join("audit.jsonl");
        let accepted = || {
            let lines = std::fs::read_to_string(&audit).ok()?;
            lines.lines().find_map(|line| {
                serde_json::from_str::<Value>(line).ok()?["provenance"]["observed_at_ms"].as_u64()
            })
        };
        eventually(|| accepted().is_some());
        accepted().unwrap()
    }

    #[test]
    fn an_opening_is_answered_before_the_source_is_read() {
        let home = TestHome::new("opening-before-source");
        let held = Arc::new(HeldRead::default());
        let _release = Release(Arc::clone(&held));
        let served = serve_reading(&home, held.reader());

        let mut socket = open_mcp(Some(tabless_token()));
        eventually(|| held.began() == 1);
        assert!(
            served.intake.workspaces.summaries().is_empty(),
            "the session opens only once its source is read"
        );

        held.release();
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        );
        let (listed, _) = response(&mut socket, 2);
        assert!(listed["result"]["tools"].is_array(), "{listed}");
        assert_eq!(held.began(), 1, "the source is read once per connection");
    }

    #[test]
    fn initialize_is_answered_before_the_source_is_read() {
        let home = TestHome::new("initialize-before-source");
        let held = Arc::new(HeldRead::default());
        let _release = Release(Arc::clone(&held));
        let served = serve_reading(&home, held.reader());

        let mut socket = open_mcp(None);
        thread::sleep(PAUSE);
        assert_eq!(
            held.began(),
            0,
            "a fresh session reads nothing at its opening"
        );
        let (initialized, _) = initialize(&mut socket);
        assert_eq!(initialized["result"]["serverInfo"]["name"], "ghostlight");
        eventually(|| held.began() == 1);
        assert!(served.intake.workspaces.summaries().is_empty());

        held.release();
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        );
        let (listed, _) = response(&mut socket, 2);
        assert!(listed["result"]["tools"].is_array(), "{listed}");
        let summaries = served.intake.workspaces.summaries();
        assert_eq!(summaries.len(), 1);
        assert!(
            summaries[0].connections[0]
                .explanation
                .contains(HELD_SOURCE),
            "{:?}",
            summaries[0].connections[0]
        );
        assert_eq!(held.began(), 1, "the source is read once per connection");
    }

    #[test]
    fn a_fresh_session_is_dated_when_it_was_accepted() {
        let home = TestHome::new("fresh-dated");
        let _served = serve_fake(&home);
        let before = now_ms();
        let mut socket = open_mcp(None);
        thread::sleep(PAUSE);
        let paused = now_ms();
        initialize(&mut socket);
        let accepted = recorded_accept_time(&home, &mut socket);
        assert!(
            (before..paused).contains(&accepted),
            "accepted at {accepted}, dialed at {before}, initialized at {paused}"
        );
    }

    #[test]
    fn a_resumed_session_is_dated_when_it_was_accepted() {
        let home = TestHome::new("resumed-dated");
        let held = Arc::new(HeldRead::default());
        let _release = Release(Arc::clone(&held));
        let _served = serve_reading(&home, held.reader());
        let before = now_ms();
        let mut socket = open_mcp(Some(tabless_token()));
        eventually(|| held.began() == 1);
        thread::sleep(PAUSE);
        let released = now_ms();
        held.release();
        let accepted = recorded_accept_time(&home, &mut socket);
        assert!(
            (before..released).contains(&accepted),
            "accepted at {accepted}, dialed at {before}, read at {released}"
        );
    }

    #[test]
    fn harness_attached_names_the_source_file_and_never_the_claim() {
        let attribution = Attribution {
            source: Some(Source {
                agent: Some("claude-desktop".into()),
                image: "claude.exe".into(),
            }),
            ..Attribution::accepted(IntakeChannel::Mcp)
        };
        let evidence = ConnectionEvidence::new(attribution.clone(), Some("PRIVATE_CLAIM"));
        let detail = super::attached(evidence.attribution());
        let connection = attribution.connection_id.as_deref().unwrap();
        assert_eq!(detail, format!("{connection} via mcp from claude.exe"));
        assert!(!detail.contains("PRIVATE_CLAIM"));

        let unseen = Attribution::accepted(IntakeChannel::Cli);
        assert_eq!(
            super::attached(&unseen),
            format!("{} via cli", unseen.connection_id.as_deref().unwrap())
        );
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

    /// The intake a session is attributed to when its `mcp` opening names `channel`.
    fn intake_of_an_opening_naming(home: &str, channel: Option<&str>) -> IntakeChannel {
        let home = TestHome::new(home);
        let served = serve_fake(&home);
        let mut opening = mcp_opening(None);
        opening.channel = channel.map(str::to_owned);
        let (mut socket, reply) = dial(&opening);
        assert!(reply.accepted, "{reply:?}");
        initialize(&mut socket);
        eventually(|| served.intake.workspaces.summaries().len() == 1);
        served.intake.workspaces.summaries()[0].connections[0].channel
    }

    #[test]
    fn an_opening_with_an_unknown_channel_is_an_mcp_client() {
        assert_eq!(
            intake_of_an_opening_naming("unknown-channel", Some("npm")),
            IntakeChannel::Mcp
        );
    }

    #[test]
    fn an_opening_with_no_channel_is_an_mcp_client() {
        assert_eq!(
            intake_of_an_opening_naming("no-channel", None),
            IntakeChannel::Mcp
        );
    }

    #[test]
    fn an_opening_on_the_command_line_channel_is_the_command_line() {
        assert_eq!(
            intake_of_an_opening_naming("cli-channel", Some(protocol::CLI_CHANNEL)),
            IntakeChannel::Cli
        );
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
                connections: Some(crate::language::workbench::ConnectionCounts::default()),
                workbench: None,
                window: None,
            }
        );
        drop(served);
    }

    #[test]
    fn control_status_after_publishing_names_the_workbench_and_never_its_bearer() {
        let home = TestHome::new("control-status-workbench");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let address = host.workbench_address().expect("the workbench is served");
        let bearer = host
            .intake
            .workbench_server
            .get()
            .unwrap()
            .bearer()
            .to_owned();

        let status = cli_client::status().unwrap();
        assert_eq!(
            status.workbench,
            Some(control::WorkbenchStatus {
                url: address.url().to_owned(),
                usual_port: address.usual(),
                watching: true,
            })
        );
        assert_eq!(
            status.connections,
            Some(crate::language::workbench::ConnectionCounts::default())
        );
        let sent = serde_json::to_string(&status).unwrap();
        assert!(!sent.contains(&bearer));
        drop(host);
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

    /// The digest a resume token carries for a tab last known at `url`.
    fn digest_of(url: &str) -> Option<String> {
        Some(crate::workspace::UrlDigest::of(url).as_str().to_owned())
    }

    /// Read frames until the next resume token and decode it.
    fn next_resume(socket: &mut Connection) -> ResumeState {
        loop {
            if let McpFrame::Resume { token } =
                frame::read_frame::<McpFrame>(socket).unwrap().unwrap()
            {
                return ResumeState::decode(&token).unwrap();
            }
        }
    }

    /// Send one `tools/call` and read its structured result.
    fn call_tool(socket: &mut Connection, id: u64, name: &str, arguments: Value) -> Value {
        send_line(
            socket,
            json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
                "params": {"name": name, "arguments": arguments}}),
        );
        let (called, _) = response(socket, id);
        called["result"]["structuredContent"].clone()
    }

    #[test]
    fn a_resume_token_restores_the_session_and_stales_a_missing_tab() {
        use crate::browser::adapter::{BrowserOutcome, BrowserReadiness, PhysicalTab};
        use crate::intake::mcp::OwnedTab;

        let home = TestHome::new("resume");
        let served = serve_fake(&home);
        let kept = OwnedTab {
            browser_id: FAKE_BROWSER.into(),
            tab_id: 7,
            handle: "tab_kept".into(),
            url_digest: digest_of("https://example.com/"),
        };
        let gone = OwnedTab {
            browser_id: FAKE_BROWSER.into(),
            tab_id: 8,
            handle: "tab_gone".into(),
            url_digest: digest_of("https://example.com/gone"),
        };
        let token = ResumeState {
            revision: "2025-06-18".into(),
            claim: Some("returning client".into()),
            owned_tabs: vec![kept.clone(), gone.clone()],
        }
        .encode();

        // The session resumes at once and asks no browser: both tabs wait in the token.
        let mut socket = open_mcp(Some(token));
        let state = next_resume(&mut socket);
        assert_eq!(state.revision, "2025-06-18");
        assert_eq!(state.claim.as_deref(), Some("returning client"));
        assert_eq!(
            state.owned_tabs,
            vec![gone, kept.clone()],
            "sorted by handle"
        );
        assert!(served.browser.calls().is_empty());
        let summaries = served.intake.workspaces.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(
            summaries[0].opener.reported_application.as_deref(),
            Some("returning client")
        );

        // A resumed session is initialized already: a tools call is served without initialize.
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 9, "method": "tools/list"}),
        );
        let (listed, _) = response(&mut socket, 9);
        assert!(listed["result"]["tools"].is_array(), "{listed}");

        // The first call confirms each tab: the browser still has tab 7; tab 8 closed while the
        // service was away.
        let live = || {
            Ok(BrowserOutcome::Tabs {
                tabs: vec![PhysicalTab {
                    tab_id: 7,
                    url: "https://example.com/".into(),
                    title: "Example".into(),
                    active: true,
                    readiness: BrowserReadiness::Complete,
                }],
            })
        };
        served.browser.push(live());
        served.browser.push(live());
        let listed = call_tool(&mut socket, 10, "browser_tabs", json!({"action": "list"}));
        assert_eq!(listed["status"], "succeeded", "{listed}");
        let tabs = listed["facts"]["tabs"].as_array().unwrap();
        assert_eq!(tabs.len(), 1, "{listed}");
        assert_eq!(tabs[0]["tab"], "tab_kept");
        assert_eq!(next_resume(&mut socket).owned_tabs, vec![kept]);
        // One listing confirms the tabs and the list's own reads them; nothing reaches a tab.
        assert_eq!(
            served.browser.calls()[..2],
            [BrowserCommand::ListTabs, BrowserCommand::ListTabs]
        );
    }

    /// The browser identity of an extension that returns after its session resumed.
    const RETURNING_BROWSER: &str = "browser_returning";

    /// The tab the returning browser still holds.
    const RETURNING_TAB: u64 = 12;

    /// The address of the returning browser's tab.
    const RETURNING_URL: &str = "https://example.com/";

    /// What the returning browser's page answers to one world message.
    fn returning_world_value(message: &Value) -> Value {
        if message["kind"] == "read_text" {
            json!({"text": "Example Domain", "truncated": false, "title": "Example",
                "url": RETURNING_URL})
        } else {
            Value::Null
        }
    }

    /// A resume token for a session that owned `tab`.
    fn token_with(tab: crate::intake::mcp::OwnedTab) -> String {
        ResumeState {
            revision: "2025-06-18".into(),
            claim: Some("returning client".into()),
            owned_tabs: vec![tab],
        }
        .encode()
    }

    /// A resume token for a session that owned the returning browser's tab.
    fn returning_token() -> String {
        token_with(returning_tab())
    }

    fn returning_tab() -> crate::intake::mcp::OwnedTab {
        crate::intake::mcp::OwnedTab {
            browser_id: RETURNING_BROWSER.into(),
            tab_id: RETURNING_TAB,
            handle: "tab_kept".into(),
            url_digest: digest_of(RETURNING_URL),
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
                        "title": "Example", "url": RETURNING_URL, "active": true,
                        "readiness": "complete"}]}),
                    Primitive::Frames(_) => json!({"frames": [{"frame_id": "T",
                        "loader_id": "t1", "parent_frame_id": null, "url": RETURNING_URL}],
                        "complete": true, "unseen": []}),
                    Primitive::Call(call) => json!({"answers": call.calls.iter().map(|world| {
                        json!({"status": "returned", "envelope": {"ok": true,
                            "value": returning_world_value(&world.message)}})
                    }).collect::<Vec<_>>()}),
                    _ => Value::Null,
                };
                FakeReply::Now(FakeExtension::ok(correlation, outcome))
            }),
        );
        served.relay.attach(service).unwrap();
        fake
    }

    /// The confirmed tabs the one session holds in the workspace store.
    fn bound_tabs(served: &Served) -> Vec<(String, u64)> {
        let summaries = served.intake.workspaces.summaries();
        assert_eq!(summaries.len(), 1);
        served
            .intake
            .workspaces
            .tab_bindings(&summaries[0].id)
            .into_iter()
            .filter(|binding| !binding.pending)
            .map(|binding| (binding.handle, binding.physical_id))
            .collect()
    }

    /// The pending tabs the one session holds in the workspace store, with their browsers.
    fn pending_tabs(served: &Served) -> Vec<(String, String)> {
        let summaries = served.intake.workspaces.summaries();
        served
            .intake
            .workspaces
            .tab_bindings(&summaries[0].id)
            .into_iter()
            .filter(|binding| binding.pending)
            .map(|binding| (binding.handle, binding.browser))
            .collect()
    }

    #[test]
    fn a_call_before_the_tokens_browser_returns_keeps_its_tabs_in_the_token() {
        let home = TestHome::new("resume-late");
        let served = serve_relayed(&home);
        let mut socket = open_mcp(Some(returning_token()));
        // The session resumed without waiting for its browser, and its tab waits in the token.
        assert_eq!(next_resume(&mut socket).owned_tabs, vec![returning_tab()]);
        assert!(bound_tabs(&served).is_empty());

        // A call that needs no tab completes while the browser is away, and forgets nothing.
        let listed = call_tool(&mut socket, 2, "browser_tabs", json!({"action": "list"}));
        assert_eq!(listed["status"], "succeeded", "{listed}");
        assert_eq!(
            pending_tabs(&served),
            vec![("tab_kept".to_owned(), RETURNING_BROWSER.to_owned())]
        );

        // The browser returns, and the next call that names the tab confirms and reads it.
        let _browser = attach_returning_browser(&served);
        let read = call_tool(&mut socket, 3, "browser_read", json!({"tab": "tab_kept"}));
        assert_eq!(read["status"], "succeeded", "{read}");
        assert_eq!(
            bound_tabs(&served),
            vec![("tab_kept".to_owned(), RETURNING_TAB)]
        );
    }

    #[test]
    fn a_call_right_after_its_browser_returns_finds_its_resumed_tab() {
        let home = TestHome::new("resume-call");
        let served = serve_relayed(&home);
        let mut socket = open_mcp(Some(returning_token()));
        let McpFrame::Resume { .. } = frame::read_frame::<McpFrame>(&mut socket).unwrap().unwrap()
        else {
            panic!("a resumed session hands back a fresh token first");
        };
        assert!(bound_tabs(&served).is_empty());

        let _browser = attach_returning_browser(&served);
        // No wait for the session to notice the browser: the call itself confirms the tab.
        let called = call_tool(&mut socket, 2, "browser_read", json!({"tab": "tab_kept"}));
        assert_eq!(called["status"], "succeeded", "{called}");
        assert_eq!(
            bound_tabs(&served),
            vec![("tab_kept".to_owned(), RETURNING_TAB)]
        );
    }

    #[test]
    fn a_token_written_before_digests_resumes_with_stale_tabs() {
        let home = TestHome::new("resume-no-digest");
        let served = serve_relayed(&home);
        let earlier = crate::intake::mcp::OwnedTab {
            url_digest: None,
            ..returning_tab()
        };
        let mut socket = open_mcp(Some(token_with(earlier)));
        assert!(next_resume(&mut socket).owned_tabs.is_empty());

        // The browser still shows a tab with that id at that address, and it is not taken.
        let _browser = attach_returning_browser(&served);
        let read = call_tool(&mut socket, 2, "browser_read", json!({"tab": "tab_kept"}));
        assert_ne!(read["status"], "succeeded", "{read}");
        assert!(bound_tabs(&served).is_empty());
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
    fn the_pages_quit_is_a_persons_quit() {
        let home = TestHome::new("workbench-quit");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        host.workbench.quit().unwrap();
        // The same quit the tray asks for: new work is refused at once, and the latch keeps a
        // browser that restarts from bringing Ghostlight back.
        assert!(host.intake.quit.drain.is_quitting());
        assert!(super::latch::is_set(
            &crate::install::layout::Layout::new(home.path()).quit_latch()
        ));
        drop(host);
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
        // The session opens once its source is read, which follows the `initialize` answer.
        eventually(|| host.workbench.snapshot().sessions.len() == 1);

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
            &crate::install::layout::Layout::new(directory.path()).service_lock()
        ));
        drop(first);
        let replacement = ServiceHost::start(&directory).unwrap();
        drop(replacement);
    }

    /// The terms of a start for `reason` that waits `lock_wait` for a held lock.
    fn waiting(reason: StartReason, lock_wait: Duration) -> StartTerms {
        StartTerms {
            lock_wait,
            ..StartTerms::for_reason(reason)
        }
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
    fn a_start_that_waits_no_time_for_a_held_lock_gives_way_at_once_and_touches_no_state() {
        let directory = temp_home("lock-held-gives-way");
        let layout = crate::install::layout::Layout::new(directory.path());
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
            let error = start_for(&directory, StartTerms::asking_once(reason))
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
    }

    #[test]
    fn a_browser_start_that_waited_for_the_lock_is_refused_when_a_quit_began_meanwhile() {
        let directory = temp_home("latch-set-while-waiting");
        let layout = crate::install::layout::Layout::new(directory.path());
        let holder = start_for(&directory, agent()).unwrap();
        let quitting = {
            let latch = layout.quit_latch();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(150));
                super::latch::set(&latch).unwrap();
                drop(holder);
            })
        };
        let error = start_for(&directory, waiting(StartReason::Browser, PATIENCE))
            .err()
            .expect("Quit holds this start back");
        assert!(error.is::<QuitLatched>());
        assert!(
            super::latch::is_set(&layout.quit_latch()),
            "the latch stays"
        );
        quitting.join().unwrap();
        assert!(
            super::ServiceLock::is_free(&layout.service_lock()),
            "the refused start released the lock"
        );
    }

    #[test]
    fn a_browser_or_recover_start_is_refused_while_the_latch_is_set_and_never_takes_the_lock() {
        let directory = temp_home("latch-refuses");
        let layout = crate::install::layout::Layout::new(directory.path());
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
    }

    #[test]
    fn a_browser_start_serves_when_no_quit_is_held() {
        let directory = temp_home("latch-clear");
        let started = start_for(&directory, StartTerms::for_reason(StartReason::Browser));
        assert!(started.is_ok());
        drop(started);
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
            let layout = crate::install::layout::Layout::new(directory.path());
            super::latch::set(&layout.quit_latch()).unwrap();
            let started = start_for(&directory, StartTerms::for_reason(reason)).unwrap();
            assert!(!super::latch::is_set(&layout.quit_latch()), "{reason:?}");
            drop(started);
        }
    }

    #[test]
    fn the_service_reports_the_reason_it_was_started_for() {
        let home = TestHome::new("started-by");
        let host = start_for(home.path(), StartTerms::for_reason(StartReason::Person)).unwrap();
        host.publish_ready().unwrap();
        assert_eq!(
            cli_client::status().unwrap().started_by,
            StartReason::Person
        );
        drop(host);
    }

    #[test]
    fn the_elevation_decision_waives_only_an_elevated_answer() {
        use super::decide_elevation;
        let refused = decide_elevation(Ok(true), false).unwrap_err();
        assert_eq!(format!("{refused:#}"), super::ELEVATED_SERVICE);
        assert!(decide_elevation(Ok(true), true).is_ok());
        assert!(decide_elevation(Ok(false), false).is_ok());
        assert!(decide_elevation(Ok(false), true).is_ok());
        for waived in [false, true] {
            let unreadable =
                decide_elevation(Err(std::io::Error::other("no token")), waived).unwrap_err();
            assert!(format!("{unreadable:#}").contains("no token"));
        }
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
    }

    /// A presentation port whose reveal waits until it is released, as one does while a shell
    /// starts.
    #[derive(Default)]
    struct HeldReveal {
        started: AtomicUsize,
        release: AtomicBool,
    }

    impl WorkbenchPresentationPort for HeldReveal {
        fn reveal(&self, _address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
            self.started.fetch_add(1, Ordering::SeqCst);
            let until = Instant::now() + PATIENCE;
            while !self.release.load(Ordering::SeqCst) && Instant::now() < until {
                thread::sleep(Duration::from_millis(5));
            }
            Ok(())
        }

        fn notify(
            &self,
            _notification: WorkbenchNotification,
        ) -> Result<(), WorkbenchPresentationError> {
            Ok(())
        }
    }

    /// A presentation port that keeps the address of each reveal.
    #[derive(Default)]
    struct RevealedAt(std::sync::Mutex<Vec<Option<PageAddress>>>);

    impl RevealedAt {
        fn addresses(&self) -> Vec<Option<PageAddress>> {
            self.0.lock().unwrap().clone()
        }
    }

    impl WorkbenchPresentationPort for RevealedAt {
        fn reveal(&self, address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
            self.0.lock().unwrap().push(address);
            Ok(())
        }

        fn notify(
            &self,
            _notification: WorkbenchNotification,
        ) -> Result<(), WorkbenchPresentationError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct RevealCounter(AtomicUsize);

    impl WorkbenchPresentationPort for RevealCounter {
        fn reveal(&self, _address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
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
    fn a_show_control_opens_the_window_at_setup_until_setup_is_complete() {
        use crate::install::setup_state::{self, SetupRecord};

        let home = TestHome::new("show-control-address");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let revealed = Arc::new(RevealedAt::default());
        host.workbench.attach_presentation(revealed.clone());

        assert!(cli_client::activate_workbench().unwrap());
        assert_eq!(revealed.addresses(), [Some(PageAddress::Setup)]);

        // Once the setup record exists the window opens where it last was.
        let record = crate::install::layout::Layout::new(home.path()).setup_record();
        assert!(setup_state::record_if_absent(&record, &SetupRecord::now()).unwrap());
        host.workbench
            .attach_setup(crate::workbench::setup::SetupProgress::load(&record));
        assert!(cli_client::activate_workbench().unwrap());
        assert_eq!(revealed.addresses(), [Some(PageAddress::Setup), None]);
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
            None,
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

    /// How an extension at a service end answers the page work a call sends it.
    #[derive(Clone, Copy)]
    enum PageWork {
        /// Answer at once, as the returning browser does.
        Answered,
        /// Hold the answer until the service cancels the request.
        HeldUntilCancelled,
        /// Never answer anything, a release command included.
        Silent,
    }

    /// Attach a protocol-4 extension for the returning browser that answers release commands and
    /// page work as `work` says, and records every frame the service sends it.
    fn attach_ending_browser(
        served: &Served,
        work: PageWork,
    ) -> crate::browser::adapter::chromium::protocol4::fake::FakeExtension {
        use crate::browser::adapter::chromium::protocol4::fake::{
            FakeExtension, Reply as FakeReply,
        };
        use crate::browser::adapter::chromium::protocol4::wire::{Failure, Primitive};

        /// What the returning browser answers to one primitive it is asked to run.
        fn answer(primitive: &Primitive) -> Value {
            match primitive {
                Primitive::ListTabs(_) => json!({"tabs": []}),
                Primitive::CloseTab(close) => json!({"tab_id": close.tab_id}),
                Primitive::Frames(_) => json!({"frames": [{"frame_id": "T",
                    "loader_id": "t1", "parent_frame_id": null, "url": RETURNING_URL}],
                    "complete": true, "unseen": []}),
                Primitive::Call(call) => json!({"answers": call.calls.iter().map(|world| {
                    json!({"status": "returned", "envelope": {"ok": true,
                        "value": returning_world_value(&world.message)}})
                }).collect::<Vec<_>>()}),
                _ => Value::Null,
            }
        }

        let (service, extension) = pair().unwrap();
        let fake = FakeExtension::connect(
            extension,
            &FakeExtension::hello(RETURNING_BROWSER),
            Box::new(move |correlation, primitive| {
                let page_work = matches!(primitive, Primitive::Frames(_) | Primitive::Call(_));
                match work {
                    PageWork::Silent => FakeReply::Never,
                    PageWork::HeldUntilCancelled if page_work => {
                        FakeReply::OnCancel(FakeExtension::failed(correlation, Failure::Cancelled))
                    }
                    _ => FakeReply::Now(FakeExtension::ok(correlation, answer(primitive))),
                }
            }),
        );
        served.relay.attach(service).unwrap();
        eventually(|| served.relay.is_connected());
        fake
    }

    /// Open an MCP session and wait until it is open, with its workspace, and return the stream
    /// and that workspace.
    fn open_session(served: &Served) -> (Connection, crate::workspace::WorkspaceId) {
        let before = served.intake.workspaces.ids();
        let mut socket = open_mcp(None);
        initialize(&mut socket);
        // A forwarded request is served only once the session has opened.
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        );
        response(&mut socket, 2);
        let workspace = served
            .intake
            .workspaces
            .ids()
            .into_iter()
            .find(|workspace| !before.contains(workspace))
            .expect("the session opened a workspace");
        (socket, workspace)
    }

    /// Bind the returning browser's tab `tab_id` at its address to `workspace` as a confirmed
    /// tab, and return its handle.
    fn bind_tab(
        workspaces: &crate::workspace::WorkspaceStore,
        workspace: &crate::workspace::WorkspaceId,
        tab_id: u64,
    ) -> String {
        use crate::browser::adapter::{BrowserReadiness, PhysicalTab};

        let lease = workspaces.acquire(workspace).unwrap();
        let tab = PhysicalTab {
            tab_id,
            url: RETURNING_URL.into(),
            title: "Example".into(),
            active: true,
            readiness: BrowserReadiness::Complete,
        };
        let added = lease.add_tab(RETURNING_BROWSER, &tab).unwrap();
        lease.apply_landing(&added.handle, &tab).unwrap();
        added.handle.as_str().to_owned()
    }

    /// Read `socket` up to its goodbye, and return the resume tokens handed out on the way and
    /// the goodbye's reason.
    fn until_goodbye(socket: &mut Connection) -> (Vec<ResumeState>, GoodbyeReason) {
        let mut tokens = Vec::new();
        loop {
            match frame::read_frame::<McpFrame>(socket).unwrap() {
                Some(McpFrame::Resume { token }) => {
                    tokens.push(ResumeState::decode(&token).unwrap());
                }
                Some(McpFrame::Goodbye { reason }) => return (tokens, reason),
                Some(_) => {}
                None => panic!("the stream closed with no goodbye"),
            }
        }
    }

    /// The handles a resume token restores.
    fn handles_in(token: &ResumeState) -> Vec<String> {
        token
            .owned_tabs
            .iter()
            .map(|tab| tab.handle.clone())
            .collect()
    }

    /// Where in what an extension saw the service's goodbye came, if it came.
    fn goodbye_at(
        seen: &crate::browser::adapter::chromium::protocol4::fake::Seen,
    ) -> Option<usize> {
        seen.frames
            .iter()
            .position(|frame| frame.get("goodbye").is_some())
    }

    /// Where in what an extension saw each request for `primitive` came, with its tab.
    fn requests_at(
        seen: &crate::browser::adapter::chromium::protocol4::fake::Seen,
        primitive: &str,
    ) -> Vec<(usize, u64)> {
        seen.frames
            .iter()
            .enumerate()
            .filter(|(_, frame)| frame["kind"] == "request")
            .filter(|(_, frame)| frame["primitive"]["name"] == primitive)
            .map(|(at, frame)| {
                (
                    at,
                    frame["primitive"]["args"]["tab_id"]
                        .as_u64()
                        .unwrap_or_default(),
                )
            })
            .collect()
    }

    /// The tabs a release closed, each before the browser goodbye, which has come.
    fn closed_before_goodbye(
        browser: &crate::browser::adapter::chromium::protocol4::fake::FakeExtension,
    ) -> Vec<u64> {
        let seen = browser.wait(|seen| goodbye_at(seen).is_some());
        let goodbye = goodbye_at(&seen).unwrap();
        let closes = requests_at(&seen, "close_tab");
        assert!(
            closes.iter().all(|(at, _)| *at < goodbye),
            "every release close comes before the browser goodbye: {:?}",
            seen.frames
        );
        let mut tabs: Vec<u64> = closes.into_iter().map(|(_, tab)| tab).collect();
        tabs.sort_unstable();
        tabs
    }

    #[test]
    fn a_replace_keeps_a_live_sessions_tabs_bound_grouped_and_resumable() {
        let home = TestHome::new("end-replace");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::Answered);
        let (mut socket, workspace) = open_session(&served);
        let handle = bind_tab(&served.intake.workspaces, &workspace, RETURNING_TAB);

        served
            .intake
            .quit
            .begin(QuitReason::Replace, Duration::ZERO);

        let (tokens, goodbye) = until_goodbye(&mut socket);
        assert_eq!(goodbye, GoodbyeReason::Replace);
        let last = tokens
            .last()
            .expect("a last token comes before the goodbye");
        assert_eq!(handles_in(last), [handle.as_str()]);
        assert!(last.owned_tabs[0].url_digest.is_some(), "{last:?}");
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the replace finishes");
        assert_eq!(
            closed_before_goodbye(&browser),
            Vec::<u64>::new(),
            "nothing closes or ungroups a live session's tab"
        );
        assert!(requests_at(&browser.wait(|_| true), "diagnostics").is_empty());
        assert_eq!(bound_tabs(&served), vec![(handle, RETURNING_TAB)]);
    }

    #[test]
    fn a_quit_releases_every_session_and_hands_each_stream_a_token_without_tabs() {
        for reason in [QuitReason::Quit, QuitReason::Uninstall, QuitReason::Unknown] {
            let home = TestHome::new("end-quit");
            let served = serve_relayed(&home);
            let browser = attach_ending_browser(&served, PageWork::Answered);
            let (mut first, first_workspace) = open_session(&served);
            let (mut second, second_workspace) = open_session(&served);
            let first_tab = bind_tab(&served.intake.workspaces, &first_workspace, RETURNING_TAB);
            let second_tab = bind_tab(
                &served.intake.workspaces,
                &second_workspace,
                RETURNING_TAB + 1,
            );
            // The session watch hands each stream a token with its tab within a second, so the
            // last token each stream gets says what the end decided.
            for (socket, handle) in [(&mut first, &first_tab), (&mut second, &second_tab)] {
                while handles_in(&next_resume(socket)) != [handle.as_str()] {}
            }

            served.intake.quit.begin(reason, Duration::ZERO);

            for socket in [&mut first, &mut second] {
                let (tokens, goodbye) = until_goodbye(socket);
                assert_eq!(goodbye, super::goodbye_for(reason));
                let last = tokens
                    .last()
                    .expect("a last token comes before the goodbye");
                assert!(last.owned_tabs.is_empty(), "{reason:?}: {last:?}");
            }
            served
                .quit_requested
                .recv_timeout(PATIENCE)
                .expect("the quit finishes");
            assert_eq!(
                closed_before_goodbye(&browser),
                [RETURNING_TAB, RETURNING_TAB + 1],
                "{reason:?}"
            );
            assert!(served.intake.workspaces.ids().is_empty(), "{reason:?}");
        }
    }

    #[test]
    fn a_command_line_session_is_released_at_every_service_end() {
        for reason in [
            QuitReason::Quit,
            QuitReason::Replace,
            QuitReason::Uninstall,
            QuitReason::Unknown,
        ] {
            let home = TestHome::new("end-command-line");
            let served = serve_relayed(&home);
            let browser = attach_ending_browser(&served, PageWork::Answered);
            let workspace = served.intake.workspaces.resume_or_admit(
                "command line".into(),
                IntakeChannel::Cli,
                SessionMarker::Declared {
                    key: "end-command-line".into(),
                },
            );
            bind_tab(&served.intake.workspaces, &workspace, RETURNING_TAB);

            served.intake.quit.begin(reason, Duration::ZERO);

            served
                .quit_requested
                .recv_timeout(PATIENCE)
                .expect("the end finishes");
            assert_eq!(
                closed_before_goodbye(&browser),
                [RETURNING_TAB],
                "{reason:?}"
            );
            assert!(served.intake.workspaces.ids().is_empty(), "{reason:?}");
        }

        // The end of the session and the idle exit send the browser nothing and forget the tab.
        for ends in [
            (|quit: &super::Quit| quit.session_end()) as fn(&super::Quit),
            |quit: &super::Quit| quit.idle_exit(Duration::ZERO),
        ] {
            let home = TestHome::new("end-command-line-quietly");
            let served = serve_relayed(&home);
            let browser = attach_ending_browser(&served, PageWork::Answered);
            let workspace = served.intake.workspaces.resume_or_admit(
                "command line".into(),
                IntakeChannel::Cli,
                SessionMarker::Declared {
                    key: "end-command-line".into(),
                },
            );
            bind_tab(&served.intake.workspaces, &workspace, RETURNING_TAB);

            ends(&served.intake.quit);

            served
                .quit_requested
                .recv_timeout(PATIENCE)
                .expect("the end finishes");
            assert!(served.intake.workspaces.ids().is_empty());
            assert!(requests_at(&browser.wait(|_| true), "close_tab").is_empty());
        }
    }

    #[test]
    fn a_session_whose_client_left_during_the_drain_is_released_on_replace() {
        let home = TestHome::new("end-replace-left");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::Answered);
        let (left, left_workspace) = open_session(&served);
        let (mut stays, stays_workspace) = open_session(&served);
        bind_tab(&served.intake.workspaces, &left_workspace, RETURNING_TAB);
        let kept = bind_tab(
            &served.intake.workspaces,
            &stays_workspace,
            RETURNING_TAB + 1,
        );
        let running = served.intake.quit.drain.admit().unwrap();

        served.intake.quit.begin(QuitReason::Replace, PATIENCE);
        drop(left);
        eventually(|| served.intake.workspaces.ids() == [stays_workspace.clone()]);
        drop(running);

        let (tokens, goodbye) = until_goodbye(&mut stays);
        assert_eq!(goodbye, GoodbyeReason::Replace);
        assert_eq!(handles_in(tokens.last().unwrap()), [kept.as_str()]);
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the replace finishes");
        assert_eq!(closed_before_goodbye(&browser), [RETURNING_TAB]);
        assert_eq!(bound_tabs(&served), vec![(kept, RETURNING_TAB + 1)]);
    }

    #[test]
    fn a_leased_workspace_is_cancelled_before_its_release() {
        let home = TestHome::new("end-leased");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::HeldUntilCancelled);
        let (mut socket, workspace) = open_session(&served);
        let handle = bind_tab(&served.intake.workspaces, &workspace, RETURNING_TAB);
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "browser_read", "arguments": {"tab": handle}}}),
        );
        drop(browser.wait(|seen| {
            !requests_at(seen, "frames").is_empty() || !requests_at(seen, "call").is_empty()
        }));

        // The drain gives up on the call, so the end meets the workspace still leased.
        served
            .intake
            .quit
            .begin(QuitReason::Quit, Duration::from_millis(100));

        let (_, goodbye) = until_goodbye(&mut socket);
        assert_eq!(goodbye, GoodbyeReason::Quit);
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the quit finishes");
        assert_eq!(closed_before_goodbye(&browser), [RETURNING_TAB]);
        let seen = browser.wait(|_| true);
        let cancelled = seen
            .frames
            .iter()
            .position(|frame| frame["kind"] == "cancel")
            .expect("the running call is cancelled");
        let closed = requests_at(&seen, "close_tab")[0].0;
        assert!(cancelled < closed, "{:?}", seen.frames);
        drop(seen);
        assert!(served.intake.workspaces.ids().is_empty());
    }

    #[test]
    fn a_session_end_says_goodbye_inside_the_fallback_with_many_tabs() {
        const TABS: u64 = 300;

        let home = TestHome::new("end-session-many");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::Silent);
        let (mut socket, workspace) = open_session(&served);
        for tab in 0..TABS {
            bind_tab(&served.intake.workspaces, &workspace, RETURNING_TAB + tab);
        }

        let began = Instant::now();
        served.intake.quit.session_end();

        let (_, goodbye) = until_goodbye(&mut socket);
        assert_eq!(goodbye, GoodbyeReason::SessionEnd);
        let seen = browser.wait(|seen| goodbye_at(seen).is_some());
        assert!(
            began.elapsed() < ghostlight_relay::os::session::FALLBACK_EXIT,
            "both goodbyes come before the fallback exit: {:?}",
            began.elapsed()
        );
        assert!(requests_at(&seen, "close_tab").is_empty());
        assert!(requests_at(&seen, "diagnostics").is_empty());
        drop(seen);
        assert!(served.intake.workspaces.ids().is_empty());
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
    }

    #[test]
    fn the_session_watch_tells_the_client_only_when_a_policy_change_changes_the_catalog() {
        use crate::governance::GovernanceFacade;

        use super::AdvertisedCatalog;

        let directory = temp_home("catalog");
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
        let governance = GovernanceFacade::new(Some(path.clone()));
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
    }

    /// An opening for the shell of this build.
    fn desktop_opening() -> RelayOpening {
        RelayOpening::new(
            Role::Desktop,
            Edge::new(Program::Desktop, env!("CARGO_PKG_VERSION").into()),
        )
    }

    /// Open a `desktop` stream and read the frame that hands over the workbench.
    fn open_desktop() -> (Connection, DesktopFrame) {
        let (mut connection, reply) = dial(&desktop_opening());
        assert!(reply.accepted, "{reply:?}");
        let handed = frame::read_frame::<DesktopFrame>(&mut connection)
            .unwrap()
            .expect("the first frame hands over the workbench");
        (connection, handed)
    }

    /// The address, the bearer and the tray of a frame that hands over the workbench.
    fn handed_over(handed: DesktopFrame) -> (String, String, TrayModel) {
        match handed {
            DesktopFrame::Workbench { url, bearer, tray } => (url, bearer, tray),
            other => panic!("not the workbench: {other:?}"),
        }
    }

    /// One request to the workbench on `port`; the status code and the whole answer come back.
    fn workbench_call(
        port: u16,
        method: &str,
        path: &str,
        bearer: Option<&str>,
        body: Option<&str>,
    ) -> (u16, String) {
        use std::io::{Read, Write};

        let mut text = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
        if let Some(bearer) = bearer {
            text.push_str(&format!("Authorization: Bearer {bearer}\r\n"));
        }
        if let Some(body) = body {
            text.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                body.len()
            ));
        }
        text.push_str("\r\n");
        text.push_str(body.unwrap_or_default());
        let mut stream = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
            .expect("the workbench answers");
        stream.set_read_timeout(Some(PATIENCE)).unwrap();
        stream.write_all(text.as_bytes()).unwrap();
        let mut bytes = Vec::new();
        let _ = stream.read_to_end(&mut bytes);
        let answer = String::from_utf8_lossy(&bytes).into_owned();
        let status = answer
            .split(' ')
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or_else(|| panic!("no status in {answer}"));
        (status, answer)
    }

    #[test]
    fn a_shell_of_this_build_is_handed_the_workbench_and_its_bearer() {
        let home = TestHome::new("desktop-handover");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let address = host.workbench_address().unwrap();

        let (_stream, handed) = open_desktop();
        let (url, bearer, tray) = handed_over(handed);

        assert_eq!(url, address.url());
        assert_eq!(
            tray,
            host.workbench.tray_model(),
            "the workbench frame carries the tray the service describes"
        );
        assert_eq!(tray.state, crate::intake::desktop::TrayState::Ready);
        let hold = r#"{"intent":"hold"}"#;
        let (status, _) = workbench_call(
            address.port(),
            "POST",
            "/api/runtime",
            Some(&bearer),
            Some(hold),
        );
        assert_eq!(status, 200, "the bearer makes the request the window's");
        assert_eq!(
            host.workbench.snapshot().configuration.runtime_state,
            RuntimeControlState::Held
        );
        let resume = r#"{"intent":"resume"}"#;
        let (status, _) =
            workbench_call(address.port(), "POST", "/api/runtime", None, Some(resume));
        assert_eq!(status, 401, "without the bearer a screen cannot resume");
        drop(host);
    }

    #[test]
    fn a_pause_sends_the_shell_one_tray_model_with_resume_enabled() {
        let home = TestHome::new("desktop-tray-model");
        let host =
            ServiceHost::start_in_world(home.path(), |_, _| Fake::default().into_world()).unwrap();
        host.publish_ready().unwrap();
        let (mut stream, handed) = open_desktop();
        let (_, _, tray) = handed_over(handed);
        assert_eq!(tray.state, crate::intake::desktop::TrayState::Ready);

        host.workbench
            .apply_runtime_control(RuntimeControlIntent::Hold, RuntimeSource::Window);

        let DesktopFrame::TrayModel { model } = next_frame(&mut stream) else {
            panic!("the first frame after a pause is the new tray");
        };
        // A pause shows on the icon with or without a browser, as it does on the band; the entries
        // follow the runtime.
        assert_eq!(model.state, crate::intake::desktop::TrayState::Paused);
        let enabled: Vec<(&str, bool)> = model
            .items
            .iter()
            .map(|item| (item.id.as_str(), item.enabled))
            .collect();
        assert_eq!(
            enabled,
            [
                ("open", true),
                ("hold", false),
                ("resume", true),
                ("quit", true)
            ]
        );
        // Asking again with nothing changed sends nothing; the next frame is a later change.
        host.workbench.refresh_live_parts();
        host.workbench
            .apply_runtime_control(RuntimeControlIntent::Resume, RuntimeSource::Window);
        let DesktopFrame::TrayModel { model } = next_frame(&mut stream) else {
            panic!("a resume is the next change, not a repeat of the pause");
        };
        assert!(model
            .items
            .iter()
            .any(|item| item.id == TrayItemId::Hold && item.enabled));
        drop(host);
    }

    #[test]
    fn tray_clicks_hold_and_resume_the_agents_and_an_open_click_changes_nothing() {
        let home = TestHome::new("desktop-tray-clicks");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let (mut stream, _) = open_desktop();
        let state = || host.workbench.snapshot().configuration.runtime_state;

        for (item, expected) in [
            (TrayItemId::Open, RuntimeControlState::Active),
            (TrayItemId::Hold, RuntimeControlState::Held),
            (TrayItemId::Open, RuntimeControlState::Held),
        ] {
            frame::write_frame(&mut stream, &DesktopFrame::Tray { item }).unwrap();
            eventually(|| state() == expected);
        }
        assert!(
            host.workbench.snapshot().runtime_note.is_some(),
            "a pause says where it came from"
        );

        frame::write_frame(
            &mut stream,
            &DesktopFrame::Tray {
                item: TrayItemId::Resume,
            },
        )
        .unwrap();
        eventually(|| state() == RuntimeControlState::Active);
        eventually(|| host.workbench.snapshot().runtime_note.is_none());
        drop(host);
    }

    #[test]
    fn a_tray_quit_is_the_persons_quit() {
        let (home, host, quit_requested) = quittable_host("desktop-tray-quit");
        let (mut stream, _) = open_desktop();

        frame::write_frame(
            &mut stream,
            &DesktopFrame::Tray {
                item: TrayItemId::Quit,
            },
        )
        .unwrap();

        quit_requested
            .recv_timeout(PATIENCE)
            .expect("the desktop is asked to exit");
        assert!(super::latch::is_set(
            &home.path().join("state").join("quit")
        ));
        drop(host);
    }

    #[test]
    fn a_quit_says_goodbye_on_the_desktop_stream_before_it_closes() {
        for (reason, goodbye) in [
            (QuitReason::Quit, GoodbyeReason::Quit),
            (QuitReason::Replace, GoodbyeReason::Replace),
            (QuitReason::Uninstall, GoodbyeReason::Uninstall),
        ] {
            let (_home, host, _quit_requested) = quittable_host("desktop-goodbye");
            let (mut stream, _) = open_desktop();

            host.quit_handle().begin(reason, Duration::from_secs(5));

            match frame::read_frame::<DesktopFrame>(&mut stream).unwrap() {
                Some(DesktopFrame::Goodbye { reason }) => assert_eq!(reason, goodbye),
                other => panic!("expected a goodbye, got {other:?}"),
            }
            drop(host);
        }
    }

    #[test]
    fn a_session_end_says_goodbye_on_the_desktop_stream() {
        let (_home, host, _quit_requested) = quittable_host("desktop-session-end");
        let (mut stream, _) = open_desktop();

        host.quit_handle().session_end();

        assert!(matches!(
            frame::read_frame::<DesktopFrame>(&mut stream).unwrap(),
            Some(DesktopFrame::Goodbye {
                reason: GoodbyeReason::SessionEnd
            })
        ));
        drop(host);
    }

    /// Open a `desktop` stream and wait until the service has attached it, so a request made
    /// next goes to it.
    fn attached_shell(host: &ServiceHost) -> Connection {
        let before = host.quit.shells().attached();
        let (stream, _) = open_desktop();
        eventually(|| host.quit.shells().attached() > before);
        stream
    }

    /// The next frame the service wrote to the shell.
    fn next_frame(stream: &mut Connection) -> DesktopFrame {
        frame::read_frame::<DesktopFrame>(stream)
            .unwrap()
            .expect("the service wrote a frame")
    }

    /// The id of the act the service asked the shell for, and the act.
    fn next_act(stream: &mut Connection) -> (u64, NativeAct) {
        match next_frame(stream) {
            DesktopFrame::Act { id, act } => (id, act),
            other => panic!("expected an act, got {other:?}"),
        }
    }

    /// What a shell answers.
    fn answer(stream: &mut Connection, id: u64, outcome: ActOutcome) {
        frame::write_frame(stream, &DesktopFrame::ActResult { id, outcome }).unwrap();
    }

    /// Run `request` on a thread of its own and give back what it answers.
    fn in_background<T: Send + 'static>(
        request: impl FnOnce() -> T + Send + 'static,
    ) -> std::sync::mpsc::Receiver<T> {
        let (answered, answers) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let _ = answered.send(request());
        });
        answers
    }

    #[test]
    fn showing_the_window_writes_a_show_frame_with_the_address_to_the_shell() {
        let home = TestHome::new("shell-show");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);

        host.workbench
            .reveal(Some(PageAddress::SettingsDiagnostics))
            .unwrap();
        host.workbench.reveal(None).unwrap();

        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Show {
                address: Some(PageAddress::SettingsDiagnostics)
            }
        );
        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Show { address: None }
        );
        drop(host);
    }

    #[test]
    fn a_notice_is_written_to_the_shell_as_the_service_worded_it() {
        let home = TestHome::new("shell-notice");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);

        host.workbench.test_notification().unwrap();

        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Notice {
                title: crate::language::workbench::NOTICE_CHECKUP_TITLE.to_owned(),
                body: crate::language::workbench::NOTICE_CHECKUP_BODY.to_owned(),
            }
        );
        drop(host);
    }

    #[test]
    fn picking_a_file_waits_for_the_shell_and_returns_what_the_person_chose() {
        let home = TestHome::new("shell-pick");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);
        let route = host.route();

        let picked = in_background(move || route.pick_file("Choose a file"));
        let (id, act) = next_act(&mut stream);
        assert_eq!(
            act,
            NativeAct::PickFile {
                title: "Choose a file".to_owned()
            }
        );
        assert!(
            picked.recv_timeout(Duration::from_millis(200)).is_err(),
            "the pick waits for the person"
        );
        answer(
            &mut stream,
            id,
            ActOutcome::Picked {
                path: "chosen.json".to_owned(),
            },
        );
        assert_eq!(
            picked.recv_timeout(PATIENCE).unwrap().unwrap(),
            Some(std::path::PathBuf::from("chosen.json"))
        );

        let route = host.route();
        let closed = in_background(move || route.pick_file("Again"));
        let (id, _) = next_act(&mut stream);
        answer(&mut stream, id, ActOutcome::Cancelled);
        assert_eq!(closed.recv_timeout(PATIENCE).unwrap().unwrap(), None);

        let route = host.route();
        let refused = in_background(move || route.pick_file("Once more"));
        let (id, _) = next_act(&mut stream);
        answer(&mut stream, id, ActOutcome::Failed);
        assert!(matches!(
            refused.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::ActFailed(_))
        ));
        drop(host);
    }

    #[test]
    fn a_link_and_a_folder_are_opened_by_the_shell_and_their_failure_is_told() {
        let home = TestHome::new("shell-open");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);

        for (outcome, opened) in [(ActOutcome::Done, true), (ActOutcome::Failed, false)] {
            let route = host.route();
            let link = in_background(move || route.open_url("https://example.com/docs"));
            let (id, act) = next_act(&mut stream);
            assert_eq!(
                act,
                NativeAct::OpenUrl {
                    url: "https://example.com/docs".to_owned()
                }
            );
            answer(&mut stream, id, outcome.clone());
            assert_eq!(link.recv_timeout(PATIENCE).unwrap().is_ok(), opened);

            let route = host.route();
            let folder = in_background(move || route.open_folder(std::path::Path::new("logs")));
            let (id, act) = next_act(&mut stream);
            assert_eq!(
                act,
                NativeAct::OpenFolder {
                    path: "logs".to_owned()
                }
            );
            answer(&mut stream, id, outcome);
            assert_eq!(folder.recv_timeout(PATIENCE).unwrap().is_ok(), opened);
        }
        drop(host);
    }

    #[test]
    fn an_answer_for_an_act_nobody_asked_is_ignored() {
        let home = TestHome::new("shell-stray-result");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);

        answer(&mut stream, 99, ActOutcome::Done);
        host.workbench.reveal(None).unwrap();

        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Show { address: None },
            "the stream goes on"
        );
        drop(host);
    }

    #[test]
    fn a_pick_is_released_when_the_shells_stream_ends() {
        let home = TestHome::new("shell-pick-ends");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);
        let route = host.route();

        let picked = in_background(move || route.pick_file("Choose"));
        next_act(&mut stream);
        drop(stream);

        assert!(matches!(
            picked.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::Unavailable)
        ));
        eventually(|| host.quit.shells().attached() == 0);
        drop(host);
    }

    #[test]
    fn a_pick_is_released_by_a_goodbye() {
        let home = TestHome::new("shell-pick-goodbye");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);
        let route = host.route();

        let picked = in_background(move || route.pick_file("Choose"));
        next_act(&mut stream);
        host.quit.shells().say_goodbye(GoodbyeReason::Replace);

        assert!(matches!(
            picked.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::Unavailable)
        ));
        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Goodbye {
                reason: GoodbyeReason::Replace
            }
        );
        drop(host);
    }

    #[test]
    fn a_quit_releases_a_pick_at_once_and_does_not_wait_for_it() {
        let (_home, host, quit_requested) = quittable_host("shell-pick-quit");
        let mut stream = attached_shell(&host);
        let route = host.route();

        let picked = in_background(move || route.pick_file("Choose"));
        next_act(&mut stream);
        let began = Instant::now();
        host.quit_handle().begin(QuitReason::Quit, QUIT_GRACE);

        assert!(matches!(
            picked.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::Unavailable)
        ));
        quit_requested
            .recv_timeout(PATIENCE)
            .expect("the desktop is asked to exit");
        assert!(
            began.elapsed() < QUIT_GRACE,
            "a person's open dialog does not hold the quit"
        );
        assert!(
            matches!(
                host.route().pick_file("Too late"),
                Err(WorkbenchPresentationError::Unavailable)
            ),
            "no act starts once a quit has begun"
        );
        drop(host);
    }

    #[test]
    fn a_session_end_releases_a_pick() {
        let (_home, host, _quit_requested) = quittable_host("shell-pick-session-end");
        let mut stream = attached_shell(&host);
        let route = host.route();

        let picked = in_background(move || route.pick_file("Choose"));
        next_act(&mut stream);
        host.quit_handle().session_end();

        assert!(matches!(
            picked.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::Unavailable)
        ));
        drop(host);
    }

    #[test]
    fn a_second_shell_is_told_it_is_already_open_and_the_first_is_shown() {
        let home = TestHome::new("shell-already-open");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut first = attached_shell(&host);

        let (mut second, handed) = open_desktop();

        assert_eq!(handed, DesktopFrame::AlreadyOpen);
        assert_eq!(next_frame(&mut first), DesktopFrame::Show { address: None });
        assert!(
            matches!(
                frame::read_frame::<DesktopFrame>(&mut second),
                Ok(None) | Err(_)
            ),
            "the stream of the shell that was turned away ends"
        );
        assert_eq!(host.quit.shells().attached(), 1);

        host.workbench.reveal(Some(PageAddress::About)).unwrap();
        assert_eq!(
            next_frame(&mut first),
            DesktopFrame::Show {
                address: Some(PageAddress::About)
            },
            "the first shell still serves"
        );

        drop(first);
        eventually(|| host.quit.shells().attached() == 0);
        let (_third, handed) = open_desktop();
        assert!(
            matches!(handed, DesktopFrame::Workbench { .. }),
            "the place is free again: {handed:?}"
        );
        drop(host);
    }

    #[test]
    fn a_shell_started_for_the_tray_that_finds_the_place_taken_brings_no_window_forward() {
        let home = TestHome::new("shell-tray-already-open");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut first = attached_shell(&host);

        let mut tray = desktop_opening();
        tray.channel = Some(ShellView::Tray.as_str().to_owned());
        let (mut second, reply) = dial(&tray);
        assert!(reply.accepted, "{reply:?}");
        assert_eq!(
            frame::read_frame::<DesktopFrame>(&mut second).unwrap(),
            Some(DesktopFrame::AlreadyOpen)
        );

        host.workbench.reveal(Some(PageAddress::About)).unwrap();
        assert_eq!(
            next_frame(&mut first),
            DesktopFrame::Show {
                address: Some(PageAddress::About)
            },
            "the first frame the holder hears is this show: the tray shell asked for none"
        );
        drop(host);
    }

    #[test]
    fn with_no_shell_a_notice_and_an_act_are_unavailable_and_a_show_has_no_shell_to_start() {
        let home = TestHome::new("shell-none");
        // A test home has no shell file, so presence starts nothing.
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        for result in [
            host.workbench.reveal(None),
            host.workbench.test_notification(),
        ] {
            assert!(
                matches!(
                    result,
                    Err(crate::workbench::WorkbenchError::PresentationUnavailable)
                ),
                "{result:?}"
            );
        }
        assert!(host
            .workbench
            .open_destination(crate::workbench::WorkbenchDestination::Home)
            .is_err());
        drop(host);
    }

    #[test]
    fn showing_the_window_with_no_shell_starts_one_without_tray_and_shows_it_when_it_attaches() {
        let home = TestHome::new("shell-show-launch");
        let fake = Arc::new(Fake::default());
        let host = ServiceHost::start_in_world(home.path(), {
            let fake = Arc::clone(&fake);
            move |_, _| fake.world()
        })
        .unwrap();
        host.publish_ready().unwrap();

        let asked = {
            let workbench = host.workbench.clone();
            in_background(move || workbench.reveal(Some(PageAddress::SettingsSharing)))
        };
        eventually(|| fake.launches() == [false]);
        let (mut stream, handed) = open_desktop();
        assert!(matches!(handed, DesktopFrame::Workbench { .. }));

        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Show {
                address: Some(PageAddress::SettingsSharing)
            },
            "the address asked for before the shell attached is shown when it does"
        );
        assert!(asked
            .recv_timeout(PATIENCE)
            .expect("the request is answered once the shell is attached")
            .is_ok());
        assert_eq!(
            fake.launches(),
            [false],
            "one launch, and not a tray one: the window is asked for"
        );
        drop(host);
    }

    #[test]
    fn the_status_names_the_window_by_what_presence_knows() {
        use crate::language::workbench::WindowReason;

        let home = TestHome::new("status-window");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let window = || cli_client::status_at(home.path()).unwrap().window;
        assert_eq!(window(), None, "presence has nothing to say yet");

        let (mut stream, _) = open_desktop();
        eventually(|| window() == Some(WindowReason::Shown));
        frame::write_frame(&mut stream, &DesktopFrame::Closed { stays: true }).unwrap();
        eventually(|| window() == Some(WindowReason::Tray));
        frame::write_frame(
            &mut stream,
            &DesktopFrame::Unavailable {
                reason: crate::intake::desktop::UnavailableReason::NoWebview,
            },
        )
        .unwrap();
        // A click is answered after the frame before it was read, so the reason is recorded.
        frame::write_frame(
            &mut stream,
            &DesktopFrame::Tray {
                item: TrayItemId::Hold,
            },
        )
        .unwrap();
        while !matches!(next_frame(&mut stream), DesktopFrame::TrayModel { .. }) {}
        drop(stream);
        eventually(|| window() == Some(WindowReason::NoWebview));
        drop(host);
    }

    #[test]
    fn a_tray_open_after_an_agent_started_shell_or_a_close_that_stays_reads_shown() {
        use crate::language::workbench::WindowReason;

        let home = TestHome::new("status-open");
        let fake = Arc::new(Fake::default());
        let host = ServiceHost::start_in_world(home.path(), {
            let fake = Arc::clone(&fake);
            move |_, _| fake.world()
        })
        .unwrap();
        host.publish_ready().unwrap();
        let window = || cli_client::status_at(home.path()).unwrap().window;
        let open = |stream: &mut _| {
            frame::write_frame(
                stream,
                &DesktopFrame::Tray {
                    item: TrayItemId::Open,
                },
            )
            .unwrap();
        };

        // Presence's own tray launch attaches with the icon only.
        host.intake.presence.published();
        fake.advance(super::presence::LAUNCH_DELAY);
        host.intake.presence.tick();
        assert_eq!(fake.launches(), [true]);
        let (mut stream, _) = open_desktop();
        eventually(|| window() == Some(WindowReason::Tray));
        open(&mut stream);
        eventually(|| window() == Some(WindowReason::Shown));

        frame::write_frame(&mut stream, &DesktopFrame::Closed { stays: true }).unwrap();
        eventually(|| window() == Some(WindowReason::Tray));
        open(&mut stream);
        eventually(|| window() == Some(WindowReason::Shown));
        drop(host);
    }

    #[test]
    fn a_tray_open_brings_the_window_to_setup_until_setup_is_complete() {
        use crate::install::setup_state::{self, SetupRecord};
        use crate::workbench::setup::SetupProgress;

        let home = TestHome::new("tray-open-setup");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);
        let open = |stream: &mut _| {
            frame::write_frame(
                stream,
                &DesktopFrame::Tray {
                    item: TrayItemId::Open,
                },
            )
            .unwrap();
        };

        open(&mut stream);
        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Show {
                address: Some(PageAddress::Setup)
            },
            "an open window comes back to setup"
        );

        // With setup complete the tray asks for nothing more than the shell already does.
        let record = host.intake.layout.setup_record();
        assert!(setup_state::record_if_absent(&record, &SetupRecord::now()).unwrap());
        host.workbench.attach_setup(SetupProgress::load(&record));
        open(&mut stream);
        frame::write_frame(
            &mut stream,
            &DesktopFrame::Tray {
                item: TrayItemId::Hold,
            },
        )
        .unwrap();
        while !matches!(next_frame(&mut stream), DesktopFrame::TrayModel { .. }) {}
        host.workbench.reveal(Some(PageAddress::About)).unwrap();
        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Show {
                address: Some(PageAddress::About)
            },
            "no show was written for the open"
        );
        drop(host);
    }

    #[test]
    fn a_show_the_shell_cannot_build_reads_as_the_reason_while_the_shell_is_attached() {
        use crate::intake::desktop::UnavailableReason;
        use crate::language::workbench::WindowReason;

        let home = TestHome::new("status-show-fails");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let window = || cli_client::status_at(home.path()).unwrap().window;
        let (mut stream, _) = open_desktop();
        eventually(|| window() == Some(WindowReason::Shown));

        host.workbench.reveal(Some(PageAddress::About)).unwrap();
        assert!(matches!(next_frame(&mut stream), DesktopFrame::Show { .. }));
        frame::write_frame(
            &mut stream,
            &DesktopFrame::Unavailable {
                reason: UnavailableReason::NoWebview,
            },
        )
        .unwrap();
        eventually(|| window() == Some(WindowReason::NoWebview));
        assert_eq!(host.quit.shells().attached(), 1, "the shell is still there");
        drop(host);
    }

    #[test]
    fn a_later_show_the_shell_builds_clears_the_reason_it_gave_before() {
        use crate::intake::desktop::UnavailableReason;
        use crate::language::workbench::WindowReason;

        let home = TestHome::new("status-show-recovers");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let window = || cli_client::status_at(home.path()).unwrap().window;
        let (mut stream, _) = open_desktop();
        eventually(|| window() == Some(WindowReason::Shown));

        host.workbench.reveal(Some(PageAddress::About)).unwrap();
        assert!(matches!(next_frame(&mut stream), DesktopFrame::Show { .. }));
        frame::write_frame(
            &mut stream,
            &DesktopFrame::Unavailable {
                reason: UnavailableReason::NoWebview,
            },
        )
        .unwrap();
        eventually(|| window() == Some(WindowReason::NoWebview));

        host.workbench.reveal(None).unwrap();
        assert!(matches!(next_frame(&mut stream), DesktopFrame::Show { .. }));
        eventually(|| window() == Some(WindowReason::Shown));
        drop(host);
    }

    #[test]
    fn the_first_close_that_stays_is_explained_once_and_never_again() {
        let home = TestHome::new("tray-hint");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let hint = home.path().join("state").join("tray-hint");
        let (mut stream, _) = open_desktop();

        frame::write_frame(&mut stream, &DesktopFrame::Closed { stays: true }).unwrap();
        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Notice {
                title: crate::language::workbench::TRAY_HINT_TITLE.to_owned(),
                body: crate::language::workbench::TRAY_HINT_BODY.to_owned(),
            }
        );
        assert!(hint.is_file(), "the marker is written");

        // A second close says nothing: a Hold click is answered by a tray model, not a notice.
        frame::write_frame(&mut stream, &DesktopFrame::Closed { stays: true }).unwrap();
        frame::write_frame(
            &mut stream,
            &DesktopFrame::Tray {
                item: TrayItemId::Hold,
            },
        )
        .unwrap();
        assert!(
            matches!(next_frame(&mut stream), DesktopFrame::TrayModel { .. }),
            "no second notice"
        );
        drop(host);
    }

    #[test]
    fn a_close_that_does_not_stay_says_nothing_and_leaves_no_marker() {
        let home = TestHome::new("tray-hint-none");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let (mut stream, _) = open_desktop();
        frame::write_frame(&mut stream, &DesktopFrame::Closed { stays: false }).unwrap();
        frame::write_frame(
            &mut stream,
            &DesktopFrame::Tray {
                item: TrayItemId::Hold,
            },
        )
        .unwrap();
        assert!(matches!(
            next_frame(&mut stream),
            DesktopFrame::TrayModel { .. }
        ));
        assert!(!home.path().join("state").join("tray-hint").exists());
        drop(host);
    }

    #[test]
    fn a_service_with_no_workbench_refuses_the_desktop_role_as_unsupported() {
        let home = TestHome::new("desktop-no-workbench");
        let served = serve_fake(&home);
        let client = served.over_a_pair();

        let reply = protocol::open(&client, &desktop_opening(), Instant::now() + PATIENCE).unwrap();

        assert!(!reply.accepted);
        assert_eq!(reply.code, Some(RefusalCode::Unsupported));
        assert_eq!(reply.say, Some(OpeningRefusal::Unsupported.message()));
    }

    #[test]
    fn a_desktop_stream_that_sends_a_frame_it_cannot_read_ends_alone() {
        let home = TestHome::new("desktop-garbage");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);

        frame::write_frame_raw(&mut stream, b"{ not json at all").unwrap();

        assert!(matches!(
            frame::read_frame::<DesktopFrame>(&mut stream),
            Ok(None) | Err(_)
        ));
        eventually(|| host.quit.shells().attached() == 0);
        let (mut other, _) = open_desktop();
        frame::write_frame(
            &mut other,
            &DesktopFrame::Tray {
                item: TrayItemId::Hold,
            },
        )
        .unwrap();
        eventually(|| {
            host.workbench.snapshot().configuration.runtime_state == RuntimeControlState::Held
        });
        drop(host);
    }

    /// The idle limit of the tests of the idle exit. It is judged on a clock the test moves, so
    /// real time passing never reaches it, however loaded the machine; in real time it only sets
    /// how often the watch looks, a tenth of it.
    const IDLE_LIMIT: Duration = Duration::from_secs(1);

    /// Terms of an agent start whose service idles out after [`IDLE_LIMIT`].
    fn idling() -> StartTerms {
        StartTerms {
            idle_exit: IDLE_LIMIT,
            ..agent()
        }
    }

    /// A world that also ends when a quit has begun, as the service's real one does, and
    /// otherwise acts as `inner`.
    struct EndsWithQuit(Box<dyn super::presence::Outside>, super::presence::Ending);

    impl super::presence::Outside for EndsWithQuit {
        fn now(&self) -> Instant {
            self.0.now()
        }

        fn display(&self) -> Result<(), crate::language::workbench::WindowReason> {
            self.0.display()
        }

        fn launch(&self, tray: bool) -> Result<(), super::presence::LaunchRefusal> {
            self.0.launch(tray)
        }

        fn ending(&self) -> bool {
            self.0.ending() || (self.1)()
        }
    }

    /// A published service that idles out after [`IDLE_LIMIT`] on `clock`, and the channel its
    /// quit signal sends on. The host is dropped before its home.
    struct Idling {
        host: ServiceHost,
        home: TestHome,
        clock: HandClock,
        quit_requested: std::sync::mpsc::Receiver<()>,
    }

    impl Idling {
        /// Wait until the idle watch has looked twice from now, and assert that the service
        /// asked for no exit, saying `why` it stays. A look is counted before it reads the clock,
        /// so the first sees everything done before this call; the second comes only because the
        /// first did not leave.
        fn looked_and_stayed(&self, why: &str) {
            let streams = &self.host.intake.streams;
            let before = streams.looks();
            let until = Instant::now() + PATIENCE;
            while streams.looks() < before + 2 {
                assert!(Instant::now() < until, "the watch stopped looking: {why}");
                thread::sleep(Duration::from_millis(5));
            }
            assert!(self.quit_requested.try_recv().is_err(), "{why}");
        }

        /// Move the idle clock well past the limit and show that the service stays.
        fn stays_past_the_limit(&self, why: &str) {
            self.clock.advance(IDLE_LIMIT * 3);
            self.looked_and_stayed(why);
        }

        /// Wait until no stream is open, so the last close has armed the limit, then move the
        /// idle clock past the limit and expect the quit within [`PATIENCE`].
        fn leaves_past_the_limit(&self, why: &str) {
            eventually(|| self.host.intake.streams.open() == 0);
            self.clock.advance(IDLE_LIMIT * 2);
            self.quit_requested.recv_timeout(PATIENCE).expect(why);
        }
    }

    /// A published host whose idle limit runs on a clock the test moves, whose quit signal is a
    /// channel and whose presence acts on `fake`.
    fn idling_host(name: &str, fake: &Arc<Fake>) -> Idling {
        let home = TestHome::new(name);
        let clock = HandClock::default();
        let fake = Arc::clone(fake);
        let host = ServiceHost::start_with_world(
            home.path(),
            idling(),
            || Ok(false),
            move |_, ending| Box::new(EndsWithQuit(fake.world(), ending)),
            Arc::new(clock.clone()),
        )
        .unwrap();
        let (quit, quit_requested) = std::sync::mpsc::channel();
        host.install_quit_signal(Arc::new(move || {
            let _ = quit.send(());
        }));
        host.publish_ready().unwrap();
        Idling {
            host,
            home,
            clock,
            quit_requested,
        }
    }

    #[test]
    fn a_service_with_no_stream_exits_and_a_browser_start_after_it_is_not_refused() {
        let idling = idling_host("idle-exit", &Arc::default());
        idling.leaves_past_the_limit("the idle service asks the process to exit");
        let Idling { host, home, .. } = idling;
        assert!(
            !super::latch::is_set(&home.path().join("state").join("quit")),
            "an idle exit is not a person's Quit"
        );
        assert!(
            host.intake.quit.drain.admit().is_err(),
            "no new work is admitted once the service is leaving"
        );
        assert!(
            host.intake.streams.admit().is_none(),
            "no stream is admitted once the service is leaving"
        );
        drop(host);

        let browser = start_for(home.path(), StartTerms::for_reason(StartReason::Browser));
        assert!(browser.is_ok(), "the next browser start serves");
    }

    #[test]
    fn an_open_stream_keeps_the_service_and_the_last_close_arms_the_limit_again() {
        let idling = idling_host("idle-stream", &Arc::default());
        let socket = open_mcp(None);
        idling.stays_past_the_limit("an open stream holds the service");
        drop(socket);
        idling.leaves_past_the_limit("the last close arms the limit and the service leaves");
    }

    #[test]
    fn a_control_call_restarts_the_span() {
        let idling = idling_host("idle-control", &Arc::default());
        for _ in 0..4 {
            idling.clock.advance(IDLE_LIMIT * 9 / 10);
            idling.looked_and_stayed("the last call came less than a limit ago");
            cli_client::status().expect("the service answers a status call");
            eventually(|| idling.host.intake.streams.open() == 0);
        }
        idling.leaves_past_the_limit("the service leaves a limit after the last call");
    }

    #[test]
    fn an_attached_shell_is_a_stream_and_keeps_the_service() {
        let idling = idling_host("idle-shell", &Arc::default());
        let (shell, _handed) = open_desktop();
        idling.stays_past_the_limit("an attached shell holds the service");
        drop(shell);
        idling.leaves_past_the_limit("the service leaves once the shell has gone");
    }

    #[test]
    fn work_the_executor_still_holds_re_arms_the_limit() {
        let idling = idling_host("idle-work", &Arc::default());
        let running = idling.host.quit.drain.admit().unwrap();
        idling.stays_past_the_limit("held work keeps the service");
        drop(running);
        idling.leaves_past_the_limit("the service leaves a limit after the work finished");
    }

    #[test]
    fn a_browser_stream_opened_through_the_intake_keeps_the_service_until_it_ends() {
        use crate::browser::adapter::chromium::protocol4::fake::FakeExtension;
        use crate::browser::adapter::chromium::protocol4::fake::Reply as FakeReply;

        let idling = idling_host("idle-browser", &Arc::default());
        let host = &idling.host;
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
            // The extension is gone: end the stream the service holds as the edge would.
            to_service.1.shutdown();
        });
        let fake = FakeExtension::connect(
            extension,
            &FakeExtension::hello("browser_idle"),
            Box::new(|correlation, _| {
                FakeReply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))
            }),
        );
        drop(fake.wait(|seen| !seen.frames.is_empty()));
        eventually(|| {
            host.browser
                .connected_browsers()
                .iter()
                .any(|browser| browser.id == "browser_idle")
        });
        // The opening is counted out once the browser is attached: from here only the browser
        // holds the service.
        eventually(|| host.intake.streams.open() == 1);
        idling.stays_past_the_limit("an attached browser holds the service");
        fake.close();
        drop(fake);
        idling.leaves_past_the_limit("the service leaves once the browser has gone");
        let _ = carrier.join();
    }

    #[test]
    fn the_browser_that_connected_is_remembered_by_the_next_service_of_the_home() {
        use crate::browser::adapter::chromium::protocol4::fake::FakeExtension;
        use crate::browser::adapter::chromium::protocol4::fake::Reply as FakeReply;
        use crate::browser::BrowserPort as _;

        let home = TestHome::new("last-browser");
        let record = crate::install::layout::Layout::new(home.path()).last_browser_record();
        {
            let host = ServiceHost::start(home.path()).unwrap();
            assert_eq!(host.browser.last_connected_browser(), None);
            let (service, extension) = pair().unwrap();
            // The fake reports the name Chrome, which is the chrome browser.
            let fake = FakeExtension::connect(
                extension,
                &FakeExtension::hello("browser_last"),
                Box::new(|correlation, _| {
                    FakeReply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))
                }),
            );
            host.browser.attach(service).unwrap();
            assert_eq!(
                host.browser.last_connected_browser().as_deref(),
                Some("chrome")
            );
            fake.close();
        }
        let fresh = ServiceHost::start(home.path()).unwrap();
        assert_eq!(
            fresh.browser.last_connected_browser().as_deref(),
            Some("chrome")
        );
        std::fs::write(&record, b"{").unwrap();
        assert_eq!(fresh.browser.last_connected_browser(), None);
    }

    #[test]
    fn a_browser_that_reports_no_name_is_remembered_as_chromium() {
        use crate::browser::adapter::chromium::protocol4::fake::FakeExtension;
        use crate::browser::adapter::chromium::protocol4::fake::Reply as FakeReply;
        use crate::browser::BrowserPort as _;

        let home = TestHome::new("last-browser-unnamed");
        let host = ServiceHost::start(home.path()).unwrap();
        let (service, extension) = pair().unwrap();
        // Plain Chromium's brands are all generic, so its extension reports no name.
        let mut hello = FakeExtension::hello("browser_plain");
        hello.as_object_mut().unwrap().remove("browser_name");
        let fake = FakeExtension::connect(
            extension,
            &hello,
            Box::new(|correlation, _| {
                FakeReply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))
            }),
        );
        host.browser.attach(service).unwrap();
        assert_eq!(
            host.browser.last_connected_browser().as_deref(),
            Some("chromium")
        );
        assert_eq!(
            host.browser.browsers()[0].name.as_deref(),
            Some("Chromium"),
            "the model reads a name, not null"
        );
        fake.close();
    }

    #[test]
    fn a_start_that_arrives_while_the_service_is_leaving_on_idle_waits_for_it_and_serves() {
        let idling = idling_host("idle-exit-race", &Arc::default());
        idling.leaves_past_the_limit("the idle service asks the process to exit");
        let Idling { host, home, .. } = idling;
        // The old service still holds the lock and the endpoint while it tears down.
        let directory = home.path().to_path_buf();
        let starter = thread::spawn(move || {
            let started = Instant::now();
            let served =
                start_for(&directory, StartTerms::for_reason(StartReason::Browser)).map(drop);
            (served, started.elapsed())
        });
        thread::sleep(Duration::from_millis(250));
        assert!(
            !starter.is_finished(),
            "the start waits for the leaving service instead of giving way"
        );
        drop(host);
        let (served, waited) = starter.join().unwrap();
        served.expect("the start takes the lock the old service released");
        assert!(waited < super::lifetime::LEAVING_LOCK_WAIT);
    }

    #[test]
    fn the_end_of_the_session_leaves_once_and_the_idle_path_adds_nothing() {
        let idling = idling_host("idle-session-end", &Arc::default());
        idling.host.quit_handle().session_end();
        idling
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the session end asks the process to exit");
        idling.clock.advance(IDLE_LIMIT * 3);
        let streams = &idling.host.intake.streams;
        let before = streams.looks();
        eventually(|| streams.looks() > before);
        // That look decided to leave and makes its request on the same thread at once; ten of
        // the watch's real poll periods cover it.
        assert!(
            idling.quit_requested.recv_timeout(IDLE_LIMIT).is_err(),
            "the idle limit passing during the exit makes no second request"
        );
    }

    #[test]
    fn presence_launches_no_shell_after_an_idle_stop() {
        let fake = Arc::new(Fake::default());
        let idling = idling_host("idle-presence", &fake);
        idling.leaves_past_the_limit("the idle service asks the process to exit");
        let Idling { host, home, .. } = idling;
        fake.advance(Duration::from_secs(10));
        thread::sleep(Duration::from_millis(1200));
        assert!(
            fake.launches().is_empty(),
            "a service that is leaving starts no shell"
        );
        drop(host);
        drop(home);

        // The control's idle clock never moves, so it stays however long presence takes.
        let live = Arc::new(Fake::default());
        let _idling = idling_host("idle-presence-control", &live);
        live.advance(Duration::from_secs(10));
        eventually(|| live.launches() == vec![true]);
    }
}
