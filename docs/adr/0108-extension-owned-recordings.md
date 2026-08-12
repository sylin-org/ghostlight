# ADR-0108: Extension-owned volatile recordings

Date: 2026-08-12
Status: Accepted
Supersedes: ADR-0053 Decisions 2-4 and ADR-0073 Decisions 1-4
Amends: ADR-0107

## Context

The unreleased recording rebuild split one state machine across the extension and orchestrator.
The extension owned Chrome capture and a safety lease. The orchestrator owned recording identity,
frames, deadlines, retention, and renewal. Unsolicited frame events then required both sides to
agree about generation, authority, expiry, finalization, and cleanup.

That split put the recording lifecycle at the wrong seam. Recording is a browser capability. The
component that starts Chrome capture can also bound it, stop it, retain it briefly, and erase it.
The orchestrator needs to ask for recording actions and govern disclosure. It does not need a
second recording coordinator.

## Decision

### 1. The extension is the sole recording owner

The extension owns a plural `RecordingRegistry`. Each entry has an extension-minted opaque id, an
opaque workspace namespace, a tab id, state, source URLs, compressed JPEG frames, byte and frame
counts, an absolute capture deadline, and an absolute frozen-retention deadline.

The registry is keyed by recording id and separately indexes active recordings by tab. It is not a
singleton current-recording variable. Equal native tab ids cannot collide within one extension,
and opaque workspace namespaces prevent one service workspace from selecting another's entry.
The extension remains policy-free: the namespace is compared for equality and never interpreted.

### 2. The physical protocol is request and receipt only

The orchestrator sends five closed browser commands:

- start on a tab;
- status by optional recording id;
- stop by optional recording id;
- read by optional recording id; and
- discard by optional recording id.

The extension returns typed summaries and, only for read, bounded base64 JPEG frames. There is no
renew command, service-side generation, recording-frame event, recording-ended event, health
lease, or service recording maintenance loop.

An omitted id succeeds only when exactly one recording belongs to the requesting workspace.
Ambiguity returns that workspace's corrective ids. An explicit foreign or expired id is simply not
found.

### 3. Capture and retention stop themselves

The extension owns fixed capture parameters and every physical bound: 120 seconds maximum capture,
100 milliseconds minimum kept-frame interval, 2 MiB per frame, 5 MiB per recording, 16 MiB across
recordings, 100 frames per recording, and 16 retained recordings. These are adapter mechanism
constants, not model inputs.

Stop captures a final bounded screenshot, stops the screencast, and freezes the entry. A hard
deadline, memory limit, oversized frame, browser detach, local runtime hold, or service disconnect
interrupts capture without waiting for the orchestrator. Frozen and interrupted entries remain
readable for five minutes, then erase themselves. Discard erases immediately. MV3 worker loss also
loses all recording memory by design.

Frames are never written to extension storage, service storage, logs, audit, restart state, or a
temporary file. Chrome and JavaScript runtime copies are outside a meaningful zeroization claim.

#### Amendment: retained visual spans

The fixed 100-frame limit is replaced by exact-frame folding. Before retaining a sampled frame,
the extension compares its bytes with the latest retained frame. If they are identical, it keeps
one frame and adds the elapsed sample time to that frame's `duration_ms`. Ten identical samples at
100 millisecond intervals therefore become one retained frame with a 1,000 millisecond duration.

Capture time and compressed bytes are the ordinary limits. A derived 1,202-frame ceiling remains
only as a defensive invariant: it is the maximum implied by the 120-second hard deadline, the
100-millisecond sampling interval, and seed/final capture. The read receipt carries each retained
frame's duration instead of its wall-clock timestamp, and the GIF renderer uses that duration
directly.

The first JPEG and PNG live trials both retained hundreds of frames on Example Domain. They did not
prove encoder instability: the composed page still contained Ghostlight's four-second controlled-
scope breath, so its pixels genuinely changed. Recording remains JPEG. While capture is active,
the extension disables only that perpetual border glow. Cursor movement, target effects, captions,
signatures, denials, and attention remain available. Every terminal recording path restores the
glow, and service-worker recovery resets it because recording memory is already gone. Comparison
remains deliberately byte-exact; differently encoded frames remain separate and the existing byte
limits stop capture safely.

The resulting live trial retained 15 frames and 121,293 JPEG bytes across 35 seconds on the same
page. The GIF was 211,458 bytes, carried 35,320 milliseconds of playback, and represented the
static tail as one 33,720-millisecond frame. Repeating save produced identical bytes. This is the
accepted mechanism; decoded-pixel hashing is not needed.

A separate Foundry trial proved the dynamic path. A tightly bounded hover, click, and type sequence
retained six distinct frames across 670 milliseconds and produced a 595,861-byte GIF. Repeated save
was byte-identical. Longer Foundry trials also exposed the limit of the deliberately basic GIF
renderer: 116 dynamic frames held 3,328,297 JPEG bytes but encoded beyond the 5 MiB output ceiling,
and a later 189-frame capture stopped itself at the recording memory limit. Improving long dynamic
GIF output remains part of the deferred GIF work, not the capture ownership mechanism.

### 4. Governance and delivery stay in the orchestrator

The orchestrator authorizes start against the source tab and authorizes disclosure before bytes
cross to an MCP client. It rechecks every sanitized source URL reported by the extension. A target
save authorizes Write against the destination, receives the bounded frames, renders the current
GIF form, and dispatches the resulting bytes through the ordinary upload primitive.

The orchestrator owns language, policy, output rendering, and delivery truth. It does not retain a
recording registry, accept asynchronous frames, renew capture, or decide capture bounds.

Enhanced GIF composition, overlays, action tagging, and palette quality are deferred. This ADR
changes ownership and lifecycle only; it keeps one bounded basic GIF export so the approved
`browser_record save` surface remains usable.

## Consequences

- One component owns each recording from start through automatic erase.
- Browser loss and service loss stop capture locally even when no command can arrive.
- The bridge carries larger receipts only on explicit read; relays remain opaque.
- A service restart cannot reconstruct extension-held workspace authority. A changed service epoch
  interrupts old capture, and retained entries expire without crossing into the new service.
- The extension is larger than ADR-0053 allowed, but the added code is one browser-mechanism
  aggregate rather than duplicated product logic.

## Rejected alternatives

### Service coordinator plus extension safety lease

Rejected because identity, deadlines, finalization, memory, and revocation were split across two
owners and synchronized by renewal plus unsolicited events.

### Stream every kept frame to the service

Rejected because it makes the service part of capture, extends sensitive-byte lifetime across
processes, and requires lifecycle arbitration for frames that arrive around stop or disconnect.

### Persist frames to survive MV3 worker loss

Rejected because restart recovery is not worth persisting browser pixels. Volatile loss is the
privacy-preserving failure mode.
