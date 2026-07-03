# G15: Shadow enforcement (observe vs enforce at manifest and grant level)

## Goal

Add the `mode` switch that turns a fully evaluated deny into either a real block
(`enforce`) or a recorded-but-not-blocked event (`observe`, called shadow enforcement).
In observe mode a call that policy would deny is evaluated to completion (matching grant,
rule class, and stable denial id all computed exactly as enforce would compute them),
written to the audit log as `decision: "shadow_deny"` with that grant and denial id, and
then allowed to proceed so the agent sees the ordinary tool result. Status surfaces badge
shadow mode plainly so that observing can never be mistaken for protection. The one
carve-out: a user-authored sacred-domain denial is always enforced, in every mode.

This is ADR-0020 commitment 4.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every field
  name, file location, enum value, decision string, and message format in this task comes
  from it verbatim. Read it before writing any code. The load-bearing sections here are
  2.1 (resolved triple), 3.4 (`governance.mode` key and effective-mode precedence), 4.1
  (manifest `mode`), 4.3 (grant `mode`), 6.1 (audit `decision`), 7.2 (shadow paragraph),
  and 9.2 (`get_status` `governance` block).
- The stage-2 configuration-registry task: it adds the typed `governance.mode` enum key
  (variants `observe`, `enforce`; defaults `observe` for `fully_open`, `enforce` for
  `safe` and `restricted`) and the layered resolver that returns the resolved triple.
  G15 consumes the resolved `governance.mode` value; it does not define the key.
- The stage-2 manifest + enforcement task: it parses the manifest and grants, resolves the
  current-tab domain, and produces the raw would-deny verdict (the matching grant id, the
  rule class, and the structured denial with its stable denial id per shared-format
  section 7). G15 consumes that verdict and decides block vs shadow.
- The stage-2 audit-subsystem task: it writes exactly one JSON Lines record per call with
  the `decision`, `grant_id`, and `denial_id` fields of shared-format section 6. G15 emits
  the `shadow_deny` decision value on that record.
- The stage-2 status-surface tasks that own the `doctor` governance output, the
  `config list` command, and the native-messaging `get_status` reply (shared-format
  section 9). G15 adds the shadow badge to whichever of these surfaces exist when it runs.

Because several prerequisites reshape `src/dispatch.rs`, `src/policy/`, `src/audit/`, and
`src/mcp/server.rs` before G15 runs, the "Current behavior" section below records the tree
as it stands today. Do NOT trust it as the state you will edit. Re-read every file named
below before changing it, and integrate against the code the prerequisites actually
produced, not against the pre-governance no-op seams.

## Project context

Browser MCP is a governed browser automation system. A single Rust binary is both the MCP
server (JSON-RPC 2.0 over stdio, hand-rolled, tokio) and the Chrome native-messaging host.
A thin Manifest V3 extension executes CDP commands. The chain is:

    MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser

The two binary roles run as separate OS processes bridged by tokio-native named-pipe (on
Windows) or Unix-domain-socket (elsewhere) IPC.

Governance is a separable overlay (ADR-0013) that attaches at a single dispatch choke
point without touching tool code. The overlay is landing in stage 2 as a sequence of
tasks: audit flight recorder first, then sacred domains, then the full manifest engine
(ADR-0018 observe-then-enforce). G15 is the mode switch that sits on top of the finished
enforcement path and the finished audit path.

Two invariants govern this whole layer and G15 must preserve both:

- All-open stays first-class. With no manifest and default config, behavior is
  byte-identical to the pre-governance engine: enforcement STEP 0 short-circuits to Allow,
  no grant-based denials happen, and there is nothing to shadow.
- The engine is truthful. Denials, holds, and observe mode are reported plainly. Observing
  must never present as protection.

## Current behavior

All facts verified against the working tree at authoring time.

`src/dispatch.rs` (30 lines) is the documented seam and is still a no-op today:

- `PolicyDecision` (lines 13-17) has a single variant, `Allow`.
- `pub fn policy_check(_tool: &str) -> PolicyDecision` (lines 23-25) always returns
  `PolicyDecision::Allow`.
- `pub fn audit(_tool: &str) {}` (line 30; doc comment lines 27-29) does nothing.

The module doc says in so many words that the v1.5 overlay replaces these in place and that
STEP 0 short-circuits to Allow when no manifest is present, preserving all-open by
construction. The enforcement and audit prerequisite tasks will have replaced both
functions (and likely `PolicyDecision`) with real signatures that thread the resolved
`Config`, the current-tab URL, and a structured denial before G15 runs.

`src/policy/mod.rs` (103 lines) today holds only the seed registry:

- `KeyDef` (lines 25-33) with a single boolean `minimal_default`.
- One key: `content.security.secrets.redact` (lines 39-47).
- `Config` (lines 51-70) with one field, `secrets_redact`, and `Config::minimal()`.
- There is NO `governance.mode` key, no `Mode` enum, no layered resolver, and no manifest
  type yet. The configuration-registry and manifest prerequisites add these.

`src/mcp/server.rs` (155 lines) is the dispatch caller:

- `run` builds `let config = Config::default();` (line 28) once per session and threads it
  through `handle_line` and `handle_tools_call`.
- `handle_tools_call` (lines 116-155) reads `name` and `arguments`, then calls
  `dispatch::policy_check(name)` and `dispatch::audit(name)` (lines 132-133) as no-ops,
  then `browser.call(name, &args)` (line 135), then applies redaction to `read_page`
  output (lines 141-143) before returning `JsonRpcResponse::success`.
- There is no branch for denial, no shadow path, and no audit record write yet. The
  enforcement and audit prerequisites add those; G15 adds the third branch.

`src/audit/` does not exist yet (no such directory). The audit prerequisite creates it.

`src/native/messages.rs` (20 lines) documents the binary/extension wire protocol as prose.
There is no `get_status` / `get_config` / `set_config_key` handling yet; the
native-messaging settings prerequisite (shared-format section 9) adds it.

The doctor lives in the installer module, not a dedicated file: `src/install/mod.rs`
defines `pub fn run_doctor(_opts: DoctorOptions) -> Result<()>` (line 719), a read-only
diagnosis routed from `src/main.rs` (line 183, `Command::Doctor`). It prints a `Browsers:`
section and an `MCP clients:` section as `println!` lines (section header, two-space
indented detail lines) and has no governance section yet.

`Cargo.toml` dependencies: `tokio`, `serde`, `serde_json` (with `preserve_order`), `clap`,
`tracing`, `tracing-subscriber`, `thiserror`, `anyhow`, `dirs`. The shared-format crate
note (preamble) says `sha2`, `uuid`, and an RFC 3339 time source are added by earlier
stage-2 tasks; G15 adds no new dependency.

## Required behavior

G15 delivers three things: the mode data model and its resolution, the mode switch at the
dispatch choke point, and the shadow badge on status surfaces. Read the code the
prerequisites produced first; reuse their types where they already exist, and add only what
is missing.

### 1. Mode data model

Define an enum for enforcement mode with exactly the two shared-format variants:

    /// Effective enforcement mode of a policy decision (shared-format 3.4, 4.1, 4.3).
    pub enum Mode { Observe, Enforce }

- It serializes and parses as the lowercase strings `"observe"` and `"enforce"`. Any other
  string is a validation error. (If the configuration-registry task already models the
  `governance.mode` enum value as a `Mode`-like type, reuse that type instead of adding a
  second one; do not define two.)
- Parse a manifest-level `mode` field (shared-format 4.1): optional; when present it must be
  `"observe"` or `"enforce"`; when absent it is `None`.
- Parse a per-grant `mode` field (shared-format 4.3): optional per grant, same enum, same
  absent-is-`None` rule.
- If the manifest and grant structs (owned by the manifest prerequisite) do not yet carry
  these fields, add them: `mode: Option<Mode>` at the manifest top level and
  `mode: Option<Mode>` on each grant. If they already carry them, use them as-is. Unknown
  string values are a manifest validation error surfaced the same way the manifest task
  surfaces its other validation errors.

### 2. Effective-mode resolution

Add one pure function with the shared-format 3.4 precedence
(per-grant `mode` > manifest `mode` > resolved `governance.mode`):

    /// Resolve the effective mode of a single decision. `grant` is the matching grant's
    /// mode (None when no grant matched, e.g. unmatched_domain or scheme rules); `manifest`
    /// is the manifest-level mode; `config` is the resolved `governance.mode` value.
    pub fn effective_mode(grant: Option<Mode>, manifest: Option<Mode>, config: Mode) -> Mode

- `grant` wins when `Some`; else `manifest` when `Some`; else `config`.
- `config` is never optional: the layered resolver always defines `governance.mode`
  (built-in Minimal is the floor), so resolution never fails.

Also add a manifest-level helper the status surfaces need:

    /// The manifest-level effective mode = manifest.mode.unwrap_or(resolved governance.mode).
    /// This is the mode the top-level shadow badge reflects; per-grant overrides are
    /// per-decision and do not change the badge.

### 3. The mode switch at the dispatch choke point

The enforcement prerequisite produces, for each call, either "allow" or a structured
would-deny denial. That denial already carries its `grant_id` (the matching grant id, or
absent for `unmatched_domain` / `scheme`), its `rule` class, and its stable `denial_id`
(shared-format section 7). G15 wraps that verdict into the final decision:

    pub enum PolicyDecision {
        Allow,
        Deny(Denial),        // blocked: effective mode enforce, OR any sacred-domain denial
        ShadowDeny(Denial),  // effective mode observe: audit as shadow_deny, then execute
    }

(`Denial` is the enforcement/denial-format task's type. Extend the existing
`PolicyDecision` in `src/dispatch.rs` with the `ShadowDeny` variant; do not fork a new
type.)

Decision rules, in this order:

1. No would-deny denial -> `Allow`.
2. Would-deny denial whose rule class is `sacred` (shared-format 7.1) -> `Deny`, ALWAYS,
   in every mode. A user-authored sacred domain is never shadow-only (shared-format 3.4).
   Sacred denials also occur with no manifest active (all-open plus a sacred list); they
   are still `Deny`.
3. Any other would-deny denial (`unmatched_domain`, `access`, `tool`, `scheme`) -> compute
   `effective_mode(grant_mode, manifest_mode, resolved_governance_mode)`:
   - `Enforce` -> `Deny(denial)`.
   - `Observe` -> `ShadowDeny(denial)`.

Then wire the three-way branch where dispatch calls into the tool (this is
`handle_tools_call` in `src/mcp/server.rs` today, or wherever the enforcement prerequisite
moved the choke point):

- `Allow`: execute the tool, write one audit record with `decision: "allow"`, return the
  normal result.
- `Deny(d)`: do NOT execute the tool. Write one audit record with `decision: "deny"`,
  `grant_id` = `d.grant_id`, `denial_id` = `d.denial_id`, `duration_ms` = 0 (denied before
  dispatch, shared-format 6.1). Return the denial message result the denial-format task
  produces (a normal MCP text result starting `Denied (D-...):`, not a JSON-RPC error).
- `ShadowDeny(d)`: execute the tool normally, exactly as `Allow` would. Write one audit
  record with `decision: "shadow_deny"`, `grant_id` = `d.grant_id`, `denial_id` =
  `d.denial_id`, and the real wall-time `duration_ms` (the call ran). Return the ordinary
  tool result with NO denial text. The agent must not be able to tell a shadowed call from
  a permitted one by its result.

Because the `Deny` and `ShadowDeny` records derive from the same `Denial`, enforce and
observe on the same call produce identical `grant_id` and identical `denial_id`; only
`decision`, `duration_ms`, and whether the tool executed differ. That identity is the point
and it is a required test below.

If the audit task's `decision` type does not already carry a `shadow_deny` value, add it and
make it serialize as the exact string `"shadow_deny"`; reuse it if present. Add nothing to
the extension: mode resolution, the switch, and the record all live in the binary
(constraint 2).

### 4. Status surfaces badge shadow mode plainly

"Shadow mode active" is true when a manifest with a non-empty `grants` array is active AND
its manifest-level effective mode (section 2 helper) is `Observe`. Per-grant overrides do
not change this top-level flag (shared-format 9.2). When no manifest is active, or grants
is empty, or the manifest-level effective mode is `Enforce`, shadow is false.

Add the badge to every status surface that exists when G15 runs. Wording is plain ASCII and
must state that events are recorded but NOT blocked.

- `get_status` reply (shared-format 9.2), MANDATORY. The `governance` object carries
  `"mode": "observe" | "enforce"` (the manifest-level effective mode) and
  `"shadow": true | false` computed as above. `governance` is `null` when no manifest is
  active. If the native-messaging settings prerequisite has not landed the `get_status`
  handler yet, still add the resolver that computes this `governance` object so that
  handler and the doctor can share it; expose it as a small pure function returning the
  `{ mode, shadow }` pair (or `None` when no manifest).
- `browser-mcp doctor` (`run_doctor` in `src/install/mod.rs`, line 719 today), MANDATORY.
  Add a `Governance:` section printed with `println!`, in the same style as the existing
  `Browsers:` and `MCP clients:` sections (section header, two-space indented detail
  lines). Exact lines:
  - No manifest active:
    `  no manifest active (all-open); no grant-based denials`
  - Manifest active, effective mode `enforce`:
    `  mode  enforce (denied calls are blocked)`
  - Manifest active, effective mode `observe` (shadow):
    `  mode  observe (SHADOW: would-deny events are recorded to the audit log but are NOT blocked; this is observation, not protection)`
  Compute the section from the same pure resolver `get_status` uses, so the two surfaces
  can never disagree. The doctor's existing sections, return value (`Result<()>`), and
  exit behavior are unchanged by this section.
- `config list` (owned by the config-registry / status-surface task): where it renders the
  resolved `governance.mode` triple, and only if that command exists when G15 runs, append
  one plain line under an active manifest whose manifest-level effective mode is observe:
  `SHADOW: would-deny events are recorded but NOT blocked; this is observation, not protection.`
  If `config list` does not exist yet, do not create it; the `get_status` field and the
  doctor section satisfy the mandatory badge requirement.

Do not add badge text to any extension file. If a popup surface exists, it is pure
presentation that renders the `governance.shadow` boolean the binary already reports; that
rendering belongs to the settings-protocol task, not G15.

### 5. Tests

Add inline `#[cfg(test)]` unit tests next to the code and integration tests under `tests/`
as appropriate. At minimum:

1. `effective_mode` precedence: grant `Some(Observe)` over manifest `Some(Enforce)` over
   config `Enforce` yields `Observe`; manifest wins when grant is `None`; config wins when
   both are `None`. Cover all combinations.
2. Manifest and grant `mode` parsing: `"observe"` and `"enforce"` parse; absent yields
   `None`; an unknown string is a validation error.
3. Mode switch on a non-sacred would-deny (use `access`, `tool`, and `unmatched_domain`
   denials): `Enforce` yields `Deny`, `Observe` yields `ShadowDeny`, and the `Denial`
   inside both carries the identical `grant_id` and `denial_id`.
4. Sacred carve-out: a `sacred`-rule would-deny yields `Deny` under BOTH `Observe` and
   `Enforce`. Assert it is never `ShadowDeny`. Include the no-manifest sacred case.
5. Same manifest, enforce vs observe, end to end: run one call that policy would deny with
   the manifest-level mode set to `enforce`, then the same manifest and call with mode
   `observe`. Assert the audit record decision is `deny` vs `shadow_deny`; assert
   `grant_id` and `denial_id` are byte-identical across the two; assert the tool did NOT
   execute under enforce and DID execute (normal result, no `Denied (` marker) under
   observe.
6. Badge: the pure `governance` resolver returns `shadow = true` with `mode = "observe"`
   when a manifest with grants is active and manifest-level effective mode is observe;
   `shadow = false` with `mode = "enforce"` under enforce; `None` when no manifest is
   active. The doctor `Governance:` section prints the SHADOW line under observe and the
   plain enforce line under enforce.
7. All-open invariant: with no manifest and default `Config`, a normal tool call is allowed
   (`decision: "allow"`), nothing is denied or shadowed, and `governance` resolves to
   `None`. This must match pre-governance behavior.

## Constraints

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or description
   strings. `tests/tool_schema_fidelity.rs` must pass unchanged. G15 changes no tool
   schema text and does not add or remove tools.
2. The extension holds mechanism only: no policy, access, redaction, or mode decision in
   any extension JS. The mode resolver, the switch, and the shadow badge computation all
   live in the binary. The extension only ever renders a boolean the binary reports, and
   that rendering is not part of this task.
3. All-open stays first-class: with no manifest and default config, behavior is
   byte-identical to today (STEP 0 short-circuits to Allow). Preserve it and test it
   (test 7). Shadow mode only exists under an active manifest with grants.
4. ASCII only in all code and docs: no em-dashes, no arrows, no curly quotes, anywhere,
   including comments and message strings. Use ` -- ` (double hyphen) where the codebase
   uses it. All badge and denial text is plain ASCII.
5. The engine is truthful: observe mode must be reported as observation, never as
   protection. The badge text must contain "NOT blocked" and "observation, not protection"
   (or the exact wording above). A shadowed call returns the ordinary tool result with no
   denial text, and the audit record tells the whole truth (`shadow_deny`, real
   `duration_ms`).
6. No new runtime dependencies. `sha2`, `uuid`, and the time source are added by earlier
   stage-2 tasks; G15 adds none. Extension stays vanilla JS (and untouched here).
7. Rust 2021 edition; `thiserror` for library error types; doc comments on every public
   item and a module doc comment on any new module; `cargo fmt` clean; `cargo clippy
   --all-targets -- -D warnings` clean. Unit tests inline, integration tests in `tests/`.
8. Do NOT copy code from the official Anthropic extension or any other project; implement
   the behavior described here from scratch.

Task-specific:

9. The sacred carve-out is not optional and not configurable: `sacred`-rule denials are
   `Deny` in every mode, including observe and including the no-manifest case. A test must
   pin this.
10. Enforce and observe on the same call must yield identical `grant_id` and `denial_id`.
    Do not recompute the denial id differently on the shadow path; derive both decisions
    from one `Denial`.
11. Reuse the prerequisite types: extend `PolicyDecision`, reuse the manifest/grant structs,
    the `Denial` type, the audit `decision` type, and the resolved `Config`. Do not
    duplicate a `Mode` enum, a denial type, or a decision enum if one already exists.
12. The `get_status` `governance` block and the doctor `Governance:` section must be
    computed by one shared pure resolver so the two surfaces cannot disagree.

## Verification

1. From the repo root: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test` are all clean. `tests/tool_schema_fidelity.rs` passes without any edit.
2. Rebuild the binary. If `target/debug/browser-mcp.exe` is locked by a running session,
   rename it aside first (for example `mv target/debug/browser-mcp.exe
   target/debug/browser-mcp.exe.old-1`) and rebuild. Binary changes require an MCP client
   restart to observe in a live session; extension changes are not part of this task.
3. Prepare a manifest with a grant that would deny a specific call (for example an
   `access: "read"` grant on the current domain, then attempt a mutate-class call). Load it
   with the manifest-level `mode` set to `enforce`. Make the call: the agent gets the
   `Denied (D-...):` result, the tool did not run, and the audit log shows one record with
   `decision: "deny"`, the grant id, the denial id, and `duration_ms: 0`.
4. Change only the manifest `mode` to `observe` (keep everything else identical) and repeat
   the call: the agent gets the ordinary tool result (no `Denied (` text, the tool ran),
   and the audit log shows one record with `decision: "shadow_deny"`, the SAME grant id and
   the SAME denial id as step 3, and a nonzero `duration_ms`.
5. Confirm the sacred carve-out: add a `content.security.sacred_domains` entry matching the
   current domain, set manifest `mode: observe`, and make any call: it is blocked
   (`Denied (D-...):`), the tool did not run, and the record is `decision: "deny"`, not
   `shadow_deny`.
6. Status badge: with the observe manifest of step 4 active, run `browser-mcp doctor` and
   confirm the `Governance:` section prints the SHADOW line naming that events are recorded
   but NOT blocked. Switch to the enforce manifest and confirm the plain `mode  enforce`
   line. With no manifest, confirm the `no manifest active (all-open)` line. If the
   `get_status` handler exists, confirm its `governance` object reports
   `"mode": "observe", "shadow": true` under the observe manifest and
   `"shadow": false` under enforce.
7. All-open: run with no manifest and default config. A normal tool call returns its normal
   result, the audit log (if enabled) records `decision: "allow"`, nothing is denied or
   shadowed, and the doctor `Governance:` section shows the no-manifest line.

## Out of scope

- Pilot-group targeting or any rollout mechanics. Choosing which users receive an observe
  manifest is the MDM / deployment channel's job (ADR-0019, ADR-0020); G15 only implements
  what the mode switch does once a manifest is active.
- Any UI beyond the status text. No popup layout, no options-page controls, no toggles, no
  new extension surface. G15 emits the `governance.shadow` boolean and the plain badge
  lines; rendering them in the extension is the settings-protocol task's concern.
- Defining the `governance.mode` registry key, the layered resolver, presets, or the
  resolved triple (configuration-registry task).
- Parsing the manifest beyond the `mode` fields, resolving the current-tab domain, matching
  grants, classifying observe vs mutate, computing the raw would-deny verdict, or forming
  the denial id and denial message text (manifest + enforcement + denial-format tasks).
  G15 consumes these; it does not build them.
- Writing the audit record shape, choosing destinations, or the JSON Lines framing (audit
  task). G15 only sets `decision`, and reuses `grant_id` / `denial_id` / `duration_ms`.
- `policy simulate`, `policy explain`, JSON Schema generation, or the `config list` command
  itself (other stage-2 tasks). G15 only appends a shadow line to `config list` if that
  command already exists.
- Any change to `src/mcp/schemas/tools.json`, the tools/list surface, tool routing, or any
  tool result text other than substituting the denial-format task's denial message on the
  `Deny` branch.
- New dependencies in `Cargo.toml`, including dev-dependencies.
- Any change to the native-host zombie-fix `std::process::exit(0)`, the IPC transport, the
  installer, or the debug/observability subsystem beyond the doctor `Governance:` section.
