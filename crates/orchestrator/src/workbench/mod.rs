//! Orchestrator-owned desktop read model, user intents, and operating-system presentation port.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use ghostlight_bridge::browser::{PresentationActivity, RuntimeControlIntent, RuntimeControlState};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::browser::{BrowserPort, RelayBrowserPort};
use crate::events::DomainEvent;
use crate::governance::{AuditRecord, AuditSink, Capability, GovernanceFacade};
use crate::install::{
    HarnessAction, HarnessActionResult, HarnessError, HarnessRegistry, HarnessSummary,
};
use crate::workspace::WorkspaceStore;

const HISTORY_LIMIT: usize = 500;
const SEARCH_LIMIT: usize = 100;

/// Disposable workbench projection fed directly from the closed domain-event vocabulary.
#[derive(Clone, Default)]
pub struct WorkbenchProjection {
    inner: Arc<Mutex<ProjectionState>>,
    presentation: Arc<Mutex<Option<Arc<dyn WorkbenchPresentationPort>>>>,
    events: Arc<Mutex<Option<Arc<dyn WorkbenchEventSink>>>>,
    seq: Arc<AtomicU64>,
}

#[derive(Default)]
struct ProjectionState {
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
    capability: Capability,
    started_at_ms: u64,
    phase: OperationPhase,
}

impl WorkbenchProjection {
    /// Restore bounded payload-free history from the orchestrator-owned audit file.
    pub fn load_history(&self, path: &Path) -> io::Result<()> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        let mut restored = VecDeque::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: AuditRecord = serde_json::from_str(&line).map_err(io::Error::other)?;
            push_bounded(&mut restored, HistoryItem::from(record));
        }
        self.lock().history = restored;
        Ok(())
    }

    /// Attach or replace the best-effort operating-system presentation adapter.
    pub fn attach_presentation(&self, port: Arc<dyn WorkbenchPresentationPort>) {
        *lock(&self.presentation) = Some(port);
    }

    /// Attach or replace the best-effort sequenced change-event sink.
    pub fn attach_events(&self, sink: Arc<dyn WorkbenchEventSink>) {
        *lock(&self.events) = Some(sink);
    }

    /// Sequence number of the most recently published change.
    #[must_use]
    pub fn current_seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    /// Publish one sequenced change to the disposable presentation surface, if any is attached.
    ///
    /// Never call this while holding the projection state lock: presentation adapters are
    /// outbound boundaries and must not be reached across a domain lock.
    fn publish(&self, change: WorkbenchChange) {
        let Some(sink) = lock(&self.events).clone() else {
            return;
        };
        let seq = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
        sink.publish(WorkbenchEvent { seq, change });
    }

    /// React to one completed domain transition without affecting its authority or truth.
    pub fn react(&self, event: &DomainEvent) {
        let (notification, change) = {
            let mut state = self.lock();
            match event {
                DomainEvent::WorkStarted {
                    invocation,
                    workspace,
                    tool,
                    activity,
                    capability,
                } => {
                    let operation = OperationState {
                        invocation: invocation.clone(),
                        workspace: workspace.clone(),
                        tool: tool.clone(),
                        activity: *activity,
                        capability: *capability,
                        started_at_ms: unix_ms(),
                        phase: OperationPhase::Running,
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
                DomainEvent::AttentionRequired { invocation, .. } => {
                    let change = state.set_phase(invocation, OperationPhase::Attention);
                    let notification = state
                        .notified
                        .insert((invocation.clone(), NotificationKind::Attention))
                        .then(|| WorkbenchNotification {
                            kind: NotificationKind::Attention,
                            title: "Ghostlight needs your attention".into(),
                            body: "A browser operation is waiting for you.".into(),
                        });
                    (notification, change)
                }
                DomainEvent::WorkBlocked { invocation, .. } => {
                    let change = state.set_phase(invocation, OperationPhase::Blocked);
                    let notification = state
                        .notified
                        .insert((invocation.clone(), NotificationKind::Blocked))
                        .then(|| WorkbenchNotification {
                            kind: NotificationKind::Blocked,
                            title: "Ghostlight blocked an action".into(),
                            body: "A configured guardrail prevented browser work.".into(),
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

    fn record(&self, record: &AuditRecord) {
        let item = HistoryItem::from(record.clone());
        {
            let mut state = self.lock();
            state.operations.remove(&record.invocation);
            state
                .notified
                .retain(|(invocation, _)| invocation != &record.invocation);
            push_bounded(&mut state.history, item.clone());
        }
        self.publish(WorkbenchChange::OperationSettled { record: item });
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

/// Audit decorator that keeps the durable log and workbench projection synchronized.
pub struct ProjectingAuditSink {
    durable: Arc<dyn AuditSink>,
    projection: WorkbenchProjection,
}

impl ProjectingAuditSink {
    /// Wrap one durable audit sink.
    #[must_use]
    pub fn new(durable: Arc<dyn AuditSink>, projection: WorkbenchProjection) -> Self {
        Self {
            durable,
            projection,
        }
    }
}

impl AuditSink for ProjectingAuditSink {
    fn record(&self, record: &AuditRecord) -> io::Result<()> {
        let durable = self.durable.record(record);
        self.projection.record(record);
        durable
    }
}

/// Narrow application boundary consumed by the Tauri adapter.
#[derive(Clone)]
pub struct WorkbenchFacade {
    projection: WorkbenchProjection,
    workspaces: WorkspaceStore,
    governance: GovernanceFacade,
    browser: Arc<RelayBrowserPort>,
    harnesses: HarnessRegistry,
    started_at_ms: u64,
}

impl WorkbenchFacade {
    /// Construct the facade over existing orchestrator owners.
    #[must_use]
    pub fn new(
        projection: WorkbenchProjection,
        workspaces: WorkspaceStore,
        governance: GovernanceFacade,
        browser: Arc<RelayBrowserPort>,
    ) -> Self {
        Self {
            projection,
            workspaces,
            governance,
            browser,
            harnesses: HarnessRegistry::discover(),
            started_at_ms: unix_ms(),
        }
    }

    /// Attach the best-effort operating-system presentation adapter.
    pub fn attach_presentation(&self, port: Arc<dyn WorkbenchPresentationPort>) {
        self.projection.attach_presentation(port);
    }

    /// Attach the best-effort sequenced change-event sink used by a live presentation surface.
    pub fn attach_events(&self, sink: Arc<dyn WorkbenchEventSink>) {
        self.projection.attach_events(sink);
    }

    /// Build an immutable, content-free snapshot for a disposable UI.
    #[must_use]
    pub fn snapshot(&self) -> WorkbenchSnapshot {
        let operations = self.projection.operations();
        let sessions = self
            .workspaces
            .summaries()
            .into_iter()
            .map(|workspace| SessionSummary {
                active_operations: operations
                    .iter()
                    .filter(|operation| operation.workspace == workspace.id)
                    .count(),
                id: workspace.id,
                client_label: workspace.client_label,
                leased: workspace.leased,
                tab_count: workspace.tab_count,
                held_tab_count: workspace.held_tab_count,
            })
            .collect::<Vec<_>>();
        let browsers = self.browser_summary().into_iter().collect::<Vec<_>>();
        let governance = self.governance.diagnostics();
        let mut diagnostics = vec![DiagnosticItem::passing(
            "service",
            "Orchestrator",
            "Ghostlight is accepting local connections.",
        )];
        diagnostics.push(if browsers.is_empty() {
            DiagnosticItem::warning(
                "browser",
                "Browser adapter",
                "Waiting for Ghostlight in Browser to connect.",
            )
        } else {
            DiagnosticItem::passing(
                "browser",
                "Browser adapter",
                "A compatible browser adapter is connected.",
            )
        });
        diagnostics.push(
            if governance.local_policy_valid && governance.managed_authority_valid {
                DiagnosticItem::passing(
                    "authority",
                    "Authority",
                    "Configured authority sources are valid.",
                )
            } else {
                DiagnosticItem::failing(
                    "authority",
                    "Authority",
                    "A configured authority source is invalid; work fails closed.",
                )
            },
        );
        let history = self.projection.history();
        // Read the sequence after gathering, so a snapshot never claims to be newer than it is.
        // A change published mid-assembly is re-delivered and applied idempotently by key.
        let seq = self.projection.current_seq();
        WorkbenchSnapshot {
            seq,
            generated_at_ms: unix_ms(),
            service: ServiceSummary {
                version: env!("CARGO_PKG_VERSION").into(),
                started_at_ms: self.started_at_ms,
                runtime_state: self.governance.runtime_state(),
            },
            overview: OverviewSummary {
                active_sessions: sessions.len(),
                active_operations: operations.len(),
                connected_browsers: browsers.len(),
                blocked_in_history: history.iter().filter(|item| !item.allowed).count(),
            },
            sessions,
            operations,
            browsers,
            history,
            diagnostics,
            harnesses: self.harnesses.summaries(),
            configuration: ConfigurationSummary {
                runtime_state: self.governance.runtime_state(),
                local_policy_configured: governance.local_policy_configured,
                local_policy_valid: governance.local_policy_valid,
                managed_authority_configured: governance.managed_authority_configured,
                managed_authority_valid: governance.managed_authority_valid,
                runtime_control_file_configured: governance.runtime_control_file_configured,
            },
        }
    }

    /// Search bounded user-visible workbench records.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let snapshot = self.snapshot();
        let mut hits = Vec::new();
        for (id, title, detail, view) in [
            (
                "monitor",
                "Monitor",
                "Live actions, connected clients and browsers, and recorded work",
                SearchDestination::Activity,
            ),
            (
                "status",
                "Status",
                "Local service, browser, and authority diagnostics",
                SearchDestination::Checkup,
            ),
            (
                "integrations",
                "MCP integrations",
                "Supported MCP client registrations",
                SearchDestination::Install,
            ),
        ] {
            push_hit(
                &mut hits,
                &query,
                SearchHit {
                    kind: SearchKind::Destination,
                    id: id.into(),
                    title: title.into(),
                    detail: detail.into(),
                    timestamp_ms: None,
                    view,
                },
            );
        }
        for operation in snapshot.operations {
            push_hit(
                &mut hits,
                &query,
                SearchHit {
                    kind: SearchKind::Operation,
                    id: operation.invocation,
                    title: operation.tool,
                    detail: format!("{} - {}", operation.workspace, operation.phase.label()),
                    timestamp_ms: operation.started_at_ms,
                    view: SearchDestination::Activity,
                },
            );
        }
        for session in snapshot.sessions {
            push_hit(
                &mut hits,
                &query,
                SearchHit {
                    kind: SearchKind::Session,
                    id: session.id.clone(),
                    title: session.client_label,
                    detail: format!("{} controlled tabs", session.tab_count),
                    timestamp_ms: None,
                    view: SearchDestination::Activity,
                },
            );
        }
        for browser in snapshot.browsers {
            push_hit(
                &mut hits,
                &query,
                SearchHit {
                    kind: SearchKind::Browser,
                    id: browser.id,
                    title: browser.family,
                    detail: format!(
                        "Connected browser adapter {}",
                        browser.adapter_version.unwrap_or_else(|| "unknown".into())
                    ),
                    timestamp_ms: None,
                    view: SearchDestination::Activity,
                },
            );
        }
        for item in snapshot.history {
            push_hit(
                &mut hits,
                &query,
                SearchHit {
                    kind: SearchKind::History,
                    id: item.invocation,
                    title: item.tool,
                    detail: format!("{} - {}", item.status, item.reason),
                    timestamp_ms: Some(item.timestamp_ms),
                    view: SearchDestination::History,
                },
            );
        }
        for diagnostic in snapshot.diagnostics {
            push_hit(
                &mut hits,
                &query,
                SearchHit {
                    kind: SearchKind::Diagnostic,
                    id: diagnostic.id,
                    title: diagnostic.label,
                    detail: diagnostic.detail,
                    timestamp_ms: None,
                    view: SearchDestination::Checkup,
                },
            );
        }
        push_hit(
            &mut hits,
            &query,
            SearchHit {
                kind: SearchKind::Configuration,
                id: "runtime-control".into(),
                title: "Runtime control".into(),
                detail: format!(
                    "Browser work is {}",
                    runtime_state_label(snapshot.configuration.runtime_state)
                ),
                timestamp_ms: None,
                view: SearchDestination::Configuration,
            },
        );
        for harness in snapshot.harnesses {
            push_hit(
                &mut hits,
                &query,
                SearchHit {
                    kind: SearchKind::Installation,
                    id: harness.id,
                    title: harness.name,
                    detail: harness.detail,
                    timestamp_ms: None,
                    view: SearchDestination::Install,
                },
            );
        }
        hits.truncate(SEARCH_LIMIT);
        hits
    }

    /// Apply an explicit local-human runtime control through the authoritative owners.
    pub fn apply_runtime_intent(&self, intent: WorkbenchRuntimeIntent) -> WorkbenchIntentResult {
        let state = self.governance.apply_runtime_intent(intent.into());
        let browser_notified = self.browser.publish_control_state(state).is_ok();
        self.projection.publish(WorkbenchChange::RuntimeChanged {
            runtime_state: state,
        });
        WorkbenchIntentResult {
            accepted: true,
            runtime_state: state,
            browser_notified,
            message: if browser_notified {
                "Runtime control updated.".into()
            } else {
                "Runtime control updated; the browser will receive it after reconnecting.".into()
            },
        }
    }

    /// Re-check supported development harnesses without changing their configuration.
    pub fn refresh_harnesses(&self) -> Result<Vec<HarnessSummary>, HarnessError> {
        self.harnesses.refresh()
    }

    /// Apply one explicit, ownership-checked development-harness registration intent.
    pub fn manage_harness(
        &self,
        id: &str,
        action: HarnessAction,
    ) -> Result<HarnessActionResult, HarnessError> {
        self.harnesses.apply(id, action)
    }

    /// Exercise the same best-effort notification port used for important domain facts.
    pub fn test_notification(&self) -> Result<(), WorkbenchError> {
        let port = lock(&self.projection.presentation)
            .clone()
            .ok_or(WorkbenchError::PresentationUnavailable)?;
        port.notify(WorkbenchNotification {
            kind: NotificationKind::Checkup,
            title: "Ghostlight checkup".into(),
            body: "Notifications are ready.".into(),
        })?;
        Ok(())
    }

    fn browser_summary(&self) -> Option<BrowserInstanceSummary> {
        self.browser.is_connected().then(|| BrowserInstanceSummary {
            id: self
                .browser
                .browser_id()
                .unwrap_or_else(|| "browser_unknown".into()),
            family: "Chromium".into(),
            adapter_version: self.browser.adapter_version(),
            connected: true,
        })
    }
}

/// Best-effort operating-system presentation port.
pub trait WorkbenchPresentationPort: Send + Sync {
    /// Deliver one high-signal, content-free notification.
    fn notify(&self, notification: WorkbenchNotification)
        -> Result<(), WorkbenchPresentationError>;
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
        /// The payload-free completion record.
        record: HistoryItem,
    },
    /// Authoritative runtime control state changed.
    RuntimeChanged {
        /// The new runtime control state.
        runtime_state: RuntimeControlState,
    },
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
    /// Work requires local human attention.
    Attention,
    /// The user explicitly tested notifications.
    Checkup,
}

/// Failure of the replaceable operating-system presentation adapter.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WorkbenchPresentationError {
    /// Native notification delivery failed.
    #[error("native presentation failed: {0}")]
    Native(String),
}

/// Failure at the typed workbench application boundary.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WorkbenchError {
    /// No native presentation adapter is currently attached.
    #[error("native presentation is unavailable")]
    PresentationUnavailable,
    /// Native presentation rejected the request.
    #[error(transparent)]
    Presentation(#[from] WorkbenchPresentationError),
}

/// Complete immutable workbench read model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkbenchSnapshot {
    /// Projection sequence this snapshot reflects.
    ///
    /// A surface applies a later change only when its sequence follows this one.
    pub seq: u64,
    /// Time at which this snapshot was assembled.
    pub generated_at_ms: u64,
    /// Service identity and control state.
    pub service: ServiceSummary,
    /// At-a-glance counts.
    pub overview: OverviewSummary,
    /// Admitted MCP sessions.
    pub sessions: Vec<SessionSummary>,
    /// Currently tracked operations.
    pub operations: Vec<OperationSummary>,
    /// Currently connected browser instances.
    pub browsers: Vec<BrowserInstanceSummary>,
    /// Newest-first bounded payload-free history.
    pub history: Vec<HistoryItem>,
    /// Current local checkup results.
    pub diagnostics: Vec<DiagnosticItem>,
    /// Cached, explicitly supported development-harness registrations.
    pub harnesses: Vec<HarnessSummary>,
    /// Content-free authority and runtime configuration facts.
    pub configuration: ConfigurationSummary,
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
    /// Presentation-only client label.
    pub client_label: String,
    /// Whether one invocation currently owns the workspace.
    pub leased: bool,
    /// Controlled tab count.
    pub tab_count: usize,
    /// Runtime-held tab count.
    pub held_tab_count: usize,
    /// Current operation count.
    pub active_operations: usize,
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
    /// Governed capability class this work required.
    pub capability: Capability,
    /// Local start time.
    pub started_at_ms: Option<u64>,
    /// Current semantic phase.
    pub phase: OperationPhase,
}

impl From<&OperationState> for OperationSummary {
    fn from(value: &OperationState) -> Self {
        Self {
            invocation: value.invocation.clone(),
            workspace: value.workspace.clone(),
            tool: value.tool.clone(),
            activity: activity_label(value.activity).into(),
            capability: value.capability,
            started_at_ms: Some(value.started_at_ms),
            phase: value.phase,
        }
    }
}

/// Closed current-operation phases.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationPhase {
    /// Work is active.
    Running,
    /// Runtime governance held work.
    Held,
    /// Local human attention is required.
    Attention,
    /// Work terminated as blocked before its audit projection arrived.
    Blocked,
    /// Work completed before its audit projection arrived.
    Completed,
}

impl OperationPhase {
    const fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Held => "held",
            Self::Attention => "attention",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
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
}

/// One payload-free terminal history record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HistoryItem {
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
    /// Whether authority admitted the final boundary.
    pub allowed: bool,
    /// Stable reason code.
    pub reason: String,
    /// Terminal status.
    pub status: String,
    /// Effect class.
    pub effect: String,
    /// Ghostlight-authored sentence naming what happened.
    pub summary: String,
    /// How long the work took, in milliseconds.
    pub duration_ms: u64,
}

impl From<AuditRecord> for HistoryItem {
    fn from(value: AuditRecord) -> Self {
        Self {
            timestamp_ms: value.timestamp_ms,
            invocation: value.invocation,
            workspace: value.workspace,
            tool: value.tool,
            capability: serde_json::to_value(value.capability)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".into()),
            allowed: value.allowed,
            reason: value.reason.as_str().into(),
            status: value.status,
            effect: value.effect,
            summary: value.summary,
            duration_ms: value.duration_ms,
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
    /// Whether local authority is valid.
    pub local_policy_valid: bool,
    /// Whether managed authority is configured.
    pub managed_authority_configured: bool,
    /// Whether managed authority is valid.
    pub managed_authority_valid: bool,
    /// Whether runtime control is backed by a configured file.
    pub runtime_control_file_configured: bool,
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
    /// Whether a currently connected browser received the new state immediately.
    pub browser_notified: bool,
    /// Fixed user-facing outcome.
    pub message: String,
}

/// One bounded global-search result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SearchHit {
    /// Result category.
    pub kind: SearchKind,
    /// Opaque result identity.
    pub id: String,
    /// Primary fixed or content-free label.
    pub title: String,
    /// Secondary content-free description.
    pub detail: String,
    /// Optional related time.
    pub timestamp_ms: Option<u64>,
    /// Workbench destination that owns this result.
    pub view: SearchDestination,
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
    /// Runtime and authority controls.
    Configuration,
    /// Supported harness registrations.
    Install,
}

fn push_hit(hits: &mut Vec<SearchHit>, query: &str, hit: SearchHit) {
    if hits.len() >= SEARCH_LIMIT {
        return;
    }
    let haystack = format!("{} {} {}", hit.id, hit.title, hit.detail).to_ascii_lowercase();
    if haystack.contains(query) {
        hits.push(hit);
    }
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

fn runtime_state_label(state: RuntimeControlState) -> &'static str {
    match state {
        RuntimeControlState::Active => "active",
        RuntimeControlState::Held => "paused",
        RuntimeControlState::Attention => "waiting for attention",
        RuntimeControlState::Ended => "ended",
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use ghostlight_bridge::browser::PresentationActivity;

    use crate::events::{DenialPresentation, DomainEvent};
    use crate::governance::{AuditRecord, AuditSink, Capability, Decision, GovernanceFacade};

    use super::{
        NotificationKind, ProjectingAuditSink, WorkbenchChange, WorkbenchEvent, WorkbenchEventSink,
        WorkbenchPresentationError, WorkbenchPresentationPort, WorkbenchProjection,
    };

    #[derive(Default)]
    struct Events(Mutex<Vec<WorkbenchEvent>>);

    impl WorkbenchEventSink for Events {
        fn publish(&self, event: WorkbenchEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    #[derive(Default)]
    struct MemoryAudit(Mutex<Vec<AuditRecord>>);

    impl AuditSink for MemoryAudit {
        fn record(&self, record: &AuditRecord) -> io::Result<()> {
            self.0.lock().unwrap().push(record.clone());
            Ok(())
        }
    }

    #[derive(Default)]
    struct Notifications(Mutex<Vec<super::WorkbenchNotification>>);

    impl WorkbenchPresentationPort for Notifications {
        fn notify(
            &self,
            notification: super::WorkbenchNotification,
        ) -> Result<(), WorkbenchPresentationError> {
            self.0.lock().unwrap().push(notification);
            Ok(())
        }
    }

    #[test]
    fn projection_tracks_current_work_then_moves_it_to_history() {
        let projection = WorkbenchProjection::default();
        projection.react(&DomainEvent::WorkStarted {
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            tool: "browser_read_page".into(),
            activity: PresentationActivity::Read,
            capability: Capability::Read,
        });
        assert_eq!(projection.operations().len(), 1);

        let durable = Arc::new(MemoryAudit::default());
        let sink = ProjectingAuditSink::new(durable.clone(), projection.clone());
        let governance = GovernanceFacade::new(None, None);
        let snapshot = governance.snapshot(&Default::default());
        let record = AuditRecord::now(
            "invocation_1",
            "workspace_1",
            "browser_read_page",
            Capability::Read,
            snapshot.id(),
            Decision {
                allowed: true,
                reason: crate::governance::ReasonCode::Permitted,
            },
            "succeeded",
            "none",
            "Page text read.",
            1200,
        );
        sink.record(&record).unwrap();

        assert!(projection.operations().is_empty());
        assert_eq!(projection.history()[0].tool, "browser_read_page");
        assert_eq!(durable.0.lock().unwrap().len(), 1);
    }

    #[test]
    fn operation_lifetime_publishes_one_gapless_sequence() {
        let projection = WorkbenchProjection::default();
        let events = Arc::new(Events::default());
        projection.attach_events(events.clone());

        projection.react(&DomainEvent::WorkStarted {
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            tool: "browser_fill_form".into(),
            activity: PresentationActivity::Fill,
            capability: Capability::Write,
        });
        projection.react(&DomainEvent::WorkCompleted {
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            physical_id: None,
        });
        let governance = GovernanceFacade::new(None, None);
        let authority = governance.snapshot(&Default::default());
        ProjectingAuditSink::new(Arc::new(MemoryAudit::default()), projection.clone())
            .record(&AuditRecord::now(
                "invocation_1",
                "workspace_1",
                "browser_fill_form",
                Capability::Write,
                authority.id(),
                Decision {
                    allowed: true,
                    reason: crate::governance::ReasonCode::Permitted,
                },
                "succeeded",
                "wrote",
                "Page text read.",
                1200,
            ))
            .unwrap();

        let published = events.0.lock().unwrap();
        assert_eq!(
            published.iter().map(|event| event.seq).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "a surface must be able to detect a gap"
        );
        assert_eq!(projection.current_seq(), 3);
        match &published[0].change {
            WorkbenchChange::OperationStarted { operation } => {
                assert_eq!(operation.tool, "browser_fill_form");
                assert_eq!(operation.capability, Capability::Write);
            }
            other => panic!("expected a started change, got {other:?}"),
        }
        assert!(matches!(
            published[1].change,
            WorkbenchChange::OperationChanged { .. }
        ));
        assert!(matches!(
            published[2].change,
            WorkbenchChange::OperationSettled { .. }
        ));
    }

    #[test]
    fn published_changes_stay_payload_free() {
        let projection = WorkbenchProjection::default();
        let events = Arc::new(Events::default());
        projection.attach_events(events.clone());
        projection.react(&DomainEvent::WorkStarted {
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            tool: "browser_run_script".into(),
            activity: PresentationActivity::Script,
            capability: Capability::Execute,
        });

        let published = events.0.lock().unwrap();
        let encoded = serde_json::to_string(&published[0]).unwrap();
        for forbidden in ["url", "selector", "content", "password"] {
            assert!(!encoded.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn a_projection_without_a_sink_publishes_nothing_and_stays_at_zero() {
        let projection = WorkbenchProjection::default();
        projection.react(&DomainEvent::WorkStarted {
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            tool: "browser_read_page".into(),
            activity: PresentationActivity::Read,
            capability: Capability::Read,
        });
        assert_eq!(projection.current_seq(), 0);
        assert_eq!(projection.operations().len(), 1);
    }

    #[test]
    fn blocked_notifications_are_content_free_and_deduplicated() {
        let projection = WorkbenchProjection::default();
        let notifications = Arc::new(Notifications::default());
        projection.attach_presentation(notifications.clone());
        let event = DomainEvent::WorkBlocked {
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            physical_id: Some(7),
            presentation: DenialPresentation::Guardrail,
        };
        projection.react(&event);
        projection.react(&event);

        let notifications = notifications.0.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].kind, NotificationKind::Blocked);
        let encoded = serde_json::to_string(&notifications[0]).unwrap();
        for forbidden in ["url", "selector", "content", "workspace_1", "invocation_1"] {
            assert!(!encoded.contains(forbidden));
        }
    }

    #[test]
    fn presentation_failure_does_not_change_projection() {
        struct Failing;
        impl WorkbenchPresentationPort for Failing {
            fn notify(
                &self,
                _notification: super::WorkbenchNotification,
            ) -> Result<(), WorkbenchPresentationError> {
                Err(WorkbenchPresentationError::Native("injected".into()))
            }
        }
        let projection = WorkbenchProjection::default();
        projection.attach_presentation(Arc::new(Failing));
        projection.react(&DomainEvent::AttentionRequired {
            invocation: "invocation_1".into(),
            workspace: "workspace_1".into(),
            physical_id: None,
        });
        projection.react(&DomainEvent::WorkStarted {
            invocation: "invocation_2".into(),
            workspace: "workspace_1".into(),
            tool: "browser_list_tabs".into(),
            activity: PresentationActivity::Read,
            capability: Capability::Read,
        });
        assert_eq!(projection.operations().len(), 1);
    }
}
