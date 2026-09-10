# Flatpak activation design milestone

Source baseline: ac1becab78e5a8a6b89e553e82f6779d93dcb39c.
Branch: codex/fleet-test-02-flatpak-activation.
Work location: ordinary Ghostlight repository; no new worktree or fleet directory.
Product source delta: none. Installed binaries and registration: unchanged in this round.

## Outcome

The [bounded proposal](../design/flatpak-host-activation.md) is ready for review.
The smallest recommended route requires an explicit new app-wide named D-Bus talk
grant and an accepted ADR before implementation. No such grant was applied.
The baseline host remains Ready; no cold-start or recovery pass is claimed.

## Bounded observations

| Check | Result |
| --- | --- |
| Installed doctor, read-only | Service 1.3.5 running; Ready, connected and idle. |
| Actual Flatpak invocation of installed authority with --version | Exit 127, loader cannot find libwebkit2gtk-4.1.so.0. No authority is created by this probe. |
| Installed portal introspection | OpenURI v5, DynamicLauncher v1; no native-messaging or WebExtensions interface. |
| Sandbox OpenURI.SchemeSupported for ghostlight | false. No URI handler was installed or invoked. |
| Host ListActivatableNames | No Ghostlight name. |
| Sandbox introspection of proposed org.sylin.ghostlight name | ServiceUnknown; no registered target exists, so not standalone proof of a denied grant. |
| Current host desktop context | All six existing forwarding names present; no environment values published. |
| Actual Flatpak permissions | Existing home/network access; no Ghostlight or general host-spawn talk grant. |
| Direct coordinator/Alpine task messages | Failed routing from this host; Git report carries the handoff. |

The OpenURI and DynamicLauncher checks did not prompt the user, create a launcher,
change a MIME default, open a browser window or operate another desktop application.
The actual installed Flatpak warm-page proof remains the earlier 6883aa59 evidence,
not a rerun or a substitute for the requested cold activation proof.

## Installed identity before and after inspection

The live installation remains in its pre-existing fleet source directory. These
hashes were rechecked and match the published integration candidate:
db57b9bf37631ff3029901f7ba43f7edc405fd24. The new source baseline has not been deployed.

| Sibling | SHA256 |
| --- | --- |
| ghostlight | 047a14947d70973e54ae34df3a33db1fa1d4ffcd633faf3636e079d3888a88dd |
| ghostlight-mcp-connector | 301d579aa8b014eb05f2d2dd711273cb015de01cac6151a48766258e46f8a9b0 |
| ghostlight-browser-connector | 30f3b26e2db7224739b357612e7cab31766b830c25a316035a4ef72dc3121159 |

No policy, browser preference, credential, installed path or executable was modified.
The manually added Flatpak manifest and preserved disposable tab remain intact.

## Source gates

The baseline was verified in this repository's target directory through the
existing Fedora Toolbox build environment:

- cargo fmt --check: passed.
- cargo clippy --workspace --all-targets -- -D warnings: passed.
- cargo test --workspace: 542 passed, including 457 orchestrator library tests.
- npm test --prefix extension: 222 passed.
- git diff --check and ASCII checks on the new documents: passed.

No changed JavaScript requires a syntax check. Process journeys and live deployment
are not claimed for this documentation-only milestone. The source gates do not
establish a new activation mechanism or physical desktop acceptance.

## Next handoff

The coordinator can approve or reject the proposed narrow trust boundary and allocate
the ADR number. If approved, registration, activation and lifecycle acceptance follow
the proposal's exact scope, with Alpine retaining generic retry suppression ownership.
Until then, implementing or exercising the grant would exceed the unresolved decision.
