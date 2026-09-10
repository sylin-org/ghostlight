# Flatpak host activation: bounded proposal

Status: Proposed, not accepted or implemented. Date: 2026-09-09.
Baseline: ac1becab78e5a8a6b89e553e82f6779d93dcb39c.
Lane: codex/fleet-test-02-flatpak-activation, ordinary Ghostlight repository.

## Decision needed

Approve or decline a single explicit per-app session-bus talk grant for
org.chromium.Chromium to one installer-owned Ghostlight activation name,
org.sylin.ghostlight, matching the existing desktop application identifier.
The grant applies to the Chromium application,
not only Ghostlight's extension. It permits starting the installed host authority;
it must not permit commands, arguments, environment changes, browser work, policy
changes, or reading product data over D-Bus.

This changes the no-additional-activation-boundary decision in ADR-0104 and the
native-only browser scope of the current contracts. An accepted numbered ADR must
precede implementation. ADR-0115's no-resident-supervisor decision and ADR-0127's
one complete desktop authority remain unchanged. This proposal is not an implicit
permission grant, a Flatpak support claim, or a request to relax the exclusions.

## Observed boundary

The prior real-extension warm open/read proof at 6883aa59 remains valid. The
manually registered connector runs inside Flatpak and authenticates to the host
authority. It cannot directly execute that GUI authority with the sandbox runtime's
libraries: WebKit 4.1 is absent. Registration alone cannot solve cold startup.

Read-only inspection on Bluefin 44.20260908 found:

- Chromium Flatpak 152.0.7977.82 grants home and shared network, but neither a
  Ghostlight talk name nor org.freedesktop.Flatpak host-execution access.
- The desktop portal exports OpenURI version 5 and DynamicLauncher version 1,
  but no native-messaging or WebExtensions portal.
- The sandbox can call OpenURI.SchemeSupported. It returns false for ghostlight.
- The host's activatable names contain no Ghostlight service. Querying the
  proposed name inside the sandbox returns ServiceUnknown. Because no such
  service is installed, this response alone does not prove a filtering denial.
- The current host user manager has all six desktop-context names used by the
  existing Codex forwarding repair. This is not proof that a future D-Bus launch
  will inherit a correct environment; that requires the actual cold-start test.

No permission override, activation registration, URI association, installed binary,
user preference, or browser profile was changed for this design investigation.

## Routes considered

| Route | Disposition |
| --- | --- |
| Existing native-messaging portal | Not exposed by this host. Do not depend on a proposed upstream interface. |
| Direct authority execution in sandbox | Wrong runtime; cannot initialize host GUI dependencies. No second authority in the sandbox. |
| Browser extension point | Supplies files and manifests, not host activation. |
| OpenURI with a new Ghostlight scheme | Supported portal mechanism, but no handler exists here. Routing and possible prompts are user-controlled; it does not bind the exact trusted installation. A separate user-facing design would be needed. |
| DynamicLauncher | Its documented sandbox launchers execute inside the requesting app, with Exec rewritten to flatpak run. This does not launch the host authority. |
| Host spawn permission, blanket bus/filesystem grant, supervisor | Excluded by the task and existing product boundaries. |
| Exact named D-Bus activation | Recommended for review, contingent on the explicit narrow grant and an accepted ADR. |

Do not borrow Chromium's own bus namespace or impersonate a portal, notification,
file-manager, or other already-allowed service to avoid the grant.

## Proposed implementation boundaries

1. The orchestrator installer owns one ordinary session D-Bus .service file with
   a fixed Name and an exactly quoted absolute Exec for the selected installed
   ghostlight executable, with no arguments. It is not a systemd user unit and
   creates no login startup, restart policy, wrapper, or helper process.
2. The same host authority acquires that name only when the owned registration
   identifies its installation. It still acquires the existing runtime lifetime
   lease before desktop/listener initialization. The bus name is not a substitute
   for the lease, readiness, or authenticated runtime handshake.
3. No Ghostlight D-Bus product method is needed: the connector requests the fixed
   name through org.freedesktop.DBus.StartServiceByName, then retries its ordinary
   authenticated runtime connection. The authority keeps the name for its lifetime.
   Activation acknowledgement alone never establishes product readiness.
4. The shared lifecycle seam selects this route only for the explicitly registered
   Flatpak installation, after existing deployment and start-attempt checks.
   Normal native callers retain direct sibling launch. An explicit runtime override
   or a registration naming another installation must not fall back to the fixed
   name. No executable path is accepted from the bus or browser wire.
5. Name acquisition must not reveal or focus the workbench. Concurrent starts still
   converge before desktop construction. Quit releases the name with the authority.
   Failed activation must participate in shared bounded retry suppression, never
   cause a browser reconnect loop to repeatedly launch GTK failures.
6. Install/check/remove cover the actual application-private native-manifest root
   and the host activation file as distinct owned artifacts. Unsupported or missing
   activation/grant must not appear Current merely because a manifest exists.
   Foreign, malformed, or ambiguous artifacts stay byte-identical. Deliberate
   cross-install adoption remains an explicit install action.
7. Installation must show the proposed app-wide grant before applying it. Existing
   user overrides and settings remain intact. Removal must distinguish a grant
   Ghostlight introduced from a pre-existing user grant; never reset all overrides.
   If exact ownership cannot be established, retain and report it rather than
   silently revoke a user's permission. No general sandbox override is a fallback.

The .service file must be checked against higher-priority/duplicate registrations
before declaring the selected target usable. The first implementation should not
claim arbitrary portable/system installation layouts are sandbox-visible: this
machine proves only the existing home-resident sibling set. Package-root exposure
needs its own supported mechanism and test, not a filesystem grant added silently.

## Alpine coordination seam

The Alpine agent owns generic failed-start suppression in crates/bridge/src/lifecycle.rs.
This lane has not edited it. Keep suppression independent of direct process creation
so a later bounded activation request shares the same retry gate. A D-Bus request
must not fabricate StartDisposition::Spawned's process_id; use a truthful distinct
disposition if the reviewed implementation cannot obtain a verified host PID.

Both attempted direct task messages failed: the coordinator task was not found on
this host, and the supplied Alpine remote host had no registered AppServerManager.
This Git milestone is the coordination handoff, not evidence that Alpine received
or accepted an interface change.

## Required implementation acceptance

- Snapshot source, installed sibling hashes, owned registrations, relevant overrides,
  extension identity and preferences; retain failed evidence.
- Prove no authority exists and the runtime lease is free before the browser-led
  request. Do not warm it with an MCP invocation first.
- Prove one host authority, no sandbox authority, expected exact executable, correct
  desktop initialization, current runtime authentication and actual extension
  open/read against a disposable local fixture.
- Stop only the exact installed authority; prove the same native relay recovers.
  Then stop only the exact native connector and prove the actual adapter reconnects
  without resetting its identity or replaying actions.
- Prove cold bursts, unavailable bus/name/grant, deployment quiescence, failed GTK
  startup, stale registration, and alternate-runtime refusal are bounded and truthful.
- Prove owned install/repeat/upgrade/remove/reinstall, foreign/malformed preservation,
  pre-existing permission preservation, and no reset of user browser preferences.
- Preserve-tabs remains on. Leave disposable tabs visibly preserved if no permitted
  human cleanup route is available; never use page script or another automation
  path to defeat its refusal.
- Run ordinary Rust/extension gates and relevant process journeys against explicitly
  named fresh binaries before deployment and final support claims.

## Sources

The [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html)
defines session activation files, exact Exec registration and StartServiceByName.
[Flatpak permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html)
define filtered bus access and recommend minimum talk permissions.
[OpenURI](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.OpenURI.html)
leaves handler selection under user control; ask=false does not guarantee a
particular handler. [DynamicLauncher](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.DynamicLauncher.html)
documents its application-local execution boundary. These describe mechanisms,
not a passed Ghostlight cold start.
