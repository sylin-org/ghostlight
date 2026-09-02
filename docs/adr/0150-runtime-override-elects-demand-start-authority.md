# ADR-0150: The runtime override elects the demand-start authority

- Status: Accepted
- Date: 2026-09-02
- Amends: ADR-0124 Decision 1 (an explicit `GHOSTLIGHT_RUNTIME_FILE` is now authoritative for
  the demand-start identity as well as the endpoint document)
- Builds on: ADR-0124, ADR-0127, ADR-0149, ADR-0146

## Context

Each installation directory elects exactly one authority: the runtime document lives beside the
executable, and N installed trees legitimately mean N single authorities. That election is what
contained the 2026-08-30 scratch-tree registration hijack (the ADR-0149 amendment), and it stays.

The shape it fails is two installations serving one person at once. Observed live on this machine
on 2026-09-02: the browser's native-host registration named the development tree, while Cline's
`npx -y ghostlight` launcher entry resolved to the installed release. The installed connector
found no endpoint beside itself, demand-started its own sibling by path, and the machine ran two
authorities: one serving the browser and its harnesses, and one serving only Cline with no
browser and no way to reach one, because the browser registration is `owned_elsewhere` and is
never adopted.

Discovery is already overridable per process through `GHOSTLIGHT_RUNTIME_FILE` (ADR-0124
Decision 1). Demand-start identity is not: the lifecycle seam spawns the sibling of the current
executable regardless of the override. The override therefore cannot express the one thing a
development machine needs -- "this machine's authority lives in that directory" -- without
splitting in half. A connector pointed at a foreign runtime document connects while that
authority is up, and spawns the wrong binary into the elected slot when it is down.

Routes that were considered and not taken:

- The retired ADR-0048 auto-shadow (unpinned clients prefer a live dev instance) was superseded
  deliberately by ADR-0064 and collapsed into the one-stack engine swap of ADR-0065. Rebuilding
  it would re-litigate two superseding decisions.
- Harness setup (ADR-0146) writes exact sibling connector paths and is the no-code answer for
  registered harnesses, but the launcher channel is deliberately floating, and setup preserves
  foreign entries by design (ADR-0135): it cannot claim an `npx -y ghostlight` entry.
- Per-machine convergence (one runtime slot for all trees, first authority up wins) is the
  deeper unification and remains rejected for now: it changes Windows' portable beside-exe
  property and makes the serving version depend on start order for every multi-tree user.

## Decision

1. **Discovery and demand-start resolve as one unit.** When `GHOSTLIGHT_RUNTIME_FILE` is set,
   its directory is the elected authority directory: connectors read that document, take the
   service lease beside it, honor that directory's `deploy.lock`, and demand-start that
   directory's `ghostlight` executable. Without the override nothing changes: per-installation
   election beside the executable remains the default (ADR-0124 Decisions 2 and 3 untouched).
2. **Fail loud, never fall back.** If the elected directory holds no `ghostlight` executable,
   demand-start fails with the existing missing-sibling error and the connector keeps retrying
   with its one startup diagnostics line. Falling back to the connector's own sibling would
   publish a foreign binary into the elected slot -- the wrong-authority shape this decision
   exists to prevent.
3. **Routing is not installation.** The override is a machine posture set by a person or the
   dev loop, in a harness's server environment or the user environment. It writes no
   native-host registration and adopts none; `owned_elsewhere` and the ADR-0149 amendment stand
   untouched. The browser's routing stays the registration; the harnesses' routing is this
   override or their exact registered connector path.
4. **Journeys elect the build under test.** The journeys already set the override; they now
   point it inside the build directory under test, so the elected directory is the directory of
   the binaries under test and their `deploy.lock` quiesce stays meaningful.
5. **Surfaces stay deferred.** The missing-sibling error and the runtime document already name
   the elected directory. A dedicated doctor row or workbench indicator for the elected
   authority is a later surface, not part of this decision.

## Consequences

- A harness connector on a development machine converges on the development authority whenever
  the override names it, and demand-starts that authority (never its own sibling) when it is
  down. The two-workbench split closes for floating launcher channels.
- The launcher stage gains the corrected demand-start on its next release. A current-release
  connector with the override set already converges while the elected authority is up, because
  discovery was always overridable; the correction matters when it is down.
- Binaries hand-staged into the launcher's versioned cache path are reverted by checksum
  verification on every launch. The launcher stage is updated only by a release; staging for
  tests belongs in a directory the launcher does not checksum.
- No new variable, file, marker, service, or registry. One resolution unit in the bridge covers
  both connectors and the CLI demand-start paths, because they all cross the same seam.
