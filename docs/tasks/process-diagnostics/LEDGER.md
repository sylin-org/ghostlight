# process-diagnostics LEDGER

Durable progress for the process-diagnostics batch. One task = one commit. This file, not the
BOOTSTRAP's task list, is the authority on where the batch stopped.

## RESUME HERE

The batch is opened (2026-08-29) with ADR-0145 accepted and no task executed. Begin at D1, the
sink module in `crates/bridge/src/diagnostics.rs`. Check the BOOTSTRAP's STOP preconditions
against the tree first; they were verified on 2026-08-29 and are recorded below.

## Verified ground truth at opening (2026-08-29)

- `crates/bridge/src/lifecycle.rs` demand-start spawns the sibling orchestrator with no
  arguments, stdin/stdout/stderr sent to null, and the parent environment inherited, so
  `GHOSTLIGHT_DIAGNOSTICS_DIR` set in a client's server configuration already reaches the
  connector and the orchestrator it starts with no code on that path.
- `ghostlight-bridge` is already a dependency of all three executable crates.
- The audit file defaults to a sibling of the runtime discovery file (`GHOSTLIGHT_AUDIT_FILE`
  overrides it), and `crates/bridge/src/runtime.rs` resolves the runtime file with the
  override/sibling/Linux-cache/temporary shape the diagnostics directory mirrors.
- No logging crate exists in the workspace. The only process log lines today are one
  `eprintln!` per disconnect streak in each connector.
- ADR-0016 is the historical shape (per-PID files, bounded bodies, swallowed I/O, stdout
  purity, distinct from audit). Its mechanism was removed with the 0.8 layer by ADR-0143;
  nothing of it remains in `crates/`.
- ADR-0145 was amended three times in place before any task executed (2026-08-29): first,
  activation is layered, an explicit-directory variable over a presence-only `diagnostics.on`
  marker beside the runtime discovery file, over off; second, the marker is actuated from
  surfaces -- the person's hand, `diagnostics on|off`, and an extension popup toggle through
  the orchestrator-owned runtime-control path; third, the live re-check became a layered OS
  watch with a 2-second safety-net tick, whichever fires first. The batch implements the
  amended decision.

## Delight bar (owner directive, 2026-08-29)

The log must be useful and easy to access, use, and infer root causes from. ADR-0145 Decision
7 turns that into acceptance criteria: findable, readable, correlatable, cause-first, bounded,
honest, shareable. Each task's ledger entry states how it met the bar.
