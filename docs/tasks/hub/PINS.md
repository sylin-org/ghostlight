# Ghostlight Hub batch: PINS (author oracle sheet)

Every value here is PINNED by the batch author. The executor TRANSCRIBES these; it never derives or
invents one (the ORACLE RULE, BOOTSTRAP). Where a task file says "PINNED in PINS.md SS<n>", use the
value below verbatim. Semantics live here in one place; the task files cite, they do not re-decide.

## SS1 -- The two local endpoints + the adapter/control session-hello (shared by H2, H3, H7)

(Amended 2026-07-04, ADR-0030 Decision 1 two-endpoint split. The earlier single "role-demuxed core
endpoint" with an `ext` hello role is REPEALED; see ADR-0030 Provenance for why. There is NO
`ROLE_EXT` and `relay_native_host` sends NO hello.)

The local core exposes TWO owner-only endpoints. A peer's role is the endpoint it arrives at, NOT a
discriminator byte on a shared endpoint:

- EXTENSION endpoint -- PINNED as the EXISTING `ipc::default_endpoint()` (the `GHOSTLIGHT_ENDPOINT`
  env override, else `DEFAULT_ENDPOINT`). Server-speaks-first, NO hello. The service accepts here via
  the UNCHANGED `ipc::serve(browser, endpoint)` -> `Browser::attach`; `relay_native_host` dials it and
  sends NOTHING first, exactly as today. `host.rs` framing, the relay, and every fake-extension test
  double are byte-for-byte UNCHANGED. This endpoint NEVER carries a hello frame.
- ADAPTER/CONTROL endpoint -- PINNED as the extension endpoint's base name with the literal suffix
  `-adapter` appended, then wrapped by the SAME `pipe_path` / socket-path helper the extension
  endpoint uses (so a test-unique `GHOSTLIGHT_ENDPOINT` automatically makes BOTH endpoints unique).
  This is the single-instance ELECTION target (H6): the process that wins the create-claim here IS the
  service. Speak-first sessions arrive here and send the session-hello below as their first frame.

The session-hello (adapter/control endpoint and the H8 web session ONLY), carried ON TOP OF the
existing 4-byte-LE `host.rs` framing (NEVER a change to that framing), is a JSON object:

```
{ "hub": 1, "role": "<role>", "guid": "<uuid-v4>"? }
```

- `hub`: the protocol major. PINNED constant `pub const HUB_PROTO: u32 = 1;`, defined in a new module
  `src/hub/handshake.rs` (created by H2).
- `role`: exactly one of the PINNED strings `"adapter"` (an MCP stdio adapter) or `"control"`
  (doctor/console; reserved, not used before H8). PINNED constants `ROLE_ADAPTER = "adapter"`,
  `ROLE_CONTROL = "control"` in `src/hub/handshake.rs`. There is NO `ROLE_EXT`: the extension is
  identified by its endpoint and sends no hello.
- `guid`: present ONLY for `role == "adapter"` (and the H8 web session); it is the adapter-minted
  session GUID (see H3). Absent for `"control"`.

H2: `run_mcp_server` claims the ADAPTER/CONTROL endpoint. The WINNER (the service) opens the EXTENSION
endpoint via the unchanged `ipc::serve` AND accepts adapter/control sessions on the adapter/control
endpoint AND serves THIS process's own stdio as the first session via `serve_session` on the shared
`ServiceContext`. The adapter/control acceptor reads the session-hello FIRST (safe -- the peer speaks
first here) and demuxes `"adapter"` -> `serve_session`; `"control"` is reserved (cleanly refused until
H8); an unknown or absent role fails the connection cleanly (never a panic). A process that LOSES the
claim (`Error::SessionBusy` on the adapter/control endpoint) becomes the ADAPTER: `relay_adapter`
dials the adapter/control endpoint, sends `{"hub":1,"role":"adapter","guid":<guid>}`, then byte-relays
its stdio. `relay_native_host` and the extension endpoint's accept path are UNCHANGED.
H3: the adapter's `guid` in this same session-hello is the session GUID; do not invent a second frame.
H7: the group-request (SS6) is a native-messaging message to the extension AFTER a session exists,
never part of the adapter hello.

### SS1 implementation pins (transcribe exactly; added 2026-07-04 after red-team)

These make the two-endpoint mechanism implementable without the executor deriving any oracle:

1. Claim/serve SPLIT, not a fused claim-and-loop. PIN `ipc::claim_adapter_endpoint` returning the
   PLATFORM listener handle (this is `#[cfg]`-split exactly as `serve` is today: Windows -> the
   pre-created `NamedPipeServer` instance; Unix -> the bound `UnixListener`; there is NO unified
   `Listener` type -- do not invent one, cfg-split like the rest of ipc.rs). It performs the SAME
   bind-with-stale-heal `serve` does today (Windows:
   `first_pipe_instance(true)`, ACCESS_DENIED / PIPE_BUSY -> `Error::SessionBusy`; Unix: bind, and on
   `AddrInUse` PROBE-connect FIRST -- a live peer -> `Error::SessionBusy`, a DEAD socket -> remove and
   rebind, exactly as `serve`'s Unix preamble does for the extension socket) and RETURNS the bound
   listener on win. `run_mcp_server` calls it FIRST so it learns win/lose; on win it opens the
   extension endpoint and spawns `ipc::serve_adapters(ctx, listener)` over the ALREADY-claimed listener
   -- NEVER re-claiming the name (a second bind self-deadlocks on Unix: the process probe-connects to
   its own listener and reads `SessionBusy`). This is the split the blocked attempt organically made
   (`claim_endpoint` / `serve_claimed`).
2. `serve_adapters` accept loop = accept-ahead + spawn-per-connection, reading and demuxing the
   session-hello INSIDE the spawned task, NEVER inline in the accept loop (exactly how `serve` spawns
   per connection). A silent peer must not head-of-line-block admission of other adapters (Decision 3).
3. TWO-PHASE adapter wire. The session-hello is ONE 4-byte-LE FRAMED message (`host::write_message` /
   `host::read_message`; `read_message` is `read_exact` with NO buffer-ahead, so the read-half hands to
   `serve_session` with zero bytes lost). Everything AFTER the hello is RAW newline-delimited JSON-RPC
   (what `serve_session`'s `BufReader::lines()` expects and what the MCP client writes). Therefore
   `relay_adapter`'s DATA phase AND the service's post-hello copy are a RAW bidirectional byte copy
   (`tokio::io::copy` / `copy_bidirectional`), NOT a `host::read_message` framed copy. `relay_adapter`
   mirrors `relay_native_host` ONLY in lifecycle shape (the `select!`, no post-`select!`
   `shutdown().await`, the `process::exit` teardown); it does NOT frame the data phase.
   (`relay_native_host` frames because the Chrome native-messaging wire is framed end-to-end; the
   adapter<->MCP-client wire is framed for the hello ONLY, then raw.)
4. Build `ServiceContext` ONCE at service start and `#[derive(Clone)]` it (Browser is Clone;
   store/recorder are `Arc`; `LoadedPolicy` is Clone); CLONE it per session for `serve_session`. Do NOT
   call `ServiceContext::from_startup` per session -- it spawns a recorder-reload task each call, so
   one-per-session leaks N duplicate watchers on the one store. One `from_startup`; clones share the
   one Recorder/store.

## SS2 -- The authenticated subject's audit home (resolves the H8 vs 14-key tension)

The authenticated subject does NOT add a 15th audit key. It populates the EXISTING `identity` field
(position 3 of the frozen 14-key order; `AuditRecord.identity: Option<Identity>` where
`Identity { principal, resolved_by }` already exists in `src/governance/ports.rs`, currently always
built as `None` in `dispatch.rs::build_record`).

- Local adapter session, or an anonymous web caller, or any all-open session: `identity = None`
  (BYTE-IDENTICAL to today; `all_open_golden` and `audit_recorder` stay green untouched).
- A web session whose policy named a principal: `identity = Some(Identity { principal: <the named
  principal>, resolved_by: "webapi" })`.

So "distinct from the self-reported `clientInfo`" (ADR-0030 Decision 9) means the existing `identity`
field, which is separate from the `client` field. No new key; the 14-key order is preserved.

## SS3 -- H4 unowned-tab refusal

- Uniform, leak-free result (IDENTICAL for ANY tabId not in the session's owned set -- whether it
  exists in another session or does not exist at all; the gate runs BEFORE any extension query and
  cannot distinguish the two, so it is uniform by construction): a SUCCESSFUL MCP text result, NOT an
  error. This follows the system's denial convention -- denials render as a normal text result, never
  `isError` (see the hold/deny path at pipeline.rs:109/193 and `hold_message`). It carries only the
  PINNED text `unknown tab` -- no host, no tabId echo.
- It IS recorded, as a deny: `decision = "deny"`, `domain = null` (the host is NEVER resolved for an
  unowned tab -- resolving it is the very leak being closed), `held = false`, `duration_ms = 0`.
- `denial_id`: computed by the existing scheme (`denial.rs`: `"D-"` + 8 lowercase hex); the rule
  label is PINNED as `cross_session/unowned_tab`. Do not hardcode a literal id (it derives from the
  manifest hash at runtime); assert the `"D-"` prefix + 8 hex shape, mirroring existing denial tests.

## SS4 -- H5 constants

- `pub const GRACE_WINDOW: Duration = Duration::from_secs(10);` (strictly < the 60s `TOOL_TIMEOUT`).
- `pub const PER_PEER_MINT_CAP: usize = 32;` (max concurrent GUID sessions per minting peer identity).
- `pub const PER_PEER_GROUP_CAP: usize = 32;` (max live tab groups per peer identity; equal to the
  mint cap by design).
- Quota-exceeded result: a plain tool error, PINNED text `session limit reached for this client`
  (no global lockout -- a flooding peer is denied while other peers are unaffected; the test asserts
  a second, different peer still succeeds).
- `pub const SCREENSHOT_CHUNK_THRESHOLD: usize = 8 * 1024 * 1024;` (payloads at/above 8 MiB are
  chunked; well under the `host.rs` `MAX_MESSAGE_LEN`). Chunking is on the SERVICE<->adapter/web hop
  only, NEVER the frozen extension `host.rs` wire.
- The `oversized_screenshot_is_chunked_not_head_of_line_blocking` test's completion bound for the
  small interleaved call: PINNED at `< 2s` (a tiny call must complete while a chunked large payload
  streams).

## SS5 -- H6 constants (REWRITTEN 2026-07-04 for the always-ready-service amendment)

ADR-0030 Decision 8 was amended (see its Provenance "always-ready-service amendment"). The service
is a STANDALONE process launched by argv (`ghostlight service`), never an in-process/promoted
service and never an in-editor spawned child; every MCP invocation is a thin ADAPTER. The old
on-demand-spawn / `CREATE_BREAKAWAY_FROM_JOB` / `IsProcessInJob` / promotion mechanism is DELETED
(never built). These pins replace the old SS5 in full.

### SS5.1 -- Role dispatch by argv (`src/main.rs`, `src/hub/mod.rs`)

- NEW clap subcommand on `Command` (RE-READ the enum): a unit variant `Service` with doc
  `/// Run the persistent Ghostlight Hub service (owns the browser link; multiplexes clients).`
- `main()`'s match gains ONE arm, mirroring the existing `command: None` arm:
  `Cli { command: Some(Command::Service), manifest, debug: debug_flag } => ghostlight::hub::run_service(manifest, debug_flag || debug_env)?,`
  The `command: None` arm KEEPS calling `ghostlight::hub::run_mcp_server(manifest, debug_flag || debug_env)` (now the adapter). `--manifest`/`--debug` are the existing top-level `Cli` fields (usage: `ghostlight --manifest <src> service`); the `chrome-extension://` relay detection at the top of `main` is UNCHANGED.
- `pub fn run_mcp_server(manifest: Option<String>, debug_on: bool) -> anyhow::Result<()>` is
  RESHAPED into the thin ADAPTER (signature UNCHANGED so `main.rs`'s None arm needs no edit):
  `role::set_role(role::Role::Adapter)` first; if `manifest.is_some() || std::env::var_os("GHOSTLIGHT_MANIFEST").is_some()`, emit exactly the one-line warning `tracing::warn!("a --manifest on a client invocation is ignored; the running Ghostlight service's policy governs all sessions")` and load NO policy; `build_debug_sink(debug_on, "adapter")`; `crate::doctor::sweep_orphans()` (now reaps orphaned adapters, SS5.5); `let parent = crate::proc::parent();`; build the runtime and `run_as_adapter(&endpoint, sink, parent).await`; `std::process::exit(code)`. It NEVER claims the endpoint, loads policy, builds a `Browser`, or builds a `ServiceContext`.
- `pub fn run_service(manifest: Option<String>, debug_on: bool) -> anyhow::Result<()>` is NEW.
  `role::set_role(role::Role::Service)` first; resolve the manifest EXACTLY as today's `run_mcp_server` does (`manifest.or_else(|| std::env::var("GHOSTLIGHT_MANIFEST").ok())`, then `source::load_policy(...)`, fatal on selected-but-unreadable); `build_debug_sink(debug_on, "mcp-server")` (KEEP the `"mcp-server"` label so `doctor`'s health anchor still finds the service); build the runtime; `block_on(run_service_loop(...))`; `std::process::exit(code)`. It NEVER captures a parent, NEVER runs `watchdog::wait_until_orphaned`, NEVER calls `sweep_orphans`, and NEVER serves its own stdio as a session.
- `run_service_loop` (name at author's discretion; the async body of `run_service`): claim the
  endpoint via `ipc::claim_adapter_endpoint(&endpoint)`. `Err(crate::Error::SessionBusy)` -> log `tracing::info!("a Ghostlight service is already running on this endpoint; exiting")` and return `0` (single-instance guard). `Err(e)` -> log and return `1`. On `Ok(listener)`: build `Browser::with_debug(sink)`; `tokio::spawn(ipc::serve(browser.clone(), &endpoint))` (UNCHANGED extension endpoint); build `ServiceContext::from_startup(browser, loaded_policy, user_source)?` ONCE; `tokio::spawn(ipc::serve_adapters(ctx.clone(), listener))`; then run the idle-grace watcher (SS5.4) as the returning future.

### SS5.2 -- Thin adapter relay + supervisor self-heal (`src/hub/mod.rs`, `src/hub/supervisor.rs`, `src/transport/native/ipc.rs`)

- `run_as_adapter(endpoint: &str, sink: DebugSink, parent: Option<crate::proc::ProcId>) -> i32`:
  if `Some(parent)`, `tokio::spawn` the ADR-0029 watchdog exactly as today's `run_as_service` did
  (`watchdog::wait_until_orphaned(parent).await` then signal shutdown) so the ADAPTER dies with its
  editor; then run `ipc::relay_adapter(endpoint, &sink)` and return its code. (The watchdog + reaper
  now live on the ADAPTER; the service runs neither.)
- NEW module `src/hub/supervisor.rs` (add `pub mod supervisor;` to `src/hub/mod.rs`). PINNED
  identifiers (the H9 installer registers these SAME names):
  - Windows Task Scheduler task name: `pub const SUPERVISOR_TASK_NAME: &str = "Ghostlight Service";`
  - Linux systemd --user unit: `pub const SUPERVISOR_UNIT: &str = "ghostlight.service";`
  - `pub fn supervisor_start_command() -> Option<(String, Vec<String>)>` (PURE; `#[cfg]`-split;
    unit-tested for the exact program+args, NEVER executed in a test):
    - Windows: `Some(("schtasks".into(), vec!["/run".into(), "/tn".into(), SUPERVISOR_TASK_NAME.into()]))`
    - Linux: `Some(("systemctl".into(), vec!["--user".into(), "start".into(), SUPERVISOR_UNIT.into()]))`
  - `pub fn start_service()`: FIRST line `crate::hub::role::assert_adapter_role("start_service");`
    (SS8 seam; a SERVICE must never trigger a service start). Then best-effort run
    `supervisor_start_command()` via `std::process::Command` (spawn + wait; ignore any failure --
    it is a hint, not a guarantee). This function is the text-scan target of the H6 role-marker test.
- Self-heal in `ipc::relay_adapter` (RE-READ its current connect-then-relay shape): before the raw
  relay, DIAL the adapter/control endpoint. On the FIRST dial failure (service down), call
  `crate::hub::supervisor::start_service()` once, then RETRY the dial every
  `SELF_HEAL_RETRY_INTERVAL` for up to `SELF_HEAL_RETRY_WINDOW`. If still unreachable, log the PINNED
  message and return without relaying (the process exits non-zero):
  - `pub const SELF_HEAL_RETRY_WINDOW: Duration = Duration::from_secs(3);`
  - `pub const SELF_HEAL_RETRY_INTERVAL: Duration = Duration::from_millis(200);`
  - PINNED self-heal failure message (verbatim): `the Ghostlight service is not running and could not be started automatically; start it with 'ghostlight service' (or reinstall to enable auto-start)`
  (Tests never exercise the self-heal path -- they spawn `ghostlight service` explicitly so the
  first dial succeeds. Only `supervisor_start_command()` is unit-tested, as a pure string.)

### SS5.3 -- Anti-squat: per-install secret + HMAC proof (`src/hub/supervisor.rs` or a new `src/hub/antisquat.rs`; wired in `ipc.rs`)

- Secret: 32 random bytes (`getrandom::getrandom`) at `crate::debug::log_dir()?/hub-key`. RE-READ
  `src/debug.rs::log_dir()`: it is the PER-USER dir `dirs::data_local_dir()/ghostlight`
  (`%LOCALAPPDATA%\ghostlight` on Windows or `~/.local/share/ghostlight` on Linux). This corrects
  the old machine-wide mismatch: the secret is PER-USER. Do NOT invent a new dir. Created lazily on the
  first `run_service` start if absent; on Unix set mode `0o600` after write; on Windows the per-user
  `%LOCALAPPDATA%` ACL suffices (NO DPAPI -- it adds no same-user defense and no dep). Threat scope:
  the secret defeats a NAIVE or CROSS-USER squatter; a determined same-user process can read any
  same-user file, so this is defense-in-depth, not a same-user sandbox (stated in Decision 8).
- Two-phase-plus-proof wire (extends SS1 pin 3; RE-READ SS1). Order, all framed via
  `host::write_message`/`host::read_message` EXCEPT the final raw phase:
  1. ADAPTER -> SERVICE: the framed session-hello `{"hub":1,"role":"adapter","guid":"<uuid-v4>"}`
     (UNCHANGED from H3). The adapter KEEPS the exact serialized hello bytes it sent.
  2. SERVICE -> ADAPTER (NEW, in `handle_adapter_connection`, AFTER reading + admitting the hello,
     BEFORE `serve_session`): one framed message
     `{"hub":1,"role":"service-proof","mac":"<hex>"}` where `<hex>` is the lowercase-hex
     HMAC-SHA256 of the EXACT hello bytes it read (item 1's payload) keyed by the `hub-key` bytes.
     `ROLE_SERVICE_PROOF = "service-proof"` (PINNED; add to `src/hub/handshake.rs` beside
     `ROLE_ADAPTER`/`ROLE_CONTROL`).
  3. ADAPTER (NEW, in `relay_adapter`, AFTER sending its hello, BEFORE the raw relay): read one
     framed message; require `role == "service-proof"`; recompute HMAC-SHA256 over its OWN sent
     hello bytes keyed by the `hub-key` bytes IT reads; verify with `hmac::Mac::verify_slice`
     (constant-time) against the hex-decoded `mac`. On ANY failure (missing/unreadable key, wrong
     role, malformed frame, MAC mismatch) ABORT: log the PINNED text `refusing to connect: the Ghostlight service on this endpoint is not the one this user installed` and return WITHOUT relaying.
  `read_message` is `read_exact` with no buffer-ahead (SS1 pin 3), so no bytes are lost transitioning
  to the raw phase. HMAC keyed by the raw 32 key bytes over the raw hello bytes -- both sides hash the
  identical byte string, so a matching key yields a matching MAC.

### SS5.4 -- Idle-grace shutdown (`src/hub/mod.rs`, `src/transport/mcp/server.rs`)

- `pub const IDLE_GRACE: Duration = Duration::from_secs(30);` (the service exits only after zero live
  sessions AND the extension link gone, continuously, for this window).
- `pub const IDLE_POLL: Duration = Duration::from_secs(1);` (author-pinned; not in ADR-0030).
- `ServiceContext` (`src/hub/mod.rs`) gains ONE field, added the SAME way `session_registry`/
  `owned_tabs`/`mint_quota` were: `pub live_sessions: Arc<std::sync::atomic::AtomicUsize>`, built in
  `from_startup` as `Arc::new(AtomicUsize::new(0))`. (Existing direct `ServiceContext` constructions
  in `tests/hub_isolation.rs` and `tests/hub_queue.rs` `build_ctx` each need one added line
  `live_sessions: Arc::new(AtomicUsize::new(0))` -- a compile-forced deviation, log it like H5's D1.)
- `serve_session` (`src/transport/mcp/server.rs`) increments `ctx.live_sessions` at entry and
  decrements at exit via a small RAII guard (so EVERY session -- adapter now, web at H8 -- is counted
  at the ONE chokepoint). This adds NO output and does NOT touch the frozen
  `notifications/tools/list_changed` line.
- Idle-grace watcher (the returning future of `run_service_loop`):
  ```
  let mut idle_for = Duration::ZERO;
  loop {
      tokio::time::sleep(IDLE_POLL).await;
      let idle = ctx.live_sessions.load(Ordering::Relaxed) == 0 && !ctx.browser.is_connected();
      idle_for = if idle { idle_for + IDLE_POLL } else { Duration::ZERO };
      if idle_for >= IDLE_GRACE { return 0; }
  }
  ```
  (`Browser::is_connected()` is the existing extension-link probe; RE-READ its name.)

### SS5.5 -- Debug labels + doctor reap re-scope (`src/hub/mod.rs`, `src/doctor.rs`, `src/proc.rs`, `src/transport/watchdog.rs`)

- Debug/session role labels: SERVICE -> `build_debug_sink(debug, "mcp-server")` (KEEP; `doctor`'s
  health anchor and its status parser look for a `"mcp-server"` session with the extension connected,
  which is now the standalone service). ADAPTER -> `build_debug_sink(debug, "adapter")` (NEW label).
  native-host stays `"native-host"`.
- `doctor::reap` and orphan detection RE-SCOPE from `"mcp-server"` to `"adapter"` (RE-READ current
  line numbers; as-of-authoring the reap filters are at doctor.rs:561 in `orphaned_server_pids`
  (`s.role == "mcp-server" && classify(s) == Liveness::Orphaned`) and doctor.rs:609 in `reap`
  (`s.role != "mcp-server" || s.pid == me`) and the reap-report text at doctor.rs:147
  (`"reaped ... orphaned mcp-server session(s)"`) and the module doc at doctor.rs:20). Change ONLY
  the REAP-target filters and text to `"adapter"`; the HEALTH-anchor + display filters that find the
  SERVICE (doctor.rs:86 `NewestServer`, doctor.rs:465/481 display) STAY `"mcp-server"`. Net: the
  reaper reaps orphaned ADAPTERS (parent editor dead), NEVER the service (which has no client parent
  and idle-graces).
- `src/proc.rs` and `src/transport/watchdog.rs`: update ONLY the module-doc narrative from
  "the mcp-server role" to "the adapter role" (the parent-death lifecycle now belongs to the
  adapter). NO API change: `ProcId`/`parent`/`orphaned`/`pid_exists`/`is_alive`/`creation_time`/
  `terminate` and `wait_until`/`wait_until_orphaned` are UNCHANGED (the adapter watchdog + doctor
  reap still use them). Keep every existing `proc`/`watchdog` inline test green and unmodified.

### SS5.6 -- Dependencies (`Cargo.toml`)

- ADD (additive; no version bump of an existing dep): `hmac = "0.12"`, `sha2 = "0.10"`,
  `getrandom = "0.2"`. Use `hmac::Mac::verify_slice` for constant-time MAC verification (no `subtle`).
- Do NOT add any `windows-sys` job-object / breakaway feature (`Win32_System_JobObjects`,
  `Win32_Security_Cryptography`): the breakaway mechanism is DELETED, not built.

## SS6 -- H7 group request

- Message type: PINNED `"group_request"` (additive; alongside the existing native-messaging message
  types in `messages.rs` -- must not alter any existing shape). Fields:
  `{ "type": "group_request", "guid": <session guid>, "tabIds": [<i64>...], "title": <string> }`.
  The extension replies with `{ "type": "group_response", "guid": <guid>, "ok": <bool> }`.
- Per-session group title: PINNED format `\u{1F47B} Ghostlight <short>` where `<short>` is the first
  8 chars of the GUID -- matches the existing `GROUP_TITLE` ghost-glyph convention in
  `service-worker.js` (RE-READ it; keep the glyph as the `\u{1F47B}` escape, ASCII source).
- Grouping module (extension side): a PURE module (e.g. `extension/lib/grouping.js`, following the
  existing `extension/lib/` IIFE pattern) that `service-worker.js` imports and calls ON a
  `group_request` ONLY, to run `chrome.tabs.group`/`tabGroups` for the named tabs and title the
  group. It makes NO policy decision (owns durable group state only) and is unit-testable in
  isolation (the `tests/extension/grouping.test.js` target). Service side: `src/hub/session.rs` sends
  the request for a session's owned tabs (from H4); reuse of the same GUID reuses the group.

## SS7 -- H8 channels + web bind

- `channels.webapi.from` denial: rule label PINNED `channel/webapi_from`; result a plain deny with
  `decision = "deny"`, `denial_id` the existing `"D-"` + 8-hex scheme (assert the shape, not a
  literal). The web adapter's BUILTIN default fragment is `channels.webapi.from: { allow: ["localhost"] }`.
- Bind representation: a resolved config value `webapi.bind` (string). PINNED default `"127.0.0.1"`
  (bound EXPLICITLY; never `0.0.0.0`). The Console "Enable remote connections" writes a user-layer
  `webapi.bind` (e.g. `"0.0.0.0"`) AND the matching `channels.webapi.from` entry -- both are ordinary
  policy/config writes, never a code gate. The port: PINNED default `webapi.port = 4180`.
- The authenticated subject is recorded via the `identity` field per SS2 -- NOT a new audit key.

## SS8 -- Role marker + fail-loud invariant assertions (shared by H3, H6; added 2026-07-04)

The process's role (Decision 1: SERVICE won the ADAPTER/CONTROL endpoint claim, or ADAPTER lost it),
once learned (by argv: `service` subcommand -> SERVICE, bare -> ADAPTER), is recorded ONCE in a
single hub-owned marker and asserted at the two seams where a mismatch would mean the SoC boundary
already failed elsewhere: the governance chokepoint (must only ever run as SERVICE) and the
supervisor-start / self-heal path (`start_service`, must only ever run as ADAPTER, H6). This is a
fail-loud backstop, NOT a substitute for the structural separation (the ADAPTER's code never calls
governance; the SERVICE's code never calls the supervisor-start path) -- it exists so a future
accidental breach of that separation crashes immediately and loudly instead of silently misbehaving. This assertion is a no-op
(no output, no behavior change) whenever the role is already correct, so it does not affect the
all-open byte-identity invariant.

- New file `src/hub/role.rs` (H3 creates it; NEVER `src/governance/**` -- a7 forbids `crate::hub`
  there too, post-H3's own scanner extension). H3 also adds `pub mod role;` to `src/hub/mod.rs`
  (RE-READ its current module declarations, e.g. `pub mod handshake;`, and add the new line in the
  same style) -- WITHOUT this, `crate::hub::role::*` does not resolve from `src/transport`.
- PINNED shape (transcribe verbatim):
  ```
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Role { Service, Adapter }

  pub fn set_role(role: Role);   // panics "ghostlight process role decided twice" if called twice
  pub fn role() -> Role;         // panics "ghostlight process role read before it was decided" if unset
  pub fn assert_role(current: Role, required: Role, what: &str); // pure; see panic message below
  pub fn assert_service_role(what: &str); // = assert_role(role(), Role::Service, what)
  pub fn assert_adapter_role(what: &str); // = assert_role(role(), Role::Adapter, what)
  ```
- PINNED panic message from `assert_role` (transcribe verbatim; `{what}`/`{required:?}`/`{current:?}`
  are the only interpolations):
  `"invariant violated: {what} must only run when this process's role is {required:?}, but it is {current:?}"`
- H3 calls `set_role` exactly once, immediately after H2's endpoint-claim result is known in
  `run_mcp_server` (RE-READ H2's landed win/lose branch in `src/hub/mod.rs` for the exact call site;
  do not guess a line number). H3 also calls `assert_service_role("<the chokepoint function's own
  name>")` as the FIRST line of the governance chokepoint (RE-READ H2's landed `serve_session` /
  `handle_tools_call` to find the single function every call path enters first; pass that function's
  own name as `what`).
- H6 calls `assert_adapter_role("start_service")` as the FIRST line of `src/hub/supervisor.rs`'s
  `start_service` fn (SS5.2), before the supervisor-start command runs -- a SERVICE must never
  trigger a service start. (The old "spawn-on-demand" seam is gone; the amended Decision 8 has no
  in-editor spawn, so this is the sole ADAPTER-only lifecycle seam.)
- PINNED unit tests (transcribe verbatim; pure, touch no global `OnceLock`, so they cannot leak state
  into other tests) in `src/hub/role.rs`'s own `#[cfg(test)]` module:
  - `adapter_role_hitting_the_governance_chokepoint_panics`: `#[should_panic(expected = "must only run
    when this process's role is Service")]`; calls `assert_role(Role::Adapter, Role::Service, "test")`.
  - `service_role_hitting_spawn_on_demand_panics`: `#[should_panic(expected = "must only run when this
    process's role is Adapter")]`; calls `assert_role(Role::Service, Role::Adapter, "test")`.
  - `matching_roles_do_not_panic`: calls `assert_role(Role::Service, Role::Service, "test")` and
    `assert_role(Role::Adapter, Role::Adapter, "test")`; a plain (non-`should_panic`) test asserting
    neither call panics.
- PINNED wiring-verification tests (text-scan, NOT live-process tests -- they guard the CALL SITE
  existing, separately from `role.rs`'s own unit tests which guard the assertion LOGIC). Anchor the
  path the SAME way `tests/architecture.rs`'s `governance_dir()` does: join
  `env!("CARGO_MANIFEST_DIR")` with the file's repo-relative path and `std::fs::read_to_string` it
  (RE-READ `governance_dir()` for the exact pattern; do not invent a different path-resolution
  scheme):
  - H3 adds `tests/hub_role_wiring.rs::governance_chokepoint_asserts_service_role`: asserts the
    source of H2's landed governance-chokepoint function (RE-READ to find it) contains the literal
    substring `assert_service_role`.
  - H6 adds `supervisor_start_asserts_adapter_role` to `tests/hub_lifecycle.rs` (a file H6 already
    creates for its own Tests section): asserts the source of `src/hub/supervisor.rs` contains the
    literal substring `assert_adapter_role` (guarding that `start_service`'s SS8 seam is wired).

## SS9 -- Per-session state: corrected location post-H2 (added 2026-07-04; H3 BLOCKED on this)

H3 BLOCKED on landing: H2's two-endpoint re-authoring put the ADAPTER/CONTROL accept loop and the
session-hello read entirely in `src/transport/native/ipc.rs` (`serve_adapters`,
`handle_adapter_connection`), NOT in `src/hub/mod.rs` as H3/H4/H5/H7/H8 were drafted to assume (they
predate H2's two-endpoint amendment). `src/hub/mod.rs` only builds the shared `ServiceContext` and
spawns `ipc::serve_adapters`/opens the extension endpoint; it holds no accept loop and no
per-connection code itself. This section is the SINGLE corrected description every later task cites
instead of re-deriving its own.

- Peer-cred capture happens INSIDE `serve_adapters` (both platform variants, `ipc.rs`), on the
  CONCRETE platform type (`NamedPipeServer` post-`.connect()` on Windows; `UnixStream` from
  `.accept()` on Unix) -- BEFORE the stream passes to `handle_adapter_connection<S>`, which is
  GENERIC over `S: AsyncRead + AsyncWrite` and therefore CANNOT itself call
  `GetNamedPipeClientProcessId`/`SO_PEERCRED` (no concrete type reachable inside a generic body).
  H3 adds a platform-specific `capture_peer_cred` fn per platform in `ipc.rs` (Windows: the pipe
  client's process id + token SID; Unix: `SO_PEERCRED`/`getpeereid`), called at the capture point
  above, threading the resulting `PeerCred` as a new plain parameter into `handle_adapter_connection`
  (signature becomes `handle_adapter_connection<S>(ctx, stream: S, peer_cred: PeerCred)`).
- The session-hello is read and demuxed UNCHANGED, inside `handle_adapter_connection` itself
  (reading framed bytes off generic `S` needs no concrete type).
- Admission: `handle_adapter_connection`, immediately after parsing the GUID from the hello, calls
  `ctx.session_registry.lock().unwrap().admit(&guid, &peer_cred)` (see the new `ServiceContext`
  field below). `Refused` drops the connection (no dispatch, no session created; do not surface the
  GUID). `Admitted` proceeds to call `crate::mcp::server::serve_session(stream, ctx, guid)`.
- FIXED 2026-07-04 (fresh-eyes review after the first version of this section): `relay_adapter`
  (`src/transport/native/ipc.rs`) currently sends a PLACEHOLDER empty `"guid": ""` in its hello
  frame (its own doc comment already flags this as "the H3 seam"). H3 fixes this: `relay_adapter`
  mints ONE `SessionGuid::mint()` as a local variable at its top (before building the hello) and
  embeds `guid.as_str()` in place of `""`. Because `relay_adapter` itself runs exactly once per
  adapter process (it is not called in a loop), minting it as a local variable there already
  satisfies Decision 4 ("same adapter process reuses its GUID; a new adapter process mints a new
  one") -- no `OnceLock` or extra plumbing needed.
- `ServiceContext` (`src/hub/mod.rs`) gains ONE new field for H3:
  `session_registry: Arc<std::sync::Mutex<SessionRegistry>>`, built once in
  `ServiceContext::from_startup` alongside `browser`/`store`/`recorder` (`ServiceContext` is already
  `Clone`, so every session shares the one registry). `SessionRegistry` itself -- the H3
  admission/binding table, GUID -> bound `PeerCred` -- is UNCHANGED from `src/hub/session.rs`'s
  original design; only WHERE it is reachable from changes (a `ServiceContext` field, not a bespoke
  table dangling in `src/hub/mod.rs`).
- `serve_session`'s signature GAINS a parameter: `serve_session<S>(stream: S, ctx: ServiceContext,
  guid: SessionGuid) -> Result<()>` -- REVISED 2026-07-04 (fresh-eyes review): NOT `Option<SessionGuid>`.
  Every session gets a REAL, uniquely-minted GUID, including the SERVICE's own directly-served stdio
  session -- `run_as_service` calls `SessionGuid::mint()` for itself too (it is not "adapter-minted"
  in the Decision-4 sense, but minting one locally costs nothing and closes a real isolation gap: a
  `None`/exempt lone session would sit OUTSIDE H4's owned-tab bookkeeping entirely, so if an adapter
  session later touched the same tabId the lone session was already using, the adapter session could
  first-touch-adopt it with no refusal ever surfacing to either side -- two sessions silently sharing
  one tab. A uniformly-real GUID for every session, checked through the SAME `owned_tabs` map,
  closes this: a genuinely lone session still "owns everything it touches" (Decision 6), simply
  because first-touch-adoption always succeeds when nothing else contests it -- no `None`-branch
  special-casing needed anywhere downstream (H4's gate, H7's group-request emit). This is NOT a
  violation of H1's byte-identical-signature pin (H1 pinned transport-genericity over the STREAM
  type and byte-identical OUTPUT for the golden tests, never an eternal 2-parameter arity) -- H3's
  own Goal was ALWAYS "give every session an opaque identity". Minting/threading a GUID writes
  nothing to stdout or audit by itself (H3 does not stamp it into audit -- that is H8), so this
  produces IDENTICAL behavior/output to today (all-open byte-identity) regardless.
- FIXED 2026-07-04 (fresh-eyes review): `src/transport/mcp/server.rs::run` (the H1-era thin wrapper,
  `pub async fn run(browser, loaded_policy, user_source)`) is DEAD CODE as of H2's landing --
  `run_mcp_server` (`src/hub/mod.rs`) now calls `run_as_service`/`run_as_adapter` directly and never
  calls `run`. Confirmed via a repo-wide grep: no remaining call site, only stale doc-comment
  mentions (in `dispatch.rs`, `hub/mod.rs`, `tests/audit_recorder.rs`, `tests/manifest_validation.rs`
  -- comments only, not compiled call sites). Since `run` still calls
  `serve_session(stream, ctx)` with the OLD 2-arg signature, it will FAIL TO COMPILE once
  `serve_session` gains the `guid` parameter. H3 DELETES `run` (do not thread a fake guid into dead
  code) and, in passing, may correct the doc comments that describe it as live (not load-bearing;
  do this only if trivial, do not let it become a scope creep hunt).
- `SessionGuid` (`src/hub/session.rs`) needs `#[derive(Clone, PartialEq, Eq)]` (comparing
  `map.get(&tab_id) == Some(&my_guid)` and cloning it into a map entry both require it -- H4's
  design, added below, depends on this). `PeerUser` needs `#[derive(Clone, PartialEq, Eq, Hash)]`
  (H5's `mint_quota: Arc<Mutex<HashMap<PeerUser, usize>>>` requires `Hash`; H3's original pin listed
  only `Clone, PartialEq, Eq`, missing it). Both fixed here so H3 lands with the derives H4/H5
  actually need, rather than H5 discovering a missing `Hash` at its own build time.
- Downstream tasks (H4, H5, H7, H8) that assumed "the per-session dispatch / accept / admission
  layer lives in `src/hub/mod.rs`" instead read: the ACCEPT/ADMISSION layer is
  `src/transport/native/ipc.rs` (`serve_adapters`/`handle_adapter_connection`); the PER-REQUEST
  GOVERNANCE DISPATCH layer (where a per-request gate like H4's ownership check or H5's chunking
  actually runs, once per tool call) is `src/transport/mcp/server.rs::serve_session`'s read loop
  (which calls `crate::transport::mcp::pipeline::handle_tools_call` per request) -- NOT
  `src/hub/mod.rs`, which only builds shared state and spawns the two endpoints. Any NEW shared,
  cross-session state (H4's owned-tab map; H5's per-peer quota counters) is added as a NEW field on
  `ServiceContext`, exactly as `session_registry` is added here -- never as a standalone table
  floating in `src/hub/mod.rs`.
- Forward guidance for H4 (not a full spec -- H4's own task file decides the details): the
  cross-session "does anyone else own this tab" check does NOT require per-session records that
  cross-reference each other. A single SHARED map on `ServiceContext`,
  `owned_tabs: Arc<std::sync::Mutex<HashMap<i64, SessionGuid>>>` (tabId -> owning GUID), makes both
  checks O(1) without a session table: ownership is `map.get(&tab_id) == Some(&my_guid)`; adoption is
  `map.entry(tab_id).or_insert_with(|| my_guid.clone())`. `src/hub/session.rs` still holds the pure
  types (`SessionGuid`/`PeerCred`/`SessionRegistry`); it does not need to become a "session table of
  records" for H4 to build on. Because every session now carries a REAL `SessionGuid` (the revision
  above, not `Option<SessionGuid>`), H4's ownership gate runs THE SAME WAY for every session --
  there is no `None`-branch to special-case. A genuinely lone session still owns everything it
  touches (Decision 6), simply because first-touch-adoption always succeeds when no other live
  session contests the tabId.
- Forward guidance for H8 (not a full spec): `SessionRegistry`'s admission/binding model (H3) exists
  to stop a DIFFERENT local OS user from hijacking a reused GUID -- it has no meaning for a remote
  TCP peer, which has no OS credential to bind. The web listener does NOT call
  `ctx.session_registry.lock().unwrap().admit(...)` at all; it mints a fresh `SessionGuid::mint()`
  per accepted connection (mirroring the MINTING half of `handle_adapter_connection`'s pattern, not
  its admission half) and calls `serve_session(stream, ctx, guid)` directly. Trust for a web session
  is decided entirely by the `channels.webapi.from` policy (Decision 5/9), not by peer-cred binding.

## Resolved AUTHOR-MUST-PIN index (so none is left open)

| Task | value | pinned in |
| --- | --- | --- |
| H2 | two endpoints (ext unchanged + adapter/control) + adapter/control session-hello; NO ext hello, `relay_native_host` unchanged | SS1 |
| H2 | distinct client-name constructor | use `Governance::all_open` + `set_client(name, version)` as today (RE-READ H1; no new constructor) |
| H4 | uniform "unknown tab" string + audited-as-deny + domain/denial | SS3 |
| H5 | grace window, per-peer caps, quota message, oversize threshold + chunk, completion bound | SS4 |
| H6 | idle-grace, anti-squat failure string, per-install secret storage + proof shape | SS5 |
| H7 | group_request type + fields + reply, grouping fn, group title format | SS6 |
| H8 | channels denial rule/message/id, remote-bind representation, trusted-subject audit field | SS7 + SS2 |
