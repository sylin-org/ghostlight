# ADR-0195: Bounded settlement and subject-scoped page work

Date: 2026-10-05. Status: Accepted by owner direction; source and installed acceptance passed.

Amends ADR-0158's physical scope validation and ADR-0171/0173/0174's implicit settlement.
Preserves configured document authority, explicit wait conditions, human controls,
exact document identity, cancellation custody and truthful effect receipts.

## Context

The owner reported that ordinary clicks failed on a previously usable SharePoint page.
The October 5 audit records permitted requests followed by missing semantic content,
an uncertain adapter result and a no-effect document-verification refusal. Retained
diagnostics do not establish the exact routing trigger for that incident.

An independent module counterproof established a broader defect: changing an unrelated
active child document prevented a top-document action, including under all-open authority.
Changing that child between native input packets interrupted the action. The adapter
required full inventory equality even when change watching was disabled. Earlier tests
asserted that conservative refusal without proving ordinary targeted work could continue.

The owner chose a simpler contract: settlement makes work complete sooner when the page
is quiet. A still-busy page remains usable after a short default budget. An explicit flag
skips waiting. Apply the same behavior across similar page tools.

## Decision

1. Ordinary read, inspect, find, screenshot, click, scroll, hover, fill, typing, key,
   drag, upload and script tools accept `visual_settle`, default true. One invocation
   shares a maximum 1000 ms preparation budget across semantic lookup and execution.
   False skips visual and local content-readiness settlement. Flow children each use
   their own preference and budget within the parent's original deadline.
2. Work supplies the physical budget through the existing document discovery request. It reserves
   at least half the remaining deadline for execution. The adapter uses the existing
   correlated observation owner and cancellation path, then executes once even when
   the sensor reports unsettled. Routing binds current identities after that wait, while
   the preparation owner fences replacement of the top document. The execution envelope
   carries only the remaining budget. Collectors and form preparation use only that remaining
   budget; expiration returns the latest valid sample instead of rejecting busy content.
3. Targeted work validates its actual documents and their ancestry. Unrelated active
   documents may navigate, appear or disappear. Same-document route changes within the
   admitted origin are harmless. Replacement of the required document, changed ancestry,
   excluded input, unresolved destinations and human controls remain decisive.
4. Broad restricted scripts and complete-page operations retain full-tree
   validation where the configured document promise requires it. Existing recording
   change watching and receipt validation remain. No new document receives
   authority merely because its predecessor or sibling was allowed.
   Screenshots preserve ADR-0194's binary permission and direct capture without document
   inventory. They skip a second implicit wait when semantic target lookup already used
   the invocation's preparation budget. Historical masking evidence remains historical.
5. A composite wait keeps a satisfied primary condition when its optional visual
   settlement expires. Its facts separately report `visual_settled`. The optional
   phase has the same 1000 ms cap. Standalone `visual_settle` and `layout_stable` waits
   retain their explicit condition and caller budget. Cancellation and real access or
   transport failures remain distinct from an unsettled sensor result.
6. Document-scope capability revision 2 negotiates the added physical `strict_tree`
   and `settle_ms` fields. Older adapters refuse before execution. Both connectors
   remain opaque relays; neither learns model tools, settlement or governance.

## Validation

Decoder/catalog coverage checks the common preference on every similar tool. Executor
coverage proves one budget across lookup and action and zero budget on opt-out. Adapter
tests prove unrelated-frame churn, exact subject replacement, shared collector budgets,
busy-page execution, opt-out and cancellation before effects. Whole-tree restricted
checks and existing screenshot permission, human-control and cleanup tests remain required.

## Release integration amendment (2026-10-05)

Latest main had independently assigned ADR-0194 to binary screenshot permission and
published service 1.3.14. This decision was renumbered from its uncommitted 0194 draft
to 0195 before publication. The release preserves that screenshot decision and its
direct capture path. Original installed acceptance used the pre-integration source;
the 1.3.15 release record owns acceptance of the combined candidate.

Installed acceptance uses the existing service, registered native host and ordinary
browser, with an owned page containing a long finite animation and a repeatedly
navigating auxiliary frame. Independent page handlers and retained values must prove
effects, with timings compared against opt-out. That fixture does not by itself prove
the original SharePoint routing failure is fixed.

Two complete installed-browser runs passed every listed tool with and without
settlement. Independent handlers confirmed input effects while animation and child
navigation continued. The verification report records timings, deployed identity,
the first run's separate attention refusal, cleanup and the original-page limitation.
