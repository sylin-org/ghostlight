# BOOTSTRAP: language delight pass

Read this file, [LEDGER.md](LEDGER.md), and the live tree. Assume no memory of earlier sessions.
`LEDGER.md` is the authority on progress; its `RESUME HERE` names the only next task.

## Objective

One deliberate pass over every model-facing sentence in the 1.0 language: tool descriptions,
validation messages, and return-payload guidance. Every failure teaches the fix. No behavior
changes -- strings and guidance only, except where a message reveals a genuine contract defect
(stop and record; defects become their own fix commits).

## Authority order

1. This file and the current task file (none yet; tasks are single-sweep slices below).
2. `AGENTS.md`, `docs/MEMORY.md` ("say what a person would say", "invisible when healthy").
3. `docs/1.0/LANGUAGE.md` and `docs/1.0/INTENT.md` (delight section).
4. Current source and tests.

## Ground rules

- Strings only. No signature, schema-shape, or behavior changes without a stop-and-record.
- Every edited message must survive the oracle rule: if a test asserts the old string, update the
  assertion in the same commit; add pinned assertions for any new teaching sentence worth keeping.
- ASCII only. Person-plain sentences. No "simply", no jargon, no blame.
- Each slice is one green commit; every prefix stays usable.

## Task sequence

| Task | Scope | Commit subject |
| --- | --- | --- |
| D1 | Every `LanguageError::Invalid` message in `crates/orchestrator/src/language/mod.rs` (~60 sites): each becomes a sentence stating the expected shape or the recovery action. Update pinned assertions. | `feat(language): teach through validation messages` |
| D2 | All 23 tool descriptions + field descriptions in `crates/orchestrator/src/language/catalog.rs`: voice pass, recovery hints where a field is commonly misused (e.g. screenshot region requires a view). Update catalog pins. | `feat(language): delight tool descriptions` |
| D3 | `crates/orchestrator/src/language/outcome.rs`: every `next_steps` and refusal summary -- recovery action first, person-plain. Update outcome pins. | `feat(language): delight result guidance` |
| D4 | Live verification through the swapped authority (rebuild release, exact-path swap per DEV-LOOP, re-run the screenshot-region mistake call and two other failures), full common gate, docs/1.0/LANGUAGE.md reconciliation, ledger close. | `test(language): prove delighted guidance live` |

## Per-task procedure

1. Read LEDGER; confirm the task is next; mark IN PROGRESS.
2. Sweep the named file top to bottom; keep a running count of edited messages.
3. Update pinned assertions in the same commit; add pins for new sentences.
4. Common gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, `npm test` from `extension/`, `node --check` changed JS,
   `pwsh scripts/check-repository-integrity.ps1`.
5. Update ledger evidence + RESUME HERE; commit with the pinned subject; push.

## Failure protocol

A message that cannot be improved without changing behavior is a finding, not a blocker: record
it in the ledger deviations table and move on. Anything else that fails: mark BLOCKED with the
exact command and output, set RESUME HERE, stop.

## Never do these

- Never change a tool name, field name, default, bound, or status vocabulary.
- Never weaken an assertion to make an old string pass.
- Never touch `docs/trust/` claims (they are red-teamed against the tree).
- Never upload, publish, push tags, or mutate anything external.
