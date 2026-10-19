# arch-1: docs ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

D0's first commit landed the ADR, its README row, the pointer lines and the batch files. Next: D0's
second commit after ux-2 closes (the row in `docs/tasks/README.md`, and STATUS says the batch
started), then D1. D2 follows each slice that lands.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| D0 | docs(adr): one owner for each fact | candidate | none (commit 2: ux-2 closes) | - | part landed: commit 1 (this commit); commit 2 after ux-2 closes |
| D1 | docs(release): the release candidate names the plugin, and LANGUAGE names where a selector works | candidate | D0; ux-2 closes (`docs/1.0/LANGUAGE.md`) | - | todo |
| D2 | docs(contracts): the contracts state what landed | as the code it follows | each landed slice; ux-2 closes (`docs/1.0/`) | - | todo |
