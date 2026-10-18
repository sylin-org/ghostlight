//! The parts of the workbench state a screen may ask for, and what each audience sees of them.
//!
//! A snapshot is cut into closed parts: status, connections, activity, integrations, diagnostics,
//! controls and setup. Policy has its own route. Each part is projected for an [`Audience`] when
//! the response is written, so a page never redacts and never holds data its context may not see
//! (ADR-0199, amendment of 2026-10-15). The projection drops what nobody reads, shows home-relative
//! paths to everyone, and turns the person's own lists of sensitive sites into counts for a watcher.
//! Setup is the window's alone: a watcher is shown the ordinary workbench while setup is under way
//! (ADR-0200, JQ5), so the part is simply not part of what a watcher is shown.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::context::{availability, Availability, WebAct, WebContext};
use super::{
    BrowserInstanceSummary, DiagnosticItem, HistoryItem, OperationSummary, ReadinessSummary,
    ServiceSummary, SessionSummary, WorkbenchChange, WorkbenchEvent, WorkbenchSnapshot,
};
use crate::browser::adapter::RuntimeControlState;
use crate::diagnostics::DiagnosticsReport;
use crate::governance::effective::{EffectiveAuthority, LayerKind, PolicyChip};
use crate::governance::ManagedPolicyPassport;
use crate::install::{HarnessActionResult, HarnessState, HarnessSummary};
use crate::language::audit_health::AuditHealth;
use crate::language::coverage::HumanCoverage;
use crate::language::setup::SetupView;
use crate::language::workbench::{self as words, ConnectionCounts};

/// The environment variable that names the user's home directory.
#[cfg(windows)]
pub const USER_HOME_VARIABLE: &str = "USERPROFILE";

/// The environment variable that names the user's home directory.
#[cfg(not(windows))]
pub const USER_HOME_VARIABLE: &str = "HOME";

/// The setting key of the person's list of sites agents must never touch.
pub const SACRED_DOMAINS_KEY: &str = "content.security.sacred_domains";

/// The etag of a policy that has no user layer.
pub const NO_POLICY_ETAG: &str = "none";

/// Who a projection is made for. Closed contexts never receive data, so they have no audience.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Audience {
    /// The Ghostlight window.
    Control,
    /// Any other screen on this computer while watching is on.
    Watch,
}

impl Audience {
    /// The context a request is in when a projection is made for this audience.
    #[must_use]
    pub const fn context(self) -> WebContext {
        match self {
            Self::Control => WebContext::Control,
            Self::Watch => WebContext::Watch,
        }
    }
}

/// One closed slice of the snapshot a screen can ask for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Part {
    /// Readiness, service facts, saving health and the policy chip.
    Status,
    /// Admitted sessions and connected browsers.
    Connections,
    /// Live operations, history and document coverage.
    Activity,
    /// The supported clients and their registrations.
    Integrations,
    /// Checkup cards and process diagnostics.
    Diagnostics,
    /// The controls Settings is drawn from: pause and the session, who can watch, process diagnostics and
    /// the notification test, with what each context may do.
    Controls,
    /// The install journey: where the person is and what comes next. The window alone is shown it.
    Setup,
}

/// A part token the service does not know.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("unknown part token")]
pub struct UnknownPart;

impl Part {
    /// Every part, in the order a snapshot lists them.
    pub const ALL: [Self; 7] = [
        Self::Status,
        Self::Connections,
        Self::Activity,
        Self::Integrations,
        Self::Diagnostics,
        Self::Controls,
        Self::Setup,
    ];

    /// The snake_case token that names this part on the wire.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Connections => "connections",
            Self::Activity => "activity",
            Self::Integrations => "integrations",
            Self::Diagnostics => "diagnostics",
            Self::Controls => "controls",
            Self::Setup => "setup",
        }
    }

    /// The part a token names.
    pub fn from_token(token: &str) -> Result<Self, UnknownPart> {
        Self::ALL
            .into_iter()
            .find(|part| part.token() == token)
            .ok_or(UnknownPart)
    }
}

/// A part whose change is announced as a hint: the page reads it again if it shows it.
///
/// Policy has its own route, and integrations, diagnostics and setup are parts of a snapshot. None
/// of them travels in an event, because each can be large or private to the window.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HintedPart {
    /// The supported clients and their registrations.
    Integrations,
    /// The compiled policy.
    Policy,
    /// Checkup cards and process diagnostics.
    Diagnostics,
    /// The install journey. A watcher is told that it changed and never what it says.
    Setup,
}

/// The status part: readiness, service facts, saving health and the policy chip.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StatusPart {
    /// Service identity and control state.
    pub service: ServiceSummary,
    /// The one aggregate answer and its words.
    pub readiness: ReadinessSummary,
    /// Current storage health and explicit historical gaps.
    pub audit_health: AuditHealth,
    /// Persistent human notice, empty during ordinary healthy operation.
    pub audit_notice: String,
    /// The band chip and the passport of a signed organization policy.
    pub configuration: StatusConfiguration,
    /// Where the pause came from, present only while agents are paused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_note: Option<String>,
}

/// The configuration facts the status part carries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StatusConfiguration {
    /// The band chip, authored by the orchestrator.
    pub policy: PolicyChip,
    /// Signed managed-policy provenance.
    pub managed_policy: ManagedPolicyPassport,
}

impl StatusPart {
    /// The status part of `snapshot`.
    #[must_use]
    pub fn of(snapshot: &WorkbenchSnapshot) -> Self {
        Self {
            service: snapshot.service.clone(),
            readiness: snapshot.readiness.clone(),
            audit_health: snapshot.audit_health.clone(),
            audit_notice: snapshot.audit_notice.clone(),
            configuration: StatusConfiguration {
                policy: snapshot.configuration.policy.clone(),
                managed_policy: snapshot.configuration.managed_policy.clone(),
            },
            runtime_note: snapshot.runtime_note.clone(),
        }
    }
}

/// The connections part: admitted sessions and connected browsers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConnectionsPart {
    /// Admitted MCP sessions.
    pub sessions: Vec<SessionSummary>,
    /// Currently connected browser instances.
    pub browsers: Vec<BrowserInstanceSummary>,
    /// What the window says in place of a browser, present only while none is connected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_browser: Option<String>,
    /// The ids of the client roster's rows in use now ([`harnesses_in_use`]), in roster order.
    /// The Integrations page reads it beside the sessions it is decided from, so it changes when
    /// they do.
    pub harnesses_in_use: Vec<String>,
}

impl ConnectionsPart {
    /// The connections part of `snapshot`.
    #[must_use]
    pub fn of(snapshot: &WorkbenchSnapshot) -> Self {
        Self {
            sessions: snapshot.sessions.clone(),
            browsers: snapshot.browsers.clone(),
            no_browser: snapshot.no_browser.clone(),
            harnesses_in_use: harnesses_in_use(&snapshot.harnesses, &snapshot.sessions),
        }
    }
}

/// Whether the roster row `harness` is in use now: a live session of its agent, with the row's
/// entry in place. A copy-only row's file is never read, so for it the session is the only sign of
/// the entry. A session says its agent, not the host it runs on ([`is_agent_of`]), so a session
/// of an agent marks each of that agent's rows with an entry, and none without one.
///
/// The one rule the setup view and the Integrations page both read.
#[must_use]
pub(crate) fn in_use(harness: &HarnessSummary, sessions: &[SessionSummary]) -> bool {
    (harness.copy_only || harness.state == HarnessState::Installed)
        && sessions.iter().any(|session| is_agent_of(session, harness))
}

/// Whether `session` is a session of the agent `harness` sets up: both carry the same catalog id.
/// An agent on several hosts is one agent here, because the name a client reports does not say
/// the host.
#[must_use]
pub(crate) fn is_agent_of(session: &SessionSummary, harness: &HarnessSummary) -> bool {
    session.agent.id == harness.agent.id
}

/// The ids of the rows of `harnesses` that are [`in_use`] given `sessions`, in roster order.
#[must_use]
pub(crate) fn harnesses_in_use(
    harnesses: &[HarnessSummary],
    sessions: &[SessionSummary],
) -> Vec<String> {
    harnesses
        .iter()
        .filter(|harness| in_use(harness, sessions))
        .map(|harness| harness.id.clone())
        .collect()
}

/// The layer name of process diagnostics that are not being written.
const DIAGNOSTICS_OFF_LAYER: &str = "off";

/// What the service knows of the watch view Settings speaks for.
#[derive(Clone, Copy, Debug)]
pub enum ViewingFacts<'a> {
    /// No watch view is served, so nobody can watch and the setting cannot change.
    NotServed,
    /// A watch view is served at `url`.
    Served {
        /// Whether other screens on this computer may watch.
        watching: bool,
        /// Where the watch view is served.
        url: &'a str,
    },
}

/// The facts the controls part is made from, before any context has seen them.
#[derive(Clone, Copy, Debug)]
pub struct ControlsFacts<'a> {
    /// Whether agents may ask for browser work.
    pub runtime_state: RuntimeControlState,
    /// Where a pause came from, while agents are paused.
    pub runtime_note: Option<&'a str>,
    /// Whether a watch view is served, and whether others may watch it.
    pub viewing: ViewingFacts<'a>,
    /// The process-diagnostics state.
    pub diagnostics: &'a DiagnosticsReport,
    /// What is connected to the service.
    pub counts: ConnectionCounts,
    /// The user's home directory, which the diagnostics folder is shown relative to.
    pub user_home: Option<&'a Path>,
}

/// The controls part: the state and acts Settings is drawn from.
///
/// Every Ghostlight web app draws the same Settings page from this part. Each act carries whether
/// the context it was made for may do it, and `you` names that context, so a screen draws what it
/// is told and never guesses. The part the service holds is the Control form; [`Self::seen_by`]
/// makes the form another context sees.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ControlsPart {
    /// Agents' browser work: whether it runs, and how to pause, resume, start and end it.
    pub runtime: RuntimeControls,
    /// "Others can watch".
    pub viewing: ViewingControls,
    /// Process diagnostics.
    pub diagnostics: DiagnosticsControls,
    /// The notification test.
    pub notifications: NotificationControls,
    /// The one sentence about the service.
    pub service: ServiceControls,
    /// The context the part was made for.
    pub you: YouControls,
}

/// The runtime row of Settings, under Browser work.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeControls {
    /// The row's label.
    pub label: &'static str,
    /// The authoritative runtime state.
    pub state: RuntimeControlState,
    /// The word for the state.
    pub word: &'static str,
    /// Where a pause came from, while agents are paused.
    pub note: Option<String>,
    /// What the context may do.
    pub acts: RuntimeActs,
}

/// The acts of the runtime row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeActs {
    /// Pause agents.
    pub hold: Availability,
    /// Let agents work again.
    pub resume: Availability,
    /// Start a fresh session.
    pub start_session: Availability,
    /// End the session.
    pub end_session: Availability,
}

/// The "Others can watch" row of Settings, under Sharing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewingControls {
    /// The row's label.
    pub label: &'static str,
    /// Whether other screens on this computer may watch.
    pub watch: bool,
    /// Where the watch view is served, when a server is up.
    pub url: Option<String>,
    /// The word for the state.
    pub word: &'static str,
    /// What the context may do.
    pub acts: ViewingActs,
    /// Whether a watch view is served, which decides whether watching can be changed at all.
    #[serde(skip)]
    served: bool,
}

/// The acts of the "Others can watch" row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewingActs {
    /// Turn watching on or off.
    pub set_watching: Availability,
}

/// The process-diagnostics row of Settings, under Diagnostics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosticsControls {
    /// The row's label.
    pub label: &'static str,
    /// Whether process diagnostics are being written.
    pub on: bool,
    /// What turned them on, or `off`.
    pub layer: &'static str,
    /// Bytes of diagnostics logs on disk.
    pub used_bytes: u64,
    /// The folder the logs live in, relative to the user's home.
    pub folder: Option<String>,
    /// The word for the state.
    pub word: String,
    /// What the context may do.
    pub acts: DiagnosticsActs,
}

/// The acts of the process-diagnostics row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosticsActs {
    /// Turn process diagnostics on or off.
    pub set_diagnostics: Availability,
    /// Show the log folder in the file manager.
    pub reveal_diagnostics: Availability,
}

/// The notifications row of Settings, under Notifications.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NotificationControls {
    /// The row's label.
    pub label: &'static str,
    /// What the test does.
    pub word: &'static str,
    /// What the context may do.
    pub acts: NotificationActs,
}

/// The acts of the notifications row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NotificationActs {
    /// Show a test notification.
    pub test_notification: Availability,
}

/// The service line, which Status shows.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ServiceControls {
    /// The sentence about the service and what is connected to it.
    pub word: String,
}

/// Who the Controls part was made for, as the band's context chip says it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct YouControls {
    /// The context's token.
    pub context: &'static str,
    /// The short word for the context.
    pub word: &'static str,
    /// The sentence that says who can see this view.
    pub sentence: &'static str,
}

impl ControlsPart {
    /// The Controls part for the window, made from `facts`.
    #[must_use]
    pub fn of(facts: &ControlsFacts<'_>) -> Self {
        let report = facts.diagnostics;
        let on = report.layer != DIAGNOSTICS_OFF_LAYER;
        // Every act starts as unavailable; `address` then fills each one for the context.
        let unavailable = Availability {
            available: false,
            why_not: None,
        };
        let mut part = Self {
            runtime: RuntimeControls {
                label: words::CONTROLS_RUNTIME_LABEL,
                state: facts.runtime_state,
                word: match facts.runtime_state {
                    RuntimeControlState::Active => words::CONTROLS_RUNTIME_ACTIVE,
                    RuntimeControlState::Held => words::CONTROLS_RUNTIME_HELD,
                    RuntimeControlState::Ended => words::CONTROLS_RUNTIME_ENDED,
                },
                note: facts.runtime_note.map(str::to_owned),
                acts: RuntimeActs {
                    hold: unavailable.clone(),
                    resume: unavailable.clone(),
                    start_session: unavailable.clone(),
                    end_session: unavailable.clone(),
                },
            },
            viewing: match facts.viewing {
                ViewingFacts::NotServed => ViewingControls {
                    label: words::CONTROLS_VIEWING_LABEL,
                    watch: false,
                    url: None,
                    word: words::WATCHING_NOT_SERVED,
                    acts: ViewingActs {
                        set_watching: unavailable.clone(),
                    },
                    served: false,
                },
                ViewingFacts::Served { watching, url } => ViewingControls {
                    label: words::CONTROLS_VIEWING_LABEL,
                    watch: watching,
                    url: Some(url.to_owned()),
                    word: if watching {
                        words::CONTROLS_VIEWING_ON
                    } else {
                        words::CONTROLS_VIEWING_OFF
                    },
                    acts: ViewingActs {
                        set_watching: unavailable.clone(),
                    },
                    served: true,
                },
            },
            diagnostics: DiagnosticsControls {
                label: words::CONTROLS_DIAGNOSTICS_LABEL,
                on,
                layer: report.layer,
                used_bytes: report.used_bytes,
                folder: report
                    .directory
                    .as_deref()
                    .map(|folder| match facts.user_home {
                        Some(home) => display_path(home, folder),
                        None => folder.to_owned(),
                    }),
                word: if on {
                    words::controls_diagnostics_on(report.layer)
                } else {
                    words::CONTROLS_DIAGNOSTICS_OFF.to_owned()
                },
                acts: DiagnosticsActs {
                    set_diagnostics: unavailable.clone(),
                    reveal_diagnostics: unavailable.clone(),
                },
            },
            notifications: NotificationControls {
                label: words::CONTROLS_NOTIFICATIONS_LABEL,
                word: words::CONTROLS_NOTIFICATIONS_WORD,
                acts: NotificationActs {
                    test_notification: unavailable,
                },
            },
            service: ServiceControls {
                word: words::service_line(Some(facts.counts)),
            },
            you: YouControls {
                context: WebContext::Control.token(),
                word: WebContext::Control.word(),
                sentence: "",
            },
        };
        part.address(WebContext::Control);
        part
    }

    /// The same part as `context` sees it: the acts it may do, and who it is.
    #[must_use]
    pub fn seen_by(&self, context: WebContext) -> Self {
        let mut seen = self.clone();
        seen.address(context);
        seen
    }

    /// Fill every act and `you` for `context`.
    fn address(&mut self, context: WebContext) {
        let may = |act| availability(act, context);
        self.runtime.acts = RuntimeActs {
            hold: may(WebAct::Hold),
            resume: may(WebAct::Resume),
            start_session: may(WebAct::StartSession),
            end_session: may(WebAct::EndSession),
        };
        let set_watching = if self.viewing.served {
            may(WebAct::SetWatching)
        } else {
            Availability {
                available: false,
                why_not: Some(words::WATCHING_NOT_SERVED.to_owned()),
            }
        };
        self.viewing.acts = ViewingActs { set_watching };
        self.diagnostics.acts = DiagnosticsActs {
            set_diagnostics: may(WebAct::SetDiagnostics),
            reveal_diagnostics: may(WebAct::RevealDiagnostics),
        };
        self.notifications.acts = NotificationActs {
            test_notification: may(WebAct::TestNotification),
        };
        self.you = YouControls {
            context: context.token(),
            word: context.word(),
            sentence: context.sentence(self.viewing.watch),
        };
    }
}

#[derive(Serialize)]
struct ActivityPart<'a> {
    operations: &'a [OperationSummary],
    history: &'a [HistoryItem],
    document_coverage: &'a [HumanCoverage],
}

#[derive(Serialize)]
struct IntegrationsPart<'a> {
    harnesses: &'a [HarnessSummary],
}

#[derive(Serialize)]
struct DiagnosticsPart<'a> {
    diagnostics: &'a [DiagnosticItem],
}

/// What one audience sees, made for one user home.
#[derive(Clone, Debug)]
pub struct Projection {
    audience: Audience,
    user_home: Option<PathBuf>,
}

impl Projection {
    /// A projection for `audience` that shows paths relative to the user's home directory.
    #[must_use]
    pub fn new(audience: Audience) -> Self {
        Self::with_home(audience, user_home())
    }

    /// A projection for `audience` over an explicit user home. Without one, paths stay as they are.
    #[must_use]
    pub const fn with_home(audience: Audience, user_home: Option<PathBuf>) -> Self {
        Self {
            audience,
            user_home,
        }
    }

    /// Whether this projection is made for a watcher.
    const fn watching(&self) -> bool {
        matches!(self.audience, Audience::Watch)
    }

    fn path(&self, path: &str) -> String {
        match &self.user_home {
            Some(home) => display_path(home, path),
            None => path.to_owned(),
        }
    }

    fn rewrite_path(&self, value: &mut Value) {
        if let Some(path) = value.as_str() {
            *value = Value::String(self.path(path));
        }
    }

    /// Whether this audience is shown `part` at all. A route asks before it asks for the part, and
    /// leaves out of its answer a part the audience is not shown.
    #[must_use]
    pub const fn shows(&self, part: Part) -> bool {
        !(self.watching() && matches!(part, Part::Setup))
    }

    /// One part of `snapshot`, as this audience sees it. A part the audience is not shown (see
    /// [`Self::shows`]) comes back null.
    ///
    /// The setup part is not in a snapshot: `setup` builds it, and is called only when this
    /// audience is shown the part, because building it may read this computer.
    #[must_use]
    pub fn part(
        &self,
        part: Part,
        snapshot: &WorkbenchSnapshot,
        setup: impl FnOnce() -> SetupView,
    ) -> Value {
        match part {
            Part::Status => self.status(&StatusPart::of(snapshot)),
            Part::Connections => self.connections(&ConnectionsPart::of(snapshot)),
            Part::Activity => {
                let mut value = to_value(&ActivityPart {
                    operations: &snapshot.operations,
                    history: &snapshot.history,
                    document_coverage: &snapshot.document_coverage,
                });
                each(&mut value, "document_coverage", |coverage| {
                    self.count_excluded_hosts(coverage);
                });
                value
            }
            Part::Integrations => {
                let mut value = to_value(&IntegrationsPart {
                    harnesses: &snapshot.harnesses,
                });
                each(&mut value, "harnesses", |harness| {
                    self.project_harness(harness);
                });
                value
            }
            Part::Controls => self.controls(&snapshot.controls),
            Part::Setup if !self.shows(part) => Value::Null,
            Part::Setup => to_value(&setup()),
            Part::Diagnostics => to_value(&DiagnosticsPart {
                diagnostics: &snapshot.diagnostics,
            }),
        }
    }

    /// The status part, as this audience sees it. Every audience sees all of it.
    #[must_use]
    pub fn status(&self, status: &StatusPart) -> Value {
        to_value(status)
    }

    /// The connections part, as this audience sees it. Every audience sees all of it.
    #[must_use]
    pub fn connections(&self, connections: &ConnectionsPart) -> Value {
        to_value(connections)
    }

    /// The Controls part, as this audience sees it: the acts it may do, and who it is.
    #[must_use]
    pub fn controls(&self, controls: &ControlsPart) -> Value {
        to_value(&controls.seen_by(self.audience.context()))
    }

    /// One published event, as this audience sees it.
    ///
    /// The stream projects an event once for each audience that has a subscriber, with the same
    /// rules as the routes: a watcher is told how many sites a document excluded and never which,
    /// and is given no policy or setup tag to test guesses against.
    #[must_use]
    pub fn event(&self, event: &WorkbenchEvent) -> Value {
        let mut value = to_value(event);
        let Some(change) = value.get_mut("change") else {
            return value;
        };
        match &event.change {
            WorkbenchChange::DocumentCoverageChanged { .. } => {
                if let Some(details) = change.get_mut("details") {
                    self.count_excluded_hosts(details);
                }
            }
            WorkbenchChange::StatusChanged { status } => {
                change["status"] = self.status(status);
            }
            WorkbenchChange::ConnectionsChanged { connections } => {
                change["connections"] = self.connections(connections);
            }
            WorkbenchChange::ControlsChanged { controls } => {
                change["controls"] = self.controls(controls);
            }
            WorkbenchChange::PartChanged {
                part: HintedPart::Policy | HintedPart::Setup,
                ..
            } if self.watching() => remove(change, &["etag"]),
            _ => {}
        }
        value
    }

    /// The excluded sites of one document, which only the window may name.
    fn count_excluded_hosts(&self, coverage: &mut Value) {
        let count = coverage
            .get("excluded_hosts")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        if self.watching() {
            remove(coverage, &["excluded_hosts"]);
        }
        if let Some(object) = coverage.as_object_mut() {
            object.insert("excluded_host_count".into(), json!(count));
        }
    }

    fn project_harness(&self, harness: &mut Value) {
        remove(harness, &["private_view"]);
        for key in ["config_path", "other_copy"] {
            if let Some(path) = harness.get_mut(key) {
                self.rewrite_path(path);
            }
        }
    }

    /// The process-diagnostics state, with its folder shown relative to the user's home.
    #[must_use]
    pub fn diagnostics_report(&self, report: &crate::diagnostics::DiagnosticsReport) -> Value {
        let mut value = to_value(report);
        if let Some(directory) = value.get_mut("directory") {
            self.rewrite_path(directory);
        }
        value
    }

    /// The supported clients, as this audience sees them.
    #[must_use]
    pub fn harnesses(&self, summaries: &[HarnessSummary]) -> Value {
        let mut value = to_value(&summaries);
        if let Some(items) = value.as_array_mut() {
            for item in items {
                self.project_harness(item);
            }
        }
        value
    }

    /// The result of one client act, with its summary projected like any other.
    #[must_use]
    pub fn harness_result(&self, result: &HarnessActionResult) -> Value {
        let mut value = to_value(result);
        if let Some(summary) = value.get_mut("summary") {
            self.project_harness(summary);
        }
        value
    }

    /// The compiled policy, as this audience sees it. Control also gets the etag of the user
    /// layer, which the policy write routes check; a watcher never writes and is not given a
    /// value it could test guesses about the withheld never-touch list against.
    #[must_use]
    pub fn policy(&self, authority: &EffectiveAuthority) -> Value {
        let mut value = to_value(authority);
        let mut never_touch: Vec<String> = Vec::new();
        each(&mut value, "layers", |layer| {
            if let Some(path) = layer.get_mut("path") {
                self.rewrite_path(path);
            }
            if self.watching() {
                layer["document"] = Value::Null;
            }
            each(layer, "settings", |setting| {
                if setting["key"] != SACRED_DOMAINS_KEY {
                    return;
                }
                let hosts = setting["value"]
                    .as_str()
                    .and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
                    .unwrap_or_default();
                if self.watching() {
                    setting["value"] = Value::String(hosts.len().to_string());
                    setting["count"] = json!(hosts.len());
                    never_touch.extend(hosts);
                }
            });
        });
        if let Some(path) = value.pointer_mut("/user_layer/path") {
            self.rewrite_path(path);
        }
        if self.watching() {
            Self::count_never_touch_ceilings(&mut value, &never_touch);
        }
        if !self.watching() {
            value["etag"] = Value::String(policy_etag(authority));
        }
        value
    }

    /// Replace the ceiling lines that name the person's never-touch sites with one count.
    fn count_never_touch_ceilings(policy: &mut Value, hosts: &[String]) {
        let Some(lines) = policy.get_mut("ceilings").and_then(Value::as_array_mut) else {
            return;
        };
        let before = lines.len();
        lines.retain(|line| {
            let text = line.as_str().unwrap_or_default();
            !hosts.iter().any(|host| {
                text.strip_prefix(host.as_str())
                    .is_some_and(|rest| rest.starts_with(','))
            })
        });
        if lines.len() < before {
            lines.push(Value::String(words::never_touch_count(hosts.len())));
        }
    }
}

fn to_value<T: Serialize + ?Sized>(value: &T) -> Value {
    serde_json::to_value(value).expect("a workbench part is plain data")
}

/// Run `edit` on every element of the array under `key`, when there is one.
fn each(value: &mut Value, key: &str, mut edit: impl FnMut(&mut Value)) {
    if let Some(items) = value.get_mut(key).and_then(Value::as_array_mut) {
        items.iter_mut().for_each(&mut edit);
    }
}

fn remove(value: &mut Value, keys: &[&str]) {
    if let Some(object) = value.as_object_mut() {
        for key in keys {
            object.remove(*key);
        }
    }
}

/// The user's home directory, when the environment names one.
#[must_use]
pub fn user_home() -> Option<PathBuf> {
    std::env::var_os(USER_HOME_VARIABLE)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// A path as `~` plus what lies below the user's home, with the platform's separator.
///
/// The prefix match ignores case on Windows, where either separator matches. A path outside the
/// home stays as it is.
#[must_use]
pub fn display_path(user_home: &Path, path: &str) -> String {
    let home = user_home.to_string_lossy();
    let home = home.trim_end_matches(is_separator);
    if home.is_empty() || path.len() < home.len() || !path.is_char_boundary(home.len()) {
        return path.to_owned();
    }
    let (prefix, rest) = path.split_at(home.len());
    if !same_prefix(prefix, home) || !(rest.is_empty() || rest.starts_with(is_separator)) {
        return path.to_owned();
    }
    let rest: String = rest
        .chars()
        .map(|character| {
            if is_separator(character) {
                std::path::MAIN_SEPARATOR
            } else {
                character
            }
        })
        .collect();
    format!("~{rest}")
}

fn same_prefix(left: &str, right: &str) -> bool {
    left.chars().count() == right.chars().count()
        && left.chars().zip(right.chars()).all(|(a, b)| {
            if is_separator(a) || is_separator(b) {
                is_separator(a) && is_separator(b)
            } else if cfg!(windows) {
                a.eq_ignore_ascii_case(&b)
            } else {
                a == b
            }
        })
}

fn is_separator(character: char) -> bool {
    character == '/' || (cfg!(windows) && character == '\\')
}

/// The tag a write must quote to change the person's own rules.
///
/// It is a hash of the user layer's document, parsed, without its top-level `version` and with
/// sorted keys, so a reformatted or re-versioned file keeps its tag. A machine with no user layer
/// has the tag [`NO_POLICY_ETAG`].
#[must_use]
pub fn policy_etag(authority: &EffectiveAuthority) -> String {
    let document = authority
        .layers
        .iter()
        .find(|layer| layer.kind == LayerKind::User)
        .and_then(|layer| layer.document.as_deref());
    let Some(document) = document else {
        return NO_POLICY_ETAG.to_owned();
    };
    let text = match serde_json::from_str::<Value>(document) {
        Ok(mut parsed) => {
            if let Some(object) = parsed.as_object_mut() {
                object.remove("version");
            }
            let mut text = String::new();
            write_sorted(&parsed, &mut text);
            text
        }
        Err(_) => document.to_owned(),
    };
    hex_digest(text.as_bytes())
}

/// The tag of a value that is read on demand: a hash of its JSON.
#[must_use]
pub fn json_etag<T: Serialize + ?Sized>(value: &T) -> String {
    hex_digest(to_value(value).to_string().as_bytes())
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_sorted(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_sorted(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_sorted(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

#[cfg(test)]
mod tests;
