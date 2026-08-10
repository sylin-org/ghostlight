# ADR-0102: Integrated desktop workbench in the orchestrator monolith

- Status: Accepted
- Date: 2026-08-10
- Amends: ADR-0030's local console process placement and ADR-0077's management surface
  placement
- Builds on: ADR-0005, ADR-0030, ADR-0057, ADR-0077, ADR-0079, ADR-0096, and ADR-0101

## Context

Ghostlight needs a calm, searchable, user-facing workbench for current sessions and operations,
history, diagnostics, configuration, supported development-harness installation, and important
notifications. It must use the preserved Ghostlight identity and operate across Windows, macOS,
and Linux. A bundled web interface inside Tauri 2 supplies the portable rendering surface, tray,
window lifecycle, and native notification delivery.

The first design considered a separate desktop companion connected to the orchestrator through a
new authenticated loopback protocol. That would isolate the processes, but would also add runtime
discovery, authentication, framing, correlation, reconnection, resynchronization, version skew,
and unknown-mutation handling for two components that are installed, started, and upgraded as one
product. Those costs duplicate mechanisms that are justified at the unavoidable MCP and browser
process boundaries.

Sharing a process must not mean sharing responsibilities. UI, WebView, notification, and window
failures are presentation failures. They must not become domain failures or own product state.
Likewise, an in-process UI is still untrusted input and must not receive arbitrary filesystem,
shell, network, or product mutation authority.

This work also dogfoods Ghostlight's fringe-stability promise. The complete workbench feature is
expressible inside the orchestrator and existing operating-system capabilities. It must therefore
cause zero source, manifest, asset, or test changes in `crates/mcp-connector`,
`crates/browser-connector`, or `extension`.

## Decision

### 1. Ship one modular-monolith executable

The existing `ghostlight` executable hosts the orchestrator service and the Tauri 2 desktop shell.
Normal desktop startup starts the orchestrator before presenting the workbench and may begin with
the window hidden in the tray. Closing the window hides it. Reloading or recreating the WebView
does not restart the orchestrator. Only an explicit whole-product quit stops the service.

The same executable retains a headless mode that starts no WebView. Domain and application tests
must run without initializing Tauri.

No GUI-specific listener, runtime credential, wire schema, companion process, or local HTTP server
is introduced.

### 2. Preserve strict dependency direction

The dependency direction is:

```text
bundled web UI
  -> allowlisted Tauri commands
  -> WorkbenchFacade application boundary
  -> orchestrator domain

orchestrator presentation facts
  -> operating-system presentation port
  -> Tauri tray, window, and notification adapter
```

Domain and application modules do not import Tauri, WebView, window, tray, or notification types.
Tauri is an inbound presentation adapter and an outbound operating-system capability adapter. It
cannot call browser mechanisms directly, bypass governance, or mutate workspace aggregates.

### 3. Keep execution contexts independent and small

The Tauri event loop owns native window and tray operations. The WebView owns rendering and
ephemeral interaction state. The orchestrator, relay listeners, persistence, and blocking work run
outside the UI context. Long-running work never blocks the UI event loop.

Contexts communicate through narrow typed calls and bounded immutable messages. Locks are not held
across domain, adapter, or I/O boundaries. Threads and tasks are implementation details, not new
actors or services. Ghostlight does not add a generic event bus, actor runtime, workflow engine,
CQRS layer, or dependency-injection container.

Expected failures use typed results. Presentation delivery, notification, tray, closed-consumer,
and WebView failures are contained and cannot change governance or completion truth. Worker
termination is observed by its owner. A disposable UI rebuilds from a fresh orchestrator snapshot.
Only process-fatal native or operating-system termination remains a shared failure class; durable
state and reconnecting relays provide restart recovery.

### 4. Make the orchestrator the sole state authority

The orchestrator owns sessions, operations, browser-instance facts, harness connections, history,
diagnostics, configuration semantics, notification decisions, and all durable state. These
collections are plural even when only one current transport instance is available.

The WebView owns only disposable view state such as the selected page, scroll position, and an
unfinished search query. Opening or reloading it obtains an immutable snapshot. It never becomes
the source of truth.

One typed `WorkbenchFacade` exposes only:

- an at-a-glance snapshot;
- bounded search over user-visible records;
- explicit user intents;
- semantic presentation facts; and
- capability and diagnostic facts.

There is no arbitrary JSON mutation dispatcher or reflection registry. Read methods and mutation
methods remain semantically distinct without introducing a separate command/query architecture.

### 5. Preserve definite mutation and recovery semantics

Every workbench mutation returns an accepted, rejected, or failed result from the application
boundary. User intents pass through the same product and governance owners as equivalent non-UI
actions. UI rendering is never part of a domain transaction.

Losing or reloading the WebView after a confirmed mutation does not change its outcome. An
uncertain side effect is never automatically replayed. Snapshot reconstruction, notification
delivery, and other read-only presentation work may retry safely.

### 6. Treat the WebView as untrusted presentation input

Tauri commands are explicitly allowlisted and validate bounded arguments at the adapter boundary.
The workbench uses a restrictive content security policy, bundled local assets, and the minimum
Tauri capabilities. Runtime tokens and secrets never enter JavaScript. The shell provides no
arbitrary shell, filesystem, process, remote navigation, or network operation.

Harness installation uses explicit orchestrator-owned implementations and visible user intent. It
does not expose a generic command runner.

### 7. Keep operating-system differences at the outer adapter

The workbench uses bundled standard HTML, CSS, and JavaScript. The frontend asks for semantic
capabilities rather than detecting an operating system. Platform-specific behavior terminates in
narrow Rust adapters. Unavailable tray or notification capabilities degrade explicitly without
changing product semantics.

The shell preserves the established Ghostlight name, icon assets, sky `#38bdf8`, ink `#eaf6ff`,
governance ground `#0c0f14`, spring curve `cubic-bezier(.22,1,.36,1)`, and reduced-motion behavior.

### 8. Keep the initial information architecture deliberate

The workbench provides one compact at-a-glance home and five high-value destinations:

- current sessions and operations, including plural browser instances;
- history;
- checkup and diagnostics;
- configuration; and
- install, check, and uninstall for explicitly supported development harnesses.

Search spans user-visible workbench records. Blocked work and attention-required transitions may
request a deduplicated high-signal operating-system notification. Routine success remains quiet.
There is no telemetry, activation service, update ping, remote application content, or decorative
placeholder state.

### 9. Protect the dogfooding boundary mechanically

Implementation and review compare the completed change against its starting revision. The
following paths must have an empty diff:

- `crates/mcp-connector`;
- `crates/browser-connector`; and
- `extension`.

The shared bridge changes only if an already-real process boundary requires it. This decision adds
no such boundary, so the workbench does not require a bridge change.

## Consequences

Ghostlight gains one packaged version, lifecycle, state authority, and transaction boundary. It
avoids a new authenticated service and its permanent compatibility surface. The WebView remains
replaceable, and the application facade remains independently testable.

The Rust host now links the platform WebView and tray runtime. Native process-fatal faults can stop
both shell and service, while ordinary JavaScript, rendering, window, notification, and command
failures are contained. This trade is accepted because hard process isolation is not worth the
additional protocol, recovery, packaging, and version-skew mechanisms without observed need.

The application boundary is intentionally movable but not prematurely serialized. If measured
reliability evidence later requires a separate UI process, a new ADR may place the existing facade
behind a transport without moving domain ownership into the desktop adapter.

## Acceptance evidence

1. Domain and `WorkbenchFacade` tests run without Tauri initialization.
2. Reloading or closing the window leaves an active orchestrator service running.
3. Injected presentation and notification failures do not change an operation result.
4. Every Tauri command is bounded, allowlisted, and delegates to the facade.
5. The workbench reconstructs plural session, operation, and browser facts from a snapshot.
6. Blocked and attention-required facts remain in history even when native notification delivery
   fails.
7. Headless startup retains the existing service journeys.
8. Native builds and smoke tests cover each supported operating system before release.
9. Diffs for the MCP connector, browser connector, shared bridge, and extension are empty.
