# tests-1: adapter ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: test-02 (Bluefin). Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Opened on 2026-10-21, not started. Read ADR-0204 and BOOTSTRAP.md, then take `browser/adapter/` module by module. The protocol 4 goldens and the pins in `protocol4/golden.rs` are contracts both sides read; keep them.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `browser/adapter/` | 337 | 55 | 337 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |

## Notes

None yet.
