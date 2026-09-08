# Linux integration evidence -- 2026-09-08

This continues the [Linux handoff](linux-release-handoff-2026-09-08.md).
Source started at `9981b63e0e5607fbb2863a3d040cf531080b68a3`, fast-forwarded by 69 commits
from `2255a0e`, with no pre-existing working-tree changes.

## Environment

- CachyOS rolling, x86_64, KDE Wayland with a live Plasma desktop. This is complementary
  development-host evidence, not Ubuntu 22.04, Debian 12, or Ubuntu 24.04 acceptance.
- Rust/Cargo 1.95.0 from the repository pin; Node 22.22.1; npm 10.9.4.
- WebKitGTK 4.1 library 2.52.6, GTK 3.24.52, Ayatana AppIndicator 0.6.0.
- Native Chromium package 152.0.7977.75-1.1. No browser was running during initial inventory.
- Downloaded CI-pinned Chrome for Testing 152.0.7977.82 into ignored test storage. Archive
  SHA-256: `0704631fb3e4f741092e08f55272f90abc3e307f991f05f332924364415b02e0`.
- Portable PowerShell 7.5.2 ran the suite and repository checks. Its archive was checked against
  the publisher's SHA-256 list. PowerShell 7.6.5 separately reproduced the submitted adapter.
- Initially installed service: 1.3.4 under `~/.ghostlight/bin/v1.3.4/`, running with one MCP
  connector and no native browser connector. Chromium registration pointed there. The absent
  Chrome, Edge, and Brave registrations pointed to an older installation. Initial inventory
  changed no registration, policy, browser profile, or installed binary.

## Source gates

The first unchanged-source run passed 17 of 18 gates. The real offline npm-install test failed
before packing: its direct-Node path assumed npm was in `bin/node_modules/npm`, a Windows layout.
Linux's working npm executable was on PATH. The test now uses the invoking npm's `npm_execpath`
when present, the existing Windows fallback on Windows, and `npm` from PATH on Linux.
It still performs actual offline pack/install and invokes npm's generated launcher entry.

Failed report: `.tmp/hardening-suite/2026-09-08T17-56-24-121Z-4626/results.json`.
Source fingerprint: `f44503d0181e60b2ee3099d7ed21477b48420e236dfd76430144c34382265e89`.

After that correction, all 18 gates passed with source unchanged throughout:
`.tmp/hardening-suite/2026-09-08T18-02-36-689Z-23452/results.json`.
Source fingerprint: `8b97f43d35d7c6e13ab51514959f87a01c9e679a04691b724eb5c2b55b325c8d`.
This includes formatting, warnings-denied Clippy, 535 Rust tests, 207 extension tests,
10 npm launcher tests, 4 MCPB tests, policy grammar, fresh executable build, process recovery,
continuity, provenance, both CLI journeys, workbench, browser harness, and all three Chromium
journeys. The frame/editor lane passed 68 checks. These browser lanes use isolated fixtures and
test native transport; they do not prove installed native messaging.

Fresh debug executables under `.target-linux-integration/debug`:

| Executable | SHA-256 |
| --- | --- |
| ghostlight | `ca4becac0a28101f3e788d8ae3aba9a2e1a5a1d7f71a8529ff34a0cebcc1bd08` |
| ghostlight-mcp-connector | `4fdd2a22dfcdee2b3de88258fa9b0a8b58a54cc54fac2c47c52fce0c5fc986be` |
| ghostlight-browser-connector | `29c045e299d76d046821a9cae4fe18778986d90bd6f523bd43821fb2724b309d` |

Every `scripts/*.sh` passed `bash -n`. Offline public-surface and repository-integrity checks
passed, including 981 tracked files, local links, source/public version separation, and the
exact permission-justification roster.

## Adapter custody

Packaging with PowerShell 7.5.2 yielded SHA-256
`559a65f55f4bb131af339dd257ffa93be43eab406b10e81e48ec7e50a208dab5`.
PowerShell 7.6.5 yielded the exact submitted SHA-256
`38d4cc9be45a43494b0803adaa1a7657fff237efd527d067241c5a23a38116d5`.
Both archives contain the same ordered 38 entries with identical extracted bytes. Compressed
sizes differ for `service-worker.js`, `vendor/acorn.js`, and `vendor/gifenc.js`.
The difference is archive compression under the two runtime distributions, not an adapter edit.
Use PowerShell 7.6.5 to reproduce this submission; record the packaging runtime with custody.
No submission or adapter source was changed.

Local artifacts: `.tmp/linux-tools/ghostlight-extension-v1.1.2.zip` and
`.tmp/linux-tools/ghostlight-extension-v1.1.2-pwsh765.zip`. These and the test reports are ignored
machine artifacts. The [submitted custody record](candidate-custody-2026-09-08.md) remains
authoritative for Google's pending review.

## Native Linux development installation

The dev-loop built release siblings and deployed them, without starting, into
`.tmp/linux-installed/bin`. Native Chromium was launched visibly on KDE Wayland in a fresh
standard XDG context under `.tmp/linux-installed/context/`, loading the unchanged source adapter.
There was no Ghostlight runtime, native-host, profile, or policy redirect and no test native pipe.
The ordinary `native-host install` command created real manifests in that context; Chromium
launched the registered connector, which demand-started its exact sibling authority.
Before registration the adapter reported `host_absent`; afterward it reported connected to 1.3.5.
The same browser process survived. This is preliminary extension-first development evidence,
without a precise first-connect latency, not a store/clean-package cold-install pass.

| Release-built executable | SHA-256 |
| --- | --- |
| ghostlight | `c9a8e8127ef8b5aa888cee35b99ec7c0ed531e57a9bb2c836d6a8deb364efb29` |
| ghostlight-mcp-connector | `c4751aa72542340ec615e12570a60fef126690931eff4af209e0ef1ad6ac1535` |
| ghostlight-browser-connector | `bba8bfd1c0cfe741d604890656ce78d1a698c2958cdb8d8d31b5b11b7d81a6a4` |

`tests/live-journey.mjs` passed 15 check groups and 88 calls through the actual native host,
covering all 23 tools. It verified retained editors, composed frames, keyboard/drag effects,
file bytes, prompts, diagnostics, recording, history, real Sylin form completion, and captures.
Report: `.tmp/installed-browser/2026-09-08T18-09-33-339Z-39076.json`.
The shell Foundry story passed, including recording delivery/erase and both dialog responses.
Log: `.tmp/linux-installed/foundry.log`. Preserve-tabs remained enabled.

The new opt-in `tests/linux/installed-journey.mjs` passed all five recovery phases:

| Phase | Measured elapsed time |
| --- | --- |
| Restore registration with the browser running | 1,558 ms |
| Replace the crashed native connector | 3,153 ms |
| Restart authority with the same native pipes and initialized MCP stream | 745 ms |
| Interrupt an independently counted page effect without replay | 2,173 ms |
| Stop worker, preserve binding, and recover on ordinary browser activity | 2,568 ms |

Report: `.tmp/installed-linux/2026-09-08T18-19-29-402Z-40951/results.json`.
Test SHA-256: `e146f6a8b37de1ba5c87a6e302c3a321ad61ccb768a4345421b384d972858cb8`.
The report binds executable hashes, browser PID/start identity, native pipe identities, and
retained MCP identity. The real interrupted fetch reached the fixture server exactly once; the
MCP connection reported the unavailable outcome, recovered, and performed new browser work.

Three earlier recovery reports remain failed evidence under `.tmp/installed-linux/`:
`2026-09-08T18-12-41-024Z-39327`, `2026-09-08T18-15-37-687Z-39927`, and
`2026-09-08T18-18-08-677Z-40494`. They exposed incorrect worker-test assumptions: automatic idle
reactivation, success on a bound request while the adapter is absent, and the unbound manual-startup
reason on a pinned workspace. `browser/recovery.rs::decide` deliberately returns
`browser_wrong_profile` for an absent pinned browser (ADR-0114), rather than choosing another
profile. The final test requires that exact no-effect refusal, an ordinary new-tab event in the
same browser, a new worker target, and successful work over the retained MCP stream. No production
or adapter code changed to obtain that result. Automatic idle recovery after a forced worker stop
was not demonstrated; the first report retains its 45-second timeout.

After adding the recovery runner, the complete 18-gate suite passed again on unchanged source:
`.tmp/hardening-suite/2026-09-08T18-23-01-088Z-42151/results.json`, fingerprint
`3e684032b37712ae67a4a7c649936ff95b53ae8b1331378443a8e4c6ca841c8b`.
The runner also passed `node --check`. Public-surface and repository-integrity checks were
repeated for the evidence/documentation update.

KWin observations against only the test authority's exact PID passed explicit Open/focus,
minimize/Open reconstruction, close without stopping service, and close/Open recreation.
Eight concurrent Open commands left one focused, unminimized compositor window across ten
observations. The window identity changed on reconstruction, as the Linux contract requires.
Both the owner's original and the isolated test authority had distinct registered tray items.
Local evidence: `.tmp/linux-installed/desktop.json`. The driver now lives at
`tests/linux/installed/desktop-check.py`; [`tests/linux/`](../../tests/linux/README.md)
documents its environment settings and the additional Linux drivers.
This proves visible KDE window behavior, not GNOME/non-tray behavior, hidden-window enumeration,
renderer-crash handling, Applications installation, or clean-package lifecycle.

Cleanup closed only the dedicated test browser through its verified DevTools connection, then
used the test authority's actual tray Quit action. Its process and runtime document disappeared.
The owner's original 1.3.4 authority and MCP connector retained their original process start
identities, and only that original Ghostlight tray item remained. Test profiles, reports, and
artifacts remain under ignored `.tmp/`; the owner's browser profile and registrations were not
changed. No source adapter change or deployment to the owner's installation occurred.

## Remaining evidence

The development native-host/browser and limited KDE lifecycle evidence above do not close
candidate-bound cold installation, governed installed-native-host, three-real-client, plural
browser, complete visible desktop lifecycle, or clean-package rows of the
[integration matrix](pre-release-integration.md).
Exact provenance-bound 1.3.5 packages and the staged store adapter remain prerequisites for
release acceptance. The earlier Windows installation and timing failures remain unresolved.
No push, release, store action, or public-version metadata change occurred.
