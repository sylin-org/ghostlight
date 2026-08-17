# Doctor readiness parity -- 2026-08-17

Status: source and real-process pass; installed-desktop S8 evidence remains open

## Decision and shape

The owner approved the literal ADR-0126 parity measure. `ghostlight doctor` may query an
already-running local authority for its current content-free readiness projection. The query is an
authenticated, read-only service opening. It does not demand-start Ghostlight, reveal the
workbench, admit a channel, open a workspace, create a session, or write audit history.

The running service serializes the existing orchestrator-owned `ReadinessSummary`. The requesting
orchestrator executable decodes that same type and prints its exact word and detail. The bridge
transports an opaque value and owns no readiness classification or product language.

When no authority is running, `doctor` retains its existing installation and runtime diagnosis and
does not start one merely to obtain readiness. If a runtime is observed but the inspection races
with its exit or cannot decode its response, text output says the current state is unavailable
instead of inventing one.

## Evidence

- The service bridge request and response round-trip through their tagged wire representation.
- The service inspection returns the exact readiness in `WorkbenchFacade::snapshot()`.
- The same test proves the inspection leaves sessions empty and history unchanged.
- A negative control proves inspection of an absent runtime returns an error and creates no runtime
  discovery file.
- A six-state guard renders every `Readiness::ALL` member with the exact shared word and detail.
- The real CLI process journey starts one isolated desktop authority, runs text and JSON `doctor`
  as separate processes, and observes `Not connected` from the live projection. Its audit-count
  assertion remains six model invocations, proving the two doctor inspections add no audit record.

## Verification

- `cargo fmt --all -- --check`
- locked workspace/all-target Clippy with warnings denied
- full workspace tests: 309 orchestrator library, 11 orchestrator binary, 33 bridge, and 6 MCP
  connector tests, all passing
- fresh isolated workspace build
- real CLI journey with live text and JSON readiness inspection
- process reconnect/open/read/recording/close/refusal journey
- 116 extension tests
- 42 workbench-surface assertions and the policy-grammar journey

This closes the implementation and automated portion of ADR-0126's parity measure. S8 still must
observe the line on the required installed Ubuntu GNOME Wayland and Windows candidates alongside
the workbench state before the epic closes.
