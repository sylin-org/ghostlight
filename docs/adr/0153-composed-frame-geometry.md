# ADR-0153: Point and target geometry follows the composed page

- Status: Accepted (implemented in this revision)
- Date: 2026-09-04
- Amends: ADR-0138 Decision 6 and ADR-0139 Decision 2
- Builds on: ADR-0078, ADR-0088, ADR-0093, ADR-0131, ADR-0138, and ADR-0139

## Context

Frame-transparent target actions translate a child rectangle through parent-side iframe boxes.
The parent collector used `document.querySelectorAll`, so an iframe hosted inside an open shadow
root had no box and target hover, drag, or screenshot could not compose its coordinates. Point
subjects used `document.elementFromPoint`, which stops at a shadow host and at an iframe element.
The physical CDP effect could still land correctly, but its receipt described a shallower subject.
A coordinate image drop was worse: its synthetic drop event stayed on the parent iframe instead of
reaching the child document.

## Decision

1. Parent-side frame-box discovery searches the document and every reachable open shadow root.
   Existing recursive URL-and-parent-frame matching remains the sole frame identity mechanism.
2. Point hit testing descends through open shadow roots. The service worker then follows the embed
   under the point through the browser's frame tree, subtracting each content-box origin. It keeps
   both the original tab-viewport point for CDP and the deepest frame-local point for DOM effects.
3. If no child or more than one child matches an embed, Ghostlight refuses before a synthetic DOM
   effect rather than guessing. Closed roots remain closed.
4. Coordinate action receipts name the deepest observable subject. Coordinate image drops dispatch
   in that subject's frame. Locator hover, drag, and target screenshot reuse the shadow-aware frame
   box collector.
5. The complete behavior is revision-negotiated: `pointer_input` revision 3 covers composed target
   and point geometry, `capture` revision 2 covers target screenshots, and `files` revision 3
   covers composed coordinate drops. Unaffected viewport and full-page capture remain revision 1.

## Consequences

- A shadow-hosted iframe behaves like an ordinary iframe for target hover, drag, capture, point
  receipts, and drops.
- Physical pointer packets still use browser-native tab coordinates. Only routing and observation
  cross frame-local seams.
- Ambiguous frame ownership is a precise refusal, never an effect on an invented destination.

## Rejected alternatives

### Treat the iframe element as the point subject

Rejected because it misreports what the person acted on and cannot deliver a DOM-local drop.

### Add frame ids to public point or target schemas

Rejected because frame identity is adapter-internal routing state and locators already stay opaque.

### Use CDP DOM inspection for all point resolution

Rejected because content scripts already own page-local observation, and the existing frame
contract can compose the geometry without a second DOM authority.

## Acceptance

- Content tests cover open-root iframe boxes, deep shadow hit testing, and child-frame point
  context.
- Browser-command tests cover the pointer, capture, and file capability revisions.
- The live iframe fixture contains a shadow-hosted iframe and composed semantic content.
- Repository and extension gates remain green.
