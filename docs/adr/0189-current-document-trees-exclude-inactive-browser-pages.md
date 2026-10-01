# ADR-0189: Current document trees exclude inactive browser pages

Date: 2026-10-01. Status: Accepted (source correction; installed acceptance pending).
Builds on ADR-0138, ADR-0151, and ADR-0181.

## Context

Installed synthetic browser execution and reading failed before dispatch with
`document_unavailable`. Chrome returned two outermost documents in one tab: active HTTP frame
0 and prerender HTTPS frame 42. The runtime was installed in the active document. The adapter
marked the prerender unsupported but still counted it as a second root and rejected the whole
inventory. The verifier is identical at the trial baseline and both reviewed local deployments.
This establishes the synthetic failure; the same summary on another operation does not prove
the same cause.

Chrome describes several outermost pages per tab, active frame 0, nonzero inactive roots, and
document lifecycle metadata in its [navigation guidance](https://developer.chrome.com/blog/extension-instantnav/)
and [lifecycle contract](https://developer.chrome.com/docs/extensions/reference/api/extensionTypes#type-DocumentLifecycle).
An inactive page is outside the current page's document authority and routing graph.

## Decision

1. Filter prerender, cached, and pending-deletion documents before constructing both the
   verified inventory and raw routing snapshot. Only active documents establish current scope.
   Adding or removing an inactive tree does not change an otherwise identical active scope.
2. Keep the raw inventory bound. Require the closed browser lifecycle vocabulary. Reject
   unknown, null, or mixed missing lifecycle metadata. A wholly legacy snapshot without lifecycle
   metadata retains compatibility through the same strict current-tree validation.
3. Validate one active root at frame 0, unique frame/document identities, connected acyclic
   ancestry, and consistent optional parent-document identity. Keep HTTP/error support checks,
   exact document-targeted dispatch, allowed-document admission, and scope comparison.
4. Locators, point routing, focused input, and coverage use that same current snapshot.
   Inactive documents cannot receive access through a reused frame ID or matching embed URL.
   A true active-document change still refuses; change after possible effects remains uncertain.
5. Change no permission, negotiated capability, protocol status, policy, or model language.
   Preserve refusals and prior failure evidence. Reload only the adapter after independent
   acceptance; this mechanism correction requires no native binary rebuild.

## Validation and limits

Executable adapter tests replay active frame 0 plus inactive frame 42, consecutive scoped
read/fill/read, inactive-tree addition/removal, inactive subtrees and focus, cached locators,
point routing, malformed active graphs, exclusion, and effect uncertainty. Existing guards remain.
Installed acceptance must separately prove consecutive reads and fills in the ordinary browser.
No owner-target page content or private host is needed to reproduce or report this failure.
