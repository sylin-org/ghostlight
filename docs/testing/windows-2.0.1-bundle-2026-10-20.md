# Windows 2.0.1 bundle -- 2026-10-20

The 2.0.1 release candidate, installed over the public 2.0.0 on leo-desktop-02 on 2026-10-20 by a
Claude Code session at the coordinator's direction. The machine and its history are in
[the 2.0.0 record](windows-public-2.0.0-2026-10-20.md). Every check passed. The two 2.0.0
defects seen on this machine are gone: VS Code is no longer registered in another app's container,
and no Ghostlight window vetoes a Windows restart. The agent task ran later with the GitHub
Copilot CLI ([addendum](#addendum-the-agent-task-with-the-github-copilot-cli)).

## The bundle

Release run 39517667200 on `dc89cd604fd8bd60d7a32b5b6c9d0b2761b663f7`. Its `candidate-bundle` job
had succeeded; the `install-from-bundle` jobs were still running. The owner signed `gh` in on this
machine for the download:
`gh run download 39517667200 -R sylin-org/ghostlight -n release-candidate -D C:\Users\leo\Downloads\release-candidate-2.0.1`,
into a new folder. From a worktree at `dc89cd60`,
`scripts/check-release-candidate.ps1 -CandidateDirectory <folder>` printed
`Release candidate verified: version 2.0.1, source dc89cd604fd8bd60d7a32b5b6c9d0b2761b663f7, 19 artifacts.`

## The steps

All Ghostlight commands ran in Windows PowerShell 5.1.26100.9444 started through WMI, outside the
Claude Desktop container, as in the 2.0.0 record. This time `gh` stayed on PATH, signed in.

| Step | Result |
| --- | --- |
| Before | Ghostlight 2.0.0, Ready. Doctor: `MCP client: Visual Studio Code -- connected -- Ghostlight is registered for this user context.` The Codex container's `Roaming\Code\User\mcp.json`: SHA-256 `3fea2903...4318`, written 15:55:30, 163 bytes. `%APPDATA%\Code` absent. |
| The upgrade | `$env:GHOSTLIGHT_FROM = "<folder>"; irm https://raw.githubusercontent.com/sylin-org/ghostlight/dc89cd60.../scripts/get.ps1 \| iex`. The fetched script equals the tree's (SHA-256 `a8a81804...`, the same text main serves) and has no `attestation`. 6 s: `Ghostlight 2.0.1 copied from C:\Users\leo\Downloads\release-candidate-2.0.1. Checksums verified.`, `Ghostlight 2.0.1 is installed.`, `Ghostlight is in your notification area, and in the Start menu.`, `Agents: Codex, Claude Code, Claude Desktop and GitHub Copilot CLI.`, `Next: open Chrome, where Ghostlight is already added.` Exit 0. VS Code is no longer listed. |
| The images | `ghostlight-service.exe` (`984f1955...`) and `ghostlight-desktop.exe` (`7a578057...`) equal the bundle's SHA256SUMS. `ghostlight.exe` stayed the 2.0.0 edge (`8af8bbba...`), not the bundle's (`31e45524...`): both are edge 4.0.0, and a release keeps an intact edge of its relay revision whatever its bytes (`deployment.rs`, `docs/RELEASE.md`). `install.json`: `service_version` 2.0.1, `relay_revision` 4, `channel` release, `edge_sha256` the kept edge. |
| Version | `ghostlight --version`: `ghostlight 2.0.1`. |
| Doctor | `Installed: Ghostlight 2.0.1 -- relay 4`, `Readiness: Ready -- Connected and idle. Agents can work when they ask.`, `Ghostlight is running; 1 browser connected.` Codex, Claude Code, Claude Desktop and GitHub Copilot CLI connected. |
| VS Code | `MCP client: Visual Studio Code -- not detected -- Not detected. You can prepare its user configuration before installing it.` and below it `Ghostlight's entry goes in C:\Users\leo\AppData\Roaming\Code/User/mcp.json; another copy of these settings is at C:\Users\leo\AppData\Local\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Roaming\Code/User\mcp.json.` |
| The Codex copy | Unchanged: same hash, same 15:55:30 write time, and its backup file untouched. |
| Chrome | The service, window and relay restarted at 17:01:36-37. The relay came back on its own and doctor counted 1 browser, with the store extension still 2.0.0. Nobody touched Chrome. |
| Restart query | The probe (below) on the 2.0.1 window: all 7 windows answer `allows`. |

## The restart query

The probe sends `WM_QUERYENDSESSION` to every top-level window of `ghostlight-desktop`, then
`WM_ENDSESSION` FALSE, as Windows does when a restart is cancelled. Nothing is ended.

On 2.0.0, before the upgrade (process 21400):

```
0xe0256    tid 15024  vis        6 ms  allows                 class 'Ghostlight Window' title 'Ghostlight'
0x60590    tid 15024  hid        0 ms  allows                 class 'tray_icon_app' title ''
0xf0588    tid 22380  hid        0 ms  allows                 class 'GhostlightSessionListener_21400_1' title 'GhostlightSessionListener_21400_1'
0x605e2    tid 15024  vis        0 ms  VETO (FALSE)           class 'Tao Thread Event Target' title ''
0x701fe    tid 15024  hid        0 ms  allows                 class 'MSCTFIME UI' title 'MSCTFIME UI'
0xe02bc    tid 15024  hid        0 ms  allows                 class 'IME' title 'Default IME'
0xc0594    tid 22380  hid        0 ms  allows                 class 'IME' title 'Default IME'
cancelled: WM_ENDSESSION FALSE sent to 7 windows
```

On 2.0.1 (process 3164):

```
0x6060c    tid 23352  vis        5 ms  allows                 class 'Ghostlight Window' title 'Ghostlight'
0x100526   tid 23352  hid        0 ms  allows                 class 'tray_icon_app' title ''
0x705e2    tid 4696   hid        0 ms  allows                 class 'GhostlightSessionListener_3164_1' title 'GhostlightSessionListener_3164_1'
0xc053c    tid 23352  vis        0 ms  allows                 class 'Tao Thread Event Target' title ''
0x50660    tid 23352  hid        0 ms  allows                 class 'MSCTFIME UI' title 'MSCTFIME UI'
0x27054c   tid 23352  hid        0 ms  allows                 class 'IME' title 'Default IME'
0x805d4    tid 4696   hid        0 ms  allows                 class 'IME' title 'Default IME'
cancelled: WM_ENDSESSION FALSE sent to 7 windows
```

Every process kept running and responding after both probes.

## Limits

- The Claude Code CLI bundled with Claude Desktop still says `Not logged in`, and no Codex CLI is
  installed. The agent task ran with the Copilot CLI once the owner chose it (addendum).
- The restart query stands in for a real restart; the machine was not restarted.
- The 2.0.0 edge and the bundle's edge are both 4.0.0 but differ in bytes. This run kept the
  installed one by design; it does not show whether the bundle's edge would behave the same.
- Logs on that machine: `C:\Users\leo\ghostlight-install-2.0.1.log`,
  `ghostlight-doctor-before-2.0.1.log`, `ghostlight-doctor-2.0.1.log`,
  `ghostlight-query-end-session-2.0.0.log` and `ghostlight-query-end-session-2.0.1.log`.

## Addendum: the agent task, with the GitHub Copilot CLI

The owner chose the GitHub Copilot CLI on the GitHub account signed in for the download. GitHub
Copilot CLI 1.0.99-2 (`%LOCALAPPDATA%\github-copilot-sdk\cli\1.0.99-2\copilot.exe`) ran once, from
an empty folder, non-interactively, with only Ghostlight's tools allowed:

```
copilot -p "Open sylin.org/ghostlight/demo/desk and ring the bell." --allow-tool ghostlight --deny-tool shell --deny-tool write --deny-tool url --disable-builtin-mcps --output-format json
```

It asked for no sign-in or consent. The only MCP server it loaded was `ghostlight`, connected.
Auto mode chose `mai-code-1.1-flash`. 38 s, 8 turns, 8 tool calls, 1 premium request, exit 0.

| Call | Result |
| --- | --- |
| `browser_navigate` the desk, new tab | "Opened sylin.org." |
| `browser_find` "bell" | 3 matches. |
| `browser_inspect` controls with `max_depth` | Refused: "root and max_depth apply only to document scope". |
| `browser_click` the bell | "Clicked the "Ring the bell" button on sylin.org. A prompt dialog is now open." |
| `browser_dialog` status | "A prompt dialog is currently visible: "The bell asks: what should it say?"" with accept, respond or dismiss as next steps. |
| `browser_read` | Refused: "A JavaScript dialog is open on this page, so nothing was sent." |
| `browser_dialog` respond "ding" | "Accepted the browser dialog." |
| `browser_read` visible | The page under the bell: `the bell says: "ding"`. |

Its answer: "Done -- I opened the page and rang the bell. It replied: "ding"."

The agent did not stop to ask what the bell should say. It made up its own answer instead of
accepting the suggestion in the field ("hello from the desk"). The dialog status names the question
and offers accept "with the text in its field", but not that text, so the agent could not know what
accept would answer. This is the open Bluefin finding
([bluefin-2.0-bundle-2026-10-19.md](bluefin-2.0-bundle-2026-10-19.md#finding-status-does-not-say-what-accept-will-answer)),
seen here with a different agent and a different outcome: there Claude Code stopped and asked, and
here Copilot guessed. The two refusals are Ghostlight working as designed and cost one turn each.

The coordinator's note: the store's 2.0.0 extension cannot report a prompt's field, so the 2.0.1
service says only that accept answers with it. With the 2.0.1 extension, in store review on
2026-10-20, status names the field's text, as the owner's machine showed.
