# Stage 2 ledger

Durable, context-wipe-safe record of stage-2 (governance) execution. This file plus
`BROWSER-TESTS.md` are the executor's memory. On every start, after any interruption, and whenever
state is unclear: read the RESUME HERE section first, then `PLAN.md` and `RECONCILIATION.md`, then the
current task prompt, then continue. Never rely on remembering earlier work; re-read files.

## RESUME HERE

- Branch: `stage-2` (off `main`, which has stage 1 merged). Never push, never merge, never commit to
  `main`.
- Progress: task `a1` (module reorg) landed.
- NEXT TASK: Phase A, task `a2` (`docs/tasks/stage-2/a2-governance-ports.md`).
- Order authority: `PLAN.md` (Phase A -> B -> C -> D). Full linear sequence is in `BOOTSTRAP.md`.
- Reconciliation: `RECONCILIATION.md` is AUTHORITATIVE over any conflicting detail in a `g`-doc.
- Invariants that must hold after every task: all-open byte-identical (the all-open golden test +
  `tests/mcp_protocol.rs`), the sacred tool surface (`tests/tool_schema_fidelity.rs`), `cargo clippy
  --all-targets -- -D warnings` clean, `cargo fmt --check` clean, full `cargo test` green, ASCII-only.

## Task log

(Append one entry per completed task, newest at the bottom. Suggested shape:)

### <task-id> <title> -- <date>
- Commit: <hash>
- Files touched: <list>
- Summary: <what landed, key decisions, any conservative choice made>
- Deviations from the g-doc per RECONCILIATION.md: <placement / hot-reload / ports notes>
- Verification: clippy/fmt/test status; which tests were added
- Browser checks queued: <count> (appended to BROWSER-TESTS.md as <task-id>-<n>), or none

### a1 module reorg (governance/ browser/ transport/) -- 2026-07-02
- Commit: (pending, see this task's commit)
- Files touched: `git mv` of `src/dispatch.rs`, `src/policy/{mod.rs,redact.rs}`, `src/tools/**`,
  `src/native/**`, `src/mcp/**` (incl. `schemas/`), `src/browser.rs`; new
  `src/{governance,browser,transport}/mod.rs`; edited `src/lib.rs`, `src/main.rs`, `src/doctor.rs`,
  `src/install/native_host.rs`, `src/transport/executor.rs`, `src/transport/native/{ipc,messages}.rs`,
  `src/transport/mcp/server.rs`, `src/governance/policy/mod.rs`; new `tests/all_open_golden.rs`.
- Summary: pure move, zero behavior change. `governance/` got `dispatch.rs` + `policy/` (minus
  `redact.rs`); `browser/` got `tools/` + `redact.rs`; `transport/` got `native/`, `mcp/`, and
  `browser.rs` (renamed `executor.rs` to avoid colliding with the new `browser/` plugin module).
  Every `use crate::...` cross-reference rewritten to the new absolute path; the one cross-bucket
  call (`transport/mcp/server.rs` redacting `read_page` output) now calls
  `crate::browser::redact::apply_to_result` directly. `lib.rs` re-exports `pub use
  transport::{mcp, native};` so `tests/tool_schema_fidelity.rs` and `tests/mcp_protocol.rs` keep
  resolving `browser_mcp::mcp::...` / `browser_mcp::native::...` unchanged, per the task's compat-
  facade requirement.
- Deviations from the g-doc per RECONCILIATION.md: none (A1 is not a g-doc; it is one of the new
  a-prompts that already encodes the current vision). Followed a1-module-reorg.md as written.
- Verification: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo
  test` green (81 lib unit tests + 2 new `tests/all_open_golden.rs` + 4 `tests/mcp_protocol.rs`
  unchanged + 1 `tests/peer_death.rs` + 6 `tests/tool_schema_fidelity.rs` unchanged = 94 total).
  ASCII scan clean on every touched/moved file. Grep confirmed no stale `crate::browser::Browser`,
  `crate::dispatch`, `crate::policy`, `crate::mcp`, `crate::native`, `crate::tools` paths remain.
  `src/mcp/schemas/tools.json` -> `src/transport/mcp/schemas/tools.json` confirmed byte-identical
  (diff empty). One environment snag: `git mv src/mcp` (whole-directory rename) twice failed with
  Windows `Permission denied` (likely a transient AV/indexer lock); worked around by moving the 6
  files inside `src/mcp/` individually with `git mv`, then removing the resulting empty leftover
  `src/mcp/schemas/` and `src/mcp/` directories (untracked by git, harmless, but removed for
  tidiness) -- no conservative policy choice involved, purely a retry mechanic. No other locked-exe
  issue this task; `target/debug/browser-mcp.exe` needed the constraint-7 rename-aside once before
  the first build.
- Browser checks queued: none (binary-internal move; no user-visible behavior change per the task's
  own scope note).

## Reminders before running BROWSER-TESTS.md

Stage 2 is mostly unit-testable (pure governance logic), but several tasks have browser-facing
behavior that needs a real browser: the take-the-wheel pause (g10), the panic kill switch (g11), tool
advertisement filtering and `tools/list_changed` on hot-reload (g14), and end-to-end manifest
enforcement (g12/g13/g15). Accumulate those checks in `BROWSER-TESTS.md` as their tasks land; a human
runs them against a live browser after the code is in, exactly as release-1 did.
