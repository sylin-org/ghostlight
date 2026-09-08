# Pre-release integration acceptance

Requested by the owner on 2026-09-08 after a fresh Windows installation failed to connect
until Chrome was fully closed. The original failure is still unexplained. A later successful
reinstall is useful regression evidence, not proof that the original failure is fixed.

This guide makes the browser and package journeys in [RELEASE.md](../RELEASE.md) executable
where possible. It does not replace the historical 1.0 release checklist or authorize publication.
The supported release matrix remains Windows and Linux.

## Evidence rules

- Test the exact candidate artifacts. Record source revision, dirty source fingerprint, executable
  and adapter hashes, package hash, OS, desktop, browser version, installation route, and MCP client.
- A green helper, fake browser receipt, replacement native pipe, or unpacked adapter cannot pass
  an installed-package or store-adapter gate. Keep those tests for their narrower contracts.
- A successful invocation is not sufficient for a mutation: inspect the retained document value,
  caret, file bytes, dialog answer, pointer effect, recording, or page completion.
- Report each gate as passed, failed, or blocked, with evidence and remaining prerequisites.
  Missing hardware, credentials, client, store candidate, or browser is blocked, never passed.
- Retain failed runs. A successful rerun records what changed; it does not erase an unexplained
  intermittent failure. Do not replay uncertain effects to obtain a green run.
- Store only bounded operational receipts and artifact identities in shared evidence. Keep
  screenshots and page content local; never collect unrelated tabs, profiles, or credentials.

## Run the Windows development lanes

First load the current unpacked extension and leave ordinary installed Chrome open. The installed
journey requires one idle Ready authority, one native connector, one Chrome root, and four ordinary
Ghostlight-owned browser registrations pointing at the requested binary directory. It refuses a
foreign installation or redirected runtime. Run it with no other Ghostlight jobs in progress.

```powershell
pwsh -NoProfile -File tests/installed-windows-journey.ps1 `
  -BinDir target/release `
  -ChromePath 'C:\Program Files\Google\Chrome\Application\chrome.exe' `
  -ExerciseInstalledStack
```

This deliberately removes and reinstalls the real native registrations, crashes the exact
connector, then crashes the exact authority. It verifies that Chrome never restarts, registration
bytes return unchanged, and service recovery retains the native connector and initialized MCP
stream, which performs fresh browser work after recovery. It then runs the
installed MCP browser journey. Registrations are restored on ordinary errors; interruption of the
PowerShell process itself can defeat cleanup. Recovery is the same installation's
`ghostlight native-host install`. No browser restart should be necessary.

Evidence is retained per run under `.tmp/installed-windows/`. The report always keeps
`release_ready:false`: this is a development installation, not the clean-machine/package matrix.
The browser journey can also run independently:

```powershell
$env:GHOSTLIGHT_BIN_DIR = (Resolve-Path target/release).Path
node tests/live-journey.mjs
```

That lane exercises all 23 advertised tools through actual MCP stdio, the installed authority,
registered native executable, and loaded extension. Its disposable localhost documents test
retained editor values, shadow/frame composition, typing, keyboard caret, drag effects, byte-exact
uploads, dialogs, diagnostics, zoom/scroll, history, recording and erase, flow effect truth,
screenshots, and stale input rejection. The public Sylin iframe demo verifies composed form
completion without sending an application. Browser tab preservation remains authoritative.
The lane requires all-open policy and does not modify the person's configured authority.
Its default report is a unique file under `.tmp/installed-browser/`.

Also run the existing source/process/browser regression suite on the same stable tree:

```powershell
$env:CARGO_TARGET_DIR = '.target-integration'
$env:GHOSTLIGHT_TEST_BROWSER = '<Chrome for Testing executable>'
node tests/hardening-suite.mjs
```

This builds and identifies its exact binaries. Its process framing and saturation tests use
controlled peers; the isolated Chromium journeys use a test native pipe. Their source fingerprint
and results do not establish real installation acceptance. Native desktop tests and installed
browser tests provide different evidence. Do not combine those claims into one misleading count.

The Foundry demo scripts additionally exercise the public stage through the actual CLI. They use
the current flow catalog and configured authority. Their old caller-authored domain restriction
is retired; configured-policy denial needs the separate governed lane below.

The 2026-09-08 Windows run passed the installed journey, all 23 tool invocations, all 19 hardening
gates, and the PowerShell Foundry story. It found and fixed a real dialog-event early return and
replaced the npm test's Windows symlink assumption with a real offline installation. See
[STATUS.md](../STATUS.md) for exact evidence and retained intermittent failures. This result does
not close the clean-package, store, multi-client, or Linux rows below.

## Candidate matrix

### Installed Linux recovery runner

`tests/installed-linux-journey.mjs` exercises the real Linux manifest and native-pipe boundary.
Run it only against an idle development installation in a dedicated browser context:

```sh
node tests/installed-linux-journey.mjs \
  --bin-dir /absolute/installed/sibling/directory \
  --browser /absolute/running/browser/image \
  --exercise-installed-stack \
  --devtools-port-file /absolute/dedicated/browser/DevToolsActivePort
```

The browser argument is its actual process image, not a distribution's shell wrapper. The runner
requires all four ordinary per-user manifests to name that exact sibling set, one matching browser
root, one Ready authority, and one native host descending from that browser. It checks process
start identities before stopping anything and restores original registration bytes on failure.
It refuses Ghostlight runtime/native-host/profile/policy redirection. Standard XDG paths remain
the environment's ordinary user-context paths; the report records the configuration root.

The journey removes/restores registrations with Chromium running, crashes the native connector,
crashes the authority while retaining native pipes and an initialized MCP stream, and interrupts
a real page effect counted independently by a local fixture server. The optional DevTools file
enables forced worker-stop testing in the dedicated browser. A pinned workspace first refuses a
replacement profile without effect; a normal new-tab event wakes the same adapter, and fresh
work completes without replay. This is event-driven recovery, not proof of idle reactivation.
Without that explicit browser control endpoint the worker phase is blocked and the run is not green.
The fixture tab remains preserved. Results live under `.tmp/installed-linux/`.

The 2026-09-08 CachyOS run passed all five phases with release-built 1.3.5 siblings deployed via
the dev-loop, native Chromium, the source 1.1.2 adapter, and fresh XDG directories. It also passed
the existing 23-tool live journey and shell Foundry story. This is actual native messaging in a
development context, not a clean account, a candidate package, or store delivery. See the
[Linux evidence record](linux-integration-2026-09-08.md).

The [additional local Linux run](linux-local-acceptance-2026-09-08.md) adds configured native
RAWX/privacy and signed managed-policy checks, Chromium plus Brave and a second ordinary profile, renderer-loss recovery,
Applications launch without a tray host, portable ownership-safe removal, actual public npm and
one-line delivery, and local baseline Debian package-consumer evidence. These narrower passes
retain the candidate/store/visible-GNOME/actual-client requirements in the matrix below.

### Required release rows

Run each row on Windows and visible Linux using the release's supported routes. One successful
example of each tool does not prove every variant or the workflows below.

| Gate | Real journey and pass criterion | Existing support / remaining work |
| --- | --- | --- |
| Cold install, service first | On a clean ordinary account, leave Chrome running, install candidate service, install store adapter, then complete a first browser task. No browser shutdown, reload, or options ritual. | Dedicated clean machine and candidate adapter required; warm reinstall is insufficient. |
| Cold install, extension first | Install adapter before the service with Chrome already open. Observe absence, install service, and complete a first task automatically. Record latency and original absence evidence. | Same clean environment; test both worker-awake and worker-suspended cases. |
| Delivery routes | Native installer, portable/one-line installer, npm launcher, and Windows MCPB install exact siblings and resolve actual native host paths. | Candidate inspection scripts plus actual installed jobs for each published route. |
| Upgrade and recovery | Upgrade latest supported public artifacts to candidate with browser and MCP clients open; no lost drafts, wrong-tree adoption, stale executable, or replay. Reinstall repairs owned missing registration. | Installed Windows journey covers warm registration repair only; packaged upgrade remains required. |
| Uninstall/reinstall | Remove only owned registrations, integration entries, and files; preserve foreign entries, config comments and unrelated servers. Honor audit-retention choice. Reinstall works in the still-open browser. | CLI/process ownership tests supplement, but do not replace, actual package removal. |
| Process continuity | Kill native connector and authority separately; browser reconnects automatically. Same initialized MCP connection accepts new work after recovery. Interrupt an actual page effect; count it once and report uncertainty without replay. | Installed Windows journey covers native-port and initialized-MCP continuity; existing process suite covers no-replay with controlled peers. Actual in-flight browser interruption remains required. |
| Browser lifetime | Browser restart, extension reload/update, suspended-worker recovery, two profiles, two windows, and two supported Chromium families. Existing drafts survive. | Controlled browser restart must use a dedicated test profile, not the owner's unrelated pages. |
| Harnesses | At least three supported real MCP clients discover tools, perform a first task, handle image and textual receipts, preserve config, and reconnect. | Raw MCP transport coverage is supplemental; client registration alone is not acceptance. |
| Browser capabilities | Every catalog tool plus semantic/coordinate input, nested frames and shadow roots, capture reuse/upload, dialogs, recording delivery/erase, history, child-tab adoption, stale handles, and bounded diagnostics. | Expanded installed journey plus Foundry and the existing frame/script journeys. Mark unsupported variants separately. |
| Governed work | Configure real local/managed policy in a dedicated test account. Exercise independent RAWX, all-open, allow/deny, redirects, excluded documents, bad cold policy, last-valid reload, and configured credential handoff. | Isolated governance and real Chromium tests already exist; repeat key allowed and denied jobs through the registered native host. Never edit the owner's policy for this. |
| Human controls | Actual Pause/Stop/session recovery prevents subsequent effects and remains usable during load. Preserve-tabs blocks close. Workbench controls and notification match receipts. | Existing process/desktop/history tests supplement actual visible installed interaction. |
| Privacy and limits | Test sentinels absent from audit/logs after failed flows, diagnostics, uploads, dialogs, and recording. Large work remains bounded; cancellation, unknown effects, and history remain truthful. | Existing hardening tests plus installed content-free evidence; inspect only the dedicated test account's records. |
| Desktop and packaging | Open/hide/close/reopen/Quit, tray and non-tray routes, startup failure, image/icon identity, and package removal in the real desktop. | Native Windows journey; visible Linux journey below. |

## Linux release gates

Linux acceptance is blocked until a real Linux desktop and exact candidate artifacts are available.
WSL, containers, Xvfb, and Windows Chrome cannot establish these results.

1. Build and run existing source/process/Chromium tests on Linux. Pass the actual build directory
   through `GHOSTLIGHT_BIN_DIR`; pin and record the browser. Run shell syntax and launcher tests.
2. On the supported oldest Debian/Ubuntu baseline and the currently supported GNOME Wayland
   environment, install the exact `.deb` as documented, then launch as an ordinary user. Use
   `scripts/check-debian-package-lifecycle.sh` for its package assertions. Verify all three
   executables run without a developer environment and without write access to system binaries.
3. Test portable and npm routes separately. Verify one user-writable runtime, correct XDG desktop
   entry/icon, Applications Open, disposable workbench reopen, tray where provided, and a usable
   non-tray route. Exercise renderer loss and desktop startup failure visibly.
4. Run both cold installation orders above with native-package Chrome and a second supported
   Chromium family. Verify system/per-user native-manifest precedence and actual connector path.
   Confined browser packaging must produce the documented supported behavior or a clear supported
   route; do not relabel a confined-browser limitation as successful connection.
5. Run the installed `tests/live-journey.mjs` and `scripts/demo-foundry.sh` against the registered
   native host. Run the governed, plural-browser, three-client, recovery and privacy rows above.
6. Upgrade from the supported public version, uninstall, and reinstall with the browser alive.
   Verify foreign state and audit retention. Attach package hashes and visible desktop evidence.

Publication remains blocked by any required failed or unevidenced candidate row. This guide does
not turn a successful development run into a release approval.
