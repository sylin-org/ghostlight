# H0: Extract the HubCore composition root

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 1: four
> roles, one binary; Decision 2: HubCore / ServiceContext vs per-session state). One task = one
> commit. Facts below are as-of-authoring 2026-07-04 -- RE-READ the named files before relying on
> any line number.

## Goal

Lift the mcp-server composition root out of `main::run_server` into a new free-licensed `src/hub`
module, verbatim. This is a PURE CODE MOVE: Browser handle creation, the `ipc::serve` spawn, the
parent-death watchdog wiring, `sweep_orphans`, and the tokio runtime block move into
`hub::run_mcp_server`; `mcp::server::run` is still called exactly as today. NO role change, NO
behavior change, NO wire or audit byte change, single stdio session only. Per ADR-0030 Decision 2
("Extract the composition root into a free-licensed `src/hub` module hosting `HubCore`"), this only
creates the seam that H1/H2 later attach `ServiceContext` and multiplex to. H0 itself adds no new
capability.

## Authority

1. docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 1, Decision 2) -- NORMATIVE. Cite by name;
   do not restate its semantics.
2. BOOTSTRAP.md ground rules.
3. This task file.

If they conflict, the higher wins.

## Current-tree facts (as-of-authoring; RE-READ before relying)

### `src/main.rs`

- `fn run_server(manifest: Option<String>, debug_on: bool) -> Result<()>` begins at approximately
  line 442 and its body runs through `std::process::exit(code)` at approximately line 546. The whole
  body is the composition root to move. In order it does:
  1. Resolve `user_source` from `manifest` or `GHOSTLIGHT_MANIFEST`, then
     `source::load_policy(user_source.as_deref(), pattern::is_valid_pattern)` with
     `.with_context(|| "loading the governance manifest")` (approx lines 448-450).
  2. Startup tracing::info! for the governance-active vs all-open cases (approx lines 452-466).
  3. `let parent = ghostlight::proc::parent();` (approx line 471).
  4. `ghostlight::doctor::sweep_orphans();` (approx line 478).
  5. `let sink = build_debug_sink(debug_on, "mcp-server");` (approx line 480).
  6. `let rt = tokio::runtime::Runtime::new()?;` (approx line 481).
  7. The `rt.block_on(async move { ... })` block (approx lines 487-538): `Browser::with_debug`, the
     parent-death watchdog `tokio::spawn`, the `ipc::serve` `tokio::spawn`, and the
     `tokio::select!` over `ghostlight::mcp::server::run(browser, loaded_policy, user_source)` vs
     `shutdown.notified()`, yielding `code`.
  8. `sink.flush(); std::process::exit(code)` (approx lines 545-546).

- `fn build_debug_sink(debug: bool, role: &'static str) -> DebugSink` is a private helper at approx
  lines 552-570. It has TWO callers: `run_server` (moving) and `run_native_host_role` (approx line
  423, which stays in `main.rs`). Because it has a caller that stays, moving it forces a call-site
  update in `run_native_host_role`.

- The `command: None` match arm (approx lines 393-397) calls `run_server(manifest, debug_flag ||
  debug_env)?`.

- Top-of-file `use` statements and their ONLY relevant usage (this is the coupling that pins scope;
  clippy runs with `-D warnings`, so every import left dangling after the move is a build failure):
  - `use anyhow::{Context, Result};` (approx line 20) -- `Context` is used ONLY at the `.with_context`
    in `run_server` (moving). After the move `Context` is unused in `main.rs`: narrow this to
    `use anyhow::Result;`.
  - `use ghostlight::debug::DebugSink;` (approx line 23) -- used ONLY by `build_debug_sink` (moving).
    REMOVE from `main.rs`.
  - `use ghostlight::governance::manifest::source;` (approx line 25) -- used ONLY at the
    `source::load_policy` in `run_server` (moving). REMOVE from `main.rs`.
  - `use ghostlight::native::ipc;` (approx line 27) -- used at `ipc::relay_native_host` in
    `run_native_host_role` (approx line 426, STAYS) AND at `ipc::default_endpoint` / `ipc::serve` in
    `run_server` (moving). KEEP in `main.rs` (the native-host role still needs it).
  - `use ghostlight::transport::executor::Browser;` (approx line 28) -- used ONLY at
    `Browser::with_debug` in `run_server` (moving). REMOVE from `main.rs`.
  - `use ghostlight::browser::pattern;` (approx line 22) -- used at `pattern::is_valid_pattern` in
    `run_server` (approx line 449, moving); the other three uses (approx lines 340, 351, 366) are
    FULLY-QUALIFIED `ghostlight::browser::pattern::is_valid_pattern` and do NOT depend on this
    `use`. After the move the `use` is unused: REMOVE `use ghostlight::browser::pattern;` from
    `main.rs`. (Do NOT touch the fully-qualified call sites.)

### `src/lib.rs`

- Module declarations are an alphabetized block (approx lines 16-24): `browser`, `debug`, `doctor`,
  `error`, `governance`, `install`, `origin`, `proc`, `transport`. There is NO `hub` module today.

### Referenced symbols (confirm they still resolve; do not change them)

- `ghostlight::native::ipc::serve(browser, &endpoint)` and `ipc::default_endpoint()` exist in
  `src/transport/native/ipc.rs`.
- `ghostlight::mcp::server::run(browser: Browser, loaded_policy: LoadedPolicy, user_source:
  Option<String>)` exists in `src/transport/mcp/server.rs` (approx line 108). Called EXACTLY as today.
- `ghostlight::doctor::sweep_orphans()`, `ghostlight::proc::parent()`,
  `ghostlight::transport::watchdog::wait_until_orphaned(parent)` are used verbatim inside the moved
  block.

## Required behavior

Mandated by ADR-0030 Decision 2 (free-licensed `src/hub` module hosting `HubCore`) and Decision 1
(the mcp-server role is one of the four roles of the one binary). The move is byte-for-byte at the
statement level; the only permitted edits are the mechanical relocation, the import re-homing above,
and the two call-site updates below.

1. Create `src/hub/mod.rs` with a module-level doc comment stating its role (the free-licensed
   composition root / seam that H1/H2 attach `ServiceContext` + multiplex to; cite ADR-0030
   Decision 2) and the SPDX header used by the rest of the free engine:
   `// SPDX-License-Identifier: Apache-2.0 OR MIT`.

2. In `src/hub/mod.rs` define, with the SAME signature as today's `run_server`:

   ```
   pub fn run_mcp_server(manifest: Option<String>, debug_on: bool) -> anyhow::Result<()>
   ```

   Its body is the CURRENT `run_server` body, moved verbatim (all eight steps listed above, ending
   with `sink.flush(); std::process::exit(code)`). Do not reorder, rename, or reword anything inside
   -- including the two `tracing::info!` startup messages, the `SessionBusy` warn text, and all
   comments. Add to `src/hub/mod.rs` the imports the moved code needs (at minimum
   `anyhow::{Context, Result}`, `ghostlight::debug::DebugSink` -> use the in-crate path
   `crate::debug::DebugSink`, `crate::governance::manifest::source`, `crate::native::ipc`,
   `crate::transport::executor::Browser`, `crate::browser::pattern`). Prefer in-crate `crate::`
   paths since `src/hub` is inside the library crate. The MCP call must stay
   `crate::mcp::server::run(browser, loaded_policy, user_source)` (equivalently
   `ghostlight::mcp::server::run` is the same item via the lib.rs facade; keep whichever path
   compiles cleanly, byte-identical arguments in the byte-identical order).

3. Move `build_debug_sink` into `src/hub/mod.rs` as:

   ```
   pub fn build_debug_sink(debug: bool, role: &'static str) -> DebugSink
   ```

   verbatim body. It becomes `pub` because `main.rs`'s native-host role calls it across the crate
   boundary.

4. In `src/lib.rs`, add `pub mod hub;` in the alphabetized module block (between `governance` and
   `install` is where `hub` sorts -- place it there to match the file's ordering, or wherever the
   existing alphabetization dictates; a doc comment is optional but keep the file's style).

5. In `src/main.rs`:
   - Change the `command: None` arm to call `ghostlight::hub::run_mcp_server(manifest, debug_flag ||
     debug_env)?` (same arguments as the old `run_server(...)`).
   - Change `run_native_host_role`'s `build_debug_sink(debug, "native-host")` call to
     `ghostlight::hub::build_debug_sink(debug, "native-host")`.
   - DELETE the old `run_server` function and the old `build_debug_sink` function from `main.rs`.
   - Apply the import re-homing from "Current-tree facts": narrow `use anyhow::{Context, Result};`
     to `use anyhow::Result;`; REMOVE `use ghostlight::debug::DebugSink;`,
     `use ghostlight::governance::manifest::source;`,
     `use ghostlight::transport::executor::Browser;`, and `use ghostlight::browser::pattern;`; KEEP
     `use ghostlight::native::ipc;`.

MUST stay byte-identical (no edit of any kind this task):
- `src/transport/mcp/tools.rs` (TOOLS_JSON).
- The MCP JSON-RPC wire and the `notifications/tools/list_changed` line in
  `src/transport/mcp/server.rs`.
- `src/transport/native/host.rs` framing (4-byte LE prefix, `MAX_MESSAGE_LEN`,
  `encode`/`read_message`).
- `Browser::attach` single-extension-link rejection (`AttachOutcome::AlreadyAttached`).
- Every statement, string literal, and comment inside the moved `run_server` body and moved
  `build_debug_sink` body -- only their LOCATION changes.

## Tests (BY NAME; assertions pinned)

- Keep green (do NOT modify): `tests/all_open_golden.rs`, `tests/tool_schema_fidelity.rs`,
  `tests/mcp_protocol.rs`, `tests/peer_death.rs`,
  `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`.
- Add: NONE. This is a pure refactor; correctness is proven by the existing suite staying green
  untouched. In particular the following ADR-0030 "Preserved invariants" oracle is what the
  UNCHANGED `tests/all_open_golden.rs` continues to assert, transcribed here verbatim so the
  executor confirms the invariant this move must preserve (do NOT add a new test for it):

  > All-open byte-identity: a lone all-open session's output stays byte-identical through H0-H8
  > (`tests/all_open_golden.rs`); every new session/isolation path is a no-op for a lone all-open
  > session.

  If `tests/all_open_golden.rs` does not stay green after the move, the move was not byte-identical;
  fix the move, never the test.

## Verification (literal commands)

```
cargo build --all-targets
cargo test --test all_open_golden --test tool_schema_fidelity --test mcp_protocol --test peer_death
cargo test --test architecture governance_core_has_no_forbidden_back_edges
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## STOP preconditions

- If `main::run_server` no longer contains the `Browser` creation + `ipc::serve` spawn +
  `mcp::server::run` composition (i.e. it was already refactored), STOP and re-read the tree; do not
  invent a new seam.
- If `src/hub` already exists with a service module, STOP and reconcile with what is there; do NOT
  duplicate a composition root.
- If `build_debug_sink` no longer has two callers (native-host + server), STOP and re-read; the
  call-site update in step 5 assumes both callers exist.
- If applying this task would require moving any never-touch fence below, STOP.
- General standing order: the line numbers and signatures above are as-of-authoring 2026-07-04.
  RE-READ each named file before relying on any of them. If a STOP precondition's assumption is
  absent, STOP -- do not improvise around a broken assumption.

## NEVER touch (this task)

- `src/transport/mcp/tools.rs` (TOOLS_JSON: the 13 trained schemas + `explain`), byte-frozen. No
  exception.
- `tests/tool_schema_fidelity.rs`. No exception; keep green untouched.
- `tests/all_open_golden.rs` and the all-open byte-identity invariant. No exception; this move must
  be a no-op for a lone all-open session.
- Any test file (no test is added or edited this task).
- `tests/architecture.rs` `governance_core_has_no_forbidden_back_edges`: `src/governance/**` names
  no browser/transport/mcp/native/url and no tabId/token/socket type. NO sanctioned exception in
  H0: the new code lands in `src/hub`, NOT in `src/governance`.
- `src/governance/**`: do not add, move, or edit anything here. NO sanctioned exception in H0.
- `src/transport/native/host.rs` framing (4-byte LE prefix, `MAX_MESSAGE_LEN`,
  `encode`/`read_message`). No exception this task.
- The MCP JSON-RPC wire + the pinned `notifications/tools/list_changed` line in
  `src/transport/mcp/server.rs`. The adapter is a byte relay, never a rewriter. No exception.
- `Browser::attach` single-extension-link rejection (`AttachOutcome::AlreadyAttached`). Retained. NO
  H0 exception (the kill-hook fan-out is an H2-only sanctioned change, not this task).
