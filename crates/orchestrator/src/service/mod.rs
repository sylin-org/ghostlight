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
mod browser_events;
mod connections;
pub mod hint;
pub mod idle;
pub mod latch;
pub mod lifetime;
pub mod lock;
mod mcp_stream;
pub mod presence;
mod quit;
mod release;
pub mod shell;
#[cfg(test)]
pub(crate) mod test_home;
mod toolset;

pub use quit::Quit;

use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle, Thread};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ghostlight_relay::endpoint::{Endpoint, Listener};
use ghostlight_relay::protocol::{QuitReason, StartReason};
use uuid::Uuid;

use crate::audit::AuditRecorder;
use crate::audit::JsonlAuditSink;
use crate::browser::adapter::chromium::protocol4::assets::page_assets;
use crate::browser::{AdapterLifecycleObserver, BrowserPort, RelayBrowserPort};
use crate::diagnostics::DiagnosticsHub;
use crate::governance::GovernanceFacade;
use crate::install;
use crate::install::icons::{self, IconCache, IconPlaces};
use crate::intake::mcp::failure_templates;
use crate::language::environment::{ELEVATED_SERVICE, QUIT_HELD};
use crate::presentation::{BrowserPresentation, PresentationReactor};
use crate::provenance::SourceReader;
use crate::sync::lock;
use crate::work::ApplicationExecutor;
use crate::workbench::setup::SetupProgress;
use crate::workbench::web::{PendingWorkbench, WorkbenchAddress};
use crate::workbench::{Watching, WorkbenchFacade, WorkbenchProjection};
use crate::workspace::WorkspaceStore;
use browser_events::ServiceBrowserEvents;
use connections::{spawn_listener, ServiceContext};
use presence::Presence;
use shell::{ShellRoute, TrayRelay};
use toolset::Toolsets;

use admission::{Capacity, QueueBudget};
use idle::Streams;
use lifetime::{Clock, ReadyWatch, SystemClock, READY_LIMIT};
use lock::ServiceLock;

/// The most tabs one diagnostics clear names: the extension takes no more, and the protocol 4
/// vocabulary's `clear_diagnostics_tabs` pins the two together.
pub(crate) const DIAGNOSTIC_CLEAR_BATCH_SIZE: usize = 256;

/// Prefix of the value that changes on every service start.
const INSTANCE_PREFIX: &str = "instance_";

/// Environment variable naming an audit log in place of the one in the state directory.
const AUDIT_FILE_VARIABLE: &str = "GHOSTLIGHT_AUDIT_FILE";

/// File name of the audit log in the state directory.
const AUDIT_FILE_NAME: &str = "audit.jsonl";

/// What the runtime-published diagnostics line says: where the relay and the workbench listen.
///
/// It names the workbench by address only. The bearer is not an input, so it cannot appear.
fn published_detail(endpoint: Option<&str>, workbench: Option<&WorkbenchAddress>) -> String {
    format!(
        "endpoint={} relay={} workbench={}",
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
    /// The process-diagnostics hub this service owns.
    pub diagnostics: Arc<DiagnosticsHub>,
    browser: Arc<RelayBrowserPort>,
    audit: Arc<AuditRecorder>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    refresher: Thread,
    home: PathBuf,
    context: Arc<ServiceContext>,
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

/// Another service holds this home's service lock and stays, so this start gives way to it: the
/// caller reaches that service by its endpoint ([`lifetime::lock_wait`]).
#[derive(Debug, thiserror::Error)]
#[error("another Ghostlight service already owns this runtime")]
pub struct ServiceAlreadyRunning;

/// The person quit Ghostlight, and this start is one that Quit holds back.
#[derive(Debug, thiserror::Error)]
#[error("{}", QUIT_HELD)]
pub struct QuitLatched;

/// The presence world a start makes from the installation and the question "has a quit begun".
type PresenceWorld =
    Box<dyn FnOnce(install::layout::Layout, presence::Ending) -> Box<dyn presence::Outside>>;

/// What a start reads and acts on outside the service: whether this process's token is elevated,
/// the world presence acts on, the clock the idle limit is judged by, and where the installed
/// agents' icons are looked for, if anywhere.
pub struct World {
    elevated: Box<dyn FnOnce() -> io::Result<bool>>,
    presence: PresenceWorld,
    idle_clock: Arc<dyn Clock>,
    /// Read once the start has succeeded. Only the system world has them, so a test service never
    /// reads this computer's apps.
    icon_places: Option<fn() -> IconPlaces>,
}

impl World {
    /// This computer: the process token, presence that starts the shell, the system clock and
    /// the installed agents' icons.
    #[must_use]
    pub fn system() -> Self {
        Self {
            elevated: Box::new(ghostlight_win_peer::is_elevated),
            presence: Box::new(presence::System::boxed),
            idle_clock: Arc::new(SystemClock),
            icon_places: Some(IconPlaces::system),
        }
    }

    /// The system world with a token that is not elevated and no icon lookup.
    #[cfg(test)]
    pub(crate) fn test() -> Self {
        Self {
            elevated: Box::new(|| Ok(false)),
            icon_places: None,
            ..Self::system()
        }
    }

    /// This world with the answer to "is this process elevated" given by `elevated`.
    #[cfg(test)]
    pub(crate) fn with_token(self, elevated: impl FnOnce() -> io::Result<bool> + 'static) -> Self {
        Self {
            elevated: Box::new(elevated),
            ..self
        }
    }

    /// This world with presence acting on what `world` makes, so a test starts no process.
    #[cfg(test)]
    pub(crate) fn with_presence(
        self,
        world: impl FnOnce(install::layout::Layout, presence::Ending) -> Box<dyn presence::Outside>
            + 'static,
    ) -> Self {
        Self {
            presence: Box::new(world),
            ..self
        }
    }

    /// This world with the idle limit judged by `clock`, which a test moves by hand.
    #[cfg(test)]
    pub(crate) fn with_idle_clock(self, clock: Arc<dyn Clock>) -> Self {
        Self {
            idle_clock: clock,
            ..self
        }
    }
}

impl ServiceHost {
    /// Prepare the service in the Ghostlight home `home` for an agent start that asks for the lock
    /// once, in the test world, without binding the endpoint.
    #[cfg(test)]
    pub(crate) fn start_for_test(home: &Path) -> Result<Self> {
        Self::start(
            home,
            StartTerms::asking_once(StartReason::Agent),
            World::test(),
        )
    }

    /// The route the workbench facade is given, for tests that drive it.
    #[cfg(test)]
    pub(crate) fn route(&self) -> Arc<ShellRoute> {
        Arc::new(ShellRoute::new(
            self.quit.shells(),
            Arc::clone(&self.context.presence),
        ))
    }

    /// Prepare the service in the Ghostlight home `home` for a start on `terms`, in `world`, under
    /// its lifetime lock, without binding the endpoint. `publish_ready` binds it.
    ///
    /// The start is admitted first ([`admit`]) and opens no state before it is. When `world` names
    /// where the installed agents' icons are, the service then shows their own icons: it
    /// publishes the icon cache in the home's state directory, which every agent's symbol reads,
    /// and looks for the icons in the background.
    pub fn start(home: &Path, terms: StartTerms, world: World) -> Result<Self> {
        let World {
            elevated,
            presence,
            idle_clock,
            icon_places,
        } = world;
        let diagnostics = DiagnosticsHub::birth(home);
        let host = Self::assemble(
            home,
            Arc::clone(&diagnostics),
            terms,
            elevated,
            presence,
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
            } else if !error.is::<ServiceAlreadyRunning>() {
                startup_failed(&diagnostics, error);
            }
        })?;
        if let Some(places) = icon_places {
            let cache = IconCache::open(
                install::layout::Layout::new(home).icons(),
                places(),
                Some(host.diagnostics.sink()),
            );
            icons::publish(&cache);
            host.workbench.attach_icons(cache);
        }
        Ok(host)
    }

    /// Admit the start, then build the service's parts and start its threads.
    fn assemble(
        home: &Path,
        diagnostics: Arc<DiagnosticsHub>,
        terms: StartTerms,
        elevated: impl FnOnce() -> io::Result<bool>,
        world: PresenceWorld,
        idle_clock: Arc<dyn Clock>,
    ) -> Result<Self> {
        let layout = install::layout::Layout::new(home);
        let lock = admit(&layout, terms, elevated)?;
        let lock_taken = Instant::now();
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
        let context = Arc::new(ServiceContext {
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
            context,
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
        if lifetime::replaces_a_service(self.context.started_by) {
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
        let thread = spawn_listener(Arc::clone(&listener), Arc::clone(&self.context));
        *bound = Some(Bound { listener, thread });
        self.ready.settle();
        self.context.streams.published();
        self.context.presence.published();
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
            .attach_quit(Arc::new(self.context.quit.clone()));
        self.context
            .governance
            .protect_workbench_port(pending.address().port());
        let watching = Arc::new(Watching::load(
            install::layout::Layout::new(&self.home).watch_off_marker(),
        ));
        let server =
            Arc::new(pending.serve(&self.workbench, self.context.instance.clone(), watching));
        let address = server.address().clone();
        let _ = self.context.workbench_server.set(server);
        Some(address)
    }

    /// Where the workbench is served, once `publish_ready` has started its server.
    #[must_use]
    #[cfg(test)]
    pub fn workbench_address(&self) -> Option<WorkbenchAddress> {
        self.context
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

    /// The process-diagnostics hub this service owns.
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
        if let Some(server) = self.context.workbench_server.get() {
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

/// Admit a start on `terms` to the home `layout` describes, and hand it the service lock.
///
/// In order: a process whose token `elevated` says is elevated is refused; a browser or recover
/// start is refused while the quit latch is set ([`QuitLatched`]); the lock is taken, and a start
/// that finds it held asks again, an update for [`lifetime::UPDATE_LOCK_WAIT`] so the service it
/// replaces can finish leaving and any other start for [`lifetime::LEAVING_LOCK_WAIT`] so it
/// outlasts a service leaving on idle, and gives way ([`ServiceAlreadyRunning`]) when the wait
/// ends; a browser or recover start that waited looks at the latch again once it holds the lock;
/// and only then does any other start clear the latch.
fn admit(
    layout: &install::layout::Layout,
    terms: StartTerms,
    elevated: impl FnOnce() -> io::Result<bool>,
) -> Result<ServiceLock> {
    refuse_elevated_token(elevated)?;
    if lifetime::refused_by_quit_latch(terms.reason) && latch::is_set(&layout.quit_latch()) {
        return Err(QuitLatched.into());
    }
    let lock = acquire_lock(&layout.service_lock(), terms.lock_wait)?;
    // A Quit that began while this start waited for the lock is the old service's own, and this
    // start must not bring back what the person just quit.
    if lifetime::refused_by_quit_latch(terms.reason) && latch::is_set(&layout.quit_latch()) {
        return Err(QuitLatched.into());
    }
    if !lifetime::refused_by_quit_latch(terms.reason) {
        latch::clear(&layout.quit_latch()).context("clear the quit latch")?;
    }
    Ok(lock)
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
            return Err(ServiceAlreadyRunning.into());
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

#[cfg(test)]
mod tests;
