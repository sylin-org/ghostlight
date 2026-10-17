//! Setup progress: whether a model client has yet had a request answered through a browser.
//!
//! The projection sees every audit record. The first one that shows an agent doing real work in
//! a browser completes setup: it writes the setup record once ([`crate::install::setup_state`]),
//! remembers in memory which workspace did it, and calls the callback a surface attached so it
//! can tell the person. The browser, the policy and the page played no part in the decision, and
//! the record carries nothing about them.
//!
//! The same module reads the machine and composes the setup part (ADR-0200 Decisions 8 to 11): the
//! facts [`setup_view`] gathers from the connected browsers, the live sessions, the cached client
//! roster and, while setup is incomplete, the native-host report and the profile scan. The words
//! are [`crate::language::setup`]'s; nothing here words anything.
//!
//! It also carries out the two setup steps that start a browser ([`perform`]): add Ghostlight in
//! it, or open it in the profile that holds the extension. The window asks, and the step is
//! checked against what the computer shows now before anything starts.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use ghostlight_relay::os;

use super::{BrowserInstanceSummary, SessionSummary, WorkbenchError};
use crate::audit::AuditRecord;
use crate::browser::session::graphical_session_environment;
use crate::install::browser_package::BrowserPackage;
use crate::install::browser_profiles::{self, BrowserProfiles};
use crate::install::native_host::{BrowserIdentity, NativeHostRegistry, NativeHostReport};
use crate::install::setup_state::{self, SetupRecord};
use crate::install::{HarnessState, HarnessSummary};
use crate::intake::session::IntakeChannel;
use crate::language::setup as words;
use crate::language::setup::{
    AgentFact, AgentState, BrowserFact, BrowserForm, Platform, ProfileFact, SetupAction,
    SetupFacts, SetupView, STORE_LISTING_URL,
};
use crate::work::result::Status;

/// The id given to a connected browser that is none of the supported families.
const OTHER_BROWSER_ID: &str = "other";
/// The short name of a connected browser that is none of the supported families.
const OTHER_BROWSER_SHORT: &str = "Chromium";
/// The start of the one argument that opens a browser in a given profile. The profile directory
/// follows it.
const PROFILE_DIRECTORY_OPTION: &str = "--profile-directory=";

/// What completed setup during this run of the service. It is not stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finished {
    /// The workspace whose request completed setup.
    pub workspace: String,
}

/// What runs once, when setup completes.
type Callback = Arc<dyn Fn() + Send + Sync>;

/// Whether setup is complete, and the one moment it becomes so.
pub struct SetupProgress {
    path: PathBuf,
    complete: AtomicBool,
    finished: Mutex<Option<Finished>>,
    callback: Mutex<Option<Callback>>,
}

impl SetupProgress {
    /// Progress for the home whose setup record is at `path`: complete already when the record
    /// exists.
    #[must_use]
    pub fn load(path: &Path) -> Arc<Self> {
        Arc::new(Self {
            path: path.to_owned(),
            complete: AtomicBool::new(setup_state::is_complete(path)),
            finished: Mutex::new(None),
            callback: Mutex::new(None),
        })
    }

    /// Whether setup is complete, from the record or from this run.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }

    /// What completed setup, when it was completed during this run.
    #[must_use]
    pub fn finished(&self) -> Option<Finished> {
        lock(&self.finished).clone()
    }

    /// Run `callback` when setup completes during this run. It runs at most once, and not at all
    /// if setup is already complete. A second callback replaces the first.
    pub fn on_complete(&self, callback: impl Fn() + Send + Sync + 'static) {
        *lock(&self.callback) = Some(Arc::new(callback));
    }

    /// Consider one audit record, and say whether this call completed setup.
    ///
    /// A record qualifies when it is a direct request from a model client, was allowed, succeeded
    /// and reached a browser. The first qualifying record writes the setup record, and a failed
    /// write still completes setup for this run.
    pub fn observe(&self, record: &AuditRecord) -> bool {
        if self.is_complete() || !qualifies(record) {
            return false;
        }
        if self.complete.swap(true, Ordering::AcqRel) {
            return false;
        }
        let _ = setup_state::record_if_absent(
            &self.path,
            &SetupRecord {
                first_request_ms: record.timestamp_ms,
            },
        );
        *lock(&self.finished) = Some(Finished {
            workspace: record.workspace.clone(),
        });
        let callback = lock(&self.callback).clone();
        if let Some(callback) = callback {
            callback();
        }
        true
    }
}

/// Whether `record` shows an agent's request answered by a browser.
fn qualifies(record: &AuditRecord) -> bool {
    record.step.is_none()
        && record.channel == Some(IntakeChannel::Mcp)
        && record.allowed
        && record.status == Status::Succeeded.stored_name()
        && record.browser.is_some()
}

/// What the setup view reads from this computer, and the one thing it starts there. Only an
/// incomplete setup asks for the report and the scan; a complete setup reads nothing. A test
/// supplies a fake that starts nothing.
pub(crate) trait SetupMachine: Send + Sync {
    /// The native-host report, when it can be read.
    fn native_hosts(&self) -> Option<NativeHostReport>;
    /// Every supported browser family, in table order. This is a table and reads nothing.
    fn browsers(&self) -> Vec<BrowserIdentity>;
    /// The ordinary installation of the browser, when one is found. An id that is not in the
    /// table has none.
    fn browser_executable(&self, browser_id: &str) -> Option<PathBuf>;
    /// The profiles that hold the Ghostlight extension, for every installed browser.
    fn extension_profiles(&self) -> Vec<BrowserProfiles>;
    /// Whether the session is ending, so that nothing may be started.
    fn session_ending(&self) -> bool;
    /// Start `program` with `arguments`, detached from this process, and return once it has been
    /// created. It does not wait for the program to do anything.
    fn start(&self, program: &Path, arguments: &[String]) -> io::Result<()>;
}

/// The machine the service runs on.
pub(super) struct ThisComputer(NativeHostRegistry);

impl ThisComputer {
    /// The machine as the current user's environment describes it.
    pub(super) fn discover() -> Self {
        Self(NativeHostRegistry::discover())
    }
}

impl SetupMachine for ThisComputer {
    fn native_hosts(&self) -> Option<NativeHostReport> {
        self.0.check().ok()
    }

    fn browsers(&self) -> Vec<BrowserIdentity> {
        self.0.browser_identities()
    }

    fn browser_executable(&self, browser_id: &str) -> Option<PathBuf> {
        self.0.browser_executable(browser_id)
    }

    fn extension_profiles(&self) -> Vec<BrowserProfiles> {
        browser_profiles::scan(&self.0)
    }

    fn session_ending(&self) -> bool {
        os::session::session_ending()
    }

    /// The start is detached, so the browser holds nothing of the service: no handle, no pipe, no
    /// socket. It needs a graphical session of this user, which is proved first; the browser
    /// inherits this process's environment, which holds the display that proof found.
    fn start(&self, program: &Path, arguments: &[String]) -> io::Result<()> {
        graphical_session_environment()?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no verified graphical user session is available",
            )
        })?;
        let arguments: Vec<&OsStr> = arguments.iter().map(OsStr::new).collect();
        let home = self.0.user_home();
        let directory = if home.as_os_str().is_empty() {
            std::env::temp_dir()
        } else {
            home.to_owned()
        };
        os::spawn_detached(program, &arguments, &directory).map(drop)
    }
}

/// Everything the setup part is composed from, as one look found it.
pub(super) struct SetupInputs<'a> {
    /// The setup progress, when the service tracks setup. Without it nothing is tracked and the
    /// part is the retired view.
    pub progress: Option<&'a SetupProgress>,
    /// The live sessions.
    pub sessions: &'a [SessionSummary],
    /// The connected browsers.
    pub browsers: &'a [BrowserInstanceSummary],
    /// The cached client roster.
    pub harnesses: &'a [HarnessSummary],
    /// What is read from this computer.
    pub machine: &'a dyn SetupMachine,
    /// The agent that finished setup, once it is known.
    pub agent_name: Option<&'a str>,
}

/// The setup part: the facts composed into the view the window draws.
///
/// An incomplete setup re-reads the native-host report and runs the profile scan, both bounded
/// local reads. A complete setup reads nothing from the machine, so a finished run shows the done
/// view and an earlier one the retired view.
pub(super) fn setup_view(inputs: &SetupInputs<'_>) -> SetupView {
    let finished = inputs.progress.and_then(SetupProgress::finished);
    let complete = inputs.progress.is_none_or(SetupProgress::is_complete);
    let mut facts = SetupFacts {
        complete,
        finished: finished.is_some(),
        platform: if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        },
        finish_on_this_computer: false,
        browsers: Vec::new(),
        agents: Vec::new(),
        mcp_command: String::new(),
        agent_name: None,
    };
    if complete {
        if finished.is_some() {
            let identities = inputs.machine.browsers();
            facts.browsers = browser_facts(&identities, inputs.browsers, |_| {
                (BrowserForm::Native, None)
            });
            facts.agent_name = inputs.agent_name.map(str::to_owned);
        }
        return words::compose(&facts);
    }
    let here = browsers_here(inputs.machine, inputs.browsers);
    facts.finish_on_this_computer = here.private_view;
    facts.browsers = here.facts;
    facts.agents = inputs
        .harnesses
        .iter()
        .filter_map(|harness| agent_fact(harness, inputs.sessions))
        .collect();
    facts.mcp_command = inputs
        .harnesses
        .first()
        .map(|harness| harness.connector_command.clone())
        .unwrap_or_default();
    words::compose(&facts)
}

/// What this computer shows of the supported browsers.
pub(crate) struct BrowsersHere {
    /// One fact for every supported family, then one for each connected browser that is none of
    /// them.
    pub facts: Vec<BrowserFact>,
    /// Whether the report was read inside an app container, whose registry is that app's own view.
    pub private_view: bool,
}

/// Read the native-host report and the profile scan, and say how each supported browser stands:
/// how it is installed, whether `connected` holds it, and the profile that holds the extension.
/// Both reads are bounded and local.
pub(crate) fn browsers_here(
    machine: &dyn SetupMachine,
    connected: &[BrowserInstanceSummary],
) -> BrowsersHere {
    let report = machine.native_hosts();
    let identities = machine.browsers();
    let profiles = machine.extension_profiles();
    let facts = browser_facts(&identities, connected, |identity| {
        let package = report.as_ref().and_then(|report| {
            report
                .browsers
                .iter()
                .find(|registered| registered.id == identity.id)
                .map(|registered| registered.package)
        });
        let form = form_of(package, || {
            machine.browser_executable(identity.id).is_some()
        });
        let extension = profiles
            .iter()
            .find(|found| found.browser.id == identity.id)
            .and_then(|found| {
                found
                    .scan
                    .with_extension
                    .first()
                    .map(|profile| ProfileFact {
                        directory: profile.directory.clone(),
                        name: profile.name.clone(),
                        profiles: found.scan.profiles,
                    })
            });
        (form, extension)
    });
    BrowsersHere {
        facts,
        private_view: report
            .as_ref()
            .is_some_and(|report| report.private_view.is_some()),
    }
}

/// How the supported browsers stand on this computer now, for the install summary: the same
/// reads the setup view makes, with no browser connected.
pub(crate) fn browsers_on_this_computer() -> BrowsersHere {
    browsers_here(&ThisComputer::discover(), &[])
}

/// Carry out one setup step that starts a browser.
///
/// `AddInBrowser` starts the browser on the store listing. `OpenBrowser` starts it, in the profile
/// when one is named. The browser must be one whose installation is found now, and a profile must
/// be one the scan returns now, for that browser; anything else, and every kind of step that is
/// not a browser start, is refused with nothing started. Nothing is started while the session is
/// ending.
pub(super) fn perform(
    machine: &dyn SetupMachine,
    action: &SetupAction,
) -> Result<(), WorkbenchError> {
    if machine.session_ending() {
        return Err(WorkbenchError::SetupSessionEnding);
    }
    let (browser, profile, arguments) = match action {
        SetupAction::AddInBrowser { browser } => {
            (browser, None, vec![STORE_LISTING_URL.to_owned()])
        }
        SetupAction::OpenBrowser { browser, profile } => {
            let arguments = profile
                .iter()
                .map(|directory| format!("{PROFILE_DIRECTORY_OPTION}{directory}"))
                .collect();
            (browser, profile.as_deref(), arguments)
        }
        SetupAction::GetChrome
        | SetupAction::ConnectAgent { .. }
        | SetupAction::ReviewAgent { .. } => return Err(WorkbenchError::SetupStepNotOffered),
    };
    let executable = machine
        .browser_executable(browser)
        .ok_or(WorkbenchError::SetupStepNotOffered)?;
    if let Some(directory) = profile {
        let scanned = machine.extension_profiles().into_iter().any(|found| {
            found.browser.id == browser.as_str()
                && found
                    .scan
                    .with_extension
                    .iter()
                    .any(|profile| profile.directory == directory)
        });
        if !scanned {
            return Err(WorkbenchError::SetupStepNotOffered);
        }
    }
    machine
        .start(&executable, &arguments)
        .map_err(|_| WorkbenchError::SetupBrowserNotStarted)
}

/// How a browser family is installed. `found` says whether an ordinary installation is found, and
/// is asked only when the package form was not inspected.
fn form_of(package: Option<BrowserPackage>, found: impl FnOnce() -> bool) -> BrowserForm {
    match package {
        Some(BrowserPackage::Native) => BrowserForm::Native,
        Some(BrowserPackage::Snap) => BrowserForm::Snap,
        Some(BrowserPackage::Flatpak) => BrowserForm::Flatpak,
        Some(BrowserPackage::MultipleSandboxes) => BrowserForm::SnapAndFlatpak,
        Some(BrowserPackage::NotDetected) => BrowserForm::Missing,
        Some(BrowserPackage::NotChecked) | None => {
            if found() {
                BrowserForm::Native
            } else {
                BrowserForm::Missing
            }
        }
    }
}

/// Whether the browser name a connected browser reported is this family.
fn names(identity: &BrowserIdentity, family: &str) -> bool {
    family.eq_ignore_ascii_case(identity.name) || family.eq_ignore_ascii_case(identity.short)
}

/// One fact for every supported family, then one for each connected browser that is none of them.
/// An unknown connected browser still completes the step; it is shown as Chromium.
fn browser_facts(
    identities: &[BrowserIdentity],
    connected: &[BrowserInstanceSummary],
    how: impl Fn(&BrowserIdentity) -> (BrowserForm, Option<ProfileFact>),
) -> Vec<BrowserFact> {
    let mut facts: Vec<BrowserFact> = identities
        .iter()
        .map(|identity| {
            let (form, extension) = how(identity);
            BrowserFact {
                id: identity.id.to_owned(),
                name: identity.name.to_owned(),
                short: identity.short.to_owned(),
                form,
                connected: connected
                    .iter()
                    .any(|browser| names(identity, &browser.family)),
                extension,
            }
        })
        .collect();
    for browser in connected {
        let known = identities
            .iter()
            .any(|identity| names(identity, &browser.family));
        let listed = facts
            .iter()
            .any(|fact| fact.id == OTHER_BROWSER_ID && fact.name == browser.family);
        if !known && !listed {
            facts.push(BrowserFact {
                id: OTHER_BROWSER_ID.to_owned(),
                name: browser.family.clone(),
                short: OTHER_BROWSER_SHORT.to_owned(),
                form: BrowserForm::Native,
                connected: true,
                extension: None,
            });
        }
    }
    facts
}

/// One client on this computer as the setup view knows it, or none for a client that is not here.
fn agent_fact(harness: &HarnessSummary, sessions: &[SessionSummary]) -> Option<AgentFact> {
    let state = match harness.state {
        HarnessState::NotDetected => return None,
        HarnessState::Installed => AgentState::Written,
        HarnessState::Available | HarnessState::Updatable => AgentState::Found,
        HarnessState::NeedsAttention => AgentState::NeedsAttention,
    };
    Some(AgentFact {
        id: harness.id.clone(),
        label: harness.label.clone(),
        state,
        private_view: harness.private_view,
        seen: sessions.iter().any(|session| is_agent_of(session, harness)),
    })
}

/// Whether `session` is a session of the agent `harness` sets up: the session's agent, named as a
/// person knows it, is that agent's product name. A product on several hosts is one agent here,
/// because the name a client reports does not say the host.
fn is_agent_of(session: &SessionSummary, harness: &HarnessSummary) -> bool {
    session.agent.eq_ignore_ascii_case(&harness.name)
}

/// The name of the agent whose request finished setup: the roster's product name for the
/// finishing session's agent, and none when the session is gone or its agent is not one
/// Ghostlight knows.
pub(super) fn agent_name(
    finished: &Finished,
    sessions: &[SessionSummary],
    harnesses: &[HarnessSummary],
) -> Option<String> {
    let session = sessions
        .iter()
        .find(|session| session.id == finished.workspace)?;
    harnesses
        .iter()
        .find(|harness| is_agent_of(session, harness))
        .map(|harness| harness.name.clone())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A computer the setup part is read from in tests: what it holds is set by the test, and it
/// counts how often the report and the scan were read.
#[cfg(test)]
pub(super) mod test_support {
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Mutex, PoisonError};

    use super::{
        BrowserIdentity, BrowserInstanceSummary, BrowserProfiles, NativeHostReport, SessionSummary,
        SetupMachine, SetupView,
    };
    use crate::install::container::AppContainer;
    use crate::install::{HarnessState, HarnessSummary};
    use crate::intake::session::IntakeChannel;

    /// A fake computer. Every browser family is known; none is installed until the test says.
    #[derive(Default)]
    pub struct Computer {
        report: Mutex<Option<NativeHostReport>>,
        found: Mutex<Vec<&'static str>>,
        profiles: Mutex<Vec<BrowserProfiles>>,
        reads: AtomicUsize,
        ending: AtomicBool,
        refuses: AtomicBool,
        started: Mutex<Vec<Started>>,
    }

    /// One browser start a test asked for.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Started {
        /// The program that was to be started.
        pub program: PathBuf,
        /// Its arguments, in order.
        pub arguments: Vec<String>,
    }

    impl Computer {
        /// Make the browser with `id` found on this computer.
        pub fn install(&self, id: &'static str) {
            self.found
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(id);
        }

        /// Say what the profile scan finds from now on.
        pub fn find_profiles(&self, profiles: Vec<BrowserProfiles>) {
            *self.profiles.lock().unwrap_or_else(PoisonError::into_inner) = profiles;
        }

        /// Say that this computer's view is an app container's private one.
        pub fn run_inside_an_app_container(&self) {
            *self.report.lock().unwrap_or_else(PoisonError::into_inner) = Some(NativeHostReport {
                connector: PathBuf::new(),
                browsers: Vec::new(),
                private_view: Some(AppContainer {
                    app: "Claude".into(),
                }),
            });
        }

        /// How many times the report or the scan was read.
        pub fn reads(&self) -> usize {
            self.reads.load(Ordering::SeqCst)
        }

        /// The path the fake gives the installed browser with `id`.
        pub fn executable_of(id: &str) -> PathBuf {
            PathBuf::from(format!("/installed/{id}"))
        }

        /// Say whether the session is ending from now on.
        pub fn end_the_session(&self, ending: bool) {
            self.ending.store(ending, Ordering::SeqCst);
        }

        /// Make every start the computer is asked for fail from now on.
        pub fn refuse_to_start(&self) {
            self.refuses.store(true, Ordering::SeqCst);
        }

        /// Every start the computer was asked for, in order. It started nothing.
        pub fn started(&self) -> Vec<Started> {
            self.started
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl SetupMachine for Computer {
        fn native_hosts(&self) -> Option<NativeHostReport> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.report
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        fn browsers(&self) -> Vec<BrowserIdentity> {
            [
                ("chrome", "Google Chrome", "Chrome"),
                ("edge", "Microsoft Edge", "Edge"),
                ("brave", "Brave", "Brave"),
                ("chromium", "Chromium", "Chromium"),
            ]
            .into_iter()
            .map(|(id, name, short)| BrowserIdentity { id, name, short })
            .collect()
        }

        fn browser_executable(&self, browser_id: &str) -> Option<PathBuf> {
            self.found
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&browser_id)
                .then(|| Self::executable_of(browser_id))
        }

        fn extension_profiles(&self) -> Vec<BrowserProfiles> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.profiles
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        fn session_ending(&self) -> bool {
            self.ending.load(Ordering::SeqCst)
        }

        fn start(&self, program: &Path, arguments: &[String]) -> io::Result<()> {
            if self.refuses.load(Ordering::SeqCst) {
                return Err(io::Error::other("the fake computer starts nothing"));
            }
            self.started
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(Started {
                    program: program.to_owned(),
                    arguments: arguments.to_vec(),
                });
            Ok(())
        }
    }

    /// A client on the roster in `state`, started by `ghostlight mcp`.
    pub fn harness(id: &str, label: &str, state: HarnessState) -> HarnessSummary {
        HarnessSummary {
            id: id.into(),
            product_id: id.into(),
            name: label.into(),
            label: label.into(),
            target: label.into(),
            icon: format!("{id}.svg"),
            state,
            detail: String::new(),
            evidence: None,
            can_install: false,
            can_fix: false,
            can_uninstall: false,
            can_download: false,
            can_locate: true,
            private_view: false,
            config_path: String::new(),
            connector_command: "ghostlight mcp".into(),
            manual_setup: String::new(),
        }
    }

    /// A session from the client that calls itself `client_label`.
    pub fn session(id: &str, client_label: &str) -> SessionSummary {
        SessionSummary {
            id: id.into(),
            client_label: client_label.into(),
            agent: crate::language::provenance::agent_name(client_label, IntakeChannel::Mcp),
            connections: Vec::new(),
            channel: IntakeChannel::Mcp,
            leased: false,
            tab_count: 0,
            held_tab_count: 0,
            active_operations: 0,
            browser: None,
        }
    }

    /// A connected browser that reported `family`.
    pub fn connected(family: &str) -> BrowserInstanceSummary {
        BrowserInstanceSummary {
            id: format!("browser_{family}"),
            family: family.into(),
            adapter_version: Some("1.0.0".into()),
            connected: true,
            tab_count: 0,
        }
    }

    /// The row with `id`, which the view must have.
    pub fn row<'a>(view: &'a SetupView, id: &str) -> &'a crate::language::setup::Row {
        view.rows
            .iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("no row {id} in {:?}", view.rows))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::AtomicUsize;

    use super::test_support::{connected, harness, row, session, Computer, Started};
    use super::*;
    use crate::governance::{CapabilitySet, Decision};
    use crate::install::browser_profiles::{ExtensionProfile, ProfileScan};
    use crate::install::scratch::Scratch;
    use crate::language::history::{CompositionKind, StepReceipt};
    use crate::language::outcome::Outcome;
    use crate::language::setup::{RowTone, SetupAction, Stage};

    fn request(status: &str) -> AuditRecord {
        let mut record = AuditRecord::now(
            "invocation_1",
            "workspace_1",
            "browser_read",
            CapabilitySet::EMPTY,
            "authority_1",
            Decision::permitted(),
            status,
            "none",
            &Outcome::StepNotStarted.audit(),
            5,
        )
        .from_channel(Some(IntakeChannel::Mcp))
        .with_browser(Some("Chrome".into()));
        record.timestamp_ms = 1234;
        record
    }

    fn succeeded() -> AuditRecord {
        request(&Status::Succeeded.stored_name())
    }

    fn progress(scratch: &Scratch) -> (Arc<SetupProgress>, PathBuf) {
        let path = scratch.path().join("state").join("setup.json");
        (SetupProgress::load(&path), path)
    }

    #[test]
    fn a_qualifying_record_writes_its_timestamp_once() {
        let scratch = Scratch::new();
        let (progress, path) = progress(&scratch);
        assert!(!progress.is_complete());
        assert!(progress.observe(&succeeded()));
        assert!(progress.is_complete());
        assert_eq!(
            setup_state::read(&path),
            Some(SetupRecord {
                first_request_ms: 1234
            })
        );
        let mut later = succeeded();
        later.timestamp_ms = 9999;
        assert!(!progress.observe(&later));
        assert_eq!(setup_state::read(&path).unwrap().first_request_ms, 1234);
    }

    #[test]
    fn the_run_remembers_which_workspace_finished_setup() {
        let scratch = Scratch::new();
        let (progress, _) = progress(&scratch);
        assert_eq!(progress.finished(), None);
        progress.observe(&succeeded());
        assert_eq!(
            progress.finished(),
            Some(Finished {
                workspace: "workspace_1".into()
            })
        );
    }

    #[test]
    fn records_that_are_not_an_agent_reaching_a_browser_write_nothing() {
        let scratch = Scratch::new();
        let (progress, path) = progress(&scratch);
        let mut child = succeeded();
        child.step = Some(StepReceipt {
            parent: CompositionKind::Flow,
            position: 0,
            total: 1,
            preparation_failed: false,
        });
        let mut command_line = succeeded();
        command_line.channel = Some(IntakeChannel::Cli);
        let mut no_channel = succeeded();
        no_channel.channel = None;
        let mut refused = succeeded();
        refused.allowed = false;
        let mut no_browser = succeeded();
        no_browser.browser = None;
        for record in [
            child,
            command_line,
            no_channel,
            refused,
            no_browser,
            request(&Status::Failed.stored_name()),
            request(&Status::Blocked.stored_name()),
            request(&Status::Cancelled.stored_name()),
            request(&Status::Unknown.stored_name()),
        ] {
            assert!(!progress.observe(&record));
        }
        assert!(!progress.is_complete());
        assert!(!path.exists());
        assert_eq!(progress.finished(), None);
    }

    #[test]
    fn an_existing_record_means_complete_and_is_not_rewritten() {
        let scratch = Scratch::new();
        let path = scratch.path().join("setup.json");
        fs::write(&path, b"junk").unwrap();
        let progress = SetupProgress::load(&path);
        assert!(progress.is_complete());
        assert!(!progress.observe(&succeeded()));
        assert_eq!(fs::read(&path).unwrap(), b"junk");
        assert_eq!(progress.finished(), None);
    }

    #[test]
    fn a_failed_write_still_completes_this_run() {
        let scratch = Scratch::new();
        // The parent of the record is a file, so the record cannot be written.
        fs::write(scratch.path().join("state"), b"not a directory").unwrap();
        let progress = SetupProgress::load(&scratch.path().join("state").join("setup.json"));
        assert!(!progress.is_complete());
        assert!(progress.observe(&succeeded()));
        assert!(progress.is_complete());
        assert!(progress.finished().is_some());
    }

    #[test]
    fn the_callback_fires_exactly_once() {
        let scratch = Scratch::new();
        let (progress, _) = progress(&scratch);
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        progress.on_complete(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });
        assert!(progress.observe(&succeeded()));
        assert!(!progress.observe(&succeeded()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_callback_attached_after_completion_never_fires() {
        let scratch = Scratch::new();
        let (progress, _) = progress(&scratch);
        assert!(progress.observe(&succeeded()));
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        progress.on_complete(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });
        assert!(!progress.observe(&succeeded()));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn the_projection_completes_setup_on_a_settled_request_and_works_without_it() {
        use crate::language::audit_health::Storage;
        use crate::workbench::WorkbenchProjection;

        let bare = WorkbenchProjection::default();
        bare.record(&succeeded(), Storage::Saved);

        let scratch = Scratch::new();
        let (progress, path) = progress(&scratch);
        let projection = WorkbenchProjection::default();
        projection.attach_setup(Arc::clone(&progress));
        let mut child = succeeded();
        child.step = Some(StepReceipt {
            parent: CompositionKind::Flow,
            position: 0,
            total: 1,
            preparation_failed: false,
        });
        projection.record(&child, Storage::Saved);
        assert!(!progress.is_complete());
        projection.record(&succeeded(), Storage::Saved);
        assert!(progress.is_complete());
        assert!(path.is_file());
    }

    // The setup part: the facts read and the view composed from them.

    /// A setup that has not completed, over `computer`, with the given look.
    fn incomplete(
        computer: &Computer,
        browsers: &[BrowserInstanceSummary],
        sessions: &[SessionSummary],
        harnesses: &[HarnessSummary],
    ) -> SetupView {
        let scratch = Scratch::new();
        let (progress, _) = progress(&scratch);
        setup_view(&SetupInputs {
            progress: Some(&progress),
            sessions,
            browsers,
            harnesses,
            machine: computer,
            agent_name: None,
        })
    }

    fn chrome_with_the_extension(profiles: usize) -> BrowserProfiles {
        BrowserProfiles {
            browser: BrowserIdentity {
                id: "chrome",
                name: "Google Chrome",
                short: "Chrome",
            },
            scan: ProfileScan {
                profiles,
                with_extension: vec![ExtensionProfile {
                    directory: "Profile 1".into(),
                    name: Some("Work".into()),
                }],
            },
        }
    }

    #[test]
    fn no_browser_and_a_native_chrome_without_the_extension_asks_to_add_it() {
        let computer = Computer::default();
        computer.install("chrome");
        let view = incomplete(&computer, &[], &[], &[]);
        assert_eq!(view.stage, Stage::Browser);
        let chrome = row(&view, "chrome");
        assert_eq!(chrome.detail, "Not added");
        assert!(matches!(
            chrome.action.as_ref().map(|action| &action.action),
            Some(SetupAction::AddInBrowser { browser }) if browser == "chrome"
        ));
    }

    #[test]
    fn a_chrome_that_holds_the_extension_is_opened_not_added() {
        let computer = Computer::default();
        computer.install("chrome");
        computer.find_profiles(vec![chrome_with_the_extension(2)]);
        let view = incomplete(&computer, &[], &[], &[]);
        assert_eq!(view.stage, Stage::Browser);
        assert!(matches!(
            row(&view, "chrome").action.as_ref().map(|action| &action.action),
            Some(SetupAction::OpenBrowser { browser, profile })
                if browser == "chrome" && profile.as_deref() == Some("Profile 1")
        ));
    }

    #[test]
    fn a_connected_chrome_moves_setup_to_the_agent_step() {
        let computer = Computer::default();
        computer.install("chrome");
        let view = incomplete(&computer, &[connected("Google Chrome")], &[], &[]);
        assert_eq!(view.stage, Stage::Agent);
        assert_eq!(connected_line(&view), "Chrome is connected.");
    }

    fn connected_line(view: &SetupView) -> String {
        view.connected_line
            .clone()
            .expect("a connected browser is named")
    }

    #[test]
    fn a_browser_none_of_the_families_still_completes_the_step_as_chromium() {
        let computer = Computer::default();
        let view = incomplete(&computer, &[connected("Vivaldi")], &[], &[]);
        assert_eq!(view.stage, Stage::Agent);
        assert_eq!(connected_line(&view), "Chromium is connected.");
    }

    #[test]
    fn an_installed_client_nobody_has_seen_says_to_restart_it_and_a_session_turns_it_connected() {
        let computer = Computer::default();
        let roster = [harness(
            "claude-code",
            "Claude Code",
            HarnessState::Installed,
        )];
        let browsers = [connected("Chrome")];
        let unseen = incomplete(&computer, &browsers, &[], &roster);
        assert_eq!(unseen.stage, Stage::Agent);
        let waiting = row(&unseen, "claude-code");
        assert_eq!(waiting.detail, "Restart Claude Code to finish.");
        assert_eq!(waiting.tone, RowTone::Attention);

        let seen = incomplete(
            &computer,
            &browsers,
            &[session("workspace_1", "CLAUDE-CODE")],
            &roster,
        );
        let connected_row = row(&seen, "claude-code");
        assert_eq!(connected_row.detail, "Connected");
        assert_eq!(connected_row.tone, RowTone::Done);
        let by_label = incomplete(
            &computer,
            &browsers,
            &[session("workspace_1", "claude code")],
            &roster,
        );
        assert_eq!(row(&by_label, "claude-code").detail, "Connected");
    }

    #[test]
    fn a_codex_session_turns_the_codex_row_connected() {
        let computer = Computer::default();
        let roster = [harness("codex", "Codex", HarnessState::Installed)];
        let view = incomplete(
            &computer,
            &[connected("Chrome")],
            &[session("workspace_1", "codex-mcp-client")],
            &roster,
        );
        let codex = row(&view, "codex");
        assert_eq!(codex.detail, "Connected");
        assert_eq!(codex.tone, RowTone::Done);
    }

    #[test]
    fn a_client_that_is_not_here_is_no_row_and_the_others_keep_their_states() {
        let computer = Computer::default();
        let roster = [
            harness("goose", "Goose", HarnessState::NotDetected),
            harness("cursor", "Cursor", HarnessState::Available),
            harness("zed", "Zed", HarnessState::Updatable),
            harness("codex", "Codex", HarnessState::NeedsAttention),
        ];
        let view = incomplete(&computer, &[connected("Chrome")], &[], &roster);
        let ids: Vec<&str> = view.rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["cursor", "zed", "codex"]);
        assert!(matches!(
            row(&view, "cursor")
                .action
                .as_ref()
                .map(|action| &action.action),
            Some(SetupAction::ConnectAgent { .. })
        ));
        assert!(matches!(
            row(&view, "zed")
                .action
                .as_ref()
                .map(|action| &action.action),
            Some(SetupAction::ConnectAgent { .. })
        ));
        assert!(matches!(
            row(&view, "codex")
                .action
                .as_ref()
                .map(|action| &action.action),
            Some(SetupAction::ReviewAgent { .. })
        ));
    }

    #[test]
    fn no_client_at_all_gives_the_command_to_paste() {
        let computer = Computer::default();
        let roster = [harness("goose", "Goose", HarnessState::NotDetected)];
        let view = incomplete(&computer, &[connected("Chrome")], &[], &roster);
        assert!(view.rows.is_empty());
        assert_eq!(view.code.expect("the command panel").text, "ghostlight mcp");
    }

    #[test]
    fn an_app_container_must_finish_on_the_computer_itself() {
        let computer = Computer::default();
        computer.install("chrome");
        computer.run_inside_an_app_container();
        let view = incomplete(&computer, &[connected("Chrome")], &[], &[]);
        assert_eq!(view.stage, Stage::Computer);
    }

    #[test]
    fn the_package_form_decides_how_a_browser_is_installed() {
        let never = || -> bool { panic!("an inspected package form is not looked up") };
        assert_eq!(
            form_of(Some(BrowserPackage::Native), never),
            BrowserForm::Native
        );
        assert_eq!(
            form_of(Some(BrowserPackage::Snap), never),
            BrowserForm::Snap
        );
        assert_eq!(
            form_of(Some(BrowserPackage::Flatpak), never),
            BrowserForm::Flatpak
        );
        assert_eq!(
            form_of(Some(BrowserPackage::MultipleSandboxes), never),
            BrowserForm::SnapAndFlatpak
        );
        assert_eq!(
            form_of(Some(BrowserPackage::NotDetected), never),
            BrowserForm::Missing
        );
        for unchecked in [Some(BrowserPackage::NotChecked), None] {
            assert_eq!(form_of(unchecked, || true), BrowserForm::Native);
            assert_eq!(form_of(unchecked, || false), BrowserForm::Missing);
        }
    }

    #[test]
    fn a_record_from_this_run_is_done_and_names_the_agent_that_made_it() {
        let scratch = Scratch::new();
        let (progress, _) = progress(&scratch);
        assert!(progress.observe(&succeeded()));
        let computer = Computer::default();
        let roster = [harness(
            "claude-code",
            "Claude Code",
            HarnessState::Installed,
        )];
        let sessions = [session("workspace_1", "claude-code")];
        let finished = progress.finished().expect("this run finished setup");
        let name = agent_name(&finished, &sessions, &roster);
        assert_eq!(name.as_deref(), Some("Claude Code"));
        let view = setup_view(&SetupInputs {
            progress: Some(&progress),
            sessions: &sessions,
            browsers: &[connected("Chrome")],
            harnesses: &roster,
            machine: &computer,
            agent_name: name.as_deref(),
        });
        assert_eq!(view.stage, Stage::Done);
        assert_eq!(view.headline.lead, "Claude Code just used your browser. ");
        assert_eq!(computer.reads(), 0, "a complete setup reads nothing");
    }

    #[test]
    fn an_agent_nobody_knows_by_that_label_is_not_named() {
        let finished = Finished {
            workspace: "workspace_1".into(),
        };
        let roster = [harness(
            "claude-code",
            "Claude Code",
            HarnessState::Installed,
        )];
        assert_eq!(
            agent_name(&finished, &[session("workspace_1", "my-script")], &roster),
            None
        );
        assert_eq!(
            agent_name(&finished, &[session("workspace_2", "claude-code")], &roster),
            None,
            "the finishing session is gone"
        );
        let mut two_hosts = [
            harness("cline-cursor", "Cline (Cursor)", HarnessState::Available),
            harness("cline-cli", "Cline (CLI)", HarnessState::Available),
        ];
        for host in &mut two_hosts {
            host.product_id = "cline".into();
            host.name = "Cline".into();
        }
        assert_eq!(
            agent_name(&finished, &[session("workspace_1", "Cline")], &two_hosts).as_deref(),
            Some("Cline"),
            "a product shared by two hosts is named by the product"
        );
    }

    #[test]
    fn a_record_from_an_earlier_run_is_retired_and_nothing_is_read() {
        let scratch = Scratch::new();
        let path = scratch.path().join("setup.json");
        fs::write(&path, br#"{"first_request_ms":1}"#).unwrap();
        let progress = SetupProgress::load(&path);
        let computer = Computer::default();
        computer.install("chrome");
        let view = setup_view(&SetupInputs {
            progress: Some(&progress),
            sessions: &[session("workspace_1", "claude-code")],
            browsers: &[connected("Chrome")],
            harnesses: &[harness(
                "claude-code",
                "Claude Code",
                HarnessState::Installed,
            )],
            machine: &computer,
            agent_name: None,
        });
        assert_eq!(view.stage, Stage::Retired);
        assert_eq!(computer.reads(), 0, "neither the report nor the scan ran");
    }

    #[test]
    fn a_service_that_tracks_no_setup_shows_the_retired_view_and_reads_nothing() {
        let computer = Computer::default();
        let view = setup_view(&SetupInputs {
            progress: None,
            sessions: &[],
            browsers: &[],
            harnesses: &[],
            machine: &computer,
            agent_name: None,
        });
        assert_eq!(view.stage, Stage::Retired);
        assert_eq!(computer.reads(), 0);
    }

    #[test]
    fn an_incomplete_setup_reads_the_report_and_the_scan_each_time() {
        let computer = Computer::default();
        incomplete(&computer, &[], &[], &[]);
        assert_eq!(computer.reads(), 2);
        incomplete(&computer, &[], &[], &[]);
        assert_eq!(computer.reads(), 4);
    }

    // The steps that start a browser.

    fn add(browser: &str) -> SetupAction {
        SetupAction::AddInBrowser {
            browser: browser.into(),
        }
    }

    fn open(browser: &str, profile: Option<&str>) -> SetupAction {
        SetupAction::OpenBrowser {
            browser: browser.into(),
            profile: profile.map(str::to_owned),
        }
    }

    fn refused(result: Result<(), WorkbenchError>) {
        assert_eq!(result, Err(WorkbenchError::SetupStepNotOffered));
    }

    #[test]
    fn adding_in_chrome_starts_chrome_on_the_listing_and_nothing_else() {
        let computer = Computer::default();
        computer.install("chrome");
        perform(&computer, &add("chrome")).unwrap();
        assert_eq!(
            computer.started(),
            [Started {
                program: Computer::executable_of("chrome"),
                arguments: vec![STORE_LISTING_URL.to_owned()],
            }]
        );
    }

    #[test]
    fn adding_starts_the_browser_the_row_names() {
        let computer = Computer::default();
        for id in ["chrome", "edge", "brave"] {
            computer.install(id);
        }
        perform(&computer, &add("brave")).unwrap();
        perform(&computer, &add("edge")).unwrap();
        let started = computer.started();
        assert_eq!(started.len(), 2);
        assert_eq!(started[0].program, Computer::executable_of("brave"));
        assert_eq!(started[1].program, Computer::executable_of("edge"));
        for launch in &started {
            assert_eq!(launch.arguments, [STORE_LISTING_URL]);
        }
    }

    #[test]
    fn opening_names_the_scanned_profile_as_the_only_argument() {
        let computer = Computer::default();
        computer.install("chrome");
        computer.find_profiles(vec![chrome_with_the_extension(2)]);
        perform(&computer, &open("chrome", Some("Profile 1"))).unwrap();
        perform(&computer, &open("chrome", None)).unwrap();
        let started = computer.started();
        assert_eq!(started[0].program, Computer::executable_of("chrome"));
        assert_eq!(started[0].arguments, ["--profile-directory=Profile 1"]);
        assert!(started[1].arguments.is_empty());
    }

    #[test]
    fn a_profile_the_scan_did_not_return_is_refused_and_nothing_starts() {
        let computer = Computer::default();
        computer.install("chrome");
        computer.install("edge");
        computer.find_profiles(vec![chrome_with_the_extension(2)]);
        for action in [
            open("chrome", Some("Profile 2")),
            open("chrome", Some("")),
            open("chrome", Some("Profile 1 --incognito")),
            open("chrome", Some("../Profile 1")),
            // A profile of another browser is not one of this browser's.
            open("edge", Some("Profile 1")),
        ] {
            refused(perform(&computer, &action));
        }
        assert_eq!(computer.started(), []);
    }

    #[test]
    fn a_browser_that_is_unknown_or_not_installed_is_refused_and_nothing_starts() {
        let computer = Computer::default();
        computer.install("chrome");
        for action in [
            add("netscape"),
            add("other"),
            add("edge"),
            add(""),
            open("brave", None),
            open("../chrome", None),
        ] {
            refused(perform(&computer, &action));
        }
        assert_eq!(computer.started(), []);
    }

    #[test]
    fn a_step_that_is_not_a_browser_start_is_refused() {
        let computer = Computer::default();
        computer.install("chrome");
        for action in [
            SetupAction::GetChrome,
            SetupAction::ConnectAgent {
                agent: "cursor".into(),
            },
            SetupAction::ReviewAgent {
                agent: "cursor".into(),
            },
        ] {
            refused(perform(&computer, &action));
        }
        assert_eq!(computer.started(), []);
    }

    #[test]
    fn nothing_starts_while_the_session_is_ending() {
        let computer = Computer::default();
        computer.install("chrome");
        computer.find_profiles(vec![chrome_with_the_extension(2)]);
        computer.end_the_session(true);
        for action in [
            add("chrome"),
            open("chrome", Some("Profile 1")),
            open("chrome", None),
            add("netscape"),
        ] {
            assert_eq!(
                perform(&computer, &action),
                Err(WorkbenchError::SetupSessionEnding)
            );
        }
        assert_eq!(computer.started(), []);
        computer.end_the_session(false);
        perform(&computer, &add("chrome")).unwrap();
        assert_eq!(computer.started().len(), 1);
    }

    #[test]
    fn a_computer_that_will_not_start_the_browser_says_so() {
        let computer = Computer::default();
        computer.install("chrome");
        computer.refuse_to_start();
        assert_eq!(
            perform(&computer, &add("chrome")),
            Err(WorkbenchError::SetupBrowserNotStarted)
        );
    }
}
