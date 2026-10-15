//! The live parts of the workbench: what a screen shows the moment something connects, pauses or
//! starts working.
//!
//! The status, connections and controls parts change without any act of a screen. A pause from the
//! tray or the browser toolbar, a session that opens, a browser that goes away, process diagnostics
//! turned on from a terminal: none of them is a domain event of the work projection. This module
//! builds those parts the light way (no history), remembers the last ones it published, and
//! publishes a part only when it differs from that. Every caller that knows something may have
//! changed asks for a refresh and nothing more (ADR-0199, amendment of 2026-10-15).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, TryLockError};

use super::parts::{
    self, ConnectionsPart, ControlsFacts, ControlsPart, HintedPart, StatusConfiguration,
    StatusPart, ViewingFacts,
};
use super::{
    BrowserInstanceSummary, OperationSummary, ReadinessSummary, RuntimeSource, ServiceSummary,
    SessionSummary, WebFacts, WorkbenchChange, WorkbenchProjection,
};
use crate::browser::adapter::RuntimeControlState;
use crate::browser::BrowserPort;
use crate::diagnostics::DiagnosticsHub;
use crate::governance::effective::PolicyChip;
use crate::governance::{GovernanceDiagnostics, GovernanceFacade};
use crate::intake::session::IntakeChannel;
use crate::language::audit_health::AuditHealth;
use crate::language::readiness;
use crate::language::workbench::ConnectionCounts;
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
        }
    }
}

/// The parts last published, which a refresh compares with.
#[derive(Default)]
struct Published {
    status: Option<StatusPart>,
    connections: Option<ConnectionsPart>,
    controls: Option<ControlsPart>,
}

/// What the facade remembers between looks.
#[derive(Default)]
struct Remembered {
    /// The state the runtime last reached through the facade, and who brought it there.
    runtime: Option<(RuntimeControlState, RuntimeSource)>,
    /// The authority id a policy hint was last published for.
    authority: String,
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
    remembered: Mutex<Remembered>,
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
        started_at_ms: u64,
    ) -> Arc<Self> {
        let live = Arc::new(Self {
            projection: projection.clone(),
            workspaces,
            remembered: Mutex::new(Remembered {
                runtime: None,
                authority: governance.snapshot().id().to_owned(),
            }),
            governance,
            browser,
            diagnostics,
            web,
            started_at_ms,
            published: Mutex::default(),
            wanted: AtomicBool::new(false),
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
        let sessions: Vec<SessionSummary> = self
            .workspaces
            .summaries()
            .into_iter()
            .map(|workspace| {
                let browser = self.workspaces.browser_of(&workspace.id).and_then(|id| {
                    browser_list
                        .iter()
                        .find(|b| b.id == id)
                        .map(|b| b.name.clone().unwrap_or_else(|| "Chromium".into()))
                        .or_else(|| Some("Chromium".into()))
                });
                SessionSummary {
                    active_operations: operations
                        .iter()
                        .filter(|operation| operation.workspace == workspace.id)
                        .count(),
                    id: workspace.id,
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
                id: browser.id,
                family: browser.name.unwrap_or_else(|| "Chromium".into()),
                adapter_version: Some(browser.adapter_version),
                connected: true,
            })
            .collect();
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

    /// The Controls sheet of one look, as the window sees it.
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
            }
            drop(published);
            if !self.wanted.load(Ordering::Acquire) {
                return;
            }
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
