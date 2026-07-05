# H2: Persistent SERVICE + thin ADAPTER + multiplex (repeal ADR-0004)

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 1, Decision 2,
> Decision 3, Decision 7). One task = one commit. Facts below are as-of-authoring 2026-07-04 --
> RE-READ the named files before relying on any line number.

## Goal

Introduce the persistent SERVICE role and the thin ADAPTER role and land genuine multiplex: N MCP
clients drive the ONE shared browser through one governed chokepoint. The SERVICE owns the `Browser`
+ the native pipe/UDS and spawns a `serve_session` per accepted connection over one shared
`ServiceContext`; the ADAPTER is a stdio<->service byte relay that connects to an already-running
service. This repeals ADR-0004 (reject-second) at the MCP-client layer (ADR-0030 "Relationship to
other decisions" + Decision 1) and converts the single-consumer kill hook into a fan-out registry so
every live subject gets exactly one `session_killed` audit record (ADR-0030 Decision 7). Why: the
single-owner model produces the reject-2nd / orphan / restart-dance bug class; the Hub designs it out
(ADR-0030 Context + Consequences).

## Authority

1. docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 1, Decision 2, Decision 3, Decision 7;
   "Preserved invariants" for the pinned oracles) -- NORMATIVE. Cite it; do not restate its semantics.
2. docs/tasks/hub/BOOTSTRAP.md -- ground rules, authority order, per-task procedure, failure protocol.
3. This task file.

If they conflict, the higher wins.

## Current-tree facts (as-of-authoring; RE-READ before relying)

### Dependency on H0 and H1 (STOP if absent -- see STOP preconditions)

- H0 extracts the composition root into `src/hub` (`HubCore`). As-of-authoring `src/hub` does NOT
  exist yet (`src/lib.rs` module list: browser, debug, doctor, error, governance, install, origin,
  proc, transport -- no `hub`). It is created by H0. RE-READ `src/lib.rs` and `src/hub/mod.rs` at
  execution time.
- H1 makes the MCP loop transport-generic: `serve_session<S>(stream, ctx)` + a `ServiceContext`
  holding the SHARED state (the one `Browser`, `ConfigStore`, audit `Recorder`) per ADR-0030
  Decision 2. As-of-authoring this does NOT exist: today the loop is
  `src/transport/mcp/server.rs::run(browser, loaded_policy, user_source)` (line 108), a single-session
  function tied to `tokio::io::stdin()`/`stdout()` (server.rs lines 122, 194). RE-READ the actual
  `serve_session` signature and `ServiceContext` shape that H1 landed before writing any wiring; the
  signatures in this file's "Required behavior" are the AS-OF-AUTHORING intent, not a substitute for
  reading H1's result.

### src/transport/executor.rs (the ONE forced executor change)

- `Browser` is `#[derive(Clone)]` (line 86) over `Arc` fields: `next_id: Arc<AtomicU64>` (line 88),
  `pending: Arc<Mutex<HashMap<String, oneshot::Sender<CallResult>>>>` (the `Pending` alias, lines
  71, 89). Two `serve_session` tasks cloning ONE `Browser` therefore already correlate replies by id
  with NO new code (ADR-0030 Decision 2: "its `Arc<AtomicU64> next_id` and `Arc<Mutex<HashMap>>
  pending` already correlate replies by id across clones, so multiplex needs no new correlation
  code"). This is the multiplex assumption; verify it holds (STOP precondition).
- `on_session_killed` (lines 177-183): stores a SINGLE hook -- "Registering a second hook replaces
  the first (single-consumer by construction: one `Governance` per session)". Backed by
  `kill_hook: Arc<Mutex<Option<KillHook>>>` (line 105). `handle_session_killed` (lines 495-505)
  latches the flag once (`killed.swap(true, ...)`), drains pending, then invokes the single hook if
  present. THIS is the surface converted to a fan-out registry (ADR-0030 Decision 7).
- `attach` (lines 340-402) enforces the single active EXTENSION link via the atomic `outgoing` slot
  claim, returning `AttachOutcome::AlreadyAttached` (lines 77-83, 353-355) for a stray/extra
  connection. This invariant is RETAINED (ADR-0030 "Relationship to other decisions"). Do NOT weaken
  it.
- The pinned hop-attributed error strings and `TOOL_TIMEOUT = 60s` live here (const at line 60; error
  strings at lines 65, 297, 307-308, 312, 384). Byte-frozen (ADR-0030 "Preserved invariants").

### src/transport/native/ipc.rs

- `serve(browser: Browser, endpoint: &str)` (Windows line 136; Unix line 349): owns the endpoint
  (single active session via `first_pipe_instance(true)` / `UnixListener::bind`), accept-loops, and
  hands each accepted connection to `browser.attach(...)` in a spawned task (lines 175-183 Windows,
  388-397 Unix). Endpoint creation failure maps to `Error::SessionBusy` (line 150 Windows; line 364
  Unix). Today EVERY accepted connection is an extension-link (native-host relay) connection.
- `relay_native_host(endpoint, debug)` (line 48): the native-host RELAY -- `connect`s, then
  `select!`s two byte-copy futures between this process's stdin/stdout and the IPC stream (lines
  55-77). The ADAPTER is a mirror of this on the MCP-client side.
- Module doc (lines 17-18) states "Single active session: a second mcp-server refuses with
  `Error::SessionBusy`." That semantic is being REPEALED at the MCP-client layer (see Required
  behavior).

### Composition root: src/hub/mod.rs (post-H0; NOT main.rs)

- H0 MOVED the whole `run_server` body verbatim into `src/hub/mod.rs::run_mcp_server` and DELETED
  `run_server` from `main.rs`. So by the time H2 runs, the composition -- the
  `ipc::serve(browser, &endpoint)` spawn, the `Err(ghostlight::Error::SessionBusy)` degrade arm
  (which today warns "another ghostlight session already owns the browser; tool calls in this session
  will report the extension as unavailable"), and the `mcp::server::run(...)` call sharing one
  `Browser` -- ALL live in `src/hub/mod.rs::run_mcp_server`, NOT `main.rs`. RE-READ `src/hub/mod.rs`;
  the original-tree `main.rs` line numbers no longer apply.
- `main.rs` after H0 only ROUTES roles: `chrome-extension://` -> `run_native_host_role` (the RELAY,
  ~lines 421-438); default (no subcommand, over stdio) -> the hub entrypoint via the `command: None`
  arm (~lines 393-397). The `Error::SessionBusy` degrade branch is NOT in `main.rs` anymore.

### docs/adr/0004-reject-second-session.md

- Header (lines 1-4): `# 0004. Reject a second concurrent session` / `- Status: Accepted` /
  `- Date: 2026-07`. This ADR is superseded at the MCP-client layer by ADR-0030.

### Coupling that pins scope

The `serve_session`/`ServiceContext` seam (H1), the `Browser` shared-Arc correlation (executor), the
two endpoint accept loops (ipc.rs: the unchanged `serve` for the extension + the new `serve_adapters`
for adapters), and the election/role branch (`run_mcp_server` in `src/hub/mod.rs`, post-H0) are one
connected change: you cannot land multiplex without all four moving together. This is the ADR's "one
large coupled commit" (Migration H2; Consequences). The ONLY governance/executor-level behavior change
is the kill-hook fan-out; everything else is transport wiring.

## Required behavior

### 1. The SERVICE owns shared state and multiplexes over TWO endpoints (ADR-0030 Decision 1, Decision 2)

Read PINS.md SS1 in full first: ADR-0030 Decision 1 was amended 2026-07-04 to TWO local endpoints, NOT
one role-demuxed endpoint. The extension endpoint is UNCHANGED and carries NO hello; the hello is a
session-hello on the adapter/control endpoint ONLY. There is NO `ROLE_EXT`.

- The SERVICE holds ONE `ServiceContext` (H1's shared state: the one `Browser`, `ConfigStore`, audit
  `Recorder`) and, for its whole life, owns BOTH local endpoints (Decision 1: "SOLE owner of the ONE
  extension link", now on its own endpoint):
  1. the EXTENSION endpoint -- the existing `ipc::default_endpoint()` -- accepted via the UNCHANGED
     `ipc::serve(browser, endpoint)` -> `Browser::attach`. Server-speaks-first, NO hello read. Do NOT
     add a hello, a pre-read, or any byte to this path; `relay_native_host` stays byte-for-byte as is.
     The single-physical-link invariant is unchanged.
  2. the ADAPTER/CONTROL endpoint -- PINNED in PINS.md SS1 as the extension base name + literal suffix
     `-adapter`, wrapped by the SAME `pipe_path`/socket-path helper (RE-READ ipc.rs for that helper).
     Claimed via `ipc::claim_adapter_endpoint(endpoint)` returning the PLATFORM listener handle
     (cfg-split, NO unified `Listener` type; PINS.md SS1 pin 1: the SAME bind-with-stale-heal `serve`
     does today, including the Unix `AddrInUse` -> probe -> remove dead / `SessionBusy` if live path --
     NOT just the accept loop, or a leftover `-adapter` socket wedges startup). A NEW acceptor
     `ipc::serve_adapters(ctx, listener)` runs the accept loop over the ALREADY-claimed listener
     (never re-claims the name; PINS.md SS1
     pin 1). It is accept-ahead + spawn-per-connection and reads+demuxes the session-hello INSIDE the
     spawned task (PINS.md SS1 pin 2; never inline -- a silent peer must not head-of-line-block other
     adapters). Reading the hello first is SAFE here because the peer speaks first (the adapter dials
     and sends the hello before any reply is expected, so no server-speaks-first deadlock). Demux:
     `"adapter"` -> `serve_session` over the SHARED `ServiceContext`; `"control"` reserved and cleanly
     refused until H8; unknown or absent role fails the connection cleanly (never a panic).
- Every `serve_session` clones the one `Browser`, so replies route by id with no new correlation code
  (Decision 2). The single governance chokepoint stays ONE `serve_session`/`handle_tools_call` that
  every transport calls (Decision 2: "never re-implemented per adapter"). Do NOT fork a second
  dispatch path. Build the `ServiceContext` ONCE and CLONE it per session (PINS.md SS1 pin 4: derive
  `Clone`; do NOT call `from_startup` per session -- it leaks one recorder-reload watcher per call).
- The session-hello is PINNED in PINS.md SS1: `{"hub":1,"role":"<role>","guid":<uuid>?}` carried ON
  TOP OF the existing 4-byte-LE framing (NEVER a change to `host.rs` framing -- STOP precondition).
  Define the constants in a new `src/hub/handshake.rs` (`HUB_PROTO`, `ROLE_ADAPTER`, `ROLE_CONTROL`;
  NO `ROLE_EXT`) per PINS.md SS1. The GUID member is the H3 seam; before H3 an empty placeholder guid
  is acceptable and H3 fills it.

### 2. The ADAPTER role is a stdio<->service byte relay (ADR-0030 Decision 1)

- Add an ADAPTER relay: connect to the running service's ADAPTER/CONTROL endpoint (item 1); send the
  `adapter` session-hello FIRST as ONE 4-byte-LE FRAMED message via `host::write_message`
  (`{"hub":1,"role":"adapter","guid":<the GUID>}`; GUID seam is H3, empty placeholder before H3); THEN
  enter a RAW bidirectional byte copy between this process's stdin/stdout and the service stream,
  exiting when either side closes.
  - CRITICAL (PINS.md SS1 pin 3): the DATA phase is a RAW copy (`tokio::io::copy`/`copy_bidirectional`),
    NOT a `host::read_message` framed copy. `relay_adapter` mirrors `relay_native_host` ONLY in
    lifecycle shape (the `select!`/exit-on-either-close, the "do NOT add a post-`select!`
    `shutdown().await`" note, the `process::exit` teardown reason), NEVER in its framing:
    `relay_native_host` frames every byte because the Chrome native-messaging wire is framed
    end-to-end; the adapter wire is framed for the hello ONLY, then raw newline JSON-RPC (what the MCP
    client writes and what the service's `serve_session` `BufReader::lines()` reads). A framed data
    copy here corrupts every multiplexed session's JSON-RPC.
  - Symmetrically on the SERVICE side: after `serve_adapters` reads the framed hello via
    `host::read_message` (`read_exact`, no buffer-ahead), it hands the RAW stream to `serve_session`;
    it does NOT keep framing the peer.
  - Pinned name: `ipc::relay_adapter(endpoint: &str, debug: &crate::debug::DebugSink) -> Result<()>`
    (the `endpoint` passed is the ADAPTER/CONTROL endpoint, not the extension endpoint).
- The ADAPTER is a BYTE relay only. It NEVER rewrites the MCP JSON-RPC wire and NEVER re-serializes
  the 13 trained schemas or the `notifications/tools/list_changed` line (ADR-0030 "Preserved
  invariants"; global never-touch). It relays bytes verbatim.
- The ADAPTER connects to an ALREADY-RUNNING service only. Spawn-on-demand (the adapter proactively
  starting a detached service when none exists) is H6 -- OUT OF SCOPE here.

### 3. Repeal ADR-0004 at the MCP-client layer (ADR-0030 Decision 1 + Relationship-to-other-decisions)

- In `src/hub/mod.rs::run_mcp_server` (post-H0; NOT `main.rs`, where `run_server` no longer exists),
  the default (no-subcommand, stdio) invocation becomes service-OR-adapter by contention on the
  ADAPTER/CONTROL endpoint (the election target; PINS.md SS1). Call `ipc::claim_adapter_endpoint`
  FIRST (PINS.md SS1 pin 1) so the branch below knows win vs lose BEFORE opening anything else:
  - On win (returns the claimed platform listener, so the process IS the SERVICE): open the EXTENSION endpoint
    via the unchanged `ipc::serve(browser, ext_endpoint)`, accept adapter/control sessions via
    `ipc::serve_adapters(ctx, listener)` over the ALREADY-claimed listener (item 1; do NOT re-claim the
    name), AND serve THIS process's own stdio as the first session via `serve_session` on the shared
    `ServiceContext`. A lone client thus stays a single self-contained process whose extension path is
    byte-identical to today (byte-identity preserved -- see Tests). Only the winner opens the extension
    endpoint, so there is no extension-endpoint race.
  - On lose (`Err(Error::SessionBusy)` from the ADAPTER/CONTROL claim): run the ADAPTER role
    (`ipc::relay_adapter`, item 2) against the adapter/control endpoint, connecting this process's stdio
    to the running service. This REPLACES the old reject-2nd DEGRADE behavior: DELETE the
    degrade-warning arm (the `Err(Error::SessionBusy)` branch relocated into `run_mcp_server` by H0
    that warned "another ghostlight session already owns the browser; tool calls ... unavailable") --
    remove its DEGRADE SEMANTICS, not the winner's extension `serve` error handling.
- The winner's `ipc::serve(browser, ext_endpoint)` keeps its OWN `Ok`/`Err` handling (a stale
  extension-endpoint owner remains possible via the retained single-physical-link guard and should
  degrade quietly, not `error!`-spam). Only the ADAPTER/CONTROL-endpoint `SessionBusy` drives the
  service-or-adapter election.
- Update the ipc.rs module doc (lines 17-18): `Error::SessionBusy` (on the adapter/control endpoint)
  now means "the singleton service is already up; connect to it as an adapter", NOT "tools
  unavailable". Do not change the `Error` variant.
- The single physical-extension-link rejection (`Browser::attach` ->
  `AttachOutcome::AlreadyAttached`) is NOT repealed and NOT weakened (ADR-0030 Relationship section).
- OUT OF SCOPE here (H6): the ADAPTER keeps the `"mcp-server"` debug role label for now (it is minted
  before the election in `build_debug_sink`), and `doctor` probes/reaps only the extension endpoint;
  the distinct `"adapter"` label and adapter-endpoint diagnosis are H6 (PINS.md SS5). Do NOT re-scope
  doctor or relabel the adapter in this task.

### 4. Kill-hook fan-out (ADR-0030 Decision 7) -- THE ONE FORCED executor change

ADR-0030 Decision 7 mandates: the single-consumer kill hook "becomes a fan-out registry so every live
session's subject gets exactly one `session_killed` audit record; one group's extension reconnect
must not clear a global kill for other groups." Convert it as follows (this is the SANCTIONED
EXCEPTION to the executor never-touch fence):

- Replace the single `kill_hook: Arc<Mutex<Option<KillHook>>>` (executor.rs line 105) with a registry:
  `kill_hooks: Arc<Mutex<Vec<(u64, KillHook)>>>` plus `next_hook_id: Arc<AtomicU64>`.
- Keep `pub fn on_session_killed(&self, hook: impl Fn() + Send + Sync + 'static)` (same signature),
  now APPENDING a PERMANENT (never-removed) hook. Its doc comment must be updated from "Registering a
  second hook replaces the first" to describe append semantics. (This keeps the pinned unit test
  `kill_hook_fires_exactly_once_per_transition` and the integration test
  `tests/audit_recorder.rs::session_killed_writes_one_session_event_record` green: each registers one
  hook and expects one fire per transition.)
- Add a session-scoped, REMOVABLE registration:
  ```
  #[must_use = "dropping the handle immediately unregisters the session kill hook"]
  pub struct KillHookHandle { /* holds the registry Arc + the entry id */ }
  impl Drop for KillHookHandle { /* remove the (id, hook) entry from kill_hooks */ }

  pub fn register_session_kill_hook(
      &self,
      hook: impl Fn() + Send + Sync + 'static,
  ) -> KillHookHandle
  ```
  A live session holds its `KillHookHandle` for its lifetime; dropping it at session end deregisters,
  so a dead session records nothing.
- `handle_session_killed` (executor.rs lines 495-505): on the false->true transition (`swap` guard
  unchanged), drain pending, then invoke EVERY registered hook (permanent + session) EXACTLY ONCE.
  The per-transition `swap` guard is what makes each individual kill fan out once per hook, never
  twice.
- In `serve_session` (H1's function -- RE-READ its module + how it currently registers the kill
  hook; as-of-authoring the registration is server.rs lines 178-184 via `on_session_killed`), replace
  the `on_session_killed(...)` registration with `let _kill_handle =
  ctx.<browser>().register_session_kill_hook(move || current_governance(&governance_slot)
  .record_session_killed())`, holding `_kill_handle` for the whole session. Keep `hold`/`killed`/
  `connected` GLOBAL by SHARING the one `Browser` across sessions (Decision 7: these latch on the
  shared handle) -- do NOT clone per-session `Browser`s.

Must stay byte-identical / unchanged: `host.rs` framing; the 13+`explain` `TOOLS_JSON`; the MCP
JSON-RPC wire and the `notifications/tools/list_changed` line; the hop-attributed error strings and
`TOOL_TIMEOUT`; `AttachOutcome::AlreadyAttached` single-link rejection.

## Tests (BY NAME; assertions pinned)

### Keep green (do not modify)

Under the amended two-endpoint design (PINS.md SS1) these pass UNMODIFIED: the extension endpoint keeps
its exact server-speaks-first contract, so NO fake-extension harness sends a hello. If
`tests/all_open_golden.rs` or `tests/mcp_protocol.rs` would need a hello added to their fake-extension
helper to go green, the endpoints were NOT actually split -- STOP, do not edit the harness (see STOP
preconditions and the LEDGER H2 log; this is the exact deadlock the first H2 attempt hit).

- `tests/mcp_protocol.rs` (a lone MCP client over the default binary invocation must still initialize
  and list tools byte-identically -- proves the service+own-session path).
- `tests/peer_death.rs` (server + native-host relay; force-kill the server; the relay exits -- proves
  the extension-link path and lifecycle are undisturbed).
- `tests/all_open_golden.rs` (the all-open byte-identity invariant: a lone all-open session's output
  stays byte-identical -- every new session/multiplex path MUST be a no-op for a lone all-open
  session).
- `tests/tool_schema_fidelity.rs` (the 13+`explain` schemas byte-frozen).
- `src/transport/executor.rs::kill_hook_fires_exactly_once_per_transition` (one registered hook fires
  exactly once across two kill frames -- must stay green under the append-registry).
- `src/transport/executor.rs::a_second_attach_is_rejected_without_disturbing_the_live_session` (the
  single physical-extension-link rejection).

Note (do not modify, must stay green): `tests/audit_recorder.rs::
session_killed_writes_one_session_event_record` (audit_recorder.rs:110) calls `on_session_killed`,
kept green by the preserved append signature. Separately,
`src/governance/dispatch.rs:1084::record_session_killed_writes_a_session_event_with_no_tool_call_fields`
is an inline test that calls `Governance::record_session_killed()` DIRECTLY (independent of the
kill-hook registry) -- unaffected by this task.

### Add: tests/hub_multiplex.rs

1. `two_sessions_route_replies_independently`

   Two `serve_session` tasks (or two direct `Browser::call` callers standing in for two sessions --
   RE-READ H1 to pick the lowest-level seam that still exercises `serve_session` if practical) share
   ONE `Browser` (one `.clone()` each) attached to ONE fake extension. Session A calls tool `"navigate"`;
   session B calls tool `"find"`. The fake extension replies to each framed request by id, echoing the
   request's `tool` back in the result (the pattern in `executor.rs::call_round_trips_a_tool_response`,
   lines 529-556).

   Pinned assertions (structural id-routing invariant per ADR-0030 Decision 2 -- NOT a value oracle):
   - session A's result is the reply to A's own id and echoes `"navigate"`;
   - session B's result is the reply to B's own id and echoes `"find"`;
   - the two replies are NEVER swapped (A never receives B's echo and vice-versa).

2. `one_kill_emits_one_audit_record_per_live_session`

   Build N = 3 sessions, each a distinct `Governance` (all-open) with a DISTINCT client name
   (`"client-a"`, `"client-b"`, `"client-c"` -- AUTHOR MUST PIN if H1/Governance requires a different
   constructor shape; RE-READ), each writing to its OWN file-backed `Recorder` (model:
   `tests/audit_recorder.rs::session_killed_writes_one_session_event_record`, lines 110-171). All three
   register via the NEW `register_session_kill_hook(move || governance.record_session_killed())` on the
   ONE SHARED `Browser`, holding their handles. Attach the shared `Browser` to one fake extension, send
   ONE `{"type":"session_killed"}` frame, wait for `is_killed()`.

   Pinned assertions:
   - EXACTLY N = 3 session-event records are written in total (one per live subject; N subjects give N
     records) -- one line in each of the three audit files, and none cross-written.
   - Each record's key order is EXACTLY the 6-key `SessionEventRecord` order, transcribed verbatim from
     ADR-0030 "Preserved invariants" (pinned oracle):
     ```
     event_id, ts, identity, client, event, manifest
     ```
     (assert the serialized object's `.keys()` equal `["event_id","ts","identity","client","event","manifest"]`,
     the same `.keys()` assertion as `src/governance/dispatch.rs:1139` / `src/governance/ports.rs:505`).
   - Each record has `rec["event"] == "session_killed"` and a distinct `rec["client"]["name"]`
     matching its session (`"client-a"` / `"client-b"` / `"client-c"`).

   (Transcribed oracle -- do NOT re-derive: the 6-key order above is quoted verbatim from ADR-0030's
   "Pinned oracles" bullet for the session-event record.)

   NOTE: the three separate file-backed Recorders are a TEST CONVENIENCE to isolate the N fan-out
   fires per subject; in production all sessions SHARE one `ServiceContext` Recorder (ADR-0030
   Decision 2) and the N records land in one stream, distinguished by `client`. Do not infer
   per-session Recorders are the design.

3. `adapter_endpoint_two_phase_wire_round_trips` (in `tests/hub_multiplex.rs`)

   Fences the PINS.md SS1 pin 3 framed-hello-then-raw-JSON-RPC wire (the framing trap that would ship
   green otherwise -- a framed data copy corrupts the JSON-RPC and this test fails). Spawn the binary
   with a unique `GHOSTLIGHT_ENDPOINT` (the service; its own stdio is the first session, and no fake
   extension is needed since `initialize` needs no browser call). Connect a raw client to the
   ADAPTER/CONTROL endpoint (`<GHOSTLIGHT_ENDPOINT>` + `-adapter`, via `ipc::connect`). Send the FRAMED
   session-hello `{"hub":1,"role":"adapter","guid":""}` via `host::write_message`, then write a RAW
   newline-terminated `{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}` line to the same
   stream.

   Pinned assertions (structural wire invariant, NOT a value oracle):
   - a RAW newline-delimited JSON-RPC reply line comes back on the same stream with `["id"] == 1`
     (proving the service read the raw JSON-RPC after the framed hello, i.e. the data phase is raw on
     both sides);
   - the reply is NOT length-prefixed / framed (read it as a line, not via `host::read_message`).

## Verification (literal commands)

```
cargo build --all-targets
cargo test --test hub_multiplex
cargo test --test mcp_protocol --test peer_death --test all_open_golden --test tool_schema_fidelity --test audit_recorder --test architecture
cargo test -p ghostlight --lib executor
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

(If the crate name for `-p` differs, RE-READ `Cargo.toml`; the lib-test filter `executor` selects the
`src/transport/executor.rs` unit tests.)

## STOP preconditions

- If `run_server` still EXISTS in `src/main.rs` (H0 has not landed / did not relocate the composition
  root into `src/hub/mod.rs::run_mcp_server`), STOP: this task's tree facts assume the H0 move; do not
  edit `main.rs`'s `run_server`.
- If `src/hub` / `HubCore` (H0) or `serve_session<S>(stream, ctx)` + `ServiceContext` (H1) are ABSENT,
  STOP: H0/H1 have not landed and this task's whole seam is void. Do not re-implement H0/H1 here.
- If `Browser`'s `next_id` or `pending` are NOT shared across clones (someone made per-session
  `Browser`s), STOP: the "multiplex needs no new correlation code" assumption (ADR-0030 Decision 2) is
  void and the design must be re-scoped.
- If `executor.rs::on_session_killed` no longer documents single-consumer replace (someone already
  changed it), STOP and re-scope the fan-out against the actual current state.
- If implementing the session-hello (item 1/2) would require changing `src/transport/native/
  host.rs` framing (the 4-byte LE prefix, `MAX_MESSAGE_LEN`, `encode`/`read_message`), STOP: the
  handshake must ride on top of framing, never alter it.
- If landing the adapter/control endpoint would require adding ANY hello, pre-read, or extra byte to
  the EXTENSION endpoint path, editing `relay_native_host`, or editing a fake-extension test harness
  (`tests/all_open_golden.rs`, `tests/mcp_protocol.rs`) to send a hello, STOP: the amended
  two-endpoint design (PINS.md SS1) keeps the extension endpoint hello-free and server-speaks-first.
  Needing to touch it means the two endpoints were not actually separated -- fix the separation, do
  NOT weaken the extension contract. (This is the exact conflict that BLOCKED the first H2 attempt;
  see the LEDGER H2 log.)
- If any change would weaken `Browser::attach`'s single-EXTENSION-link rejection
  (`AttachOutcome::AlreadyAttached`), STOP.
- If a never-touch fence below would have to move (other than the two sanctioned exceptions named),
  STOP.

## NEVER touch (this task)

- `src/transport/mcp/tools.rs` (`TOOLS_JSON`: the 13 trained schemas + `explain`), byte-frozen. No
  exception.
- `tests/tool_schema_fidelity.rs`. No exception; keep green untouched.
- `tests/all_open_golden.rs` + the all-open byte-identity invariant. No exception; every new
  session/multiplex/adapter path MUST be a no-op for a lone all-open session, and this file MUST pass
  UNMODIFIED. The two-endpoint design (PINS.md SS1) guarantees the extension path is untouched, so if
  this test fails the implementation is wrong, not the test -- do NOT edit it to add a hello.
- `tests/architecture.rs` a7 (`governance_core_has_no_forbidden_back_edges`): all session / multiplex
  / isolation code lands in `src/hub`, never `src/governance/**`. No exception in this task (the H8
  `channels.webapi.from` allowlist exception does NOT apply here).
- `src/transport/native/host.rs` framing (4-byte LE prefix, `MAX_MESSAGE_LEN`, `encode`/
  `read_message`). No exception this task.
- The MCP JSON-RPC wire + the pinned `notifications/tools/list_changed` line (server.rs). The ADAPTER
  is a byte relay, never a rewriter. No exception.
- `Browser::attach` single-EXTENSION-link rejection (`AttachOutcome::AlreadyAttached`). RETAINED.
  SANCTIONED EXCEPTION: add the kill-audit FAN-OUT (`register_session_kill_hook` + `KillHookHandle` +
  the `handle_session_killed` fan-out loop); do NOT weaken the single physical-link invariant.
- The hop-attributed error strings + `TOOL_TIMEOUT = 60s` in `src/transport/executor.rs`. Byte-frozen;
  the only sanctioned executor edit is the kill-hook fan-out named above.
