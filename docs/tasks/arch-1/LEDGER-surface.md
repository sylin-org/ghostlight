# arch-1: surface ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

Opened by D0. Nothing has started. The lane starts after ux-2 closes; S1 waits for P10 and S2 for
C12a.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| S1 | fix(tray): every surface offers the acts the runtime state accepts | candidate | P10 | - | todo |
| S2 | fix(workbench): history rows and counts say what the service decided | candidate | C12a | - | todo |
| S4a | fix(workbench): the Policy page previews and names what governance decides | candidate | P3, P4 | - | todo |
| S8 | fix(window): a window that cannot be built says so | none | ux-2 closes | - | todo |
| S6 | fix(setup): setup starts browsers the way recovery does, and marks the right host in use | none | C3c, C13 | - | todo |
| S3 | refactor(workbench): every part comes from one gather | none | P5, I6, C2 | - | todo |
| S4b | refactor(workbench): the Policy page's words come from governance | none | P7, P8 | - | todo |
| S5 | refactor(workbench): the window's words come from language/ and are pinned | none | C18, C12b | - | todo |
| S7 | test(desktop): the shell decodes and re-encodes every desktop golden | none | ux-2 closes | - | todo |
