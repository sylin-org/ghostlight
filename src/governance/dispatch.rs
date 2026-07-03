//! Tool-call dispatch chokepoint -- the single Policy Enforcement Point (PEP).
//!
//! Every `tools/call` opens a per-call audit scope ([`Governance::begin`], producing a
//! [`CallAudit`]) and passes through [`Governance::authorize`] exactly once, before the tool
//! executes; the scope is then completed through one of its own consuming methods (`held`,
//! `sacred_deny`, `landing_allow`/`landing_deny`, `complete`) after the call resolves (ADR-0024
//! Decision 3). The [`Governance`] facade holds the governance ports (a
//! [`PolicyDecisionPoint`](crate::governance::ports::PolicyDecisionPoint), an
//! [`AuditSink`](crate::governance::ports::AuditSink), and later the browser plugin halves) and is
//! the one place the stage-2 overlay attaches.
//!
//! [`Governance::all_open`] is the ungoverned engine: [`Governance::authorize`] short-circuits to
//! `Gate::Proceed` for it, querying no port and resolving no resource, so a session with no
//! manifest and default config is byte-identical to stage 1 (ADR-0013). Audit is orthogonal to
//! that short-circuit (shared format doc section 4.5: the flight recorder still records under
//! all-open when `audit.enabled` is true), so the audit sink is a field of `Governance` itself,
//! not nested inside the governed-only state.
//!
//! `Governance` holds no browser-domain fn pointer at all: this module lives in the
//! domain-agnostic governance core, and the concrete action directory is browser-domain
//! (`browser::directory::requires`, ADR-0022 Decision 2; the a7 arch-test forbids a
//! `governance -> browser` edge). The transport layer performs the ONE per-call directory lookup
//! itself and hands the result to [`Governance::begin`]; [`CallAudit`] carries that same result
//! forward to both [`Governance::authorize`] (the decision) and the eventual audit record's
//! `capability` field (ADR-0022 Decision 8) -- there is no second, independent lookup anywhere
//! in this module.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::governance::manifest::document::Grant;
use crate::governance::manifest::identity::ManifestIdentity;
use crate::governance::ports::{
    AuditRecord, AuditSink, Capability, ClientInfo, Decision, DecisionRequest, Denial,
    EffectiveMode, GoverningResource, PolicyDecisionPoint, SessionEventRecord,
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
    let label = crate::governance::ports::call_label(tool, action);
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
             it, from the Ghostlight extension: the popup Pause/Resume button or the toggle \
             keyboard shortcut.",
        );
    }
    message
}

/// The status-surface governance summary (g15, shared format doc section 9.2): the
/// manifest-level effective mode (`manifest_mode.unwrap_or(config_mode)`) and whether shadow
/// enforcement is active. Rendered by `get_status`'s `governance` object and the doctor
/// `Governance:` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GovernanceStatus {
    pub mode: EffectiveMode,
    pub shadow: bool,
}

/// The pure computation behind [`GovernanceStatus`] (g15): `mode` is the manifest-level
/// effective mode; `shadow` is true only when `grants` is non-empty AND that mode is
/// `Observe` -- per-grant overrides never change this top-level flag, and an empty `grants`
/// array (a manifest with no policy content yet) is deliberately reported as non-shadow even
/// though an individual would-deny call under it would still be classified `shadow_deny` by
/// [`crate::governance::enforcement::apply_mode`] (the badge describes whether a MEANINGFUL
/// policy is being observed, not the literal per-call decision vocabulary). A free function
/// (not a `Governance` method) so a standalone caller with no live session -- `ghostlight
/// doctor`, which resolves its own manifest independently -- computes the identical summary
/// [`Governance::governance_status`] does, from the same three inputs.
pub fn governance_status(
    grants: &[Grant],
    manifest_mode: Option<EffectiveMode>,
    config_mode: EffectiveMode,
) -> GovernanceStatus {
    let mode = manifest_mode.unwrap_or(config_mode);
    let shadow = !grants.is_empty() && mode == EffectiveMode::Observe;
    GovernanceStatus { mode, shadow }
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
    /// The MCP client identity captured from the `initialize` request, first-wins for the
    /// whole session (shared format doc section 6.1 `client` field).
    client: Mutex<Option<ClientInfo>>,
}

/// The two shapes of the decision path. `AllOpen` holds nothing so its decide path is a
/// zero-cost short-circuit; `Governed` holds the decision port plus the active manifest's
/// grants and content hash (g13).
enum Mode {
    /// STEP-0: the ungoverned engine. No manifest, default config. Every call is `Allow`.
    AllOpen,
    /// The governed overlay, active once a manifest is loaded (g13).
    Governed(GovernedState),
}

/// The decision port a governed facade holds, plus the request fields that come from the
/// active manifest itself rather than from any one call (g13): the resolved grants (in
/// manifest order; the pure decision core re-resolves the matching grant per call), the
/// manifest's content hash (denial ids are computed from it, shared format doc section 7.1),
/// and the manifest-level `mode` (g15, shared format 4.1: the mode precedence's middle tier,
/// between a resolving grant's own `mode` and the resolved `governance.mode`). `dyn` on the
/// PDP is deliberate: the decision point has multiple impls (Noop today, `LocalPdp` in stage
/// 2, a future Remote).
struct GovernedState {
    pdp: Box<dyn PolicyDecisionPoint>,
    grants: Vec<Grant>,
    manifest_hash: String,
    manifest_mode: Option<EffectiveMode>,
}

impl Governance {
    /// The ungoverned engine: a zero-port decision path whose `authorize` short-circuits to
    /// `Gate::Proceed`, paired with an audit sink built independently from config (audit is
    /// orthogonal to all-open). This is the facade used in production until the manifest task
    /// lands.
    pub fn all_open(audit: Arc<dyn AuditSink>) -> Self {
        Self {
            mode: Mode::AllOpen,
            audit,
            client: Mutex::new(None),
        }
    }

    /// A governed facade over the given decision point, audit sink, the active manifest's
    /// resolved grants, its content hash (g13), and its own `mode` field, if any (g15).
    /// `transport::mcp::server::run` constructs this with a `LocalPdp` once a manifest is
    /// active; `all_open` stays the facade for a session with no manifest.
    pub fn governed(
        pdp: Box<dyn PolicyDecisionPoint>,
        audit: Arc<dyn AuditSink>,
        grants: Vec<Grant>,
        manifest_hash: String,
        manifest_mode: Option<EffectiveMode>,
    ) -> Self {
        Self {
            mode: Mode::Governed(GovernedState {
                pdp,
                grants,
                manifest_hash,
                manifest_mode,
            }),
            audit,
            client: Mutex::new(None),
        }
    }

    /// The audit sink held by this facade. Always present (a disabled configuration holds a
    /// null sink); the audit recorder (G06) is what this points at in production.
    pub fn audit_sink(&self) -> &dyn AuditSink {
        self.audit.as_ref()
    }

    /// True when a manifest is active ([`Mode::Governed`]); false under all-open. The dispatch
    /// chokepoint (`transport::mcp::server`) uses this to skip grant-resource resolution --
    /// including every extension tab-URL round trip it would otherwise make -- entirely under
    /// all-open (g13 constraint 3: STEP 0 must add zero new frames and zero new latency).
    pub fn is_governed(&self) -> bool {
        matches!(self.mode, Mode::Governed(_))
    }

    /// The active manifest's resolved grants (g14, tool advertisement filtering): `None` under
    /// all-open, `Some(&state.grants)` once a manifest is active. Read-only; a static snapshot
    /// captured once at construction, same as everything else `GovernedState` holds -- there is
    /// no live re-resolution yet (see `browser::advertise`'s module doc).
    pub fn grants(&self) -> Option<&[Grant]> {
        match &self.mode {
            Mode::AllOpen => None,
            Mode::Governed(state) => Some(&state.grants),
        }
    }

    /// The status-surface governance summary (g15, shared format doc section 9.2): `None`
    /// under all-open; `Some(governance_status(...))` once a manifest is active, computed from
    /// this facade's own held grants and manifest-level mode. Delegates to the free function
    /// [`governance_status`] so a standalone reader with no live `Governance` instance
    /// (`ghostlight doctor`, which resolves its own manifest independently) computes the
    /// IDENTICAL summary from the same inputs -- the two surfaces can never disagree (g15
    /// constraint 12).
    pub fn governance_status(&self, config_mode: EffectiveMode) -> Option<GovernanceStatus> {
        match &self.mode {
            Mode::AllOpen => None,
            Mode::Governed(state) => Some(governance_status(
                &state.grants,
                state.manifest_mode,
                config_mode,
            )),
        }
    }

    /// The single inbound governance decision for one ALREADY-CLASSIFIED tool call, taken at the
    /// dispatch chokepoint before the tool executes.
    ///
    /// A pure pass-through into [`DecisionRequest`] (ADR-0024 Decision 3): unlike its pre-ADR-0024
    /// shape, this performs no directory lookup and no miss handling of its own -- `requires` is
    /// the caller's own bound capability requirement set (ADR-0022 Decision 2), supplied
    /// directly. [`Self::authorize`] is the only in-crate caller for a governed, resource-bearing
    /// call (its precedence table's arm 5); it is also called directly for the navigate landing
    /// re-check (`transport::mcp::server::post_navigate_landing_check`) and by tests. Under
    /// [`Mode::AllOpen`] this returns [`Decision::Allow`] without touching any port or resolving
    /// any resource, so all-open output is byte-identical to stage 1. Under [`Mode::Governed`] it
    /// builds the full [`DecisionRequest`] from the held grants, manifest hash, and
    /// manifest-level mode, plus the caller-supplied `requires`/`resource`/`config_mode`, and
    /// delegates to the held decision point (`LocalPdp`/`check_call`, which applies the mode
    /// switch internally, g15, and short-circuits an empty `requires` to `Allow` on its own --
    /// callers that already know `requires` is empty should prefer [`Self::authorize`]'s free-action
    /// arm, which skips this call entirely rather than building a `DecisionRequest` for nothing).
    pub fn decide(
        &self,
        tool: &str,
        action: Option<&str>,
        requires: &[Capability],
        resource: GoverningResource,
        config_mode: EffectiveMode,
    ) -> Decision {
        match &self.mode {
            Mode::AllOpen => Decision::Allow { grant_id: None },
            Mode::Governed(state) => {
                let req = DecisionRequest {
                    grants: state.grants.clone(),
                    tool: tool.to_string(),
                    action: action.map(str::to_string),
                    requires: requires.to_vec(),
                    resource,
                    manifest_mode: state.manifest_mode,
                    config_mode,
                    manifest_hash: state.manifest_hash.clone(),
                };
                state.pdp.decide(&req)
            }
        }
    }

    /// Phase 0 (ADR-0024 Decision 3): open the per-call audit scope. `requires` is THE one
    /// directory lookup's result the transport layer performed (`None` is a registry/variant
    /// miss; [`Self::authorize`] turns it into the `unknown_action` denial). For the eventual
    /// record's `capability` field a miss renders `"none"`, exactly as the pre-ADR-0024
    /// `unwrap_or(&[])` convention did. Captures the sink handle, the tool/action strings, the
    /// `requires` result, the current client identity, and a start [`Instant`]; domain, grant,
    /// and shadow state all start empty.
    pub fn begin(
        &self,
        tool: &str,
        action: Option<&str>,
        requires: Option<&'static [Capability]>,
    ) -> CallAudit {
        CallAudit {
            audit: Arc::clone(&self.audit),
            client: self.current_client(),
            tool: tool.to_string(),
            action: action.map(str::to_string),
            requires,
            started: Instant::now(),
            domain: None,
            grant_id: None,
            shadow: None,
            duration_ms: None,
        }
    }

    /// Phase 1 (ADR-0024 Decision 3): the one policy gate. `resource: None` means no resource
    /// applies (all-open, a free action, or an unresolvable navigate target falling through
    /// ungoverned). Precedence, each arm terminal:
    ///
    /// 1. All-open: `Gate::Proceed`, a literal short-circuit before any lookup use -- an
    ///    all-open miss still dispatches.
    /// 2. `requires == Some(&[])` (free action): `Gate::Proceed`, no grant attribution, no PDP
    ///    consultation.
    /// 3. `requires == None` (a governed directory miss): builds the `unknown_action` denial and
    ///    routes it through [`crate::governance::enforcement::apply_mode`] (the only call site
    ///    outside `check_call`) -- `Deny` records via the scope and returns `Gate::Deny`;
    ///    `ShadowDeny` stores the shadow denial on the scope and returns `Gate::Proceed`. This is
    ///    the ADR-0024 sanctioned delta: a GOVERNED miss is now denied (restoring ADR-0022's
    ///    absent-means-DENY), where the pre-ADR-0024 code let it dispatch ungoverned.
    /// 4. `resource == None` (non-empty `requires`, unresolvable/ungoverned target):
    ///    `Gate::Proceed` (today's fall-through).
    /// 5. `Some(resource)`, governed: delegates to [`Self::decide`] exactly as before this ADR;
    ///    `Allow { grant_id }` stores attribution and proceeds; `Deny` records via the scope and
    ///    returns `Gate::Deny`; `ShadowDeny` stores the shadow denial (attribution from the
    ///    denial) and proceeds.
    ///
    /// On `Gate::Deny` the audit record is already written when this returns; the caller renders
    /// `message` as the denial text and drops the (already-recorded) scope.
    pub fn authorize(
        &self,
        audit: &mut CallAudit,
        resource: Option<GoverningResource>,
        config_mode: EffectiveMode,
    ) -> Gate {
        let state = match &self.mode {
            Mode::AllOpen => return Gate::Proceed,
            Mode::Governed(state) => state,
        };

        let Some(reqs) = audit.requires else {
            let denial = crate::governance::enforcement::unknown_action_denial(
                &audit.tool,
                audit.action.as_deref(),
                &state.manifest_hash,
            );
            return match crate::governance::enforcement::apply_mode(
                Decision::Deny(denial),
                &state.grants,
                state.manifest_mode,
                config_mode,
            ) {
                Decision::Deny(d) => {
                    audit.record_terminal_deny(&d, 0);
                    Gate::Deny { message: d.message }
                }
                Decision::ShadowDeny(d) => {
                    audit.shadow = Some(d);
                    Gate::Proceed
                }
                Decision::Allow { .. } => {
                    unreachable!("apply_mode never turns a Deny into an Allow")
                }
            };
        };

        if reqs.is_empty() {
            return Gate::Proceed;
        }

        let Some(resource) = resource else {
            return Gate::Proceed;
        };

        match self.decide(
            &audit.tool,
            audit.action.as_deref(),
            reqs,
            resource,
            config_mode,
        ) {
            Decision::Allow { grant_id } => {
                audit.grant_id = grant_id;
                Gate::Proceed
            }
            Decision::Deny(d) => {
                audit.record_terminal_deny(&d, 0);
                Gate::Deny { message: d.message }
            }
            Decision::ShadowDeny(d) => {
                audit.grant_id = d.grant_id.clone();
                audit.shadow = Some(d);
                Gate::Proceed
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

    /// The MCP client identity captured from `initialize`, if any. `pub(crate)` (ADR-0025
    /// Decision 2/6): the manifest hot-reload policy-subscription task
    /// (`transport::mcp::server`) reads this off the OUTGOING `Governance` snapshot before a
    /// swap and re-applies it to the rebuilt instance via [`Self::set_client`], so client
    /// identity survives every swap even though a rebuilt `Governance` otherwise starts with
    /// none.
    pub(crate) fn current_client(&self) -> Option<ClientInfo> {
        self.client
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Record the panic kill switch's session event (g11): the user severed the session. A
    /// session event, not a tool call -- carries no
    /// `tool`/`action`/`capability`/`domain`/`decision`/`grant_id`/`denial_id`/`duration_ms`,
    /// only the shared `event_id`/`ts`/`identity`/`client`/`manifest` fields plus
    /// `event: "session_killed"`. Called from the
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

    /// Record the manifest hot-reload session event (ADR-0025 Decision 5): on every successful
    /// swap, `manifest` carries the NEW manifest's identity (`None` for a swap to all-open). A
    /// failed reload records nothing (the ERROR log carries it; the audit stream records what IS
    /// in force, not what failed to be) -- callers only invoke this on a successful swap.
    pub fn record_manifest_reload(&self, manifest: Option<ManifestIdentity>) {
        let record = SessionEventRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            identity: None,
            client: self.current_client(),
            event: "manifest_reload",
            manifest,
        };
        self.audit.record_session_event(&record);
    }

    /// Record the `user_manifest_ignored` session event (ADR-0025 Decision 5): an org policy
    /// file displaced a user-supplied manifest's grants. Callers record this once at startup
    /// when the condition already holds, and again only on a TRANSITION (the condition newly
    /// re-establishing itself after it had lapsed) -- never on a repeat while it stays true
    /// across consecutive reloads (see [`crate::governance::ports::
    /// user_manifest_ignored_transitioned`], the pure gate callers apply before calling this).
    pub fn record_user_manifest_ignored(&self) {
        let record = SessionEventRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            identity: None,
            client: self.current_client(),
            event: "user_manifest_ignored",
            manifest: None,
        };
        self.audit.record_session_event(&record);
    }
}

/// The outcome of [`Governance::authorize`]: either a terminal denial (the audit record is
/// already written; the caller renders `message` as the denial text and does nothing further
/// with the scope) or proceed (the caller dispatches the tool and later completes the scope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    Deny { message: String },
    Proceed,
}

/// The per-call audit scope (ADR-0024 Decision 3): opened by [`Governance::begin`] right after
/// the registry lookup, threaded through [`Governance::authorize`], and completed through
/// exactly one of its own consuming methods. Fields are private; every mutation goes through a
/// method so the one-record-per-call invariant lives in the type, not in caller discipline.
pub struct CallAudit {
    audit: Arc<dyn AuditSink>,
    client: Option<ClientInfo>,
    tool: String,
    action: Option<String>,
    requires: Option<&'static [Capability]>,
    started: Instant,
    domain: Option<String>,
    grant_id: Option<String>,
    shadow: Option<Denial>,
    duration_ms: Option<u64>,
}

impl CallAudit {
    /// Overwrite the audit-record `domain` field. Called unconditionally right after the sacred
    /// check passes (seeding the pre-grant tab host so an all-open or free-action allow still
    /// carries it), and again whenever grant-stage resource resolution produces one.
    pub fn set_domain(&mut self, domain: Option<String>) {
        self.domain = domain;
    }

    /// Record a held call (g10, the take-the-wheel pause): `decision: "allow"`, `held: true`,
    /// `duration_ms: 0`, `domain: null` (a held call must never touch the extension, so no
    /// current-tab host is ever resolved for it). Terminal: consumes the scope.
    pub fn held(self) {
        let record = self.build_record(None, "allow", None, None, 0, true);
        self.audit.record(&record);
    }

    /// Record a pre-dispatch sacred-domains denial (g08): `decision: "deny"`, `duration_ms: 0`
    /// per shared format doc section 6.1 (no tool call ever ran). `domain` is the current tab's
    /// host at decision time (independent of which host the denial message itself names).
    /// Terminal: consumes the scope.
    pub fn sacred_deny(self, denial: &Denial, domain: Option<&str>) {
        let record = self.build_record(
            domain,
            "deny",
            denial.grant_id.clone(),
            Some(denial.denial_id.clone()),
            0,
            false,
        );
        self.audit.record(&record);
    }

    /// Freeze the call's duration at elapsed-since-[`Governance::begin`]. Called immediately
    /// after `Browser::call` returns, transcribing today's clock stop at that exact point, so
    /// [`Self::complete`]/[`Self::landing_deny`] reproduce the pre-ADR-0024 duration bytes even
    /// when the navigate landing probe runs after this. A no-op if called more than once (the
    /// first freeze wins); if never called, [`Self::duration`] falls back to elapsed-at-completion.
    pub fn dispatch_finished(&mut self) {
        if self.duration_ms.is_none() {
            self.duration_ms = Some(self.elapsed_ms());
        }
    }

    /// Amend the scope's attribution after a successful navigate landing re-check (g13/g15,
    /// point 5): overwrites `grant_id`/`domain` with the landing's own resolution and clears any
    /// shadow denial captured pre-dispatch (an on-grant landing is a real allow, not a shadow).
    pub fn landing_allow(&mut self, grant_id: Option<String>, domain: Option<String>) {
        self.grant_id = grant_id;
        self.domain = domain;
        self.shadow = None;
    }

    /// Amend the scope after a navigate landing re-check that resolves to a SHADOW deny (g15):
    /// unlike [`Self::landing_allow`], this does not clear the shadow state -- it REPLACES
    /// whatever shadow denial the pre-dispatch check may have captured with the landing's own,
    /// and updates the domain to the landing host, so [`Self::complete`] later records a
    /// `shadow_deny` attributed to the landing rather than the pre-dispatch check. Not named in
    /// the pinned `CallAudit` method list (which covers only the landing-allow and landing-deny
    /// `Decision` variants); added because the third `Decision::ShadowDeny` variant the landing
    /// re-check can also produce (g15's mode switch applies there exactly as it does
    /// pre-dispatch) must still be recorded as a `shadow_deny` -- reusing `landing_allow` would
    /// silently clear that shadow state and misrecord it as a plain `allow`.
    pub fn landing_shadow_deny(&mut self, denial: Denial, domain: Option<String>) {
        self.domain = domain;
        self.shadow = Some(denial);
    }

    /// Record the navigate point-5 post-landing denial (g13, shared format doc section 6.1):
    /// unlike [`Self::sacred_deny`], the call DID dispatch and the browser actually navigated
    /// before landing off-grant, so `duration_ms` is the frozen real elapsed time (via
    /// [`Self::dispatch_finished`]), not `0`. `domain` is the FINAL (post-redirect) host the tab
    /// landed on, or `None` for a non-host landing -- never the denial message's `(unknown)`
    /// placeholder. Terminal: consumes the scope.
    pub fn landing_deny(self, denial: &Denial, domain: Option<&str>) {
        let duration_ms = self.duration();
        let record = self.build_record(
            domain,
            "deny",
            denial.grant_id.clone(),
            Some(denial.denial_id.clone()),
            duration_ms,
            false,
        );
        self.audit.record(&record);
    }

    /// Record the scope's final outcome (ADR-0024 Decision 3): a shadow denial captured along
    /// the way (g15, shadow enforcement) yields a `"shadow_deny"` record with that denial's
    /// `grant_id`/`denial_id`; otherwise an `"allow"` record with whatever attribution the scope
    /// accumulated. Uses the frozen duration when [`Self::dispatch_finished`] ran (an ordinary
    /// dispatched call), else elapsed-at-completion (the `explain` free action, answered with no
    /// dispatch at all). Terminal: consumes the scope.
    pub fn complete(self) {
        let duration_ms = self.duration();
        let domain = self.domain.clone();
        let (decision, grant_id, denial_id) = match &self.shadow {
            Some(denial) => (
                "shadow_deny",
                denial.grant_id.clone(),
                Some(denial.denial_id.clone()),
            ),
            None => ("allow", self.grant_id.clone(), None),
        };
        let record = self.build_record(
            domain.as_deref(),
            decision,
            grant_id,
            denial_id,
            duration_ms,
            false,
        );
        self.audit.record(&record);
    }

    /// Record a terminal denial from inside [`Governance::authorize`] WITHOUT consuming the
    /// scope (`authorize` only borrows `&mut CallAudit`): the caller drops the scope naturally
    /// on its own early return, so no further completion call is needed or possible. Always
    /// `duration_ms: 0` (pre-dispatch), matching [`Self::sacred_deny`]'s convention.
    fn record_terminal_deny(&self, denial: &Denial, duration_ms: u64) {
        let record = self.build_record(
            self.domain.as_deref(),
            "deny",
            denial.grant_id.clone(),
            Some(denial.denial_id.clone()),
            duration_ms,
            false,
        );
        self.audit.record(&record);
    }

    /// Elapsed time since [`Governance::begin`], in milliseconds.
    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// The duration to record: the frozen value from [`Self::dispatch_finished`] if it ran,
    /// else elapsed-at-completion.
    fn duration(&self) -> u64 {
        self.duration_ms.unwrap_or_else(|| self.elapsed_ms())
    }

    /// Build one [`AuditRecord`] from the scope's captured fields plus this call's own outcome
    /// fields. `domain` is taken as a parameter (not always `self.domain`): [`Self::sacred_deny`]
    /// and [`Self::landing_deny`] each name a domain that differs from whatever the scope has
    /// accumulated so far (the sacred check's own tab resolution; the post-landing host).
    #[allow(clippy::too_many_arguments)]
    fn build_record(
        &self,
        domain: Option<&str>,
        decision: &'static str,
        grant_id: Option<String>,
        denial_id: Option<String>,
        duration_ms: u64,
        held: bool,
    ) -> AuditRecord {
        let capability = self
            .requires
            .and_then(|r| r.first())
            .map(Capability::as_str)
            .unwrap_or("none");
        AuditRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            identity: None,
            client: self.client.clone(),
            tool: self.tool.clone(),
            action: self.action.clone(),
            capability,
            domain: domain.map(str::to_string),
            decision,
            grant_id,
            denial_id,
            duration_ms,
            manifest: None,
            held,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::ports::NoopPdp;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A stand-in for the browser plugin's real action directory: `computer`/`screenshot` and
    /// `read_page` require `read`; `computer`/`left_click` requires `action`; `tabs_create_mcp`
    /// requires nothing (ADR-0022 `requires: []`); everything else misses.
    fn sample_requires(tool: &str, action: Option<&str>) -> Option<&'static [Capability]> {
        match (tool, action) {
            ("computer", Some("screenshot")) => Some(&[Capability::Read]),
            ("computer", Some("left_click")) => Some(&[Capability::Action]),
            ("read_page", None) => Some(&[Capability::Read]),
            ("tabs_create_mcp", None) => Some(&[]),
            _ => None,
        }
    }

    /// A PDP that always denies, so a test built on it can prove a call NEVER reached it
    /// (ADR-0022 Decision 5 step 2: a `requires: []` action short-circuits to `Allow` before any
    /// decision-point consultation).
    struct AlwaysDenyPdp;
    impl PolicyDecisionPoint for AlwaysDenyPdp {
        fn decide(&self, _req: &DecisionRequest) -> Decision {
            Decision::Deny(Denial {
                rule: "would-have-fired".to_string(),
                grant_id: None,
                denial_id: "D-00000000".to_string(),
                domain: String::new(),
                message: "the PDP was consulted when it should not have been".to_string(),
            })
        }
    }

    /// A PDP that denies naming exactly the `requires` slice it was handed, so a test can prove
    /// WHICH capability set actually reached the decision (ADR-0024 Decision 3: `authorize`
    /// consults `Governance`'s own held decision point with the CALLER's `requires`, never a
    /// second, independently looked-up one -- there is no fn pointer left to look one up with).
    struct EchoRequiresPdp;
    impl PolicyDecisionPoint for EchoRequiresPdp {
        fn decide(&self, req: &DecisionRequest) -> Decision {
            Decision::Deny(Denial {
                rule: "echo".to_string(),
                grant_id: None,
                denial_id: "D-00000002".to_string(),
                domain: String::new(),
                message: format!("saw requires={:?}", req.requires),
            })
        }
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

    /// A sink that keeps every record, so tests can assert on the actual built fields
    /// (`capability`, `action`, `client`) rather than just call count.
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

    /// Test 1: `begin` + `set_domain` + `complete` reproduces the pre-ADR-0024
    /// `record_call_passes_the_resolved_domain_through` assertion PLUS the 14-key field order
    /// pin transcribed from `tests/audit_recorder.rs` (there is no single pinned JSON blob
    /// today; these two named sources are the oracle).
    #[test]
    fn begin_complete_produces_the_allow_record_bytes() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone());
        let mut audit = g.begin("read_page", None, None);
        audit.set_domain(Some("www.mybank.com".to_string()));
        audit.complete();
        let rec = sink.last();
        assert_eq!(rec.domain.as_deref(), Some("www.mybank.com"));
        assert_eq!(rec.decision, "allow");
        assert_eq!(rec.capability, "none");
        assert_eq!(rec.grant_id, None);
        assert!(!rec.held);

        // The 14-key field order pin, transcribed from tests/audit_recorder.rs.
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&rec).unwrap()).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            vec![
                "event_id",
                "ts",
                "identity",
                "client",
                "tool",
                "action",
                "capability",
                "domain",
                "decision",
                "grant_id",
                "denial_id",
                "duration_ms",
                "manifest",
                "held",
            ],
            "field order matches the shared format"
        );
    }

    /// Test 2: a GOVERNED directory miss (`requires: None`) denies via `unknown_action` through
    /// the mode switch (enforce -> real Gate::Deny; observe -> Proceed, with `complete()` later
    /// recording the SAME denial id as a shadow_deny); an ALL-OPEN miss still dispatches
    /// (Proceed), the precedence table's arm 1. Transcribes the pre-ADR-0024
    /// `directory_miss_denies_via_unknown_action_through_the_mode_switch` expectations.
    #[test]
    fn authorize_miss_is_unknown_action_through_the_mode_switch() {
        // Enforce: a real Gate::Deny, already recorded with rule unknown_action.
        let enforce_sink = Arc::new(CapturingAuditSink::default());
        let enforce_g = Governance::governed(
            Box::new(NoopPdp),
            enforce_sink.clone(),
            Vec::new(),
            "hash".to_string(),
            None,
        );
        let mut enforce_audit = enforce_g.begin("no_such_tool", None, None);
        let denial_id = match enforce_g.authorize(&mut enforce_audit, None, EffectiveMode::Enforce)
        {
            Gate::Deny { message } => {
                assert!(message.starts_with("Denied (D-"), "{message}");
                let rec = enforce_sink.last();
                assert_eq!(rec.decision, "deny");
                assert_eq!(rec.capability, "none");
                assert_eq!(rec.duration_ms, 0);
                rec.denial_id.expect("denial id present")
            }
            Gate::Proceed => panic!("expected a deny for a governed miss"),
        };

        // Observe: Proceed (the tool dispatches), then complete() records a shadow_deny with the
        // SAME denial id as the enforce run above.
        let observe_sink = Arc::new(CapturingAuditSink::default());
        let observe_g = Governance::governed(
            Box::new(NoopPdp),
            observe_sink.clone(),
            Vec::new(),
            "hash".to_string(),
            None,
        );
        let mut observe_audit = observe_g.begin("no_such_tool", None, None);
        assert!(matches!(
            observe_g.authorize(&mut observe_audit, None, EffectiveMode::Observe),
            Gate::Proceed
        ));
        observe_audit.complete();
        let observe_rec = observe_sink.last();
        assert_eq!(observe_rec.decision, "shadow_deny");
        assert_eq!(observe_rec.denial_id.as_deref(), Some(denial_id.as_str()));

        // All-open: a miss still dispatches (Proceed), never even reaching the mode switch.
        let all_open_sink = Arc::new(CountingAuditSink::default());
        let all_open_g = Governance::all_open(all_open_sink);
        let mut all_open_audit = all_open_g.begin("no_such_tool", None, None);
        assert!(matches!(
            all_open_g.authorize(&mut all_open_audit, None, EffectiveMode::Enforce),
            Gate::Proceed
        ));
    }

    /// Test 3: `requires: Some(&[])` (a free action) proceeds without ever consulting the
    /// decision point (ADR-0022 Decision 5 step 2, ADR-0024 Decision 3 arm 2), proven by wiring
    /// a PDP that always denies and showing the call still allows; `complete()` records the
    /// allow with no grant attribution and `capability: "none"`.
    #[test]
    fn authorize_free_action_proceeds_without_grant_attribution() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::governed(
            Box::new(AlwaysDenyPdp),
            sink.clone(),
            Vec::new(),
            "hash".to_string(),
            None,
        );
        let mut audit = g.begin("tabs_create_mcp", None, Some(&[]));
        assert!(matches!(
            g.authorize(
                &mut audit,
                Some(GoverningResource::None),
                EffectiveMode::Enforce
            ),
            Gate::Proceed
        ));
        audit.complete();
        let rec = sink.last();
        assert_eq!(rec.decision, "allow");
        assert_eq!(rec.grant_id, None);
        assert_eq!(rec.capability, "none");
    }

    /// Test 4: transcribed from the pre-ADR-0024 `record_deny_writes_a_zero_duration_deny_record`
    /// and `record_held_writes_an_allow_record_with_held_true_and_no_domain`.
    #[test]
    fn sacred_deny_and_held_records_are_byte_stable() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone());

        let denial = Denial {
            rule: "sacred/*.mybank.com".to_string(),
            grant_id: None,
            denial_id: "D-af6633ec".to_string(),
            domain: "www.mybank.com".to_string(),
            message: "Denied (D-af6633ec): www.mybank.com is on the user's never-touch list."
                .to_string(),
        };
        let audit = g.begin("read_page", None, sample_requires("read_page", None));
        audit.sacred_deny(&denial, Some("www.mybank.com"));
        let rec = sink.last();
        assert_eq!(rec.decision, "deny");
        assert_eq!(rec.denial_id.as_deref(), Some("D-af6633ec"));
        assert_eq!(rec.grant_id, None);
        assert_eq!(rec.duration_ms, 0);
        assert_eq!(rec.domain.as_deref(), Some("www.mybank.com"));
        assert_eq!(rec.capability, "read");
        assert!(!rec.held);

        let held_audit = g.begin(
            "computer",
            Some("screenshot"),
            sample_requires("computer", Some("screenshot")),
        );
        held_audit.held();
        let rec = sink.last();
        assert_eq!(rec.decision, "allow");
        assert!(rec.held);
        assert_eq!(rec.duration_ms, 0);
        assert_eq!(rec.domain, None);
        assert_eq!(rec.grant_id, None);
        assert_eq!(rec.denial_id, None);
        assert_eq!(rec.capability, "read");
        assert_eq!(rec.action.as_deref(), Some("screenshot"));
    }

    /// Test 5: `landing_allow` overwrites attribution, then `complete` reflects the amendment;
    /// `landing_deny` reproduces the field assertions transcribed from
    /// `server.rs::point5_navigate_landing_off_grant_parks_and_denies` (decision deny, the
    /// landing domain, `grant_id` null, a real duration, tool `navigate` now coming from the
    /// scope itself rather than a hardcoded literal).
    #[test]
    fn landing_amendments_match_the_old_navigate_records() {
        // landing_allow: overwrites attribution; complete reflects the amended allow.
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone());
        let mut audit = g.begin("navigate", None, Some(&[Capability::Read][..]));
        audit.set_domain(Some("example.com".to_string()));
        audit.dispatch_finished();
        audit.landing_allow(Some("g1".to_string()), Some("example.com".to_string()));
        audit.complete();
        let rec = sink.last();
        assert_eq!(rec.decision, "allow");
        assert_eq!(rec.grant_id.as_deref(), Some("g1"));
        assert_eq!(rec.domain.as_deref(), Some("example.com"));
        assert_eq!(rec.tool, "navigate");

        // landing_deny: transcribed from point5_navigate_landing_off_grant_parks_and_denies.
        let sink2 = Arc::new(CapturingAuditSink::default());
        let g2 = Governance::all_open(sink2.clone());
        let mut audit2 = g2.begin("navigate", None, Some(&[Capability::Read][..]));
        std::thread::sleep(Duration::from_millis(2));
        audit2.dispatch_finished();
        let denial = Denial {
            rule: "unmatched_domain".to_string(),
            grant_id: None,
            denial_id: "D-00000003".to_string(),
            domain: "evil.com".to_string(),
            message: "Denied (D-00000003): no grant covers evil.com. Tool use is limited to \
                      domains your policy grants."
                .to_string(),
        };
        audit2.landing_deny(&denial, Some("evil.com"));
        let rec2 = sink2.last();
        assert_eq!(rec2.decision, "deny");
        assert_eq!(rec2.domain.as_deref(), Some("evil.com"));
        assert_eq!(rec2.grant_id, None);
        assert_eq!(rec2.tool, "navigate");
        assert!(
            rec2.duration_ms > 0,
            "a landing deny carries the real elapsed duration, not the pre-dispatch 0: {}",
            rec2.duration_ms
        );
    }

    /// Test 6: structural (ADR-0024 Decision 3) -- `Governance` no longer holds a `requires` fn
    /// pointer at all (this call would not compile against the old fn-pointer-taking
    /// constructor), and `authorize` drives the decision from exactly the `requires` value the
    /// caller handed `begin`, never a second, independent lookup: proven by handing a
    /// deliberately WRONG value to a PDP that echoes back what it saw.
    #[test]
    fn one_lookup_feeds_decision_and_audit() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::governed(
            Box::new(EchoRequiresPdp),
            sink,
            Vec::new(),
            "hash".to_string(),
            None,
        );
        let mut audit = g.begin("read_page", None, Some(&[Capability::Execute][..]));
        match g.authorize(
            &mut audit,
            Some(GoverningResource::Resource("example.com".to_string())),
            EffectiveMode::Enforce,
        ) {
            Gate::Deny { message } => assert!(
                message.contains("Execute"),
                "the PDP must see exactly the caller's own requires value: {message}"
            ),
            Gate::Proceed => panic!("expected the echo PDP's deny"),
        }
    }

    #[test]
    fn set_client_first_capture_wins() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone());
        g.set_client("a", "1");
        g.set_client("b", "2");
        let stored = g.client.lock().unwrap();
        assert_eq!(stored.as_ref().unwrap().name, "a");
        assert_eq!(stored.as_ref().unwrap().version, "1");
        drop(stored);

        let audit = g.begin("navigate", None, None);
        audit.complete();
        let client = sink.last().client.expect("client info recorded");
        assert_eq!(client.name, "a");
        assert_eq!(client.version, "1");
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
        let g = Governance::all_open(sink.clone());
        g.set_client("claude-code", "2.1.0");
        g.record_session_killed();
        let rec = sink.last_session_event();
        assert_eq!(rec.event, "session_killed");
        assert_eq!(rec.client.as_ref().unwrap().name, "claude-code");
        assert_eq!(rec.identity, None);
        assert_eq!(rec.manifest, None);
    }

    /// ADR-0025 Decision 5: both new producers emit the frozen `SessionEventRecord` shape (same
    /// key order as `session_killed`'s own pin) with their pinned event strings;
    /// `record_manifest_reload` carries the given identity (`None` for a swap to all-open).
    /// Plus the transition gate itself (`ports::user_manifest_ignored_transitioned`): a reload
    /// that keeps `user_manifest_ignored` true must NOT be treated as a fresh transition.
    #[test]
    fn manifest_reload_and_user_manifest_ignored_events_are_shaped() {
        let sink = Arc::new(CapturingAuditSink::default());
        let g = Governance::all_open(sink.clone());
        g.set_client("claude-code", "2.1.0");

        let identity = ManifestIdentity {
            name: "acme".to_string(),
            version: "1".to_string(),
            hash: "deadbeef".to_string(),
        };
        g.record_manifest_reload(Some(identity.clone()));
        let rec = sink.last_session_event();
        assert_eq!(rec.event, "manifest_reload");
        assert_eq!(rec.client.as_ref().unwrap().name, "claude-code");
        assert_eq!(rec.identity, None);
        assert_eq!(rec.manifest.as_ref().unwrap().name, "acme");

        g.record_manifest_reload(None);
        let rec2 = sink.last_session_event();
        assert_eq!(rec2.event, "manifest_reload");
        assert_eq!(
            rec2.manifest, None,
            "a swap to all-open carries manifest: null"
        );

        g.record_user_manifest_ignored();
        let rec3 = sink.last_session_event();
        assert_eq!(rec3.event, "user_manifest_ignored");
        assert_eq!(rec3.manifest, None);
        assert_eq!(rec3.client.as_ref().unwrap().name, "claude-code");

        // Key order matches session_killed's own pin (same shared, frozen record shape).
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&rec3).unwrap()).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            vec!["event_id", "ts", "identity", "client", "event", "manifest"]
        );

        // ADR-0025 Decision 5: transition gate, not a repeat.
        assert!(crate::governance::ports::user_manifest_ignored_transitioned(false, true));
        assert!(!crate::governance::ports::user_manifest_ignored_transitioned(true, true));
        assert!(!crate::governance::ports::user_manifest_ignored_transitioned(false, false));
        assert!(!crate::governance::ports::user_manifest_ignored_transitioned(true, false));
    }

    // --- g15: the governance_status badge resolver ---

    fn one_grant() -> Grant {
        Grant {
            id: "g1".to_string(),
            hosts: crate::governance::manifest::document::HostRules {
                allow: vec!["example.com".to_string()],
                deny: Vec::new(),
            },
            allowed: vec![Capability::Read, Capability::Action, Capability::Write],
            description: None,
            mode: None,
        }
    }

    #[test]
    fn governance_status_is_none_under_all_open() {
        let sink = Arc::new(CountingAuditSink::default());
        let g = Governance::all_open(sink);
        assert_eq!(g.governance_status(EffectiveMode::Enforce), None);
    }

    #[test]
    fn governance_status_reports_shadow_true_with_grants_under_observe() {
        assert_eq!(
            governance_status(
                &[one_grant()],
                Some(EffectiveMode::Observe),
                EffectiveMode::Enforce
            ),
            GovernanceStatus {
                mode: EffectiveMode::Observe,
                shadow: true,
            }
        );
        // The manifest's own mode wins; config alone would have said enforce here.
        assert_eq!(
            governance_status(&[one_grant()], None, EffectiveMode::Observe),
            GovernanceStatus {
                mode: EffectiveMode::Observe,
                shadow: true,
            }
        );
    }

    #[test]
    fn governance_status_reports_shadow_false_under_enforce() {
        assert_eq!(
            governance_status(
                &[one_grant()],
                Some(EffectiveMode::Enforce),
                EffectiveMode::Observe
            ),
            GovernanceStatus {
                mode: EffectiveMode::Enforce,
                shadow: false,
            }
        );
    }

    #[test]
    fn governance_status_never_shadows_with_empty_grants_even_under_observe() {
        assert_eq!(
            governance_status(&[], Some(EffectiveMode::Observe), EffectiveMode::Enforce),
            GovernanceStatus {
                mode: EffectiveMode::Observe,
                shadow: false,
            }
        );
    }

    #[test]
    fn governance_status_via_the_live_facade_matches_the_free_function() {
        let sink = Arc::new(CountingAuditSink::default());
        let g = Governance::governed(
            Box::new(NoopPdp),
            sink,
            vec![one_grant()],
            String::new(),
            Some(EffectiveMode::Observe),
        );
        assert_eq!(
            g.governance_status(EffectiveMode::Enforce),
            Some(GovernanceStatus {
                mode: EffectiveMode::Observe,
                shadow: true,
            })
        );
    }
}
