# LEDGER: language delight pass

Durable progress for the language delight pass. Update before work, after findings, and at each
task boundary. `RESUME HERE` names the only next task.

## RESUME HERE

- State: D1 IN PROGRESS.
- Next task: D1, validation messages in `crates/orchestrator/src/language/mod.rs`.
- Baseline: `dev` at `6944507a` (docs(status): full catalog proven live).
- Already landed before the batch: decode-failure next-steps carry the specific validation detail;
  the four region-view sites teach the screenshot-first remedy; dialog attempt-first handling;
  dialog-aware EffectUnknown guidance. D1-D3 must not regress those.

## Task table

| Task | State | Commit subject | Evidence |
| --- | --- | --- | --- |
| D1 validation messages | READY | `feat(language): teach through validation messages` | -- |
| D2 tool descriptions | READY | `feat(language): delight tool descriptions` | -- |
| D3 result guidance | READY | `feat(language): delight result guidance` | -- |
| D4 live proof + close | READY | `test(language): prove delighted guidance live` | -- |

## Deviations

None.

## Task log

| Task | Date | Status | Findings |
| --- | --- | --- | --- |
| Planning | 2026-08-23 | COMPLETE | Batch scaffolded from the R8 live-lane finding: circular "Correct the call" guidance. Baseline fix already committed (`dcabf582`). |
