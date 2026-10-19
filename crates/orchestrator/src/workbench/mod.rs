//! Orchestrator-owned desktop read model, user intents, and operating-system presentation port.

mod address;
pub mod context;
mod history;
mod input;
mod live;
pub mod parts;
pub mod setup;
#[cfg(test)]
mod surface_tests;
mod watching;
pub mod web;

pub use address::PageAddress;
pub use history::{HistoryStep, StepHistoryState};
pub use input::{
    validate_policy_document, validate_search_query, POLICY_DOCUMENT_LIMIT, SEARCH_QUERY_LIMIT,
};
pub use watching::Watching;

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::browser::adapter::{PresentationActivity, RuntimeControlIntent, RuntimeControlState};
use crate::intake::desktop::TrayModel;
use crate::intake::session::IntakeChannel;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::agent::image::ServedImage;
use crate::audit::AuditRecord;
use crate::browser::BrowserPort;
use crate::events::DomainEvent;
use crate::governance::effective::{EffectiveAuthority, PolicyChip};
use crate::governance::{AuthoringError, CapabilitySet, GovernanceFacade, ManagedPolicyPassport};
use crate::install::icons::IconCache;
use crate::install::{
    HarnessAction, HarnessActionResult, HarnessError, HarnessRegistry, HarnessSetupResult,
    HarnessSummary,
};
use crate::language::outcome::Observed;
use crate::language::readiness;
use crate::language::workbench::{self as words, ConnectionCounts};
use crate::workspace::WorkspaceStore;

const HISTORY_LIMIT: usize = 500;
const SEARCH_LIMIT: usize = 100;
const PREVIEW_DETAIL_LIMIT: usize = 8;

/// What a candidate policy would have done to work this machine already did.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolicyPreview {
    /// How many recorded actions were checked.
    pub considered: usize,
    /// How many of them the candidate would have refused.
    pub refused_total: usize,
    /// The most frequent refusals, grouped by what was done and where.
    pub refused: Vec<PreviewRefusal>,
    /// One authored sentence stating the result.
    pub summary: String,
}

/// One kind of work a candidate policy would have refused.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PreviewRefusal {
    /// Catalog tool name.
    pub tool: String,
    /// Where it happened, when a host was recorded.
    pub host: Option<String>,
    /// How many recorded actions this covers.
    pub count: usize,
}

/// What the projection calls after a change that can alter the live parts of the workbench.
type StatusHook = Arc<dyn Fn() + Send + Sync>;

/// Disposable workbench projection fed directly from the closed domain-event vocabulary.
#[derive(Clone, Default)]
pub struct WorkbenchProjection {
    inner: Arc<Mutex<ProjectionState>>,
    presentation: Arc<Mutex<Option<Arc<dyn WorkbenchPresentationPort>>>>,
    events: Arc<Mutex<Vec<Arc<dyn WorkbenchEventSink>>>>,
    status_hook: Arc<Mutex<Option<StatusHook>>>,
    setup: Arc<Mutex<Option<Arc<setup::SetupProgress>>>>,
    /// Held while a change takes its sequence number and reaches every sink, so every sink sees
    /// the changes in sequence order.
    delivery: Arc<Mutex<()>>,
    seq: Arc<AtomicU64>,
}

#[derive(Default)]
struct ProjectionState {
    audit_health: crate::language::audit_health::AuditHealth,
    document_coverage: VecDeque<crate::language::coverage::HumanCoverage>,
    operations: HashMap<String, OperationState>,
    history: VecDeque<HistoryItem>,
    notified: HashSet<(String, NotificationKind)>,
}

impl ProjectionState {
    /// Move one tracked operation to a new phase and describe the change for the workbench.
    fn set_phase(&mut self, invocation: &str, phase: OperationPhase) -> Option<WorkbenchChange> {
        let operation = self.operations.get_mut(invocation)?;
        operation.phase = phase;
        Some(WorkbenchChange::OperationChanged {
            operation: OperationSummary::from(&*operation),
        })
    }
}

struct OperationState {
    invocation: String,
    workspace: String,
    tool: String,
    activity: PresentationActivity,
    capabilities: CapabilitySet,
    /// When the service received the request; a row keeps it from waiting to running.
    started_at_ms: u64,
    phase: OperationPhase,
    provenance: crate::provenance::ConnectionDetails,
}

impl WorkbenchProjection {
    /// Read current persistence health independently of browser activity.
    #[must_use]
    pub fn audit_health(&self) -> crate::language::audit_health::AuditHealth {
        self.lock().audit_health.clone()
    }

    /// Publish only changed health, without notifications or changes to human runtime controls.
    pub fn audit_health_changed(&self, health: crate::language::audit_health::AuditHealth) {
        {
            let mut state = self.lock();
            if state.audit_health == health {
                return;
            }
            state.audit_health = health.clone();
        }
        self.publish(WorkbenchChange::AuditHealthChanged { health });
    }
    /// Retain bounded volatile document details exclusively for the local human surface.
    pub fn document_coverage(&self, mut details: crate::language::coverage::HumanCoverage) {
        {
            let mut state = self.lock();
            if let Some(previous) = state
                .document_coverage
                .iter()
                .find(|item| item.invocation == details.invocation)
            {
                details.coverage.include(&previous.coverage);
                details.summary = crate::language::coverage::human_summary(&details.coverage);
                details.proactive |= previous.proactive;
                for host in &previous.excluded_hosts {
                    if details.excluded_hosts.len() < 256 && !details.excluded_hosts.contains(host)
                    {
                        details.excluded_hosts.push(host.clone());
                    }
                }
            }
            state
                .document_coverage
                .retain(|item| item.invocation != details.invocation);
            state.document_coverage.push_front(details.clone());
            state.document_coverage.truncate(HISTORY_LIMIT);
        }
        self.publish(WorkbenchChange::DocumentCoverageChanged { details });
    }
    /// Restore bounded content-minimized history from the orchestrator-owned audit file.
    pub fn load_history(&self, path: &Path) -> io::Result<()> {
        let mut restored = VecDeque::new();
        let mut health = crate::language::audit_health::AuditHealth::default();
        let result = (|| {
            let file = match File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            };
            let mut reader = BufReader::new(file);
            let mut last_gap = String::new();
            while let Some(line) = crate::audit::read_line(&mut reader)? {
                match line {
                    crate::audit::AuditLine::Receipt(record) => {
                        history::merge(&mut restored, &record, false);
                    }
                    crate::audit::AuditLine::Gap(gap) => {
                        // A failed sync may have written the marker before a retry wrote it again.
                        if last_gap != gap.id {
                            health.unconfirmed_receipts = health
                                .unconfirmed_receipts
                                .saturating_add(gap.unconfirmed_receipts);
                            last_gap = gap.id;
                        }
                    }
                    crate::audit::AuditLine::Unreadable => {
                        health.unreadable_entries = health.unreadable_entries.saturating_add(1);
                    }
                    crate::audit::AuditLine::Empty => {}
                }
            }
            Ok(())
        })();
        health.history_unavailable = result.is_err();
        self.lock().history = restored;
        self.audit_health_changed(health);
        result
    }

    /// Attach or replace the best-effort operating-system presentation adapter.
    pub fn attach_presentation(&self, port: Arc<dyn WorkbenchPresentationPort>) {
        *lock(&self.presentation) = Some(port);
    }

    /// Add a best-effort sequenced change-event sink. Every attached sink receives every event,
    /// in sequence order.
    pub fn attach_events(&self, sink: Arc<dyn WorkbenchEventSink>) {
        lock(&self.events).push(sink);
    }

    /// Attach the progress that every non-child audit record is shown to.
    pub fn attach_setup(&self, progress: Arc<setup::SetupProgress>) {
        *lock(&self.setup) = Some(progress);
    }

    /// The attached setup progress, when one was attached.
    #[must_use]
    pub fn setup(&self) -> Option<Arc<setup::SetupProgress>> {
        lock(&self.setup).clone()
    }

    /// Register what runs after a change that can alter the status or connections parts.
    fn set_status_hook(&self, hook: StatusHook) {
        *lock(&self.status_hook) = Some(hook);
    }

    /// Sequence number of the most recently published change.
    #[must_use]
    pub fn current_seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    /// Number one change and deliver it to every attached sink.
    ///
    /// The sequence advances whether or not anyone listens, so a surface that attaches later can
    /// tell what it missed. A sink must not publish from inside its own delivery. Never call this
    /// while holding the projection state lock: presentation adapters are outbound boundaries and
    /// must not be reached across a domain lock.
    fn publish(&self, change: WorkbenchChange) {
        let refreshes = change.refreshes_live_parts();
        {
            let _ordered = lock(&self.delivery);
            let seq = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
            let sinks = lock(&self.events).clone();
            let event = WorkbenchEvent { seq, change };
            for sink in &sinks {
                sink.publish(event.clone());
            }
        }
        if refreshes {
            let hook = lock(&self.status_hook).clone();
            if let Some(hook) = hook {
                hook();
            }
        }
    }

    /// React to one completed domain transition without affecting its authority or truth.
    pub fn react(&self, event: &DomainEvent) {
        let (notification, change) = {
            let mut state = self.lock();
            match event {
                DomainEvent::WorkWaiting {
                    invocation,
                    workspace,
                    tool,
                    activity,
                    capabilities,
                    provenance,
                    received_at_ms,
                }
                | DomainEvent::WorkStarted {
                    invocation,
                    workspace,
                    tool,
                    activity,
                    capabilities,
                    provenance,
                    received_at_ms,
                } => {
                    let operation = OperationState {
                        invocation: invocation.clone(),
                        workspace: workspace.clone(),
                        tool: tool.clone(),
                        activity: *activity,
                        capabilities: *capabilities,
                        provenance: provenance.clone(),
                        started_at_ms: *received_at_ms,
                        phase: if matches!(event, DomainEvent::WorkWaiting { .. }) {
                            OperationPhase::Waiting
                        } else {
                            OperationPhase::Running
                        },
                    };
                    let started = WorkbenchChange::OperationStarted {
                        operation: OperationSummary::from(&operation),
                    };
                    state.operations.insert(invocation.clone(), operation);
                    (None, Some(started))
                }
                DomainEvent::WorkPhaseStarted {
                    invocation,
                    activity,
                    ..
                } => {
                    let change = state.operations.get_mut(invocation).map(|operation| {
                        operation.activity = *activity;
                        WorkbenchChange::OperationChanged {
                            operation: OperationSummary::from(&*operation),
                        }
                    });
                    (None, change)
                }
                DomainEvent::HoldEntered { invocation, .. } => {
                    (None, state.set_phase(invocation, OperationPhase::Held))
                }
                DomainEvent::WorkBlocked { invocation, .. } => {
                    let change = state.set_phase(invocation, OperationPhase::Blocked);
                    let notification = state
                        .notified
                        .insert((invocation.clone(), NotificationKind::Blocked))
                        .then(|| WorkbenchNotification {
                            kind: NotificationKind::Blocked,
                            title: words::NOTICE_BLOCKED_TITLE.into(),
                            body: words::NOTICE_BLOCKED_BODY.into(),
                        });
                    (notification, change)
                }
                DomainEvent::WorkCompleted { invocation, .. } => {
                    (None, state.set_phase(invocation, OperationPhase::Completed))
                }
                DomainEvent::TabCreated { .. }
                | DomainEvent::DocumentCommitted { .. }
                | DomainEvent::TargetIndicated { .. } => (None, None),
            }
        };
        if let Some(notification) = notification {
            if let Some(port) = lock(&self.presentation).clone() {
                let _ = port.notify(notification);
            }
        }
        if let Some(change) = change {
            self.publish(change);
        }
    }

    #[cfg(test)]
    pub(crate) fn record(
        &self,
        record: &AuditRecord,
        storage: crate::language::audit_health::Storage,
    ) {
        self.record_with_provenance(record, storage, None);
    }

    pub(crate) fn record_with_provenance(
        &self,
        record: &AuditRecord,
        storage: crate::language::audit_health::Storage,
        provenance: Option<crate::provenance::ConnectionDetails>,
    ) {
        let child = record.step.is_some();
        let item = {
            let mut state = self.lock();
            let live = state.operations.contains_key(&record.invocation);
            if !child {
                state.operations.remove(&record.invocation);
                state
                    .notified
                    .retain(|(invocation, _)| invocation != &record.invocation);
            }
            history::merge_stored(&mut state.history, record, live, storage, provenance)
        };
        self.publish(if child {
            WorkbenchChange::CompositionChanged {
                record: Box::new(item),
            }
        } else {
            WorkbenchChange::OperationSettled {
                record: Box::new(item),
            }
        });
        if !child {
            let progress = lock(&self.setup).clone();
            if let Some(progress) = progress {
                progress.observe(record);
            }
        }
    }

    fn operations(&self) -> Vec<OperationSummary> {
        let state = self.lock();
        let mut operations: Vec<_> = state
            .operations
            .values()
            .map(OperationSummary::from)
            .collect();
        operations.sort_by_key(|operation| std::cmp::Reverse(operation.started_at_ms));
        operations
    }

    fn history(&self) -> Vec<HistoryItem> {
        self.lock().history.iter().rev().cloned().collect()
    }

    fn lock(&self) -> MutexGuard<'_, ProjectionState> {
        lock(&self.inner)
    }
}

/// Narrow application boundary consumed by the loopback server.
#[derive(Clone)]
pub struct WorkbenchFacade {
    projection: WorkbenchProjection,
    governance: GovernanceFacade,
    diagnostics: Arc<crate::diagnostics::DiagnosticsHub>,
    harnesses: HarnessRegistry,
    /// The agents' own icons, once the service attaches its cache.
    icons: Arc<OnceLock<IconCache>>,
    live: Arc<live::Live>,
    web: Arc<OnceLock<WebFacts>>,
    native_acts: Arc<Mutex<Option<Arc<dyn WorkbenchNativeActs>>>>,
    quit: Arc<Mutex<Option<Arc<dyn WorkbenchQuitPort>>>>,
    policy_writes: Arc<Mutex<()>>,
    diagnostics_writes: Arc<Mutex<()>>,
}

/// What the facade knows of the loopback server that serves it.
///
/// The server never hands the facade its bearer: a surface in the service process needs the
/// address and who can watch, not the key.
#[derive(Clone, Debug)]
pub struct WebFacts {
    /// Where the workbench is served.
    pub address: web::WorkbenchAddress,
    /// The value that changes on every start of this service.
    pub instance: String,
    /// Whether other screens may watch.
    pub watching: Arc<Watching>,
}

/// Why a precondition-guarded policy write did not happen.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum PolicyWriteError {
    /// The rules on disk are not the ones the caller saw.
    #[error("the rules changed since the caller read them")]
    Stale,
    /// The authority refused the write.
    #[error(transparent)]
    Authoring(#[from] AuthoringError),
}

/// Why "Others can watch" was not changed.
#[derive(Debug, Error)]
pub enum WatchingError {
    /// No watch view is being served, so there is nothing to turn on or off.
    #[error("{}", words::WATCHING_NOT_SERVED)]
    NotServed,
    /// The setting could not be written to this computer.
    #[error("{}", words::WATCHING_NOT_SAVED)]
    NotSaved(#[source] io::Error),
}

/// Why a state-guarded client act did not happen.
#[derive(Debug, Error)]
pub enum HarnessActError {
    /// The client is not in the state the caller saw.
    #[error("the client changed since the caller saw it")]
    Stale,
    /// The registry refused the act.
    #[error(transparent)]
    Failed(#[from] HarnessError),
}

impl WorkbenchFacade {
    /// Construct the facade over existing orchestrator owners.
    #[must_use]
    pub fn new(
        projection: WorkbenchProjection,
        workspaces: WorkspaceStore,
        governance: GovernanceFacade,
        browser: Arc<dyn BrowserPort>,
        diagnostics: Arc<crate::diagnostics::DiagnosticsHub>,
    ) -> Self {
        let web: Arc<OnceLock<WebFacts>> = Arc::default();
        let harnesses = HarnessRegistry::discover();
        let live = live::Live::new(
            &projection,
            workspaces,
            governance.clone(),
            browser,
            Arc::clone(&diagnostics),
            Arc::clone(&web),
            harnesses.clone(),
        );
        Self {
            projection,
            governance,
            diagnostics,
            harnesses,
            icons: Arc::default(),
            live,
            web,
            native_acts: Arc::default(),
            quit: Arc::default(),
            policy_writes: Arc::default(),
            diagnostics_writes: Arc::default(),
        }
    }

    /// The same facade over a client registry that never touches this user's clients.
    #[cfg(test)]
    pub(crate) fn with_harnesses(mut self, harnesses: HarnessRegistry) -> Self {
        self.live.use_harnesses(harnesses.clone());
        self.harnesses = harnesses;
        self
    }

    /// The same facade over installed browsers the no-browser line reads from a fake, for tests
    /// that must not read this computer's.
    #[cfg(test)]
    pub(crate) fn with_installed_browsers(
        self,
        candidates: Vec<crate::browser::recovery::RecoveryCandidate>,
    ) -> Self {
        self.live.use_installed_browsers(candidates);
        self
    }

    /// The same facade over a computer the setup part reads from a fake, for tests that must not
    /// read this one.
    #[cfg(test)]
    pub(crate) fn with_setup_machine(self, machine: Arc<dyn setup::SetupMachine>) -> Self {
        self.live.use_machine(machine);
        self
    }

    /// Record where the loopback server serves this workbench. The first call wins.
    pub fn attach_web(&self, facts: WebFacts) {
        let _ = self.web.set(facts);
    }

    /// The address the workbench is served at, once a server is up.
    #[must_use]
    #[cfg(test)]
    pub fn web_address(&self) -> Option<web::WorkbenchAddress> {
        self.web.get().map(|facts| facts.address.clone())
    }

    /// Whether other screens may watch. Before a server is up, nothing is closed.
    #[must_use]
    pub fn watching(&self) -> bool {
        self.web.get().is_none_or(|facts| facts.watching.is_on())
    }

    /// The body of GET /api/context for the given context, built from what the server reported.
    #[must_use]
    pub fn context_document(&self, context: context::WebContext) -> serde_json::Value {
        let instance = self.web.get().map_or("", |facts| facts.instance.as_str());
        context::context_document(context, self.watching(), instance)
    }

    /// The current process-diagnostics state for surfaces.
    #[must_use]
    pub fn diagnostics_report(&self) -> crate::diagnostics::DiagnosticsReport {
        self.diagnostics.report()
    }

    /// Set process diagnostics on or off, then report the state.
    ///
    /// It flips only when the state differs, so two screens or a retry converge on the request.
    pub fn set_diagnostics(&self, on: bool) -> crate::diagnostics::DiagnosticsReport {
        let _writing = lock(&self.diagnostics_writes);
        if self.diagnostics.directory().is_some() != on {
            self.diagnostics.toggle();
        }
        self.diagnostics_changed()
    }

    /// Tell every screen that process diagnostics may have changed, and report the state.
    fn diagnostics_changed(&self) -> crate::diagnostics::DiagnosticsReport {
        let report = self.diagnostics.report();
        self.projection.publish(WorkbenchChange::PartChanged {
            part: parts::HintedPart::Diagnostics,
            etag: parts::json_etag(&report),
        });
        self.live.refresh();
        report
    }

    /// Turn "Others can watch" on or off, and answer the Controls part as the window sees it.
    ///
    /// This is the only way watching changes. A request that arrives afterwards is in the context
    /// the new state gives it, and turning it off ends every open watching stream.
    pub fn set_watching(&self, on: bool) -> Result<parts::ControlsPart, WatchingError> {
        let facts = self.web.get().ok_or(WatchingError::NotServed)?;
        facts.watching.set(on).map_err(WatchingError::NotSaved)?;
        self.live.refresh();
        Ok(self.live.controls(&self.live.gather()))
    }

    /// What is connected to the service right now, counted the light way.
    #[must_use]
    pub fn connection_counts(&self) -> ConnectionCounts {
        self.live.connection_counts()
    }

    /// Attach the best-effort operating-system presentation adapter.
    pub fn attach_presentation(&self, port: Arc<dyn WorkbenchPresentationPort>) {
        self.projection.attach_presentation(port);
    }

    /// Attach the setup progress that watches the service's audit records for the first agent
    /// request that reaches a browser. When setup completes, every screen is told over the stream.
    pub fn attach_setup(&self, progress: Arc<setup::SetupProgress>) {
        let live = Arc::downgrade(&self.live);
        progress.on_complete(move || {
            if let Some(live) = live.upgrade() {
                live.setup_changed();
            }
        });
        self.projection.attach_setup(progress);
    }

    /// The attached setup progress, when one was attached.
    #[must_use]
    pub fn setup(&self) -> Option<Arc<setup::SetupProgress>> {
        self.projection.setup()
    }

    /// Where a request to show Ghostlight opens the window: at the setup view while setup is
    /// incomplete, and otherwise where the window last was.
    #[must_use]
    pub fn show_address(&self) -> Option<PageAddress> {
        self.setup()
            .filter(|progress| !progress.is_complete())
            .map(|_| PageAddress::Setup)
    }

    /// Attach the cache of the agents' own icons, which the window serves, and look for them in
    /// the background. The first call wins.
    ///
    /// The copies the cache already holds are shown at once; each look that changes an icon tells
    /// every screen, so the agents' symbols follow without a reload.
    pub fn attach_icons(&self, cache: IconCache) {
        if self.icons.set(cache).is_ok() {
            self.icons_changed();
            self.refresh_icons();
        }
    }

    /// The image the window serves for the catalog agent `agent_id` now: its app's own icon when
    /// the attached cache holds one, else its monogram ([`crate::agent::image::served`]); none for
    /// an id outside the catalog.
    #[must_use]
    pub fn agent_image(&self, agent_id: &str) -> Option<ServedImage> {
        crate::agent::image::served(agent_id, self.icons.get())
    }

    /// Look for the agents' icons again in the background, off every request.
    fn refresh_icons(&self) {
        if let Some(cache) = self.icons.get() {
            let facade = self.clone();
            cache.refresh_in_background(move || facade.icons_changed());
        }
    }

    /// Tell every screen that an agent's icon changed: the roster is looked at again, since each
    /// row carries its agent's symbol, and the look at the service that follows carries the
    /// sessions' agents.
    fn icons_changed(&self) {
        if let Ok(summaries) = self.harnesses.refresh() {
            self.integrations_changed(&summaries);
        }
    }

    /// Attach the adapter that opens links and folders and asks for files on this computer.
    pub fn attach_native_acts(&self, acts: Arc<dyn WorkbenchNativeActs>) {
        *lock(&self.native_acts) = Some(acts);
    }

    /// Attach what ends the service when the person asks for it.
    pub fn attach_quit(&self, port: Arc<dyn WorkbenchQuitPort>) {
        *lock(&self.quit) = Some(port);
    }

    /// Attach what is told of the tray each time a look at the service finds it changed.
    pub fn attach_tray(&self, port: Arc<dyn WorkbenchTrayPort>) {
        self.live.attach_tray(port);
    }

    /// The tray as the service describes it right now.
    #[must_use]
    pub fn tray_model(&self) -> TrayModel {
        self.live.tray_model()
    }

    /// Begin a person's Quit, exactly as the tray's Quit does. Returns at once: the service
    /// finishes what is in flight, says goodbye on every stream and then leaves.
    pub fn quit(&self) -> Result<(), WorkbenchError> {
        let port = lock(&self.quit)
            .clone()
            .ok_or(WorkbenchError::QuitUnavailable)?;
        port.quit();
        Ok(())
    }

    /// The attached native acts, or the failure every native act ends in without them.
    fn native_acts(&self) -> Result<Arc<dyn WorkbenchNativeActs>, NativeActError> {
        lock(&self.native_acts)
            .clone()
            .ok_or_else(|| WorkbenchError::PresentationUnavailable.into())
    }

    /// Open one of the destinations Ghostlight is willing to point at.
    ///
    /// The surface names a destination from a closed vocabulary, never an address, so this cannot
    /// be talked into opening something the product did not choose.
    pub fn open_destination(
        &self,
        destination: WorkbenchDestination,
    ) -> Result<(), NativeActError> {
        self.open_secure_link(&*self.native_acts()?, destination.url())
    }

    /// Open the official download page of one host, named by its harness id.
    ///
    /// An id outside the roster or a host with no download page is refused and opens nothing.
    pub fn open_harness_download(&self, id: &str) -> Result<(), NativeActError> {
        let acts = self.native_acts()?;
        let url = self.harnesses.download_url(id)?;
        self.open_secure_link(&*acts, url)
    }

    /// Open the diagnostics folder in the operating system's file manager.
    pub fn reveal_diagnostics(&self) -> Result<(), NativeActError> {
        let acts = self.native_acts()?;
        let directory = self
            .diagnostics_report()
            .directory
            .ok_or(NativeActError::DiagnosticsOff)?;
        acts.open_folder(Path::new(&directory)).map_err(native)
    }

    /// Ask the person for a client's application or settings file, and remember the one chosen.
    ///
    /// Answers `None` when the person closes the dialog without choosing.
    pub fn locate_harness_by_picking(
        &self,
        id: &str,
    ) -> Result<Option<HarnessActionResult>, NativeActError> {
        let acts = self.native_acts()?;
        let Some(path) = acts.pick_file(words::LOCATE_DIALOG_TITLE).map_err(native)? else {
            return Ok(None);
        };
        Ok(Some(self.locate_harness(id, &path)?))
    }

    /// Open `url` through `acts`, which only ever gets a secure address.
    fn open_secure_link(
        &self,
        acts: &dyn WorkbenchNativeActs,
        url: &str,
    ) -> Result<(), NativeActError> {
        if !url.starts_with(SECURE_SCHEME) {
            return Err(NativeActError::LinkNotSecure);
        }
        acts.open_url(url).map_err(native)
    }

    /// Add a best-effort sequenced change-event sink used by a live presentation surface. Every
    /// attached sink receives every event, in order.
    pub fn attach_events(&self, sink: Arc<dyn WorkbenchEventSink>) {
        self.projection.attach_events(sink);
    }

    /// Sequence number of the most recently published change.
    #[must_use]
    pub fn current_seq(&self) -> u64 {
        self.projection.current_seq()
    }

    /// Publish the status and connections parts if they differ from the ones last published.
    ///
    /// Call it after anything that may have changed who is connected or what is working. It never
    /// waits and is safe from any thread.
    pub fn refresh_live_parts(&self) {
        self.live.refresh();
    }

    /// Call it after the authority has been checked again: publish the live parts, and a policy
    /// hint when the published authority is not the one last announced.
    pub fn authority_refreshed(&self) {
        self.live.refresh();
        self.live.authority_refreshed();
    }

    /// The one aggregate answer alone, without the rest of a snapshot: no history is read and an
    /// unfinished setup does not read this computer.
    #[must_use]
    pub fn readiness(&self) -> ReadinessSummary {
        self.live.gather().readiness
    }

    /// Build an immutable, content-minimized snapshot for a disposable UI.
    #[must_use]
    pub fn snapshot(&self) -> WorkbenchSnapshot {
        // Read the sequence before anything is gathered. A change publishes after it has been
        // made, so one numbered at or below this is already in what follows, and one above it is
        // newer: the surface applies it by key, and applying it twice does no harm. Reading the
        // sequence last would count a change that landed mid-assembly as included when it is not.
        let seq = self.projection.current_seq();
        let gathered = self.live.gather();
        let status = self.live.status(&gathered);
        let controls = self.live.controls(&gathered);
        let live::Gathered {
            operations,
            sessions,
            browsers,
            no_browser,
            starts,
            authority,
            audit_health,
            audit_required,
            enforced,
            ..
        } = gathered;
        let mut diagnostics = vec![DiagnosticItem::passing(
            "service",
            words::STATUS_SERVICE_LABEL,
            words::STATUS_SERVICE_RUNNING,
        )];
        diagnostics.push(if !browsers.is_empty() {
            let names: Vec<&str> = browsers
                .iter()
                .map(|browser| browser.family.as_str())
                .collect();
            DiagnosticItem::passing(
                "browser",
                words::STATUS_BROWSER_LABEL,
                &words::status_browser_connected(&names),
            )
        } else if let Some(browser) = &starts {
            DiagnosticItem::passing(
                "browser",
                words::STATUS_BROWSER_LABEL,
                &readiness::starts_when_asked(browser),
            )
        } else {
            DiagnosticItem::warning(
                "browser",
                words::STATUS_BROWSER_LABEL,
                words::STATUS_BROWSER_WAITING,
            )
        });
        // The same validity governance enforces, so this card, the header, and the Policy tab
        // agree. A failed reload that keeps the last valid policy in force is not failing closed.
        diagnostics.push(if !authority.authority_valid {
            DiagnosticItem::failing(
                "authority",
                words::STATUS_RULES_LABEL,
                words::STATUS_RULES_INVALID,
            )
        } else if !authority.local_policy_valid {
            DiagnosticItem::warning(
                "authority",
                words::STATUS_RULES_LABEL,
                words::STATUS_RULES_EDIT_INVALID,
            )
        } else {
            DiagnosticItem::passing(
                "authority",
                words::STATUS_RULES_LABEL,
                &words::status_rules_in_force(
                    authority.local_policy_active,
                    authority.managed_authority_active,
                    enforced.channels_off(),
                ),
            )
        });
        diagnostics.push(if audit_health.quiet() {
            DiagnosticItem::passing(
                "audit",
                words::STATUS_HISTORY_LABEL,
                &audit_health.detail(audit_required),
            )
        } else {
            DiagnosticItem::warning(
                "audit",
                words::STATUS_HISTORY_LABEL,
                &audit_health.detail(audit_required),
            )
        });
        let history = self.projection.history();
        WorkbenchSnapshot {
            audit_notice: status.audit_notice,
            audit_health: status.audit_health,
            document_coverage: self
                .projection
                .lock()
                .document_coverage
                .iter()
                .cloned()
                .collect(),
            seq,
            generated_at_ms: unix_ms(),
            runtime_note: status.runtime_note,
            readiness: status.readiness,
            overview: OverviewSummary {
                active_sessions: sessions.len(),
                active_operations: operations.len(),
                connected_browsers: browsers.len(),
                blocked_in_history: history.iter().filter(|item| !item.allowed).count(),
            },
            sessions,
            operations,
            browsers,
            no_browser,
            history,
            diagnostics,
            harnesses: self.harnesses.summaries(),
            configuration: ConfigurationSummary {
                runtime_state: status.service.runtime_state,
                local_policy_configured: authority.local_policy_configured,
                local_policy_active: authority.local_policy_active,
                local_policy_valid: authority.local_policy_valid,
                managed_authority_configured: authority.managed_authority_configured,
                managed_authority_active: authority.managed_authority_active,
                managed_authority_valid: authority.managed_authority_valid,
                managed_policy: status.configuration.managed_policy,
                policy: status.configuration.policy,
            },
            service: status.service,
            controls,
        }
    }

    /// The setup part, as the window sees it.
    ///
    /// It is not part of a snapshot: an unfinished setup reads this computer (the native-host
    /// report and the browser profiles), so only a request for this part pays for those reads.
    #[must_use]
    pub fn setup_view(&self) -> crate::language::setup::SetupView {
        self.live.setup(&self.live.gather())
    }

    /// Carry out one step of the setup view that starts a browser, and answer the setup part as it
    /// is now.
    ///
    /// The step is checked against what this computer shows now, not against what the window drew:
    /// the browser must be installed and a profile must be one the profile scan returns. Nothing is
    /// started while the session is ending. The service keeps no pending state; the page words what
    /// it is waiting for.
    pub fn setup_act(
        &self,
        action: &crate::language::setup::SetupAction,
    ) -> Result<crate::language::setup::SetupView, WorkbenchError> {
        self.live.perform_setup_act(action)?;
        Ok(self.setup_view())
    }

    /// Search what the window shows: its pages, the work in flight, the agents and browsers
    /// connected now, the agents on this computer, the browser-work setting, the checks, and the
    /// recorded actions, newest first.
    ///
    /// A hit matches on the words a person reads for it, never on an opaque id: a recorded action
    /// on its tool, its sentence, the host it landed on, its agent, its browser and its outcome,
    /// so "sylin" finds the actions on sylin.org. Matching allocates nothing per record and only a
    /// match is worded, so a long history stays quick to search. At most [`SEARCH_LIMIT`] hits.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let snapshot = self.snapshot();
        let mut hits = Hits::new(&query);
        for (page, view) in [
            (&words::SEARCH_GLANCE, SearchDestination::Activity),
            (&words::SEARCH_INTEGRATIONS, SearchDestination::Install),
            (&words::SEARCH_STATUS, SearchDestination::Checkup),
            (&words::SEARCH_POLICY, SearchDestination::Configuration),
            (&words::SEARCH_SETTINGS, SearchDestination::Settings),
        ] {
            if hits.finds(&[page.title, page.detail]) {
                hits.push(SearchHit::new(
                    SearchKind::Destination,
                    page.id,
                    page.title,
                    page.detail.to_owned(),
                    view,
                ));
            }
        }
        for operation in &snapshot.operations {
            let phase = operation.phase.word();
            if hits.finds_action(&operation.tool, &[&operation.agent.name, phase]) {
                hits.push(SearchHit::new(
                    SearchKind::Operation,
                    &operation.invocation,
                    &words::tool_words(&operation.tool),
                    words::search_operation_detail(&operation.agent.name, phase),
                    SearchDestination::Activity,
                ));
            }
        }
        for session in &snapshot.sessions {
            let line = words::session_line(
                session.tab_count,
                session.browser.as_deref(),
                session.running_operations > 0,
                session.channel == IntakeChannel::Cli,
            );
            if hits.finds(&[&session.agent.name, &line]) {
                hits.push(SearchHit::new(
                    SearchKind::Session,
                    &session.id,
                    &session.agent.name,
                    line,
                    SearchDestination::Activity,
                ));
            }
        }
        for browser in &snapshot.browsers {
            let detail = words::search_browser_detail(browser.tab_count);
            if hits.finds(&[&browser.family, &detail]) {
                hits.push(SearchHit::new(
                    SearchKind::Browser,
                    &browser.id,
                    &browser.family,
                    detail,
                    SearchDestination::Activity,
                ));
            }
        }
        for harness in &snapshot.harnesses {
            let word = words::harness_state_word(
                harness.state,
                harness.copy_only,
                parts::in_use(harness, &snapshot.sessions),
            );
            if hits.finds(&[&harness.label, word]) {
                hits.push(SearchHit::new(
                    SearchKind::Installation,
                    &harness.id,
                    &harness.label,
                    word.to_owned(),
                    SearchDestination::Install,
                ));
            }
        }
        let runtime = runtime_state_word(snapshot.configuration.runtime_state);
        if hits.finds(&[words::CONTROLS_RUNTIME_LABEL, runtime]) {
            hits.push(SearchHit::new(
                SearchKind::Configuration,
                RUNTIME_SETTING_ID,
                words::CONTROLS_RUNTIME_LABEL,
                runtime.to_owned(),
                SearchDestination::BrowserWork,
            ));
        }
        for diagnostic in &snapshot.diagnostics {
            if hits.finds(&[&diagnostic.label, &diagnostic.detail]) {
                hits.push(SearchHit::new(
                    SearchKind::Diagnostic,
                    &diagnostic.id,
                    &diagnostic.label,
                    diagnostic.detail.clone(),
                    SearchDestination::Checkup,
                ));
            }
        }
        for item in &snapshot.history {
            let outcome = words::search_outcome(item.outcome);
            let browser = item.browser.as_deref().unwrap_or_default();
            if hits.finds_action(
                &item.tool,
                &[
                    &item.summary,
                    item.observed.host.as_deref().unwrap_or_default(),
                    &item.agent.name,
                    browser,
                    outcome.unwrap_or_default(),
                ],
            ) {
                hits.push(SearchHit {
                    outcome: outcome.map(|_| item.outcome),
                    timestamp_ms: Some(item.timestamp_ms),
                    ..SearchHit::new(
                        SearchKind::History,
                        &item.invocation,
                        &words::search_action_title(&item.tool, &item.summary),
                        words::search_action_detail(&item.agent.name, browser),
                        SearchDestination::History,
                    )
                });
            }
        }
        hits.found
    }

    /// Apply an explicit local-human runtime control through the authoritative owners.
    ///
    /// The authority's state is the whole effect: it is checked before every browser effect, with
    /// or without a connected browser. Telling adapters is a best-effort notice the authority sends
    /// itself when the state changes, so it never changes what the person is told.
    pub fn apply_runtime_intent(&self, intent: WorkbenchRuntimeIntent) -> WorkbenchIntentResult {
        self.apply_runtime_control(intent.into(), RuntimeSource::Window)
    }

    /// The state of browser work now: active, paused or ended.
    #[must_use]
    pub fn runtime_state(&self) -> RuntimeControlState {
        self.governance.runtime_state()
    }

    /// Apply one runtime control from `source`, the one path every screen takes.
    ///
    /// The window, a watching screen, the tray and the browser toolbar all end here. The state is
    /// applied through the authority, remembered with where it came from, announced, and then the
    /// live parts are refreshed so every screen can say which of them paused the agents.
    pub fn apply_runtime_control(
        &self,
        intent: RuntimeControlIntent,
        source: RuntimeSource,
    ) -> WorkbenchIntentResult {
        let state = self.governance.apply_runtime_intent(intent);
        self.live.record_runtime(state, source);
        self.projection.publish(WorkbenchChange::RuntimeChanged {
            runtime_state: state,
        });
        self.live.refresh();
        WorkbenchIntentResult {
            accepted: true,
            runtime_state: state,
            message: crate::language::control::confirmation(state).to_owned(),
        }
    }

    /// The compiled policy, for the destination that shows a person what applies to them.
    #[must_use]
    pub fn policy(&self) -> EffectiveAuthority {
        self.governance.effective_authority()
    }

    /// Say what a candidate policy would have changed, before anyone applies it.
    ///
    /// Recorded work only. This states what would have happened to actions this machine already
    /// took; it is deliberately worded as history rather than as a promise about work that has not
    /// happened yet (ADR-0122 Decision 7).
    pub fn preview_user_policy(&self, document: &str) -> Result<PolicyPreview, String> {
        let candidate = self
            .governance
            .candidate_snapshot(document)
            .map_err(|error| error.to_string())?;
        let snapshot = &candidate.snapshot;
        let history = self.projection.history();
        let history = history::operations(&history);
        let considered = history.iter().filter(|item| item.allowed).count();
        let mut refused: Vec<PreviewRefusal> = Vec::new();
        for item in history.iter().filter(|item| item.allowed) {
            let decision = item.observed.host.as_deref().map_or_else(
                || snapshot.authorize_requirements(item.requirements),
                |host| snapshot.authorize_landing(item.requirements, &format!("https://{host}")),
            );
            if decision.allowed {
                continue;
            }
            let host = item.observed.host.clone();
            if let Some(existing) = refused
                .iter_mut()
                .find(|entry| entry.tool == item.tool && entry.host == host)
            {
                existing.count += 1;
            } else {
                refused.push(PreviewRefusal {
                    tool: item.tool.clone(),
                    host,
                    count: 1,
                });
            }
        }
        refused.sort_by_key(|entry| std::cmp::Reverse(entry.count));
        let refused_total: usize = refused.iter().map(|entry| entry.count).sum();
        // Every sentence is about the draft, in the conditional, so the count never reads as a
        // claim that recorded work was refused.
        let summary = if candidate.rules == 0 {
            "These rules allow nothing yet, because there are none. Add at least one rule; \
             until then this policy would refuse everything."
                .to_owned()
        } else if considered == 0 {
            "There is no recorded work on this machine to check these rules against yet.".to_owned()
        } else if refused_total == 0 {
            format!(
                "If these rules had been applied, nothing in the last {considered} recorded \
                 actions would have been refused."
            )
        } else {
            format!(
                "If these rules had been applied, {refused_total} of the last {considered} \
                 recorded actions would have been refused."
            )
        };
        refused.truncate(PREVIEW_DETAIL_LIMIT);
        // An empty draft refuses everything by definition. Listing what it would refuse turns one
        // explainable sentence into a wall of alarming detail about work that was never at risk.
        if candidate.rules == 0 {
            refused.clear();
        }
        Ok(PolicyPreview {
            considered,
            refused_total,
            refused,
            summary,
        })
    }

    /// Apply a user policy only if the rules on disk still have the tag the caller read.
    ///
    /// The check and the write happen under one lock.
    pub fn apply_user_policy_if(
        &self,
        document: &str,
        expected_etag: &str,
    ) -> Result<WorkbenchIntentResult, PolicyWriteError> {
        let _writing = lock(&self.policy_writes);
        self.require_policy_tag(expected_etag)?;
        Ok(self.apply_policy_unguarded(document)?)
    }

    fn apply_policy_unguarded(
        &self,
        document: &str,
    ) -> Result<WorkbenchIntentResult, AuthoringError> {
        let policy = self.governance.apply_user_policy(document)?;
        self.live.policy_changed();
        Ok(self.policy_result(format!(
            "Your rules are applied: {} {}.",
            policy.name, policy.version
        )))
    }

    /// Remove the user policy only if the rules on disk still have the tag the caller read.
    pub fn remove_user_policy_if(
        &self,
        expected_etag: &str,
    ) -> Result<WorkbenchIntentResult, PolicyWriteError> {
        let _writing = lock(&self.policy_writes);
        self.require_policy_tag(expected_etag)?;
        Ok(self.remove_policy_unguarded()?)
    }

    fn remove_policy_unguarded(&self) -> Result<WorkbenchIntentResult, AuthoringError> {
        self.governance.remove_user_policy()?;
        self.live.policy_changed();
        Ok(self.policy_result("Your rules are removed.".to_owned()))
    }

    fn require_policy_tag(&self, expected_etag: &str) -> Result<(), PolicyWriteError> {
        if parts::policy_etag(&self.governance.effective_authority()) == expected_etag {
            Ok(())
        } else {
            Err(PolicyWriteError::Stale)
        }
    }

    /// Report one applied policy change the same way every other workbench mutation is reported.
    fn policy_result(&self, message: String) -> WorkbenchIntentResult {
        let state = self.governance.runtime_state();
        WorkbenchIntentResult {
            accepted: true,
            runtime_state: state,
            message,
        }
    }

    /// Re-check supported development harnesses without changing their configuration, and look
    /// for the agents' icons again in the background.
    pub fn refresh_harnesses(&self) -> Result<Vec<HarnessSummary>, HarnessError> {
        let summaries = self.harnesses.refresh()?;
        self.integrations_changed(&summaries);
        self.refresh_icons();
        Ok(summaries)
    }

    /// Tell every screen that the roster of clients was looked at again, with its tag. The setup
    /// part lists the same clients, so it is looked at again too.
    fn integrations_changed(&self, summaries: &[HarnessSummary]) {
        self.projection.publish(WorkbenchChange::PartChanged {
            part: parts::HintedPart::Integrations,
            etag: parts::json_etag(&summaries),
        });
        self.live.setup_changed();
    }

    /// Tell every screen that an act changed the roster of clients.
    fn roster_changed(&self) {
        self.integrations_changed(&self.harnesses.summaries());
    }

    /// The last complete check of the supported clients, without scanning again.
    #[must_use]
    pub fn cached_harnesses(&self) -> Vec<HarnessSummary> {
        self.harnesses.summaries()
    }

    /// Apply one client act only if the client is still in the state the caller drew.
    ///
    /// The expected state is its wire token, for example installed.
    pub fn manage_harness_expecting(
        &self,
        id: &str,
        action: HarnessAction,
        expected_state: &str,
    ) -> Result<HarnessActionResult, HarnessActError> {
        let current = self
            .harnesses
            .summaries()
            .into_iter()
            .find(|summary| summary.id == id)
            .map(|summary| serde_json::to_value(summary.state));
        if !matches!(current, Some(Ok(state)) if state == expected_state) {
            return Err(HarnessActError::Stale);
        }
        let result = self.harnesses.apply(id, action)?;
        self.roster_changed();
        Ok(result)
    }

    /// Set up and update every detected harness target Ghostlight can change safely.
    pub fn setup_detected_harnesses(&self) -> Result<HarnessSetupResult, HarnessError> {
        let result = self.harnesses.setup_detected()?;
        self.roster_changed();
        Ok(result)
    }

    /// Validate and remember one user-located harness executable or configuration.
    pub fn locate_harness(
        &self,
        id: &str,
        path: &Path,
    ) -> Result<HarnessActionResult, HarnessError> {
        let result = self.harnesses.locate(id, path)?;
        self.roster_changed();
        Ok(result)
    }

    /// Exercise the same best-effort notification port used for important domain facts.
    pub fn test_notification(&self) -> Result<(), WorkbenchError> {
        let port = lock(&self.projection.presentation)
            .clone()
            .ok_or(WorkbenchError::PresentationUnavailable)?;
        port.notify(WorkbenchNotification {
            kind: NotificationKind::Checkup,
            title: words::NOTICE_CHECKUP_TITLE.into(),
            body: words::NOTICE_CHECKUP_BODY.into(),
        })?;
        Ok(())
    }

    /// Ask the attached native presentation adapter to reveal the existing workbench, at
    /// `address` when one is given and where the window last was when it is not.
    pub fn reveal(&self, address: Option<PageAddress>) -> Result<(), WorkbenchError> {
        let port = lock(&self.projection.presentation)
            .clone()
            .ok_or(WorkbenchError::PresentationUnavailable)?;
        port.reveal(address)?;
        Ok(())
    }
}

/// Best-effort operating-system presentation port.
pub trait WorkbenchPresentationPort: Send + Sync {
    /// Reveal and focus the existing native workbench surface, at `address` when one is given.
    fn reveal(&self, address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError>;

    /// Deliver one high-signal, content-free notification.
    fn notify(&self, notification: WorkbenchNotification)
        -> Result<(), WorkbenchPresentationError>;
}

/// What only the operating system can do for the workbench: open a link, open a folder, ask for
/// a file.
///
/// The service owns what to open and when; the adapter that implements this owns how this
/// computer does it. It is kept apart from [`WorkbenchPresentationPort`] so a presentation
/// adapter that only shows and notifies needs none of it.
pub trait WorkbenchNativeActs: Send + Sync {
    /// Open an address in the person's browser. The facade passes only `https://` addresses.
    fn open_url(&self, url: &str) -> Result<(), WorkbenchPresentationError>;

    /// Show a folder in the file manager.
    fn open_folder(&self, path: &Path) -> Result<(), WorkbenchPresentationError>;

    /// Ask the person to choose a file, under `title`. Answers `None` when they close the dialog.
    /// The dialog blocks the calling thread until the person answers.
    fn pick_file(&self, title: &str) -> Result<Option<PathBuf>, WorkbenchPresentationError>;
}

/// What ends the service when the person asks for it.
///
/// The service attaches it, so the workbench asks for a Quit without depending on how the service
/// ends.
pub trait WorkbenchQuitPort: Send + Sync {
    /// Begin a person's Quit. Returns at once; the service finishes what is in flight first.
    fn quit(&self);
}

/// What draws the tray when the service describes it differently.
///
/// The service attaches it, so the workbench says what the tray is without depending on how it
/// reaches the shell that draws it.
pub trait WorkbenchTrayPort: Send + Sync {
    /// The tray changed. This returns at once: it is called from inside the refresh of the live
    /// parts, so it must not wait on anything slow.
    fn tray_changed(&self, model: TrayModel);
}

/// The start every address the service opens has.
const SECURE_SCHEME: &str = "https://";

/// Why a native act did not happen.
#[derive(Debug, Error)]
pub enum NativeActError {
    /// No adapter is attached, or this computer refused.
    #[error(transparent)]
    Workbench(#[from] WorkbenchError),
    /// The client roster refused: an unknown product or a path that is not a client.
    #[error(transparent)]
    Harness(#[from] HarnessError),
    /// Process diagnostics are off, so there is no folder to show.
    #[error("{}", words::DIAGNOSTICS_OFF)]
    DiagnosticsOff,
    /// The address is not a secure one.
    #[error("{}", words::LINK_NOT_SECURE)]
    LinkNotSecure,
}

/// A refusal of the adapter, as the failure of the act.
fn native(error: WorkbenchPresentationError) -> NativeActError {
    WorkbenchError::from(error).into()
}

/// Best-effort outbound port for sequenced workbench changes.
///
/// Delivery failures are presentation failures. The adapter contains them, and they never reach
/// domain authority, governance, or completion truth.
pub trait WorkbenchEventSink: Send + Sync {
    /// Deliver one sequenced change to the attached presentation surface.
    fn publish(&self, event: WorkbenchEvent);
}

/// One sequenced workbench change fact for a disposable presentation surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkbenchEvent {
    /// Monotonic publication sequence.
    ///
    /// A surface that receives a sequence other than its last plus one has missed a change and
    /// must resynchronize from a fresh snapshot rather than trust its local cache.
    pub seq: u64,
    /// What changed.
    pub change: WorkbenchChange,
}

/// Closed vocabulary of workbench changes worth rendering without a full snapshot.
///
/// Operation lifetime is per-item because it drives live presentation. Collections that change
/// rarely stay snapshot-owned rather than growing a second authority here.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkbenchChange {
    /// Saving health changed independently of browser execution.
    AuditHealthChanged {
        health: crate::language::audit_health::AuditHealth,
    },
    /// Volatile document details available only to the local human.
    DocumentCoverageChanged {
        details: crate::language::coverage::HumanCoverage,
    },
    /// One operation entered the live set.
    OperationStarted {
        /// The newly tracked operation.
        operation: OperationSummary,
    },
    /// One live operation changed activity or phase.
    OperationChanged {
        /// The operation in its current state.
        operation: OperationSummary,
    },
    /// One operation reached its terminal record and left the live set.
    OperationSettled {
        /// The content-minimized completion record.
        ///
        /// Boxed because a settled record carries far more than a live one, and every other
        /// variant would otherwise be padded to its size for the whole life of the channel. The
        /// published JSON is unchanged: a box serializes as the value it holds.
        record: Box<HistoryItem>,
    },
    /// A child receipt updated an existing composition without settling its parent.
    CompositionChanged { record: Box<HistoryItem> },
    /// Authoritative runtime control state changed.
    RuntimeChanged {
        /// The new runtime control state.
        runtime_state: RuntimeControlState,
    },
    /// The status part differs from the one last published.
    StatusChanged {
        /// The whole part, which replaces the one a surface holds.
        status: parts::StatusPart,
    },
    /// The connections part differs from the one last published.
    ConnectionsChanged {
        /// The whole part, which replaces the one a surface holds.
        connections: parts::ConnectionsPart,
    },
    /// The controls part differs from the one last published.
    ControlsChanged {
        /// The whole part as the window sees it, which replaces the one a surface holds. Each
        /// context is shown its own form when the change is projected. Boxed because the part is
        /// far larger than most changes, and the published JSON is unchanged.
        controls: Box<parts::ControlsPart>,
    },
    /// A part that is read on demand changed: read it again if you show it.
    ///
    /// The tag lets a surface skip a change it has already read.
    PartChanged {
        /// Which part changed.
        part: parts::HintedPart,
        /// The tag of the part now.
        etag: String,
    },
}

impl WorkbenchChange {
    /// Whether this change can alter what the status and connections parts say.
    ///
    /// The two changes that carry those parts are not among them: a refresh never causes one.
    const fn refreshes_live_parts(&self) -> bool {
        matches!(
            self,
            Self::AuditHealthChanged { .. }
                | Self::OperationStarted { .. }
                | Self::OperationChanged { .. }
                | Self::OperationSettled { .. }
        )
    }
}

/// Where a runtime control came from, so every screen can say who paused the agents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSource {
    /// The Ghostlight window.
    Window,
    /// A loopback request without the window's key while watching is on: a watching page or any
    /// local program, since nothing proves which.
    Watcher,
    /// The tray menu.
    Tray,
    /// The browser toolbar.
    Browser,
}

impl RuntimeSource {
    /// The sentence that says a pause came from here.
    const fn paused_note(self) -> &'static str {
        match self {
            Self::Window => words::PAUSED_IN_WINDOW,
            Self::Watcher => words::PAUSED_FROM_WATCHER,
            Self::Tray => words::PAUSED_FROM_TRAY,
            Self::Browser => words::PAUSED_FROM_BROWSER,
        }
    }
}

/// One content-free notification decision made by the orchestrator.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkbenchNotification {
    /// Semantic reason for the notification.
    pub kind: NotificationKind,
    /// Fixed Ghostlight-authored title.
    pub title: String,
    /// Fixed Ghostlight-authored body.
    pub body: String,
}

/// Closed notification reasons.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// Governance blocked work.
    Blocked,
    /// The user explicitly tested notifications.
    Checkup,
}

/// Failure of the replaceable operating-system presentation adapter.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WorkbenchPresentationError {
    /// Native notification delivery failed.
    #[error("native presentation failed: {0}")]
    Native(String),
    /// The computer refused an open or a dialog. The text is the computer's own.
    #[error("{0}")]
    ActFailed(String),
    /// There is no window program to carry the request out, or it went away before it answered.
    #[error("native presentation is unavailable")]
    Unavailable,
}

/// Failure at the typed workbench application boundary.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WorkbenchError {
    /// No native presentation adapter is currently attached.
    #[error("native presentation is unavailable")]
    PresentationUnavailable,
    /// Native presentation rejected the request.
    #[error(transparent)]
    Presentation(WorkbenchPresentationError),
    /// No service is attached to end.
    #[error("{}", words::QUIT_UNAVAILABLE)]
    QuitUnavailable,
    /// A setup step names a browser or profile this computer no longer offers, or is not a step
    /// the service performs.
    #[error("{}", words::SETUP_STEP_NOT_OFFERED)]
    SetupStepNotOffered,
    /// A setup step would start a process while the session is ending.
    #[error("{}", words::SETUP_SESSION_ENDING)]
    SetupSessionEnding,
    /// The computer would not start the browser a setup step names.
    #[error("{}", words::SETUP_BROWSER_NOT_STARTED)]
    SetupBrowserNotStarted,
}

impl From<WorkbenchPresentationError> for WorkbenchError {
    /// A presentation adapter with nothing to talk to is the facade's own unavailable failure, so
    /// a person reads one failure however the adapter came to be missing.
    fn from(error: WorkbenchPresentationError) -> Self {
        match error {
            WorkbenchPresentationError::Unavailable => Self::PresentationUnavailable,
            other => Self::Presentation(other),
        }
    }
}

/// Complete immutable workbench read model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkbenchSnapshot {
    /// Current storage health and explicit historical gaps.
    pub audit_health: crate::language::audit_health::AuditHealth,
    /// Persistent human notice, empty during ordinary healthy operation.
    pub audit_notice: String,
    /// Volatile human-only embedded-host explanations, absent from model and audit surfaces.
    pub document_coverage: Vec<crate::language::coverage::HumanCoverage>,
    /// Projection sequence this snapshot reflects.
    ///
    /// A surface applies a later change only when its sequence follows this one.
    pub seq: u64,
    /// Time at which this snapshot was assembled.
    pub generated_at_ms: u64,
    /// Service identity and control state.
    pub service: ServiceSummary,
    /// The one aggregate answer the front door leads with.
    pub readiness: ReadinessSummary,
    /// Where the pause came from, present only while agents are paused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_note: Option<String>,
    /// At-a-glance counts.
    pub overview: OverviewSummary,
    /// Admitted MCP sessions.
    pub sessions: Vec<SessionSummary>,
    /// Currently tracked operations.
    pub operations: Vec<OperationSummary>,
    /// Currently connected browser instances.
    pub browsers: Vec<BrowserInstanceSummary>,
    /// What the window says in place of a browser while none is connected
    /// ([`words::no_browser`]): the browser a request would start, or that none is connected.
    pub no_browser: Option<String>,
    /// Newest-first bounded content-minimized history.
    pub history: Vec<HistoryItem>,
    /// Current local checkup results.
    pub diagnostics: Vec<DiagnosticItem>,
    /// Cached, explicitly supported development-harness registrations.
    pub harnesses: Vec<HarnessSummary>,
    /// Content-free authority and runtime configuration facts.
    pub configuration: ConfigurationSummary,
    /// The controls part Settings is drawn from, as the window sees it.
    pub controls: parts::ControlsPart,
}

/// Current service facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ServiceSummary {
    /// Product version.
    pub version: String,
    /// Process-local service start time.
    pub started_at_ms: u64,
    /// Authoritative runtime control state.
    pub runtime_state: RuntimeControlState,
}

/// The aggregate answer plus the words for it, authored once by the orchestrator.
///
/// The surface renders these fields. It does not decide which state holds, and it does not author
/// the word: a window that invents its own readiness vocabulary is a second source of truth about
/// the one thing that must have exactly one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReadinessSummary {
    /// The closed state.
    pub state: readiness::Readiness,
    /// The word to show.
    pub word: String,
    /// One sentence explaining the word.
    pub detail: String,
    /// Presentation tone, so the surface styles without classifying.
    pub tone: String,
    /// Whether the front door should offer runtime control here.
    ///
    /// Runtime control is this authority's own state, so a reachable authority always offers it,
    /// with or without a connected browser.
    pub invites_control: bool,
}

impl ReadinessSummary {
    /// Resolve the answer and carry its words.
    #[must_use]
    pub fn resolve(facts: &readiness::ReadinessFacts<'_>) -> Self {
        let state = readiness::resolve(facts);
        Self {
            state,
            word: state.word().to_owned(),
            detail: readiness::explain(state, facts),
            tone: state.tone().to_owned(),
            invites_control: state.invites_control(),
        }
    }
}

/// At-a-glance counts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OverviewSummary {
    /// Admitted sessions.
    pub active_sessions: usize,
    /// Current operations.
    pub active_operations: usize,
    /// Connected browser adapters.
    pub connected_browsers: usize,
    /// Bounded history records whose final boundary was not allowed.
    pub blocked_in_history: usize,
}

/// One admitted MCP session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SessionSummary {
    /// Opaque workspace identity.
    pub id: String,
    /// The agent of the connection that opened the session, wherever the window names the
    /// session. A later connection renames nothing.
    pub agent: crate::agent::Agent,
    /// Each current connection has its own observations and reported application.
    pub connections: Vec<crate::provenance::ConnectionDetails>,
    /// The intake of the connection that opened the session, so live work can be attributed
    /// before it settles.
    pub channel: IntakeChannel,
    /// Controlled tab count.
    pub tab_count: usize,
    /// How many of the session's operations are running now. Work waiting its turn or held by a
    /// pause is not counted: only running work is shown as working.
    pub running_operations: usize,
    /// Persistent browser family or product name this session's workspace is pinned to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
}

/// One current orchestrator operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OperationSummary {
    /// Opaque invocation identity.
    pub invocation: String,
    /// Owning workspace identity.
    pub workspace: String,
    /// Catalog tool name.
    pub tool: String,
    /// Fixed presentation activity name.
    pub activity: String,
    /// Compact label for the complete governed requirement set.
    pub capability: String,
    /// Local start time.
    pub started_at_ms: Option<u64>,
    /// Current semantic phase.
    pub phase: OperationPhase,
    /// Evidence captured for this operation at intake.
    pub provenance: Option<crate::provenance::ConnectionDetails>,
    /// The operation's agent, as its connection details name it
    /// ([`crate::provenance::ConnectionDetails::agent`]); the unknown agent when it kept none.
    pub agent: crate::agent::Agent,
}

impl From<&OperationState> for OperationSummary {
    fn from(value: &OperationState) -> Self {
        Self {
            invocation: value.invocation.clone(),
            workspace: value.workspace.clone(),
            tool: value.tool.clone(),
            activity: if value.phase == OperationPhase::Waiting {
                crate::language::outcome::WAITING_FOR_WORK.into()
            } else {
                activity_label(value.activity).into()
            },
            capability: value.capabilities.label(),
            started_at_ms: Some(value.started_at_ms),
            phase: value.phase,
            agent: value.provenance.agent.clone(),
            provenance: Some(value.provenance.clone()),
        }
    }
}

/// Closed current-operation phases.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationPhase {
    /// Admitted work is waiting within its original deadline.
    Waiting,
    /// Work is active.
    Running,
    /// Runtime governance held work.
    Held,
    /// Work terminated as blocked before its audit projection arrived.
    Blocked,
    /// Work completed before its audit projection arrived.
    Completed,
}

impl OperationPhase {
    /// The word the search palette says for where the work stands.
    const fn word(self) -> &'static str {
        match self {
            Self::Waiting => words::SEARCH_WAITING,
            Self::Running => words::SEARCH_RUNNING,
            Self::Held => words::SEARCH_HELD,
            Self::Blocked => words::SEARCH_BLOCKED,
            Self::Completed => words::SEARCH_FINISHED,
        }
    }
}

/// One connected browser adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserInstanceSummary {
    /// Persistent opaque adapter installation identity.
    pub id: String,
    /// User-facing browser family supported by the current adapter contract.
    pub family: String,
    /// Adapter version if supplied by the connection.
    pub adapter_version: Option<String>,
    /// Current connection state.
    pub connected: bool,
    /// The tabs the sessions working in this browser control, together.
    pub tab_count: usize,
}

/// One content-minimized terminal history record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HistoryItem {
    /// Storage confirmation is separate from execution and receipt presence.
    pub storage: crate::language::audit_health::Storage,
    /// Authored human explanation for unconfirmed storage.
    pub storage_detail: String,
    /// Completion time.
    pub timestamp_ms: u64,
    /// Opaque invocation identity.
    pub invocation: String,
    /// Opaque workspace identity.
    pub workspace: String,
    /// Catalog tool name.
    pub tool: String,
    /// Requested capability.
    pub capability: String,
    /// The exact independent requirement set, kept so a policy preview can decide this record
    /// again offline rather than parsing its own label back.
    #[serde(skip)]
    pub requirements: CapabilitySet,
    /// Whether authority admitted the final boundary.
    pub allowed: bool,
    /// Stable reason code.
    pub reason: String,
    /// Tighten-only tier that supplied the deciding rule, when policy decided this.
    pub policy_tier: Option<String>,
    /// Authored rule that supplied the decision, when policy decided this.
    pub grant_id: Option<String>,
    /// Deterministic denial handle, so a refusal in the window and a line in audit are the same
    /// event to whoever has to ask about it.
    pub denial_id: Option<String>,
    /// Terminal status.
    pub status: String,
    /// What the call came to, decided once from the typed record
    /// ([`crate::language::history::OutcomeClass::recorded`]). The window draws the row from it
    /// and never from `allowed` and `status`.
    pub outcome: crate::language::history::OutcomeClass,
    /// Effect class.
    pub effect: String,
    /// Ghostlight-authored sentence naming what happened, as a person reads it: the record's
    /// summary, or the person's words for one that speaks to the model
    /// ([`crate::language::history::for_the_person`]).
    pub summary: String,
    /// How long the work took, in milliseconds.
    pub duration_ms: u64,
    /// What the action did, as stated by the orchestrator's outcome language.
    pub observed: Observed,
    /// Which intake the work arrived on, when the workspace was still known.
    pub channel: Option<IntakeChannel>,
    /// Original connection evidence; claimed names are retained only in bounded live history.
    pub provenance: Option<crate::provenance::ConnectionDetails>,
    /// The work's agent, as its connection details name it
    /// ([`crate::provenance::ConnectionDetails::agent`]); the unknown agent when it kept none.
    pub agent: crate::agent::Agent,
    /// Persistent browser family or product name that performed this work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    /// Whether a terminal parent/direct receipt was recorded.
    pub complete: bool,
    /// The same safe composition account retained in audit.
    pub composition: Option<crate::language::composition::CompositionProgress>,
    /// One bounded level of step detail; missing records stay explicit.
    pub steps: Vec<HistoryStep>,
    /// Actual authority evidence, never reconstructed using current policy.
    pub permissions: crate::governance::evidence::PermissionTrace,
    /// Language-owned explanations for the captured checks.
    pub permission_explanations: Vec<String>,
}

impl From<AuditRecord> for HistoryItem {
    fn from(value: AuditRecord) -> Self {
        let outcome = crate::language::history::OutcomeClass::recorded(&value);
        let provenance = value
            .provenance
            .as_ref()
            .map(|evidence| evidence.details())
            .or_else(|| {
                value
                    .channel
                    .map(|channel| crate::provenance::Attribution::legacy(channel).details())
            });
        let requirements = value.capabilities;
        let capability = requirements.label();
        // A record that kept no channel arrived on the default one: an undeclared channel is MCP.
        let channel = value
            .channel
            .or_else(|| provenance.as_ref().map(|details| details.channel))
            .unwrap_or_default();
        let tier = value.policy_tier.as_deref();
        let permission_explanations = value
            .permissions
            .checks
            .iter()
            .map(|check| crate::language::history::permission(check, channel, tier))
            .collect();
        let summary = crate::language::history::for_the_person(
            &value.summary,
            value.reason,
            value.refusal_facts.as_ref(),
            channel,
            tier,
        )
        .unwrap_or(value.summary);
        Self {
            storage: crate::language::audit_health::Storage::Saved,
            storage_detail: if value.unconfirmed_history_steps > 0 {
                "Some step history could not be saved.".into()
            } else {
                String::new()
            },
            complete: true,
            composition: value.composition,
            steps: vec![],
            permissions: value.permissions,
            permission_explanations,
            requirements,
            timestamp_ms: value.timestamp_ms,
            invocation: value.invocation,
            workspace: value.workspace,
            tool: value.tool,
            capability,
            allowed: value.allowed,
            reason: value.reason.as_str().into(),
            policy_tier: value.policy_tier,
            grant_id: value.grant_id,
            denial_id: value.denial_id,
            status: value.status,
            outcome,
            effect: value.effect,
            summary,
            duration_ms: value.duration_ms,
            observed: value.observed,
            channel: value.channel,
            agent: provenance
                .as_ref()
                .map_or_else(crate::agent::Agent::unknown, |details| {
                    details.agent.clone()
                }),
            provenance,
            browser: value.browser,
        }
    }
}

/// One local checkup result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosticItem {
    /// Stable diagnostic identity.
    pub id: String,
    /// Fixed label.
    pub label: String,
    /// Current severity.
    pub severity: DiagnosticSeverity,
    /// Fixed content-free explanation.
    pub detail: String,
}

impl DiagnosticItem {
    fn passing(id: &str, label: &str, detail: &str) -> Self {
        Self::new(id, label, DiagnosticSeverity::Passing, detail)
    }

    fn warning(id: &str, label: &str, detail: &str) -> Self {
        Self::new(id, label, DiagnosticSeverity::Warning, detail)
    }

    fn failing(id: &str, label: &str, detail: &str) -> Self {
        Self::new(id, label, DiagnosticSeverity::Failing, detail)
    }

    fn new(id: &str, label: &str, severity: DiagnosticSeverity, detail: &str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            severity,
            detail: detail.into(),
        }
    }
}

/// Closed diagnostic severities.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// Healthy.
    Passing,
    /// Usable with something awaiting attention.
    Warning,
    /// Fail-closed or unavailable.
    Failing,
}

/// Content-free configuration state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConfigurationSummary {
    /// Current runtime state.
    pub runtime_state: RuntimeControlState,
    /// Whether local authority is configured.
    pub local_policy_configured: bool,
    /// Whether a validated local policy is currently applied.
    pub local_policy_active: bool,
    /// Whether local authority is valid.
    pub local_policy_valid: bool,
    /// Whether managed authority is configured.
    pub managed_authority_configured: bool,
    /// Whether a verified managed policy is currently applied.
    pub managed_authority_active: bool,
    /// Whether managed authority is valid.
    pub managed_authority_valid: bool,
    /// Signed managed-policy provenance suitable for the Policy Passport.
    pub managed_policy: ManagedPolicyPassport,
    /// The band chip, authored by the orchestrator so the surface renders rather than derives it.
    pub policy: PolicyChip,
}

/// Every place the workbench is willing to send someone.
///
/// The surface names a destination; the orchestrator owns the address. A closed vocabulary is the
/// difference between "the workbench can open a link" and "the workbench can open any URL": there
/// is no free string to smuggle a destination through, so a page that is not on this list cannot
/// be reached from here however the surface is coaxed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchDestination {
    /// The Ghostlight project page.
    Home,
    /// The live demo stage.
    Demo,
    /// The guided "which mode fits me" aid.
    DecisionAid,
    /// The source repository.
    Source,
    /// Installing Ghostlight and connecting a client.
    InstallGuide,
    /// Writing and applying a governance manifest.
    GovernanceGuide,
    /// Driving Ghostlight from a script or shell.
    ScriptingGuide,
    /// Shipping the audit trail into a SIEM.
    AuditGuide,
    /// What the product promises, and how each claim is checked.
    TrustCenter,
    /// Every architectural decision, and why it was taken.
    DecisionRecords,
    /// The licensing split in plain language.
    LicensingGuide,
    /// The rest of the Sylin toolkit.
    SylinTools,
    /// Google's download page for Chrome, which the setup view offers a person with no browser
    /// Ghostlight supports.
    GetChrome,
}

/// The branch of the source repository that holds the documentation of the published product.
/// A person running a release reads the documents of the product they run, and `main` is that
/// branch from the cutover on.
macro_rules! documentation_branch {
    () => {
        "main"
    };
}

/// One document of the source repository, on the documentation branch.
macro_rules! repository_document {
    ($path:literal) => {
        concat!(
            "https://github.com/sylin-org/ghostlight/blob/",
            documentation_branch!(),
            "/",
            $path
        )
    };
}

impl WorkbenchDestination {
    /// Resolve the exact address. Every one of these was checked to resolve when it was added.
    ///
    /// Documentation links name the branch of `documentation_branch!`, so a release shows the
    /// documents of the product it is.
    #[must_use]
    pub const fn url(self) -> &'static str {
        match self {
            Self::Home => "https://sylin.org/ghostlight/",
            Self::Demo => "https://sylin.org/ghostlight/demo/",
            Self::DecisionAid => "https://sylin.org/ghostlight/decision-aid/",
            Self::Source => "https://github.com/sylin-org/ghostlight",
            Self::InstallGuide => {
                repository_document!("docs/guides/installation.md")
            }
            Self::GovernanceGuide => {
                repository_document!("docs/guides/governance-configuration.md")
            }
            Self::ScriptingGuide => {
                repository_document!("docs/guides/scripting.md")
            }
            Self::AuditGuide => {
                repository_document!("docs/guides/siem-integration.md")
            }
            Self::TrustCenter => {
                repository_document!("docs/trust/README.md")
            }
            Self::DecisionRecords => {
                repository_document!("docs/adr/README.md")
            }
            Self::LicensingGuide => {
                repository_document!("docs/guides/licensing.md")
            }
            Self::SylinTools => "https://sylin.org/tools/",
            Self::GetChrome => "https://www.google.com/chrome/",
        }
    }

    /// The full vocabulary, so a new destination cannot be added without the guards seeing it.
    #[must_use]
    pub const fn all() -> [Self; 13] {
        [
            Self::Home,
            Self::Demo,
            Self::DecisionAid,
            Self::Source,
            Self::InstallGuide,
            Self::GovernanceGuide,
            Self::ScriptingGuide,
            Self::AuditGuide,
            Self::TrustCenter,
            Self::DecisionRecords,
            Self::LicensingGuide,
            Self::SylinTools,
            Self::GetChrome,
        ]
    }

    /// The token the surface uses to name this destination.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Demo => "demo",
            Self::DecisionAid => "decision_aid",
            Self::Source => "source",
            Self::InstallGuide => "install_guide",
            Self::GovernanceGuide => "governance_guide",
            Self::ScriptingGuide => "scripting_guide",
            Self::AuditGuide => "audit_guide",
            Self::TrustCenter => "trust_center",
            Self::DecisionRecords => "decision_records",
            Self::LicensingGuide => "licensing_guide",
            Self::SylinTools => "sylin_tools",
            Self::GetChrome => "get_chrome",
        }
    }
}

/// Explicit user-facing runtime actions.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchRuntimeIntent {
    /// Hold later effects.
    Hold,
    /// Resume held work.
    Resume,
    /// End the current runtime session.
    EndSession,
    /// Start a fresh active runtime session.
    StartSession,
}

impl From<WorkbenchRuntimeIntent> for RuntimeControlIntent {
    fn from(value: WorkbenchRuntimeIntent) -> Self {
        match value {
            WorkbenchRuntimeIntent::Hold => Self::Hold,
            WorkbenchRuntimeIntent::Resume => Self::Resume,
            WorkbenchRuntimeIntent::EndSession => Self::EndSession,
            WorkbenchRuntimeIntent::StartSession => Self::StartSession,
        }
    }
}

/// Definite result of one local-human intent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkbenchIntentResult {
    /// Whether the orchestrator accepted the intent.
    pub accepted: bool,
    /// Resulting authoritative runtime state.
    pub runtime_state: RuntimeControlState,
    /// Fixed user-facing outcome.
    pub message: String,
}

/// One search result, worded for the person who searched (`crate::language::workbench`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SearchHit {
    /// Result category.
    pub kind: SearchKind,
    /// Opaque result identity: for a recorded action its invocation, so the window can open its
    /// row.
    pub id: String,
    /// What the hit is, in a person's words.
    pub title: String,
    /// The second line, in a person's words.
    pub detail: String,
    /// When a recorded action settled; the window says how long ago.
    pub timestamp_ms: Option<u64>,
    /// How a recorded action ended, present only when its row says so in a word
    /// ([`words::search_outcome`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<crate::language::history::OutcomeClass>,
    /// Workbench destination that owns this result.
    pub view: SearchDestination,
}

impl SearchHit {
    fn new(
        kind: SearchKind,
        id: &str,
        title: &str,
        detail: String,
        view: SearchDestination,
    ) -> Self {
        Self {
            kind,
            id: id.to_owned(),
            title: title.to_owned(),
            detail,
            timestamp_ms: None,
            outcome: None,
            view,
        }
    }
}

/// Closed search-result categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    /// A first-class workbench destination.
    Destination,
    /// Current operation.
    Operation,
    /// Admitted session.
    Session,
    /// Terminal history record.
    History,
    /// Connected browser instance.
    Browser,
    /// Checkup finding.
    Diagnostic,
    /// Runtime or authority configuration.
    Configuration,
    /// Supported development-harness registration.
    Installation,
}

/// Closed workbench destinations used by global search.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchDestination {
    /// At-a-glance home.
    Home,
    /// Sessions, operations, and browser instances.
    Activity,
    /// Completed work.
    History,
    /// Local diagnostics.
    Checkup,
    /// The rules that apply here, on the Policy page.
    Configuration,
    /// Settings, as the person last left it.
    Settings,
    /// The browser-work section of Settings, which shows the runtime control.
    BrowserWork,
    /// Supported harness registrations.
    Install,
}

/// The search identity of the browser-work setting.
const RUNTIME_SETTING_ID: &str = "runtime-control";

/// The hits of one search: what matches `query`, up to [`SEARCH_LIMIT`].
struct Hits<'a> {
    query: &'a str,
    found: Vec<SearchHit>,
}

impl<'a> Hits<'a> {
    const fn new(query: &'a str) -> Self {
        Self {
            query,
            found: Vec::new(),
        }
    }

    /// Whether there is room for another hit and one of `fields` holds the query, ignoring ASCII
    /// case. The query is already lower case.
    fn finds(&self, fields: &[&str]) -> bool {
        self.found.len() < SEARCH_LIMIT
            && fields
                .iter()
                .any(|field| holds_folded(field, self.query.as_bytes()))
    }

    /// Whether there is room for another hit and the catalog `tool` or one of `fields` holds the
    /// query. A tool matches by its name or by its words, so "fill form" and "browser_fill_form"
    /// both find `browser_fill_form`.
    fn finds_action(&self, tool: &str, fields: &[&str]) -> bool {
        self.finds(fields)
            || (self.found.len() < SEARCH_LIMIT && holds_tool(tool, self.query.as_bytes()))
    }

    fn push(&mut self, hit: SearchHit) {
        self.found.push(hit);
    }
}

/// Whether the catalog name `tool` holds `needle`, reading an underscore and a space as the same
/// character, so the tool's words match its name ([`words::tool_words`]).
fn holds_tool(tool: &str, needle: &[u8]) -> bool {
    let same = |left: u8, right: u8| {
        left.eq_ignore_ascii_case(&right) || matches!((left, right), (b'_', b' ') | (b' ', b'_'))
    };
    tool.as_bytes().windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(&left, &right)| same(left, right))
    })
}

/// Whether `field` holds `needle`, comparing ASCII letters without case and allocating nothing.
fn holds_folded(field: &str, needle: &[u8]) -> bool {
    field
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn push_bounded(history: &mut VecDeque<HistoryItem>, item: HistoryItem) {
    if history.len() == HISTORY_LIMIT {
        history.pop_front();
    }
    history.push_back(item);
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn activity_label(activity: PresentationActivity) -> &'static str {
    match activity {
        PresentationActivity::Quiet => "Ghostlight",
        PresentationActivity::Navigate => "Navigating",
        PresentationActivity::Click => "Clicking",
        PresentationActivity::Hover => "Hovering",
        PresentationActivity::Drag => "Dragging",
        PresentationActivity::Type => "Typing",
        PresentationActivity::Key => "Keyboard",
        PresentationActivity::Scroll => "Scrolling",
        PresentationActivity::Read => "Reading page",
        PresentationActivity::Find => "Finding on page",
        PresentationActivity::Screenshot => "Screenshot",
        PresentationActivity::Zoom => "Zooming",
        PresentationActivity::Fill => "Filling form",
        PresentationActivity::Upload => "Uploading file",
        PresentationActivity::Script => "Running JavaScript",
        PresentationActivity::Wait => "Waiting",
        PresentationActivity::Dialog => "Browser dialog",
    }
}

/// The word Settings says for the browser-work state.
const fn runtime_state_word(state: RuntimeControlState) -> &'static str {
    match state {
        RuntimeControlState::Active => words::CONTROLS_RUNTIME_ACTIVE,
        RuntimeControlState::Held => words::CONTROLS_RUNTIME_HELD,
        RuntimeControlState::Ended => words::CONTROLS_RUNTIME_ENDED,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
