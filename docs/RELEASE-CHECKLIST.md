# Ghostlight 1.0 release checklist

Last updated: 2026-08-22.

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

Release state: **G2 CLOSED at `b2c27993`** (2026-08-26); G3's distribution state is reached
(the store review of the exact candidate bytes is approved and staged). G4 through G8 remain
open, and G10 publication is proceeding at the owner's explicit 2026-08-26 direction: binaries
out and published, then the store adapter flips to 1.0.

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
- [x] Decide how the live lanes obtain a matching 1.0 store adapter, and record it in G3. G4, G5,
  and G7 refuse an unpacked build, so either name a pre-publication route (trusted testers, or an
  unlisted item) or accept that the adapter is submitted and approved before the live lanes run.
  Resolved 2026-08-24: submit-and-wait staged. The live lanes wait for the already-pending
  STAGED_PUBLISH review of the exact candidate bytes to clear, rather than opening a trusted-tester
  or unlisted route; no store mutation beyond that review is authorized. Recorded against G3's
  distribution-state row. Evidence:
  [extension-store-submission-2026-08-24](testing/extension-store-submission-2026-08-24.md).
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
- [x] Confirm that the non-gating debt under `STATUS.md` "Owed" remains outside 1.0 unless the
  owner explicitly promotes an item. Confirmed 2026-08-24 with one promotion: the owner moved four
  non-extension items into a pre-freeze window while the store review pends (mcp-connector
  ServiceClient adoption, unsettled-row color treatment, model-facing policy explain per a new ADR,
  ADR-0105 stages 2 and 3 through one audited FFI crate). The extension-touching debt stays owed;
  everything else listed under "Owed" stays outside 1.0. Evidence:
  [pre-freeze-debt ledger](tasks/pre-freeze-debt/LEDGER.md).
- [x] Confirm that every reference-experience S8 decision that does not require installed-product
  observation is closed. Resolved 2026-08-17 and recorded in the dated evaluation under "What is
  decidable without a new machine": public feedback is out, the parity decision is closed, the
  cross-platform wording comparison needs no third environment because one guard-tested closed
  table supplies the words to both install and `doctor`, the KDE accessibility half runs on the
  existing host, and the WSL sentence needs a WSL harness rather than the clean Windows machine.
  The observation-dependent rows are evidence and close under G8.
- [x] Name one final source revision and stop product and packaging changes while its candidate is
  evaluated. First frozen 2026-08-24 (UTC) at `08f368606f3deac4115a148f6c20590a7c9afb9b`;
  re-declared the same session at `e7d8986bb96625335cd9cff7d04d7e8b083f845d` after ordinary CI
  exposed a Linux-only Clippy failure in the new `ghostlight-win-peer` test module -- fixed at
  its owning seam with a cross-platform negative-control pin, then re-frozen per the gate rule
  that a journey which cannot close honestly is repaired, not waived. Machine-readable
  declaration: [release/freeze.json](release/freeze.json). From this moment, no product or
  packaging changes: anything discovered goes to the batch ledgers as a documented limitation
  unless the owner declares it a blocker. Unfreezing means re-declaring and restarting the
  affected gates. `extension/` remains byte-identical to its state at `70869631`, so the pending
  STAGED_PUBLISH store review continues to cover the candidate's extension bytes without a
  resubmission.

Evidence:

- [Reference-experience evaluation](testing/reference-experience-evaluation-2026-08-16.md)
- [Reference-experience ledger](tasks/reference-experience/LEDGER.md)
- [Doctor readiness parity evidence](testing/doctor-readiness-parity-2026-08-17.md)
- [ADR index](adr/README.md)

### G1. Pass the frozen source gate

Commands live in [RELEASE.md](RELEASE.md) under Source. Applicable operating systems are Windows and
Linux.

- [x] Formatting passes.
- [x] Workspace/all-target Clippy passes with warnings denied.
- [x] Full Rust workspace tests pass.
- [x] Extension, npm launcher, and MCPB launcher tests pass.
- [x] Changed JavaScript and all release-owned script syntax checks pass. (Windows JS checks pass;
  Linux `sh -n` passed against the frozen revision on 2026-08-25.)
- [x] The fringe-stability review passes: the connectors, the shared bridge, and the extension
  changed only where an already-real process boundary required it, and the extension remains
  policy-free. (Reviewed on the frozen revision: the batch touches exactly
  `bridge/src/client.rs` and `mcp-connector/src/service_session.rs` at the service handshake
  boundary; browser-connector and extension diffs are empty.)
- [x] Fresh isolated workspace build and process, CLI, PowerShell, policy, and workbench journeys
  pass on both Windows and Linux. (Linux passed against exact isolated binaries on 2026-08-25. Its
  custom-target run also found that `release-preflight.ps1` restores `GHOSTLIGHT_BIN_DIR` before
  queued journeys execute; the dated evidence distinguishes the green product result from that
  frozen runner defect.)
- [x] The whole-catalog foundry demo (`scripts/demo-foundry.ps1` or `demo-foundry.sh`) runs green
  end to end against the deployed release graph, including the desk-stage dialog beats. Rerun it
  whenever an input-path, extension, or browser-relay batch lands: automated suites have missed
  page-state interactions that this rehearsal catches (foundry press_key and desk-bell defects,
  2026-08-24). (Closed after reconciliation: Linux passed all 41 beats at the frozen revision and
  flagged that both runners omitted the new 24th tool; the owner-dispositioned repair added an
  `explain policy` beat to both runners, and this host reran the deployed frozen graph green at
  42 beats on 2026-08-25 -- see
  [release-tooling repairs](testing/release-tooling-repairs-2026-08-25.md). The Linux 42-beat
  confirmation is linux-codex's one optional follow-up.)
- [x] Dependency license, ban, source, and advisory gates pass. The 17 accepted GTK/Tauri-chain
  warnings are rechecked against the frozen graph rather than assumed. (Proven locally
  2026-08-25 UTC: `cargo deny check bans licenses sources` ok; `cargo audit` exits zero with
  exactly the 17 documented allowances.)
- [x] Release truth, repository integrity, documentation links, ASCII policy, and the complete 0.8
  recovery disposition pass. (Proven locally on the frozen revision, 2026-08-25 UTC: 852 files
  readable, links valid, version aligned, ASCII exceptions fixed at 25, all 1,388 recovery entries
  and 34 Lightbox scenarios dispositioned.)

Evidence: [2026-08-24 frozen-revision preflight, Windows half](testing/release-preflight-2026-08-24.md),
[2026-08-25 frozen-source CachyOS verification](testing/frozen-source-cachyos-verification-2026-08-25.md),
[2026-08-25 release-tooling repairs](testing/release-tooling-repairs-2026-08-25.md), the
[2026-09-04 1.3.3/1.1.1 preflight](testing/release-preflight-2026-09-04.md), and the
[2026-09-05 1.3.4/1.1.1 preflight](testing/release-preflight-2026-09-05.md).

G1 is closed on both operating systems at frozen revision `e7d8986b`. The three runner defects the
Linux lane surfaced were repaired as release tooling (outside the freeze's product paths) and
proven on this host; no product or extension byte changed after the freeze.

Extension-specific pre-freeze evidence:
[2026-08-22 extension release preparation](testing/extension-release-preparation-2026-08-22.md)
compared the exact published 0.8 ZIP with 1.0, fixed the real package and disclosure gaps, and
produced a byte-reproducible local ZIP. Its later store update records the current public policy,
the manually saved dashboard disclosures, and a successful `1.0.0` staged submission with state
`PENDING_REVIEW`. ADR-0131 changed the package afterward, and the stale review was replaced on
2026-08-24 by a resubmission built from the foundry-sprint source (see
[extension-store-submission-2026-08-24.md](testing/extension-store-submission-2026-08-24.md)).
That submission is current unless the extension source changes again before G0 freeze, in which
case it must be replaced the same way. It does not close G1, G2, or G3 for an unfrozen revision.

### G2. Assemble, verify, and take custody of the candidate

- [x] Inspect release access before spending a candidate build, so a dead GitHub or npm credential
  is found before the run rather than after it. The check is read-only and reports optional Chrome
  and MCP Registry credentials without making them blockers. (Ran online 2026-08-25 UTC against
  `~/.ghostlight-release.env`: GITHUB_AUTH valid, NPM_AUTH valid, CHROME_WEB_STORE v2-item-valid,
  all optional values present as states only.)
- [x] Ordinary CI passes all Windows and Linux jobs for the frozen revision. (Green on dev through 2026-08-25, including the release run's quality gate.)
- [x] The manual build-only workflow builds the Windows NSIS package on Windows 2025 and the Debian
  package on Ubuntu 22.04.
- [x] Debian 12 and Ubuntu 24.04 package lifecycle smokes pass before assembly. (Ubuntu 24.04 initially failed on the image's dpkg manpage excludes; repaired in the smoke script -- see the custody record.)
- [x] The candidate contains exactly 18 artifacts: six raw binaries, two native packages, two
  portable archives, the extension ZIP, npm tarball, MCPB, and five SBOMs (one per workspace
  crate, including the audited `ghostlight-win-peer` FFI crate).
- [x] `release-candidate.json` and `SHA256SUMS` independently bind exact names, lengths, hashes,
  version, and source revision.
- [x] GitHub provenance verifies for all 18 assets, the manifest, and the checksum file against the
  exact repository, workflow, source revision, and source ref.
- [x] Take local custody of the candidate and re-verify it from the local copy, because the live
  gates outlast GitHub's retention: 7 days on the `native-*` and `chrome-extension` inputs, 14 days
  on the assembled bundle. Losing them means rebuilding, and a rebuild is a new revision, which
  reopens G1 through G8. Do this on the day the workflow finishes:

  ```sh
  gh run download <candidate-run-id> --dir <local-durable-path>
  pwsh -File scripts/check-release-candidate.ps1 -CandidateDirectory <local-durable-path>/release-candidate
  ```

  The verifier already checks manifest validity, the exact 18-artifact count, every name, length,
  and hash, and that `SHA256SUMS` is the exact sorted manifest. Record the local path in
  machine-local notes, never here.

Evidence: [2026-08-25 candidate custody](testing/candidate-custody-2026-08-25.md) -- build run 32846030216 at revision 994b6c85, superseded; [2026-08-26 candidate custody](testing/candidate-custody-2026-08-26.md) -- build run 33020313866 at revision `b2c27993a223c220f8828736b125676ae6f9d027`, two verified local copies, provenance green, and the candidate extension ZIP byte-identical to the approved store revision `3570494f`.; [2026-08-31 candidate custody](testing/candidate-custody-2026-08-31.md) -- build run 33355735166 at revision `0d7b7759`, published; [2026-09-02 candidate custody](testing/candidate-custody-2026-09-02.md) -- build run 33643387463 at revision `45639541`, two verified local copies, provenance green, published 2026-09-02; [2026-09-04 candidate custody](testing/candidate-custody-2026-09-04.md) -- build run 33912620937 at revision `fe5b9de8`, two verified byte-identical local copies, provenance green, held pending adapter review

Current patch evidence: [2026-09-05 1.3.4 custody and publication](testing/candidate-custody-2026-09-05.md)
records run `33991341425` at frozen source `768ee7383da1988a2d6b0217812e23d3fe580680`, two verified
copies, all 20 provenance-bound release files, and observed publication of service 1.3.4 and
Chrome adapter 1.1.1.

### G3. Place the matching adapter where the live lanes can install it

G4, G5, and G7 all require the matching store adapter rather than an unpacked build. That adapter has
to exist before those lanes run, which is why this gate sits here instead of inside publication.
Nothing in this gate makes the adapter publicly visible.

- [x] Upload the exact candidate extension ZIP, and confirm the uploaded bytes match the candidate
  hash. The store build is deterministic, so a mismatch means the wrong artifact.
- [x] Submit for review with staged publication, so review approval does not silently make the
  adapter public.
- [x] Take G2 custody before opening this gate: a staged review goes stale the moment the package
  changes, so submission is ordered strictly after the frozen candidate's bytes are assembled,
  verified, and held locally.
- [x] Reach the distribution state chosen in G0, and record which route was used. (The staged
  review of the exact candidate bytes cleared on 2026-08-26 -- dashboard "Ready to publish", API
  `STAGED` -- and the owner-directed G10 publication then published that approved staged revision
  publicly the same day; the live lanes can now install the public adapter directly. Evidence:
  [extension-store-resubmission-2026-08-25-frames-shadow](testing/extension-store-resubmission-2026-08-25-frames-shadow.md),
  [candidate-custody-2026-08-26](testing/candidate-custody-2026-08-26.md).)
- [ ] Install the reviewed adapter from the store on a live-lane machine and confirm its extension id
  and version match the candidate.

Evidence: [2026-08-25 store resubmission](testing/extension-store-resubmission-2026-08-25.md) --
custody ZIP `9ae88e67...` uploaded over the canceled stale review (`f7b9a6ad...`), submitted
STAGED_PUBLISH, now PENDING_REVIEW; public listing still serves 0.8.0.

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
- [ ] Verify the exact 24-tool catalog and compatible MCP revision negotiation.
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

- [ ] Correct `STATUS.md` against the tree before any public wording is drawn from it. The two
  drifts named here on 2026-08-17 were re-verified against the tree and fixed on 2026-08-25: the
  live tab row is At a glance, Integrations, Status, Policy, and About (the last
  three-destination wording is gone), and `main` remains an ancestor of `dev` (verified by
  `git merge-base --is-ancestor main dev`), so the G10 promotion is still a fast-forward. Re-run
  both checks at G9 time rather than trusting this note.
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
Executed 2026-08-26 at revision `b2c27993` per the owner's explicit direction; evidence observed
and recorded the same day in [STATUS.md](STATUS.md) and
[candidate-custody-2026-08-26](testing/candidate-custody-2026-08-26.md).

- [x] Create the remote `v1.0.0` tag at the candidate revision only with owner approval.
  (Tagged `b2c27993a223c220f8828736b125676ae6f9d027` and pushed 2026-08-26.)
- [x] Create the GitHub draft, re-download every asset, compare names and hashes, then publish the
  immutable release with owner approval. (`publish-github-release.ps1` draft and publish, 2026-08-26;
  release `v1.0.0` carries all 20 files; the script re-downloads every asset for exact hash
  comparison before publishing.)
- [x] Publish the exact candidate-bound `ghostlight@1.0.0` tarball only after all six raw GitHub
  assets are observable and provenance verifies. (`publish-npm.ps1 Publish -Execute`, 2026-08-26;
  tarball SHA-256 `ca43a866f30e839d608596835c9120d7f35c54c2486de4f8859f56a2e176e49b`; `npm view`
  observed 1.0.0.)
- [x] Publish the reviewed Chrome adapter from its staged state. (`publish-extension.ps1 Publish
  -Execute` returned `PUBLISHED`/`DEFAULT_PUBLISH`, 2026-08-26; the public listing and the CRX feed
  were then observed serving 1.0.0; the published bytes are the approved `3570494f` revision.)
- [ ] Publish candidate-derived Scoop and WinGet metadata only after their referenced assets are
  observable. (The referenced assets are observable; the metadata is prepared from the candidate
  under the machine-local `.target-pkg-metadata` directory. The external bucket submissions are
  owner actions and remain owed.)
- [x] Publish MCP Registry metadata only after the exact npm coordinate is observable.
  (`publish-mcp-registry.ps1` returned "Successfully published" for `org.sylin/ghostlight` 1.0.0,
  2026-08-26; `server.json` points at the published coordinate.)
- [x] Execute the `main` outcome decided in G0. (Fast-forward push `0116feca..4ca4e6a1`,
  2026-08-26; `main` now carries the published 1.0 line.)
- [x] Reconcile the Chrome feed and every public surface from independently downloaded artifacts.
  (`reconcile-chrome-store.ps1` observed public adapter 1.0.0 matching the recorded state;
  `check-public-surfaces.ps1 -Online` reports GitHub, npm, the Chrome update feed, the MCP
  Registry, and the website in agreement at 1.0.0.)
- [x] Update `docs/public-status.json`, README, trust review stamps, website copy, distribution
  records, and changelog to observed public state. (public-status.json, README release language,
  server.json, and the changelog date updated 2026-08-26; the online check reports the website in
  agreement; the trust-center review stamps were not re-stamped and follow with the post-publication
  documentation pass.)
- [ ] Run one public install-to-first-task smoke on Windows and Linux. (A bounded public-channel
  smoke ran on Windows 2026-08-26 in an isolated profile: `npx -y ghostlight@1.0.0` downloaded and
  checksum-verified all three binaries from the public release and the 1.0.0 orchestrator answered
  `doctor --json`. The full installed first-task browser journey on both platforms remains owed --
  this host's live development graph contends with the installed swap, and the clean machines are
  the G4/G5 lanes.)
- [x] Record final hashes, links, compatibility, known limitations, and recovery guidance in
  `STATUS.md`. (The "1.0 is published" section, 2026-08-26.)

Evidence: independently observed public URLs and hashes. Publication failure never rewrites or
reuses a released version; recovery uses a higher version.

## Decision record

Fill this in once, when the release is called. A GO here is the owner's, not an agent's.

| Field | Value |
| --- | --- |
| Decision | GO (owner-directed publication) |
| Date | 2026-08-26 |
| Frozen revision | `b2c27993a223c220f8828736b125676ae6f9d027` |
| Candidate | build run 33020313866, custody verified 2026-08-26 |
| Open gates at publication | G4, G5, G6, G7, G8 remain open at the owner's 2026-08-26 direction ("get the binaries out and guarantee publication, and then we flip the extension to 1.0"); the environment lanes continue after publication and their evidence stays owed. |

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
