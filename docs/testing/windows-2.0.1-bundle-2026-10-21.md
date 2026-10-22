# Windows 2.0.1 bundle -- 2026-10-21

The 2.0.1 release candidate from `064d5e17`, installed on leo-desktop-02 on 2026-10-21 by a Claude
Code session at the coordinator's direction, with the same checks as
[the 2026-10-20 bundle check](windows-2.0.1-bundle-2026-10-20.md) and a browser call. Every check
passed. One finding: the tab group of a Claude Code session is named for Claude Desktop, the app it
runs inside, because this Claude Code runs from a folder the agent catalog does not name
([finding](#finding-claude-code-is-named-claude-desktop)).

## The bundle

Release run 39706944349 on `064d5e17af531e46c3a9dab2b5596113e941e6df`, every job green. The owner
approved the download in chat: `gh run download 39706944349 -R sylin-org/ghostlight -n
release-candidate -D C:\Users\leo\Downloads\release-candidate-2.0.1-064d5e17`, a new folder. From the
tree at `064d5e17`, `scripts/check-release-candidate.ps1 -CandidateDirectory <folder>` printed
`Release candidate verified: version 2.0.1, source 064d5e17af531e46c3a9dab2b5596113e941e6df, 19
artifacts.`

## The steps

Ghostlight commands ran in Windows PowerShell 5.1 started through WMI, outside the Claude Desktop
container, as before. The browser calls ran from the Claude Code session's own shell.

| Step | Result |
| --- | --- |
| Before | A development build from the dev loop: `Installed: Ghostlight 2.0.1 -- relay 5`, `Channel: development`, `Readiness: Not connected`. Codex, Claude Code and GitHub Copilot CLI connected; Claude Desktop detected, not connected. Chrome closed, with no Ghostlight extension (the store extension's entry in `Secure Preferences` was empty and its folder gone, as since 2026-10-20). |
| The upgrade | A restore, since `bin` held the development channel: get.ps1 fetched at `064d5e17` (its raw bytes have the tree's SHA-256, `a8a81804...`), run as `& ([scriptblock]::Create($text)) -Restore` with `GHOSTLIGHT_FROM` set to the folder. 4 s: `Ghostlight 2.0.1 copied from <folder>. Checksums verified.`, `Ghostlight 2.0.1 is installed.`, `Ghostlight is in your notification area, and in the Start menu.`, `Agents: Codex, Claude Code, Claude Desktop and GitHub Copilot CLI.`, `Restart Claude Desktop to use Ghostlight.` Exit 0. |
| The images | `ghostlight.exe` (`8f0531dd...`), `ghostlight-service.exe` (`70883cae...`) and `ghostlight-desktop.exe` (`d2fe0a17...`) equal the bundle's SHA256SUMS. `install.json`: `service_version` 2.0.1, `relay_revision` 5, `channel` release. The restore replaced the development edge. |
| Version | `ghostlight --version`: `ghostlight 2.0.1`. |
| Doctor | `Installed: Ghostlight 2.0.1 -- relay 5`, no channel line, `Readiness: Ready -- Starts Google Chrome when an agent asks.` Codex, Claude Code, Claude Desktop (the Store edition's own copy) and GitHub Copilot CLI connected. `Ghostlight is running; 3 agents connected.` |
| VS Code | Still `not detected`, with its entry's place and the Codex container's copy named, as on 2026-10-20. |
| Processes | The service and the window restarted at 18:59:38-40. The three edges the agents had started at 18:33-18:34 kept running across the swap and stayed connected to the new service. |
| Restart query | The probe on the 2.0.1 window process: all 5 windows answer `allows` (the main window was closed, so 5, not 7). Both processes kept responding. |
| The extension | The owner loaded `extension\` of the tree at `064d5e17` unpacked in Chrome (development id `cjcmhepmagomefjggkcohdbfemacojoa`, location 4). The store zip in the bundle has no `key`, so an unpacked copy of it would get an id the native host does not allow. `ghostlight status`: `3 agents and 1 browser connected.` |
| Browser calls | Below. All passed. |

## The browser calls

`ghostlight call` from the Claude Code session's shell, against the desk demo:

| Call | Result |
| --- | --- |
| `browser_navigate` the desk, new tab | `Opened sylin.org.` |
| `browser_tabs` list | 2 tabs, the desk twice (a second navigate from another shell of the same session). |
| `browser_tabs` close one | `Kept the tab open: Preserve Ghostlight tabs is on.` As designed. |
| `browser_find` "bell" | 3 matches, the `Ring the bell` button among them. |
| `browser_click` the button | `Clicked the "Ring the bell" button on sylin.org. A prompt dialog is now open.` |
| `browser_dialog` status | `A prompt dialog is currently visible: "The bell asks: what should it say?". Its field holds "hello from the desk".` |
| `browser_dialog` accept | `Accepted the browser dialog.` |
| `browser_find` "the bell says" | `the bell says: "hello from the desk"` |

The dialog status now names the prompt's field text, so an agent knows what accept answers. That
closes, with the 2.0.1 extension, the Bluefin finding the 2026-10-20 addendum saw with the GitHub
Copilot CLI.

Calls from two separate invocations of the agent's shell shared one session: the second
`browser_navigate` found the first one's tab, so `browser_read` without a tab asked which of two to
read. A call from a `bash -c` nested inside that shell did not: its session had no tab. Git Bash
emulates `fork` with a process that exits, so the nested shell's Windows parent (here pid 1172) is
gone and the walk to the agent stops there. `GHOSTLIGHT_SESSION` is the documented way to share a
session across such shells.

## Finding: Claude Code is named Claude Desktop

The tab strip (read through UI Automation) showed both desk tabs in the group `U+1F47B Claude
Desktop`. The calls came from Claude Code, which runs inside the Store Claude Desktop here:

```
claude.exe  C:\Users\leo\AppData\Local\Claude-Data\claude_pzs8sxrjxfjjc\claude-code\2.1.304\aed62990b91e\claude.exe
Claude.exe  C:\Program Files\WindowsApps\Claude_3.1623.1.0_x64__pzs8sxrjxfjjc\app\Claude.exe
```

The ADR-0106 amendment of 2026-10-21 keys a session on the nearest ancestor that is a catalog
agent's installed app and names it for that agent. I-D4 taught the catalog the folder the Claude
desktop app downloaded Claude Code into, `%APPDATA%\Claude\claude-code`. This Claude Desktop
(3.1623.1.0) keeps it in `%LOCALAPPDATA%\Claude-Data\claude_pzs8sxrjxfjjc\claude-code` instead, so
the walk passes Claude Code and stops at Claude Desktop. The session still works and is still one
per agent process; only its name is the host's. A fix is one more `AGENT_APP_FOLDERS` row for
Claude Code under the local application data, whose middle segment is the Store package's family
suffix.

The other tab-strip checks hold: Chrome has one Ghostlight extension, and no older
`Ghostlight - ...` group was there to rename (the extension was freshly loaded).

## Limits

- The restart query stands in for a real restart; the machine was not restarted.
- The extension is the unpacked development build from the tree, not the store build: the store
  extension is gone from this Chrome, and the bundle's zip cannot load unpacked with an id the
  native host allows.
- No agent task ran this time; the calls were made through `ghostlight call` from the agent's shell.
- Logs on that machine: `C:\Users\leo\ghostlight-doctor-before-2.0.1-1021.log`,
  `ghostlight-install-2.0.1-1021.log`, `ghostlight-doctor-2.0.1-1021.log`,
  `ghostlight-status-2.0.1-1021.log` and `ghostlight-query-end-session-2.0.1-1021.log`.
