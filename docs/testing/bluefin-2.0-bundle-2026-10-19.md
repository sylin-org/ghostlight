# Bluefin and the 2.0 release candidate bundle

The Bluefin step of [the owner's page](linux-other-environments-2026-10-19.md), run on test-02 on
2026-10-19 by a Claude Code session at the owner's direction, on the machine's real installation
and the owner's real browser. Every step passed except the first task as the page words it: the
agent stopped at the bell's prompt and asked the person what to say
([finding](#finding-status-does-not-say-what-accept-will-answer)).

## The system

- Bluefin 44.20261013 (`ghcr.io/ublue-os/bluefin-nvidia-open:stable`, Fedora Atomic), kernel
  7.2.5-200.fc44, GNOME Shell 50.5 on Wayland. SELinux enforcing. `/usr` read-only.
- Browser: Chromium 155.0.8059.39 (`chromium-155.0.8059.39-1.fc44`), layered with rpm-ostree before
  this session, on the owner's own profile ("Work", `~/.config/chromium/Default`). The owner opened
  it from Applications and loaded the extension. No other browser or profile was used.
- The Flathub Chromium (`org.chromium.Chromium`) was also installed. Applications listed it as
  "Chromium" beside the layered "Chromium Web Browser". 2.0 has no Flatpak browser route (ADR-0197
  Decision 22), so the owner had it uninstalled before the extension step to rule out a mix-up.
- Agent: Claude Code 2.1.304, the owner's own configuration, model haiku, started fresh for each
  task with `claude -p` and only the Ghostlight tools allowed.
- The GitHub CLI 2.102.0 was installed with Homebrew to download the bundle, so `get.sh` also
  verified build provenance.

## Before the run

- A September 1.3.5 development install ran from the fleet worktrees under
  `~/ghostlight-fleet-test-02`, with browser registrations, a D-Bus activation file and a Flatpak
  bus override from the Flatpak experiment of that month. It was removed with its own
  `ghostlight uninstall`, its Flatpak leftovers were deleted, and the worktrees and fleet folder
  were removed (owner, 2026-10-19).
- `~/.ghostlight/bin/v1.3.4` held a 1.3.4 launcher image. Nothing else of Ghostlight was installed.

## The bundle

Release run 39323041650 on `eba78013a70381e1a8b80eba6efb886961005811` (CI green), downloaded with
`gh run download 39323041650 -n release-candidate -D ~/Downloads/release-candidate`. The manifest
says 2.0.0 and that revision; the extension ZIP's SHA-256 is the recorded `ee224962...`. The
extension folder came from `https://github.com/sylin-org/ghostlight/archive/<revision>.zip`. The
`get.sh` served at the revision is byte-identical to the tree's; the session ran that verified copy
with `GHOSTLIGHT_FROM=~/Downloads/release-candidate`.

## The steps

| Step | Result |
| --- | --- |
| The install | `Ghostlight 2.0.0 copied from /var/home/test/Downloads/release-candidate. Checksums and build provenance verified.`, then the summary: in Applications, agents Codex, Claude Code and Zed, and "Next: add Ghostlight to Chromium in the Ghostlight window." The window started. |
| 1.x leftovers | The install removed `~/.ghostlight/bin/v1.3.4`. |
| With only Flathub browsers | Not applicable: the layered Chromium was already present, and doctor read "A native Chromium package can start Ghostlight. -- registered". |
| The extension | The owner loaded the `extension` folder unpacked in the layered Chromium. Doctor read `Readiness: Ready -- Connected and idle.` right after, with one browser relay for `chrome-extension://cjcmhepmagomefjggkcohdbfemacojoa/`. |
| An agent | Codex, Claude Code and Zed listed as connected; Claude Code's entry is `~/.ghostlight/bin/ghostlight mcp`. |
| The first task, as worded | The agent opened the page, found and clicked the bell, read the dialog's status and stopped: "You didn't say what the bell should say, so I haven't guessed." 6 turns. A defect, below. |
| The first task, with accept asked for | "...ring the bell. When it asks what to say, accept its suggestion as it stands." The agent read status, accepted, and the page read `the bell says: "hello from the desk"`. 9 turns. The prompt-default fix (5bcaff42) holds here. |
| SELinux | No AVC record since the install. The audit journal is readable without sudo on this machine; the only denials of the day were two `tuned` `chcon` records at boot. The browser registration under `~/.config/chromium` is labelled `chrome_sandbox_home_t` and Chromium starts the relay through it. |
| The tray | The Ghostlight item registered with GNOME's AppIndicator host (`tray_icon_tray_app_ghostlight`) with the window, and again after the reinstall, where the service started `ghostlight-desktop --tray` 2 s after it was ready. |
| Uninstall | `Ghostlight is uninstalled.`, removed from the three agents and four browsers, "The running Ghostlight was stopped.", and `~/.ghostlight` kept with its programs, audit and settings, as it says. No Ghostlight process remained. |
| Reinstall | The same command printed the same verified line; the extension reconnected without a reload and doctor read Ready. test-02 now runs this bundle. |

A resumed agent (`claude -p --resume`) is a new edge process and a new session: the tab handle the
first agent held read "That tab handle names no open tab in this session.", as designed.

## Finding: status does not say what accept will answer

The bell's prompt opens with "hello from the desk" in its field. A person sees it and presses OK.
The agent sees only:

```text
A JavaScript dialog is currently visible: "The bell asks: what should it say?".
```

with `facts: {dialog_type: "prompt", present: true}` and the next step "Call browser_dialog with
action accept or dismiss (or respond with text for a prompt) before continuing." Nothing says the
prompt has a default or that accept answers with it, so asking the person was the agent's safest
choice, and the page's task did not complete on the first attempt.

The extension already holds the default (`sessions.js` `openDialog`, used by `promptText` in
`primitives.js`), but `dialogStatus` returns only the type, the message and the url, and the wire's
`DialogFacts` has no field for it. Two ways to close the gap, for the owner and the coordinator:

- Carry the default through: the status reply adds the bounded default, `DialogFacts` an optional
  field, and the status sentence names it and says accept answers with it. This changes the
  extension, which the store submission of this bundle freezes.
- The language alone: for a prompt, status says accept answers with the text the prompt's field
  starts with, as OK does. No extension change, but the agent still cannot say what that text is.
