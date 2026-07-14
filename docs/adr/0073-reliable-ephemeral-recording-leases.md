# ADR-0073: Reliable ephemeral recording leases

Date: 2026-07-13
Status: Accepted
Amends: ADR-0050, ADR-0052, ADR-0053, ADR-0058
Builds on: ADR-0060, ADR-0072

## Context

The first live coordinate export of a useful GIF failed at the browser boundary. Benchmarking the
preserved frames then disproved the suspected encoder bottleneck: twelve 1277x915 frames encoded in
well under one second. The failure happened after the service sent a host-to-extension native
message larger than Chrome's 1 MiB limit. Chrome disconnected the native host, the request lost its
reply, and the caller waited for the generic 60-second tool timeout.

The investigation exposed deeper recording-lifecycle problems:

- recording state is keyed only by native tab id, so equal tab ids in two browser slots collide;
- recording ownership is not bound to an MCP session or a recording generation;
- start destroys a frozen recording before the replacement proves it can capture;
- stop marks the store inactive before the extension supplies a final frame;
- delayed frames can cross start/stop boundaries;
- a frame-count cap does not bound compressed bytes, decoded memory, or encoded output;
- frames are written to an OS temporary directory even though pixels may contain personal data;
- capture can outlive its initiating session, authority, or useful workflow;
- the current encoder holds all decoded and indexed frames at once;
- debug instrumentation can persist full MCP bodies and successful browser results, including
  image content.

ADR-0052 chose durable frames to survive a killable extension worker. ADR-0053 correctly moved the
state into the normal service process, but retained disk storage. Once the service owns capture,
privacy is the stronger requirement: internal reliability must not require persistent copies of
captured content.

## Decision

### 1. Recording is an owned state machine

The service owns a `RecordingCoordinator`. A recording has:

- an opaque recording id;
- the owner session GUID;
- a surface id containing browser slot and native tab id;
- a monotonically fresh generation;
- an explicit state;
- idle, hard, health-lease, and retention deadlines;
- bounded compressed frames and content-free summary metadata.

The lifecycle is:

```text
Starting -> Recording -> Finalizing -> Frozen
    |           |             |
    +-----------+-----------> Interrupted

Frozen/Interrupted -> export snapshot -> Frozen/Interrupted
Any state -> Erased
```

`Expired` and `Erased` may remain as content-free tombstones for truthful status. Every frame names
its recording id, generation, and sequence. A frame for another owner, surface, generation, or
non-accepting state is dropped.

### 2. Start and stop are transactional

Start creates a staging generation. A prior frozen recording remains intact until the extension
confirms capture started. A failed or timed-out start discards only the staging generation. Starting
while already recording reports that recording rather than silently replacing it.

Stop is a finalization barrier. The extension captures and sends a final frame before replying to
`gif_capture_stop`; native-message ordering makes receipt of that frame precede the reply. The
service freezes only after the reply. If finalization cannot complete within its deadline, the
recording becomes `Interrupted` and keeps every accepted frame.

Export of an active recording first runs the same finalization barrier. Explicit stop remains
available when a caller wants to freeze before interacting with an export destination.

### 3. Capture is a renewable, time-bounded lease

Capture is never represented by an unbounded boolean. Initial product defaults are:

```text
idle timeout       30 seconds
hard duration     120 seconds
health lease       15 seconds
lease renewal       5 seconds
frozen retention    5 minutes
```

The service refreshes idle time when an authorized browser operation targeting the same surface is
accepted and again when it completes. Status, export, unrelated tabs, and non-browser work do not
refresh it. The hard deadline and frozen-retention deadline never move.

The service renews the extension's short health lease independently of tool calls. Every renewal
names the recording id and generation. A stale renewal cannot revive a replacement. The extension
checks expiry before forwarding a frame and stops the screencast when the lease expires. It also
stops every screencast immediately when the native port disconnects; an MV3 timer is a backstop, not
the sole cleanup mechanism.

Idle and hard expiry attempt finalization, then freeze. A browser loss interrupts. Take-the-wheel
stops capture. Explicit clear, owner-session end, panic kill, policy revocation, retention expiry,
and service exit erase captured content.

### 4. Captured content is memory-only

Before explicit export, frame and GIF bytes exist only in bounded volatile memory owned by the
recording. Ghostlight does not intentionally write them to files, logs, audit records, extension
storage, persistent queues, or restart state.

Frames remain compressed until an encoder pass needs them. The store enforces per-frame,
per-recording, and service-wide byte bounds in addition to the frame cap. It preserves seed,
action-tagged, and final frames preferentially while thinning ordinary change frames. If protected
content cannot fit, capture stops with `memory_limit`; it never spills to disk.

Owned byte buffers are zeroized on final release as best-effort process hygiene. This is not a claim
against OS paging, crash dumps, allocator copies, browser internals, microarchitectural leakage, or
copies held by an MCP client or page after explicit export.

Service restart recovery is intentionally content-destructive. A restart may retain ordinary
content-free audit facts, but no recording bytes survive.

### 5. Encoding and delivery are bounded and revocable

GIF encoding becomes a two-pass streaming pipeline. The first pass decodes, composites, and samples
one frame at a time into a bounded global-palette training set. The second pass decodes,
composites, quantizes, and writes one frame at a time. It does not retain every RGBA and indexed
frame simultaneously.

The writer enforces a hard encoded-output ceiling. Clear, expiry, session teardown, panic, and
policy change revoke delivery: an encode that raced one of those events may finish its current
bounded computation, but its bytes cannot cross the export boundary. Stage metrics may include
durations, counts, and byte sizes, never pixels or base64.

### 6. Export is immutable, repeatable, and truthful

Frozen and interrupted recordings are immutable and exportable until their non-renewing retention
deadline. Export does not consume or clear them. Failure leaves the recording available for retry.
Clear is the only ordinary destructive tool action.

The model-facing `gif_creator` surface grows additively:

- `status` reports the current recording summary;
- export may use a stable element `ref` as well as a coordinate;
- export automatically finalizes an active recording;
- every result carries structured state, deadlines, counts, stop reason, and next actions.

The original four actions remain. None of the 13 trained schemas changes.

Delivery results distinguish `prepared_for_client`, `dispatched` with `unverified` acceptance, and
`outcome_unknown`. No current path claims observed acceptance. Dispatching a DOM drop event is
never described as proof that a page or remote server accepted the file. A disconnect after a
possibly side-effecting dispatch is not retried automatically.

### 7. Governance follows the actual branch

The tool directory gains data-driven per-variant resource and requirement resolution:

- start: Read against the live tab;
- stop, status, and clear: recording-scoped, capability none;
- download export: Read against the session-owned recording resource;
- ref/coordinate export: Write against the live destination tab.

Stop and clear remain possible when the browser has disappeared. Ownership is checked against the
recording's session GUID, never inferred from native tab id alone. A policy change that removes the
capture authority erases the bytes rather than retaining now-unauthorized content.

### 8. Diagnostics are payload-free

Operational debug events retain method, tool, action, ids safe for diagnostics, state transitions,
durations, counts, and byte sizes. They do not retain complete MCP request/response bodies,
successful browser results, frame payloads, GIF payloads, page text, file bytes, or form values.
Audit remains argument-free and content-free as today.

## Consequences

- A recording cannot silently mix browsers, sessions, or generations.
- An acknowledged stop includes the final-frame barrier; failures remain exportable and explicit.
- Capture cannot outlive its owner, absolute deadline, health lease, or current authority.
- Sensitive pixels are not deliberately persisted inside Ghostlight. A service crash loses the
  recording by design.
- Large recordings are bounded by bytes and streaming memory, not only frame count.
- The common LLM workflow is `start_recording -> ordinary work -> export`.
- The extension gains only Chrome-mechanism lease enforcement and transient generation state. It
  gains no policy, durable recording store, encoder, or telemetry.

## Rejected alternatives

- Keep disk frames and encrypt them. Rejected because an implicit persistent copy still exists and
  restart recovery is not worth the sensitive-data lifetime.
- Rely only on the service timeout. Rejected because a dead service cannot command the extension to
  stop.
- Rely only on an extension timer. Rejected because MV3 workers and their timers can terminate.
- Refresh the hard timeout on tool use. Rejected because busy activity could keep capture alive
  forever.
- Automatically replay coordinate/ref export after disconnect. Rejected because the page effect is
  not idempotent and may already have occurred.

## Prior art

- Chrome DevTools Protocol `Page.startScreencast`, `screencastFrameAck`, and `stopScreencast`.
- W3C MediaStream Recording final `dataavailable` before `stop` ordering.
- Kubernetes Lease holder identity, renewal time, and lease duration.
- gRPC deadlines and cancellation propagation.
- W3C Screen Capture indicator and user-stop guidance.
- OWASP logging data-exclusion guidance and RustCrypto `zeroize` limitations.

## Amendment: truthful recording indicator (2026-07-14)

ADR-0079 adds a user-visible REC state tied to the real screencast start/stop lifecycle. V1 uses
extension chrome and popup feedback rather than an in-page live preview, so the cue cannot be
recursively captured into the recording. It creates no new recording authority, persistence,
capture path, or policy behavior.
