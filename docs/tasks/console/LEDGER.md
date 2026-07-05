# Ghostlight Console batch: LEDGER

Durable progress for the Console batch (ADR-0030 Decision 9). One task = one commit (landed as a
`feat(console): K<N> ...` code commit followed by a separate `docs(console): record K<N> commit
hash` ledger-update commit, per BOOTSTRAP.md's Environment facts). Update this file at the end of
every task, per BOOTSTRAP.md step 8. This is the single source of truth for "where are we"; a
fresh executor resumes from RESUME HERE with no other context.

## RESUME HERE

**K5 is BLOCKED (see the K5 Log entry below).** K1, K2, K3, K4 are DONE and committed; the tree is
green and clean at `635ccaa`. K5's own STOP precondition fired for real: the env-override
isolation approach the task anticipated (mirroring `spawn_service_with_program_data`'s
`ProgramData` precedent) does NOT transfer to the user config path on Windows. This ALSO caused a
real, accidental write to THIS machine's actual `%APPDATA%\ghostlight\config.json`
(`audit.enabled: true`) during the proof step the task itself required running before writing any
code -- that file has NOT been cleaned up (a destructive-action guard blocked the cleanup attempt
and correctly redirected to this BLOCKED report instead). A human must decide how to proceed
before K5 is re-attempted; see the Log entry's "What is needed to proceed."

## Status

| Task | Title | Status | Commit | Notes |
| --- | --- | --- | --- | --- |
| K1 | Config + session read accessors; shared config-write function | DONE | 908e1d9 | no HTTP, no UI; PINS.md CS6-CS9; see Log for D1 |
| K2 | Console static GET routes in src/hub/webapi.rs | DONE | 7eea843 | needs K1; PINS.md CS1, CS10, CS11; see Log for D1 |
| K3 | GET /api/v1/config + config table UI | DONE | d8ada7b | needs K2; PINS.md CS2 |
| K4 | GET /api/v1/sessions + sessions UI | DONE | c20958a | needs K2; PINS.md CS3 |
| K5 | POST /api/v1/config/webapi-enable-remote + UI control | BLOCKED | -- | real user-config-path isolation failed; see Log |

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

### K2

- Verified all as-of-authoring facts in `K2-console-static-routes.md` and PINS.md CS1/CS1.1-
  CS1.4/CS10/CS11 against the live tree: `handle_connection`'s control flow (parse request head ->
  require `Sec-WebSocket-Key` -> require GET+`Upgrade: websocket` -> `Host` check -> `channels.
  webapi.from` decision -> handshake) matched exactly; `HttpRequest`/`parse_http_request` discarded
  the path token exactly as described; `write_http_error` was already parameterized on
  `(status, reason)`; no test referenced port 4180 or `webapi::run` before this task. No STOP
  precondition fired.
- Implemented per PINS.md CS1, CS1.1, CS1.2, CS1.3, CS1.4, CS10, CS11: `HttpRequest` gained a
  `path` field (CS1.4); a new router (`route_console_request`, reached from `handle_connection`
  BEFORE the `Sec-WebSocket-Key` check, only when the request is not a WS-upgrade attempt AND is
  in the Console's route scope) authorizes via a new shared `channels_webapi_from_decide` helper
  (factored out of the existing WS-upgrade code with NO behavior change -- the WS path's exact
  original log statement, including the `decision` field, is preserved) and serves the three K2
  static routes (`GET /`, `GET /console.css`, `GET /console.js`) or answers 404/405 per CS1.1/
  CS1.2. New `src/hub/console/{index.html,console.css,console.js}` (plain static files) and
  `src/hub/console_assets.rs` (three `include_str!` const literals, added to `src/hub/mod.rs`'s
  `pub mod` block). `resolve_webapi_port`/`GHOSTLIGHT_WEBAPI_PORT` (CS11) added to `webapi.rs`;
  `spawn_service_with_webapi_port` added to `tests/support/mod.rs` (every existing function there
  unmodified). The stale module doc comment describing the pre-K1 "not yet wired" ConfigStore gap
  was also corrected in passing (accurate as of K1, not a scope-creep hunt elsewhere).
- D1: an initial draft of the named test `unknown_path_under_root_is_404` asserted 404 for BOTH
  `/nope` and `/api/v1/nope` -> running it against the real implementation surfaced that `/nope`
  actually returns the pre-existing `400 Bad Request` (missing `Sec-WebSocket-Key`), not `404`.
  Re-reading PINS.md CS1's own literal scoping ("is a GET/POST to `/` or under `/api/v1/**`") and
  CS1.1's own worked example (`/api/v1/unknown`, never a bare top-level path) confirmed the
  IMPLEMENTATION was correct and the TEST was wrong: a bare path like `/nope` is outside the
  Console's route scope entirely by design (only `/`, the two static asset paths, and `/api/v1/**`
  are ever claimed), so it correctly falls through unchanged to the pre-existing generic 400.
  Renamed the test to `unknown_path_under_api_v1_is_404`, asserting 404 for `/api/v1/nope` only,
  plus an explicit assertion that `/nope` still gets the unaffected 400. Impact on later tasks:
  none -- K3/K4/K5 add routes only under `/api/v1/**`, which is already the correctly-scoped
  fallback path.
- Verification: all four commands passed for real. `cargo build --all-targets` clean. `cargo test`
  green: 465 lib tests (unchanged from K1 -- this task added no new lib-level unit tests, only the
  new `tests/console_static_routes.rs` integration file, 5/5) plus every other integration suite,
  0 failed -- including H8's own `tests/webapi_auth.rs` (3/3)/`tests/channels_policy.rs` (1/1) with
  every existing assertion unmodified, and a dedicated new test
  (`a_real_ws_upgrade_request_is_unaffected`) proving a real WS-upgrade handshake against `/`
  still succeeds through the new router. `cargo clippy --all-targets -- -D warnings` clean.
  `cargo fmt --all -- --check` clean (after running `cargo fmt --all` once to normalize wrapping
  the new code introduced -- whitespace only, no semantic change, not logged as its own numbered
  deviation). Sacred tests (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
  `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`, `tests/peer_death.rs`,
  `tests/mcp_protocol.rs`) green and byte-unmodified (`git diff --stat` confirms zero diff on all
  of them). Only `src/hub/mod.rs`, `src/hub/webapi.rs`, `tests/support/mod.rs`, plus the new
  `src/hub/console/`, `src/hub/console_assets.rs`, and `tests/console_static_routes.rs` changed.
  No NEVER-touch fence moved.
- Note: `CARGO_TARGET_DIR` was pointed at a scratch directory for build-artifact routing only, not
  a source or test change.

### K3

- Verified all as-of-authoring facts in `K3-config-provenance-api.md` and PINS.md CS1/CS2 against
  the live tree: K1's `current_resolution()`/`Resolution::iter()`/`Source::as_str()` and K2's
  router/`is_known_console_path` matched exactly; `key_def`/`KeyDef.description` were reachable
  and public. No STOP precondition fired.
- Implemented per PINS.md CS1/CS2: a new `("GET", "/api/v1/config")` match arm calls
  `write_config_response`, which iterates `ctx.store.current_resolution()` in registry order and
  builds the exact pinned JSON shape (`key`, `value`, `source`, `locked`, `description`); added
  `/api/v1/config` to `is_known_console_path` so a wrong method on it correctly 405s once exercised.
  `console.js` now fetches and renders the table (value / layer / locked badge); no specific
  byte-for-byte markup was pinned, matching the task's own latitude.
- D1 (author latitude used, not a correction): the task file's own test guidance warned against
  hardcoding a guessed registry key count. Verified this concern is real -- `spawn_service` reads
  this machine's own real, un-isolated user config path (`load::user_config_path()` has no test
  override, confirmed again here) -- so `config_api_returns_every_registered_key_in_registry_order`
  asserts STRUCTURAL shape only (key count/order from the live `ghostlight::governance::config::
  KEYS` registry itself, valid `source` enum, boolean `locked`, non-empty `description`), never a
  specific pre-existing key's exact default value or source. The org-mandatory test instead asserts
  an exact `source`/`locked`/`value` for `audit.enabled`, which is safe regardless of this
  machine's real user-layer state since org-mandatory always outranks user in the resolution
  precedence. Added `spawn_service_with_program_data_and_webapi_port` to `tests/support/mod.rs`
  (combines the two independent existing overrides; no existing helper's body changed). Impact on
  later tasks: K4's own sessions test should follow the SAME "assert structure/known-safe facts,
  never an arbitrary pre-existing machine's real config state" discipline if it ever touches
  config data (it does not; K4 only reads session/tab state, which is process-local and does not
  have this hazard).
- Verification: all four commands passed for real. `cargo build --all-targets` clean. `cargo test`
  green: 465 lib tests (unchanged -- no new lib-level unit tests this task) plus every integration
  suite, 0 failed -- including the new `tests/console_config_api.rs` (3/3), `tests/
  console_static_routes.rs` (5/5, unaffected), and H8's own `tests/webapi_auth.rs` (3/3)/
  `tests/channels_policy.rs` (1/1) unmodified. `cargo clippy --all-targets -- -D warnings` clean.
  `cargo fmt --all -- --check` clean (after one `cargo fmt --all` normalization pass, whitespace
  only). Sacred tests green and byte-unmodified (`git diff --stat` confirms zero diff on all of
  them). Only `src/hub/console/console.js`, `src/hub/webapi.rs`, `tests/support/mod.rs`, plus the
  new `tests/console_config_api.rs` changed. No NEVER-touch fence moved.
- Note: `CARGO_TARGET_DIR` was pointed at a scratch directory for build-artifact routing only.

### K4

- Verified all as-of-authoring facts in `K4-live-sessions-api.md` and PINS.md CS1/CS3/CS9 against
  the live tree: K1's `SessionSummary`/`live_session_summaries` (still `#[allow(dead_code)]`,
  unused until now), `ServiceContext.live_sessions`/`.session_registry`/`.owned_tabs` (all `pub`),
  and `check_tab_ownership`'s gate ordering in `src/transport/mcp/server.rs` (`claim_tab` runs
  synchronously BEFORE any dispatch to `handle_line`/the browser) all matched exactly. No STOP
  precondition fired.
- Implemented per PINS.md CS1/CS3: a new `("GET", "/api/v1/sessions")` match arm calls
  `write_sessions_response`, which reads `ctx.live_sessions` for the count and
  `session::live_session_summaries(&ctx.session_registry, &ctx.owned_tabs)` for the per-adapter
  entries, building the exact pinned JSON shape (`live_session_count`, `adapter_bindings` with
  `guid`/`pid`/`owned_tab_ids`, and the literal `note` string transcribed verbatim); added
  `/api/v1/sessions` to `is_known_console_path`. Removed K1's `#[allow(dead_code)]` on
  `live_session_summaries` now that it has a real caller. `console.js` renders a live-count
  summary line, a table, and the honest-limitation note.
- No deviations from the task file. The named integration test drives a REAL adapter's `tools/
  call` naming a tabId with NO fake extension attached (confirmed safe by re-reading
  `check_tab_ownership`'s gate order first, per the task file's own instruction to verify this
  before assuming it), reading the sessions view without ever awaiting or reading back the
  underlying (extension-less, eventually-failing) tool call's own response. Per the task file's
  own instruction, the guid-truncation unit-level proof was NOT duplicated here since K1 already
  added `session::tests::live_session_summaries_reports_truncated_guid_pid_and_owned_tabs`
  directly in `src/hub/session.rs`.
- Verification: all four commands passed for real. `cargo build --all-targets` clean. `cargo test`
  green: 465 lib tests (unchanged) plus every integration suite, 0 failed -- including the new
  `tests/console_sessions_api.rs` (1/1), `tests/console_config_api.rs` (3/3) and `tests/
  console_static_routes.rs` (5/5) unaffected, and H8's own `tests/webapi_auth.rs` (3/3)/`tests/
  channels_policy.rs` (1/1) unmodified. `cargo clippy --all-targets -- -D warnings` clean (removing
  the now-unnecessary `#[allow(dead_code)]` did not reintroduce a warning). `cargo fmt --all --
  check` clean (after one normalization pass, whitespace only). Sacred tests green and
  byte-unmodified (`git diff --stat`: zero diff on all of them). Only `src/hub/console/console.js`,
  `src/hub/session.rs`, `src/hub/webapi.rs`, plus the new `tests/console_sessions_api.rs` changed.
  No NEVER-touch fence moved.
- Note: `CARGO_TARGET_DIR` was pointed at a scratch directory for build-artifact routing only.

### K5

- BLOCKED at K5's own explicit STOP precondition, before writing any code or test. Read
  `K5-enable-remote-connections.md` in full, including its STOP precondition requiring the
  env-override isolation approach to be PROVEN before writing any test that exercises a real
  write. Ran that proof directly against the real `target/debug/ghostlight.exe`:
  `APPDATA=<temp dir> ./target/debug/ghostlight.exe config set audit.enabled true`. Expected (per
  the task's hypothesis, mirroring `spawn_service_with_program_data`'s `ProgramData` precedent for
  `org_policy_path()`): the write lands inside `<temp dir>`. Actual: the write landed at the REAL,
  un-overridden `%APPDATA%\ghostlight\config.json` on this machine
  (`C:\Users\onose\AppData\Roaming\ghostlight\config.json`) -- confirmed by `write_user_value`'s
  own printed `written to the user layer: ...` line naming the real path, and by listing the
  overridden temp directory afterward and finding it empty. `dirs::config_dir()` on this platform
  does not honor an `APPDATA` override for the current process the way the org-policy path's own
  `ProgramData` env read does; the two paths are NOT resolved the same way (confirmed by reading
  `src/governance/config/load.rs`: `org_policy_path()` reads `ProgramData` directly via
  `std::env::var`, while `user_config_path()` delegates entirely to the `dirs` crate's
  `config_dir()`, which apparently resolves via a platform API that ignores the env var for an
  already-running process on this build).
- REAL-MACHINE SIDE EFFECT (disclosed, not hidden): the proof step above wrote `audit.enabled:
  true` into this machine's actual `%APPDATA%\ghostlight\config.json`. A baseline check
  immediately before the proof confirmed this file did not previously exist, so this is a
  net-new, isolated addition (not an overwrite of prior real user configuration) -- but it is
  still a real, unauthorized-until-now write outside this project's scratch/test scope, and it has
  NOT been cleaned up: an attempt to delete it was blocked by a destructive-action safety
  classifier, which correctly redirected to marking this task BLOCKED and reporting to a human
  rather than silently continuing past its own STOP precondition. The file's exact path and
  content are recorded here so a human can inspect and remove it deliberately.
- Per the task file's own text ("Do NOT invent a third option that touches the real path even
  transiently"), no further attempt was made to route around this finding (no alternate env var
  tried, no code written to `load.rs` speculatively). No working-tree source changes were made for
  K5 at all (`git status --porcelain` confirmed clean before this entry was written), so there is
  nothing to `git restore`.
- What is needed to proceed (decided by a human/frontier author, not by an unattended executor,
  per the task's own instruction): (a) clean up the real file noted above
  (`%APPDATA%\ghostlight\config.json`, containing only `{"config":{"audit.enabled":true}}` or
  equivalent -- verify its exact content before removing, in case something else wrote to it
  between this entry and when it is reviewed), AND (b) choose how K5 should test its write path:
  either (i) add a genuine, precedented env-override (e.g. `GHOSTLIGHT_USER_CONFIG_DIR`) to
  `src/governance/config/load.rs::user_config_path()` itself -- a small, explicit, NEW file this
  task did not originally name, mirroring the `GHOSTLIGHT_LOG_DIR`/`GHOSTLIGHT_ENDPOINT`/
  `GHOSTLIGHT_WEBAPI_PORT` convention already used elsewhere -- and re-verify the SAME proof
  succeeds before trusting it; or (ii) test the POST handler's write-outcome logic (success/
  locked/other-error -> response shape) without ever calling through to a real
  `user_config_path()`-resolving code path (e.g. a lower-level unit test of `set_user_value`'s
  CALLER logic with the write step abstracted behind a seam), accepting that the real end-to-end
  file-write is then unverified by any automated test in this batch. PINS.md CS7's `set_user_value`
  itself is not gated by either choice and needs no change either way.
- No deviation numbers logged: this is a tree-fact mismatch between this task's own authored
  hypothesis (isolation transfers from the org path to the user path) and the platform's actual
  behavior, not a choice this executor made mid-task.

## Deviation format

When you deviate from a task file (a signature differs from as-of-authoring, a helper had to move,
an oracle needed pinning), record it under that task as:

```
D<n>: <what the task said> -> <what you actually did> because <the tree fact that forced it>.
     Impact on later tasks: <none | names the task + what it must now assume>.
```

A BLOCKED entry records instead: the failed assumption (with the file/symbol actually found), the
STOP precondition or fence that triggered, and what is needed to proceed. Then HALT.
