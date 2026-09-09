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

| Lane | Verified environment or milestone | Acceptance state |
| --- | --- | --- |
| leo-desktop-02 | Windows 11 Pro 25H2, build 26200.9445, x64; no Ghostlight standard installation, running siblings, or native registrations at baseline. Native Chrome 153 and Edge 152; Codex and VS Code installed. Existing WebView2, VC runtimes, and developer tools. Published 1.3.4 installer checksum verified; Chrome opened through Explorer. | Consumer attempt in progress. First Ghostlight install, not a pristine Windows image. |
| test-01 | Owner confirmed full app permissions after an approval pause; a new turn is active without an approval flag. Machine results not yet retrievable. | Execution active; no fleet acceptance result. |
| test-02 | Execution assigned; machine results not yet retrievable. | Active task; no fleet acceptance result. |
| test-03 | Execution assigned; machine results not yet retrievable. | Active task; no fleet acceptance result. |

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
