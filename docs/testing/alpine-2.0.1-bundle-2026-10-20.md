# Alpine and the 2.0.1 release candidate bundle

The 2.0.1 candidate on test-03, run on 2026-10-20 by linux-claude at windows-claude's request.
The 2.0.0 release install from L2-14 was upgraded in place with the bundle's own `get.sh`, and used
in the owner's Chromium with the store extension 2.0.0. Everything passed. The 2.0.0 extension's
dialog stall behaves as the 2.0.1 notes say, and answering the dialog ends it.

## The system

- Alpine Linux 3.24.1, kernel 6.18.48-0-lts, x86-64, musl. KDE Plasma 6.6.6 on Wayland.
- Chromium 152.0.7977.82, the machine's own Alpine package unpacked under the home, on its default
  profile. Ghostlight in Browser 2.0.0 from the Chrome Web Store (`lejccfmoeogmhemakeknjjdhkfkgncdl`),
  byte-identical to `v2.0.0`.
- Before: Ghostlight 2.0.0 on the release channel from the bundle of release run 39508116197
  (627940a8), every image equal to that bundle's musl image. One service, one browser relay, the
  window. `doctor` Ready. No page dialog open.

## The bundle

Release run 39517667200 on `dc89cd604fd8bd60d7a32b5b6c9d0b2761b663f7` passed all ten jobs, among
them `release-binaries-musl`, `install-from-bundle-alpine` and `install-from-bundle` on ubuntu-22.04
and ubuntu-24.04. The `release-candidate` artifact was downloaded into a new folder,
`~/Downloads/release-candidate-39517667200`, and checked from a checkout at dc89cd60:

```text
$ pwsh -NoProfile -File scripts/check-release-candidate.ps1 -CandidateDirectory ~/Downloads/release-candidate-39517667200
Release candidate verified: version 2.0.1, source dc89cd604fd8bd60d7a32b5b6c9d0b2761b663f7, 19 artifacts.
```

`gh attestation verify` by hand ([supply chain](../trust/supply-chain.md)) accepts all three musl
images.

## The upgrade

```text
$ curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/dc89cd604fd8bd60d7a32b5b6c9d0b2761b663f7/scripts/get.sh | GHOSTLIGHT_FROM=~/Downloads/release-candidate-39517667200 sh
Ghostlight 2.0.1 copied from /home/test/Downloads/release-candidate-39517667200. Checksums verified.
Ghostlight 2.0.1 is installed.
Ghostlight is in your system tray and your Applications menu.
Agents: Codex and Claude Code.
```

It took one second.

| Check | Result |
| --- | --- |
| Images in `~/.ghostlight/bin` | Equal to the bundle's musl images: service `38c62b79...`, window `9b4f90d1...`, edge `104a6adc...` |
| `install.json` | `service_version` 2.0.1, relay 4, channel `release` |
| `ghostlight --version` | `ghostlight 2.0.1` |
| `ghostlight doctor` | `Installed: Ghostlight 2.0.1 -- relay 4`, `Readiness: Ready`, Chromium registered, Codex and Claude Code connected |
| The browser | The service was replaced at 00:01:05Z and Chromium's extension started a new relay 0.5 s later, with no help |

The edge's bytes are the same as 2.0.0's, and the relay revision did not change, so the installer
kept it, as ADR-0197 Decision 14 says.

## Real use

A fresh Claude Code process (`claude -p --model haiku`, 8 turns, 11 s, $0.0054) through the
installer's entry opened the demo desk in a new tab, clicked "Ring the bell", read the prompt with
`browser_dialog` status and accepted it. Status said `A prompt dialog is currently visible: "The
bell asks: what should it say?".` The page read `the bell says: "hello from the desk"`, the
prompt's default. The service's diagnostics show navigate, find, click, status, accept and read in
that order.

## A dialog left open across a reconnect

With the 2.0.1 service and the store extension 2.0.0. One MCP session on the installed edge opened
the desk, rang the bell, read the prompt and held it open:

| Step | Result |
| --- | --- |
| A second agent navigates, no reconnect | `Opened sylin.org.` |
| The browser relay process ended; the extension reconnected in 3 s | |
| A navigate 2 s later | `The job ran out of time before anything changed in the browser.` (12 s) |
| Another, 3 minutes later | `The job ran out of time before the browser confirmed what the sent command did.` (10 s) |
| Escape on the held desk tab, which is the prompt's Cancel | |
| A navigate 2 s later | `Opened sylin.org.` |
| The held session reads its desk tab | Succeeded, so the dialog was closed |

Chromium's session record has no tab for either timed-out navigate, so neither ran after the
dialog closed. The key was sent the way [the Alpine 2.0 record](alpine-2.0-bundle-2026-10-19.md)
and the linux-2 ledger (L2-14) describe: KWin's input emulation over D-Bus and Alpine's libei.

## What surprised me

1. `get.sh` no longer says "and build provenance". That is intended: 53ce4654 removed the GitHub
   CLI from both installers, which now check `SHA256SUMS` alone. The manual check above passes.
2. The first timed-out call after the reconnect read "before anything changed in the browser"
   (diagnostics: `failed none`), and the second read "before the browser confirmed what the sent
   command did" (`unknown unknown`). No browser code changed between the 2.0.0 and 2.0.1 bundles,
   and 2.0.0's runs on this machine read `unknown` both times. So whether the first call after a
   reconnect is written to the browser at all depends on timing. Both sentences were true here:
   neither call ran.
3. Here the prompt was not a window of its own, as it was on 2026-10-20 with 2.0.0. The control
   navigate opened its tab in the desk tab's window, so the desk tab went to the background, and
   when the desk tab came back to the front, the prompt was drawn inside the window. A person who
   switches tabs still finds it on its tab; Escape there answered it.
4. With the 2.0.0 extension, `browser_dialog` status gives the question but not the field's text.
   That is expected: only a 2.0.1 extension reports the text a prompt's field holds.
5. Unconfirmed: during the check the Ghostlight window was the active window, which I had not
   opened. The update restarted the window process with `--updating`, but the window had been
   shown since the L2-14 install, and the diagnostics record no window event. Nothing shows what
   moved the focus.

## State left on test-03

- Ghostlight 2.0.1 from this bundle, release channel, diagnostics on. One service, one browser relay,
  the window.
- The demo tabs these runs opened stay open in Chromium, because Preserve Ghostlight tabs is on.
- The bundles of runs 39508116197 and 39517667200 stay in `~/Downloads` as evidence.
