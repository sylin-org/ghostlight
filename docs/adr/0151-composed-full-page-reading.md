# ADR-0151: Composed full-page reading is the default

- Status: Accepted (implemented in this revision)
- Date: 2026-09-04
- Amends: ADR-0133 Decision 5 and ADR-0138 Decisions 3 and 5
- Builds on: ADR-0050, ADR-0078, ADR-0093, ADR-0133, ADR-0138, and ADR-0139

## Context

The shortest `browser_read` call did not perform the page read its language promised. The
language decoder left `mode` absent, and the executor routed that absence through the older
top-frame `read_text` primitive. Only an explicitly supplied `visible` mode reached the
cross-frame `read_document` path. The extension then implemented visible text with
`document.body.innerText`, which stops at open shadow-root boundaries. The result could omit form
labels, instructions, validation messages, and other visible content in both embedded documents
and web components even though Ghostlight's inspect and action paths could reach those controls.

ADR-0133 made article extraction the default. That choice optimized a content-reader heuristic,
not the ordinary meaning of asking Ghostlight to read a page. ADR-0138 later made explicit visible
reads frame-transparent but deliberately kept article reading top-frame only. Together those
decisions left the shortest user call narrower than the page the person could see.

The page is a composed browser surface, not just one light DOM. A complete default read must cover
the visible top document, open shadow trees, and injected http(s) frames without piercing closed
roots, disclosing editable values, or widening governance and audit data.

## Decision

1. **The shortest call means the full visible page.** `browser_read {}` selects `visible` mode and
   dispatches the composed-document primitive. `visible` remains an accepted explicit value.
   `article` remains available only when the caller asks for it.
2. **A frame-local visible read follows the rendered composed tree.** The content adapter walks
   visible text nodes through open shadow roots. A shadow root replaces its host's light children,
   and slots contribute their flattened assigned nodes. Hidden, layoutless, `aria-hidden`, and
   editable content is excluded. Form control values are never inferred as visible prose. Closed
   shadow roots remain closed and honestly absent.
3. **A page read composes frames under one budget.** The extension service worker reads injected
   http(s) frames in stable numeric frame order, top frame first, and joins nonempty sections with
   a blank line. `max_chars` is one ceiling for the complete page, not a separate allowance per
   frame. Local truncation, a later nonempty frame beyond the remaining budget, or skipped frames
   after exhaustion makes the result `truncated: true`. A navigating or absent frame contributes
   no text rather than failing the complete page read.
4. **Article mode is explicit and has a full-page fallback.** It probes useful article candidates
   in the top document, including candidates inside open shadow roots. A useful candidate returns
   without reading child frames. If none exists, the same composed visible page read used by the
   default path runs across frames. Top-document title and URL remain the page identity through
   that fallback.
5. **Target reads use the same frame-local composed traversal.** A target read covers the visible
   composed subtree rooted at that target. Target routing and handle semantics do not change.
6. **The stronger meaning is revision-negotiated.** `read_document` requires
   `semantic_document` revision 4. Revision 3 retains its historical article and document-tree
   meaning. A new service refuses an older adapter before dispatch instead of silently returning
   an incomplete page. The browser connector remains an opaque relay, and the command schema does
   not change.
7. **No new authority or disclosure surface is created.** The extension still makes only
   page-local observation decisions. Embedded-frame origins remain absent from results and audit.
   The orchestrator still owns the default, bounds, governance, completion sentence, and facts.

## Consequences

- The shortest read matches what a person means by the page, including ordinary web-component and
  embedded-document content.
- Article extraction remains useful for callers that deliberately want it without narrowing the
  default behavior.
- The character budget is predictable across the complete result. Earlier frames win their share
  deterministically, and truncation truth survives both local and aggregate limits.
- Read semantics now align with the existing shadow-complete and frame-transparent interaction
  paths while preserving the boundary around closed roots and editable values.
- The extension capability revision advances even though the wire shape does not. Existing
  revision-3 adapters fail precisely until updated.

## Rejected alternatives

### Keep article-first as the default

Rejected because it treats an extraction heuristic as the ordinary page. It can omit visible
navigation, surrounding instructions, web-component content, and embedded application state that
the person reasonably expects a page read to include.

### Fix only the absent-mode routing

Rejected because explicit visible mode would still stop at every shadow boundary. Routing through
the wider primitive is necessary but not sufficient.

### Concatenate each frame's independently bounded result

Rejected because N frames would receive N times the documented disclosure budget before a final
slice. One page request has one output ceiling.

### Use `innerText` plus special cases for known components

Rejected because the failure is the composed-tree boundary itself. Component-specific selectors
would be incomplete, fragile, and page-specific.

### Pierce closed shadow roots or include editable values

Rejected because closed roots are an explicit browser boundary and editable values may contain
credentials or other user data. Visible page reading does not expand Ghostlight's disclosure
authority.

## Acceptance

- Decoder and executor tests prove that an omitted mode dispatches `read_document` with `visible`,
  while explicit `article` remains distinct and invalid mode values are rejected by the closed
  type.
- Content tests prove nested open-root and assigned-slot text, hidden and editable exclusions,
  exact character ceilings, shadow-contained article candidates, and article absence.
- Frame tests prove stable order, one global budget, truthful truncation, explicit article
  short-circuit, and fallback with top-document identity retained.
- Capability tests prove `read_document` requires `semantic_document` revision 4 while document
  trees retain revision 3.
- The process and PowerShell journeys exercise document reading through the real executable
  boundaries, and all ordinary repository gates remain green.
