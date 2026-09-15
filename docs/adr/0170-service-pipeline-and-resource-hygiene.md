# ADR-0170: Service pipeline and resource hygiene

Date: 2026-09-14. Status: Accepted by owner direction.

Builds on ADR-0005, ADR-0007, ADR-0016, ADR-0045, ADR-0050, ADR-0106, and ADR-0169.

## Context

Following the extension pipeline review in ADR-0169, a comprehensive analysis of the Ghostlight
service (`crates/orchestrator/`, `crates/bridge/`, `crates/mcp-connector/`, and
`crates/browser-connector/`) identified several concurrency and structural inefficiencies:

1. **TCP Listener Spin-Wait**: Both `service_listener` and `browser_listener` configured non-blocking
   listeners with a 20ms sleep loop on `WouldBlock`. This created 100 context switches per second
   (6,000 wakeups per minute) when idle and added 0--20ms latency jitter to incoming connections.
2. **Audit Health Mutex Polling**: An OS thread looped every 250ms calling `audit.recover_if_due()`.
   During normal healthy operations, this thread executed 240 mutex acquisitions per minute solely to
   verify that health remained normal.
3. **Workspace Lease Contention Spinning and Presentation Event Flooding**: Invocation threads waiting
   for a workspace lease looped on a 5ms sleep. On every single 5ms iteration, `self.show_waiting()`
   was fired, flooding `DomainEvent::WorkWaiting` into the workbench presentation layer 200 times
   per second.
4. **Monolithic Source Files and Inline Test Bloat**: Core modules contained extreme test inlining:
   - `crates/orchestrator/src/work/mod.rs` exceeded 5,300 lines, with 3,128 lines of unit tests dumped
     directly in `mod.rs` despite `work/tests/` already existing with 7 modular test suites.
   - `crates/orchestrator/src/governance/mod.rs` contained 2,377 lines of tests for 976 lines of logic.
   - `crates/orchestrator/src/workbench/mod.rs` and `browser/mod.rs` each contained ~1,700 lines of
     tests for ~350 lines of production logic.
   Over 12,000 lines of unit tests were inlined inside source files across the workspace.
5. **Lint and Warning Suppressions**: Suboptimal configuration branching resulted in
   `#[allow(unreachable_code)]` in `install/mod.rs`, and returning a large unboxed `Terminal` struct on
   the stack caused `#[allow(clippy::result_large_err)]` in `work/mod.rs`.

## Decision

1. **Blocking TCP Listeners with Loopback Unblock**: Configure service and browser listeners with
   blocking `accept()`. On service shutdown, connect loopback probes to wake blocking listeners
   immediately, eliminating the 20ms polling loop, idle wakeups, and connection latency jitter.
2. **Event-Driven and Bounded Audit Recovery**: Sleep for the actual recovery interval (5 seconds)
   or park the audit recovery thread when healthy, eliminating idle mutex polling.
3. **Workspace Lease Condvar and Single Presentation Signal**: Coordinate workspace lease acquisition
   via a `Condvar` on `WorkspaceStore`. Fire `self.show_waiting()` once upon entering the wait state
   rather than every 5ms.
4. **Decouple Inline Unit Tests**: Extract inline test suites out of `work/mod.rs`, `governance/mod.rs`,
   `workbench/mod.rs`, and `browser/mod.rs` into dedicated test modules under `tests/`.
5. **Clean Compiler and Clippy Suppressions**: Resolve unreachable code branches in `install/mod.rs`
   with strict platform `cfg` gates, and box large error results in `resolve_semantic`.

## Consequences

- Idle background CPU wakeups drop by over 95%, eliminating thousands of context switches per minute.
- Incoming MCP client and browser connector connections attach with zero polling delay.
- The UI workbench is protected from event storms during workspace lease contention.
- `work/mod.rs` shrinks from 5,348 lines down to ~2,220 lines, restoring clear architectural focus.
- Zero changes to model-facing language schemas, wire protocol framing, or trust center boundaries.
