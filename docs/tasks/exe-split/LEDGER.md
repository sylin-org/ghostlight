# exe-split LEDGER

Durable batch progress. One task = one CODE commit + one ledger commit = one log entry
(BOOTSTRAP per-task procedure). Update after EVERY task, before starting the next.

## RESUME HERE

- Next task: **S9** (`S9-no-supervisor-and-dev-loop-doc.md`)
- Base commit: `fccca60` on `dev` (tree green at batch authoring; later docs-only commits carry
  the batch itself)
- Batch state: IN PROGRESS (S1..S8 complete)

## Task table

| Task | Title | Status | Commit |
|---|---|---|---|
| S1 | Workspace + transport crate skeleton | done | 14a8bd0 |
| S2 | Move leaf utilities to transport | done | bbb02da |
| S3 | Move wire + handshake to transport | done | a48c136 |
| S4 | Create ghostlight-core; root becomes facade | done | 4d8767a |
| S5 | ghostlight-adapter-agent bin + rewire clients + test harness | done | a6ff4e0 |
| S6 | ghostlight-adapter-browser bin + host install rework | done | 4a95f68 |
| S7 | Retire roles from the ghostlight bin | done | 583a25a |
| S8 | Reconnect patience (120s) + ADR-0045 amendment | done | cbe3761 |
| S9 | --no-supervisor + DEV-LOOP.md | pending | - |
| S10 | Packaging + distribution sweep | pending | - |

## Log

(Append one entry per finished task:)

```
### S<n> -- <title>
- Commit: <hash>
- Verification: fmt OK / clippy OK / test --workspace OK / linux cross-check OK
- Deviations:
  1. <none | numbered list, one line each>
```

### S1 -- Workspace + transport crate skeleton
- Commit: 14a8bd0
- Verification: fmt OK / clippy OK / test --workspace OK (524 root unit + full integration suite pass; new ghostlight-transport crate builds, 0 tests) / linux cross-check OK
- Deviations:
  1. none. (Git reported routine CRLF->LF normalization on Cargo.toml; committed blobs are LF per repo convention -- no content or requirement change.)

### S2 -- Move leaf utilities to transport
- Commit: bbb02da
- Verification: fmt OK / clippy OK / test --workspace OK (full suite green; ghostlight-transport now runs 36 moved unit tests, 0 failed) / linux cross-check OK
- Deviations:
  1. Promoted three observability fns from `pub(crate)` to `pub` -- `now_ms`, `fmt_ms`, `session_state_files`. Reason: `src/hub/manage/doctor.rs` (root crate) calls all three, and once observability moved to ghostlight-transport the `pub(crate)` visibility scoped them to transport, breaking the cross-crate calls. SPEC section 2 sanctions exactly this ("items that were `pub(crate)` or private and are now needed across the crate boundary become `pub`"). No behavior change; not a governance/tool-surface item.
  2. watchdog.rs keeps a now-dangling rustdoc intra-doc link `[`crate::main`]` (the lib-only transport crate has no `main`). Left unmodified per the mechanical-move rule (not in the SPEC section 2 rewrite list). Harmless to SPEC-12 verification: intra-doc links are a rustdoc lint, not built by clippy/test/check, and CI runs no `cargo doc`.

### S3 -- Move wire + handshake to transport
- Commit: a48c136
- Verification: fmt OK / clippy OK / test --workspace OK (full suite green; ghostlight-transport now runs 56 unit tests, 0 failed; the 3 adapter-side ipc tests + 2 service-side ipc tests both run in their new homes) / linux cross-check OK. Merge shim confirmed at src/transport/native/ipc.rs:43.
- Method note: the root ipc.rs adapter/service split was done with a checked Python script (scratchpad) that extracts service-half line ranges by number with a boundary assertion on every range, so the delicate unsafe FFI (capture_peer_cred, win_security) is preserved byte-exact rather than retyped. The adapter half (transport/src/ipc.rs) was written fresh with the SPEC section 2 path rewrites.
  1. transport/src/ipc.rs doc-prose adjustments (3), all to avoid dangling rustdoc links that would point OUTSIDE the transport crate (a core dep transport must never take) or at a renamed item: (a) the module doc carries the original's general pre-endpoint paragraphs plus a new one-line ADR-0046 split note, dropping the endpoint-enumeration paragraph whose links name the now-relocated serve/claim/serve_adapters/handle_adapter_connection/send_service_proof; (b) two `[`crate::hub::outbound::browser::Browser::attach`]` links in the probe_endpoint docs became the prose "the browser executor" (Browser is a core type); (c) a stale `[`dial_with_self_heal`]` link became `[`connect_and_handshake`]` (its current name). No code/behavior change.
  2. root ipc.rs tests module gained `use tokio::time::{sleep, Duration};`. The module-level tokio::time import was dropped because the service half's non-test code never uses sleep/Duration (only the two service tests do); the import moved into the tests module so clippy -D warnings stays clean either way.
  3. hub/mod.rs: role/antisquat/handshake/supervisor consolidated into ONE re-export line `pub use ghostlight_transport::{antisquat, handshake, role, supervisor};` (rather than a separate `role` line plus a new three-item line). Same effect, one fewer line; fmt keeps the alphabetical order.
  4. Carried forward: transport now also has a dangling rustdoc link in handshake.rs (`[`crate::transport::mcp::server::serve_session`]`), same rustdoc-only class as the S2 watchdog note; harmless to SPEC-12.

### S4 -- Create ghostlight-core; root becomes facade
- Commit: 4d8767a
- Verification: fmt OK / clippy OK / test --workspace OK (40 test-result lines, 0 FAILED; ghostlight-core runs 468 unit tests, 0 failed) / linux cross-check OK. tests/ diff since HEAD = EXACTLY tests/architecture.rs + tests/hub_role_wiring.rs (SPEC section 12 pin met).
- Method note: the SPEC section 3 path rewrites were applied by a checked Python script (13 patterns across crates/core/src; 18 files, e.g. crate::transport::mcp -> crate::mcp x32, crate::instance -> ghostlight_transport::instance x12). The ipc references were fixed BY HAND (not the script) for the adapter-vs-service split: doctor.rs -> ghostlight_transport::ipc (all-adapter), pipe.rs -> `use crate::hub::endpoint as ipc` (all-service), hub/mod.rs split into `use ghostlight_transport::ipc` (default_endpoint/relay_adapter) + child module `endpoint::` for serve/claim. core lib.rs (SPEC 3) and root facade (SPEC 6) written exactly as pinned.
  1. Kept `anyhow` in ghostlight-core's [dependencies] though SPEC section 3's minus-list drops it: crates/core/src/hub/mod.rs uses `anyhow::{Context, Result}`, so the compiler demands it (SPEC section 3: "the compiler is the referee; log every kept-but-questionable dep"). getrandom/hmac/tracing-subscriber/clap were dropped as SPEC directs (0 refs in the moved tree); sha2/uuid/chrono/url kept (in use).
  2. Root [dependencies] keeps dirs, uuid, chrono, url beyond SPEC section 4's list (ghostlight-core, ghostlight-transport, clap, anyhow, tokio, tracing, serde_json). Reason: the integration tests in tests/ use those crates directly (dirs x2 files, chrono x1, uuid x1, url x1, serde_json x23, tokio x8) and the package has no [dev-dependencies] section, so [dependencies] is the only place they can live. main.rs itself needs only clap/anyhow/tokio/tracing. Removed serde, tracing-subscriber, thiserror, sha2, hmac, getrandom, winreg, windows-sys, libc from root.
  3. Straggler fix (compiler-demanded): crates/core/src/governance/templates.rs `include_str!` paths went `../../examples/` -> `../../../../examples/` (the file is two directory levels deeper after the move; examples/ stays at the repo root, unmoved). Path-only; the embedded template bytes and all governance semantics are byte-unchanged. The a7 governance-purity test (tests/architecture.rs) still passes because `ghostlight_transport::...` does NOT contain the forbidden token `crate::transport` (the ban is on the `crate::`-prefixed path edge).
  4. Straggler fix: removed the `use crate::hub::endpoint;` I first added to hub/mod.rs -- it collided (E0255) with the `pub mod endpoint;` child-module declaration; the child module already puts `endpoint` in scope, so `endpoint::serve` / `endpoint::claim_adapter_endpoint` resolve directly.
  5. endpoint.rs (moved service half): the S3 merge shim became a plain `use ghostlight_transport::ipc::*;` (was `pub use`) -- the root facade (SPEC 6) re-exports both ipc halves under `ghostlight::native::ipc`, so a `pub use` here would double-export. Module doc collapsed to the SPEC section 3 one-liner.

### S5 -- ghostlight-adapter-agent bin + rewire clients + test harness
- Commit: a6ff4e0
- Verification: fmt OK / clippy OK / test --workspace OK (full suite green; adapter_reconnect + mcp_protocol + hub_lifecycle spawn the NEW ghostlight-adapter-agent bin and pass; new clients test passes) / linux cross-check OK. `cargo build -p ghostlight-adapter-agent` OK; `cargo tree -p ghostlight-adapter-agent` does NOT contain ghostlight-core (the load-bearing ADR-0046 rule; its tree is transport-only).
  1. clients.rs had NO `#[cfg(test)] mod tests` block; created one to house the pinned `server_entry_points_at_the_agent_adapter_sibling` test (the task's "NEW in clients.rs tests module" implied it exists -- it did not).
  2. crates/adapter-agent/Cargo.toml uses tokio features `["rt-multi-thread", "macros"]` exactly as SPEC 5.1 pins. The main uses `tokio::sync::Notify` (the "sync" feature), which resolves via Cargo feature unification: ghostlight-transport (a dependency) enables tokio "sync", and the single tokio build in the adapter's dep graph carries the union. Build + core-absence both verified, so no feature was added; noted only so a future transport dep change that drops "sync" is understood to affect this bin.
  3. tests/adapter_reconnect.rs got its OWN local `adapter_bin()` helper (it does not `mod support;`); tests/hub_lifecycle.rs uses `support::adapter_bin()` (it does). tests/peer_death.rs (native-host role) and the install_instance/policy_* CLI spawns were left untouched per S5's CAUTION.

### S6 -- ghostlight-adapter-browser bin + host install rework
- Commit: 4a95f68
- Verification: fmt OK / clippy OK / test --workspace OK (full suite green; the 4 oracles pass: from_exe_stem_with_base_resolves_the_browser_adapter_family, instance_launcher_default_is_the_adapter_browser_sibling, dev_install_plan_copies_a_named_binary_and_suffixes_the_whole_stack, native_host_exits_when_server_dies -- the last now spawning the browser adapter) / linux cross-check OK. `node --check tests/e2e/run-smoke.mjs` OK; `cargo build -p ghostlight-adapter-browser` OK; `cargo tree -p ghostlight-adapter-browser` has 0 ghostlight-core refs (native + linux).
  1. tests/peer_death.rs: removed its now-unused local `fn bin()` when switching the native-host spawn to `support::browser_bin()` (it was the fn's only caller; leaving it would trip clippy -D warnings dead_code).
  2. Added `pub fn browser_bin()` to tests/support/mod.rs (symmetric with S5's `adapter_bin()`); peer_death uses it. adapter_reconnect keeps its own local helpers (still no `mod support;`).
  3. install_instance dev test: updated BOTH the assertion (`ghostlight-dev` -> `ghostlight-adapter-browser-dev`) and its panic-message text; the default-plan test's `!plan.contains("ghostlight-dev")` still holds (the default plan copies nothing and names only the two adapter siblings).
  4. instance_launcher named-branch derives the copy file name as `ghostlight-adapter-browser-<name>[.exe]` directly from `instance.name()` (the old code used `mcp_server_name()` = `ghostlight-<n>`); default branch uses `sibling_bin(current_exe, "ghostlight-adapter-browser")` instead of `normalize_exe_path(current_exe)`. install/mod.rs copies FROM the browser-adapter sibling (computed once into `copy_from`, used by the size-check, the manual hint, and the CopyBinary op).

### S7 -- Retire roles from the ghostlight bin
- Commit: 583a25a
- Verification: fmt OK / clippy OK / test --workspace OK (full suite green incl. new bare_invocation_prints_guidance_and_exits_2; the S5/S6 re-points mean nothing else references the deleted roles) / linux cross-check OK. S7-specific grep: `run_mcp_server|run_native_host_role|run_as_adapter` across src+crates returns ONLY doc-comment prose (6 hits, all `//`/`///`), ZERO code hits.
  1. Removed `use ghostlight::native::ipc;` from src/main.rs -- it was used only inside the deleted `run_native_host_role`; leaving it would trip clippy -D warnings (unused import).
  2. `doctor::sweep_orphans()` (a `pub fn`) lost its only internal caller (the deleted run_mcp_server) but was NOT removed: it is public API reachable via the facade (`ghostlight::hub::manage::doctor::sweep_orphans`), so it raises no dead_code warning; the standalone `doctor --fix` reaper is the other user of the reap machinery.
  3. Six stale doc-PROSE mentions of the retired role names survive in transport/handshake.rs, transport/ipc.rs (x2), adapter-agent/main.rs (x2, describing what it was "transcribed from"), and core/hub/mod.rs:~325 (ServiceContext doc). All are `//`/`///` prose (never `[intra-doc links]`), rustdoc-only, explicitly sanctioned by the S7 verify ("doc-comment prose mentions are acceptable"). The role-ENUMERATION docs the task named (root main.rs module doc, core hub/mod.rs module doc) WERE updated to name the two adapter executables.
  4. The bare-invocation guidance is printed as TWO `eprintln!` lines (the exact SPEC section 9 text) + `std::process::exit(2)` in the `Cli { command: None, .. }` arm.

### S8 -- Reconnect patience (120s) + ADR-0045 amendment
- Commit: cbe3761
- Verification: fmt OK / clippy OK / test --workspace OK / linux cross-check OK. S8-specific oracle: `cargo build -p ghostlight-adapter-agent` then `cargo test --test adapter_reconnect` 3x -- each run 2 passed (restart + the new 5s-gap test), 0 failed (~5-6s per run, i.e. the 5s gap is really exercised).
- Deviations:
  1. none of substance. connect_and_handshake's first-connect path is byte-identical (interval/window resolve to SELF_HEAL_RETRY_INTERVAL/SELF_HEAL_RETRY_WINDOW when `reconnect == false`); relay_adapter passes `!first`. The two new pub consts sit at the top of transport/ipc.rs (module level, after the imports). The 5s-gap test was DUPLICATED from the restart test (per the task's "otherwise duplicate"; the setup was not trivial to factor without obscuring both) with the two pinned changes: a 5s `thread::sleep` between kill and respawn, and a 30s post-restart recv timeout.

## Blocked

(Only if the failure protocol fired: task id, exact failing step/error text, one-paragraph
diagnosis. The batch HALTS here.)
