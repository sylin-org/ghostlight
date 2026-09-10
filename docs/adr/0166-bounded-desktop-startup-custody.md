# ADR-0166: Bounded desktop startup custody

- Status: Accepted
- Date: 2026-09-09
- Amends: ADR-0096 local admission, ADR-0127 desktop startup, ADR-0145 diagnostics,
  ADR-0150 elected startup scope
- Preserves: ADR-0119 disposable workbench, ADR-0160 bounded recovery, ADR-0164 human browsing

## Evidence and ownership

On native musl, four real MCP connector processes with no desktop environment
created 32 transient authorities in four seconds. The service published discovery
before GTK initialization, then each failed desktop waited up to 15 seconds for
activation. A lifetime-lease probe released before spawn did not serialize those
attempts. The caller also discarded the child handle without reaping it.

The owner delegated this lifecycle repair and native acceptance to test-03. The
existing Codex forwarding repair remains the way that client supplies live desktop
context. This decision does not discover or copy environment from parent processes,
capture session addresses, authorize host execution, or introduce a service-only mode.

## Decision

### One bounded launch exchange per elected runtime

Keep the authority's existing lifetime lease. Add a sibling `.startup` admission
file derived from the same runtime path; its OS lock serializes the spawn-to-ready
exchange across callers. Its only retained value is a failed-start timestamp.
No endpoint, token, environment or page data is stored there.

One existing reconnect worker performs a bounded exchange. Native startup spawns
the exact elected sibling with no arguments and null standard streams. It waits
up to 30 seconds for fresh discovery under a held authority lease. Failed children
are reaped before admission is released; on timeout only this caller's own unready
child is stopped. A successful child's waiter only reaps eventual exit and makes
no restart decisions. No new resident supervisor or process is introduced.

After failure, a shared five-second cooldown prevents callers taking turns in an
immediate retry loop. Expiry permits another attempt automatically. Missing,
malformed or future timestamps cannot disable recovery indefinitely. A deployment
marker suppresses a new exchange and interrupts a pending native exchange.
The existing connectors continue reconnecting; neither waiting nor cooldown is
reported as desktop readiness, and no uncertain browser work is replayed.

### Native spawn and named OS activation share admission

`request_orchestrator_start` owns native sibling execution.
`request_orchestrator_activation` accepts one bounded, narrowly authorized OS
activation callback and then uses the same discovery wait and cooldown. Activation
has no child handle, invents no PID and cannot kill the externally activated
authority. Its caller owns platform-specific consent and named activation details.
It never falls back to broad host execution. ADR-0165 owns the Flatpak use of that
platform boundary; this decision adds no Flatpak permission or registration.

The closed dispositions distinguish a spawned child, completed activation, held
authority lease, startup in progress, retry cooldown and deployment quiescence.
Their content-free diagnostics are rendered once in the shared lifecycle module.
No MCP, service, browser-relay or extension protocol revision is required.

### Discovery means the desktop interaction route is ready

Service construction obtains the lifetime lease and prepares private authenticated
listeners. It does not publish runtime discovery. The desktop Ready handler first
finishes initial workbench construction/backgrounding and attaches presentation,
then publishes discovery and logs readiness. A usable tray permits the existing
recoverable workbench fallback; absence of both routes still exits.

Lease contention has a typed error and may wait for the winning authority's
activation. Failure of this process's own GTK/Tauri startup is different: it logs
the failed stage, tears down the prepared service and exits promptly. It does not
enter the activation timeout. A failed native startup never leaves a headless
authority or advertises a usable runtime before its desktop is ready.

## Scope of proof

The [native Linux record](../testing/linux-startup-recovery-2026-09-09.md) owns
before/after source and binary identities, real concurrent process observations,
corrected-context recovery, installed Codex/browser/workbench evidence and gate
results. Native musl is a feasibility lane, not a new public support promise.
OS activation correctness and permission remain the platform lane's responsibility.
These bounds do not claim host-level denial-of-service containment or recovery
from an operating system that cannot schedule or terminate a process.
