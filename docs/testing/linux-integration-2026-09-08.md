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

## Remaining evidence

The installed Linux recovery/browser, cold installation, governed installed-native-host,
three-real-client, plural browser, visible desktop lifecycle, and clean-package rows of the
[integration matrix](pre-release-integration.md) are not closed by this source run.
Exact provenance-bound 1.3.5 packages and the staged store adapter remain prerequisites for
release acceptance. The earlier Windows installation and timing failures remain unresolved.
No push, release, store action, or public-version metadata change occurred.
