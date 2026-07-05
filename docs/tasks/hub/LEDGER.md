# Ghostlight Hub batch: LEDGER

Durable progress for the Hub batch (ADR-0030). One task = one commit. Update this file at the end of
every task, per BOOTSTRAP step 8. This is the single source of truth for "where are we"; a fresh
executor resumes from RESUME HERE with no other context.

## RESUME HERE

**Next task: H6 (`H6-*.md`, detached non-admin lifecycle + anti-squat).** H0 landed (pure
code move; `src/hub` composition root extracted). H1 landed (transport-generic `serve_session<S>`
+ `ServiceContext`, byte-identical single-session refactor). H2 landed (persistent SERVICE + thin
ADAPTER + genuine multiplex over the amended two-endpoint design; the kill-hook fan-out; ADR-0004
repealed at the MCP-client layer). H3 landed on its RE-ISSUED run (see the H3 Log entry for the
prior BLOCKED attempt's provenance): `src/hub/session.rs` (`SessionGuid`, `PeerCred`, `PeerUser`,
`SessionRegistry`, `Admission`) and `src/hub/role.rs` (the process role marker + fail-loud
chokepoint assertion) are new; `ServiceContext` gained a `session_registry` field;
`handle_adapter_connection`/`serve_adapters` (`src/transport/native/ipc.rs`) now capture the real
peer OS credential, parse and admit the presented GUID, and refuse cleanly on a malformed/foreign
one; `relay_adapter` mints a real GUID instead of the old `""` placeholder; `serve_session` takes a
plain `guid: SessionGuid` (never `Option`) for every session, including the service's own lone
stdio session; the dead `server::run()` 2-arg wrapper is deleted; `tests/architecture.rs`'s a7
scanner is EXTENDED (the sanctioned H3 edit) to also reject bare `tabId`/`token`/`socket`
identifiers in `src/governance/**`, scoped to code lines (see the H3 Log's D3 for why). H4 landed:
`ServiceContext` gained an `owned_tabs: Arc<Mutex<HashMap<i64, SessionGuid>>>` field
(`src/hub/mod.rs`); `src/hub/session.rs` gained the pure `owns_or_adopts_tab` map operation;
`src/transport/mcp/server.rs`'s `serve_session` read loop now runs a NEW `check_tab_ownership`
pre-dispatch gate (ahead of `handle_line`/`pipeline::handle_tools_call`) that refuses a `tools/call`
naming a `tabId` a DIFFERENT session already owns with the uniform `"unknown tab"` text, recorded as
a deny via `Governance::begin`/`CallAudit::sacred_deny` (`domain: null`, `held: false`,
`duration_ms: 0`, rule `cross_session/unowned_tab`); `pipeline.rs` and `src/governance/**` are
UNTOUCHED. New `tests/hub_isolation.rs` (2 tests, both green). See the H4 Log for 3 deviations
(D1: `tabs_create_mcp`-response adoption not implemented, no pinned extraction oracle exists --
first-touch adoption alone covers the realistic case; D2: the test's "nonexistent" tabId is
pre-seeded as owned by A rather than left absent from the map, since a genuinely absent tabId is
first-touch-ADOPTED, not refused, under the pinned mechanism; D3: the test drives `serve_session`
directly over an in-process `tokio::io::duplex`, constructing `ServiceContext` via its own pub
fields rather than spawning the real binary, and must set the `hub::role` marker itself). H5
landed (orthogonal after H2): `transport::executor::Browser::attach` now holds pending calls
across a bounded `GRACE_WINDOW` (10s, PINS.md SS4) instead of draining them the instant the
extension stream closes (`Browser::spawn_grace_drain`, spawned so `attach` still returns
`Detached` promptly); only a REAL drop (the window elapsing with no reconnect) drains pending,
with the byte-identical disconnect error text. `ServiceContext` gained a `mint_quota` field
(`src/hub/mod.rs`); `try_mint`/`MintGuard`/`PER_PEER_MINT_CAP`/`PER_PEER_GROUP_CAP`/
`MINT_QUOTA_EXCEEDED` (`src/hub/mod.rs`) implement the per-peer (never global), RAII-scoped mint
quota, wired into `handle_adapter_connection` (`src/transport/native/ipc.rs`) ahead of
`SessionRegistry::admit`. `transport::mcp::server`'s writer task now relays a reply through the
new `write_chunked` (chunked at `SCREENSHOT_CHUNK_THRESHOLD` = 8 MiB with a yield between chunks)
instead of one unconditional `write_all`. Module-doc bottleneck note added to `src/hub/mod.rs`;
a cross-reference amendment note added to `docs/adr/0004-reject-second-session.md` (Status and
the retained single-physical-extension-link invariant left untouched). New `tests/hub_queue.rs`
(2 tests, both named by the task, both green) plus 2 supplementary (not task-named) tests in
`src/transport/executor.rs`'s own test module validating the grace-hold/real-drop mechanics. See
the H5 Log for 2 deviations (D1: a forced one-line `mint_quota` field addition to
`tests/hub_isolation.rs`'s `build_ctx`, a file H5 does not name, required for the tree to compile
once `ServiceContext` gained the field; D2: the grace-window mechanism has no task-named test, so
2 supplementary tests were added directly in `executor.rs`, transcribing only already-pinned
literals). RE-READ H6's task file plus PINS.md SS5 (idle-grace, anti-squat) before starting.
Follow the per-task procedure in `BOOTSTRAP.md`.

**H6 is now BLOCKED (see the H6 Log entry below) on a genuine requirements conflict discovered
before any code was written**: ADR-0030 Decision 8's detached, job-breakaway-verified SERVICE
process is structurally incompatible with `tests/peer_death.rs::native_host_exits_when_server_dies`
staying green unmodified, per this task's own "Keep green (do not modify)" list. The frontier
author must reconcile the two (see the H6 Log's "What is needed to proceed") before H6 can be
re-issued and re-attempted. No working-tree changes were made or reverted.

## Status

| Task | Title | Status | Commit | Notes |
| --- | --- | --- | --- | --- |
| H0 | Extract the HubCore composition root | DONE | a4e87b6 | |
| H1 | Transport-generic serve_session + ServiceContext | DONE | 4463b07 | |
| H2 | Persistent service + thin adapter + multiplex | DONE | 96a54fb | landed on the RE-ISSUED, two-endpoint-amended task; prior BLOCKED attempt superseded, see Log |
| H3 | Adapter-minted GUID identity + peer-cred binding | DONE | 81b3bea | RE-ISSUED after PINS.md SS9 fix; prior BLOCKED attempt superseded, see Log |
| H4 | Binary-authoritative cross-session tab isolation | DONE | 1490951 | |
| H5 | Reconnect grace window + honest bounded queue | DONE | 33b361d | |
| H6 | Detached non-admin lifecycle + anti-squat | BLOCKED | -- | conflicts with tests/peer_death.rs; see Log |
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
- Verified all as-of-authoring facts in `H1-serve-session-generic.md` against the live tree:
  `mcp::server::run` (lines 108-301, matching the task's line ranges within a few lines),
  `pipeline::handle_tools_call`'s signature (line 50, byte-identical to the task's quote), the
  `src/main.rs` call site (now `ghostlight::hub::run_mcp_server`, which itself calls
  `crate::mcp::server::run(browser, loaded_policy, user_source)` unchanged), and `LoadedPolicy`'s
  `#[derive(Debug, Clone, PartialEq)]`. All matched; no STOP precondition fired.
- D1: the STOP precondition reads "If `src/hub/mod.rs` does not exist or does not host `HubCore`,
  STOP." -> Re-read `H0-extract-hubcore.md` (the higher-priority per-task file for H0) and found
  its own "Required behavior" never mandates a literal Rust type/struct named `HubCore`: it only
  requires `pub fn run_mcp_server(...)` and `pub fn build_debug_sink(...)` inside `src/hub/mod.rs`,
  which is exactly what H0 landed (a4e87b6) and what the live tree contains today. `HubCore` is
  ADR-0030 Decision 2's and this task's own conceptual label for "the module hosting the
  composition root" (the module's doc comment self-identifies as that seam, citing Decision 2 by
  name), not a pinned identifier. Proceeded treating the existing `src/hub/mod.rs` (composition
  root present, doc-commented as the ServiceContext-attachment seam) as satisfying the
  precondition's substantive check, because reading it literally (no file/type may ever be named
  `HubCore`) would make the precondition permanently un-satisfiable even by H0 done correctly to
  its own letter -- which cannot be the intent of a linear, executable batch. Impact on later
  tasks: none functionally; H2/H3/H4/H5/H6/H8 task files use the same "hosts HubCore" phrasing to
  mean this module, and none of their own "Required behavior" sections require a literal `HubCore`
  struct either -- a future executor should read their STOP preconditions the same way (module
  presence + composition-root content, not a literal type name).
- Implemented per the task's exact prescriptions: added `ServiceContext` (fields `browser: Browser,
  store: Arc<ConfigStore>, recorder: Arc<Recorder>, initial_policy: LoadedPolicy`) and
  `ServiceContext::from_startup(browser, loaded_policy, user_source) -> crate::Result<Self>` to
  `src/hub/mod.rs`, moving the shared-lifetime setup (store load -> `spawn_watcher` -> recorder
  build -> recorder-reload subscription spawn), verbatim, out of `server::run`. Added
  `serve_session<S>(stream: S, ctx: ServiceContext) -> Result<()>` to
  `src/transport/mcp/server.rs`, moving the per-session setup (governance build, kill hook, writer
  task now writing to the split `write_half` instead of `tokio::io::stdout()`, policy-subscription
  task, read loop over `BufReader::new(read_half)`, ordered teardown), verbatim except for the
  stdout/stdin substitution the task itself specifies. `run` is now the thin wrapper:
  `ServiceContext::from_startup(...)` + `tokio::io::join(stdin, stdout)` + `serve_session(...)`,
  byte-identical signature, so `src/main.rs` (which calls `hub::run_mcp_server`, itself calling
  `mcp::server::run`) needed no edit.
- D2: mechanical import cleanup forced by the move, not called out by name in the task's "Imports
  today" note -> removed `use crate::governance::audit::Recorder;` and narrowed
  `use crate::browser::{advertise, pattern, polarity};` to `use crate::browser::{advertise,
  polarity};` in `server.rs` (both became unused once `Recorder::from_config` and
  `pattern::is_valid_pattern` moved into `ServiceContext::from_startup`); added `use
  crate::governance::audit::Recorder;`, `use crate::governance::config::reload::ConfigStore;`, `use
  crate::governance::manifest::source::LoadedPolicy;`, and `use std::sync::Arc;` to `src/hub/mod.rs`
  for the new struct/fn. Required for `cargo clippy --all-targets -- -D warnings` to pass (unused
  imports are hard errors under `-D warnings`). Impact on later tasks: none -- purely mechanical,
  covered by "the executor transcribes the mechanical relocation and import re-homing" latitude the
  task file itself grants for H0-style moves.
- OPTIONAL seam test (`serve_session_over_duplex_matches_stdio_initialize_reply`) SKIPPED per the
  task's own instruction ("SKIP it rather than improvise -- it is not required for the commit to be
  complete"); the kept-green suites (`tests/all_open_golden.rs`, `tests/mcp_protocol.rs`) already
  exercise `serve_session` over the real stdin/stdout join.
- All four verification commands passed for real: `cargo build --all-targets`; `cargo test` (423
  lib tests + every named integration suite -- `all_open_golden` 3/3,
  `architecture::governance_core_has_no_forbidden_back_edges` green, `audit_recorder` 2/2,
  `hot_reload` 1/1, `mcp_protocol` 6/6, `tool_schema_fidelity` 7/7, plus every other existing suite,
  all green); `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --all -- --check` clean
  (no reformatting needed). Sacred tests (`tests/tool_schema_fidelity.rs`,
  `tests/all_open_golden.rs`, `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`)
  green and byte-unmodified (`git diff --stat` shows only `src/hub/mod.rs` and
  `src/transport/mcp/server.rs` changed). No NEVER-touch fence moved.
- Note: as in H0, `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's `target/`)
  because a live `ghostlight.exe` held `target/debug/ghostlight.exe` locked on Windows (`Access is
  denied. (os error 5)` on the first build attempt); build-artifact routing only, not a source or
  test change.

### H2
- RE-ISSUED 2026-07-04 (frontier author). The BLOCKED entry below stands as provenance. Resolution:
  the design was AMENDED, not patched. ADR-0030 Decision 1 now specifies TWO local endpoints (a
  hello-free EXTENSION endpoint + an ADAPTER/CONTROL session-hello endpoint) instead of one
  role-demuxed endpoint; `ROLE_EXT` is deleted; the extension endpoint keeps its exact
  server-speaks-first contract so `tests/all_open_golden.rs` and `tests/mcp_protocol.rs` pass
  UNMODIFIED. PINS.md SS1 was rewritten, and H2 + H3 re-authored, to match. The first BLOCKED attempt
  was the golden test doing its job (it faithfully encodes the extension's spoken-to contract), not a
  stale double. Chosen over the three stopgaps the executor listed because those either edited a
  sacred file, invented an unpinned timeout, or bolted a second endpoint on WITHOUT removing the
  role-demux -- the amendment removes the role discriminator entirely (fewer, more meaningful parts).
- BLOCKED. Implemented the task in full (`src/hub/handshake.rs` new; `src/transport/native/ipc.rs`
  split `serve` into `claim_endpoint`/`serve_claimed` + added `relay_adapter` + a shared
  `handle_connection` hello-demux per PINS.md SS1; `src/transport/executor.rs` converted the
  single-consumer kill hook to the `kill_hooks`/`KillHookHandle` fan-out registry per Decision 7;
  `src/transport/mcp/server.rs` swapped `on_session_killed` for `register_session_kill_hook` in
  `serve_session`; `src/hub/mod.rs` rewired `run_mcp_server` to claim-or-adapt; new
  `tests/hub_multiplex.rs` with both named tests passing). `cargo build --all-targets` was clean
  and the new `tests/hub_multiplex.rs` (`two_sessions_route_replies_independently`,
  `one_kill_emits_one_audit_record_per_live_session`) passed. Then the task's own verification
  block (`cargo test --test mcp_protocol --test peer_death --test all_open_golden --test
  tool_schema_fidelity --test audit_recorder --test architecture`) surfaced a real, reproducible
  failure in a file this task's own NEVER-touch fence forbids editing, with no exception:
  `tests/all_open_golden.rs::read_page_redaction_is_still_wired_at_the_chokepoint` failed with
  `"[hop: extension] Browser extension not connected"` instead of succeeding. Root cause, traced
  and confirmed (not a hunch): the task's Required Behavior item 1 (PINS.md SS1) requires `serve`
  / `serve_claimed` to read the hub hello frame FIRST and demux BEFORE dispatching to
  `Browser::attach` (`"ext"`) or `serve_session` (`"adapter"`), with "an unknown or absent role
  fails the connection cleanly." `Browser::is_connected()` only becomes `true` INSIDE `attach()`,
  which under this design cannot run until a hello has been read from the peer. But
  `read_page_redaction_is_still_wired_at_the_chokepoint`'s fake extension (and
  `tests/mcp_protocol.rs::tools_call_waits_for_a_late_extension_and_notes_the_wait`'s, structurally
  identical -- confirmed failing the same way, though that file is only in the softer "Keep green"
  list, not the hard NEVER-touch one) connects via `ipc::connect` and calls
  `host::read_message` BEFORE ever writing anything -- it relies on the PRE-H2 behavior where the
  mcp-server can start writing a queued `tools/call`'s framed `tool_request` to a freshly accepted
  connection the instant `Browser::attach` claims it, with zero bytes required from the peer
  first. Under the hello-first gate, `attach()` never runs (the peer never sends first), so the
  pending `read_page` call's bounded `wait_connected(first_call_wait_ms, default 5000ms --
  src/governance/config/mod.rs `ENGINE_CONNECTION_FIRST_CALL_WAIT_MS`) window elapses with the
  extension never marked connected, and `pipeline::handle_tools_call` (src/transport/mcp/pipeline.rs:206)
  then calls `browser.call()` anyway, which fails fast with the exact "not connected" message
  observed -- matching the test's ~5s runtime exactly. This is a genuine, hello-first vs.
  receive-first-peer deadlock, not a coding mistake: I confirmed it by actually implementing the
  task, running the exact verification commands the task names, and tracing the failure to its
  root cause (both fake-extension tests reproduce it identically). I considered and rejected three
  workarounds because each either touches the NEVER-touch fence or invents an unpinned value: (a)
  editing the two tests' fake-extension helpers to send `{"hub":1,"role":"ext"}` first --
  forbidden for `tests/all_open_golden.rs` ("No exception"); (b) a bounded-timeout peek that
  defaults an as-yet-silent connection to `"ext"` -- contradicts the task's literal "absent role
  fails the connection cleanly" and requires inventing a timeout constant that is not pinned
  anywhere in PINS.md (the ORACLE RULE forbids deriving one); (c) a second, adapter-only endpoint
  so the original endpoint's `"ext"` path needs no hello at all -- deviates from PINS.md SS1's
  explicitly PINNED single-endpoint, hello-demuxed design, which is normative and cited, not mine
  to re-derive. Per BOOTSTRAP's Failure protocol ("a never-touch fence would have to move" /
  "verification cannot go green without violating a rule"), I reverted every H2 working-tree
  change (`git restore` on the four modified files; deleted the two new files) back to the clean
  H1 baseline, re-ran the sacred/named suite there to confirm it is green and byte-unmodified
  (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
  `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`, plus `mcp_protocol`,
  `peer_death`, `audit_recorder` -- all pass), and HALTED without attempting H2 again or any later
  task.
- RESOLVED (see the RE-ISSUED note at the top of this H2 section): the frontier author chose a variant
  of option (iii) -- a full two-endpoint split that DELETES the role-demux entirely, not merely a
  second endpoint bolted beside it. Options (i) and (ii) below were REJECTED (they edit a sacred file
  or invent an unpinned timeout) and the `role:"ext"` strings in them are historical, not live.
- What is needed to proceed (any one, decided by the frontier author, not by this executor):
  (i) amend `tests/all_open_golden.rs` (and likely `tests/mcp_protocol.rs`'s
  `tools_call_waits_for_a_late_extension_and_notes_the_wait`) to send the `{"hub":1,"role":"ext"}`
  hello from their fake-extension harness before their first read, and explicitly lift the
  NEVER-touch fence for that one mechanical accommodation; or (ii) re-pin the hello mechanism with
  an explicit, named, pinned sequencing rule for this exact race (e.g. a pinned bounded timeout
  after which an as-yet-silent connection defaults to `"ext"`, with the exact duration and
  fallback semantics stated in PINS.md so it is transcribed, not invented); or (iii) redesign the
  demux so it does not require a blocking pre-read gate on the shared endpoint (e.g. a second,
  adapter-only endpoint), with PINS.md SS1 and this task file re-authored to match. No deviation
  numbers logged (the implementation matched the task's Required Behavior to the letter; the
  conflict is between two of the task's own requirements, not a tree-fact mismatch this executor
  introduced).

**RE-ISSUED RUN (2026-07-04, DONE).** Verified all as-of-authoring facts in the re-authored
`H2-service-adapter-multiplex.md` and the amended `PINS.md` SS1 against the live tree: `src/hub`
and `HubCore`-equivalent composition root present (H0), `serve_session<S>(stream, ctx)` +
`ServiceContext` present (H1) with `next_id`/`pending` shared `Arc` fields on `Browser` confirmed
at their as-of-authoring locations, `on_session_killed`'s single-consumer replace doc confirmed
still in force, `Browser::attach`'s `AttachOutcome::AlreadyAttached` confirmed unchanged, and no
`run_server` in `main.rs` (H0 already moved it). No STOP precondition fired.

Implemented per the re-authored task + PINS.md SS1's two-endpoint split:
- `src/hub/handshake.rs` (new): `HUB_PROTO = 1`, `ROLE_ADAPTER = "adapter"`, `ROLE_CONTROL =
  "control"` -- no `ROLE_EXT`, per the amendment.
- `src/transport/native/ipc.rs`: added `adapter_endpoint_name` (base name + literal `-adapter`
  suffix, wrapped by the same `pipe_path`/`socket_path` helper); `AdapterListener` (cfg-split type
  alias, no unified `Listener` type); `claim_adapter_endpoint` (cfg-split, same bind-with-stale-heal
  `serve` already does, PINS.md SS1 pin 1); `serve_adapters(ctx, listener)` (accept-ahead +
  spawn-per-connection on the ALREADY-claimed listener, never re-claiming the name); the shared
  `handle_adapter_connection` (reads the framed hello INSIDE the spawned task via
  `host::read_message`, demuxes `"adapter"` into `transport::mcp::server::serve_session`,
  `"control"` cleanly refused, unknown/absent role refused, never a panic); `relay_adapter`
  (dials the adapter/control endpoint, sends the framed `{"hub":1,"role":"adapter","guid":""}`
  hello, then a RAW `tokio::io::copy` bidirectional relay -- PINS.md SS1 pin 3 -- mirroring
  `relay_native_host`'s lifecycle shape only, never its framing). The EXTENSION endpoint's `serve`,
  `connect`, `relay_native_host`, and every fake-extension test double are byte-for-byte unchanged.
- `src/transport/executor.rs` (the one sanctioned executor change, ADR-0030 Decision 7): replaced
  the single `kill_hook: Arc<Mutex<Option<KillHook>>>` with a `kill_hooks: Arc<Mutex<Vec<(u64,
  KillHook)>>>` fan-out registry plus `next_hook_id`; `on_session_killed` now APPENDS a permanent
  hook (doc comment updated from "replaces the first" to append semantics); added
  `register_session_kill_hook` returning a `#[must_use]` `KillHookHandle` whose `Drop` removes
  exactly its own entry; `handle_session_killed` now invokes every registered hook once per
  false->true transition. `Browser::attach`'s single-physical-link rejection is untouched.
- `src/transport/mcp/server.rs`: `serve_session`'s kill-hook registration swapped from
  `on_session_killed` to `register_session_kill_hook`, held as `_kill_handle` for the whole
  function body (session-scoped, deregisters on session end; `hold`/`killed`/`connected` stay
  global on the one shared `Browser`).
- `src/hub/mod.rs`: `ServiceContext` now `#[derive(Clone)]` (PINS.md SS1 pin 4; built ONCE via
  `from_startup`, cloned per session, never re-run per session). `run_mcp_server` now calls
  `ipc::claim_adapter_endpoint` FIRST; on win, `run_as_service` builds the `Browser`, spawns the
  UNCHANGED extension `ipc::serve`, builds the shared `ServiceContext` once, spawns
  `ipc::serve_adapters` over the already-claimed listener, and serves this process's own stdio as
  the first session over the shared context (byte-identical lone-client extension path); on loss
  (`Error::SessionBusy` from the adapter/control claim), `run_as_adapter` runs
  `ipc::relay_adapter` instead of the old reject-2nd degrade-and-continue arm, which no longer
  exists in this path (the loser never reaches the extension `serve` call at all).
- `tests/hub_multiplex.rs` (new): `two_sessions_route_replies_independently` (two `Browser::call`
  callers standing in for two sessions, per the task's own sanctioned lower-level alternative,
  share one `Browser`/one fake extension; asserts neither ever receives the other's reply);
  `one_kill_emits_one_audit_record_per_live_session` (three all-open `Governance`s with distinct
  client names, three file-backed `Recorder`s, one shared `Browser`; asserts exactly 3
  `session_killed` records, each with the 6-key `SessionEventRecord` order transcribed verbatim
  from ADR-0030's pinned oracle, each `client.name` matching its own session);
  `adapter_endpoint_two_phase_wire_round_trips` (spawns the real binary, connects to
  `<endpoint>-adapter` via `ipc::connect`, sends the framed hello then a RAW newline JSON-RPC
  `initialize` line, asserts a RAW newline-delimited reply with `id == 1` comes back -- fencing the
  PINS.md SS1 pin 3 framing trap).

D1: PINS.md SS1's "Pinned name: `ipc::relay_adapter(endpoint: &str, debug: &crate::debug::DebugSink)
    -> Result<()>` (the `endpoint` passed is the ADAPTER/CONTROL endpoint, not the extension
    endpoint)" -> implemented `relay_adapter` to take the SAME plain BASE endpoint every other
    call site threads (`ipc::default_endpoint()`), computing the `-adapter` suffix internally via
    the same `adapter_endpoint_name()` helper `claim_adapter_endpoint`/`serve_adapters` use, rather
    than requiring the caller to pre-suffix the argument -- because PINS.md SS1's own naming pin
    ("wrapped by the SAME `pipe_path`/socket-path helper") centralizes the derivation in one place,
    and every sibling adapter/control function already takes the base endpoint and suffixes
    internally; making `relay_adapter` alone expect a pre-suffixed argument would be an
    inconsistent, easy-to-misuse convention that no pinned test distinguishes from this reading
    either way (the resulting wire bytes and endpoint paths are identical). Impact on later tasks:
    none -- H6's spawn-on-demand call site should keep passing the plain base endpoint to
    `relay_adapter`, exactly as H2's own `run_as_adapter` does.
D2: the task's prose names the new acceptor `ipc::serve_adapters(ctx, listener)` (two arguments)
    -> implemented exactly that two-argument signature on both platforms (an earlier draft added a
    third `endpoint: &str` parameter so the Windows accept-ahead loop could re-create pipe
    instances, then was simplified to re-derive the same path via `default_endpoint()` internally,
    since that is already the single source of truth for the process's one endpoint name) --
    because the task's own text is closer to a two-argument shape than the explicitly-labeled
    "Pinned name:" bullet is for `relay_adapter`, and re-deriving avoids threading an extra
    parameter through every call site for no behavioral difference. Impact on later tasks: none --
    H6/H8 call sites should keep calling `ipc::serve_adapters(ctx, listener)` with no endpoint
    argument.

Verification: all four commands passed for real. `cargo build --all-targets` clean.
`cargo test --test hub_multiplex --test mcp_protocol --test peer_death --test all_open_golden
--test tool_schema_fidelity --test audit_recorder --test architecture` all green (26 tests across
the seven suites); `cargo test -p ghostlight --lib executor` green (17/17, including
`kill_hook_fires_exactly_once_per_transition` and
`a_second_attach_is_rejected_without_disturbing_the_live_session`); the full `cargo test` is green
(423 lib tests + every integration suite, 0 failed). `cargo clippy --all-targets -- -D warnings`
clean. `cargo fmt --all -- --check` clean (after running `cargo fmt --all` twice to normalize
wrapping introduced by the edits and by the D2 simplification -- whitespace only, no semantic
change, not logged as its own numbered deviation). Sacred tests
(`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
`tests/architecture.rs::governance_core_has_no_forbidden_back_edges`) green and byte-unmodified;
`git diff --stat` shows only `src/hub/mod.rs`, `src/transport/executor.rs`,
`src/transport/mcp/server.rs`, `src/transport/native/ipc.rs` modified plus the two new files
(`src/hub/handshake.rs`, `tests/hub_multiplex.rs`). No NEVER-touch fence moved; the sanctioned
kill-hook-fan-out exception to the executor fence, and the sanctioned two-endpoint-split scoping of
the extension fence, are the only fences touched, both as pinned.
- Note: as in H0/H1, `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's
  `target/`) because three live `ghostlight.exe` processes (this environment's own dogfooded
  MCP/native-host session) held the repo's `target/debug/ghostlight.exe`; build-artifact routing
  only, not a source or test change.

### H3
- RE-ISSUED 2026-07-04 (frontier author). The BLOCKED entry below stands as provenance. Resolution:
  PINS.md SS9 pins the corrected architecture (accept/admission in `ipc.rs`, not `src/hub/mod.rs`;
  `ServiceContext` gains `session_registry`/`owned_tabs`/quota fields as siblings; `serve_session`
  gains a plain `guid: SessionGuid`, not `Option`). H3, H4, H5, H7, H8 were all re-authored to match
  (H4/H5/H7/H8 shared the exact same stale assumption and would have blocked in turn). Two further
  fresh-eyes passes against the live H2 code closed 7 more gaps (derives, dead code, the
  `relay_adapter` placeholder guid, H5's chunking mechanism, H8's admission model, a guid
  parse-failure path). See commits `9402312`-adjacent amendment history and `18746aa` for the full
  fix. Chosen over re-deriving each task's location independently, to guarantee cross-file pin
  agreement rather than risk 4 more independently-worded (and possibly inconsistent) corrections.
- BLOCKED at the per-task procedure's step 2 (RE-READ every source file the task names; verify
  each as-of-authoring fact), before writing any test or implementation code -- no working-tree
  changes exist to revert.
- Re-read the task's "Current-tree facts" bullet 1 verbatim: "`src/hub/` is created by H0-H2 (the
  composition root + `ServiceContext` + per-session state + `serve_session<S>(stream, ctx)` + the
  multiplex accept loop) ... this task adds a `guid` field to H2's per-session record and hooks the
  accept path; it does NOT invent the session record." And Required Behavior item 2: "The real OS
  capture (Windows `GetNamedPipeClientProcessId` + token SID; Unix `SO_PEERCRED` / `getpeereid`)
  happens in the accept path in `src/hub/mod.rs` on the raw pipe/UDS handle H2 already owns." And
  item 3 ("Service routing"): "In `src/hub/mod.rs`, after H2's handshake reads the presented GUID
  and the accept layer captures the `PeerCred`, call `SessionRegistry::admit` ... On `Admitted`,
  key the H2 per-session record (its `Governance` facade + owned-handle set) by the GUID's
  canonical string."
- Verified against the live tree: `src/hub/` currently contains exactly two files, `handshake.rs`
  and `mod.rs` (confirmed via directory listing). `src/hub/mod.rs` (H2's actual landed shape) holds
  only `run_mcp_server`, `run_as_service`, `run_as_adapter`, `build_debug_sink`, and
  `ServiceContext` -- `run_as_service` never itself loops over connections or touches a raw
  platform handle; it builds the `Browser`/`ServiceContext` once and SPAWNS
  `ipc::serve_adapters(ctx, adapter_listener)`, a function living in
  `src/transport/native/ipc.rs`, not `src/hub`. The actual ADAPTER/CONTROL accept loop, the
  session-hello read, and the concrete platform types (`AdapterListener` = `NamedPipeServer` on
  Windows / `UnixListener` + the accepted `UnixStream` on Unix) live entirely inside
  `src/transport/native/ipc.rs`'s `serve_adapters`/`handle_adapter_connection`; by the time
  `handle_adapter_connection` runs, the stream is already type-erased to a generic
  `S: AsyncRead + AsyncWrite + Send + Unpin + 'static` (its own signature) -- the concrete OS
  handle (`GetNamedPipeClientProcessId`/`SO_PEERCRED`-capable) is only reachable at the call sites
  inside `serve_adapters`, before that erasure, never from anything `src/hub` owns. There is also
  NO per-session record type anywhere in the tree (grepped `SessionGuid|PeerCred|owned_handle|
  SessionRecord` across all of `src/`: zero matches) holding "the `Governance` facade + owned-handle
  set" for item 3 to key by GUID -- `serve_session` (`src/transport/mcp/server.rs`) builds its
  per-session `Arc<Mutex<Arc<Governance>>>` as a local variable inside the function body, not as a
  `src/hub`-owned record H3 could add a `guid` field to.
- STOP precondition triggered (transcribed verbatim from `H3-session-identity-guid.md`): "If the
  accept layer in `src/hub` has NO access to the connecting peer's raw pipe/UDS handle to read its
  OS credential, STOP -- the peer-cred capture seam belongs to the transport/accept in `src/hub`;
  build it there, never by reaching into `src/governance`." Per BOOTSTRAP's authority order item 4
  ("If the tree contradicts a task's load-bearing assumption, follow that task's STOP precondition;
  do NOT improvise around it") and the per-task procedure's "keep the change inside the files the
  task names", satisfying Required Behavior items 2/3 as written would require either editing
  `src/transport/native/ipc.rs` (a file this task does not name, and the only file where the
  concrete platform handle and the real accept loop actually exist) or inventing a per-session
  record type the task explicitly says H3 must not invent ("it does NOT invent the session
  record"). Neither is a sanctioned move under this task's scope, so no code was written and
  nothing was reverted.
- What is needed to proceed (decided by the frontier author, not by this executor): re-author H3
  (or insert a small intermediate step) to reconcile with H2's ACTUAL landed shape -- either (i)
  name `src/transport/native/ipc.rs` as an in-scope file for the peer-credential capture (e.g.
  capture `PeerCred` inside `serve_adapters` immediately after `listener.accept()` /
  `server.connect()`, where the concrete `UnixStream`/`NamedPipeServer` handle is still live,
  thread it into `handle_adapter_connection`, and call `SessionRegistry::admit` there before
  dispatching to `transport::mcp::server::serve_session`); or (ii) explicitly defer the live wiring
  (item 2's OS-capture code and item 3's routing/keying) to whichever task first introduces a
  per-session record type (H4, which builds the owned-handle set), re-scoping H3 itself to the
  pure `SessionGuid`/`PeerCred`/`SessionRegistry` types plus the role marker (item 6) and the a7
  scanner extension (item 5) -- with PINS.md and the task file updated to say so explicitly, so a
  future executor does not re-hit this same STOP. No deviation numbers logged: this is a tree-fact
  mismatch in the task's own authoring assumptions (H2 was re-authored 2026-07-04 for the
  two-endpoint split after H3 was first drafted), not a choice this executor made.

**RE-ISSUED RUN (2026-07-04, DONE).** Verified all as-of-authoring facts in the re-authored
`H3-session-identity-guid.md` and PINS.md SS1/SS8/SS9 against the live tree: `src/hub/` held
exactly `handshake.rs` + `mod.rs` (no accept loop, no per-session record type, no
`SessionGuid`/`PeerCred`/`SessionRegistry` anywhere -- confirmed via a repo-wide grep before
writing any code); `serve_adapters`/`handle_adapter_connection` in `src/transport/native/ipc.rs`
matched PINS.md SS9's description exactly (the generic-vs-concrete split, the accept-ahead +
spawn-per-connection shape); `ServiceContext` was `#[derive(Clone)]` with `browser`/`store`/
`recorder`/`initial_policy` fields, built once in `from_startup`; `run_as_service`/`run_as_adapter`/
`serve_session` existed under those exact names and cleanly separated the two roles; `server.rs::run`
had zero compiled call sites (only stale doc-comment mentions in `dispatch.rs`, `hub/mod.rs`,
`tests/audit_recorder.rs`, `tests/manifest_validation.rs`), confirmed via a repo-wide grep for
`server::run(`/`mcp::server::run` before relying on it. No STOP precondition fired.

Implemented per the re-authored task + PINS.md SS8/SS9:
- `src/hub/session.rs` (new): `SessionGuid` (mint via `uuid::Uuid::new_v4()`; `parse` requires
  version-4 AND a byte-exact round-trip to the presented string, so uppercase/braced/urn forms and
  non-v4 UUIDs are refused identically to empty/malformed; redacted `Display`/`Debug`), `PeerCred`/
  `PeerUser` (`PeerUser`'s tuple field made `pub` -- see D1), `SessionRegistry::admit` (first
  presentation binds; same-user re-presentation reuses; a different user is `Refused` and the
  original binding is left untouched), `Admission`. Own `#[cfg(test)]` unit tests plus the pinned
  `tests/hub_identity.rs` suite (3 tests, all transcribed assertions).
- `src/hub/role.rs` (new): `Role`, `set_role`/`role` (backed by a `OnceLock`, panicking verbatim per
  PINS.md SS8 on double-set / read-before-set), `assert_role`/`assert_service_role`/
  `assert_adapter_role` (verbatim pinned panic message), and the 3 pinned `#[cfg(test)]` unit tests.
  `pub mod role;` added to `src/hub/mod.rs` alongside the existing `pub mod handshake;`.
- `src/hub/mod.rs`: `role::set_role(Role::Service)` as the absolute first line of `run_as_service`;
  `role::set_role(Role::Adapter)` as the absolute first line of `run_as_adapter`; `ServiceContext`
  gained `session_registry: Arc<Mutex<SessionRegistry>>` (built once in `from_startup` alongside the
  other shared fields); the service's own lone stdio session now mints `SessionGuid::mint()` and
  passes it to `serve_session`.
- `src/transport/native/ipc.rs`: `capture_peer_cred` added per platform (Windows:
  `GetNamedPipeClientProcessId` on the concrete, still-connected `NamedPipeServer`, then
  `OpenProcess`+`OpenProcessToken`+`GetTokenInformation(TokenUser)`+`ConvertSidToStringSidW` for the
  SID string, called BEFORE the pipe instance is replaced/moved into the spawned task; Unix:
  `SO_PEERCRED` on non-macOS, `getpeereid` on macOS, called on the concrete, just-accepted
  `UnixStream` before it moves into the spawned task) and threaded into `handle_adapter_connection`
  as a new plain parameter, exactly as PINS.md SS9 describes (a capture failure refuses the
  connection cleanly rather than dispatching with no credential). `handle_adapter_connection` now
  parses the hello's `guid` via `SessionGuid::parse` (a malformed/empty/non-canonical guid refuses
  cleanly, never surfacing the raw string), calls `ctx.session_registry.lock()...admit(&guid,
  &peer_cred)`, and on `Admitted` calls `serve_session(stream, ctx, guid)`; on `Refused` the
  connection is dropped without creating a session or logging the GUID. `relay_adapter` now mints
  `SessionGuid::mint()` once (a local variable, since it runs exactly once per adapter process) and
  embeds it in place of the old `""` placeholder.
- `src/transport/mcp/server.rs`: `serve_session` gained the pinned `guid: SessionGuid` parameter
  (not `Option`) and calls `crate::hub::role::assert_service_role("serve_session")` as its first
  line; the dead 2-arg `run` wrapper is deleted (confirmed dead first, per the STOP-precondition-
  adjacent guidance); its now-orphaned doc-comment fragment describing `run`'s own
  `browser`/`loaded_policy` parameters was trimmed in passing (trivial, at the deletion site, per the
  task's own "may correct stale doc comments... not load-bearing" latitude -- not a scope-creep
  hunt elsewhere).
- `src/transport/native/messages.rs`: added the doc-only section for the hello's `guid` member
  (item 4), citing H2's existing `hub`/`role` hello and `src/hub/handshake.rs`; no new Rust types,
  no second handshake frame.
- `tests/architecture.rs` (the ONE sanctioned edit this task, item 5): `FORBIDDEN_IDENTIFIERS =
  ["tabId", "token", "socket"]`, scanned via the existing `contains_path_token` boundary matcher;
  every existing crate-edge/`url` rule is untouched. Added
  `governance_core_rejects_tabid_token_socket_identifiers` (pinned via `scan_line` on synthetic
  code-shaped strings, mirroring the existing `scanner_detects_forbidden_crate_edges` pattern).
- `tests/hub_multiplex.rs` (the sanctioned one-line fix, item 3):
  `adapter_endpoint_two_phase_wire_round_trips`'s hand-built hello's placeholder `"guid": ""`
  literal replaced with a well-formed v4 UUID literal (`00000000-0000-4000-8000-000000000000`) so it
  keeps exercising successful admission and the two-phase wire mechanics once the parse-failure
  refusal landed; no other change to that file.
- New `tests/hub_identity.rs` (3 tests) and `tests/hub_role_wiring.rs` (1 test), all passing, all
  transcribed per the task's pinned assertions.

D1: the task's pinned shape shows `pub struct PeerUser(String);` with no `pub` on the tuple field,
    but the task's OWN pinned test (`guid_is_v4_csprng_and_bound_to_minting_peer`, transcribed into
    `tests/hub_identity.rs`, a separate integration-test crate) constructs
    `PeerCred { user: PeerUser("user-A".into()), pid: 100 }` directly -> made the tuple field `pub`,
    since an external crate cannot name a private tuple-struct field. Impact on later tasks: none --
    any later task constructing a `PeerUser` (H4/H5/H8) does so the same way.
D2: the task marks `SessionGuid`'s redacted `Display`/`Debug` string form "AUTHOR MUST PIN before
    execution," but PINS.md's own "Resolved AUTHOR-MUST-PIN index" lists no H3 row for this value,
    and the task's very next sentence says the pinned test asserts only the non-leak STRUCTURAL
    invariant (never equality to a specific string) -> implemented a fixed literal redacted
    rendering (`"<redacted-session-guid>"` for `Display`, `"SessionGuid(<redacted>)"` for `Debug`)
    with no pinned value found anywhere to transcribe, since no test reads or compares against an
    exact string. Impact on later tasks: none -- no later task's Required Behavior or pinned test
    reads `SessionGuid`'s `Display`/`Debug` output.
D3: item 5's a7 scanner extension, implemented literally (a bare `tabId`/`token`/`socket` match
    anywhere in `src/governance/**`, matching the existing scanner's whole-file including-doc-
    comments philosophy), broke the currently-green `governance_core_has_no_forbidden_back_edges`
    against the REAL live tree: it flagged 6 pre-existing, unrelated doc-comment uses of the
    ordinary English words "token"/"socket" (a UDP "socket" for the syslog audit destination in
    `destinations.rs`; an HTML `autocomplete` "token" in `config/mod.rs`; a grammar/wildcard "token"
    in `enforcement.rs` and `manifest/document.rs` (twice) -- none related to session/credential/
    handle types) plus one real pre-existing local variable literally named `socket` in
    `src/governance/audit/destinations.rs::send_line_to_syslog` (a `std::net::UdpSocket` for syslog
    delivery, unrelated to H3's session-identity concern) -> (a) scoped the NEW identifier check to
    non-doc-comment lines only (added `is_doc_comment` to `tests/architecture.rs`; the pre-existing
    crate-edge/`url` checks still scan doc comments, UNCHANGED -- purely additive, no existing rule
    weakened), resolving 6 of the 7 hits; (b) renamed `destinations.rs`'s one remaining local
    variable (`socket` -> `udp_socket`, a zero-behavior-change rename in a file this task does not
    name) to resolve the last hit, since ADR-0030's own text frames this check as a code-level "the
    core additionally names no tabId/token/socket TYPE" concern, not a ban on the English words
    themselves in unrelated prose or an incidental local-variable name. Impact on later tasks: none
    -- H4/H5/H6/H7/H8 do not reference `destinations.rs`'s local variable name, and no test anywhere
    asserts its identifier; a future task adding NEW governance-core code must still avoid naming
    `tabId`/`token`/`socket` as a real identifier (the check remains live and enforced for code).
D4: `Cargo.toml` (not named by the task) needed one new `windows-sys` feature,
    `Win32_System_Pipes`, to call `GetNamedPipeClientProcessId` for item 2's real Windows
    peer-credential capture -> added the feature (purely additive; no version bump, no other
    dependency change). Impact on later tasks: none.

Verification: all four commands passed for real. `cargo build --all-targets` clean.
`cargo test --test hub_identity --test hub_role_wiring --test hub_multiplex --lib role --test
all_open_golden --test architecture --test audit_recorder` all green, and the FULL `cargo test` is
green (430 lib tests + every integration suite, 0 failed -- up from H2's 423 lib tests, the +7 being
`hub::session::tests` (4) + `hub::role::tests` (3)). `cargo clippy --all-targets -- -D warnings`
clean. `cargo fmt --all -- --check` clean (after running `cargo fmt --all` once to normalize
wrapping the new code introduced -- whitespace only, no semantic change, not logged as its own
numbered deviation). Sacred tests (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`)
green and byte-unmodified (confirmed via `git diff --stat` on both paths: no output);
`tests/architecture.rs::governance_core_has_no_forbidden_back_edges` green, EXTENDED per the one
sanctioned edit, every pre-existing back-edge rule intact. No NEVER-touch fence moved; the H3
sanctioned exceptions (the `tests/architecture.rs` a7 extension, the `tests/hub_multiplex.rs`
one-literal fix) are the only fences touched, both as pinned.
- Note: as in H0/H1/H2, `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's
  `target/`) for build-artifact routing only, not a source or test change.

### H4
- Verified all as-of-authoring facts in `H4-binary-authoritative-isolation.md` and PINS.md SS3/
  SS9 against the live tree before writing any code: `src/hub/mod.rs`'s `ServiceContext` was
  `#[derive(Clone)]` with `browser`/`store`/`recorder`/`initial_policy`/`session_registry`, all
  `pub`, built once in `from_startup`; `src/hub/session.rs` held only the pure `SessionGuid`/
  `PeerCred`/`PeerUser`/`SessionRegistry`/`Admission` types (no per-session record, confirmed via
  grep); `src/transport/mcp/server.rs`'s `serve_session<S>(stream, ctx, guid: SessionGuid)` (NOT
  `Option`) called `crate::hub::role::assert_service_role("serve_session")` as its first line and
  held a `_guid` placeholder binding, exactly as PINS.md SS9 describes; `pipeline.rs`'s
  `handle_tools_call` (~:50) and `LazyTabUrl::new` (~:118) matched the task's quoted line numbers
  within a few lines; `denial.rs::denial_id` and `dispatch.rs`'s `Governance::begin`/
  `CallAudit::sacred_deny` (the public API used to record the refusal) matched their quoted
  shapes. No STOP precondition fired.
- Implemented per the task's Required Behavior, entirely inside `src/hub/mod.rs`,
  `src/hub/session.rs`, and `src/transport/mcp/server.rs` (no edit to `src/governance/**` or
  `src/transport/mcp/pipeline.rs`):
  - `ServiceContext` (`src/hub/mod.rs`) gained `owned_tabs: Arc<Mutex<HashMap<i64,
    session::SessionGuid>>>`, built once in `from_startup` alongside `session_registry`.
  - `src/hub/session.rs` gained `owns_or_adopts_tab(owned_tabs, guid, tab_id) -> bool`: the ONE
    pinned map operation (PINS.md SS9 forward guidance) --
    `map.entry(tab_id).or_insert_with(|| guid.clone()) == guid` -- answering both "do I own it"
    and "can I adopt it" with no per-session record. Plus its own `#[cfg(test)]` unit test.
  - `src/transport/mcp/server.rs`: `serve_session`'s read loop gained a NEW
    `check_tab_ownership(line, &owned_tabs, &guid, &governance)` call, run BEFORE
    `handle_line` (hence before `pipeline::handle_tools_call`'s own `LazyTabUrl` probe). It
    re-parses the raw line itself (a separate, cheap `Value` parse, deliberately NOT threaded
    through `handle_line`'s own parse -- see D shape note below) to read `method`/`params.name`/
    `params.arguments.tabId`; for a `tools/call` naming a numeric `tabId` a DIFFERENT guid
    already owns, it records the refusal as a deny via `Governance::begin` +
    `CallAudit::sacred_deny(&denial, None)` (domain `None` per the call site, matching PINS.md
    SS3's `domain: null`/`held: false`/`duration_ms: 0` shape exactly, since `sacred_deny`
    already builds a zero-duration, non-held deny record) with a `Denial { rule:
    "cross_session/unowned_tab", grant_id: None, denial_id: denial::denial_id("", "", rule),
    domain: String::new(), message: "unknown tab".to_string() }`, then returns the uniform
    `text_content("unknown tab")` success result immediately -- never entering `handle_line` or
    `pipeline::handle_tools_call` for that line. Every other line (not `tools/call`, unparseable,
    no numeric `tabId`, or a `tabId` this session already owns/first-touch-adopts) falls through
    unchanged. The `guid: SessionGuid` parameter (H3's placeholder) is now genuinely consumed;
    its doc comment on `serve_session` was updated accordingly.
  - New `tests/hub_isolation.rs` (2 tests, both from the task's named list, both green):
    `unowned_tab_is_refused_before_any_tab_url_probe` and
    `unknown_tab_result_leaks_no_host_or_existence`. Both drive a real `serve_session` session
    (B) over an in-process `tokio::io::duplex`, sharing one `Browser` with a fake extension
    double mirroring `pipeline.rs::attach_fake_extension_with_tab_urls` (panics on any
    unregistered `tab_url_request`/`tool_request`, proving a leaked probe/dispatch fails loudly).
- D1: Required Behavior item 1 names TWO ownership-map insertion paths -- "(a) `tabs_create_mcp`
  returns it successfully to this session, or (b) this session issues a tab-scoped call naming a
  tabId that no OTHER live session owns (first-touch adoption)" -> implemented ONLY path (b); no
  code parses `tabs_create_mcp`'s response to eagerly register its newly created tabId. Because:
  no oracle exists anywhere (PINS.md, the task file, or any Rust-side type) for HOW to extract the
  created tabId from that call's free-text MCP result (the only signal is the extension's OWN JS
  string, `"Created tab ${tab.id}.\n"` in `extension/service-worker.js`, which is not a frozen
  Rust string, not part of any named test's fixture, and not reachable from a Rust-side fake
  extension test double without inventing a text-parsing convention the ORACLE RULE forbids
  deriving); and the task's own test-setup note explicitly sanctions bypassing a live
  `tabs_create_mcp` round trip ("via `tabs_create_mcp` returning tabId 5, OR the H3-established
  ownership path"), which is the latitude this deviation uses. Path (b) alone covers the
  realistic case: whichever session creates a tab is the only one who initially knows its tabId,
  so its own next reference to that tabId first-touch-adopts it before any other session could
  plausibly name the same number. Impact on later tasks: H7 (tab-group-per-session, PINS.md SS6)
  sends a `group_request` for a session's owned tabs -- RE-READ H7's own live tree state before
  assuming a tab is already in `owned_tabs` immediately after `tabs_create_mcp` returns and before
  any other call references it; if H7 needs that guarantee, it must add the
  `tabs_create_mcp`-response adoption path itself, with its own pinned extraction oracle.
- D2: the task's test-setup prose for `unknown_tab_result_leaks_no_host_or_existence` describes
  the second case as "a tabId that no session owns and no extension knows (does NOT exist)" ->
  implemented it as a tabId (999) pre-seeded as owned by session A (the SAME owner as the
  existing-tab case), not left absent from `owned_tabs`. Because: under the pinned first-touch-
  adoption mechanism (Required Behavior item 2's own words, "first-touch always succeeds for an
  unowned tabId"), a tabId genuinely ABSENT from the map is ADOPTED and ALLOWED for whichever
  session names it first -- it is NOT refused. If B were the first to name tabId 999, B's own
  call would first-touch-adopt it and dispatch for real, which cannot produce the pinned uniform
  `"unknown tab"` text (the two pinned assertions require `text_for_existing_other_session_tab ==
  text_for_nonexistent_tab`, both equal to `"unknown tab"` -- only achievable if BOTH calls are
  refused by the SAME cross-session-ownership mechanism). Pre-seeding tab 999 as owned by A (a
  guid other than B) makes both cases refused identically, while "no extension knows [it]" is
  still satisfied literally: the fake extension has zero configuration for tabId 999 in either
  table, so if either refusal leaked into a real dispatch/probe, the test's panic-on-unregistered
  fake extension would fail loudly. Impact on later tasks: none (test construction only).
- D3: to exercise `serve_session` for two independently-identified sessions without spawning the
  real binary (the pattern `tests/hub_multiplex.rs`/`tests/all_open_golden.rs` use for their own
  subprocess-driven suites), `tests/hub_isolation.rs` constructs `ServiceContext` directly via its
  own `pub` fields (`browser: Browser::new()`, `store: ConfigStore::load_initial(...)`, a disabled
  `Recorder`, an all-open `LoadedPolicy`, a fresh `SessionRegistry`, and a fresh `owned_tabs` map
  the test pre-seeds per scenario -- "the H3-established ownership path" the task file itself
  sanctions) and drives session B over an in-process `tokio::io::duplex`. Because `serve_session`'s
  first line asserts `crate::hub::role::assert_service_role`, and this test never goes through
  `run_as_service` (which normally calls `role::set_role(Role::Service)`), the test calls
  `hub::role::set_role(Role::Service)` itself, guarded by a `std::sync::Once` so it runs exactly
  once for the whole test binary (multiple `#[tokio::test]` functions in one file share the same
  process-global role marker, which panics if set twice). Impact on later tasks: H5/H7/H8's own
  `hub_*`/`webapi_*` tests that want to drive `serve_session` in-process (rather than via a real
  subprocess) will need the same `ensure_service_role()`-style guard; note it here rather than
  rediscovering it.
- Verification: all four commands passed for real. `cargo build --all-targets` clean. `cargo test
  --test hub_isolation --test all_open_golden --test tool_enforcement --test architecture` all
  green (2 + 3 + 10 + 5 = 20 tests); `cargo test -p ghostlight --lib transport::mcp::pipeline`
  green (23/23, unaffected); the FULL `cargo test` is green (431 lib tests -- up from H3's 430,
  the +1 being `hub::session::tests::owns_or_adopts_tab_first_touch_then_refuses_a_different_guid`
  -- plus every integration suite, 0 failed). `cargo clippy --all-targets -- -D warnings` clean.
  `cargo fmt --all -- --check` clean (after running `cargo fmt --all` once to normalize wrapping
  in the new code -- whitespace only, no semantic change, not logged as its own numbered
  deviation, per H0/H1/H2/H3 precedent). Sacred tests (`tests/tool_schema_fidelity.rs`,
  `tests/all_open_golden.rs`, `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`)
  green and byte-unmodified (`git diff --stat` on all three: no output). `src/governance/**` and
  `src/transport/mcp/pipeline.rs` are untouched (`git status --porcelain` shows only
  `src/hub/mod.rs`, `src/hub/session.rs`, `src/transport/mcp/server.rs` modified, plus the new
  `tests/hub_isolation.rs`). No NEVER-touch fence moved.
- Note: as in H0-H3, `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's
  `target/`) for build-artifact routing only, not a source or test change.

### H5
- Verified all as-of-authoring facts in `H5-grace-window-honest-queue.md` and PINS.md SS4/SS9
  against the live tree before writing any code: `src/transport/executor.rs`'s `TOOL_TIMEOUT`
  (60s), `kill_error()`, `send_and_await`'s fail-fast-if-`outgoing`-is-`None` path, and `attach`'s
  drain-on-detach tail (`*self.outgoing.lock().unwrap() = None;` then
  `for (_, tx) in self.pending.lock().unwrap().drain() { ... }`) all matched the task's quoted
  shapes -- the STOP precondition ("if `attach` no longer drains and FAILS pending calls on
  detach ... the premise is wrong") did NOT fire; `handle_session_killed`'s drain-with-kill-error
  and the killed-check precedence in `call`/`tab_url` matched too. `src/hub/mod.rs`'s
  `ServiceContext` and `src/transport/native/ipc.rs`'s `serve_adapters`/
  `handle_adapter_connection` matched PINS.md SS9's corrected description exactly (accept/
  admission in `ipc.rs`, generic-vs-concrete split, `ServiceContext` built once and cloned per
  session) -- the STOP precondition for a diverged H2/H3 landed shape did NOT fire. No
  AUTHOR-MUST-PIN value was still literally unpinned (GRACE_WINDOW, PER_PEER_MINT_CAP,
  PER_PEER_GROUP_CAP, the quota-exceeded text, SCREENSHOT_CHUNK_THRESHOLD, and the completion
  bound are all pinned verbatim in PINS.md SS4). No STOP precondition fired.
- Implemented per the task's Required Behavior, inside the files the task names:
  - `src/transport/executor.rs`: added `pub const GRACE_WINDOW: Duration =
    Duration::from_secs(10)` next to `TOOL_TIMEOUT`. `attach`'s tail no longer drains pending
    inline on detach; it calls the new private `spawn_grace_drain(GRACE_WINDOW, drain_err)`,
    which spawns a task that awaits `Browser::wait_connected(window)` (an EXISTING method, no new
    reconnect-watching logic needed) and drains pending with the byte-identical `drain_err` ONLY
    if the window elapses with no reconnect. `spawn_grace_drain` is spawned so `attach` itself
    still returns `Detached` promptly regardless of the window's length -- no caller of `attach`
    blocks on the grace hold. The four pinned hop-attributed error strings, `TOOL_TIMEOUT`,
    `kill_error()`'s precedence, and the single-physical-extension-link `AttachOutcome` are all
    byte-unchanged.
  - `src/hub/mod.rs`: added `PER_PEER_MINT_CAP`/`PER_PEER_GROUP_CAP`/`MINT_QUOTA_EXCEEDED`/
    `MintQuota`/`MintGuard`/`try_mint` (a per-peer, never-global, RAII-scoped mint-quota
    mechanism: `try_mint` checks-and-increments a `PeerUser`-keyed counter, returning a
    `MintGuard` that decrements the SAME counter on drop, so the cap counts CONCURRENT sessions,
    never lifetime mints). `ServiceContext` gained a `mint_quota: MintQuota` field, built once in
    `from_startup` alongside `session_registry`/`owned_tabs`. Added the module-doc bottleneck
    note (Required Behavior item 4) citing ADR-0030 Decision 3 without restating its semantics.
  - `src/transport/native/ipc.rs`: `handle_adapter_connection`'s `ROLE_ADAPTER` arm now calls
    `crate::hub::try_mint(&ctx.mint_quota, &peer_cred.user)` BEFORE
    `ctx.session_registry.lock()...admit(...)`; on `Err`, the connection is refused (logged,
    dropped, never surfacing a GUID) exactly like an existing `Admission::Refused`; on `Ok`, the
    `MintGuard` is held for the connection's whole lifetime (including a `Refused` admission),
    freeing the slot only when the connection genuinely ends.
  - `src/transport/mcp/server.rs`: added `SCREENSHOT_CHUNK_THRESHOLD` (8 MiB, PINNED), a private
    `CHUNK_SIZE` (1 MiB, NOT pinned -- PINS.md SS4 only pins the threshold, the completion bound,
    and the yield-between-chunks behavior), and `write_chunked` (below the threshold: one
    `write_all`, byte-identical to pre-H5; at/above it: fixed-`CHUNK_SIZE` `write_all` calls with
    an explicit `tokio::task::yield_now().await` between them). `serve_session`'s writer task now
    calls `write_chunked(&mut out, buf.as_bytes())` instead of one unconditional `write_all`; the
    JSON-RPC content and framing are byte-identical either way, only the number of write calls
    (and the scheduling yield points) changes. Added a short module-doc pointer to this.
  - `docs/adr/0004-reject-second-session.md`: appended an "Amendment (2026-07-04, ADR-0030)"
    section cross-referencing ADR-0030's repeal at the MCP-client layer; the ADR's `Status` field
    and its retained single-physical-extension-link invariant are untouched (only an append, 0
    deletions).
  - New `tests/hub_queue.rs`: `per_peer_mint_cap_denies_a_flooding_peer_without_locking_out_others`
    (peer A mints up to `PER_PEER_MINT_CAP` = 32, then is denied with the exact pinned text
    `session limit reached for this client`; peer B, distinct, still succeeds while A is over
    cap; freeing one of A's slots lets A mint again) and
    `oversized_screenshot_is_chunked_not_head_of_line_blocking` (two independent `serve_session`
    sessions, each its own `Browser`; session 1's fake extension answers one `computer`
    `screenshot` call with a 9 MiB text reply relayed through a `CountingWriter` `AsyncWrite`
    double wrapping session 1's own server-side stream; session 2 issues a bare `ping`
    concurrently on a `current_thread` runtime; asserts session 2 completes in `< 2s` and session
    1's reply required `> 1` write calls). Both tests, BY NAME from the task file, green.
  - `src/transport/executor.rs`'s own `#[cfg(test)]` module gained 3 supplementary tests (NOT
    named by the task file, which names no test for Required Behavior item 1): a direct
    transcription check that `GRACE_WINDOW == Duration::from_secs(10)` and `< TOOL_TIMEOUT`; a
    test that a reconnect within the grace window leaves a pending call untouched (drives the
    private `spawn_grace_drain` directly with a short window so the test stays fast, rather than
    waiting out the real 10s constant); and a test that the window elapsing with no reconnect
    drains pending with the exact, unchanged disconnect text. All three transcribe only
    already-pinned literals (no invented oracle).
- D1: `tests/hub_isolation.rs`'s `build_ctx` (a file this task does not name) constructs
  `ServiceContext` via its own public fields, per H4's own precedent -> adding the new
  `mint_quota` field to `ServiceContext` made that construction stop compiling
  (`E0063: missing field 'mint_quota'`); added one line, `mint_quota: Arc::new(Mutex::new(
  HashMap::new()))`, matching the exact construction `build_ctx` already uses for
  `session_registry`/`owned_tabs`. Because: this is a compile-forced, purely mechanical
  one-field addition with no semantic change to that file's own tests (both continued to pass
  unchanged), the same category of forced adjustment H1's D2 (import cleanup) and H3's D4
  (a new Cargo.toml feature) already used this batch. Impact on later tasks: none -- H6/H7/H8's
  own `ServiceContext` constructions (if any, in new test files) must include `mint_quota` too,
  the same way they already must include `session_registry`/`owned_tabs`.
- D2: the task file's "Tests (BY NAME; assertions pinned)" section names exactly two new tests,
  both for `tests/hub_queue.rs` (the per-peer quota and the chunking property); Required
  Behavior item 1 (the bounded reconnect grace window) has NO named test anywhere in the task
  file -> added 3 supplementary tests directly in `src/transport/executor.rs`'s own test module
  (listed above) to gain confidence the grace-hold mechanism is actually correct, since leaving
  it fully untested seemed like an unacceptable quality gap for load-bearing timing logic, not
  because any pinned assertion required them. Every assertion in these 3 tests transcribes an
  already-pinned literal (`GRACE_WINDOW`'s value, the unchanged disconnect error text) or checks
  a structural property (`<` between the two pinned durations) -- no value was derived or
  invented. Impact on later tasks: none -- these are supplementary, not part of the task's named
  completion surface; a later task auditing "what tests exist for H5" should expect
  `tests/hub_queue.rs`'s 2 named tests as the CONTRACTUAL surface and these 3 as extra confidence
  only.
- Verification: all four commands passed for real. `cargo build --all-targets` clean.
  `cargo test --test hub_queue` (2/2, both named tests green); `cargo test --test
  all_open_golden` (3/3); `cargo test --test peer_death` (1/1); `cargo test --lib --
  transport::executor::tests::call_without_a_connection_fails_fast
  transport::executor::tests::kill_error_outlives_the_disconnect` (2/2, both kept-green tests
  unchanged and passing); the FULL `cargo test` is green (434 lib tests -- up from H4's 431, the
  +3 being the supplementary grace-window tests above -- plus every integration suite including
  the new `hub_queue` suite, 0 failed). `cargo clippy --all-targets -- -D warnings` clean.
  `cargo fmt --all -- --check` clean (no reformatting needed). Sacred tests
  (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
  `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`) green and byte-unmodified
  (`git diff --stat` on all three plus `src/transport/mcp/tools.rs` and
  `src/transport/native/host.rs`: no output). `git status --porcelain` shows only
  `docs/adr/0004-reject-second-session.md`, `src/hub/mod.rs`, `src/transport/executor.rs`,
  `src/transport/mcp/server.rs`, `src/transport/native/ipc.rs`, and `tests/hub_isolation.rs`
  (D1) modified, plus the new `tests/hub_queue.rs`. No NEVER-touch fence moved.
- Note: as in H0-H4, `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's
  `target/`) because live `ghostlight.exe` processes (this environment's own dogfooded session)
  held the repo's `target/debug/ghostlight.exe`; build-artifact routing only, not a source or
  test change.

### H6
- BLOCKED at the per-task procedure's step 2 (RE-READ every source file the task names; verify
  each as-of-authoring fact; trace Required Behavior against the live tree AND the existing test
  suite before writing any test or implementation code) -- no working-tree changes exist to
  revert.
- Re-verified the as-of-authoring facts that still hold: `src/hub/mod.rs` exists with
  `run_mcp_server`/`run_as_service`/`run_as_adapter`/`ServiceContext` exactly as H2-H5 landed them;
  `run_as_service` (src/hub/mod.rs, current `run_as_service`) still spawns
  `transport::watchdog::wait_until_orphaned(parent)` against the captured MCP-client parent and
  serves this process's OWN stdio as the first session over the SAME `ServiceContext` every other
  adapter session clones (its own doc comment says so verbatim: "SERVICE role only -- ADR-0030
  Decision 8 re-scopes the reaper to the ADAPTER at H6; until then a lone-client SERVICE keeps
  today's behavior"); `src/hub/role.rs` (H3) exists with `assert_adapter_role` exactly as PINS.md
  SS8 pins it; `src/proc.rs`/`src/transport/watchdog.rs` are unchanged from their as-of-authoring
  shape; `src/transport/native/ipc.rs`'s `DEFAULT_ENDPOINT`/`default_endpoint()` and the
  claim/serve_adapters/relay_adapter/handle_adapter_connection shape match PINS.md SS9's corrected
  description. `src/debug.rs`'s `log_dir()` resolves to `dirs::data_local_dir()/ghostlight`
  (`%LOCALAPPDATA%\ghostlight` on Windows), NOT the task's stated `%ProgramData%\ghostlight` (a
  machine-wide dir) -- a minor, easily-resolved mismatch (the task's own parenthetical says "RE-READ
  src/debug.rs ... do not invent a new dir", so the per-user dir debug.rs actually uses would have
  been the one to reuse; not what blocked this task). No dependency (`hmac`/`rand`/the
  `Win32_Security_Cryptography` / `Win32_System_JobObjects` windows-sys features) exists yet for
  the anti-squat HMAC or the job-breakaway verification, but that too is not what blocked this task
  -- ordinary additive Cargo.toml work the executor could have done.
- STOP precondition triggered (transcribed verbatim from `H6-detached-lifecycle-antisquat.md`):
  "If, after H2, the persistent SERVICE still wires `transport::watchdog::wait_until_orphaned` to a
  client parent, STOP and resolve that FIRST -- a persistent service must not exit on client death
  (ADR-0030 Decision 8; task NEVER-touch below). Do not layer H6 on top of a service that still
  dies with a client." This is confirmed true of the live tree (see above). Attempting to
  "resolve that FIRST" surfaced a genuine, irreconcilable requirements conflict rather than a
  mechanical fix, which also trips the second STOP precondition: "If detached-spawn cannot
  GUARANTEE the service escapes the Chrome job object on Windows (breakaway cannot be verified),
  STOP and mark BLOCKED in the ledger with reasoning."
- The conflict, traced by actually reasoning through both the required end-state and the existing,
  must-stay-green test suite (not merely a hunch):
  - ADR-0030 Decision 1 requires the persistent SERVICE to sit "in neither the adapter's nor
    Chrome's job object", and Decision 8 requires it "spawned DETACHED and unparented (Windows: no
    job inheritance / verified breakaway; Unix: setsid)" and that it "shuts down on an idle-grace
    window ... never on parent-death." Required Behavior item 1 in this task's own file says "The
    adapter, on finding the service absent, spawns the SAME binary in the service role DETACHED."
  - Windows job-object semantics make this unavoidably a TWO-PROCESS design: a process cannot
    retroactively leave a job object it was created into -- only a freshly `CreateProcess`'d CHILD
    with `CREATE_BREAKAWAY_FROM_JOB` can escape one. So the process an MCP client (or a test
    harness) directly launches can NEVER itself become "the detached service" after the fact; a
    faithful implementation must make EVERY normal invocation a thin ADAPTER that either connects to
    an already-running SERVICE or spawns a SEPARATE, distinct, always-detached SERVICE process.
  - This directly contradicts `tests/peer_death.rs::native_host_exits_when_server_dies`
    (tests/peer_death.rs:19-81), which this task's own "Tests (BY NAME; assertions pinned)" section
    lists under "Keep green (do not modify)". That test spawns ONE plain `ghostlight` invocation
    (named "server" in the test, with no service/adapter marker of any kind -- none exists in the
    tree; confirmed via a repo-wide grep for `GHOSTLIGHT_HUB`/`service_role`/`CREATE_BREAKAWAY`/
    `DETACHED_PROCESS`, zero hits), waits for its debug snapshot to show the native-host connected,
    force-KILLS THAT SAME DIRECTLY-SPAWNED PROCESS, and asserts the native-host relay exits within
    5s because its peer died. Under a faithful Decision-8 implementation, "server" is the ADAPTER,
    and the actual extension-endpoint owner the native-host is peered to is a DIFFERENT, separately
    spawned, always-detached SERVICE process -- which by design (and by this SAME task's own NEW
    named test `tests/hub_lifecycle.rs::service_survives_the_spawning_adapter_exit`, pinned to
    prove exactly this) MUST NOT die when the spawning adapter dies. Killing "server" would then
    kill only the adapter half; the native-host's true peer (the SERVICE) stays alive, so the
    native-host would never observe its peer dying, `exited` would stay `false` past the 5s
    deadline, and `native_host_exits_when_server_dies` would go from green to permanently red.
  - The narrower alternative (keep H2's "whichever process wins `claim_adapter_endpoint` becomes
    the in-process service, exactly as landed; only strip the parent-watchdog and add idle-grace")
    preserves `peer_death.rs` (the single spawned process still owns the extension link and dies
    with itself), but then NEVER exercises a genuinely detached, job-breakaway-verified spawn for
    ANY scenario in the existing test suite: `tests/all_open_golden.rs`, `tests/mcp_protocol.rs`,
    `tests/hub_multiplex.rs`, and `tests/peer_death.rs` itself all spawn exactly one `ghostlight`
    instance against a fresh, uncontested endpoint, so that one instance always just wins the claim
    outright under the narrower model -- "spawn-on-demand" and "verified job-object breakaway"
    (Required Behavior item 1; the second STOP precondition explicitly calls this "the whole point
    of Decision 8" and "an explicit acceptance gate") would have NO exercised code path anywhere
    the existing suite reaches, which is the STOP precondition's own failure mode ("cannot
    GUARANTEE the service escapes ... breakaway cannot be verified" -- here because the mechanism
    is never even invoked in the scenarios that must stay green).
  - These two readings are mutually exclusive; there is no third design available to a literal
    executor without inventing a resolution the task does not pin (e.g., which test's premise
    yields). Per BOOTSTRAP's Failure protocol and the standing instruction to prefer BLOCKED over
    improvising, no code or test was written, and nothing was reverted (there was nothing to
    revert).
- What is needed to proceed (decided by the frontier author, not by this executor): reconcile the
  two pinned requirements, e.g. (i) amend `tests/peer_death.rs` (lift it off the "keep green, do
  not modify" list for this task and rewrite its scenario -- e.g. spawn with an explicit
  service-only marker this task would introduce, or add a second scenario that kills the actual
  SERVICE process and asserts the native-host dies with IT, leaving the adapter-death case to
  `tests/hub_lifecycle.rs::service_survives_the_spawning_adapter_exit`'s existing coverage instead);
  or (ii) re-scope H6's Required Behavior so a lone MCP-client invocation is NOT required to spawn a
  truly separate detached service (accept the narrower, winner-becomes-in-process-service model for
  the common case), and correspondingly re-word the job-breakaway STOP precondition so it is
  satisfied by a dedicated new unit test rather than by the existing integration suite; or (iii)
  some other explicit resolution naming which requirement yields. No deviation numbers logged: this
  is a requirements conflict between two of the task's own pinned requirements, discovered by
  tracing Required Behavior against the live, must-stay-green test suite before writing any code --
  not a choice this executor made under the task's own latitude.

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
