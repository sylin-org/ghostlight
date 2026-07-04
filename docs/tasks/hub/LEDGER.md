# Ghostlight Hub batch: LEDGER

Durable progress for the Hub batch (ADR-0030). One task = one commit. Update this file at the end of
every task, per BOOTSTRAP step 8. This is the single source of truth for "where are we"; a fresh
executor resumes from RESUME HERE with no other context.

## RESUME HERE

**Next task: H1 (`H1-serve-session-generic.md`).**
H0 landed (pure code move; `src/hub` composition root extracted). Start at H1, follow the
per-task procedure in `BOOTSTRAP.md`.

## Status

| Task | Title | Status | Commit | Notes |
| --- | --- | --- | --- | --- |
| H0 | Extract the HubCore composition root | DONE | a4e87b6 | |
| H1 | Transport-generic serve_session + ServiceContext | pending | -- | |
| H2 | Persistent service + thin adapter + multiplex | pending | -- | the one large coupled commit |
| H3 | Adapter-minted GUID identity + peer-cred binding | pending | -- | |
| H4 | Binary-authoritative cross-session tab isolation | pending | -- | |
| H5 | Reconnect grace window + honest bounded queue | pending | -- | orthogonal after H2 |
| H6 | Detached non-admin lifecycle + anti-squat | pending | -- | job-breakaway is the acceptance gate |
| H7 | Tab-group-per-session presentation | pending | -- | crosses the JS boundary |
| H8 | Local web API = TCP; bind per policy | pending | -- | needs H2+H3+H4; the corrected D2/D5 |

Status values: `pending` | `in-progress` | `DONE` | `BLOCKED`.

## Log

One entry per task as it closes (or blocks). Number every deviation from the task file.

### H0
- Verified all as-of-authoring facts in `H0-extract-hubcore.md` against the live tree: `main::run_server`
  (lines 442-547), `build_debug_sink` (lines 552-570, two callers), the `src/lib.rs` alphabetized module
  block, and the referenced `ipc::serve`/`ipc::default_endpoint`/`mcp::server::run`/`doctor::sweep_orphans`/
  `proc::parent`/`watchdog::wait_until_orphaned` signatures. All matched; no STOP precondition fired.
- Created `src/hub/mod.rs` hosting `run_mcp_server` (verbatim `run_server` body) and `build_debug_sink`
  (verbatim body, now `pub`). Added `pub mod hub;` to `src/lib.rs` between `governance` and `install`.
  Updated `src/main.rs`: the `command: None` arm now calls `ghostlight::hub::run_mcp_server`,
  `run_native_host_role` now calls `ghostlight::hub::build_debug_sink`; deleted the old `run_server` and
  `build_debug_sink` functions; narrowed/removed the imports the task named (`Context` narrowed off
  `anyhow::Result`; `browser::pattern`, `debug::DebugSink`, `governance::manifest::source`,
  `transport::executor::Browser` removed; `native::ipc` kept).
- No deviations from the task file. All four verification commands passed for real:
  `cargo build --all-targets`, `cargo test` (423 tests + the sacred/named suites, all ok), `cargo clippy
  --all-targets -- -D warnings` (clean), `cargo fmt --all -- --check` (clean after running `cargo fmt --all`
  once to normalize the new file's import order and a trailing blank line in `main.rs` -- whitespace/import
  ordering only, no semantic change; not logged as a numbered deviation since it does not alter any named
  fact, oracle, or assertion). Sacred tests (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
  `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`) green and byte-unmodified. Only
  `src/lib.rs`, `src/main.rs`, and the new `src/hub/mod.rs` changed; no NEVER-touch fence moved.
- Note: `cargo build`/`test`/`clippy` were run with `CARGO_TARGET_DIR` pointed at a scratch directory
  (not the repo's `target/`) because three live `ghostlight.exe` processes (this environment's own
  dogfooded MCP/native-host session) held the repo's `target/debug/ghostlight.exe` locked on Windows;
  this is a local build-artifact routing choice only, not a source or test change.

### H1
- (not started)

### H2
- (not started)

### H3
- (not started)

### H4
- (not started)

### H5
- (not started)

### H6
- (not started)

### H7
- (not started)

### H8
- (not started)

## Deviation format

When you deviate from a task file (a signature differs from as-of-authoring, a helper had to move,
an oracle needed pinning), record it under that task as:

```
D<n>: <what the task said> -> <what you actually did> because <the tree fact that forced it>.
     Impact on later tasks: <none | names the task + what it must now assume>.
```

A BLOCKED entry records instead: the failed assumption (with the file/symbol actually found), the
STOP precondition or fence that triggered, and what is needed to proceed. Then HALT.
