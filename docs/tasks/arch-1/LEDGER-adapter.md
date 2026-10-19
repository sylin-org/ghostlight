# arch-1: adapter ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

A1, A2 and A3 have landed. A2's top-frame ring reset waits on a package-lane fix (see Notes). A4
is next. ux-2 has closed, so every slice can start.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| A1 | fix(adapter): an embedded document's change is stale, and a targeted read reports the tab | candidate | none | see Notes | landed (fa3bcb02 and a9d40c83) |
| A2 | fix(adapter): a Downloads save carries effect, and diagnostics bind only when asked | candidate | none | see Notes | landed (a0dfc56b) |
| A3 | refactor(adapter): no content command runs without a scope | none | none | see Notes | landed (this commit) |
| A4 | refactor(adapter): the authority's budgets reach the adapter as values | none | ux-2 closes (`assets.sha256`) | cross-lane work's `DOCUMENT_TREE_NODE_LIMIT` | todo |

## Notes

- A1, cross-lane test: core's `work/tests/protocol4.rs` pinned a click whose input met
  `document_changed` as "Document access could not be verified.". A click names one locator, so
  Decision 5.4 makes it a stale target that navigated. The test is renamed
  `a_document_that_changed_before_input_is_a_stale_target_not_unknown` and asserts that, in this
  commit, so the tree stays green.
- A1, a malformed locator: with the locator branch of `unresolved` gone, a locator that does not
  decode fails the description as `StaleLocator { Unknown }`, the error `Locator::decode` already
  gives, instead of marking the inventory unresolved. Nothing is read in either case.
- A1, scope of the receipt rule: the `document_changed` receipt failure is restated in the Link
  (`link.rs`), as the ADR names it. A world call's own `document_changed` answer
  (`translate::value`) still reads as an unavailable document; the guard on the receipt fires
  first in the common case.
- A1, proofs: `a_target_in_a_reloaded_iframe_is_refused_as_navigated`,
  `an_incomplete_inventory_keeps_document_unavailable`,
  `a_targeted_read_in_an_iframe_reports_the_tabs_url_and_host`, the rewritten
  `describing_documents_binds_locators_points_and_focus_to_current_documents`, and
  `a_document_change_on_a_one_locator_call_is_a_navigated_target`, all in
  `protocol4/translate/tests.rs`. The work-level proof is C9's. No journey is needed.
- A1, review fix: a recording exported into a page names one locator, the file input, and runs
  under the scope guard, so `BrowserCommand::sole_locator` names it. A `document_changed` on that
  export is a navigated target, pinned by
  `a_document_change_during_a_page_export_is_a_navigated_target` in
  `protocol4/translate/tests/points.rs`.
- A2, effect: a Downloads save now carries effect, so runtime control admits it like any other
  effect, and a pause refuses it before anything is sent. Work already reported a delivered
  Downloads save as applied and not repeat-safe, so the two now agree.
- A2, binding: `Scope::watches` names the authority's `watch_changes`, and the doc of
  `DocumentScope.watch_changes` says it also binds a diagnostics read. Work sets it when its
  policy does not admit every document, so no work file changes.
- A2, proofs: `a_downloads_save_whose_receipt_is_lost_reads_as_after_dispatch` in
  `protocol4/link/tests.rs`; `an_unrestricted_diagnostics_read_sends_no_binding` and
  `a_restricted_read_still_binds_its_documents`, which replaces
  `the_diagnostics_binding_follows_the_admitted_set`, in `protocol4/translate/tests.rs`. The
  work-level proof is C9's. No journey is needed.
- A2, review fix, open dependency on the package lane: with no binding sent when the scope does
  not watch, the default all-open mode binds no tab, and the extension's `beforeNavigate`
  (`extension/lib/buffers.js`) resets a ring only when it deletes a binding. So a top-frame
  navigation no longer starts a fresh ring there, and a diagnose after it returns the prior page's
  console and network entries with the new page's. Decision 6.3 keeps the top-frame reset for every
  tab, so this is a defect, not a choice. The fix is the extension's: a top-frame `beforeNavigate`
  starts a fresh ring for any tab with diagnostics capture, bound or not, and deletes the binding
  if there is one. Its proof is an unbound read in `worker.test.js` or `buffers.test.js`: a
  subframe navigation keeps the entries and a top-frame navigation clears them. It changes a
  packaged file, so that commit regenerates the package record (ADR-0200 Decision 16). Recorded in
  LEDGER-package.md; A2's candidate gate holds until it lands.
- A3, `Reach` goes whole, not only `Reach::Open`: with one variant left it only forwarded to the
  scope. `reach()` in `translate/mod.rs` turns the dispatch's optional scope into the scope a
  content command needs, or `BrowserError::Protocol` before anything is sent, and every content
  translator takes `&mut Scope` (the diagnostics and recording translators `&Scope`), not only
  `script::evaluate` and the recordings. `Reach::target` becomes `Scope::target`, which replaces
  the test-only `doc_of` and `doc_or_top`.
- A3, `PlannedCall::unscoped` stays for the calls no scope covers: metadata (a route's hit point,
  a description's focus probe) and a screenshot's target measurement. Work scopes no screenshot,
  so a screenshot is not a content command, and its locator is the agent's own handle. The one
  content-command use, a page export sent without a scope, is gone.
- A3, `EffectClass::Prepare` goes: no rule reads it, because the link counts only `Effect`. Its
  messages are `Read`, whose doc now says it covers positioning a target for an action.
- A3, `Listing.forms` goes, so core's `work/tests/controls.rs` drops its `..Listing::default()`,
  which clippy reads as a needless update. That is the only cross-lane edit.
- A3, tests: eight tests sent content work bare, not six. The fill tests
  `a_submit_outside_the_fields_documents_is_refused_before_anything_is_sent` and
  `malformed_locators_are_plain_failures_before_anything_is_sent`, the three recording-start tests
  in `protocol4/link/tests.rs` (through a `start_recording` helper) and three page-export tests in
  `translate/tests/points.rs` now run inside an all-open scope. Removed as pinning unguarded
  behavior: `a_point_without_a_scope_is_clicked_unguarded`,
  `a_recording_exported_into_a_page_without_a_scope_carries_no_guard`, and the unscoped tails of
  the key-press and script tests in `translate/tests/interaction.rs`. The tests of the deleted
  `FrameTree::ancestors` and `geometry::output_scale` go with them. The proof is
  `a_content_command_without_a_scope_is_a_protocol_error` in `protocol4/translate/tests.rs`. No
  journey is needed.
