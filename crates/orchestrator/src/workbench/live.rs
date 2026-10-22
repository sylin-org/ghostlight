//! The live parts of the workbench: what a screen shows the moment something connects, pauses or
//! starts working.
//!
//! The status, connections, controls and setup parts change without any act of a screen. A pause from the
//! tray or the browser toolbar, a session that opens, a browser that goes away, process diagnostics
//! turned on from a terminal: none of them is a domain event of the work projection. This module
//! builds those parts the light way (no history), remembers the last ones it published, and
//! publishes a part only when it differs from that. Every caller that knows something may have
//! changed asks for a refresh and nothing more (ADR-0199, amendment of 2026-10-15).
//!
//! The setup part is read on demand like the integrations: a look tells screens it changed with a
//! hint that carries its tag. It is built again only when a browser connects or leaves, when a
//! session's client arrives or leaves, or when a caller says setup changed (the client roster was
//! looked at again, or setup completed), because an incomplete setup reads the machine each time
//! it is built (ADR-0200 Decision 8).
//!
//! The connections part also names the client roster's rows in use, read from the cached roster
//! beside the sessions, so the Integrations page learns it with the sessions it is decided from.
//!
//! A screen's read is one [`Look`]: the sequence it reflects and one gather, from which each part
//! it asks for is built when asked, so every part of one answer describes one moment. A gather
//! reads the published authority once, so the status and connections parts it builds describe
//! one publication.
//!
//! While no browser is connected, the connections part says which browser a request would start.
//! Browser recovery answers that from the installed browsers, which it reads once for each stretch
//! with no browser connected and each browser startup posture in it, and never under `manual`. The
//! read happens outside every lock, so no snapshot waits on another's; two snapshots that miss at
//! once may each read them.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, TryLockError};

use super::parts::{
    self, ActivityPart, ConnectionsPart, ControlsFacts, ControlsPart, HintedPart, OrganizationView,
    StatusConfiguration, StatusPart, WatchingFacts,
};
use super::setup::{self, SetupInputs, SetupMachine, ThisComputer};
use super::{
    BrowserInstanceSummary, DiagnosticId, DiagnosticItem, OperationPhase, OperationSummary,
    ReadinessSummary, RuntimeSource, ServiceSummary, SessionSummary, WebFacts, WorkbenchChange,
    WorkbenchError, WorkbenchProjection, WorkbenchTrayPort,
};
use crate::browser::adapter::RuntimeControlState;
use crate::browser::recovery::BrowserRecovery;
use crate::browser::BrowserPort;
use crate::diagnostics::DiagnosticsHub;
use crate::governance::effective::PolicyChip;
use crate::governance::manifest::BrowserStartup;
use crate::governance::{AuthoritySnapshot, GovernanceDiagnostics, GovernanceFacade};
use crate::install::{HarnessRegistry, HarnessSummary};
use crate::intake::desktop::TrayModel;
use crate::intake::session::IntakeChannel;
use crate::language::audit_health::AuditHealth;
use crate::language::readiness;
use crate::language::setup::{SetupAction, SetupView};
use crate::language::workbench::{self as words, ConnectionCounts};
use crate::provenance::ConnectionDetails;
use crate::sync::lock;
use crate::workspace::WorkspaceStore;

/// What one look at the service says, gathered once so every part built from it agrees.
pub(super) struct Gathered {
    /// Operations tracked right now, newest first.
    pub operations: Vec<OperationSummary>,
    /// Admitted sessions.
    pub sessions: Vec<SessionSummary>,
    /// Connected browser instances.
    pub browsers: Vec<BrowserInstanceSummary>,
    /// What the window says in place of a browser, while none is connected.
    pub no_browser: Option<String>,
    /// The browser an agent's request would start, while none is connected and Ghostlight would
    /// start one.
    pub starts: Option<String>,
    /// The cached client roster, read once for the Integrations page, the rows in use and setup.
    pub harnesses: Vec<HarnessSummary>,
    /// The ids of the roster's rows these sessions put in use ([`parts::harnesses_in_use`]).
    pub harnesses_in_use: Vec<String>,
    /// Content-free authority facts.
    pub authority: GovernanceDiagnostics,
    /// The band chip.
    pub policy: PolicyChip,
    /// Whether work is held, ended or active.
    pub runtime_state: RuntimeControlState,
    /// Where the pause came from, while there is one.
    pub runtime_note: Option<String>,
    /// Saving health.
    pub audit_health: AuditHealth,
    /// Whether the authority requires receipts to be saved.
    pub audit_required: bool,
    /// The published authority this look read, the one the executor enforces per request.
    pub enforced: Arc<AuthoritySnapshot>,
    /// The one aggregate answer.
    pub readiness: ReadinessSummary,
}

impl Gathered {
    /// The connections part of this look.
    pub fn connections(&self) -> ConnectionsPart {
        ConnectionsPart {
            sessions: self.sessions.clone(),
            browsers: self.browsers.clone(),
            no_browser: self.no_browser.clone(),
            counts: self.counts(),
            harnesses_in_use: self.harnesses_in_use.clone(),
        }
    }

    /// What is connected in this look: the agents as At a glance groups them, the connections by
    /// intake, and the browsers, as [`count_connections`] counts them.
    pub fn counts(&self) -> ConnectionCounts {
        count_connections(
            self.sessions
                .iter()
                .map(|session| (session.agent.id.clone(), session.connections.as_slice())),
            self.browsers.len(),
        )
    }

    /// The Status page's checks of this look: the service, the browser, the rules in force and
    /// the history.
    pub fn checks(&self) -> Vec<DiagnosticItem> {
        let mut checks = vec![DiagnosticItem::passing(
            DiagnosticId::Service,
            words::STATUS_SERVICE_LABEL,
            words::STATUS_SERVICE_RUNNING,
        )];
        checks.push(if !self.browsers.is_empty() {
            let names: Vec<&str> = self
                .browsers
                .iter()
                .map(|browser| browser.family.as_str())
                .collect();
            DiagnosticItem::passing(
                DiagnosticId::Browser,
                words::STATUS_BROWSER_LABEL,
                &words::status_browser_connected(&names),
            )
        } else if let Some(browser) = &self.starts {
            DiagnosticItem::passing(
                DiagnosticId::Browser,
                words::STATUS_BROWSER_LABEL,
                &readiness::starts_when_asked(browser),
            )
        } else {
            DiagnosticItem::warning(
                DiagnosticId::Browser,
                words::STATUS_BROWSER_LABEL,
                words::STATUS_BROWSER_WAITING,
            )
        });
        // The same validity governance enforces, so this card, the header, and the Policy tab
        // agree. A failed reload that keeps the last valid policy in force is not failing closed.
        let authority = &self.authority;
        checks.push(if !authority.authority_valid {
            DiagnosticItem::failing(
                DiagnosticId::Authority,
                words::STATUS_RULES_LABEL,
                words::STATUS_RULES_INVALID,
            )
        } else if !authority.local_policy_valid {
            DiagnosticItem::warning(
                DiagnosticId::Authority,
                words::STATUS_RULES_LABEL,
                words::STATUS_RULES_EDIT_INVALID,
            )
        } else {
            DiagnosticItem::passing(
                DiagnosticId::Authority,
                words::STATUS_RULES_LABEL,
                &words::status_rules_in_force(
                    authority.local_policy_active,
                    authority.managed_authority_active,
                    self.enforced.channels_off(),
                ),
            )
        });
        let detail = self.audit_health.detail(self.audit_required);
        checks.push(if self.audit_health.quiet() {
            DiagnosticItem::passing(DiagnosticId::Audit, words::STATUS_HISTORY_LABEL, &detail)
        } else {
            DiagnosticItem::warning(DiagnosticId::Audit, words::STATUS_HISTORY_LABEL, &detail)
        });
        checks
    }
}

/// One read of the workbench a screen asked for: the sequence it reflects and one gather.
///
/// Each part is built from that gather only when it is asked for, so the parts of one answer agree
/// and a part nobody asked for costs nothing. The setup part may read this computer, so it is
/// built only for a request that names it.
pub struct Look<'a> {
    live: &'a Live,
    seq: u64,
    gathered: Gathered,
}

impl Look<'_> {
    /// The projection sequence this look reflects. A surface applies a later change only when its
    /// sequence follows this one.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }

    /// The status part.
    #[must_use]
    pub fn status(&self) -> StatusPart {
        self.live.status(&self.gathered)
    }

    /// The connections part.
    #[must_use]
    pub fn connections(&self) -> ConnectionsPart {
        self.gathered.connections()
    }

    /// The controls part, as the window sees it.
    #[must_use]
    pub fn controls(&self) -> ControlsPart {
        self.live.controls(&self.gathered)
    }

    /// The activity part: the work in flight this look saw, and the history and document coverage
    /// the projection holds now.
    #[must_use]
    pub fn activity(&self) -> ActivityPart {
        ActivityPart {
            operations: self.gathered.operations.clone(),
            history: self.live.projection.history(),
            document_coverage: self.live.projection.coverage(),
        }
    }

    /// The cached client roster this look read, which the Integrations page shows.
    #[must_use]
    pub fn harnesses(&self) -> &[HarnessSummary] {
        &self.gathered.harnesses
    }

    /// The Status page's checks.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<DiagnosticItem> {
        self.gathered.checks()
    }

    /// The setup part, as the window sees it. An unfinished setup reads this computer.
    #[must_use]
    pub fn setup(&self) -> SetupView {
        self.live.setup(&self.gathered)
    }

    /// What this look gathered.
    pub(super) const fn gathered(&self) -> &Gathered {
        &self.gathered
    }
}

/// The parts last published, which a refresh compares with.
#[derive(Default)]
struct Published {
    status: Option<StatusPart>,
    connections: Option<ConnectionsPart>,
    controls: Option<ControlsPart>,
    tray: Option<TrayModel>,
    /// What the setup part was last built for, so it is built again only when this changes.
    setup_inputs: Option<SetupInputsKey>,
    /// The tag of the setup part last announced.
    setup_etag: Option<String>,
}

/// What, among the things a look sees, decides whether the setup part is built again: which
/// browsers are connected, which clients have sessions, and whether setup is complete.
#[derive(Debug, Eq, PartialEq)]
struct SetupInputsKey {
    browsers: Vec<String>,
    clients: Vec<String>,
    complete: bool,
}

/// What the facade remembers between looks.
#[derive(Default)]
struct Remembered {
    /// The state the runtime last reached through the facade, and who brought it there.
    runtime: Option<(RuntimeControlState, RuntimeSource)>,
    /// The authority id a policy hint was last published for.
    authority: String,
    /// The name of the agent that finished setup, kept once it is known because its session may
    /// end before the person has read it.
    agent_name: Option<String>,
}

/// What the setup part is read from besides the look itself.
#[derive(Clone)]
struct SetupSources {
    /// The client roster, which a look reads from its cache.
    harnesses: HarnessRegistry,
    /// This computer.
    machine: Arc<dyn SetupMachine>,
}

/// Which browser a request would start, as browser recovery answers it, kept for one stretch with
/// no browser connected.
struct BrowserStarts {
    /// The recovery whose decision the answer is.
    recovery: BrowserRecovery,
    /// Advances whenever a look sees a browser connected, so an answer read before that is never
    /// kept for the stretch after it.
    stretch: u64,
    /// The answer of the current stretch, with the posture it was read under. Nothing is kept
    /// while a browser is connected, so the next stretch reads the installed browsers again.
    answer: Option<(BrowserStartup, Option<String>)>,
}

impl BrowserStarts {
    /// Forget the answer: a browser is connected, or the installed browsers changed.
    fn end_stretch(&mut self) {
        self.answer = None;
        self.stretch = self.stretch.wrapping_add(1);
    }
}

/// Count `sessions`, each its agent's id and its connections: the agents as At a glance groups
/// them, one per agent id, and the connections by the intake that admitted each, beside
/// `browsers`.
fn count_connections<'a>(
    sessions: impl Iterator<Item = (String, &'a [ConnectionDetails])>,
    browsers: usize,
) -> ConnectionCounts {
    let mut counts = ConnectionCounts {
        browsers,
        ..ConnectionCounts::default()
    };
    let mut agents = BTreeSet::new();
    for (agent, connections) in sessions {
        agents.insert(agent);
        for connection in connections {
            match connection.channel {
                IntakeChannel::Mcp => counts.mcp_clients += 1,
                IntakeChannel::Cli => counts.command_line += 1,
            }
        }
    }
    counts.agents = agents.len();
    counts
}

/// The live parts of one workbench.
pub(super) struct Live {
    projection: WorkbenchProjection,
    workspaces: WorkspaceStore,
    governance: GovernanceFacade,
    browser: Arc<dyn BrowserPort>,
    diagnostics: Arc<DiagnosticsHub>,
    web: Arc<OnceLock<WebFacts>>,
    started_at_ms: u64,
    published: Mutex<Published>,
    wanted: AtomicBool,
    /// Whether the next look builds the setup part whatever it saw.
    setup_wanted: AtomicBool,
    sources: Mutex<SetupSources>,
    remembered: Mutex<Remembered>,
    tray: Mutex<Option<Arc<dyn WorkbenchTrayPort>>>,
    starts: Mutex<BrowserStarts>,
}

impl Live {
    /// Build the live parts over the owners they read, and register them with the projection so
    /// that every change of work or of saving health refreshes them.
    ///
    /// The projection holds only a weak reference, so dropping the facade drops this.
    pub fn new(
        projection: &WorkbenchProjection,
        workspaces: WorkspaceStore,
        governance: GovernanceFacade,
        browser: Arc<dyn BrowserPort>,
        diagnostics: Arc<DiagnosticsHub>,
        web: Arc<OnceLock<WebFacts>>,
        harnesses: HarnessRegistry,
    ) -> Arc<Self> {
        let live = Arc::new(Self {
            projection: projection.clone(),
            workspaces,
            remembered: Mutex::new(Remembered {
                runtime: None,
                authority: governance.snapshot().id().to_owned(),
                agent_name: None,
            }),
            setup_wanted: AtomicBool::new(false),
            sources: Mutex::new(SetupSources {
                harnesses,
                machine: Arc::new(ThisComputer::discover()),
            }),
            starts: Mutex::new(BrowserStarts {
                recovery: BrowserRecovery::discover(governance.clone(), Arc::clone(&browser)),
                stretch: 0,
                answer: None,
            }),
            governance,
            browser,
            diagnostics,
            web,
            started_at_ms: super::unix_ms(),
            published: Mutex::default(),
            wanted: AtomicBool::new(false),
            tray: Mutex::new(None),
        });
        let weak = Arc::downgrade(&live);
        projection.set_status_hook(Arc::new(move || {
            if let Some(live) = weak.upgrade() {
                live.refresh();
            }
        }));
        live
    }

    /// One read for a screen: the sequence first, then one gather.
    pub fn look(&self) -> Look<'_> {
        // Read the sequence before anything is gathered. A change publishes after it has been
        // made, so one numbered at or below this is already in what follows, and one above it is
        // newer: the surface applies it by key, and applying it twice does no harm. Reading the
        // sequence last would count a change that landed mid-assembly as included when it is not.
        let seq = self.projection.current_seq();
        Look {
            live: self,
            seq,
            gathered: self.gather(),
        }
    }

    /// Look at the service: operations, sessions, browsers, the client roster, authority and
    /// runtime. The published authority is read once, so every fact of it here is one
    /// publication's.
    pub fn gather(&self) -> Gathered {
        let operations = self.projection.operations();
        let browser_list = self.browser.browsers();
        let published = self.governance.published();
        let mut tabs_in = HashMap::<String, usize>::new();
        let sessions: Vec<SessionSummary> = self
            .workspaces
            .summaries()
            .into_iter()
            .map(|workspace| {
                let pinned = workspace.browser;
                if let Some(id) = &pinned {
                    *tabs_in.entry(id.clone()).or_default() += workspace.tab_count;
                }
                // Only a connected browser is named; a pinned browser that is gone has no name
                // to give, and a guessed family would name the wrong one.
                let browser = pinned.and_then(|id| {
                    browser_list
                        .iter()
                        .find(|b| b.id == id)
                        .map(|summary| summary.name.clone())
                });
                SessionSummary {
                    running_operations: operations
                        .iter()
                        .filter(|operation| {
                            operation.workspace == workspace.id
                                && operation.phase == OperationPhase::Running
                        })
                        .count(),
                    id: workspace.id,
                    agent: workspace.opener.agent,
                    connections: workspace.connections,
                    channel: workspace.opener.channel,
                    tab_count: workspace.tab_count,
                    browser,
                }
            })
            .collect();
        let browsers: Vec<BrowserInstanceSummary> = browser_list
            .into_iter()
            .map(|browser| BrowserInstanceSummary {
                tab_count: tabs_in.get(&browser.id).copied().unwrap_or_default(),
                family: browser.name,
                id: browser.id,
                adapter_version: Some(browser.adapter_version),
                connected: true,
            })
            .collect();
        let starts = if browsers.is_empty() {
            self.starts_on_request(published.authority().browser_startup.value)
        } else {
            lock(&self.starts).end_stretch();
            None
        };
        let no_browser = browsers
            .is_empty()
            .then(|| words::no_browser(starts.as_deref()));
        let roster = lock(&self.sources).harnesses.clone();
        let harnesses = roster.summaries();
        let harnesses_in_use = parts::harnesses_in_use(&harnesses, &sessions);
        let authority = published.diagnostics().clone();
        let runtime_state = self.governance.runtime_state();
        let audit_health = self.projection.audit_health();
        let enforced = Arc::clone(published.snapshot());
        let audit_required = enforced.requires_audit();
        let agents_on = |channel| sessions.iter().any(|session| session.channel == channel);
        let readiness = ReadinessSummary::resolve(&readiness::ReadinessFacts {
            audit_required_unavailable: audit_required && audit_health.unavailable(),
            browser_connected: !browsers.is_empty(),
            starts: starts.as_deref(),
            session_ended: runtime_state == RuntimeControlState::Ended,
            paused: runtime_state == RuntimeControlState::Held,
            authority_invalid: !authority.authority_valid,
            working: operations
                .iter()
                .any(|operation| operation.phase == OperationPhase::Running),
            channels_off: enforced.channels_off(),
            agents_on_mcp: agents_on(IntakeChannel::Mcp),
            agents_on_cli: agents_on(IntakeChannel::Cli),
        });
        Gathered {
            runtime_note: self.runtime_note(runtime_state),
            policy: published.chip().clone(),
            operations,
            sessions,
            browsers,
            no_browser,
            starts,
            harnesses,
            harnesses_in_use,
            authority,
            runtime_state,
            audit_health,
            audit_required,
            enforced,
            readiness,
        }
    }

    /// The status part of one look.
    pub fn status(&self, gathered: &Gathered) -> StatusPart {
        StatusPart {
            service: ServiceSummary {
                version: env!("CARGO_PKG_VERSION").into(),
                started_at_ms: self.started_at_ms,
                runtime_state: gathered.runtime_state,
            },
            readiness: gathered.readiness.clone(),
            audit_health: gathered.audit_health.clone(),
            audit_notice: if gathered.audit_health.quiet() {
                String::new()
            } else {
                gathered.audit_health.detail(gathered.audit_required)
            },
            configuration: StatusConfiguration {
                policy: gathered.policy.clone(),
                managed_policy: gathered.authority.managed_policy.clone(),
                organization: OrganizationView::of(gathered.enforced.organization()),
            },
            runtime_note: gathered.runtime_note.clone(),
        }
    }

    /// The controls part of one look, as the window sees it.
    pub fn controls(&self, gathered: &Gathered) -> ControlsPart {
        let facts = self.web.get();
        let report = self.diagnostics.report();
        let user_home = parts::user_home();
        ControlsPart::of(&ControlsFacts {
            runtime_state: gathered.runtime_state,
            runtime_note: gathered.runtime_note.as_deref(),
            watching: facts.map_or(WatchingFacts::NotServed, |facts| WatchingFacts::Served {
                watching: facts.watching.is_on(),
                url: facts.address.url(),
            }),
            diagnostics: &report,
            counts: gathered.counts(),
            user_home: user_home.as_deref(),
        })
    }

    /// The setup part of one look, as the window sees it.
    ///
    /// Only an incomplete setup reads this computer. The name of the agent that finished setup is
    /// resolved once and kept.
    pub fn setup(&self, gathered: &Gathered) -> SetupView {
        let progress = self.projection.setup();
        let machine = Arc::clone(&lock(&self.sources).machine);
        let harnesses = &gathered.harnesses;
        let agent_name = progress
            .as_ref()
            .and_then(|progress| progress.finished())
            .and_then(|finished| {
                let mut remembered = lock(&self.remembered);
                if remembered.agent_name.is_none() {
                    remembered.agent_name =
                        setup::agent_name(&finished, &gathered.sessions, harnesses);
                }
                remembered.agent_name.clone()
            });
        setup::setup_view(&SetupInputs {
            progress: progress.as_deref(),
            sessions: &gathered.sessions,
            browsers: &gathered.browsers,
            harnesses,
            machine: machine.as_ref(),
            agent_name: agent_name.as_deref(),
        })
    }

    /// Carry out one setup step that starts a browser, over this computer as it is now, noted in
    /// this service's launch guard.
    pub fn perform_setup_act(&self, action: &SetupAction) -> Result<(), WorkbenchError> {
        let machine = Arc::clone(&lock(&self.sources).machine);
        setup::perform(machine.as_ref(), self.browser.as_ref(), action)
    }

    /// What decides whether the setup part is built again.
    fn setup_inputs_key(gathered: &Gathered, complete: bool) -> SetupInputsKey {
        let mut browsers: Vec<String> = gathered
            .browsers
            .iter()
            .map(|browser| browser.family.clone())
            .collect();
        browsers.sort();
        // Keyed on the catalog id, the same identity the setup view matches agents by.
        let mut clients: Vec<String> = gathered
            .sessions
            .iter()
            .map(|session| session.agent.id.clone())
            .collect();
        clients.sort();
        SetupInputsKey {
            browsers,
            clients,
            complete,
        }
    }

    /// The tray of one look: the icon state from the readiness, the tooltip from the readiness and
    /// the connections, and the entries from the runtime state.
    pub fn tray(&self, gathered: &Gathered) -> TrayModel {
        TrayModel::of(
            gathered.readiness.state,
            gathered.runtime_state,
            gathered.counts(),
        )
    }

    /// The tray as it is right now.
    pub fn tray_model(&self) -> TrayModel {
        self.tray(&self.gather())
    }

    /// Attach what draws the tray. It is told the tray each time a look finds it changed.
    pub fn attach_tray(&self, port: Arc<dyn WorkbenchTrayPort>) {
        *lock(&self.tray) = Some(port);
    }

    /// What is connected right now, without gathering operations, history or authority.
    pub fn connection_counts(&self) -> ConnectionCounts {
        let summaries = self.workspaces.summaries();
        count_connections(
            summaries.iter().map(|workspace| {
                (
                    workspace.opener.agent.id.clone(),
                    workspace.connections.as_slice(),
                )
            }),
            self.browser.browsers().len(),
        )
    }

    /// Say that the setup part may have changed in a way a look cannot see, and look.
    ///
    /// Call it after the client roster was looked at again and when setup completes.
    pub fn setup_changed(&self) {
        self.setup_wanted.store(true, Ordering::Release);
        self.refresh();
    }

    /// Replace the client roster the setup part is read from.
    #[cfg(test)]
    pub fn use_harnesses(&self, harnesses: HarnessRegistry) {
        lock(&self.sources).harnesses = harnesses;
    }

    /// Replace the computer the setup part is read from.
    #[cfg(test)]
    pub fn use_machine(&self, machine: Arc<dyn SetupMachine>) {
        lock(&self.sources).machine = machine;
    }

    /// The name of the browser a request would start now under `posture`, read again only when
    /// the stretch or the posture is new. The installed browsers are read with the lock released,
    /// and the answer is kept only when no browser connected meanwhile.
    fn starts_on_request(&self, posture: BrowserStartup) -> Option<String> {
        let (recovery, stretch) = {
            let starts = lock(&self.starts);
            if let Some((known, answer)) = &starts.answer {
                if *known == posture {
                    return answer.clone();
                }
            }
            (starts.recovery.clone(), starts.stretch)
        };
        let answer = recovery.starts_on_request();
        let mut starts = lock(&self.starts);
        if starts.stretch == stretch {
            starts.answer = Some((posture, answer.clone()));
        }
        answer
    }

    /// Replace the installed browsers the no-browser line is read from.
    #[cfg(test)]
    pub fn use_installed_browsers(
        &self,
        candidates: Vec<crate::browser::recovery::RecoveryCandidate>,
    ) {
        let mut starts = lock(&self.starts);
        starts.recovery.set_test_candidates(candidates);
        starts.end_stretch();
    }

    /// Remember who brought the runtime to `state`. A request that leaves the state as it was
    /// keeps the first source: the pause came from there.
    ///
    /// Returns whether it remembered anything new, which changes what a screen says of the pause.
    pub fn record_runtime(&self, state: RuntimeControlState, source: RuntimeSource) -> bool {
        let mut remembered = lock(&self.remembered);
        let new = remembered.runtime.map(|(known, _)| known) != Some(state);
        if new {
            remembered.runtime = Some((state, source));
        }
        new
    }

    /// The sentence that says where a pause came from. There is none while agents are not paused,
    /// or when the pause did not come through the facade.
    fn runtime_note(&self, state: RuntimeControlState) -> Option<String> {
        if state != RuntimeControlState::Held {
            return None;
        }
        match lock(&self.remembered).runtime {
            Some((RuntimeControlState::Held, source)) => Some(source.paused_note().to_owned()),
            _ => None,
        }
    }

    /// Publish the status and connections parts that differ from the ones last published.
    ///
    /// Callers may be on any thread and inside any lock but this one's: a request made while
    /// another refresh is running marks the parts as wanted again and returns, and the running
    /// refresh looks once more before it lets go. So the last look is always published, in order,
    /// and nothing here waits.
    pub fn refresh(&self) {
        self.wanted.store(true, Ordering::Release);
        loop {
            let mut published = match self.published.try_lock() {
                Ok(guard) => guard,
                Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
                Err(TryLockError::WouldBlock) => return,
            };
            while self.wanted.swap(false, Ordering::AcqRel) {
                let gathered = self.gather();
                let status = self.status(&gathered);
                if published.status.as_ref() != Some(&status) {
                    published.status = Some(status.clone());
                    self.projection
                        .publish(WorkbenchChange::StatusChanged { status });
                }
                let connections = gathered.connections();
                if published.connections.as_ref() != Some(&connections) {
                    published.connections = Some(connections.clone());
                    self.projection
                        .publish(WorkbenchChange::ConnectionsChanged { connections });
                }
                let controls = self.controls(&gathered);
                if published.controls.as_ref() != Some(&controls) {
                    published.controls = Some(controls.clone());
                    self.projection.publish(WorkbenchChange::ControlsChanged {
                        controls: Box::new(controls),
                    });
                }
                self.publish_setup(&mut published, &gathered);
                let tray = self.tray(&gathered);
                if published.tray.as_ref() != Some(&tray) {
                    published.tray = Some(tray.clone());
                    let port = lock(&self.tray).clone();
                    if let Some(port) = port {
                        port.tray_changed(tray);
                    }
                }
            }
            drop(published);
            if !self.wanted.load(Ordering::Acquire) {
                return;
            }
        }
    }

    /// Announce the setup part when it differs from the one last announced.
    ///
    /// Nothing is built or announced unless the service tracks setup. The part is built only when
    /// what it is read from changed, or when a caller said it did.
    fn publish_setup(&self, published: &mut Published, gathered: &Gathered) {
        let wanted = self.setup_wanted.swap(false, Ordering::AcqRel);
        let Some(progress) = self.projection.setup() else {
            return;
        };
        let key = Self::setup_inputs_key(gathered, progress.is_complete());
        if !wanted && published.setup_inputs.as_ref() == Some(&key) {
            return;
        }
        published.setup_inputs = Some(key);
        let etag = parts::json_etag(&self.setup(gathered));
        if published.setup_etag.as_ref() != Some(&etag) {
            published.setup_etag = Some(etag.clone());
            self.projection.publish(WorkbenchChange::PartChanged {
                part: HintedPart::Setup,
                etag,
            });
        }
    }

    /// Tell every screen that the person's rules changed, tagged with the authority now in force.
    ///
    /// The refresher does not announce the same authority again.
    pub fn policy_changed(&self) {
        let id = self.governance.published().authority().id.clone();
        lock(&self.remembered).authority.clone_from(&id);
        self.projection.publish(WorkbenchChange::PartChanged {
            part: HintedPart::Policy,
            etag: id,
        });
    }

    /// Tell every screen that something other than the person's own act changed the authority,
    /// when the id of the published authority is not the one last announced.
    pub fn authority_refreshed(&self) {
        let id = self.governance.snapshot().id().to_owned();
        {
            let mut remembered = lock(&self.remembered);
            if remembered.authority == id {
                return;
            }
            remembered.authority.clone_from(&id);
        }
        self.projection.publish(WorkbenchChange::PartChanged {
            part: HintedPart::Policy,
            etag: id,
        });
    }
}
