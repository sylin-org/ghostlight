# ADR-0207: Observe without changing the page, and take the browser's word

Date: 2026-11-13. Status: Accepted by owner direction (2026-11-13: "Yes, go ahead" to the plan
below; "Mandate: eliminate fragile structures, concentrate complexity at the correct seams, proper
separation of concerns."; "I'm completely fine with ideating a more resilient and simpler to keep
model."). The refusal words of Decision 6 are decided by the agent under the ADR-0198 delegation.

Restores ADR-0010 for captures (no mutation of the person's tab). Amends ADR-0131 Decision 4 (a
magnified region), ADR-0158 (what an incomplete inventory means and who refuses on it; its
amendment of 2026-11-13 lands with this), ADR-0189 (tree shape is decided in one place) and
ADR-0198 Decision 29 (what a refusal says). Lands with the ADR-0200 amendment of 2026-11-13, which
records the owner's decision to change packaged extension files.

Evidence: [the field session and Chrome probes of 2026-11-13](../testing/office-web-field-session-2026-11-13.md).

## Context

An agent reviewing Office documents in a SharePoint library met a tab that refused every read,
click, scroll and key with "Document access could not be verified", blank and squeezed
screenshots, and a version-mismatch message on a wait. All of it was reproduced, and a fourth
defect was found on a plain page. Each had the same shape: Ghostlight rebuilt something the
browser already had, then trusted its own copy over the browser.

- **A picture.** Every capture asked Chrome for a clip with `captureBeyondViewport`. Chrome then
  re-lays out the page (a 1 x 1 emulated view, then the view resized to the clip). A busy
  cross-process frame is caught squeezed to its CSS minimum size, or blank. ADR-0010 had already
  ruled out changing the person's tab to take a screenshot; the 1.0 rewrite put every capture on
  the clip path without a decision.
- **A list of sessions.** The extension keeps a registry of debugger sessions, emptied only by
  detach events. Chrome 157 sends no detach event for a session attached through another child
  session when that child ends. The dead session stayed registered, its failed read marked the
  tab's inventory incomplete for the life of the attachment, and the service refused every
  document-scoped command on that mark, under every policy.
- **A point.** The executor routed a view point to its document when it described the page,
  before the adapter's view check and scroll into view. A point below the fold of a full-page
  screenshot resolved to nothing and was refused as a document problem.
- **A sentence.** Thirty-three sites raised one refusal with one sentence, and its next step
  crossed the check that had just refused.

Cost was not the problem: a read on a heavy multi-frame page takes 12 to 55 ms. The describe,
admit, act protocol stays.

## Decisions

1. **A screenshot is a photograph.** A capture that lies inside the visible viewport is one
   `Page.captureScreenshot` with no clip, cropped and scaled inside the extension's worker. The
   page is not touched. Only a full-page capture, or a clip that reaches beyond the viewport, asks
   Chrome to render. The plain path is taken only when the picture is the window that was
   measured (no pinch zoom; the image equals the inner size times the device pixel ratio);
   otherwise the old path runs. A recording's still frames follow the same rule. A magnified
   region inside the viewport is an enlargement of the pixels on screen: it no longer gains
   re-rendered detail (ADR-0131 Decision 4, amended).

2. **The session module owns liveness, and Chrome's answer ends a session.** Each child session
   records the session that announced it. One function ends a session and everything announced
   through it. It is called by the detach event, by the end of the announcing session, and when
   Chrome answers a command to a child with "Session with given id not found". Only that probed
   text ends a child. A command waiting on a session that ends settles at once. A child's setup
   error lives with the child. No other module knows what a dead session looks like.

3. **The inventory reports documents, not doubt about bookkeeping.** A session that ended has no
   documents: it contributes nothing and does not mark the inventory incomplete. A child that is
   still registered and fails to answer does mark it incomplete, as before. The root must be
   read, or there is no inventory. The frames receipt keeps its shape.

4. **Tree shape has one owner, and it is total.** The service's frame tree places every frame
   that reaches the top, leaves out the rest and marks the inventory incomplete. Only a list with
   no top document fails. The extension does not prune. A frame whose identity cannot be named is
   left out the same way.

5. **The gate admits documents and does nothing else.** `work/documents.rs` decides which
   documents the policy admits, and keeps two rules for a restricting policy: an incomplete
   inventory refuses, and unbounded work refuses when a document cannot be judged. A handle is
   bound when the page is described, because that needs no page. A point and the keyboard focus
   are bound by the adapter where input is sent, after the view check and the scroll, and are
   checked there against the admitted documents.

6. **A refusal names its cause.** `document_unavailable` keeps its code and carries one cause from
   a closed vocabulary in `language/document_refusal.rs`, set by the site that knows the fact.
   Each cause has one sentence and at most two next steps, and a step is always a call that can
   work. `docs/1.0/LANGUAGE.md` lists them. A document that changed during a call reads as the
   existing `page_changed`, or as a stale target when one handle names it. A spent time budget
   reads as a deadline. When coverage is limited, an absence wait says absence could not be
   established; it is still not satisfied (ADR-0158 Decision 4 stands).
   A whole-page read that no document answered fails with the first document's own failure,
   not with a guess.

7. **All-open does not pay for governance.** Under a policy that admits every document, an
   incomplete or partly placed inventory does not refuse: work runs on the documents shown and
   coverage counts the gap. Under a restricting policy every refusal that existed still happens.

## What a restricting policy sees change

One thing, and it removes a false refusal: a tab that carried a dead session was refused for the
life of the attachment, and now works, because an ended session no longer counts against the
inventory. A session Chrome no longer knows has no documents to judge. Everything else a
restricting policy fails closed on is unchanged.

## Compatibility

No wire change. The published 2.0.0 and 2.0.1 extensions keep the old behavior for the life of
adapter protocol 4, and the service cannot tell builds apart. So the service keeps tolerating an
inventory that is incomplete for the rest of a tab's life, a frame list that is not one tree, and
a refused zero time budget. Those tolerances are not leftovers and must not be removed as cleanup.
Decisions 1 to 3 reach people only through a new extension version, which the owner submits.

## Considered and not taken

- **Frames through `chrome.scripting` and `chrome.webNavigation`, the debugger on the root only.**
  It removes child sessions altogether. It also requires the page runtime to be packaged with the
  extension, so every page-code change would wait for store review (against ADR-0193).
- **Child sessions attached per call.** Page worlds are per session, so the runtime would be
  installed again on every call.
- **A single-phase call that resolves documents at action time.** It needs the policy's host
  rules inside the extension, a second implementation of one policy.
- **Not pausing new frames at start, admitting `about:blank` and `srcdoc` frames with their
  parent, letting an absence wait succeed with a note, sending unrouted input under all-open.**
  Each may be worth doing. Each changes an accepted decision or a trust statement and is the
  owner's to take up separately.

## Tests

`tests/nested-frame-browser-journey.mjs` runs the shipped extension in a real browser on a fixture
shaped like a heavyweight app: a cross-site frame with a CSS minimum size that is slow to grow,
holding another cross-site frame. It fails against the extension as it was. Unit tests model
Chrome's silence with a debugger fake that ends a session without a detach event.
