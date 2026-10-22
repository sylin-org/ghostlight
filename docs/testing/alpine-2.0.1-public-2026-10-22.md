# Alpine and the public 2.0.1 release

The public install path on test-03, run on 2026-10-22 by linux-claude at the coordinator's request,
right after v2.0.1 was published (04:39:11Z), the first release with musl images. The public
`get.sh` chose the musl images and Ghostlight reads 2.0.1, Ready. Chromium took the store's 2.0.1
extension, and the desk bell answered with the 2.0.1 extension's words. One thing did not happen:
this machine kept the candidate's service image, because it was already on 2.0.1.

## The system

- Alpine Linux 3.24.1, kernel 6.18.48-0-lts, x86-64, musl. KDE Plasma 6.6.6 on Wayland.
- Chromium from the machine's own Alpine package unpacked under the home, on its default profile,
  with Ghostlight in Browser 2.0.0 from the Chrome Web Store.
- Before: Ghostlight 2.0.1 from the second candidate's bundle (run 39706944349, 064d5e17), release
  channel, as [the candidate's record](alpine-2.0.1-bundle-2026-10-21.md) left it.

## The install

The public script is byte-identical to `scripts/get.sh` on `main` (SHA-256 `f79f80d1...`). It picks
the C library from `ldd --version`, which names musl here.

```text
$ curl -fsSL https://sylin.org/ghostlight/get.sh | sh
Ghostlight 2.0.1 downloaded. Checksums verified.
Ghostlight 2.0.1 is installed.
Ghostlight is in your system tray and your Applications menu.
Agents: Codex and Claude Code.
```

It took 2.5 seconds.

| Check | Result |
| --- | --- |
| `ghostlight --version` | `ghostlight 2.0.1` (`ghostlight version` is not a command; it lists the commands) |
| `install.json` | `service_version` 2.0.1, `relay_revision` 5, channel `release` |
| The window and the edge in `~/.ghostlight/bin` | Equal to the release's musl images, `3a0d809a...` and `a24bd4c6...`, which are also the candidate's |
| The service in `~/.ghostlight/bin` | `643cf741...`, the candidate's image, not the release's `c87bf16f...`; the service process from the candidate's install kept running |
| `ghostlight doctor` | `Installed: Ghostlight 2.0.1 -- relay 5`, Chromium registered, Codex and Claude Code connected. `Readiness: Not connected` until Chromium was started, then `Ready` |

The release is tagged at 8e185d67, one commit after the candidate, and that commit changed the
service (`fix(install): Claude Code in the Store build of Claude Desktop is named Claude Code`).
The installer keeps an installed service of the same version whose image matches its own record
(`install::record::decide`, `Standing::Same`), and `--restore` keeps it too. That is intended for a
reinstall, and no one but a candidate tester has a different 2.0.1 installed. But it means this
check ran the release's window and edge with the candidate's service. The service change is about
naming an agent on Windows, so nothing below depends on it.

## The browser

Chromium was not running when the check began: its adapter had disconnected at 04:32:50Z, before
2.0.1 was published, and nothing in this check had touched it. The cause is not known. It was
started from its `chromium-fleet` launcher at 04:41:48Z and its relay attached at once.

| Time (UTC) | Step | Result |
| --- | --- | --- |
| 04:41:48 | Chromium started | Relay attached; the store extension 2.0.0 running |
| 04:42:00 | Chromium's own update check | `2.0.1_0` beside `2.0.0_0` in the profile's extensions, 12 s after the start; the store's update service offered 2.0.1 |
| 04:42:49 | The desk bell | Passed, but the prompt's status gave no field text: 2.0.0 was still the one running |
| 04:43:00 | Chromium restarted, as the owner authorized | Relay attached 2 s after the stop |
| 04:43:03 | The desk bell again | Passed with 2.0.1's words, below |

So the update downloaded on its own, but took effect only when Chromium restarted.

## The browser call

A fresh Claude Code process (`claude -p --model haiku`, 8 turns, 14 s, $0.0060) opened the demo
desk in a new tab, rang the bell, read the prompt, accepted it and read the page:
`the bell says: "hello from the desk"`. After the restart the same calls over the installed edge
read, for the prompt's status:

```text
A prompt dialog is currently visible: "The bell asks: what should it say?". Its field holds "hello from the desk".
```

Only a 2.0.1 extension reports a prompt's field text, so this is the 2.0.1 extension at work. The
accept and the read followed, and the page read `the bell says: "hello from the desk"`.

## What surprised me

1. A machine on the 2.0.1 candidate stays on the candidate's service after the public install,
   since both say 2.0.1. Only candidate testers meet this.
2. Chromium fetched the 2.0.1 extension 12 s after starting, but ran 2.0.0 until it restarted.
3. Chromium closed itself, or was closed, at 04:32:50Z, with nothing in the record to say why.

## State left on test-03

- Ghostlight 2.0.1, release channel, from the public `get.sh`, with the candidate's service image.
  One service, one browser relay, the window.
- Chromium running with Ghostlight in Browser 2.0.1 from the store. Its windows from before 04:32
  were not restored, since its profile does not restore them.
- The release's musl images are kept in the session scratchpad for comparison, not in the repo.
