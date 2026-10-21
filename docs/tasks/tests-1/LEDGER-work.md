# tests-1: work ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: test-03 (Alpine). Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Opened on 2026-10-21, not started. Read ADR-0204 and BOOTSTRAP.md, then finish arch-1's W-D4 in core-work's ledger, then take `work/` and `workspace/`, starting with `work/tests/execution.rs`, then `governance/`.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `work/` and `workspace/` | 371 | 55 | 371 |
| `governance/` | 136 | 40 | 136 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |

## Notes

None yet.
