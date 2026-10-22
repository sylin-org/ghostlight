# Windows public 2.0.1 -- 2026-10-22

The public 2.0.1 install paths, run once on leo-desktop-02 on 2026-10-22 by a Claude Code session at
the coordinator's direction, after GitHub release v2.0.1 became Latest, the Chrome Web Store began
serving extension 2.0.1 and npm published `ghostlight@2.0.1`. The owner approved running the public
script and npx in chat, and swapped the extension in Chrome. Every check passed.

Before: the 2.0.1 candidate from `8e185d67`, installed the night before
([its record](windows-2.0.1-bundle-2026-10-21.md#addendum-the-naming-fix-candidate-8e185d67)),
with the unpacked development extension in Chrome. Main was at `571e38fa`, documentation commits
after the release.

## The steps

Ghostlight commands ran in Windows PowerShell 5.1 started through WMI, outside the Claude Desktop
container. The browser calls ran from the Claude Code session's own shell.

| Step | Result |
| --- | --- |
| The script | `https://sylin.org/ghostlight/get.ps1` answers `302` to main's `scripts/get.ps1`, whose SHA-256 (`a8a81804...`) is the tree's. |
| `irm https://sylin.org/ghostlight/get.ps1 \| iex` | 3 s: `Ghostlight 2.0.1 downloaded. Checksums verified.`, `Ghostlight 2.0.1 is installed.`, `Ghostlight is in your notification area, and in the Start menu.`, `Agents: Codex, Claude Code, Claude Desktop and GitHub Copilot CLI.` Exit 0. |
| The images | `ghostlight-service.exe` (`9b01dcf5...`), `ghostlight-desktop.exe` (`abfcf310...`) and the kept edge (`8f0531dd...`) are the `8e185d67` candidate's: the release carries the candidate's binaries. `install.json`: 2.0.1, relay 5, channel release. |
| Version | `ghostlight --version`: `ghostlight 2.0.1`. (`ghostlight version` is not a command; the grammar takes `--version`.) |
| The extension | The owner removed the unpacked development extension and added the store's. Chrome's profile: `lejccfmoeogmhemakeknjjdhkfkgncdl`, location 1 (the store), folder `2.0.1_0`; the development id is gone. `ghostlight status`: `3 agents and 1 browser connected.` |
| Doctor | `Installed: Ghostlight 2.0.1 -- relay 5`, `Readiness: Ready -- Connected and idle. Agents can work when they ask.` Codex, Claude Code, Claude Desktop and GitHub Copilot CLI connected. |
| The desk bell | Below. Passed. |
| npm | `npm view ghostlight dist-tags`: `{ latest: '2.0.1' }`. The published tarball's SHA-1 (`536aa42f...`) equals the candidate bundle's `ghostlight-2.0.1.tgz`. |
| `npx -y ghostlight@2.0.1 deployment install --restore` | 4 s: `Downloading Ghostlight 2.0.1 for win32/x64` three times, each `Verified`, then `Installed ghostlight-service.exe in C:\Users\leo\.ghostlight\bin.`, `The release now serves from C:\Users\leo\.ghostlight\bin\ghostlight-service.exe.`, `The Ghostlight service started.`, `Updated the Start-menu shortcut.` Exit 0. The images and `install.json` are unchanged, and status still reads `3 agents and 1 browser connected.` |

## The desk bell

`ghostlight call` from the Claude Code session's shell, with the store's 2.0.1 extension:

| Call | Result |
| --- | --- |
| `browser_navigate` the desk, new tab | `Opened sylin.org.` |
| `browser_find` "Ring the bell" | The button among the matches. |
| `browser_click` the button | `Clicked the "Ring the bell" button on sylin.org. A prompt dialog is now open.` |
| `browser_dialog` status | `A prompt dialog is currently visible: "The bell asks: what should it say?". Its field holds "hello from the desk".` |
| `browser_dialog` accept | `Accepted the browser dialog.` |
| `browser_find` "the bell says" | `the bell says: "hello from the desk"` |

The tab strip, read through UI Automation, showed the tab in the group `U+1F47B Claude Code`.

## Limits

- `npx` in this machine's Windows PowerShell 5.1 resolved first to fnm's `npx.ps1`, which the
  `Restricted` execution policy does not run, and nothing happened; `npx.cmd` ran the same command.
  This is the machine's Node shim, not Ghostlight.
- The restart query and the agent task were not repeated; the 2026-10-21 record covers them for the
  same binaries.
- Logs on that machine: `C:\Users\leo\ghostlight-public-2.0.1.log` and
  `C:\Users\leo\ghostlight-npx-restore-2.0.1.log`.
