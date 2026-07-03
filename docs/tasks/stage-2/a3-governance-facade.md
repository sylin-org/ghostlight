# A3: Governance facade + the dispatch chokepoint (PEP)

## Goal

Replace the current no-op dispatch seam (`policy_check` / `audit` / `PolicyDecision` in
`src/dispatch.rs`) with a single `Governance` facade that holds the governance ports (a
`PolicyDecisionPoint`, an `AuditSink`, and later the domain plugin halves) and exposes ONE
per-call entry the MCP server invokes at the `tools/call` chokepoint. `Governance::all_open()`
constructs a facade whose decide path is a literal, zero-cost `Allow` that queries NO port and
resolves NO resource (the STEP-0 short-circuit), so a session with no manifest and default config
stays byte-identical to stage 1. Wire the MCP server to call the facade exactly once per tool call,
and extend the all-open golden test so the facade path is proven byte-identical: the `tools/list`
bytes, a dispatch round-trip, and `read_page` secret redaction (still governed by the existing
`content.security.secrets.redact` key) all unchanged.

This task builds the enforcement-point CHASSIS. It does NOT build real enforcement (acting on a
`Deny`), audit records, classification, resource resolution, or any config change. Those attach
inside `Governance::decide` and the port impls in later stage-2 tasks.

## Depends on

- **A1 (module reorg).** A1 regroups the tree into `governance/` (domain-agnostic core), `browser/`
  (the domain plugin), and `transport/` (infra), and relocates `dispatch.rs` and `policy/` under
  `governance/`, and `mcp/` and `native/` under `transport/`. This task edits the relocated
  `dispatch.rs` (the facade lives there per the PLAN) and the relocated MCP server. Trust the actual
  post-A1 tree: use the real module paths A1 produced, not the pre-A1 paths quoted in "Current
  behavior" below. If A1 has NOT landed in your tree, apply the same edits at the pre-A1 paths
  (`src/dispatch.rs`, `src/mcp/server.rs`) and the task still holds.
- **A2 (ports).** A2 adds `governance/ports.rs` with the seam contract this task consumes:
  `PolicyDecisionPoint` (the `dyn` decision port), `Decision` (`Allow { grant_id } / Deny(Denial) /
  ShadowDeny(Denial)`), `DecisionRequest` (the pure serializable request), `GoverningResource`,
  `RwClass`, `EffectiveMode`, `Denial`, `AuditSink` (the `dyn` audit port) and its `AuditRecord`
  input type, plus a `Noop` decision point (the v1 impl that always allows). Consume A2's exact type
  and variant names; where this prompt guesses a variant name (for example `RwClass::Observe`,
  `EffectiveMode::Observe`, the Noop PDP type name), substitute the name A2 actually shipped.
- `docs/tasks/stage-2/PLAN.md` (Phase A, items A1-A3; the "Resolved decisions" and the STEP-0 rule)
  and `docs/design/ghostlight-service-architecture.md` sections 3 (bounded contexts) and 4 (the seam
  trait sketches). These are authoritative for the seam shape and the dependency direction.
- No G-numbered task is a prerequisite. G05 (classification), G06 (audit records), G07 (matcher),
  G08 (`Deny`), G12/G13 (manifest + grant enforcement), and G15 (shadow mode) all BUILD ON this
  facade: they fill in `Governance::decide` and the port impls, they do not reintroduce the removed
  free functions.

## Project context

Browser MCP is governed browser automation. A single Rust binary is both the MCP server (JSON-RPC
2.0 over stdio, hand-rolled on tokio, no MCP SDK crate) and the Chrome native-messaging host; a thin
Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

Stage 1 shipped and merged to `main`: the all-open engine, hardened, with the tool schemas frozen
(ADR-0007). Stage 2 is the governance layer, added as a separable overlay per ADR-0013 (all-open
stays first-class and byte-identical), ADR-0018 (observe before enforce), ADR-0019 (layered typed
config), and ADR-0021 (Ghostlight family; single binary, in-crate module split now, crate extraction
deferred). The module split is `governance/` (core: the pure decision skeleton, config, audit,
manifest; names no browser type), `browser/` (the domain plugin: URL/domain matcher, the 13-tool
classification table, sacred list, redaction, the extension wire), and `transport/` (infra: MCP
session, native messaging, IPC, the executor handle, and the dispatch chokepoint). Dependency
direction is strictly inward and enforced by a fail-closed arch-test (A7): nothing under
`governance/**` may name `browser`, `transport`, `mcp`, `native`, or the `url` crate.

The decision is a pure, serializable function (`DecisionRequest -> Decision`), split from impure,
local resource resolution. The facade this task builds is the Policy Enforcement Point (PEP): the one
place per call where the overlay attaches. Under all-open it is a no-op by construction.

## Current behavior

Verified against the working tree. Line numbers drift (A1 relocates files and T-tasks restructured
the server); trust the prose over the numbers, and use the post-A1 module paths.

- `src/dispatch.rs` (31 lines) is the no-op seam. It defines `PolicyDecision` (a `Debug, Clone, Copy,
  PartialEq, Eq` enum with a single variant `Allow`), `pub fn policy_check(_tool: &str) ->
  PolicyDecision` returning `PolicyDecision::Allow`, and `pub fn audit(_tool: &str) {}`. The module
  doc says the v1.5 overlay replaces these in place.
- `src/mcp/server.rs`: `run(browser: Browser)` builds `let config = Config::default();` once, spawns
  a single stdout writer task, and loops over stdin lines calling
  `handle_line(&browser, config, line, &tx)`. `config` is passed by value because `Config` is `Copy`
  (that is still true at this task's point in the sequence; G01/A4 makes it owned LATER, not here).
  `handle_line` routes `initialize` / `tools/list` / `tools/call` / `ping`. The `tools/call` arm
  clones `browser` and `tx`, moves `config` (Copy) into a spawned task, and calls
  `handle_tools_call(&browser, config, id, params.as_ref())`.
- Inside `handle_tools_call`, after the unknown-tool pre-check and BEFORE the bounded first-call
  wait, the two no-op seam calls run:

  ```rust
  // v1.0 engine: the policy and audit seams are no-ops (all-open). The v1.5 overlay slots in here
  // without touching this code (see src/dispatch.rs).
  let _decision = dispatch::policy_check(name);
  dispatch::audit(name);
  ```

  Then the bounded wait, then `browser.call(name, &args).await`. On `Ok(mut result)`, if
  `name == "read_page"` it calls `policy::redact::apply_to_result(&mut result, config.secrets_redact())`
  before returning; the wait note is appended if the call waited. That redaction call site is the
  `content.security.secrets.redact` overlay and MUST stay exactly where it is.
- `src/mcp/server.rs` imports `use crate::dispatch;` and `use crate::policy::{self, Config};`.
- The only callers of `dispatch::policy_check` / `dispatch::audit` / `PolicyDecision` anywhere in
  `src/` are these two lines in `server.rs` (grep-verified). Removing the free functions is
  contained.
- Tests: `tests/mcp_protocol.rs` drives the binary over stdio. `initialize_tools_list_and_tool_call_over_stdio`
  asserts the `tools/list` result equals the sacred fixture byte for byte AND that a `tools/call`
  with no extension returns the exact hop-attributed message
  `"[hop: extension] Browser extension not connected. Next step: check chrome://extensions and that
  Chrome is running."`. `tools_call_waits_for_a_late_extension_and_notes_the_wait` connects a fake
  extension over the real IPC and asserts a successful `navigate` round-trip plus the wait note.
  `tests/tool_schema_fidelity.rs` guards the schemas. `src/policy/redact.rs` has thorough inline unit
  tests for the `secret_value="..."` marker rewrite.
- A1 is expected to add an all-open golden test (for example `tests/all_open_golden.rs`) asserting
  `tools/list` byte-equality plus a dispatch round-trip. This task EXTENDS that test (or adds it if
  A1 left it as a stub). A2 is expected to add `governance/ports.rs`.

## Required behavior

Six parts. Parts 1-4 are the facade and the wiring; parts 5-6 are the tests. Literal Rust is given
where it removes ambiguity; where it references an A2 type by a guessed variant name, use A2's real
name.

### 1. Remove the no-op seam from `dispatch.rs`

Delete `PolicyDecision`, `policy_check`, and `audit` entirely. The decision type is now A2's
`Decision`; the facade replaces both free functions. Rewrite the module doc to describe the PEP:

```rust
//! Tool-call dispatch chokepoint -- the single Policy Enforcement Point (PEP).
//!
//! Every `tools/call` passes through [`Governance::decide`] exactly once, before the tool
//! executes. The [`Governance`] facade holds the governance ports (a
//! [`PolicyDecisionPoint`](crate::governance::ports::PolicyDecisionPoint), an
//! [`AuditSink`](crate::governance::ports::AuditSink), and later the browser plugin halves) and is
//! the one place the stage-2 overlay attaches. It replaces the v1.0 no-op `policy_check` / `audit`
//! seams.
//!
//! [`Governance::all_open`] is the ungoverned engine: its decide path is a literal STEP-0
//! short-circuit to [`Decision::Allow`](crate::governance::ports::Decision) that queries no port and
//! resolves no resource, so a session with no manifest and default config is byte-identical to
//! stage 1 (ADR-0013).
```

### 2. The `Governance` facade

Add to `dispatch.rs` (adjust the `use` paths to the post-A1 layout):

```rust
use std::sync::Arc;

use crate::governance::ports::{
    AuditSink, Decision, DecisionRequest, EffectiveMode, GoverningResource, PolicyDecisionPoint,
    RwClass,
};

/// The governance facade held at the dispatch chokepoint: the Policy Enforcement Point.
///
/// One instance lives for the whole MCP session. It is either the ungoverned engine
/// ([`Governance::all_open`], holding no port) or a governed overlay holding the ports. The MCP
/// server calls [`Governance::decide`] once per tool call.
pub struct Governance {
    mode: Mode,
}

/// The two shapes of the facade. `AllOpen` holds nothing so its decide path is a zero-cost
/// short-circuit; `Governed` holds the ports that later tasks drive.
enum Mode {
    /// STEP-0: the ungoverned engine. No manifest, default config. Every call is `Allow`.
    AllOpen,
    /// The governed overlay. Populated by later stage-2 tasks; the pure/impure browser plugin
    /// halves (DomainPolicy classify/match, ResourceResolver) attach through builder methods added
    /// by G05/G07/G13.
    Governed(GovernedState),
}

/// The ports a governed facade holds. `dyn` here is deliberate: the decision point has multiple
/// impls (Noop today, Local in stage 2, a future Remote), and the audit sink has multiple impls
/// (file/stderr/syslog, added by G06). Single-impl domain ports stay concrete/generic and attach
/// later, so they are not fields yet (keeping this facade free of unread state).
struct GovernedState {
    pdp: Box<dyn PolicyDecisionPoint>,
    audit: Arc<dyn AuditSink>,
}
```

Constructors and the accessor:

```rust
impl Governance {
    /// The ungoverned engine: a zero-port facade whose decide path short-circuits to `Allow`.
    /// This is the only facade used in production until the manifest/config tasks land, and it
    /// preserves byte-identical all-open behavior (ADR-0013).
    pub fn all_open() -> Self {
        Self { mode: Mode::AllOpen }
    }

    /// A governed facade over the given decision point and audit sink. Not yet used by any
    /// production path; exercised by the facade unit tests. Later tasks add builder methods to
    /// attach the browser plugin's `DomainPolicy` (classify/match) and `ResourceResolver`.
    pub fn governed(pdp: Box<dyn PolicyDecisionPoint>, audit: Arc<dyn AuditSink>) -> Self {
        Self {
            mode: Mode::Governed(GovernedState { pdp, audit }),
        }
    }

    /// The audit sink held by a governed facade, or `None` under all-open. The audit recorder (G06)
    /// emits one record per call through this; this task only holds it.
    pub fn audit_sink(&self) -> Option<&dyn AuditSink> {
        match &self.mode {
            Mode::AllOpen => None,
            Mode::Governed(state) => Some(state.audit.as_ref()),
        }
    }
}
```

### 3. The per-call entry: `Governance::decide`

The single inbound decision. Under `AllOpen` it is a literal short-circuit that touches no field and
resolves nothing. Under `Governed` it asks the held decision point with a placeholder request (the
real inbound pipeline is filled in by later tasks; the Noop PDP still returns `Allow`).

```rust
impl Governance {
    /// The single inbound governance decision for one tool call, taken at the dispatch chokepoint
    /// before the tool executes.
    ///
    /// Under [`Mode::AllOpen`] this is a literal STEP-0 short-circuit: it returns
    /// [`Decision::Allow`] without touching any port or resolving any resource, so all-open output
    /// is byte-identical to stage 1. Under [`Mode::Governed`] it asks the held decision point; the
    /// real pipeline (classify -> resolve resource -> grant check -> effective mode) is filled in by
    /// G05/G07/G13/G15, and with the Noop decision point the result is still `Allow`.
    pub fn decide(&self, tool: &str) -> Decision {
        match &self.mode {
            Mode::AllOpen => Decision::Allow { grant_id: None },
            Mode::Governed(state) => {
                // Wiring stub. Placeholder request fields: G05 classifies for a real `rw`, the
                // resolver task resolves the governing resource, G12/G13 supply grants, G15 resolves
                // the effective mode. The Noop PDP ignores them and allows.
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
}
```

Notes:

- Keep `decide` SYNC in this task. The `AllOpen` branch is a single constant return; the `Governed`
  branch calls the sync `PolicyDecisionPoint::decide`. No resource is resolved, so nothing async is
  needed. The resolver task widens this signature to `async fn decide(&self, tool: &str, args:
  &Value)` when it adds live resolution; that is that task's churn, not this one's.
- The `Governed` branch reads `state.pdp`; `audit_sink()` reads `state.audit`. Both fields are thus
  live, so no `dead_code` warning under `-D warnings`. Do NOT add `#[allow(dead_code)]`.
- If A2's `DecisionRequest` field set or variant names differ (for example `RwClass::Read` instead of
  `RwClass::Observe`, or a `Grant` that needs a different empty construction), adapt the placeholder
  to compile against A2. The only load-bearing behavior is the `AllOpen` arm.

### 4. Wire the MCP server chokepoint

In the relocated MCP server (`transport/mcp/server.rs` post-A1, else `src/mcp/server.rs`):

- Replace `use crate::dispatch;` with an import of the facade, for example
  `use crate::governance::dispatch::Governance;` (use the real post-A1 path). Add `use std::sync::Arc;`
  if not already present.
- In `run`, construct the facade once, next to the existing config:

  ```rust
  let config = Config::default();
  let governance = Arc::new(Governance::all_open());
  ```

- Thread `&Arc<Governance>` through `handle_line`. Its signature becomes:

  ```rust
  async fn handle_line(
      browser: &Browser,
      config: Config,
      governance: &Arc<Governance>,
      line: &str,
      tx: &mpsc::UnboundedSender<JsonRpcResponse>,
  ) -> Option<JsonRpcResponse>
  ```

  Update the call in `run` to pass `&governance`.

- In the `tools/call` arm, clone the `Arc` into the spawned task alongside `browser` and `tx`:

  ```rust
  "tools/call" => {
      let browser = browser.clone();
      let governance = Arc::clone(governance);
      let tx = tx.clone();
      let params = raw.get("params").cloned();
      tokio::spawn(async move {
          let resp = handle_tools_call(&browser, config, &governance, id, params.as_ref()).await;
          let _ = tx.send(resp);
      });
      None
  }
  ```

- `handle_tools_call` takes the facade by reference:

  ```rust
  async fn handle_tools_call(
      browser: &Browser,
      config: Config,
      governance: &Governance,
      id: Option<Value>,
      params: Option<&Value>,
  ) -> JsonRpcResponse
  ```

- Replace the two no-op seam lines with one facade call, in the same position (after the unknown-tool
  pre-check, before the bounded wait):

  ```rust
  // Inbound governance decision at the single dispatch chokepoint (the PEP). Under all-open
  // (no manifest, default config) this is a literal STEP-0 short-circuit to Allow that queries no
  // port and resolves no resource, so behavior is byte-identical to the ungoverned engine. Acting
  // on a Deny (enforcement) and emitting the audit record attach here in later stage-2 tasks.
  let _decision = governance.decide(name);
  ```

  Keep the leading-underscore binding: this task does not act on the decision (enforcement is G13).
- Change NOTHING else in the server. The bounded first-call wait, `browser.call`, the `read_page`
  redaction call (`policy::redact::apply_to_result(&mut result, config.secrets_redact())`), the wait
  note, the error result shapes, and all JSON-RPC semantics stay exactly as they are. `config` is
  still passed by value (`Config` is `Copy` at this point); do not change its threading.

### 5. Facade unit tests (inline `#[cfg(test)]` in `dispatch.rs`)

A minimal `AuditSink` is needed to construct a governed facade. Define it in the test module (do not
add a production null sink; the real sinks are G06):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::ports::{AuditRecord, NoopPolicyDecisionPoint};

    /// A sink that drops every record. Lets the tests build a governed facade without pulling in
    /// the G06 sinks. Its `record` is never called here; it exists to satisfy the trait.
    struct NullAuditSink;
    impl AuditSink for NullAuditSink {
        fn record(&self, _record: &AuditRecord) {}
    }

    #[test]
    fn all_open_decide_is_allow_with_no_grant_and_no_sink() {
        let g = Governance::all_open();
        assert!(matches!(g.decide("navigate"), Decision::Allow { grant_id: None }));
        assert!(g.audit_sink().is_none());
    }

    #[test]
    fn governed_over_noop_still_allows_and_holds_the_sink() {
        let g = Governance::governed(Box::new(NoopPolicyDecisionPoint), Arc::new(NullAuditSink));
        assert!(matches!(g.decide("navigate"), Decision::Allow { .. }));
        assert!(g.audit_sink().is_some());
    }
}
```

Use A2's real names for the Noop PDP type and the `AuditRecord` type. Use `matches!` rather than
`assert_eq!` so the tests do not require `Decision`/`Denial` to implement `PartialEq`. These two
tests exercise both facade shapes and read every held field, keeping the tree green under
`-D warnings`.

### 6. Extend the all-open golden test (byte-identical proof)

The goal is to prove the facade path did not change any output. Three assertions:

1. **`tools/list` bytes and a dispatch round-trip.** These already exist in
   `tests/mcp_protocol.rs::initialize_tools_list_and_tool_call_over_stdio` (exact fixture equality
   plus the exact `[hop: extension] Browser extension not connected...` message) and in the late-
   extension test. Because the facade now sits on that path, keeping those tests passing UNCHANGED is
   the byte-identical guard. Do not modify `tests/mcp_protocol.rs`. If A1 added
   `tests/all_open_golden.rs`, extend it with an equivalent `tools/list` byte-equality assertion and
   a no-extension `tools/call` round-trip through the freshly built binary; if A1 left only a stub,
   fill it in.
2. **The facade decides `Allow` under all-open.** Covered by the part-5 unit test.
3. **`read_page` redaction still governed by `content.security.secrets.redact`.** Add ONE integration
   test (in `tests/all_open_golden.rs`, or a new `tests/read_page_redaction.rs`) that drives the
   binary with a fake extension over the real IPC (mirror the fake-extension pattern in
   `tests/mcp_protocol.rs::tools_call_waits_for_a_late_extension_and_notes_the_wait`), has the fake
   extension answer a `read_page` call with a result whose text carries the engine's secret marker,
   and asserts the client-visible text is redacted and the marker is gone. Concretely, the fake
   extension replies:

   ```rust
   let reply = json!({
       "id": v["id"],
       "type": "tool_response",
       "result": { "content": [ {
           "type": "text",
           "text": "textbox \"Password\" [ref_3] secret_value=\"hunter2\" type=\"password\""
       } ] },
   });
   ```

   and the test asserts the received `read_page` result text contains `value="[value redacted]"`,
   does NOT contain `secret_value=`, and does NOT contain `hunter2` (the safe default keeps redaction
   on). This proves the redaction overlay is still wired at the chokepoint after the facade change.
   The marker constant (`secret_value="`) and the replacement (`value="[value redacted]"`) come from
   `src/policy/redact.rs`; read it to keep the strings exact.

Do not add or change the tool schemas or `tests/tool_schema_fidelity.rs`.

## Constraints

1. ASCII only in all code and docs: no em-dashes, no arrows, no curly quotes, anywhere (comments,
   tests, strings). Use Rust `\u{..}` escapes if a test needs a non-ASCII input.
2. All-open stays first-class and byte-identical: with no manifest and default config, every tool
   result is exactly what stage 1 produced. `Governance::all_open().decide(...)` is a literal
   STEP-0 short-circuit that queries no port and resolves no resource. The unchanged
   `tests/mcp_protocol.rs` and `tests/tool_schema_fidelity.rs` are the guard.
3. NEVER modify the tool schemas (`src/mcp/schemas/tools.json`), tool names, params, or
   descriptions; `tests/tool_schema_fidelity.rs` must pass unchanged (ADR-0007, the sacred surface).
4. The extension holds mechanism only; no policy, access, or redaction decisions in extension JS.
   This task touches no extension file.
5. Rust 2021, `thiserror` for typed errors, doc comments on all public items and modules, `rustfmt`
   clean, `cargo clippy --all-targets -- -D warnings` clean. No `#[allow(dead_code)]`; the facade
   fields are kept live by `decide` and `audit_sink`.
6. One task = one commit (code + tests + ledger/browser-test updates). Keep the tree green between
   tasks (full suite + clippy + fmt).
7. Windows dev gotcha: if `target/debug/browser-mcp.exe` is locked by a running session, rename it
   aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and rebuild, or stop
   the MCP client first.

Task-specific:

8. Consume A2's ports; do not define parallel decision/audit types. The facade uses A2's `Decision`,
   `PolicyDecisionPoint`, `AuditSink`, `AuditRecord`, `DecisionRequest`, `GoverningResource`,
   `RwClass`, `EffectiveMode`, and the Noop PDP. Match A2's exact names; adapt the placeholder
   `DecisionRequest` construction to whatever A2 shipped.
9. `governance/**` must not name `browser`, `transport`, `mcp`, `native`, or the `url` crate (the A7
   arch-test). The facade holds domain ports only behind the core traits (`dyn`), never a concrete
   browser type. That is why `DomainPolicy` / `ResourceResolver` are NOT fields yet.
10. The decision is unused this task (`let _decision = ...`). Do not add a `Deny` branch, do not
    block any call, do not emit any audit record, do not classify, do not resolve a resource, do not
    read any config key other than the untouched `secrets_redact()` redaction call already present.
11. Reconciliation: several later G-docs (G05, G06, G07, G08, G13, G15) were written before this
    facade existed and still reference `dispatch::policy_check` / `dispatch::audit` /
    `PolicyDecision`. After this task those no longer exist. Those tasks attach their logic inside
    `Governance::decide` and the `PolicyDecisionPoint` / `AuditSink` impls instead. Do not
    reintroduce the removed free functions to satisfy an out-of-date reference.
12. Do not change `Config` (still `Copy`, single `secrets_redact` field here). G01/A4 makes it owned
    later; that is not this task.

## Verification

1. `cargo fmt` then `cargo clippy --all-targets -- -D warnings` from the repo root: clean.
2. `cargo test` from the repo root: all pass, including the two new facade unit tests, the new
   `read_page` redaction integration test, `tests/mcp_protocol.rs` UNCHANGED (byte-identical
   `tools/list` and the exact no-extension message), `tests/tool_schema_fidelity.rs` unchanged, and
   the extended all-open golden test.
3. Grep checks: `policy_check`, `PolicyDecision`, and the old `dispatch::audit` free function no
   longer appear anywhere in `src/` (the only `Governance`/`decide`/`audit_sink` references are the
   facade and its one call site). The old two-line seam is gone from the server.
4. If `target/debug/browser-mcp.exe` is locked, rename it aside and rebuild (see constraint 7).
5. Manual check (binary-only change; restart the MCP client to pick up the new binary; no extension
   reload needed): a normal session behaves exactly as before. `tools/list` shows all 13 tools;
   a tool call with Chrome closed still fails after about 5 seconds with the T04 timeout message;
   `read_page` on a page with a password field still shows `[value redacted]` (safe default keeps
   redaction on).

## Out of scope

- Real enforcement: acting on a `Deny`, blocking a call, denial responses, the stable denial id
  (G08, G13). `decide`'s result is bound to `_decision` and ignored.
- Audit records and sinks (G06): no `AuditRecord` is constructed or emitted; the held `AuditSink` is
  only stored (and a null test sink is used to construct a governed facade). No file/stderr/syslog
  sink.
- Read/write classification (G05): the `Governed` stub uses a placeholder `RwClass`; no
  `tool + action -> observe|mutate` table.
- Resource resolution (the resolver task): no `ResourceResolver`, no host/URL parsing, no `args`
  inspection; `decide` takes no `args` and resolves `GoverningResource::None`.
- Domain matching and sacred domains (G07/G08): the `url` crate stays out of `governance/`; no
  matcher, no sacred list read.
- The manifest engine and grant model (G12/G13): no manifest parse, no `Grant` population beyond an
  empty vec, no `--manifest`/env source selection.
- Shadow mode and effective-mode resolution (G15): the `Governed` stub uses a placeholder
  `EffectiveMode`.
- The config registry growth and layered resolution (G01/G02/A4): `Config` is untouched and stays
  `Copy` with its single field. No new config key is read.
- Hot-reload, `notifications/tools/list_changed`, the control-plane listener, and the persistent
  service split: all later.
- Moving the `read_page` redaction into an outbound PEP: it stays exactly where it is in the server
  for this task.
