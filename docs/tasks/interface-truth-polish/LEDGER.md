# LEDGER -- interface truth polish

One task = one logical commit set. This ledger is the authority on progress.

## RESUME HERE

Both tasks are complete. Remaining optional follow-ups recorded in T1/T2 verdicts: a live
proof of the fill submit-leg fix after the next unpacked-adapter reload, and the option-B
enumeration decision if the owner ever wants unbound-tab visibility.

## Tasks

### T1 -- reply-before-dispatch for the remaining dispatch-tail paths

Status: COMPLETE and proven live (windows-codex, 2026-08-24). After the owner reloaded the
unpacked adapter, a live probe injected a form whose submit handler opens a synchronous
`window.confirm()`, then drove `browser_fill_form` with a verified submit against it: the
call succeeded in 995 ms (the pre-fix shape hung about 8 s and ended unknown), `dialog
respond` accepted the real dialog, and the page handler recorded `sent:true`. The dialog
status race appeared here too -- two immediate status reads truthfully saw no dialog before
the accept landed -- consistent with the bell-path nuance documented above.

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

Status: COMPLETE and proven live on the Windows development graph (windows-codex,
2026-08-24).

What changed:

- `navigation.rs list_tabs` now dispatches `BrowserCommand::ListTabs` through the ordinary
  browser seam (the wire path already existed end to end and was simply unused by this
  tool) and joins the live result against this workspace's bindings on `physical_id`.
  Bound tabs show their CURRENT title/url/active/readiness from the browser, bindings that
  are gone in reality disappear from the answer, and unbound tabs never surface -- listing
  is freshness for the workspace's own inventory, not new visibility into the person's
  other tabs.
- With no browser connected the call now refuses (`browser_startup_manual`, "No browser is
  connected ...") instead of answering from remembered state.
- Model-facing language updated at the language chokepoint: catalog description,
  list-action constant, capability-map sentence, and the outcome summary
  ("Listed N bound tabs.").

Design decision recorded: the owner's "current read of real state" was implemented as
freshness for the controlled inventory (option A). Full enumeration of unbound tabs
(option B) would expand model visibility into the person's unrelated browsing and needs an
explicit product decision of its own; it is deliberately not part of this pass.

Evidence: 330 Rust tests green including two rewritten list tests and a new regression test
(live titles shown, gone bindings dropped); clippy/fmt clean; deployed via dev-loop; live
probe showed the dispatching read end to end -- empty roster refuses honestly, a bound tab
returns its live title/url, and an idled MV3 worker reconnects on the next call while the
refusal covers the gap.

Deviations: none.

## Evidence

- (appended per task)
