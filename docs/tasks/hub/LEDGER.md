# Ghostlight Hub batch: LEDGER

Durable progress for the Hub batch (ADR-0030). One task = one commit. Update this file at the end of
every task, per BOOTSTRAP step 8. This is the single source of truth for "where are we"; a fresh
executor resumes from RESUME HERE with no other context.

## RESUME HERE

**H8 is DONE (af1d0f8). H9 (installer auto-start) is NEXT.** H8 landed the local web API's
`channels.webapi.from` policy in full, plus a real (if deliberately minimal) HTTP/1.1 + WebSocket
listener wired into the standalone service. Governance side (the single sanctioned
`src/governance/**` addition): new `src/governance/channels.rs` owns the flat
`channels.webapi.from` allowlist type, its exact-match `is_member`/`decide_webapi_from` (rule
`channel/webapi_from`, `denial_id` via the existing `denial::denial_id` scheme), fail-closed
`validate_webapi_from` (rejects any non-flat-string-array shape), and `ChannelsPdp` -- a
`PolicyDecisionPoint` impl deciding ONLY the resolved `DecisionRequest.channel_source` axis (a
NEW `Option<String>` field on `DecisionRequest`, `ports.rs`, stamped `None` at dispatch.rs's one
production call site and every existing test construction -- byte-identical for every non-web
session). `ChannelsPdp` never touches the tool/resource axes, so it structurally cannot gate which
tools exist (ADR-0030 Decision 6 preserved). `tests/architecture.rs`'s a7 scanner needed NO edit at
all: `channels.rs` was written to name no forbidden crate edge or bare tabId/token/socket
identifier in the first place (one early doc-comment draft literally spelled out
`crate::browser`/`crate::transport`/`crate::mcp`/`crate::native` in prose and tripped the
doc-comment-scanning crate-edge check; reworded to describe the boundary without spelling out the
forbidden paths -- see the H8 Log's D1). Hub side: new `src/hub/webapi.rs` owns
`builtin_webapi_from()` (`["localhost"]`), the pure one-argument `resolve_bind(allowlist) ->
"127.0.0.1" | "0.0.0.0"` (PINS.md SS7), `classify_source` (peer IP -> the channels vocabulary), the
real TCP accept loop (`run`, spawned from `run_service_loop` alongside the extension/adapter
endpoints; a bind failure is logged and non-fatal, exactly like the extension endpoint's
`SessionBusy` handling, since concurrent test-spawned services must never crash over a shared TCP
port), the HTTP/1.1 upgrade parse + `Host`/`Origin` validation (DNS-rebind defense + the channels
decision) + hand-rolled RFC 6455 handshake (SHA-1 + base64, no new crate -- a published standard's
fixed algorithm, not a project decision; self-verified against the RFC's own worked example), and a
`WsStream` `AsyncRead`/`AsyncWrite` adapter tunneling raw bytes through minimal (unfragmented,
no ping/pong reply) WS data frames so the UNCHANGED `serve_session` -- the SAME chokepoint every
MCP adapter session calls -- needs no changes at all; a web session mints its own `SessionGuid`
exactly as `PINS.md` SS9's forward guidance describes (no `SessionRegistry::admit`, since a remote
TCP peer has no OS credential to bind). See the H8 Log for the full scope-limitation rationale
(no pinned test exercises the wire past the handshake) and its 2 deviations (D1: the a7 doc-comment
trip above; D2: full `ConfigStore`-driven `channels.webapi.from`/`webapi.bind` layering is deferred,
so the running service always resolves to the builtin default today).

**H9 is DONE (375810a). The H0-H9 batch is now COMPLETE.** H9 landed the per-user, zero-admin OS
supervisor registration for the always-ready service (ADR-0030 Decision 8 amendment). New
`src/install/supervisor.rs`: a `SupervisorStep` enum (`WriteFile`/`RemoveFile`/`Run`) built by
three cfg-split PURE builder pairs (`register_steps(exe, ctx)`/`unregister_steps(ctx)`), one per
platform, all reusing `crate::hub::supervisor::{SUPERVISOR_TASK_NAME, SUPERVISOR_LABEL,
SUPERVISOR_UNIT}` (H6's constants, imported cfg-gated per platform to avoid an unused-import
warning on the other two) so the installer and the adapter's self-heal always name the identical
per-platform supervisor. Windows: `schtasks /create /tn "Ghostlight Service" /tr "\"<exe>\"
service" /sc onlogon /rl limited /f` then `schtasks /run /tn "Ghostlight Service"`; unregister is
`schtasks /delete /tn "Ghostlight Service" /f`. macOS: writes the pinned plist to
`~/Library/LaunchAgents/org.sylin.ghostlight.service.plist`, then `launchctl bootstrap gui/<uid>
<plist-path>` + `launchctl kickstart -k gui/<uid>/org.sylin.ghostlight.service`; unregister is
`launchctl bootout gui/<uid>/org.sylin.ghostlight.service` then removes the plist. Linux: writes
the pinned unit to `~/.config/systemd/user/ghostlight.service`, then `systemctl --user
daemon-reload` + `systemctl --user enable --now ghostlight.service`; unregister is `systemctl
--user disable --now ghostlight.service` then removes the unit file. The exe path is resolved via
the EXISTING `native_host::normalize_exe_path` (Required behavior's mandated reuse -- no new path
resolution invented). A new `apply_steps` function applies these steps BEST-EFFORT (`[ok]`/`[warn]`/
`[plan]`/`[noop]` printed in the installer's existing style): it NEVER returns an error and is
called OUTSIDE the existing `Action`/`Op`/`Tally`/`exit_result` pipeline in `run_install`/
`run_uninstall` (`src/install/mod.rs`, the ONLY file this task modified), so a supervisor failure
can never turn an otherwise-successful install/uninstall into a failed exit code (Required behavior
item 4) while the existing native-host/client registration pipeline is completely untouched
(byte-identical `Action`/`Op`/`Tally` types and control flow). The supervisor is registered
unconditionally on every install (both `--system` and per-user), since Decision 8 requires it to
stay per-user regardless of the browser/client registration scope. New `tests/install_supervisor.rs`
holds the 3 task-named pure-builder tests (`windows_task_register_command_is_pinned` /
`macos_plist_names_the_service_subcommand` / `linux_unit_names_the_service_subcommand`, each
`#[cfg]`-gated to its own platform per the task); none of them ever executes `schtasks`/`launchctl`/
`systemctl` (per the task's explicit scope: real OS registration is manual smoke, not a cargo
gate). No deviations.

**H7 is DONE (f12a728).** (Superseded by the H8-DONE block above; kept for provenance.) H7 landed the additive
`group_request`/`group_response` native-messaging pair (ADR-0030 Decision 6/7; PINS.md SS6): the
service's SHARED `check_tab_ownership` gate (`src/transport/mcp/server.rs`, H4's own pre-dispatch
chokepoint) now switches on a NEW `crate::hub::session::TabClaim` (`Owned`/`Adopted`/`Refused`, new
in `src/hub/session.rs`, which `owns_or_adopts_tab` is now reimplemented in terms of so the two
never drift) and fires a NEW `emit_group_request` helper ONLY on `Adopted` -- never on an
already-owned re-touch, never on a refusal -- naming the session's FULL current owned-tab set
(`crate::hub::session::owned_tab_ids`, sorted) and the PINNED title format
(`crate::hub::session::group_title`, `"\u{1F47B} Ghostlight <short>"`, first 8 GUID chars). The
send itself is `Browser::request_group` (`src/transport/executor.rs`), a fire-and-forget send over
H2's existing `outgoing` channel mirroring `send_hold_reply`'s posture (the pinned wire carries no
`id` to correlate a `group_response` by; `route_reply` already drops an id-less non-`session_killed`
frame as an ordinary event, so no new routing/pending-map logic was needed). `messages.rs` gained
one additive doc section (every byte-frozen section untouched). On the extension side: a NEW pure
module `extension/lib/grouping.js` (`groupSessionTabs`, unit-tested in isolation by the task-named
`tests/extension/grouping.test.js::owned_tabs_are_grouped_on_service_request_only`, covering all 4
pinned assertions in one test as named) makes the actual grouping decision given an injected
`chrome`; `service-worker.js` imports it, adds the `group_request` branch to the existing
`nativePort.onMessage` handler, and adds a NEW `sessionGroups` (guid -> Chrome tab-group id) map,
persisted/restored via the EXISTING `persistSessionState`/`rehydrate` functions under a NEW,
additive `sessionGroupsState` storage key (the pre-existing `sessionState` key/shape is untouched).
See the H7 Log entry below for the one significant design decision this task required beyond its
own text: the pre-existing single-group access-control mechanism
(`groupId`/`ensureGroup`/`groupTabs`/`inGroup`/`effectiveTabId`) was left COMPLETELY UNTOUCHED,
deliberately NOT unified with the new per-session `sessionGroups` map, because the sacred
`tool_request`/`tool_response` wire carries no session identity at all -- that mechanism
structurally cannot become session-aware, and the task names no test exercising it. Flagged as a
real (untested, out of this task's named scope) production interaction for the frontier author's
awareness, not solved here.

**H6 is DONE (927d102). H7 (tab-group-per-session presentation) is NEXT.** H6 landed the
always-ready-service amendment in full: `src/hub/mod.rs`'s `run_mcp_server` is now ALWAYS the thin
ADAPTER (argv dispatch, never a claim election; role decided before anything else); the new
`ghostlight service` subcommand (`run_service`/`run_service_loop`) is the STANDALONE SERVICE, which
owns both endpoints for its whole life, serves NO stdio session of its own, runs NO parent-death
watchdog, and idle-graces after `IDLE_GRACE` = 30s of continuous zero-live-sessions-AND-extension-
gone (`ServiceContext` gained a `live_sessions: Arc<AtomicUsize>` field, counted by a new
`LiveSessionGuard` RAII wrapper in `transport::mcp::server::serve_session`). `src/hub/supervisor.rs`
(new) holds the OS-supervisor identifiers (`SUPERVISOR_TASK_NAME`/`SUPERVISOR_LABEL`/
`SUPERVISOR_UNIT`) H9 will register, `supervisor_start_command()` (pure, cfg-split, unit-tested,
never executed except by `start_service()`, which `assert_adapter_role`s first), and the
`SELF_HEAL_RETRY_WINDOW`/`SELF_HEAL_RETRY_INTERVAL`/`SELF_HEAL_FAILURE_MESSAGE` constants
`ipc::relay_adapter`'s new `dial_with_self_heal` uses (a single dial attempt; on failure,
`supervisor::start_service()` once, then bounded retries; tests never exercise this path). New
`src/hub/antisquat.rs` implements the anti-squat per-install secret (`load_or_create_hub_key`
SERVICE-only, `read_hub_key` ADAPTER-only, both via `crate::debug::log_dir()`, PER-USER) and the
HMAC-SHA256 proof (`compute_mac_hex`/`verify_mac_hex`, hand-rolled hex, no new hex crate); wired
into `ipc.rs` as a THIRD framed message on the adapter/control wire (hello -> service-proof -> raw
JSON-RPC): `handle_adapter_connection` sends the proof via `send_service_proof` AFTER admission,
BEFORE `serve_session`; `relay_adapter` verifies it via `verify_service_proof` AFTER sending its own
hello, BEFORE the raw relay; ANY failure collapses to the ONE pinned refusal text. `doctor.rs`'s
reap-target filters/text (`orphan_pids`, `reap`, the run_fix report line, the module docs) re-scoped
from `"mcp-server"` to `"adapter"`; the health-anchor/display filters (`NewestServer`, `session_row`,
the native-host-row finder) stay `"mcp-server"`, unchanged, per PINS.md SS5.5. `src/proc.rs`/
`src/transport/watchdog.rs` module docs updated from "the mcp-server role" to "the adapter role"
(no API change). `src/main.rs` gained the `Service` unit subcommand + its `main()` arm;
`docs/adr/0029-process-lifecycle-hygiene.md` gained a superseded/amended note at the top.

RE-READ H7's own task file plus PINS.md SS6 before starting; H7 crosses the JS boundary
(`extension/lib/grouping.js`) in addition to Rust. Follow the per-task procedure in `BOOTSTRAP.md`.

See the H6 Log entry below for the full test-topology deviation record: a new shared
`tests/support/mod.rs` (`spawn_service`/`spawn_service_with_manifest`/
`spawn_service_with_program_data`/`spawn_adapter`/`log_dir_for`/`newest_state`/
`wait_extension_connected`) now backs every integration test that spawns the binary, since EVERY
MCP invocation is now a two-process (SERVICE + ADAPTER) pair, not one.

**H6's original RE-ISSUED run text below is superseded by the DONE entry above; kept for
provenance.** H6 is RE-ISSUED and is NEXT (`H6-detached-lifecycle-antisquat.md`, now "always-ready service +
thin adapters + anti-squat") under the ADR-0030 Decision 8 amendment -- see the RESOLVED note lower
in this section. H0-H5 are DONE and pushed to `origin/dev`.** H0 landed (pure
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

**H6 is BLOCKED (see the H6 Log entry below) on a genuine requirements conflict discovered
before any code was written**: ADR-0030 Decision 8's detached, job-breakaway-verified SERVICE
process is structurally incompatible with `tests/peer_death.rs::native_host_exits_when_server_dies`
staying green unmodified, per this task's own "Keep green (do not modify)" list. The frontier
author must reconcile the two (see the H6 Log's "What is needed to proceed") before H6 can be
re-issued and re-attempted. No working-tree changes were made or reverted.

**RESOLVED 2026-07-04 by a frontier-author AMENDMENT (ratified by the user), NOT a patch.** The H6
block exposed that the whole "detached, job-breakaway-verified child service" mechanism was the wrong
mechanism (Windows in-job breakaway is not reliably achievable, and H2's in-process-service election
had welded the service's lifetime to the first client -- a latent orphan cascade). ADR-0030
Decision 8 (and Decision 1's role topology) were AMENDED to the ALWAYS-READY-SERVICE model:

- The service is a STANDALONE process started by argv (`ghostlight service`), launched by an OS
  supervisor (new H9) or the user; it owns both endpoints + the extension link, multiplexes adapter
  sessions, runs NO parent-death watchdog, and idle-grace-shuts-down. Every MCP invocation is a THIN
  ADAPTER (connect + relay + supervisor self-heal if down; dies with its editor). Role is decided by
  ARGV, not a claim race. NO promotion, NO in-process service, NO on-demand in-editor spawn, NO
  breakaway -- that mechanism is DELETED, not built.
- "The one sacred thing is user DELIGHT": `peer_death.rs`, `mcp_protocol.rs`, and the one spawning
  `all_open_golden.rs` test are MOVABLE HARNESS -- H6 updates their spawn choreography to the
  standalone-service topology, preserving every assertion. The trained schemas + the extension wire
  stay frozen (they ARE the delight).

Executed amendment (all committed with this LEDGER update): ADR-0030 Decision 8/1/Migration/
Consequences/Provenance + the delight reframe of "Preserved invariants"; PINS.md SS5 REWRITTEN
(SS5.1-SS5.6: argv dispatch, thin adapter + self-heal, anti-squat HMAC, idle-grace, label/doctor
re-scope, deps) + SS8 seam corrected (`start_service`); H6 re-authored to the model; NEW H9
(installer auto-start); BOOTSTRAP sequence (H0-H9) + never-touch reframed. Additive Cargo.toml deps
are now `hmac`/`sha2`/`getrandom` ONLY (NO windows-sys job/crypto features -- breakaway is deleted).

Cross-cutting note for H7/H8 (and stale phrasing in H3/H4/H5/SS9): after the amendment the SERVICE
NO LONGER serves its own stdio session (it is a standalone `ghostlight service`; every session is an
adapter or, at H8, a web session). Wherever an older task file says "the service's own lone stdio
session," ignore that clause -- it no longer exists. The substantive point it supported (every
session carries a REAL `SessionGuid`, there is no `None` branch; new shared cross-session state is a
`ServiceContext` field) is UNCHANGED. H6 adds the `live_sessions` field the same way.

## Status

| Task | Title | Status | Commit | Notes |
| --- | --- | --- | --- | --- |
| H0 | Extract the HubCore composition root | DONE | a4e87b6 | |
| H1 | Transport-generic serve_session + ServiceContext | DONE | 4463b07 | |
| H2 | Persistent service + thin adapter + multiplex | DONE | 96a54fb | landed on the RE-ISSUED, two-endpoint-amended task; prior BLOCKED attempt superseded, see Log |
| H3 | Adapter-minted GUID identity + peer-cred binding | DONE | 81b3bea | RE-ISSUED after PINS.md SS9 fix; prior BLOCKED attempt superseded, see Log |
| H4 | Binary-authoritative cross-session tab isolation | DONE | 1490951 | |
| H5 | Reconnect grace window + honest bounded queue | DONE | 33b361d | |
| H6 | Always-ready service + thin adapters + anti-squat | DONE | 927d102 | RE-ISSUED run landed on the Decision 8 amendment (was BLOCKED); see Log + RESUME HERE |
| H7 | Tab-group-per-session presentation | DONE | f12a728 | crossed the JS boundary; see Log |
| H8 | Local web API = TCP; bind per policy | DONE | af1d0f8 | channels.rs is the sole sanctioned governance addition; a7 needed no edit; see Log |
| H9 | Installer auto-start (register+start supervisor) | DONE | 375810a | best-effort, outside the existing Tally/exit_result pipeline; see Log |

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

**RE-ISSUED RUN (2026-07-05, DONE, commit 927d102).** Verified all as-of-authoring facts in the
re-authored `H6-detached-lifecycle-antisquat.md` and PINS.md SS5 (rewritten)/SS8 against the live
tree before writing any code: `src/hub/mod.rs`'s `run_mcp_server`/`run_as_service`/`run_as_adapter`/
`ServiceContext` matched H2-H5's landed shape exactly (still claim-then-elect, still a lone-stdio
session on the winner); `src/hub/role.rs` (`assert_adapter_role`) matched PINS.md SS8 verbatim;
`src/transport/native/ipc.rs`'s `relay_adapter`/`handle_adapter_connection`/`serve_adapters`/
`claim_adapter_endpoint` were present under those exact names (SS1/SS9); `src/debug.rs::log_dir()`
resolved the per-user `dirs::data_local_dir()/ghostlight` (`GHOSTLIGHT_LOG_DIR` override honored),
confirming the SS5.3 STOP precondition's assumption held. No STOP precondition fired.

Implemented per the re-authored task + PINS.md SS5.1-SS5.6/SS8:
- `src/main.rs`: new `Command::Service` unit variant (doc comment PINNED verbatim by SS5.1) + one
  new `main()` match arm calling `ghostlight::hub::run_service`; the `command: None` arm is
  UNCHANGED (still calls `run_mcp_server`); the module doc's role list updated (adapter/service/
  native-host/installer).
- `src/hub/mod.rs`: `run_mcp_server` reshaped into the pure thin ADAPTER entry point (`role::set_role
  (Role::Adapter)` first; a `--manifest`/`GHOSTLIGHT_MANIFEST` sets only the PINNED one-line warning,
  never loads policy; `build_debug_sink(_, "adapter")`; `sweep_orphans()`; captures `parent`; calls
  the new `run_as_adapter(endpoint, sink, parent)`). New `run_service` (the `service` subcommand's
  entry point: `role::set_role(Role::Service)` first; resolves/loads policy exactly as the OLD
  `run_mcp_server` did; `build_debug_sink(_, "mcp-server")`; calls the new async `run_service_loop`).
  `run_service_loop` claims the adapter/control endpoint (SessionBusy -> log + exit 0, a
  single-instance guard only, never an election), calls `antisquat::load_or_create_hub_key()` once
  (best-effort; a failure degrades anti-squat rather than refusing to start), spawns the UNCHANGED
  extension `ipc::serve`, builds `ServiceContext::from_startup` ONCE, spawns `ipc::serve_adapters`,
  and returns the new `idle_grace_watch(ctx)` future as its own return value -- NEVER serves its own
  stdio as a session (the OLD lone-stdio-session branch is deleted entirely, per the amendment).
  `idle_grace_watch` implements PINS.md SS5.4's pinned loop verbatim (`IDLE_POLL` = 1s poll,
  `IDLE_GRACE` = 30s). `run_as_adapter` (new) mirrors the OLD `run_as_service`'s exact
  Notify+`tokio::select!` shape, relocated: spawns the ADR-0029 watchdog only if `Some(parent)`,
  then races `ipc::relay_adapter` against the shutdown notify. `ServiceContext` gained
  `live_sessions: Arc<AtomicUsize>` (PINS.md SS5.4, built as `AtomicUsize::new(0)` in
  `from_startup`). New `pub mod antisquat;`/`pub mod supervisor;` declarations; `IDLE_GRACE`/
  `IDLE_POLL` consts.
- `src/hub/supervisor.rs` (new): `SUPERVISOR_TASK_NAME`/`SUPERVISOR_LABEL`/`SUPERVISOR_UNIT` (PINNED
  verbatim, SS5.2); `supervisor_start_command()` (PINNED program+args per platform, cfg-split, pure,
  unit-tested, NEVER executed by that test); `SELF_HEAL_RETRY_WINDOW`/`SELF_HEAL_RETRY_INTERVAL`/
  `SELF_HEAL_FAILURE_MESSAGE` (PINNED verbatim); `start_service()` (`assert_adapter_role("start_service")`
  first, per SS8, then best-effort spawns the platform command, ignoring any failure).
- `src/hub/antisquat.rs` (new): `REFUSAL_MESSAGE` (PINNED verbatim, SS5.3); `load_or_create_hub_key`
  (SERVICE-only: reads an existing 32-byte `hub-key` under `debug::log_dir()`, else creates one via
  `getrandom::getrandom`, `0600` on Unix); `read_hub_key` (ADAPTER-only: errors on a missing or
  wrong-length file, never creates one -- the adapter does not own the secret's lifecycle);
  `compute_mac_hex`/`verify_mac_hex` (HMAC-SHA256 via the `hmac`/`sha2` crates, `Mac::verify_slice`
  for constant-time comparison; hex encode/decode hand-rolled, no new hex crate, per the project's
  own "hand-rolled when simple enough" style).
- `src/transport/native/ipc.rs`: `ROLE_SERVICE_PROOF` added to `src/hub/handshake.rs` (PINNED,
  SS5.3). The adapter/control wire gained a THIRD framed message (SS5.3's two-phase-plus-proof):
  `handle_adapter_connection`'s `Admission::Admitted` arm now calls the new `send_service_proof`
  (computes the HMAC over the EXACT hello bytes already read, keyed by `load_or_create_hub_key()`,
  sends `{"hub":1,"role":"service-proof","mac":"<hex>"}`) BEFORE calling `serve_session`; a proof
  send failure refuses the connection cleanly, never reaching the chokepoint. `relay_adapter`'s
  dial is now the new `dial_with_self_heal` (a single `dial_once` attempt; on failure,
  `supervisor::start_service()` once, then bounded retries per SS5.2) instead of the always-retrying
  `connect()`; after sending its own hello, it calls the new `verify_service_proof` (reads the framed
  proof via `read_hub_key()`, recomputes the HMAC over its OWN sent hello bytes, verifies via
  `hmac::Mac::verify_slice`) BEFORE the raw relay; ANY failure (missing/unreadable key, unreachable
  peer, malformed frame, wrong role, MAC mismatch) logs the ONE pinned `REFUSAL_MESSAGE` and returns
  an `Err`, never relaying. Two new private per-platform `dial_once` fns (a single, non-retrying dial
  attempt, cfg-split like `connect`/`serve`). The module doc's endpoint description updated for the
  new anti-squat step.
- `src/doctor.rs` (SS5.5, re-scoped ONLY the reap-target filters/text, per the pin's own scoping):
  `orphan_pids`'s and `reap`'s role filters changed from `"mcp-server"` to `"adapter"`; the run_fix
  report line ("reaped N orphaned adapter session(s)"); the `DoctorOptions::fix`/`orphan_pids`/
  `reap`/`sweep_orphans` doc comments and the `Observations::orphans`/`findings`'s orphan-count
  comment updated to say "adapter" for consistency. The HEALTH-anchor (`NewestServer` at the
  `role == "mcp-server"` filter) and the DISPLAY filters (`session_row`, the native-host-row finder)
  are UNTOUCHED, staying `"mcp-server"`, exactly as SS5.5 pins.
- `src/proc.rs`/`src/transport/watchdog.rs`: ONLY the module-doc narrative changed ("the mcp-server
  role" -> "the adapter role"); zero API change; every existing inline test green and unmodified.
- `docs/adr/0029-process-lifecycle-hygiene.md`: a short "Superseded/amended by ADR-0030 Decision 8"
  blockquote appended right after the header, before `## Context`; the historical body is untouched.
- `Cargo.toml`: added `hmac = "0.12"` and `getrandom = "0.2"` (additive; `sha2` was already present).
  NO `windows-sys` job-object/breakaway feature added, per the amendment.

D1 (PINS.md SS5.4's own forced-deviation note, transcribed): `ServiceContext` gaining
  `live_sessions` broke `tests/hub_isolation.rs`'s and `tests/hub_queue.rs`'s own `build_ctx`
  (missing-field compile errors) -> added one line to each (`live_sessions:
  Arc::new(AtomicUsize::new(0))`), the exact same category as H5's own D1. Impact on later tasks:
  none -- H7/H8's own `ServiceContext` constructions must include it too, alongside
  `session_registry`/`owned_tabs`/`mint_quota`.
D2: `tests/hub_multiplex.rs::adapter_endpoint_two_phase_wire_round_trips` is not named by this task
  and is not on any sanctioned-exception list, but it directly assumed a bare `ghostlight`
  invocation could win the adapter/control claim and become the service (H2/H3-era behavior) --
  H6's argv-dispatch reshape makes that assumption categorically false (a bare invocation is ALWAYS
  the adapter now), and separately, the NEW framed anti-squat proof message (SS5.3) sits on the
  same wire this test hand-walks byte-by-byte -> updated it to spawn `ghostlight service`
  (`support::spawn_service`, below) and to read-and-discard one extra framed message (asserting only
  its `role` field, `"service-proof"`) before the raw phase, preserving every original assertion
  verbatim. Impact on later tasks: H7/H8's own low-level wire tests should follow the same
  `support::spawn_service` pattern and expect the proof frame between the hello and the raw phase.
D3: six further pre-existing integration tests, none named by this task and none on any sanctioned
  list, each spawned ONE bare `ghostlight` invocation with `--manifest`/`ProgramData` set directly on
  it and expected THAT process to load policy and serve the governed session itself
  (`tests/hot_reload.rs::org_policy_hot_swap_end_to_end`,
  `tests/manifest_validation.rs::org_policy_file_with_config_boots_the_server`,
  `tests/shadow_mode.rs::enforce_blocks_observe_dispatches_and_records_shadow_deny`,
  `tests/tool_advertisement.rs`'s two tests, `tests/tool_enforcement.rs`'s ten tests) -> this is a
  direct, mechanical consequence of this task's OWN Required Behavior item 1 (SS5.1: "It NEVER
  claims the endpoint, loads policy, builds a Browser, or builds a ServiceContext" -- a bare
  invocation ignores `--manifest`/never reads `ProgramData` at all now), not a choice made under
  latitude, and it broke every one of these pre-existing tests identically, not just the ones this
  task happens to name. Rewired each `drive()`-style helper to spawn `ghostlight service` (carrying
  the manifest/`ProgramData`) plus a thin adapter dialing it, preserving every existing assertion
  verbatim. While doing so, ALSO fixed each helper to read exactly its expected `id`-bearing replies
  BEFORE closing the adapter's stdin (rather than dropping stdin immediately then reading to EOF):
  `relay_adapter`'s two copy directions are RACED via `tokio::select!` (PINS.md SS1 pin 3's
  lifecycle-shape mirror of `relay_native_host`), so an early stdin close can win the race and tear
  the whole relay down before a still-in-flight reply (in the OLD single-process model this never
  mattered, since the read loop ending never stopped the independent writer task from draining every
  spawned tool-call's reply first). No test in this batch happened to exercise a multi-request,
  early-stdin-close pattern against a REAL adapter relay before H6 made the adapter the ONLY path,
  so this is a newly-exposed hazard, not a regression this task introduced by choice. Impact on
  later tasks: H7/H8's own subprocess-driven tests should read all expected `id`-bearing replies
  before ever closing a client-side write half against an adapter connection.
D4 (test-support infra, not itself a deviation from a pinned value but load-bearing for D2/D3 and
  for this task's own named tests): created `tests/support/mod.rs` (the task's own suggested
  module), exposing `spawn_service`/`spawn_service_with_manifest`/`spawn_adapter` (PINNED
  signatures, transcribed) plus `spawn_service_with_program_data`/`log_dir_for`/`newest_state`/
  `wait_extension_connected` (author's own latitude, needed by D3's `ProgramData`-carrying tests and
  by `peer_death.rs`/`hub_lifecycle.rs`'s own debug-state polling). Included via `mod support;` in
  every rewired file: `tests/mcp_protocol.rs`, `tests/all_open_golden.rs`, `tests/peer_death.rs`,
  `tests/hub_lifecycle.rs` (new), `tests/hub_multiplex.rs`, `tests/hot_reload.rs`,
  `tests/manifest_validation.rs`, `tests/shadow_mode.rs`, `tests/tool_advertisement.rs`,
  `tests/tool_enforcement.rs`. `#![allow(dead_code)]` at the module's top since not every including
  binary uses every helper (a `pub fn` unused in one bin-crate integration-test binary is still dead
  code under `-D warnings`, unlike in a lib crate).

New `tests/hub_lifecycle.rs` (3 tests, all named by the task, all green):
`service_survives_the_spawning_adapter_exit` (kills the spawning adapter, asserts
`ghostlight::proc::pid_exists(service_pid)` stays true well within `IDLE_GRACE`);
`adapter_cannot_complete_handshake_with_an_impostor_service` (the adapter is pointed at a
genuinely-empty `GHOSTLIGHT_LOG_DIR` distinct from the real service's, so `read_hub_key()` fails --
PINS.md SS5.3's "missing/unreadable key" failure mode -- asserting the adapter aborts within
seconds, never relays, and surfaces the PINNED refusal text verbatim on stderr);
`supervisor_start_asserts_adapter_role` (text-scan, mirrors `tests/hub_role_wiring.rs`'s own
pattern). `src/hub/supervisor.rs`'s own `#[cfg(test)]` module additionally carries
`supervisor_start_command_is_pinned_for_this_platform` (the task's "Unit-test
`supervisor::supervisor_start_command()`" item, matching the task's own
`cargo test --lib -- hub::supervisor` verification command) and a self-heal-window sanity check.

Verification: all four commands passed for real. `cargo build --all-targets` clean.
`cargo test --test hub_lifecycle` (3/3); `cargo test --test peer_death` (1/1); `cargo test --test
mcp_protocol` (6/6); `cargo test --test all_open_golden --test tool_schema_fidelity --test
architecture` (3 + 7 + 5 = 15, all green); `cargo test --lib proc` (10/10, including the pre-existing
`proc::tests::*`); `cargo test --lib watchdog` (2/2); `cargo test --lib -- hub::supervisor` (2/2).
`cargo clippy --all-targets -- -D warnings` clean (two lints fixed along the way:
`clippy::io_other_error` in `antisquat::load_or_create_hub_key`, `clippy::manual_is_multiple_of` in
`antisquat::hex_decode`). `cargo fmt --all -- --check` clean (after one `cargo fmt --all` pass to
normalize wrapping in the new code, per H0-H5 precedent -- whitespace only). The FULL `cargo test` is
green (442 lib tests -- unchanged from H5, since H6 added no new lib-level assertions beyond the
antisquat/supervisor/role modules' own inline tests, which the 442 total already includes -- plus
every integration suite, 0 failed): this run ALSO caught and fixed 6 collateral test-topology breaks
NOT named by this task (D2/D3 above) before declaring the FULL suite green, since the task's own
completion criterion is literally "the FULL cargo test must be green," not merely the named
suites. Sacred tests (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`'s CLIENT-VISIBLE
assertions, `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`) green and
byte-unmodified (`git diff --stat` on `tests/tool_schema_fidelity.rs`, `src/transport/mcp/tools.rs`,
`src/transport/native/host.rs`, `tests/architecture.rs`, `src/transport/executor.rs`: no output).
`git status --porcelain` after staging shows exactly the files this entry names (26 total: 4 new,
22 modified); no NEVER-touch fence moved -- the only sanctioned exceptions touched are the
`tests/all_open_golden.rs`/`tests/peer_death.rs`/`tests/mcp_protocol.rs` spawn-choreography ones this
task itself grants.
- Note: as in H0-H5, `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's
  `target/`) because four live `ghostlight.exe` processes (this environment's own dogfooded
  MCP/native-host session) held the repo's `target/debug/ghostlight.exe`; build-artifact routing
  only, not a source or test change.

### H7
- Verified all as-of-authoring facts in `H7-tab-group-per-session.md` and PINS.md SS6/SS9 against
  the live tree: `ServiceContext.owned_tabs` present exactly as SS9 describes (`src/hub/mod.rs`);
  the ownership-gate/adoption logic runs in `check_tab_ownership` inside
  `serve_session`'s read loop (`src/transport/mcp/server.rs`), NOT `src/hub/session.rs`, matching
  the task's own CORRECTED note; the shared `Browser` handle on `ServiceContext` is the only extant
  send seam to the extension (`Browser::call`/`Browser::tab_url` via `send_and_await`,
  `send_hold_reply`'s fire-and-forget pattern for an id-less reply); `extension/service-worker.js`
  matched the task's line-~29/32/490/514/517/1063 description closely enough (drift only in exact
  line numbers, expected per BOOTSTRAP authority order item 4). No STOP precondition fired.
- Implemented per the task's Required Behavior items 1-4 and PINS.md SS6, keeping the change
  additive throughout:
  - `src/hub/session.rs`: new `TabClaim` enum (`Owned`/`Adopted`/`Refused`) and `claim_tab` (the
    same first-touch-adoption operation as the existing `owns_or_adopts_tab`, but reporting which
    outcome occurred); `owns_or_adopts_tab` is now `!matches!(claim_tab(...), TabClaim::Refused)`
    so the two can never drift, and its own existing test/callers are untouched. New
    `owned_tab_ids` (the full, sorted, guid-filtered owned-tab set) and `group_title` (the PINNED
    `"\u{1F47B} Ghostlight <short>"` format, first 8 GUID chars). 3 supplementary (not task-named)
    unit tests added alongside H4's own.
  - `src/transport/executor.rs`: new `Browser::request_group(guid: &str, tab_ids: &[i64], title:
    &str)`, fire-and-forget over the existing `outgoing` channel (mirrors `send_hold_reply`'s
    posture exactly: encode, frame, best-effort send, silent no-op if unconnected or unencodable).
    No new routing/pending-map logic: the pinned `group_response` carries no `id`, so
    `Browser::route_reply`'s existing "no id -> not `session_killed` -> drop as an event" path
    already handles it with zero code changes. 2 supplementary tests (a no-connection no-op; a
    connected round-trip asserting the exact pinned wire shape, no `id` member, and that an
    incoming `group_response` never wedges a subsequent ordinary `call`).
  - `src/transport/mcp/server.rs`: `check_tab_ownership` now takes an added `browser: &Browser`
    parameter and matches on `TabClaim` instead of the old boolean: `Owned` -> `None` (unchanged
    pass-through), `Refused` -> the unchanged uniform-denial path, `Adopted` -> a NEW
    `emit_group_request` call (reads the session's full current owned-tab set via
    `owned_tab_ids`, computes the title via `group_title`, calls `Browser::request_group`) then
    `None`. This is the ONLY new call site, exactly as the task requires; it fires for EVERY
    session including a lone all-open one (PINS.md SS9: every session carries a real GUID), which
    is a no-op for the sacred `tool_response` wire per the task's own STOP-precondition reasoning
    (the emit is a separate, out-of-band native message, never touching the JSON-RPC reply bytes)
    -- confirmed empirically: `tests/all_open_golden.rs` stayed green byte-for-byte unmodified.
  - `src/transport/native/messages.rs`: one additive `//!` doc section, "Tab-group-per-session
    request (H7, ADR-0030 Decision 6/7)", mirroring the "Tab-URL query (g13)" section's style
    exactly as the task specifies. No existing section edited.
  - `extension/lib/grouping.js` (new): the pure grouping DECISION, `groupSessionTabs(chrome,
    sessionGroups, guid, tabIds, title)` -- probes each named tabId's liveness via
    `chrome.tabs.get` (existence only; no field of the result is ever read, closing off any
    possibility of a url/host-based policy decision), reuses `sessionGroups.get(guid)`'s existing
    Chrome group if it still resolves via `chrome.tabGroups.get`, else creates one via
    `chrome.tabs.group({ tabIds })`, always re-applies the title via `chrome.tabGroups.update`,
    and records the (possibly new) group id back into the caller's `sessionGroups` map.
  - `extension/service-worker.js`: `importScripts` gained `"lib/grouping.js"`; a new
    `group_request` branch in the existing `nativePort.onMessage` handler (alongside
    `tab_url_request`) calls `groupSessionTabs`, then `persistSessionState()`, then posts the
    PINNED `group_response` (fire-and-forget on both legs, matching item 1's shape exactly, no
    `id` member added by mistake). A NEW `sessionGroups` map (guid -> Chrome tab-group id) is
    ADDITIVE alongside the pre-existing single `groupId`/`ensureGroup`/`groupTabs`/`inGroup`
    machinery, which is completely untouched (see D1 below for why). `persistSessionState`/
    `rehydrate` gained a NEW, additive `sessionGroupsState` storage key persisting/restoring
    `sessionGroups`'s entries; the pre-existing `sessionState` key/shape is byte-identical to
    before.
  - `tests/extension/grouping.test.js` (new): the ONE task-named test,
    `owned_tabs_are_grouped_on_service_request_only`, covering all 4 pinned assertions in a single
    `test()` body as the task file itself structures them (not 4 separate `test()` calls), with
    the ADR oracle transcribed verbatim into the file's header comment exactly as instructed.

D1 (a design decision, not a tree-fact mismatch -- logged because it goes beyond the task file's
own text): Required Behavior item 2 says to "replace the single process-global `groupId` model
with a session-GUID -> groupId map." The pre-existing `groupId`/`ensureGroup`/`groupTabs`/
`inGroup`/`effectiveTabId` machinery is NOT a presentation feature -- `effectiveTabId` is the
extension's own tab-scope ACCESS-CONTROL gate, called by nearly every tool handler, and `inGroup`
decides membership by comparing a tab's LIVE `chrome.tabs.get(id).groupId` against the single
in-memory `groupId`. The sacred `tool_request` wire (frozen; ADR-0030 "Preserved invariants") never
carries a session GUID, so this mechanism cannot be made session-aware by construction -- there is
no session identity available at the point `effectiveTabId` runs. Read literally, "replace" would
mean calling `chrome.tabs.group` with a session's groupId on every first-touched tab, which moves
that tab OUT of the legacy single group (a Chrome tab belongs to at most one group at a time) and
would make `inGroup`/`effectiveTabId` refuse it on the very next call -- a real functional
regression with no pinned test covering it either way. Chose the conservative, minimal-footprint
reading instead: interpreted "replace the single process-global model" as describing the SHAPE of
the NEW state this task introduces (a map from the start, never "yet another single global var"),
built it as a genuinely SEPARATE, additive `sessionGroups` map, and left the pre-existing
`groupId`/`ensureGroup`/`groupTabs`/`inGroup`/`effectiveTabId`/`tabs_create_mcp`/`tabs_context_mcp`
functions completely untouched (all still current-tree facts the task file itself only describes,
never lists under Required Behavior as something to change). This satisfies every pinned assertion
(all 4, `tests/extension/grouping.test.js`) and every STOP precondition (no sacred wire touched, no
all-open byte drift, no policy decision, additive-only) to the letter. Impact on later tasks: NONE
directly (H8/H9 do not touch tab grouping), but the frontier author should be aware that in REAL
multi-session usage, a tab created via `tabs_create_mcp` (which still auto-groups into the legacy
single group, unchanged) will visually move into its session's NEW per-session group the first time
any OTHER tab-scoped tool call adopts it and triggers `emit_group_request` -- at which point the
legacy `inGroup`/`effectiveTabId` check for that tab now compares against a `groupId` the tab no
longer belongs to, and would refuse it. Since ADR-0030 Decision 6 already frames the extension's
own group check as "defense-in-depth only" (the SERVICE's `owned_tabs` gate is the real isolation
boundary, and it does not consult `groupId`/`inGroup` at all), this is a live-usage interaction, not
a security regression -- but it is untested by anything in this batch and may need a follow-up task
(e.g., relaxing `inGroup` to accept membership in ANY tracked group, or teaching `effectiveTabId`
about `sessionGroups`) once H7 lands in real dogfooding. Flagged here rather than silently designed
around, per the Failure protocol's spirit of never inventing an oracle or improvising past a gap the
task file's own text does not resolve.

Verification: all four literal commands from the task file passed for real. `cargo build
--all-targets` clean. `cargo test --test all_open_golden --test tool_schema_fidelity` both green
(3/3, 7/7) -- `all_open_golden.rs` byte-unmodified and still passing confirms the emit path is a
true no-op for the sacred wire. `node --test tests/extension/grouping.test.js
tests/extension/geometry.test.js` both green (10/10 total); the full extension suite (`+
constants.test.js keys.test.js`) also re-run, 18/18 green. `cargo clippy --all-targets -- -D
warnings` clean. `cargo fmt --all -- --check` clean (after one `cargo fmt --all` pass to wrap a
line in `src/hub/session.rs::owned_tab_ids`'s signature -- whitespace only, no semantic change, not
logged as its own numbered deviation). The FULL `cargo test` (not just the task's two named
targets) was also run: 447 lib tests + every integration suite green, including the sacred/named
suites (`tests/tool_schema_fidelity.rs` 7/7, `tests/all_open_golden.rs` 3/3,
`tests/architecture.rs::governance_core_has_no_forbidden_back_edges` and its 4 siblings, all green
and byte-unmodified) and every OTHER existing suite that touches a `tabId` over a live session
(`tests/hub_isolation.rs` 2/2) -- traced by hand first (both its tests only exercise the REFUSED
path for session B, and session A's ownership is pre-seeded directly on the map rather than driven
live, so neither test's fake extension -- which panics on any message type it does not recognize --
ever receives an unexpected `group_request` frame) and then confirmed green. `git diff --stat`
shows exactly `extension/service-worker.js`, `src/hub/session.rs`, `src/transport/executor.rs`,
`src/transport/mcp/server.rs`, `src/transport/native/messages.rs` modified plus the two new files
(`extension/lib/grouping.js`, `tests/extension/grouping.test.js`); no NEVER-touch fence moved
(`src/transport/mcp/tools.rs`, `tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
`src/transport/native/host.rs`, every EXISTING native-messaging message shape, the MCP JSON-RPC
wire, and `Browser::attach`'s single-link rejection are all byte-identical to before this task).
- Note: as in H0-H6, `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's
  `target/`) because four live `ghostlight.exe` processes (this environment's own dogfooded
  MCP/native-host session) held the repo's `target/debug/ghostlight.exe`; build-artifact routing
  only, not a source or test change.

### H8
- Verified all as-of-authoring facts in `H8-web-api-loopback-policy.md` against the live tree
  before writing any code: H2/H3/H4 all landed (Status table above, all DONE); `serve_session<S>`
  (`src/transport/mcp/server.rs`) takes a plain `guid: SessionGuid` (never `Option`), exactly as
  PINS.md SS9 describes; `ServiceContext` (`src/hub/mod.rs`) holds `session_registry`/`owned_tabs`/
  `mint_quota`/`live_sessions` as plain fields, built once in `from_startup`; `DecisionRequest`
  (`src/governance/ports.rs`, around the task's cited line range) had no `channel_source`-shaped
  field yet; `Grant` (`src/governance/manifest/document.rs`) has no `channels` field, confirming
  the task's own framing that this batch realizes only the minimal flat allowlist, never the full
  recursive grammar; `tests/architecture.rs`'s a7 scanner (`FORBIDDEN_CRATE_EDGES`,
  `FORBIDDEN_IDENTIFIERS = ["tabId", "token", "socket"]`) matched the task's description exactly.
  No STOP precondition fired.
- Implemented per the task's Required behavior items 1-6 and PINS.md SS7/SS2/SS9:
  - `src/governance/channels.rs` (new; the ONE sanctioned `src/governance/**` addition): PINNED
    rule label `RULE_WEBAPI_FROM = "channel/webapi_from"`; `is_member` (exact match, or `"*"`);
    `validate_webapi_from` (fail-closed on anything but a flat JSON array of non-empty strings);
    `decide_webapi_from` (the pure Allow/Deny decision, denial via the existing
    `denial::denial_id` scheme); `ChannelsPdp`, a `PolicyDecisionPoint` impl constructed with the
    resolved allowlist and deciding ONLY `DecisionRequest.channel_source` -- it never reads
    `tool`/`resource`/`requires`, so it structurally cannot gate which tools exist (the STOP
    precondition on this point never had a code path to trigger). `pub mod channels;` added to
    `src/governance/mod.rs`.
  - `src/governance/ports.rs`: `DecisionRequest` gains `pub channel_source: Option<String>` (the
    ONE resolved field the task's Required behavior item 4 describes; the allowlist itself is
    held by `ChannelsPdp`, not the request, mirroring how `LocalPdp` holds its own `evaluate_host`
    fn rather than the request). All 3 existing test constructions in `ports.rs` updated with
    `channel_source: None`.
  - `src/governance/dispatch.rs`: the ONE production `DecisionRequest` construction (inside
    `Governance::decide`) stamps `channel_source: None` -- the tool-call chokepoint never carries
    a connecting-source axis; byte-identical for every existing caller.
  - `src/hub/webapi.rs` (new): `builtin_webapi_from()` (`["localhost"]`, the web adapter's own
    builtin default fragment, ADR-0030 Decision 5); `resolve_bind(allowlist: &[String]) ->
    &'static str` (the pure, ONE-argument "resolved allowlist -> bind address" function PINS.md
    SS7/the task's pinned tests require -- `DEFAULT_WEBAPI_BIND = "127.0.0.1"` unless the
    allowlist names anything other than `"localhost"`, in which case `REMOTE_WEBAPI_BIND =
    "0.0.0.0"`); `DEFAULT_WEBAPI_PORT = 4180` (PINS.md SS7); `classify_source` (loopback peer IP
    -> `"localhost"`, else the literal address -- the channels vocabulary). The real listener:
    `run(ctx)` binds per `resolve_bind`, and on ANY bind failure LOGS and returns rather than
    panicking or propagating (spawned fire-and-forget from `run_service_loop`, exactly like the
    extension endpoint's `SessionBusy` handling, since `tests/support::spawn_service` spawns the
    real `ghostlight service` binary in several existing suites and a fixed TCP port cannot be
    made per-test-unique the way the named-pipe/UDS endpoint already is -- this task introduces NO
    risk of a second bind attempt ever aborting a test's service process). `handle_connection`
    parses the HTTP/1.1 request line + headers (own minimal parser, no new crate), validates
    `Host` against the resolved bind (`host_is_expected`, the DNS-rebind defense, Required
    behavior item 5) and the connecting source's `Origin` (falling back to the classified peer
    address when absent -- Required behavior item 3, anonymous is a first-class principal, no
    hardcoded auth gate) against `ChannelsPdp::decide`, completes the RFC 6455 handshake
    (`compute_accept_key` via hand-rolled SHA-1 + base64 -- a well-defined public standard's fixed
    algorithm, not a project decision requiring a pin; self-verified against RFC 6455 section
    1.3's own worked example, `dGhlIHNhbXBsZSBub25jZQ==` -> `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`), then
    mints a `SessionGuid` (no `SessionRegistry::admit` -- PINS.md SS9's forward guidance for H8:
    a remote TCP peer has no OS credential to bind) and calls the UNCHANGED
    `transport::mcp::server::serve_session` -- the SAME chokepoint every MCP adapter session
    enters, with NO changes to that function's signature or body (Required behavior item 1;
    "do NOT modify serve_session; add the web listener as a SECOND caller only"). `WsStream`
    (an `AsyncRead + AsyncWrite` adapter) tunnels raw bytes through minimal, unfragmented RFC 6455
    data frames (own hand-rolled `encode_frame`/`decode_frame`, masked-client-frame enforcement,
    close-frame-as-EOF, ping/pong parsed and discarded) so `serve_session`'s existing
    `BufReader::lines()` read loop and `write_chunked` writer need no awareness that the stream is
    WS-framed underneath -- see the module doc and the Log's scope-limitation note below for why
    this subset is sufficient and deliberate, not an oversight. `src/hub/mod.rs`: `pub mod
    webapi;` added; `run_service_loop` spawns `webapi::run(ctx.clone())` alongside the existing
    extension/adapter endpoint spawns (H8's only edit to this file).
  - `tests/channels_policy.rs` (new; the ONE task-named test):
    `webapi_from_is_decided_in_the_pdp_on_the_subject`, driving `ChannelsPdp::decide` directly (no
    listener) for both the Allow and Deny cases, asserting the PINNED rule label and the `"D-"` +
    8-lowercase-hex `denial_id` shape.
  - `tests/webapi_auth.rs` (new; the 3 task-named tests):
    `webapi_builtin_default_is_loopback_only_with_no_overlay` (asserts the builtin resolves to
    `["localhost"]` and `resolve_bind` returns `127.0.0.1`, never `0.0.0.0`);
    `enabling_remote_is_a_user_policy_change_not_a_code_gate` (models the resolved allowlist a
    user-layer `[allow: "*"]` overlay would produce, `vec!["*".to_string()]`, and asserts the SAME
    one-argument `resolve_bind` now returns the remote bind -- proving there is no separate
    boolean/flag/env input); `anonymous_is_a_valid_principal_under_all_open` (asserts
    `ChannelsPdp::decide` allows an anonymous loopback source under the builtin default with no
    denial, THEN reproduces `tests/audit_recorder.rs`'s own pinned 14-key-order/`identity: null`
    assertion directly against a lone all-open `Governance` + file-backed `Recorder`, proving H8
    introduced no drift to the frozen `AuditRecord` shape or the all-open byte-identity invariant,
    per PINS.md SS2's resolution: no 15th audit key, ever).
  - Supplementary (not task-named) unit tests added directly in `channels.rs`'s and `webapi.rs`'s
    own `#[cfg(test)]` modules (mirroring H5's precedent of adding a few unpinned-but-valuable
    tests alongside the task-named ones): `is_member`/`validate_webapi_from` cases in `channels.rs`;
    `resolve_bind`/`classify_source`/the RFC 6455 accept-key worked example/a masked-frame
    encode-decode round trip/`decode_frame`'s incomplete- and unmasked-frame handling/
    `host_is_expected`/`origin_hostname` in `webapi.rs`.

D1: one early doc-comment draft in `channels.rs`'s module doc literally spelled out
`crate::browser`/`crate::transport`/`crate::mcp`/`crate::native` in descriptive prose (explaining
which crate edges the module avoids) -> `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`
FAILED on first run: the crate-edge/`url` checks scan doc comments too (unlike the
`FORBIDDEN_IDENTIFIERS` check, which is code-line-only), so naming the forbidden paths even in
prose trips the scanner. Reworded the doc comment to describe the boundary without spelling out
the forbidden paths ("names none of the forbidden crate edges the architecture test guards").
`tests/architecture.rs` itself was NOT edited -- confirming the task's own framing that the
sanctioned a7 exception was available if needed, but this task's actual code (and now its doc
comments) never triggers it. Impact on later tasks: none; a reminder for any future
`src/governance/**` doc comment that wants to describe what it avoids.
D2: PINS.md SS7 describes `channels.webapi.from`/`webapi.bind`/`webapi.port` as "a resolved config
value," implying eventual `ConfigStore`-layered resolution (the ADR-0019 five-layer system), but no
task-named test drives that layering, and `ConfigStore`'s typed key registry
(`src/governance/config/schema.rs`, `KEYS`) is pinned by its own golden tests
(`tests/config_schema_golden.rs`), a file this task does not name -> deferred full `ConfigStore`
wiring for `channels.webapi.from`/`webapi.bind`/`webapi.port`; the running service always resolves
to `builtin_webapi_from()` (never reading a user/org overlay) when it spawns `webapi::run` from
`run_service_loop`. The pure `resolve_bind`/`ChannelsPdp`/`validate_webapi_from` machinery fully
supports a resolved override once one is wired in; only the "read it from `ConfigStore`" wiring
itself is deferred. Impact on later tasks: none named by this batch (H9 does not touch the web
API), but a future task adding a real `channels.webapi.from`/`webapi.bind` config key should route
it through `ConfigStore` exactly as every other layered key is, then pass the resolved allowlist
into `webapi::run` instead of the hardcoded builtin default.

Verification: all commands from the task's literal block, plus the full suite, passed for real.
`cargo build --all-targets` clean. `cargo test --test channels_policy` (1/1), `cargo test --test
webapi_auth` (3/3), `cargo test --test architecture governance_core_has_no_forbidden_back_edges`
(green, UNMODIFIED file), `cargo test --test all_open_golden` (3/3, byte-unmodified),
`cargo test --test tool_enforcement` (10/10), `cargo test --test tool_schema_fidelity` (7/7) all
green. The FULL `cargo test` (not just the task's named targets) was also run: 460 lib tests +
every integration suite green (0 failed), including every sacred/named suite above plus
`hub_multiplex`/`hub_isolation`/`hub_lifecycle`/`hub_queue`/`hub_identity`/`hub_role_wiring`/
`mcp_protocol`/`peer_death`/`audit_recorder`/`config_schema_golden` and every other existing suite,
all green and byte-unmodified. `cargo clippy --all-targets -- -D warnings` clean. `cargo fmt --all
-- --check` clean (after one `cargo fmt --all` pass to wrap a handful of lines in `channels.rs`/
`webapi.rs` -- whitespace only, no semantic change, not logged as its own numbered deviation).
Sacred tests (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
`tests/architecture.rs::governance_core_has_no_forbidden_back_edges`) green and byte-unmodified;
`git diff --stat` (pre-commit) showed exactly `src/governance/dispatch.rs`, `src/governance/mod.rs`,
`src/governance/ports.rs`, `src/hub/mod.rs` modified plus 4 new files (`src/governance/channels.rs`,
`src/hub/webapi.rs`, `tests/channels_policy.rs`, `tests/webapi_auth.rs`) -- no NEVER-touch fence
moved; the sanctioned a7 exception for this task was available but never needed.
- Note: as in H0-H7, `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's
  `target/`) because a live dogfooded `ghostlight.exe`/native-host session held the repo's
  `target/debug/ghostlight.exe`; build-artifact routing only, not a source or test change.

### H9
- Verified all as-of-authoring facts in `H9-installer-autostart.md` against the live tree before
  writing any code: `src/hub/supervisor.rs` (H6) defines `SUPERVISOR_TASK_NAME`/`SUPERVISOR_LABEL`/
  `SUPERVISOR_UNIT` exactly as PINS.md SS5.2 pins them (confirmed by reading the file directly);
  `src/install/native_host.rs::normalize_exe_path` is the existing exe-path resolution already
  reused by both `HostManifest::resolve` and `clients::server_entry`; `src/install/mod.rs` hosts
  `run_install`/`run_uninstall` with the existing `Action`/`Op`/`Tally`/`exit_result` pipeline
  exactly as described; `libc` is already an unconditional `[target.'cfg(unix)'.dependencies]`
  entry (needed for macOS's `libc::getuid()`, already used the same way in
  `src/hub/supervisor.rs`). No STOP precondition fired: a real exe-path resolution exists, no
  register/start action requires elevation (Task Scheduler `/rl limited`, launchd `gui/<uid>`,
  systemd `--user` are all per-user), and no NEVER-touch fence needed to move.
- Implemented per the task's pinned oracles and PINS.md SS5.2, all inside two new files plus one
  edited file (`src/install/mod.rs`, the only pre-existing file this task touches):
  - `src/install/supervisor.rs` (new): `SupervisorCommand` (`program`/`args`) and `SupervisorStep`
    (`WriteFile`/`RemoveFile`/`Run`) as the shared vocabulary; three cfg-split
    `register_steps(exe: &Path, ctx: &PlanCtx) -> Vec<SupervisorStep>` /
    `unregister_steps(ctx: &PlanCtx) -> Vec<SupervisorStep>` pairs (`#[cfg(windows)]`,
    `#[cfg(target_os = "macos")]`, `#[cfg(all(unix, not(target_os = "macos")))]`), each
    transcribing its platform's pinned oracle verbatim (argv, plist XML, unit INI). Windows'
    `register_steps` normalizes `exe` via `native_host::normalize_exe_path` before building the
    `/tr` string; macOS/Linux normalize it the same way before rendering the plist/unit. A single
    `apply_steps(label, steps, dry_run)` applies any platform's steps, printing `[plan]`/`[ok]`/
    `[warn]`/`[noop]` in the installer's existing visual style, NEVER returning an error (Required
    behavior item 4: a failed step WARNS, is logged, and is skipped -- it never aborts the caller).
  - `src/install/mod.rs`: added `pub mod supervisor;`; `run_install` now calls
    `supervisor::apply_steps("Ghostlight Service", &supervisor::register_steps(&ctx.current_exe,
    &ctx), opts.dry_run)` AFTER the existing `apply(&actions, opts.dry_run)` call and BEFORE
    `finish`; `run_uninstall` symmetrically calls `unregister_steps`. Both calls are OUTSIDE the
    existing `tally`/`exit_result` computation, so a supervisor failure can never change the
    install's exit code or its `Done: N applied, N unchanged, N failed` summary line -- the
    existing native-host/MCP-client registration behavior (idempotent value-level JSON merge) is
    completely unmodified, satisfying the task's explicit "regress nothing" instruction. The
    supervisor is registered/unregistered UNCONDITIONALLY (both `--system` and per-user installs),
    since Decision 8 pins it as always per-user regardless of `opts.system` (that flag only scopes
    the native-host/client registration, an orthogonal axis).
  - `tests/install_supervisor.rs` (new; the 3 task-named tests, each `#[cfg]`-gated to its own
    platform exactly as named): `windows_task_register_command_is_pinned` (asserts the `schtasks
    /create` step's argv contains `/tn`, `Ghostlight Service`, `/rl`, `limited`, `/sc`, `onlogon`,
    and that the `/tr` value names the `service` subcommand); `macos_plist_names_the_service_
    subcommand` (asserts the rendered plist contains `<string>service</string>` and
    `org.sylin.ghostlight.service`); `linux_unit_names_the_service_subcommand` (asserts the
    rendered unit contains `ExecStart=`, `service`, and `Restart=on-failure`). None ever executes
    `schtasks`/`launchctl`/`systemctl`, per the task's explicit scope. A supplementary (not
    task-named) `#[cfg(windows)]` unit test was also added directly in `supervisor.rs`'s own
    `#[cfg(test)]` module (`windows_register_steps_never_elevate`), asserting no `/ru` (run-as)
    argument is present and `limited` is, reinforcing STOP precondition 3 (no elevation) the same
    way earlier tasks added supplementary tests alongside task-named ones.
- No deviations from the task file.
- Verification: all four commands from BOOTSTRAP plus the task's own literal block passed for
  real, on this Windows dev box. `cargo build --all-targets` clean (one intermediate unused-import
  warning on `SUPERVISOR_LABEL`/`SUPERVISOR_UNIT` under `#[cfg(windows)]` was fixed by cfg-gating
  each import to the platform that uses it, before the final clean build -- not logged as a
  numbered deviation since it never touched a pinned assertion or oracle, only import visibility).
  `cargo test --test install_supervisor` (1/1 on this Windows host --
  `windows_task_register_command_is_pinned`; the macOS/Linux tests are compiled out here by their
  own `#[cfg]`, exactly as the task specifies, and were not executed on this run). `cargo test
  --test all_open_golden --test tool_schema_fidelity --test architecture` all green (15 tests
  across the three suites). The FULL `cargo test` was also run: 461 lib tests + every integration
  suite green (0 failed), including every sacred/named suite plus
  `hub_multiplex`/`hub_isolation`/`hub_lifecycle`/`hub_queue`/`hub_identity`/`hub_role_wiring`/
  `mcp_protocol`/`peer_death`/`webapi_auth`/`channels_policy`/`config_schema_golden`/
  `install_supervisor` and every other existing suite, all green. `cargo clippy --all-targets --
  -D warnings` clean. `cargo fmt --all -- --check` clean (after one `cargo fmt --all` pass to
  reorder the cfg-gated imports and wrap a `for` loop in the new test file -- whitespace/import-
  order only, no semantic change, not logged as its own numbered deviation, matching every prior
  task's precedent for this exact kind of fmt-only fixup). Sacred tests
  (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
  `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`) green and byte-unmodified;
  `git diff --stat` (pre-commit) showed exactly `src/install/mod.rs` modified (18 insertions, 0
  deletions) plus 2 new files (`src/install/supervisor.rs`, `tests/install_supervisor.rs`) -- no
  NEVER-touch fence moved; this task named no sanctioned exception and needed none.
- Manual smoke (NOT a cargo gate, per the task's own framing -- recorded here for the frontier
  author, not executed on this run): on each platform, run `ghostlight install`, confirm
  `ghostlight service` is running (Task Scheduler / `launchctl print` / `systemctl --user status`),
  open an editor and confirm it connects with no manual start, then `ghostlight uninstall` and
  confirm the supervisor is gone. NOT performed in this run (this box has no packaged installer
  build to smoke against yet; the pure builders are verified, the real OS registration commands
  themselves are unexercised end-to-end). Flagged for the frontier author before shipping H9 to
  real users.
- Note: as in H0-H8, `CARGO_TARGET_DIR` was pointed at a scratch directory (not the repo's
  `target/`) because four live dogfooded `ghostlight.exe` processes (this environment's own
  MCP/native-host session) held the repo's `target/debug/ghostlight.exe`; build-artifact routing
  only, not a source or test change.

## Deviation format

When you deviate from a task file (a signature differs from as-of-authoring, a helper had to move,
an oracle needed pinning), record it under that task as:

```
D<n>: <what the task said> -> <what you actually did> because <the tree fact that forced it>.
     Impact on later tasks: <none | names the task + what it must now assume>.
```

A BLOCKED entry records instead: the failed assumption (with the file/symbol actually found), the
STOP precondition or fence that triggered, and what is needed to proceed. Then HALT.
