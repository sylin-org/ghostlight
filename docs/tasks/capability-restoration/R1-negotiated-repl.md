# R1: Capability revisions and REPL-grade execute

## Goal

Make physical capability revisions enforceable per command, then restore the REPL behavior behind
the unchanged `browser_execute` signature.

## Required work

- Extend the typed browser contract so every command can state a minimum capability revision.
- Reject a command before dispatch when the attached adapter advertises the right capability name
  at an older revision. The result must name the missing mechanism without inventing an effect.
- Keep every current revision-1 command working with a revision-1 adapter.
- In `extension/service-worker.js`, evaluate with `awaitPromise`, `returnByValue`, `userGesture`,
  and `replMode`.
- On the specific bare-top-level-return syntax failure, retry once inside an async function. Do not
  retry arbitrary syntax or runtime failures.
- Preserve bounded values, useful exception descriptions, landing governance, cancellation, and
  source/result audit exclusion.

## Evidence

- Bridge serialization and required-revision tests.
- Browser-coordination test proving revision-1 refusal occurs before dispatch.
- Extension tests for expression value, promise, top-level await, bare return, thrown error, and
  truncation.
- Real process journey proving the value crosses MCP ordinary text and structured content.

## STOP conditions

- Capability revision cannot be checked without changing the opaque browser connector.
- REPL mode changes the declared `browser_execute` input or output contract.
- The fallback would need string rewriting broader than the one diagnosed bare-return case.

## Commit

`feat(browser): restore repl-grade execution`

