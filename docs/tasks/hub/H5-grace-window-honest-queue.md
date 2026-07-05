# H5: Reconnect grace window + honest bounded queue

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 3;
> Decision 4 peer-rate-limit-key clause; Decision 7 kill-precedence; "Preserved invariants (and
> the pinned oracles the batch transcribes)"). One task = one commit. Facts below are
> as-of-authoring 2026-07-04 -- RE-READ the named files before relying on any line number.

## Goal

Make a brief extension disconnect HOLD the session and its in-flight pending calls for a bounded
grace window (strictly less than the 60s `TOOL_TIMEOUT`) awaiting reconnect, instead of failing
every pending call the instant the stream closes. Add a per-peer (never global) mint/group quota
so one flooding peer cannot lock out honest peers, and a mandatory screenshot-chunking relay path
at the hub so a large payload cannot head-of-line-block the shared native port and starve honest
sessions. This lands ADR-0030 Decision 3 ("D1 -- the honest singleton queue"): we do not engineer
around the singleton, we queue honestly -- fair ordering, truthful failure on a REAL drop,
per-peer-identity quotas, mandatory chunking. Orthogonal to H3/H4; may land any time after H2.

## Authority

1. docs/adr/0030-ghostlight-hub-orchestrator.md is the single NORMATIVE design doc (Decision 3 and
   the cited clauses). CITE it; never restate its semantics.
2. docs/tasks/hub/BOOTSTRAP.md ground rules.
3. This task file.

If they conflict, the higher wins.

## Current-tree facts (as-of-authoring; RE-READ before relying)

- `src/transport/executor.rs` is the `Browser` handle module (the mcp-server's view of the
  connected extension). RE-READ it before touching anything below.
  - `const TOOL_TIMEOUT: Duration = Duration::from_secs(60);` (approx line 60). The pinned 60s.
  - `fn kill_error() -> ToolError` (approx lines 64-67): the kill error string, unchanged.
  - `send_and_await(...)` (approx lines 281-321): registers the pending oneshot, fails fast with
    `"Browser extension not connected"` if `outgoing` is `None` (approx line 297), then awaits the
    reply up to `TOOL_TIMEOUT`. The timeout match arm emits `"Tool request timed out after 60s"`
    (approx line 312); the `Ok(Err(_closed))` arm emits `"Browser extension disconnected before
    responding"` (approx lines 306-308).
  - `attach<S>(...)` (approx lines 340-402): on the live session's stream closing it runs
    `*self.outgoing.lock().unwrap() = None;` then
    `for (_, tx) in self.pending.lock().unwrap().drain() { let _ = tx.send(Err(drain_err.clone())); }`
    (approx lines 394-400), where `drain_err` is the disconnect error built at the loop break (approx
    lines 384-385). **This is the immediate-fail-on-detach behavior H5 replaces with a bounded hold.**
  - `handle_session_killed(...)` (approx lines 495-505) drains pending with `kill_error()`; the
    killed check at the top of `call`/`tab_url` (approx lines 222-224, 252-254) makes the kill error
    win over everything. The grace window MUST NOT weaken this: a kill still fails immediately and
    outlives a disconnect (see `kill_error_outlives_the_disconnect`).
- CORRECTED 2026-07-04 (PINS.md SS9; RE-READ it in full): the accept/admission layer is
  `src/transport/native/ipc.rs` (`serve_adapters`/`handle_adapter_connection`), NOT `src/hub/mod.rs`
  -- H2's two-endpoint re-authoring put it there. `src/hub/mod.rs` hosts the composition root and
  `ServiceContext` (the SHARED state every session clones), including H3's `session_registry` and
  H4's `owned_tabs` fields, added the same way this task adds its quota table. The per-peer
  OS-credential rate-limit key is Decision 4's "per-peer rate-limit key" clause: the `PeerCred` H3
  captures inside `serve_adapters` (per SS9), threaded into `handle_adapter_connection`. Session/
  isolation/quota code lives in `src/hub` and `src/transport/native/ipc.rs`, never in
  `src/governance/**`, per the a7 arch-test. RE-READ `src/hub/mod.rs` and `src/transport/native/
  ipc.rs`; if H2's landed shape does not match, see STOP preconditions.
- `src/transport/native/host.rs`: `MAX_MESSAGE_LEN = 128 * 1024 * 1024` (128 MiB), the `encode` /
  `read_message` framing (4-byte LE prefix). FROZEN this task; the chunking H5 adds is a HUB relay /
  scheduling property, never a change to this extension wire.
- Coupling that pins scope: the grace-hold touches `attach()`/`send_and_await` in `executor.rs`; the
  per-peer quota and the chunked relay live in `src/hub/mod.rs`. The error strings and the native
  wire are frozen, so the change is observable only as (a) pending calls surviving a sub-60s blip and
  (b) hub-level fairness under flooding / oversized replies.

## Required behavior

### 1. Bounded reconnect grace window (Decision 3: "truthful failure on a real drop")

On the live session's stream closing (the `AttachOutcome::Detached` path in `attach`), do NOT drain
and fail pending calls immediately. Instead:

- Mark the port disconnected (`outgoing = None`, as today, so no new frame can be sent) and start a
  bounded grace timer of duration `GRACE_WINDOW`, where `GRACE_WINDOW` is STRICTLY LESS THAN
  `TOOL_TIMEOUT` (60s, transcribed oracle below). `GRACE_WINDOW` is PINNED in PINS.md SS4:
  `pub const GRACE_WINDOW: Duration = Duration::from_secs(10);` (10s, strictly < the 60s
  `TOOL_TIMEOUT`).
- If a fresh `attach` arrives within `GRACE_WINDOW`, the session continues and pending calls are NOT
  failed by the disconnect (each still bounded by its own outer `TOOL_TIMEOUT`).
- If `GRACE_WINDOW` elapses with no reconnect (a REAL drop), THEN drain pending with the EXACT,
  UNCHANGED disconnect error `"Browser extension disconnected before responding"` (byte-identical to
  today's `drain_err`). The grace window changes WHEN pending fail on a real drop, never the error
  TEXT or hop.
- A `session_killed` event during the grace window still wins immediately (kill drains pending with
  `kill_error()` and latches `killed`); the grace hold must not delay or mask a kill. A never-had-a-
  connection `call` still fails fast with `"Browser extension not connected"` (the grace window
  applies only to a session that WAS connected with pending calls in flight).

Cite: ADR-0030 Decision 3; Decision 7 (kill is global and precedes). MUST stay byte-identical: all
four hop-attributed error strings and the `[hop: extension]` prefix (oracle below).

### 2. Per-peer (never global) mint/group quota (Decision 3 + Decision 4)

CORRECTED 2026-07-04 (PINS.md SS9): in `handle_adapter_connection` (`src/transport/native/ipc.rs`),
using the `PeerCred` captured in `serve_adapters` (per SS9 and H3), check/increment a per-peer quota
counter BEFORE admission (`SessionRegistry::admit`) proceeds. The counter is a NEW shared
`ServiceContext` field (`src/hub/mod.rs`), added the same way H3's `session_registry` and H4's
`owned_tabs` were: e.g. `mint_quota: Arc<std::sync::Mutex<HashMap<PeerUser, usize>>>` (RE-READ
`src/hub/session.rs` for the exact `PeerUser` type H3 landed). The quota is keyed on the peer
credential (Decision 4's "per-peer rate-limit key" clause; the GUID is treated as secret material,
so the QUOTA key is the peer credential, not the GUID value in logs). The quota is PER PEER, NEVER a
single global cap (a global cap is itself a lockout DoS, per Decision 3). When a peer exceeds its
cap, the offending mint/enqueue is DENIED; other peers are unaffected and continue to be served.

- `PER_PEER_MINT_CAP` is PINNED in PINS.md SS4: `pub const PER_PEER_MINT_CAP: usize = 32;` (and the
  paired `pub const PER_PEER_GROUP_CAP: usize = 32;`, equal by design), never a single global cap.
- The quota-exceeded result is PINNED in PINS.md SS4: a plain tool error with the exact text
  `session limit reached for this client` (no denial-id is minted for this path -- SS4 resolves it as
  a plain tool error, not a governance denial; no global lockout, a second, different peer still
  succeeds). This denial is a HUB admission decision, not a change to the 13+`explain` tool surface.

Cite: ADR-0030 Decision 3 ("per-peer-identity mint/group quotas (never a single global cap)");
Decision 4 ("the per-peer rate-limit key" transport-side amendment, in `src/hub`, never
`src/governance`).

### 3. Mandatory screenshot chunking so a large payload cannot head-of-line-block (Decision 3)

CORRECTED 2026-07-04 (PINS.md SS9, RESOLVED after fresh-eyes review found the original phrasing
would likely re-block): the file "src/hub/mod.rs" no longer holds per-connection relay code (see
SS9) -- RE-READ H2's landed reply-routing before writing a line. `Browser`
(`src/transport/executor.rs`) routes each extension reply to its requesting session's pending
oneshot by id (the `Arc<Mutex<HashMap>> pending` correlation H2 relies on); that hop is a single
in-memory channel send (not itself a bottleneck). The actual relay to the outside world is each
session's OWN writer task inside `serve_session` (`src/transport/mcp/server.rs`), writing the
JSON-RPC reply to that session's OWN stream (adapter pipe, or the web socket from H8). Sessions
already run as independent tokio tasks, so one session's large write does not block another
session's task BY DEFAULT under a multi-threaded runtime -- but it CAN starve others under a
single-threaded runtime, or if a future change introduces a shared lock across the write path, and
Decision 3 wants this guaranteed, not incidental. RESOLVED mechanism: in `serve_session`'s writer
task, when the serialized reply is `>= SCREENSHOT_CHUNK_THRESHOLD` bytes, write it in FIXED-SIZE
chunks (e.g. 1 MiB) via repeated `write_all` calls, with an explicit `tokio::task::yield_now().await`
between chunks. This is a concrete, testable guarantee -- the runtime is given an explicit
scheduling point between chunks, so another session's small reply is never starved for longer than
one chunk's write time, even in the worst case. This is a WRITE-OUT chunking on the
service<->adapter/web hop ONLY: it never touches the frozen `host.rs` extension wire (the
extension's OWN reply to the service is read as a single frame, unchanged) and never changes the
JSON-RPC message's content or framing, only how many `write_all` calls deliver it. The single
service worker + single native port is an accepted, DOCUMENTED serialization bottleneck (fair
ordering, truthful failure on a real drop); H5 does not hide it. The chunking is a HUB relay /
scheduling property only.

- The oversize threshold is PINNED in PINS.md SS4:
  `pub const SCREENSHOT_CHUNK_THRESHOLD: usize = 8 * 1024 * 1024;` (payloads at/above 8 MiB are
  chunked, well under `MAX_MESSAGE_LEN`; chunking is on the service<->adapter/web hop ONLY, never the
  frozen `host.rs` extension wire).
- Out of scope, DO NOT DO: any change to `src/transport/native/host.rs` framing or `MAX_MESSAGE_LEN`,
  and any fair-chunking that alters the EXTENSION wire (splitting frames on the native-messaging
  channel is a separate, later concern; this task's chunking is strictly hub-internal relay).

Cite: ADR-0030 Decision 3 ("MANDATORY screenshot chunking so a large payload ... cannot
head-of-line-block the shared port and starve honest sessions"; "We do not engineer around the
singleton; we queue honestly").

### 4. Document the accepted bottleneck (Decision 3)

Add a normative-pointer note (module doc in `src/hub/mod.rs` and a short amendment note in
`docs/adr/0004-reject-second-session.md` cross-referencing that ADR-0030 repeals it at the MCP-client
layer) recording the single service worker + single native port as an ACCEPTED, TRUTHFUL
serialization bottleneck -- fair ordering and truthful failure on a real drop, no hidden work-around.
Do NOT restate Decision 3's semantics; cite it. Do NOT alter ADR-0004's Status or its retained
single-physical-extension-link invariant; only add a cross-reference note.

## Tests (BY NAME; assertions pinned)

### Keep green (do not modify)

- `src/transport/executor.rs::call_without_a_connection_fails_fast` (inline; approx line 585): a
  never-connected `call` still fails fast with `"not connected"` under `[hop: extension]`. The grace
  window must not touch this path.
- `src/transport/executor.rs::kill_error_outlives_the_disconnect` (inline; approx line 877): a kill
  before a disconnect still makes the next `call` fail with the kill string, not the disconnect
  string. The grace hold must not weaken kill precedence.
- `tests/peer_death.rs` (whole file): the native-host still exits when its server peer dies.
- `tests/all_open_golden.rs` (whole file): a lone all-open session's output stays byte-identical; the
  grace window / quota / chunk relay MUST be a no-op for a lone all-open session (single peer, never
  over cap, no concurrent session to starve).

### Add

New integration test file `tests/hub_queue.rs`. Every expected value below that is not a transcribed
oracle is marked AUTHOR MUST PIN and MUST be replaced with a concrete pinned literal before the
executor runs; the executor TRANSCRIBES, never derives.

- `tests/hub_queue.rs::per_peer_mint_cap_denies_a_flooding_peer_without_locking_out_others`
  - Arrange two distinct peers A and B against the hub quota keyed on peer credential.
  - Peer A mints/enqueues up to `PER_PEER_MINT_CAP` (PINNED in PINS.md SS4 = 32) successfully, then
    its next mint is DENIED with the quota-exceeded tool error `session limit reached for this
    client` (PINNED in PINS.md SS4, verbatim as the pinned literal).
  - Pinned assertion 1: A's over-cap mint result equals the pinned quota tool-error text
    `session limit reached for this client` (PINS.md SS4; this path mints NO denial-id).
  - Pinned assertion 2: peer B, a distinct peer, mints and is served successfully WHILE A is over its
    cap (proves the cap is per-peer, never global -- B is not locked out). Assert B's mint succeeds.
  - Oracle note: no ADR-pinned string exists for this denial; it is PINNED in PINS.md SS4 as the
    plain tool error `session limit reached for this client`. Do NOT reuse a governance denial-id;
    this is a hub admission denial.

- `tests/hub_queue.rs::oversized_screenshot_is_chunked_not_head_of_line_blocking`
  - Arrange two concurrent sessions through the hub. Session 1 receives a large reply of size
    `>= OVERSIZE_THRESHOLD` and up to `MAX_MESSAGE_LEN` (128 MiB; transcribed from
    `src/transport/native/host.rs`). Session 2 issues a small honest call concurrently.
  - Pinned assertion 1: session 2's small call completes within a bounded time WELL UNDER
    `TOOL_TIMEOUT` while session 1's large payload is still being relayed -- i.e. session 2 is not
    head-of-line-blocked behind session 1. The exact completion bound is PINNED in PINS.md SS4 at
    `< 2s` (a tiny call must complete while a chunked large payload streams; strictly less than the
    60s oracle).
  - Pinned assertion 2: the large payload is delivered to session 1's stream in more than one
    `write_all` call (assert the chunk count `> 1` given a payload `>= OVERSIZE_THRESHOLD`). CLARIFIED
    2026-07-04 (PINS.md SS9): the resolved chunking mechanism (item 3) writes the SAME bytes as an
    unchunked write would -- chunking changes the NUMBER of write calls and inserts a yield between
    them, not the byte content -- so this assertion CANNOT be observed by reading the final byte
    stream (chunked and unchunked writes are byte-identical on the wire). Session 1's test-side
    stream/writer must be a wrapped/counting `AsyncWrite` double that records each `poll_write`/
    `write_all` invocation, so the test observes the WRITE CALLS, not just the delivered bytes.
    Separately, assert `src/transport/native/host.rs` framing is untouched (this is a build-time
    invariant, not a runtime assert).
  - Oracle transcribed: `MAX_MESSAGE_LEN = 128 * 1024 * 1024` and `TOOL_TIMEOUT = 60s`. The oversize
    threshold (`SCREENSHOT_CHUNK_THRESHOLD` = 8 MiB), chunk size, and the session-2 completion bound
    (`< 2s`) are PINNED in PINS.md SS4.

## Transcribed oracles (verbatim from ADR-0030 "Preserved invariants"; MUST stay byte-identical)

These four hop-attributed error strings and the timeout constant are UNCHANGED by H5. Transcribed
here to prove the grace window does not alter them:

- not-connected: `Browser extension not connected`
- kill: `The user ended the browser session (kill switch)`
- disconnect: `Browser extension disconnected before responding`
- timeout: `Tool request timed out after 60s`
- All render under the `[hop: extension]` prefix (kill / not-connected / disconnect) as their tests
  pin.
- `TOOL_TIMEOUT` = 60s. `GRACE_WINDOW` MUST be strictly less than this.
- Single-consumer kill hook to preserve precedence: `Browser::on_session_killed`
  (`src/transport/executor.rs`).

## Verification (literal commands)

```
cargo build --all-targets
cargo test --test hub_queue
cargo test --test all_open_golden
cargo test --test peer_death
cargo test --lib -- transport::executor::tests::call_without_a_connection_fails_fast transport::executor::tests::kill_error_outlives_the_disconnect
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## STOP preconditions

- If `src/transport/executor.rs`'s `attach` no longer drains and FAILS pending calls on detach
  (i.e. it already buffers/holds pending across a disconnect), the premise of this task is wrong:
  STOP and re-scope. Do not invent a hold on top of an existing hold.
- If introducing `GRACE_WINDOW` would change the exact hop-attributed error TEXT that the executor
  tests pin (`Browser extension disconnected before responding`, `Tool request timed out after 60s`,
  `Browser extension not connected`, `The user ended the browser session (kill switch)`), STOP and
  keep the strings byte-identical; the grace window changes timing only.
- If `src/hub/mod.rs`'s `ServiceContext`, or `src/transport/native/ipc.rs`'s `serve_adapters`/
  `handle_adapter_connection`, do not exist or no longer match PINS.md SS9's description, H5's
  prerequisite (H2/H3 landed as SS9 describes) is absent or diverged: STOP and reconcile against the
  ACTUAL landed shape.
- If any AUTHOR-MUST-PIN value in this file is still literally "AUTHOR MUST PIN" when you reach it,
  STOP: those are pinned by the batch author before execution, never derived by the executor.
- If landing any part of this task would require moving a NEVER-touch fence below, STOP.

## NEVER touch (this task)

- `src/transport/mcp/tools.rs` (TOOLS_JSON: the 13 trained schemas + `explain`), byte-frozen. No
  exception.
- `tests/tool_schema_fidelity.rs`. No exception; keep green untouched.
- `tests/all_open_golden.rs` + the all-open byte-identity invariant. No exception; every new
  grace/quota/chunk path MUST be a no-op for a lone all-open session.
- `tests/architecture.rs` a7 (`governance_core_has_no_forbidden_back_edges`): `src/governance/**`
  names no browser/transport/mcp/native/url and no tabId/token/socket type. All quota / session /
  isolation code lands in `src/hub`. No sanctioned exception for H5 (the H8-only
  `channels.webapi.from` allowlist exception does not apply here).
- `src/transport/native/host.rs` framing (4-byte LE prefix, `MAX_MESSAGE_LEN`,
  `encode`/`read_message`). No exception this task; the chunking is hub-internal relay only, and any
  fair-chunking that changes the EXTENSION wire is OUT OF SCOPE here.
- The four pinned hop-attributed error strings (transcribed above). Byte-frozen; the grace window
  changes timing, never text.
- `Browser::attach` single-EXTENSION-link rejection (`AttachOutcome::AlreadyAttached`). Retained; H5
  must not weaken the single physical-extension-link invariant. The grace window holds the SESSION
  and its pending across a reconnect of the SAME single link; it does not admit a second physical
  link.
- The MCP JSON-RPC wire + the `notifications/tools/list_changed` line (`server.rs`). Untouched.
- `docs/adr/0004-reject-second-session.md`: add only a cross-reference note to ADR-0030; do NOT change
  its Status or its retained single-physical-extension-link invariant.
