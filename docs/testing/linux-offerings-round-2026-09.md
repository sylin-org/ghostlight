# Linux offerings implementation and coverage

Status: Deferred before dispatch. The owner first requested coordinated coverage,
then asked to wrap the campaign and prepare the release on 2026-09-09. None of the
assignments below was sent and no new monitor was created. This is a proposed future
ownership map, not active work or new support. Baseline:
`94e5387f435c685dcc41e64fc775208641c9c34f`. The completed fleet round is preserved
in [its board](fleet-focused-round-2026-09.md). Release preparation is recorded in
[the wrap-up](release-wrap-2026-09-09.md).

## Ownership

| Machine | Owned work | Completion evidence |
| --- | --- | --- |
| test-01, CachyOS | Arch PKGBUILD/package, Fedora RPM recipe and conventional Fedora consumer; shared Linux package build/check scripts | Exact candidate builds, dependency/payload checks, install/upgrade/removal and retained-client recovery; native CachyOS smoke; Fedora container/VM scope stated accurately |
| test-02, Bluefin | System-package + Flatpak-browser route and container-based MCP client connection; Flatpak installer and activation boundary | Existing /usr-installed authority works with a real Flatpak browser without broad host execution; explicit registration/permission ownership; an actual container client reaches the host authority/browser with correct identity, failure and recovery behavior |
| test-03, Alpine | Linux setup diagnostics and supported client repairs; native APK recipe | Clear native/Snap/Flatpak/ambiguous states; actionable desktop-context failure and explicit repair preserving foreign configs; actual native clients; APK build, ownership, lifecycle and native-musl installed checks |
| leo-desktop-02, Windows | Ubuntu 24.04 LTS GNOME guest acceptance; CI/distribution metadata and Linux Homebrew formula | Dedicated VM with exact candidate, normal desktop/browser/client journeys using allowed tools; truthful virtual/physical boundaries; existing CI extended to real recipe checks and provenance; stale formula reconciled and validated on a Linux machine |

The six topics map to these lanes: setup/diagnostics to Alpine; system-package
Flatpak and container clients to Bluefin; fresh Ubuntu GNOME to Windows; distribution
channels jointly to CachyOS/Alpine recipes and Windows CI/metadata; Fedora RPM to CachyOS.
The coordinator reviews/integrates and relays shared interfaces through Git milestones.

## Shared boundaries

- Work in the ordinary repositories on owned `codex/linux-*` branches. Preserve
  previous installed directories, artifacts, failures, credentials and user work.
- Agent autonomy includes prerequisites, controlled machine/VM operation, scoped
  fixes, required gates, signed-off commits and own-branch pushes. No force push,
  owner-branch merge, public publication or external messaging is included.
- Bluefin owns shared activation/lifecycle changes and Flatpak registration. Alpine
  owns harness configuration and onboarding semantics. Coordinate changes to shared
  CLI/install/language files through published narrow diffs; do not independently
  rewrite each other's seams. Reserve ADR-0167 for Bluefin and ADR-0168 for Alpine
  if decisions change. CachyOS may use ADR-0169 for a needed packaging decision;
  Windows may use ADR-0170 for a needed CI/distribution decision.
- Container access is an explicit host trust/identity boundary, not permission to
  expose the authority on all interfaces, add unauthenticated endpoints, scrape
  desktop environments, share an entire home, or introduce a second authority.
  Prefer existing bounded authenticated mechanisms; document any new decision.
- System package + Flatpak support must preserve one selected host authority,
  owned registration, bounded startup and explicit grants. No blanket filesystem
  or bus access, generic host command launcher, hidden supervisor or silent fallback.
- Human browsing remains outside Ghostlight control and activity. Customer setup
  never restarts browsers or edits tabs. Controlled test restarts preserve profiles
  and preferences. Enforced tool restrictions still apply; no alternate-route bypass.
- Existing installed paths outside the ordinary repository may be used through
  narrowly scoped local controller adaptations after verifying exact installed
  registration/process custody. Retain deployment locks and exact-image checks.
  Routine repository guards are not a reason to stop for owner permission when
  the authorized exact-path adaptation suffices. Execute deployment on the host.
- No host reformat is needed. Use a dedicated Ubuntu VM if resources and allowed
  controls support it. Virtual GNOME/Wayland tests do not prove physical suspend,
  GPU, HiDPI or multiple-monitor behavior. Record unavailable resources precisely
  and continue independent coverage; do not mark those rows passed.

## Delivery

Each machine publishes an early source/evidence milestone and continues autonomously
through implementation and installed acceptance. Milestones are coordination aids,
not mandatory coordinator approval stops. Tool readers may omit task text; Git
reports must carry exact source/artifact identities, actual results and remaining gaps.
Report real unresolved restrictions or decisions promptly instead of sitting idle.

Use existing release/CI seams. Each new gate must prove a specific install, upgrade,
ownership or recovery promise. No new release conductor or repeated broad campaign.
CI configuration work does not itself prove a workflow ran or establish attestation
for a local artifact. Do not publish or silently update a tap/store/package repository.

The coordinator will integrate scoped completed changes, run required central checks,
and return only affected combined candidates for targeted platform verification.
Keep a matrix of native, container, virtual desktop, physical desktop, client and
package evidence so success in one does not substitute for another.

If this round resumes, quiet follow-up monitoring should continue until all assigned topics are implemented and
verified, or remaining resource/product boundaries are explicitly recorded with no
autonomous work left. Remove the monitor at that point. Do not expand this round
into ARM64, NixOS, more desktops or public release work without another request.
No retrieval state or recurring task was created for this deferred round.
