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
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
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

impl RecoveryFailure {
    /// Every closed failure in stable order.
    pub const ALL: [Self; 7] = [
        Self::BrowserAbsent,
        Self::LaunchFailed,
        Self::SandboxedPackage,
        Self::ExtensionAbsent,
        Self::NativeHostUnavailable,
        Self::OwnedElsewhere,
        Self::HandshakeTimeout,
    ];

    /// Stable structured fact vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BrowserAbsent => "browser_absent",
            Self::LaunchFailed => "browser_launch_failed",
            Self::SandboxedPackage => "browser_sandboxed",
            Self::ExtensionAbsent => "browser_extension_absent",
            Self::NativeHostUnavailable => "native_host_unavailable",
            Self::OwnedElsewhere => "native_host_owned_elsewhere",
            Self::HandshakeTimeout => "browser_handshake_timeout",
        }
    }
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
    /// A later physical seam may make one bounded attempt for this exact browser.
    Launch {
        /// The uniquely selected installed browser.
        browser: RecoveryCandidate,
        /// Whether an owned stale registration may be reconciled first.
        repair_owned_registration: bool,
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
            browser,
            deadline,
            cancelled,
        )
    }

    fn leaves_startup_to_the_person(&self) -> bool {
        Residence::detect().container().is_some()
    }
}

impl SystemRecoveryMechanism {
    /// Launch `browser`, unless the session is ending: then nothing is started and the call ends
    /// as cancelled (ADR-0199 Decision 13). In a service `contained` in an app container nothing
    /// is started either, and startup is left to the person.
    fn launch_unless_ending(
        &self,
        session_ending: bool,
        contained: bool,
        browser: &RecoveryCandidate,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), MechanismError> {
        ensure_live(deadline, cancelled).map_err(MechanismError::Wait)?;
        if session_ending {
            return Err(MechanismError::Wait(RecoveryWaitError::Cancelled));
        }
        if contained {
            return Err(MechanismError::LeftToThePerson);
        }
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
enum RecoveryPlan {
    Complete(RecoveryDecision),
    Prepare {
        browser: RecoveryCandidate,
        repair_owned_registration: bool,
        launch: bool,
    },
    /// Repair every stale Ghostlight-owned registration, then leave startup to the person.
    RepairAll {
        queue: Vec<RecoveryCandidate>,
    },
}

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
            RecoveryPlan::Prepare {
                browser,
                launch: true,
                ..
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
                        phase_from_plan(decide(
                            self.governance.browser_startup(),
                            &candidates,
                            remembered.as_deref(),
                        ))
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
            |candidates| phase_from_plan(decide_browser(&candidates, installed)),
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
) -> RecoveryPlan {
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
                return RecoveryPlan::Prepare {
                    browser: browser.clone(),
                    repair_owned_registration,
                    launch: true,
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
            return RecoveryPlan::Complete(RecoveryDecision::Manual {
                browsers: connectable,
            });
        }
        let repairable: Vec<_> = installed
            .iter()
            .filter(|browser| browser.registration == NativeHostState::Updatable)
            .cloned()
            .collect();
        if !repairable.is_empty() {
            return RecoveryPlan::RepairAll { queue: repairable };
        }
        // A registration another installation owns explains the whole machine at once, so it
        // wins over the generic unusable-registration remedy (ADR-0149 amendment).
        let elsewhere: Vec<_> = installed
            .iter()
            .filter(|browser| browser.registration == NativeHostState::OwnedElsewhere)
            .map(|browser| browser.name.clone())
            .collect();
        if !elsewhere.is_empty() {
            return RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::OwnedElsewhere,
                details: elsewhere,
            });
        }
        return RecoveryPlan::Complete(RecoveryDecision::Failed {
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
        return RecoveryPlan::Complete(RecoveryDecision::Failed {
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
fn decide_browser(candidates: &[RecoveryCandidate], installed: &str) -> RecoveryPlan {
    candidates
        .iter()
        .find(|candidate| candidate.id == installed && startable(candidate))
        .map_or_else(
            || {
                RecoveryPlan::Complete(RecoveryDecision::Manual {
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
fn start_plan(mode: BrowserStartup, browser: RecoveryCandidate) -> RecoveryPlan {
    match browser.registration {
        NativeHostState::Missing | NativeHostState::NeedsAttention => {
            RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::NativeHostUnavailable,
                details: vec![browser.name],
            })
        }
        NativeHostState::OwnedElsewhere => RecoveryPlan::Complete(RecoveryDecision::Failed {
            reason: RecoveryFailure::OwnedElsewhere,
            details: vec![browser.name],
        }),
        NativeHostState::Updatable => RecoveryPlan::Prepare {
            browser,
            repair_owned_registration: true,
            launch: mode == BrowserStartup::OnDemand,
        },
        NativeHostState::Current if mode == BrowserStartup::Manual => {
            RecoveryPlan::Complete(RecoveryDecision::Manual {
                browsers: vec![browser],
            })
        }
        NativeHostState::Current => RecoveryPlan::Prepare {
            browser,
            repair_owned_registration: false,
            launch: true,
        },
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

fn phase_from_plan(plan: RecoveryPlan) -> FlightPhase {
    match plan {
        RecoveryPlan::Complete(decision) => FlightPhase::Complete(decision),
        RecoveryPlan::RepairAll { queue } => FlightPhase::RepairingQueue {
            remaining: queue.into(),
            named: Vec::new(),
        },
        RecoveryPlan::Prepare {
            browser,
            repair_owned_registration: true,
            launch,
        } => FlightPhase::Repairing {
            browser,
            launch_after: launch,
        },
        RecoveryPlan::Prepare {
            browser,
            repair_owned_registration: false,
            launch: true,
        } => FlightPhase::Launching { browser },
        RecoveryPlan::Prepare {
            browser,
            repair_owned_registration: false,
            launch: false,
        } => FlightPhase::Complete(RecoveryDecision::Manual {
            browsers: vec![browser],
        }),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
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
        /// Additional adapter ids that arrive inside the same launch wait.
        extra_arrivals: Vec<String>,
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
                    let mut arrivals = vec![summary("browser_chromium", true)];
                    arrivals.extend(self.extra_arrivals.iter().map(|id| summary(id, true)));
                    browser.connect(arrivals);
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
            extra_arrivals: Vec::new(),
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

    #[test]
    fn simultaneous_requests_produce_one_attempt() {
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
        recovery.mechanism = mechanism.clone();
        let first = recovery.clone();
        let first_thread = thread::spawn(move || {
            first.request(
                Instant::now() + Duration::from_secs(2),
                &AtomicBool::new(false),
            )
        });
        entered_rx.recv().unwrap();
        let second = recovery.clone();
        let second_thread = thread::spawn(move || {
            second.request(
                Instant::now() + Duration::from_secs(2),
                &AtomicBool::new(false),
            )
        });
        thread::sleep(Duration::from_millis(20));
        release_tx.send(()).unwrap();
        assert_eq!(first_thread.join().unwrap(), second_thread.join().unwrap());
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cancelling_the_flight_creator_does_not_cancel_a_joiner() {
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

        let creator_cancelled = Arc::new(AtomicBool::new(false));
        let creator = recovery.clone();
        let creator_token = Arc::clone(&creator_cancelled);
        let creator_thread = thread::spawn(move || {
            creator.request(Instant::now() + Duration::from_secs(2), &creator_token)
        });
        entered_rx.recv().unwrap();

        let joiner = recovery.clone();
        let joiner_thread = thread::spawn(move || {
            joiner.request(
                Instant::now() + Duration::from_secs(2),
                &AtomicBool::new(false),
            )
        });
        wait_for_participants(&recovery, 2);
        creator_cancelled.store(true, Ordering::SeqCst);
        release_tx.send(()).unwrap();

        assert_eq!(
            creator_thread.join().unwrap(),
            Err(RecoveryWaitError::Cancelled)
        );
        assert!(matches!(
            joiner_thread.join().unwrap(),
            Ok(RecoveryDecision::Ready { .. })
        ));
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
        assert!(lock(&recovery.flights).scopes.is_empty());
    }

    #[test]
    fn a_short_deadline_does_not_end_a_longer_joiner() {
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

        let creator = recovery.clone();
        let creator_thread = thread::spawn(move || {
            creator.request(
                Instant::now() + Duration::from_millis(50),
                &AtomicBool::new(false),
            )
        });
        entered_rx.recv().unwrap();

        let joiner = recovery.clone();
        let joiner_thread = thread::spawn(move || {
            joiner.request(
                Instant::now() + Duration::from_secs(2),
                &AtomicBool::new(false),
            )
        });
        wait_for_participants(&recovery, 2);
        thread::sleep(Duration::from_millis(75));
        release_tx.send(()).unwrap();

        assert_eq!(
            creator_thread.join().unwrap(),
            Err(RecoveryWaitError::Deadline)
        );
        assert!(matches!(
            joiner_thread.join().unwrap(),
            Ok(RecoveryDecision::Ready { .. })
        ));
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
        assert!(lock(&recovery.flights).scopes.is_empty());
    }

    #[test]
    fn cancellation_during_inventory_prevents_repair_and_launch() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let inventory = Arc::new(FakeInventory {
            candidates: vec![candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Updatable,
            )],
            inspections: AtomicUsize::new(0),
            entered: Some(entered_tx),
            release: Mutex::new(Some(release_rx)),
        });
        let mut recovery = coordinator(Some("on_demand"), inventory);
        let mechanism = mechanism(Ok(()), None);
        recovery.mechanism = Arc::clone(&mechanism) as Arc<dyn RecoveryMechanism>;
        let cancelled = Arc::new(AtomicBool::new(false));
        let active = recovery.clone();
        let active_cancelled = Arc::clone(&cancelled);
        let active_thread = thread::spawn(move || {
            active.request(Instant::now() + Duration::from_secs(2), &active_cancelled)
        });

        entered_rx.recv().unwrap();
        cancelled.store(true, Ordering::SeqCst);
        release_tx.send(()).unwrap();

        assert_eq!(
            active_thread.join().unwrap(),
            Err(RecoveryWaitError::Cancelled)
        );
        assert_eq!(mechanism.repairs.load(Ordering::SeqCst), 0);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 0);
        assert!(lock(&recovery.flights).scopes.is_empty());
    }

    #[test]
    fn a_completed_flight_is_not_replaced_before_its_joiners_leave() {
        let inventory = Arc::new(FakeInventory {
            candidates: Vec::new(),
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
        let recovery = coordinator(Some("manual"), inventory);
        let first = recovery.join_flight(FlightScope::Unbound);
        let joined = recovery.join_flight(FlightScope::Unbound);
        assert!(Arc::ptr_eq(&first, &joined));
        lock(&first.state).phase = FlightPhase::Complete(RecoveryDecision::Failed {
            reason: RecoveryFailure::BrowserAbsent,
            details: Vec::new(),
        });

        recovery.leave_flight(&first);
        let late_joiner = recovery.join_flight(FlightScope::Unbound);
        assert!(Arc::ptr_eq(&joined, &late_joiner));
        recovery.leave_flight(&joined);
        recovery.leave_flight(&late_joiner);
        assert!(lock(&recovery.flights).scopes.is_empty());

        let next_generation = recovery.join_flight(FlightScope::Unbound);
        assert!(!Arc::ptr_eq(&first, &next_generation));
        recovery.leave_flight(&next_generation);
    }

    #[test]
    fn manual_mode_never_launches_and_returns_one_useful_outcome() {
        let inventory = Arc::new(FakeInventory {
            candidates: vec![candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Current,
            )],
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
        let mut recovery = coordinator(Some("manual"), Arc::clone(&inventory));
        let mechanism = mechanism(Ok(()), None);
        recovery.mechanism = Arc::clone(&mechanism) as Arc<dyn RecoveryMechanism>;
        let decision = recovery
            .request(
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert!(matches!(
            decision,
            RecoveryDecision::Manual {
                browsers
            } if browsers.iter().map(|browser| browser.name.as_str()).eq(["Chromium"])
        ));
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.repairs.load(Ordering::SeqCst), 0);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn four_current_windows_registrations_with_one_executable_select_that_browser() {
        let decision = decide(
            BrowserStartup::OnDemand,
            &[
                candidate(
                    "Google Chrome",
                    BrowserPackage::NotChecked,
                    NativeHostState::Current,
                ),
                candidate_without_executable(
                    "Microsoft Edge",
                    BrowserPackage::NotChecked,
                    NativeHostState::Current,
                ),
                candidate_without_executable(
                    "Brave",
                    BrowserPackage::NotChecked,
                    NativeHostState::Current,
                ),
                candidate_without_executable(
                    "Chromium",
                    BrowserPackage::NotChecked,
                    NativeHostState::Current,
                ),
            ],
            None,
        );

        assert!(matches!(
            decision,
            RecoveryPlan::Prepare {
                browser: RecoveryCandidate { name, .. },
                repair_owned_registration: false,
                launch: true,
            } if name == "Google Chrome"
        ));
    }

    #[test]
    fn current_registration_without_an_executable_is_not_a_candidate() {
        let decision = decide(
            BrowserStartup::OnDemand,
            &[candidate_without_executable(
                "Microsoft Edge",
                BrowserPackage::NotChecked,
                NativeHostState::Current,
            )],
            None,
        );

        assert_eq!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::BrowserAbsent,
                details: Vec::new(),
            })
        );
    }

    #[test]
    fn two_verified_candidates_leave_startup_to_the_person() {
        let decision = decide(
            BrowserStartup::OnDemand,
            &[
                candidate(
                    "Google Chrome",
                    BrowserPackage::NotChecked,
                    NativeHostState::Current,
                ),
                candidate(
                    "Microsoft Edge",
                    BrowserPackage::NotChecked,
                    NativeHostState::Current,
                ),
            ],
            None,
        );

        assert!(matches!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Manual { browsers })
                if browsers.iter().map(|browser| browser.name.as_str())
                    .eq(["Google Chrome", "Microsoft Edge"])
        ));
    }

    #[test]
    fn manual_mode_names_every_current_registered_browser() {
        let decision = decide(
            BrowserStartup::Manual,
            &[
                candidate(
                    "Google Chrome",
                    BrowserPackage::Native,
                    NativeHostState::Current,
                ),
                candidate(
                    "Microsoft Edge",
                    BrowserPackage::Native,
                    NativeHostState::Current,
                ),
            ],
            None,
        );

        assert!(matches!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Manual { browsers })
                if browsers.iter().map(|browser| browser.name.as_str())
                    .eq(["Google Chrome", "Microsoft Edge"])
        ));
    }

    #[test]
    fn manual_diagnoses_missing_registration_before_asking_for_startup() {
        let decision = decide(
            BrowserStartup::Manual,
            &[candidate(
                "Microsoft Edge",
                BrowserPackage::NotChecked,
                NativeHostState::Missing,
            )],
            None,
        );

        assert_eq!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::NativeHostUnavailable,
                details: vec!["Microsoft Edge".into()],
            })
        );
    }

    #[test]
    fn manual_repairs_owned_stale_registration_without_launching() {
        let inventory = Arc::new(FakeInventory {
            candidates: vec![candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Updatable,
            )],
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
        let mut recovery = coordinator(Some("manual"), inventory);
        let mechanism = mechanism(Ok(()), None);
        recovery.mechanism = Arc::clone(&mechanism) as Arc<dyn RecoveryMechanism>;

        let decision = recovery
            .request(
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false),
            )
            .unwrap();

        assert!(matches!(decision, RecoveryDecision::Manual { .. }));
        assert_eq!(mechanism.repairs.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_sandboxed_browser_package_is_diagnosed_not_launched() {
        let decision = decide(
            BrowserStartup::OnDemand,
            &[candidate(
                "Chromium",
                BrowserPackage::Snap,
                NativeHostState::Current,
            )],
            None,
        );
        assert!(matches!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::SandboxedPackage,
                details
            }) if details[0].contains("Snap") && details[0].contains("native browser package")
        ));
    }

    #[test]
    fn installed_browsers_without_usable_registrations_name_the_remedy() {
        let decision = decide(
            BrowserStartup::Manual,
            &[
                candidate(
                    "Google Chrome",
                    BrowserPackage::Native,
                    NativeHostState::Missing,
                ),
                candidate("Chromium", BrowserPackage::Native, NativeHostState::Missing),
            ],
            None,
        );
        assert_eq!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::NativeHostUnavailable,
                details: vec!["Google Chrome".into(), "Chromium".into()],
            })
        );
    }

    #[test]
    fn a_registration_owned_elsewhere_names_the_deliberate_remedy() {
        let decision = decide(
            BrowserStartup::OnDemand,
            &[candidate(
                "Google Chrome",
                BrowserPackage::Native,
                NativeHostState::OwnedElsewhere,
            )],
            None,
        );
        assert_eq!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::OwnedElsewhere,
                details: vec!["Google Chrome".into()],
            })
        );
    }

    #[test]
    fn plural_ownership_mixed_states_ask_only_for_current_browsers() {
        let decision = decide(
            BrowserStartup::Manual,
            &[
                candidate(
                    "Google Chrome",
                    BrowserPackage::Native,
                    NativeHostState::Current,
                ),
                candidate(
                    "Microsoft Edge",
                    BrowserPackage::Native,
                    NativeHostState::OwnedElsewhere,
                ),
            ],
            None,
        );
        assert!(matches!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Manual { browsers })
                if browsers.iter().map(|browser| browser.name.as_str())
                    .eq(["Google Chrome"])
        ));
    }

    #[test]
    fn plural_ownership_all_elsewhere_explains_the_whole_machine() {
        let decision = decide(
            BrowserStartup::OnDemand,
            &[
                candidate(
                    "Google Chrome",
                    BrowserPackage::Native,
                    NativeHostState::OwnedElsewhere,
                ),
                candidate(
                    "Chromium",
                    BrowserPackage::Native,
                    NativeHostState::OwnedElsewhere,
                ),
            ],
            None,
        );
        assert_eq!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::OwnedElsewhere,
                details: vec!["Google Chrome".into(), "Chromium".into()],
            })
        );
    }

    #[test]
    fn plural_evidence_names_only_the_connectable_browsers() {
        let decision = decide(
            BrowserStartup::OnDemand,
            &[
                candidate(
                    "Google Chrome",
                    BrowserPackage::Native,
                    NativeHostState::Missing,
                ),
                candidate(
                    "Microsoft Edge",
                    BrowserPackage::Native,
                    NativeHostState::Current,
                ),
            ],
            None,
        );
        assert!(matches!(
            decision,
            RecoveryPlan::Complete(RecoveryDecision::Manual { browsers })
                if browsers.iter().map(|browser| browser.name.as_str())
                    .eq(["Microsoft Edge"])
        ));
    }

    #[test]
    fn plural_stale_registrations_are_repaired_then_startup_is_left_to_the_person() {
        let inventory = Arc::new(FakeInventory {
            candidates: vec![
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
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
        let mut recovery = coordinator(Some("on_demand"), inventory);
        let mechanism = mechanism(Ok(()), None);
        recovery.mechanism = Arc::clone(&mechanism) as Arc<dyn RecoveryMechanism>;

        let decision = recovery
            .request(
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false),
            )
            .unwrap();

        assert!(matches!(
            decision,
            RecoveryDecision::Manual { browsers }
                if browsers.iter().map(|browser| browser.name.as_str())
                    .eq(["Google Chrome", "Microsoft Edge"])
        ));
        assert_eq!(mechanism.repairs.load(Ordering::SeqCst), 2);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 0);
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

    /// A coordinator over `candidates` under `policy`, whose port remembers `last` as the browser
    /// that last connected.
    fn answering(
        policy: &str,
        candidates: Vec<RecoveryCandidate>,
        last: Option<&str>,
    ) -> (BrowserRecovery, Arc<FakeInventory>) {
        let inventory = Arc::new(FakeInventory {
            candidates,
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
        let mut recovery = coordinator(Some(policy), Arc::clone(&inventory));
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        if let Some(last) = last {
            browser.remember_last_connected(last);
        }
        recovery.browser = browser;
        (recovery, inventory)
    }

    #[test]
    fn a_browser_starts_on_request_only_under_on_demand_with_one_browser_selected() {
        let unique = vec![installed(
            "chrome",
            "Google Chrome",
            NativeHostState::Current,
        )];
        let (recovery, _) = answering("on_demand", unique.clone(), None);
        assert_eq!(
            recovery.starts_on_request().as_deref(),
            Some("Google Chrome")
        );

        let (recovery, _) = answering(
            "on_demand",
            chrome_and_edge(NativeHostState::Current),
            Some("edge"),
        );
        assert_eq!(
            recovery.starts_on_request().as_deref(),
            Some("Microsoft Edge"),
            "the browser that last connected"
        );

        let (recovery, _) = answering("on_demand", chrome_and_edge(NativeHostState::Current), None);
        assert_eq!(
            recovery.starts_on_request(),
            None,
            "several browsers and no memory: the person starts one"
        );

        let broken = vec![installed(
            "chrome",
            "Google Chrome",
            NativeHostState::OwnedElsewhere,
        )];
        let (recovery, _) = answering("on_demand", broken, None);
        assert_eq!(recovery.starts_on_request(), None);

        let (recovery, inventory) = answering("manual", unique.clone(), Some("chrome"));
        assert_eq!(recovery.starts_on_request(), None);
        assert_eq!(
            inventory.inspections.load(Ordering::SeqCst),
            0,
            "a manual posture reads no installed browser"
        );

        let (mut recovery, _) = answering("on_demand", unique, Some("chrome"));
        recovery.mechanism = Arc::new(Contained);
        assert_eq!(
            recovery.starts_on_request(),
            None,
            "a service inside an app container starts no browser"
        );
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

    #[test]
    fn plural_on_demand_launches_the_browser_that_last_connected() {
        let candidates = chrome_and_edge(NativeHostState::Current);
        assert_eq!(
            decide(BrowserStartup::OnDemand, &candidates, Some("chrome")),
            RecoveryPlan::Prepare {
                browser: candidates[0].clone(),
                repair_owned_registration: false,
                launch: true,
            }
        );
        assert_eq!(
            decide(BrowserStartup::OnDemand, &candidates, Some("edge")),
            RecoveryPlan::Prepare {
                browser: candidates[1].clone(),
                repair_owned_registration: false,
                launch: true,
            }
        );
    }

    #[test]
    fn a_remembered_stale_registration_is_repaired_then_that_browser_is_launched() {
        let candidates = chrome_and_edge(NativeHostState::Updatable);
        assert_eq!(
            decide(BrowserStartup::OnDemand, &candidates, Some("chrome")),
            RecoveryPlan::Prepare {
                browser: candidates[0].clone(),
                repair_owned_registration: true,
                launch: true,
            }
        );

        let inventory = Arc::new(FakeInventory {
            candidates,
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
        let mut recovery = coordinator(Some("on_demand"), inventory);
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_last_connected("chrome");
        let mechanism = mechanism(Ok(()), Some(Arc::clone(&browser)));
        recovery.browser = browser;
        recovery.mechanism = Arc::clone(&mechanism) as Arc<dyn RecoveryMechanism>;

        let decision = recovery
            .request(
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false),
            )
            .unwrap();

        assert_eq!(
            decision,
            RecoveryDecision::Ready {
                browser: "browser_chromium".into(),
            }
        );
        assert_eq!(mechanism.repairs.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn without_a_usable_remembered_browser_plural_evidence_still_names_every_browser() {
        let candidates = chrome_and_edge(NativeHostState::Current);
        let named_both = RecoveryPlan::Complete(RecoveryDecision::Manual {
            browsers: candidates.clone(),
        });
        for remembered in [None, Some("brave"), Some("not-a-browser")] {
            assert_eq!(
                decide(BrowserStartup::OnDemand, &candidates, remembered),
                named_both,
                "{remembered:?}"
            );
        }

        // A remembered browser whose executable is gone is no longer installed.
        let mut uninstalled = chrome_and_edge(NativeHostState::Current);
        uninstalled[0].ordinary_executable = None;
        uninstalled.push(installed("brave", "Brave", NativeHostState::Current));
        assert_eq!(
            decide(BrowserStartup::OnDemand, &uninstalled, Some("chrome")),
            RecoveryPlan::Complete(RecoveryDecision::Manual {
                browsers: uninstalled[1..].to_vec(),
            })
        );

        // A remembered browser Ghostlight cannot repair is not usable.
        for unusable in [
            NativeHostState::Missing,
            NativeHostState::OwnedElsewhere,
            NativeHostState::NeedsAttention,
        ] {
            let candidates = chrome_and_edge(unusable);
            assert_eq!(
                decide(BrowserStartup::OnDemand, &candidates, Some("chrome")),
                RecoveryPlan::Complete(RecoveryDecision::Manual {
                    browsers: vec![candidates[1].clone()],
                }),
                "{unusable:?}"
            );
        }
    }

    #[test]
    fn the_manual_posture_with_a_memory_still_leaves_startup_to_the_person() {
        let candidates = chrome_and_edge(NativeHostState::Current);
        assert_eq!(
            decide(BrowserStartup::Manual, &candidates, Some("chrome")),
            RecoveryPlan::Complete(RecoveryDecision::Manual {
                browsers: candidates.clone(),
            })
        );
    }

    /// A recovery for the installed browser `named` under `posture`, over Chrome and Edge in
    /// `chrome`'s registration state, with `browser` as its port and `mechanism` as its launcher.
    fn named_recovery(
        posture: &str,
        chrome: NativeHostState,
        browser: &Arc<FakeBrowser>,
        mechanism: &Arc<FakeMechanism>,
    ) -> (BrowserRecovery, Arc<FakeInventory>) {
        let inventory = Arc::new(FakeInventory {
            candidates: chrome_and_edge(chrome),
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
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

    #[test]
    fn a_named_browser_is_started_and_its_arriving_connection_is_ready() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chromium", "chrome");
        let mechanism = mechanism(Ok(()), Some(Arc::clone(&browser)));
        let (recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);

        assert_eq!(
            request_chrome(&recovery, Duration::from_secs(1)),
            RecoveryDecision::Ready {
                browser: "browser_chromium".into(),
            }
        );
        assert_eq!(mechanism.repairs.load(Ordering::SeqCst), 0);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
        assert!(lock(&recovery.flights).scopes.is_empty());
    }

    #[test]
    fn a_named_browser_with_a_stale_registration_is_repaired_then_started() {
        assert_eq!(
            decide_browser(&chrome_and_edge(NativeHostState::Updatable), "chrome"),
            RecoveryPlan::Prepare {
                browser: installed("chrome", "Google Chrome", NativeHostState::Updatable),
                repair_owned_registration: true,
                launch: true,
            }
        );
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chromium", "chrome");
        let mechanism = mechanism(Ok(()), Some(Arc::clone(&browser)));
        let (recovery, _) = named_recovery(
            "on_demand",
            NativeHostState::Updatable,
            &browser,
            &mechanism,
        );

        assert_eq!(
            request_chrome(&recovery, Duration::from_secs(1)),
            RecoveryDecision::Ready {
                browser: "browser_chromium".into(),
            }
        );
        assert_eq!(mechanism.repairs.load(Ordering::SeqCst), 1);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_named_browser_is_never_answered_by_another_installed_browser() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(vec![summary("browser_edge", true)]);
        browser.remember_installed("browser_edge", "edge");
        let mechanism = mechanism(Ok(()), None);
        let (recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);

        assert_eq!(
            request_chrome(&recovery, Duration::from_millis(50)),
            RecoveryDecision::Failed {
                reason: RecoveryFailure::HandshakeTimeout,
                details: vec!["Google Chrome".into()],
            }
        );
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_named_browser_running_in_another_profile_is_not_started_again() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(vec![summary("browser_other_profile", true)]);
        browser.remember_installed("browser_other_profile", "chrome");
        let mechanism = mechanism(Ok(()), None);
        let (recovery, inventory) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);

        assert_eq!(
            request_chrome(&recovery, Duration::from_secs(1)),
            RecoveryDecision::Ready {
                browser: "browser_other_profile".into(),
            }
        );
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 0);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 0);
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

    /// The answer to a Chrome start that never connects within the call.
    fn chrome_timeout() -> RecoveryDecision {
        RecoveryDecision::Failed {
            reason: RecoveryFailure::HandshakeTimeout,
            details: vec!["Google Chrome".into()],
        }
    }

    #[test]
    fn an_unbound_and_a_named_request_for_one_closed_browser_start_it_once() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chrome", "chrome");
        browser.remember_last_connected("chrome");
        let mechanism = mechanism(Ok(()), None);
        let (recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);

        let unbound = {
            let recovery = recovery.clone();
            thread::spawn(move || {
                recovery.request(
                    Instant::now() + Duration::from_secs(2),
                    &AtomicBool::new(false),
                )
            })
        };
        let named = {
            let recovery = recovery.clone();
            thread::spawn(move || request_chrome(&recovery, Duration::from_secs(2)))
        };
        wait_for_launched(&recovery, &FlightScope::Unbound);
        wait_for_launched(&recovery, &FlightScope::Browser("chrome".into()));
        browser.connect(vec![summary("browser_chrome", true)]);

        let ready = RecoveryDecision::Ready {
            browser: "browser_chrome".into(),
        };
        assert_eq!(unbound.join().unwrap(), Ok(ready.clone()));
        assert_eq!(named.join().unwrap(), ready);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_retry_inside_the_guard_window_waits_for_the_first_start() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chrome", "chrome");
        let mechanism = mechanism(Ok(()), None);
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);
        let now = hand_clock(&mut recovery);

        assert_eq!(
            request_chrome(&recovery, Duration::from_millis(50)),
            chrome_timeout()
        );
        *lock(&now) += Duration::from_secs(5);
        let retry = {
            let recovery = recovery.clone();
            thread::spawn(move || request_chrome(&recovery, Duration::from_secs(2)))
        };
        wait_for_launched(&recovery, &FlightScope::Browser("chrome".into()));
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
        browser.connect(vec![summary("browser_chrome", true)]);

        assert_eq!(
            retry.join().unwrap(),
            RecoveryDecision::Ready {
                browser: "browser_chrome".into(),
            }
        );
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_start_that_connected_frees_the_guard() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chromium", "chrome");
        let mechanism = mechanism(Ok(()), Some(Arc::clone(&browser)));
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);
        let _still = hand_clock(&mut recovery);
        let ready = RecoveryDecision::Ready {
            browser: "browser_chromium".into(),
        };

        assert_eq!(request_chrome(&recovery, Duration::from_secs(1)), ready);
        browser.connect(Vec::new());
        assert_eq!(request_chrome(&recovery, Duration::from_secs(1)), ready);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn after_the_guard_window_the_browser_is_started_again() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_installed("browser_chrome", "chrome");
        let mechanism = mechanism(Ok(()), None);
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);
        let now = hand_clock(&mut recovery);

        assert_eq!(
            request_chrome(&recovery, Duration::from_millis(50)),
            chrome_timeout()
        );
        *lock(&now) += LAUNCH_HANDSHAKE_WINDOW;
        assert_eq!(
            request_chrome(&recovery, Duration::from_millis(50)),
            chrome_timeout()
        );
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_start_that_began_no_process_frees_the_guard() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        let mechanism = mechanism(
            Err((RecoveryFailure::LaunchFailed, "fake launch refused".into())),
            None,
        );
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);
        let _still = hand_clock(&mut recovery);
        let refused = RecoveryDecision::Failed {
            reason: RecoveryFailure::LaunchFailed,
            details: vec!["fake launch refused".into()],
        };

        assert_eq!(request_chrome(&recovery, Duration::from_secs(1)), refused);
        assert_eq!(request_chrome(&recovery, Duration::from_secs(1)), refused);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_service_with_no_display_leaves_startup_to_the_person() {
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        browser.remember_last_connected("chrome");
        let mechanism = mechanism(Ok(()), None);
        *lock(&mechanism.outcome) = Err(MechanismError::LeftToThePerson);
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);
        let _still = hand_clock(&mut recovery);
        let left_to_the_person = RecoveryDecision::Manual {
            browsers: vec![chrome_and_edge(NativeHostState::Current).remove(0)],
        };

        assert_eq!(
            recovery
                .request(
                    Instant::now() + Duration::from_secs(1),
                    &AtomicBool::new(false),
                )
                .unwrap(),
            left_to_the_person
        );
        assert_eq!(
            request_chrome(&recovery, Duration::from_secs(1)),
            left_to_the_person
        );
        // Nothing was started, so nothing holds the guard against the next try.
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn an_unreadable_inventory_answers_a_named_request_as_it_answers_an_unbound_one() {
        struct Unreadable;
        impl BrowserInventory for Unreadable {
            fn inspect(&self) -> Result<Vec<RecoveryCandidate>, ()> {
                Err(())
            }
        }
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        let mechanism = mechanism(Ok(()), None);
        let (mut recovery, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);
        recovery.inventory = Arc::new(Unreadable);
        let unreadable = RecoveryDecision::Failed {
            reason: RecoveryFailure::NativeHostUnavailable,
            details: Vec::new(),
        };

        assert_eq!(
            recovery
                .request(
                    Instant::now() + Duration::from_secs(1),
                    &AtomicBool::new(false),
                )
                .unwrap(),
            unreadable
        );
        assert_eq!(
            request_chrome(&recovery, Duration::from_secs(1)),
            unreadable
        );
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_named_browser_is_left_to_the_person_under_manual_or_when_ghostlight_cannot_start_it() {
        let left_to_the_person = RecoveryDecision::Manual {
            browsers: Vec::new(),
        };
        let browser = Arc::new(FakeBrowser::default());
        browser.connect(Vec::new());
        let mechanism = mechanism(Ok(()), Some(Arc::clone(&browser)));
        let (manual, inventory) =
            named_recovery("manual", NativeHostState::Current, &browser, &mechanism);
        assert_eq!(
            request_chrome(&manual, Duration::from_secs(1)),
            left_to_the_person
        );
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 0);

        let (on_demand, _) =
            named_recovery("on_demand", NativeHostState::Current, &browser, &mechanism);
        assert_eq!(
            on_demand
                .request_browser(
                    "brave",
                    Instant::now() + Duration::from_secs(1),
                    &AtomicBool::new(false),
                )
                .unwrap(),
            left_to_the_person
        );
        let mut uninstalled = chrome_and_edge(NativeHostState::Current);
        uninstalled[0].ordinary_executable = None;
        assert_eq!(
            decide_browser(&uninstalled, "chrome"),
            RecoveryPlan::Complete(left_to_the_person)
        );
        assert_eq!(mechanism.repairs.load(Ordering::SeqCst), 0);
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_named_browser_without_a_usable_registration_names_the_remedy() {
        assert_eq!(
            decide_browser(&chrome_and_edge(NativeHostState::Missing), "chrome"),
            RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::NativeHostUnavailable,
                details: vec!["Google Chrome".into()],
            })
        );
        assert_eq!(
            decide_browser(&chrome_and_edge(NativeHostState::OwnedElsewhere), "chrome"),
            RecoveryPlan::Complete(RecoveryDecision::Failed {
                reason: RecoveryFailure::OwnedElsewhere,
                details: vec!["Google Chrome".into()],
            })
        );
    }

    #[test]
    fn adapters_arriving_together_bind_the_first_arrival_without_a_choice() {
        let browser = Arc::new(FakeBrowser::default());
        let inventory = Arc::new(FakeInventory {
            candidates: vec![candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Current,
            )],
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
        let mut recovery = coordinator(Some("on_demand"), inventory);
        recovery.browser = Arc::clone(&browser) as Arc<dyn BrowserPort>;
        recovery.mechanism = Arc::new(FakeMechanism {
            repairs: AtomicUsize::new(0),
            launches: AtomicUsize::new(0),
            outcome: Mutex::new(Ok(())),
            connect: Some(Arc::clone(&browser)),
            extra_arrivals: vec!["browser_second".into()],
        });

        let decision = recovery
            .request(
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false),
            )
            .unwrap();

        assert_eq!(
            decision,
            RecoveryDecision::Ready {
                browser: "browser_chromium".into(),
            }
        );
    }

    #[test]
    fn closed_failure_reason_fact_names_are_distinct() {
        let rendered: HashSet<_> = RecoveryFailure::ALL
            .into_iter()
            .map(RecoveryFailure::as_str)
            .collect();
        assert_eq!(rendered.len(), RecoveryFailure::ALL.len());
    }

    #[test]
    fn cancellation_leaves_no_abandoned_operation() {
        let inventory = Arc::new(FakeInventory {
            candidates: vec![candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Current,
            )],
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
        let mut recovery = coordinator(Some("on_demand"), inventory);
        let mechanism = mechanism(Ok(()), None);
        recovery.mechanism = mechanism.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let active = recovery.clone();
        let active_cancelled = Arc::clone(&cancelled);
        let active_thread = thread::spawn(move || {
            active.request(Instant::now() + Duration::from_secs(2), &active_cancelled)
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while mechanism.launches.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(mechanism.launches.load(Ordering::SeqCst), 1);
        cancelled.store(true, Ordering::SeqCst);
        assert_eq!(
            active_thread.join().unwrap(),
            Err(RecoveryWaitError::Cancelled)
        );
        assert!(lock(&recovery.flights).scopes.is_empty());
    }

    #[test]
    fn recovery_changes_no_authority_and_no_foreign_state() {
        let root = crate::scratch::Scratch::named("recovery-foreign");
        let foreign = root.join("foreign.json");
        std::fs::write(&foreign, b"foreign bytes").unwrap();
        let before = std::fs::read(&foreign).unwrap();
        let decision = decide(
            BrowserStartup::OnDemand,
            &[candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Updatable,
            )],
            None,
        );
        assert!(matches!(
            decision,
            RecoveryPlan::Prepare {
                repair_owned_registration: true,
                launch: true,
                ..
            }
        ));
        assert_eq!(std::fs::read(&foreign).unwrap(), before);
    }

    #[test]
    fn a_launch_uses_the_ordinary_profile_with_no_automation_flags() {
        let command = ordinary_browser_command(Path::new("ordinary-browser"));
        assert_eq!(command.get_program(), Path::new("ordinary-browser"));
        assert_eq!(command.get_args().count(), 0);
    }

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

    #[test]
    fn a_bounded_launch_wait_names_handshake_timeout() {
        let inventory = Arc::new(FakeInventory {
            candidates: vec![candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Current,
            )],
            inspections: AtomicUsize::new(0),
            entered: None,
            release: Mutex::new(None),
        });
        let mut recovery = coordinator(Some("on_demand"), inventory);
        recovery.mechanism = mechanism(Ok(()), None);
        assert_eq!(
            recovery
                .request(
                    Instant::now() + Duration::from_millis(20),
                    &AtomicBool::new(false),
                )
                .unwrap(),
            RecoveryDecision::Failed {
                reason: RecoveryFailure::HandshakeTimeout,
                details: vec!["Chromium".into()],
            }
        );
        assert!(lock(&recovery.flights).scopes.is_empty());
    }

    #[test]
    fn no_browser_is_launched_while_the_session_is_ending() {
        let browser = candidate_without_executable(
            "Chromium",
            BrowserPackage::Native,
            NativeHostState::Current,
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancelled = AtomicBool::new(false);

        let refused = SystemRecoveryMechanism
            .launch_unless_ending(true, false, &browser, deadline, &cancelled)
            .unwrap_err();
        assert!(matches!(
            refused,
            MechanismError::Wait(RecoveryWaitError::Cancelled)
        ));

        // The browser has no executable, so a launch that is attempted fails before it looks for
        // a display or starts anything, on any host; that failure is how the test tells an
        // attempt from a refusal.
        let attempted = SystemRecoveryMechanism
            .launch_unless_ending(false, false, &browser, deadline, &cancelled)
            .unwrap_err();
        assert!(matches!(attempted, MechanismError::Failed(_)));
    }

    #[test]
    fn a_service_inside_an_app_container_starts_no_browser() {
        let browser = candidate_without_executable(
            "Chromium",
            BrowserPackage::Native,
            NativeHostState::Current,
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancelled = AtomicBool::new(false);

        // A browser with no executable fails any launch that is attempted, so this answer shows
        // that none was.
        assert_eq!(
            SystemRecoveryMechanism
                .launch_unless_ending(false, true, &browser, deadline, &cancelled)
                .unwrap_err(),
            MechanismError::LeftToThePerson
        );
        assert_eq!(
            settled(MechanismError::LeftToThePerson, browser.clone()).unwrap(),
            FlightPhase::Complete(RecoveryDecision::Manual {
                browsers: vec![browser],
            })
        );
    }
}
