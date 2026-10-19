# arch-1: adapter ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

Opened by D0. Nothing has started. ux-2 has closed, so every slice can start in the ADR's order:
A1, A2, A3, then A4.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| A1 | fix(adapter): an embedded document's change is stale, and a targeted read reports the tab | candidate | none | - | todo |
| A2 | fix(adapter): a Downloads save carries effect, and diagnostics bind only when asked | candidate | none | - | todo |
| A3 | refactor(adapter): no content command runs without a scope | none | none | - | todo |
| A4 | refactor(adapter): the authority's budgets reach the adapter as values | none | ux-2 closes (`assets.sha256`) | cross-lane work's `DOCUMENT_TREE_NODE_LIMIT` | todo |
