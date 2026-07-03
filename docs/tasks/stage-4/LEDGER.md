# Stage 4 ledger

Durable, context-wipe-safe record of stage-4 (registry/pipeline architecture, ADR-0023/0024/0025)
execution. This file plus `docs/tasks/stage-2/BROWSER-TESTS.md` are the executor's memory. On
every start, after any interruption, and whenever state is unclear: read the RESUME HERE section
first, then `BOOTSTRAP.md` and the ADR(s) the current task cites, then the current task prompt,
then continue. Never rely on remembering earlier work; re-read files.

## RESUME HERE

- Branch: `stage-4` (created from `stage-3`; create it if absent). Never push, never merge,
  never commit to `main`, `stage-2`, or `stage-3`.
- Progress: nothing landed yet.
- NEXT TASK: `t01` (`docs/tasks/stage-4/t01-one-loader.md`).
- Authority: ADR-0023/0024/0025 (each in its own scope) over task prompts over ADR-0022 over
  the stage-2 shared-format doc over SPEC.
- Invariants after every task: tree green (`cargo test`, `clippy -D warnings`, `fmt --check`),
  `tests/architecture.rs` passing, all-open byte-identical, tools.json and
  `tests/tool_schema_fidelity.rs` byte-untouched (NO exception task this stage), behavior
  preserved except the two sanctioned additions (t01 org-policy loading works, t06 hot-reload),
  ASCII-only, no new dependencies, superseded code deleted in the task that supersedes it.

## Task log

(Append one entry per completed task, newest at the bottom. Shape:)

### <task-id> <title> -- <date>
- Commit: (see this task's commit)
- Files touched: <list>
- Summary: <what landed, key decisions, any conservative choice made>
- Deviations from the prompt/ADR: <numbered, each with reasoning; "none" if none>
- Deletions performed: <the removed functions/files this task retired, or "none">
- Verification: <clippy/fmt/test status; test counts before -> after; which suites unchanged>
- Browser checks queued: <count and ids appended to BROWSER-TESTS.md, or "none">
