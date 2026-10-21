# tests-1: service ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: the coordinator, on the owner's machine. Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Opened on 2026-10-21, not started. Read ADR-0204 and BOOTSTRAP.md, then take `service/mod.rs` first, the largest test file in the workspace (133 tests), then the rest of the area, then `crates/relay` and `crates/edge`, then the guard.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `service/`, `intake/`, `browser/` outside `browser/adapter/`, `presentation/`, `audit/`, top-level files, `crates/orchestrator/tests` | 396 | 60 | 396 |
| `crates/relay` | 157 | 35 | 157 |
| `crates/edge` | 66 | 20 | 66 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |
| G | test(guards): the Rust test ceiling, `tests/guards/test-budget.test.mjs` (ADR-0204 Decision 4) | - | - | - | todo, after every area is under its budget |

## Notes

None yet.
