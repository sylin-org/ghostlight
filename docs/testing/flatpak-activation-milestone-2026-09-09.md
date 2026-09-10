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

## Authorized mechanism implementation checkpoint

The coordinator subsequently authorized the controlled test-02 grant and accepted
ADR-0165 under the owner's delegated fleet authority (41cb6d91). No separate owner
approval is claimed. The earlier proposal and measurements above remain historical.

The exact single-name user override was added after confirming there was no prior
per-app user override file and no per-app system override. It contains only
org.sylin.ghostlight=talk in Session Bus Policy. No blanket permission was added.
The running Chromium sandbox's /.flatpak-info lacks the new name; a fresh command-only
sandbox contains it. Therefore this route cannot provide first-time cold activation
to the already-running browser without a new sandbox. The browser was not restarted.
The first-time hot-install promise needs an explicit disposition before support claims.

Source now contains the Linux-only named-activation mechanism and a desktop readiness
hook. The authority claims its exact registered name only after RunEvent::Ready has
constructed/backgrounded the workbench and attached authenticated presentation.
No registration means no bus connection. Name acquisition neither queues nor replaces
an owner. Requests verify byte-exact selected registration and return no invented PID.
The request helper is not yet wired into Alpine's shared lifecycle admission gate;
crates/bridge/src/lifecycle.rs remains untouched. No installed binary was replaced.
Customer registration/permission ownership and installed cold/recovery proofs remain
unfinished. This is a mechanism checkpoint, not completed Flatpak support.

The private-session-bus gate starts a test-only no-argument fixture from a path with
spaces, verifies its ready marker before activation returns, verifies the owning PID,
reuses the same owner on a repeated request, refuses another installation, and proves
name release. It creates no host session-bus registration or product installation.
Toolbox initially failed this gate because dbus-run-session is absent (exit 101);
the same fresh Rust-only test executable passed on the host, where it is installed.
The failed run's .tmp/private-activation-* directory was retained, not overwritten.

Run this opt-in gate with cargo test -p ghostlight-bridge --test desktop_activation --
--ignored --nocapture in an environment with dbus-run-session. It is separate from
the actual installed Tauri/native-host acceptance still owed above.

Mechanism source gates pass: format, all-target Clippy with warnings denied,
547 Rust tests and 222 extension tests. The opt-in private-bus test is separately
passed on the host, not counted as an ordinary workspace pass. The no-argument
session-bus launch also bypasses the normal reveal-existing-workbench branch, so
a losing cold-start process cannot turn a bus activation into an Open intent.
The real-process journey also passed with GHOSTLIGHT_BIN_DIR=target/debug after a
fresh workspace build. It exercises normal relays/recovery with test adapters;
it is not the installed Flatpak cold-start proof.

## Installer and full-exchange timeout checkpoint

The coordinator directed continued implementation, treating the static permission
boundary as a reported platform constraint. Controlled acceptance restarts of the
dedicated browser are authorized; customer setup never forces a restart or edits tabs.

The opt-in package seam is now:

```text
ghostlight native-host check --flatpak-chromium
ghostlight native-host install --flatpak-chromium --allow-flatpak-activation
ghostlight native-host uninstall --flatpak-chromium
```

The grant flag is install-only. Without it, install succeeds only with the explicit
per-user app grant already present. The installer does not alter global or system
overrides. A pre-existing grant remains user-owned and survives removal. When setup
adds the grant, bounded private custody retains the previous bytes. Removal restores
those bytes if unchanged, or removes only the still-owned key while preserving later
unrelated settings. Changed grant values, foreign files, symlinks and malformed
custody remain protected. Multi-file failure attempts an exact-state rollback only.
Explicit version-path upgrades transfer custody; an older installation cannot remove
the new owner's registration. Default host data roots and home-resident siblings are
the initial scope; this is not system-package or custom-data-root support.

The host check rejects same-name .service files in the user activation directory and
the higher-priority runtime activation directory. This is an installation-state check,
not containment against later hostile host changes. Shared-runtime authentication is
still required before browser work. Seven installer regression cases pass, including
permission selection, repeat/remove/reinstall, preservation, upgrade and precedence.

The first method-only timeout did not cover D-Bus authentication. The whole exchange
now has one three-second deadline that cancels the pending async exchange. An already
submitted OS activation can still finish; the shared readiness gate must confirm it.
A real Unix socket that accepts but never authenticates passes the timeout regression.
Its first fixture failed on Unix socket path length; that failed .tmp directory was
retained and the fixture now canonicalizes its repository-local path before binding.

Source checks pass with 556 Rust tests (one opt-in private-bus test separate), format
and all-target Clippy. The new private-bus executable passed on the host again.
No installer artifact or new binary has yet been deployed to the live installation.
Alpine source d67e3b33 is now available and is the next shared-lifecycle integration.

## Shared lifecycle composition checkpoint

Alpine d67e3b33 is integrated as 56ecd4bb. Both desktop hooks are retained: publish
authenticated discovery after native readiness, then claim the optional activation
name. The Linux demand-start entry now selects named activation only for the exact
org.chromium.Chromium Flatpak identity. Other sandbox identities refuse rather than
falling back to executing the host GUI inside the sandbox. Native callers keep their
existing sibling route. The selected runtime installation must match registration.

The Flatpak callback runs inside the shared startup admission, deployment quiescence,
readiness wait and failed-start cooldown. It returns ActivationRequested, never an
invented child PID. The expanded private-bus proof invokes the actual shared entry,
proves a deployment marker prevents activation, observes fresh discovery under an
authority lease, and confirms a repeated request reuses the owner. This remains a
test fixture, not installed-browser acceptance.

Format, warnings-denied Clippy, 562 Rust tests and 222 extension tests pass. The
private-bus test passes separately. The real Linux startup-recovery journey also
passes on Bluefin: four missing-context callers produce at most one authority at
once, three bounded failed attempts in 11.5 seconds, no published runtime, and no
initialized client. Corrected context recovers all four callers to one authority
in 5,842 ms. Evidence is in .tmp/linux-startup-recovery/after/result.json.
The fresh GHOSTLIGHT_BIN_DIR=target/debug real-process journey also passes, including
relay reconnect, browser fixture work, audit failure/recovery and cold-start refusal.

The installed graph remains unchanged. Deployment has a layout constraint: the
mandatory scripts/dev-loop.ps1 requires both build and live directories inside its
repository, while the existing installation is still in the retained external fleet
tree. This checkpoint does not bypass that guard, manually swap live images, or move
the installation. A deliberate layout decision is needed before deployed acceptance.
Installer precedence/name-claim review and actual cold/recovery/remove/reinstall
acceptance remain open; this does not finish the Flatpak assignment.

## Deployment scope resolved and ownership review

The coordinator authorized the existing exact live sibling directory as a scoped
exception in the ignored host controller. New builds, drivers and evidence remain
in the ordinary repository. No migration or generic external-directory switch is
needed. Registration, installed hashes and exact authority process custody were
verified before preparing the controller; the existing profile/session and binary
baseline is preserved opaquely in .tmp/flatpak/before-live.

Review now requires the host runtime directory before setup can certify activation
precedence. The desktop also checks current paired registration and precedence
before claiming the optional name; failure leaves its warm authenticated route
available. An upgrade refuses native registration changed to another installation,
including when the activation registration still matches prior custody. The new
regression and all 563 Rust tests pass, as do format, Clippy, 222 extension tests,
the private-bus gate and the fresh real-process journey.

The controller incorporates Alpine f3396e31's exact-destination connector recheck
on copy retry. This keeps browser respawn from pinning an obsolete connector image
through a locked deployment. Product and controller source are checkpointed before
the installed swap; no installed activation pass is implied by these source gates.
