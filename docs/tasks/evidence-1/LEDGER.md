# LEDGER: blocked-target evidence re-land

Durable progress for the evidence-1 batch. Update before work, after findings, and at each task
boundary. `RESUME HERE` names the only next task.

## RESUME HERE

- State: E1 IN PROGRESS.
- Next task: E1, projection in `crates/orchestrator/src/install/mod.rs`.
- Baseline: `dev` at `4d5ddb95` (docs(tasks): record D4 and close the delight ledger).

## Task table

| Task | State | Commit subject | Evidence |
| --- | --- | --- | --- |
| E1 projection evidence | IN PROGRESS | `feat(install): carry blocked-target evidence` | -- |
| E2 card rendering | READY | `feat(workbench): show blocked-target evidence` | -- |
| E3 live proof + close | READY | `test(integrations): prove blocked-target evidence live` | -- |

## Deviations

None.

## Task log

| Task | Date | Status | Findings |
| --- | --- | --- | --- |
| Planning | 2026-08-24 | COMPLETE | Batch scaffolded from ADR-0129 Decision 4 and the STATUS owed item. Prior art (`5403b339`, reverted by `eb7cf4ed`) supplies the found-command technique; ADR-0135 re-decides the substance after ADR-0130 Decision 4 was superseded. Research confirmed no remnants of the reverted implementation remain in code. |
