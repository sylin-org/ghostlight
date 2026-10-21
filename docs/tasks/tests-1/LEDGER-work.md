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

- From the coordinator (2026-10-21), out of the batch's reserve: `work/tests/presentation.rs` gains
  `a_presentation_failure_changes_no_result_browser_command_or_audit_line`, the first real test of
  ACCEPTANCE Presentation 3 (the old one in `presentation/mod.rs` only asserted that nothing
  panics). The same read, drawn once and with every signal refused once, gives the same result,
  browser commands and audit line. Keep it in this lane's cut; it counts against the reserve, not
  this lane's budget.
