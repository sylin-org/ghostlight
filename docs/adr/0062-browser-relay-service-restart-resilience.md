# ADR-0062: Browser-relay service-restart resilience (reconnect + identity replay)

- Status: Accepted
- Date: 2026-07-12
- Amends: ADR-0045 (adapter reconnect + handshake replay), ADR-0058/0061 (browser identity)

## Context

The MCP-side ADAPTER relay is resilient to a service restart (ADR-0045): when the service drops, the
adapter does not exit -- it reconnects to the restarted service, replays the captured MCP handshake,
and resumes, so a service upgrade/crash/rebuild is invisible to the editor. The BROWSER-side relay
never got the same treatment. It is one-shot: it dials the service once, pumps frames, and exits the
moment either side closes. Reconnection depends entirely on the extension noticing its native port
died and respawning the relay (`onDisconnect` -> retry, keepalive alarm).

Live testing ADR-0061 surfaced how weak that is. Restarting the dev service left the extension
attached to the wrong instance until a manual reload: the unpinned relay resolves its instance at
connect time (preferring a live dev instance, ADR-0048), so when dev was briefly down it fell back to
the live default service and never returned to dev, because its connection to default never dropped.
Even in the single-instance production case, a service restart forces native-port churn and relies on
the extension's respawn timing rather than a relay that simply rides the gap.

The asymmetry is the real defect: the adapter treats a service drop as "reconnect," the browser relay
treats it as "die." There is no reason the browser side should be the fragile one.

## Decision

**Give the browser relay the same service-restart resilience the adapter has.** On a service drop the
browser relay reconnects (re-resolving the endpoint, so it prefers a live dev instance again) and
resumes, keeping the extension's native port alive the whole time. Only the EXTENSION side closing
(the browser/Chrome going away) exits the relay.

Two mechanics carry it, both mirrored from ADR-0045:

1. **A long-lived Chrome-frame reader feeding a channel.** The reader task owns Chrome's stdin and is
   never inside a `select!`, so a frame is never cancelled mid-read, and frames buffer in the channel
   across a brief reconnect instead of being lost. The reader ending (Chrome's stdin EOF) is the ONE
   signal that the browser is gone -> the relay exits.
2. **Identity replay (the ADR-0061 tie-in).** The extension announces its persistent `browserId` as
   the opening frame ONCE per native-port connection. Because the relay now keeps that port alive
   across a service reconnect, the extension will NOT re-send it. So the relay caches its opening
   frame (opaquely -- it still never parses the extension's frames) and replays it, right after its
   own `ROLE_BROWSER` hello, to every freshly reconnected service. The service reads hello + identity
   exactly as on a first connect and re-admits the session under the same UUID (hence the same slot,
   ADR-0061), so composite tab ids minted before the restart still route.

Close classification is the crux, identical to ADR-0047's fix for the adapter: a service-side read
EOF or error (including a Windows broken pipe) is `ServiceClosed` (reconnect); only the Chrome-frame
channel closing is `ClientClosed` (exit). The first connect stays fail-fast; a reconnect episode is
patient (the existing `RECONNECT_RETRY_WINDOW`), so a rebuild-length gap never gives up prematurely.

## Consequences

- A service restart/upgrade/crash is now invisible to the extension: no native-port churn, no
  reload, no lost tab group. The dev workflow benefits directly -- if the extension is on dev and dev
  restarts, the relay re-resolves and snaps back to dev.
- Not fixed here (out of scope, disclosed): the "extension sits on a live DEFAULT instance while a dev
  instance comes up later" case. That connection never drops, so nothing triggers re-resolution;
  switching to it cleanly is a dev-only concern (pin `GHOSTLIGHT_INSTANCE=dev` during testing) and not
  a production reconnect failure.
- The relay stays a pure byte pipe: it caches and replays its opening frame as opaque bytes and never
  inspects any extension frame. The one added piece of state is a single `Option<Vec<u8>>`.
- A tool response mid-flight to the instant the service dropped is lost and its call times out -- the
  same accepted baseline ADR-0045 Decision 3 already documents for the adapter; the model retries.

## Implementation

`crates/transport/src/ipc.rs`: `relay_native_host_over` becomes a reconnect loop over a long-lived
Chrome-frame reader channel; a new `native_relay_session` classifies `ClientClosed`/`ServiceClosed`
and captures/re-frames the opening identity frame; the service->Chrome direction reuses the adapter's
raw-byte `copy_service_to_client` (which already classifies the two close sides correctly on Windows).
`connect_native_with_retry` mirrors `connect_and_handshake`'s fail-fast-first / patient-reconnect
split, minus the adapter-only anti-squat proof. `run_browser` is unchanged -- the parent-death
watchdog still exits the process when the browser itself is gone.
