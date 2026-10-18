# Ghostlight 2.0.0-rc.1

This is the release candidate of Ghostlight 2.0.0, published as a GitHub prerelease so that
reviewers and testers can install 2.0 before it is public. It is the 2.0.0 build, and `v2.0.0` is
cut from the same commit. The short install commands, `npx ghostlight` and the Chrome Web Store
keep serving 1.x until the cutover.

### Install the candidate

Windows PowerShell 5.1 or PowerShell 7, in a regular terminal:

```powershell
$env:GHOSTLIGHT_VERSION = "2.0.0-rc.1"; irm https://raw.githubusercontent.com/sylin-org/ghostlight/v2.0.0-rc.1/scripts/get.ps1 | iex
```

A Linux shell:

```sh
curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/v2.0.0-rc.1/scripts/get.sh | GHOSTLIGHT_VERSION=2.0.0-rc.1 sh
```

The install says `Ghostlight 2.0.0-rc.1 downloaded`, because that is the release it fetched, and
its summary and `ghostlight --version` then say `2.0.0`. That is the right build.

### The extension

The Chrome Web Store serves the 1.x extension until the cutover, and a 1.x extension does not
connect to a 2.0 service. The window's Add button opens that store listing. To try 2.0 end to end,
download the `v2.0.0-rc.1` source, open `chrome://extensions`, turn on Developer mode, choose Load
unpacked and pick its `extension` folder, then turn off the store copy of Ghostlight in Browser.
The folder's pinned key gives it the id the install registers, so it connects.

### Tab groups

When a session ends, the tabs Ghostlight keeps open leave the `Ghostlight - <client>` group and
stay open. That is on purpose: Chromium saves a group that is still open when its window closes or
the browser exits, and the groups piled up in the bookmarks bar. When the same client reuses one
of those tabs, it goes back into the group (ADR-0201).

### If you use Ghostlight 1.x

- If you added the 1.x Ghostlight extension to Claude Desktop, remove it or install the 2.0
  `.mcpb` from this release. Claude Desktop runs a bundle's installer on every start, so the 1.x
  bundle would put 1.x back.
- Do not run `npx ghostlight` until the cutover, and remove any agent entry you wrote that starts
  `npx -y ghostlight`. npm still serves 1.3.14, which would put 1.x back over 2.0 and start a
  second service. The agents the 2.0 install sets up start `~/.ghostlight/bin/ghostlight` directly.

The 2.0.0 release notes follow.

## Ghostlight 2.0.0

Ghostlight 2.0.0 is a new install and a new shape for the same product. Install takes three steps
and ends in a window that shows you where you are. One service runs the product for each user, one
small executable is the front door for every agent and browser, and the Chromium extension is
version 2.0.0. It is a breaking release: the service and the extension must be updated together,
and 1.x installs move once.

### What changes for you

- **Three steps.** Install Ghostlight, add Ghostlight in Browser, then ask your agent to do
  something. Install with `npx -y ghostlight install`, or with the one-line installer for your
  system and no Node.js (`irm https://sylin.org/ghostlight/get.ps1 | iex` on Windows,
  `curl -fsSL https://sylin.org/ghostlight/get.sh | sh` on Linux). When it finishes it prints the
  version, where Ghostlight lives, which agents have it and one next step, and opens the Ghostlight
  window on the setup steps.
- **A window.** The window shows This computer, Your browser and Your agent, and moves to the next
  step on its own. The Browser step lists the browsers installed on your computer: Add opens the
  Chrome Web Store listing in that browser, and Open starts a browser that already holds the
  extension. After setup the window is the workbench: activity, history, policy and Settings.
  `ghostlight open` or the Start menu or Applications entry brings it back, and the tray icon is
  there whenever your desktop can show one. Each agent appears with its own app icon, read from
  the app on your computer. The window, the tray and `ghostlight status` count agents and browsers
  the same way and say Pause agents and Resume agents; the tray's tooltip says when no browser is
  connected. Settings > This screen offers a larger text size.
- **One home.** Everything lives under `~/.ghostlight`. Three executables sit in
  `~/.ghostlight/bin`: `ghostlight` (the edge), `ghostlight-service` and `ghostlight-desktop` (the
  window). An update replaces them as one tested set.
- **The command on your PATH.** On Windows the install adds `~/.ghostlight/bin` to your user PATH,
  so `ghostlight` works in terminals you open afterwards. On Linux it links
  `~/.local/bin/ghostlight` and adds a manual page and shell completions; when `~/.local/bin` is
  not on your PATH, the install says the full path to run instead. Uninstall removes them.
- **No startup ritual.** Launching a registered MCP client or browser starts Ghostlight if it is
  not running. After you Quit, only an agent, a command or opening Ghostlight starts it again. A
  service with nothing connected to it, the window included, exits after 10 minutes, so one an
  agent started over SSH does not linger.
- **The browser you use starts on request.** With several browsers installed and none open, an
  agent request starts the one that last connected to Ghostlight, instead of asking you to open
  one. Ghostlight remembers only which browser that was. A session whose browser you closed starts
  that same browser again. `Auto-open browser on request` off still never starts one. When the
  service runs inside a Store app such as Claude Desktop or Codex, it leaves starting the browser
  to you, because a browser started there would run inside that app.
- **Others can watch.** The service serves the window's pages on `127.0.0.1` only. While "Others
  can watch" is on, which is the default, anyone on this computer can open that address in a
  browser tab, see what agents are doing and where, the client names and the policy rules, and
  pause agents. Every other change needs the Ghostlight window. Turn it off in Settings > Sharing;
  `ghostlight status` shows the address and whether watching is on.
- **The edge.** `ghostlight` is the one executable an MCP client or a browser starts. It relays
  and starts the service when it is missing, and it decides nothing. The service owns the MCP
  server, the tools and their words, governance, the audit trail and every state change. The
  edge and the relay protocol it speaks are at relay revision 4 and change only with their own
  protocol.
- **A service update needs no extension update.** The service gives the extension the page code it
  runs when it connects. The extension carries no policy and no page code of its own.
- **The popup.** It reads Ready, Paused, Session ended, Not connected or Update needed. Paused and
  Session ended show the held lantern, so neither looks like Ready, and while connected an Open
  Ghostlight link brings the window forward.
- **A session works in its own tabs.** The `browser_workspace` tool is gone, and the catalog has
  23 tools. A session cannot list other sessions or switch into another session's workspace. A
  call to the old tool fails before any browser work and points to `browser_tabs`. Ghostlight does
  not adopt a tab on its own: `browser_tabs` lists tabs opened from a tab the session owns, and
  the model may adopt one explicitly under the same governance as navigation.
- **A call with no tab never guesses.** With several tabs in a session, a call that names no tab
  is refused before any browser work and lists the tabs with their handles. Subagents that share
  one client's connection can no longer act in each other's tabs by leaving `tab` out.
- **Tab groups stay tidy.** When a session ends, the tabs Ghostlight keeps open leave the
  `Ghostlight - <client>` group, so Chromium deletes the idle group instead of saving it to your
  bookmarks bar. When the same client reuses one of those tabs, it goes back into the group. A
  session still open when its window closes or the browser exits leaves one saved group; groups
  saved earlier stay until you remove them.
- **Each connection names its app.** The window, the history and the audit say which app started
  each agent connection, read from the operating system, beside the name the client gives. It
  never decides what an agent may do.
- **Calls wait out a short absence.** A call no longer fails while the browser is briefly away.
  When the browser is reconnecting, still starting, or the service has just restarted, the call
  waits up to about seven seconds and then goes ahead.
- **Handles follow their control.** A handle survives a re-render that Ghostlight can prove is the
  same control, and a stale handle says why it is stale. Every tool accepts `timeout_ms`. A
  control the page refuses (hidden, disabled, read-only, or one that takes no text) is refused in
  Ghostlight's words, with one step that works.
- **Agents' settings are found where they are.** Ghostlight sets up an agent wherever it keeps its
  settings, a Microsoft Store app's own copy included. Run from such an app's terminal, `doctor`
  says the browsers cannot be checked from there instead of calling them broken.
- **Uninstall says what it did.** `ghostlight uninstall` stops Ghostlight, removes what it
  registered, and says what it removed and what it kept. `~/.ghostlight` stays; delete it to remove
  the programs, the audit history and the settings. `--dry-run` lists what it would remove.

### Governance

- Policy files use schema 4. `ghostlight policy migrate <file>` rewrites a schema-3 file only where
  the meaning stays the same, keeps a `.schema3.bak` copy and refuses anything it cannot move
  without changing what the policy does. Ghostlight moves its own user policy the same way.
  Policy files an administrator owns are never rewritten; they fail closed in schema 3 until their
  owner moves them.
- Observe mode, the channel enable switches, the user-policy enable switch and the frame options
  are removed. The managed policy keeps its HTTPS fetch, signatures and rollback protection; the
  fetch runs on the refresher thread, never on the path of a call.
- The 1.x audit logs are moved aside under `state` and are not displayed. They keep the records
  they had and gain no new ones.

### Removed

- The two connector executables, replaced by the single edge.
- The Windows setup program (NSIS), the Debian package, Scoop, the separate portable archive and
  the Homebrew template. The routes are `npx -y ghostlight install`, the one-line installers and
  WinGet.
- The Flatpak route. Install removes the Flatpak connection a 1.x install left in your home and
  says what it did.
- MCPB as a channel after this release. The Claude Desktop bundle in this release is the last one,
  and it carries 2.0.0; Claude Desktop runs its bundled installer on every start, so the
  bundle moves existing users to 2.0. After this release use the npm launcher or the one-line
  installer.
- The popup's diagnostics toggle, the form-reset trace and the YAML editor for goose and Continue
  (they get a Copy setup action).

WinGet stays as a carrier. Run `ghostlight install` once through the WinGet alias; it installs into
`~/.ghostlight/bin`, and after `winget upgrade` the next install carries the newer version there.
An older copy never replaces a newer one. WinGet carries 2.0 once its 2.0.0 manifest is accepted
into winget-pkgs; until then `winget install Sylin.Ghostlight` installs a 1.x release.

### Upgrading from 1.x

The service and the extension speak a new adapter protocol. A 2.0 service refuses a 1.x extension,
and a 2.0 extension tells you to update Ghostlight when it meets an older service. Update both, in
this order:

1. Run the install again (`npx -y ghostlight install`, or the one-line installer). The install
   carries out the one 2.0 break migration, then stops the old programs and starts the new service.
   It moves your user policy, your managed-policy files and your client locations to the fixed
   home, writes the setup record for a home already in use, archives the 1.x audit log and stops
   the 1.x service. It copies before it removes, and `ghostlight deployment install --plan` prints
   what it will do without changing anything.
2. Update Ghostlight in Browser from the Chrome Web Store (Chrome updates extensions on its own
   schedule; a manual update from the extensions page is faster). Until then the 1.x popup reads
   Ghostlight version mismatch.
3. If you wrote policy files yourself, run `ghostlight policy migrate` on each (see Governance).

If you added the 1.x Ghostlight extension to Claude Desktop, remove it or install the 2.0 `.mcpb`
from this release. Claude Desktop runs a bundle's installer on every start, so a 1.x bundle would
put 1.x back.

The install replaces the Windows setup program's Start-menu shortcut and its Installed apps entry
with Ghostlight 2.0's own, so there is no old uninstaller to run afterwards. It also removes the
folders 1.x left that 2.0 reads nothing from: the 1.x window's web view data and the emptied 1.x
policy folder, and it names each one. A Debian package leaves `/usr/bin/ghostlight` behind; remove
it with `sudo apt remove` so that it cannot shadow the new command. Scoop and the portable archive
are not cleaned up either: remove them with `scoop uninstall` or by deleting their folder.

For the short time between updating one side and the other, Ghostlight tells you which side to
update instead of working half-way.

### Compatibility

Supported browsers are Chrome, Edge, Brave and Chromium, and Chromium 125 or newer is required.
Edge installs the extension from the same Chrome Web Store listing. The extension is version 2.0.0
and speaks adapter protocol 4. The service and the window are version 2.0.0; the edge and the relay
are version 4.0.0, the relay revision, and keep their numbers across service releases.

### Verify

`SHA256SUMS` binds every uploaded artifact. Download it beside the files you fetched and check
them; on Windows, compare `(Get-FileHash <file> -Algorithm SHA256).Hash` with the file's line:

```sh
sha256sum -c SHA256SUMS --ignore-missing
```

GitHub build provenance binds those bytes to this repository's release workflow and source
revision:

```sh
gh attestation verify <downloaded-file> \
  --repo sylin-org/ghostlight \
  --signer-workflow sylin-org/ghostlight/.github/workflows/release.yml
```

Ghostlight does not phone home. The release is not Authenticode-signed; checksums and the
provenance attestations are the integrity controls.
