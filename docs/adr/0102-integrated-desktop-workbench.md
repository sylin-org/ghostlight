# ADR-0102: Integrated desktop workbench in the orchestrator monolith

- Status: Accepted; amended 2026-08-11 (see the amendment below)
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

## Amendment (2026-08-11): live monitor surface, published palette, three destinations

Status: Accepted. Implemented in `73ee6d6`. Supersedes the palette list in Decision 7 and the
destination list in Decision 8, and refines Decisions 4 and 5 with a sequenced change channel.
Decisions 1, 2, 3, 6, and 9 stand exactly as written. Additionally builds on ADR-0083, whose
signature medallion vocabulary the workbench reuses.

### A1. The read model gains sequenced changes (refines D4 and D5)

D4 said the WebView obtains an immutable snapshot when it opens or reloads. That is still true and
the orchestrator is still the only authority, but a snapshot-only surface could not show work
happening. The implementation polled a full snapshot every 1.5 seconds, so any operation that
started and finished inside one poll window was never seen running, only recorded afterwards. The
product promise is that browser work stays visible; a surface that samples cannot keep it.

The orchestrator now also publishes a closed sequenced change vocabulary through a
`WorkbenchEventSink` outbound port, shaped like the existing `WorkbenchPresentationPort`:

- `OperationStarted`, `OperationChanged`, `OperationSettled`;
- `RuntimeChanged`.

Every change carries a monotonic `seq`, and `WorkbenchSnapshot` carries the `seq` it reflects,
read after assembly so a snapshot never claims to be newer than its contents. A surface receiving
a sequence other than its last plus one has missed a change and must resynchronize from a fresh
snapshot instead of trusting its cache. Application is idempotent by invocation, so a change
re-delivered across a snapshot boundary is harmless. The WebView therefore holds a cache it can
prove is current, and never becomes the source of truth.

This adds no process, listener, credential, wire protocol, or generic event bus. Delivery is best
effort: a closed or reloading WebView is an ordinary presentation outcome and cannot change
governance or completion truth. A projection with no sink attached publishes nothing and leaves
its sequence at zero, so headless runs and domain tests stay free of presentation. Locks are never
held across a publish, per D3. The WebView may listen but is not granted permission to emit.

Collections that change rarely -- sessions, browser instances, diagnostics, harnesses, and
configuration -- remain snapshot-owned. Granularity is spent where it buys live presentation
rather than uniformly.

`OperationSummary` now carries the governed `Capability`, so live work can be classified as
plainly as completed history already could. `DomainEvent::WorkStarted` carries it to get there.

### A2. Colour follows the published sylin.org palette (supersedes D7's palette list)

D7 pinned sky `#38bdf8`, ink `#eaf6ff`, and governance ground `#0c0f14`. The workbench now follows
the sylin.org night-garden standard using Ghostlight's own published accent:

- accent teal `#5eead4`, carried as `--a` / `--al` / `--argb`;
- ground `#0f0e12`, the five-step ink ramp, and hairline edges.

No rule hard-codes the hue, matching the site standard that a project accent is a property rather
than a literal. The accent is reserved for live activity, so the window stays neutral at rest and
brightens only when work is genuinely happening.

The in-page renderer keeps sky. It is a different surface -- Ghostlight drawing inside somebody
else's page, where the signal colour is already trained on users and frozen by an extension test --
and `extension/` sits inside D9's must-be-empty diff. The two surfaces now differ deliberately.
What they still share is the spring curve `cubic-bezier(.22,1,.36,1)` and the medallion vocabulary
of ADR-0083, which the workbench reuses so a settled action in the window shows the same shape the
user saw floating in the page. The name, icon assets, and reduced-motion behaviour named by D7 are
unchanged. Unifying the two palettes would require an extension change and a new decision.

### A3. Three destinations, and one runtime control (supersedes D8's destination list)

D8 specified one at-a-glance home and five destinations. In use, home, sessions and operations,
and history proved to be one dataset at three ages, and the split made a user asking "what
happened" check three pages. Configuration held three read-only cards once the runtime control
left it. Runtime control had three separate affordances: a home button, a segmented control, and
the tray.

The workbench now provides:

- **Monitor**, the landing surface. One hero action that is never empty, and a newest-first queue
  that a finished action lands in as the next one rises, so idle shows the last completed action
  rather than an empty panel. Connected sessions and browser instances appear as a compact strip.
- **MCP integrations**, to check, connect, and disconnect explicitly supported MCP clients. The
  former "Install" label did not say what was being installed.
- **Status**, holding checkup diagnostics, authority sources, and the end-session intent.

The runtime control lives in the persistent lamp band beside the connection state, giving pause
and resume one affordance that matches the tray. The rare, consequential end-session intent stays
on Status rather than in always-visible chrome. Search still spans user-visible records. The rest
of D8 holds unchanged: blocked and attention-required transitions may still request a deduplicated
notification, routine success stays quiet, and there is no telemetry, activation service, update
ping, remote application content, or decorative placeholder state.

### Acceptance evidence added

10. Every change the orchestrator can publish has a handler in the surface, and every governed
    capability class has a visual treatment.
11. Every runtime intent stays reachable from the surface, guarded by an exhaustive match that
    fails to compile when a new intent is added.
12. The published palette is present and the accent is defined once, so no rule hard-codes it.
13. A projection with no sink attached publishes nothing and leaves its sequence at zero.
14. One operation lifetime publishes a gapless sequence, and published changes stay payload-free.
15. The workbench capability grants listen and unlisten, and does not grant emit.
