# Installing Ghostlight

Ghostlight installs as one local product: three executables in `~/.ghostlight/bin`
(`ghostlight`, the edge that MCP clients and browsers start; `ghostlight-service`, the service; and
`ghostlight-desktop`, the window and the tray) and a compatible Chromium extension. No account, no
resident launcher service, no admin rights.

This guide describes the 2.0 install and the source-development path. Until 2.0 is published, the
latest published release is the one the [README](../../README.md) names under "Where it stands
today". Every route installs into `~/.ghostlight/bin`. The extension connects by the fixed
native-host name, and the service gives it the page code it runs when it connects, so a service
update needs no extension update. A 1.x extension speaks protocol 3 and must be updated once to the
2.0 extension; until then its popup says Ghostlight version mismatch. A 2.0 extension that meets
an older service says Update needed.

## Installation journey

Install is three steps: This computer, Your browser, Your agent. The Ghostlight window shows them
and what to do next.

1. Install Ghostlight with `npx -y ghostlight install`, or with the one-line installer for your
   system and no Node.js. On Windows, in PowerShell:

   ```powershell
   irm https://sylin.org/ghostlight/get.ps1 | iex
   ```

   On Linux:

   ```sh
   curl -fsSL https://sylin.org/ghostlight/get.sh | sh
   ```

   The checksum-bound launcher downloads one exact set of executables, registers detected native
   browsers for the current user, creates the Linux Applications entry, and connects detected MCP
   clients. When it finishes it prints the version, where Ghostlight lives, which agents have it
   and one Next line, and opens the Ghostlight window on its setup steps. It opens no window with
   `--dry-run`, `--no-open`, in CI or inside an app container.

   WinGet (`winget install Sylin.Ghostlight`) or the Claude Desktop MCPB may deliver the same
   executables instead. With WinGet, run `ghostlight install` once through the WinGet alias: it
   installs into `~/.ghostlight/bin` and registers browsers and clients. After `winget upgrade`,
   running `ghostlight install` again (or any launch through the alias) carries the newer version
   into `~/.ghostlight/bin`. An older copy never replaces a newer one.
2. Add Ghostlight in Browser from the step the window shows. It lists the browsers installed on
   your computer. Add opens the Chrome Web Store listing in that browser (Edge installs the
   extension from the same listing). If a browser already holds the extension but is closed, the
   row reads Open, and Open starts that browser in the right profile. A computer with no supported
   browser is offered Get Google Chrome. The window moves to the next step on its own once the
   extension connects. If the extension is added before Ghostlight is installed, it opens its own
   setup page, which holds the one command to run.
3. Ask your agent: "Open sylin.org/ghostlight/demo/desk and ring the bell." The first request that
   reaches a browser and succeeds completes setup, and the window shows Ready.

If the install named an agent to restart, restart it before you ask; the install says so. There
is no other restart step. "Skip for now" in the window's setup view goes to the workbench, and the
tray's Open brings the setup steps back until setup is done. A browser tab that watches Ghostlight
shows the ordinary workbench.

`npx -y ghostlight doctor` is recovery when something does not connect. It is not another
required setup step, and `ghostlight doctor --fix` opens no window.

After setup there is no startup ritual. Launching the registered MCP client or Chromium
demand-starts Ghostlight if it is absent. The service starts the window's tray whenever the desktop
can show it; an agent that starts Ghostlight shows only the icon, and a Start-menu launch,
Applications or `ghostlight open` shows the window.

Linux packages support x86_64 glibc-based desktops and a native Chrome, Edge, Brave, or Chromium
package. Snap and Flatpak browsers cannot start Ghostlight's browser relay through native
messaging. Install and doctor identify those package forms and name the native-package remedy. Use
`--all-browsers` only when deliberately preparing registrations before browser installation.

Only checksum-bound, provenance-verified packages and a compatible published extension satisfy
this journey. Provenance verification, clean-machine install, upgrade, and uninstall are release
gates, not claims made by this source tree.

The npm process is a download and launch edge, not the product authority. Supported-client
registrations point directly at the installed native `ghostlight` edge with the argument `mcp`. A
bare `npx -y ghostlight` remains the stdio command for another compatible MCP client.

## Workbench installation behavior

MCP integrations supports these explicit user-level configurations:

| Harness | Configuration family |
| --- | --- |
| Codex | `mcp_servers.ghostlight` in TOML |
| Claude Code, Claude Desktop, Cursor, Windsurf | `mcpServers.ghostlight` in JSON or JSONC |
| Visual Studio Code | `servers.ghostlight` in JSON or JSONC |
| Zed | `context_servers.ghostlight` in JSON or JSONC |
| OpenCode | detected v1 `mcp.ghostlight` or v2 `mcp.servers.ghostlight` dialect |
| Crush | `mcp.ghostlight` in JSON or JSONC |
| goose, Continue | Copy setup: you paste the YAML entry into the client's file. Ghostlight never edits it. |

**Check** is read-only. **Install** and **Uninstall** are explicit, serialized operations. They:

- start the installed `ghostlight` edge with the argument `mcp`;
- create the parent directory when preparing a not-yet-detected harness;
- preserve unrelated properties, JSONC comments, trailing commas, TOML comments, and formatting;
- create a `.ghostlight-backup` beside an existing file before replacement;
- are idempotent; and
- refuse malformed, unreadable, or foreign-owned `ghostlight` entries.

Uninstall removes only an entry whose command identifies Ghostlight's edge. An entry written
for an earlier build that names `ghostlight-mcp-connector` is still Ghostlight's: Install
rewrites it, and `ghostlight doctor` names it. It does not
uninstall the harness, delete unrelated empty sections, remove the browser extension, or stop the
orchestrator.

## Build from source

Use Rust 1.89 or newer:

```sh
git clone https://github.com/sylin-org/ghostlight
cd ghostlight
cargo build --workspace
```

A build directory never serves on its own. Deploy the build into `~/.ghostlight/bin` as the
development channel, then work from the installed copy:

```powershell
pwsh scripts/dev-loop.ps1
```

The script builds in its own target directory, installs the three executables the way a release
install does, and starts the service. [`../DEV-LOOP.md`](../DEV-LOOP.md) covers it in full,
including `-Plan` to see what a deploy would do. A deploy replaces the Ghostlight installed for
your user, so run it only on a machine whose Ghostlight is yours, and do not run another product
instance to check a change.

For browser development, load `extension/` unpacked in Chromium 125 or newer. Its pinned key
preserves the established development identity. The platform native-messaging host must point at
the sibling source-built `ghostlight`; use the isolated procedure in
[`../DEV-LOOP.md`](../DEV-LOOP.md) and never replace an installed stack you do not own.

The repository process journey runs the real executables in a fresh test home without modifying a
user installation:

```sh
cargo build --workspace --target-dir .target-ghostlight-1.0
node tests/process-journey.mjs
```

## Verify

Open **Status**. A useful report distinguishes:

- orchestrator runtime state;
- connected browser adapter state;
- local and managed authority validity;
- audit/history readiness; and
- native notification availability through an explicit test.

The terminal equivalent is `ghostlight doctor`; `ghostlight doctor --fix` applies only
ownership-safe repairs. Use `ghostlight install --dry-run` before a scripted rollout.

Open **MCP integrations** and confirm your client reads Connected. Then ask it:

> Open https://example.com in a new Ghostlight tab, summarize the page, and tell me which tab you
> used. Do not click, type, submit, or change the page after it opens.

The browser should create or reuse one blue `Ghostlight - <client label>` group. A new first group
opens in a dedicated normal window rather than disrupting the user's active window.

## Recovery

- **Service is not running:** the MCP edge starts the installed service the next time an agent
  calls, and so does `ghostlight open` or opening Ghostlight from the Start menu or Applications.
  A browser or extension reconnect starts it too, unless the person chose Quit: after Quit only
  an agent, a command or opening Ghostlight brings it back.
- **Workbench window is gone:** open Ghostlight from the Start menu or Applications, run
  `ghostlight open`, or choose Open in the tray. Closing the window leaves the service and the
  tray running.
- **There is no window:** the service and the browser connection work without one, because the
  window is a separate program that draws what the service shows. `ghostlight status` says why in
  one line: no display, the window program missing from `~/.ghostlight/bin`, no web view
  (WebView2 on Windows, WebKitGTK 4.1 on Linux), or no tray. Fix that, then run
  `ghostlight open`. A desktop that shows no tray icons can still open the window from
  Applications or `ghostlight open`.
- **Tools are absent:** re-check the registration in MCP integrations, then reconnect that MCP
  server in its own client.
- **Browser is disconnected:** keep Ghostlight running, enable the matching extension, and inspect
  Status again.
- **A client needs attention:** inspect its configuration. Ghostlight deliberately did not
  overwrite malformed or foreign data.
- **Browser is a Snap or Flatpak:** install Chrome, Edge, Brave, or Chromium as a native package,
  then run `ghostlight doctor --fix`. Ghostlight does not punch through the browser sandbox. The
  install removes the Flatpak connection a 1.x install left in your home, and says what it did.
- **A tab close was blocked:** the tab is retained as visible evidence. Change both applicable
  orchestrator policy and the extension's local preserve-tabs setting only if the user wants
  model-driven close.
- **An in-flight call lost transport:** inspect before taking another effect. Relays reconnect, but
  Ghostlight never replays an unknown effect.

## Update and uninstall

An update replaces the three version-matched executables as one set. It does not silently edit
harness registrations or extension settings. The installer asks the running service to quit,
starts the new one, and the window reopens on the new build; MCP clients reconnect on their own.
Check `ghostlight doctor` for the running service version; `ghostlight --version` reports the
executable on disk and can differ until the old process has exited.

On Windows an install writes the Start-menu shortcut and the installed-apps entry, as every
deploy does, and says each one it changed. `ghostlight uninstall` removes both again, and says each one it
removed. It removes the shortcut only when it starts a program in `~/.ghostlight/bin` or carries the
Ghostlight application id, and the installed-apps entry only when its command or location points into
`~/.ghostlight`; an entry that names anything else, such as one an older installer wrote, is left and
named. An install also adds
`~/.ghostlight/bin` to your user PATH and removes it again on uninstall. `ghostlight` works in terminals opened after the install; one that was already open
needs to be reopened. The machine PATH is never changed.

On Windows the browsers' registration names one manifest, `~/.ghostlight/manifests/org.sylin.ghostlight.json`.
Windows runs the processes of an MSIX-packaged app, Claude desktop and Codex among them, in that
app's container, where registry and `AppData` writes are visible only to that app. An install run
from such a terminal writes everything else and says which registrations it left, the PATH entry,
the Start-menu shortcut and the installed-apps entry among them while they are out of date; run
`~/.ghostlight/bin/ghostlight.exe install` once from a regular terminal to finish them. After that,
installs from inside the container only rewrite the manifest.

Before removing Ghostlight, run `ghostlight uninstall` or use **MCP integrations** to remove Ghostlight
from each agent it was added to.
Then remove the matching browser extension and use the operating system's package uninstall. A
release package must remove only its own native-messaging registration and files; clean-machine
verification of that behavior is required before publication.
