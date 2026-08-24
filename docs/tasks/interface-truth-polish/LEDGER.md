# LEDGER -- interface truth polish

One task = one logical commit set. This ledger is the authority on progress.

## RESUME HERE

Next up: T2 (browser_tabs live read) unless T1's commit says otherwise. T1 status is below.

## Tasks

### T1 -- reply-before-dispatch for the remaining dispatch-tail paths

Status: COMPLETE (windows-codex, 2026-08-24). Extension commit; live proof pending the
unpackaged-adapter reload, as BOOTSTRAP requires stating.

Verdicts per content-script branch:

- CONVERTED: `fill` with `submit_locator`. The containment verification ("verified before
  clicking", R3's contract) already precedes the click, so replying `{filled_count,
  submitted}` immediately before `submitElement.click()` preserves semantics exactly. A
  submit handler opening a blocking dialog can no longer swallow the receipt.
- EXEMPT (CDP-response class): hover, drag geometry/observation, viewport points, keyboard.
  These prepare geometry content-side and dispatch through `Input.*` CDP commands whose
  response-stall semantics differ. Documented here rather than braided into this change.
- EXEMPT (event tails): `upload_files` and `clear` end in input/change event dispatches;
  a change handler that blocks is rare and the result values are computed around those
  events, so splitting them adds risk without a demonstrated failure. Revisit on evidence.
- NO DISPATCH: scroll, scroll_point, observe, present, inspect/find/read families.

Evidence: 135 extension tests pass (+3: plain fill receipt, reply-before-submit ordering,
uncontained-submit refusal), plus source-order pins beside the activation ones in
`tests/shared.test.js`.

Deviations: none.

### T2 -- browser_tabs list becomes a current read of real state

Status: NOT STARTED.

Design direction agreed with the owner: enumerate real tabs from the live browser, flag
workspace binding, keep target handles stable, update LANGUAGE.md and the tool description,
decide the no-browser answer explicitly. List becomes a dispatching call on purpose.

Deviations: none yet.

## Evidence

- (appended per task)
