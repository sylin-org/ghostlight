# Four-machine acceptance campaign

Date: 2026-09-09. Status: execution assigned on all four machines; baseline results pending.

The owner assigned test-01, test-02, test-03, and leo-desktop-02 to real-environment Ghostlight
testing and packaging. This campaign follows the existing
[pre-release evidence rules](pre-release-integration.md). It does not declare new platform
support or authorize external package publication.

The owner explicitly authorized full control of these dedicated machines by the coordinator
and each machine's local agents. This includes prerequisite installation, desktop and browser
interaction, builds, local package installation, test settings, and controlled process crashes,
browser restarts, reboots, upgrade, uninstall/reinstall, and recovery. Agents should use this
authorization without repeatedly asking for routine test actions. Preserve baselines and
failed state, respect enforced tool restrictions, and keep private material out of evidence.

## Initial state and roles

Remote task access is verified. Fresh baseline reports must establish the current distro,
desktop, host/container boundary, browser packaging, installation, privileges, and reset options
before a test assignment changes anything. The owner identifies leo-desktop-02 as having no
Ghostlight installation; absence of developer runtimes has not been established.

| Machine | Known repository | Intended role, subject to inventory |
| --- | --- | --- |
| leo-desktop-02 | `D:\repo\github\sylin-org\ghostlight` | Windows customer installation, then delivery and upgrade lifecycle |
| test-01 | `/run/media/test/WORKBENCH/repos/github/sylin-org/ghostlight` | Established Linux regression and package baseline |
| test-02 | `/var/home/test/repos/github/sylin-org/ghostlight` | Linux desktop/package lane; confirm distribution and host/container boundaries |
| test-03 | `/home/test/repos/github/sylin-org/ghostlight` | Linux desktop/package lane; confirm distribution and libc |

Assign Bluefin, if confirmed, to atomic-host, GNOME, browser sandbox, and container/client
integration. Assign Alpine, if confirmed, to musl, GTK/WebKit, native messaging, and APK
feasibility. Assign the established CachyOS desktop to the broad regression and Arch package
lane. Do not assign OS names from path shape alone.

The coordinator's first assigned source round is `a8cfd0337d83079f5fad3d5dfec846c31ba3b8fd`.
Capture each checkout before updating it; several remote tasks last reported older `main`.
Record exact sibling, package, and loaded-adapter hashes. Current source adapter metadata is
still 1.1.2, but its bytes differ from the submitted 1.1.2 ZIP. Store delivery and source-adapter
development acceptance are separate rows. Google's public update feed served 1.1.1 on this date.

## Sequence

1. Capture read-only baselines. Preserve a machine image or snapshot where available, especially
   before spending the fresh Windows install. Establish how remote access survives reboot.
2. Test a customer installation on Windows before adding build tools or repairing dependencies.
   Use packaged artifacts built elsewhere and a browser started independently through the
   normal desktop. Measure first useful task and whether manual intervention was necessary.
   Missing runtime prerequisites must be identified or installed by the supported delivery route.
3. Test the opposite service/extension installation order only after a verified reset to a
   clean baseline. Uninstall/reinstall is not a substitute for a fresh machine. Keep awake-worker
   and suspended-worker extension-first cases distinct.
4. Run the common installed journey on every viable environment. Begin Linux feasibility work
   in parallel with Windows customer testing; a platform that cannot run the whole desktop
   product receives a precise blocked result before new packaging work begins.
5. Deepen each machine's unique lane. Spread real-client and browser diversity across the fleet
   rather than running every possible combination. Keep one common client/browser pairing as
   a comparison baseline where supported.
6. Run disruptive lifecycle cases last, on an idle test installation: exact-process crashes,
   sleep/resume, reboot, upgrade, uninstall/reinstall, and ownership-preserving cleanup. Reserve
   explicit windows for events that interrupt remote access or interactive work.
7. Fix shared product defects centrally and platform defects at their owning seam. Freeze a
   new candidate after each change, rerun the original failing case, then rerun affected common
   cases on the other machines. A final acceptance round uses unchanged candidate artifacts.

## Common evidence

- Correct installation paths, native registration, demand-start, and one authoritative process.
- Actual supported MCP-client discovery, first useful job, text/image delivery, and reconnect.
- Retained document values, frames/shadow content, uploads, dialogs, screenshots, recordings,
  and stale-reference rejection through the installed service and real native messaging.
- Human browsing is outside Ghostlight work: navigation, protected URLs, child tabs, movement,
  and closure create no passive action receipts, policy holds, or automatic acquisition.
  Include overlap with an in-flight agent operation. The next agent action still checks the
  current destination. ADR-0164 and current ACCEPTANCE item 15 govern these expectations.
- Agent-only policy denial, pause/stop, preserve-tabs, and truthful unknown effects without replay.
- Visible desktop Open/minimize/close/reopen, Applications entry, focus, scaling, and tray/no-tray
  behavior appropriate to the actual compositor.

## Reuse and adaptation

Use `tests/live-journey.mjs` with the exact installed `GHOSTLIGHT_BIN_DIR` for the common
browser journey. Use `tests/installed-windows-journey.ps1` and
`tests/linux/installed-journey.mjs` for their documented warm recovery cases after baseline
acceptance. Missing worker-control access blocks that phase, not unrelated tests.

The drivers under `tests/linux/` are reusable components, not a universal setup script.
Some launch dedicated browsers, mount disposable homes, or assume KWin, qdbus6, bubblewrap,
prepared guest roots, and native Chromium. Adapt and label their scope before use. Run
`scripts/check-debian-package-lifecycle.sh` only in disposable Debian/Ubuntu consumers; it
purges packages and modifies system configuration. Do not run it on an ordinary desktop.

Release, pre-release integration, and adapter acceptance prose now follows ADR-0164 rather than
expecting implicit child-tab adoption. Discover the live catalog rather than copying an old tool
count. Raw MCP runners supplement, but do not replace, actual-client tests.

## Packaging scope

Prove existing Windows installer, portable, npm, and MCPB routes and existing Linux portable,
npm, and DEB routes before adding formats. Use disposable consumers for dependency and package
lifecycle breadth, and physical desktop sessions for visible behavior. A container pass does not
prove the host desktop. Bluefin does not stand in for conventional Fedora RPM acceptance; Alpine
does not stand in for a glibc Linux target. APK, RPM, and Arch package work starts from verified
whole-product feasibility and produces local candidates before any publishing decision.

## Coordination and results

Each remote task owns its machine's execution and local artifacts. The coordinator owns source
rounds, cross-platform regressions, integration, and a consolidated result matrix. Platform
patches should use separate branches/worktrees only after baseline capture and should be
integrated before freezing the next round. Do not let test machines independently modify shared
product semantics or silently test different revisions.

The owner subsequently requested more autonomy. Agents own the full lane, choose their test and
installation sequence, install needed tools, fix concrete product/platform/test-driver failures
at their owning seam, make local signed-off commits, and verify fixes without waiting for a
coordinator checkpoint. Capture both the initial candidate and each changed revision. Keep
established architectural decisions intact; escalate material product changes and unresolved
restrictions or missing resources, while continuing independent work. The coordinator integrates
shared fixes and freezes the final fleet round after those local investigations.

The ten-minute scheduled check-in resumes idle agents only when meaningful work remains and
collects evidence without repeatedly interrupting active execution. Once campaign work and
consolidated results are in place, delete automation `follow-ghostlight-fleet-testing`; do not
leave a recurring check or merely paused task behind.

The owner also authorized provisioning local Ghostlight Git credentials to all four machines
so they can commit and push as him. The coordinator verified source account `lbotinelly` has
push access to `sylin-org/ghostlight`; repository commit identity is Leo Botinelly. Each agent
may push its own campaign branch after verification. Use secure credential helpers and an
authenticated confidential transfer or encryption to a destination-owned key. Tokens and private
keys must never enter task messages, logs, command arguments, source, Git URLs, or evidence.
Provisioning is pending; the coordinator has requested nonsecret intake state from each machine.

Every result records scenario, candidate identity, environment, installation route, expected and
observed behavior, passed/failed/blocked/not-run status, and bounded evidence coordinates. Retain
failed runs and identify what changed on rerun. Keep screenshots, browser content, personal
configuration, generated packages, and raw diagnostic files in their appropriate local artifact
areas. Success means a reproducible user journey and a repeatable package, not just a green build.

Read-only inventory was dispatched first. After the owner's explicit machine-control grant,
all four tasks received execution assignments, including installation and lifecycle testing.
Their first slice covers established Linux continuity (test-01), immutable-host integration if
confirmed (test-02), alternate-libc feasibility if confirmed (test-03), and fresh Ghostlight
Windows installation followed by upgrade (leo-desktop-02). Each task captures the baseline
before mutations and works on its own branch or worktree at the frozen candidate.

The remote reader returned status but empty contents for baseline turns. Agents were asked
to send compact milestone results directly to the coordinator task and retain sanitized local
evidence. Dispatch and task completion alone do not count as a passed test.

## Current evidence

The Windows reader subsequently exposed its baseline and active execution. Direct milestone
messages from that machine cannot reach the coordinator task, so its supported read channel
is the current result route. Linux turns still expose status without their content. test-01
encountered another tool approval with no visible command or reason. The owner then confirmed
full app permissions; the coordinator relayed that change and instructed the task to resume.
The subsequent snapshot shows a new active turn without an approval flag.

A later immediate check found all four tasks idle. Windows' visible turn ended at the known
evidence-query error; Linux completed turns still returned no report content. All four received
targeted continuation requests and the owner's expanded autonomy. Their completed turns are
not completed acceptance: the matrix below remains provisional until evidence is retrieved.

| Lane | Verified environment or milestone | Acceptance state |
| --- | --- | --- |
| leo-desktop-02 | Windows 11 Pro 25H2, x64; public install and actual VS Code discovery. | Original upgrade failed. Peer FFI and exact-process installer repairs pass source gates; corrected upgrade passes. Final uninstall/refusal and adapter/browser acceptance blocked. Owner credentials verified. |
| test-01 | CachyOS rolling, KDE Wayland, glibc 2.44; real Chromium/Brave and three MCP client families. | Installed 23-tool journey and 21 hardening gates passed with repaired candidate. Reboot, store and accessibility limits remain. Existing owner credentials verified. |
| test-02 | Bluefin 44/Fedora atomic, GNOME Wayland, glibc 2.43; real host service and Codex cold startup. | Codex environment forwarding fixed; review follow-up preserves malformed members. Native visible browser/GNOME coverage blocked. Existing owner credentials verified. |
| test-03 | Alpine 3.24.1, musl, OpenRC, KDE Wayland; native-musl Chromium and service. | Installed browser/recovery/human-boundary and OpenCode/Inspector passed after schema repair. Prototype packaging only. Host unavailable; reboot and credential-helper verification pending. |

Windows candidate worktree creation is confirmed at
`D:\repo\github\sylin-org\ghostlight-fleet-leo-desktop-02`, branch
`codex/fleet-leo-desktop-02`, at the assigned revision. Its local evidence directory is
`D:\repo\github\sylin-org\ghostlight\.tmp\fleet-leo-desktop-02`, including `BASELINE.md`,
`published-package.json`, `desktop-browser-launch.json`, and `extension-baseline.json`.
The public installer hash is
`2c2225a75e208e0164b79d92b413b1b156ce9bd557162e97157009308aed250b`.
An evidence query failed because PowerShell cannot pipe that `foreach` statement directly;
this is a test-script failure, not evidence of an installation defect. Successful native UI
tool calls alone do not yet establish completed installation or first MCP connection.

### Windows findings retrieved at the 21:05 UTC check

The resumed Windows agent reports completed public 1.3.4 installation and actual VS Code MCP
discovery of 24 tools. Its later silent 1.3.5 upgrade returned 0, but the recorded size/hash
still identifies the running MCP connector as 1.3.4. The interactive upgrade had stalled in an
uninstaller that the available native-control surface could not target. Package replacement is
therefore failed/investigating, not passed. The surviving connector did receive the new service's
catalog-change notification; VS Code refreshed from 24 to 23 tools. This is a distinct reconnect
pass. The coordinator has not yet inspected every underlying installation artifact.

The coordinator retrieved the actual Windows source patch and focused test output for a second
defect. In `crates/win-peer/src/table.rs`, the remote agent changed `AF_INET` and the
`GetExtendedTcpTable` address-family argument from u16 to u32. The corrected remote release
test run passed both win-peer tests. The previous suite had reported a win-peer failure and an
orchestrator provenance failure. Full validation, the remote commit and central integration are
pending; the coordinator checkout still has the old declaration.

Browser Use explicitly refused `chrome://extensions/`, including alternate-surface workarounds.
Opening `https://chromewebstore.google.com/detail/lejccfmoeogmhemakeknjjdhkfkgncdl` returned
`Not allowed` (code -32000), without enough detail to classify its cause more narrowly. No loaded
Ghostlight adapter is confirmed, so browser/model effects and human-browsing live acceptance
remain blocked/not-run. Separate execution policy rejected 7-Zip MSI extraction with only
`blocked by policy`; the agent retained the existing package evidence and continued other work.
These enforced restrictions were not bypassed.

The remote checkpoint is `.tmp/fleet-leo-desktop-02/CHECKPOINT.md` in its coordinating checkout.
Its evidence pointers include `checkpoint-installed-files.json`, `checkpoint-registration.json`,
`candidate-artifacts.json`, `candidate-rust-tests.log`, `win-peer-tests.log`,
`default-upgrade-stall.json`, `silent-upgrade-result.json`, and `upgraded-file-check.json`.
Presence and contents of every named file have not all been independently retrieved.

### Linux reports retrieved through Git at 21:30 UTC

Completed Linux task turns still return empty contents, but the agents pushed sanitized reports
and signed-off fixes through their authorized branches. The coordinator fetched and read:

- `origin/codex/fleet-test-01` at `9da0bbf7`, report
  `docs/testing/fleet-test-01-2026-09-09.md`.
- `origin/codex/fleet-test-02` at `f51130d0`, report
  `docs/testing/bluefin-fleet-2026-09-09.md`.
- `origin/codex/fleet-test-03` at `5205c1c3`, report
  `docs/testing/alpine-musl-fleet-2026-09-09.md`.

These paths currently live on the fetched branches, not the coordinator branch. Raw evidence
remains on each machine at the locations in its report. The matrix reports their documented
results; central integration and regression remain outstanding. CachyOS and Alpine independently
found the same closed output-schema defect. Integrate one implementation, retaining both sets
of validation evidence. Other fixes cover targeted typing, Debian inspection pipes, recovery
reporting, and Linux Codex desktop environment forwarding. The Bluefin review follow-up was
verified by its agent but is not deployed or included in prior live acceptance.

CachyOS and Bluefin both verified existing `lbotinelly` authentication, repository push access,
owner commit identity and secure libsecret storage. Do not copy tokens or regenerate intake keys
there. Alpine's actual branch push is established; its authenticated login/helper is not yet
confirmed. The host is unavailable after preparing a reboot, with no post-reboot result yet.
Windows' completed short intake also returned empty output. Its agent was asked to use an
available authenticated repository-write route for sanitized status or public-key material only,
and retain its intake key if the route is unavailable. No token or private key may be published.

### Local artifact preparation and packaging finding

The coordinator prepared artifacts under `.tmp/fleet-acceptance/windows-a8cfd033/` from existing
verified 1.3.5 siblings. Their bytes match the deployed binaries and the human-browsing fix's
recorded authority hash. Runtime and packaging inputs matched the assigned source revision;
these local binaries have no compiled-revision provenance attestation. No new native Windows
installer was available locally. The portable artifact is supplemental source acceptance.

| Artifact | SHA-256 |
| --- | --- |
| `ghostlight-v1.3.5-x86_64-pc-windows-msvc.zip` | `31c6d15035dc29cf85b4eeaa2672061fafbdab23af27a01ff856147f05dcda59` |
| `ghostlight-extension-v1.1.2-a8cfd033-source.zip` (default store shape) | `206c11847a49cc009eb21e6c855e385697dc6cf2a48a5977f8cf0d74d2a1e632` |
| `ghostlight-extension-v1.1.2-development-validated.zip` (development identity retained) | `87293708c103d73a80f7666d255e2d1e8d753cfec10707a91eb0537fb6fc4a7d` |

The source ZIP is not a store delivery or an end-user installation route. Use the repository
extension or the explicitly labelled development artifact for source acceptance.

Preparation reproduced one packaging defect: `-KeepDevelopmentKey` intentionally retained the
manifest key, then failed the unconditional final no-key check with exit 1. The local fix makes
that check conditional on ordinary packaging. Both modes now exit 0; all 39 entries are checked,
the development manifest matches source, and the other 38 entries match between modes. Default
package bytes are unchanged. The submitted `dist/ghostlight-extension-v1.1.2.zip` is unchanged.
`keep-development-key-repro.json` retains the failure and marks its partial ZIP unaccepted;
`package-verification.json` records the rerun. The patch changes no browser runtime bytes and
requires no new runtime candidate round. No artifacts have been transferred or published.

### Windows credential handoff at 21:33 UTC

The authenticated GitHub connector published public intake commit `76464c06` on
`codex/fleet-leo-desktop-02`. Connector access is already the owner account, but local Git GCM
has no stored credential. The coordinator verified the destination RSA-4096 public-key
fingerprint and delivered an RSA-OAEP-SHA256 encrypted credential envelope. Only the destination
holds its private key; plaintext credentials were not written to evidence or task messages.
Secure Windows GCM installation, authenticated account and local push verification are pending.
The remote peer fix is now local commit `5afcf7b9`; it is not yet on the fetched report branch.

### Windows completion report retrieved at 21:45 UTC

`origin/codex/fleet-leo-desktop-02` at `21d4e3ed` contains the peer repair, secure credential
verification and installer correction. Reports are `docs/testing/windows-fleet-peer-2026-09-09.md`,
`docs/testing/windows-fleet-credential-intake-2026-09-09.md` and
`docs/testing/windows-fleet-packaging-2026-09-09.md` on that branch. Local Git authenticated as
`lbotinelly` through GCM Windows secure storage, verified repository push access and passed its
branch push dry-run. The encrypted handoff is complete; do not resend it.

The original mixed-binary upgrade is retained as a failure. The repaired NSIS hooks use the
existing deployment marker and Windows Restart Manager process identities (exact physical image
paths plus creation times). Upgrade stopped the installed authority/MCP while a foreign authority
and unrelated file reader survived. The final installer passed with all three expected siblings
and no marker left. All 531 Rust tests, 210 extension tests, formatting, Clippy, launcher and
workbench checks passed; the real executable process journey also passed. Codex invoked the
installed policy tool successfully. Final uninstall/refusal acceptance was rejected by automatic
approval review with only `blocked by policy`; it was not retried through another route. Browser
adapter, reboot and pristine Windows lanes remain explicitly unproved. Central review and
integration are underway on a separate `codex/fleet-integration` branch.

### Combined candidate checked centrally

The four branch histories, reports and shared repairs are now combined on
`codex/fleet-integration`. See the [integration report](fleet-integration-2026-09-09.md)
for reconciliation and evidence. Central Windows verification passes 533 Rust tests,
222 extension tests, formatting, Clippy, changed JavaScript syntax, the eight harness/reporting
checks, process and provenance journeys against fresh siblings, and the workbench surface.
A targeted Linux check of this combined candidate remains; original machine acceptance retains
its recorded version boundaries. The owner dev checkout has not received these runtime fixes.
