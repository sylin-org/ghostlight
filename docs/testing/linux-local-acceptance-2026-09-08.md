# Additional local Linux acceptance, 2026-09-08

The owner requested every remaining locally feasible Linux check after the earlier
[Linux integration run](linux-integration-2026-09-08.md). This record separates source,
development installation, public delivery, and disposable package-consumer evidence.
None of these results is a release approval or a store-adapter acceptance result.

## Scope and custody

- Source base: `38aaa4971cf1bb6d5dd5d2629727c9dc2e749504` on `dev`.
- Delivery repairs: local commits `4a7f4dc` (portable archive) and `c6128d0` (shell installer).
- Host: CachyOS x86_64, KDE Wayland; native Chromium 152.0.7977.75.
- Adapter: unchanged source 1.1.2 with its pinned development identity. Google's submitted ZIP
  and all extension source remained unchanged.
- Dedicated ordinary XDG directories, browser profiles, and sibling sets live under
  `.tmp/linux-local/`. Installer checks mount a disposable home with bubblewrap. The owner's
  installation, browser profiles, MCP configuration, and machine-local notes are outside that work.
- The host-built sibling hashes are those in the earlier Linux record. They require GLIBC 2.39
  and cannot prove the older release baseline. A separate Ubuntu 22.04 build below addresses it.
- Reports and test drivers remain local under `.tmp/linux-local/`; shared evidence carries
  bounded receipts and artifact identities, not page contents or screenshots.

## Two delivery defects fixed

### Portable archive process identity

Two calls to the unchanged portable packager, with identical binaries and PowerShell 7.6.5,
produced different hashes:

- `0bf38de37ede1ac689699bbfa983ff3cfc57d7dcae0c0570fce3d4e274141f14`
- `61cf70faf00b9eb951acd240635f9042e11b36475569cfb9fcb5b82826192288`

The decompressed tar streams differed in seven extended headers: .NET named each one with
`PaxHeaders.<process id>`. File payloads, modes, and timestamps matched. The fixed, short roster
fits Ustar, so the packager now uses Ustar entries without process-specific extended headers.
Two new processes produced the same archive:
`6e4fec3d81c80693e645a49a156adf1728229503a375cb2076149cb5aee126a9`.
The original and fixed archives remain under `portable-a`, `portable-b`, and `portable-fixed-*`.

`tests/portable-package.ps1` exercises separate packager processes and verifies exact payloads,
file roster, owner ids, timestamps, and executable/legal-file modes. It is part of the full suite.
This proves repeatability with the recorded runtime, not identical compression across runtimes.

### One-line installer release resolution

The actual public one-line installer stopped with `release checksums resolved to an unexpected
URL`. GitHub's asset redirect ends at its CDN, so the final asset URL cannot identify the release.
The installer now resolves the latest release page, validates its stable tag, then pins that tag
for the checksum manifest and all three binaries. Version mismatch is checked before downloads.

The fixed script completed against public 1.3.4 in a disposable home. `tests/installer-shell.mjs`
adds offline coverage for CDN delivery, one pinned release, exact siblings and modes, invalid
release hosts/tags, requested-version mismatch, and checksum refusal. The real network run is
separate evidence under `oneline/`; its initial failure remains there too. No public script or
package was published by this work.

## Actual installed browser and desktop results

| Local lane | Result and independently observed boundary | Evidence |
| --- | --- | --- |
| Configured governance | 14 checks passed: permitted RAWX and retained edits; each missing capability denied without effect; restored authority retains the draft; excluded-frame read/script handling; delivered JPEG pixels masked; allowed child read; stale embedded target refused; complete-page exclusion; redirected landing refusal; popup Pause/Resume/Stop/new-session; preserved tab; audit/log sentinel absence. | `governance-1788900362505/results.json` |
| Plural browsers | Six checks passed: Chromium and Brave retain separate workspaces/drafts; a second ordinary Chromium profile is independent; moving a controlled tab between windows retains ownership; extension reload retains the draft and permits fresh work after native negotiation; absent Brave never reroutes its pinned workspace; Brave restart retains adapter identity and initialized MCP for new work. | `plural-1788901146664/results.json` |
| No-tray desktop | Private session bus has no StatusNotifierWatcher. Real KWin windows pass Open/focus, minimize/Open, close/Open, eight concurrent Open requests, exact descendant WebKit renderer termination and explicit recovery, and Applications launch into the same authority. | `no-tray/desktop.json` |
| Portable removal | Actual installation into a disposable home preserves a foreign Claude server. Uninstall preserves foreign native-host and command entries, unrelated client state, and byte-identical audit; reinstall restores owned surfaces and retains audit. | `no-tray/lifecycle.json` |
| Desktop startup failure | Missing Wayland display returns a failed Open in 15.039 seconds. A subsequent ordinary Open with the valid desktop environment succeeds. | `startup/results.json` |
| Public npm route | Real npm global entry downloads and verifies public 1.3.4 siblings, initializes MCP, and opens/reads/captures a disposable document through the registered native host. Adapter is source 1.1.2. | `npm/native.json` |
| Public one-line route | Fixed installer downloads all three public 1.3.4 siblings into a disposable home. Their bytes match the independently installed npm route. | `oneline/install-fixed-2.log` |

Brave 1.94.121 was extracted from its vendor Debian package into the test area, without modifying
host package state. Package SHA-256:
`21d7ac36b64a408dc598bb6ec3db84b07b2cbca854d26b28055a2fb5b94a2e77`.
It reports `Brave Browser 152.1.94.121 unknown` and uses the ordinary Brave native manifest path.
This proves the real Brave browser/native boundary on this host, not its distro installation.

Both public delivery routes produced these identical executable hashes:

| Executable | SHA-256 |
| --- | --- |
| ghostlight | `4387188681a7eab38f3ab21598d4cd9fe158dc49c82c1903f39f5451f3ca8d1a` |
| ghostlight-mcp-connector | `ffa1bc7a1922433a5070601a1dc0238306a4b771db221e7718011fd488ef1ada` |
| ghostlight-browser-connector | `f2878c3de309130229db95e8256670dd95213ee9839af96837aa4d48c4f90279` |

The native governance and plural drivers retain earlier failed runs. Corrections concerned test
catalog names, strict coverage refusing a readiness read, selecting the exact profile worker,
waiting for worker initialization/native negotiation, and distinguishing closed-tab refusal from
absent-browser refusal. These failures were not fixed by relaxing product policy or replaying work.

One reload failed because Chromium disabled the unpacked adapter with reason `16777216`, the
[unsupported developer-extension reason](https://chromium.googlesource.com/chromium/src.git/+/HEAD/extensions/browser/disable_reason.h).
Enabling Developer mode in the dedicated profile and re-enabling that adapter restored the
supported development setup. A custom `--user-data-dir` also lacked its own native registration;
an ordinary second profile under Chromium's normal user-data root passed. Neither observation
explains the earlier Windows installation failure or establishes store-update acceptance.

The private-bus desktop test could not obtain a usable AT-SPI application tree. Its KWin window,
renderer-process, and Applications evidence is valid; it does not claim workbench button or
session-attention recovery acceptance through accessibility. Popup human controls were tested
separately through their real handlers in the installed Chromium adapter.

## Installed signed managed policy

Seven checks passed through the actual registered Chromium native host using a signed file
source. A private mount namespace supplies `/etc/ghostlight`; both native and MCP connectors
inherit it, so authority demand-start after a crash still sees the same organization bootstrap.
The host's `/etc` and the owner's policy were not changed.

- An invalid configured cold source refuses the authenticated service Hello with
  `invalid_authority`. MCP initialization remains pending until a valid signed policy arrives.
- A verified Ed25519-signed source admits that connector and permits real browser navigation/read.
- A managed host denial produces no HTTP request to the denied fixture host.
- Invalid replacement bytes retain the last verified authority and report stale source status.
- Restarting the exact test authority with the source still invalid loads its verified cache;
  the retained MCP negotiates and completes fresh browser work through the native relay.
- A higher signed sequence narrows existing work to read. Execute is refused without effect;
  independent DevTools reads confirm that both fixture documents retain their original titles.
- Replacing the source with the older signed sequence does not expand that authority.

Report: `managed/run-1788904196202/results.json`. Earlier failed fixture runs remain. They exposed
an incorrect assumption that invalid cold policy would admit MCP before refusing an operation,
a mistyped runtime filename, and a restart whose MCP connector had escaped the policy namespace.
The final run corrects all three preconditions. It proves fresh work after restart, not retention
of an old tab handle across authority loss. Organization HTTPS delivery, customer credentials,
and configured credential handoff were not exercised by this signed-file lane.

## Extension-first development installation

Fresh XDG contexts began with the source adapter installed, a running Chromium process, no native
manifest, and observed `host_absent`. Installing the local portable service connected automatically
and completed a first browser task without restarting Chromium or reloading the adapter.

- Awake worker: connection in 3,633 ms.
- Explicitly stopped worker: connection in 3,621 ms. Its pre-existing absence/reconnect alarm
  wakes it after installation; this does not prove idle recovery after stopping a connected worker.
- Report: `cold-1788902552150/results.json`.

Earlier cold-run failures remain: polling readiness before an authority existed needed to tolerate
an absent readiness value, and an attached worker debugger prevented the intended stopped-worker
precondition. Detaching that debugger before the stop established the real stopped state. These
are development installation results; the adapter did not arrive from the staged store package.

## Baseline package build and consumer checks

A rootless user namespace contains a disposable Ubuntu 22.04 root filesystem. It shares neither
host package state nor host processes. Build tools use mapped root inside that guest; package
journeys launch Ghostlight as ordinary uid 1000 in their own disposable namespaces.

- Canonical Ubuntu rootfs SHA-256:
  `b07e12931a86b14d224c92d2c60e50b35a6d10e22aec4e42ea35757e86691b26`.
- Rust 1.95.0, locked source dependencies, Tauri CLI 2.11.0, and the existing sidecar,
  finalization, and package-inspection scripts were used.
- All three baseline executables require GLIBC 2.34, within the 2.35 ceiling.
- Finalized local Debian package SHA-256:
  `6af66bf13fb42453a736fe547230349110d91da5dabd20e526f3d2f6f84c0333`.
- `check-native-package.ps1` passed against that exact package.
- Debian 12 and Ubuntu 24.04 package-consumer lifecycle checks both passed. They use official
  image layers verified against their registry manifest digests. No Rust or Node toolchain is
  mounted into those consumer guests. Both runs prove ordinary-user launch and MCP initialization,
  system manifests, dependency and GLIBC bounds, runtime permissions, desktop/icon/manual assets,
  package removal/reinstall/purge, and retained user runtime state. Virtual-display results remain
  supplemental to visible desktop acceptance. Logs: `debian-lifecycle.log`, `ubuntu-lifecycle.log`.

The public 1.3.4 Debian upgrade predecessor has SHA-256
`30e525d6cd21da30c6569e713a53e9e892adb8b1f0f2aaf325086a0c76eea401`.
Build, package, image custody, and consumer logs remain under `guest/`.

## Forward package upgrade and rollback limit

Both consumers also passed public 1.3.4 to local 1.3.5 upgrade on fresh ordinary-user state.
Unrelated MCP configuration and a foreign native manifest remain byte-identical. Package update
alone leaves the old running authority at 1.3.4. After explicitly ending that test authority and
using normal Open, the same initialized MCP process negotiates with 1.3.5, retrieves the current
23-tool catalog, and completes new policy inspection. No browser ran in this supplemental upgrade
lane. Logs: `guest/debian-upgrade-5.log` and `guest/ubuntu-upgrade-5.log`.

The [installation guide](../guides/installation.md#update-and-uninstall) now names the required
post-update Quit/Open and client-reconnect steps. A new executable's `--version` does not identify
the still-running service. The predecessor package also passed GitHub provenance verification;
its local 1.3.5 successor has only local build custody.

Earlier runs remain failed evidence. Startup must establish the actual old authority before the
MCP can independently demand-start one. After service restart, a listening new service does not
prove the retained MCP has finished negotiation; a safe catalog read establishes that boundary.
The runs retain their transient `service is reconnecting` refusals rather than replaying a browser
operation or silently widening deadlines.

Repeating an upgrade fixture by downgrading it exposed a separate real limitation: public 1.3.4's
strict history loader cannot read the new `permissions` field in 1.3.5 audit. The older authority
then fails at startup. Forward upgrade passes with fresh predecessor state, but rollback with
newer history is not proven. Preserve that history and the failed log; do not claim transparent
rollback or remove audit to make the test pass. The final forward runs use new ordinary-user state
and leave the failed downgrade fixture intact.

## Stable source validation

All 20 Linux hardening gates pass on unchanged source after the final installation-guide update:

- Report: `.tmp/hardening-suite/2026-09-08T21-31-40-763Z-113207/results.json`.
- Source fingerprint: `d5b65ea751a6d6b00c1b9eb1db7f4ea772d3b0846a8910efe473fabbf137162e`.
- Formatting, warnings-denied Clippy, all Rust and extension tests, launcher checks, both new
  packaging checks, fresh build, process/continuity/provenance, both CLI lanes, workbench, browser
  harness, and real Chromium script/frame/history lanes passed.

## Remaining release evidence

These local results still do not establish provenance-bound candidate acceptance with the staged
store adapter, visible Ubuntu GNOME Wayland acceptance, or three actual MCP application journeys.
The public npm/one-line results are 1.3.4; candidate 1.3.5 delivery needs its complete real artifact
set. Ordinary desktop accessibility/session-attention recovery, configured credential handoff, and
organization HTTPS policy delivery remain distinct from the passing popup/local and signed-file
policy checks. The original Windows
installation failure and retained timing observations remain unresolved. No release, store action,
public-version metadata change, or additional push was performed.

## Cleanup

All dedicated browsers were closed through DevTools after matching their browser process ids.
The ordinary test authorities were stopped through their own tray Quit actions. The remaining
exact-path no-tray test process was terminated after its private session bus had ended. No test
authority remains. One abandoned cold-policy fixture connector was terminated by its exact image
and process id after its deliberately inadmissible initialization remained pending. The owner's original authority and MCP connector retain their original process
start identities. Evidence: `browser-cleanup.json`, `cleanup-before-managed.json`, `cleanup.json`, and
`managed/abandoned-initialize-cleanup.json`. Local reports, profiles,
packages, and disposable root filesystems remain under the ignored test area.
