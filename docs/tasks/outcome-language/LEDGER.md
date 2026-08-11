# LEDGER: language-owned outcome voice

Durable record of executing [SPEC.md](SPEC.md). Decision record: [ADR-0103](../../adr/0103-language-owned-outcome-voice.md).

## RESUME HERE

Nothing. The batch is complete and the tree is green. Follow-ups live in the Owed section of
[STATUS.md](../../STATUS.md), not here.

## What landed

| Step | Commit | Result |
| --- | --- | --- |
| Per-action observation at the browser seam (the batch's precondition) | `e5228c01` | `Observed` gathered in `Executor::dispatch`, read and cleared by the one completion path |
| The voice moved into `crates/orchestrator/src/language/outcome.rs` | `9609fcfc` | `Outcome`, `Refusal`, `WorkspaceReason`, and `Observed` in one module; every completed-action sentence, its next steps, and its measurements come from one typed value |

Executed in one pass rather than as the five separate task commits the spec anticipated. Gate green
at each stage: `cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings` clean;
`cargo test --workspace` 98 Rust tests; `npm test --prefix extension` 39 tests;
`node tests/process-journey.mjs` against freshly built binaries.

## Verified against the spec

- Every sentence in SPEC section 3.1 is transcribed, including the grouped thousands separator,
  singular and plural forms, `Stopped at step {completed + 1} of {total}.`, and the two `Waited`
  forms.
- The seam and the voice own disjoint facts as decided in SPEC section 2.1: `observed_from` records
  host and readiness only and stays exhaustive over `BrowserOutcome`; counts and capture sizes come
  from `Outcome::observed`; `finish` merges the outcome over the seam.
- SPEC section 2.2 holds: what a sentence names, the observation carries. It has its own test.
- Refusal wording is unchanged, as SPEC section 3.4 required.
- `Observed` moved without changing its five fields or a serialized byte, and has a round-trip test.

## Findings (the feedback channel)

Findings 1 and 2 were fixed together after the pass, in the commit that dropped the hero chip and
moved the host guard to the guide. Finding 3 remains owed in [STATUS.md](../../STATUS.md).

1. **The hero says the host twice.** SPEC section 2.3 removed the surface's guess in `describe`,
   which the change did correctly, but it left the hero's separate host chip in place. When the
   sentence was boilerplate the chip carried the only useful fact; now the sentence names the host
   itself. The spec never said to remove the chip, so this is an authoring gap, not an executor
   error: a spec that changes what a sentence contains must say what the surrounding chrome should
   stop repeating.
2. **A guard now pins that duplication.** `surface_renders_seam_facts_and_trusts_outcome_language_for_measurements`
   asserts `app.js` contains `observed.host`, which only the chip satisfies. The assertion was
   reasonable when written and became stale in the same commit that made it stale. Removing the
   chip requires moving that half of the guard: readiness is the fact the surface can only get from
   the observation; the host's consumer is the audit record and its guide.
3. **Refusal sentences are now the only boilerplate left.** Holding them unchanged was the right
   call for a mechanical batch, but the contrast is visible on the monitor: every success reads
   plainly and every refusal still reads like an engineer wrote it.

## Lesson for the next batch

Both findings are the same authoring miss. The spec pinned what each sentence would say and what
each projection would carry, but not what became redundant elsewhere once a sentence grew richer. A
future spec that changes a rendered string should carry a short "what this makes redundant" section
listing the chrome and the guards that quietly depend on the old wording.
