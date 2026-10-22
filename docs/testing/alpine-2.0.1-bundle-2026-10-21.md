# Alpine and the second 2.0.1 release candidate bundle

The second 2.0.1 candidate on test-03, run on 2026-10-22 (UTC) by linux-claude at the
coordinator's request, repeating [the first candidate's check](alpine-2.0.1-bundle-2026-10-20.md).
The bundle, the upgrade, `doctor`, the browser's reconnect and the desk bell all passed. The
dialog held across a relay reconnect stalled the 2.0.0 extension as before, but this time it was
not answered by a key, and closing its window did not end the stall: the browser stays stalled
until its extension restarts.

## The system

- Alpine Linux 3.24.1, kernel 6.18.48-0-lts, x86-64, musl. KDE Plasma 6.6.6 on Wayland.
- Chromium from the machine's own Alpine package unpacked under the home, the same browser and
  profile as on 2026-10-20. Ghostlight in Browser 2.0.0 from the Chrome Web Store
  (`lejccfmoeogmhemakeknjjdhkfkgncdl`).
- Before: not 2.0.0. The install held that day's development build, deployed with
  `scripts/dev-loop.ps1` (development channel, relay 5). `doctor` Ready.

## The bundle

Release run 39706944349 on `064d5e17af531e46c3a9dab2b5596113e941e6df` passed all ten jobs, among
them `release-binaries-musl`, `install-from-bundle-alpine` and `install-from-bundle` on
ubuntu-22.04 and ubuntu-24.04. The `release-candidate` artifact was downloaded into a new folder,
`~/Downloads/release-candidate-39706944349`, and checked from a checkout at 064d5e17:

```text
$ pwsh -NoProfile -File scripts/check-release-candidate.ps1 -CandidateDirectory ~/Downloads/release-candidate-39706944349
Release candidate verified: version 2.0.1, source 064d5e17af531e46c3a9dab2b5596113e941e6df, 19 artifacts.
```

`gh attestation verify --format json` accepts all three musl images, each attested by
`release.yml` at 064d5e17. Without `--format json`, gh 2.102.0 printed nothing on this machine and
exited 0.

## The upgrade

The plain command refused, as it should over a development build:

```text
$ curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/064d5e17af531e46c3a9dab2b5596113e941e6df/scripts/get.sh | GHOSTLIGHT_FROM=~/Downloads/release-candidate-39706944349 sh
Ghostlight 2.0.1 copied from /home/test/Downloads/release-candidate-39706944349. Checksums verified.
Error: The installed Ghostlight is a development build, so this package did not replace it. To return to a release, run one of these:
...
```

With `--restore`, which `get.sh` accepts together with `GHOSTLIGHT_FROM`:

```text
$ curl -fsSL .../064d5e17.../scripts/get.sh | GHOSTLIGHT_FROM=~/Downloads/release-candidate-39706944349 sh -s -- --restore
Ghostlight 2.0.1 copied from /home/test/Downloads/release-candidate-39706944349. Checksums verified.
Ghostlight 2.0.1 is installed.
Ghostlight is in your system tray and your Applications menu.
Agents: Codex and Claude Code.
```

It took under a second.

| Check | Result |
| --- | --- |
| Images in `~/.ghostlight/bin` | Equal to the bundle's musl images: service `643cf741...`, window `3a0d809a...`, edge `a24bd4c6...` |
| `install.json` | `service_version` 2.0.1, `relay_revision` 5, channel `release` |
| `ghostlight --version` | `ghostlight 2.0.1` |
| `ghostlight doctor` | `Installed: Ghostlight 2.0.1 -- relay 5`, `Readiness: Ready`, Chromium registered, Codex and Claude Code connected |
| The browser | The service started at 01:50:42.537Z and the extension's relay attached 27 ms later, with no help |

## Real use

A fresh Claude Code process (`claude -p --model haiku`, 7 turns, 14 s, $0.0056) opened the demo
desk in a new tab, clicked "Ring the bell", read the prompt with `browser_dialog` status and
accepted it. Status said `A prompt dialog is currently visible: "The bell asks: what should it
say?".` The page read `the bell says: "hello from the desk"`, the prompt's default. The service's
diagnostics show navigate, click, status, accept and read, each succeeded, in that order.

## A dialog left open across a reconnect

With the 2.0.1 service and the store extension 2.0.0, through two MCP sessions on the installed
edge. The held session opened the desk, rang the bell and read the prompt, and held it open.

| Time (UTC) | Step | Result |
| --- | --- | --- |
| 01:52:04 | A second agent navigates, no reconnect | `Opened sylin.org.` (0.3 s) |
| 01:52:04 | The browser relay process ended; the extension reconnected | in 3.1 s |
| 01:52:17 | A navigate 2 s later | `The job ran out of time before anything changed in the browser.` (8 s; `failed none`) |
| 01:52:25 | The held session dismisses the prompt with `browser_dialog` | the same sentence (8 s) |
| 01:52:35, 01:52:43 | A navigate; the held session reads its desk tab | the same sentence, 8 s each |
| 01:55:04 | A navigate | `The job ran out of time before the browser confirmed what the sent command did.` (8 s; `unknown unknown`) |
| 01:58:09 | Both desk windows closed through a KWin script, the held prompt's among them | the windows are gone |
| 01:58:19, 01:58:37 | A navigate, twice | `... before the browser confirmed what the sent command did.` |
| 01:59:00 | The browser relay ended again; the extension reconnected | in 3.1 s |
| 01:59:11 | A navigate | the same |
| 02:04:28 | A navigate after five idle minutes | the same |

A tab list, which asks no browser, answered at once throughout.

On 2026-10-20 Escape on the held tab, the prompt's Cancel, ended the stall at once. This time no key
could be sent. Alpine's libei client library is no longer on the machine. A small ei client written
for this check reached KWin's EIS seat and its keyboard capability, but KWin dropped it at the
keyboard bind. AT-SPI saw Chromium's windows, but not the prompt inside them. So the prompt was
not answered. Its window was closed instead, and that did not end the stall, nor did a second
reconnect or five idle minutes.

## What surprised me

1. Closing the held prompt's window did not end the 2.0.0 extension's stall, where answering the
   prompt did on 2026-10-20. Not shown here: whether answering would have ended it this time.
   2.0.1's note says to answer the dialog; it says nothing about a closed one.
2. The first two timed-out calls after the reconnect read "before anything changed in the browser"
   (`failed none`) and the later ones "before the browser confirmed what the sent command did"
   (`unknown unknown`), the same split as on 2026-10-20. Both were true: none ran.
3. A release `get.sh` will not replace a development build. That is intended, and `--restore` with
   `GHOSTLIGHT_FROM` installs the candidate. A machine left on a development build needs that flag.

## State left on test-03

- Ghostlight 2.0.1 from this bundle, release channel, diagnostics on. One service, one browser relay,
  the window.
- The store extension 2.0.0 is stalled since 01:52:07Z: every browser command times out. It needs
  its worker restarted, by reloading the extension or restarting Chromium, which is the owner's
  call. Nothing was changed in the browser's profile.
- The demo tabs these runs opened stay open, except the two desk windows closed above. The session
  accessibility flag `ScreenReaderEnabled`, set for a moment while looking for the prompt, is back
  to false.
- The bundle of run 39706944349 stays in `~/Downloads` as evidence.
