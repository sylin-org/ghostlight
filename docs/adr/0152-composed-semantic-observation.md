# ADR-0152: Semantic observation follows the composed page

- Status: Accepted (implemented in this revision)
- Date: 2026-09-04
- Amends: ADR-0138 Decision 5 and ADR-0151 Decision 6
- Builds on: ADR-0078, ADR-0093, ADR-0133, ADR-0138, ADR-0139, and ADR-0151

## Context

ADR-0151 corrected full-page text reading, but sibling semantic tools still described narrower
DOMs. Text waits used `innerText` inside each frame. Find mixed accessible names with light-DOM
text. A custom control whose only visible label lived in its open root could therefore be unnamed.
Document-tree inspection tried to build a semantic node for `ShadowRoot` itself, which is a
`DocumentFragment`, and could fail. A rootless tree also stopped at the top frame even though the
ordinary page is a composed surface.

These are the same semantic boundary, not separate feature exceptions. A person sees one page made
from the top document, open shadow roots, assigned slots, and embedded documents.

## Decision

1. The extension uses one rendered composed-tree model for readable text, fallback accessible
   names, find matching, text waits, and document trees. Open roots replace unslotted light
   children. Slots contribute flattened assigned nodes. Hidden, layoutless, `aria-hidden`, fully
   transparent, and editable content stays absent from visible text. Closed roots remain closed.
2. Document-tree inspection walks composed element children. It never treats a `ShadowRoot` as an
   element. A tree without an explicit target includes injected http(s) frames in stable numeric
   order, top document first, under one global 400-node ceiling. Each child document is appended as
   a subtree without exposing frame identity or origin. An explicit target remains one routed
   subtree in its owning frame.
3. Page-wide text waits retain their existing cross-frame any/all behavior, but each frame matches
   against composed visible text. Find and semantic target descriptions use composed fallback
   names without reading editable values.
4. Every semantic-document command that depends on this common meaning requires
   `semantic_document` revision 4. Text-present and text-absent observation require `observation`
   revision 2. Other observation conditions remain revision 1.
5. This changes no model-facing schemas, authority classes, or audit fields. The extension owns
   page-local observation; the service worker owns frame composition; the orchestrator owns
   bounds, governance, and language.

## Consequences

- Read, inspect, find, and wait now agree about what page content exists.
- A rootless document tree matches the full visible page while retaining one predictable bound.
- Older adapters fail capability negotiation instead of answering a newer semantic command with a
  narrower DOM model. The refusal identifies an outdated extension and asks the user to reload or
  update it.
- Closed roots, editable values, frame origins, and frame identifiers remain undisclosed.

## Rejected alternatives

### Patch each tool independently

Rejected because duplicated shadow and visibility rules would drift again.

### Keep document trees top-frame only

Rejected because that makes the default structure view narrower than the default text view and the
person's visible page.

### Expose frame nodes with URLs or ids

Rejected because the model needs page structure, not browser-internal routing metadata.

## Acceptance

- Content tests cover shadow text waits, composed find names, safe tree traversal, hidden and
  unslotted exclusions, and one frame-local node bound.
- Frame tests cover stable subtree order, depth, and one page-wide node ceiling.
- Capability tests prove all semantic-document commands require revision 4 and only text
  observation requires observation revision 2.
- Outcome tests keep the outdated-extension refusal and recovery instruction exact.
- The process journeys and ordinary repository gates remain green.
