# ADR-0063: Deploy-quiesce lock (suppress service self-heal during a binary swap)

- Status: Accepted
- Date: 2026-07-12
- Relates: ADR-0054 (Windows detached self-heal), ADR-0030 D8 (always-ready service), ADR-0062 (persistent browser relay)

## Context

Rebuilding or upgrading the Ghostlight binaries in place fights two independent respawners:

1. **The service self-heals.** When the `ghostlight` service dies, an adapter's failed dial calls
   `supervisor::start_service()`, which (on Windows) spawns the sibling `ghostlight.exe` detached
   (ADR-0054). During a rebuild this races the compiler: kill the service to free its `.exe`, and the
   self-heal relaunches the OLD image milliseconds later, re-locking the file mid-write. Observed
   live: killing the default service, it reappeared under a new pid within seconds.
2. **The extension respawns the relay** every ~2s via `connectNative`, re-locking `ghostlight-relay.exe`.
   ADR-0062 sharpened this: the relay is now persistent (it reconnects instead of exiting), so it holds
   its `.exe` locked until something explicitly kills it.

The service side (1) has no clean "hold off" signal today. The relay side (2) cannot be signalled by a
lock file at all -- a Chrome service worker has no filesystem access.

## Decision

**A deploy-quiesce lock: a `deploy.lock` file next to the service executable.** While
`<service-exe-dir>/deploy.lock` exists (and is fresh), `supervisor::start_service()` refuses to
self-heal that binary. A deploy creates the lock, kills + replaces the service, then removes the lock;
the self-heal never races the swap.

Two deliberate design points:

- **Scoped to the exe's directory, not the instance.** The lock lives beside `ghostlight.exe`, and
  `start_service` resolves the sibling exe it is about to spawn and checks for `deploy.lock` there.
  This is what correctly covers the ADR-0048 unpinned adapter: its own identity is `default`, but its
  sibling exe is whatever build directory it ships in (e.g. the dev `target/release` being rebuilt),
  so it consults the right lock regardless of instance identity. A default install and a dev build in
  different directories have independent locks.
- **Stale locks self-expire.** A lock older than `DEPLOY_LOCK_MAX_AGE` (30 min, far longer than any
  real deploy) is ignored, so a crashed deploy never permanently disables self-heal.

The **relay** side is handled without any protocol or product change, using the standard Windows
technique: a deploy renames the running `ghostlight-relay.exe` out of the way (Windows permits
renaming a running image even though it forbids overwrite) before building. The old process keeps its
renamed file; the fresh `relay.exe` is written to the canonical path; extension respawns during the
build simply find no exe and retry harmlessly until the new one lands. `dev-loop.ps1` does exactly
this, paired with the `deploy.lock` for the service.

## Consequences

- A one-command dev redeploy no longer fights the self-heal or the relay respawn; and the same
  `deploy.lock` gives a production installer a clean quiesce point for an in-place upgrade (drop the
  lock, stop the service, swap the binary, remove the lock -- no self-heal race).
- The lock is a Windows-self-heal quiesce. On Unix the service is run by the OS supervisor
  (launchd/systemd); a deploy there stops the unit (which suppresses restart) rather than relying on
  this file. The lock check is a no-op cost on Unix.
- The lock only governs the SERVICE self-heal. The relay respawn is governed by the rename technique
  in the deploy script, not by this file (the extension cannot read it).

## Implementation

`crates/transport/src/supervisor.rs`: a private `deploy_lock_present(service_exe)` (exists + fresh
check on `<dir>/deploy.lock`) guards the Windows branch of `start_service` before it spawns; a
`DEPLOY_LOCK_MAX_AGE` constant bounds staleness; a unit test covers present / absent / stale.
`scripts/dev-loop.ps1`: create the lock, rename any running `relay.exe` aside, kill this repo's dev
processes, build, remove the lock, start the service, best-effort clean the `*.old` relay files.
