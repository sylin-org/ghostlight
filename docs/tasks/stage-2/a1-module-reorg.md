# A1: Module reorg into governance/ browser/ transport/

## Goal

Regroup the flat `src/` module tree into three domain buckets that the whole stage-2
governance layer will be built into, plus a shared-infra crate root. This is a PURE MOVE
plus import-path updates: NO behavior change, NO new types, NO logic change.

- `governance/` -- the domain-agnostic policy core. Receives `src/dispatch.rs` and
  `src/policy/` (minus `redact.rs`).
- `browser/` -- the browser-domain plugin. Receives `src/tools/` and
  `src/policy/redact.rs` (page-content redaction is a browser-domain concern, not a
  core concern).
- `transport/` -- the I/O and protocol infra. Receives `src/native/`, `src/mcp/`, and
  `src/browser.rs` (the `Browser` executor handle, moved and renamed to
  `transport/executor.rs` so it does not collide with the new `browser/` plugin module).
- The crate root KEEPS the shared infra that belongs to no single bucket: `install/`,
  `debug.rs`, `doctor.rs`, `origin.rs`, `error.rs`, and the `main.rs` binary. Do NOT force
  these into the three buckets. The point of this task is to ISOLATE the domain-agnostic
  `governance/` core, not to tidy everything into three folders.

The deliverable is ONE reviewable commit that relocates files, rewrites every `use` path,
and proves the move changed nothing observable via a new all-open golden test. All-open
stays byte-identical: with no manifest and default config every tool result is exactly
what stage 1 produced.

## Depends on

- Nothing. This is the FIRST stage-2 task. Do it before any governance code is written,
  while the tree is clean of stage-2 additions, so the reorg is a cheap mechanical move
  and not a later churn-heavy refactor (PLAN.md "Lay the seams now").
- Reading, for context (do not edit): `docs/tasks/stage-2/PLAN.md` (Phase A, item A1) and
  `docs/design/ghostlight-service-architecture.md` section 3 (bounded contexts) and
  section 4 (the seam sketches). The three-bucket split here is exactly the bounded-context
  table's core / domain / infra rows. A2 (ports) is the NEXT task and adds the trait
  contracts; A1 only moves files so A2 has clean homes to land in.

## Project context

Browser MCP is governed browser automation over the user's own authenticated Chromium
session. A single Rust binary is both the MCP server (JSON-RPC 2.0 over stdio, hand-rolled
on tokio, no MCP SDK crate) and the Chrome native-messaging host; a thin Manifest V3
extension executes CDP commands. The chain:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

Stage 1 (the engine) shipped and merged to `main`. Stage 2 is the governance layer, built
per ADR-0013 (separable overlay; all-open stays first-class), ADR-0018 (observe then
enforce), ADR-0019 (layered typed config), and ADR-0021 (Ghostlight family baseline: the
S1 chassis plus S4 pure-serializable decision contract). The architecture baseline is a
dependency-inverted governance core, a browser domain plugin, and transport infra, in ONE
crate, with the dependency direction (infra -> {domain, core}; domain -> core; core ->
std/serde only) enforced later by a fail-closed arch-test (task A7). This task lays the
module homes that make that split real. All-open stays byte-identical: the no-manifest
STEP-0 short-circuit means an ungoverned engine behaves exactly as stage 1, and this task
adds no governance logic at all.

## Current behavior

Verified against the working tree at the start of this task. Line numbers drift as files
move; trust the prose over the numbers.

Flat `src/` layout today:

```
src/
  lib.rs            pub mod {browser, debug, dispatch, doctor, error, install, mcp,
                    native, origin, policy, tools}; pub use error::{...}
  main.rs           the binary
  browser.rs        the Browser executor handle (Browser, AttachOutcome, attach())
  dispatch.rs       the no-op governance seam (PolicyDecision::Allow, policy_check, audit)
  debug.rs doctor.rs error.rs origin.rs
  install/          mod.rs clients.rs merge.rs native_host.rs
  policy/           mod.rs (KeyDef, KEYS, Config), redact.rs (apply_to_result, apply_to_tree)
  mcp/              mod.rs server.rs tools.rs types.rs  schemas/tools.json
  native/           mod.rs host.rs ipc.rs messages.rs
  tools/            mod.rs + 10 per-tool stub files (doc-only in stage 1)
```

The complete set of cross-module references that the move must rewrite (every other file
references only crate-root items like `crate::{Error, Result}`, `crate::debug`, which do
not move):

- `src/browser.rs` (the executor): `use crate::native::host;` and a doc link
  `crate::native::ipc::serve`. Also `use crate::debug::DebugSink;` and `use crate::ToolError;`
  (crate-root, do NOT change).
- `src/native/ipc.rs`: `use crate::native::host;`, `use crate::browser::{AttachOutcome, Browser};`,
  and two doc links `crate::browser::Browser::attach`. Also `use crate::{Error, Result};`
  (crate-root, do NOT change).
- `src/native/messages.rs`: two doc-comment references to `crate::browser`.
- `src/mcp/server.rs`: `use crate::browser::Browser;`, `use crate::dispatch;`,
  `use crate::mcp::tools::{is_known_tool, TOOLS_JSON};`,
  `use crate::mcp::types::{text_content, JsonRpcResponse};`,
  `use crate::policy::{self, Config};`, a doc link `crate::dispatch`, and the one
  cross-module call site `policy::redact::apply_to_result(&mut result, config.secrets_redact());`
  (redact moves to `browser/`, so this call must be repointed).
- `src/install/native_host.rs`: `use crate::native::host::{HOST_DESCRIPTION, HOST_NAME};`.
- `src/doctor.rs`: `use crate::native::ipc::{self, EndpointProbe};`.
- `src/main.rs` (the binary): `use browser_mcp::browser::Browser;` (line ~19), a doc link
  `browser_mcp::browser::Browser` (line ~9), `use browser_mcp::native::ipc;` (line ~23),
  and `browser_mcp::mcp::server::run(browser)` (line ~268).
- `src/mcp/tools.rs`: `pub const TOOLS_JSON: &str = include_str!("schemas/tools.json");` --
  a path RELATIVE to that source file. Moving the whole `mcp/` directory (including
  `schemas/`) keeps this relative path valid; do not split the schema away from `tools.rs`.
- `src/policy/mod.rs`: declares `pub mod redact;` (line ~19). Since `redact.rs` moves out
  of `policy/` into `browser/`, this declaration is deleted here and re-declared in
  `browser/mod.rs`.
- `src/policy/redact.rs`, `src/dispatch.rs`, and every file under `src/tools/` have NO
  `use crate::...` cross-module references (redact uses only `serde_json` and `super::*` in
  its inline tests; dispatch is fully self-contained; the tool files are doc-only stubs).

Tests in the tree that reference library paths (these constrain the re-export decision in
Required behavior part 3):

- `tests/tool_schema_fidelity.rs`: `use browser_mcp::mcp::tools::TOOLS_JSON;` -- MUST pass
  UNCHANGED (the sacred surface guard, ADR-0007).
- `tests/mcp_protocol.rs`: uses `browser_mcp::native::ipc::connect`,
  `browser_mcp::native::host::{read_message, write_message}`, and
  `browser_mcp::mcp::tools::TOOLS_JSON`.
- `tests/peer_death.rs`: no moved-module references.

## Required behavior

Five parts. This is a mechanical relocation; the only judgment calls are named explicitly.

### 1. The new module layout

Use `git mv` for every file move so history and blame follow the file (this is one commit;
`git mv` keeps the diff readable as renames). Target layout:

```
src/
  lib.rs                       (updated: mod decls + compat re-exports; part 3)
  main.rs                      (bin; executor import updated; part 3)
  error.rs debug.rs doctor.rs origin.rs   (UNCHANGED, stay at root)
  install/                     (UNCHANGED tree; one use-path edit in native_host.rs)
  governance/
    mod.rs                     (NEW: module doc + `pub mod dispatch; pub mod policy;`)
    dispatch.rs                (moved from src/dispatch.rs, byte-identical)
    policy/
      mod.rs                   (moved from src/policy/mod.rs; drop `pub mod redact;`)
  browser/
    mod.rs                     (NEW: module doc + `pub mod redact; pub mod tools;`)
    redact.rs                  (moved from src/policy/redact.rs, byte-identical)
    tools/
      mod.rs + the 10 stub files (moved wholesale from src/tools/)
  transport/
    mod.rs                     (NEW: module doc + `pub mod executor; pub mod mcp; pub mod native;`)
    executor.rs                (moved AND renamed from src/browser.rs; type stays `Browser`)
    mcp/                       (moved wholesale, INCLUDING schemas/tools.json)
      mod.rs server.rs tools.rs types.rs schemas/tools.json
    native/                    (moved wholesale)
      mod.rs host.rs ipc.rs messages.rs
```

The three new parent `mod.rs` files each need a module-level doc comment (constraint 5)
naming the bounded context. For example, `governance/mod.rs`:

```rust
//! Governance core -- the domain-agnostic policy layer.
//!
//! This bounded context (see docs/design/ghostlight-service-architecture.md section 3)
//! names no browser type. It owns the dispatch seam ([`dispatch`]) and the typed config
//! registry ([`policy`]). The dependency direction is strictly inward: infra and the
//! browser plugin may depend on this module; this module depends only on std and serde.
//! A fail-closed arch-test (task A7) will enforce that.

pub mod dispatch;
pub mod policy;
```

Write `browser/mod.rs` and `transport/mod.rs` in the same spirit (browser = the domain
plugin: tools plus page-content redaction; transport = composition-root I/O: the MCP
session, native messaging, and the executor handle). Keep them ASCII, no em-dashes.

### 2. Rewrite every internal `use` path

Rewrite each reference from the Current-behavior inventory to its new absolute `crate::`
path. Prefer absolute `crate::...` paths over `super::...` for greppability and to survive
future moves. Concretely:

- `transport/executor.rs` (was `src/browser.rs`):
  - `use crate::native::host;` -> `use crate::transport::native::host;`
  - doc link `crate::native::ipc::serve` -> `crate::transport::native::ipc::serve`
  - `use crate::debug::DebugSink;` and `use crate::ToolError;` stay (crate-root).
- `transport/native/ipc.rs`:
  - `use crate::native::host;` -> `use crate::transport::native::host;`
  - `use crate::browser::{AttachOutcome, Browser};` -> `use crate::transport::executor::{AttachOutcome, Browser};`
  - both doc links `crate::browser::Browser::attach` -> `crate::transport::executor::Browser::attach`
  - `use crate::{Error, Result};` stays.
- `transport/native/messages.rs`: doc-comment `crate::browser` -> `crate::transport::executor`.
- `transport/mcp/server.rs`:
  - `use crate::browser::Browser;` -> `use crate::transport::executor::Browser;`
  - `use crate::dispatch;` -> `use crate::governance::dispatch;`
  - `use crate::mcp::tools::{is_known_tool, TOOLS_JSON};` -> `use crate::transport::mcp::tools::{is_known_tool, TOOLS_JSON};`
  - `use crate::mcp::types::{text_content, JsonRpcResponse};` -> `use crate::transport::mcp::types::{text_content, JsonRpcResponse};`
  - `use crate::policy::{self, Config};` -> `use crate::governance::policy::{self, Config};`
  - doc link `crate::dispatch` -> `crate::governance::dispatch`
  - the redact call site: `policy::redact::apply_to_result(...)` no longer exists under
    `policy` (redact moved to the browser plugin). Change it to
    `crate::browser::redact::apply_to_result(&mut result, config.secrets_redact());`
    (add `use crate::browser::redact;` if you prefer the short call). This is the ONE
    cross-bucket call in the codebase: transport (infra) calling the browser plugin
    (domain), which is an allowed inward edge.
- `governance/policy/mod.rs`: delete the `pub mod redact;` line (line ~19). Its inline
  `#[cfg(test)]` module uses `super::*` and needs no change. The `redact` re-declaration
  lives in `browser/mod.rs` now.
- `src/install/native_host.rs`: `use crate::native::host::{HOST_DESCRIPTION, HOST_NAME};`
  -> `use crate::transport::native::host::{HOST_DESCRIPTION, HOST_NAME};`
- `src/doctor.rs`: `use crate::native::ipc::{self, EndpointProbe};`
  -> `use crate::transport::native::ipc::{self, EndpointProbe};`
- `browser/redact.rs`, `governance/dispatch.rs`, and every `browser/tools/*.rs`: no edits
  beyond the move (verify with grep that none gained or needs a `crate::` path).

### 3. `src/lib.rs` and the binary; the compatibility facade

Rewrite the `lib.rs` module declarations to the new shape and add a small public
re-export facade so the two integration tests keep resolving their paths UNCHANGED:

```rust
pub mod browser;
pub mod debug;
pub mod doctor;
pub mod error;
pub mod governance;
pub mod install;
pub mod origin;
pub mod transport;

pub use error::{Error, Result, ToolError};

/// Compatibility facade: transport-owned submodules whose paths external consumers
/// (integration tests, including the sacred `tool_schema_fidelity` guard) import at the
/// crate root. Internal code uses the real `crate::transport::...` paths; these aliases
/// exist only so the move is byte-transparent to callers outside the crate. They are
/// public API, so they raise no unused-import warning.
pub use transport::{mcp, native};
```

Notes on this facade:

- `tests/tool_schema_fidelity.rs` (`browser_mcp::mcp::tools::TOOLS_JSON`) MUST pass
  unchanged; the `pub use transport::mcp;` alias is what makes `browser_mcp::mcp` resolve
  after `mcp` physically moved under `transport/`. This is mandatory.
- `tests/mcp_protocol.rs` (`browser_mcp::native::...` and `browser_mcp::mcp::tools`) is not
  sacred, but the `native` alias keeps it byte-unchanged too, which is the lowest-risk
  outcome. Leave both integration test files untouched.
- Do NOT alias the executor to a crate-root `browser` name: `browser_mcp::browser` now
  refers to the NEW browser PLUGIN module (tools + redact), which has no `Browser` type.
  The executor is reached at `browser_mcp::transport::executor::Browser`.
- Keep `init_tracing` and its doc comment in `lib.rs` exactly as they are (only the `pub mod`
  block and the new `pub use` line change).

Update the binary `src/main.rs`:

- `use browser_mcp::browser::Browser;` -> `use browser_mcp::transport::executor::Browser;`
- the doc link `browser_mcp::browser::Browser` (line ~9) -> `browser_mcp::transport::executor::Browser`
- `use browser_mcp::native::ipc;` (line ~23) and `browser_mcp::mcp::server::run` (line ~268)
  resolve through the compat facade and may stay as-is; leaving them minimizes bin churn.
  Do not touch the `debug`/`doctor`/`install`/`error` references (crate-root, unchanged).

### 4. The new all-open golden test

Add ONE new integration test file, `tests/all_open_golden.rs`, that pins the two
observable invariants the move must preserve. This is the guard the reviewer relies on to
believe "pure move, zero behavior change". It complements (does not duplicate)
`tool_schema_fidelity.rs` (which pins individual schemas) and `mcp_protocol.rs` (which does
the live IPC round-trip): this test pins the advertised-surface order/count and the moved
dispatch seam's all-open decision, reached at their NEW locations.

```rust
//! All-open golden guard for the A1 module reorg. The regroup into governance/ browser/
//! transport/ must change NOTHING observable. Two invariants, reached through the NEW
//! module locations:
//!   1. tools/list byte-stability -- the advertised tool surface is the same 13 tools in
//!      the same order, and `is_known_tool` still resolves them.
//!   2. dispatch round-trip -- the moved `governance::dispatch` seam resolves every call
//!      to `Allow` (all-open), and `audit` is a no-op that does not panic.

use browser_mcp::governance::dispatch::{self, PolicyDecision};
use browser_mcp::transport::mcp::tools::{is_known_tool, TOOLS_JSON};
use serde_json::Value;

/// The 13 tool names in advertised order. COPY these once from the parsed `TOOLS_JSON`
/// (the sacred fixture is the source of truth for the exact order); do not guess the
/// order. A reorder, rename, or drop during the move fails here.
const GOLDEN_TOOL_NAMES: [&str; 13] = [
    // fill from TOOLS_JSON, e.g. the first is "tabs_context_mcp"
];

#[test]
fn tools_list_is_byte_stable_through_the_move() {
    let v: Value = serde_json::from_str(TOOLS_JSON).expect("TOOLS_JSON parses");
    let tools = v["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), GOLDEN_TOOL_NAMES.len(), "all 13 tools advertised");
    for (i, name) in GOLDEN_TOOL_NAMES.iter().enumerate() {
        assert_eq!(tools[i]["name"], *name, "tool #{i} name and order preserved");
        assert!(is_known_tool(name), "{name} must be a known tool");
    }
    assert!(!is_known_tool("bogus_tool"), "unknown tools stay unknown");
}

#[test]
fn dispatch_seam_is_all_open_after_the_move() {
    for name in GOLDEN_TOOL_NAMES {
        assert_eq!(
            dispatch::policy_check(name),
            PolicyDecision::Allow,
            "{name} must be allowed in the all-open engine"
        );
        dispatch::audit(name); // no-op seam; must not panic
    }
}
```

Fill `GOLDEN_TOOL_NAMES` by reading the parsed `TOOLS_JSON` order (the current fixture
begins with `tabs_context_mcp`; the 13 tool ids are `tabs_context_mcp`, `tabs_create_mcp`,
`navigate`, `computer`, `read_page`, `get_page_text`, `find`, `form_input`,
`javascript_tool`, `read_console_messages`, `read_network_requests`, `resize_window`,
`update_plan` -- but pin them in the EXACT order they appear in the fixture, not this
prose list). Using the `browser_mcp::transport::mcp::tools` path in this new test proves
the real new location resolves; the two existing tests prove the compat facade resolves.

### 5. Ledger and browser-test docs

If the repo carries a stage-2 ledger or a running-summary doc (as stage 1 did), add a one
-line entry recording the A1 reorg (files moved, buckets created, golden test added). Do
not invent a new ledger format; match whatever the tree already uses. No BROWSER-TESTS.md
change is warranted (this is a binary-internal move with no user-visible behavior change),
but note in the commit body that a full session smoke check confirmed byte-identical
behavior.

## Constraints

1. ASCII only in all code and docs: no em-dashes, no arrows, no curly quotes, anywhere
   (comments, tests, strings, doc comments). Use Rust `\u{..}` escapes if a test ever needs
   a non-ASCII input (none is needed here).
2. All-open stays first-class and byte-identical: with no manifest and default config,
   every tool result is exactly what stage 1 produced. This task moves files; it changes
   NO runtime behavior. The STEP-0 short-circuit and the no-op dispatch seam are unchanged
   in content, only relocated. The new golden test plus the unchanged `mcp_protocol.rs` are
   the guard.
3. NEVER modify the tool schemas (`src/mcp/schemas/tools.json`, which becomes
   `src/transport/mcp/schemas/tools.json`), tool names, params, or descriptions. Move the
   `schemas/` directory together with `mcp/` so the `include_str!` relative path stays
   valid; do not edit its bytes. `tests/tool_schema_fidelity.rs` must pass UNCHANGED
   (ADR-0007, the sacred surface).
4. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task touches no file under `extension/`.
5. Rust 2021, `thiserror` for typed errors, doc comments on all public items and a
   module-level doc comment on every module (the three new `mod.rs` files each need one),
   `rustfmt` clean, `cargo clippy --all-targets -- -D warnings` clean.
6. One task = one commit: the file moves, every use-path rewrite, the new golden test, and
   the ledger line land together. Keep the tree green (full suite + clippy + fmt) in that
   single commit. Use `git mv` so the moves show as renames.
7. Windows dev gotcha: if `target/debug/browser-mcp.exe` is locked by a running MCP-client
   session, rename it aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`)
   and rebuild, or stop the MCP client first.
8. NO logic change, NO new types, NO new dependencies, NO `Cargo.toml` edit. If a file's
   body needs anything beyond a relocation and a `use`-path rewrite, you have exceeded this
   task's scope -- stop and reconsider. The single non-move edit permitted is deleting
   `pub mod redact;` from `governance/policy/mod.rs` and re-adding it in `browser/mod.rs`,
   and repointing the one redact call site in `transport/mcp/server.rs`.
9. Re-verify EVERY use path after moving: grep the whole `src/` tree for stale
   `crate::browser`, `crate::dispatch`, `crate::policy`, `crate::mcp`, `crate::native`, and
   `crate::tools` paths and confirm each remaining hit is intentional (there should be none
   left except the new `crate::governance::...`, `crate::browser::...`, and
   `crate::transport::...` forms). The compiler will catch broken paths, but grep catches
   stale doc-comment links that do not fail the build.

## Verification

1. `cargo fmt` then `cargo clippy --all-targets -- -D warnings` from the repo root: clean.
2. `cargo test` from the repo root: all tests pass, including the new
   `tests/all_open_golden.rs`, `tests/tool_schema_fidelity.rs` UNCHANGED,
   `tests/mcp_protocol.rs` UNCHANGED, `tests/peer_death.rs`, and every relocated inline
   unit test (the `policy/mod.rs` registry tests and the `redact.rs` tests still run at
   their new module paths).
3. If `target/debug/browser-mcp.exe` is locked, rename it aside (see constraint 7) and
   rebuild.
4. Grep checks (all must return only the NEW paths):
   - `crate::browser`, `crate::dispatch`, `crate::policy`, `crate::mcp`, `crate::native`,
     `crate::tools` no longer appear anywhere under `src/` (the new forms
     `crate::governance::`, `crate::transport::`, `crate::browser::redact`,
     `crate::browser::tools` are the only survivors).
   - `src/dispatch.rs`, `src/policy/`, `src/tools/`, `src/native/`, `src/mcp/`, and
     `src/browser.rs` no longer exist as files at those old paths.
   - `src/transport/mcp/schemas/tools.json` exists and is byte-identical to the pre-move
     `src/mcp/schemas/tools.json` (`git mv` guarantees this; confirm with the fidelity
     test).
5. `git status` shows the moves as renames (from `git mv`) plus edits only in the files
   named in Required behavior parts 2 and 3, the new `tests/all_open_golden.rs`, the three
   new `mod.rs` files, and the ledger line. No stray file is touched.
6. Manual check (binary-only change; restart the MCP client to pick up the new binary; no
   extension reload needed): a normal all-open session behaves exactly as before -- the
   client sees the same 13 tools and a `navigate` + `read_page` round-trip works
   identically. `read_page` on a page with a password field still shows `[value redacted]`
   (redaction moved to `browser/redact.rs` but its behavior is unchanged).

## Out of scope

- Any governance LOGIC: no PDP, no grant resolution, no classification, no matcher, no
  denials, no shadow mode. Dispatch stays the documented no-op seam it is today, only at a
  new path.
- Any NEW types or traits: the ports (`PolicyDecisionPoint`, `DomainPolicy`,
  `ResourceResolver`, `AuditSink`, `RwClass`, `GoverningResource`, `Denial`, etc.) are task
  A2's scope. A1 only creates module homes for them.
- The `Governance` facade and making dispatch the enforcement point: task A3.
- Config `Copy`-to-owned, the typed key registry growth (G01), and the layered resolver
  (G02): tasks A4 and beyond. `policy/mod.rs` moves verbatim; its `KeyDef`/`KEYS`/`Config`
  content is NOT changed here.
- Hot-reload substrate, config CLI/schema surfaces, and the fail-closed arch-test (A5, A6,
  A7). This task deliberately does NOT add the arch-test; it only creates the layout the
  arch-test will later police.
- The control-plane listener, the persistent-service split, and the web adapter (the
  Ghostlight service phases B/C).
- Any `Cargo.toml` change, any new dependency, any extension change, any schema byte change.
