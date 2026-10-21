//! Pre-effect browser-readiness recovery decisions.
//!
//! The executor asks this service in two cases. A session that works in no browser yet asks when
//! the ordinary plural-browser resolver proves that no usable adapter exists and no browser is on
//! its way back. This module then inspects local installation facts and never presents a browser
//! choice: unique connectable evidence acts under the configured posture; among several installed
//! browsers, the one that last connected acts the same way, and with no such memory plural
//! evidence repairs what Ghostlight already owns and leaves startup to the person (ADR-0149
//! amendment, 2026-10-17). No refusal spends its words declining to choose.
//!
//! A session whose own browser stayed gone through the return grace asks for that installed
//! browser by name. Under `on_demand` the flight starts that browser and never another. It leaves
//! startup to the person when the posture is `manual` or the browser is not one Ghostlight can
//! start, and starts nothing when the browser is already running in another profile (ADR-0196
//! Decision 12 amendment, 2026-10-17).
//!
//! A service that reaches no display, such as one an agent started over SSH, cannot show the
//! browser it would start, so either flight leaves startup to the person there too (ADR-0149
//! amendment "A service with no display"). So does a service inside a Windows app container, such
//! as one an agent in a Store app started: a browser it started would write to that app's private
//! view and might close with the app (ADR-0149 amendment "A service inside an app container").
//!
//! Where the selected platform and policy permit it, a flight performs one ordinary-profile
//! launch and waits within the invocation deadline for an inbound adapter. Every flight shares
//! one launch guard per installed browser: while a start Ghostlight made is younger than
//! [`LAUNCH_HANDSHAKE_WINDOW`] and no connection of that browser has arrived, no flight starts
//! it again, so two sessions asking at once, or a retry after a slow start, never open a second
//! window.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::os::session;

use super::{choose_browser, BrowserPort};
use crate::governance::manifest::BrowserStartup;
use crate::governance::GovernanceFacade;
use crate::install::browser_package::BrowserPackage;
use crate::install::container::Residence;
use crate::install::native_host::{NativeHostRegistry, NativeHostState};
use crate::language::MAX_TIMEOUT_MS;
use crate::sync::lock;

/// How long a browser start Ghostlight made holds the launch guard while no connection of that
/// browser has arrived: the longest wait any call can give a handshake. A request inside it waits
/// for that start instead of starting the browser again.
const LAUNCH_HANDSHAKE_WINDOW: Duration = Duration::from_millis(MAX_TIMEOUT_MS);

/// One browser installation that can inform a recovery decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryCandidate {
    /// Stable installed-browser id.
    pub id: String,
    /// Human-readable browser name.
    pub name: String,
    /// Whether its package can start a native messaging host.
    pub package: BrowserPackage,
    /// Existing package diagnosis, including the native-package remedy for sandboxes.
    pub package_detail: String,
    /// Ownership and freshness of its native-host registration.
    pub registration: NativeHostState,
    /// Exact ordinary-profile executable verified during the inventory snapshot.
    pub ordinary_executable: Option<PathBuf>,
}

/// Every terminal recovery failure, kept closed so each one can have an exact remedy.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RecoveryFailure {
    /// No supported browser installation was found.
    BrowserAbsent,
    /// Starting the selected browser process failed.
    LaunchFailed,
    /// Only an unsupported Snap or Flatpak browser was found.
    SandboxedPackage,
    /// The selected browser came up without the Ghostlight extension.
    ExtensionAbsent,
    /// Native-host registration is missing, foreign, or otherwise unusable.
    NativeHostUnavailable,
    /// A different Ghostlight installation owns the native-host registration.
    OwnedElsewhere,
    /// No adapter handshake arrived within the bounded wait.
    HandshakeTimeout,
}

/// One deterministic recovery answer before any browser process is started.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryDecision {
    /// Startup is left to the person: either the configured posture is manual, or more than one
    /// browser could serve and none of them is a usable browser that last connected, or the one
    /// browser a request named is not one Ghostlight can start, or no display is reachable from
    /// the service, or the service runs inside an app container.
    Manual {
        /// Installed browsers with a current Ghostlight native-host registration.
        browsers: Vec<RecoveryCandidate>,
    },
    /// One connected adapter is ready for the ordinary resolver. For a request that named an
    /// installed browser, it is a connection of that browser, which may be another profile.
    Ready {
        /// Opaque connected browser identity reported by the adapter.
        browser: String,
    },
    /// Recovery cannot safely select or prepare a browser.
    Failed {
        /// Exact closed reason.
        reason: RecoveryFailure,
        /// Candidate names or package diagnoses that make the reason useful.
        details: Vec<String>,
    },
}

/// Cancellation or deadline won while joining another recovery request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryWaitError {
    /// The invocation was cancelled before any physical attempt.
    Cancelled,
    /// The invocation deadline expired before any physical attempt.
    Deadline,
}

impl From<RecoveryWaitError> for super::BrowserError {
    /// A wait that ended before any physical attempt sent nothing.
    fn from(error: RecoveryWaitError) -> Self {
        match error {
            RecoveryWaitError::Cancelled => Self::CancelledBeforeDispatch,
            RecoveryWaitError::Deadline => Self::DeadlineBeforeDispatch,
        }
    }
}

trait BrowserInventory: Send + Sync {
    fn inspect(&self) -> Result<Vec<RecoveryCandidate>, ()>;
}

type MechanismFailure = (RecoveryFailure, String);

#[derive(Clone, Debug, Eq, PartialEq)]
enum MechanismError {
    Wait(RecoveryWaitError),
    Failed(MechanismFailure),
    /// A browser the service started would not be the person's: no display is reachable from the
    /// service, or the service runs inside an app container. The person can still open the
    /// browser on their desktop, where its connection reaches the same service. Nothing was
    /// started.
    LeftToThePerson,
}

trait RecoveryMechanism: Send + Sync {
    fn repair(
        &self,
        browser: &RecoveryCandidate,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), MechanismError>;

    fn launch(
        &self,
        browser: &RecoveryCandidate,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), MechanismError>;

    /// Whether every launch leaves startup to the person, because the service runs inside an app
    /// container.
    fn leaves_startup_to_the_person(&self) -> bool {
        false
    }
}

#[derive(Debug)]
struct SystemBrowserInventory;

impl BrowserInventory for SystemBrowserInventory {
    fn inspect(&self) -> Result<Vec<RecoveryCandidate>, ()> {
        let registry = NativeHostRegistry::discover();
        registry
            .check()
            .map(|report| {
                report
                    .browsers
                    .into_iter()
                    .map(|browser| {
                        let ordinary_executable = registry.browser_executable(&browser.id);
                        RecoveryCandidate {
                            id: browser.id,
                            name: browser.name,
                            package: browser.package,
                            package_detail: browser.package_detail,
                            registration: browser.state,
                            ordinary_executable,
                        }
                    })
                    .collect()
            })
            .map_err(|_| ())
    }
}

/// A mechanism whose repair and launch both succeed, and whose launch runs one test action in
/// place of a browser process, such as connecting a fake adapter or counting.
#[cfg(test)]
struct TestLaunch(Box<dyn Fn() + Send + Sync>);

#[cfg(test)]
impl RecoveryMechanism for TestLaunch {
    fn repair(
        &self,
        _browser: &RecoveryCandidate,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> Result<(), MechanismError> {
        Ok(())
    }

    fn launch(
        &self,
        _browser: &RecoveryCandidate,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> Result<(), MechanismError> {
        (self.0)();
        Ok(())
    }
}

#[cfg(test)]
#[derive(Debug)]
struct FixedBrowserInventory {
    candidates: Vec<RecoveryCandidate>,
}

#[cfg(test)]
impl BrowserInventory for FixedBrowserInventory {
    fn inspect(&self) -> Result<Vec<RecoveryCandidate>, ()> {
        Ok(self.candidates.clone())
    }
}

/// Installed browsers a unit test answers by running a function, which may hold the read open.
#[cfg(test)]
struct InspectingInventory(Box<dyn Fn() -> Vec<RecoveryCandidate> + Send + Sync>);

#[cfg(test)]
impl BrowserInventory for InspectingInventory {
    fn inspect(&self) -> Result<Vec<RecoveryCandidate>, ()> {
        Ok((self.0)())
    }
}

#[derive(Debug)]
struct SystemRecoveryMechanism;

impl RecoveryMechanism for SystemRecoveryMechanism {
    fn repair(
        &self,
        browser: &RecoveryCandidate,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), MechanismError> {
        ensure_live(deadline, cancelled).map_err(MechanismError::Wait)?;
        let registry = NativeHostRegistry::discover();
        let repaired = registry
            .repair_owned_registration(&browser.id)
            .map_err(|error| {
                MechanismError::Failed((
                    RecoveryFailure::NativeHostUnavailable,
                    format!("{}: {error}", browser.name),
                ))
            })?;
        let current = repaired
            .report
            .browsers
            .iter()
            .find(|observed| observed.id == browser.id)
            .is_some_and(|observed| observed.state == NativeHostState::Current);
        if !current {
            return Err(MechanismError::Failed((
                RecoveryFailure::NativeHostUnavailable,
                format!(
                    "{}: automatic repair did not verify a current registration",
                    browser.name
                ),
            )));
        }
        Ok(())
    }

    fn launch(
        &self,
        browser: &RecoveryCandidate,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), MechanismError> {
        self.launch_unless_ending(
            session::session_ending(),
            self.leaves_startup_to_the_person(),
            deadline,
            cancelled,
            || start_ordinary_browser(browser, deadline, cancelled),
        )
    }

    fn leaves_startup_to_the_person(&self) -> bool {
        Residence::detect().container().is_some()
    }
}

impl SystemRecoveryMechanism {
    /// Run `start`, the launch of a browser, unless the session is ending: then nothing is started
    /// and the call ends as cancelled (ADR-0199 Decision 13). In a service `contained` in an app
    /// container nothing is started either, and startup is left to the person.
    fn launch_unless_ending(
        &self,
        session_ending: bool,
        contained: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
        start: impl FnOnce() -> Result<(), MechanismError>,
    ) -> Result<(), MechanismError> {
        ensure_live(deadline, cancelled).map_err(MechanismError::Wait)?;
        if session_ending {
            return Err(MechanismError::Wait(RecoveryWaitError::Cancelled));
        }
        if contained {
            return Err(MechanismError::LeftToThePerson);
        }
        start()
    }
}

/// Start `browser`'s ordinary executable in the person's graphical session. With no display
/// reachable from the service nothing is started, and startup is left to the person.
fn start_ordinary_browser(
    browser: &RecoveryCandidate,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), MechanismError> {
    let executable = browser.ordinary_executable.as_ref().ok_or_else(|| {
        MechanismError::Failed((
            RecoveryFailure::LaunchFailed,
            format!(
                "{} has no ordinary executable Ghostlight can verify",
                browser.name
            ),
        ))
    })?;
    let environment = crate::browser::session::graphical_session_environment()
        .map_err(|error| {
            MechanismError::Failed((RecoveryFailure::LaunchFailed, error.to_string()))
        })?
        .ok_or(MechanismError::LeftToThePerson)?;
    ensure_live(deadline, cancelled).map_err(MechanismError::Wait)?;
    let mut command = ordinary_browser_command(executable);
    command.envs(environment.values());
    start_reaped(command).map(drop).map_err(|error| {
        MechanismError::Failed((
            RecoveryFailure::LaunchFailed,
            format!("{}: {error}", executable.display()),
        ))
    })
}

/// Where a flight goes when a repair or a launch of `browser` did not happen: a lost wait ends
/// the request, a failure is the answer, and a service with no display or inside an app container
/// leaves startup to the person, who can open `browser` on their desktop.
fn settled(
    error: MechanismError,
    browser: RecoveryCandidate,
) -> Result<FlightPhase, RecoveryWaitError> {
    match error {
        MechanismError::Wait(error) => Err(error),
        MechanismError::Failed((reason, detail)) => {
            Ok(FlightPhase::Complete(RecoveryDecision::Failed {
                reason,
                details: vec![detail],
            }))
        }
        MechanismError::LeftToThePerson => Ok(FlightPhase::Complete(RecoveryDecision::Manual {
            browsers: vec![browser],
        })),
    }
}

fn ensure_live(deadline: Instant, cancelled: &AtomicBool) -> Result<(), RecoveryWaitError> {
    if cancelled.load(Ordering::SeqCst) {
        Err(RecoveryWaitError::Cancelled)
    } else if Instant::now() >= deadline {
        Err(RecoveryWaitError::Deadline)
    } else {
        Ok(())
    }
}

/// The ordinary-profile command for `executable`: no arguments, null standard streams and, on
/// Unix, a process group of its own, so a signal meant for the service's group never reaches the
/// person's browser.
fn ordinary_browser_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    command
}

/// Start `command` and collect its exit on a thread of its own, and return its process id. The
/// browser, or a second start that only hands a window to the running browser and exits at
/// once, never lingers as a zombie of the service.
fn start_reaped(mut command: Command) -> std::io::Result<u32> {
    let mut child = command.spawn()?;
    let process_id = child.id();
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(process_id)
}

type RecoveryResult = Result<RecoveryDecision, RecoveryWaitError>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum FlightPhase {
    Inspecting,
    Repairing {
        browser: RecoveryCandidate,
        launch_after: bool,
    },
    RepairingQueue {
        remaining: VecDeque<RecoveryCandidate>,
        named: Vec<RecoveryCandidate>,
    },
    Launching {
        browser: RecoveryCandidate,
    },
    Launched {
        browser: RecoveryCandidate,
    },
    Complete(RecoveryDecision),
}

#[derive(Debug)]
struct FlightState {
    phase: FlightPhase,
    owner: bool,
    participants: usize,
}

/// What one flight recovers, and so which requests share it.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum FlightScope {
    /// A browser for sessions that work in no browser yet.
    Unbound,
    /// One installed browser, by native-host id, for sessions whose own browser it is.
    Browser(String),
}

#[derive(Debug)]
struct RecoveryFlight {
    scope: FlightScope,
    state: Mutex<FlightState>,
    changed: Condvar,
}

impl RecoveryFlight {
    fn new(scope: FlightScope) -> Self {
        Self {
            scope,
            state: Mutex::new(FlightState {
                phase: FlightPhase::Inspecting,
                owner: false,
                participants: 1,
            }),
            changed: Condvar::new(),
        }
    }
}

#[derive(Debug, Default)]
struct Flights {
    scopes: HashMap<FlightScope, Arc<RecoveryFlight>>,
}

/// One browser start Ghostlight made, as the launch guard holds it.
#[derive(Clone, Copy, Debug)]
struct PendingLaunch {
    /// When the start was made, by the recovery clock.
    at: Instant,
    /// The port's count of arrivals of that installed browser when the start was made; a higher
    /// count means a connection of it has arrived since.
    arrivals: u64,
}

/// The source of the current time for the launch guard.
type RecoveryClock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// Cloneable, service-scoped single-flight recovery decision service.
#[derive(Clone)]
pub struct BrowserRecovery {
    governance: GovernanceFacade,
    inventory: Arc<dyn BrowserInventory>,
    browser: Arc<dyn BrowserPort>,
    mechanism: Arc<dyn RecoveryMechanism>,
    flights: Arc<Mutex<Flights>>,
    /// The launch guard: the latest start of each installed browser, by native-host id, shared
    /// by every flight scope.
    launches: Arc<Mutex<HashMap<String, PendingLaunch>>>,
    /// The clock that times the launch guard.
    clock: RecoveryClock,
}

impl BrowserRecovery {
    /// Discover the production governance and browser-installation facts.
    #[must_use]
    pub fn discover(governance: GovernanceFacade, browser: Arc<dyn BrowserPort>) -> Self {
        Self {
            governance,
            inventory: Arc::new(SystemBrowserInventory),
            browser,
            mechanism: discovered_mechanism(),
            flights: Arc::new(Mutex::new(Flights::default())),
            launches: Arc::new(Mutex::new(HashMap::new())),
            clock: Arc::new(Instant::now),
        }
    }

    /// Replace machine discovery with deterministic browser facts in unit tests.
    #[cfg(test)]
    pub(crate) fn set_test_candidates(&mut self, candidates: Vec<RecoveryCandidate>) {
        self.inventory = Arc::new(FixedBrowserInventory { candidates });
    }

    /// Read the installed browsers by running `inspect` in unit tests.
    #[cfg(test)]
    pub(crate) fn set_test_inspection(
        &mut self,
        inspect: impl Fn() -> Vec<RecoveryCandidate> + Send + Sync + 'static,
    ) {
        self.inventory = Arc::new(InspectingInventory(Box::new(inspect)));
    }

    /// Make every launch succeed without an adapter ever arriving, as a browser does when it
    /// starts without the Ghostlight extension. No process is started.
    #[cfg(test)]
    pub(crate) fn set_test_launch_without_adapter(&mut self) {
        self.set_test_launch(|| {});
    }

    /// Make every repair and launch succeed, each launch running `launch` instead of starting a
    /// process.
    #[cfg(test)]
    pub(crate) fn set_test_launch(&mut self, launch: impl Fn() + Send + Sync + 'static) {
        self.mechanism = Arc::new(TestLaunch(Box::new(launch)));
    }

    /// The name of the installed browser that a request from a session working in no browser yet
    /// would start now, when it would start one: the posture is `on_demand` and one browser is
    /// selected, the one that last connected or the only one installed, with a registration
    /// Ghostlight can use, and the service is not inside an app container. Nothing is started or
    /// repaired; the installed browsers are read only under `on_demand`.
    #[must_use]
    pub fn starts_on_request(&self) -> Option<String> {
        let mode = self.governance.browser_startup();
        if mode != BrowserStartup::OnDemand || self.mechanism.leaves_startup_to_the_person() {
            return None;
        }
        let candidates = self.inventory.inspect().ok()?;
        let remembered = self.browser.last_connected_browser();
        match decide(mode, &candidates, remembered.as_deref()) {
            FlightPhase::Launching { browser }
            | FlightPhase::Repairing {
                browser,
                launch_after: true,
            } => Some(browser.name),
            _ => None,
        }
    }

    /// Decide one missing-browser recovery request for a session that works in no browser yet,
    /// joining an overlapping request.
    pub fn request(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<RecoveryDecision, RecoveryWaitError> {
        self.fly(FlightScope::Unbound, deadline, cancelled)
    }

    /// Start the installed browser `installed` (a native-host id such as `chrome`) for a session
    /// whose own browser it is, joining an overlapping request for the same browser.
    ///
    /// Only that browser is ever started. The answer is `Ready` once a connection of it arrives,
    /// or at once when one is already connected, since launching a running browser again would
    /// only open a window in the profile it already runs; that connection may be another
    /// profile. It is `Manual` when the posture is `manual` or the browser is not one Ghostlight
    /// can start, and the ordinary failures, `HandshakeTimeout` among them, otherwise.
    pub fn request_browser(
        &self,
        installed: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<RecoveryDecision, RecoveryWaitError> {
        self.fly(FlightScope::Browser(installed.into()), deadline, cancelled)
    }

    fn fly(
        &self,
        scope: FlightScope,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<RecoveryDecision, RecoveryWaitError> {
        let flight = self.join_flight(scope);
        let result = self.participate(&flight, deadline, cancelled);
        self.leave_flight(&flight);
        result
    }

    fn join_flight(&self, scope: FlightScope) -> Arc<RecoveryFlight> {
        let mut flights = lock(&self.flights);
        if let Some(flight) = flights.scopes.get(&scope) {
            let flight = Arc::clone(flight);
            lock(&flight.state).participants += 1;
            return flight;
        }
        let flight = Arc::new(RecoveryFlight::new(scope.clone()));
        flights.scopes.insert(scope, Arc::clone(&flight));
        flight
    }

    fn leave_flight(&self, flight: &Arc<RecoveryFlight>) {
        let mut flights = lock(&self.flights);
        let scope = &flight.scope;
        let Some(current) = flights.scopes.get(scope) else {
            return;
        };
        if !Arc::ptr_eq(current, flight) {
            return;
        }
        let remove = {
            let mut state = lock(&flight.state);
            debug_assert!(state.participants > 0);
            state.participants = state.participants.saturating_sub(1);
            state.participants == 0
        };
        if remove {
            flights.scopes.remove(scope);
        }
    }

    fn participate(
        &self,
        flight: &RecoveryFlight,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> RecoveryResult {
        loop {
            let mut state = lock(&flight.state);
            if let Err(error) = ensure_live(deadline, cancelled) {
                return match (&state.phase, error) {
                    (FlightPhase::Launched { browser }, RecoveryWaitError::Deadline) => {
                        Ok(RecoveryDecision::Failed {
                            reason: RecoveryFailure::HandshakeTimeout,
                            details: vec![browser.name.clone()],
                        })
                    }
                    (_, error) => Err(error),
                };
            }
            if let FlightPhase::Complete(decision) = &state.phase {
                return Ok(decision.clone());
            }
            if !state.owner {
                state.owner = true;
                let phase = state.phase.clone();
                drop(state);

                let next = self.advance(&flight.scope, phase.clone(), deadline, cancelled);

                let mut state = lock(&flight.state);
                state.owner = false;
                match next {
                    Ok(next) => state.phase = next,
                    Err(error) => {
                        state.phase = phase;
                        flight.changed.notify_all();
                        return Err(error);
                    }
                }
                flight.changed.notify_all();
                continue;
            }
            let wait = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(10));
            let (next, _) = flight
                .changed
                .wait_timeout(state, wait)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            drop(next);
        }
    }

    fn advance(
        &self,
        scope: &FlightScope,
        phase: FlightPhase,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<FlightPhase, RecoveryWaitError> {
        match (scope, phase) {
            (FlightScope::Browser(installed), FlightPhase::Inspecting) => {
                Ok(self.inspect_for(installed))
            }
            (FlightScope::Browser(installed), FlightPhase::Launched { browser }) => {
                let Some(connected) = self.connected_as(installed) else {
                    thread::sleep(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(10)),
                    );
                    return Ok(FlightPhase::Launched { browser });
                };
                Ok(FlightPhase::Complete(RecoveryDecision::Ready {
                    browser: connected,
                }))
            }
            (FlightScope::Unbound, FlightPhase::Inspecting) => {
                Ok(self.inventory.inspect().map_or_else(
                    |()| unreadable_inventory(),
                    |candidates| {
                        let remembered = self.browser.last_connected_browser();
                        decide(
                            self.governance.browser_startup(),
                            &candidates,
                            remembered.as_deref(),
                        )
                    },
                ))
            }
            (
                _,
                FlightPhase::Repairing {
                    browser,
                    launch_after,
                },
            ) => match self.mechanism.repair(&browser, deadline, cancelled) {
                Ok(()) if launch_after => Ok(FlightPhase::Launching { browser }),
                Ok(()) => Ok(FlightPhase::Complete(RecoveryDecision::Manual {
                    browsers: vec![browser],
                })),
                Err(error) => settled(error, browser),
            },
            (
                _,
                FlightPhase::RepairingQueue {
                    mut remaining,
                    named,
                },
            ) => {
                let Some(browser) = remaining.pop_front() else {
                    return Ok(FlightPhase::Complete(RecoveryDecision::Manual {
                        browsers: named,
                    }));
                };
                match self.mechanism.repair(&browser, deadline, cancelled) {
                    Ok(()) => {
                        let mut named = named;
                        named.push(browser);
                        Ok(FlightPhase::RepairingQueue { remaining, named })
                    }
                    Err(error) => settled(error, browser),
                }
            }
            (_, FlightPhase::Launching { browser }) => {
                if !self.claim_launch(&browser.id) {
                    return Ok(FlightPhase::Launched { browser });
                }
                match self.mechanism.launch(&browser, deadline, cancelled) {
                    Ok(()) => Ok(FlightPhase::Launched { browser }),
                    Err(error) => {
                        self.release_launch(&browser.id);
                        settled(error, browser)
                    }
                }
            }
            (FlightScope::Unbound, FlightPhase::Launched { browser }) => {
                let connected = self.browser.browsers();
                if connected.is_empty() {
                    thread::sleep(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(10)),
                    );
                    return Ok(FlightPhase::Launched { browser });
                }
                let decision = match choose_browser(None, None, &connected) {
                    Ok(browser) => RecoveryDecision::Ready { browser },
                    // Several adapters arrived inside the same bounded wait. The workspace binds
                    // to the first arrival; the ordinary pinned-session rules own placement from
                    // here, and no refusal ever asks anyone to resolve a browser choice.
                    Err(super::BrowserError::AmbiguousBrowser(_)) => RecoveryDecision::Ready {
                        browser: connected[0].id.clone(),
                    },
                    Err(_) => RecoveryDecision::Failed {
                        reason: RecoveryFailure::HandshakeTimeout,
                        details: vec![browser.name],
                    },
                };
                Ok(FlightPhase::Complete(decision))
            }
            (_, FlightPhase::Complete(decision)) => Ok(FlightPhase::Complete(decision)),
        }
    }

    /// The first phase of a flight that starts the installed browser `installed`.
    fn inspect_for(&self, installed: &str) -> FlightPhase {
        if self.governance.browser_startup() == BrowserStartup::Manual {
            return FlightPhase::Complete(RecoveryDecision::Manual {
                browsers: Vec::new(),
            });
        }
        if let Some(running) = self.connected_as(installed) {
            return FlightPhase::Complete(RecoveryDecision::Ready { browser: running });
        }
        self.inventory.inspect().map_or_else(
            |()| unreadable_inventory(),
            |candidates| decide_browser(&candidates, installed),
        )
    }

    /// Take the launch guard of the installed browser `installed` for a start, or learn that the
    /// flight must not start it: a connection of it is present, or a start Ghostlight made less
    /// than [`LAUNCH_HANDSHAKE_WINDOW`] ago has not connected yet. Either way the flight then
    /// waits for that browser exactly as if it had started it. A start that connected, or whose
    /// window has passed, no longer holds the guard.
    fn claim_launch(&self, installed: &str) -> bool {
        if self.connected_as(installed).is_some() {
            return false;
        }
        let arrivals = self.browser.arrivals_of(installed);
        let now = (self.clock)();
        let mut launches = lock(&self.launches);
        let pending = launches.get(installed).is_some_and(|launch| {
            launch.arrivals == arrivals
                && now.saturating_duration_since(launch.at) < LAUNCH_HANDSHAKE_WINDOW
        });
        if pending {
            return false;
        }
        launches.insert(installed.into(), PendingLaunch { at: now, arrivals });
        true
    }

    /// Free the launch guard of `installed` after a start that began no process.
    fn release_launch(&self, installed: &str) {
        lock(&self.launches).remove(installed);
    }

    /// A connected browser identity that reported the installed browser `installed`.
    fn connected_as(&self, installed: &str) -> Option<String> {
        self.browser
            .browsers()
            .into_iter()
            .map(|connected| connected.id)
            .find(|id| self.browser.installed_browser_of(id).as_deref() == Some(installed))
    }
}

/// Plan recovery from the installed `candidates` under `mode`, given the native-host id of the
/// browser that last connected, when one is `remembered`.
fn decide(
    mode: BrowserStartup,
    candidates: &[RecoveryCandidate],
    remembered: Option<&str>,
) -> FlightPhase {
    let installed: Vec<_> = candidates
        .iter()
        .filter(|candidate| startable(candidate))
        .cloned()
        .collect();
    if installed.len() > 1 {
        // Several installed browsers: under `on_demand`, the one that last connected is the
        // browser this person uses with Ghostlight, so it starts exactly as a unique candidate
        // would, after repairing its own stale registration (ADR-0149 amendment, 2026-10-17).
        let last = remembered.and_then(|id| installed.iter().find(|browser| browser.id == id));
        if let Some(browser) = last.filter(|_| mode == BrowserStartup::OnDemand) {
            let repair_owned_registration = match browser.registration {
                NativeHostState::Current => Some(false),
                NativeHostState::Updatable => Some(true),
                NativeHostState::Missing
                | NativeHostState::OwnedElsewhere
                | NativeHostState::NeedsAttention => None,
            };
            if let Some(repair_owned_registration) = repair_owned_registration {
                return if repair_owned_registration {
                    FlightPhase::Repairing {
                        browser: browser.clone(),
                        launch_after: true,
                    }
                } else {
                    FlightPhase::Launching {
                        browser: browser.clone(),
                    }
                };
            }
        }
        // Otherwise plural evidence never chooses and never says it declined to. Name every
        // connectable browser and leave startup to the person; repair only what Ghostlight
        // already owns so the named browsers can actually connect when opened.
        let connectable: Vec<_> = installed
            .iter()
            .filter(|browser| browser.registration == NativeHostState::Current)
            .cloned()
            .collect();
        if !connectable.is_empty() {
            return FlightPhase::Complete(RecoveryDecision::Manual {
                browsers: connectable,
            });
        }
        let repairable: Vec<_> = installed
            .iter()
            .filter(|browser| browser.registration == NativeHostState::Updatable)
            .cloned()
            .collect();
        if !repairable.is_empty() {
            return FlightPhase::RepairingQueue {
                remaining: repairable.into(),
                named: Vec::new(),
            };
        }
        // A registration another installation owns explains the whole machine at once, so it
        // wins over the generic unusable-registration remedy (ADR-0149 amendment).
        let elsewhere: Vec<_> = installed
            .iter()
            .filter(|browser| browser.registration == NativeHostState::OwnedElsewhere)
            .map(|browser| browser.name.clone())
            .collect();
        if !elsewhere.is_empty() {
            return FlightPhase::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::OwnedElsewhere,
                details: elsewhere,
            });
        }
        return FlightPhase::Complete(RecoveryDecision::Failed {
            reason: RecoveryFailure::NativeHostUnavailable,
            details: installed
                .iter()
                .map(|browser| browser.name.clone())
                .collect(),
        });
    }
    let Some(browser) = installed.into_iter().next() else {
        let sandboxed: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.package.sandboxed())
            .map(|candidate| candidate.package_detail.clone())
            .collect();
        return FlightPhase::Complete(RecoveryDecision::Failed {
            reason: if sandboxed.is_empty() {
                RecoveryFailure::BrowserAbsent
            } else {
                RecoveryFailure::SandboxedPackage
            },
            details: sandboxed,
        });
    };

    start_plan(mode, browser)
}

/// Plan starting the installed browser `installed` and no other, under `on_demand`. A browser
/// Ghostlight cannot start, because it is not installed or has no verified ordinary executable,
/// leaves startup to the person.
fn decide_browser(candidates: &[RecoveryCandidate], installed: &str) -> FlightPhase {
    candidates
        .iter()
        .find(|candidate| candidate.id == installed && startable(candidate))
        .map_or_else(
            || {
                FlightPhase::Complete(RecoveryDecision::Manual {
                    browsers: Vec::new(),
                })
            },
            |browser| start_plan(BrowserStartup::OnDemand, browser.clone()),
        )
}

/// Whether Ghostlight could start `candidate`: an ordinary executable it verified, from a package
/// that can start a native messaging host.
fn startable(candidate: &RecoveryCandidate) -> bool {
    candidate.ordinary_executable.is_some() && candidate.package.native_messaging_usable()
}

/// Plan starting the one selected `browser` under `mode`, by its registration.
fn start_plan(mode: BrowserStartup, browser: RecoveryCandidate) -> FlightPhase {
    match browser.registration {
        NativeHostState::Missing | NativeHostState::NeedsAttention => {
            FlightPhase::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::NativeHostUnavailable,
                details: vec![browser.name],
            })
        }
        NativeHostState::OwnedElsewhere => FlightPhase::Complete(RecoveryDecision::Failed {
            reason: RecoveryFailure::OwnedElsewhere,
            details: vec![browser.name],
        }),
        NativeHostState::Updatable => FlightPhase::Repairing {
            browser,
            launch_after: mode == BrowserStartup::OnDemand,
        },
        NativeHostState::Current if mode == BrowserStartup::Manual => {
            FlightPhase::Complete(RecoveryDecision::Manual {
                browsers: vec![browser],
            })
        }
        NativeHostState::Current => FlightPhase::Launching { browser },
    }
}

/// The answer when the installed browsers cannot be read: the native host is not usable.
fn unreadable_inventory() -> FlightPhase {
    FlightPhase::Complete(RecoveryDecision::Failed {
        reason: RecoveryFailure::NativeHostUnavailable,
        details: Vec::new(),
    })
}

/// The launcher a discovered recovery service holds. A test build holds one that starts no
/// process, so no test can open a browser on the machine that runs it.
fn discovered_mechanism() -> Arc<dyn RecoveryMechanism> {
    #[cfg(not(test))]
    let mechanism = Arc::new(SystemRecoveryMechanism);
    #[cfg(test)]
    let mechanism = Arc::new(TestLaunch(Box::new(|| {})));
    mechanism
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::thread;

    use super::*;
    use crate::browser::testing::{summary, FakeBrowser};

    #[derive(Debug)]
    struct FakeInventory {
        candidates: Vec<RecoveryCandidate>,
        inspections: AtomicUsize,
        entered: Option<mpsc::Sender<()>>,
        release: Mutex<Option<mpsc::Receiver<()>>>,
    }

    impl BrowserInventory for FakeInventory {
        fn inspect(&self) -> Result<Vec<RecoveryCandidate>, ()> {
            self.inspections.fetch_add(1, Ordering::SeqCst);
            if let Some(entered) = &self.entered {
                let _ = entered.send(());
            }
            if let Some(release) = lock(&self.release).take() {
                let _ = release.recv();
            }
            Ok(self.candidates.clone())
        }
    }

    #[derive(Debug)]
    struct FakeMechanism {
        repairs: AtomicUsize,
        launches: AtomicUsize,
        outcome: Mutex<Result<(), MechanismError>>,
        connect: Option<Arc<FakeBrowser>>,
    }

    impl RecoveryMechanism for FakeMechanism {
        fn repair(
            &self,
            _browser: &RecoveryCandidate,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), MechanismError> {
            ensure_live(deadline, cancelled).map_err(MechanismError::Wait)?;
            self.repairs.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn launch(
            &self,
            _browser: &RecoveryCandidate,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), MechanismError> {
            ensure_live(deadline, cancelled).map_err(MechanismError::Wait)?;
            self.launches.fetch_add(1, Ordering::SeqCst);
            let outcome = lock(&self.outcome).clone();
            if outcome.is_ok() {
                if let Some(browser) = &self.connect {
                    browser.connect(vec![summary("browser_chromium", true)]);
                }
            }
            outcome
        }
    }

    fn mechanism(
        outcome: Result<(), MechanismFailure>,
        connect: Option<Arc<FakeBrowser>>,
    ) -> Arc<FakeMechanism> {
        Arc::new(FakeMechanism {
            repairs: AtomicUsize::new(0),
            launches: AtomicUsize::new(0),
            outcome: Mutex::new(outcome.map_err(MechanismError::Failed)),
            connect,
        })
    }

    fn candidate(
        name: &str,
        package: BrowserPackage,
        registration: NativeHostState,
    ) -> RecoveryCandidate {
        RecoveryCandidate {
            id: name.to_ascii_lowercase().replace(' ', "-"),
            name: name.into(),
            package,
            package_detail: crate::install::browser_package::detail(name, package),
            registration,
            ordinary_executable: package
                .native_messaging_usable()
                .then(|| PathBuf::from(format!("{name}-browser"))),
        }
    }

    fn candidate_without_executable(
        name: &str,
        package: BrowserPackage,
        registration: NativeHostState,
    ) -> RecoveryCandidate {
        RecoveryCandidate {
            ordinary_executable: None,
            ..candidate(name, package, registration)
        }
    }

    fn coordinator(policy: Option<&str>, inventory: Arc<FakeInventory>) -> BrowserRecovery {
        let governance = if let Some(value) = policy {
            let path = crate::scratch::Scratch::file("recovery-policy.json");
            std::fs::write(
                &path,
                format!(
                    r#"{{"schema":4,"name":"test","version":"1","grants":[],"config":[{{"key":"browser.startup","value":"{value}","level":"mandatory"}}]}}"#
                ),
            )
            .unwrap();
            // The facade reads the file once here; nothing in these tests refreshes it.
            GovernanceFacade::new(Some(path.to_path_buf()))
        } else {
            GovernanceFacade::new(None)
        };
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        BrowserRecovery {
            governance,
            inventory,
            browser,
            mechanism: mechanism(
                Err((RecoveryFailure::LaunchFailed, "fake launch refused".into())),
                None,
            ),
            flights: Arc::new(Mutex::new(Flights::default())),
            launches: Arc::new(Mutex::new(HashMap::new())),
            clock: Arc::new(Instant::now),
        }
    }

    /// An inventory over `candidates` that answers at once.
    fn inventory(candidates: Vec<RecoveryCandidate>) -> Arc<FakeInventory> {
        Arc::new(FakeInventory {
            candidates,
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        })
    }

    fn wait_for_participants(recovery: &BrowserRecovery, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let participants = {
                let flights = lock(&recovery.flights);
                flights
                    .scopes
                    .get(&FlightScope::Unbound)
                    .map(|flight| lock(&flight.state).participants)
                    .unwrap_or(0)
            };
            if participants == expected {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {expected} recovery participants; found {participants}"
            );
            thread::yield_now();
        }
    }

    /// A recovery on demand over one installed Chromium whose inventory read tells `entered` it
    /// began and then waits for `release`, and whose launch connects that browser.
    fn held_flight() -> (
        BrowserRecovery,
        Arc<FakeInventory>,
        Arc<FakeMechanism>,
        mpsc::Receiver<()>,
        mpsc::Sender<()>,
    ) {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let inventory = Arc::new(FakeInventory {
            candidates: vec![candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Current,
            )],
            inspections: AtomicUsize::new(0),
            entered: Some(entered_tx),
            release: Mutex::new(Some(release_rx)),
        });
        let mut recovery = coordinator(Some("on_demand"), Arc::clone(&inventory));
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        let mechanism = mechanism(Ok(()), Some(Arc::clone(&browser)));
        recovery.browser = browser;
        recovery.mechanism = Arc::clone(&mechanism) as Arc<dyn RecoveryMechanism>;
        (recovery, inventory, mechanism, entered_rx, release_tx)
    }

    /// ACCEPTANCE C19 ("a session with no browser, a bound session and a retry asking meanwhile
    /// all wait for that one start"): requests that overlap join one flight, which reads the
    /// installed browsers once and starts the browser once. Each participant's wait ends only by
    /// its own cancellation or deadline: a flight's creator that is cancelled, or whose deadline
    /// passes, while it reads the inventory leaves a joiner to finish the start and find the
    /// browser ready, and no flight is left behind.
    #[test]
    fn overlapping_requests_share_one_start_and_each_ends_only_its_own_wait() {
        let patient = || Instant::now() + Duration::from_secs(5);

        let (recovery, inventory, mechanism, entered, release) = held_flight();
        let first = {
            let recovery = recovery.clone();
            thread::spawn(move || recovery.request(patient(), &AtomicBool::new(false)))
        };
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = {
            let recovery = recovery.clone();
            thread::spawn(move || recovery.request(patient(), &AtomicBool::new(false)))
        };
        wait_for_participants(&recovery, 2);
        release.send(()).unwrap();
        let first = first.join().unwrap();
        assert!(
            matches!(first, Ok(RecoveryDecision::Ready { .. })),
            "{first:?}"
        );
        assert_eq!(first, second.join().unwrap());
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
        assert!(lock(&recovery.flights).scopes.is_empty());

        let (recovery, inventory, mechanism, entered, release) = held_flight();
        let cancelled = Arc::new(AtomicBool::new(false));
        let creator = {
            let recovery = recovery.clone();
            let cancelled = Arc::clone(&cancelled);
            thread::spawn(move || recovery.request(patient(), &cancelled))
        };
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        let joiner = {
            let recovery = recovery.clone();
            thread::spawn(move || recovery.request(patient(), &AtomicBool::new(false)))
        };
        wait_for_participants(&recovery, 2);
        cancelled.store(true, Ordering::SeqCst);
        release.send(()).unwrap();
        assert_eq!(creator.join().unwrap(), Err(RecoveryWaitError::Cancelled));
        assert!(matches!(
            joiner.join().unwrap(),
            Ok(RecoveryDecision::Ready { .. })
        ));
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
        assert!(lock(&recovery.flights).scopes.is_empty());

        let (recovery, inventory, mechanism, entered, release) = held_flight();
        let creator_deadline = Instant::now() + Duration::from_millis(500);
        let creator = {
            let recovery = recovery.clone();
            thread::spawn(move || recovery.request(creator_deadline, &AtomicBool::new(false)))
        };
        entered
            .recv_timeout(Duration::from_secs(5))
            .expect("the creator reads the inventory before its deadline");
        let joiner = {
            let recovery = recovery.clone();
            thread::spawn(move || recovery.request(patient(), &AtomicBool::new(false)))
        };
        wait_for_participants(&recovery, 2);
        // The creator's deadline passes while it still reads the inventory.
        while Instant::now() < creator_deadline {
            thread::sleep(creator_deadline.saturating_duration_since(Instant::now()));
        }
        release.send(()).unwrap();
        assert_eq!(creator.join().unwrap(), Err(RecoveryWaitError::Deadline));
        assert!(matches!(
            joiner.join().unwrap(),
            Ok(RecoveryDecision::Ready { .. })
        ));
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
        assert!(lock(&recovery.flights).scopes.is_empty());
    }

    /// An installed native browser under the native-host id the inventory reports for it.
    fn installed(id: &str, name: &str, registration: NativeHostState) -> RecoveryCandidate {
        RecoveryCandidate {
            id: id.into(),
            ..candidate(name, BrowserPackage::Native, registration)
        }
    }

    /// Chrome in `chrome`'s registration state beside a connectable Edge.
    fn chrome_and_edge(chrome: NativeHostState) -> Vec<RecoveryCandidate> {
        vec![
            installed("chrome", "Google Chrome", chrome),
            installed("edge", "Microsoft Edge", NativeHostState::Current),
        ]
    }

    /// The answer that leaves startup to the person, naming `browsers`.
    fn left_to_the_person(browsers: &[RecoveryCandidate]) -> FlightPhase {
        FlightPhase::Complete(RecoveryDecision::Manual {
            browsers: browsers.to_vec(),
        })
    }

    /// The answer that names one remedy, with `details` naming what it is about.
    fn remedy(reason: RecoveryFailure, details: &[&str]) -> FlightPhase {
        FlightPhase::Complete(RecoveryDecision::Failed {
            reason,
            details: details.iter().map(|detail| (*detail).to_owned()).collect(),
        })
    }

    /// ACCEPTANCE B21, B22 and B23 (ADR-0149 and its amendment of 2026-10-17): the decision for a
    /// session that works in no browser yet never presents a choice. Under `on_demand` it starts
    /// the one browser Ghostlight can start, or among several the one that last connected,
    /// repairing that browser's own stale registration first. Otherwise it names every
    /// connectable browser and leaves startup to the person, and when nothing is usable it names
    /// one remedy, a registration another installation owns explaining the whole machine.
    /// `manual` never starts a browser.
    #[test]
    fn the_unbound_decision_never_presents_a_choice() {
        use BrowserStartup::{Manual, OnDemand};
        use NativeHostState::{Current, Missing, NeedsAttention, OwnedElsewhere, Updatable};

        let windows = vec![
            candidate("Google Chrome", BrowserPackage::NotChecked, Current),
            candidate_without_executable("Microsoft Edge", BrowserPackage::NotChecked, Current),
            candidate_without_executable("Brave", BrowserPackage::NotChecked, Current),
            candidate_without_executable("Chromium", BrowserPackage::NotChecked, Current),
        ];
        let stale = vec![candidate("Chromium", BrowserPackage::Native, Updatable)];
        let no_executable = vec![candidate_without_executable(
            "Microsoft Edge",
            BrowserPackage::NotChecked,
            Current,
        )];
        let verified = vec![
            candidate("Google Chrome", BrowserPackage::NotChecked, Current),
            candidate("Microsoft Edge", BrowserPackage::NotChecked, Current),
        ];
        let native = vec![
            candidate("Google Chrome", BrowserPackage::Native, Current),
            candidate("Microsoft Edge", BrowserPackage::Native, Current),
        ];
        let missing = vec![candidate(
            "Microsoft Edge",
            BrowserPackage::NotChecked,
            Missing,
        )];
        let snap = vec![candidate("Chromium", BrowserPackage::Snap, Current)];
        let unregistered = vec![
            candidate("Google Chrome", BrowserPackage::Native, Missing),
            candidate("Chromium", BrowserPackage::Native, Missing),
        ];
        let elsewhere = vec![candidate(
            "Google Chrome",
            BrowserPackage::Native,
            OwnedElsewhere,
        )];
        let mixed = vec![
            candidate("Google Chrome", BrowserPackage::Native, Current),
            candidate("Microsoft Edge", BrowserPackage::Native, OwnedElsewhere),
        ];
        let all_elsewhere = vec![
            candidate("Google Chrome", BrowserPackage::Native, OwnedElsewhere),
            candidate("Chromium", BrowserPackage::Native, OwnedElsewhere),
        ];
        let one_connectable = vec![
            candidate("Google Chrome", BrowserPackage::Native, Missing),
            candidate("Microsoft Edge", BrowserPackage::Native, Current),
        ];
        let plural = chrome_and_edge(Current);
        let remembered_stale = chrome_and_edge(Updatable);
        let mut uninstalled = chrome_and_edge(Current);
        uninstalled[0].ordinary_executable = None;
        uninstalled.push(installed("brave", "Brave", Current));

        let mut cases = vec![
            (
                "the one browser with a verified executable starts",
                OnDemand,
                windows.clone(),
                None,
                FlightPhase::Launching {
                    browser: windows[0].clone(),
                },
            ),
            (
                "the one browser's own stale registration is repaired, then it starts",
                OnDemand,
                stale.clone(),
                None,
                FlightPhase::Repairing {
                    browser: stale[0].clone(),
                    launch_after: true,
                },
            ),
            (
                "a registration without an executable is no candidate",
                OnDemand,
                no_executable,
                None,
                remedy(RecoveryFailure::BrowserAbsent, &[]),
            ),
            (
                "two verified browsers and no memory: the person starts one",
                OnDemand,
                verified.clone(),
                None,
                left_to_the_person(&verified),
            ),
            (
                "manual names every current browser",
                Manual,
                native.clone(),
                None,
                left_to_the_person(&native),
            ),
            (
                "manual diagnoses a missing registration before asking for startup",
                Manual,
                missing,
                None,
                remedy(RecoveryFailure::NativeHostUnavailable, &["Microsoft Edge"]),
            ),
            (
                "a sandboxed package is diagnosed, never started",
                OnDemand,
                snap.clone(),
                None,
                FlightPhase::Complete(RecoveryDecision::Failed {
                    reason: RecoveryFailure::SandboxedPackage,
                    details: vec![snap[0].package_detail.clone()],
                }),
            ),
            (
                "browsers without a usable registration name the remedy",
                Manual,
                unregistered,
                None,
                remedy(
                    RecoveryFailure::NativeHostUnavailable,
                    &["Google Chrome", "Chromium"],
                ),
            ),
            (
                "a registration owned elsewhere names the deliberate remedy",
                OnDemand,
                elsewhere,
                None,
                remedy(RecoveryFailure::OwnedElsewhere, &["Google Chrome"]),
            ),
            (
                "mixed ownership asks only for the current browsers",
                Manual,
                mixed.clone(),
                None,
                left_to_the_person(&mixed[..1]),
            ),
            (
                "every registration owned elsewhere explains the whole machine",
                OnDemand,
                all_elsewhere,
                None,
                remedy(
                    RecoveryFailure::OwnedElsewhere,
                    &["Google Chrome", "Chromium"],
                ),
            ),
            (
                "plural evidence names only the connectable browsers",
                OnDemand,
                one_connectable.clone(),
                None,
                left_to_the_person(&one_connectable[1..]),
            ),
            (
                "the browser that last connected starts: Chrome",
                OnDemand,
                plural.clone(),
                Some("chrome"),
                FlightPhase::Launching {
                    browser: plural[0].clone(),
                },
            ),
            (
                "the browser that last connected starts: Edge",
                OnDemand,
                plural.clone(),
                Some("edge"),
                FlightPhase::Launching {
                    browser: plural[1].clone(),
                },
            ),
            (
                "the remembered browser's stale registration is repaired, then it starts",
                OnDemand,
                remembered_stale.clone(),
                Some("chrome"),
                FlightPhase::Repairing {
                    browser: remembered_stale[0].clone(),
                    launch_after: true,
                },
            ),
            (
                "a remembered browser whose executable is gone is no longer installed",
                OnDemand,
                uninstalled.clone(),
                Some("chrome"),
                left_to_the_person(&uninstalled[1..]),
            ),
            (
                "manual with a memory still leaves startup to the person",
                Manual,
                plural.clone(),
                Some("chrome"),
                left_to_the_person(&plural),
            ),
        ];
        for remembered in [None, Some("brave"), Some("not-a-browser")] {
            cases.push((
                "a memory that names no installed browser asks for every one",
                OnDemand,
                plural.clone(),
                remembered,
                left_to_the_person(&plural),
            ));
        }
        for unusable in [Missing, OwnedElsewhere, NeedsAttention] {
            let candidates = chrome_and_edge(unusable);
            let expected = left_to_the_person(&candidates[1..]);
            cases.push((
                "a remembered browser Ghostlight cannot repair asks for the rest",
                OnDemand,
                candidates,
                Some("chrome"),
                expected,
            ));
        }
        for (case, mode, candidates, remembered, expected) in cases {
            assert_eq!(
                decide(mode, &candidates, remembered),
                expected,
                "{case}, remembering {remembered:?}"
            );
        }
    }

    /// ACCEPTANCE B21 and B22: a flight carries out its decision and nothing more. It repairs
    /// only the stale registrations Ghostlight owns, without asking, and starts a browser only
    /// under `on_demand` with one browser selected: `manual` reads the installed browsers once,
    /// repairs what it owns and starts nothing; several stale browsers are repaired and left to
    /// the person; and the remembered one is repaired, started and found ready.
    #[test]
    fn a_flight_repairs_silently_and_launches_only_under_on_demand() {
        let patient = || Instant::now() + Duration::from_secs(5);
        let names = |decision: &RecoveryDecision| match decision {
            RecoveryDecision::Manual { browsers } => browsers
                .iter()
                .map(|browser| browser.name.clone())
                .collect::<Vec<_>>(),
            other => panic!("expected startup left to the person, got {other:?}"),
        };
        let flight = |policy: &str, candidates| {
            let inventory = inventory(candidates);
            let mut recovery = coordinator(Some(policy), Arc::clone(&inventory));
            let mechanism = mechanism(Ok(()), None);
            recovery.mechanism = Arc::clone(&mechanism) as Arc<dyn RecoveryMechanism>;
            let decision = recovery
                .request(patient(), &AtomicBool::new(false))
                .unwrap();
            (decision, inventory, mechanism)
        };

        let (decision, read, launcher) = flight(
            "manual",
            vec![candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Current,
            )],
        );
        assert_eq!(names(&decision), ["Chromium"]);
        assert_eq!(read.inspections.load(Ordering::SeqCst), 1);
        assert_eq!(launcher.repairs.load(Ordering::SeqCst), 0);
        assert_eq!(launcher.launches.load(Ordering::SeqCst), 0);

        let (decision, _, launcher) = flight(
            "manual",
            vec![candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Updatable,
            )],
        );
        assert_eq!(names(&decision), ["Chromium"]);
        assert_eq!(launcher.repairs.load(Ordering::SeqCst), 1);
        assert_eq!(launcher.launches.load(Ordering::SeqCst), 0);

        let (decision, _, launcher) = flight(
            "on_demand",
            vec![
                candidate(
                    "Google Chrome",
                    BrowserPackage::Native,
                    NativeHostState::Updatable,
                ),
                candidate(
                    "Microsoft Edge",
                    BrowserPackage::Native,
                    NativeHostState::Updatable,
                ),
            ],
        );
        assert_eq!(names(&decision), ["Google Chrome", "Microsoft Edge"]);
        assert_eq!(launcher.repairs.load(Ordering::SeqCst), 2);
        assert_eq!(launcher.launches.load(Ordering::SeqCst), 0);

        let mut recovery = coordinator(
            Some("on_demand"),
            inventory(chrome_and_edge(NativeHostState::Updatable)),
        );
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_last_connected("chrome");
        let launcher = mechanism(Ok(()), Some(Arc::clone(&browser)));
        recovery.browser = browser;
        recovery.mechanism = Arc::clone(&launcher) as Arc<dyn RecoveryMechanism>;
        assert_eq!(
            recovery
                .request(patient(), &AtomicBool::new(false))
                .unwrap(),
            RecoveryDecision::Ready {
                browser: "browser_chromium".into(),
            }
        );
        assert_eq!(launcher.repairs.load(Ordering::SeqCst), 1);
        assert_eq!(launcher.launches.load(Ordering::SeqCst), 1);
    }

    /// A coordinator over `candidates` under `policy`, whose port remembers `last` as the browser
    /// that last connected.
    fn answering(
        policy: &str,
        candidates: Vec<RecoveryCandidate>,
        last: Option<&str>,
    ) -> (BrowserRecovery, Arc<FakeInventory>) {
        let inventory = inventory(candidates);
        let mut recovery = coordinator(Some(policy), Arc::clone(&inventory));
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        if let Some(last) = last {
            browser.remember_last_connected(last);
        }
        recovery.browser = browser;
        (recovery, inventory)
    }

    /// The launcher of a service inside an app container: it starts nothing.
    struct Contained;

    impl RecoveryMechanism for Contained {
        fn repair(
            &self,
            _browser: &RecoveryCandidate,
            _deadline: Instant,
            _cancelled: &AtomicBool,
        ) -> Result<(), MechanismError> {
            Ok(())
        }

        fn launch(
            &self,
            _browser: &RecoveryCandidate,
            _deadline: Instant,
            _cancelled: &AtomicBool,
        ) -> Result<(), MechanismError> {
            Err(MechanismError::LeftToThePerson)
        }

        fn leaves_startup_to_the_person(&self) -> bool {
            true
        }
    }

    /// A recovery for the installed browser `named` under `posture`, over Chrome and Edge in
    /// `chrome`'s registration state, with `browser` as its port and `mechanism` as its launcher.
    fn named_recovery(
        posture: &str,
        chrome: NativeHostState,
        browser: &Arc<FakeBrowser>,
        mechanism: &Arc<FakeMechanism>,
    ) -> (BrowserRecovery, Arc<FakeInventory>) {
        let inventory = inventory(chrome_and_edge(chrome));
        let mut recovery = coordinator(Some(posture), Arc::clone(&inventory));
        recovery.browser = Arc::clone(browser) as Arc<dyn BrowserPort>;
        recovery.mechanism = Arc::clone(mechanism) as Arc<dyn RecoveryMechanism>;
        (recovery, inventory)
    }

    fn request_chrome(recovery: &BrowserRecovery, wait: Duration) -> RecoveryDecision {
        recovery
            .request_browser("chrome", Instant::now() + wait, &AtomicBool::new(false))
            .unwrap()
    }

    /// Time `recovery`'s launch guard by a clock the test moves by hand.
    fn hand_clock(recovery: &mut BrowserRecovery) -> Arc<Mutex<Instant>> {
        let now = Arc::new(Mutex::new(Instant::now()));
        let read = Arc::clone(&now);
        recovery.clock = Arc::new(move || *lock(&read));
        now
    }

    /// Wait until the flight for `scope` waits for a started browser to connect.
    fn wait_for_launched(recovery: &BrowserRecovery, scope: &FlightScope) {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let launched = lock(&recovery.flights)
                .scopes
                .get(scope)
                .is_some_and(|flight| {
                    matches!(lock(&flight.state).phase, FlightPhase::Launched { .. })
                });
            if launched {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the {scope:?} flight never waited for a started browser"
            );
            thread::yield_now();
        }
    }

    /// Ask for Chrome, wait until the flight waits for the browser it started, then give up the
    /// call. Nothing connects, so the start stays the last one Ghostlight made.
    fn start_chrome_and_give_up(recovery: &BrowserRecovery) {
        let cancelled = Arc::new(AtomicBool::new(false));
        let request = {
            let recovery = recovery.clone();
            let cancelled = Arc::clone(&cancelled);
            thread::spawn(move || {
                recovery.request_browser(
                    "chrome",
                    Instant::now() + Duration::from_secs(5),
                    &cancelled,
                )
            })
        };
        wait_for_launched(recovery, &FlightScope::Browser("chrome".into()));
        cancelled.store(true, Ordering::SeqCst);
        assert_eq!(request.join().unwrap(), Err(RecoveryWaitError::Cancelled));
    }

    /// Defect d81114a0 ("never open a second window of a browser being started") and ACCEPTANCE
    /// C19: a start that has not connected is never repeated within LAUNCH_HANDSHAKE_WINDOW. A
    /// session with no browser and a session whose closed browser it is start it once together,
    /// and a retry inside the window waits for that start instead of starting it again. A start
    /// whose browser connected, a window that has passed, and a start that began no process all
    /// free the guard.
    #[test]
    fn a_browser_start_is_never_repeated_while_it_may_still_connect() {
        let ready = |browser: &str| RecoveryDecision::Ready {
            browser: browser.into(),
        };

        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chrome", "chrome");
        browser.remember_last_connected("chrome");
        let launcher = mechanism(Ok(()), None);
        let (recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &launcher);
        let unbound = {
            let recovery = recovery.clone();
            thread::spawn(move || {
                recovery.request(
                    Instant::now() + Duration::from_secs(5),
                    &AtomicBool::new(false),
                )
            })
        };
        let named = {
            let recovery = recovery.clone();
            thread::spawn(move || request_chrome(&recovery, Duration::from_secs(5)))
        };
        wait_for_launched(&recovery, &FlightScope::Unbound);
        wait_for_launched(&recovery, &FlightScope::Browser("chrome".into()));
        browser.connect(vec![summary("browser_chrome", true)]);
        assert_eq!(unbound.join().unwrap(), Ok(ready("browser_chrome")));
        assert_eq!(named.join().unwrap(), ready("browser_chrome"));
        assert_eq!(
            launcher.launches.load(Ordering::SeqCst),
            1,
            "both sessions wait for the one start"
        );

        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chrome", "chrome");
        let launcher = mechanism(Ok(()), None);
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &launcher);
        let now = hand_clock(&mut recovery);
        start_chrome_and_give_up(&recovery);
        *lock(&now) += Duration::from_secs(5);
        let retry = {
            let recovery = recovery.clone();
            thread::spawn(move || request_chrome(&recovery, Duration::from_secs(5)))
        };
        wait_for_launched(&recovery, &FlightScope::Browser("chrome".into()));
        assert_eq!(
            launcher.launches.load(Ordering::SeqCst),
            1,
            "a retry inside the window starts nothing"
        );
        browser.connect(vec![summary("browser_chrome", true)]);
        assert_eq!(retry.join().unwrap(), ready("browser_chrome"));
        assert_eq!(launcher.launches.load(Ordering::SeqCst), 1);

        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chromium", "chrome");
        let launcher = mechanism(Ok(()), Some(Arc::clone(&browser)));
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &launcher);
        let _still = hand_clock(&mut recovery);
        assert_eq!(
            request_chrome(&recovery, Duration::from_secs(5)),
            ready("browser_chromium")
        );
        browser.connect(Vec::new());
        assert_eq!(
            request_chrome(&recovery, Duration::from_secs(5)),
            ready("browser_chromium")
        );
        assert_eq!(
            launcher.launches.load(Ordering::SeqCst),
            2,
            "a start that connected frees the guard"
        );

        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chrome", "chrome");
        let launcher = mechanism(Ok(()), None);
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &launcher);
        let now = hand_clock(&mut recovery);
        start_chrome_and_give_up(&recovery);
        *lock(&now) += LAUNCH_HANDSHAKE_WINDOW;
        start_chrome_and_give_up(&recovery);
        assert_eq!(
            launcher.launches.load(Ordering::SeqCst),
            2,
            "after the window the browser is started again"
        );

        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        let launcher = mechanism(
            Err((RecoveryFailure::LaunchFailed, "fake launch refused".into())),
            None,
        );
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &launcher);
        let _still = hand_clock(&mut recovery);
        let refused = RecoveryDecision::Failed {
            reason: RecoveryFailure::LaunchFailed,
            details: vec!["fake launch refused".into()],
        };
        assert_eq!(request_chrome(&recovery, Duration::from_secs(5)), refused);
        assert_eq!(request_chrome(&recovery, Duration::from_secs(5)), refused);
        assert_eq!(
            launcher.launches.load(Ordering::SeqCst),
            2,
            "a start that began no process frees the guard"
        );
    }

    /// Launch attempts that reach the start of a browser, counted in place of a process.
    fn counted_launch(
        session_ending: bool,
        contained: bool,
        cancelled: &AtomicBool,
    ) -> (Result<(), MechanismError>, usize) {
        let starts = AtomicUsize::new(0);
        let answer = SystemRecoveryMechanism.launch_unless_ending(
            session_ending,
            contained,
            Instant::now() + Duration::from_secs(5),
            cancelled,
            || {
                starts.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        );
        (answer, starts.load(Ordering::SeqCst))
    }

    /// Defects cb37cdbf and c1deccb1 (ADR-0149 amendments "A service with no display" and "A
    /// service inside an app container"): where a browser the service started would not reach
    /// the person, nothing is started. A request of either kind leaves startup to the person,
    /// naming the browser it would have started, the launch guard stays free for the next try,
    /// and the window promises no start.
    #[test]
    fn a_browser_is_left_to_the_person_where_a_start_would_not_reach_them() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_last_connected("chrome");
        let launcher = mechanism(Ok(()), None);
        *lock(&launcher.outcome) = Err(MechanismError::LeftToThePerson);
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &launcher);
        let _still = hand_clock(&mut recovery);
        let left = RecoveryDecision::Manual {
            browsers: vec![chrome_and_edge(NativeHostState::Current).remove(0)],
        };
        assert_eq!(
            recovery
                .request(
                    Instant::now() + Duration::from_secs(5),
                    &AtomicBool::new(false),
                )
                .unwrap(),
            left
        );
        assert_eq!(request_chrome(&recovery, Duration::from_secs(5)), left);
        assert_eq!(
            launcher.launches.load(Ordering::SeqCst),
            2,
            "nothing was started, so nothing holds the guard against the next try"
        );

        assert_eq!(
            counted_launch(false, true, &AtomicBool::new(false)),
            (Err(MechanismError::LeftToThePerson), 0)
        );
        let (mut recovery, _) = answering(
            "on_demand",
            vec![installed(
                "chrome",
                "Google Chrome",
                NativeHostState::Current,
            )],
            Some("chrome"),
        );
        recovery.mechanism = Arc::new(Contained);
        assert_eq!(
            recovery.starts_on_request(),
            None,
            "a service inside an app container promises no browser start"
        );
    }

    /// ACCEPTANCE D1 (ADR-0199 Decision 13): while the session is ending no browser is started,
    /// and the call ends as cancelled; otherwise the start goes ahead.
    #[test]
    fn no_browser_is_launched_while_the_session_is_ending() {
        let cancelled = AtomicBool::new(false);
        assert_eq!(
            counted_launch(true, false, &cancelled),
            (Err(MechanismError::Wait(RecoveryWaitError::Cancelled)), 0)
        );
        assert_eq!(counted_launch(false, false, &cancelled), (Ok(()), 1));
    }

    /// Defect 7769fd97: a browser the service starts, including a start that only hands a window
    /// to a running browser and exits at once, is reaped and never lingers as the service's
    /// zombie.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_started_browser_that_exits_leaves_no_zombie() {
        let process_id = start_reaped(ordinary_browser_command(Path::new("/bin/true"))).unwrap();
        let stat = format!("/proc/{process_id}/stat");
        let deadline = Instant::now() + Duration::from_secs(5);
        while std::path::Path::new(&stat).exists() {
            assert!(
                Instant::now() < deadline,
                "{process_id} was never collected"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}
