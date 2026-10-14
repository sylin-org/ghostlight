# Checkpoint live acceptance -- 2026-10-14

Deployed: dev at f28bcbd7 (step 1 + ADR-0197 + M2), orchestrator only, via scripts/dev-loop.ps1.
Installed adapter: the owner's unpacked dev extension (protocol 3), unchanged. Connectors unchanged.
Driven through this session's Ghostlight MCP connector, in one Ghostlight-owned tab.

| Check | Result |
| --- | --- |
| Catalog refresh on deploy | browser_workspace dropped from the live tool list without reconnecting |
| browser_tabs list (empty) | "Listed 0 tabs in this session." next: browser_navigate with url |
| browser_diagnose {} with no tab | "This session has no open tab yet." (was "That tab is no longer open.") |
| policy_explain | "Explained what agents may do: 4 capability areas, no policy layers." |
| form/: inspect, fill 5 fields incl. select by visible label, submit | pass; "Form received"; read shows "checked Governance", values retained (LU-2, EXT-7/HM-7) |
| reading/: inspect scope document | tree returned; "Returned the document tree: 14 nodes." (CAP-1) |
| desk/: find plain div text | "drag across me" found (LU-6) |
| desk/: dialog status, respond | hint names action accept/dismiss/respond; prompt answered; page echoed it (LU-5 hint) |
| desk/: drag by viewport points | pass |
| signals/: console + network diagnose, slow task wait | pass |
| iframe/: inspect across 2 documents, fill 5 fields, submit | pass; "Application received" |
| shadow/: inspect open root, fill, submit | pass; "Tally received"; closed root honestly empty |
| local fixture: browser_read masking | card, CVV, OTP, API token masked; nickname shown; placeholder not a value; checkbox/radio state |
| close own tab with preserve-tabs on | calm refusal and next step |

Known, still present (protocol-4 or switch work):
- LU-1: text_present on a page with an iframe took 4250 ms for text already present; summary says "appeared in 4 seconds".
- Drag points from a target screenshot taken before the page scrolled: refused as "Document access could not be verified" instead of a stale-view message.
- Drag summary reads "Dragged the X control onto the X control" when source and destination are the same element.
- find reports role "div" for plain containers instead of an accessibility role.
- Masked credential values render as 64 asterisks each; a short marker would save tokens.
- Handles are still 39-character UUIDs (D21, switch-14).
- The document-scope tree labels are long text dumps of containers.
- The click that opened a dialog does not say so (D21 / LU-5 remainder, switch-14).

## Owner session (2026-10-14, morning)

The owner opened https://sylin.org/ghostlight/demo/form/ in an ordinary tab of his own. A fresh
MCP session (zero tabs after the overnight Windows Update restart) then called browser_navigate
for the same URL without new_tab. The result was created:true, reused:false: Ghostlight opened its
own tab in its group and left the owner's tab alone (CAP-2, ADR-0196 Decision 9). The public
1.3.14 build would have adopted that tab.

In the Ghostlight tab: one browser_fill_form by semantic selectors (six fields, dropdown by visible
label) and one click with an expect check completed the form with no inspect step. On the desk
stage a five-step browser_flow stopped cleanly at a missing reference (three steps kept, no
replay), then typing, the bell and the prompt answer worked.

Owner observations, verbatim answers to three questions:
- His own tab was left exactly as he had it: "Yes."
- Cursor, ripples, group border, and focus: "Yup, working as expected."
- Surprises: "No, it was very communicative and unsurprising, the best kind."

New finding from the session: semantic names should fall back to a field's placeholder when it
has no label, as browsers do. Queued in the 2.0 switch lane.
