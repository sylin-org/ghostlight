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

- From the service lane's pilot (2026-10-21): `cli/desktop.rs` has a test near line 739 that
  repeats the service's deleted `show_reveals_the_existing_workbench` through a real host, with
  a 2 s sleep and a deadline under 1 s. Take it in this lane's `cli/` slice: keep it only as a
  condition-waiting test if no kept test proves the reveal.
- From the coordinator (2026-10-21): `crates/desktop/src/link.rs`'s golden test now checks the
  service's longest tray, its bearer and its address with the shell's own `validate()`,
  `well_formed_bearer` and `validate_workbench_url`, against the desktop goldens the service writes.
  Keep that test in this lane's `crates/desktop` cut; `tray.rs`'s `assert_eq!(MAX_TOOLTIP_LEN, 127)`
  only restates the number and can go.
