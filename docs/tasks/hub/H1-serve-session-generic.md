# H1: Transport-generic serve_session + ServiceContext

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 2;
> "Preserved invariants (and the pinned oracles the batch transcribes)"). One task = one commit.
> Facts below are as-of-authoring 2026-07-04 -- RE-READ the named files before relying on any line
> number.

## Goal

Refactor the MCP server loop from a hardcoded-stdio `run` into a transport-generic
`serve_session<S>(stream, ctx)` over a `S: AsyncRead + AsyncWrite + Send + 'static` stream, taking a
`ServiceContext` that owns the SHARED `Browser` + `ConfigStore` + audit `Recorder`. Per-session state
(the swappable `Governance` + client identity, the writer task, the policy-subscription task) is
built PER `serve_session` invocation. This is the byte-identical single-session prerequisite for the
genuine multiplex H2 lands. Why: ADR-0030 Decision 2 ("HubCore / ServiceContext vs per-session
state") mandates ONE `serve_session` / `handle_tools_call` that EVERY transport calls, split into
shared-per-service state and per-session state. This task lands ONLY the split and the generic
signature; it changes NO observable behavior.

## Authority

1. docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 2; "Preserved invariants") -- NORMATIVE; cited, never restated.
2. BOOTSTRAP.md ground rules.
3. This task file.
   Higher wins on conflict.

## Current-tree facts (as-of-authoring; RE-READ before relying)

### `src/hub` does not exist yet -- H0 creates it (STOP gate below)

As-of-authoring `src/hub` is ABSENT (`src/lib.rs` `pub mod` list has no `hub`; `grep hub::` finds
only docs). Per the ADR "Migration" list, H0 ("Extract HubCore composition root into src/hub") lands
IMMEDIATELY before this task and creates `src/hub/mod.rs` hosting `HubCore`. H1 BUILDS ON H0: it adds
`ServiceContext` into `src/hub`. If H0 has not landed, STOP (see STOP preconditions).

### `src/transport/mcp/server.rs` -- the loop to refactor

- `pub async fn run(browser: Browser, loaded_policy: LoadedPolicy, user_source: Option<String>) -> Result<()>`
  starts near line 108. It is the ONLY caller-facing entry today.
- Line ~122: `let mut lines = BufReader::new(tokio::io::stdin()).lines();` -- the hardcoded read side.
- Lines ~133-138: `ConfigStore::load_initial_with_policy(...)` + `store.clone().spawn_watcher()` --
  SHARED-lifetime store setup.
- Lines ~144-154: `let recorder = Arc::new(Recorder::from_config(&store.current()));` + the spawned
  recorder-reload subscription (`store.subscribe()` loop calling `recorder.reload`) -- SHARED-lifetime
  recorder setup.
- Lines ~160-169: `build_governance(...)` + `record_user_manifest_ignored()` + `governance_slot:
  Arc<Mutex<Arc<Governance>>>` -- PER-SESSION governance.
- Lines ~178-184: `browser.on_session_killed({...})` -- PER-SESSION kill hook.
- Lines ~186-220: `mpsc::unbounded_channel::<Outbound>()` + `let debug = browser.debug().clone();` +
  the writer task, which does `let mut stdout = tokio::io::stdout();` at line ~194 -- PER-SESSION
  writer that owns the write side.
- Lines ~235-281: the `policy_subscription` task -- PER-SESSION.
- Lines ~283-300: the `while let Some(line) = lines.next_line().await?` read loop, then the ordered
  teardown (`policy_subscription.abort(); ... drop(tx); let _ = writer.await;`).
- Helpers to KEEP in `server.rs` and reuse from `serve_session`: `Outbound` (line ~58),
  `TOOLS_LIST_CHANGED_LINE` (line ~48), `build_governance` (line ~67), `manifest_identity_of`
  (line ~83), `current_governance` (line ~94), `handle_line` (line ~313), `tools_list_result`,
  `initialize_result`, `capture_client_info`.
- Imports today: `use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};` (add `AsyncRead,
  AsyncWrite`).

### `src/main.rs` -- the mcp-server call site (DO NOT TOUCH)

- Line ~527: `result = ghostlight::mcp::server::run(browser, loaded_policy, user_source) => { ... }`
  inside a `tokio::select!`. `run`'s signature stays byte-identical, so main.rs is NOT edited.

### `src/transport/mcp/pipeline.rs` -- the shared chokepoint (signature frozen)

- Line ~50: `pub(crate) async fn handle_tools_call(browser: &Browser, store: &Arc<ConfigStore>,
  governance: &Governance, id: Option<Value>, params: Option<&Value>) -> JsonRpcResponse`. It already
  takes and BORROWS `Browser` / `ConfigStore` / `Governance`. Its signature is UNCHANGED by H1. The
  only permitted edit is a doc-comment pointer noting it is the transport-generic chokepoint; if no
  doc edit is needed, leave the file untouched.

### Coupling that pins scope

`run` consumes `loaded_policy` in BOTH the shared store setup (`load_initial_with_policy` reads it +
`user_source`) AND the per-session governance seed (`build_governance` + `record_user_manifest_ignored`
read it). `ServiceContext` must therefore carry the initial `LoadedPolicy` forward so `serve_session`
can seed the first `Governance` from it. `LoadedPolicy` is `Clone` (pipeline/server already clone it
via `policy_changes.borrow_and_update().clone()`).

## Required behavior

Cite: ADR-0030 Decision 2 mandates the shared/per-session split and the single transport-generic
`serve_session`. The all-open byte-identity invariant ("Preserved invariants") pins the single-session
output as unchanged.

### 1. `ServiceContext` in `src/hub`

Add to `src/hub/mod.rs` (Decision 2: session/composition state lives in `src/hub`, never
`src/governance`; the a7 arch-test allows `src/hub` to name `browser`/transport types):

```
pub struct ServiceContext {
    pub browser: Browser,
    pub store: Arc<ConfigStore>,
    pub recorder: Arc<Recorder>,
    pub initial_policy: LoadedPolicy,
}
```

Give it a constructor that performs the SHARED-lifetime setup exactly as `run` does today, in the
same order (store load -> spawn_watcher -> recorder build -> recorder-reload subscription spawn):

```
impl ServiceContext {
    pub fn from_startup(
        browser: Browser,
        loaded_policy: LoadedPolicy,
        user_source: Option<String>,
    ) -> Result<Self>
}
```

`from_startup` moves server.rs lines ~133-154 verbatim (the `ConfigStore::load_initial_with_policy`
call, `store.clone().spawn_watcher()`, `Recorder::from_config`, and the `store.subscribe()`
reload-subscription `tokio::spawn`), stores `browser`, `store` (as `Arc<ConfigStore>`), `recorder`
(as `Arc<Recorder>`, the CONCRETE type so `serve_session` can still cast it to `Arc<dyn AuditSink>`
and the reload path can reach it), and keeps `loaded_policy.clone()` as `initial_policy`. It is a
plain (non-async) fn that calls `tokio::spawn` internally, same as `run` does today (it is invoked
from within the runtime by `run`). `Result` is `crate::Result`. Keep the existing tracing::debug!
"active manifest held" note (server.rs lines ~113-120) either in `from_startup` or at the top of the
new `run`; its position relative to the store load is immaterial to observable output but keep it
before the read loop.

### 2. `serve_session<S>` in `src/transport/mcp/server.rs`

Replace the body of `run` with a transport-generic session function:

```
pub async fn serve_session<S>(stream: S, ctx: ServiceContext) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + 'static,
```

Body:

- Destructure `ctx` into `browser`, `store`, `recorder`, `initial_policy`.
- `let (read_half, write_half) = tokio::io::split(stream);`
- `let mut lines = BufReader::new(read_half).lines();` (replaces the `tokio::io::stdin()` reader).
- PER-SESSION setup, moved verbatim from `run` lines ~160-300, in the SAME order:
  - `build_governance(&initial_policy, recorder.clone() as Arc<dyn AuditSink>)`, the
    `record_user_manifest_ignored()` guard, and `governance_slot`.
  - `browser.on_session_killed({...})` (unchanged closure).
  - `let (tx, mut rx) = mpsc::unbounded_channel::<Outbound>();`, `let debug = browser.debug().clone();`,
    and the writer task -- BUT the writer now owns `write_half` instead of `tokio::io::stdout()`:
    `let mut out = write_half;` in place of `let mut stdout = tokio::io::stdout();`, and the
    `out.write_all(...).await` / `out.flush().await` calls take the place of the `stdout.*` calls.
    Everything else in the writer (the `Outbound::Response` / `Outbound::ToolsListChanged` match, the
    `debug.mcp_response` branch, the `buf.push('\n')`, the break-on-error) stays byte-identical.
  - The `policy_subscription` task (lines ~235-281) unchanged.
  - The read loop (lines ~283-292) and the ordered teardown (lines ~293-300:
    `policy_subscription.abort(); let _ = policy_subscription.await; drop(tx); let _ = writer.await;
    Ok(())`) unchanged.

`write_half` is `tokio::io::WriteHalf<S>`, which is `Send + 'static` when `S: Send + 'static`, so the
writer `tokio::spawn` still compiles. Add `AsyncRead, AsyncWrite` to the existing
`use tokio::io::{...}` import.

### 3. `run` becomes the thin mcp-server wrapper (signature FROZEN)

Keep `pub async fn run(browser: Browser, loaded_policy: LoadedPolicy, user_source: Option<String>) ->
Result<()>` byte-identical in signature so `src/main.rs` is NOT edited. Its new body:

```
let ctx = crate::hub::ServiceContext::from_startup(browser, loaded_policy, user_source)?;
let stream = tokio::io::join(tokio::io::stdin(), tokio::io::stdout());
serve_session(stream, ctx).await
```

`tokio::io::join(stdin, stdout)` produces a single `S: AsyncRead + AsyncWrite`; `serve_session`
splits it back into the same underlying stdin/stdout handles, so reads still come from stdin and
writes still go to stdout -- BYTE-IDENTICAL to the pre-refactor single-session path.

### Must stay byte-identical

- Every stdout line `serve_session` emits under a lone all-open session (the initialize reply, every
  tools/list and tools/call reply, the `notifications/tools/list_changed` line) -- tests/all_open_golden.rs
  and tests/mcp_protocol.rs pin this.
- The MCP JSON-RPC wire and the pinned `TOOLS_LIST_CHANGED_LINE` in server.rs (line ~48):
  `{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}` -- NOT edited.
- The audit output (the shared `Recorder` is merely hoisted into `ServiceContext`, built once with
  the same inputs in the same order) -- tests/audit_recorder.rs and the pipeline inline audit test
  pin this.
- `pipeline::handle_tools_call`'s signature.

## Preserved-invariant oracles transcribed (from ADR "Preserved invariants")

H1 hoists the `Recorder` into `ServiceContext`, so the audit output must stay byte-identical. The
guarding oracles, transcribed verbatim from docs/adr/0030-ghostlight-hub-orchestrator.md
"Preserved invariants (and the pinned oracles the batch transcribes)" -- asserted by the KEPT-GREEN
tests below, not by any new H1 test:

- Audit record field order, exactly 14 keys, in order:
  `event_id, ts, identity, client, tool, action, capability, domain, decision, grant_id, denial_id,
  duration_ms, manifest, held` (guarded by tests/audit_recorder.rs).
- Session-event record field order, exactly 6 keys, in order:
  `event_id, ts, identity, client, event, manifest` (guarded by tests/audit_recorder.rs).

All-open byte-identity ("a lone all-open session's output stays byte-identical through H0-H8;
tests/all_open_golden.rs") is the invariant this whole task defends.

## Tests (BY NAME; assertions pinned)

### Keep green (do not modify)

- tests/all_open_golden.rs (all cases: `tools_list_is_byte_stable_through_the_move`,
  `facade_decide_is_all_open_after_the_move`, `read_page_redaction_is_still_wired_at_the_chokepoint`).
- tests/hot_reload.rs
- tests/audit_recorder.rs
- tests/mcp_protocol.rs
- src/transport/mcp/pipeline.rs::tools_call_produces_one_audit_record_with_client_identity
- src/transport/mcp/server.rs::advertised_set_diff_gates_the_notification (the existing inline test).

### Add

None required. This is a byte-identical refactor; the kept-green suites above (especially
tests/all_open_golden.rs and tests/mcp_protocol.rs, which drive the real binary over stdio) already
prove `serve_session` over the stdin/stdout join is unchanged.

OPTIONAL seam test (add ONLY if it is mechanical to wire; SKIP it rather than improvise -- it is not
required for the commit to be complete). If added, place it inline in `src/transport/mcp/server.rs`
`#[cfg(test)]` as:

- Name: `serve_session_over_duplex_matches_stdio_initialize_reply`
- Shape: build a `ServiceContext` from an all-open `LoadedPolicy` (no manifest), a fresh
  `Browser::new()`, and a test `ConfigStore`/`Recorder` (reuse the exact construction the pipeline
  inline tests use). Create `let (client, server) = tokio::io::duplex(64 * 1024);` write the line
  `{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}` + `\n` into `client`, spawn
  `serve_session(server, ctx)`, read one response line back from `client`, drop `client` to end the
  loop.
- Pinned assertion (oracle transcribed VERBATIM from tests/all_open_golden.rs
  `read_page_redaction_is_still_wired_at_the_chokepoint`):
  `assert_eq!(first["id"], 1, "first response is the initialize reply");`
  plus `assert_eq!(first["result"]["protocolVersion"], "2024-11-05");` (transcribed from
  `PROTOCOL_VERSION` in server.rs) and
  `assert_eq!(first["result"]["serverInfo"]["name"], "ghostlight");` (transcribed from
  `initialize_result`).

## Verification (literal commands)

```
cargo build --all-targets
cargo test --lib
cargo test --test all_open_golden --test hot_reload --test audit_recorder --test mcp_protocol
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

All must pass with zero modifications to the kept-green tests.

## STOP preconditions

- If `mcp::server::run` (or a `serve_session`) already accepts a generic
  `S: AsyncRead + AsyncWrite` stream instead of hardcoded stdin/stdout, STOP -- H1 is already done.
- If `src/hub/mod.rs` does not exist or does not host `HubCore`, STOP -- H0 has not landed and H1
  builds directly on it.
- If hoisting the `Recorder` / `ConfigStore` to `ServiceContext` (shared) scope would change the
  all-open audit bytes or any all-open stdout line (i.e. tests/audit_recorder.rs, tests/all_open_golden.rs,
  or tests/mcp_protocol.rs would need editing to pass), STOP and keep the single-session path
  byte-identical -- the correct outcome is that those suites pass UNTOUCHED.
- If `pipeline::handle_tools_call`'s signature would have to change, STOP -- Decision 2 freezes it.
- If any never-touch fence below would have to move, STOP.

## NEVER touch (this task)

- src/transport/mcp/tools.rs (TOOLS_JSON: the 13 trained schemas + `explain`), byte-frozen. No exception.
- tests/tool_schema_fidelity.rs. No exception; keep green untouched.
- tests/all_open_golden.rs and the all-open byte-identity invariant. No exception; `serve_session`
  and `ServiceContext` must be a no-op for a lone all-open session.
- The MCP JSON-RPC wire and the pinned `notifications/tools/list_changed` line
  (`TOOLS_LIST_CHANGED_LINE` in server.rs). The loop is a byte relay, never a rewriter.
- The audit 14-key / session-event 6-key field orders (transcribed above).
- src/governance/** -- no session/multiplex/composition code lands here; the a7 arch-test
  (tests/architecture.rs `governance_core_has_no_forbidden_back_edges`) must stay green.
  `ServiceContext` lands in `src/hub`, which MAY name `browser`/transport types. No exception for H1.
- src/transport/native/host.rs framing (4-byte LE prefix, MAX_MESSAGE_LEN, encode/read_message).
  No exception this task.
- Browser::attach single-EXTENSION-link rejection (`AttachOutcome::AlreadyAttached`). Retained;
  no exception in H1 (the kill-hook fan-out is H2, not this task).
- src/main.rs -- NOT edited; `run`'s signature stays byte-identical so the call site at line ~527 is
  unchanged. No exception.
