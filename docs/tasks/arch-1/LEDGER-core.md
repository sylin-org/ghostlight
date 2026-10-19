# arch-1: core ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

Opened by D0. Nothing has started. Core's slices touch files ux-2 touched, so the lane starts after
ux-2 closes, with C1 and C2.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| C1 | fix(audit): the durable record keeps only what may be retained | candidate | ux-2 closes | - | todo |
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
