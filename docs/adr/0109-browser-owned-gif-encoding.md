# ADR-0109: The browser encodes the GIF; frames never cross

Date: 2026-08-12
Status: Accepted
Supersedes: ADR-0053 Decision 1 (GIF encode placement)
Amends: ADR-0108 Decision 2 (the `read` command that returns frames)

## Context

ADR-0108 made the extension the sole owner of recording lifecycle: identity, frames, deadlines,
retention, and erase. It left one thing behind. Its `read` command returns bounded base64 JPEG
frames to the orchestrator, which decodes them and encodes an animated GIF in `gif_output.rs`.

So a replay of the page still leaves the browser as frames, is re-encoded in Rust, and for a
page-attached save crosses back to the same browser it came from. Nothing about that round trip
serves the user, and it is the source of several problems that looked unrelated:

- A transfer ceiling of 8 MB bounds what can cross, while retained frames may reach 5 MB and inflate
  roughly threefold when encoded. The last stretch of a legal recording could never be delivered.
- Fidelity had to be traded twice, once in `recording.js` at the byte bound and once in
  `gif_output.rs` at the output bound: one policy, two implementations, two languages, either side
  of a process boundary. The Rust one shipped a bug the JavaScript one did not have, dropping each
  discarded frame's duration so a thinned replay played back faster than the work it recorded.
- Page pixels transit the service process for an artifact that begins and ends in the browser,
  weakening a privacy claim for no functional gain.

ADR-0053 moved the encoder out of the extension in the first place, on the rule that the extension
should carry "the least responsibilities". That is a different rule from the one this project
actually holds.

**Thin means nothing bleeds through the extension. It does not mean the extension does little.**
Policy, authority, workspace, journey, and model-facing language must never live there. A capability
that is physically the browser's belongs at the browser layer, because that is the only layer that
has it. Recording and encoding pixels is such a capability. Deciding whether recording is permitted,
what it may be attached to, and what the result is called is not, and stays where it is.

## Decision

### 1. The orchestrator governs; the browser records

The orchestrator decides whether recording may take place at all, and then signals. It holds no
frames, no encoder, and no second copy of the recording state machine. Its vocabulary stays what
ADR-0108 gave it: start, status, stop, save, discard.

### 2. The extension owns the whole capability, including the encode

The extension already owns plural recording state, capture, bounds, deadlines, retention, and
erase. It now also owns encoding. It bounds its own recordings and may stop one autonomously when
its own limits are reached, or on the orchestrator's stop.

It keeps a finished GIF ready to hand over as a single operation. Frames never cross the boundary.
`gif_output.rs`, the frame-returning `read` command, and `PhysicalRecordingFrame` on the wire are
removed.

### 3. Three destinations, and only one of them crosses

- **To the page**: the extension attaches the GIF to a page target itself. Nothing crosses.
- **To a file**: the extension writes it through the browser's own download mechanism. Nothing
  crosses. The browser chooses where downloads land; the orchestrator does not name a path.
- **To the client**: the finished GIF crosses once, as one artifact, for an MCP client that asked
  for it. This is the only path where bytes leave the browser, and it is the only one where the
  caller is outside it.

### 4. Fidelity is one implementation, in the browser

The orchestrator may state a budget. The extension meets it. Thinning exists once, where the frames
are.

Two rules travel with it, both learned the hard way. **Trade fidelity, never coverage**: a bounded
recorder that stops at its limit produces a replay that silently omits everything after, which is
worse than a coarser replay of the whole span. And **whoever drops a frame folds its time into the
frame before it**, or the replay plays back faster than the work it recorded and quietly misreports
how long anything took.

### 5. The result is described in human terms

A replay's model-facing sentence reports how long it plays, not how it was made:
"Recorded 30 seconds of page changes." Frame counts, captured counts, and encoded size are
mechanism; they stay in the facts for anything that needs them, and out of the sentence. Nobody
watching a replay wants to know it is 17 of 65 frames.

### 6. Reclaim or lose it

A finished GIF is held briefly for collection and then flushed. The extension owns that deadline as
it owns the others. A save requested after the flush is a decisive, truthful refusal, not a silent
empty result.

## Consequences

- The published extension grows by an encoder. That is the cost ADR-0053's ship gate existed to
  avoid, and it is accepted here: one owner for a browser capability is worth more than a smaller
  artifact.
- Encoding cannot run in an MV3 service worker that Chrome may evict mid-encode. It needs the
  offscreen-document route, and that is an implementation constraint rather than an open question.
- Writing a file needs the `downloads` permission, which the manifest does not currently request.
  Permissions are a published surface and a store-review concern, so this is a deliberate addition,
  not an incidental one.
- The 8 MB transfer ceiling stops binding the common case. It applies only to a client-return save.
- A vendored JavaScript encoder is supply chain in the most scrutinised component and must be
  pinned, reviewed, and licensed like any other dependency.

## Alternatives considered

- **Keep the encoder in Rust and raise the bounds.** Rejected. Retained frames inflate about
  threefold when encoded, so the retention bound and the transfer ceiling already contradicted each
  other; raising a number moves the contradiction rather than removing it, and leaves two thinning
  implementations in place.
- **Keep the encoder in Rust and negotiate a budget across the boundary.** Rejected. It keeps the
  round trip and the second implementation, and buys only a better failure message.
- **Encode in the MV3 service worker.** Rejected. Eviction mid-encode is a real failure mode; the
  offscreen document exists for exactly this.
