# arch-1: adapter ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

A1 has landed. A2 is next, then A3 and A4. ux-2 has closed, so every slice can start.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| A1 | fix(adapter): an embedded document's change is stale, and a targeted read reports the tab | candidate | none | see Notes | landed (this commit) |
| A2 | fix(adapter): a Downloads save carries effect, and diagnostics bind only when asked | candidate | none | - | todo |
| A3 | refactor(adapter): no content command runs without a scope | none | none | - | todo |
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
