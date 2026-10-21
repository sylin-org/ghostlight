# tests-1: language ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: test-01 (CachyOS). Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

In progress since 2026-10-21. Every test in `language/` and `workbench/` was classified read-only first (keep, rewrite or delete, each with what it protects), then cut one file per commit. The decoder defect the gate-2 control found landed first as arch-1 L-D7 (590a35fc). Next: the rest of `language/` against its budget, then `workbench/`.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `language/` | 349 | 55 | 203 |
| `workbench/` | 252 | 35 | 252 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |
| TL1 | test(language): `self_check/` keeps the 7 tests of the model surface contract | 21 | 7 | 3 promise, 3 contract, 1 defect | landed (f25261cc) |
| TL2 | test(language): provenance.rs keeps the 3 tests of connection reporting | 18 | 3 | 2 promise, 1 defect | landed (9c73d440) |
| TL3 | test(language): workbench.rs keeps the words two surfaces share and the agent count | 26 | 2 | 1 contract, 1 defect | landed (5bccbc51) |
| TL4 | test(language): area.rs keeps the one test of what left_out may carry | 8 | 1 | 1 contract | landed (6184fd15) |
| TL5 | test(language): setup.rs keeps the 9 tests of the install journey | 44 | 9 | 4 promise, 3 contract, 2 defect | landed (f345dc18) |
| TL6 | test(language): deployment.rs keeps the way back and the dry run | 23 | 2 | 1 contract, 1 defect | landed (55b55938) |
| TL7 | test(language): readiness.rs keeps the precedence and what the answer means | 15 | 2 | 2 promise | landed (ceccbf4a) |
| TL8 | test(language): environment.rs keeps one table of the machine row | 7 | 1 | 1 promise | landed (f265c219) |
| TL9 | test(language): channel_switch.rs keeps the policy sentence | 4 | 1 | 1 promise | landed (1a7b3f32) |
| TL10 | test(language): agent_icon.rs keeps the doctor line | 2 | 1 | 1 promise | landed (c3728706) |
| TL11 | test(language): intake.rs, migration.rs and adapter.rs drop tests of their literals | 7 | 0 | - | landed (77a4b37d) |

## Notes

None yet.
