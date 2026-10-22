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
//! checked against what the computer shows now before anything starts. The start is recovery's
//! own ([`crate::browser::recovery::start_browser`]), noted first in the port's launch guard
//! ([`BrowserPort::note_start`]), so recovery never opens a second window of a browser setup
//! started (ADR-0203 Decision 12.5).

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ghostlight_relay::os;

use super::{parts, BrowserInstanceSummary, SessionSummary, WorkbenchError};
use crate::audit::AuditRecord;
use crate::browser::{recovery, BrowserPort};
use crate::install::browser_package::BrowserPackage;
use crate::install::browser_profiles::{self, BrowserProfiles};
use crate::install::native_host::{
    browser_id_named, BrowserIdentity, NativeHostRegistry, NativeHostReport,
};
use crate::install::setup_state::{self, SetupRecord};
use crate::install::{HarnessState, HarnessSummary};
use crate::intake::session::IntakeChannel;
use crate::language::setup as words;
use crate::language::setup::{
    AgentFact, AgentState, BrowserFact, BrowserForm, Platform, ProfileFact, SetupAction,
    SetupFacts, SetupView, STORE_LISTING_URL,
};
use crate::sync::lock;
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
    /// Start the browser at `program` with `arguments` in the person's graphical session, and
    /// return its process id once it has been created; `None` when no display is reachable, so
    /// nothing started. It does not wait for the browser to do anything, and notes nothing:
    /// [`perform`] notes the start in the launch guard first.
    fn start(&self, program: &Path, arguments: &[String]) -> io::Result<Option<u32>>;
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

    /// The one browser start recovery makes too: reaped, in a process group of its own on Unix,
    /// and given the graphical session's environment.
    fn start(&self, program: &Path, arguments: &[String]) -> io::Result<Option<u32>> {
        let arguments: Vec<&OsStr> = arguments.iter().map(OsStr::new).collect();
        recovery::start_browser(program, &arguments)
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
///
/// Every start is noted first in `browser_port`'s launch guard ([`BrowserPort::note_start`]).
/// While a start of that browser, setup's or recovery's, is younger than
/// [`recovery::LAUNCH_HANDSHAKE_WINDOW`] and the browser has not connected since, the step starts nothing and
/// succeeds, because that browser is already on its way. A start that began no process is
/// forgotten, so the next one may start it.
pub(super) fn perform(
    machine: &dyn SetupMachine,
    browser_port: &dyn BrowserPort,
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
        | SetupAction::ReviewAgent { .. }
        | SetupAction::CopySetup { .. } => return Err(WorkbenchError::SetupStepNotOffered),
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
    if !browser_port.note_start(browser, Instant::now(), recovery::LAUNCH_HANDSHAKE_WINDOW) {
        return Ok(());
    }
    match machine.start(&executable, &arguments) {
        Ok(Some(_)) => Ok(()),
        Ok(None) | Err(_) => {
            browser_port.forget_start(browser);
            Err(WorkbenchError::SetupBrowserNotStarted)
        }
    }
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

/// One fact for every supported family, then one for each connected browser that is none of them.
/// A connected browser is a family's when the name it goes by names that family the way the port
/// maps it ([`browser_id_named`]), so an adapter that reports no name is Chromium's. An unknown
/// connected browser still completes the step; it is shown as Chromium.
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
                    .any(|browser| browser_id_named(&browser.family) == Some(identity.id)),
                extension,
            }
        })
        .collect();
    for browser in connected {
        let known = browser_id_named(&browser.family)
            .is_some_and(|id| identities.iter().any(|identity| identity.id == id));
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
/// What may be done for it is the roster's word, and whether it is in use the workbench's one
/// rule ([`parts::in_use`]).
fn agent_fact(harness: &HarnessSummary, sessions: &[SessionSummary]) -> Option<AgentFact> {
    let state = match harness.state {
        HarnessState::NotDetected => return None,
        HarnessState::Installed => AgentState::Written,
        HarnessState::Available => AgentState::Found,
        HarnessState::Updatable => AgentState::Updatable,
        HarnessState::NeedsAttention => AgentState::NeedsAttention,
    };
    Some(AgentFact {
        id: harness.id.clone(),
        label: harness.label.clone(),
        state,
        can_install: harness.can_install,
        copy_only: harness.copy_only,
        in_use: parts::in_use(harness, sessions),
    })
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
        .find(|harness| parts::is_agent_of(session, harness))
        .map(|harness| harness.name.clone())
}

/// A computer the setup part is read from in tests: what it holds is set by the test, and it
/// counts how often the report and the scan were read.
#[cfg(test)]
pub(super) mod test_support {
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;

    use super::{
        BrowserIdentity, BrowserInstanceSummary, BrowserProfiles, NativeHostReport, SessionSummary,
        SetupMachine,
    };
    use crate::install::container::AppContainer;
    use crate::install::native_host::supported_browsers;
    use crate::install::{HarnessState, HarnessSummary};
    use crate::intake::session::IntakeChannel;
    use crate::provenance::ConnectionDetails;
    use crate::sync::lock;

    /// A fake computer. Every browser family of the browser table is known; none is installed
    /// until the test says.
    #[derive(Default)]
    pub struct Computer {
        report: Mutex<Option<NativeHostReport>>,
        found: Mutex<Vec<&'static str>>,
        profiles: Mutex<Vec<BrowserProfiles>>,
        reads: AtomicUsize,
        ending: AtomicBool,
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
            lock(&self.found).push(id);
        }

        /// Say what the profile scan finds from now on.
        pub fn find_profiles(&self, profiles: Vec<BrowserProfiles>) {
            *lock(&self.profiles) = profiles;
        }

        /// Say that this computer's view is an app container's private one.
        pub fn run_inside_an_app_container(&self) {
            *lock(&self.report) = Some(NativeHostReport {
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

        /// Every start the computer was asked for, in order. It started nothing.
        pub fn started(&self) -> Vec<Started> {
            lock(&self.started).clone()
        }
    }

    impl SetupMachine for Computer {
        fn native_hosts(&self) -> Option<NativeHostReport> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            lock(&self.report).clone()
        }

        fn browsers(&self) -> Vec<BrowserIdentity> {
            supported_browsers()
        }

        fn browser_executable(&self, browser_id: &str) -> Option<PathBuf> {
            lock(&self.found)
                .contains(&browser_id)
                .then(|| Self::executable_of(browser_id))
        }

        fn extension_profiles(&self) -> Vec<BrowserProfiles> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            lock(&self.profiles).clone()
        }

        fn session_ending(&self) -> bool {
            self.ending.load(Ordering::SeqCst)
        }

        fn start(&self, program: &Path, arguments: &[String]) -> io::Result<Option<u32>> {
            let mut started = lock(&self.started);
            started.push(Started {
                program: program.to_owned(),
                arguments: arguments.to_vec(),
            });
            Ok(u32::try_from(started.len()).ok())
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
            agent: crate::agent::Agent::listed(id, label, crate::agent::image::Monogram::FIXTURE),
            state,
            detail: String::new(),
            evidence: None,
            attention: None,
            can_install: false,
            can_fix: false,
            can_uninstall: false,
            can_download: false,
            download_url: None,
            can_locate: true,
            copy_only: false,
            private_view: false,
            config_path: String::new(),
            packaged: false,
            other_copy: None,
            owned_copies: Vec::new(),
            connector_command: "ghostlight mcp".into(),
            manual_setup: String::new(),
        }
    }

    /// A session from the client that calls itself `reported`.
    pub fn session(id: &str, reported: &str) -> SessionSummary {
        SessionSummary {
            id: id.into(),
            agent: crate::agent::Agent::of(Some(reported), IntakeChannel::Mcp),
            connections: Vec::new(),
            channel: IntakeChannel::Mcp,
            tab_count: 0,
            running_operations: 0,
            browser: None,
        }
    }

    /// A session from the client that calls itself `reported`, over one connection Ghostlight saw
    /// started by the installed app of the catalog agent `app`.
    pub fn session_from(id: &str, reported: &str, app: &str) -> SessionSummary {
        let mut session = session(id, reported);
        session.connections.push(ConnectionDetails {
            connection_id: Some(format!("connection_{id}")),
            reported_application: Some(reported.into()),
            channel: IntakeChannel::Mcp,
            explanation: String::new(),
            agent: session.agent.clone(),
            source_agent: Some(crate::install::catalog_agent(app).expect("a catalog app")),
        });
        session
    }

    /// A connected browser that goes by `family`.
    pub fn connected(family: &str) -> BrowserInstanceSummary {
        BrowserInstanceSummary {
            id: format!("browser_{family}"),
            family: family.into(),
            adapter_version: Some("1.0.0".into()),
            connected: true,
            tab_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::AtomicUsize;

    use super::test_support::{connected, harness, session, session_from, Computer, Started};
    use super::*;
    use crate::browser::testing::FakeBrowser;
    use crate::governance::{CapabilitySet, Decision};
    use crate::install::browser_profiles::{ExtensionProfile, ProfileScan};
    use crate::language::history::{CompositionKind, StepReceipt};
    use crate::language::outcome::Outcome;
    use crate::language::setup::{Stage, WORD_CONNECTED};
    use crate::scratch::Scratch;

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

    /// A callback that counts how often it ran, and the count.
    fn counting() -> (Arc<AtomicUsize>, impl Fn() + Send + Sync + 'static) {
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        (calls, move || {
            counted.fetch_add(1, Ordering::SeqCst);
        })
    }

    /// Protects desktop gate 24 and ADR-0200 Decisions 10 and 13: only a top-level, allowed,
    /// succeeded MCP request that reached a browser completes setup (a `ghostlight call` does
    /// not); it writes `state/setup.json` once, with that request's time, remembers which
    /// workspace finished it, and the window's callback fires once, never again in this run or a
    /// later one, whose start finds setup complete and rewrites nothing.
    #[test]
    fn only_the_first_agent_request_answered_in_a_browser_completes_setup() {
        let scratch = Scratch::new();
        let (progress, path) = progress(&scratch);
        let (calls, callback) = counting();
        progress.on_complete(callback);

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
            assert!(!progress.observe(&record), "{record:?}");
        }
        assert!(!progress.is_complete());
        assert!(!path.exists());
        assert_eq!(progress.finished(), None);
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let first = succeeded();
        assert!(progress.observe(&first));
        assert!(progress.is_complete());
        assert_eq!(
            setup_state::read(&path),
            Some(SetupRecord {
                first_request_ms: first.timestamp_ms
            })
        );
        assert_eq!(
            progress.finished(),
            Some(Finished {
                workspace: first.workspace.clone()
            })
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let mut later = succeeded();
        later.timestamp_ms = first.timestamp_ms + 1;
        assert!(!progress.observe(&later));
        assert_eq!(
            setup_state::read(&path).unwrap().first_request_ms,
            first.timestamp_ms
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no second celebration");

        let written = fs::read(&path).unwrap();
        let next = SetupProgress::load(&path);
        let (next_calls, next_callback) = counting();
        next.on_complete(next_callback);
        assert!(next.is_complete(), "the next start finds setup complete");
        assert_eq!(next.finished(), None);
        assert!(!next.observe(&succeeded()));
        assert_eq!(next_calls.load(Ordering::SeqCst), 0);
        assert_eq!(fs::read(&path).unwrap(), written);
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

    /// Protects desktop gate 23 and ADR-0200 Decision 8: each supported browser is read as
    /// installed or not, connected by the name it goes by as the port maps it (ADR-0203 Decision
    /// 12.5), so an adapter that reports no name reads as Chromium, and holding the extension in
    /// the profile the scan found; an unknown connected browser still counts, once; and an app
    /// container's private view is said.
    #[test]
    fn the_browser_step_reads_each_browser_as_this_computer_shows_it() {
        let fact = |here: &BrowsersHere, id: &str| -> BrowserFact {
            here.facts
                .iter()
                .find(|fact| fact.id == id)
                .cloned()
                .unwrap_or_else(|| panic!("no {id} fact"))
        };
        let others = |here: &BrowsersHere| -> Vec<BrowserFact> {
            here.facts
                .iter()
                .filter(|fact| fact.id == OTHER_BROWSER_ID)
                .cloned()
                .collect()
        };

        let computer = Computer::default();
        computer.install("chrome");
        let here = browsers_here(&computer, &[]);
        let chrome = fact(&here, "chrome");
        assert_eq!(chrome.form, BrowserForm::Native);
        assert!(!chrome.connected);
        assert_eq!(chrome.extension, None);
        assert_eq!(fact(&here, "edge").form, BrowserForm::Missing);
        assert!(!here.private_view);

        let scanned = chrome_with_the_extension(2);
        computer.find_profiles(vec![scanned.clone()]);
        let here = browsers_here(&computer, &[]);
        let holding = &scanned.scan.with_extension[0];
        assert_eq!(
            fact(&here, "chrome").extension,
            Some(ProfileFact {
                directory: holding.directory.clone(),
                name: holding.name.clone(),
                profiles: scanned.scan.profiles,
            })
        );

        for family in ["Google Chrome", "Chrome"] {
            let here = browsers_here(&computer, &[connected(family)]);
            assert!(fact(&here, "chrome").connected, "{family}");
            assert!(others(&here).is_empty(), "{family}");
        }

        // an_unnamed_adapter_reads_as_chromium_the_way_the_port_maps_it
        let unnamed =
            crate::browser::validated_browser_name(None).expect("an unnamed browser is named");
        let here = browsers_here(&computer, &[connected(&unnamed)]);
        assert_eq!(
            here.facts
                .iter()
                .filter(|fact| fact.connected)
                .map(|fact| fact.id.as_str())
                .collect::<Vec<_>>(),
            [browser_id_named(&unnamed).expect("a family")],
            "an unnamed adapter is the Chromium row and no other"
        );
        assert_eq!(fact(&here, "chromium").short, OTHER_BROWSER_SHORT);

        let vivaldi = "Vivaldi";
        let here = browsers_here(&computer, &[connected(vivaldi), connected(vivaldi)]);
        assert_eq!(
            others(&here),
            [BrowserFact {
                id: OTHER_BROWSER_ID.into(),
                name: vivaldi.into(),
                short: OTHER_BROWSER_SHORT.into(),
                form: BrowserForm::Native,
                connected: true,
                extension: None,
            }]
        );

        computer.run_inside_an_app_container();
        assert!(browsers_here(&computer, &[]).private_view);
    }

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

    /// Reproduces 80c6f54a and c360f6c0 (ADR-0200 amendment of 2026-10-17): the agent step lists
    /// the agents found, in roster order, and a row reads Connected only while a live session of
    /// its agent is there with that row's entry in place, or, for a copy-only agent, while its
    /// session is live; one Cline session marks only the Cline host that has the entry, and a
    /// Codex session matches the Codex row. A session Ghostlight saw start inside an app marks
    /// only the hosts inside that app (ADR-0203 Decision 9, G3-04), and one from the agent's own
    /// app marks its rows as before. With no agent found, the roster's command is the action.
    #[test]
    fn an_agent_row_reads_connected_only_while_that_agent_is_in_use() {
        use crate::install::HarnessState::{
            Available, Installed, NeedsAttention, NotDetected, Updatable,
        };

        let cline = |id: &str, state| HarnessSummary {
            agent: crate::agent::Agent::listed(
                "cline",
                "Cline",
                crate::agent::image::Monogram::FIXTURE,
            ),
            ..harness(id, id, state)
        };
        let roster = [
            harness("zed", "Zed", NotDetected),
            cline("cline-cli", Installed),
            cline("cline-vscode", Available),
            cline("cline-cursor", Updatable),
            cline("cline-windsurf", NeedsAttention),
            harness("claude-code", "Claude Code", Installed),
            harness("codex", "Codex", Installed),
            HarnessSummary {
                copy_only: true,
                ..harness("goose", "goose", Available)
            },
        ];
        let sessions = [
            session("workspace_1", "cline"),
            session("workspace_2", "CLAUDE-CODE"),
            session("workspace_3", "codex-mcp-client"),
            session("workspace_4", "goose"),
        ];
        let computer = Computer::default();
        let browsers = [connected("Chrome")];
        let connected_rows = |view: &SetupView| -> Vec<String> {
            view.rows
                .iter()
                .filter(|row| row.detail == WORD_CONNECTED)
                .map(|row| row.id.clone())
                .collect()
        };

        let in_use = incomplete(&computer, &browsers, &sessions, &roster);
        assert_eq!(in_use.stage, Stage::Agent);
        assert_eq!(
            in_use
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            roster
                .iter()
                .filter(|harness| harness.state != NotDetected)
                .map(|harness| harness.id.as_str())
                .collect::<Vec<_>>(),
            "an agent that is not here is no row"
        );
        assert_eq!(
            connected_rows(&in_use),
            ["cline-cli", "claude-code", "codex", "goose"]
        );

        // cline_in_vs_code_marks_only_its_row_in_use
        // claude_code_from_a_terminal_still_marks_its_row
        let entries = [
            cline("cline-cli", Installed),
            cline("cline-vscode", Installed),
            harness("claude-code", "Claude Code", Installed),
        ];
        let inside = [
            session_from("workspace_1", "cline", "vscode"),
            session_from("workspace_2", "claude-code", "claude-code"),
        ];
        assert_eq!(
            connected_rows(&incomplete(&computer, &browsers, &inside, &entries)),
            ["cline-vscode", "claude-code"]
        );
        assert_eq!(
            connected_rows(&incomplete(
                &computer,
                &browsers,
                &[session("workspace_1", "cline")],
                &entries
            )),
            ["cline-cli", "cline-vscode"],
            "a session that says no host marks every host with an entry"
        );

        let unseen = incomplete(&computer, &browsers, &[], &roster);
        assert!(
            connected_rows(&unseen).is_empty(),
            "an entry nobody has used yet is not Connected"
        );

        let nothing_found = [harness("zed", "Zed", NotDetected)];
        let view = incomplete(&computer, &browsers, &[], &nothing_found);
        assert!(view.rows.is_empty());
        assert_eq!(
            view.code.expect("the command panel").text,
            nothing_found[0].connector_command
        );
    }

    /// Protects desktop gate 23 and ARCHITECTURE (setup facts are read only while setup is
    /// incomplete): the native-host report and the profile scan are read on every look of an
    /// incomplete setup, and a setup finished in this run, in an earlier run, or not tracked at all
    /// reads nothing from this computer.
    #[test]
    fn a_complete_setup_reads_nothing_from_this_computer() {
        let look = |progress: Option<&SetupProgress>, computer: &Computer| {
            setup_view(&SetupInputs {
                progress,
                sessions: &[session("workspace_1", "claude-code")],
                browsers: &[connected("Chrome")],
                harnesses: &[harness(
                    "claude-code",
                    "Claude Code",
                    HarnessState::Installed,
                )],
                machine: computer,
                agent_name: None,
            })
            .stage
        };
        let computer = || {
            let computer = Computer::default();
            computer.install("chrome");
            computer
        };

        let scratch = Scratch::new();
        let (unfinished, _) = progress(&scratch);
        let reading = computer();
        for _ in 0..2 {
            let before = reading.reads();
            let stage = look(Some(&unfinished), &reading);
            assert!(!matches!(stage, Stage::Done | Stage::Retired), "{stage:?}");
            assert!(
                reading.reads() > before,
                "an incomplete setup reads each look"
            );
        }

        let scratch = Scratch::new();
        let (finished, _) = progress(&scratch);
        assert!(finished.observe(&succeeded()));
        let earlier = Scratch::new();
        let path = earlier.path().join("setup.json");
        fs::write(&path, br#"{"first_request_ms":1}"#).unwrap();
        let earlier = SetupProgress::load(&path);
        for (label, progress, stage) in [
            ("finished in this run", Some(&*finished), Stage::Done),
            (
                "finished in an earlier run",
                Some(&*earlier),
                Stage::Retired,
            ),
            ("not tracked", None, Stage::Retired),
        ] {
            let quiet = computer();
            assert_eq!(look(progress, &quiet), stage, "{label}");
            assert_eq!(
                quiet.reads(),
                0,
                "{label}: neither the report nor the scan ran"
            );
        }
    }

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

    /// Protects desktop gates 23 and 1, and ARCHITECTURE (`POST /api/setup/act`): Add starts the
    /// named installed browser at the store listing and Open starts it in a profile the scan
    /// returns now; any other browser, profile or step is refused and starts nothing, so a request
    /// cannot make the service start another program or pass another argument. Once the session is
    /// ending, even a step the computer offers starts nothing, and that refusal comes before the
    /// step is checked. Every start is noted in the port's launch guard (ADR-0203 Decision 12.5):
    /// a browser setup started is not started again, by setup or by recovery, until it connects.
    #[test]
    fn a_setup_step_starts_only_what_this_computer_offers_now_and_nothing_once_the_session_ends() {
        let computer = Computer::default();
        for id in ["chrome", "brave", "edge"] {
            computer.install(id);
        }
        computer.find_profiles(vec![chrome_with_the_extension(2)]);
        let at_the_listing = |id: &str| Started {
            program: Computer::executable_of(id),
            arguments: vec![STORE_LISTING_URL.to_owned()],
        };
        let offered = [
            (add("chrome"), at_the_listing("chrome")),
            (add("brave"), at_the_listing("brave")),
            (
                open("chrome", Some("Profile 1")),
                Started {
                    program: Computer::executable_of("chrome"),
                    arguments: vec![format!("{PROFILE_DIRECTORY_OPTION}Profile 1")],
                },
            ),
            (
                open("chrome", None),
                Started {
                    program: Computer::executable_of("chrome"),
                    arguments: Vec::new(),
                },
            ),
        ];
        let not_offered = [
            // A profile the scan did not return, or one of another browser.
            open("chrome", Some("Profile 2")),
            open("chrome", Some("")),
            open("chrome", Some("Profile 1 --incognito")),
            open("chrome", Some("../Profile 1")),
            open("edge", Some("Profile 1")),
            // A browser that is not installed, or not in the table.
            add("chromium"),
            open("chromium", None),
            add("netscape"),
            add("other"),
            add(""),
            open("../chrome", None),
            // A step that is not a browser start.
            SetupAction::GetChrome,
            SetupAction::ConnectAgent {
                agent: "cursor".into(),
            },
            SetupAction::ReviewAgent {
                agent: "cursor".into(),
            },
            SetupAction::CopySetup {
                agent: "goose".into(),
            },
        ];
        // Each offered step over a port that has noted no start yet.
        for (action, _) in &offered {
            assert_eq!(
                perform(&computer, &FakeBrowser::default(), action),
                Ok(()),
                "{action:?}"
            );
        }
        let port = FakeBrowser::default();
        for action in &not_offered {
            assert_eq!(
                perform(&computer, &port, action),
                Err(WorkbenchError::SetupStepNotOffered),
                "{action:?}"
            );
        }
        let started: Vec<Started> = offered.iter().map(|(_, start)| start.clone()).collect();
        assert_eq!(computer.started(), started);

        // opening_chrome_from_setup_is_never_followed_by_a_second_window
        let (open_chrome, opened) = &offered[2];
        assert_eq!(perform(&computer, &port, open_chrome), Ok(()));
        for (again, _) in &offered[..] {
            assert_eq!(perform(&computer, &port, again), Ok(()), "{again:?}");
        }
        let mut once = started.clone();
        once.extend([opened.clone(), at_the_listing("brave")]);
        assert_eq!(
            computer.started(),
            once,
            "Chrome started once while it has not connected; Brave is its own browser"
        );
        assert!(
            !port.note_start("chrome", Instant::now(), recovery::LAUNCH_HANDSHAKE_WINDOW),
            "recovery's claim on the started browser is refused"
        );

        computer.end_the_session(true);
        for action in offered.iter().map(|(action, _)| action).chain(&not_offered) {
            assert_eq!(
                perform(&computer, &port, action),
                Err(WorkbenchError::SetupSessionEnding),
                "{action:?}"
            );
        }
        assert_eq!(
            computer.started(),
            once,
            "nothing started once the session ended"
        );
    }
}
