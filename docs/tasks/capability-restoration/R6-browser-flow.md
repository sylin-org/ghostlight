# R6: Governed result-aware browser flows

## Goal

Restore general bounded composition while keeping `browser_sequence` as the short known-action
surface.

## Required work

- Add `browser_flow` as the twenty-third tool with one through twenty uniquely named steps.
- Each step names a current advertised non-composite tool and supplies an argument object.
- Add explicit result reference objects with an earlier step id and JSON Pointer. Reject forward,
  missing, cyclic, out-of-bounds, and non-result references before dispatch when knowable.
- Resolve references, then run the ordinary child decoder again. Do not forward model tool names or
  arguments to the browser.
- Intersect top-level and child restrictions. Classify and authorize each child normally under the
  invocation's immutable authority ceiling.
- Implement default `on_error:"stop"`, explicit `continue`, `dry_run`, and a bounded total budget.
- Return bounded per-step canonical envelopes and truthful aggregate partial or unknown effects.

## Evidence

- Portable typo-closed catalog schema and exact decoder parity.
- Reference success and every invalid-reference family.
- Dry run proves zero browser dispatch and returns decoded capability requirements.
- Per-step RAWX, audit, deadline, cancellation, landing governance, and stale-handle tests.
- Stop, continue, partial, unknown, and repeat-safety tests.
- Process journey finds a target, references its handle in a later action, and reads the final
  result through ordinary MCP text and structured content.

## STOP conditions

- Implementation requires recursive MCP calls, another workspace lease, or nested flow/sequence.
- A child can skip the canonical decoder, capability map, executor, or completion gate.
- Flow output can exceed the sum of existing bounded child results without its own cap.

## Commit

`feat(browser): add governed result-aware flows`

