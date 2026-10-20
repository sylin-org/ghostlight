# Windows public 2.0.0 -- 2026-10-20

Test-04, run on leo-desktop-02 on 2026-10-20 by a Claude Code session at the coordinator's
direction, with the owner clicking Add in Chrome. The public path was run twice: once over 1.3.5
leftovers, then again after `ghostlight uninstall`. Both runs reached `Readiness: Ready`. Two
findings: `get.ps1` stops in Windows PowerShell 5.1 when `gh` is installed but not signed in
([finding](#finding-getps1-stops-on-a-gh-that-writes-to-stderr)), and the VS Code registration was
written into the Codex app's package container
([finding](#finding-vs-code-registered-inside-another-apps-container)). The first agent task was
not run; the owner skipped it ([limits](#limits)).

## The system

- Windows 11 Pro 25H2, build 26200.9445, installed 2026-09-08. Local account `leo`, a standard
  (not elevated) token. The profile is six weeks old and used for development, not fresh.
- Chrome 153.0.8010.37 and Edge 153.0.4234.32. Neither had Ghostlight in Browser before the run.
- git 2.55.0, gh 2.100.0 (installed, not signed in), node 24.21.0, cargo 1.95.0, VS Code 1.137.0
  at `C:\Tools\VSCode` (never launched by this user: `%APPDATA%\Code` does not exist).
- The session ran inside the Microsoft Store (MSIX) Claude Desktop, whose children inherit its
  container. Every Ghostlight command that writes the registry or AppData was started outside it,
  through WMI, in Windows PowerShell 5.1:
  `Invoke-CimMethod Win32_Process Create` with
  `cmd.exe /d /s /c ""...\powershell.exe" -NoProfile -EncodedCommand <command>" > <log> 2>&1`.
  The Ghostlight window appeared on the desktop that way; the scheduled-task fallback was not needed.
- The window was read with Windows UI Automation, not screenshots.

## Before the run

- `%USERPROFILE%\.ghostlight\bin\v1.3.5\` with the three 1.3.5 executables, dated 2026-09-10.
- A Start-menu `Ghostlight.lnk` pointing to `%LOCALAPPDATA%\Ghostlight\ghostlight.exe`, which did
  not exist.
- `HKCU\Software\Google\Chrome\NativeMessagingHosts\org.sylin.ghostlight` pointing to a manifest in
  the Codex app's container,
  `%LOCALAPPDATA%\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\Ghostlight\`, the leftover of
  an earlier install run inside Codex. That folder held a full 1.x install (14 entries).
- No uninstall entry, Run key, PATH entry, process, or agent registration.

## The steps

| Step | Result |
| --- | --- |
| Run one, as published | `irm https://sylin.org/ghostlight/get.ps1 \| iex` (redirects to `main/scripts/get.ps1`, SHA-256 `4ee5a47b...`). Stopped after 3 s with `NativeCommandError` at line 166, `& gh attestation verify ... *> $null`. Nothing was written. A finding, below. |
| Run one, without gh | The same, with `gh` removed from that process's PATH. 7 s: `Ghostlight 2.0.0 downloaded. Checksums verified.`, `Ghostlight 2.0.0 is installed.`, `Ghostlight is in your notification area, and in the Start menu.`, `Agents: Codex, Claude Code, Claude Desktop, Visual Studio Code and GitHub Copilot CLI (restart Codex, Claude Code, Claude Desktop and GitHub Copilot CLI before you ask).`, `Next: add Ghostlight to Chrome in the Ghostlight window.` |
| 1.x leftovers | `bin\v1.3.5` removed. The dead `Ghostlight.lnk` rewritten to `~/.ghostlight\bin\ghostlight-desktop.exe`. The Chrome host key repointed to `~/.ghostlight\manifests\org.sylin.ghostlight.json`, and Edge, Chromium and Brave keys added. The Codex-container Ghostlight folder untouched, now orphaned (no key points to it). |
| The window | Opened on `#setup`. This computer: done, "Installed". Your browser: "Add Ghostlight to your browser. One browser is enough. Waiting for a browser." with Google Chrome and Microsoft Edge, each "Not added" with an Add button, and Skip for now. Your agent: not yet reached. |
| The extension | The owner clicked Add to Google Chrome. Ghostlight in Browser 2.0.0 (`lejccfmoeogmhemakeknjjdhkfkgncdl`) installed from the Web Store. Your browser: done, "Connected in Chrome". |
| Your agent | "Ask your agent." with "Open sylin.org/ghostlight/demo/desk and ring the bell." and Copy prompt, "Waiting for your agent.", then Codex, Claude Code, Claude Desktop, Visual Studio Code and GitHub Copilot CLI, each "Restart <agent> to finish." |
| Doctor | `Readiness: Ready -- Connected and idle. Agents can work when they ask.`, `Ghostlight is running; 1 browser connected.` Installed 2.0.0, relay 4. Five agents "connected"; Claude Desktop's line names the Store edition's own copy under `Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming`, which the installer wrote correctly. |
| The first task | Not run. See [limits](#limits). |
| Uninstall | `Ghostlight is uninstalled.`, removed from the five agents and from Chrome, Edge, Brave and Chromium, `The running Ghostlight was stopped.`, kept `~/.ghostlight`, and said the extension stays until removed from the browser. |
| What remained | `~/.ghostlight` (bin, run, state, install.lock; `manifests` gone), as stated. The Chrome extension, as stated. The Codex-container Ghostlight folder, by design. Gone: the Start-menu entry, all four host keys, the uninstall entry, the PATH entry, the processes, and every agent's `ghostlight` entry. |
| Run two | The same public path, `gh` hidden again (main still served `4ee5a47b...`). 4 s, the same summary plus `Open a new terminal to use ghostlight.`, ending `Next: open Chrome, where Ghostlight is already added.` The window opened with This computer and Your browser done ("Connected in Chrome"); no Add was needed. |
| Doctor, run two | Ready, 1 browser connected. Identical to run one's output. |

Run two is a reinstall after uninstall, not a new person: `~/.ghostlight` and the extension were
kept, and nothing was deleted by hand.

## Finding: get.ps1 stops on a gh that writes to stderr

`get.ps1` sets `$ErrorActionPreference = "Stop"`. In Windows PowerShell 5.1, redirecting a native
command's stderr turns each stderr line into an error record, so `gh attestation verify ... *>
$null` throws when `gh` prints anything there. A `gh` that is not signed in prints
`To get started with GitHub CLI, please run:  gh auth login`, and the install ends before anything
is written. The same call returns normally in PowerShell 7.6 (`LASTEXITCODE=4`). The coordinator
reports that a signed-in `gh` passes in 5.1 on the owner's machine. The script means such a `gh`
to drop only the provenance claim. From 2.0.1 neither `get.ps1` nor `get.sh` runs `gh` at all,
since a failed check never stopped an install anyway; main, which the short link serves, still had
the old script during this run.

## Finding: VS Code registered inside another app's container

Both installs reported Visual Studio Code as connected. Doctor said
`MCP client: Visual Studio Code -- connected -- Ghostlight is registered for this user context.`
with no settings path. Plain `%APPDATA%\Code\User\mcp.json` was never written. The registration went
to `%LOCALAPPDATA%\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Roaming\Code\User\mcp.json`, a
`Code\User` folder that something running inside the Codex container created on 2026-09-08. The
uninstall emptied it (leaving `mcp.json.ghostlight-backup`) and run two wrote it again.

The cause is the rule in `crates/orchestrator/src/install/packaged.rs` (ADR-0197 amendment
"Packaged clients seen from outside", 2026-10-17): another package's copy beats the plain path when
the plain settings are not there at all. Here VS Code is an ordinary Windows program (doctor's icon
line says "Windows program"), never launched, so its plain folder was absent and Codex's copy won.
When VS Code first starts it creates the plain folder and never sees Ghostlight. Two things follow:
the fallback should not pick a different app's container for an agent with no Store edition, and
doctor should name the settings file whenever it is not the plain path, as it already does for
Claude Desktop. This also means the install does write inside an app container in this case.

## Limits

- The first task was not run. The only Claude Code CLI on the machine is the one bundled with
  Claude Desktop, and run on its own it said `Not logged in`. The session's own sign-in is supplied
  by the Desktop host and cannot be handed to a child process. The owner chose to skip it.
- `gh` was hidden from the installer in both successful runs, so build provenance was not checked
  and the output said `Checksums verified.` only.
- The machine was not restarted (the 2.0.0 window process blocks restart and shutdown; fixed for
  2.0.1).
- Logs are in `C:\Users\leo\ghostlight-install.log`, `ghostlight-install-run2.log`,
  `ghostlight-doctor.log`, `ghostlight-doctor-run2.log` and `ghostlight-uninstall.log` on that
  machine.
