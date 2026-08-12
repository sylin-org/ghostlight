# ADR-0112: One minimized desktop startup

- Status: Accepted
- Date: 2026-08-12
- Amends: ADR-0104 Decisions 1 and 3
- Builds on: ADR-0102

## Context

ADR-0104 gave adapter demand-start a `--background` launch mode distinct from direct desktop
startup. Both modes were meant to create the same service and tray. They differed only in whether
the workbench began visible or hidden.

That distinction does not earn a second production startup path. The installed product should
always have one visible local presence when its desktop authority is running: its tray icon. A
demand-started workbench can avoid interrupting the user by beginning minimized instead of being
hidden behind a special launch mode.

The separate path also made deployment and diagnosis harder. A running service with no visible
tray looked indistinguishable from an intentional presentation-free process even though
`--background` was supposed to initialize the complete desktop runtime.

## Decision

### 1. There is one normal desktop launch

Running `ghostlight` starts the full desktop authority. It always initializes the Tauri desktop,
creates the tray icon, shows the workbench in a minimized state, and runs the orchestrator in the
same process.

`--background` is removed. `--headless` remains the one explicit presentation-free mode for tests
and constrained environments. `ghostlight call` remains a separate scripted intake and does not
own a desktop event loop.

### 2. Connectors start the normal executable

The bridge-owned demand-start seam launches the exact sibling `ghostlight` executable with no
application arguments. Detachment, null standard streams, deployment-lock handling, and the
lifetime lease remain unchanged.

The connector does not choose a presentation mode. It only asks the installed authority to exist.

### 3. Existing-authority activation remains explicit

Launching `ghostlight` while an authority already exists still sends the authenticated workbench
activation request. The existing authority restores and focuses its minimized or hidden window.
No second service, tray, listener, or workspace is created.

Tray clicks use the same restore-and-focus behavior. Closing the workbench still hides it without
ending the service; explicit Quit remains the terminal desktop action.

### 4. Desktop failure remains contained

A recoverable Tauri startup or event-loop failure may leave the orchestrator serving in headless
fallback, as ADR-0102 already permits. That is a degraded failure outcome, not a normal launch
mode. A successful normal launch owns one tray.

## Consequences

- MCP and browser demand-start cannot select a tray-less production path.
- Human launch and adapter launch converge on one executable behavior.
- Automatic startup does not steal focus because the workbench begins minimized.
- A second direct launch is the deliberate signal to restore and focus the workbench.
- `--headless` remains available when desktop presentation is intentionally impossible or
  unwanted.

## Evidence

- Bridge lifecycle tests prove demand-start supplies no application arguments.
- Orchestrator launch-mode tests prove normal, headless, and scripted intake are the only modes.
- A live Windows journey starts from no authority, verifies one minimized workbench and tray,
  launches Ghostlight again, and verifies that the same authority restores its workbench.
