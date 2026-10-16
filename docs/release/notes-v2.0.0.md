# Ghostlight 2.0.0

Ghostlight 2.0.0 is a new install and a new shape for the same product. Install takes three steps
and ends in a window that shows you where you are. One service runs the product for each user, one
small executable is the front door for every agent and browser, and the Chromium extension is
frozen at its 2.0 form. It is a breaking release: the service and the extension must be updated
together, and 1.x installs move once.

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
  there whenever your desktop can show one.
- **One home.** Everything lives under `~/.ghostlight`. Three executables sit in
  `~/.ghostlight/bin`: `ghostlight` (the edge), `ghostlight-service` and `ghostlight-desktop` (the
  window). An update replaces them as one tested set. There is no startup ritual: launching a
  registered MCP client or browser starts Ghostlight if it is not running. After you Quit, only an
  agent, a command or opening Ghostlight starts it again.
- **The edge.** `ghostlight` is the one executable an MCP client or a browser starts. It relays
  and starts the service when it is missing, and it decides nothing. The service owns the MCP
  server, the tools and their words, governance, the audit trail and every state change. The
  edge and the relay protocol it speaks are frozen at relay revision 4 and change only with their
  own protocol.
- **A service update needs no extension update.** The service gives the extension the page code it
  runs when it connects. The extension carries no policy and no page code of its own.
- **A session works in its own tabs.** The `browser_workspace` tool is gone, and the catalog has
  23 tools. A session cannot list other sessions or switch into another session's workspace. A
  call to the old tool fails before any browser work and points to `browser_tabs`. Ghostlight does
  not adopt a tab on its own: `browser_tabs` lists tabs opened from a tab the session owns, and
  the model may adopt one explicitly under the same governance as navigation.
- **Calls wait out a short absence.** A call no longer fails while the browser is briefly away.
  When the browser is reconnecting, still starting, or the service has just restarted, the call
  waits up to about seven seconds and then goes ahead.
- **Handles follow their control.** A handle survives a re-render that Ghostlight can prove is the
  same control, and a stale handle says why it is stale. Every tool accepts `timeout_ms`.

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
- The popup's diagnostics toggle, the form-reset trace, the YAML editor for goose and Continue
  (they get a Copy setup action) and observed peer identity.

WinGet stays as a carrier. Run `ghostlight install` once through the WinGet alias; it installs into
`~/.ghostlight/bin`, and after `winget upgrade` the next install carries the newer version there.
An older copy never replaces a newer one.

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

The install replaces the Windows setup program's Start-menu shortcut and its Installed apps entry
with Ghostlight 2.0's own, so there is no old uninstaller to run afterwards. A Debian package
leaves `/usr/bin/ghostlight` behind; remove it with `sudo apt remove` so that it cannot shadow the
new command. Scoop and the portable archive are not cleaned up either: remove them with
`scoop uninstall` or by deleting their folder.

For the short time between updating one side and the other, Ghostlight tells you which side to
update instead of working half-way.

### Compatibility

Supported browsers are Chrome, Edge, Brave and Chromium, and Chromium 125 or newer is required.
Edge installs the extension from the same Chrome Web Store listing. The extension is version 2.0.0
and speaks adapter protocol 4. The service and the window are version 2.0.0; the edge and the relay
are version 4.0.0, the relay revision, and keep their numbers across service releases.

### Verify

`SHA256SUMS` binds every uploaded artifact, and GitHub build provenance binds those bytes to this
repository's release workflow and source revision:

```sh
gh attestation verify <downloaded-file> \
  --repo sylin-org/ghostlight \
  --signer-workflow sylin-org/ghostlight/.github/workflows/release.yml
```

Ghostlight does not phone home. The release is not Authenticode-signed; checksums and the
provenance attestations are the integrity controls.
