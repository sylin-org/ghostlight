# G4 through G10 preparation -- 2026-08-25

This document prepares the remaining 1.0 gates. It mutates nothing. G4, G5, and G7 run on
owner machines; the steps below are the runbooks for those runs. G8, G9, and G10 are
owner-authorization boundaries; the sections below are drafts waiting for decisions, not
authorizations to act.

## Candidate facts every lane starts from

```text
candidate revision: 994b6c85dcd7c8df74237cf329461d85ce49b13a (pinned by docs/release/freeze.json)
build:              GitHub release workflow run 32846030216, all jobs green
custody:            two verified copies on the development host, both passing
                    scripts/verify-custody.ps1 <copy> -IncludeProvenance
                    (local paths live in machine-local notes, never here)
store item:         lejccfmoeogmhemakeknjjdhkfkgncdl, submitted 1.0.0 PENDING_REVIEW
                    STAGED_PUBLISH, sha256 below; public listing still serves 0.8.0
```

Digests each lane needs, from the custody `SHA256SUMS`:

```text
ghostlight-v1.0.0-x86_64-unknown-linux-gnu.deb        3c0fff384374c5b15344aca21b765a46bd12394b641438702d069db97c9901f2
ghostlight-v1.0.0-x86_64-pc-windows-msvc-setup.exe    a9a2e1e20a8757cfeb2259c4ee46ee40321bfede51cff21fbc880f27e5b08d13
ghostlight-x86_64-pc-windows-msvc.exe                 d9cdb7c9f3bc7710679fbdca9b9d789422303615a5fcac8dbfaa41daf4730bec
ghostlight-browser-connector-x86_64-pc-windows-msvc.exe  1f70c4727f3b248fefc7c0e4fd75cdd394ffc5b1a46cefead503b0e007758bda
ghostlight-mcp-connector-x86_64-pc-windows-msvc.exe   f80bc6996f9729f5462936fb34329b1ef52419b415cde60c43af26ab3cd77ad6
ghostlight-x86_64-unknown-linux-gnu                   982e14fcffb87ba62c041b901cf49a9480e1cffba9aa43540ac409dc72dac729
ghostlight-browser-connector-x86_64-unknown-linux-gnu 144520e8ec19c4641c22b058a073ba77b9b8f222b98f074aa4e724430d081f0d
ghostlight-mcp-connector-x86_64-unknown-linux-gnu     450497363fd5a0ce541db9eabe1b4e5578f705277e043dda6005daa76e4539ab
ghostlight-1.0.0.tgz                                  f663e3e5c8556eb8d3d2295f6be779d7f2b07e0fe81545040efd08c24e6854fe
ghostlight-v1.0.0.mcpb                                1324e08650b9062dd72a1f71279df5c8b9a9efbd737c2705389c41a0e0c7deb6
ghostlight-extension-v1.0.0.zip                       9ae88e6729c830a9871802a39a2301c27c1d2baa00a2213332c310a7746a6db8
```

Copy assets to a lane machine by any ordinary means, then verify before any install:

```text
PowerShell:  Get-FileHash <file> -Algorithm SHA256
Linux:       sha256sum <file>
```

A digest that does not match stops the lane. Do not improvise around it.

The store adapter: once Google's review completes, install from the Chrome Web Store listing
(chrome.google.com/webstore is unscriptable; this step is human-driven by platform design).
Confirm extension id `lejccfmoeogmhemakeknjjdhkfkgncdl` and version 1.0.0 on chrome://extensions
before running any journey that requires "the matching store adapter from G3".

## G4 runbook: Ubuntu GNOME Wayland installed-product lane

Environment: a real Ubuntu GNOME Wayland session on a physical or attached display, no
Ghostlight development checkout, no virtual display. Owner-run.

1. Verify the `.deb` digest (`3c0fff38...`), then install:
   `sudo apt install ./ghostlight-v1.0.0-x86_64-unknown-linux-gnu.deb`
2. Install the reviewed store adapter (see above) once review completes.
3. Run L1 through L9 exactly as defined by [testing/linux-live-lifecycle.md](../testing/linux-live-lifecycle.md);
   that page owns the per-lane result table.
4. Verify native window, tray where GNOME provides one, Applications activation, notifications,
   and that no resident supervisor exists.
5. Run the GNOME half of the G8 accessibility matrix here (keyboard-only, screen-reader-name,
   large-text, high-contrast, reduced-motion, browser-zoom, fractional-scaling) and record the
   rendered environment sentence verbatim for the G8 desk check.
6. Record a dated record under `docs/testing/` with digests verified, lane results, and the
   environment sentence. Tick G4 rows only for what actually passed.

## G5 runbook: clean installed-Windows lane

Environment: a Windows machine or VM with no Visual C++ Redistributable, no prior Ghostlight
state, no developer toolchain. The development host does not satisfy this lane. Owner-run.

1. Verify the setup digest (`a9a2e1e2...`) and install
   `ghostlight-v1.0.0-x86_64-pc-windows-msvc-setup.exe` as an ordinary user.
2. Start every packaged executable without any separately installed redistributable: the three
   siblings under `bin/v1.0.0/` plus the raw orchestrator, browser connector, and MCP connector
   digests listed above.
3. Verify minimized first start, exact workbench activation, native Close and recreation, tray
   and notification-area behavior, notifications, explicit Quit, and that exactly one authority
   stays alive throughout.
4. Verify on-demand browser launch uses the ordinary profile with no automation flags, one
   bounded single flight, and the exact ambiguity or refusal outcomes.
5. Install the reviewed store adapter, then run the visible browser and governance journeys;
   verify connector demand-start and deploy quiesce.
6. Run logout/login and reboot demand-start; prove no Run key, scheduled task, or resident
   service carries it.
7. Upgrade from public 0.8 without clobbering user or foreign state; uninstall twice and prove
   only Ghostlight-owned state changed.
8. Record the rendered environment sentence for the G8 desk check and a dated record under
   `docs/testing/`.

## G7 runbook: candidate browser and public-harness matrix

Run the accepted matrix in [1.0/ACCEPTANCE.md](../1.0/ACCEPTANCE.md) as directed by
[RELEASE.md](../RELEASE.md) under Browser and MCP journeys. Owner-run where real harnesses are
involved. Release-specific additions:

1. Matrix passes with a visible ordinary browser profile, the matching store adapter from G3,
   and two supported Chromium families where available.
2. Exercise at least three public MCP harnesses against the packaged connector, including a
   portability-sensitive client such as Kiro/Bedrock. Prior roster work found Junie negotiates
   `2025-03-26` and Antigravity falls back from `2026-07-28` discovery; expect per-client
   revisions, not one number.
3. Verify the exact 24-tool catalog and compatible revision negotiation.
4. Verify popup and options disconnected, incompatible, setup, connected, and hold states, and
   extension identity across reload.
5. Every defect found gets a focused regression proof and a rerun of its visible journey.
6. Record a dated candidate-bound browser and harness record under `docs/testing/`.

## G8 draft: close reference-experience evaluation (waits for nothing new)

G8 consumes G4/G5/G7 evidence but several rows need no new machine and do not queue behind them:

- KDE Wayland half of the accessibility matrix can run now on the existing host.
- WSL environment-sentence observation needs a WSL harness, not the clean Windows machine.
- Extension-arrived-by-sync disposition: decide whether the existing production-id KDE Chromium
  evidence stands or must repeat against the store build once installed.
- Desk check: compare the environment sentences recorded by G4, G5, and WSL against
  `language/environment.rs`.
- Mark each of the seven ADR-0126 acceptance measures met, accepted by the owner, or not met
  with a named follow-up; then mark S8 and the epic complete only when all mandatory rows close.

Evidence lands by updating the dated evaluation and the reference-experience ledger.

## G9 draft: reconcile release-facing truth (drafts only, then wait)

- Correct STATUS.md against the tree before public wording draws from it. Two known drifts are
  already named in the checklist: destination list (live tab row is At a glance, Integrations,
  Status, Policy, About) and the `main`-is-ancestor-of-`dev` claim.
- Finalize the `1.0.0` changelog entry from `Unreleased`; finalize release notes, supported
  platforms, compatibility, install instructions, trust claims, rollback language.
- Verify icon bytes and visual identity across desktop binary, bundles, workbench, extension,
  launcher, MCPB; verify licensing files per package.
- Publication adapters default to non-mutating plan mode: `publish-github-release.ps1`,
  `publish-npm.ps1`, `publish-mcp-registry.ps1`, `prepare-package-manager-metadata.ps1`,
  `reconcile-chrome-store.ps1`. Plan-mode runs still wait for the owner's go, because their
  output shapes public wording.
- Prepare public updates (status, README, website, store, package managers, registry) without
  claiming 1.0 anywhere before independently observable artifacts exist.

## G10 draft: publish one recoverable channel at a time (all owner-approved)

Order matters; each step's referenced artifact must be observable before the next publishes:

1. Remote tag `v1.0.0` at 994b6c85 (owner approval).
2. GitHub draft, re-download and compare every asset, then immutable release publish.
3. npm tarball `ghostlight@1.0.0` only after all six raw GitHub assets are observable and
   provenance verifies.
4. Chrome adapter from staged state (review already running from G3).
5. Scoop and WinGet metadata after their referenced assets are observable.
6. MCP Registry metadata after the npm coordinate is observable.
7. Execute the `main` outcome decided in G0.
8. Reconcile the Chrome feed and every public surface from independently downloaded artifacts;
   update `docs/public-status.json`, README, trust stamps, website, distribution records,
   changelog to observed state.
9. One public install-to-first-task smoke on Windows and Linux; final hashes, links,
   compatibility, limitations, and recovery guidance recorded in STATUS.md.

Recovery rule: publication failure never rewrites or reuses a released version; recovery uses
a higher version.
