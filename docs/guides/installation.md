# Installing Ghostlight

Ghostlight installs as one local product: three executables in `~/.ghostlight/bin`
(`ghostlight`, the edge that MCP clients and browsers start; `ghostlight-service`, the service; and
`ghostlight-desktop`, the window and the tray) and a compatible Chromium extension. No account, no
resident launcher service, no admin rights.

This guide describes the 2.0 install and the source-development path. The latest
published release is the one the [README](../../README.md) names under "Where it stands
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
   `--dry-run`, `--no-open`, in CI or inside an app container. It needs no GitHub account and no
   GitHub tool: every executable is checked against the release's SHA-256 checksums before it runs.
   [Verify a release](../trust/supply-chain.md#verify-a-release) shows how to check build
   provenance by hand.

   WinGet (`winget install Sylin.Ghostlight`) or the Claude Desktop MCPB may deliver the same
   executables instead. With WinGet, run `ghostlight install` once through the WinGet alias: it
   installs into `~/.ghostlight/bin` and registers browsers and clients. After `winget upgrade`,
   running `ghostlight install` again (or any launch through the alias) carries the newer version
   into `~/.ghostlight/bin`. An older copy never replaces a newer one. WinGet carries 2.0 once its
   2.0.0 manifest is accepted into winget-pkgs; until then it installs a 1.x release, so use the
   one-line installer.
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

`ghostlight doctor` is recovery when something does not connect. It is not another required setup
step, and `ghostlight doctor --fix` opens no window. After an npm install, `npx -y ghostlight
doctor` runs the same check. On Linux the command is a link in `~/.local/bin`; when that directory
is not on the PATH the install ran with, the install names the full path to run instead.

After setup there is no startup ritual. Launching the registered MCP client or Chromium
demand-starts Ghostlight if it is absent. The service starts the window's tray whenever the desktop
can show it; an agent that starts Ghostlight shows only the icon, and a Start-menu launch,
Applications or `ghostlight open` shows the window.

Ghostlight supports the stable channel of each browser it lists. Beta, Dev and Canary channels
install separately with their own profiles, and Ghostlight does not support them.

Linux binaries support x86_64 desktops with either C library: glibc, which most distributions use,
and, from 2.0.1, musl, which Alpine uses. The installer and the npm launcher pick the images for
the system's C library. The musl edge and service are static and run on any musl system. The musl
window is built on Alpine and needs its WebKitGTK 4.1, and the tray needs AppIndicator: on Alpine,
`doas apk add webkit2gtk-4.1 libayatana-appindicator`. Without them the service, the browser
connection and the agents still work, and `get.sh` says what the window needs. Every Linux
system needs a native Chrome, Edge, Brave, or Chromium package; on Alpine, `chromium`. Snap and
Flatpak browsers cannot start Ghostlight's browser relay through native messaging. Install and doctor identify those package forms and name the native-package remedy. Use
`--all-browsers` only when deliberately preparing registrations before browser installation.

`ghostlight install --client <id>` sets up only the agents you name: an agent's id selects each
of its hosts that Ghostlight detects (`--client cline` selects whichever of Cline's CLI and its
Visual Studio Code, Cursor and Windsurf hosts are installed, and all four with `--all-clients`), a host id such as
`cline-vscode` selects that one host, and `ghostlight --help` lists them.

Only checksum-bound, provenance-verified binaries and a compatible published extension satisfy
this journey. Provenance verification, clean-machine install, upgrade, and uninstall are release
gates, not claims made by this source tree.

The npm process is a download and launch edge, not the product authority. Supported-client
registrations point directly at the installed native `ghostlight` edge with the argument `mcp`. A
bare `npx -y ghostlight` remains the stdio command for another compatible MCP client.

## Agents Ghostlight sets up

Agent setup supports Windows and Linux; macOS is not supported yet.

`ghostlight install` and the **Integrations** page set up every agent in Ghostlight's catalog, on
each host the agent runs on, in the user-level configuration the table names. The Entry column is
where Ghostlight's entry sits in that file. OpenCode's entry keeps the form its file already uses,
or takes the newer one when only `opencode2` is installed. Copy setup means Ghostlight never edits
that file: you paste the YAML entry it shows into the file yourself. `--client` takes a host id or
an agent's id; an agent's id is shown beside its name when it is none of its host ids.

<!-- BEGIN agent list -->
<!-- Generated from the agent catalog by install::agent_list. Rewrite it with GHOSTLIGHT_AGENT_LIST_UPDATE=1 cargo test -p ghostlight-service agent_list -->

The catalog holds 20 agents on 23 hosts.

| Agent | Host id | Target | Entry | Windows | Linux |
| --- | --- | --- | --- | --- | --- |
| Codex | `codex` | User | `mcp_servers.ghostlight` (TOML) | `%CODEX_HOME%\config.toml` when CODEX_HOME is set, else `%USERPROFILE%\.codex\config.toml` | `$CODEX_HOME/config.toml` when CODEX_HOME is set, else `~/.codex/config.toml` |
| Claude Code | `claude-code` | User | `mcpServers.ghostlight` (JSON) | `%CLAUDE_CONFIG_DIR%\.claude.json` when CLAUDE_CONFIG_DIR is set, else `%USERPROFILE%\.claude.json` | `$CLAUDE_CONFIG_DIR/.claude.json` when CLAUDE_CONFIG_DIR is set, else `~/.claude.json` |
| Claude Desktop | `claude-desktop` | User | `mcpServers.ghostlight` (JSON) | `%APPDATA%\Claude\claude_desktop_config.json` | `$XDG_CONFIG_HOME/Claude/claude_desktop_config.json` |
| Cursor | `cursor` | User | `mcpServers.ghostlight` (JSON) | `%USERPROFILE%\.cursor\mcp.json` | `~/.cursor/mcp.json` |
| Visual Studio Code | `vscode` | User | `servers.ghostlight` (JSON) | `%APPDATA%\Code\User\mcp.json` | `$XDG_CONFIG_HOME/Code/User/mcp.json` |
| Windsurf | `windsurf` | User | `mcpServers.ghostlight` (JSON) | `%USERPROFILE%\.codeium\windsurf\mcp_config.json` | `~/.codeium/windsurf/mcp_config.json` |
| Devin | `devin` | Devin user | `mcpServers.ghostlight` (JSON) | `%APPDATA%\devin\mcp_config.json` | `$XDG_CONFIG_HOME/devin/mcp_config.json` |
| Zed | `zed` | User | `context_servers.ghostlight` (JSON) | `%APPDATA%\Zed\settings.json` | `$XDG_CONFIG_HOME/zed/settings.json` |
| ZCode | `zcode` | User | `mcp.servers.ghostlight` (JSON) | `%USERPROFILE%\.zcode\cli\config.json` | `~/.zcode/cli/config.json` |
| OpenCode | `opencode` | User | `mcp.ghostlight` or `mcp.servers.ghostlight` (JSON) | `%USERPROFILE%\.config\opencode\`: the first found of `opencode.jsonc`, `opencode.json`; else a new `opencode.json` | `~/.config/opencode/`: the first found of `opencode.jsonc`, `opencode.json`; else a new `opencode.json` |
| Crush | `crush` | User | `mcp.ghostlight` (JSON) | `%CRUSH_GLOBAL_CONFIG%` when CRUSH_GLOBAL_CONFIG is set, else `%USERPROFILE%\.config\crush\crush.json` | `$CRUSH_GLOBAL_CONFIG` when CRUSH_GLOBAL_CONFIG is set, else `~/.config/crush/crush.json` |
| GitHub Copilot CLI | `copilot-cli` | CLI | `mcpServers.ghostlight` (JSON) | `%USERPROFILE%\.copilot\mcp-config.json` | `~/.copilot/mcp-config.json` |
| Cline (`cline`) | `cline-cli` | CLI | `mcpServers.ghostlight` (JSON) | `%USERPROFILE%\.cline\data\settings\cline_mcp_settings.json` | `~/.cline/data/settings/cline_mcp_settings.json` |
| Cline (`cline`) | `cline-vscode` | Visual Studio Code | `mcpServers.ghostlight` (JSON) | `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json` | `$XDG_CONFIG_HOME/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` |
| Cline (`cline`) | `cline-cursor` | Cursor | `mcpServers.ghostlight` (JSON) | `%APPDATA%\Cursor\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json` | `$XDG_CONFIG_HOME/Cursor/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` |
| Cline (`cline`) | `cline-windsurf` | Windsurf | `mcpServers.ghostlight` (JSON) | `%APPDATA%\Windsurf\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json` | `$XDG_CONFIG_HOME/Windsurf/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` |
| Kiro | `kiro` | CLI and IDE | `mcpServers.ghostlight` (JSON) | `%USERPROFILE%\.kiro\settings\mcp.json` | `~/.kiro/settings/mcp.json` |
| Qwen Code | `qwen-code` | CLI | `mcpServers.ghostlight` (JSON) | `%USERPROFILE%\.qwen\settings.json` | `~/.qwen/settings.json` |
| Junie | `junie` | CLI and JetBrains | `mcpServers.ghostlight` (JSON) | `%USERPROFILE%\.junie\mcp\mcp.json` | `~/.junie/mcp/mcp.json` |
| Kilo Code | `kilo-code` | CLI | `mcp.ghostlight` (JSON) | `%APPDATA%\kilo\`: the first found of `kilo.jsonc`, `kilo.json`, `opencode.jsonc`, `opencode.json`; else a new `kilo.json` | `$XDG_CONFIG_HOME/kilo/`: the first found of `kilo.jsonc`, `kilo.json`, `opencode.jsonc`, `opencode.json`; else a new `kilo.json` |
| goose | `goose` | CLI and desktop | Copy setup (YAML) | `%APPDATA%\goose\config.yaml` | `$XDG_CONFIG_HOME/goose/config.yaml` |
| Continue | `continue` | CLI and IDE | Copy setup (YAML) | `%USERPROFILE%\.continue\config.yaml` | `~/.continue/config.yaml` |
| Antigravity | `antigravity` | CLI | `mcpServers.ghostlight` (JSON) | `%USERPROFILE%\.gemini\config\mcp_config.json` | `~/.gemini/config/mcp_config.json` |

Where XDG_CONFIG_HOME is not set it is `~/.config`, and where APPDATA is not set it is `%USERPROFILE%\AppData\Roaming`.
<!-- END agent list -->

On Windows an agent installed from the Microsoft Store, such as Claude Desktop, keeps its settings
in its own folder instead of the `%APPDATA%` path the table names: for Claude Desktop,
`%LOCALAPPDATA%\Packages\Claude_<publisher id>\LocalCache\Roaming\Claude\claude_desktop_config.json`.
Ghostlight looks there every time it checks, sets that file up, and says so on the **Integrations**
page and in `ghostlight doctor`. Until the Store edition writes that file, it reads the plain
`%APPDATA%` file, so Ghostlight sets up that one instead. Once the Store file exists, a plain file
left from an earlier install is not edited. `ghostlight uninstall` removes Ghostlight's own entry
from both, also when only the plain file still holds one.

## Workbench installation behavior

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

## Test a build before its release

The release workflow builds and checks a release candidate without tagging or publishing anything
([`../RELEASE.md`](../RELEASE.md)). To install that build the way a person installs a release, run
the real one-line installer against the workflow's bundle:

1. Download the run's `release-candidate` artifact and unpack it, for example with
   `gh run download <run id> -n release-candidate -D release-candidate`. The folder holds
   `SHA256SUMS`, `release-candidate.json` and `assets`.
2. Run the installer from the bundle's own revision (`sourceRevision` in `release-candidate.json`),
   with `GHOSTLIGHT_FROM` naming the folder. On Linux:

   ```sh
   curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/<revision>/scripts/get.sh | GHOSTLIGHT_FROM=~/Downloads/release-candidate sh
   ```

   On Windows, in PowerShell:

   ```powershell
   $env:GHOSTLIGHT_FROM = "$HOME\Downloads\release-candidate"; irm https://raw.githubusercontent.com/sylin-org/ghostlight/<revision>/scripts/get.ps1 | iex
   ```

The installer copies each executable from the folder and checks it against `SHA256SUMS` exactly as
it checks a download. It refuses an image that `SHA256SUMS` does not bind, and it fetches nothing
from GitHub. A folder of downloaded release assets, with `SHA256SUMS` beside the images, works the same way.
`GHOSTLIGHT_FROM` cannot be combined with `GHOSTLIGHT_VERSION`.

Until the 2.0 extension is in the Chrome Web Store, load the repository's `extension` folder at the
bundle's revision unpacked (Developer mode, Load unpacked in `chrome://extensions`). It carries the
development key, whose extension ID the installed browser registration allows. The bundle's
extension ZIP is the store upload and has no key, so an unpacked copy of it gets another ID, which
the registration refuses.

## Verify

Open **Status**. A useful report distinguishes:

- orchestrator runtime state;
- connected browser adapter state;
- local and managed authority validity;
- audit/history readiness; and
- native notification availability through an explicit test.

The terminal equivalent is `ghostlight doctor`; `ghostlight doctor --fix` applies only
ownership-safe repairs. Use `ghostlight install --dry-run` before a scripted rollout.

Open **Integrations** and confirm your client reads Connected. Then ask it:

> Open https://example.com in a new Ghostlight tab, summarize the page, and tell me which tab you
> used. Do not click, type, submit, or change the page after it opens.

The browser should create or reuse one blue group named for your agent as the Ghostlight window
names it, such as `Ghostlight - Claude Code`. A new first group opens in a dedicated normal window
rather than disrupting the user's active window. After an update from an earlier build, each agent
gets one new group; the old one goes once it is idle, and a group Chromium already saved keeps its
old title until you remove it.

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
- **Tools are absent:** re-check the registration in Integrations, then reconnect that MCP
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
deploy does. `ghostlight uninstall` removes both again. It removes the shortcut only when it starts
a program in `~/.ghostlight/bin` or carries the Ghostlight application id, and the installed-apps
entry only when its command or location points into `~/.ghostlight`; an entry that names anything
else, such as one an older installer wrote, is left and named. An install also adds
`~/.ghostlight/bin` to your user PATH and removes it again on uninstall. `ghostlight` works in
terminals opened after the install; one that was already open needs to be reopened. The machine
PATH is never changed.

On Windows the browsers' registration names one manifest, `~/.ghostlight/manifests/org.sylin.ghostlight.json`.
Windows runs the processes of an MSIX-packaged app, Claude desktop and Codex among them, in that
app's container, where registry and `AppData` writes are visible only to that app. An install run
from such a terminal writes everything else and says which registrations it left, the PATH entry,
the Start-menu shortcut and the installed-apps entry among them while they are out of date; run
`~/.ghostlight/bin/ghostlight.exe install` once from a regular terminal to finish them. After that,
installs from inside the container only rewrite the manifest. `ghostlight doctor` run from such a
terminal reads that app's own view of the registry, so it says it cannot check the browsers'
registrations there; run it from a regular terminal to check them.

### Uninstall

`ghostlight uninstall` removes what Ghostlight registered: its entry in each agent, its connection
to each browser, the `ghostlight` command (the PATH entry on Windows; `~/.local/bin/ghostlight`,
the manual page and the completions on Linux), the applications-menu entry on Linux, and on
Windows the Start-menu shortcut and the installed-apps entry. It stops the running Ghostlight,
then says what it did:

```text
Ghostlight is uninstalled.
Removed from agents: Claude Code and Codex.
Removed from browsers: Google Chrome.
The running Ghostlight was stopped.
Kept C:\Users\you\.ghostlight, with the Ghostlight programs, your audit history and your settings. Delete that folder to remove them too.
Ghostlight in Browser stays in your browser until you remove it from the browser's extensions page.
```

`ghostlight uninstall --dry-run` lists what it would remove, the PATH entry among it, and changes
nothing.

`--browser` limits which browsers lose their connection, and `--client` limits which agents lose
their entry. The kind you do not name is removed everywhere: `ghostlight uninstall --browser chrome`
removes Chrome's connection and every agent's entry, and `--client zed` removes Zed's entry and
every browser's connection. `--no-clients` leaves every agent's entry alone. Each of these still
removes the `ghostlight` command, the documentation, the shortcuts and the installed-apps entry,
and leaves Ghostlight running. The summary says when the command was removed and when Ghostlight
is still running.

Uninstall keeps the Ghostlight home, `~/.ghostlight`: the executables, the audit history, your
policy and your settings. To remove those too, delete the folder after the uninstall finishes:

```text
PowerShell:       Remove-Item -Recurse -Force "$HOME\.ghostlight"
Linux:            rm -rf ~/.ghostlight
```

Then remove Ghostlight in Browser from each browser's extensions page (`chrome://extensions`,
`edge://extensions`, `brave://extensions`).
