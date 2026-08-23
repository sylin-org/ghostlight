# Ghostlight 0.8 harvest

Status: active input to the 1.0 release

This document preserves what Ghostlight learned while building and publishing 0.8. It is not a
request to restore the old internals. The 1.0 source and contracts remain implementation authority.
The tests, platform evidence, delivery mechanisms, public identity, and release lessons remain
project knowledge and must be translated onto the current architecture.

## Source snapshots

| Revision | Role in the harvest |
| --- | --- |
| `993135b048b60622157266b53b21f1719c9df4b3` | The exact public v0.8.0 release commit. Use it for claims about shipped bytes. |
| `95468758ab56b38da8b5ea5b717d51642c8cd56d` | The reconciled public source state after checksum and package-manager updates. |
| `c01cc3276102471f3e18de2ae90cb90abf98ed88` | The last mature pre-rebuild implementation. It includes post-release fixes and is evidence, not a claim about v0.8.0. |
| `f5d43768d56e5acf258c80bfef01ea341bba77d7` | The archived pre-1.0 worktree. Use it to recover work that had not reached the mature commit. |

Always say which snapshot supports a fact. Do not attribute a post-release fix from `c01cc327` to
the v0.8.0 artifacts unless the released commit contains it.

## What the rebuild removed

Commit `bf4f4724a287c54206f6968bd69ec23b7211682a` rebuilt the implementation and deleted 289 files.
The commit removed 116,475 lines while adding 16,481. The clean-room implementation boundary was
valid; treating every deleted non-document file as disposable was not.

Deleted material included:

- four GitHub workflow and dependency-management files;
- `compatibility.json`, `deny.toml`, and the pinned toolchain policy;
- fourteen build, install, package, release, publication, and store-reconciliation scripts;
- package definitions and launcher tests across npm, MCPB, Scoop, and WinGet;
- the native-host and supervisor installer implementation;
- the Lightbox process-contract runner;
- eighty-three files under `tests/`; and
- the old source crates, which remain history rather than 1.0 implementation authority.

The present tree retained the documentation corpus, public identity, release packet, ADRs, and Git
history. It did not retain a working release pipeline or a durable index of the removed executable
evidence.

## Test evidence

### Behavioral capability restoration

The recovery inventory gives every historical test and artifact a disposition, but that does not
by itself prove that every published user capability remains reachable. A later direct comparison
of the exact published 0.8 catalog with the current 1.0 language found genuine contractions beyond
the already corrected region screenshot path.

[ADR-0133](../adr/0133-behavioral-capability-restoration.md) defines their 1.0 expression, and the
[capability-restoration ledger](../tasks/capability-restoration/LEDGER.md) records implementation
and evidence. Until a ledger row is complete, its planned mapping is not an implemented parity
claim. The old names and source remain historical evidence only.

### Region screenshot parity correction

The published `v0.8.0` extension supported a bounded `computer` zoom action that cropped a region,
magnified it, retained the new coordinate transform, and allowed another region to be selected
from the result. The initial 1.0 harvest inventory retained its tests but failed to turn that user
capability into an explicit parity disposition. ADR-0131 corrects the gap on current seams through
the `browser_screenshot` view branch. The old action name, tuple signature, and implementation do
not return.

[`test-inventory.json`](test-inventory.json) is generated from `c01cc327` by
`scripts/harvest-0.8-test-inventory.ps1`. It records every Rust test attribute, every JavaScript
`test` or `it` declaration, and every named Lightbox scenario in the source registries.

The source contains 1,354 ordinary test declarations:

| Behavior area | Declarations |
| --- | ---: |
| Governance | 344 |
| Browser hub and coordination | 176 |
| Extension | 177 |
| Integration | 149 |
| Tool execution | 137 |
| MCP edge | 101 |
| Transport | 94 |
| Installation | 62 |
| Supporting units | 105 |
| Lightbox helper units | 5 |
| Real-browser E2E mechanics | 4 |

The Lightbox registries enumerate 34 process scenarios in addition to those declarations: ten
managed-governance scenarios, twenty-one migrated legacy process scenarios, and three mechanism
wire-skew scenarios.

The historical Lightbox ledger says that two consecutive 37-scenario Windows runs and one clean
37-scenario Linux run passed. The actual source registries at `c01cc327` enumerate 34. No evidence
found during this harvest explains the difference. Preserve both facts and use 34 as the
source-derived inventory until an archived runner output establishes the other three.

The current 1.0 tree has 184 Rust tests and 99 extension tests. Raw counts do not measure equivalent
coverage because 1.0 deliberately removed many old mechanisms. They do show why every old behavior
needs a disposition instead of an unreviewed deletion.

[`RECOVERY-MATRIX.md`](RECOVERY-MATRIX.md) and its checked
[`test-recovery.json`](test-recovery.json) now cover all 1,388 entries in twelve reviewed behavior
groups and give every one of the 34 Lightbox process scenarios an explicit disposition. The matrix
is a map, not a false claim of one-for-one private implementation equivalence.

[`ARTIFACT-RECOVERY.md`](ARTIFACT-RECOVERY.md) adds the file-level half of the harvest. Its generated
scoped inventory content-addresses 809 paths from the mature 0.8 tree, and its checked recovery
ledger gives each artifact an explicit treatment. A later rewrite may change a treatment, but it
cannot make a file disappear from project memory without the drift gate noticing.

### Translation rule

For each inventory group, choose one of four outcomes:

1. Re-express the behavior through the current owner and public contract.
2. Point to an existing 1.0 test that proves the same invariant.
3. Mark the capability deferred by the 1.0 contract.
4. Mark the old mechanism superseded while retaining the evidence entry.

Do not copy an old implementation test that reaches private types which no longer exist. Preserve
the assertion and put the new test at the current seam.

## Linux knowledge

Linux was live-tested before 1.0. It was not a blank platform.

The 2026-07-14 run used Ubuntu Desktop 26.04, Chrome Stable 150, VS Code 1.128.1, and Cline 4.0.8.
It found that graphical and MCP launchers can scrub `XDG_RUNTIME_DIR` and
`DBUS_SESSION_BUS_ADDRESS`. The service listened under `/run/user/1000` while a relay fell back to
another socket location. Each component could look healthy while the complete chain was broken.

ADR-0082 captured the durable behavior:

- prefer the normal XDG runtime directory;
- on Linux, accept `/run/user/<effective-uid>` only when it is a real directory, owned by that
  user, with no group or other permission bits;
- reject symlinks, foreign ownership, and permissive modes;
- fill only missing systemd user-bus environment values; and
- use one transport-owned resolver for MCP and browser relays.

Other recovered Linux details:

- user native-messaging directories end in case-sensitive `NativeMessagingHosts`;
- Chrome, Edge, Brave, and Chromium use different config roots;
- Zed uses lowercase `~/.config/zed`;
- the real Chromium/native-messaging CI gate was blocking and normally finished in about one
  minute after its stale binary-name and wrong-tool bugs were fixed;
- a clean Rust 1.95 Linux environment ran the Lightbox process suite; and
- a full packaged L1-L9 lifecycle was still not completed.

The 1.0 Linux release therefore needs a new package and visible lifecycle run, but it should reuse
these failure models, paths, and checks.

## Installation and upgrade knowledge

The 0.8 installer carried useful cross-platform facts:

- native host name `org.sylin.ghostlight`;
- public extension id `lejccfmoeogmhemakeknjjdhkfkgncdl`;
- development extension id `cjcmhepmagomefjggkcohdbfemacojoa`;
- exact per-browser Windows registry keys and Linux manifest directories;
- atomic manifest writes and ownership-checked removal;
- unrelated MCP configuration must survive install and uninstall;
- an upgrade is incomplete while an older managed executable still owns the service endpoint; and
- an unverified or external endpoint owner must never be killed.

The old supervisor architecture does not return. The current shared demand-start seam owns runtime
availability. Migration code may identify and remove only exact legacy registrations:

- Windows HKCU Run value `Ghostlight Service` and the old Task Scheduler entry of the same name;
- Linux `~/.config/systemd/user/ghostlight.service`.

The common 0.8 npm install cache was `~/.ghostlight/bin/v0.8.0`. A harness entry that points there
is updatable, not currently installed. The current installer must distinguish current, updatable,
foreign, unavailable, and malformed states.

## CI and release knowledge

The high-value 0.8 CI shape was:

- least-privilege read access;
- pinned Rust and Node inputs;
- format, strict clippy, build, and all tests on Windows and Linux;
- extension tests on Windows and Linux;
- a blocking real Linux Chromium/native-messaging journey with a hard timeout;
- dependency advisory and license/source checks;
- exact release artifacts built in a target matrix;
- one publisher job with no source checkout;
- a checked, sorted artifact manifest;
- checksums, SBOM, and GitHub attestations; and
- downstream packages published only after immutable release assets existed.

The old `scripts/release.ps1` was 901 lines and combined preparation, mutation, publication,
recovery, and channel coordination. Do not restore it. Preserve its valuable properties as small
independent commands and one declarative release workflow:

- dry-run before mutation;
- exact version and artifact checks;
- idempotent reruns;
- registry immutability;
- release assets before downstream metadata; and
- browser-store publication only when explicitly requested.

## Publication knowledge

The publication process produced several durable practices:

- one claim-to-evidence matrix;
- a clear fifteen-second recognition, two-minute fit, five-minute first-success, and symptom-led
  recovery path;
- explicit fit and anti-fit language;
- separate candidate, submitted, public, and observed states;
- no fabricated telemetry or reception claims;
- distribution activity is not user reception; and
- every immutable channel has its own forward-recovery procedure.

The current live observation is in
[`PUBLICATION-STATE-2026-08-13.md`](PUBLICATION-STATE-2026-08-13.md). The original packet remains an
immutable account of what was known on 2026-08-07.

## Disposition

| Artifact or lesson | 1.0 treatment |
| --- | --- |
| Product name, icons, visual language, store identity, public character | Preserve exactly unless the owner changes it. |
| Test behaviors and process scenarios | Translate onto current seams and keep the inventory. |
| Linux failure models and platform paths | Reuse and retest in package and live journeys. |
| Native-host registration and ownership-safe removal | Re-express under `crates/orchestrator/src/install/`. |
| CI, checksums, SBOM, attestations, immutable publisher | Adapt to the four current crates and Tauri packages. |
| Compatibility and public-state reconciliation | Restore as small checked data and scripts. |
| Publication copy and channel recovery | Adapt for 1.0 after public artifacts exist. |
| Old core, hub, transport, and extension implementation | Git history only. Do not copy into 1.0 internals. |
| Old always-ready supervisor and named-instance model | Superseded; migration cleanup only. |
| Raw-binary npm, MCPB, Scoop, and WinGet entry points | Restored on current seams. All are generated from the same checked 1.0 candidate and exact raw or portable hashes. |
| Giant release conductor and mechanical trust restamping | Retire. |
| Directory submission sweeps and reception monitoring | Independent, optional post-release work. Never a candidate gate. |

## Release-process guardrail

A 1.0 candidate needs the smallest process that proves the product:

1. One green source and browser CI matrix.
2. One immutable artifact build and verification workflow.
3. One clean package lifecycle per platform.
4. One visible first-success journey with the matching store adapter.
5. Owner-approved publication, one channel at a time.

Every step must either prevent a real failure, prove a user promise, or make recovery safer. A step
that only creates a document, restamps a date, repeats another check, or coordinates an optional
directory is not a release gate.
