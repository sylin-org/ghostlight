# arch-1: package ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

Opened by D0. Nothing has started. Both slices start after ux-2 closes. B1 lands in the same
candidate as C21, or neither does.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| B2 | fix(extension): custody follows the service that granted it, and an outdated extension says so | candidate | ux-2 closes | package items 2, 3, 4 and 7 | todo |
| B1 | feat(extension): the browser names its life | candidate | ux-2 closes | package item 1; lands with C21 | todo |
