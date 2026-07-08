# 0045. The resilient reconnecting adapter

- Status: Accepted (2026-07-08)

## Relationship to other decisions

- AMENDS ADR-0030 Decision 1 and Decision 8: the thin ADAPTER graduates from "a pure byte relay
  that dies when the service stream closes" to "a resilient conduit that reconnects to a restarted
  service and replays the MCP handshake, so the MCP client rides through transparently." The raw
  data-phase relay and the ARGV-decided role are preserved; only the lifecycle on a service-side
  drop changes.
- ENABLES ADR-0044's launch-path split: because the adapter is a stateless conduit whose version
  does not matter, the ONLY binary whose version matters is the service, which is launched fresh
  via `--instance`. This is what makes "rebuild plus restart the service" live with no client
  reload.

## Context

The bare `ghostlight` invocation is the thin ADAPTER (ADR-0030): it dials the running service's
adapter/control endpoint and does a raw bidirectional relay between the MCP client's stdio and the
service. It holds no state and runs no governance -- all of that lives in the service.

Today, when the service stream closes, the adapter's relay `select!` completes, `relay_adapter`
returns, and the process exits. The MCP client sees its stdio server vanish and must be reloaded
(or restarted) to respawn the adapter, which then reconnects to the new service. That reload is
the friction: during development you rebuild the binary and restart the service constantly, and
each restart forces a client reload even though the adapter itself is a version-irrelevant pipe.
The same friction appears in production as a service crash, an upgrade, or an idle-grace restart:
every connected client is disrupted.

The insight (2026-07-08): the adapter is already the dumb forwarder we want. The only reason a
restart disrupts the client is that the adapter dies WITH the service instead of reconnecting to
its successor.

## Decision

### Decision 1: the adapter reconnects on a service-side drop, and exits only on a client-side close

The adapter distinguishes WHO closed by which side of the raw relay ended, with no heartbeat:

- The client -> service copy ending with a clean EOF means the MCP client closed its stdin -> the
  adapter EXITS (unchanged). The parent-death watchdog (ADR-0029, re-scoped by ADR-0030 D8) remains
  the second, reliable exit trigger for an unclean client kill.
- The service -> client copy ending (EOF, or a broken-pipe on the write to the service) means the
  SERVICE dropped -> the adapter RECONNECTS: re-dial with the existing self-heal (ask the OS
  supervisor to start the service if it is down, PINS.md SS5.2), then resume relaying.

The reconnect (and the first connect) retries the WHOLE connect+handshake, not just the dial,
within that same bounded self-heal window. A restarting or cold-starting service may have already
CLAIMED its endpoint (so the dial succeeds) while it is momentarily not yet serving, or has not yet
written its per-install anti-squat key -- a transient handshake failure (a torn-down connection,
Windows os error 232, or a not-yet-verifiable proof). Retrying the whole handshake makes that
window survivable instead of a fatal adapter exit, which is what makes both a cold-start (self-heal)
connection and a reconnect actually resilient. Security is preserved: a genuine squatter never
yields a valid proof, so it simply keeps failing until the window elapses and the adapter exits.

### Decision 2: the adapter replays the MCP handshake on every reconnect

A reconnected service is a fresh process that never saw this session's `initialize`. So the adapter
runs in two phases:

- **Preamble phase:** as it forwards the opening handshake it CAPTURES the client's `initialize`
  request and its `notifications/initialized` notification (the two client-originated handshake
  messages). This is the only place the adapter is message-aware.
- **Relay phase:** a raw bidirectional stream, exactly as today.
- **On reconnect:** send the session hello and verify the service proof (unchanged), then REPLAY
  the captured `initialize` and `initialized` to the new service, READ AND DISCARD the new
  service's duplicate `initialize` result (the client already has one), and resume the raw relay.

Session state on the service side (the `SessionGuid`, owned tabs, in-flight governance) is NOT
persisted across a restart; a reconnect yields a fresh session and the client re-drives. Persisting
session state to survive restarts is explicitly rejected as over-engineering for the win it buys.

### Decision 3: in-flight requests at the drop time out (baseline); a clean error is the fast-follow

A request that was in flight at the instant the service dropped gets no response from the dead
service. The BASELINE keeps the relay phase a raw stream (so every large reply -- notably
screenshots -- streams without buffering or per-message parsing) and lets such a request time out;
the agent naturally retries. Because a developer restarts the service BETWEEN agent actions, a
request in flight at the exact drop instant is rare.

A FAST-FOLLOW may track outstanding request ids and synthesize a JSON-RPC "the Ghostlight service
restarted; please retry" error on a drop, so the client fails fast instead of waiting for a
timeout. That keeps the relay line-aware, so it is deferred rather than baked into the first cut.
Auto-re-driving an in-flight request against the new service is REJECTED: replaying a
non-idempotent action (`left_click`, `form_fill`) could double-execute it.

### Decision 4: the browser conduit stays Tier B (extension reconnect)

The native host remains a stateless dumb pipe that dies on a service drop. The extension's existing
service-worker recovery re-spawns it against the new service on the next action, so the browser
side self-heals without new server code. Promoting the native host to the same Tier-A transparent
reconnect is a possible future enhancement, deliberately NOT taken now: it would require handling
the server-speaks-first re-hello on the extension protocol, and the proven extension recovery
already covers the gap.

## Consequences

### If taken

- The dev loop becomes: run `ghostlight --instance dev service --keep-warm --debug` in a terminal,
  edit Rust, `cargo build`, Ctrl-C and rerun the service. The MCP client never reloads.
- Production upgrades and crashes become transparent: swap and restart the service, and every
  connected MCP client rides through. Resilient reconnect is therefore the DEFAULT adapter
  behavior, not a dev-only toggle.
- The `--keep-warm` flag on `service` disables the idle-grace shutdown for an interactively run
  (dev) service, so a terminal-run service stays up between actions; the supervisor-launched
  production service keeps idle-grace.

### Cost

- The adapter gains a reconnect loop and a small, bounded amount of message-awareness (capturing
  and replaying the handshake preamble). The data phase stays a raw stream.
- A reconnect resets service-side session state; a mid-restart in-flight call times out until the
  fast-follow lands.

### Risks

- **Handshake-replay drift.** If the captured preamble does not satisfy a future service's
  `initialize` expectations, the replayed session could misbehave. Mitigation: capture the exact
  client bytes and replay them verbatim; the service sees precisely what it saw the first time.
- **Reconnect storm.** A service that crash-loops would make the adapter re-dial repeatedly.
  Mitigation: the existing bounded self-heal retry window governs each re-dial, and a client-side
  close still exits promptly.

## Amendment (2026-07-08, pre-implementation of the split batch)

Reconnect patience is asymmetric by design: the FIRST connect keeps the fail-fast 3s self-heal
window (a misconfigured install should error quickly), while a RECONNECT episode (an established
session whose service dropped) retries every 500ms for up to 120s, asking the OS supervisor to
start the service once at the episode's start. This covers a rebuild-length gap in development
and a crash/upgrade in production; if the window elapses, the adapter exits and the client
reload path is the fallback, exactly the pre-0045 behavior.

## Amendment (2026-07-08, ADR-0047 D6): down-relay error classification

The service->client relay direction now classifies a service-side READ error the same as a
service-side EOF: both are ServiceClosed, which reconnects. Only a failed write toward the client
is ClientClosed, which exits. The original arm used `tokio::io::copy` and mapped its single
`Err` into ClientClosed, so on Windows -- where an abrupt service death typically surfaces as
ERROR_BROKEN_PIPE on the READ, not the write -- the adapter exited and forced the very MCP-client
reload this ADR exists to prevent. A hand-rolled copy loop separates the two failure sides. See
ADR-0047 D6.

## Amendment (2026-07-08, ADR-0047 D2): stable session identity across reconnects

This ADR originally minted a fresh `SessionGuid` on every (re)connect, on the reasoning that a
reconnect is a brand-new session. ADR-0047 D2 supersedes that: the adapter mints ONE guid for its
whole process and re-presents the SAME guid on every reconnect. The service's `SessionRegistry`
already sanctions a user re-presenting an identity, so nothing server-side changes. The reason is
downstream of resilience: a fresh guid per reconnect orphaned that session's tab ownership and its
Chrome tab group every time the service blinked, which is exactly the transparency this ADR set out
to deliver. Identity is now stable for the life of the adapter process. See ADR-0047 D2.
