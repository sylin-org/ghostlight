# LEDGER: language delight pass

Durable progress for the language delight pass. Update before work, after findings, and at each
task boundary. `RESUME HERE` names the only next task.

## RESUME HERE

- State: D2 COMPLETE. D3 is next.
- Next task: D3, result guidance in `crates/orchestrator/src/language/outcome.rs`.
- Baseline: `dev` at `1a138a87` (feat(language): teach through validation messages).
- D4 debt so far: none beyond the standing live re-probe; the D1 messages are unit-pinned and
  gate-green but have not been exercised through the swapped authority.

## Task table

| Task | State | Commit subject | Evidence |
| --- | --- | --- | --- |
| D1 validation messages | COMPLETE | `feat(language): teach through validation messages` | `1a138a87`; 19 message families rewritten (shared validators name the allowed set and received value; every "X is required" site states the expected shape and where the value comes from); new pin test `validation_messages_teach_the_expected_shape`; full gate green (316 lib tests). |
| D2 tool descriptions | COMPLETE | `feat(language): delight tool descriptions` | -- |
| D3 result guidance | READY | `feat(language): delight result guidance` | -- |
| D4 live proof + close | READY | `test(language): prove delighted guidance live` | -- |

## Deviations

None.

## Task log

| Task | Date | Status | Findings |
| --- | --- | --- | --- |
| Planning | 2026-08-23 | COMPLETE | Batch scaffolded from the R8 live-lane finding: circular "Correct the call" guidance. Baseline fix already committed (`dcabf582`). |
| D1 | 2026-08-23 | COMPLETE | Shared validators now teach: choice names the full allowed set plus the rejected value; range and timeout include the received value; handles say where handles come from; text overflow reports the received length; validate_key lists the named keys; unknown fields point at the advertised schema. Every "X is required" decode site states the expected shape (zoom percent 25-500, resize needs both dimensions, respond text may be empty, drag needs both endpoints or all four coordinates, region needs the full rectangle). |
| D2 | 2026-08-23 | COMPLETE | All 23 tool descriptions revoiced around when to reach for the tool and what to do instead (history and window teach handle recollection, screenshot teaches that every capture returns a view_, click prefers targets over points, execute defers to semantic tools, sequence states stop-at-first-failure, dialog teaches blocking, diagnose teaches observation starts on the first call). Misuse-prone field descriptions now name their source: region/coordinate views say take a screenshot first; upload source_image says it comes from an earlier screenshot; dialog actions describe themselves instead of a shared "Dialog action." Two new pins: `catalog_descriptions_teach_the_remedy` and `catalog_strings_are_ascii`. Full gate green (318 lib tests, 127 extension tests, repository integrity). |

## Deviations

- D1: per-tool teaching lives at the call sites, not inside the shared coordinate helper -- a generic helper cannot name the tool. No behavior change.
