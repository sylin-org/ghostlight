# Ghostlight 1.0 for a solo developer

Ghostlight gives your local MCP client a visible workspace inside the Chromium profile where you
are already signed in. It runs as you, on your machine, and that is the whole arrangement: no
account, no extra service to stand up, no activation.

## Start

1. Install the matching signed 1.0 package and `Ghostlight in Browser` 1.0 adapter when the release
   is available. For the source candidate, follow [`installation.md`](installation.md).
2. Open Ghostlight from the tray.
3. In **MCP integrations**, find your MCP client and choose **Connect**.
4. Reconnect that client and ask for the bounded `example.com` proof in the README.

Ghostlight creates one blue group named for that client. If the group already exists in another
normal browser window, new work goes there. If none exists, Ghostlight opens a dedicated window
instead of changing your active personal window.

## Stay in control

- Use the extension popup or `Alt+Shift+P` to pause and resume browser work.
- Use the panic/end-session control when work must stop completely.
- Keep **Preserve controlled tabs** enabled when you want browser evidence to survive a malicious
  or mistaken close request.
- Open the tray workbench to watch the current action and scroll back through what already
  happened, check health, and see anything that was blocked.
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

Open **Status** for service, browser, authority, and notification state, and **MCP integrations**
to see exactly which clients Ghostlight is registered with. If transport dropped during a call that
had effects, look at the page before you act again. The relays reconnect on their own, but work
whose outcome is unknown is never replayed for you.
