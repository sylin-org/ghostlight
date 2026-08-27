# D2 -- GIF palette quality

The STATUS "Owed" item: "GIF quality remains deferred. The vendored encoder quantizes each frame
to its own 256-colour palette with no dithering, which suits flat interface pixels and not
photographs. Overlays, action tagging, and perceptual palettes are still unbuilt."

## Current-tree facts (as of authoring, 2026-08-26)

- The extension encodes recordings in the offscreen document with the pinned MIT `gifenc` under
  `extension/vendor/`; thinning, folding, and retention live in `extension/lib/recording.js`
  (ADR-0108/0109: one owner, frames never cross a process boundary, a thinned replay plays for
  as long as the work took).
- Each frame is quantized to its own local palette. Adjacent frames of a photograph, gradient,
  or smooth-shadow page can therefore shift colors frame to frame even when the page is static.
- Output size is no longer the pressure: a browser-local save may spend 16 MiB and thins rather
  than refuses.

## Behavior

Scope: palette stability and honest color for the frames people actually record.

1. Shared palette: derive one palette for a replay from its sampled frames (bounded sample set)
   instead of one palette per frame, so static content stops shifting between frames. Fall back
   to per-frame palettes when no stable shared palette covers a frame within a chosen error
   bound (mixed-content replays stay truthful instead of banding).
2. Optional dithering: apply ordered or error-diffusion dithering when quantizing against the
   shared palette, bounded by the existing byte budget and thinned like any other cost.
3. Prove quality without trusting eyes: tests compare re-encoded output against the source
   frames (per-frame delta against the previous frame, palette count, byte budget adherence,
   thinning behavior unchanged), not just "it produced a GIF".

The exact palette-derivation algorithm, error bound, sample bounds, and dithering choice are
decided during the task and recorded here; the oracle rule applies -- pin expected values from
computed fixtures before implementing.

## Out of scope (separate decisions, not this task)

- Overlays burned into frames and action tagging of the timeline: product-visible features that
  need an owner decision; proposed separately if wanted.
- The recording ownership/lifecycle contract (ADR-0108/0109) does not move.

## Verification

- `npm test --prefix extension` with new focused tests named in the ledger entry.
- A real recording on the dev authority saved and inspected: byte budget respected, playback
  duration equals capture duration (the thinning invariant), and a visual spot check by the
  owner.

## STOP preconditions

- If `extension/lib/recording.js` has moved or its thinning contract changed, STOP and re-author.
- If a palette decision would change the recording contract (frames crossing a boundary again),
  STOP -- that contradicts ADR-0109 and needs an ADR first.
