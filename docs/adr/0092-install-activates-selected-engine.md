# ADR-0092: Install activates the selected engine

- Status: Accepted
- Date: 2026-08-01
- Amends: ADR-0054 Decision 2 and the Linux supervisor registration from ADR-0030 H9
- Builds on: ADR-0063 (deploy-quiesce lock), ADR-0065 (the endpoint owner is the engine)

## Context

Installing a newer packaged release updated the native host, MCP client entries, and Windows Run
value, then spawned the new service. If the previous always-ready service still owned the singleton
endpoint, the new process lost that race and exited. The install reported success while the old
engine kept serving indefinitely.

This happened during the v0.7.1 stale-workspace verification. The v0.7.1 binaries and Zed entry were
present, but the v0.7.0 service still owned the endpoint. The released recovery code was correct and
could not run until the service was replaced by hand.

ADR-0065 makes endpoint ownership the engine selection. An install is therefore incomplete when it
registers one binary but leaves another installed binary holding that endpoint.

## Decision

### 1. Registration activates the selected installed engine

After updating the per-user supervisor, `ghostlight install` makes the selected binary active when
the endpoint is absent or owned by an older managed installation. Activation remains best-effort,
like the existing supervisor registration, but it reports the verified outcome instead of treating a
successful spawn as sufficient.

### 2. Windows replacement uses exact OS ownership and image proof

The installer opens the adapter/control named pipe and asks Windows for the server process ID. It
then reads that process's executable path.

The installer may stop the process only when all of these are true:

- it still owns the exact resolved endpoint;
- its process generation is still alive;
- its executable is beneath the per-user Ghostlight install root;
- it is not already the selected executable.

An unverified owner is never stopped. A process outside the managed install root is treated as an
external engine and left in place. This preserves ADR-0065's repository/dev engine-swap workflow and
avoids taking ownership of another package manager's lifecycle.

### 3. Persistent relays are quiesced during replacement

Before inspection and replacement, the installer creates `deploy.lock` with exclusive create
semantics in every installed engine directory beneath the managed root. Every relay from the
current lock-aware one-stack line therefore holds off instead of self-healing an old service between
termination and startup. Releases older than ADR-0063 predate this upgrade contract and remain
outside the current greenfield compatibility boundary.

Only locks created by this activation attempt are removed. A pre-existing lock aborts activation and
is never overwritten or removed. Locks are released on every exit path.

After stopping the verified predecessor, the installer starts the selected service detached and
waits for the adapter endpoint to report that exact executable as its owner. A different owner or a
startup timeout is a warning, not a false success.

### 4. Unix supervisors restart after registration

macOS already uses `launchctl kickstart -k`, which replaces an active predecessor. Linux now runs
`systemctl --user restart` after writing, reloading, and enabling the user unit. Unix package
replacement remains the supervisor's responsibility; the Windows process-owner path is not copied
there.

## Consequences

- A packaged upgrade takes effect during the install that selected it, rather than at the next
  login or unrelated service restart.
- Agent and browser relays ride through the brief service replacement using ADR-0045 and ADR-0062
  reconnect behavior.
- A browser call active at the moment of an explicit upgrade can be interrupted. Its normal
  unknown-outcome rules still apply.
- Repository/dev engines and other external service owners are preserved. The newly registered
  installed service becomes the next login/self-heal choice without silently displacing them.
- Windows gains two narrow read-only primitives: named-pipe server PID lookup and process image-path
  lookup. Neither expands Ghostlight's network or browser authority.

## Amendment (2026-08-25)

This record stands as written; the text above is the decision as it was made. The 1.0 candidate
narrows the supported operating-system matrix to Windows and Linux
([1.0/ACCEPTANCE.md](../1.0/ACCEPTANCE.md)). macOS mentions above describe the scope considered at
decision time; macOS remains a later row of the platform table, deferred for want of test hardware,
not removed from the product's future.
