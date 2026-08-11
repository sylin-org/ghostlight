# ADR-0104: Demand-start, single engine, and workbench activation

- Status: Accepted
- Date: 2026-08-11
- Amends: ADR-0054, ADR-0063, ADR-0065, and ADR-0102 Decision 1
- Builds on: ADR-0045, ADR-0062, and ADR-0096

## Context

The clean-room 1.0 connectors already survive service restarts, but they only retry a service that
some other process has started. That leaves a poor installation journey: the user must understand
which process is the authority and remember to launch it before either client-side adapter can do
useful work.

The service runtime document identifies the current authority, but it is not a lifetime lock. Two
service processes can otherwise bind different random ports and replace that document. Directly
launching the product also needs to focus the existing workbench instead of creating a second
authority.

The desired product behavior is one installed system with three ordinary entry points:

1. An MCP client can demand-start Ghostlight through the MCP connector.
2. Chromium can demand-start Ghostlight through the browser connector.
3. A human can launch Ghostlight to see its workbench, configuration, and logs.

These entry points must converge on one small lifecycle mechanism. They must not create a second
supervisor, listener, protocol, or policy owner.

## Decision

### 1. The bridge owns one local service-lifecycle seam

The bridge exposes one shared demand-start operation used by both connectors. It locates only the
trusted sibling `ghostlight` executable in the connector's own installation directory. It never
searches `PATH` and never accepts an executable path from the wire.

Before spawning, the seam:

- observes the service lifetime lease;
- honors a fresh deployment lock so replacement cannot race demand-start; and
- starts `ghostlight --background` detached from the connector with null standard streams.

The connector remains a relay. Its only lifecycle judgment is that a failed service connection is
the existing recovery seam at which demand-start is allowed.

### 2. The service holds an operating-system lifetime lease

The service acquires an exclusive lease next to its runtime document before opening listeners or
initializing the desktop. It holds that lease for the process lifetime. A competing process exits
before it can publish a second runtime identity or create a second tray authority.

The runtime document remains discovery data. It is not promoted into a lock or a supervisor.

### 3. Launch modes express user intent

The executable has three launch modes:

- default launch, with `--show` retained as an explicit alias, shows or focuses the workbench;
- `--background` starts the full desktop authority with the tray available and the workbench
  initially hidden; and
- `--headless` starts the service without desktop presentation and cannot reveal a workbench.

Connectors use only `--background`. Headless mode remains explicit for tests and constrained
environments.

### 4. Workbench activation reuses the authenticated service bridge

A direct launch first reads the runtime document and sends an authenticated workbench-activation
request over the existing typed service bridge. The request is allowed as the first message, does
not admit a workspace, and terminates after the response.

The orchestrator invokes one `reveal` method on its workbench presentation port. Desktop mechanics
remain behind that port. No new activation listener, port, socket, or extension message is added.

If the running authority is headless, activation reports that no presentation is available. The
human launch fails clearly instead of starting a conflicting desktop authority.

### 5. Recovery stays idempotent and best effort

Concurrent connectors may both request startup. The shared seam narrows unnecessary spawns, and
the lifetime lease is the final authority that admits exactly one service. Losing launches exit
before desktop initialization.

An adapter continues its existing bounded reconnect loop after requesting startup. Failure to
spawn is logged but does not replace the established connection error or protocol behavior.

## Consequences

- Once the native host and MCP client registration exist, either installed adapter can make the
  service available without a separate startup ritual.
- A direct launch becomes a reliable route to the existing workbench.
- Tray ownership, configuration, logs, governance, and execution remain in one orchestrator
  process.
- Deployment has an explicit quiesce seam, and stale deployment locks stop suppressing recovery
  after a bounded interval.
- The connectors gain one shared lifecycle dependency but no product semantics.
- Operating-system process creation can still fail. The system guarantees convergence when local
  execution is available; it reports ordinary connection failure otherwise.

## Evidence

- Bridge lifecycle unit tests cover exclusive leases, release, trusted sibling resolution,
  deployment-lock freshness, and connector adoption.
- Orchestrator tests cover singleton service ownership and authenticated workbench activation.
- Connector process tests continue to cover restart recovery while deliberately quiescing
  demand-start with the deployment lock.
- The live demand-start journey starts from no service and verifies that a connector creates one
  background authority.
