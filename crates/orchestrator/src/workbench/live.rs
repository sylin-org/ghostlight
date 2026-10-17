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
//! While no browser is connected, the connections part says which browser a request would start.
//! Browser recovery answers that from the installed browsers, which it reads once for each stretch
//! with no browser connected and each browser startup posture in it, and never under `manual`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, TryLockError};

use super::parts::{
    self, ConnectionsPart, ControlsFacts, ControlsPart, HintedPart, StatusConfiguration,
    StatusPart, ViewingFacts,
};
use super::setup::{self, SetupInputs, SetupMachine, ThisComputer};
use super::{
    BrowserInstanceSummary, OperationSummary, ReadinessSummary, RuntimeSource, ServiceSummary,
    SessionSummary, WebFacts, WorkbenchChange, WorkbenchError, WorkbenchProjection,
    WorkbenchTrayPort,
};
use crate::browser::adapter::RuntimeControlState;
use crate::browser::recovery::BrowserRecovery;
use crate::browser::{BrowserPort, BrowserSummary};
use crate::diagnostics::DiagnosticsHub;
use crate::governance::effective::PolicyChip;
use crate::governance::manifest::BrowserStartup;
use crate::governance::{GovernanceDiagnostics, GovernanceFacade};
use crate::install::HarnessRegistry;
use crate::intake::desktop::TrayModel;
use crate::intake::session::IntakeChannel;
use crate::language::audit_health::AuditHealth;
use crate::language::readiness;
use crate::language::setup::{SetupAction, SetupView};
use crate::language::workbench::{self as words, ConnectionCounts};
use crate::provenance::ConnectionDetails;
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
        }
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
    /// The answer of the current stretch, with the posture it was read under. Nothing is kept
    /// while a browser is connected, so the next stretch reads the installed browsers again.
    answer: Option<(BrowserStartup, Option<String>)>,
}

impl BrowserStarts {
    /// The name of the browser a request would start now, read again only when the stretch or
    /// the posture is new.
    fn answer(&mut self, posture: BrowserStartup) -> Option<String> {
        match &self.answer {
            Some((known, answer)) if *known == posture => answer.clone(),
            _ => {
                let answer = self.recovery.starts_on_request();
                self.answer = Some((posture, answer.clone()));
                answer
            }
        }
    }
}

/// Count `connections` by the intake that admitted each, beside `browsers`.
fn count_connections<'a>(
    connections: impl Iterator<Item = &'a ConnectionDetails>,
    browsers: usize,
) -> ConnectionCounts {
    let mut counts = ConnectionCounts {
        browsers,
        ..ConnectionCounts::default()
    };
    for connection in connections {
        match connection.channel {
            IntakeChannel::Mcp => counts.mcp_clients += 1,
            IntakeChannel::Cli => counts.command_line += 1,
        }
    }
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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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

    /// Look at the service: operations, sessions, browsers, authority and runtime.
    pub fn gather(&self) -> Gathered {
        let operations = self.projection.operations();
        let browser_list = self.browser.browsers();
        let mut tabs_in = HashMap::<String, usize>::new();
        let sessions: Vec<SessionSummary> = self
            .workspaces
            .summaries()
            .into_iter()
            .map(|workspace| {
                let pinned = self.workspaces.browser_of(&workspace.id);
                if let Some(id) = &pinned {
                    *tabs_in.entry(id.clone()).or_default() += workspace.tab_count;
                }
                // Only a connected browser is named; a pinned browser that is gone has no name
                // to give, and a guessed family would name the wrong one.
                let browser = pinned.and_then(|id| {
                    browser_list
                        .iter()
                        .find(|b| b.id == id)
                        .map(BrowserSummary::display_name)
                });
                SessionSummary {
                    active_operations: operations
                        .iter()
                        .filter(|operation| operation.workspace == workspace.id)
                        .count(),
                    id: workspace.id,
                    agent: crate::language::provenance::agent_name(
                        &workspace.client_label,
                        workspace.channel,
                    ),
                    client_label: workspace.client_label,
                    connections: workspace.connections,
                    channel: workspace.channel,
                    leased: workspace.leased,
                    tab_count: workspace.tab_count,
                    held_tab_count: workspace.held_tab_count,
                    browser,
                }
            })
            .collect();
        let browsers: Vec<BrowserInstanceSummary> = browser_list
            .into_iter()
            .map(|browser| BrowserInstanceSummary {
                tab_count: tabs_in.get(&browser.id).copied().unwrap_or_default(),
                family: browser.display_name(),
                id: browser.id,
                adapter_version: Some(browser.adapter_version),
                connected: true,
            })
            .collect();
        let no_browser = {
            let mut starts = lock(&self.starts);
            if browsers.is_empty() {
                let starts = starts.answer(self.governance.browser_startup());
                Some(words::no_browser(starts.as_deref()))
            } else {
                starts.answer = None;
                None
            }
        };
        let authority = self.governance.diagnostics();
        let runtime_state = self.governance.runtime_state();
        let audit_health = self.projection.audit_health();
        let audit_required = self.governance.snapshot().requires_audit();
        let readiness = ReadinessSummary::resolve(&readiness::ReadinessFacts {
            audit_required_unavailable: audit_required && audit_health.unavailable(),
            browser_connected: !browsers.is_empty(),
            session_ended: runtime_state == RuntimeControlState::Ended,
            paused: runtime_state == RuntimeControlState::Held,
            authority_invalid: !authority.authority_valid,
            working: !operations.is_empty(),
        });
        Gathered {
            runtime_note: self.runtime_note(runtime_state),
            policy: self.governance.policy_chip(),
            operations,
            sessions,
            browsers,
            no_browser,
            authority,
            runtime_state,
            audit_health,
            audit_required,
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
            viewing: facts.map_or(ViewingFacts::NotServed, |facts| ViewingFacts::Served {
                watching: facts.watching.is_on(),
                url: facts.address.url(),
            }),
            diagnostics: &report,
            counts: count_connections(
                gathered
                    .sessions
                    .iter()
                    .flat_map(|session| session.connections.iter()),
                gathered.browsers.len(),
            ),
            user_home: user_home.as_deref(),
        })
    }

    /// The setup part of one look, as the window sees it.
    ///
    /// Only an incomplete setup reads this computer. The name of the agent that finished setup is
    /// resolved once and kept.
    pub fn setup(&self, gathered: &Gathered) -> SetupView {
        let progress = self.projection.setup();
        let SetupSources { harnesses, machine } = lock(&self.sources).clone();
        let harnesses = harnesses.summaries();
        let agent_name = progress
            .as_ref()
            .and_then(|progress| progress.finished())
            .and_then(|finished| {
                let mut remembered = lock(&self.remembered);
                if remembered.agent_name.is_none() {
                    remembered.agent_name =
                        setup::agent_name(&finished, &gathered.sessions, &harnesses);
                }
                remembered.agent_name.clone()
            });
        setup::setup_view(&SetupInputs {
            progress: progress.as_deref(),
            sessions: &gathered.sessions,
            browsers: &gathered.browsers,
            harnesses: &harnesses,
            machine: machine.as_ref(),
            agent_name: agent_name.as_deref(),
        })
    }

    /// Carry out one setup step that starts a browser, over this computer as it is now.
    pub fn perform_setup_act(&self, action: &SetupAction) -> Result<(), WorkbenchError> {
        let machine = Arc::clone(&lock(&self.sources).machine);
        setup::perform(machine.as_ref(), action)
    }

    /// What decides whether the setup part is built again.
    fn setup_inputs_key(gathered: &Gathered, complete: bool) -> SetupInputsKey {
        let mut browsers: Vec<String> = gathered
            .browsers
            .iter()
            .map(|browser| browser.family.clone())
            .collect();
        browsers.sort();
        let mut clients: Vec<String> = gathered
            .sessions
            .iter()
            .map(|session| session.agent.clone())
            .collect();
        clients.sort();
        SetupInputsKey {
            browsers,
            clients,
            complete,
        }
    }

    /// The tray of one look: the icon state from the readiness, the tooltip from the connections
    /// and the entries from the runtime state.
    pub fn tray(&self, gathered: &Gathered) -> TrayModel {
        TrayModel::of(
            gathered.readiness.state,
            gathered.runtime_state,
            count_connections(
                gathered
                    .sessions
                    .iter()
                    .flat_map(|session| session.connections.iter()),
                gathered.browsers.len(),
            ),
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
            summaries
                .iter()
                .flat_map(|workspace| workspace.connections.iter()),
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

    /// Replace the installed browsers the no-browser line is read from.
    #[cfg(test)]
    pub fn use_installed_browsers(
        &self,
        candidates: Vec<crate::browser::recovery::RecoveryCandidate>,
    ) {
        let mut starts = lock(&self.starts);
        starts.recovery.set_test_candidates(candidates);
        starts.answer = None;
    }

    /// Remember who brought the runtime to `state`. A request that leaves the state as it was
    /// keeps the first source: the pause came from there.
    pub fn record_runtime(&self, state: RuntimeControlState, source: RuntimeSource) {
        let mut remembered = lock(&self.remembered);
        if remembered.runtime.map(|(known, _)| known) != Some(state) {
            remembered.runtime = Some((state, source));
        }
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

    /// Tell every screen that the person's rules changed, with their new tag.
    ///
    /// The refresher does not announce the same authority again.
    pub fn policy_changed(&self) {
        lock(&self.remembered).authority = self.governance.snapshot().id().to_owned();
        self.projection.publish(WorkbenchChange::PartChanged {
            part: HintedPart::Policy,
            etag: parts::policy_etag(&self.governance.effective_authority()),
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
