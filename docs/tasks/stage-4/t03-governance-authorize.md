# T03: governance authorize + CallAudit (audit ownership inversion), typed governance_mode

## Goal

Implement ADR-0024 Decision 3: governance becomes the single owner of audit correctness via
a two-phase API -- `Governance::begin` (per-call audit scope, `CallAudit`) +
`Governance::authorize` (the one policy gate) + scope completion methods -- replacing the
five public `record_*` methods and the caller-side record-variant state machine in
`server.rs`. Exactly one capability lookup per call feeds both decision and audit. Also:
the `computer (<action>)` label formatter collapses to one shared function, and
`Config.governance_mode` becomes a typed `EffectiveMode` (ADR-0024 Decision 3, last
bullet). The pipeline keeps its current shape and name-branches (t04 rewrites it); this
task adapts the existing call sites in place. Behavior, audit bytes, denial texts, and
denial ids are unchanged EXCEPT the one sanctioned delta this task owns (BOOTSTRAP rule
8): a GOVERNED directory miss (known tool, unknown sub-action) becomes a real
`unknown_action` denial instead of dispatching ungoverned -- the deliberate fix of the
`b4b2faf` fail-open regression, per ADR-0024 Decision 3. All-open misses still dispatch.

## Authority

ADR-0024 Decision 3 is normative (two-phase shape; single lookup; sacred never-shadowable
and outside `check_call`; pure core untouched; record bytes frozen). Where this prompt and
the ADR disagree, THE ADR WINS.

## Depends on

t01 and t02 landed (LEDGER RESUME HERE shows t02 committed; tree green). t02's registry is
NOT consumed here beyond the unchanged `requires()` fn (the pipeline still calls it; this
task changes who holds the result).

STOP preconditions: `src/governance/dispatch.rs` has exactly these public record methods:
`record_call`, `record_deny`, `record_navigate_landing_deny`, `record_shadow_deny`,
`record_held`, `record_session_killed`; `Governance` holds a `requires` fn-pointer field
consumed inside `decide`; `Config::governance_mode()` returns `&str`. If any is untrue,
STOP and record.

## Current behavior (verified 2026-07-03 against `b4b2faf`; re-read the tree)

- `src/governance/dispatch.rs`: `hold_message` (line ~47) inlines the
  `("computer", Some(action)) => "computer ({action})"` label match; `Governance` holds
  `requires: fn(&str, Option<&str>) -> Option<&'static [Capability]>` (line ~119),
  consumed inside `decide` (line ~261: a SECOND lookup per call; miss becomes the
  `unknown_action` deny routed through `apply_mode` at line ~267 -- but NOTE: this arm is
  PRODUCTION-DEAD at `b4b2faf`, because the server maps a miss to `&[]` via
  `unwrap_or(&[])` BEFORE the `!requires.is_empty()` guard, so a governed miss skips
  governance entirely and dispatches ungoverned; only inline tests reach the arm). The
  five record methods (lines ~323-448) each encode one outcome; `build_record` (line
  ~450) derives `capability = requires.first().map(as_str).unwrap_or("none")`.
  `record_navigate_landing_deny` hardcodes `tool: "navigate"` (line ~394).
- `src/governance/enforcement.rs::tool_label` (line ~92) is the second copy of the label
  match, used by the denial builders. `apply_mode` is also invoked inside `check_call`
  (line ~144).
- `src/transport/mcp/server.rs::handle_tools_call`: one `directory::requires` lookup
  (line ~337, miss mapped to `&[]` for audit), then the caller-side machine: mutables
  `audit_domain`/`audit_grant_id`/`shadow_denial`/`navigate_post_check` (lines ~406-409)
  and six record call sites (held ~346, explain allow ~386, sacred deny ~381, grant deny
  ~419, landing deny ~477, shadow ~496 / allow ~505). `config_mode` built per call via
  `EffectiveMode::from_config_str(config.governance_mode())` (line ~408).
- `src/governance/config/mod.rs`: `governance_mode: String` field (line ~559), built via
  `preset_string_like`/`resolved_string_like`; getter returns `&str` (line ~635); inline
  tests pin `cfg.governance_mode()` against `"observe"`-style strings (lines ~796, ~814).
  `EffectiveMode::from_config_str` consumers: server.rs ~408, doctor.rs ~203, cli.rs ~147
  (reads a raw Resolution value, NOT Config), explain.rs ~137 and ~149 (minimal-config
  baseline and manifest entry values).
- `tests/all_open_golden.rs` has a `no_requires` stub passed to `Governance::all_open`;
  `tests/audit_recorder.rs` drives `record_call` with the real directory lookup.
- `src/governance/simulate.rs` replays `check_call` DIRECTLY (not `Governance`); it is
  unaffected by this task.

## Required behavior

### 1. One label formatter (`ports.rs`)

Add to `src/governance/ports.rs` (near `EffectiveMode`):

    /// Render a call's display label (shared format doc section 7.2): a tool with a
    /// sub-action renders as `{tool} ({action})`; every other call renders the bare tool
    /// name. The one implementation; hold messages and denial messages share it.
    pub fn call_label(tool: &str, action: Option<&str>) -> String {
        match action {
            Some(action) => format!("{tool} ({action})"),
            None => tool.to_string(),
        }
    }

`hold_message` and the enforcement denial builders call it; DELETE
`enforcement::tool_label` and the inline match in `hold_message`. NOTE the generalization:
the old formatters keyed on `tool == "computer"`; the new one keys on `action.is_some()`.
These are equivalent today because only `computer` calls ever carry an action (the server
extracts `action` only for `computer`); pin that equivalence in the doc comment.

### 2. The two-phase API (`dispatch.rs`)

    pub struct CallAudit { /* private */ }

    pub enum Gate {
        Deny { message: String },
        Proceed,
    }

    impl Governance {
        /// Phase 0: open the per-call audit scope. `requires` is THE one directory
        /// lookup's result (None = registry/variant miss; authorize turns it into the
        /// unknown_action denial). For the record's capability field a miss renders
        /// "none", exactly as the old unwrap_or(&[]) did.
        pub fn begin(&self, tool: &str, action: Option<&str>,
                     requires: Option<&'static [Capability]>) -> CallAudit

        /// Phase 1: the one policy gate. `resource: None` means no resource applies
        /// (all-open, free action, or unresolvable navigate target falling through
        /// ungoverned). Owns: the miss -> unknown_action denial (mode-switched), the
        /// free-action pass, the check_call delegation, shadow capture. On Deny the
        /// record is already written when this returns.
        pub fn authorize(&self, audit: &mut CallAudit,
                         resource: Option<GoverningResource>,
                         config_mode: EffectiveMode) -> Gate
    }

    impl CallAudit {
        pub fn set_domain(&mut self, domain: Option<String>);      // audit domain source
        pub fn held(self);                                          // held record, dur 0
        pub fn sacred_deny(self, denial: &Denial, domain: Option<&str>); // deny, dur 0
        pub fn dispatch_finished(&mut self);                        // freeze the duration
        pub fn landing_allow(&mut self, grant_id: Option<String>,
                             domain: Option<String>);               // clears shadow
        pub fn landing_deny(self, denial: &Denial, domain: Option<&str>); // frozen duration
        pub fn complete(self);                                       // allow OR shadow_deny
    }

Naming note (BOOTSTRAP rule 4): ADR-0024 calls this scope concept "AuditScope"/"the
per-call scope"; `CallAudit` is its normative Rust identifier, pinned here. Do not rename
and record no deviation.

Pinned semantics (each maps one existing record site; output bytes byte-identical except
the sanctioned miss delta):

- `CallAudit` captures at `begin`: the sink handle, tool/action strings, the requires
  result, a start `Instant`, and empty domain/grant/shadow state. Pre-dispatch denial
  records (`sacred_deny`, authorize-internal denies) hardcode `duration_ms: 0` exactly as
  today. A new `pub fn dispatch_finished(&mut self)` FREEZES the duration (elapsed since
  begin) and is called immediately after `Browser::call` returns -- transcribing today's
  clock stop at that exact point -- so `complete`/`landing_deny` reproduce today's
  duration bytes even when the navigate landing probe runs after it; if
  `dispatch_finished` was never called, they fall back to elapsed-at-completion.
- `authorize` precedence, pinned IN THIS ORDER (each arm terminal):
  1. all-open -> `Proceed` (a literal short-circuit before any lookup use, mirroring
     today's `Mode::AllOpen` arm; an all-open miss therefore still dispatches).
  2. `requires == Some(&[])` (free action) -> `Proceed`, no grant attribution, no PDP.
  3. `requires == None` (directory miss, governed): build the `unknown_action` denial,
     route through `apply_mode` (this becomes the ONLY `apply_mode` call site outside
     `check_call`); Deny -> record via the scope, return `Gate::Deny`; ShadowDeny ->
     store shadow, return `Proceed`. THE SANCTIONED DELTA: this arm is reachable from
     the pipeline after this task (see section 3), restoring ADR-0022 absent-means-DENY.
  4. `resource == None` (non-empty requires, unresolvable/ungoverned target) ->
     `Proceed` (today's fall-through).
  5. `Some(resource)` governed: delegate to the PDP exactly as `decide` does today;
     `Allow{grant_id}` stores attribution; `Deny` records and returns `Gate::Deny`;
     `ShadowDeny` stores shadow (attribution from the denial) and proceeds.
- `complete`: one record -- `shadow` set -> the shadow_deny shape, else the allow shape,
  with stored domain/grant attribution.
- The five public `record_*` methods are DELETED (their bodies fold into `CallAudit` /
  `build_record`, which stays private). `record_session_killed` stays (session event,
  different shape). `record_navigate_landing_deny`'s hardcoded tool name dies with it:
  `landing_deny` uses the scope's own tool/action.
- `decide` remains public for the navigate landing re-check (and tests) but is reshaped
  to take `requires: &[Capability]` as a parameter (pure pass-through into
  `DecisionRequest`); its internal lookup and the `Governance.requires` fn-pointer FIELD
  are DELETED, along with the constructor parameters on `all_open`/`governed`. Update the
  composition roots (`server.rs`, `main.rs` simulate wiring is separate and untouched)
  and retype helpers in `tests/all_open_golden.rs` (delete `no_requires`) --
  compile-necessary edits only, zero expectation changes.

### 3. Server call-site adaptation (`server.rs`, minimal motion)

`handle_tools_call` keeps its current stage structure and name branches. Replace the
lookup + six record sites + decide call with: one `directory::requires` lookup KEPT AS
THE `Option` (do NOT `unwrap_or(&[])` it away; the miss must stay distinguishable) ->
`governance.begin(name, action, lookup)` (before the hold check); `audit.held()`;
`audit.sacred_deny(...)`. IMMEDIATELY after the sacred stage passes, call
`audit.set_domain(tab_domain.clone())` unconditionally -- transcribing today's
`let mut audit_domain = tab_domain.clone()` pre-g13 seeding, so allow records for
all-open/free-action calls on a resolvable tab keep their `domain` value byte-identical.
Explain branch -> `audit.complete()` (no grant). Grant stage: RESOURCE RESOLUTION stays
gated on `is_governed() && matches!(lookup, Some(r) if !r.is_empty())` (a miss resolves
nothing -- no wasted probe before its denial); then call
`governance.authorize(&mut audit, resource, config_mode)` for EVERY call that reaches
this stage (governed or not, miss or not -- the precedence table above makes the
ungoverned/free/miss arms cheap and correct), overwriting the audit domain via
`audit.set_domain(...)` when resolution produced one, and rendering `Gate::Deny{message}`
exactly as today's denial text path. Landing stage -> `audit.landing_allow` /
`audit.landing_deny`; after `Browser::call` returns -> `audit.dispatch_finished()`;
final -> `audit.complete()`. The four mutables die.

### 4. Typed `governance_mode` (`config/mod.rs` + consumers)

- Field becomes `governance_mode: EffectiveMode`; `from_preset`/`from_resolution` parse
  once via `EffectiveMode::from_config_str`; getter becomes
  `pub fn governance_mode(&self) -> EffectiveMode` (Copy).
- Consumers: server.rs ~408 and doctor.rs ~203 drop their `from_config_str` calls;
  explain.rs ~137 simplifies (direct enum). cli.rs ~147 and explain.rs ~149 read raw
  resolution/entry values, not `Config` -- they keep `from_config_str`, which remains on
  `EffectiveMode`.
- Config inline tests asserting `"observe"` strings assert `EffectiveMode::Observe`.

## Constraints

1. One commit: `refactor(architecture): t03 governance authorize and call audit`.
2. Byte-frozen: every audit record shape and value, every denial message and id, the
   hold message text, `check_call` and `DecisionRequest` (untouched), simulate
   (untouched). The black-box suites (`tests/tool_enforcement.rs`, `tests/shadow_mode.rs`,
   `tests/mcp_protocol.rs`, `tests/all_open_golden.rs`, `tests/audit_recorder.rs`) must
   pass with expectations unchanged, with ONE sanctioned exception: audit_recorder's
   record-driving sites are reworked onto begin/dispatch_finished/complete, and its two
   INJECTED-duration pins (the literal `42` and the second call's literal) become
   `rec["duration_ms"].as_u64().is_some()` -- the two-phase API owns the clock, so an
   injected literal duration is no longer expressible. Every OTHER assertion in that file
   (the 14-key field order, all other field values) stays byte-identical. Record as a
   sanctioned edit, not a deviation.
3. `tests/architecture.rs` green: everything added is governance-internal; `CallAudit`
   crosses nothing.
4. tools.json / fidelity test untouched. No new dependencies. ASCII only.
5. Delete what you replace: `rg -n "record_call|record_deny|record_shadow_deny|record_held|record_navigate_landing_deny" src/`
   after the change hits only `CallAudit`'s private internals (or nothing, if folded);
   `rg -n "tool_label" src/` -> nothing; `rg -n "from_config_str" src/` -> only
   `ports.rs` (the definition), `config/mod.rs` (the single parse where the Config
   snapshot is built), `cli.rs` (~147, raw Resolution value), and ONE `explain.rs` site
   (~149, raw entry value; the ~137 minimal-config site simplifies away with the typed
   getter).

## Tests (minimum)

Rework the dispatch.rs inline record tests onto the new API, keeping every pinned record
byte. Exactly these NEW names:

1. `begin_complete_produces_the_allow_record_bytes` -- begin + set_domain + complete
   reproduces the field assertions of the current
   `record_call_passes_the_resolved_domain_through` (dispatch.rs inline) PLUS the 14-key
   order pin transcribed from `tests/audit_recorder.rs` (there is no single pinned JSON
   blob today; these two named sources ARE the oracle -- transcribe, do not re-derive).
2. `authorize_miss_is_unknown_action_through_the_mode_switch` -- requires `None`,
   GOVERNED: enforce-mode -> `Gate::Deny` whose message and recorded rule transcribe the
   current classification-miss inline test's expectations; observe-mode -> `Proceed` and
   `complete()` records shadow_deny (same denial id as the enforce run). ALL-OPEN ->
   `Proceed` (the precedence table's arm 1).
2b. `governed_unknown_computer_action_is_denied_unknown_action` (NEW, black-box, in
   `tests/tool_enforcement.rs`): governed schema-3 manifest (any read grant), drive
   `computer` with `{"action":"bogus_action","tabId":1}`; the response text starts with
   `Denied (D-` and the audit line carries `decision == "deny"`, rule-derived denial id,
   `capability == "none"`. Transcribe the denial MESSAGE from
   `enforcement.rs::unknown_action_denial`'s template (read it in the tree; keep it
   byte-stable). This pins the sanctioned miss delta end to end.
3. `authorize_free_action_proceeds_without_grant_attribution` -- requires `Some(&[])`
   with a grants fixture that would deny: `Proceed`; complete records allow,
   `grant_id: null`, `capability: "none"`.
4. `sacred_deny_and_held_records_are_byte_stable` -- transcribed from the current
   record_deny/record_held pinned tests.
5. `landing_amendments_match_the_old_navigate_records` -- landing_allow overwrites
   attribution then complete; landing_deny reproduces the field assertions transcribed
   from `server.rs::point5_navigate_landing_off_grant_parks_and_denies` (decision deny,
   the landing domain, `grant_id` null, a real duration, tool "navigate" now coming from
   the scope). There is no old pinned-bytes blob; that named test IS the oracle.
5b. `sacred_domain_seeding_survives_on_allow_records` (NEW): all-open governance +
   non-empty sacred list + a call carrying a resolvable `tabId` on a NON-sacred host ->
   the allow record's `domain` equals the resolved tab host (pinning the section-3
   unconditional `set_domain(tab_domain)` seeding that today's
   `audit_domain = tab_domain.clone()` performs).
6. `one_lookup_feeds_decision_and_audit` -- structural: `Governance` no longer holds a
   requires fn (compile-level, plus a test driving authorize with a deliberately WRONG
   requires value proving the decision uses the caller's value, not a second lookup).
7. `governance_mode_is_typed` (config/mod.rs inline) -- minimal/preset configs yield
   `EffectiveMode` values directly; the two string-pinned tests updated.

## Verification

`cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test` all green
with the black-box suites expectation-unchanged; the three rg checks of Constraint 5;
ASCII scan; tools.json/fidelity byte-untouched. No browser checks queued (behavior
identical; the stage-3 s-live backlog already covers the observable surface). Update
LEDGER (entry + RESUME HERE -> t04) and commit.

## Out of scope

- The pipeline rewrite, registry consumption, pipeline.rs extraction (t04).
- Tab-URL unification (t05), hot-reload (t06), port-seam deletions and ports.rs split
  (t07), docs (t08).
- Any change to `check_call`, `DecisionRequest`, denial building, simulate, or the audit
  record FORMAT.
