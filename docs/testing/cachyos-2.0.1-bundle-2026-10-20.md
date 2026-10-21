# The 2.0.1 candidate bundle on CachyOS

test-01, the owner's CachyOS machine (KDE Plasma 6.8.0 on Wayland, system Chromium 154.0.8037.97),
on its real installation and the owner's Chromium, 2026-10-20. The upgrade a 2.0.0 user makes:
from the 2.0.0 release install to the 2.0.1 candidate of release run 39517667200 (dc89cd60), with
the store extension 2.0.0. Asked for by windows-claude.

## Baseline

test-01 was not on the 2.0.0 release install. It ran a development build (the dev-loop deploy of
8df3143e recorded in [cachyos-public-2.0.0-2026-10-20.md](cachyos-public-2.0.0-2026-10-20.md)):
`install.json` read channel `development`, and none of its three images matched v2.0.0's
`SHA256SUMS` (edge `8e0b780e...`, service `ee825cca...`, window `46dd4e94...`). `doctor` read
Ready, with `Channel: development`.

The public restore route put the release back. The script `https://sylin.org/ghostlight/get.sh`
served was byte-equal to `scripts/get.sh` on main (2303c404); `sh get.sh --restore` printed
"Ghostlight 2.0.0 downloaded. Checksums verified." and "Ghostlight 2.0.0 is installed." and
replaced the development images in place. The baseline for the check:

| Item | Value |
| --- | --- |
| `ghostlight --version` | `ghostlight 2.0.0` |
| Images | Equal to v2.0.0's `SHA256SUMS`: edge `fcf48bd1...`, service `381b7aa8...`, window `6f70882d...` |
| `install.json` | channel `release`, service_version `2.0.0`, relay_revision `4` |
| `doctor` | Ready, "Connected and idle". On the release channel it prints no Channel line. |
| The extension | Store "Ghostlight in Browser" 2.0.0 (`lejccfmoeogmhemakeknjjdhkfkgncdl`) in Chromium's Default profile, enabled; no unpacked development extension |
| `gh` | Signed in with `repo` and `workflow` scopes; an Actions artifact downloads |

## The upgrade

| Step | Result |
| --- | --- |
| The run | candidate-bundle succeeded; the three install-from-bundle jobs (Alpine, Ubuntu 22.04 and 24.04) succeeded later. |
| `gh run download 39517667200 -n release-candidate` into a new folder | `SHA256SUMS`, `release-candidate.json` and an `assets` folder of 19 artifacts. |
| `scripts/check-release-candidate.ps1` from dc89cd60 | "Release candidate verified: version 2.0.1, source dc89cd604fd8bd60d7a32b5b6c9d0b2761b663f7, 19 artifacts." Exit 0. |
| `curl -fsSL .../dc89cd60.../scripts/get.sh \| GHOSTLIGHT_FROM=<folder> sh` | The fetched script was byte-equal to dc89cd60's. "Ghostlight 2.0.1 copied from /home/test/Downloads/release-candidate-39517667200. Checksums verified.", "Ghostlight 2.0.1 is installed.", "Ghostlight is in your system tray and your Applications menu." and the agents: Codex, Claude Code, Cursor, Visual Studio Code, Windsurf, Devin, Zed, ZCode, OpenCode, Crush, GitHub Copilot CLI, Cline (CLI), Cline (Visual Studio Code), Kiro, Qwen Code, Junie, Kilo Code and Antigravity. Exit 0, five seconds. |
| Images | Equal to the bundle's `SHA256SUMS`: edge `fcf48bd1...`, service `5038353f...`, window `3a4f5743...`. |
| `install.json` | channel `release`, service_version `2.0.1`, relay_revision `4`, service_sha256 `5038353f...`, edge_sha256 `fcf48bd1...`. |
| `ghostlight --version` | `ghostlight 2.0.1`; the service says the same. |
| `doctor` | Ready, "Connected and idle". Chromium registered; 18 MCP clients connected, goose and Continue detected but not connected (they need a pasted config). |
| Chromium | Reconnected by itself: a new browser relay for the store extension 2.0.0 started with the new service (`serve --reason update`). `status` read 1 agent and 1 browser connected. |

## The window and the tray

| Step | Result |
| --- | --- |
| `tests/linux/installed/desktop-check.py` | Passed all five phases: Open shows one active window, Open after minimize brings one back, close keeps the service, eight concurrent Opens give one window, and a lost service comes back without opening the window. After the lost-service phase the service ran as `serve --reason recover` and Chromium reconnected by itself. |
| `ghostlight open` | The window shows Ready, 0 agents, 1 browser, 0 running. The activity from before the upgrade is still listed. |
| The tray | StatusNotifierItem "Ghostlight", Active, owned by `ghostlight-desktop --tray`. Its menu: Open Ghostlight, Pause agents, Resume agents (disabled while agents run) and Quit Ghostlight. |

## An agent rings the bell

A fresh `claude -p --model haiku` (Claude Code 2.1.305, 8 turns), given only the Ghostlight tools,
opened `https://sylin.org/ghostlight/demo/desk` in its own tab, rang the bell, read the dialog's
status, accepted the prompt and read the page:

- `browser_dialog` status: `A prompt dialog is currently visible: "The bell asks: what should it
  say?".` Its facts held `dialog_type: prompt` and `present: true`, and nothing about the field.
- The page: `the bell says: "hello from the desk"`.

The same steps repeated by hand showed the next step that status returns: "Call browser_dialog with
action accept to answer with the text in its field, as OK does, respond with your own text, or
dismiss, before continuing." This is what the 2.0.1 notes promise with an extension older than
2.0.1: the field's text comes from the 2.0.1 extension, and the next step still says what accept
answers with. The agent accepted without asking, but its answer said it could not tell what text
went in until it read the page.

## Findings

- The upgrade, the window, the tray and the bell all work on CachyOS with the store extension 2.0.0.
- The bundle's edge is byte-equal to v2.0.0's (`fcf48bd1...`), so the upgrade left
  `~/.ghostlight/bin/ghostlight` untouched and replaced only the service and the window.
  `ghostlight --version` still reads 2.0.1, from the service.
- With the store extension 2.0.0, an agent sees a prompt's field text only after it accepts and
  reads the page. That ends when the store carries 2.0.1.
- Preserve Ghostlight tabs is on in the owner's extension, so the agents' demo tabs stay open.

test-01 now runs the 2.0.1 candidate (release channel) with the store extension 2.0.0.
