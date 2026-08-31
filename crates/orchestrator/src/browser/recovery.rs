//! Pre-effect browser-readiness recovery decisions.
//!
//! The executor asks this service only when the ordinary plural-browser resolver proves that no
//! usable adapter exists. This module inspects local installation facts and never presents a
//! browser choice: unique connectable evidence acts under the configured posture, plural
//! evidence repairs what Ghostlight already owns and leaves startup to the person, and no
//! refusal spends its words declining to choose. Where the selected platform and policy permit
//! it, the same flight performs one ordinary-profile launch and waits within the invocation
//! deadline for an inbound adapter.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use super::{choose_browser, BrowserPort};
use crate::governance::manifest::BrowserStartup;
use crate::governance::GovernanceFacade;
use crate::install::browser_package::BrowserPackage;
use crate::install::native_host::{NativeHostRegistry, NativeHostState};

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
    /// A browser opened, but not the profile the workspace belongs to.
    WrongProfile,
    /// No adapter handshake arrived within the bounded wait.
    HandshakeTimeout,
}

impl RecoveryFailure {
    /// Every closed failure in stable order.
    pub const ALL: [Self; 8] = [
        Self::BrowserAbsent,
        Self::LaunchFailed,
        Self::SandboxedPackage,
        Self::ExtensionAbsent,
        Self::NativeHostUnavailable,
        Self::OwnedElsewhere,
        Self::WrongProfile,
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
            Self::WrongProfile => "browser_wrong_profile",
            Self::HandshakeTimeout => "browser_handshake_timeout",
        }
    }
}

/// One deterministic recovery answer before any browser process is started.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryDecision {
    /// Startup is left to the person: either the configured posture is manual, or more than one
    /// browser could serve and Ghostlight does not choose where to direct attention.
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
    /// One newly connected adapter is ready for the ordinary resolver.
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
        ensure_live(deadline, cancelled).map_err(MechanismError::Wait)?;
        let executable = browser.ordinary_executable.as_ref().ok_or_else(|| {
            MechanismError::Failed((
                RecoveryFailure::LaunchFailed,
                format!(
                    "{} has no ordinary executable Ghostlight can verify",
                    browser.name
                ),
            ))
        })?;
        let environment = ghostlight_bridge::session::graphical_session_environment()
            .map_err(|error| {
                MechanismError::Failed((RecoveryFailure::LaunchFailed, error.to_string()))
            })?
            .ok_or_else(|| {
                MechanismError::Failed((
                    RecoveryFailure::LaunchFailed,
                    "No verified graphical user session is available for browser startup.".into(),
                ))
            })?;
        ensure_live(deadline, cancelled).map_err(MechanismError::Wait)?;
        let mut command = ordinary_browser_command(executable);
        command.envs(environment.values());
        command.spawn().map(drop).map_err(|error| {
            MechanismError::Failed((
                RecoveryFailure::LaunchFailed,
                format!("{}: {error}", executable.display()),
            ))
        })
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

fn ordinary_browser_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
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

#[derive(Debug)]
struct RecoveryFlight {
    state: Mutex<FlightState>,
    changed: Condvar,
}

impl RecoveryFlight {
    fn new() -> Self {
        Self {
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
    scopes: HashMap<String, Arc<RecoveryFlight>>,
}

/// Cloneable, service-scoped single-flight recovery decision service.
#[derive(Clone)]
pub struct BrowserRecovery {
    governance: GovernanceFacade,
    inventory: Arc<dyn BrowserInventory>,
    browser: Arc<dyn BrowserPort>,
    mechanism: Arc<dyn RecoveryMechanism>,
    flights: Arc<Mutex<Flights>>,
}

impl BrowserRecovery {
    /// Discover the production governance and browser-installation facts.
    #[must_use]
    pub fn discover(governance: GovernanceFacade, browser: Arc<dyn BrowserPort>) -> Self {
        Self {
            governance,
            inventory: Arc::new(SystemBrowserInventory),
            browser,
            mechanism: Arc::new(SystemRecoveryMechanism),
            flights: Arc::new(Mutex::new(Flights::default())),
        }
    }

    /// Replace machine discovery with deterministic browser facts in unit tests.
    #[cfg(test)]
    pub(crate) fn set_test_candidates(&mut self, candidates: Vec<RecoveryCandidate>) {
        self.inventory = Arc::new(FixedBrowserInventory { candidates });
    }

    /// Decide one missing-browser recovery request, joining an overlapping request in the same
    /// owning scope.
    pub fn request(
        &self,
        requested: Option<&str>,
        pinned: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<RecoveryDecision, RecoveryWaitError> {
        let scope = requested.or(pinned).unwrap_or("unbound").to_owned();
        let flight = self.join_flight(&scope);
        let result = self.participate(&flight, requested, pinned, deadline, cancelled);
        self.leave_flight(&scope, &flight);
        result
    }

    fn join_flight(&self, scope: &str) -> Arc<RecoveryFlight> {
        let mut flights = lock(&self.flights);
        if let Some(flight) = flights.scopes.get(scope) {
            let flight = Arc::clone(flight);
            lock(&flight.state).participants += 1;
            return flight;
        }
        let flight = Arc::new(RecoveryFlight::new());
        flights.scopes.insert(scope.into(), Arc::clone(&flight));
        flight
    }

    fn leave_flight(&self, scope: &str, flight: &Arc<RecoveryFlight>) {
        let mut flights = lock(&self.flights);
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
        requested: Option<&str>,
        pinned: Option<&str>,
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

                let next = self.advance(phase.clone(), requested, pinned, deadline, cancelled);

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
        phase: FlightPhase,
        requested: Option<&str>,
        pinned: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<FlightPhase, RecoveryWaitError> {
        match phase {
            FlightPhase::Inspecting => Ok(self.inventory.inspect().map_or_else(
                |_| {
                    FlightPhase::Complete(RecoveryDecision::Failed {
                        reason: RecoveryFailure::NativeHostUnavailable,
                        details: Vec::new(),
                    })
                },
                |candidates| {
                    phase_from_plan(decide(
                        self.governance.browser_startup(),
                        requested,
                        pinned,
                        &candidates,
                    ))
                },
            )),
            FlightPhase::Repairing {
                browser,
                launch_after,
            } => match self.mechanism.repair(&browser, deadline, cancelled) {
                Ok(()) if launch_after => Ok(FlightPhase::Launching { browser }),
                Ok(()) => Ok(FlightPhase::Complete(RecoveryDecision::Manual {
                    browsers: vec![browser],
                })),
                Err(MechanismError::Wait(error)) => Err(error),
                Err(MechanismError::Failed((reason, detail))) => {
                    Ok(FlightPhase::Complete(RecoveryDecision::Failed {
                        reason,
                        details: vec![detail],
                    }))
                }
            },
            FlightPhase::RepairingQueue {
                mut remaining,
                named,
            } => {
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
                    Err(MechanismError::Wait(error)) => Err(error),
                    Err(MechanismError::Failed((reason, detail))) => {
                        Ok(FlightPhase::Complete(RecoveryDecision::Failed {
                            reason,
                            details: vec![detail],
                        }))
                    }
                }
            }
            FlightPhase::Launching { browser } => {
                match self.mechanism.launch(&browser, deadline, cancelled) {
                    Ok(()) => Ok(FlightPhase::Launched { browser }),
                    Err(MechanismError::Wait(error)) => Err(error),
                    Err(MechanismError::Failed((reason, detail))) => {
                        Ok(FlightPhase::Complete(RecoveryDecision::Failed {
                            reason,
                            details: vec![detail],
                        }))
                    }
                }
            }
            FlightPhase::Launched { browser } => {
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
            FlightPhase::Complete(decision) => Ok(FlightPhase::Complete(decision)),
        }
    }
}

fn decide(
    mode: BrowserStartup,
    requested: Option<&str>,
    pinned: Option<&str>,
    candidates: &[RecoveryCandidate],
) -> RecoveryPlan {
    if requested.is_some() || pinned.is_some() {
        return RecoveryPlan::Complete(RecoveryDecision::Failed {
            reason: RecoveryFailure::WrongProfile,
            details: requested
                .or(pinned)
                .map(str::to_owned)
                .into_iter()
                .collect(),
        });
    }

    let installed: Vec<_> = candidates
        .iter()
        .filter(|candidate| {
            candidate.ordinary_executable.is_some() && candidate.package.native_messaging_usable()
        })
        .cloned()
        .collect();
    if installed.len() > 1 {
        // Plural evidence never chooses and never says it declined to. Name every connectable
        // browser and leave startup to the person; repair only what Ghostlight already owns so
        // the named browsers can actually connect when opened.
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
        outcome: Mutex<Result<(), MechanismFailure>>,
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
            outcome.map_err(MechanismError::Failed)
        }
    }

    fn mechanism(
        outcome: Result<(), MechanismFailure>,
        connect: Option<Arc<FakeBrowser>>,
    ) -> Arc<FakeMechanism> {
        Arc::new(FakeMechanism {
            repairs: AtomicUsize::new(0),
            launches: AtomicUsize::new(0),
            outcome: Mutex::new(outcome),
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
            let path = std::env::temp_dir().join(format!(
                "ghostlight-recovery-policy-{}.json",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::write(
                &path,
                format!(
                    r#"{{"schema":3,"name":"test","version":"1","grants":[],"config":[{{"key":"browser.startup","value":"{value}","level":"mandatory"}}]}}"#
                ),
            )
            .unwrap();
            GovernanceFacade::new(Some(path), None)
        } else {
            GovernanceFacade::new(None, None)
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
        }
    }

    fn wait_for_participants(recovery: &BrowserRecovery, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let participants = {
                let flights = lock(&recovery.flights);
                flights
                    .scopes
                    .get("unbound")
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
                None,
                None,
                Instant::now() + Duration::from_secs(2),
                &AtomicBool::new(false),
            )
        });
        entered_rx.recv().unwrap();
        let second = recovery.clone();
        let second_thread = thread::spawn(move || {
            second.request(
                None,
                None,
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
            creator.request(
                None,
                None,
                Instant::now() + Duration::from_secs(2),
                &creator_token,
            )
        });
        entered_rx.recv().unwrap();

        let joiner = recovery.clone();
        let joiner_thread = thread::spawn(move || {
            joiner.request(
                None,
                None,
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
                None,
                None,
                Instant::now() + Duration::from_millis(50),
                &AtomicBool::new(false),
            )
        });
        entered_rx.recv().unwrap();

        let joiner = recovery.clone();
        let joiner_thread = thread::spawn(move || {
            joiner.request(
                None,
                None,
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
            active.request(
                None,
                None,
                Instant::now() + Duration::from_secs(2),
                &active_cancelled,
            )
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
        let first = recovery.join_flight("unbound");
        let joined = recovery.join_flight("unbound");
        assert!(Arc::ptr_eq(&first, &joined));
        lock(&first.state).phase = FlightPhase::Complete(RecoveryDecision::Failed {
            reason: RecoveryFailure::BrowserAbsent,
            details: Vec::new(),
        });

        recovery.leave_flight("unbound", &first);
        let late_joiner = recovery.join_flight("unbound");
        assert!(Arc::ptr_eq(&joined, &late_joiner));
        recovery.leave_flight("unbound", &joined);
        recovery.leave_flight("unbound", &late_joiner);
        assert!(lock(&recovery.flights).scopes.is_empty());

        let next_generation = recovery.join_flight("unbound");
        assert!(!Arc::ptr_eq(&first, &next_generation));
        recovery.leave_flight("unbound", &next_generation);
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
                None,
                None,
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
            None,
            None,
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
            None,
            None,
            &[candidate_without_executable(
                "Microsoft Edge",
                BrowserPackage::NotChecked,
                NativeHostState::Current,
            )],
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
            &[candidate(
                "Microsoft Edge",
                BrowserPackage::NotChecked,
                NativeHostState::Missing,
            )],
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
                None,
                None,
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
            None,
            None,
            &[candidate(
                "Chromium",
                BrowserPackage::Snap,
                NativeHostState::Current,
            )],
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
            None,
            None,
            &[
                candidate(
                    "Google Chrome",
                    BrowserPackage::Native,
                    NativeHostState::Missing,
                ),
                candidate("Chromium", BrowserPackage::Native, NativeHostState::Missing),
            ],
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
            None,
            None,
            &[candidate(
                "Google Chrome",
                BrowserPackage::Native,
                NativeHostState::OwnedElsewhere,
            )],
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
                None,
                None,
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
                None,
                None,
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
            active.request(
                None,
                None,
                Instant::now() + Duration::from_secs(2),
                &active_cancelled,
            )
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
        let root = std::env::temp_dir().join(format!(
            "ghostlight-recovery-foreign-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let foreign = root.join("foreign.json");
        std::fs::write(&foreign, b"foreign bytes").unwrap();
        let before = std::fs::read(&foreign).unwrap();
        let decision = decide(
            BrowserStartup::OnDemand,
            None,
            None,
            &[candidate(
                "Chromium",
                BrowserPackage::Native,
                NativeHostState::Updatable,
            )],
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
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_launch_uses_the_ordinary_profile_with_no_automation_flags() {
        let command = ordinary_browser_command(Path::new("ordinary-browser"));
        assert_eq!(command.get_program(), Path::new("ordinary-browser"));
        assert_eq!(command.get_args().count(), 0);
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
                    None,
                    None,
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
}
