# arch-1: core ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

C1 has landed. C2 is next, then C3a. ux-2 has closed, so every slice whose "After" is met can
start.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| C1 | fix(audit): the durable record keeps only what may be retained | candidate | ux-2 closes | see Notes | landed (this commit) |
| C2 | fix(workspace): a tab binding has one lifecycle | candidate | ux-2 closes | - | todo |
| C3a | fix(work): a session's browser is bound by its first tab | candidate | C2 | - | todo |
| C4 | fix(workspace): a remembered tab is pending until its browser confirms it | candidate | C3a | two commits | todo |
| C5 | fix(service): a service end decides its sessions' tabs once | candidate | C4 | - | todo |
| C6 | fix(language): a held tab is described truthfully | candidate | C2 | - | todo |
| C7 | fix(language): a workspace refusal says what is true | candidate | C6 | - | todo |
| C13 | fix(provenance): the opener is named once | candidate | C7 | - | todo |
| C9 | fix(work): a duration wait says what its deadline did | candidate | A1, A2 | - | todo |
| C10a | fix(work): closing and listing a tab look at its host | candidate | C2 | - | todo |
| C10b | fix(work): every authority check of a frame is on the frame's path | candidate | ux-2 closes | - | todo |
| C11 | fix(service): a call during a quit and a token that cannot be read both get an honest answer | candidate | ux-2 closes | - | todo |
| C12a | feat(language): history rows carry one closed outcome class | candidate | ux-2 closes | - | todo |
| C19a | fix(audit): a step that never started carries the policy sequence | candidate | ux-2 closes | - | todo |
| C21 | feat(adapter): the service reads the browser's life | candidate | C4, B1 | lands with B1; cross-lane `protocol4/wire.rs` | todo |
| C3b | fix(browser): a pinned wait reads its own browser | none | C3a | - | todo |
| C3c | refactor(browser): one browser start, recorded in the launch guard | none | C3b | - | todo |
| C12b | refactor(language): each refusal is typed where it is learned, and its code is written once | none | ux-2 closes | two commits | todo |
| C10c | refactor(work): the executor reads one publication | none | P1, P5 | - | todo |
| C16 | refactor(work): which recording a workspace holds is a workspace fact | none | ux-2 closes | cross-lane `protocol4/link.rs`, `workbench/tests.rs` | todo |
| C17 | refactor(language): one wait vocabulary and one coverage predicate | none | ux-2 closes | cross-lane `translate/observe.rs` | todo |
| C18 | refactor(presentation): one table of activity and denial words, with the crossing names pinned | none | C6 | cross-lane `protocol4/golden.rs` and the page assets | todo |
| C19b | feat(audit): the record names the browser | none | I1 | - | todo |
| C19c | refactor(work): the completion path's leftovers go | none | ux-2 closes | - | todo |
| C20 | test(language): the plugin skill names only catalog tools and fields | none | I9b | - | todo |
| C22 | refactor(intake): the "MCP client" placeholder goes | none | C13 | cross-lane `agent/mod.rs`, `workbench/tests.rs` | todo |

## Notes

- C1, the four durable sentences: `Refusal::audit` renders from `retained()`, and four refusals
  keep a durable sentence of their own because their own sentence is made of what history never
  keeps: the browser's words (`BrowserPrimitive`), the fields a fill could not set
  (`FieldValuesRejected`, `FieldsReverted`) and the credential control. These are sentences, not
  strip arms, and they read the retained value. A refused or undone fill keeps its field list in
  the retained value only so its sentence can count it.
- C1, beyond the ADR's list: `retained()` also clears the browser's text from `BrowserPrimitive`
  and the credential control's handle, not only its name. `WorkspaceReason::retained` is its own
  exhaustive match, so C7's new causes decide there what the audit keeps. Every `retained()` match
  names each variant, so a new variant cannot skip the decision.
- C1, 1.3.14: the fill read-backs and the dialog message did not exist in 1.3.14, so that leak is
  2.0-only. The ownership leak is in 1.3.14: an ownership refusal wrote the other workspace's
  client label, or its workspace id when it had none, into the audit summary and into
  `refusal_facts`. D2's correction paragraph in `data-flows.md` applies to it.
- C1, proofs: `no_page_text_reaches_the_audit_through_any_wrapper`,
  `an_ownership_refusal_writes_no_other_clients_name` and
  `the_retained_refusal_drops_the_control_page_address_the_credential_name_and_the_browsers_words`
  in `language/audit.rs`; `a_fill_whose_submit_raises_a_confirm_writes_no_field_value` in
  `work/tests/execution.rs`. No audit golden changed: none of them carries a wrapper or an
  ownership refusal. No journey is needed.
