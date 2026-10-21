# tests-1: install ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: leo-desktop-02. Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Opened on 2026-10-21, not started. Read ADR-0204 and BOOTSTRAP.md, then take `install/` module by module, starting with `install/mod.rs` and `install/deployment.rs`. The installer writes golden (`tests/golden/agents/`) is a contract; keep `writer_golden`. `crates/win-peer` is FFI and keeps its tests that cover unsafe calls.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `install/`, `cli/`, `agent/`, `crates/bridge`, `crates/win-peer` | 626 | 75 | 626 |
| `crates/desktop` | 161 | 20 | 161 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |

## Notes

None yet.
