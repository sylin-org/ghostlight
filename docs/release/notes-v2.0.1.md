# Ghostlight 2.0.1

Ghostlight 2.0.1 fixes what the first days of 2.0 found, and adds Alpine and other musl systems.
Update with the same command you installed with: `irm https://sylin.org/ghostlight/get.ps1 | iex`
on Windows, `curl -fsSL https://sylin.org/ghostlight/get.sh | sh` on Linux, or
`npx -y ghostlight install`. Ghostlight in Browser updates itself from the Chrome Web Store to
2.0.1. The 2.0.1 service also works with the 2.0.0 extension until it does; the fixes marked
"extension 2.0.1" below arrive with that update.

### Fixed

- **Windows shutdown.** Ghostlight no longer stops a shutdown, restart or sign-out on Windows. A
  hidden window of the window toolkit answered Windows' question whether the session may end with
  no, so Windows listed `ghostlight-desktop.exe` as preventing shutdown.
- **A dialog left open** (extension 2.0.1). A JavaScript dialog left open in a tab Ghostlight
  controls no longer stops all browser work when the service reconnects or updates, or when
  control pauses or releases that tab. The tab catches up when its dialog closes, and the dialog
  can still be read and answered meanwhile. With the 2.0.0 extension, if agents stop getting
  answers from the browser after this update, answer or cancel the dialog that is open on the page,
  and work resumes. The dialog may show only once you switch back to its tab. Closing the dialog's
  tab or window instead does not end the stall; restart the browser then.
- **Back and forward** (extension 2.0.1). `browser_history` back and forward go to the previous or
  next page of the tab's history, even when the agent moved between pages with
  `browser_navigate` alone. With no page that way, the call says so and sends nothing.
- **Clients that fill in defaults.** A client that fills in a tool's advertised defaults before it
  sends a call, as Claude Code began to, is no longer refused for opening a new tab, going back or
  forward, or scrolling to a target.
- **The installers need nothing from GitHub but the files.** The one-line installers no longer run
  the GitHub CLI. In Windows PowerShell 5.1, a GitHub CLI that was installed but not signed in
  stopped `get.ps1` before it installed anything. Each image is checked against the release's
  `SHA256SUMS`, and docs/trust/supply-chain.md shows how to check build provenance by hand. The
  short links have served this fix since 2026-10-20.
- **Agents in another app's container.** On Windows, the install no longer registers an agent
  inside another app's Microsoft Store container. With Visual Studio Code installed but never
  started, the install wrote Ghostlight's entry into the Codex app's private copy of VS Code's
  settings, which VS Code never reads, and called VS Code connected.

- **One huge answer no longer drops every agent's browser.** A read, inspect or find across a page
  with many frames could pass the relay's 8 MiB bound and close the browser connection for every
  agent. The service now returns what fits, marked truncated with what it left out (extension
  2.0.1 also refuses to send such an answer, with a next step).
- **The missing extension.** A browser started without Ghostlight in Browser, or with it turned off,
  says so and how to add or turn it on.
- **Emoji in page text and tab titles.** Text is cut by characters and never inside an emoji, and
  a cut that left half an emoji no longer drops the browser connection.
- **Agents after a quit, and agents that end their process tree.** An agent waiting after a quit
  connects as soon as the service is back, and an agent that ends its process tree no longer
  takes the service with it.
- **Strict clients and older clients.** A JSON-RPC batch is answered as one array, and a client
  older than MCP 2025-06-18 gets the result's JSON in a cut-off or expired answer too. A misspelled
  key inside a nested object is refused instead of ignored.
- **A call too large to send** is answered instead of left waiting. A call over the 8 MiB the
  relay carries, such as an inline upload of about 6.3 MB or more, says nothing was sent and to
  send less in one call.
- **`ghostlight call`** from an agent joins one session named for that agent.
- **Organization policy.** Publishing refuses an `http://` source before it writes anything, a
  long non-ASCII organization name no longer locks a machine out, and the same publication in
  another line ending no longer reads as stale.

### Changed

- **Tab groups** (extension 2.0.1) are named with a ghost emoji and the agent's name, so the agent's name comes
  first in the tab strip. A group an earlier version named is renamed in place.
- **A prompt's field** (extension 2.0.1). `browser_dialog` status says what a prompt's field
  holds, and that accept answers with it, as OK does, so an agent can accept a prompt without
  asking. The text is never audited.
- **Tab and window actions** say what they do: focus, close and resize had no description.
- **The Claude Desktop bundle continues.** A `.mcpb` ships with this release too, instead of
  ending with 2.0.0, so a person who installs Ghostlight through it gets every fix.

### Added

- **Alpine and other musl systems.** The release carries `x86_64-unknown-linux-musl` images, and
  `get.sh` and the npm launcher pick them on a musl system. On Alpine, install Chromium with `apk`;
  `get.sh` says what the window needs when its libraries are missing.

### Security

- A host written with a trailing dot (`bank.example.`) no longer steps around a never-touch
  entry or a deny rule.
- TLS certificate checks take rustls-webpki 0.103.16 and webpki-roots 1.0.10, for RUSTSEC-2026-0382
  and RUSTSEC-2026-0381 (a distrusted root). The organization-policy fetch moves to ureq 3, which
  uses those roots directly, so no advisory is ignored.

### Compatibility

The 2.0.1 edge speaks relay protocol 5, which is protocol 4 with the wake and the too-large answer;
the 2.0.1 service admits both, so an agent still running a 2.0.0 edge keeps working. The extension speaks adapter protocol 4,
with two words added that an older reader ignores. A 2.0.0 service and a 2.0.1 extension work
together, and so do a 2.0.1 service and a 2.0.0 extension. The full list of changes is in the [CHANGELOG](../../CHANGELOG.md).
