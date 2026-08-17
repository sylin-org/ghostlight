# Ghostlight 1.0 release checklist

Last updated: 2026-08-17.

This is the one status-bearing checklist for the 1.0 release. It routes work to the authoritative
process in [RELEASE.md](RELEASE.md), current implementation truth in [STATUS.md](STATUS.md), the
acceptance matrix in [1.0/ACCEPTANCE.md](1.0/ACCEPTANCE.md), and the reference-experience evidence in
[tasks/reference-experience/LEDGER.md](tasks/reference-experience/LEDGER.md). It does not restate
their commands or their acceptance detail. Where a gate has an owning document with its own result
table, that table holds the state and this checklist holds one box.

No checked box authorizes a tag, push, release, store action, registry action, package-manager
submission, website change, or external message. Each outward action still requires explicit owner
approval.

Gate numbering changed in the 2026-08-17 rework. G0 and G1 keep their meaning. Old G3 and G4 are now
G4 and G5, old G5 is now G8, old G6 is now G7, and old G7 and G8 are now G9 and G10. G3 and G6 are
new gates that close holes described in their own sections.

## Rules

- A gate is checked only when its linked evidence covers the exact frozen source revision and, when
  applicable, the exact checksum-bound candidate.
- Stamp a closed gate with the date, the exact source revision, and the evidence link. A checked box
  carrying none of those three is not evidence.
- Any product or packaging change after candidate assembly reopens G1 through G8 as affected.
- A development-host, unpacked-extension, source-only, or virtual-display pass does not substitute
  for a required installed-product or visible-desktop lane.
- Record content-free evidence only. Never put browser content, credentials, or machine-local notes
  in tracked files.
- Fix a failed journey at its owning seam, add a focused regression proof, rebuild the candidate,
  and rerun the affected live gate.
- A gate that cannot close honestly blocks the release. An accurate BLOCKED is worth more than an
  optimistic pass.
- Keep optional automation optional. Chrome API V2 access and Windows Authenticode are not release
  gates under the accepted 0.8 trust model.

## Current decision

Current source head at checklist creation: `eb7cf4edf271ca81bb292df3d43b313be35265ba` on `dev`.

The prior provenance-bound candidate was built from `fd8640336b11ed12cd47fe96deb7eb06adfbdcd1`,
an ancestor of the current head. That bundle is historical evidence, not the publishable 1.0
candidate. The next candidate must be built from the final approved revision after G0 closes.
Do not restate the commit distance here; `git rev-list --count fd86403..HEAD` answers it without
going stale.

Release state: **NO-GO**. Every gate, G0 through G10, is open.

## Long-lead work: start these before G0 closes

Three things on this checklist have external latency that no amount of local diligence shortens. If
they are started in gate order they serialize, and the release calendar becomes their sum instead of
their maximum. None of them is a reason to skip a gate; each is a reason to begin early.

- **Chrome Web Store review.** G3 needs a reviewed adapter and G4, G5, and G7 all refuse an unpacked
  build. Review time is Google's, not ours, and the 1.0 manifest adds the `offscreen` and `downloads`
  permissions, which is a published-surface change that can draw a deeper review than a version bump.
  Decide the pre-publication distribution route in G0 and submit as soon as G2 produces the exact ZIP.
- **The two absent environments.** G4 needs a real Ubuntu GNOME Wayland desktop and G5 needs a clean
  Windows machine with no Visual C++ Redistributable and no developer toolchain. Neither exists yet.
  Provision both while the source gate runs.
- **Candidate custody.** GitHub retention is 7 days for the native-package and extension inputs and
  14 days for the candidate bundle. The live gates will outlast both. G2 therefore takes local
  custody rather than trusting retention.

## Checklist

### G0. Close release decisions and freeze the source

This gate holds decisions only. Nothing here may depend on an assembled candidate or on
installed-product observation, or the checklist deadlocks against itself.

- [x] Resolve the literal `doctor` aggregate-readiness parity measure required by S8. The owner
  approved a live read-only query, implemented and source-verified on 2026-08-17; installed-desktop
  observation is evidence and closes under G8.
- [x] Resolve the duplicate ADR number `0127` without deleting or silently rewriting either
  historical decision. Resolved 2026-08-17: the governing `0127-one-invoked-desktop-authority.md`
  keeps its number, and the superseded switch roster became
  `0130-integration-switches-and-evidence.md`. No decision text was reopened; the renumber is
  marked in ADR-0130's header and in the ADR-0128 and ADR-0129 references to it.
- [ ] Decide how the live lanes obtain a matching 1.0 store adapter, and record it in G3. G4, G5,
  and G7 refuse an unpacked build, so either name a pre-publication route (trusted testers, or an
  unlisted item) or accept that the adapter is submitted and approved before the live lanes run.
- [x] Record a marked amendment to ADR-0102 reconciling its Decision 9 body with its acceptance item
  9. Resolved 2026-08-17: the amendment names the body as governing, explains that the check is
  per-feature and therefore cannot be evaluated against a candidate at all, and states the durable
  fringe-stability invariant a release does review its diff against. No decision was reopened.
- [x] Decide whether public first-use feedback is part of 1.0 at all. Resolved 2026-08-17: it is out
  of 1.0 entirely. It is not one of the seven ADR-0126 acceptance measures, it repeats the substance
  of [greenfield-first-success.md](testing/greenfield-first-success.md), which the tree marks
  rejected as a release gate, and it could not precede publication in any case. ADR-0126 Decision 11
  already supplies the adoption signal from store, npm, registry, and GitHub counts.
- [x] Decide what happens to `main`. Resolved 2026-08-17: `main` was merged into `dev` with the
  `ours` strategy, so its history is contained without importing 0.8-line content, and `dev` keeps
  its own Dependabot configuration. The tree was byte-identical after the merge. `main` is an
  ancestor of `dev` again, so G10 promotes it by fast-forward.
- [ ] Confirm that the non-gating debt under `STATUS.md` "Owed" remains outside 1.0 unless the
  owner explicitly promotes an item.
- [x] Confirm that every reference-experience S8 decision that does not require installed-product
  observation is closed. Resolved 2026-08-17 and recorded in the dated evaluation under "What is
  decidable without a new machine": public feedback is out, the parity decision is closed, the
  cross-platform wording comparison needs no third environment because one guard-tested closed
  table supplies the words to both install and `doctor`, the KDE accessibility half runs on the
  existing host, and the WSL sentence needs a WSL harness rather than the clean Windows machine.
  The observation-dependent rows are evidence and close under G8.
- [ ] Name one final source revision and stop product and packaging changes while its candidate is
  evaluated.

Evidence:

- [Reference-experience evaluation](testing/reference-experience-evaluation-2026-08-16.md)
- [Reference-experience ledger](tasks/reference-experience/LEDGER.md)
- [Doctor readiness parity evidence](testing/doctor-readiness-parity-2026-08-17.md)
- [ADR index](adr/README.md)

### G1. Pass the frozen source gate

Commands live in [RELEASE.md](RELEASE.md) under Source. Applicable operating systems are Windows and
Linux.

- [ ] Formatting passes.
- [ ] Workspace/all-target Clippy passes with warnings denied.
- [ ] Full Rust workspace tests pass.
- [ ] Extension, npm launcher, and MCPB launcher tests pass.
- [ ] Changed JavaScript and all release-owned script syntax checks pass.
- [ ] The fringe-stability review passes: the connectors, the shared bridge, and the extension
  changed only where an already-real process boundary required it, and the extension remains
  policy-free.
- [ ] Fresh isolated workspace build and process, CLI, PowerShell, policy, and workbench journeys
  pass on both Windows and Linux.
- [ ] Dependency license, ban, source, and advisory gates pass. The 17 accepted GTK/Tauri-chain
  warnings are rechecked against the frozen graph rather than assumed.
- [ ] Release truth, repository integrity, documentation links, ASCII policy, and the complete 0.8
  recovery disposition pass.

Evidence: add a dated record under `docs/testing/` and link it here.

Pre-freeze evidence: [2026-08-17 local release preflight](testing/release-preflight-2026-08-17.md)
passed every locally runnable source check. G1 remains open until G0 names the frozen revision and
both operating-system passes cover that exact revision.

### G2. Assemble, verify, and take custody of the candidate

- [ ] Inspect release access before spending a candidate build, so a dead GitHub or npm credential
  is found before the run rather than after it. The check is read-only and reports optional Chrome
  and MCP Registry credentials without making them blockers.
- [ ] Ordinary CI passes all Windows and Linux jobs for the frozen revision.
- [ ] The manual build-only workflow builds the Windows NSIS package on Windows 2025 and the Debian
  package on Ubuntu 22.04.
- [ ] Debian 12 and Ubuntu 24.04 package lifecycle smokes pass before assembly.
- [ ] The candidate contains exactly 17 artifacts: six raw binaries, two native packages, two
  portable archives, the extension ZIP, npm tarball, MCPB, and four SBOMs.
- [ ] `release-candidate.json` and `SHA256SUMS` independently bind exact names, lengths, hashes,
  version, and source revision.
- [ ] GitHub provenance verifies for all 17 assets, the manifest, and the checksum file against the
  exact repository, workflow, source revision, and source ref.
- [ ] Take local custody of the complete candidate and of the native-package and extension inputs,
  then re-verify every hash from the local copy. Do this within 7 days of the run: retention is 7
  days on `native-*` and `chrome-extension` and 14 days on the candidate bundle, and the live gates
  outlast both. Record the local location in machine-local notes, never here.

Evidence: replace the stale candidate reference with a new dated candidate record.

### G3. Place the matching adapter where the live lanes can install it

G4, G5, and G7 all require the matching store adapter rather than an unpacked build. That adapter has
to exist before those lanes run, which is why this gate sits here instead of inside publication.
Nothing in this gate makes the adapter publicly visible.

- [ ] Upload the exact candidate extension ZIP, and confirm the uploaded bytes match the candidate
  hash. The store build is deterministic, so a mismatch means the wrong artifact.
- [ ] Submit for review with staged publication, so review approval does not silently make the
  adapter public.
- [ ] Reach the distribution state chosen in G0, and record which route was used.
- [ ] Install the reviewed adapter from the store on a live-lane machine and confirm its extension id
  and version match the candidate.

Evidence: add a dated adapter record under `docs/testing/`, content-free, with no credential values.

### G4. Pass the Ubuntu GNOME Wayland installed-product lane

Environment: a real Ubuntu GNOME Wayland session on a physical or attached display, with no
Ghostlight development checkout. A virtual display does not satisfy this lane.

[linux-live-lifecycle.md](testing/linux-live-lifecycle.md) defines L1 through L9 and owns their
per-lane result table. This checklist does not repeat those rows.

- [ ] Install from the exact Debian candidate taken into custody in G2, verifying the digest first.
- [ ] Use the matching store adapter from G3.
- [ ] All nine lanes pass in the lifecycle table, covering clean install, visible browser work,
  authority restart and unknown-effect non-replay, browser and extension recovery, logout/login and
  reboot demand-start, concurrent harnesses, 0.8 package upgrade, recovery and diagnostics, and
  ownership-safe uninstall.
- [ ] Verify native window, tray where GNOME provides it, Applications activation, notifications,
  and no resident supervisor.
- [ ] Run the GNOME half of the G8 accessibility matrix here, and record the rendered environment
  sentence for the G8 desk check.

Evidence: [Linux live lifecycle](testing/linux-live-lifecycle.md), completed for the current
candidate.

### G5. Pass the clean installed-Windows lane

Environment: a Windows machine or virtual machine with no Visual C++ Redistributable, no prior
Ghostlight state, and no developer toolchain. The development host does not satisfy this lane.

- [ ] Verify the candidate digest and install the exact NSIS package as an ordinary user.
- [ ] Start every packaged executable on a clean machine without a separately installed Visual C++
  Redistributable.
- [ ] Verify minimized first start, exact workbench activation, native Close/recreation, tray and
  notification-area behavior, notifications, and explicit Quit.
- [ ] Verify on-demand browser launch uses the ordinary profile, no automation flags, one bounded
  single flight, and exact ambiguity or refusal outcomes.
- [ ] Run the visible browser and governance journeys with the matching store adapter from G3.
- [ ] Verify connector demand-start and deploy quiesce.
- [ ] Run logout/login and reboot demand-start without a Run key, scheduled task, or resident
  service.
- [ ] Upgrade from public 0.8 without clobbering user or foreign state.
- [ ] Uninstall twice and prove only Ghostlight-owned state changed.
- [ ] Record the rendered environment sentence for the G8 desk check.

Evidence: add a dated installed-Windows candidate record under `docs/testing/`.

### G6. Pass the npm launcher channel lane

`npx -y ghostlight install` is the primary user journey, and it is the one delivery channel whose
upgrade path no other gate covers. G4 and G5 prove native package upgrade; this gate proves the
channel most people will actually arrive through. Run it on both Windows and Linux against the
candidate tarball taken into custody in G2, before that tarball is published.

- [ ] A clean consumer installs from the candidate tarball, verifies all six raw-binary checksums,
  and refuses to execute incomplete or unverified bytes.
- [ ] A bare launch is an MCP stdio edge, and subcommands reach the native orchestrator with ordered
  output and preserved exit status.
- [ ] An existing public 0.8 npm installation upgrades in place without clobbering user state,
  harness configuration, browser registrations, or older version directories.
- [ ] Repeat install changes zero configuration bytes, and dry-run, `--no-open`, and CI-suppressed
  paths stay non-interactive.
- [ ] Uninstall removes only owned entries and preserves malformed or foreign configuration
  byte-for-byte.

Evidence: add a dated npm channel record under `docs/testing/`.

### G7. Pass the candidate browser and public-harness matrix

Run the accepted matrix in [1.0/ACCEPTANCE.md](1.0/ACCEPTANCE.md) as directed by
[RELEASE.md](RELEASE.md) under Browser and MCP journeys. Only the release-specific additions are
listed here.

- [ ] The matrix passes with a visible ordinary browser profile, the matching store adapter from G3,
  and two supported Chromium families where available.
- [ ] Exercise at least three public MCP harnesses against the packaged connector. Include a
  portability-sensitive client such as Kiro/Bedrock.
- [ ] Verify the exact 22-tool catalog and compatible MCP revision negotiation.
- [ ] Verify popup and options disconnected, incompatible, setup, connected, and hold states plus
  extension identity across reload.
- [ ] Add a focused regression proof for every defect found and rerun its visible journey.

Evidence: add a dated candidate-bound browser and harness record under `docs/testing/`.

### G8. Close reference-experience evaluation

This gate consumes the installed-product evidence produced by G4, G5, and G7, which is why it sits
after them rather than inside the freeze.

- [ ] Disposition the extension-arrived-by-sync state. Live KDE Chromium already proved both
  surfaces and the online and offline recovery routes against the production extension id, so what
  remains is deciding whether that development-host evidence stands or must repeat against the
  store build.
- [ ] Run the KDE Wayland half of the accessibility matrix on the existing host: keyboard-only,
  screen-reader-name, large-text, high-contrast, reduced-motion, browser-zoom, and
  fractional-scaling. This needs no new machine and must not queue behind G4.
- [ ] Run the GNOME Wayland half of the same matrix inside G4.
- [ ] Observe the WSL sentence with the browser on Windows. This needs a WSL harness, not the clean
  installed-Windows machine, so it does not wait for G5.
- [ ] Compare the environment sentences recorded by G4, G5, and the WSL run. Those are the same
  person's machines, and `language/environment.rs` is one guard-tested closed table feeding both
  install and `doctor`, so this is a desk check rather than a separate run.
- [ ] Mark each of the seven ADR-0126 acceptance measures, and every reference-experience deviation,
  met, accepted by the owner, or not met with a named follow-up.
- [ ] Mark S8 and the reference-experience epic complete only after all mandatory rows close.

Evidence: update the dated evaluation and reference-experience ledger.

### G9. Reconcile release-facing truth

- [ ] Correct `STATUS.md` against the tree before any public wording is drawn from it. Two known
  drifts: it describes three workbench destinations while the live tab row is At a glance,
  Integrations, Status, Policy, and About; and it claims `main` is an ancestor of `dev`, which is no
  longer true.
- [ ] Finalize the `1.0.0` changelog entry from `Unreleased`.
- [ ] Finalize release notes, supported-platform wording, compatibility rows, install instructions,
  trust claims, and rollback language from observed candidate evidence.
- [ ] Verify original icon bytes and visual identity in the desktop binary, native bundles,
  workbench, extension, launcher, and MCPB.
- [ ] Verify exact Apache, MIT, commercial-module, and plain-language licensing files in every
  applicable package.
- [ ] Run publication adapters in non-mutating plan mode and record prerequisites per channel.
- [ ] Prepare updates to public status, README, website, store, package-manager, and registry
  metadata, but do not claim 1.0 before independently observable artifacts exist.
- [ ] Obtain explicit owner approval for each outward channel action.

Evidence: release-note draft, plan output, and an owner-approved channel sequence.

### G10. Publish and verify one recoverable channel at a time

The adapter was already submitted under G3. This gate makes it public and publishes everything else.

- [ ] Create the remote `v1.0.0` tag at the candidate revision only with owner approval.
- [ ] Create the GitHub draft, re-download every asset, compare names and hashes, then publish the
  immutable release with owner approval.
- [ ] Publish the exact candidate-bound `ghostlight@1.0.0` tarball only after all six raw GitHub
  assets are observable and provenance verifies.
- [ ] Publish the reviewed Chrome adapter from its staged state.
- [ ] Publish candidate-derived Scoop and WinGet metadata only after their referenced assets are
  observable.
- [ ] Publish MCP Registry metadata only after the exact npm coordinate is observable.
- [ ] Execute the `main` outcome decided in G0.
- [ ] Reconcile the Chrome feed and every public surface from independently downloaded artifacts.
- [ ] Update `docs/public-status.json`, README, trust review stamps, website copy, distribution
  records, and changelog to observed public state.
- [ ] Run one public install-to-first-task smoke on Windows and Linux.
- [ ] Record final hashes, links, compatibility, known limitations, and recovery guidance in
  `STATUS.md`.

Evidence: independently observed public URLs and hashes. Publication failure never rewrites or
reuses a released version; recovery uses a higher version.

## Decision record

Fill this in once, when the release is called. A GO here is the owner's, not an agent's.

| Field | Value |
| --- | --- |
| Decision | NO-GO |
| Date | 2026-08-17 |
| Frozen revision | not yet named |
| Candidate | not yet built |
| Open gates | G0 through G10 |

## After publication, not gates

These belong to the released product, not to the release decision. Nothing here may be promoted into
a blocking gate without an owner decision recorded in G0.

- Adoption signal from the Chrome Web Store, npm, the MCP Registry, and GitHub, which already count
  installs and downloads without the product reporting anything (ADR-0126 Decision 11). G0 decided
  on 2026-08-17 that public first-use feedback is not part of 1.0 in any form, blocking or
  otherwise, so it is deliberately absent from this list.
- RPM, AppImage, Snap, Flatpak, AUR, and Nix artifacts, none of which became 1.0 gates merely because
  their packaging tools exist.

## Historical foundation already proved

These reduce uncertainty but do not close a current-revision gate, so they are deliberately not
checkboxes:

- The build-only workflow has produced a complete 17-artifact candidate.
- Deterministic extension construction has passed.
- Debian 12 and Ubuntu 24.04 noninteractive lifecycle smokes have passed.
- Candidate checksums and GitHub build provenance have verified end to end.
- Windows development-host NSIS lifecycle and Linux development-host rehearsals have passed.
- The 0.8 recovery inventory and disposition machinery exist and are CI-checked.
- Publication scripts default to non-mutating plans and require explicit execution switches.

The exact historical hashes and limitations remain in
[release-candidate-2026-08-16.md](testing/release-candidate-2026-08-16.md).
