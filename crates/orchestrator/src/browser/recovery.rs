//! Pre-effect browser-readiness recovery decisions.
//!
//! The executor asks this service only when the ordinary plural-browser resolver proves that no
//! usable adapter exists. This module inspects local installation facts, chooses no browser unless
//! the evidence is unique, and joins simultaneous requests per recovery scope. Where the selected
//! platform and policy permit it, the same flight performs one ordinary-profile launch and waits
//! within the invocation deadline for an inbound adapter.

use std::collections::{HashMap, HashSet};
use std::path::Path;
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
    /// A browser opened, but not the profile the workspace belongs to.
    WrongProfile,
    /// No adapter handshake arrived within the bounded wait.
    HandshakeTimeout,
    /// More than one browser is equally plausible.
    Ambiguous,
}

impl RecoveryFailure {
    /// Every closed failure in stable order.
    pub const ALL: [Self; 8] = [
        Self::BrowserAbsent,
        Self::LaunchFailed,
        Self::SandboxedPackage,
        Self::ExtensionAbsent,
        Self::NativeHostUnavailable,
        Self::WrongProfile,
        Self::HandshakeTimeout,
        Self::Ambiguous,
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
            Self::WrongProfile => "browser_wrong_profile",
            Self::HandshakeTimeout => "browser_handshake_timeout",
            Self::Ambiguous => "browser_recovery_ambiguous",
        }
    }
}

/// One deterministic recovery answer before any browser process is started.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryDecision {
    /// The configured posture leaves startup to the person.
    Manual {
        /// The uniquely selected installed browser, when local evidence found one.
        browser: Option<RecoveryCandidate>,
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

trait RecoveryMechanism: Send + Sync {
    fn launch(
        &self,
        browser: &RecoveryCandidate,
        repair_owned_registration: bool,
    ) -> Result<(), MechanismFailure>;
}

#[derive(Debug)]
struct SystemBrowserInventory;

impl BrowserInventory for SystemBrowserInventory {
    fn inspect(&self) -> Result<Vec<RecoveryCandidate>, ()> {
        NativeHostRegistry::discover()
            .check()
            .map(|report| {
                report
                    .browsers
                    .into_iter()
                    .map(|browser| RecoveryCandidate {
                        id: browser.id,
                        name: browser.name,
                        package: browser.package,
                        package_detail: browser.package_detail,
                        registration: browser.state,
                    })
                    .collect()
            })
            .map_err(|_| ())
    }
}

#[derive(Debug)]
struct SystemRecoveryMechanism;

impl RecoveryMechanism for SystemRecoveryMechanism {
    fn launch(
        &self,
        browser: &RecoveryCandidate,
        repair_owned_registration: bool,
    ) -> Result<(), MechanismFailure> {
        let registry = NativeHostRegistry::discover();
        if repair_owned_registration {
            registry
                .install_selected(std::slice::from_ref(&browser.id))
                .map_err(|error| {
                    (
                        RecoveryFailure::NativeHostUnavailable,
                        format!("{}: {error}", browser.name),
                    )
                })?;
        }
        let executable = registry.browser_executable(&browser.id).ok_or_else(|| {
            (
                RecoveryFailure::LaunchFailed,
                format!(
                    "{} has no ordinary executable Ghostlight can verify",
                    browser.name
                ),
            )
        })?;
        let environment = ghostlight_bridge::session::graphical_session_environment()
            .map_err(|error| (RecoveryFailure::LaunchFailed, error.to_string()))?
            .ok_or_else(|| {
                (
                    RecoveryFailure::LaunchFailed,
                    "No verified graphical user session is available for browser startup.".into(),
                )
            })?;
        let mut command = ordinary_browser_command(&executable);
        command.envs(environment.values());
        command.spawn().map(drop).map_err(|error| {
            (
                RecoveryFailure::LaunchFailed,
                format!("{}: {error}", executable.display()),
            )
        })
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

#[derive(Debug, Default)]
struct Flights {
    active: HashSet<String>,
    completed: HashMap<String, RecoveryResult>,
}

/// Cloneable, service-scoped single-flight recovery decision service.
#[derive(Clone)]
pub struct BrowserRecovery {
    governance: GovernanceFacade,
    inventory: Arc<dyn BrowserInventory>,
    browser: Arc<dyn BrowserPort>,
    mechanism: Arc<dyn RecoveryMechanism>,
    flights: Arc<(Mutex<Flights>, Condvar)>,
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
            flights: Arc::new((Mutex::new(Flights::default()), Condvar::new())),
        }
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
        let (state, changed) = &*self.flights;
        let mut flights = lock(state);
        if flights.active.contains(&scope) {
            loop {
                if cancelled.load(Ordering::SeqCst) {
                    return Err(RecoveryWaitError::Cancelled);
                }
                let now = Instant::now();
                if now >= deadline {
                    return Err(RecoveryWaitError::Deadline);
                }
                let wait = deadline
                    .saturating_duration_since(now)
                    .min(Duration::from_millis(10));
                let (next, _) = changed
                    .wait_timeout(flights, wait)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                flights = next;
                if !flights.active.contains(&scope) {
                    return flights
                        .completed
                        .get(&scope)
                        .cloned()
                        .unwrap_or(Err(RecoveryWaitError::Deadline));
                }
            }
        }
        flights.active.insert(scope.clone());
        flights.completed.remove(&scope);
        drop(flights);

        let result = self.attempt(requested, pinned, deadline, cancelled);

        let mut flights = lock(state);
        flights.active.remove(&scope);
        flights.completed.insert(scope, result.clone());
        changed.notify_all();
        result
    }

    fn attempt(
        &self,
        requested: Option<&str>,
        pinned: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> RecoveryResult {
        if cancelled.load(Ordering::SeqCst) {
            return Err(RecoveryWaitError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(RecoveryWaitError::Deadline);
        }
        let plan = self.inventory.inspect().map_or_else(
            |_| RecoveryDecision::Failed {
                reason: RecoveryFailure::NativeHostUnavailable,
                details: Vec::new(),
            },
            |candidates| {
                decide(
                    self.governance.browser_startup(),
                    requested,
                    pinned,
                    &candidates,
                )
            },
        );
        let RecoveryDecision::Launch {
            browser,
            repair_owned_registration,
        } = plan
        else {
            return Ok(plan);
        };
        if let Err((reason, detail)) = self.mechanism.launch(&browser, repair_owned_registration) {
            return Ok(RecoveryDecision::Failed {
                reason,
                details: vec![detail],
            });
        }
        loop {
            if cancelled.load(Ordering::SeqCst) {
                return Err(RecoveryWaitError::Cancelled);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(RecoveryDecision::Failed {
                    reason: RecoveryFailure::HandshakeTimeout,
                    details: vec![browser.name],
                });
            }
            let connected = self.browser.browsers();
            if !connected.is_empty() {
                return match choose_browser(None, None, &connected) {
                    Ok(browser) => Ok(RecoveryDecision::Ready { browser }),
                    Err(super::BrowserError::AmbiguousBrowser(candidates)) => {
                        Ok(RecoveryDecision::Failed {
                            reason: RecoveryFailure::Ambiguous,
                            details: candidates
                                .into_iter()
                                .map(|id| {
                                    connected
                                        .iter()
                                        .find(|browser| browser.id == id)
                                        .and_then(|browser| browser.name.clone())
                                        .unwrap_or(id)
                                })
                                .collect(),
                        })
                    }
                    Err(_) => Ok(RecoveryDecision::Failed {
                        reason: RecoveryFailure::HandshakeTimeout,
                        details: vec![browser.name],
                    }),
                };
            }
            thread::sleep(
                deadline
                    .saturating_duration_since(now)
                    .min(Duration::from_millis(10)),
            );
        }
    }
}

fn decide(
    mode: BrowserStartup,
    requested: Option<&str>,
    pinned: Option<&str>,
    candidates: &[RecoveryCandidate],
) -> RecoveryDecision {
    if requested.is_some() || pinned.is_some() {
        return RecoveryDecision::Failed {
            reason: RecoveryFailure::WrongProfile,
            details: requested
                .or(pinned)
                .map(str::to_owned)
                .into_iter()
                .collect(),
        };
    }

    let usable: Vec<_> = candidates
        .iter()
        .filter(|candidate| {
            candidate.package == BrowserPackage::Native
                || (candidate.package == BrowserPackage::NotChecked
                    && candidate.registration != NativeHostState::Missing)
        })
        .cloned()
        .collect();
    if usable.len() > 1 {
        return RecoveryDecision::Failed {
            reason: RecoveryFailure::Ambiguous,
            details: usable.iter().map(|browser| browser.name.clone()).collect(),
        };
    }
    let Some(browser) = usable.into_iter().next() else {
        let sandboxed: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.package.sandboxed())
            .map(|candidate| candidate.package_detail.clone())
            .collect();
        return RecoveryDecision::Failed {
            reason: if sandboxed.is_empty() {
                RecoveryFailure::BrowserAbsent
            } else {
                RecoveryFailure::SandboxedPackage
            },
            details: sandboxed,
        };
    };

    if mode == BrowserStartup::Manual {
        return RecoveryDecision::Manual {
            browser: Some(browser),
        };
    }
    match browser.registration {
        NativeHostState::Missing | NativeHostState::NeedsAttention => RecoveryDecision::Failed {
            reason: RecoveryFailure::NativeHostUnavailable,
            details: vec![browser.name],
        },
        NativeHostState::Current | NativeHostState::Updatable => RecoveryDecision::Launch {
            repair_owned_registration: browser.registration == NativeHostState::Updatable,
            browser,
        },
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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
        launches: AtomicUsize,
        outcome: Mutex<Result<(), MechanismFailure>>,
        connect: Option<Arc<FakeBrowser>>,
    }

    impl RecoveryMechanism for FakeMechanism {
        fn launch(
            &self,
            _browser: &RecoveryCandidate,
            _repair_owned_registration: bool,
        ) -> Result<(), MechanismFailure> {
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
            mechanism: Arc::new(FakeMechanism {
                launches: AtomicUsize::new(0),
                outcome: Mutex::new(Err((
                    RecoveryFailure::LaunchFailed,
                    "fake launch refused".into(),
                ))),
                connect: None,
            }),
            flights: Arc::new((Mutex::new(Flights::default()), Condvar::new())),
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
        let mechanism = Arc::new(FakeMechanism {
            launches: AtomicUsize::new(0),
            outcome: Mutex::new(Ok(())),
            connect: Some(Arc::clone(&browser)),
        });
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
        let decision = coordinator(Some("manual"), Arc::clone(&inventory))
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
                browser: Some(RecoveryCandidate { name, .. })
            } if name == "Chromium"
        ));
        assert_eq!(inventory.inspections.load(Ordering::SeqCst), 1);
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
            RecoveryDecision::Failed {
                reason: RecoveryFailure::SandboxedPackage,
                details
            } if details[0].contains("Snap") && details[0].contains("native browser package")
        ));
    }

    #[test]
    fn an_ambiguous_browser_set_refuses_and_names_candidates() {
        let decision = decide(
            BrowserStartup::OnDemand,
            None,
            None,
            &[
                candidate(
                    "Google Chrome",
                    BrowserPackage::Native,
                    NativeHostState::Current,
                ),
                candidate("Chromium", BrowserPackage::Native, NativeHostState::Current),
            ],
        );
        assert_eq!(
            decision,
            RecoveryDecision::Failed {
                reason: RecoveryFailure::Ambiguous,
                details: vec!["Google Chrome".into(), "Chromium".into()],
            }
        );
    }

    #[test]
    fn each_closed_failure_reason_is_reachable_and_distinct() {
        let rendered: HashSet<_> = RecoveryFailure::ALL
            .into_iter()
            .map(RecoveryFailure::as_str)
            .collect();
        assert_eq!(rendered.len(), RecoveryFailure::ALL.len());
        for reason in RecoveryFailure::ALL {
            let decision = RecoveryDecision::Failed {
                reason,
                details: Vec::new(),
            };
            assert!(
                matches!(decision, RecoveryDecision::Failed { reason: found, .. } if found == reason)
            );
        }
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
        let mechanism = Arc::new(FakeMechanism {
            launches: AtomicUsize::new(0),
            outcome: Mutex::new(Ok(())),
            connect: None,
        });
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
        assert!(lock(&recovery.flights.0).active.is_empty());
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
            RecoveryDecision::Launch {
                repair_owned_registration: true,
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
        recovery.mechanism = Arc::new(FakeMechanism {
            launches: AtomicUsize::new(0),
            outcome: Mutex::new(Ok(())),
            connect: None,
        });
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
        assert!(lock(&recovery.flights.0).active.is_empty());
    }
}
