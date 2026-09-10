# Focused fleet implementation round

Started 2026-09-09 at the owner's instruction to coordinate the machines autonomously.
Starting revision: `ac1becab78e5a8a6b89e553e82f6779d93dcb39c`.
Coordinator branch: `codex/fleet-next`, ordinary repository.
The preceding [campaign](fleet-acceptance-2026-09.md) remains historical evidence.

## Assignments and completion evidence

| Machine | Ownership | Required result |
| --- | --- | --- |
| test-01, CachyOS/KDE | Linux packaging, build/consumer scripts | Fresh portable and Debian candidates containing integrated fixes; Ubuntu 22.04 build and Debian 12/Ubuntu 24.04 consumers; exact hashes, dependencies and installed lifecycle evidence. Earlier a8cfd033 packages do not close this lane. |
| test-02, Bluefin/GNOME | Flatpak registration and bounded host activation; ADR-0165 | Actual browser-led cold activation of one host authority, open/read, reconnect, deployment and owned registration/permission lifecycle. Warm attachment alone is insufficient. |
| test-03, Alpine/KDE | Shared bridge failed-start suppression and orchestrator readiness; ADR-0166 if needed | Reproduce sanitized-environment failure; bound concurrent failed launches; recover when desktop context is corrected; verify installed native-musl client/browser behavior. |
| leo-desktop-02, Windows | Windows hooks, peer boundary and customer acceptance | Current-source installer artifacts and real failure-path evidence; actual installed browser journey if permitted controls/adapter become available; retain exact blocked limits. |

Each agent controls its dedicated machine, chooses its sequence, fixes concrete defects,
runs required gates, signs off commits and pushes its own `codex/fleet-*` branch. Work
uses the ordinary repository. Existing installed paths and prior worktrees remain intact.
No new credentials are needed: all four machines have verified owner identity and push access.
No force push, owner-branch merge, store submission or public package release is included.

## Coordinator decisions and shared seams

Bluefin's `b12234d0` proposal on `codex/fleet-test-02-flatpak-activation` narrows
activation to one named session-bus grant: Chromium may start `org.sylin.ghostlight`.
The coordinator authorizes this controlled test-machine experiment under the owner's
full machine-control delegation. This is not separate owner acceptance of a public
support claim. Bluefin must record ADR-0165 before implementation and explicitly
disclose/select the app-wide grant during customer installation.

The bus may start only the fixed installed no-argument authority. It must not carry
product operations, arbitrary commands, environment overrides or data access. Exact
registration ownership, prior permission preservation, runtime authentication, lifetime
custody and desktop readiness remain required. No general host spawn permission,
blanket bus/filesystem grant, sandbox authority or resident supervisor is allowed.

Alpine owns generic startup suppression. Bluefin consumes that shared mechanism;
an activation request must not invent a spawned PID. The coordinator relays interface
milestones because direct cross-host task messages failed. The reviewed baseline
published ServiceHost before Tauri initialization and waited for activation after every
desktop startup error. Alpine's integrated repair resolves both with real failure evidence.

Human browsing remains outside Ghostlight control and activity (ADR-0164). Test code
must respect preserve-tabs and other user choices. Full machine authority does not
override enforced tool restrictions or authorize alternate routes around rejected actions.

## Milestones received

- Bluefin `46577c6d`: bounded named activation and opt-in installer now pass actual
  installed cold startup, open/read, idle authority/relay recovery and owned
  remove/reinstall. Cold startup took 1,904 ms with one host authority. The first
  cache-refresh failure led to an installer correction; the unassisted repeated
  lifecycle passed. All 564 Linux Rust and 222 extension tests, private-bus and
  process fixtures pass. Existing profile, identity, permission and preserve-tabs
  choices remain intact. New talk permissions need a new sandbox; setup never
  restarts the browser. The home-resident/default-root scope remains explicit.
- Windows `13e378de` and `e4a972d4`: production NSIS hooks exercised through an isolated fixture.
  Empty destination, foreign deployment marker, executable-path obstruction, explicit
  failure cleanup and unrelated authority preservation pass. Product hooks are unchanged.
  All 533 Windows Rust and 222 extension tests pass. A combined release package then
  replaced all three siblings with verified expected bytes; real Codex policy work,
  VS Code catalog recovery, process and provenance journeys pass. The final installer
  exit code was not retained. Browser and held-file uninstall limits remain open.
  Follow-up `d27b176d` then validates exact startup candidate 09890c9f: installed
  package exited 0 in 20.35 seconds, all three expected images match, actual Codex
  cold startup observed one authority, VS Code recovered its unchanged configuration,
  and 19 unrelated VS Code/Edge processes survived. All 537 Windows Rust and 222
  extension tests pass. The Codex invocation uses an explicit per-run configuration;
  it is not saved-registration discovery. Browser/uninstall restrictions are unchanged.
- CachyOS `cd37fd54`: exact ac1becab portable/Debian candidates pass Ubuntu-baseline
  build, Debian 12/Ubuntu 24.04 consumer lifecycle and retained-MCP upgrades. Normal
  installed Chromium/Brave typing/readback pass after authority-only deployment.
  The initial concurrent Ubuntu readiness timeout is retained; a fresh serial run
  passed unchanged bytes. Report and candidate manifest integrated in `7291e68d`.
- Alpine `f3396e31`: completed shared startup custody/readiness (runtime d67e3b33),
  installed native-musl Codex and browser cold starts, adapter rejoin, open/read and
  native workbench acceptance. Four sanitized callers produced at most one transient
  authority and no runtime publication; corrected-context callers recovered to one
  authority in 5,874 ms including cooldown. All 547 native Rust and 222 extension tests
  pass. The deployment controller also repairs exact selected connector respawn races,
  with a real Linux executable-lock regression. No physical reboot is claimed.
  Follow-up `85d2d556` verifies exact combined candidate 0129ff15 with the new Linux
  dependencies: 564 Rust and 222 extension tests, private-bus and process/startup
  recovery gates, installed configured Codex cold start, native Chromium rejoin,
  adapter reload and fresh open/read all pass. Saved configuration, registration
  and adapter bytes are unchanged. No product fix or Alpine Flatpak setup was needed.

CachyOS and Windows evidence, Windows hook fixtures and Alpine's startup/deployment
repairs are integrated on this branch. Required central checks pass with 537 Rust and
222 extension tests. Bluefin's reviewed activation implementation is also integrated.
Packages previously verified at ac1becab
do not contain the new startup repair; affected package checks follow the combined
Linux candidate rather than rebuilding once for every intermediate milestone.

Flatpak integration is committed at `0129ff15`. Its Cargo inputs, product crates,
extension and dev-loop controller are byte-identical to Bluefin's tested 46577c6d
tree. Central Windows gates pass; Linux-specific private-bus and live evidence come
from Bluefin. Combined native-musl compatibility is complete. The only remaining
autonomous assignment is CachyOS's targeted package build and consumer validation,
followed by evidence collection.

## Integration and follow-through

Review source and evidence before integrating into `codex/fleet-next`. Reconcile shared
lifecycle changes, run the required central gates against the merged source, and ask the
affected machine to validate the exact resulting candidate when the change warrants it.
Keep package identity separate from source-test identity. Do not rerun settled broad
campaigns or retry unchanged tool blocks just to produce activity.

The Windows browser lane currently needs the current repository's `extension` directory
loaded in ordinary Chrome through an allowed human action. Browser Use prohibits extension
management. The previously rejected held-file uninstall remains unverified; automatic
approval review reported `blocked by policy`. Independent package work continues.

The `coordinate-ghostlight-focused-fleet-round` heartbeat is active every ten minutes.
It stays quiet for unchanged/non-actionable state, reports meaningful changes or required
user action, and removes itself once results are integrated or the remaining limits are
explicitly recorded with no autonomous work left. The prior campaign monitor stays deleted.
Local retrieval state lives in `.tmp/fleet-acceptance/focused-round-state.json`, never in
model-private memory.
