# ADR-0127: One invoked desktop authority

- Status: Accepted
- Date: 2026-08-16
- Amends: ADR-0102 Decision 1 and acceptance evidence 7, ADR-0104 Decisions 3 and 4,
  ADR-0112 Decisions 1 and 4, ADR-0118 Decision 1, and ADR-0124 acceptance evidence 3
- Builds on: ADR-0104, ADR-0112, ADR-0118, and ADR-0123

## Context

Ghostlight had two explicit ways to start its orchestrator without desktop presentation:
`ghostlight service` and the compatibility flag `ghostlight --headless`. A real installed-product
test used the flag and produced an authority with no tray. That looked like a product defect on a
KDE desktop that supports tray icons, even though the process had done exactly what the flag asked.

The mode is also inconsistent with the installed lifecycle. An MCP connector invokes the sibling
orchestrator when a client uses it, the browser connector invokes it when the extension connects,
`ghostlight call` invokes it for command-line work, and a person can invoke it directly. Each is a
concrete request for Ghostlight to exist. None needs a second, presentation-free product identity.

Tray availability is still an operating-system and desktop-session capability. Some supported or
future environments may not expose a tray. Ghostlight must use a tray where one exists without
making tray support the universal definition of a desktop authority.

## Decision

### 1. Every supported start initializes the desktop authority

The orchestrator has one startup path. Direct `ghostlight`, explicit `ghostlight open`, connector
demand-start, and command-line demand-start all converge on the same no-argument executable. That
path initializes the Tauri desktop authority and the orchestrator host in one process.

Connectors remain demand-driven. They do not supervise Ghostlight at login or launch it before an
MCP client or browser extension asks for it. Direct execution and `ghostlight call` are equally
explicit invocations.

### 2. Remove both presentation-free command-line routes

`ghostlight service` and `ghostlight --headless` are removed without compatibility aliases. Both
inputs are unknown commands. Help, manual pages, shell completions, active guides, package smokes,
and process journeys offer only the single desktop-authority launch.

The internal launch vocabulary has no service-only or headless variant, and the executable has no
standalone `run_forever` entry point.

### 3. Do not degrade into an invisible authority

Failure to construct the Tauri application, a panic during desktop startup, an abnormal desktop
event-loop exit, or an event-loop panic ends the process. Dropping the host tears down its listeners
and runtime discovery. Ghostlight does not keep serving invisibly after losing its desktop
authority.

A tray-construction failure is narrower when the workbench can still be constructed. The authority
may continue and names the Applications entry and `ghostlight open` as recovery. If neither the
tray nor the workbench can be constructed, startup exits. On Windows and Linux desktop sessions
that provide a tray, release evidence must prove the Ghostlight tray icon appears. On a session
without a tray, the Applications entry and explicit Open remain the interaction routes.

### 4. Tests use the production launch

Process and CLI journeys start the no-argument executable. Linux CI and package containers provide
a virtual display when they exercise the desktop authority; they do not gain a test-only service
mode. Visible platform release tests remain responsible for proving the real tray and window.

### 5. Preserve retirement evidence without restoring the mode

Immutable earlier ADRs retain their historical command spelling. The narrow 0.8 supervisor
migration fixtures also retain old `service` and `--headless` command lines so a current install can
recognize and retire Ghostlight-owned legacy startup artifacts. Those strings are migration input,
not accepted current commands.

## Consequences

- A tray-capable desktop can no longer be made tray-less through an ordinary Ghostlight option.
- Demand-start remains automatic only in response to an MCP client, browser extension, or CLI call.
- Desktop initialization failures are visible process failures rather than silent service
  degradation.
- Test environments must provide a desktop display to exercise the real executable boundary.
- Tray-less desktop sessions retain explicit workbench access without pretending to have a tray.

## Acceptance evidence

1. `ghostlight service` and `ghostlight --headless` both fail as unknown commands.
2. Help, manual pages, and bash, zsh, and fish completions offer neither route.
3. Process, CLI, and package journeys launch `ghostlight` with no application arguments.
4. A desktop build or event-loop failure tears down runtime discovery and exits non-zero.
5. A tray-construction failure leaves `ghostlight open` able to reveal the workbench, or startup
   exits when the workbench is also unavailable.
6. A visible test on each tray-capable supported desktop observes the Ghostlight tray icon.
