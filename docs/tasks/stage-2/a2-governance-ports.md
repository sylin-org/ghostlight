# A2: Define the governance seam in governance/ports.rs

## Goal

Introduce the governance port layer as a single new file, `src/governance/ports.rs`: the
trait and type contract that every later stage-2 task (g05, g06, g07, g08, g12, g13, g14,
g15, g17) implements or consumes. This task is PURELY ADDITIVE and changes NO runtime
behavior: it defines the traits `PolicyDecisionPoint`, `DomainPolicy`, `ResourceResolver`,
and `AuditSink`; the decision types `DecisionRequest`, `Decision`, `GoverningResource`,
`RwClass`, `EffectiveMode`; the placeholder supporting types `Grant`, `ToolId`,
`ResourcePattern`, `Denial`, `AuditRecord`; and two zero-policy implementations, `NoopPdp`
(always `Allow`) and `NullSink` (drops every record), so the all-open facade (A3) can wire
a real port with zero real policy. It wires NOTHING into `dispatch` and reads NO config.
The load-bearing property is that `DecisionRequest` and `Decision` are serde-serializable,
so the decision point can relocate out-of-process later (the persistent-service direction)
and so g17 (simulate) can replay recorded requests through the same decision function.

## Depends on

- **A1 (module reorg).** A1 regroups the tree into `governance/` (domain-agnostic core),
  `browser/` (the domain plugin), and `transport/` (infra), and moves `dispatch.rs` and the
  `policy/` module under `src/governance/`. A2 adds a file inside that new `governance/`
  module, so A1 MUST be landed first. If `src/governance/mod.rs` does not exist in your
  tree, stop: A1 is the prerequisite and has not landed.
- `docs/design/ghostlight-service-architecture.md` sections 3 (bounded contexts) and 4 (the
  seam trait sketches) are authoritative for the shapes in this task. Read them before
  writing any code; the trait and type names in this prompt come from there.
- `docs/tasks/stage-2/PLAN.md` "Phase A" (item A2) and the "Resolved decisions". No other
  stage-2 task is a prerequisite. A2 is a prerequisite for A3 (the facade) and is consumed
  by the Phase B/C/D tasks.

## Project context

Browser MCP is governed browser automation. A single Rust binary is both the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled on tokio, no MCP SDK crate) and the Chrome
native-messaging host; a thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

Stage 1 (`docs/tasks/release-1/`) shipped and merged to `main`: the engine is hardened and
all-open. Stage 2 is the governance layer per ADR-0013 (separable overlay; all-open stays
first-class), ADR-0018 (observe-then-enforce sequencing), ADR-0019 (layered configuration),
and ADR-0021 (Ghostlight family + the S1 chassis / S4 PDP contract baseline).

The stage-2 module split (from A1) is:

- `governance/` -- the domain-agnostic core: the decision skeleton, config registry, audit
  recorder, manifest identity. Names NO browser type. Depends on `std` + `serde` only.
- `browser/` -- the domain plugin: the URL/domain matcher, the 13-tool classification table,
  the tab-URL resolver, sacred domains, redaction, and the extension wire. Depends on the
  core; the core NEVER depends on it.
- `transport/` -- infra: MCP session, native messaging, IPC, the executor handle, and the
  dispatch chokepoint (the enforcement point).

An arch-test (A7) will fail CI if anything under `governance/**` names `browser`,
`transport`, `mcp`, `native`, or the `url` crate. This file (`governance/ports.rs`) must
stay inside that fence: it uses only `std`, `serde`, and `serde_json`.

All-open stays byte-identical: with no manifest and default config, every tool result is
exactly what stage 1 produced. This task defines the seam that makes that guarantee cheap
(the facade in A3 short-circuits to `Allow` at STEP-0), but changes nothing that runs today.

## Current behavior

Verified against the tree; line numbers drift after A1 lands, so trust the prose.

- After A1, `src/governance/mod.rs` exists and declares the moved submodules (at least the
  former `policy` module and `dispatch`). `src/governance/ports.rs` does NOT exist. The
  exact `mod` declaration list in `governance/mod.rs` is A1's output; read it in your tree.
- `dispatch` (now under `src/governance/`) still holds the stage-1 no-op governance hooks:
  a `PolicyDecision` enum with a single `Allow` variant, a `policy_check(_tool: &str) ->
  PolicyDecision` that always returns `Allow`, and an `audit(_tool: &str)` that does nothing.
  A2 does NOT touch, replace, or reference these: they are A3's concern. A2 only adds a new
  file whose types A3 will later wire in.
- `Cargo.toml` already has `serde = { version = "1", features = ["derive"] }` and
  `serde_json = { version = "1", features = ["preserve_order"] }`. Both derive macros and
  `serde_json::Value` are available. `thiserror = "2"` is present. There is NO `async-trait`
  dependency, and this task does NOT add one (see Required behavior part 4).
- The config registry (the former `policy/mod.rs`, now under `governance/`) is stage-1
  bool-only; A4/G01 grows it. A2 does not touch it and does not depend on it.
- `tests/tool_schema_fidelity.rs`, `tests/mcp_protocol.rs`, and `tests/peer_death.rs` exist
  and must pass unchanged. None references governance ports.

## Required behavior

Everything lands in ONE new file, `src/governance/ports.rs`, plus a single `pub mod ports;`
line added to `src/governance/mod.rs`. Six parts. Every public item and the module itself
gets a doc comment.

### 1. Module wiring

Add to `src/governance/mod.rs`, next to the other submodule declarations, exactly:

```rust
pub mod ports;
```

Give `src/governance/ports.rs` a module-level doc comment stating: this is the governance
seam (the S4 PDP/PEP contract); the decision is a PURE, serializable function so it can run
in-process today and out-of-process later; the pure half (`DomainPolicy`) travels WITH the
decision, the impure half (`ResourceResolver`) stays at the enforcement point; single-impl
ports use generics/concrete types (zero vtable), and `dyn` is used only for
`PolicyDecisionPoint` and `AuditSink` (each has more than one impl today or a known
future one). Add `use serde::{Deserialize, Serialize};` at the top.

### 2. Supporting placeholder and axis types

These are minimal now; the named g-doc fleshes each out and MAY minimally adjust the shape
when it lands. Every one derives what `DecisionRequest`/`Decision` need to be serde
round-trippable and comparable in tests.

```rust
/// Read/write classification of a tool call: the observe-vs-mutate axis (the core owns the
/// axis; g05 owns the tool+action -> class table in the browser plugin). `Read` is an
/// observation; `Write` is a mutation. g05 maps each tool/action onto this and MAY extend
/// the type minimally when it lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RwClass {
    Read,
    Write,
}

/// The effective enforcement mode for a call (g15 resolves it: per-grant > manifest >
/// `governance.mode`). `Observe` records a shadow denial but allows; `Enforce` blocks.
/// Wire names are `observe` / `enforce`, matching the `governance.mode` config enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveMode {
    Observe,
    Enforce,
}

/// One resolved manifest grant. Placeholder: g12 (manifest engine) fleshes this out to
/// `{ domains, access, tools, mode }`. Only `id` is defined now, so `Decision::Allow` can
/// attribute the matching grant (g13). Kept minimal and serde-round-trippable on purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    /// Stable identifier of this grant, used for allow-attribution and audit.
    pub id: String,
}

/// A tool identifier as advertised on the MCP surface. Placeholder newtype; g07/g14 flesh
/// out the tool-surface handling. The sacred tool schemas (ADR-0007) are the source of
/// truth for the actual names; this type never mutates them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolId(pub String);

/// A resource-matching pattern (a domain pattern for the browser plugin). Placeholder
/// newtype; g07 (the CVE-hardened matcher) and g12 (grant domains) flesh out the semantics.
/// Only syntax/shape is a wrapper here; no matching logic lives in the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourcePattern(pub String);

/// A structured denial. Placeholder: g08 introduces the stable denial-id scheme and g13 the
/// full reason set. Two fields now, both serde-round-trippable, so `Decision::Deny` and
/// `Decision::ShadowDeny` carry something meaningful before g08 lands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Denial {
    /// Stable denial identifier (g08 pins the scheme).
    pub denial_id: String,
    /// Human-readable reason surfaced to the caller.
    pub reason: String,
}

/// One audit record: the flight-recorder line for a single tool call. Placeholder: g06
/// fleshes out the full record (identity, client, tool, action, rw, domain, decision,
/// timing). Only `tool` is defined now so `AuditSink` has a concrete argument type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    /// The tool that was called.
    pub tool: String,
}
```

### 3. The core decision types (serde is load-bearing)

```rust
/// A generic governing resource, so the decision core stays domain-agnostic. The browser
/// plugin fills `Resource(host)`; a filesystem module would fill `Resource(path)`.
/// `AlwaysAllow` is the resource-exempt case (browser: `about:blank`); `None` is a
/// resource-less call; `Indeterminate` means resolution failed and the decision must fail
/// closed under a manifest. g07/g12 refine how these are produced; the enum shape is stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoverningResource {
    /// A concrete governed resource (browser: a host such as `github.com`).
    Resource(String),
    /// The call targets an always-allowed resource (browser: `about:blank`).
    AlwaysAllow,
    /// The resource is outside the governed scope; carries a describing string.
    OutOfScope(String),
    /// The call has no governing resource (a resource-less tool).
    None,
    /// The resource could not be resolved; fail closed under a manifest.
    Indeterminate,
}

/// The complete, self-contained input to a policy decision. PURE and serde-serializable so
/// the decision can run in-process today and out-of-process later without a rewrite, and so
/// g17 (simulate) can replay a recorded request through the same decision function. Nothing
/// here references live state: resource resolution already happened (see `ResourceResolver`)
/// and its result is baked into `resource`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionRequest {
    /// The grants in force for this subject (empty under all-open).
    pub grants: Vec<Grant>,
    /// The tool being called.
    pub tool: String,
    /// The tool call's read/write classification.
    pub rw: RwClass,
    /// The resolved governing resource.
    pub resource: GoverningResource,
    /// The effective enforcement mode.
    pub mode: EffectiveMode,
}

/// The outcome of a policy decision. `Allow` optionally names the grant that permitted the
/// call (for attribution/audit). `Deny` blocks; `ShadowDeny` would have blocked but the
/// mode is observe, so the call is allowed and the denial is recorded (g15). Serde-derived
/// so an out-of-process PDP can return it over the wire and g17 can compare replays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// The call is permitted; `grant_id` is the matching grant, if any.
    Allow { grant_id: Option<String> },
    /// The call is blocked.
    Deny(Denial),
    /// The call would be blocked under enforce; observe mode allows it and records the denial.
    ShadowDeny(Denial),
}
```

### 4. The traits

`PolicyDecisionPoint` and `AuditSink` are `dyn` (each has more than one impl: Noop/Local/
future-Remote, and file/stderr/syslog). `DomainPolicy` and `ResourceResolver` are single-impl
plugin ports consumed via generics/concrete types (zero vtable); do NOT box them.

```rust
/// The policy decision point: a PURE, relocatable function from a serializable request to a
/// decision. `dyn` because it has multiple impls (the `NoopPdp` here, a Local PDP in g13,
/// and a future out-of-process Remote PDP). Send + Sync so it can be shared across the
/// tokio runtime.
pub trait PolicyDecisionPoint: Send + Sync {
    /// Decide the outcome for a fully-resolved request. Must be pure: no I/O, no live state.
    fn decide(&self, req: &DecisionRequest) -> Decision;
}

/// The domain plugin's PURE half: classification, resource matching, sacred detection, and
/// the advertised tool surface. It travels WITH the decision (it can relocate out-of-process
/// with the PDP). Single-impl (the browser plugin); consumed via a concrete type or a
/// generic bound, never `dyn`. g05 provides `classify`, g07 provides `matches`, g08 provides
/// `is_sacred`, g07/g14 provide `tool_surface`; the trait MAY be minimally adjusted when they
/// land (for example splitting `classify`/`matches` into sub-traits if that reads cleaner).
pub trait DomainPolicy {
    /// Classify a tool (and optional sub-action) as read or write. `None` if unknown.
    fn classify(&self, tool: &str, action: Option<&str>) -> Option<RwClass>;
    /// True if `pattern` matches `resource` under the plugin's matching semantics.
    fn matches(&self, pattern: &ResourcePattern, resource: &GoverningResource) -> bool;
    /// True if `resource` is a sacred never-touch resource (always enforced).
    fn is_sacred(&self, resource: &GoverningResource) -> bool;
    /// The tools this plugin advertises on the MCP surface.
    fn tool_surface(&self) -> &[ToolId];
}

/// The domain plugin's IMPURE half: resolve the governing resource from live state (browser:
/// the active tab's URL). It stays at the enforcement point forever and NEVER relocates
/// out-of-process (it needs live state). Single-impl; consumed via a concrete type or a
/// generic bound, never `dyn`. Async because resolving the resource is I/O (a CDP round-trip
/// for the browser plugin). g07/g13 provide the browser impl.
///
/// This uses a native `async fn` in a trait (stable since Rust 1.75) rather than the
/// `async-trait` crate: the port is single-impl and consumed concretely, so it does not need
/// to be `dyn`-compatible, and avoiding `async-trait` keeps the dependency set lean (no
/// per-call boxing). The `async_fn_in_trait` lint is allowed for exactly this reason.
#[allow(async_fn_in_trait)]
pub trait ResourceResolver {
    /// Resolve the governing resource for a tool call from its arguments and live state.
    async fn governing_resource(&self, tool: &str, args: &serde_json::Value) -> GoverningResource;
}

/// A sink for audit records. `dyn` because it has multiple impls (the `NullSink` here, plus
/// file/stderr/syslog in g06). Send + Sync so it can be shared across the runtime. Recording
/// is fire-and-forget: it returns nothing and must not fail the call.
pub trait AuditSink: Send + Sync {
    /// Record one audit line. Must not panic and must not block the call path meaningfully.
    fn record(&self, record: &AuditRecord);
}
```

### 5. Zero-policy implementations

These make A3's all-open facade wireable with no real policy. Both are unit structs.

```rust
/// The no-op policy decision point: allows every call. This is the STEP-0 all-open PDP; the
/// facade (A3) uses it when there is no manifest, preserving byte-identical stage-1 behavior.
/// g13 provides the real (Local) PDP that runs the grant-check decision.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopPdp;

impl PolicyDecisionPoint for NoopPdp {
    fn decide(&self, _req: &DecisionRequest) -> Decision {
        Decision::Allow { grant_id: None }
    }
}

/// An audit sink that drops every record. Used under all-open (audit disabled) so the audit
/// seam is always wired without emitting anything. g06 provides the file/stderr/syslog sinks.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl AuditSink for NullSink {
    fn record(&self, _record: &AuditRecord) {}
}
```

### 6. Unit tests (inline `#[cfg(test)]` in `src/governance/ports.rs`)

Cover the load-bearing properties: the zero-policy impls behave, the dyn ports are actually
object-safe, and the decision types round-trip through serde.

- `noop_pdp_allows_every_request`: build a couple of distinct `DecisionRequest` values
  (varying `rw`, `resource`, `mode`, with and without grants) and assert
  `NoopPdp.decide(&req) == Decision::Allow { grant_id: None }` for each.
- `null_sink_record_is_a_noop`: call `NullSink.record(&AuditRecord { tool: "navigate".into() })`
  and assert it returns (it cannot fail; this proves it compiles and does not panic).
- `pdp_is_object_safe`: store `NoopPdp` as `Box<dyn PolicyDecisionPoint>`, call `decide`
  through the box, assert `Allow`. This pins that `PolicyDecisionPoint` stays `dyn`-compatible.
- `audit_sink_is_object_safe`: store `NullSink` as `Box<dyn AuditSink>` and call `record`
  through the box. Pins `AuditSink` `dyn`-compatibility.
- `decision_request_round_trips_through_serde`: build a `DecisionRequest` with a non-empty
  `grants` vec, serialize with `serde_json::to_string`, deserialize back with
  `serde_json::from_str`, and assert equality with the original. This is the load-bearing
  serializability test (the PDP relocation seam and g17 replay depend on it).
- `decision_round_trips_through_serde`: round-trip each `Decision` variant
  (`Allow { grant_id: Some(..) }`, `Allow { grant_id: None }`, `Deny(Denial { .. })`,
  `ShadowDeny(Denial { .. })`) through `serde_json` and assert equality.
- `rw_and_mode_wire_names_are_lowercase`: assert `serde_json::to_string(&RwClass::Read)` is
  `"\"read\""`, `RwClass::Write` is `"\"write\""`, `EffectiveMode::Observe` is
  `"\"observe\""`, and `EffectiveMode::Enforce` is `"\"enforce\""` (pins the wire vocabulary
  so g05/g15 and the config enum stay aligned).

No new files under `tests/`; existing integration tests must pass unchanged.

## Constraints

1. ASCII only in ALL code and docs: no em-dashes, no arrows, no curly quotes, anywhere,
   including comments and test strings. Use Rust `\u{..}` escapes if a test ever needs a
   non-ASCII input (none here should).
2. All-open stays first-class and byte-identical: with no manifest and default config, every
   tool result is exactly what stage 1 produced (the STEP-0 short-circuit). This task changes
   NO runtime behavior at all: it adds one file and one `pub mod` line. Nothing calls the new
   types yet (A3 does the wiring).
3. NEVER modify the tool schemas (`src/mcp/schemas/tools.json`), tool names, params, or
   descriptions; `tests/tool_schema_fidelity.rs` must pass unchanged (ADR-0007, the sacred
   surface). This task does not touch the schema surface at all.
4. The extension holds mechanism only: no policy, access, or redaction decisions in extension
   JS. This task touches no extension file.
5. Rust 2021, `thiserror` for typed errors (none are introduced here; `Denial` is a data
   struct, not an error type), doc comments on ALL public items and the module, `rustfmt`
   clean, `cargo clippy --all-targets -- -D warnings` clean. The only permitted lint
   suppression is `#[allow(async_fn_in_trait)]` on `ResourceResolver`, with the documented
   rationale in part 4.
6. One task = one commit (code + tests + ledger/browser-test updates). Keep the tree green
   between tasks (full suite + clippy + fmt).
7. Windows dev gotcha: if `target/debug/browser-mcp.exe` is locked by a running session,
   rename it aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`)
   and rebuild, or stop the MCP client first.

Task-specific:

8. Stay inside the core fence (A7 arch-test): `governance/ports.rs` may `use` only `std`,
   `serde`, and `serde_json`. It must NOT name `browser`, `transport`, `mcp`, `native`, or
   the `url` crate.
9. Do NOT add any dependency. `serde` (with `derive`) and `serde_json` already cover this
   task. Do NOT add `async-trait`; use native `async fn` in the `ResourceResolver` trait
   (see part 4). Do NOT add `uuid`, `sha2`, `chrono`, or a schema crate; those belong to
   later tasks.
10. `dyn` only for `PolicyDecisionPoint` and `AuditSink`. `DomainPolicy` and
    `ResourceResolver` are single-impl ports; define them as plain traits consumed via
    generics/concrete types, and do NOT box them anywhere in this task.
11. Do NOT touch `dispatch` (the stage-1 no-op `PolicyDecision`/`policy_check`/`audit`), the
    config registry (the former `policy/mod.rs`), `src/main.rs`, anything under `src/native/`,
    `src/mcp/`, `src/tools/`, `src/browser/`, `src/transport/`, or `extension/`. A2 is only
    the new `ports.rs` file plus the one-line `pub mod ports;` in `governance/mod.rs`.
12. No concrete policy logic. `NoopPdp` unconditionally allows and `NullSink` unconditionally
    drops; the placeholder types carry only the minimal fields named above. Any real
    classification, matching, sacred detection, grant check, denial-id scheme, or audit
    record shape belongs to the named g-doc, not here.

## Verification

1. `cargo fmt` then `cargo clippy --all-targets -- -D warnings` from the repo root: clean
   (the single `#[allow(async_fn_in_trait)]` is the only suppression).
2. `cargo test` from the repo root: all tests pass, including every new unit test in part 6,
   and the unchanged `tests/tool_schema_fidelity.rs`, `tests/mcp_protocol.rs`, and
   `tests/peer_death.rs`.
3. Arch-fence check (manual until A7 lands): `governance/ports.rs` imports only `serde` and
   `serde_json` (grep the file for `browser`, `transport`, `mcp`, `native`, `url ::`, `use
   url`; none should appear).
4. Byte-identical smoke: nothing runtime changed. `tests/mcp_protocol.rs` (the tools/list and
   dispatch round-trip) passes unchanged, which is the all-open guard for this task.
5. If `target/debug/browser-mcp.exe` is locked, rename it aside and rebuild (see constraint 7).

## Out of scope

- The `Governance` facade and any `dispatch` wiring of these ports (A3). A2 defines the
  contract; A3 makes `dispatch` the enforcement point that holds a `Governance` facade whose
  `all_open()` is a literal zero-cost `Allow`.
- Any concrete policy logic: classification tables (g05), the CVE-hardened domain matcher
  (g07), sacred-domain detection and the denial-id scheme (g08), the manifest engine and the
  real `Grant` shape (g12), the Local PDP `check_call` decision and the 5 enforcement points
  (g13), tool-advertisement filtering (g14), shadow mode (g15), and simulate replay (g17).
- The audit recorder and the file/stderr/syslog sinks and the full `AuditRecord` shape (g06).
- The typed key registry and layered resolver (A4 / g01 / g02), file loading, and hot-reload
  (A5). A2 reads no config and watches no files.
- Making `DomainPolicy` / `ResourceResolver` object-safe or boxing them: they are single-impl
  ports; only `PolicyDecisionPoint` and `AuditSink` are `dyn` here.
- Adding any dependency (`async-trait`, `uuid`, `sha2`, `chrono`, or a schema crate).
