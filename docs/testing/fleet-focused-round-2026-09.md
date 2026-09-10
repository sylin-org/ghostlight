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
publishes ServiceHost before Tauri initialization and waits for activation after every
desktop startup error; Alpine is investigating both with real failure evidence.

Human browsing remains outside Ghostlight control and activity (ADR-0164). Test code
must respect preserve-tabs and other user choices. Full machine authority does not
override enforced tool restrictions or authorize alternate routes around rejected actions.

## Milestones received

- Bluefin `b12234d0`: bounded activation design and baseline source gates, 542 Rust
  and 222 extension tests. No activation implementation or changed sandbox grant yet.
  Implementation continuation was sent after coordinator review.
- Windows `13e378de`: production NSIS hooks exercised through an isolated fixture.
  Empty destination, foreign deployment marker, executable-path obstruction, explicit
  failure cleanup and unrelated authority preservation pass. Product hooks are unchanged.
  All 533 Windows Rust and 222 extension tests pass. This is not a held-file uninstall
  or current installed-browser pass. Fresh release artifacts remain in progress.
- CachyOS and Alpine: new work dispatched; no new implementation result collected yet.

These milestones are reviewed reports from remote branches, not merged changes on this
coordination branch. Update this section when integration and checks actually complete.

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
