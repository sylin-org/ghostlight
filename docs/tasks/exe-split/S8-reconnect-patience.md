# S8: Reconnect patience (survive a rebuild-length service gap)

Goal: a reconnect episode (an established session whose service dropped) retries for 120s instead
of the first-connect 3s window, so the dev loop (Ctrl-C -> cargo build -> rerun, up to two
minutes) and prod restarts never force a client reload. First-connect behavior is UNCHANGED.

## STOP preconditions

- S7 not logged complete -> STOP.
- `crates/transport/src/ipc.rs` does not contain `connect_and_handshake` -> STOP.

## Required changes

1. `crates/transport/src/ipc.rs` per SPEC section 8:
   - Add the two pinned pub consts (`RECONNECT_RETRY_WINDOW` 120s, `RECONNECT_RETRY_INTERVAL`
     500ms) with doc comments citing ADR-0045's amendment.
   - `connect_and_handshake(adapter_endpoint: &str, reconnect: bool)`: first-connect path
     byte-identical to today; reconnect path = one `start_service()` at episode start, then retry
     `try_connect_once` every RECONNECT_RETRY_INTERVAL until RECONNECT_RETRY_WINDOW elapses, then
     log `SELF_HEAL_FAILURE_MESSAGE` and return the last error.
   - `relay_adapter`'s loop passes `!first` as `reconnect`.
2. `docs/adr/0045-resilient-reconnecting-adapter.md` (sanctioned exception): append at the end:

```
## Amendment (2026-07-08, pre-implementation of the split batch)

Reconnect patience is asymmetric by design: the FIRST connect keeps the fail-fast 3s self-heal
window (a misconfigured install should error quickly), while a RECONNECT episode (an established
session whose service dropped) retries every 500ms for up to 120s, asking the OS supervisor to
start the service once at the episode's start. This covers a rebuild-length gap in development
and a crash/upgrade in production; if the window elapses, the adapter exits and the client
reload path is the fallback, exactly the pre-0045 behavior.
```

## Tests (pinned)

- `tests/adapter_reconnect.rs` NEW test `adapter_survives_a_five_second_service_gap` per SPEC
  section 8 (5s sleep between kill and respawn; 30s recv timeout on the post-restart reply; all
  other structure identical to the existing restart test -- factor a shared helper if trivial,
  otherwise duplicate).
- The existing `adapter_reconnects_across_a_service_restart_without_a_client_reload` stays green
  unmodified.

## Verify (literal)

SPEC section 12. Plus rebuild the bin under test, then run the reconnect file 3x (the reconnect
logic executes inside ghostlight-adapter-agent, which `cargo test --test ...` alone does NOT
rebuild):
`cargo build -p ghostlight-adapter-agent && for i in 1 2 3; do cargo test --test adapter_reconnect 2>&1 | grep "test result"; done`
-- all three runs 0 failed.

## Out of scope

Line-aware in-flight error synthesis (ADR-0045 fast-follow; NOT this batch). Any change to
first-connect timing or `SELF_HEAL_*` constants.

## Commit

`feat(adapter-agent): patient reconnect window (120s) so rebuilds and restarts never force a client reload (S8)`
