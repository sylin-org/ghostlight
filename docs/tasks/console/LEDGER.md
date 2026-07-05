# Ghostlight Console batch: LEDGER

Durable progress for the Console batch (ADR-0030 Decision 9). One task = one commit (landed as a
`feat(console): K<N> ...` code commit followed by a separate `docs(console): record K<N> commit
hash` ledger-update commit, per BOOTSTRAP.md's Environment facts). Update this file at the end of
every task, per BOOTSTRAP.md step 8. This is the single source of truth for "where are we"; a
fresh executor resumes from RESUME HERE with no other context.

## RESUME HERE

**K2 is NEXT (`K2-console-static-routes.md`).** K1 is DONE and committed. Read
`docs/tasks/console/BOOTSTRAP.md` in full, then `K2-console-static-routes.md` and the PINS.md
sections it cites (CS1, CS1.1, CS1.2, CS1.3, CS1.4, CS10, CS11). Follow the per-task procedure in
BOOTSTRAP.md exactly.

## Status

| Task | Title | Status | Commit | Notes |
| --- | --- | --- | --- | --- |
| K1 | Config + session read accessors; shared config-write function | DONE | 908e1d9 | no HTTP, no UI; PINS.md CS6-CS9; see Log for D1 |
| K2 | Console static GET routes in src/hub/webapi.rs | pending | -- | needs K1; PINS.md CS1, CS10, CS11 |
| K3 | GET /api/v1/config + config table UI | pending | -- | needs K2; PINS.md CS2 |
| K4 | GET /api/v1/sessions + sessions UI | pending | -- | needs K2; PINS.md CS3 |
| K5 | POST /api/v1/config/webapi-enable-remote + UI control | pending | -- | needs K1+K2; PINS.md CS4, CS5 |

Status values: `pending` | `in-progress` | `DONE` | `BLOCKED`.

## Log

One entry per task as it closes (or blocks). Number every deviation from the task file.

### K1

- Verified all as-of-authoring facts in `K1-config-session-accessors.md` and PINS.md CS6-CS9
  against the live tree before writing any code: `ConfigStore`'s `snapshot: Mutex<Arc<Config>>`
  field and both write sites (`load_initial_with_policy`, `apply_plan`) matched exactly;
  `layers::Resolution` derives `#[derive(Debug, Clone)]`; `KeyDef` has exactly the six fields CS8
  names; `run_set`/`write_user_value` in `cli.rs` matched CS7's transcription; `SessionRegistry`'s
  `bindings` field and `SessionGuid`'s redacted `Display`/`Debug` matched CS9; `webapi.rs::run`
  opened with the hardcoded `builtin_webapi_from()` exactly as CS8.2 described. No STOP
  precondition fired.
- Implemented per PINS.md CS6, CS7, CS8, CS8.1, CS8.2, CS9 exactly as pinned: `ConfigStore` gained
  the `resolution: Mutex<Arc<layers::Resolution>>` field and `current_resolution()` accessor,
  written at both existing resolve sites (unconditionally in `apply_plan`, never gated by
  `Config`'s own `changed` check) and seeded in both test-only constructors
  (`for_test`/`for_test_with_user_source`) via CS6's own non-oracle fallback
  (`layers::resolve(&layers::LayerInputs::default())`). `cli.rs` gained `pub(crate) fn
  set_user_value` exactly as CS7's code block shows; `run_set` now calls it. `mod.rs` gained the
  `CHANNELS_WEBAPI_FROM` constant and its `KeyDef` (all three preset defaults `["localhost"]`,
  byte-identical to `builtin_webapi_from()`'s existing value). `webapi.rs` gained
  `live_channels_webapi_from` and now reads it at `run()`'s startup AND fresh on every accepted
  connection (never the loop-hoisted value), while `bind` itself stays resolved once at startup.
  `session.rs` gained `SessionSummary`/`live_session_summaries` exactly as CS9 shows, marked
  `#[allow(dead_code)]` (unused until K4, matching `tests/support/mod.rs`'s own precedent for
  forward-referenced helpers). Golden files (`tests/golden/config-schema.json`,
  `tests/golden/config-keys.md`) regenerated via `cargo run -- config schema`/`config docs` and
  diff-reviewed: the ONLY change in either file is the new `channels.webapi.from` entry.
- D1: the task file's own "Tests to write FIRST" asked for a `set_user_value` test "confirming it
  returns `Ok` with the written path and `Err` with the EXACT lock-refusal message" via a direct
  call -> found, on re-reading `set_user_value`'s actual body, that BOTH the success and
  lock-refusal branches call `resolve_with_warnings` first, which reads the REAL, non-injectable
  `governance::config::load::user_config_path()`/`org_policy_path()` (confirmed: no
  `GHOSTLIGHT_*`-style env override exists for either in `load.rs`) -- a real call down either
  branch in a unit test would read (and, on the success branch, WRITE) this literal machine's
  actual Ghostlight config file, an unauthorized real-machine side effect. Instead wrote
  `set_user_value_rejects_an_unregistered_key_before_touching_any_file`, which exercises
  `set_user_value` for real down the ONE branch that returns before ever calling
  `resolve_with_warnings` (an unregistered key), confirming the function is genuinely reachable
  and correctly wired without touching any real path. The existing
  `lock_refusal_exact_message_and_no_file_touched` test (unmodified) already covers the exact
  message string as a string-literal check, not a real call. Impact on later tasks: K5's own task
  file already anticipates this exact hazard for its POST-handler write path and pins a STOP
  precondition requiring the executor to prove a real path-isolation mechanism (mirroring
  `spawn_service_with_program_data`'s existing `ProgramData` env-override precedent) before
  writing any test that exercises `set_user_value`'s success or lock-refusal branch through a real
  spawned service. This deviation does not change K5's own required behavior, only confirms the
  same gap was independently re-derived from the live tree, not assumed.
- Verification: all four commands passed for real. `cargo build --all-targets` clean. `cargo
  test` green: 465 lib tests (up from 461; the +4 are the two new `reload::tests::
  current_resolution_*` tests, `cli::tests::set_user_value_rejects_an_unregistered_key_before_
  touching_any_file`, and `session::tests::live_session_summaries_reports_truncated_guid_pid_and_
  owned_tabs`) plus every integration suite, 0 failed -- including `tests/config_schema_golden.rs`
  (5/5) against the regenerated files, and H8's own `tests/webapi_auth.rs` (3/3)/
  `tests/channels_policy.rs` (1/1) with every existing assertion unmodified. `cargo clippy
  --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean. Sacred tests
  (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
  `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`, `tests/peer_death.rs`,
  `tests/mcp_protocol.rs`) green and byte-unmodified (`git diff --stat` confirms zero diff on all
  of them plus `src/transport/mcp/tools.rs` and `src/transport/native/host.rs`). Only
  `src/governance/config/{cli,mod,reload}.rs`, `src/hub/{session,webapi}.rs`, and the two golden
  files changed. No NEVER-touch fence moved.
- Note: `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's `target/`) for
  build-artifact routing only, not a source or test change (consistent with every Hub-batch task).

## Deviation format

When you deviate from a task file (a signature differs from as-of-authoring, a helper had to move,
an oracle needed pinning), record it under that task as:

```
D<n>: <what the task said> -> <what you actually did> because <the tree fact that forced it>.
     Impact on later tasks: <none | names the task + what it must now assume>.
```

A BLOCKED entry records instead: the failed assumption (with the file/symbol actually found), the
STOP precondition or fence that triggered, and what is needed to proceed. Then HALT.
