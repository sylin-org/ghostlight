# Ghostlight 1.0 for a solo developer

Ghostlight gives a compatible local MCP client a visible workspace in the Chromium profile where
you are already signed in. There is no account, Node.js service, telemetry, activation, or remote
control plane.

## Start

1. Install the matching signed 1.0 package and `Ghostlight in Browser` 1.0 adapter when the release
   is available. For the source candidate, follow [`installation.md`](installation.md).
2. Open Ghostlight from the tray.
3. In **Installations**, Check and Install the registration for your MCP harness.
4. Reconnect the harness and ask for the bounded `example.com` proof in the README.

Ghostlight creates one blue group named for that client. If the group already exists in another
normal browser window, new work goes there. If none exists, Ghostlight opens a dedicated window
instead of changing your active personal window.

## Stay in control

- Use the extension popup or `Alt+Shift+P` to pause and resume browser work.
- Use the panic/end-session control when work must stop completely.
- Keep **Preserve controlled tabs** enabled when you want browser evidence to survive a malicious
  or mistaken close request.
- Use the tray workbench for plural activity, payload-free history, health, and high-signal blocked
  notices.
- Ghostlight never enters credentials. A credential-class target becomes a visible handoff to you.

No policy is required for ordinary remote browsing. Loopback and link-local destinations remain
protected. If you want narrower personal boundaries, create the small version-1 file documented in
[`governance-configuration.md`](governance-configuration.md) and set `GHOSTLIGHT_POLICY_FILE`
before launching Ghostlight.

## Pick tools by intent

Let the client use semantic reads and targets before screenshot coordinates. Target handles become
stale after a committed document change; view handles also become stale after viewport or zoom
changes. This refusal is useful evidence, not a reason to reach around Ghostlight with a lower-level
mechanism.

File upload accepts only explicitly supplied absolute paths, at most five files, after governance
and credential preflight. Script execution requires `execute` authority. Neither file content nor
script source enters audit or desktop history.

## When something needs attention

Open **Checkup** for service, browser, authority, audit, and notification state. Open
**Installations** for an exact harness registration check. If transport was lost during an
effectful call, inspect the visible page before taking another action; durable relays reconnect,
but unknown work is never replayed.
