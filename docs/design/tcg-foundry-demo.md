# Sylin Card Foundry demo

Status: Implemented demo design. Rebuilt for 1.0 as
[`scripts/demo-foundry.ps1`](../../scripts/demo-foundry.ps1) and
[`scripts/demo-foundry.sh`](../../scripts/demo-foundry.sh) rather than the `ghostlight demo`
subcommand it shipped as on 0.8; the command line reaches the same catalog through the same
governance. All seven beats run, with both scripts verified live.

One bound changed underneath the story, and then changed again. The 0.8 rehearsals below exported
100 frames as 21.5 MB and 23.1 MB GIFs, which 1.0 cannot carry: recordings retain at most 5 MB of
frames and the browser transfer ceiling is 8 MB. Rather than raise those, both bounds now trade
fidelity instead of failing -- the extension thins retained frames at its byte bound, and the
encoder thins again to fit its output bound. The whole story is recorded and the replay says what
it gave up: "17 of 65 frames".

## Purpose

The demo tells one complete, simulated story instead of walking through unrelated browser tools.
A foil trading-card proof fails QA. Ghostlight inspects the defect, records a rejection, requests a
revision, attaches evidence, completes the release packet, and then demonstrates that promotion to
production remains outside the granted boundary.

The page says that it is a simulated workspace. It uses no account, personal data, or remote
application state.

## Story beats

1. Open the Sylin Card Foundry and start a memory-only recording lease.
2. Inspect the full workspace, hover the foil treatment, rotate the card to its Sylin-stamped back,
   and zoom the defect.
3. Check the failed QA criteria, type the rejection reason, and drag the defect ticket to Request
   revision.
4. Observe the page's local console and same-origin request, then wait for Revision B.
5. Capture the corrected proof, attach the screenshot, check final QA, and fill the release packet.
6. Attempt an off-domain navigation. The real session policy refuses it in plain language.
7. Export the animated replay into the page, verify that the page received it, and clear the
   service-side recording bytes.

These are story phases, not a promise to invoke every tool. Each interaction must advance the card
release or demonstrate one product guarantee.

## Composition

The intended recording area is a 1280 x 720 page viewport. The stable shell has four regions:

- a compact header for collection and simulation context;
- a left card stage that stays visually anchored while the proof rotates in 3D;
- a central workbench that changes from defect review to the Revision B release packet;
- a right evidence rail for readiness, screenshot proof, replay proof, and the governed boundary;
- a shallow footer rail that makes the story's current phase legible.

The card is the visual hero. Its front carries the Ghostlight mascot, foil sweeps, print marks, and
the visible registration defect. Its back uses a quiet Sylin stamp so the rotation has a designed
destination instead of an empty reverse face. Motion exists to communicate state: card rotation,
cursor ripples, the revision transition, upload previews, and phase changes.

The layout collapses for narrow screens without horizontal overflow, but the recorded composition
is intentionally desktop-first.

## User and model contract

The user should see a coherent job with visible cause and effect. The model should have stable,
semantic controls and observable completion states.

- Controls have explicit accessible names. The runner locates them by meaning, not DOM position.
- The runner reads interactive controls once per stable page phase and reuses their references.
  The read-scan animation therefore appears only when Ghostlight is actually inspecting Revision A
  or Revision B; clicks, typing, and screenshots retain their own distinct visual treatments.
- Screenshots establish the current viewport before coordinate interactions.
- Drag coordinates are derived from the live page and transformed with the same canonical geometry
  constants used by the extension. Resizing the browser does not invalidate the script.
- Machine-shaped JavaScript results retain Ghostlight's model-facing provenance boundary on the
  wire. Before parsing geometry, the runner validates the structured page-sourced marker, origin,
  and matching session nonce in both control markers, then unwraps only that verified outer layer.
  It accepts raw values only after `tools/list` explicitly advertises a pre-ADR-0078 tool contract;
  a capable or unnegotiated service that omits provenance fails closed.
- Long transitions expose text states such as `Revision B ready` and `Replay ready`; the runner
  waits for those outcomes instead of relying on optimistic sleeps.
- A failed policy assertion terminates the run. The demo cannot call itself complete if the
  off-domain action succeeds.

## Privacy lifecycle

The page and runner are designed so demo data has an explicit, short lifetime.

- The page uses fixed fictional values and has no authentication.
- Revision data comes from a same-origin static JSON file. The page makes no third-party request.
- Screenshot and GIF inputs use in-memory browser `File` objects and object URLs. Object URLs are
  revoked when replaced and when the page unloads.
- Ghostlight recording stays memory-only and bounded by ADR-0073. The runner clears it after the
  page confirms receipt. Session end, policy change, panic, lease loss, and retention cleanup are
  independent erasure paths.
- Tool payloads and captured page bytes are not written to debug logs.

The exported GIF exists only where the user explicitly places it. The service does not keep a
second hidden copy after clear.

## Reliability bounds

Recording has two deadlines: a 30-second idle lease and a 120-second hard lifetime. Ordinary
recordable browser activity refreshes the idle lease but never extends the hard lifetime. Export
auto-finalizes the recording. Large GIF delivery uses bounded, negotiated, hash-verified,
memory-only chunks.

The scripted story must remain comfortably inside the hard deadline, including deliberate pacing.
A final normal-paced local rehearsal on 2026-07-13 completed inside the hard lifetime, exported 100
frames as a 21,466,581-byte GIF, verified `Replay ready` in the page, cleared the recording, and
observed the real off-domain denial. The enclosing build-and-run command took 113.3 seconds,
including a 3.98-second build and work before recording began. A compressed rehearsal of the same
two-scan flow also passed. On 2026-07-15, after adding ADR-0078 boundary-aware machine parsing, a
second normal-paced visible run completed the same story, exported 100 frames as a 23,141,963-byte
GIF, verified page receipt, cleared the recording, and observed the off-domain denial. Any future
story expansion must repeat the normal-paced end-to-end check rather than assuming that individual
tool calls imply a reliable demo.

## Acceptance checks

- The generated site validates and the Foundry route has no broken internal references.
- The page fits 1280 x 720 and a 390-pixel-wide viewport without horizontal overflow.
- The Rust demo parser and geometry helpers pass their unit tests, including verified boundary
  unwrapping and refusal of missing or mismatched provenance.
- The full Rust workspace and strict clippy gates pass in an isolated target directory.
- A live run reaches `Replay ready`, clears recording state, and proves the policy denial.
- No trained tool-schema field or extension policy boundary changes for this presentation layer.
