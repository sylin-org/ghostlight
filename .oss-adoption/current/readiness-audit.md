Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z

# Readiness audit

## Executive finding

Ghostlight has a stronger proof, trust, and distribution foundation than most projects at this
age. The remaining broad-launch blockers are concentrated in the last mile: the store path,
greenfield onboarding evidence, and public-surface consistency. The right move is a narrow proof
cohort now, while repairing the inconsistent public state, then a founder-present launch after the
Chrome Web Store path is public and verified.

## Remediation update - 2026-07-18

All repository-controlled repairs from this audit are implemented in the working tree:

- `docs/public-status.json` is now the canonical release, platform, and extension-store record;
- CI and release preflight validate that record against package manifests and the README;
- website source consumes a synchronized fallback, and the release publisher now refreshes it;
- the README links the browser-control decision aid from the entry path;
- bug, installation, and pull-request intake templates are present, with Discussions selected for
  questions, ideas, feature requests, and workflows;
- root support and governance routers point to the existing authoritative policies;
- an accessible 1280x640 social preview and reproducible SVG source are ready for upload;
- the proof-cohort acceptance contract is executable without private success criteria; and
- an evidence-linked OSPS Baseline 2026.02.19 self-assessment records both strengths and gaps.

The remaining gates are not repository defects: deploy the website source, upload the social card,
wait for Chrome Web Store review, run the non-author cohort, complete macOS live verification, and
decide whether to change GitHub security settings identified by the OSPS assessment. No external
action is authorized by this update.

## Blockers before broad publication

### Blocker: public product truth is inconsistent

The repository and latest release are v0.6.0. The public project page still labels Ghostlight
v0.5.7. The README says Windows and Linux are live verified and macOS live verification is owed;
the website says only Windows is live verified and both Linux and macOS are owed. The website also
uses 0.5 in its project index. A new visitor cannot know which platform statement is current.

Repair: update the public site from canonical release and verification state, then add a release
check that detects stale version and platform-copy surfaces. Verify the logged-out page afterward.

Evidence: `../../README.md`, `../../docs/STATUS.md`, https://sylin.org/ghostlight/, and
https://github.com/sylin-org/ghostlight/releases/tag/v0.6.0.

### Blocker: the default extension path is still pre-release

The Chrome Web Store listing remains under review. Installation therefore requires downloading,
extracting, enabling Developer mode, and loading an unpacked extension. That path is honest and
usable for informed testers, but it creates enough friction and perceived risk to undermine a
broad practitioner launch. Chrome's listing policy also requires complete, accurate, current
metadata and privacy fields, so final store copy must remain aligned with runtime behavior.

Repair: let review complete, install the accepted package from a clean profile, and make the store
path the default while retaining the inspectable manual path.

Evidence: `../../docs/guides/installation.md`, `../../ROADMAP.md`, and
https://developer.chrome.com/docs/webstore/program-policies/listing-requirements.

### Blocker: greenfield first success needs a recorded acceptance pass

The public path is well documented, and Windows/Linux live work has been exercised during
development. Before borrowed attention, a stranger should complete the exact public path on clean
Windows and Linux machines with no private correction. The acceptance record should name release,
browser, client, store/manual extension path, doctor result, first task, and any intervention.

Repair: run five to ten proof users across at least three clients. Treat any private explanation
as an onboarding defect. Exit when most reach first success unaided and repeat the product boundary
accurately.

## Important improvements

### Community intake is incomplete

GitHub reports a 75% community profile. README, license, Code of Conduct, contribution guide, and
security policy exist, but issue and pull-request templates do not. There are no public Issues or
Discussions yet, so a launch would create an unstructured first impression.

Repair: add a minimal bug form, install-friction form, workflow/show-and-tell Discussion guidance,
and pull-request template. Do not create more lanes than the solo maintainer can support.

### Social preview is generic

Repository metadata is now strong: a practitioner-first description, homepage, and ten relevant
topics are set. GitHub reports no custom Open Graph image, so shared links use a generated preview.
The existing mascot and hero provide the ingredients for a distinct, legible social card.

Repair: create one accessible social preview with the mascot, product truth, and no small copy.

### Public support and governance paths are distributed

Support expectations live in CONTRIBUTING, MAINTENANCE, SECURITY, and the Trust Center. That is
substantively good, but the inventory finds no root SUPPORT or GOVERNANCE file. This is not a
launch blocker for a solo pre-1.0 project, though a short routing file could reduce uncertainty.

### The decision aid is not visible from the repository entry path

The public website links the browser-control decision aid, but the README's comparison path goes
only to the repository comparison. A single link near "Is this your problem?" or the comparison
guide would help evaluators choose without pulling organization material into the first screen.

### macOS scope must remain explicit

macOS artifacts and CI exist, but live-browser verification remains owed. This is acceptable if
every surface calls it out consistently. Do not turn CI success into an end-to-end claim.

## Legal and ownership

- **Strong:** the engine is Apache-2.0 OR MIT; governance has an explicit separate license.
- **Strong:** CONTRIBUTING explains DCO versus CLA boundaries.
- **Strong:** pricing explains exactly when organization governance needs a paid license.
- **Important:** before soliciting funding, settle recipient, entity, tax, and provider details and
  state that support earns gratitude only.

## First success and installation

- **Strong:** practitioner-first README, one command, visual four-stage path, first read-only task,
  doctor, uninstall, and manual path.
- **Strong:** canonical AI-readable install page is live and returns HTTP 200.
- **Important:** extension review prevents the intended low-friction default.
- **Helpful:** publish a 60-90 second store-to-first-task walkthrough after acceptance.

## Product evidence and limitations

- **Strong:** current hero, live demos, exact tool table, comparison, ADRs, tests, release assets,
  and Trust Center.
- **Strong:** non-goals are unusually clear: no headless, cloud, stealth, bulk, or isolated browser.
- **Important:** reconcile all platform and version claims.
- **Helpful:** obtain two or three permissioned practitioner workflows after the proof cohort.

## Repository and community health

- **Strong:** description, homepage, topics, README, changelog, releases, security, conduct, and
  contribution files.
- **Strong:** npm and MCP Registry metadata are complete and agree at v0.6.0.
- **Important:** add issue and PR templates and decide whether Discussions is the primary public
  first-use lane.
- **Helpful:** add a custom social card and later a funding link when operationally ready.

## Security and operational trust

- **Strong:** local runtime, no product telemetry, explicit residual prompt-injection risk,
  private reporting route, SBOM, checksums, attestations, separated release jobs, and public trust
  documentation.
- **Important:** a third-party penetration test is absent and correctly disclosed.
- **Important:** solo-account release authority is a named continuity risk.
- **Helpful:** complete an OpenSSF Best Practices or OSPS Baseline self-assessment as a public,
  bounded trust artifact rather than adding a decorative badge.

## Maintainer capacity

One founder is the product, release, support, moderation, security, and public voice. Start with
five to ten proof users, one anchor launch, and at most two or three adapted communities. Pause if
the same install problem occurs three times, support work exceeds roughly eight focused hours in a
week, or a security issue cannot be handled without deferring public discussion.

## Ordered repairs

1. Reconcile website v0.5.7/Linux copy with v0.6.0 canonical truth.
2. Complete Chrome Web Store review and clean install acceptance.
3. Add minimal issue and PR intake templates.
4. Run the proof cohort and turn private help into public onboarding fixes.
5. Create a custom social preview and logged-out link check.
6. Choose the founder launch account, public feedback lane, and protected launch window.
7. Only then execute the staged publication plan.

Repository repair status: items 1, 3, and the local portions of 5 and 6 are complete. Items 2 and
4 require external systems or people. Website deployment and social-preview upload are explicit
external actions still awaiting authorization.
