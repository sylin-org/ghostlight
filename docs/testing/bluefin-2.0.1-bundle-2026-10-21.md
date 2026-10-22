# Bluefin and the second 2.0.1 release candidate bundle

The 2.0.1 bundle check on test-02, run on 2026-10-21 by a Claude Code session at the coordinator's
direction, repeating the [2026-10-20 bundle check](bluefin-2.0.1-bundle-2026-10-20.md) on the new
candidate. The upgrade from the 2.0.0 release installed the bundle's images exactly, the tray came
back on its own, Chromium reconnected without help, the window opened on the new shell, and the desk
bell answered. One finding: worded as the owner's page words it, the task stopped to ask in one run
of three ([finding](#finding-one-run-in-three-stops-to-ask)). The task that asks for accept passed
three of three.

## The system

- Bluefin 44.20261013 (`bluefin-nvidia-open`, Fedora Atomic), kernel 7.2.5-200.fc44, GNOME on
  Wayland, SELinux enforcing. `ldd` reports GNU libc 2.43.
- Chromium 155.0.8059.84 (`chromium-155.0.8059.84-1.fc44`, layered with rpm-ostree; it was
  155.0.8059.39 on 2026-10-20) on the owner's profile.
- The extension is the one this profile has: the unpacked copy with the development identity
  (`cjcmhepmagomefjggkcohdbfemacojoa`) in
  `~/Downloads/ghostlight-eba78013a70381e1a8b80eba6efb886961005811/extension`. Its 80 files equal
  the 2.0.0 tree at eba78013, unchanged since the 2026-10-20 restore (19:47). Chromium started
  after that (2026-10-21 13:12), so its worker runs the 2.0.0 extension, and no restart was needed.
- Agent: Claude Code 2.1.307, model haiku (`claude-haiku-5-5`), started fresh for each run with
  `claude -p --model haiku --allowedTools mcp__ghostlight`, so only the Ghostlight tools ran.

## Baseline

The machine was on the development channel (`install.json`: 2.0.1, relay 5, `channel`
development) from a development deploy earlier in the day, and a release package refuses a
development installation. As on 2026-10-20, `main`'s `get.sh` (2303c404, the script that check
read and ran; read again in full before running) with `--restore`: `Ghostlight 2.0.0 downloaded.
Checksums verified.`, `Ghostlight 2.0.0 is installed.`, `Agents: Codex, Claude Code and Zed.`

| Fact | Value |
| --- | --- |
| `ghostlight --version` | `ghostlight 2.0.0` |
| `bin/install.json` | `service_version` 2.0.0, `relay_revision` 4, `channel` release |
| Images | edge `fcf48bd1...`, service `381b7aa8...`, shell `6f70882d...`: the 2.0.0 images, as on 2026-10-20 |
| doctor | `Installed: Ghostlight 2.0.0 -- relay 4`, `Readiness: Ready -- Connected and idle.`, Chromium registered |
| status | `1 agent and 1 browser connected`; the window closed, its icon in the tray |

## The bundle

Release run 39706944349 on `064d5e17af531e46c3a9dab2b5596113e941e6df`, every job green. Downloaded
into a new, empty folder with
`gh run download 39706944349 -R sylin-org/ghostlight -n release-candidate -D ~/Downloads/release-candidate-2.0.1-39706944349`.
From a detached checkout of 064d5e17, `scripts/check-release-candidate.ps1` printed
`Release candidate verified: version 2.0.1, source 064d5e17af531e46c3a9dab2b5596113e941e6df, 19 artifacts.`

All three Linux images are new: edge `f05a5516...`, service `6a6f31ab...`, shell `ae6eda9f...`. On
2026-10-20 the edge was byte-equal to 2.0.0's; this candidate's edge carries the edge fixes since.

## The steps

| Step | Result |
| --- | --- |
| The upgrade | The `get.sh` of 064d5e17, read before running; it differs from `main`'s only as on 2026-10-20 (it picks glibc or musl images and warns when WebKitGTK is missing). With `GHOSTLIGHT_FROM` naming the folder: `Ghostlight 2.0.1 copied from /var/home/test/Downloads/release-candidate-2.0.1-39706944349. Checksums verified.`, `Ghostlight 2.0.1 is installed.`, `Ghostlight is in your Applications menu.`, `Agents: Codex, Claude Code and Zed.` |
| Images and record | The three images in `bin` equal the bundle's `SHA256SUMS`. `install.json` reads `service_version` 2.0.1, `relay_revision` 5, `channel` release. Nothing else is in `bin`. |
| Version and doctor | `ghostlight --version` reads `ghostlight 2.0.1`. doctor: `Installed: Ghostlight 2.0.1 -- relay 5`, `Readiness: Ready -- Connected and idle.`, Chromium registered, Codex, Claude Code and Zed connected. `status`: `1 agent and 1 browser connected`. |
| The service | `serve --reason update --relaunched`, started by the upgrade. |
| Chromium | Reconnected with the unpacked 2.0.0 copy without help: a new browser relay process from `bin` a second after the service. This session's MCP edge, started at 13:08 under the earlier development install, kept running through both installs and stayed connected. |
| The tray | The window was closed, so the shell had only the icon. The upgrade's new shell came up by itself as `ghostlight-desktop --tray` (on 2026-10-20 the window was open and the new shell carried `--updating`). Its item is registered with GNOME's AppIndicator watcher (`tray_icon_tray_app_ghostlight`, owned by the new shell's process), `Status` Active, menu Open Ghostlight, Pause agents, Resume agents (disabled) and Quit Ghostlight. |
| The window | `ghostlight open` brought it up: `status --json` reads `window` `shown`, and its `WebKitNetworkProcess` and `WebKitWebProcess` (webkit2gtk-4.1) run as children of the new shell. The titlebar was not looked at. |
| The first task, as the owner's page words it | "Open sylin.org/ghostlight/demo/desk and ring the bell." Three fresh runs. Run 1: navigate, read, find, click, status, then it stopped and asked what to answer (7 turns); the prompt was left open. Runs 2 and 3: navigate, find, click, status, then `accept` with text, refused (`text is only valid when action is respond`), then `respond`, then read. The page read `the bell says: "Hello from Claude"` and `the bell says: "ding"`. 9 turns each. |
| The task with accept asked for | "Open sylin.org/ghostlight/demo/desk, ring the bell, accept the prompt, and tell me what the bell says." Three fresh runs, each: navigate, find, click, status, `accept`, read (one run read the page once more first), no refusal. The page read `the bell says: "hello from the desk"` every time. 8, 8 and 9 turns. |
| SELinux | Two AVC records today, both `tuned`'s `chcon` at boot; none names Ghostlight. |

What dialog status said about the field, in every run: nothing, as on 2026-10-20.

```text
A prompt dialog is currently visible: "The bell asks: what should it say?".
```

with the next step "Call browser_dialog with action accept to answer with the text in its field, as
OK does, respond with your own text, or dismiss, before continuing." The 2.0.0 extension does not
report `default_prompt`, so the 2.0.1 language adds no field sentence, as designed.

## Finding: one run in three stops to ask

Worded as the owner's page words it, the task ended once (run 1) the way the 2026-10-19 defect did:
the agent rang the bell, read the prompt, and handed the choice back ("Tell me what to answer, or
whether to dismiss it, and I'll do that."). The page showed no answer. The other two runs answered
it themselves, as the one run on 2026-10-20 did. Ghostlight's words were the same in every run,
and the same as on 2026-10-20, and the extension is the same 2.0.0. So this is the agent's choice
varying, not a change in the candidate. It is the rollout window that check named: until the
browser runs a 2.0.1 extension, status cannot say what the field holds, and a task that does not
say "accept" leaves the agent to decide whether to answer at all, and with what. With the field
reported, the 2026-10-20 re-test read `Its field holds "hello from the desk".`

## Side effects on the machine

- test-02 now runs this candidate on the release channel.
- Seven demo tabs were opened in Chromium, one per run. The tab from the first task's run 1 still
  holds the bell's open prompt.
- The Ghostlight window was opened and left open.
