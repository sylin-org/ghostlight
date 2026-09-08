# Linux release continuation -- 2026-09-08

## Objective and starting point

Continue the real integration and release-readiness work for service/package 1.3.5 on Linux.
Start from current `origin/dev`, including `85ecfb06` and this handoff. Fix demonstrated failures
at their owning seams and retain evidence. The owner wants tests against the actual installed
product; a synthetic browser response or test native pipe cannot close an installed-browser gate.

On a clean checkout, fetch `dev` and fast-forward it. Preserve any existing local work; do not
reset, clean, or overwrite another agent's checkout to match these instructions.

Read `AGENTS.md` and its required onboarding documents first. Then read, in order:

1. [Current status](../STATUS.md).
2. [Candidate and Google submission custody](candidate-custody-2026-09-08.md).
3. [Pre-release integration matrix](pre-release-integration.md), especially the Linux gates.
4. [Installation incident](installation-recovery-2026-09-08.md).
5. [Development loop](../DEV-LOOP.md) and [release process](../RELEASE.md).

For Linux installation/startup work, read ADRs 0115, 0116, 0118, 0119, 0123, 0124, 0125, 0127,
0149, and 0150 from the [ADR index](../adr/README.md). ADR-0127 supersedes historical references
to `ghostlight --headless` and `ghostlight service`: neither is a supported startup command.
Read each other subsystem's ADR before changing it.

## What is already done

- Source and service-derived package metadata are 1.3.5; adapter source is 1.1.2.
- Public versions remain service 1.3.4 and adapter 1.1.1. Public-status and MCP Registry metadata
  deliberately still describe those published versions.
- Adapter 1.1.2 was uploaded to the existing Chrome Web Store item and submitted for review.
  The dashboard confirmed `Pending review`. Automatic publication is OFF; approval will stage
  the item. A staged approval expires 30 days after review passes, according to the dashboard.
- Submitted ZIP SHA-256:
  `38d4cc9be45a43494b0803adaa1a7657fff237efd527d067241c5a23a38116d5`.
  Its 38 entries were compared with source; only the development key was removed from the
  packaged manifest. Preserve those adapter bytes while Google reviews them.
- All 19 Windows hardening gates passed after the bumps. Fingerprint and report coordinates
  are in the custody record. No newly numbered service package has been published or installed
  merely to change its version string.
- The real Windows installation passed native registration repair without closing Chrome,
  native connector crash recovery, service crash recovery retaining both native port and
  initialized MCP stream, all 23 tools, and the Foundry CLI story.
- Real prompt testing found and fixed an early return in `Page.javascriptDialogOpening`.
  Prompt detection, response, and dismissal passed after the owner reloaded the extension.
- Iframes and returned text/values were checked through installed Chrome. Deeper governed
  iframe exclusion, masking, and stale-frame cases passed in isolated Chromium with a test
  transport; they still need installed-native-host evidence.

The Windows `.tmp/` reports, `dist/` ZIP, binaries, and browser profiles are ignored machine
artifacts and will NOT arrive with `git pull`. Durable findings and hashes are tracked. Generate
new Linux evidence; never treat absent Windows files as a reason to invent or reconstruct a pass.
Do not read `local/`, `private/`, or `saps/` without the owner's explicit authorization.

## First: inventory and run existing gates

Record the actual distro, architecture, desktop/session type, browser packaging and version,
Rust/Node/npm/PowerShell versions, and whether a real visible desktop is available. Inspect
existing Ghostlight image paths and registrations before installing or stopping anything.
Use the repository toolchain pin and the current workflow's dependency list. The release builder
baseline is Ubuntu 22.04; Debian 12 and Ubuntu 24.04 package smokes do not replace visible desktop
acceptance. Do not claim those environments were exercised merely because a different distro ran.

In a separate shell from the installed product, set an isolated build directory and an actual
Chrome for Testing executable with unpacked-extension support:

```sh
export CARGO_TARGET_DIR="$PWD/.target-linux-integration"
export GHOSTLIGHT_TEST_BROWSER="/absolute/path/to/chrome-for-testing/chrome"
node tests/hardening-suite.mjs
```

Replace the browser placeholder with the verified installed path. The runner builds exact
binaries, supplies its own `GHOSTLIGHT_BIN_DIR`, fingerprints source, and saves unique evidence
under `.tmp/hardening-suite/`. Its Windows desktop lane does not run on Linux, so expect 18 gates
with the current runner, not a copied Windows count of 19. Missing prerequisites are failures or
blocked evidence, not skipped acceptance. Do not edit tested source during a final evidence run.

Also validate the platform scripts and repository truth:

```sh
for script in scripts/*.sh; do bash -n "$script" || exit; done
pwsh -NoProfile -File scripts/check-public-surfaces.ps1
pwsh -NoProfile -File scripts/check-repository-integrity.ps1
```

Xvfb/container execution is useful for isolated process/package checks. It cannot establish
actual tray, Applications, Wayland activation, ordinary browser profile, or installed-client
behavior. An unavailable visible desktop blocks those rows, not the independent source work.

## Next: installed Linux journeys

Use a dedicated test account/profile for destructive lifecycle and governed-policy scenarios.
Ordinary smoke tests may use an authorized idle development installation with disposable pages.
Identify the installation's three real sibling paths first. Run the actual supported installer
and native-host registration seam, then load the intended adapter through the browser's supported
route. A development unpacked adapter is development evidence; store delivery remains separate.

Run these with the observed installed directory, not a build directory selected by convenience:

```sh
GHOSTLIGHT_BIN_DIR="/absolute/installed/sibling/directory" node tests/live-journey.mjs
scripts/demo-foundry.sh --ghostlight /absolute/installed/sibling/directory/ghostlight
```

Remove test-only runtime/native-host redirection from this shell before installed acceptance.
`live-journey.mjs` requires all-open authority, creates disposable pages, and does not change
policy. It verifies returned content and real effects, including framed form completion, editor
values, upload bytes, dialogs, screenshots and GIF output. The Foundry story resizes its browser
window and leaves a demo tab. Respect preserve-tabs and never reload unrelated unsaved pages.

Build a small Linux installed-recovery journey using the Windows journey's observable assertions,
not its registry implementation. It must use real Linux manifests and registered native messaging:

1. Record exact browser, authority, native connector, and initialized MCP identities.
2. Remove and restore only this test installation's owned native registration; preserve original
   bytes and ensure cleanup on failure. Chrome must discover the host without a restart or reload.
3. Stop only the observed connector, checked by executable path and process start identity.
   Prove automatic replacement while the browser stays alive.
4. Stop only the observed authority. Prove demand-start, the same native port, and fresh browser
   work on the same initialized MCP connection. Do not replay an uncertain request.
5. Add an actual in-flight browser-effect interruption with independent effect counting, plus
   worker suspension/recovery. This was not closed by the Windows installed run.

Prioritize both COLD installation orders on clean test state with Chrome already running:
service first then extension, and extension first then service. Warm unregister/reinstall is not
cold installation. A complete Chrome shutdown is not an acceptable installation requirement.
Record latency and failure evidence before changing or restarting anything.

Then execute the matrix's remaining installed cases: two Chromium families and plural profiles/
windows; at least three actual MCP clients; configured RAWX allow/deny, excluded iframe content,
masking and returned coverage; real Pause/Stop/session recovery; privacy sentinels; and visible
Linux workbench/tray/non-tray behavior. Raw MCP JSON exchange is not a real-client acceptance test.

## Package lifecycle and remaining failures

Build or obtain exact 1.3.5 candidate artifacts through the existing release process. Record
artifact hashes and source identity. Exercise clean install, upgrade from public 1.3.4, uninstall,
and reinstall for the supported Debian, portable, and npm routes. Verify foreign entries and
unrelated config survive, and record the audit-retention choice. No developer toolchain should be
needed on the clean package-consumer machine.

`scripts/check-debian-package-lifecycle.sh` is a DISPOSABLE GUEST/CONTAINER script: it purges an
existing package, edits dpkg exclusion configuration, and stops packaged authorities. Do not run
it on the owner's workstation or a shared live host. Its virtual-display smoke is supplemental;
run separate ordinary-user visible desktop acceptance. Never run the product as root merely to
hide a user-cache or graphical-session failure.

Keep these observed failures open until explained or properly reproduced and fixed:

- Original Windows `Not installed here` despite apparently valid registration; a later full
  browser shutdown recovered it, but no root cause was proven.
- One post-navigation `document scope changed` failure before an explicit readiness wait.
- One public iframe demo text-wait timeout during a concurrent fresh Rust build. Contention was
  not proven. A later installed run passed without compilation. Do not erase the failed run,
  silently widen deadlines, or automatically retry uncertain effects to manufacture a pass.

## Finish and report

Keep small fixes at their owning seams. Apply the normal per-commit gates in `AGENTS.md`, test
the real affected boundary, and update `docs/STATUS.md` plus a dated Linux evidence record with
pass/fail/blocked rows, artifact/source hashes, exact environments, retained failures, and next
steps. Keep secrets and arbitrary browser content out of committed evidence.

This continuation is testing and release preparation. Do not publish service packages, merge
`main`, change observed public-version metadata, or cancel/replace/publish Google's pending
adapter without an explicit release instruction. An extension fix during review needs owner
coordination: it changes the artifact Google is reviewing. Do not bump versions again simply
because testing moved to another machine.
