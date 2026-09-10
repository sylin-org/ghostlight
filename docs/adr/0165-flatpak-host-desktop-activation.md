# ADR-0165: Named host desktop activation for Flatpak Chromium

- Status: Accepted for implementation; installed acceptance pending.
- Date: 2026-09-09
- Authority: coordinator engineering decision under the owner's delegated control
  of the dedicated fleet machines. This is not separately obtained owner approval.
- Amends: ADR-0104's activation-boundary restriction and the current native-only
  browser installation scope, subject to the acceptance boundary below.
- Preserves: ADR-0115, ADR-0127, ADR-0149, ADR-0150 and ADR-0164.

## Context

The real Flatpak Chromium adapter connects to a running host authority through the
installed native connector. The same sandbox cannot execute the full GUI authority:
its WebKit runtime dependency is absent. This host exposes no native-messaging
portal. The bounded investigation and alternatives are recorded in
[the activation proposal](../design/flatpak-host-activation.md).

## Decision

Use one ordinary session D-Bus activation name, org.sylin.ghostlight, naming exactly
the selected installed host executable with no arguments. The existing host authority
owns the name only after its lifetime lease and desktop startup have completed.
Do not publish an activation acknowledgment before desktop readiness. A bus response
does not replace the connector's ordinary authenticated runtime handshake.

The bus has no Ghostlight workbench or product methods. Workbench reveal stays on the
authenticated service bridge. No shell, arbitrary command, caller-selected path,
environment, browser request, policy operation or data response is exposed over D-Bus.
There is no extra authority, helper daemon, resident supervisor, login startup or
systemd user unit. Existing lifetime lease, deployment quiescence, bounded startup
suppression and uncertain-effect non-replay remain authoritative.

The coordinator authorizes a controlled test-02 per-app talk grant from
org.chromium.Chromium to org.sylin.ghostlight. It is app-wide, not extension-specific.
No general host execution, wildcard bus access or broader filesystem grant follows.
Customer setup must disclose and explicitly select this permission; automatic
detection or aggregate setup cannot silently grant it. Preserve prior permissions.

The orchestrator installer owns activation and native registration. Every check,
update and removal verifies exact installation ownership and preserves foreign,
malformed and ambiguous files. Cross-tree routing or an explicit runtime override
must never activate a different selected installation as a fallback. Package layouts
not visible in the sandbox remain unsupported unless independently implemented and
proved without expanding these grants.

## Delivery and acceptance

The Alpine lane owns generic failed-start suppression. Keep the Flatpak request
behind that same admission gate and report requested activation truthfully, without
inventing a spawned PID. Coordinate the small lifecycle hook before overlapping work.

Prove authority absence before browser-led launch, one host authority, completed
desktop startup, actual installed adapter open/read, authority and native-relay
recovery, bounded failure and deploy-lock behavior, and ownership-safe lifecycle
preserving user settings. Preserve-tabs stays unchanged; unavailable permitted
cleanup means a visible retained fixture, not a bypass through another tool.

Static Flatpak permissions may apply only to new sandbox instances. Verify this
before claiming first-time setup works in an already-running browser; do not close
the person's browser or fabricate a hot permission update. If this contradicts an
installation promise, record the exact limitation for coordinator review.

This decision does not advertise Flatpak as supported. Update active support claims
only after cold activation, recovery and ownership evidence pass on the installed
graph. Native platforms retain their existing launch path and authenticated Open.
