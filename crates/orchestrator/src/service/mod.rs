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
mod toolset;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
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
use crate::audit::JsonlAuditSink;
use crate::browser::adapter::chromium::protocol4::assets::page_assets;
use crate::browser::adapter::{BrowserCommand, BrowserEvent};
use crate::browser::transport::{SocketReader, SocketWriter};
use crate::browser::{
    AdapterLifecycleObserver, BrowserError, BrowserEventSink, BrowserPort, RelayBrowserPort,
};
use crate::diagnostics::DiagnosticsHub;
use crate::governance::{AuthoritySnapshot, GovernanceFacade};
use crate::install;
use crate::install::icons::{self, IconCache, IconPlaces};
use crate::intake::desktop::{DesktopFrame, ShellView, TrayItemId, TrayModel};
use crate::intake::listener::{self, control, Judgement};
use crate::intake::mcp::{
    failure_templates, McpSession, OwnedTab, ResumeState, Seating, Step, OLDEST_REVISION,
};
use crate::language::catalog_for;
use crate::language::environment::{ELEVATED_SERVICE, QUIT_HELD};
use crate::language::intake::OpeningRefusal;
use crate::language::outcome::{CancelCause, DialogKind, QuitCause};
use crate::language::workbench::{TRAY_HINT_BODY, TRAY_HINT_TITLE};
use crate::presentation::{BrowserPresentation, PresentationReactor};
use crate::provenance::{Attribution, ConnectionEvidence, SourceReader};
use crate::sync::lock;
use crate::work::{Admission, ApplicationExecutor, PreparedInvocation};
use crate::workbench::setup::SetupProgress;
use crate::workbench::web::{PendingWorkbench, WorkbenchAddress, WorkbenchServer};
use crate::workbench::{
    PageAddress, RuntimeSource, Watching, WorkbenchFacade, WorkbenchProjection, WorkbenchQuitPort,
    WorkbenchTrayPort,
};
use crate::workspace::{ReleasedTabs, UrlDigest, WorkspaceId, WorkspaceStore};
use presence::Presence;
use shell::{ShellRoute, Shells};
use toolset::Toolsets;

/// The most tabs one diagnostics clear names: the extension takes no more, and the protocol 4
/// vocabulary's `clear_diagnostics_tabs` pins the two together.
pub(crate) const DIAGNOSTIC_CLEAR_BATCH_SIZE: usize = 256;

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

/// Wake every edge that waits for a service, from a thread of its own so that publication never
/// waits on an edge, and record what it woke (ADR-0199 amendment of 2026-10-20).
fn wake_waiting_edges(home: &Path, diagnostics: &DiagnosticsHub) {
    let home = home.to_path_buf();
    let sink = diagnostics.sink();
    thread::spawn(move || {
        let wakes = ghostlight_relay::wake::wake_all(&home);
        if wakes != ghostlight_relay::wake::Wakes::default() {
            sink.emit(
                ghostlight_bridge::diagnostics::event::EDGES_WOKEN,
                ghostlight_bridge::diagnostics::Level::Info,
                None,
                &format!("woken={} removed={}", wakes.woken, wakes.removed),
            );
        }
    });
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
            toolsets: Toolsets::new(Arc::clone(&quit.drain)),
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
        // browser waits for them from this moment, not from construction. After a replacement
        // they are the browsers of the service that just ended, on their relays' backoff.
        if lifetime::replaces_a_service(self.intake.started_by) {
            self.browser.replacement_published();
        } else {
            self.browser.endpoint_published();
        }
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
        wake_waiting_edges(&self.home, &self.diagnostics);
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
        if !self.drain.begin_quit(quit_cause(reason)) {
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
        self.drain.begin_quit(QuitCause::SignOut);
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
        if !self.drain.begin_quit(QuitCause::Idle) {
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

/// What a call that a quit for `reason` cancels is told about it.
fn quit_cause(reason: QuitReason) -> QuitCause {
    match reason {
        QuitReason::Quit => QuitCause::Quit,
        QuitReason::Replace => QuitCause::Replace,
        QuitReason::Uninstall => QuitCause::Uninstall,
        QuitReason::Unknown => QuitCause::Unknown,
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
        cleanup_released_tabs(&released, self.port.as_ref(), &self.workspaces, None);
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
            wire.hand_last_resume(&self.workspaces, self.port.as_ref(), keeps_tabs);
        }
        for wire in &streams {
            wire.goodbye(goodbye);
        }
        self.shells.say_goodbye(goodbye);
        self.release_all_but(&kept, &streams, until, quit_cause(reason));
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
    /// The work of each session that ends is cancelled first, for `quit`, so a running call can
    /// finish and give its workspace up. Workspaces no call holds are released before any is
    /// waited for.
    fn release_all_but(
        &self,
        kept: &HashSet<WorkspaceId>,
        streams: &[Arc<McpWire>],
        until: Instant,
        quit: QuitCause,
    ) {
        let ending: Vec<WorkspaceId> = self
            .workspaces
            .ids()
            .into_iter()
            .filter(|workspace| !kept.contains(workspace))
            .collect();
        for live in streams.iter().filter_map(|wire| wire.live.get()) {
            if ending.contains(&live.workspace) {
                live.queue.stop(CancelCause::Quit(quit));
            }
        }
        let mut held = Vec::new();
        for workspace in ending {
            match self.workspaces.release_unleased(&workspace, Instant::now()) {
                Some(released) => self.close_released(&released, until),
                None => held.push(workspace),
            }
        }
        for workspace in held {
            if let Some(released) = self.workspaces.release_unleased(&workspace, until) {
                self.close_released(&released, until);
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
    fn close_released(&self, released: &ReleasedTabs, until: Instant) {
        cleanup_released_tabs(released, self.port.as_ref(), &self.workspaces, Some(until));
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
    /// Every live MCP connection, and which offers each agent the tools.
    toolsets: Arc<Toolsets>,
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
/// Once the opening is answered, the connection's attribution is minted with the accept time, and
/// the app that started the session is read once from the stream's peer process. The opening's
/// answer never waits on that read, and nothing the client sends takes part in it. The client's
/// `initialize` does wait on it: whether the session stands by, because another connection of
/// the same agent already offers the tools, decides its instructions ([`toolset`]).
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
    // A client that sent a token initialized with an earlier service and never initializes
    // again, so a token that cannot be read still resumes an initialized session.
    let resumed = requested.resume.as_deref().map(|token| {
        ResumeState::decode(token).unwrap_or_else(|error| {
            intake.diagnostics.sink().emit(
                ghostlight_bridge::diagnostics::event::HARNESS_ATTACHED,
                ghostlight_bridge::diagnostics::Level::Warn,
                None,
                &format!(
                    "{error}; the session resumes at MCP revision {OLDEST_REVISION} with no tab"
                ),
            );
            ResumeState::unreadable()
        })
    });
    listener::accept(&connection, &intake.accepted(Role::Mcp))?;
    let attribution =
        Attribution::accepted(IntakeChannel::of_opening(requested.channel.as_deref()));
    let observation = intake.sources.read(connection.peer_process_id());
    let accepted = Accepted {
        attribution: attribution.observed(observation.as_ref()),
        marker,
    };
    let mut reader =
        SocketReader::new(connection.try_clone()?).interrupted_by(Arc::clone(&intake.stop));
    let writer = Arc::new(SocketWriter::new(connection)?);
    let (session, resumed_tabs) = match resumed {
        Some(state) => {
            let kept = KeptTabs {
                owned: state.owned_tabs.clone(),
                lives: state.lives.clone(),
            };
            (McpSession::resumed(state), Some(kept))
        }
        None => (McpSession::new(), None),
    };
    let wire = Arc::new(McpWire::new(Arc::clone(&writer), session));
    let _enrolled = intake.quit.enroll(Arc::clone(&wire));
    let seat = intake.toolsets.enter(Arc::clone(&wire));
    wire.seat(seat.seating(accepted.attribution.channel, observation.as_ref()));

    let mut live = match resumed_tabs {
        Some(kept) => {
            let live = LiveSession::open(intake, &wire, &accepted)?;
            // Its client listed tools from an earlier service and will not list them again, so
            // the session watch tells it to, unless the session stands by with no tools.
            if !wire.stands_by() {
                live.advertised.given_earlier();
            }
            remember_resumed_tabs(&intake.workspaces, &live.workspace, &kept);
            wire.publish_resume(
                &intake.workspaces,
                intake.browser_port.as_ref(),
                &live.workspace,
            );
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
            // far, so its `initialize` answer never waits on opening its workspace.
            let mut open_when_initialized = |live: &mut Option<LiveSession>| -> Result<()> {
                if live.is_none() && wire.is_initialized() {
                    *live = Some(LiveSession::open(intake, &wire, &accepted)?);
                    opened = true;
                }
                Ok(())
            };
            let initialized_before = wire.is_initialized();
            let steps = wire.accept(&line);
            if !initialized_before && wire.is_initialized() {
                wire.announce_initialized();
            }
            for step in steps {
                match step {
                    Step::Reply(reply) => wire.send(&reply),
                    Step::Forward(request) => {
                        open_when_initialized(&mut live)?;
                        if let Some(live) = &live {
                            live.handle(intake, &wire, request);
                        }
                    }
                }
            }
            open_when_initialized(&mut live)?;
            if let (true, Some(live)) = (opened, &live) {
                wire.publish_resume(
                    &intake.workspaces,
                    intake.browser_port.as_ref(),
                    &live.workspace,
                );
            }
        }
        Ok(())
    })();

    writer.close();
    if let Some(live) = live {
        live.close(intake);
    }
    drop(seat);
    served
}

/// An accepted `mcp` stream: what the service knew of it once it answered the opening and read
/// its source.
struct Accepted {
    /// The connection's identity, accept time, declared channel and source.
    attribution: Attribution,
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
    /// The tabs the last token restores, so a promotion writes the same tabs again.
    kept: KeptTabs,
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

    /// Let `seating` decide whether the session stands by ([`McpSession::seat`]).
    fn seat(&self, seating: Seating) {
        lock(&self.session).seat(seating);
    }

    /// Whether the session stands by, so its catalog is empty.
    fn stands_by(&self) -> bool {
        lock(&self.session).stands_by()
    }

    /// Stop standing by, hand the edge a token that says so, and then tell the client its tool
    /// list changed. A restart after the promotion resumes the session offering the tools its
    /// client was told about.
    fn promote(&self) {
        let lines = lock(&self.session).promote();
        let mut resume = lock(&self.resume);
        if !resume.sealed {
            let kept = resume.kept.clone();
            self.write_resume(&mut resume, kept);
        }
        drop(resume);
        for line in &lines {
            self.send(line);
        }
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
    /// of its URL, never the URL, so the edge holds no page address, and each browser carries the
    /// extension life its tabs were confirmed in, read from `port`.
    fn publish_resume(
        &self,
        workspaces: &WorkspaceStore,
        port: &dyn BrowserPort,
        workspace: &WorkspaceId,
    ) {
        let mut resume = lock(&self.resume);
        if !resume.sealed {
            self.write_resume(&mut resume, kept_tabs(workspaces, port, workspace));
        }
    }

    /// Hand the edge the first token of a session that has just been initialized, ahead of the
    /// `initialize` answer, so a client that holds its answer always has a token to resume with
    /// (ADR-0203 Decision 1). A fresh session holds no tab, so this token is the one a service end
    /// would hand it, and it is written even after an end handed the stream its last.
    fn announce_initialized(&self) {
        let mut resume = lock(&self.resume);
        self.write_resume(&mut resume, KeptTabs::default());
    }

    /// Hand the edge the last resume token of this service, with the session's tabs only when
    /// `keeps_tabs` says its workspace stays bound, and publish nothing after it.
    ///
    /// A session that ends at a quit resumes later with no tab, so its handles read as no longer
    /// open instead of rebinding tabs the release left behind.
    fn hand_last_resume(
        &self,
        workspaces: &WorkspaceStore,
        port: &dyn BrowserPort,
        keeps_tabs: bool,
    ) {
        let mut resume = lock(&self.resume);
        if resume.sealed {
            return;
        }
        let kept = match self.workspace() {
            Some(workspace) if keeps_tabs => kept_tabs(workspaces, port, &workspace),
            _ => KeptTabs::default(),
        };
        self.write_resume(&mut resume, kept);
        resume.sealed = true;
    }

    /// Write the token that restores `kept`, unless it is the one last written or the session is
    /// not initialized yet.
    fn write_resume(&self, resume: &mut Resume, kept: KeptTabs) {
        let Some(state) = lock(&self.session).resume_state(kept.owned.clone(), kept.lives.clone())
        else {
            return;
        };
        let token = state.encode();
        if resume.last.as_deref() != Some(token.as_str()) {
            if self
                .writer
                .native(&McpFrame::Resume {
                    token: token.clone(),
                })
                .is_err()
            {
                return;
            }
            resume.last = Some(token);
        }
        resume.kept = kept;
    }
}

/// What a resume token restores: the tabs, and the extension life of each of their browsers.
#[derive(Clone, Default)]
struct KeptTabs {
    owned: Vec<OwnedTab>,
    lives: BTreeMap<String, String>,
}

/// The tabs a resume token restores for `workspace`: every binding, a pending one included, by
/// its own browser and the digest of its URL, and the extension life of each of their browsers.
///
/// A pending binding remembers the life it was confirmed in. A confirmed one is in the life its
/// browser's connection reports, so those lives are read from `port` before the bindings are:
/// a gap marks a browser's bindings pending before a new connection takes its place, so a binding
/// still confirmed after that read was confirmed in the life it returned. A browser whose tabs
/// name two lives, or none, carries no life, and its tabs are confirmed by their URL digests.
fn kept_tabs(
    workspaces: &WorkspaceStore,
    port: &dyn BrowserPort,
    workspace: &WorkspaceId,
) -> KeptTabs {
    let browsers: BTreeSet<String> = workspaces
        .tab_bindings(workspace.as_str())
        .into_iter()
        .map(|binding| binding.browser)
        .collect();
    let current: BTreeMap<String, Option<String>> = browsers
        .into_iter()
        .map(|browser| {
            let life = port.life_of(&browser);
            (browser, life)
        })
        .collect();
    let bindings = workspaces.tab_bindings(workspace.as_str());
    let mut lives: BTreeMap<String, Option<String>> = BTreeMap::new();
    for binding in &bindings {
        let life = if binding.pending {
            binding.life.clone()
        } else {
            current.get(&binding.browser).cloned().flatten()
        };
        lives
            .entry(binding.browser.clone())
            .and_modify(|known| {
                if *known != life {
                    *known = None;
                }
            })
            .or_insert(life);
    }
    KeptTabs {
        owned: bindings
            .into_iter()
            .map(|binding| OwnedTab {
                browser_id: binding.browser,
                tab_id: binding.physical_id,
                handle: binding.handle,
                url_digest: Some(binding.url.as_str().to_owned()),
            })
            .collect(),
        lives: lives
            .into_iter()
            .filter_map(|(browser, life)| Some((browser, life?)))
            .collect(),
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
    /// None when an earlier service gave the client its catalog, which this one never saw.
    tools: Option<Vec<ToolDefinition>>,
}

impl AdvertisedCatalog {
    /// Build the catalog `authority` admits and remember it as what the client now has.
    fn advertise(&self, authority: &AuthoritySnapshot) -> Vec<ToolDefinition> {
        let tools = catalog_for(authority);
        *lock(&self.0) = Some(Advertised {
            authority: authority.id().to_owned(),
            tools: Some(tools.clone()),
        });
        tools
    }

    /// Remember that an earlier service gave the client a catalog this one never saw: the client
    /// of a resumed session listed its tools before the restart and does not list them again on
    /// its own. The next comparison therefore tells it to, whatever the authority is now.
    fn given_earlier(&self) {
        *lock(&self.0) = Some(Advertised {
            authority: String::new(),
            tools: None,
        });
    }

    /// The catalog `authority` admits when it differs from the one given, and `None` otherwise.
    ///
    /// Nothing is compared until the client has a catalog, from this service or an earlier one, and
    /// the catalog is rebuilt only when the authority itself changed.
    fn changed_by(&self, authority: &AuthoritySnapshot) -> Option<Vec<ToolDefinition>> {
        let mut guard = lock(&self.0);
        let advertised = guard.as_mut()?;
        if advertised.authority == authority.id() {
            return None;
        }
        advertised.authority = authority.id().to_owned();
        let tools = catalog_for(authority);
        if advertised.tools.as_ref() == Some(&tools) {
            return None;
        }
        advertised.tools = Some(tools.clone());
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
    /// This is where the connection's one evidence is built, from its observed attribution and
    /// its client's claim.
    fn open(intake: &Arc<Intake>, wire: &Arc<McpWire>, accepted: &Accepted) -> Result<Self> {
        // Before opening anything, release workspaces whose owner is gone and close the tabs they
        // still hold. Sweeping here rather than on a timer keeps the cost proportional to use.
        reap_finished_sessions(&intake.workspaces, intake.browser_port.as_ref());
        // Evidence belongs to this connection. Workspace continuity never replaces it.
        let connection = Arc::new(ConnectionEvidence::new(
            accepted.attribution.clone(),
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
                    wire.publish_resume(
                        &intake.workspaces,
                        intake.browser_port.as_ref(),
                        &workspace,
                    );
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
        let port = Arc::clone(&intake.browser_port);
        let wire = Arc::clone(wire);
        thread::Builder::new()
            .name("ghostlight-session-work".into())
            .spawn(move || {
                while let Some(request) = queue.next(independent) {
                    let result = executor.execute_prepared(&request, Admission::Queued);
                    deliver_result(&wire, result);
                    wire.publish_resume(&workspaces, port.as_ref(), &workspace);
                    queue.complete(&request.id);
                }
            })
            .context("start bounded session worker")
    }

    /// Serve one request the engine forwarded.
    ///
    /// A call the queue does not take is answered here, on the thread that reads the stream, so
    /// its answer is written before the stream can close: over capacity, it is refused; while
    /// Ghostlight quits, it is cancelled before it started, with the quit's cause. Each goes
    /// through the one completion path and its audit line.
    fn handle(&self, intake: &Arc<Intake>, wire: &Arc<McpWire>, request: ServiceRequest) {
        match request {
            ServiceRequest::Catalog => {
                let tools = if wire.stands_by() {
                    Vec::new()
                } else {
                    self.advertised.advertise(&intake.governance.snapshot())
                };
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
                let (admission, _flight) = match self.queue.admit(Arc::clone(&request), bytes) {
                    Ok(()) => return,
                    Err(Refused::Full) => match self.queue.begin_unqueued() {
                        Ok(flight) => (Admission::OverCapacity, Some(flight)),
                        Err(by) => (Admission::Cancelled(by), None),
                    },
                    Err(Refused::Cancelled(by)) => (Admission::Cancelled(by), None),
                };
                let result = intake.executor.execute_prepared(&request, admission);
                deliver_result(wire, result);
                wire.publish_resume(
                    &intake.workspaces,
                    intake.browser_port.as_ref(),
                    &self.workspace,
                );
            }
            ServiceRequest::Cancel { id } => self.queue.cancel(&id),
        }
    }

    /// Stop the workers, detach the connection, and release a workspace nobody owns unless a
    /// service end has taken over, which then decides it.
    ///
    /// The work still admitted is cancelled because Ghostlight quit, while a quit drains, and
    /// because the session ended otherwise.
    fn close(self, intake: &Intake) {
        self.watch_stop.store(true, Ordering::SeqCst);
        self.queue.stop(
            intake
                .quit
                .drain
                .cause()
                .map_or(CancelCause::SessionEnded, CancelCause::Quit),
        );
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

/// Bring a resumed session's tabs back as pending bindings, each in its own browser and with the
/// extension life the token names for that browser.
///
/// No browser is asked here: the session's first call confirms each tab against its browser's
/// own list (ADR-0203 Decision 1). A tab written without a URL digest, by a build before digests,
/// cannot be confirmed and is not brought back, so its handle names no open tab.
fn remember_resumed_tabs(workspaces: &WorkspaceStore, workspace: &WorkspaceId, kept: &KeptTabs) {
    for tab in &kept.owned {
        if let Some(url) = tab.url_digest.as_deref().and_then(UrlDigest::parse) {
            workspaces.remember_tab(
                workspace,
                &tab.handle,
                &tab.browser_id,
                tab.tab_id,
                url,
                kept.lives.get(&tab.browser_id).map(String::as_str),
            );
        }
    }
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
        cleanup_released_tabs(&released, browser, workspaces, None);
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
            BrowserEvent::Disconnected { life } => {
                self.workspaces
                    .apply_browser_disconnect(browser, life.as_deref());
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
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::intake::session::{IntakeChannel, SessionMarker};
    use crate::provenance::{ConnectionEvidence, Observation, Source, SourceProcess, SourceReader};
    use ghostlight_relay::endpoint::{pair, Connection, Listener};
    use ghostlight_relay::frame;
    use ghostlight_relay::protocol::{
        self, Control, Edge, GoodbyeReason, McpFrame, Opening as RelayOpening, Program, QuitReason,
        Reply, Role, StartReason,
    };
    use serde_json::{json, Value};

    use super::lifetime::HandClock;
    use super::presence::testing::Fake;
    use super::test_home::TestHome;
    use crate::browser::adapter::BrowserCommand;
    use crate::browser::adapter::RuntimeControlState;
    use crate::browser::testing::{FakeBrowser, FAKE_BROWSER};
    use crate::cli::client as cli_client;
    use crate::install::layout::Layout;
    use crate::intake::desktop::{
        ActOutcome, DesktopFrame, NativeAct, ShellView, TrayItemId, TrayModel,
    };
    use crate::intake::listener::control;
    use crate::intake::mcp::failure_templates;
    use crate::intake::mcp::ResumeState;
    use crate::language::outcome::{CancelCause, QuitCause, Refusal};
    use crate::scratch::Scratch;
    use crate::work::result::{Effect, Status};
    use crate::workbench::{
        PageAddress, WorkbenchNativeActs, WorkbenchNotification, WorkbenchPresentationError,
        WorkbenchPresentationPort,
    };
    use crate::workspace::ReleasedTabs;

    use super::{
        cleanup_released_tabs, AuthorityAlreadyRunning, QuitLatched, ServiceHost, StartTerms,
        QUIT_GRACE,
    };

    /// The MCP revision the test clients speak and the resume tokens name.
    const REVISION: &str = "2025-06-18";

    /// The name the client of an ordinary test session claims.
    const CLIENT_NAME: &str = "relay test";

    /// The name the client of a session that returns with a resume token claims.
    const RETURNING_CLIENT: &str = "returning client";

    /// The terms of an agent start, which asks for the lock once.
    fn agent() -> StartTerms {
        StartTerms::asking_once(StartReason::Agent)
    }

    /// A fresh directory to serve from, removed when the returned guard drops.
    fn temp_home(name: &str) -> Scratch {
        Scratch::named(&format!("service-{name}"))
    }

    /// ACCEPTANCE B15 (ADR-0164) and defect d8d27be0: a page the person loads in a controlled tab
    /// never authorizes, audits or reaches the executor, and a tab Ghostlight never opened stays
    /// unowned; the next agent request checks the current page and is refused there.
    #[test]
    fn passive_browsing_never_becomes_work_or_claims_a_child_tab() {
        use crate::audit::{AuditRecorder, JsonlAuditSink};
        use crate::browser::adapter::{BrowserEvent, BrowserReadiness, PhysicalTab};
        use crate::browser::BrowserEventSink;
        use crate::governance::GovernanceFacade;
        use crate::presentation::{BrowserPresentation, PresentationReactor};
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

    /// The tab a later session of the same client opening example.com would reuse, if any.
    fn reusable_example_tab(workspaces: &crate::workspace::WorkspaceStore) -> Option<u64> {
        let key = ConnectionEvidence::claimed(RELEASED_CLIENT, IntakeChannel::Mcp)
            .reuse_key()
            .expect("the claim names an agent");
        workspaces
            .claim_released(FAKE_BROWSER, &key, "https://example.com/")
            .map(|released| released.physical_id)
    }

    /// ACCEPTANCE B25, ADR-0196 Decisions 8 and 9, and defect 740d5e77: release cleanup clears the
    /// tabs' diagnostics in batches of the size the extension accepts before any close, and a tab
    /// becomes reusable only once the preserve-tabs interlock kept it open.
    #[test]
    fn release_cleanup_clears_diagnostics_in_batches_before_any_close_and_only_a_tab_kept_open_is_reusable(
    ) {
        use crate::browser::adapter::BrowserOutcome;
        use crate::browser::BrowserError;
        use crate::workspace::WorkspaceStore;

        // Every batch of diagnostics is cleared before the first close is attempted.
        let browser = FakeBrowser::default();
        let ids: Vec<u64> = (1..=super::DIAGNOSTIC_CLEAR_BATCH_SIZE as u64 + 1).collect();
        let released = ReleasedTabs {
            browser: Some(FAKE_BROWSER.into()),
            physical_ids: ids.clone(),
        };
        cleanup_released_tabs(&released, &browser, &WorkspaceStore::default(), None);
        let (batch, rest) = ids.split_at(super::DIAGNOSTIC_CLEAR_BATCH_SIZE);
        let closes: Vec<BrowserCommand> = ids
            .iter()
            .map(|&tab_id| BrowserCommand::CloseTab {
                tab_id,
                released: true,
            })
            .collect();
        let calls = browser.calls();
        assert_eq!(
            calls[..2],
            [
                BrowserCommand::ClearDiagnostics {
                    tab_ids: batch.to_vec()
                },
                BrowserCommand::ClearDiagnostics {
                    tab_ids: rest.to_vec()
                },
            ]
        );
        assert_eq!(calls[2..], closes[..]);

        // Only a tab the browser kept open after its close is reusable, and only once.
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
        cleanup_released_tabs(&released, &browser, &workspaces, None);

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

    /// Defect C4 (e4f353ab, findings T-02 and X-01) and ADR-0203 Decision 1: a hello that registers
    /// a browser id again replaces its connection, seen to end or not, so the bindings in that
    /// browser wait for it to confirm them instead of acting on a reused tab id.
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

    /// Defect e74e3a23: a browser's request to show Ghostlight is carried out off the browser's
    /// reader, one at a time, so a reveal that waits never holds the link the browser's next
    /// event and its receipts arrive on, and a request with nothing to show frees the next.
    #[test]
    fn a_show_request_that_waits_does_not_hold_the_browser_link() {
        use crate::browser::adapter::{BrowserEvent, RuntimeControlIntent};
        use crate::browser::BrowserEventSink;
        use crate::language::workbench::PAUSED_FROM_BROWSER;

        let workspaces = crate::workspace::WorkspaceStore::default();
        let workbench = test_workbench(&workspaces);
        let heard = Arc::new(HeardChanges::default());
        workbench.attach_events(heard.clone());
        let sink = super::ServiceBrowserEvents::new(workbench.clone(), workspaces);

        // Nothing is attached yet, and that is not an error: the request ends, so the next one is
        // carried out.
        sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
        eventually(|| !sink.showing.load(Ordering::SeqCst));

        let held = Arc::new(HeldReveal::default());
        workbench.attach_presentation(held.clone());
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
        held.release();
        eventually(|| !sink.showing.load(Ordering::SeqCst));
        assert_eq!(held.started.load(Ordering::SeqCst), 1);
    }

    /// Defect S-D3 (b0290246) and ACCEPTANCE C10: a service that becomes ready wakes every edge
    /// that registered to wait for it, so a browser left open after a Quit comes back at once.
    #[test]
    fn a_published_service_wakes_the_edges_that_wait_for_it() {
        let home = TestHome::new("wakes-waiting-edges");
        let waiting = ghostlight_relay::wake::Waiting::register(home.path()).unwrap();
        let (woken, wakes) = mpsc::channel();
        thread::spawn(move || {
            if waiting.next().is_ok() {
                let _ = woken.send(());
            }
        });
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        wakes
            .recv_timeout(PATIENCE)
            .expect("the waiting edge was woken");
    }

    /// The instance every test service reports.
    const TEST_INSTANCE: &str = "instance_test";

    /// How long a test waits for the service to answer.
    const PATIENCE: Duration = Duration::from_secs(10);

    /// A relay listener over a fake browser, bound at the test home's endpoint.
    struct Served {
        intake: Arc<super::Intake>,
        relay: Arc<crate::browser::RelayBrowserPort>,
        listener: Arc<Listener>,
        thread: Option<thread::JoinHandle<()>>,
        quit_requested: mpsc::Receiver<()>,
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
        serve(home, false, SourceReader::system(), None)
    }

    /// Serve the relay listener over a fake browser, reading each session's source with
    /// `sources`.
    fn serve_reading(home: &TestHome, sources: SourceReader) -> Served {
        serve(home, false, sources, None)
    }

    /// Serve the relay listener over a fake browser, reading each session's source with
    /// `sources` and governing with the user policy at `policy`.
    fn serve_governed(home: &TestHome, sources: SourceReader, policy: PathBuf) -> Served {
        serve(home, false, sources, Some(policy))
    }

    /// Write a user policy named `name` at `path` that allows the classes `allowed` on every host.
    fn write_policy(path: &Path, name: &str, allowed: Value) {
        std::fs::write(
            path,
            serde_json::to_vec(&json!({
                "schema": 4,
                "name": name,
                "version": "1",
                "grants": [{"id": "all", "hosts": {"allow": ["*"]}, "allowed": allowed}]
            }))
            .unwrap(),
        )
        .unwrap();
    }

    /// Serve the relay listener with the relay port as the session's browser port, so sessions
    /// meet browsers only as protocol-4 extensions attach, for their calls as well as their
    /// resumed tabs.
    fn serve_relayed(home: &TestHome) -> Served {
        serve(home, true, SourceReader::system(), None)
    }

    /// Serve the relay listener in `home`; `relayed` puts the relay port behind the sessions,
    /// `sources` reads each session's source, and `policy` is the user policy, if any.
    fn serve(
        home: &TestHome,
        relayed: bool,
        sources: SourceReader,
        policy: Option<PathBuf>,
    ) -> Served {
        use crate::audit::{AuditRecorder, JsonlAuditSink};
        use crate::governance::GovernanceFacade;
        use crate::presentation::{BrowserPresentation, PresentationReactor};
        use crate::work::ApplicationExecutor;
        use crate::workbench::{WorkbenchFacade, WorkbenchProjection};
        use crate::workspace::WorkspaceStore;

        let relay = Arc::new(crate::browser::RelayBrowserPort::new(
            "service_test".into(),
            test_assets(),
        ));
        let port: Arc<dyn crate::browser::BrowserPort> = if relayed {
            relay.clone()
        } else {
            Arc::new(FakeBrowser::default())
        };
        let workspaces = WorkspaceStore::default();
        let governance = GovernanceFacade::new(policy);
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
        let (signal, quit_requested) = mpsc::channel();
        *super::lock(&quit.signal) = Some(Arc::new(move || {
            let _ = signal.send(());
        }));
        let toolsets = super::Toolsets::new(Arc::clone(&quit.drain));
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
            layout: Layout::new(home.path()),
            sources,
            toolsets,
        });
        let listener = Arc::new(Listener::bind_first(home.path()).unwrap());
        let thread = super::spawn_listener(Arc::clone(&listener), intake.clone());
        Served {
            intake,
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
        initialize_as(socket, CLIENT_NAME)
    }

    /// Initialize a session whose client claims to be `claim`.
    fn initialize_as(socket: &mut Connection, claim: &str) -> (Value, Vec<String>) {
        send_line(
            socket,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": REVISION,
                    "capabilities": {},
                    "clientInfo": {"name": claim, "version": "1"}
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

    /// The audit lines of the test home, oldest first.
    fn audit_lines(home: &TestHome) -> Vec<Value> {
        std::fs::read_to_string(home.path().join("audit.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect()
    }

    /// The executable a held source read names.
    const HELD_SOURCE: &str = "held.exe";

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
                Some(Observation {
                    source: Source {
                        agent: None,
                        image: HELD_SOURCE.into(),
                    },
                    process: SourceProcess { id: 1, started: 1 },
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
            revision: REVISION.into(),
            claim: Some(RETURNING_CLIENT.into()),
            owned_tabs: Vec::new(),
            lives: BTreeMap::new(),
            standby: false,
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

    /// Defect 967d2147 (ADR-0202 Decision 3 as ADR-0203 amends it): the accept, not the source
    /// read, is when a session begins, so the opening is answered before the read, which happens
    /// once, and a fresh or resumed session is dated at its accept.
    #[test]
    fn an_opening_is_answered_before_the_source_is_read_and_the_session_is_dated_at_its_accept() {
        for resumed in [false, true] {
            let home = TestHome::new("accept");
            let held = Arc::new(HeldRead::default());
            let _release = Release(Arc::clone(&held));
            let served = serve_reading(&home, held.reader());
            let before = now_ms();

            // The opening is answered while the read is still held.
            let mut socket = open_mcp(resumed.then(tabless_token));
            eventually(|| held.began() == 1);
            assert!(
                served.intake.workspaces.summaries().is_empty(),
                "the session opens only once its source is read"
            );

            // The accept came before the read began; the wall clock passes that moment before the
            // read ends, so a session dated by its read would read later than `released`.
            let read_began = now_ms();
            eventually(|| now_ms() > read_began);
            let released = now_ms();
            held.release();
            if !resumed {
                initialize(&mut socket);
            }
            let accepted = recorded_accept_time(&home, &mut socket);
            assert!(
                (before..released).contains(&accepted),
                "resumed {resumed}: accepted at {accepted}, dialed at {before}, read released at {released}"
            );
            assert_eq!(held.began(), 1, "the source is read once per connection");
        }
    }

    /// The catalog id and client name of the agent the toolset tests connect as.
    const CLAUDE_CODE: &str = "claude-code";

    /// What a source read sees: the installed app of `agent`, running as process `id`.
    fn running(agent: &str, id: u32) -> Option<Observation> {
        Some(Observation {
            source: Source {
                agent: Some(agent.into()),
                image: "claude".into(),
            },
            process: SourceProcess { id, started: 7 },
        })
    }

    /// A reader that answers each connection's read with the next of `observations`, in the order
    /// the connections were accepted.
    fn scripted(observations: Vec<Option<Observation>>) -> SourceReader {
        let script = Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from(
            observations,
        )));
        SourceReader::with(move |_| script.lock().unwrap().pop_front().flatten())
    }

    /// A connection initialized as Claude Code: its instructions and the tools its first catalog
    /// offers.
    fn connect_as_claude_code() -> (Connection, Value, Vec<Value>) {
        let mut socket = open_mcp(None);
        let (initialized, _) = initialize_as(&mut socket, CLAUDE_CODE);
        let tools = list_tools(&mut socket, 2);
        (socket, initialized["result"]["instructions"].clone(), tools)
    }

    /// List the tools on `socket` with request `id`.
    fn list_tools(socket: &mut Connection, id: u64) -> Vec<Value> {
        send_line(
            socket,
            json!({"jsonrpc": "2.0", "id": id, "method": "tools/list"}),
        );
        let (listed, _) = response(socket, id);
        listed["result"]["tools"].as_array().unwrap().clone()
    }

    /// Defect L2-04 (4f52e834) and ADR-0203's amendment of 2026-10-19, items 1 to 3: connections
    /// from one running app of the agent their clients claim share one set of tools, which passes
    /// to the next when the holder ends; connections that are not one agent's each get it.
    #[test]
    fn one_agents_own_app_gets_one_set_of_tools_and_the_next_connection_takes_it_over() {
        {
            let home = TestHome::new("toolset-one-agent");
            let _served = serve_reading(
                &home,
                scripted(vec![running(CLAUDE_CODE, 40), running(CLAUDE_CODE, 40)]),
            );
            let (entry, instructions, offered) = connect_as_claude_code();
            assert!(!offered.is_empty(), "the first connection offers the tools");
            assert_eq!(instructions, crate::language::SERVER_INSTRUCTIONS);

            let (mut plugin, instructions, none) = connect_as_claude_code();
            assert!(
                none.is_empty(),
                "the same agent is offered no second set: {none:?}"
            );
            assert_eq!(instructions, crate::language::STANDBY_INSTRUCTIONS);

            // When the holder ends, the one that stood by offers the tools and says so.
            drop(entry);
            let tokens = until_list_changed(&mut plugin);
            let last = tokens.last().expect("the promotion writes a token");
            assert!(
                !ResumeState::decode(last).unwrap().standby,
                "a restart after the promotion resumes it offering the tools"
            );
            assert_eq!(
                list_tools(&mut plugin, 3),
                offered,
                "the agent keeps one working set of tools"
            );
        }

        let editor = Some(Observation {
            source: Source {
                agent: Some("vscode".into()),
                image: "code".into(),
            },
            process: SourceProcess { id: 40, started: 7 },
        });
        for (case, first, second) in [
            (
                "two agent processes",
                running(CLAUDE_CODE, 40),
                running(CLAUDE_CODE, 41),
            ),
            (
                "an editor whose terminal runs the agent",
                editor.clone(),
                editor,
            ),
            ("a source that could not be read", None, None),
        ] {
            let home = TestHome::new("toolset-two-agents");
            let _served = serve_reading(&home, scripted(vec![first, second]));
            let (_one, _, one) = connect_as_claude_code();
            let (_other, instructions, other) = connect_as_claude_code();
            assert!(!one.is_empty() && !other.is_empty(), "{case}");
            assert_eq!(other, one, "{case}");
            assert_eq!(instructions, crate::language::SERVER_INSTRUCTIONS, "{case}");
        }
    }

    /// Read frames on `socket` until it is told its tool list changed, and return the resume
    /// tokens handed on the way.
    fn until_list_changed(socket: &mut Connection) -> Vec<String> {
        let mut tokens = Vec::new();
        loop {
            match frame::read_frame::<McpFrame>(socket).unwrap().unwrap() {
                McpFrame::Message { line } => {
                    let message: Value = serde_json::from_str(&line).unwrap();
                    if message["method"] == "notifications/tools/list_changed" {
                        return tokens;
                    }
                }
                McpFrame::Resume { token } => tokens.push(token),
                McpFrame::Goodbye { .. } | McpFrame::Unknown => {}
            }
        }
    }

    /// A resume token for a Claude Code session that owned no tabs and stood by when `standby`.
    fn claude_code_token(standby: bool) -> String {
        ResumeState {
            revision: REVISION.into(),
            claim: Some(CLAUDE_CODE.into()),
            owned_tabs: Vec::new(),
            lives: BTreeMap::new(),
            standby,
        }
        .encode()
    }

    /// Defects d28bb1dc and L2-12 (0285ce2d), ADR-0203's amendment of 2026-10-19 item 5: after a
    /// restart a resumed session keeps the toolset role its client saw, so the one that stood by
    /// is never offered a second set and the one that offered the tools is told to list again.
    #[test]
    fn a_resumed_session_keeps_the_toolset_role_its_client_saw_and_a_holder_lists_again() {
        // A resumed holder's client listed its tools with the earlier service and does not list
        // them again on its own, so the new service tells it to.
        {
            let home = TestHome::new("resumed-relist");
            let _served = serve_fake(&home);
            let mut resumed = open_mcp(Some(claude_code_token(false)));
            until_list_changed(&mut resumed);
            assert!(!list_tools(&mut resumed, 2).is_empty());
        }

        // After a replace, the edge that stood by reconnects first and still stands by.
        let home = TestHome::new("toolset-resumed");
        let policy = home.path().join("policy.json");
        write_policy(
            &policy,
            "rules",
            json!(["read", "action", "write", "execute"]),
        );
        let served = serve_governed(
            &home,
            scripted(vec![running(CLAUDE_CODE, 40), running(CLAUDE_CODE, 40)]),
            policy.clone(),
        );
        let mut plugin = open_mcp(Some(claude_code_token(true)));
        assert!(next_resume(&mut plugin).standby);
        let mut entry = open_mcp(Some(claude_code_token(false)));
        assert!(!next_resume(&mut entry).standby);
        let offered = list_tools(&mut entry, 2);
        assert!(!offered.is_empty(), "the former holder offers the tools");
        assert!(list_tools(&mut plugin, 2).is_empty());

        // A policy change reaches the holder, and a re-list still offers no second set.
        write_policy(&policy, "narrower", json!(["read"]));
        served.intake.governance.refresh_tick();
        until_list_changed(&mut entry);
        let narrowed = list_tools(&mut entry, 3);
        assert!(!narrowed.is_empty() && narrowed.len() < offered.len());
        assert!(
            list_tools(&mut plugin, 3).is_empty(),
            "the client that stood by is never offered a second set"
        );
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

    /// The resume token that restores exactly `owned`: the first of `earlier`, the tokens read on
    /// the way to a call's answer, that does, or else the next one on the stream that does. The
    /// worker publishes after the answer and the session watch can publish during the call, so the
    /// token can come on either side of the answer.
    fn token_restoring(
        socket: &mut Connection,
        earlier: &[String],
        owned: &[crate::intake::mcp::OwnedTab],
    ) -> ResumeState {
        earlier
            .iter()
            .map(|token| ResumeState::decode(token).unwrap())
            .find(|state| state.owned_tabs == owned)
            .unwrap_or_else(|| loop {
                let state = next_resume(socket);
                if state.owned_tabs == owned {
                    break state;
                }
            })
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

    /// The browser identity of an extension that returns after its session resumed.
    const RETURNING_BROWSER: &str = "browser_returning";

    /// The tab the returning browser still holds.
    const RETURNING_TAB: u64 = 12;

    /// The address of the returning browser's tab.
    const RETURNING_URL: &str = "https://example.com/";

    /// What the returning browser's page answers to one world message.
    fn returning_world_value(message: &Value) -> Value {
        world_value_at(message, RETURNING_URL)
    }

    /// What a page at `url` answers to one world message.
    fn world_value_at(message: &Value, url: &str) -> Value {
        if message["kind"] == "read_text" {
            json!({"text": "Example Domain", "truncated": false, "title": "Example",
                "url": url})
        } else {
            Value::Null
        }
    }

    /// A resume token for a session that owned `tab`.
    fn token_with(tab: crate::intake::mcp::OwnedTab) -> String {
        token_in(tab, None)
    }

    /// A resume token for a session that owned `tab`, whose browser was in `life` when the token
    /// was written, or named no life for `None`.
    fn token_in(tab: crate::intake::mcp::OwnedTab, life: Option<&str>) -> String {
        ResumeState {
            revision: REVISION.into(),
            claim: Some(RETURNING_CLIENT.into()),
            lives: life
                .map(|life| BTreeMap::from([(tab.browser_id.clone(), life.to_owned())]))
                .unwrap_or_default(),
            owned_tabs: vec![tab],
            standby: false,
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
        attach_returning_browser_in(served, None, RETURNING_URL)
    }

    /// Attach a protocol-4 extension for the returning browser whose hello names `life`, or no
    /// life for `None`, and that lists its tab at `url`.
    fn attach_returning_browser_in(
        served: &Served,
        life: Option<&str>,
        url: &'static str,
    ) -> crate::browser::adapter::chromium::protocol4::fake::FakeExtension {
        use crate::browser::adapter::chromium::protocol4::fake::{
            FakeExtension, Reply as FakeReply,
        };
        use crate::browser::adapter::chromium::protocol4::wire::Primitive;

        let mut hello = FakeExtension::hello(RETURNING_BROWSER);
        if let Some(life) = life {
            hello["life"] = json!(life);
        }
        let (service, extension) = pair().unwrap();
        let fake = FakeExtension::connect(
            extension,
            &hello,
            Box::new(move |correlation, primitive| {
                let outcome = match primitive {
                    Primitive::ListTabs(_) => json!({"tabs": [{"tab_id": RETURNING_TAB,
                        "title": "Example", "url": url, "active": true,
                        "readiness": "complete"}]}),
                    Primitive::Frames(_) => json!({"frames": [{"frame_id": "T",
                        "loader_id": "t1", "parent_frame_id": null, "url": url}],
                        "complete": true, "unseen": []}),
                    Primitive::Call(call) => json!({"answers": call.calls.iter().map(|world| {
                        json!({"status": "returned", "envelope": {"ok": true,
                            "value": world_value_at(&world.message, url)}})
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

    /// The extension life the returning browser was in before the service was replaced.
    const RETURNING_LIFE: &str = "life_returning";

    /// The address the returning browser's single-page app routed its tab to meanwhile.
    const ROUTED_URL: &str = "https://example.com/inbox";

    /// ACCEPTANCE B26 and C10, ADR-0203 Decision 1 (slice C4, 1d10bf74; and a10882c5): a resumed
    /// tab waits, kept in the token, until its own browser confirms it by id and its life or its
    /// page; one its browser cannot confirm reads as no longer open, and the token follows.
    #[test]
    fn a_resumed_tab_is_confirmed_by_its_own_browser_before_its_first_use() {
        use crate::intake::mcp::OwnedTab;

        let succeeded = json!(Status::Succeeded);

        // The session resumes at once with its tab pending, asking no browser; a call that needs
        // no tab completes while the browser is away and forgets nothing; and the first call
        // after the browser returns confirms the tab and reads it, with no wait for the session to
        // notice the browser.
        {
            let home = TestHome::new("resume-late");
            let served = serve_relayed(&home);
            let mut socket = open_mcp(Some(returning_token()));
            let resumed = next_resume(&mut socket);
            assert_eq!(resumed.revision, REVISION);
            assert_eq!(resumed.claim.as_deref(), Some(RETURNING_CLIENT));
            assert_eq!(resumed.owned_tabs, vec![returning_tab()]);
            assert!(bound_tabs(&served).is_empty());
            assert_eq!(
                served.intake.workspaces.summaries()[0]
                    .opener
                    .reported_application
                    .as_deref(),
                Some(RETURNING_CLIENT)
            );

            let listed = call_tool(&mut socket, 2, "browser_tabs", json!({"action": "list"}));
            assert_eq!(listed["status"], succeeded, "{listed}");
            assert_eq!(
                pending_tabs(&served),
                vec![("tab_kept".to_owned(), RETURNING_BROWSER.to_owned())]
            );

            let _browser = attach_returning_browser(&served);
            let read = call_tool(&mut socket, 3, "browser_read", json!({"tab": "tab_kept"}));
            assert_eq!(read["status"], succeeded, "{read}");
            assert_eq!(
                bound_tabs(&served),
                vec![("tab_kept".to_owned(), RETURNING_TAB)]
            );
        }

        // A tab the browser no longer lists is forgotten and leaves the token, and confirming
        // reads only the browser's tab list: nothing reaches a tab.
        {
            let home = TestHome::new("resume-missing");
            let served = serve_relayed(&home);
            let gone = OwnedTab {
                browser_id: RETURNING_BROWSER.into(),
                tab_id: RETURNING_TAB + 1,
                handle: "tab_gone".into(),
                url_digest: digest_of("https://example.com/gone"),
            };
            let token = ResumeState {
                revision: REVISION.into(),
                claim: Some(RETURNING_CLIENT.into()),
                owned_tabs: vec![returning_tab(), gone],
                lives: BTreeMap::new(),
                standby: false,
            }
            .encode();
            let mut socket = open_mcp(Some(token));
            assert_eq!(next_resume(&mut socket).owned_tabs.len(), 2);
            let browser = attach_returning_browser(&served);
            send_line(
                &mut socket,
                json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                    "params": {"name": "browser_tabs", "arguments": {"action": "list"}}}),
            );
            let (called, tokens) = response(&mut socket, 2);
            let listed = &called["result"]["structuredContent"];
            assert_eq!(listed["status"], succeeded, "{listed}");
            let tabs = listed["facts"]["tabs"].as_array().unwrap();
            assert_eq!(tabs.len(), 1, "{listed}");
            assert_eq!(tabs[0]["tab"], "tab_kept");
            token_restoring(&mut socket, &tokens, &[returning_tab()]);
            let seen = browser.wait(|seen| !requests_at(seen, "list_tabs").is_empty());
            let named_a_tab: Vec<&Value> = seen
                .frames
                .iter()
                .filter(|frame| {
                    frame["kind"] == "request" && frame["primitive"]["args"].get("tab_id").is_some()
                })
                .collect();
            assert!(
                named_a_tab.is_empty(),
                "nothing reaches a tab: {named_a_tab:?}"
            );
        }

        // In the same browser life the tab is kept though its page routed itself, and the token
        // takes its live address and keeps the life.
        {
            let home = TestHome::new("resume-same-life");
            let served = serve_relayed(&home);
            let mut socket = open_mcp(Some(token_in(returning_tab(), Some(RETURNING_LIFE))));
            let resumed = next_resume(&mut socket);
            assert_eq!(
                resumed.lives.get(RETURNING_BROWSER).map(String::as_str),
                Some(RETURNING_LIFE),
                "a pending tab keeps the life its token named"
            );

            let _browser = attach_returning_browser_in(&served, Some(RETURNING_LIFE), ROUTED_URL);
            send_line(
                &mut socket,
                json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                    "params": {"name": "browser_read", "arguments": {"tab": "tab_kept"}}}),
            );
            let (called, tokens) = response(&mut socket, 2);
            let read = &called["result"]["structuredContent"];
            assert_eq!(read["status"], succeeded, "{read}");
            assert_eq!(
                bound_tabs(&served),
                vec![("tab_kept".to_owned(), RETURNING_TAB)]
            );
            // The token the confirmation changed, published during the call or right after it.
            let token = tokens.last().map_or_else(
                || next_resume(&mut socket),
                |token| ResumeState::decode(token).unwrap(),
            );
            assert_eq!(token.owned_tabs.len(), 1);
            assert_eq!(
                token.owned_tabs[0].url_digest,
                digest_of(ROUTED_URL),
                "the confirmed tab is remembered at its live address"
            );
            assert_eq!(
                token.lives.get(RETURNING_BROWSER).map(String::as_str),
                Some(RETURNING_LIFE),
                "a confirmed tab is in the life its browser reports"
            );
        }

        // In a new browser life a tab at another address is forgotten.
        {
            let home = TestHome::new("resume-new-life");
            let served = serve_relayed(&home);
            let mut socket = open_mcp(Some(token_in(returning_tab(), Some(RETURNING_LIFE))));
            next_resume(&mut socket);
            let _browser = attach_returning_browser_in(&served, Some("life_restarted"), ROUTED_URL);
            let read = call_tool(&mut socket, 2, "browser_read", json!({"tab": "tab_kept"}));
            assert_ne!(read["status"], succeeded, "{read}");
            assert!(bound_tabs(&served).is_empty());
        }

        // A token written before digests resumes with its tabs stale, even where the browser
        // still shows a tab with that id at that address.
        {
            let home = TestHome::new("resume-no-digest");
            let served = serve_relayed(&home);
            let earlier = OwnedTab {
                url_digest: None,
                ..returning_tab()
            };
            let mut socket = open_mcp(Some(token_with(earlier)));
            assert!(next_resume(&mut socket).owned_tabs.is_empty());
            let _browser = attach_returning_browser(&served);
            let read = call_tool(&mut socket, 2, "browser_read", json!({"tab": "tab_kept"}));
            assert_ne!(read["status"], succeeded, "{read}");
            assert!(bound_tabs(&served).is_empty());
        }
    }

    /// Defect C11 (8c216920, finding E-06) and ADR-0203 Decision 1: the edge always holds a token
    /// its session resumes by, since a token that cannot be read still resumes an initialized
    /// session and every answered initialize comes with a token, even after a service end.
    #[test]
    fn every_session_can_be_resumed_by_the_token_its_edge_holds() {
        // A token that cannot be read resumes at the oldest revision with no tab, and the client,
        // which initialized with an earlier service, is served its next call.
        {
            let home = TestHome::new("unreadable-token");
            let served = serve_fake(&home);
            let mut socket = open_mcp(Some("rs1.***".into()));
            let resumed = next_resume(&mut socket);
            assert_eq!(resumed.revision, crate::intake::mcp::OLDEST_REVISION);
            assert_eq!(resumed.claim, None);
            assert!(resumed.owned_tabs.is_empty());
            let listed = call_tool(&mut socket, 2, "browser_tabs", json!({"action": "list"}));
            assert_eq!(listed["status"], json!(Status::Succeeded), "{listed}");
            assert_eq!(served.intake.workspaces.summaries().len(), 1);
        }

        // A fresh session's token comes before its initialize answer, ahead of its workspace.
        {
            let home = TestHome::new("initialize-token-fresh");
            let _served = serve_fake(&home);
            let mut socket = open_mcp(None);
            let (initialized, tokens) = initialize(&mut socket);
            assert!(initialized["result"].is_object(), "{initialized}");
            let state =
                ResumeState::decode(tokens.first().expect("a token before the answer")).unwrap();
            assert_eq!(state.revision, REVISION);
            assert_eq!(state.claim.as_deref(), Some(CLIENT_NAME));
            assert!(state.owned_tabs.is_empty());
        }

        // So does one that initializes after a quit handed every stream its last token.
        {
            let home = TestHome::new("initialize-token-quit");
            let served = serve_fake(&home);
            let mut socket = open_mcp(None);
            // A ping is answered once the stream is enrolled for the quit's last tokens.
            send_line(
                &mut socket,
                json!({"jsonrpc": "2.0", "id": 9, "method": "ping"}),
            );
            response(&mut socket, 9);
            served
                .intake
                .quit
                .begin(QuitReason::Replace, Duration::ZERO);
            served
                .quit_requested
                .recv_timeout(PATIENCE)
                .expect("an idle service ends at once");
            let (initialized, tokens) = initialize(&mut socket);
            assert!(initialized["result"].is_object(), "{initialized}");
            let state =
                ResumeState::decode(tokens.first().expect("a token before the answer")).unwrap();
            assert!(state.owned_tabs.is_empty());
        }
    }

    /// The workbench origin is Ghostlight's own ceiling (ADR-0199's web apps amendment and
    /// ADR-0203's amendment of 2026-10-19): publishing starts the workbench and bars agents from
    /// its address before the page answers, and from nothing else.
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

    /// Open one MCP session, end its connection the way `goodbye` does, and wait until the
    /// session is gone.
    fn session_is_released_after(name: &str, goodbye: impl FnOnce(&mut Connection)) {
        let home = TestHome::new(name);
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut socket = open_mcp(None);
        initialize(&mut socket);
        // The session opens once its source is read, which follows the `initialize` answer.
        eventually(|| host.workbench.snapshot().sessions.len() == 1);

        goodbye(&mut socket);
        drop(socket);
        // The teardown runs on the connection's own thread.
        eventually(|| host.workbench.snapshot().sessions.is_empty());
    }

    /// Defect 79cf4cc6: a client that goes away through the error path, with one frame that is not
    /// JSON, still releases its session and every tab it held, as a tidy goodbye always did.
    #[test]
    fn a_client_that_goes_away_badly_still_releases_its_session() {
        // The negative control: a tidy goodbye has always worked, so if this were the only case
        // the wait below would prove nothing about the error path.
        session_is_released_after("goodbye-clean", |_| {});
        // An unowned workspace has no owning process for the reaper, so only the connection's own
        // teardown can release it.
        session_is_released_after("goodbye-malformed", |socket| {
            frame::write_frame_raw(socket, b"{ not json at all").unwrap();
        });
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

    /// ADR-0199 Decisions 9 and 10 and ACCEPTANCE C13: a start applies the quit latch, then takes
    /// the lock, and touches nothing before it holds it, so Quit holds a browser or recover start
    /// back, also one that waited for the lock, and any other start clears the latch.
    #[test]
    fn a_start_applies_the_quit_latch_and_takes_the_lock_before_it_touches_any_state() {
        const HELD_BACK: [StartReason; 2] = [StartReason::Browser, StartReason::Recover];
        const CLEARING: [StartReason; 4] = [
            StartReason::Agent,
            StartReason::Command,
            StartReason::Person,
            StartReason::Update,
        ];

        // With no latch, a browser start serves.
        let directory = temp_home("latch-clear");
        assert!(start_for(&directory, StartTerms::for_reason(StartReason::Browser)).is_ok());

        // A set latch holds a browser or recover start back before it takes the lock.
        let directory = temp_home("latch-refuses");
        let layout = Layout::new(directory.path());
        super::latch::set(&layout.quit_latch()).unwrap();
        for reason in HELD_BACK {
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

        // Any other start clears it once it holds the lock, and serves.
        for reason in CLEARING {
            let directory = temp_home("latch-cleared");
            let layout = Layout::new(directory.path());
            super::latch::set(&layout.quit_latch()).unwrap();
            let started = start_for(&directory, StartTerms::for_reason(reason)).unwrap();
            assert!(!super::latch::is_set(&layout.quit_latch()), "{reason:?}");
            drop(started);
        }

        // A start that finds the lock held gives way and touches nothing, the latch included.
        let directory = temp_home("lock-held-gives-way");
        let layout = Layout::new(directory.path());
        let holder = start_for(&directory, agent()).unwrap();
        super::latch::set(&layout.quit_latch()).unwrap();
        let before = files_under(&directory);
        for reason in CLEARING {
            let error = start_for(&directory, StartTerms::asking_once(reason))
                .err()
                .expect("the lock is held");
            assert!(error.is::<AuthorityAlreadyRunning>(), "{reason:?}");
            assert!(
                super::latch::is_set(&layout.quit_latch()),
                "{reason:?}: a start that gives way leaves the latch alone"
            );
        }
        assert_eq!(files_under(&directory), before, "no state was opened");
        drop(holder);

        // A browser start that began before a Quit, and waited for the lock the quitting service
        // held, is refused once it holds the lock.
        let directory = temp_home("latch-set-while-waiting");
        let layout = Layout::new(directory.path());
        let holder = start_for(&directory, agent()).unwrap();
        let (began, beginning) = mpsc::channel();
        let starter = {
            let directory = directory.path().to_path_buf();
            thread::spawn(move || {
                // The token check comes before the start's first look at the latch.
                ServiceHost::start_with_token(
                    &directory,
                    waiting(StartReason::Browser, PATIENCE),
                    move || {
                        let _ = began.send(());
                        Ok(false)
                    },
                )
                .map(drop)
            })
        };
        beginning.recv_timeout(PATIENCE).expect("the start began");
        super::latch::set(&layout.quit_latch()).unwrap();
        drop(holder);
        let error = starter
            .join()
            .unwrap()
            .expect_err("Quit holds this start back");
        assert!(error.is::<QuitLatched>());
        assert!(
            super::latch::is_set(&layout.quit_latch()),
            "the latch stays"
        );
        assert!(
            super::ServiceLock::is_free(&layout.service_lock()),
            "the refused start released the lock"
        );
    }

    /// How many of a browser relay's looks for a returning service a replacement may take to
    /// publish and still have that relay waited for. The fourth look comes 7.5 s after the old
    /// service ended; the measured updates published within about a second of that end.
    const REPLACEMENT_LOOKS: usize = 4;

    /// How long a relay's look that finds the service takes to put its browser back: the relay
    /// exits, the extension opens a new native port at once and the new relay's hello is
    /// answered. Well under a second where it was measured
    /// (`docs/testing/alpine-2.0.1-bundle-2026-10-20.md`); two seconds leaves room for a slow
    /// native-host start.
    const RELAY_HANDOVER: Duration = Duration::from_secs(2);

    /// A home that remembers a browser, as every home a browser has connected to does.
    fn home_that_remembers_a_browser(name: &str) -> Scratch {
        let directory = temp_home(name);
        let layout = Layout::new(directory.path());
        crate::install::last_browser::record(&layout.last_browser_record(), "chrome").unwrap();
        directory
    }

    /// Defect S-D2 (9c257daa) and ACCEPTANCE C19: after an update or a recovery a call with no
    /// browser waits for the replaced service's relays, on their own backoff, up to the return
    /// grace, while any other start, or one in a home no browser has used, waits only the short window.
    #[test]
    fn a_replacement_waits_for_the_relays_of_the_service_it_replaced() {
        use crate::browser::{BrowserPort, RELAY_RETURN_WINDOW, RETURN_GRACE};
        use ghostlight_relay::spawn::Backoff;

        // Each relay of the ended service looks again on its own doubling backoff, counted from
        // the moment it lost the old service, so a replacement that publishes just after one look
        // waits nearly the whole next wait for the look that finds it.
        let mut backoff = Backoff::new();
        let longest_wait = (0..REPLACEMENT_LOOKS)
            .map(|_| backoff.next_wait())
            .max()
            .expect("the relay looks at least once");
        let slowest = longest_wait + RELAY_HANDOVER;
        assert!(
            slowest < RETURN_GRACE,
            "a relay can be back {slowest:?} after the publication, past the return grace"
        );

        for reason in [StartReason::Update, StartReason::Recover] {
            let directory = home_that_remembers_a_browser("replacement-relays");
            let host = start_for(&directory, StartTerms::for_reason(reason)).unwrap();
            host.publish_ready().unwrap();

            host.browser.backdate(slowest);
            assert!(
                host.browser.browser_returning(),
                "{reason:?}: a relay back {slowest:?} after the publication is still waited for"
            );
            host.browser.backdate(RETURN_GRACE - slowest);
            assert!(
                !host.browser.browser_returning(),
                "{reason:?}: the wait for the replaced service's relays ends with the return grace"
            );
            drop(host);
        }

        // A start that replaced no service, and an update in a home no browser has connected to,
        // as on a first install, keep nearly the whole budget of a call for starting a browser.
        for (reason, directory) in [
            (StartReason::Update, temp_home("first-install")),
            (
                StartReason::Agent,
                home_that_remembers_a_browser("agent-start"),
            ),
            (
                StartReason::Browser,
                home_that_remembers_a_browser("browser-start"),
            ),
        ] {
            let host = start_for(&directory, StartTerms::for_reason(reason)).unwrap();
            host.publish_ready().unwrap();
            assert!(
                host.browser.browser_returning(),
                "{reason:?}: relays may be reconnecting"
            );
            host.browser.backdate(RELAY_RETURN_WINDOW);
            assert!(
                !host.browser.browser_returning(),
                "{reason:?}: nothing replaced holds the wait past the relay window"
            );
            drop(host);
        }
    }

    /// ADR-0199 Decision 9 step 2 and its amendment of 2026-10-16: an elevated token, or one that
    /// cannot be read, is refused before the start takes any state, and the debug-build waiver
    /// lets only an elevated answer through.
    #[test]
    fn an_elevated_or_unreadable_token_is_refused_before_any_state() {
        use super::decide_elevation;

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
        assert!(!directory.join("run").exists());
        assert!(ServiceHost::start_with_token(&directory, agent(), || Ok(false)).is_ok());

        // The waiver: only an elevated answer can be waived.
        let refused = decide_elevation(Ok(true), false).unwrap_err();
        assert_eq!(format!("{refused:#}"), super::ELEVATED_SERVICE);
        assert!(decide_elevation(Ok(true), true).is_ok());
        assert!(decide_elevation(Ok(false), false).is_ok());
        assert!(decide_elevation(Ok(false), true).is_ok());
        for waived in [false, true] {
            let unreadable =
                decide_elevation(Err(std::io::Error::other("no token")), waived).unwrap_err();
            assert!(format!("{unreadable:#}").contains("no token"), "{waived}");
        }
    }

    /// A presentation port whose reveal waits until it is released, as one does while a shell
    /// starts.
    #[derive(Default)]
    struct HeldReveal {
        started: AtomicUsize,
        released: std::sync::Mutex<bool>,
        changed: std::sync::Condvar,
    }

    impl HeldReveal {
        /// Let every reveal finish.
        fn release(&self) {
            *self.released.lock().unwrap() = true;
            self.changed.notify_all();
        }
    }

    impl WorkbenchPresentationPort for HeldReveal {
        fn reveal(&self, _address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
            self.started.fetch_add(1, Ordering::SeqCst);
            let released = self.released.lock().unwrap();
            let _waited = self
                .changed
                .wait_timeout_while(released, PATIENCE, |released| !*released)
                .unwrap();
            Ok(())
        }

        fn notify(
            &self,
            _notification: WorkbenchNotification,
        ) -> Result<(), WorkbenchPresentationError> {
            Ok(())
        }
    }

    /// A host that signals through a channel instead of a desktop.
    fn quittable_host(name: &str) -> (TestHome, ServiceHost, mpsc::Receiver<()>) {
        let home = TestHome::new(name);
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let (quit, quit_requested) = mpsc::channel();
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

    /// Read frames from a desktop `stream` until a goodbye arrives and return its reason.
    fn desktop_goodbye(stream: &mut Connection) -> GoodbyeReason {
        loop {
            match frame::read_frame::<DesktopFrame>(stream).unwrap() {
                Some(DesktopFrame::Goodbye { reason }) => return reason,
                Some(_) => {}
                None => panic!("the desktop stream closed with no goodbye"),
            }
        }
    }

    /// ACCEPTANCE C13 and ADR-0199 Decision 10: a person's Quit, from the tray, the page, control
    /// or the service itself, writes the quit latch as it begins, and a replace, an uninstall or
    /// the end of the session leaves it unset.
    #[test]
    fn only_a_persons_quit_sets_the_quit_latch() {
        /// One way the service is asked to end.
        #[derive(Clone, Copy, Debug)]
        enum End {
            /// The tray's Quit, over the desktop stream.
            Tray,
            /// The page's Quit, through the workbench.
            Page,
            /// A control quit with reason quit.
            Control,
            /// The service's own quit for a reason.
            Service(QuitReason),
            /// The end of the person's sign-in session.
            SignOut,
        }

        for (end, latched) in [
            (End::Tray, true),
            (End::Page, true),
            (End::Control, true),
            (End::Service(QuitReason::Quit), true),
            (End::Service(QuitReason::Replace), false),
            (End::Service(QuitReason::Uninstall), false),
            (End::SignOut, false),
        ] {
            let (home, host, quit_requested) = quittable_host("quit-latch");
            let latch = Layout::new(home.path()).quit_latch();
            assert!(!super::latch::is_set(&latch), "{end:?}");
            let _shell = match end {
                End::Tray => {
                    let mut shell = attached_shell(&host);
                    frame::write_frame(
                        &mut shell,
                        &DesktopFrame::Tray {
                            item: TrayItemId::Quit,
                        },
                    )
                    .unwrap();
                    Some(shell)
                }
                End::Page => {
                    host.workbench.quit().unwrap();
                    None
                }
                End::Control => {
                    cli_client::control(Control::Quit {
                        reason: QuitReason::Quit,
                    })
                    .unwrap();
                    None
                }
                End::Service(reason) => {
                    host.begin_quit(reason, PATIENCE);
                    None
                }
                End::SignOut => {
                    host.quit_handle().session_end();
                    None
                }
            };
            // The latch is written as the quit begins, before it answers and before the service
            // has left, so a browser that restarts meanwhile is held back too.
            if !matches!(end, End::Tray) {
                assert_eq!(super::latch::is_set(&latch), latched, "{end:?} at once");
            }
            quit_requested
                .recv_timeout(PATIENCE)
                .expect("the end asks the process to exit");
            assert_eq!(super::latch::is_set(&latch), latched, "{end:?}");
            drop(host);
        }
    }

    /// Relay protocol 4 contract (ADR-0199 Decisions 5 and 13, ACCEPTANCE C13): before it closes
    /// them, the service says goodbye with its end's reason on each MCP stream and the desktop
    /// stream, and only then asks the process to exit.
    #[test]
    fn every_service_end_says_goodbye_with_its_reason_on_the_mcp_and_desktop_streams() {
        for (quit, goodbye) in [
            (Some(QuitReason::Quit), GoodbyeReason::Quit),
            (Some(QuitReason::Replace), GoodbyeReason::Replace),
            (Some(QuitReason::Uninstall), GoodbyeReason::Uninstall),
            (None, GoodbyeReason::SessionEnd),
        ] {
            let (_home, host, quit_requested) = quittable_host("goodbye");
            let mut mcp = open_mcp(None);
            initialize(&mut mcp);
            let mut shell = attached_shell(&host);

            match quit {
                Some(reason) => {
                    let answer = cli_client::control(Control::Quit { reason }).unwrap();
                    assert_eq!(
                        answer,
                        serde_json::to_value(control::Quitting { quitting: true }).unwrap(),
                        "{reason:?}"
                    );
                }
                None => host.quit_handle().session_end(),
            }

            assert_eq!(goodbye_on(&mut mcp), goodbye, "on the MCP stream");
            assert_eq!(
                desktop_goodbye(&mut shell),
                goodbye,
                "on the desktop stream"
            );
            quit_requested
                .recv_timeout(PATIENCE)
                .expect("the process is asked to exit after the goodbyes");
            drop(host);
        }
    }

    /// ARCHITECTURE's quit and ADR-0199 Decision 13: an end admits no new work, and a quit lets
    /// work in flight finish, for at most its grace, before its goodbye and exit, keeping the
    /// reason it began with, while the end of the session does not wait for work at all.
    #[test]
    fn a_quit_lets_work_in_flight_finish_within_its_grace_and_the_first_quit_wins() {
        // Work that finishes: a quit over control answers at once, and the quit ends with the
        // work, well before its grace would have ended it.
        {
            let (_home, host, quit_requested) = quittable_host("quit-drains");
            let mut mcp = open_mcp(None);
            initialize(&mut mcp);
            let running = host.quit.drain.admit().unwrap();
            let began = Instant::now();
            let answer = cli_client::control(Control::Quit {
                reason: QuitReason::Quit,
            })
            .unwrap();
            assert_eq!(
                answer,
                serde_json::to_value(control::Quitting { quitting: true }).unwrap()
            );
            assert!(
                host.quit.drain.admit().is_err(),
                "no new work is admitted once the quit began"
            );
            drop(running);
            assert_eq!(goodbye_on(&mut mcp), GoodbyeReason::Quit);
            quit_requested
                .recv_timeout(PATIENCE)
                .expect("the quit ends once its work has finished");
            assert!(
                began.elapsed() < QUIT_GRACE,
                "the quit ended with its work, not with its grace"
            );
            drop(host);
        }

        // Work that never finishes: the goodbye and the exit wait out the grace, and a second
        // quit while one is under way changes neither the reason nor the grace.
        {
            const GRACE: Duration = Duration::from_millis(300);

            let (_home, host, quit_requested) = quittable_host("quit-bound");
            let mut mcp = open_mcp(None);
            initialize(&mut mcp);
            let _stuck = host.quit.drain.admit().unwrap();
            let began = Instant::now();
            host.begin_quit(QuitReason::Quit, GRACE);
            host.begin_quit(QuitReason::Replace, Duration::ZERO);
            assert_eq!(goodbye_on(&mut mcp), GoodbyeReason::Quit);
            assert!(
                began.elapsed() >= GRACE,
                "no goodbye while work was in flight inside the grace"
            );
            quit_requested
                .recv_timeout(PATIENCE)
                .expect("the quit ends when its grace does");
            drop(host);
        }

        // The end of the session says goodbye and leaves while work is still in flight.
        {
            let (_home, host, quit_requested) = quittable_host("session-end-drain");
            let mut mcp = open_mcp(None);
            initialize(&mut mcp);
            let running = host.quit.drain.admit().unwrap();
            host.quit_handle().session_end();
            assert_eq!(goodbye_on(&mut mcp), GoodbyeReason::SessionEnd);
            quit_requested
                .recv_timeout(PATIENCE)
                .expect("the end of the session does not wait for work in flight");
            assert!(
                host.quit.drain.admit().is_err(),
                "no new work is admitted once the session is ending"
            );
            drop(running);
            drop(host);
        }
    }

    /// Call `browser_tabs` list under `id` and return its tool result and the audit lines of its
    /// invocation. The audit line is written before the result is delivered.
    fn listed_with_audit(home: &TestHome, socket: &mut Connection, id: u64) -> (Value, Vec<Value>) {
        send_line(
            socket,
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {"name": "browser_tabs", "arguments": {"action": "list"}}
            }),
        );
        let (called, _) = response(socket, id);
        let result = called["result"]["structuredContent"].clone();
        assert!(
            result.is_object(),
            "a tool result, not a protocol error: {called}"
        );
        let records = audit_lines(home)
            .into_iter()
            .filter(|record| record["invocation"] == result["invocation"])
            .collect();
        (result, records)
    }

    /// One call the service ended before it started: the tool result its client received, when a
    /// client was there to receive it, and the audit lines of its invocation.
    type EndedCall = (Option<Value>, Vec<Value>);

    /// The tool results and audit lines of two calls made during a quit for `reason`: one while
    /// the quit drains work in flight, and one after its end stopped the session's queue.
    ///
    /// Each step waits on what it needs: the session is open once its first call is answered,
    /// the drain refuses work once `begin` returns, and the end has run once the quit signal is
    /// made. The answers are written by the thread that reads the stream, so nothing races them.
    fn calls_during_a_quit(reason: QuitReason) -> Vec<EndedCall> {
        let home = TestHome::new("quit-calls");
        let served = serve_fake(&home);
        let mut socket = open_mcp(None);
        initialize(&mut socket);
        let (opened, _) = listed_with_audit(&home, &mut socket, 2);
        assert_eq!(opened["status"], json!(Status::Succeeded), "{opened}");

        let running = served.intake.quit.drain.admit().unwrap();
        served.intake.quit.begin(reason, PATIENCE);
        let draining = listed_with_audit(&home, &mut socket, 3);
        drop(running);
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the quit ends once its work in flight finishes");
        let ended = listed_with_audit(&home, &mut socket, 4);
        [draining, ended]
            .into_iter()
            .map(|(result, records)| (Some(result), records))
            .collect()
    }

    /// The audit lines of two calls that wait behind a workspace the test holds when their client
    /// goes away, so their session's end cancels them before they start, with no client left to
    /// hear it.
    ///
    /// The service reads both calls, and queues them, before the end of the stream it then reads,
    /// and the session's end has run once both calls have an audit line.
    fn calls_queued_when_their_session_ends() -> Vec<EndedCall> {
        const CALLS: usize = 2;

        let home = TestHome::new("session-end-calls");
        let served = serve_fake(&home);
        // The calls join a declared session whose workspace the test holds, so they wait for it.
        let marker = SessionMarker::Declared {
            key: "session-end-calls".into(),
        };
        let workspace = served.intake.workspaces.connect(
            Arc::new(ConnectionEvidence::claimed("holder", IntakeChannel::Cli)),
            Some(marker.clone()),
        );
        let _held = served.intake.workspaces.acquire(&workspace).unwrap();
        let mut opening = mcp_opening(None);
        opening.session = Some(serde_json::to_value(marker).unwrap());
        let (mut socket, reply) = dial(&opening);
        assert!(reply.accepted, "{reply:?}");
        initialize(&mut socket);
        for id in 2..2 + CALLS {
            send_line(
                &mut socket,
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "tools/call",
                    "params": {
                        "name": "browser_tabs",
                        "arguments": {
                            "action": "list",
                            "timeout_ms": crate::language::MAX_TIMEOUT_MS
                        }
                    }
                }),
            );
        }
        drop(socket);
        eventually(|| audit_lines(&home).len() >= CALLS);
        let mut calls: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for record in audit_lines(&home) {
            let invocation = record["invocation"].as_str().unwrap_or_default().to_owned();
            calls.entry(invocation).or_default().push(record);
        }
        assert_eq!(calls.len(), CALLS, "{calls:?}");
        calls.into_values().map(|records| (None, records)).collect()
    }

    /// Defect C11 (8c216920, finding E-03), ACCEPTANCE B27 and ADR-0203 Decision 4.2: a call the
    /// service ends before it starts, because Ghostlight quits or because the call's session ends,
    /// is cancelled with one audit line worded for that cause, and a client still there to hear it
    /// gets a cancelled tool result that says to try again only when a call repeated in a moment
    /// is served.
    #[test]
    fn a_call_ended_before_it_starts_is_cancelled_with_one_audit_line_worded_for_its_cause() {
        for (cause, calls) in [
            (
                CancelCause::Quit(QuitCause::Quit),
                calls_during_a_quit(QuitReason::Quit),
            ),
            (
                CancelCause::Quit(QuitCause::Replace),
                calls_during_a_quit(QuitReason::Replace),
            ),
            (
                CancelCause::Quit(QuitCause::Unknown),
                calls_during_a_quit(QuitReason::Unknown),
            ),
            (
                CancelCause::Quit(QuitCause::Uninstall),
                calls_during_a_quit(QuitReason::Uninstall),
            ),
            (
                CancelCause::SessionEnded,
                calls_queued_when_their_session_ends(),
            ),
        ] {
            let refusal = Refusal::CancelledBeforeStart { by: cause };
            let steps = refusal.next_steps();
            for (result, records) in calls {
                assert_eq!(records.len(), 1, "{cause:?}: {records:?}");
                assert_eq!(
                    records[0]["status"],
                    Status::Cancelled.stored_name(),
                    "{records:?}"
                );
                assert_eq!(
                    records[0]["effect"],
                    Effect::None.stored_name(),
                    "{records:?}"
                );
                assert_eq!(records[0]["summary"], refusal.summary(), "{cause:?}");
                let Some(result) = result else {
                    continue;
                };
                assert_eq!(
                    result["status"],
                    json!(Status::Cancelled),
                    "{cause:?}: {result}"
                );
                assert_eq!(result["effect"], json!(Effect::None), "{result}");
                assert_eq!(result["repeat_safe"], true, "{result}");
                assert_eq!(result["summary"], refusal.summary(), "{cause:?}");
                if steps.is_empty() {
                    assert!(result.get("next_steps").is_none(), "{cause:?}: {result}");
                } else {
                    assert_eq!(result["next_steps"], json!(steps), "{cause:?}");
                }
            }
        }
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

    /// ACCEPTANCE B27 and ADR-0203 Decision 3 (slice C5, 9beb7976): a service end decides every
    /// session's tabs once, by its reason and the session's kind, and the end of the session and
    /// the idle exit forget every tab without a browser command.
    #[test]
    fn a_service_end_decides_each_sessions_tabs_once_by_its_reason() {
        // A replace keeps a live session's tab bound, closes and ungroups nothing, and hands the
        // tab to the session's last token for its edge's resume.
        {
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

        // A quit, an uninstall or a reason this build does not know releases every session, and
        // each stream's last token carries no tab.
        for (reason, said) in [
            (QuitReason::Quit, GoodbyeReason::Quit),
            (QuitReason::Uninstall, GoodbyeReason::Uninstall),
            (QuitReason::Unknown, GoodbyeReason::Unknown),
        ] {
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
                assert_eq!(goodbye, said);
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

        // A command-line session is released at every end, because nothing can resume it.
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

        // On a replace, a session whose client left during the drain is released, and the
        // session that stays keeps its tab.
        {
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

        // A workspace a call still holds is cancelled before its tabs close.
        {
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

        // However many tabs a session holds, the end of the session says both goodbyes inside its
        // fallback exit and sends the browser nothing.
        {
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
    }

    /// ADR-0199 Decision 9 step 6: a start that does not accept openings in time stops by itself,
    /// and becoming ready settles that limit, so a ready service never stops for it.
    #[test]
    fn a_stalled_start_stops_by_itself_and_becoming_ready_cancels_the_stop() {
        // A start whose limit passes before it accepts openings stops.
        let stalled_home = temp_home("ready-stalled");
        let stalled = ServiceHost::start(&stalled_home).unwrap();
        let (stopped, stop) = mpsc::channel::<()>();
        stalled
            .expect_ready_by(Instant::now(), move || {
                let _ = stopped.send(());
            })
            .unwrap();
        stop.recv_timeout(PATIENCE)
            .expect("a start that is not ready by its limit stops");
        drop(stalled);

        let directory = temp_home("ready-published");
        let host = ServiceHost::start(&directory).unwrap();
        let (expired, fired) = mpsc::channel::<()>();
        // A limit far past what this test waits, armed before the endpoint is bound, as serve does.
        host.expect_ready_by(Instant::now() + PATIENCE * 6, move || {
            let _ = expired.send(());
        })
        .unwrap();
        host.publish_ready().unwrap();
        // A settled limit ends its watcher without a stop, which drops the stop unmade.
        assert_eq!(
            fired.recv_timeout(PATIENCE),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
        drop(host);
    }

    /// ACCEPTANCE Gov21: the session watch tells a client its tool list changed once, and only when
    /// a policy change changes the catalog it was given, including one an earlier service gave.
    #[test]
    fn the_session_watch_tells_the_client_only_when_a_policy_change_changes_the_catalog() {
        use crate::governance::GovernanceFacade;

        use super::AdvertisedCatalog;

        let directory = temp_home("catalog");
        let path = directory.join("policy.json");
        let write = |name: &str, allowed: Value| write_policy(&path, name, allowed);
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

        let resumed = AdvertisedCatalog::default();
        resumed.given_earlier();
        assert!(
            resumed.changed_by(&governance.snapshot()).is_some(),
            "a catalog an earlier service gave is told to list again"
        );
        assert!(resumed.changed_by(&governance.snapshot()).is_none());
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

    /// The desktop-role contract (ADR-0199 Decision 7 and its shell milestone 2 amendment),
    /// ACCEPTANCE D1 and D11, and defect 17ec7571: the shell that takes the one desktop place gets
    /// the workbench and its bearer, and one that finds it taken is told so, with the holder shown
    /// only for a shell that wanted a window.
    #[test]
    fn a_shell_that_attaches_gets_the_workbench_and_its_bearer_or_is_told_the_place_is_taken() {
        let home = TestHome::new("desktop-place");
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let address = host.workbench_address().unwrap();

        // The shell that takes the place is handed the workbench and the tray the service
        // describes.
        let (mut first, handed) = open_desktop();
        let (url, bearer, tray) = handed_over(handed);
        assert_eq!(url, address.url());
        assert_eq!(
            tray,
            host.workbench.tray_model(),
            "the workbench frame carries the tray the service describes"
        );
        eventually(|| host.quit.shells().attached() == 1);

        // A second shell is told the place is taken and leaves, and the first is shown.
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

        // Presence's own tray shell that finds the place taken brings no window forward: the next
        // frame the holder hears is a later show.
        let mut tray_opening = desktop_opening();
        tray_opening.channel = Some(ShellView::Tray.as_str().to_owned());
        let (mut tray_shell, reply) = dial(&tray_opening);
        assert!(reply.accepted, "{reply:?}");
        assert_eq!(
            frame::read_frame::<DesktopFrame>(&mut tray_shell).unwrap(),
            Some(DesktopFrame::AlreadyOpen)
        );
        host.workbench.reveal(Some(PageAddress::About)).unwrap();
        assert_eq!(
            next_frame(&mut first),
            DesktopFrame::Show {
                address: Some(PageAddress::About)
            },
            "the first shell still serves, and the tray shell asked it to show nothing"
        );

        // The bearer makes the shell's page the window's; without it a screen cannot act as one.
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

        // The place is free again once the holder goes.
        drop(first);
        eventually(|| host.quit.shells().attached() == 0);
        let (_third, handed) = open_desktop();
        assert!(
            matches!(handed, DesktopFrame::Workbench { .. }),
            "the place is free again: {handed:?}"
        );
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
    ) -> mpsc::Receiver<T> {
        let (answered, answers) = mpsc::channel();
        thread::spawn(move || {
            let _ = answered.send(request());
        });
        answers
    }

    /// ACCEPTANCE C16, ADR-0200 Decision 11 and defect c4c05e24 (JQ11): every way a person asks to
    /// see Ghostlight shows the window at setup until setup is complete, and with no shell
    /// attached starts one that shows the window at the address asked.
    #[test]
    fn a_persons_request_for_the_window_shows_it_at_setup_until_setup_is_complete() {
        use crate::browser::adapter::BrowserEvent;
        use crate::browser::BrowserEventSink;
        use crate::install::setup_state::{self, SetupRecord};
        use crate::workbench::setup::SetupProgress;

        {
            let home = TestHome::new("show-setup");
            let host = ServiceHost::start(home.path()).unwrap();
            host.publish_ready().unwrap();
            let mut shell = attached_shell(&host);
            let browser = super::ServiceBrowserEvents::new(
                host.workbench.clone(),
                host.intake.workspaces.clone(),
            );
            let click = |shell: &mut Connection, item| {
                frame::write_frame(shell, &DesktopFrame::Tray { item }).unwrap();
            };
            let at_setup = DesktopFrame::Show {
                address: Some(PageAddress::Setup),
            };

            // While setup is incomplete, the tray's Open, a control show (the install and
            // `ghostlight open`) and the browser's show request (the extension's Open Ghostlight)
            // each bring the window to setup.
            click(&mut shell, TrayItemId::Open);
            assert_eq!(next_frame(&mut shell), at_setup, "the tray's Open");
            assert!(cli_client::activate_workbench().unwrap());
            assert_eq!(next_frame(&mut shell), at_setup, "a control show");
            browser.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
            assert_eq!(
                next_frame(&mut shell),
                at_setup,
                "the browser's show request"
            );
            eventually(|| !browser.showing.load(Ordering::SeqCst));

            // With setup complete the window opens where it last was, and the tray's Open asks for
            // nothing more than the shell already does: a Hold click is answered after it, by a
            // tray model, and the next show is a later one.
            let record = host.intake.layout.setup_record();
            assert!(setup_state::record_if_absent(&record, &SetupRecord::now()).unwrap());
            host.workbench.attach_setup(SetupProgress::load(&record));
            click(&mut shell, TrayItemId::Open);
            click(&mut shell, TrayItemId::Hold);
            while !matches!(next_frame(&mut shell), DesktopFrame::TrayModel { .. }) {}
            host.workbench.reveal(Some(PageAddress::About)).unwrap();
            assert_eq!(
                next_frame(&mut shell),
                DesktopFrame::Show {
                    address: Some(PageAddress::About)
                },
                "no show was written for the open"
            );
            assert!(cli_client::activate_workbench().unwrap());
            assert_eq!(
                next_frame(&mut shell),
                DesktopFrame::Show { address: None },
                "a control show"
            );
            browser.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
            assert_eq!(
                next_frame(&mut shell),
                DesktopFrame::Show { address: None },
                "the browser's show request"
            );
            drop(host);
        }

        // With no shell attached, the request starts one without the tray argument and shows the
        // window at the address asked once it attaches.
        {
            let home = TestHome::new("show-launch");
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
    }

    /// ACCEPTANCE D13 and defects c51808e7 and a034fc2d: status names the window by what the shell
    /// last reported and a person last did, and a reason the shell gave for a window it cannot
    /// build stands, also after the shell goes, until a person acts.
    #[test]
    fn the_status_names_the_window_by_what_the_shell_last_reported() {
        use crate::install::setup_state::{self, SetupRecord};
        use crate::intake::desktop::UnavailableReason;
        use crate::language::workbench::WindowReason;
        use crate::workbench::setup::SetupProgress;

        let home = TestHome::new("status-window");
        let fake = Arc::new(Fake::default());
        let host = ServiceHost::start_in_world(home.path(), {
            let fake = Arc::clone(&fake);
            move |_, _| fake.world()
        })
        .unwrap();
        host.publish_ready().unwrap();
        // With setup complete a tray Open only says the person has the window open.
        let record = host.intake.layout.setup_record();
        assert!(setup_state::record_if_absent(&record, &SetupRecord::now()).unwrap());
        host.workbench.attach_setup(SetupProgress::load(&record));
        let window = || cli_client::status_at(home.path()).unwrap().window;
        let send = |stream: &mut Connection, sent: DesktopFrame| {
            frame::write_frame(stream, &sent).unwrap();
        };
        let open = DesktopFrame::Tray {
            item: TrayItemId::Open,
        };
        let cannot_build = DesktopFrame::Unavailable {
            reason: UnavailableReason::NoWebview,
        };
        assert_eq!(window(), None, "presence has nothing to say yet");

        // Presence's own tray launch attaches with the icon only, and a tray Open is a person
        // opening the window.
        fake.advance(super::presence::LAUNCH_DELAY);
        host.intake.presence.tick();
        assert_eq!(fake.launches(), [true]);
        let (mut stream, _) = open_desktop();
        eventually(|| window() == Some(WindowReason::Tray));
        send(&mut stream, open.clone());
        eventually(|| window() == Some(WindowReason::Shown));

        // A show the shell cannot build reads as its reason while the shell stays attached, and a
        // later show the shell builds ends it.
        host.workbench.reveal(Some(PageAddress::About)).unwrap();
        assert!(matches!(next_frame(&mut stream), DesktopFrame::Show { .. }));
        send(&mut stream, cannot_build.clone());
        eventually(|| window() == Some(WindowReason::NoWebview));
        assert_eq!(host.quit.shells().attached(), 1, "the shell is still there");
        host.workbench.reveal(None).unwrap();
        assert!(matches!(next_frame(&mut stream), DesktopFrame::Show { .. }));
        eventually(|| window() == Some(WindowReason::Shown));

        // A close that stays leaves the icon, and the tray's Open brings the window back.
        send(&mut stream, DesktopFrame::Closed { stays: true });
        eventually(|| window() == Some(WindowReason::Tray));
        send(&mut stream, open);
        eventually(|| window() == Some(WindowReason::Shown));

        // The reason a shell gave outlives it, until a person starts a shell again.
        send(&mut stream, cannot_build);
        eventually(|| window() == Some(WindowReason::NoWebview));
        drop(stream);
        eventually(|| host.quit.shells().attached() == 0);
        assert_eq!(window(), Some(WindowReason::NoWebview));
        let (_person, _) = open_desktop();
        eventually(|| window() == Some(WindowReason::Shown));
        drop(host);
    }

    /// Write `sent` to the shell's stream, then a click on the tray entry `item`, and return the
    /// frames the shell hears before the tray model that answers the click. The service reads a
    /// shell's frames in order and the click changes the tray, so whatever `sent` made the
    /// service write comes first.
    fn heard_before_a_click(
        stream: &mut Connection,
        sent: &DesktopFrame,
        item: TrayItemId,
    ) -> Vec<DesktopFrame> {
        frame::write_frame(stream, sent).unwrap();
        frame::write_frame(stream, &DesktopFrame::Tray { item }).unwrap();
        let mut heard = Vec::new();
        loop {
            match next_frame(stream) {
                DesktopFrame::TrayModel { .. } => return heard,
                other => heard.push(other),
            }
        }
    }

    /// ADR-0199's shell milestone 2 amendment and docs/trust/data-flows.md: the hint that
    /// Ghostlight keeps running follows only a person's close that keeps the icon, once per home.
    /// An update's goodbye, a shell that vanishes and a close that leaves no icon say nothing and
    /// leave the hint to that first close, which writes `state/tray-hint` before it is said.
    #[test]
    fn only_the_first_close_that_keeps_the_icon_says_that_ghostlight_keeps_running() {
        use crate::language::workbench::{TRAY_HINT_BODY, TRAY_HINT_TITLE};

        let home = TestHome::new("tray-hint");
        let marker = Layout::new(home.path()).tray_hint();
        let hint = DesktopFrame::Notice {
            title: TRAY_HINT_TITLE.to_owned(),
            body: TRAY_HINT_BODY.to_owned(),
        };
        let stays = DesktopFrame::Closed { stays: true };
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();

        // An update's goodbye and a shell that vanishes leave the hint unused.
        let mut updated = attached_shell(&host);
        host.quit.shells().say_goodbye(GoodbyeReason::Replace);
        assert_eq!(desktop_goodbye(&mut updated), GoodbyeReason::Replace);
        drop(updated);
        eventually(|| host.quit.shells().attached() == 0);
        drop(attached_shell(&host));
        eventually(|| host.quit.shells().attached() == 0);
        assert!(
            !marker.exists(),
            "neither an update nor a crash uses up the hint"
        );

        // A close that leaves no icon says nothing; the first close that keeps it says it, once.
        let mut shell = attached_shell(&host);
        let heard = heard_before_a_click(
            &mut shell,
            &DesktopFrame::Closed { stays: false },
            TrayItemId::Hold,
        );
        assert!(heard.is_empty(), "{heard:?}");
        assert!(!marker.exists());
        let heard = heard_before_a_click(&mut shell, &stays, TrayItemId::Resume);
        assert_eq!(heard, [hint]);
        assert!(
            marker.is_file(),
            "the marker is written before the hint is said"
        );
        let heard = heard_before_a_click(&mut shell, &stays, TrayItemId::Hold);
        assert!(heard.is_empty(), "{heard:?}");
        drop(shell);
        drop(host);

        // The next service of the home, which starts with its agents active, says it no more.
        let host = ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut shell = attached_shell(&host);
        let heard = heard_before_a_click(&mut shell, &stays, TrayItemId::Hold);
        assert!(heard.is_empty(), "{heard:?}");
        drop(host);
    }

    /// ADR-0199 Decision 7 and its shell milestone 2 amendment (079727e9): every native act the
    /// service asks of the shell gets one answer, the person's choice or the shell's failure, or
    /// unavailable once the shell or the service goes, and no open dialog holds a quit.
    #[test]
    fn every_native_act_gets_one_answer_from_the_shell_or_unavailable_when_it_goes() {
        {
            let home = TestHome::new("shell-acts");
            let host = ServiceHost::start(home.path()).unwrap();
            host.publish_ready().unwrap();
            let mut stream = attached_shell(&host);

            // An answer for an act nobody asked is ignored, and the acts that follow are answered.
            answer(&mut stream, 99, ActOutcome::Done);

            // A pick returns what the person chose, nothing when they cancelled, and a failure.
            let route = host.route();
            let picked = in_background(move || route.pick_file("Choose a file"));
            let (id, act) = next_act(&mut stream);
            assert_eq!(
                act,
                NativeAct::PickFile {
                    title: "Choose a file".to_owned()
                }
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
                Some(PathBuf::from("chosen.json"))
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

            // A link and a folder are opened by the shell, and their failure is told.
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
                let folder = in_background(move || route.open_folder(Path::new("logs")));
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

            // A pick is released when the shell's stream ends.
            let route = host.route();
            let picked = in_background(move || route.pick_file("Choose"));
            next_act(&mut stream);
            drop(stream);
            assert!(matches!(
                picked.recv_timeout(PATIENCE).unwrap(),
                Err(WorkbenchPresentationError::Unavailable)
            ));
            eventually(|| host.quit.shells().attached() == 0);

            // A pick is released by a goodbye, which the shell hears.
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

        // A quit releases a pick at once and does not wait for it, and no act starts after it
        // began. Its grace is far longer than this test waits, so a quit that waited for the
        // person's dialog would not end in time.
        {
            let (_home, host, quit_requested) = quittable_host("shell-acts-quit");
            let mut stream = attached_shell(&host);
            let route = host.route();
            let picked = in_background(move || route.pick_file("Choose"));
            next_act(&mut stream);
            host.quit_handle().begin(QuitReason::Quit, PATIENCE * 6);
            assert!(matches!(
                picked.recv_timeout(PATIENCE).unwrap(),
                Err(WorkbenchPresentationError::Unavailable)
            ));
            quit_requested
                .recv_timeout(PATIENCE)
                .expect("a person's open dialog does not hold the quit");
            assert!(
                matches!(
                    host.route().pick_file("Too late"),
                    Err(WorkbenchPresentationError::Unavailable)
                ),
                "no act starts once a quit has begun"
            );
            drop(host);
        }

        // The end of the session releases a pick.
        {
            let (_home, host, _quit_requested) = quittable_host("shell-acts-session-end");
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
        quit_requested: mpsc::Receiver<()>,
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
        let (quit, quit_requested) = mpsc::channel();
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

    /// ACCEPTANCE C22 (ADR-0199 Decision 12): a stream of any role, or work in flight, keeps an
    /// idle service, and its idle exit says nothing, latches nothing, starts no shell and adds
    /// nothing to another end, so the next browser start is served.
    #[test]
    fn the_idle_exit_waits_for_every_stream_and_work_and_leaves_quietly() {
        use crate::browser::adapter::chromium::protocol4::fake::FakeExtension;
        use crate::browser::adapter::chromium::protocol4::fake::{Reply as FakeReply, Script};

        // A browser that answers every request with an empty tab list.
        let listing = || -> Script {
            Box::new(|correlation, _| {
                FakeReply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))
            })
        };

        // An open MCP stream holds the service, and the last close arms the limit again.
        {
            let idling = idling_host("idle-stream", &Arc::default());
            let socket = open_mcp(None);
            idling.stays_past_the_limit("an open stream holds the service");
            drop(socket);
            idling.leaves_past_the_limit("the last close arms the limit and the service leaves");
        }

        // A control call is an opening, so each one restarts the span.
        {
            let idling = idling_host("idle-control", &Arc::default());
            for _ in 0..4 {
                idling.clock.advance(IDLE_LIMIT * 9 / 10);
                idling.looked_and_stayed("the last call came less than a limit ago");
                cli_client::status().expect("the service answers a status call");
                eventually(|| idling.host.intake.streams.open() == 0);
            }
            idling.leaves_past_the_limit("the service leaves a limit after the last call");
        }

        // An attached shell is a stream.
        {
            let idling = idling_host("idle-shell", &Arc::default());
            let (shell, _handed) = open_desktop();
            idling.stays_past_the_limit("an attached shell holds the service");
            drop(shell);
            idling.leaves_past_the_limit("the service leaves once the shell has gone");
        }

        // Work the executor still holds re-arms the limit instead of leaving.
        {
            let idling = idling_host("idle-work", &Arc::default());
            let running = idling.host.quit.drain.admit().unwrap();
            idling.stays_past_the_limit("held work keeps the service");
            drop(running);
            idling.leaves_past_the_limit("the service leaves a limit after the work finished");
        }

        // A browser stream opened through the intake holds the service until it ends, and a
        // connection that replaces it for the same browser is still that one stream (the browser
        // port reports the replacement as an attach and never the retired connection's end).
        {
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
            let fake =
                FakeExtension::connect(extension, &FakeExtension::hello("browser_idle"), listing());
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

            let (service, extension) = pair().unwrap();
            let again =
                FakeExtension::connect(extension, &FakeExtension::hello("browser_idle"), listing());
            host.browser.attach(service).unwrap();
            assert_eq!(
                host.intake.streams.open(),
                1,
                "a connection that replaces another for the same browser is the same stream"
            );
            idling.stays_past_the_limit("the browser holds the service on its new connection");
            again.close();
            drop(again);
            idling.leaves_past_the_limit("the service leaves once the browser has gone");
            fake.close();
            drop(fake);
            let _ = carrier.join();
        }

        // The idle exit sets no latch, admits nothing more, starts no shell when one falls due,
        // and the next browser start is served.
        {
            let fake = Arc::new(Fake::default());
            let idling = idling_host("idle-exit", &fake);
            idling.leaves_past_the_limit("the idle service asks the process to exit");
            fake.advance(super::presence::LAUNCH_DELAY);
            idling.host.intake.presence.tick();
            assert!(
                fake.launches().is_empty(),
                "a service that is leaving starts no shell"
            );
            let Idling { host, home, .. } = idling;
            assert!(
                !super::latch::is_set(&Layout::new(home.path()).quit_latch()),
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

        // The control: on a service that stays, presence starts the tray shell when it falls due.
        {
            let live = Arc::new(Fake::default());
            let idling = idling_host("idle-exit-control", &live);
            live.advance(super::presence::LAUNCH_DELAY);
            idling.host.intake.presence.tick();
            assert_eq!(live.launches(), [true]);
        }

        // Once the end of the session is under way, the idle limit passing adds nothing.
        {
            let idling = idling_host("idle-session-end", &Arc::default());
            idling.host.quit_handle().session_end();
            idling
                .quit_requested
                .recv_timeout(PATIENCE)
                .expect("the session end asks the process to exit");
            idling.clock.advance(IDLE_LIMIT * 3);
            let streams = Arc::clone(&idling.host.intake.streams);
            let before = streams.looks();
            eventually(|| streams.looks() > before);
            let Idling {
                host,
                quit_requested,
                ..
            } = idling;
            // Dropping the host joins the idle watch, so what that look decided has been done, and
            // the quit signal, with its sender, is gone.
            drop(host);
            assert_eq!(
                quit_requested.recv_timeout(PATIENCE),
                Err(mpsc::RecvTimeoutError::Disconnected),
                "the idle limit passing during the exit makes no second request"
            );
        }
    }

    /// Defect m2-06 (987fe943, ADR-0199 amendment of 2026-10-16): a start that arrives while the
    /// service is leaving on idle waits for the lock the leaving service still holds and serves,
    /// instead of giving way to a service that is about to exit.
    #[test]
    fn a_start_that_arrives_while_the_service_is_leaving_on_idle_waits_for_it_and_serves() {
        let idling = idling_host("idle-exit-race", &Arc::default());
        idling.leaves_past_the_limit("the idle service asks the process to exit");
        let Idling { host, home, .. } = idling;
        // The old service still holds the lock and the endpoint while it tears down.
        let directory = home.path().to_path_buf();
        let (began, beginning) = mpsc::channel();
        let starter = thread::spawn(move || {
            let started = Instant::now();
            // The token check comes before the start first asks for the lock.
            let served = ServiceHost::start_with_token(
                &directory,
                StartTerms::for_reason(StartReason::Browser),
                move || {
                    let _ = began.send(());
                    Ok(false)
                },
            )
            .map(drop);
            (served, started.elapsed())
        });
        beginning.recv_timeout(PATIENCE).expect("the start began");
        drop(host);
        let (served, waited) = starter.join().unwrap();
        served.expect("the start takes the lock the old service released");
        assert!(waited < super::lifetime::LEAVING_LOCK_WAIT);
    }

    /// ACCEPTANCE B22 and defect a241b633: a browser that connects is remembered for the next
    /// service of the home by the installed browser its name names, one that reports no name is
    /// Chromium by that name, and a record that cannot be read is no memory.
    #[test]
    fn a_connected_browser_is_remembered_for_the_next_service_by_its_name() {
        use crate::browser::adapter::chromium::protocol4::fake::{
            FakeExtension, Reply as FakeReply, Script,
        };
        use crate::browser::{BrowserPort as _, BrowserSummary};
        use crate::install::native_host::browser_id_named;

        let listing = || -> Script {
            Box::new(|correlation, _| {
                FakeReply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))
            })
        };
        let home = temp_home("last-browser");
        let record = Layout::new(home.path()).last_browser_record();

        // A browser that reports its name is remembered as the installed browser it names, and
        // the next service of the home reads it.
        let hello = FakeExtension::hello("browser_last");
        let named = browser_id_named(hello["browser_name"].as_str().unwrap());
        assert!(named.is_some(), "{hello}");
        {
            let host = ServiceHost::start(home.path()).unwrap();
            assert_eq!(host.browser.last_connected_browser(), None);
            let (service, extension) = pair().unwrap();
            let fake = FakeExtension::connect(extension, &hello, listing());
            host.browser.attach(service).unwrap();
            assert_eq!(host.browser.last_connected_browser().as_deref(), named);
            fake.close();
        }
        let fresh = ServiceHost::start(home.path()).unwrap();
        assert_eq!(
            fresh.browser.last_connected_browser().as_deref(),
            named,
            "the next service of the home remembers it"
        );

        // Plain Chromium's brands are all generic, so its extension reports no name: it is
        // Chromium, the family every adapter belongs to, remembered and read by that name.
        let family = BrowserSummary {
            id: String::new(),
            name: None,
            adapter_version: String::new(),
            attended: false,
        }
        .display_name();
        let mut hello = FakeExtension::hello("browser_plain");
        hello.as_object_mut().unwrap().remove("browser_name");
        let (service, extension) = pair().unwrap();
        let fake = FakeExtension::connect(extension, &hello, listing());
        fresh.browser.attach(service).unwrap();
        assert_eq!(
            fresh.browser.last_connected_browser().as_deref(),
            browser_id_named(&family)
        );
        assert!(
            fresh
                .browser
                .browsers()
                .iter()
                .any(|browser| browser.id == "browser_plain"
                    && browser.name.as_deref() == Some(family.as_str())),
            "the model reads a name, not null"
        );
        fake.close();

        // A record that cannot be read is no memory.
        std::fs::write(&record, b"{").unwrap();
        assert_eq!(fresh.browser.last_connected_browser(), None);
    }
}
