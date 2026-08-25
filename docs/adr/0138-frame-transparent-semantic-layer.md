# ADR-0138: Frame-transparent semantic layer

Date: 2026-08-25. Status: Accepted (implemented in this revision).
Builds on ADR-0078 (closed-loop browser core; D8 deferred cross-origin frames), ADR-0050
(policy-free extension boundary), ADR-0133 (capability-restoration seams). Amends ADR-0078
D3's "V1 operates in the top document" limitation and lifts D8's deferral with the owner's
explicit authorization.

## Context

Real forms live inside embedded frames: signpath.org renders its application form through a
HubSpot iframe, and every hosted-form vendor does the same. Ghostlight's content script ran in
the top frame only (`all_frames: false`), so `browser_inspect`, `browser_find`, and
`browser_fill_form` were blind to everything inside a cross-origin frame. A model that could
see the fields in a screenshot had no semantic route to them and fell back to coordinate
guessing. That is the failure this decision removes.

ADR-0078 D8 deferred framed targets because a frame can serve a different origin than the tab
that landed, and it wanted origin-aware governance designed first. The 1.0 governance model
that has since landed makes a narrower answer correct:

- RAWX governs navigation and landing hosts at their own seams. An embedded frame is part of
  the page the person or model already landed on under existing authority.
- The extension stays policy-free (ADR-0050). Frame membership is browser mechanism, not a
  product or policy decision.
- Audit is metadata-only (standing memory rule). It records the governed landing host and the
  action subject, never page content. Adding child-frame origins to audit would widen the
  recorded surface for no enforcement need.

## Decision

1. **The extension observes and acts in every frame.** The content script runs with
   `all_frames: true`. Each frame instance keeps its own element registry. The service worker
   routes every locator-bearing command to the owning frame and aggregates document-wide reads
   across frames in stable frame order.

2. **Locators are frame-scoped at minting, opaque beyond the extension.** A locator carries its
   frame id internally (`<frameId>:<local>`). The bridge and orchestrator already treat
   locators as opaque strings behind `TargetHandle`, so the wire protocol, the tool schemas,
   and every Rust crate stay byte-for-byte unchanged. Old persisted handles fail stale, as
   handles always have.

3. **The model sees one document.** `inspect`, `find`, `query_semantic`, visible-mode `read`,
   text waits, and focused-control discovery merge results across frames ordered by frame id,
   top frame first, under the same bounded ceilings as today. `fill_form` groups fields by
   owning frame, fills them in order, and validates a contained submit within the submit's own
   frame group. Nothing in a result names a frame unless a human reads the wire directly.

4. **DOM-level actions need no coordinates.** Activation, fill, focus, typing by target,
   uploads, and scroll-into-view dispatch inside the owning frame, so they are exact for any
   frame depth. Pointer paths that consume element geometry (`hover` by target, drag between
   two targets, target-scoped screenshots) translate the owning frame's viewport rectangle
   into tab space by walking up the frame tree: each frame's parent reports the content-box
   origin of the embed that shows it, matched by embed URL, and the offsets compose
   recursively. When a parent page hides the embed, renames its target, or offers several
   identical embeds, the command refuses with that named reason instead of clicking the
   wrong pixel. No debugger attachment and no second frame-id vocabulary are involved.

5. **Top-frame semantics stay top-frame.** Article reading, document-tree snapshots,
   JavaScript execution, URL and load-readiness waits, landing governance, and screenshots of
   the whole tab keep their current meaning. Coordinate input from screenshot views already
   crosses frames natively and is unchanged.

6. **Presentation follows the action.** Perpetual visuals (managed-scope border, runtime
   state) render only in the top frame. Target-anchored transient effects render inside the
   owning frame, so the person still sees exactly what the agent touched when that thing lives
   in an embed.

### Rejected alternatives

- **Coordinate-only guidance ("click by screenshot points").** It works today but spends
  turns, breaks on scroll, hides field structure, and is exactly the tax this project refuses
  to leave on models.
- **Frame-origin fields on `ObservedTarget`.** The bridge decodes with `deny_unknown_fields`,
  so an additive field fails closed against older services for zero enforcement benefit.
  Surfacing child origins to models is deferred until a concrete need appears.
- **Origin-aware re-authorization per frame interaction** (the full D8 program): it would put
  a second policy surface on embedded content while navigation and landing already decide what
  may load. Revisit only if a governance need is demonstrated.
- **CDP flattened-document inspection instead of per-frame content scripts:** OOPIF internals
  are not visible to the tab-level session, so coverage would be worse than injection, not
  better.

## Consequences

- Embedded vendor forms become ordinary form fills: inspect once, fill once, submit.
- Every http(s) frame on every page now runs three small scripts; observation cost stays
  bounded by the existing candidate caps.
- Same-origin iframes, srcdoc-free embeds, and cross-origin embeds take one identical path;
  there is no origin branch to maintain.
- Console and network diagnostics remain tab-session scoped and do not see out-of-process
  frames; `browser_diagnose` fidelity inside cross-origin embeds is unchanged (limited), and
  nothing in this decision claims otherwise.

## Acceptance

- Extension unit tests pin the scoped-locator codec, cross-frame aggregation order, grouped
  fill validation, top-frame gating of perpetual presentation, and the refusal shape for
  untranslatable nested geometry.
- The public demo stage `/ghostlight/demo/iframe/` exercises an embedded form end to end:
  inspect lists its fields among ordinary targets, one `fill_form` completes them, submit is
  verified contained, and the completion sentence arrives.
- All existing gates stay green: formatting, warnings-denied Clippy, workspace tests,
  extension suite, JavaScript syntax, process journeys.
