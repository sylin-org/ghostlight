# 0005. Policy-free extension; DOM reads in a content script

- Status: Accepted
- Date: 2026-07

## Context

The extension is untrusted execution and the binary is the trusted governor (SPEC 7.4,
9.2). If any access decision lived in the extension, the audit trail and the governance
overlay would only be as trustworthy as code running inside the browser, code the binary
cannot verify at runtime. The governance overlay must therefore live entirely in the
binary, never in the extension (SPEC 1.1, 2.4; CLAUDE.md, Extension Design Principle).

At the same time, some capabilities are inherently DOM-side: building the accessibility
tree, `find`, `form_input` with shadow-DOM traversal, and `get_page_text` are far more
reliable run in-page than driven remotely over CDP.

## Decision

The extension holds mechanism only: CDP execution, DOM reads via a content script
(accessibility tree with element refs, `find`, shadow-DOM-aware `form_input`,
`get_page_text`), console/network buffering, tab-group lifecycle, keepalive, and
service-worker state recovery (SPEC 2.4; commit 9be07e5). It makes no access, tool
classification, audit, or identity decision. Those are the binary's (SPEC 2.4,
Non-responsibilities). "Policy-free" is the invariant, not "minimal": the content script
may carry real mechanism, but it never governs (SPEC 2.4; CLAUDE.md). Governance-level
redaction (omitting parameters or screenshots from audit) is a manifest-driven binary
concern (SPEC 7.2), not an extension decision.

## Consequences

- The audit trail is as trustworthy as the binary: it records what the binary dispatched
  and what came back, independent of the extension (SPEC 7.4).
- The governance overlay attaches to the binary's dispatch chokepoint without touching
  extension code, so the extension can be replaced or force-installed without re-reviewing
  policy (SPEC 2.4; commit cd9e6d4's no-op policy/audit seams).
- Negative: a tampered extension can bypass domain restrictions by lying about the current
  URL or intercepting CDP responses; the binary cannot verify extension integrity at
  runtime, so full mitigation needs enterprise force-install plus CRX signature checks
  (SPEC 9.2).
- Trade-off: the extension carries more code than a bare CDP relay, accepted because
  in-page DOM reads are more reliable than remote DOM walking.
