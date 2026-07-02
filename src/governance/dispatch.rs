//! Tool-call dispatch chokepoint -- the single Policy Enforcement Point (PEP).
//!
//! Every `tools/call` passes through [`Governance::decide`] exactly once, before the tool
//! executes, and through [`Governance::record_call`] exactly once after it resolves. The
//! [`Governance`] facade holds the governance ports (a
//! [`PolicyDecisionPoint`](crate::governance::ports::PolicyDecisionPoint), an
//! [`AuditSink`](crate::governance::ports::AuditSink), and later the browser plugin halves) and is
//! the one place the stage-2 overlay attaches.
//!
//! [`Governance::all_open`] is the ungoverned engine: its decide path is a literal STEP-0
//! short-circuit to [`Decision::Allow`](crate::governance::ports::Decision) that queries no port and
//! resolves no resource, so a session with no manifest and default config is byte-identical to
//! stage 1 (ADR-0013). Audit is orthogonal to that STEP-0 short-circuit (shared format doc
//! section 4.5: the flight recorder still records under all-open when `audit.enabled` is true), so
//! the audit sink is a field of `Governance` itself, not nested inside the governed-only state.
//!
//! `classify` is injected as a function pointer rather than named directly: this module lives in
//! the domain-agnostic governance core, and the concrete tool+action classification table is
//! browser-domain (`browser::classify`, g05's RECONCILIATION-driven placement; the a7 arch-test
//! forbids a `governance -> browser` edge). The crate-root binary supplies the browser plugin's
//! real classifier at construction.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use crate::governance::ports::{
    AuditRecord, AuditSink, ClientInfo, Decision, DecisionRequest, Denial, EffectiveMode,
    GoverningResource, PolicyDecisionPoint, RwClass, SessionEventRecord,
};

/// How long a take-the-wheel hold may last before [`hold_message`] appends the resume hint
/// (g10, ADR-0018 step 2). A constant for now; a future registry key
/// (`engine.hold.hint_after_ms`) may make it configurable -- not this task's job.
pub const HOLD_HINT_AFTER: Duration = Duration::from_secs(120);

/// The take-the-wheel pause reply for a held tool call (g10, ADR-0018 step 2): a plain,
/// truthful statement that the call was NOT executed, why, and what the agent should do
/// (stop and wait, never retry-spin), rendered as a normal successful MCP text result --
/// never an error, never a hint that the action happened. `action` is the `computer`
/// sub-action, rendering the label `computer (<action>)`; every other tool renders its bare
/// name (mirrors the denial-format convention, shared format doc section 7.2). Past
/// [`HOLD_HINT_AFTER`], a second sentence names the only way to resume: the user, from the
/// extension.
pub fn hold_message(tool: &str, action: Option<&str>, held_for: Duration) -> String {
    let label = match (tool, action) {
        ("computer", Some(action)) => format!("computer ({action})"),
        _ => tool.to_string(),
    };
    let mut message = format!(
        "Paused: the user has taken control of the browser (take-the-wheel). The '{label}' \
         call was NOT executed. This is not an error, and retrying will not help: every \
         browser tool call receives this same reply until the user resumes. Stop issuing \
         browser tool calls, tell the user the session is paused and you are waiting, and \
         continue only after the user says they have resumed."
    );
    if held_for >= HOLD_HINT_AFTER {
        message.push(' ');
        message.push_str(
            "This session has been paused for more than 2 minutes. Only the user can resume \
             it, from the Browser MCP extension: the popup Pause/Resume button or the toggle \
             keyboard shortcut.",
        );
    }
    message
}

/// The governance facade held at the dispatch chokepoint: the Policy Enforcement Point.
///
/// One instance lives for the whole MCP session. The decision path is either the ungoverned
/// engine ([`Governance::all_open`], holding no decision port) or a governed overlay holding the
/// ports; the audit sink and client identity are held regardless, since recording is orthogonal
/// to whether a manifest is active.
pub struct Governance {
    mode: Mode,
    /// Always present; `NullSink` when audit is disabled. Recording is orthogonal to
    /// [`Mode`] (shared format doc section 4.5).
    audit: Arc<dyn AuditSink>,
    /// The browser plugin's tool+action -> observe/mutate table, injected so this core module
    /// never names the browser plugin directly.
    classify: fn(&str, Option<&str>) -> Option<RwClass>,
    /// The MCP client identity captured from the `initialize` request, first-wins for the
    /// whole session (shared format doc section 6.1 `client` field).
    client: Mutex<Option<ClientInfo>>,
}

/// The two shapes of the decision path. `AllOpen` holds nothing so its decide path is a
/// zero-cost short-circuit; `Governed` holds the decision port later tasks drive.
enum Mode {
    /// STEP-0: the ungoverned engine. No manifest, default config. Every call is `Allow`.
    AllOpen,
    /// The governed overlay. Populated by later stage-2 tasks; the pure/impure browser plugin
    /// halves (DomainPolicy classify/match, ResourceResolver) attach through builder methods added
    /// by G07/G13.
    Governed(GovernedState),
}

/// The decision port a governed facade holds. `dyn` here is deliberate: the decision point has
/// multiple impls (Noop today, Local in stage 2, a future Remote).
struct GovernedState {
    pdp: Box<dyn PolicyDecisionPoint>,
}

impl Governance {
    /// The ungoverned engine: a zero-port decision path whose decide path short-circuits to
    /// `Allow`, paired with an audit sink built independently from config (audit is orthogonal
    /// to all-open). This is the facade used in production until the manifest task lands.
    pub fn all_open(
        audit: Arc<dyn AuditSink>,
        classify: fn(&str, Option<&str>) -> Option<RwClass>,
    ) -> Self {
        Self {
            mode: Mode::AllOpen,
            audit,
            classify,
            client: Mutex::new(None),
        }
    }

    /// A governed facade over the given decision point, audit sink, and classifier. Not yet
    /// used by any production path; exercised by the facade unit tests. Later tasks add builder
    /// methods to attach the browser plugin's `DomainPolicy` (classify/match) and
    /// `ResourceResolver`.
    pub fn governed(
        pdp: Box<dyn PolicyDecisionPoint>,
        audit: Arc<dyn AuditSink>,
        classify: fn(&str, Option<&str>) -> Option<RwClass>,
    ) -> Self {
        Self {
            mode: Mode::Governed(GovernedState { pdp }),
            audit,
            classify,
            client: Mutex::new(None),
        }
    }

    /// The audit sink held by this facade. Always present (a disabled configuration holds a
    /// null sink); the audit recorder (G06) is what this points at in production.
    pub fn audit_sink(&self) -> &dyn AuditSink {
        self.audit.as_ref()
    }

    /// The single inbound governance decision for one tool call, taken at the dispatch chokepoint
    /// before the tool executes.
    ///
    /// Under [`Mode::AllOpen`] this is a literal STEP-0 short-circuit: it returns
    /// [`Decision::Allow`] without touching any port or resolving any resource, so all-open output
    /// is byte-identical to stage 1. Under [`Mode::Governed`] it asks the held decision point; the
    /// real pipeline (classify -> resolve resource -> grant check -> effective mode) is filled in by
    /// G07/G13/G15, and with the Noop decision point the result is still `Allow`.
    pub fn decide(&self, tool: &str) -> Decision {
        match &self.mode {
            Mode::AllOpen => Decision::Allow { grant_id: None },
            Mode::Governed(state) => {
                // Wiring stub. Placeholder request fields: the resolver task resolves the
                // governing resource, G12/G13 supply grants, G15 resolves the effective mode.
                // The Noop PDP ignores them and allows.
                let req = DecisionRequest {
                    grants: Vec::new(),
                    tool: tool.to_string(),
                    rw: RwClass::Observe,
                    resource: GoverningResource::None,
                    mode: EffectiveMode::Observe,
                };
                state.pdp.decide(&req)
            }
        }
    }

    /// Capture the MCP client identity from the `initialize` request's `clientInfo`
    /// (shared format doc section 6.1 `client` field). First capture wins for the whole
    /// session; a no-op if a client identity is already stored.
    pub fn set_client(&self, name: &str, version: &str) {
        let mut guard = self.client.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.is_none() {
            *guard = Some(ClientInfo {
                name: name.to_string(),
                version: version.to_string(),
            });
        }
    }

    /// Build and record one audit record for a completed, ALLOWED tool call (ADR-0018 step 1:
    /// the flight recorder). Called at the dispatch chokepoint after the call resolves, so the
    /// record carries the real duration. `action` is the `computer` sub-action when
    /// `tool == "computer"`, `None` otherwise. `domain` is the current tab's host at decision
    /// time when the sacred-domains check (g08) resolved one, `None` otherwise (shared format
    /// doc section 6.1: `domain` is a decision-time fact, not derived from tool arguments).
    ///
    /// `identity`, `grant_id`, and `manifest` are always `None` until the manifest task (G12)
    /// lands; `decision` is always `"allow"` (a denied call goes through [`Self::record_deny`]
    /// instead). A classification miss (`self.classify` returns `None`: an unknown tool, or a
    /// `computer` call with a missing or unknown action) records [`RwClass::Mutate`]: the
    /// record vocabulary is only observe/mutate, and an unclassifiable call must never be
    /// presented as harmless observation.
    pub fn record_call(
        &self,
        tool: &str,
        action: Option<&str>,
        duration_ms: u64,
        domain: Option<&str>,
    ) {
        let record = self.build_record(
            tool,
            action,
            domain,
            "allow",
            None,
            None,
            duration_ms,
            false,
        );
        self.audit.record(&record);
    }

    /// Build and record one audit record for a call DENIED before dispatch (the sacred-domains
    /// rule, g08; later the grant-enforcement rules, g13). No tool call ever ran, so
    /// `duration_ms` is `0` per shared format doc section 6.1. `action` is the `computer`
    /// sub-action when `tool == "computer"`, `None` otherwise (the same classification wiring
    /// [`Self::record_call`] uses -- a denial is still classified, since the record's `rw` field
    /// is about the call's nature, not its outcome). `domain` is the current tab's host at
    /// decision time when a current-tab check resolved one, `None` otherwise -- this is
    /// independent of which host the denial itself names (`denial.domain`): a navigate-target
    /// denial with an unresolvable current tab still records `domain: null` even though the
    /// denial message names the target (shared format doc section 6.1).
    pub fn record_deny(
        &self,
        tool: &str,
        action: Option<&str>,
        denial: &Denial,
        domain: Option<&str>,
    ) {
        let record = self.build_record(
            tool,
            action,
            domain,
            "deny",
            denial.grant_id.clone(),
            Some(denial.denial_id.clone()),
            0,
            false,
        );
        self.audit.record(&record);
    }

    /// Build and record one audit record for a call answered with the take-the-wheel pause
    /// text instead of executing (a user hold, g10). The call was not policy-denied (policy
    /// was never consulted) and no tool ran, so `decision` is `"allow"` and `duration_ms` is
    /// `0`, exactly like [`Self::record_deny`]'s zero-duration convention -- but `held` is
    /// `true` and `grant_id`/`denial_id` stay `None`. `domain` is always `None`: a held call
    /// must not touch the extension, so no current-tab host is ever resolved for it.
    pub fn record_held(&self, tool: &str, action: Option<&str>) {
        let record = self.build_record(tool, action, None, "allow", None, None, 0, true);
        self.audit.record(&record);
    }

    #[allow(clippy::too_many_arguments)]
    fn build_record(
        &self,
        tool: &str,
        action: Option<&str>,
        domain: Option<&str>,
        decision: &'static str,
        grant_id: Option<String>,
        denial_id: Option<String>,
        duration_ms: u64,
        held: bool,
    ) -> AuditRecord {
        let rw = (self.classify)(tool, action).unwrap_or(RwClass::Mutate);
        AuditRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            identity: None,
            client: self.current_client(),
            tool: tool.to_string(),
            action: action.map(str::to_string),
            rw,
            domain: domain.map(str::to_string),
            decision,
            grant_id,
            denial_id,
            duration_ms,
            manifest: None,
            held,
        }
    }

    fn current_client(&self) -> Option<ClientInfo> {
        self.client
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Record the panic kill switch's session event (g11): the user severed the session. A
    /// session event, not a tool call -- carries no `tool`/`action`/`rw`/`domain`/`decision`/
    /// `grant_id`/`denial_id`/`duration_ms`, only the shared `event_id`/`ts`/`identity`/
    /// `client`/`manifest` fields plus `event: "session_killed"`. Called from the
    /// `Browser::on_session_killed` hook, registered once at session startup; the extension
    /// signals the event at most once per kill (the flag transition is idempotent), so this
    /// fires at most once per kill too.
    pub fn record_session_killed(&self) {
        let record = SessionEventRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            identity: None,
            client: self.current_client(),
            event: "session_killed",
            manifest: None,
        };
        self.audit.record_session_event(&record);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::ports::NoopPdp;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn no_classification(_tool: &str, _action: Option<&str>) -> Option<RwClass> {
        None
    }

    /// A sink that counts records instead of dropping them, so tests can assert recording
    /// actually happened without pulling in the G06 file/stderr sinks.
    #[derive(Default)]
    struct CountingAuditSink {
        count: AtomicUsize,
    }
    impl AuditSink for CountingAuditSink {
        fn record(&self, _record: &AuditRecord) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
        fn record_session_event(&self, _record: &SessionEventRecord) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// A sink that keeps every record, so tests can assert on the actual built fields (`rw`,
    /// `action`, `client`) rather than just call count.
    #[derive(Default)]
    struct CapturingAuditSink {
        records: Mutex<Vec<AuditRecord>>,
        session_events: Mutex<Vec<SessionEventRecord>>,
    }
    impl AuditSink for CapturingAuditSink {
        fn record(&self, record: &AuditRecord) {
            self.records
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(record.clone());
        }
        fn record_session_event(&self, record: &SessionEventRecord) {
            self.session_events
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(record.clone());
        }
    }
    impl CapturingAuditSink {
        fn last(&self) -> AuditRecord {
            self.records
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last()
                .cloned()
                .expect("at least one record was captured")
        }

        fn last_session_event(&self) -> SessionEventRecord {
            self.session_events
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last()
                .cloned()
                .expect("at least one session event was captured")
        }
    }

    /// A stand-in for the browser plugin's real classifier: `computer`/`screenshot` observes,
    /// `computer`/`left_click` mutates, `read_page` observes, everything else misses.
    fn sample_classify(tool: &str, action: Option<&str>) -> Option<RwClass> {
        match (tool, action) {
            ("computer", Some("screenshot")) => Some(RwClass::Observe),
            ("computer", Some("left_click")) => Some(RwClass::Mutate),
            ("read_page", None) => Some(RwClass::Observe),
            _ => None,
        }
    }

    #[test]
    fn all_open_decide_is_allow_and_still_records() {
        let sink = Arc::new(CountingAuditSink::default());
        let g = Governance::all_open(sink.clone(), no_classification);
        assert!(matches!(
            g.decide("navigate"),
            Decision::Allow { grant_id: None }
        ));
        g.record_call("navigate", None, 5, None);
        assert_eq!(sink.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn governed_over_noop_still_allows_and_holds_the_sink() {
        let sink = Arc::new(CountingAuditSink::default());
        let g = Governance::governed(Box::new(NoopPdp), sink.clone(), no_classification);
        assert!(matches!(g.decide("navigate"), Decision::Allow { .. }));
        g.record_call("navigate", None, 0, None);
        assert_eq!(sink.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn classification_miss_records_mutate() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone(), no_classification);
        g.record_call("no_such_tool", None, 0, None);
        assert_eq!(sink.last().rw, RwClass::Mutate);
        g.record_call("computer", None, 0, None);
        assert_eq!(
            sink.last().rw,
            RwClass::Mutate,
            "a computer call with no action is also a classification miss"
        );
    }

    #[test]
    fn computer_action_classification_flows_into_rw() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone(), sample_classify);

        g.record_call("computer", Some("screenshot"), 0, None);
        let rec = sink.last();
        assert_eq!(rec.rw, RwClass::Observe);
        assert_eq!(rec.action.as_deref(), Some("screenshot"));

        g.record_call("computer", Some("left_click"), 0, None);
        assert_eq!(sink.last().rw, RwClass::Mutate);

        g.record_call("read_page", None, 0, None);
        let rec = sink.last();
        assert_eq!(rec.rw, RwClass::Observe);
        assert_eq!(rec.action, None);
    }

    #[test]
    fn set_client_first_capture_wins() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone(), no_classification);
        g.set_client("a", "1");
        g.set_client("b", "2");
        let stored = g.client.lock().unwrap();
        assert_eq!(stored.as_ref().unwrap().name, "a");
        assert_eq!(stored.as_ref().unwrap().version, "1");
        drop(stored);

        g.record_call("navigate", None, 0, None);
        let client = sink.last().client.expect("client info recorded");
        assert_eq!(client.name, "a");
        assert_eq!(client.version, "1");
    }

    #[test]
    fn record_call_passes_the_resolved_domain_through() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone(), no_classification);
        g.record_call("read_page", None, 0, Some("www.mybank.com"));
        assert_eq!(sink.last().domain.as_deref(), Some("www.mybank.com"));
    }

    #[test]
    fn record_deny_writes_a_zero_duration_deny_record() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone(), sample_classify);
        let denial = Denial {
            rule: "sacred/*.mybank.com".to_string(),
            grant_id: None,
            denial_id: "D-af6633ec".to_string(),
            domain: "www.mybank.com".to_string(),
            message: "Denied (D-af6633ec): www.mybank.com is on the user's never-touch list."
                .to_string(),
        };
        g.record_deny("read_page", None, &denial, Some("www.mybank.com"));
        let rec = sink.last();
        assert_eq!(rec.decision, "deny");
        assert_eq!(rec.denial_id.as_deref(), Some("D-af6633ec"));
        assert_eq!(rec.grant_id, None);
        assert_eq!(rec.duration_ms, 0);
        assert_eq!(rec.domain.as_deref(), Some("www.mybank.com"));
        assert_eq!(rec.rw, RwClass::Observe);
    }

    #[test]
    fn record_held_writes_an_allow_record_with_held_true_and_no_domain() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone(), sample_classify);
        g.record_held("computer", Some("screenshot"));
        let rec = sink.last();
        assert_eq!(rec.decision, "allow");
        assert!(rec.held);
        assert_eq!(rec.duration_ms, 0);
        assert_eq!(rec.domain, None);
        assert_eq!(rec.grant_id, None);
        assert_eq!(rec.denial_id, None);
        assert_eq!(rec.rw, RwClass::Observe);
        assert_eq!(rec.action.as_deref(), Some("screenshot"));
    }

    #[test]
    fn record_call_and_record_deny_leave_held_false() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone(), no_classification);
        g.record_call("navigate", None, 5, None);
        assert!(!sink.last().held);

        let denial = Denial {
            rule: "sacred/mybank.com".to_string(),
            grant_id: None,
            denial_id: "D-171052e3".to_string(),
            domain: "mybank.com".to_string(),
            message: "Denied (D-171052e3): mybank.com is on the user's never-touch list."
                .to_string(),
        };
        g.record_deny("navigate", None, &denial, None);
        assert!(!sink.last().held);
    }

    #[test]
    fn hold_message_states_not_executed_with_no_hint_below_the_threshold() {
        let msg = hold_message("navigate", None, Duration::from_secs(1));
        assert!(msg.starts_with("Paused:"));
        assert!(msg.contains("NOT executed"));
        assert!(msg.contains("'navigate' call"));
        assert!(!msg.contains("2 minutes"));
    }

    #[test]
    fn hold_message_appends_the_hint_at_and_above_the_threshold() {
        let at_threshold = hold_message("navigate", None, HOLD_HINT_AFTER);
        assert!(at_threshold.contains("2 minutes"));
        assert!(at_threshold.contains("Only the user can resume it"));

        let above_threshold =
            hold_message("navigate", None, HOLD_HINT_AFTER + Duration::from_secs(1));
        assert!(above_threshold.contains("2 minutes"));

        let below_threshold =
            hold_message("navigate", None, HOLD_HINT_AFTER - Duration::from_secs(1));
        assert!(!below_threshold.contains("2 minutes"));
    }

    #[test]
    fn hold_message_renders_computer_action_label() {
        let msg = hold_message("computer", Some("left_click"), Duration::from_secs(0));
        assert!(msg.contains("'computer (left_click)' call"));

        let plain = hold_message("read_page", None, Duration::from_secs(0));
        assert!(plain.contains("'read_page' call"));
    }

    #[test]
    fn record_session_killed_writes_a_session_event_with_no_tool_call_fields() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone(), no_classification);
        g.set_client("claude-code", "2.1.0");
        g.record_session_killed();
        let rec = sink.last_session_event();
        assert_eq!(rec.event, "session_killed");
        assert_eq!(rec.client.as_ref().unwrap().name, "claude-code");
        assert_eq!(rec.identity, None);
        assert_eq!(rec.manifest, None);
    }
}
