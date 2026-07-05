# H6: Adapter spawn-on-demand + detached non-admin lifecycle + anti-squat

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 8;
> Decision 1 for the role topology; "Preserved invariants" for the fences this task must not move).
> One task = one commit. Facts below are as-of-authoring 2026-07-04 -- RE-READ the named files before
> relying on any line number.

## Goal

Land the persistent-service lifecycle from ADR-0030 Decision 8. The thin ADAPTER checks whether the
service is up and, if not, SPAWNS it DETACHED and unparented, as the logged-in user (never
admin/SYSTEM), so the service sits in NEITHER the adapter's NOR Chrome's job object. The Chrome-
launched RELAY continues to ONLY connect (it never spawns). The persistent SERVICE stops running the
parent-death watchdog (it has no single parent) and instead shuts down on an idle-grace window; the
ADR-0029 parent-death reaper is re-scoped to the ADAPTER. The service proves possession of a
per-install secret to the adapter on connect (anti-squat) so a same-user process that squatted the
endpoint name cannot complete the handshake. The core's proc-identity/liveness ROLE is deleted;
`src/proc.rs` is retained only for the adapter's own parent-death lifecycle and the doctor reap. Why:
ADR-0030 Decision 8 ("lifecycle -- detached, non-admin, idle-grace, anti-squat") and the Consequences
note that this is "the largest core deletion the project has earned ... at H6."

## Authority

1. docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 8; Decision 1; Preserved invariants) -- NORMATIVE. Cite it; never restate its semantics.
2. BOOTSTRAP.md ground rules.
3. This task file.

If they conflict, the higher wins.

## Current-tree facts (as-of-authoring; RE-READ before relying)

STANDING ORDER: every line number and signature below is a 2026-07-04 snapshot. RE-READ the named
file before relying on any of it. H0-H5 land BEFORE this task and move code; the service/adapter split
(H2) in particular relocates the roles described here. If a STOP precondition's assumption is absent,
STOP -- do not improvise around a broken assumption.

- `src/hub/mod.rs` -- created by H0 (the `HubCore`/`ServiceContext` composition root, ADR-0030
  Decision 2). It does NOT exist in the 2026-07-04 tree. This task edits it for the detached-spawn and
  anti-squat logic. If it is absent, H0 has not landed: STOP (see STOP preconditions).

- `src/main.rs` (as-of-authoring, single dual-role binary; H2 splits adapter vs service):
  - `run_server(manifest, debug_on)` at ~L442 is TODAY the one server role. It captures the parent at
    ~L471 (`ghostlight::proc::parent()`), calls `ghostlight::doctor::sweep_orphans()` at ~L478, and at
    ~L496-506 spawns the parent-death watchdog: `if let Some(parent) = parent { ...
    ghostlight::transport::watchdog::wait_until_orphaned(parent).await; ... shutdown.notify_one(); }`.
    The single ordered teardown is `std::process::exit(code)` at ~L546.
  - `run_native_host_role(debug)` at ~L421 is the RELAY: it only calls `ipc::relay_native_host(...)`
    and `std::process::exit(0)`. It never spawns anything. This must STAY only-connect (ADR-0030
    Decision 1: "the relay only connects, so the service is never trapped in Chrome's job object").
  - Role detection precedes clap: the `chrome-extension://` positional selects the relay (~L310).
  - COUPLING that pins scope: after H2 the no-subcommand default role is the ADAPTER, and a distinct
    role (e.g. an argv selector introduced by H2) is the persistent SERVICE. This task wires the
    parent-death watchdog + reaper to the ADAPTER role ONLY and gives the SERVICE role idle-grace +
    the anti-squat listener. RE-READ how H2 named the two roles before editing.

- `src/transport/watchdog.rs` (as-of-authoring):
  - `POLL_INTERVAL: Duration = Duration::from_millis(1500)` at L23.
  - `wait_until<F: Fn() -> bool>(is_orphaned, poll)` at L27 (generic, unit-tested predicate loop).
  - `wait_until_orphaned(parent: ProcId)` at L37 -- the client-parent watchdog. After this task, ONLY
    the adapter role may call it. The persistent service must NEVER call it.
  - This module must keep its two inline tests green (`returns_once_the_predicate_reports_orphaned`,
    `does_not_return_while_the_predicate_stays_false`).

- `src/proc.rs` (as-of-authoring): platform process-liveness primitives (`ProcId`, `parent()`,
  `pid_exists`, `is_alive`, `orphaned`, `creation_time`, `terminate`). Module doc (L2-14) currently
  frames these as "for the mcp-server role" parent-death. This task retains proc.rs but RE-SCOPES its
  narrative to the ADAPTER's parent-death lifecycle and the doctor reap; the persistent service's
  liveness/identity ROLE is DELETED (Decision 8: "delete the core's proc-identity role"). Do NOT delete
  `orphaned`, `parent`, `terminate`, `pid_exists`, `is_alive`, or `ProcId`: the adapter watchdog and
  `doctor::reap` still use them. RE-READ `src/doctor.rs` `reap` (doctor.rs:600; role filter at
  doctor.rs:86/465, today `s.role != "mcp-server"`) and `sweep_orphans` (~L633) before touching role
  labels. Per PINS.md SS5 the SERVICE keeps the existing "mcp-server" debug/session role label and the
  ADAPTER gets a new "adapter" label at its `build_debug_sink` call site; re-scope `doctor::reap` to reap
  orphaned "adapter" sessions ONLY, NEVER the service (idle-grace only, never parent-reaped).

- `src/transport/native/ipc.rs`: `DEFAULT_ENDPOINT = "org.sylin.ghostlight.v1"` (L28);
  `default_endpoint()` (L31) reads `GHOSTLIGHT_ENDPOINT` else the default. The endpoint name is the
  well-known name a squatter would try to claim; the anti-squat secret defends the handshake ON it.

- `docs/adr/0029-process-lifecycle-hygiene.md`: AMEND (do not rewrite) to record that ADR-0030
  Decision 8 re-scopes the parent-death watchdog/reaper from the mcp-server to the ADAPTER, and that
  the persistent service uses idle-grace, not parent-death. Add a short "Superseded/amended by
  ADR-0030 Decision 8" note near the top; keep the historical body intact.

## Required behavior

Cite the ADR decision for each. What MUST stay byte-identical: the native-messaging wire, the 13+explain
schemas, the all-open byte-identity, and the a7 arch-test (see NEVER touch).

1. Detached, unparented, non-admin spawn (ADR-0030 Decision 8). The adapter, on finding the service
   absent, spawns the SAME binary in the service role DETACHED so it is in neither the adapter's nor
   Chrome's job object, and as the logged-in user, never elevated. Role-marker addendum (PINS.md SS8,
   added 2026-07-04 after H2/H3): call `hub::role::assert_adapter_role("<this function's own name>")`
   as the ABSOLUTE first line of the spawn-on-demand function's body, before any process-spawn call --
   a SERVICE must never spawn another service; this is the fail-loud backstop for that invariant
   (`src/hub/role.rs` is created by H3; RE-READ it, do not redefine `Role`/`assert_adapter_role` here):
   - Windows: create with `DETACHED_PROCESS` AND `CREATE_BREAKAWAY_FROM_JOB` (no job inheritance), and
     VERIFY breakaway (the spawned service is not a member of the adapter's / Chrome's job object).
     Do NOT elevate; inherit the caller's medium integrity (Decision 8: "must not exceed the medium
     integrity of the user's Chrome").
   - Unix: `setsid` / daemonize so it detaches from the controlling terminal and process group.
   NOTE (Decision 8 + ADR-0030 Consequences): there is NO existing job-object breakaway code to
   remove -- no service exists to be trapped yet; this phase ADDS the detached spawn.

2. The relay stays connect-only (ADR-0030 Decision 1). `run_native_host_role` must not gain any spawn
   path. Only the ADAPTER spawns the service.

3. Persistent service: NO client-parent watchdog; idle-grace shutdown instead (ADR-0030 Decision 8:
   "It shuts down on an idle-grace window (no sessions AND the extension link gone for the window),
   never on parent-death"). The service role must NOT call `watchdog::wait_until_orphaned`. It exits
   only when, for the whole grace window, there are zero live sessions AND the extension link is gone.
   - Idle-grace window duration: PINNED in PINS.md SS5 -- `pub const IDLE_GRACE: Duration = Duration::from_secs(30);` (30s; the service exits only after no sessions AND the extension link gone for this window). (ADR-0030 Decision 8 mandates an
     idle-grace window but pins no numeric value). Pin one constant (name + Duration) and its exact
     definition of "idle" here, then transcribe it into the test assertion below.

4. Parent-death reaper re-scoped to the ADAPTER (ADR-0030 Decision 8: "The ADR-0029 parent-death
   reaper is re-scoped to the ADAPTER"). The adapter role keeps `proc::parent()` capture, the
   `watchdog::wait_until_orphaned` detector, and `doctor::sweep_orphans()`. The service role runs
   none of these.

5. Anti-squat (ADR-0030 Decision 8: "the service proves possession of a per-install secret to the
   adapter on connect ... before any GUID/pairing flow proceeds"). On connect, the SERVICE proves it
   holds a per-install secret; the ADAPTER validates the proof against the secret it reads from an
   owner-only store, BEFORE the GUID/pairing flow (ADR-0030 Decision 4) proceeds. A same-user process
   that squatted the endpoint name but lacks the secret cannot complete the handshake.
   - Per-install secret storage (owner-only: 0600 on Unix / DPAPI-per-user on Windows, mirroring the
     GUID at-rest rule in ADR-0030 Decision 4), the handshake proof message shape, and the exact
     handshake-failure behavior/string/denial-id: PINNED in PINS.md SS5 -- the per-install secret is
     32 random bytes at `<data-dir>/hub-key` (0600 / DPAPI-per-user), generated on first service start;
     on connect the service sends
     `{"hub":1,"role":"service-proof","mac":<hex hmac-sha256(secret, the adapter's hello bytes)>}` and
     the adapter verifies by reading the same file, aborting on mismatch with the exact text "refusing
     to connect: the Ghostlight service on this endpoint is not the one this user installed" (a
     transport-admission abort, not a denial-id); data-dir is the existing %ProgramData%\ghostlight /
     platform equivalent already used by the debug/session files (RE-READ src/debug.rs) (ADR-0030 does not
     pin these values). Once pinned, transcribe the failure string/denial-id into the test below.

6. Delete the core's proc-identity/liveness ROLE (ADR-0030 Decision 8 + Consequences). The persistent
   service gains NO pid/ancestor/creation-time concept (Decision 4: "the governance core gains NO
   concept of pid / ancestor / creation-time"). `src/proc.rs` is retained ONLY for (a) the adapter's
   own parent-death lifecycle and (b) the doctor reap. Update proc.rs and watchdog.rs module docs to
   say "adapter role", not "mcp-server role".

MUST stay byte-identical / unchanged in behavior: a lone all-open session's output (the new spawn,
idle-grace, and anti-squat paths must be no-ops for a lone all-open session -- the secret handshake is
transport admission, not a tool decision), the native-messaging framing, and the a7 core/back-edge
boundary (all session/lifecycle/anti-squat code lands in `src/hub` or the binary shell, never in
`src/governance/**`).

## Tests (BY NAME; assertions pinned)

- Keep green (do not modify):
  - `tests/peer_death.rs::native_host_exits_when_server_dies`
  - `src/proc.rs` tests, especially
    `terminated_process_reads_as_dead_even_while_a_handle_is_held` (the ADR-0029 liveness landmine
    regression) and `wrong_creation_time_reads_as_dead_on_windows`.
  - `src/transport/watchdog.rs` inline tests: `returns_once_the_predicate_reports_orphaned`,
    `does_not_return_while_the_predicate_stays_false`.
  - `tests/all_open_golden.rs` (all-open byte-identity) and `tests/tool_schema_fidelity.rs`.
  - `tests/architecture.rs::governance_core_has_no_forbidden_back_edges` (a7).

- Add:
  - `tests/hub_lifecycle.rs::service_survives_the_spawning_adapter_exit`
    - Scenario: spawn an adapter process; it spawns the service detached; confirm the service is up
      (via the endpoint / its debug snapshot); kill the adapter; assert the service is STILL alive
      after the adapter's exit, for at least the idle-grace window (a session is held open, or the
      window has not elapsed).
    - PINNED assertion: the service process must still read alive (`ghostlight::proc::pid_exists`
      false only after the grace window) after the adapter is killed and reaped. Transcribe the
      idle-grace window constant pinned in Required behavior item 3 into the wait bound here.
      PINNED in PINS.md SS5: the idle-grace window constant is `IDLE_GRACE = Duration::from_secs(30)` (30s) -- not pinned in ADR-0030.
    - This directly exercises Decision 8: "It shuts down on an idle-grace window ... never on
      parent-death."

  - `tests/hub_lifecycle.rs::adapter_cannot_complete_handshake_with_an_impostor_service`
    - Scenario: an IMPOSTOR listener squats the well-known endpoint name (same user) but does NOT hold
      the per-install secret; an adapter connects and runs the anti-squat handshake.
    - PINNED assertion: the adapter REFUSES to proceed past the handshake (no GUID/pairing flow runs)
      and surfaces the exact handshake-failure result.
      PINNED in PINS.md SS5: the adapter aborts on mismatch with the exact text "refusing to connect:
      the Ghostlight service on this endpoint is not the one this user installed" (a transport-admission
      abort, not a denial-id; not pinned in ADR-0030).
    - This exercises Decision 8: "the service proves possession of a per-install secret to the adapter
      on connect (anti-squat) ... before any GUID/pairing flow proceeds."

  - `tests/hub_lifecycle.rs::spawn_on_demand_asserts_adapter_role` (PINS.md SS8; role-marker addendum,
    added 2026-07-04): a text-scan test (a7-style, NOT a live-process test) asserting the source of
    this task's own spawn-on-demand function (Required behavior item 1) contains the literal
    substring `assert_adapter_role`. This guards the WIRING; `src/hub/role.rs`'s own unit tests (added
    by H3; do not re-add them here) guard the assertion LOGIC.

## Verification (literal commands)

cargo build --all-targets
cargo test --test hub_lifecycle
cargo test --test peer_death
cargo test --lib proc
cargo test --lib watchdog
cargo test --test all_open_golden --test tool_schema_fidelity --test architecture
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check

## STOP preconditions

- If `src/hub/mod.rs` (the H0 composition root) is absent, STOP: H0 has not landed and this task has
  no home for its lifecycle code.
- If, after H2, the persistent SERVICE still wires `transport::watchdog::wait_until_orphaned` to a
  client parent, STOP and resolve that FIRST -- a persistent service must not exit on client death
  (ADR-0030 Decision 8; task NEVER-touch below). Do not layer H6 on top of a service that still dies
  with a client.
- If detached-spawn cannot GUARANTEE the service escapes the Chrome job object on Windows (breakaway
  cannot be verified), STOP and mark BLOCKED in the ledger with reasoning. The kill-on-close trap is
  the whole point of Decision 8; a service that Chrome can kill on window close defeats the task.
- If any AUTHOR-MUST-PIN value in this file is still unpinned at execution time (idle-grace window;
  anti-squat failure string/denial-id; per-install secret storage + handshake shape), STOP: the
  executor transcribes oracles, it never derives them.
- If `src/hub/role.rs` (created by H3; PINS.md SS8) is absent or its `assert_adapter_role` function
  no longer exists under that name, STOP -- H3 has not landed or was implemented differently; do not
  redefine the role marker here, and do not skip the spawn-on-demand assertion silently.
- If landing this task would require moving any NEVER-touch fence below, STOP.

## NEVER touch (this task)

Global fences (repeat, relevant to this task):
- `src/transport/mcp/tools.rs` (TOOLS_JSON: the 13 trained schemas + `explain`), byte-frozen. No
  exception.
- `tests/tool_schema_fidelity.rs`. No exception; keep green untouched.
- `tests/all_open_golden.rs` and the all-open byte-identity invariant (transcribed verbatim from
  ADR-0030 "Preserved invariants"): "a lone all-open session's output stays byte-identical through
  H0-H8 (`tests/all_open_golden.rs`); every new session/isolation path is a no-op for a lone all-open
  session." The spawn / idle-grace / anti-squat paths added here MUST be no-ops for a lone all-open
  session. No exception.
- `tests/architecture.rs` a7 (`governance_core_has_no_forbidden_back_edges`), transcribed verbatim
  from ADR-0030 "Preserved invariants": "`src/governance/**` names no browser/transport/mcp/native
  type nor the `url` crate; extended so the core also names no tabId/token/socket type. All
  session/multiplex/isolation code lands in `src/hub`." All lifecycle, anti-squat-secret, and
  detached-spawn code lands in `src/hub` or the binary shell -- NEVER in `src/governance/**`. No
  exception in this task (the H8-only `channels.webapi.from` allowlist exception does not apply here).
- `src/transport/native/host.rs` framing (4-byte LE prefix, `MAX_MESSAGE_LEN`, `encode`/`read_message`).
  No exception this batch.
- The MCP JSON-RPC wire + the pinned `notifications/tools/list_changed` line (`server.rs`). The
  adapter is a byte relay, never a rewriter.
- `Browser::attach` single-EXTENSION-link rejection (`AttachOutcome::AlreadyAttached`). Retained; not
  in scope for this task.

Task-specific fences:
- Do NOT let the persistent SERVICE inherit a client-parent watchdog: the service role must NEVER call
  `watchdog::wait_until_orphaned` and must NEVER capture a "parent" to die with (ADR-0030 Decision 8).
  Its only shutdown trigger is the idle-grace window.
- Do NOT add any spawn path to `run_native_host_role` (the relay). The relay only connects (Decision 1).
  No exception.
- Do NOT delete `ProcId`, `parent`, `orphaned`, `pid_exists`, `is_alive`, `creation_time`, or
  `terminate` from `src/proc.rs`: the ADAPTER watchdog and `doctor::reap` still depend on them. Only
  the SERVICE's use of them is removed (the "core proc-identity role" deletion). No exception.
- Do NOT spawn the service elevated or as SYSTEM (Decision 8: "NEVER admin/SYSTEM"). No exception.
