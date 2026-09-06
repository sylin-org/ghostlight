# H2a: Stop a flow when its error policy requires it

Status: implemented and committed as `8103c69b`; required gates and the process journey pass;
not deployed. The ledger retains both the original failing regressions and completion evidence.

Owner direction: "Flow stopping - that's simply a bug. we need to fix."
The [ledger](LEDGER.md) records execution. This task fixes SA-01's continuation defect under
ADR-0133 Decision 9 and the current 1.0 language. It does not add transactional rollback.

## Source and placement

- `crates/orchestrator/src/work/flow.rs`: the loop's child-decode error and non-success terminal
  branches set `stopped` without exiting. Reference-error branches already exit immediately.
- `crates/orchestrator/src/work/mod.rs`: reuse the executor test fixture and fake browser command
  log for regression evidence through the actual application boundary.
- `language/mod.rs`, `work/result.rs`, and `work/sequence.rs` provide existing flow validation,
  status/effect types, and immediate stop behavior. No new type or protocol is required.

Before editing, verify those two missing exits still exist. If a later change moved control-flow
ownership, inspect the current owner rather than transplanting the old loop. The optional
`.agentic/reference/utilities.md` catalog was absent during exploration.

## Change

Exit the loop immediately after recording the stopping child in both branches. Preserve all
previous results and browser effects. Keep explicit `continue` working, and leave the existing
reference-error stop behavior consistent with the repaired branches.

This removes the ineffective flag-only behavior at its owning seam. Broader aggregate status,
counts, wording, and child audit completion remain separately tracked work; this task does not
pretend to complete all of H2 or H4.

## Pinned evidence

The fake browser supplies successful OpenTab outcomes for tabs 7 and 8 on `example.com`.

| Scenario | Expected browser work and result evidence |
| --- | --- |
| Default stop: open, denied Execute under Read-only restrictions, later open | Exactly one OpenTab command. Two child rows: succeeded/applied, blocked/none. `stopped: true`, `completed: 2`, `total: 3`, aggregate effect partial, not repeat-safe. |
| Explicit stop: open, Read `max_chars` resolves to the earlier tab handle, later open | Exactly one OpenTab command. The invalid numeric argument produces an error row; no Read or later OpenTab dispatch. The earlier applied effect remains in the partial result. |
| Explicit stop: open, Read argument references a missing earlier result field, later open | Same no-later-dispatch guarantee, through the existing reference-resolution failure branch. |
| Explicit continue with either argument failure | Exactly two OpenTab commands and no Read command. Three rows include the failure and the successful later open. `stopped: false`; the flow is not repeat-safe. |

Run the new executor regressions against the old implementation first and record the observed
extra browser command. After the fix, run the orchestrator library suite and formatting. Required
whole-repository gates apply before any commit. This change uses an isolated build directory and
does not need a live browser or deployment to prove its orchestration decision.
