# Ghostlight data flows

Ghostlight has no vendor-hosted runtime or cloud backend. Browser work, policy decisions, and
audit run on the endpoint.

## Flows that exist

| Flow | Transport | Destination |
| --- | --- | --- |
| MCP client to MCP relay | stdio | Same endpoint. |
| MCP relay to service | A per-user named pipe (Windows) or Unix socket (Linux) that only the same user can open; each end checks the other's user before anything is sent. Typed framing | Same endpoint. |
| Service to browser relay | The same per-user named pipe or Unix socket, with the same checks. Typed framing | Same endpoint. |
| Ghostlight window to service | The same per-user named pipe or Unix socket, with the same checks. It carries the workbench address and the secret that lets the window make changes, requests to show the window at a section, notice titles and bodies, links and folders the person asked to open, and file paths the person picked. All of it stays on this computer, and nothing new leaves it | Same endpoint. |
| Browser relay to extension | Chromium native messaging | Same endpoint. |
| Extension to the controlled page | Chrome DevTools Protocol | The user's existing browser profile. |
| Browser page traffic | Browser networking | Sites the user or MCP client directs the browser to. |
| Audit records | Append-only JSON Lines file | Local path selected by the endpoint owner. |
| Setup page | The extension's own bundled page, opened by the extension on its first connection when Ghostlight is absent, and by the setup buttons in the popup and the options page. It loads nothing from the network | Same endpoint. |
| Setup profile check | Local file reads by the service, only while setup is incomplete. For each installed Chromium browser it reads the `Local State` file for the names of the profiles and checks whether the extension's folder exists in each. Nothing is stored, logged or sent, and the names are shown only in the Ghostlight window | Same endpoint. |
| Agent icons | Local file reads by the service, at its start and on Re-check, only to show each installed agent's own icon. It reads the agents' app files the catalog names: a program's icon resource, and a Store package's manifest and logo (the package is found by the names of the person's package folders and the install folder Windows names) on Windows, a desktop entry and the PNG icon it names in the icon theme on Linux. It keeps a copy in `state/icons` and sends nothing | Same endpoint. |
| Add and Open in a browser | A click in the Ghostlight window makes the service start the named browser, which opens the Chrome Web Store listing for Add, or the profile that holds the extension for Open | The browser then loads the store listing itself. |
| Get Google Chrome | A click in the Ghostlight window opens google.com in the default browser | Google, in the person's browser. |
| Setup command | The command a person pastes in a terminal. Its address is on sylin.org, which answers with a redirect to the script on GitHub; the script downloads the release files from GitHub | The person's own terminal, when the person runs it. |
| npm launcher download | When npm runs the `ghostlight` package (`npx ghostlight`, or an MCP client entry that starts it) and the package is newer than the set installed in `~/.ghostlight/bin`, or nothing is installed, the launcher downloads that release's programs over HTTPS from GitHub's release host, checks each against the checksums the package carries, and installs them; `doctor`, `status` and the help commands run the checked download without installing it. When the installed set is the same or newer, the launcher runs it and downloads nothing. An entry that names an unpinned `ghostlight` therefore updates Ghostlight on a client start once npm serves a newer release; an entry that starts `~/.ghostlight/bin/ghostlight` or names an exact `ghostlight@<version>` does not | GitHub, for the release files. |
| Connection source | Local reads by the service when an agent connects: the names of the programs above the connecting process, at most 16, to name the app that started it in the window, the history and the audit. Nothing is sent | Same endpoint. |
| Suggested first request | The window shows a sentence for the person to give their agent. The agent's browser then opens a demo page on sylin.org, which the site states submits nothing elsewhere, calls out to nothing else and stores nothing | Browser page traffic the person asked for. |
| Managed policy fetch | Local file read or conditional HTTPS GET | Customer-configured source, only when an administrator provisions `managed.json`. |
| Workbench page | HTTP on 127.0.0.1 only, answered only when the Host header is that address and port. While "Others can watch" is on, anyone on this computer can see activity with hosts, client names and policy rules, and can pause agents; the person's never-touch list shows only as a count. Changes need a secret made at each service start, which the service hands out only on its per-user pipe, to the Ghostlight window. A person turns watching off in the Ghostlight window's Settings > Sharing; while it is off, a browser tab shows only the About page | Same endpoint. |

The managed HTTPS source may use a bearer token and organization CA pin. The signed bundle is the
authority: Ed25519 is required, optional ML-DSA-65 makes both legs mandatory, and rollback is
refused by monotonic sequence. No bootstrap means no policy network work.

## Flows that do not exist

- No telemetry, analytics, crash upload, activation callback, or update ping.
- No audit upload, direct syslog delivery, HTTP collector, or listener on any address but loopback.
- No Ghostlight model-provider call. The MCP client owns its model relationship.
- No vendor policy endpoint or embedded vendor policy key.

There is zero vendor-bound traffic. Two Ghostlight-initiated network flows may leave the endpoint:
the explicitly configured managed HTTPS fetch to the customer's own source, and the npm launcher's
download of release files from GitHub when npm runs a package newer than the installed set.
Ordinary site traffic remains browser traffic.

When the extension is installed on a computer where the Ghostlight service is absent, it opens its
own bundled setup page once. That page loads nothing from the network. No vendor page opens, and
nothing Ghostlight does contacts the vendor.

The same holds for the rest of setup. No web page opens without a click: the window opens the
store listing only when a person presses Add, and google.com only on Get Google Chrome. Open starts
the browser in its profile. The browser makes those requests, not Ghostlight. The two sylin.org addresses in the table are used by the
person's own act: the terminal that runs the setup command, and the browser that opens the demo
page when the person asks their agent to. The Ghostlight programs send the vendor nothing in
either case.

## Local artifacts

- `audit.jsonl`, or the path in `GHOSTLIGHT_AUDIT_FILE`, contains content-minimized terminal
  records. Use the endpoint's existing file collector for SIEM delivery.
- There is no discovery file. The workbench listens on one loopback port, which ghostlight status
  shows on its Workbench line; it carries no MCP and starts no browser work. `run/` in the home
  holds the service lock and, on Linux, the relay socket.
- `state/tray-hint` in the home is an empty file. It exists once the window has been closed for
  the first time, so the notification that explains the tray icon is shown once and never again.
  It holds nothing and leaves the computer nowhere.
- `bin/install.json` in the home is the install record: the service version, the relay revision,
  the hashes of the service image and the edge, and the channel (release or development) that
  installed them. It holds nothing about the person, the agent or any page.
- `install.lock` in the home is an empty file. An install holds it while it runs so two installs
  do not overlap. It holds nothing.
- `manifests/org.sylin.ghostlight.json` in the home exists on Windows only. It is the browser
  native-host manifest: the path of the edge and the extension's allowed origin, nothing about
  the person, the agent or any page. On Linux each browser reads its own manifest from its
  configuration directory instead.
- `state/quit` in the home is an empty file, the Quit latch. A person's Quit writes it, and a
  browser or recover start that finds it does not bring Ghostlight back. It holds nothing.
- `state/watch.off` in the home is an empty file that exists exactly while "Others can watch" is
  off. It holds nothing.
- `state/setup.json` in the home records, once, the time of the first agent request that reached a
  browser and succeeded, or the time of the upgrade for a 1.x home already in use. It holds that
  time and nothing about the person, the agent or the page. While it is absent, setup is
  incomplete and the Ghostlight window can show its setup steps.
- `state/last-browser.json` in the home names the installed browser whose extension last
  connected, by one browser id such as `chrome` or `edge`. It holds that id and nothing about the
  person, the profile, the agent or any page, and it leaves the computer nowhere. When several
  browsers are installed, none is connected and `Auto-open browser on request` is on, an agent
  request starts that browser.
- `state/icons` in the home holds a copy of each installed agent's own icon, as
  `<agent id>.png` or `<agent id>.ico`, and beside each copy a manifest, `<agent id>.json`: the
  path of the app file the icon came from, that file's size and modification time, the copy's
  SHA-256, the catalog id of the agent whose app gave it and the kind of place it was read from. It holds nothing about the person, the agent's work or any page, and it leaves the
  computer nowhere. The window shows the copies; deleting the directory brings back the
  monograms until the next look.
- `state/audit-v<version>.jsonl` and `state/audit-pre-<version>.jsonl` in the home are the audit
  logs of 1.x, moved aside when an install finds them (from a 1.x per-version directory in `bin`,
  or from the 1.x log location). They keep the content-minimized records they had and gain no
  new ones. They can be deleted or kept for a collector.
- `state/desktop-webview` in the home is the web-engine profile of the Ghostlight window
  (WebView2 on Windows, WebKitGTK on Linux). It holds the web storage of the workbench page,
  such as the Density and Motion choices, and the engine's own caches. On Linux it is closed to
  other users. It can be deleted while the window is closed.
- Managed deployments keep a verified signed bundle cache and a content-minimized status sidecar.
  The sidecar includes verification, sequence, freshness, source class, timing, organization, and
  contacts, but no source address, bearer token, verification key, or policy rules.
- Browser adapter settings and MCP-client registrations remain in their platform-native local
  stores. On Windows that includes a Microsoft Store app's own copy of its settings below
  `%LOCALAPPDATA%\Packages`, which Ghostlight lists and reads on this computer only.
- On Linux, the audit log, policy, state, run and diagnostics files that Ghostlight creates
  under `~/.ghostlight` are readable only by the user, and the directories it creates there are
  closed to other users. That includes `bin/install.json`, `install.lock`, `state/quit`,
  `state/watch.off`, `state/setup.json`, `state/last-browser.json`, `state/icons`, the archived 1.x logs, and the files an upgrade from 1.x
  copies there. The Windows native-host manifest is not in this list. A file that already existed
  keeps its mode.

The governed host and optional bounded target name are the only page-derived audit text. Audit
does not contain paths, queries, fragments, arbitrary page text, selectors, target handles, typed
values, scripts, screenshots, recordings, or credentials. Retention and deletion are customer
controls.

The September 6 H1 source correction enforces this boundary for failed-flow results and arbitrary
browser error text. Earlier versions could retain those values. Existing files are not rewritten;
see the [H1 implementation record](../tasks/security-hardening/h1-readable-audit.md) for verification
and publication state. The correction must be deployed before relying on it for new records.

See [security-overview.md](security-overview.md),
[the SIEM guide](../guides/siem-integration.md), and
[the governance guide](../guides/governance-configuration.md).

Last reviewed: 2026-10-18 against the current source | Contact: hello@sylin.org
