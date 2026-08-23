# BOOTSTRAP: published capability restoration on 1.0 seams

Read this file, [LEDGER.md](LEDGER.md), the current task file, and the live tree. Assume no memory
of earlier sessions. `LEDGER.md` is the authority on progress and its `RESUME HERE` block names the
only next task.

## Objective

Implement [ADR-0133](../../adr/0133-behavioral-capability-restoration.md): restore the genuine
browser behaviors published in 0.8 while keeping the current 1.0 names, typed operations,
governance, opaque handles, stable connectors, and truthful outcomes.

The exact published comparison baseline is
`993135b048b60622157266b53b21f1719c9df4b3`. It is evidence only. Never copy its source.

The implementation baseline before this batch was
`c8a181cc15e39b25b2cdc6864c8303efe345f561` on `dev`.

## Authority order

1. The active request and [ADR-0133](../../adr/0133-behavioral-capability-restoration.md).
2. `AGENTS.md`, `docs/MEMORY.md`, and `docs/STATUS.md`.
3. `docs/1.0/INTENT.md`, `LANGUAGE.md`, `ARCHITECTURE.md`, and `ACCEPTANCE.md`.
4. The current source and tests for exact placement and current behavior.
5. Historical ADRs and 0.8 evidence for behavior that ADR-0133 translates.
6. The current task file.

If two authorities conflict, stop and record the conflict. Do not silently choose one.

## Architecture pins

- The orchestrator owns model language, semantic matching rules, composition, governance,
  workspace resources, and completed outcome language.
- The bridge owns closed typed physical contracts and capability revision requirements.
- The extension owns Chromium and page-local mechanics only. It receives no policy and authors no
  product decision.
- MCP and browser connectors remain generic and byte-stable unless a demonstrated framing defect
  requires a new ADR. This batch expects neither connector to change.
- Every direct, semantic, and composed operation crosses the existing executor, workspace lease,
  governance facade, browser port, and completion gate.
- Every model object is typo-closed. Decoder requirements, catalog schemas, examples, defaults, and
  active documentation move together.
- New repeated literals and closed vocabularies receive named constants or typed enums beside their
  owner.
- No task may revive an old tool alias, numeric tab id, page selector, or policy decision in the
  adapter.

## Task sequence

One task is one green behavior commit. Complete them in order because later tasks reuse earlier
typed seams.

| Task | File | Checkpoint | Depends on |
| --- | --- | --- | --- |
| R1 | [R1-negotiated-repl.md](R1-negotiated-repl.md) | Capability revisions and REPL-grade execute | -- |
| R2 | [R2-precision-input.md](R2-precision-input.md) | Modified input, focused typing, duration wait, point wheel | R1 |
| R3 | [R3-semantic-actions.md](R3-semantic-actions.md) | Atomic semantic targeting, typed forms, postconditions | R1, R2 |
| R4 | [R4-document-reading.md](R4-document-reading.md) | Article reads, document trees, subtree snapshots and diffs | R1, R3 |
| R5 | [R5-image-and-file-flow.md](R5-image-and-file-flow.md) | Inline upload and generation-bound screenshot reuse | R1, R2, R3 |
| R6 | [R6-browser-flow.md](R6-browser-flow.md) | General decoded composition with result references | R1-R5 |
| R7 | [R7-guarded-navigation.md](R7-guarded-navigation.md) | Explicit beforeunload discard | R1 |
| R8 | [R8-integration-and-parity.md](R8-integration-and-parity.md) | Full source, process, live-browser, and parity proof | R1-R7 |
| R9 | [R9-release-handoff.md](R9-release-handoff.md) | Deterministic replacement package and honest release state | R8 |

At most one task is `IN PROGRESS`. Every completed prefix must remain usable and green. If a task
cannot be green without starting a later task, the task boundary is wrong: stop and amend the plan
before widening it.

## Per-task procedure

1. Read `LEDGER.md` and confirm this task is the named next action.
2. Confirm the worktree. Preserve unrelated user changes. Stop if they overlap the task.
3. Read the ADRs and exact source files named by the task. Re-run targeted searches instead of
   trusting line numbers in this plan.
4. Check every STOP condition in the task.
5. Mark the ledger task `IN PROGRESS` while working.
6. Implement the smallest complete vertical slice. Update active 1.0 contracts only for behavior
   that now exists.
7. Run the task-specific tests and the common gate below.
8. Update `docs/STATUS.md`, the 0.8 recovery explanation when its truth changed, this ledger's
   evidence and deviations, and `RESUME HERE`.
9. Commit once with the task's pinned conventional subject. The ledger identifies tasks by their
   unique subject; a later session may add the resulting short hash without creating a second
   checkpoint commit.

## Common gate

Use an isolated target so a live deployment cannot make the result stale or hold an executable:

```powershell
$env:CARGO_TARGET_DIR='.target-capability-restoration'
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
Push-Location extension
npm test
Pop-Location
node --check extension/content.js
node --check extension/service-worker.js
pwsh -File scripts/check-repository-integrity.ps1
```

Also run `node --check` on every changed extension JavaScript file. Run the process journey when a
task changes language, execution, framing, capability negotiation, or extension commands. Always
set `GHOSTLIGHT_BIN_DIR` to the isolated build directory so it cannot pass against stale binaries.

## Failure protocol

If a STOP condition or gate fails:

1. Do not skip the task and do not weaken the assertion.
2. Preserve useful diagnostics without committing broken production behavior.
3. Mark the task `BLOCKED` with the exact failed command, output summary, and suspected owner.
4. Set `RESUME HERE` to the blocker and stop.

Difficulty, duration, or a test needing investigation is not itself a reason to redefine the
capability. Change ADR-0133 only through a new ADR or a clearly marked amendment approved by the
owner.

## Never do these

- Never copy code from `reference/` or an old commit. Interface and technique observations only.
- Never read or modify `local/`, `/private/`, or `saps/` without the owner's explicit permission.
- Never add telemetry, remote assets, an update ping, or any hidden network dependency.
- Never put governance, capability classification, audit, result language, or workspace ownership
  in the extension.
- Never make a newer physical command appear supported by an older capability revision.
- Never store screenshot or inline-file bytes in audit, presentation, durable workspace state,
  extension storage, or logs.
- Never weaken credential handoff, landing governance, stale-handle checks, cancellation, or
  uncertain-effect reporting to make a parity journey pass.
- Never upload a Store package, resubmit a review, push, tag, publish, or mutate a public channel.
  R9 prepares and verifies local artifacts only. External mutation requires a new explicit owner
  confirmation.

## Completion

R1 through R9 are complete, every row in the ledger has evidence, the current contracts describe
the shipped 23-tool language, all gates pass from a fresh isolated build, live Chrome exercises
every restored family through a real MCP client boundary, and the replacement extension package is
deterministic and ready for a separately authorized Store handoff.

