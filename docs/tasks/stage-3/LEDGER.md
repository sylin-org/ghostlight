# Stage 3 ledger

Durable, context-wipe-safe record of stage-3 (capability model, ADR-0022) execution. This file
plus `docs/tasks/stage-2/BROWSER-TESTS.md` are the executor's memory. On every start, after any
interruption, and whenever state is unclear: read the RESUME HERE section first, then
`BOOTSTRAP.md` and ADR-0022, then the current task prompt, then continue. Never rely on
remembering earlier work; re-read files.

## RESUME HERE

- Branch: `stage-3` (created from `stage-2`; create it if absent). Never push, never merge,
  never commit to `main` or `stage-2`.
- Progress: nothing landed yet.
- NEXT TASK: `s01` (`docs/tasks/stage-3/s01-navigate-is-read.md`).
- Authority: ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) over task prompts over
  the stage-2 shared-format doc (superseded in sections 4.3 / 6.1-rw / 8) over SPEC.
- Invariants after every task: tree green (`cargo test`, `clippy -D warnings`, `fmt --check`),
  `tests/architecture.rs` passing, all-open byte-identical, the 13 trained tool schemas
  byte-identical (s07 adds the one sanctioned 14th; no other tools.json change ever),
  ASCII-only, no new dependencies, superseded code deleted in the task that supersedes it.

## Task log

(Append one entry per completed task, newest at the bottom. Shape:)

### <task-id> <title> -- <date>
- Commit: (see this task's commit)
- Files touched: <list>
- Summary: <what landed, key decisions, any conservative choice made>
- Deviations from the prompt/ADR: <numbered, each with reasoning; "none" if none>
- Verification: <clippy/fmt/test status; test counts before -> after; which suites unchanged>
- Browser checks queued: <count and ids appended to BROWSER-TESTS.md, or "none">
