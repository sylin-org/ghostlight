# LEDGER: language delight pass

Durable progress for the language delight pass. Update before work, after findings, and at each
task boundary. `RESUME HERE` names the only next task.

## RESUME HERE

- State: D1 COMPLETE. D2 is next.
- Next task: D2, tool descriptions in `crates/orchestrator/src/language/catalog.rs`.
- Baseline: `dev` at `1a138a87` (feat(language): teach through validation messages).
- D4 debt so far: none beyond the standing live re-probe; the D1 messages are unit-pinned and
  gate-green but have not been exercised through the swapped authority.

## Task table

| Task | State | Commit subject | Evidence |
| --- | --- | --- | --- |
| D1 validation messages | COMPLETE | `feat(language): teach through validation messages` | `1a138a87`; 19 message families rewritten (shared validators name the allowed set and received value; every "X is required" site states the expected shape and where the value comes from); new pin test `validation_messages_teach_the_expected_shape`; full gate green (316 lib tests). |
| D2 tool descriptions | READY | `feat(language): delight tool descriptions` | -- |
| D3 result guidance | READY | `feat(language): delight result guidance` | -- |
| D4 live proof + close | READY | `test(language): prove delighted guidance live` | -- |

## Deviations

None.

## Task log

| Task | Date | Status | Findings |
| --- | --- | --- | --- |
| Planning | 2026-08-23 | COMPLETE | Batch scaffolded from the R8 live-lane finding: circular "Correct the call" guidance. Baseline fix already committed (`dcabf582`). |
| D1 | 2026-08-23 | COMPLETE | Shared validators now teach: choice names the full allowed set plus the rejected value; range and timeout include the received value; handles say where handles come from; text overflow reports the received length; validate_key lists the named keys; unknown fields point at the advertised schema. Every "X is required" decode site states the expected shape (zoom percent 25-500, resize needs both dimensions, respond text may be empty, drag needs both endpoints or all four coordinates, region needs the full rectangle). |

## Deviations

- D1: per-tool teaching lives at the call sites, not inside the shared coordinate helper -- a generic helper cannot name the tool. No behavior change.
