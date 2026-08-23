# LEDGER: language delight pass

Durable progress for the language delight pass. Update before work, after findings, and at each
task boundary. `RESUME HERE` names the only next task.

## RESUME HERE

- State: READY. No task has started.
- Next task: D1, validation messages in `crates/orchestrator/src/language/mod.rs`.
- Baseline: `dev` at `dcabf582` (fix(language): teach through invalid-input guidance).
- Already landed in the baseline: decode-failure next-steps carry the specific validation detail;
  the four "view is required" sites teach the screenshot-first remedy.
- Known live-verification debt: the swapped authority has not yet been re-probed for these two
  changes; D4 owns that check.

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
