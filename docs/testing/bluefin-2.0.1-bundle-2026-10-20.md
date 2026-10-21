# Bluefin and the 2.0.1 release candidate bundle

The 2.0.1 bundle check on test-02, run on 2026-10-20 by a Claude Code session at the coordinator's
direction, on the machine's real installation and the owner's layered Chromium. The upgrade from
the 2.0.0 release installed the bundle's images exactly, the window and tray came back on their
own, Chromium reconnected without help, and the desk bell answered. With the 2.0.0 extension, a
prompt's status does not say what its field holds, so the task as the owner's page words it ends
with an answer the agent chose ([finding](#finding-with-the-200-extension-the-agent-picks-the-answer)).

## The system

- Bluefin 44.20261013 (`bluefin-nvidia-open`, Fedora Atomic), kernel 7.2.5-200.fc44, GNOME on
  Wayland, SELinux enforcing. `ldd` reports GNU libc 2.43.
- Chromium 155.0.8059.39 (`chromium-155.0.8059.39-1.fc44`, layered with rpm-ostree) on the owner's
  profile. The extension is the unpacked copy with the development identity
  (`cjcmhepmagomefjggkcohdbfemacojoa`) from the 2.0.0 archive of eba78013, in
  `~/Downloads/ghostlight-eba78013.../extension`. The store extension
  (`lejccfmoeogmhemakeknjjdhkfkgncdl`) is not in this profile; the coordinator ruled the unpacked
  copy equivalent for this check, and the browser registration allows both.
- Agent: Claude Code 2.1.305, model haiku, started fresh for each task with `claude -p` and only the
  Ghostlight tools allowed.
- GitHub CLI 2.102.0, signed in by the owner for this check.

## Baseline

The machine was still on the development channel from the
[2.0.1 fixes re-test](bluefin-dev-2.0.1-2026-10-20.md), and a release package refuses a development
installation. To start from a 2.0.0 release install, the session ran `main`'s `get.sh` (byte-equal
to `origin/main` 2303c404, read before running) with `--restore`: `Ghostlight 2.0.0 downloaded.
Checksums verified.`, then the summary. It also put the four `extension/lib` files the re-test had
copied into the unpacked folder back to their 2.0.0 contents (the folder now equals the 2.0.0
archive), and restarted Chromium once with its `Service Worker` folder cleared, so the 2.0.0 worker
runs (DEV-LOOP: a browser restart alone does not reload it).

| Fact | Value |
| --- | --- |
| `ghostlight --version` | `ghostlight 2.0.0` |
| `bin/install.json` | `service_version` 2.0.0, `relay_revision` 4, `channel` release |
| Images | edge `fcf48bd1...`, service `381b7aa8...`, shell `6f70882d...`: each equal to the v2.0.0 `SHA256SUMS` |
| doctor | `Installed: Ghostlight 2.0.0 -- relay 4`, `Readiness: Ready`, Chromium registered |
| Extension | unpacked, version 2.0.0, files equal to the 2.0.0 archive |

## The bundle

Release run 39517667200 on `dc89cd604fd8bd60d7a32b5b6c9d0b2761b663f7`. The session started once
`candidate-bundle` succeeded (every other job had succeeded; the run itself was still finishing).
Downloaded into a new, empty folder with
`gh run download 39517667200 -R sylin-org/ghostlight -n release-candidate -D ~/Downloads/release-candidate-2.0.1`.
From the checkout at dc89cd60, `scripts/check-release-candidate.ps1` printed
`Release candidate verified: version 2.0.1, source dc89cd604fd8bd60d7a32b5b6c9d0b2761b663f7, 19 artifacts.`

The bundle's Linux edge (`fcf48bd1...`) is byte-equal to the 2.0.0 edge; the service (`5038353f...`)
and the shell (`3a4f5743...`) are new.

## The steps

| Step | Result |
| --- | --- |
| The upgrade | The `get.sh` served at dc89cd60, byte-equal to the tree's (its only changes from `main` pick glibc or musl images and warn when WebKitGTK is missing), with `GHOSTLIGHT_FROM` naming the folder: `Ghostlight 2.0.1 copied from /var/home/test/Downloads/release-candidate-2.0.1. Checksums verified.`, `Ghostlight 2.0.1 is installed.`, `Ghostlight is in your Applications menu.`, `Agents: Codex, Claude Code and Zed.` Exit 0. |
| Images and record | The three images in `bin` equal the bundle's `SHA256SUMS`. `install.json` reads `service_version` 2.0.1, `relay_revision` 4, `channel` release. The edge file kept its 19:47 time from the restore: an identical edge stays, as the release rule says. Nothing else is in `bin`. |
| Version and doctor | `ghostlight --version` reads `ghostlight 2.0.1`. doctor: `Installed: Ghostlight 2.0.1 -- relay 4`, `Readiness: Ready -- Connected and idle.`, Chromium registered, Codex, Claude Code and Zed connected. `status`: `1 agent and 1 browser connected`. |
| Chromium | Reconnected with the unpacked 2.0.0 copy without help: a new browser relay process from `bin` right after the upgrade. This session's MCP edge kept running and reconnected. |
| The window | It was open before the upgrade. The old shell saw its image replaced and started `bin`'s shell with `--updating`, as `crates/desktop/src/machine.rs` describes; the new shell attached, kept the window open (its WebKitGTK processes run under it) and stays running with that argument. |
| The titlebar | Not seen. GNOME refuses this session's direct screenshot and window calls, and a desktop-portal screenshot got no answer in 25 s (most likely a permission prompt nobody was there to answer) and was closed. The look of the decorations under Tao 0.36.0 is still the owner's to check. |
| The tray | The new shell's item is registered with GNOME's AppIndicator watcher (`tray_icon_tray_app_ghostlight`), `Status` Active, with the menu Open Ghostlight, Pause agents, Resume agents (disabled) and Quit Ghostlight. |
| The first task, as the owner's page words it | "Open sylin.org/ghostlight/demo/desk and ring the bell." Navigate, find, click, status, then `accept` with text "Hello", refused (`text is only valid when action is respond`), then `respond` "Hello", then read. The page read `the bell says: "Hello"`. 9 turns. The agent did not stop to ask. |
| The task with accept asked for | "Open sylin.org/ghostlight/demo/desk, ring the bell, accept the prompt, and tell me what the bell says." Navigate, find, click, status, `accept`, read: no refusal. The page read `the bell says: "hello from the desk"`. 8 turns. |
| SELinux | No AVC record today, none naming Ghostlight. |

What dialog status said about the field, in both runs: nothing.

```text
A prompt dialog is currently visible: "The bell asks: what should it say?".
```

with the next step "Call browser_dialog with action accept to answer with the text in its field, as
OK does, respond with your own text, or dismiss, before continuing." The 2.0.0 extension does not
report `default_prompt`, and the 2.0.1 language adds no field sentence when the field is unreported
(`PromptField::Unreported` in `language/outcome.rs`), as designed. With the 2.0.1 extension on the
same service, the re-test read `Its field holds "hello from the desk".`

## Finding: with the 2.0.0 extension, the agent picks the answer

The 2026-10-19 defect, an agent that stops to ask, is gone with the 2.0.1 service alone. But until
the browser runs a 2.0.1 extension, status cannot say what the field holds, and a task that does
not say "accept" leaves the agent to choose: here it answered "Hello" and the page did not show its
default. The owner's page asks only that the page show the answer under the bell, so the step
passes; the result differs from what a person pressing OK gets. This is the rollout window the
coordinator noted on the re-test: the store's 2.0.0 extension behind a 2.0.1 service.

On the way, the agent sent `accept` with `text` once. The refusal named the fix and the next call
used `respond`.

## Seen on the way

- Doctor lists Google Chrome, Microsoft Edge and Brave as "No supported ... installation was
  detected. -- registered". Their registration files date from the development deploy of the
  re-test (18:18:41, which printed "Browser connection registered for Google Chrome, Microsoft Edge,
  Brave."), into config folders left from the September fleet tests. The 2.0.1 upgrade did not
  write them. The 2.0.0 release doctor at the start of the day read "not registered" for the same
  three.
- The shell's command line keeps `--updating` for the life of the process after an update.
  Harmless, but a process list read after an update shows it rather than `--tray`.

## Side effects on the machine

- test-02 now runs the 2.0.1 candidate on the release channel.
- Chromium was restarted once for the baseline, with its `Service Worker` folder cleared; its tabs
  were not restored and remain in history. Preserve Ghostlight tabs kept the demo tabs.
- One desktop-portal screenshot request was made and closed unanswered.
