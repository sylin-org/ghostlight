# arch-1: surface ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

S1 and S2 landed. Next: S8 and S7, which can start now; S4a waits for P3 and P4.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| S1 | fix(tray): every surface offers the acts the runtime state accepts | candidate | P10 | - | landed (37554eab) |
| S2 | fix(workbench): history rows and counts say what the service decided | candidate | C12a | see Notes | landed (this commit) |
| S4a | fix(workbench): the Policy page previews and names what governance decides | candidate | P3, P4 | - | todo |
| S8 | fix(window): a window that cannot be built says so | none | ux-2 closes | - | todo |
| S6 | fix(setup): setup starts browsers the way recovery does, and marks the right host in use | none | C3c, C13 | - | todo |
| S3 | refactor(workbench): every part comes from one gather | none | P5, I6, C2 | - | todo |
| S4b | refactor(workbench): the Policy page's words come from governance | none | P7, P8 | - | todo |
| S5 | refactor(workbench): the window's words come from language/ and are pinned | none | C18, C12b | - | todo |
| S7 | test(desktop): the shell decodes and re-encodes every desktop golden | none | ux-2 closes | - | todo |

## Notes

- S1: ux-2's 77840ccc had already relabeled the tray's Resume entry "Start a new session" for an
  ended session, with the StartSession intent, and corrected the doc the ADR names at
  `intake/desktop.rs:383-384`. So the entry stays enabled once the session ended: what it asks for
  is offered. `TrayModel::of` now enables a runtime entry exactly when `governance::offers` names
  its act, so Resume is enabled only while paused. The test the ADR names replaces
  `an_ended_tray_offers_a_new_session_but_not_pause`.
- S1: the controls part carries `runtime.offers`. Settings shows a runtime button when its act is
  offered and the context may do it; the band's wheel does the first offered act. Before any
  controls part has arrived, the band shows neither the wheel nor the Continue line.
- S1, cross-lane: the fake controls parts in core's `tests/workbench-surface.mjs` and
  `tests/workbench-preview-server.mjs` gain `offers`, and the status-push check in
  `workbench-surface.mjs` also pushes the controls part, as the service does. Nothing else in
  those files changes.
- S2, the rows: `HistoryItem.outcome` is `OutcomeClass::recorded` of the record, and a flow's
  placeholder parent is `unknown`. The page keeps the class words as `OUTCOME` in `words.js` and
  draws the row, the running card, the screen reader's word and a flow's step labels from it.
  `isBlocked` reads the `refused` class and `isFailed` the `failed` and `unknown` ones; the entry
  no longer carries `status` or `allowed`, which nothing drew from once the class arrived. The
  refused class keeps the word "Blocked"; a credential refusal reads "Needs you" in the quiet
  look a kept tab has. A flow held back by a refused step, which the page used to call Kept, is
  now Blocked.
- S2, search (beyond the ADR's list, same finding V-05): `HitOutcome` re-derived the outcome a
  third time and also called a credential refusal Kept. It is deleted, and `SearchHit.outcome` is
  the row's class. `language::workbench::search_outcome` gives the word each class is found by,
  the row's word in lower case ("blocked", "kept", "needs you", "failed", "unknown"), and
  `surface_tests.rs` pins `OUTCOME` and `OUTCOME_WORD` to `OutcomeClass` and to it.
- S2, the counts: the connections part carries `counts`, the `ConnectionCounts` the tray and
  `status` already say, and the band and About read them. The ADR's "count connections, not
  sessions" is read as these service counts: agents stay counted one per agent id, as At a glance
  groups them (ux-2's 7a5065d3, ADR-0199 amendment of 2026-10-18). At a glance keeps grouping the
  sessions it draws. `OverviewSummary` and the page's `overview` stay for S3.
- S2, proofs: `a_credential_refusal_row_says_needs_you`, `a_kept_tab_row_says_kept`,
  `the_connections_part_carries_the_counts_the_tray_says` and the reworked search test in
  `workbench/tests.rs`; `the_page_draws_every_outcome_class_and_is_found_by_its_word` in
  `surface_tests.rs`; the page tests "a credential refusal row says needs you, in ink, and is never
  drawn as kept or blocked" and "the band and About count what the tray counts", which replaces
  "the band, the pill and About count one thing".
- S2, cross-lane: core's `tests/workbench-surface.mjs` and `tests/workbench-preview-server.mjs`
  fakes now carry `counts` on the connections part and `outcome` on their records, as the service
  does. Nothing else in those files changes. No golden changed, and no journey needs a test
  machine.
