# ADR-0113: Browser availability requires an end-to-end acknowledgement

Date: 2026-08-13
Status: Accepted
Builds on: ADR-0003, ADR-0062, ADR-0093, ADR-0096, and ADR-0098

## Context

The service treated an attached browser-relay socket as proof that Chrome was available. That is
not always true. A live browser connector can retain both of its pipes after the extension service
worker stops consuming native messages. The service can then write a physical request into the
relay pipe successfully, wait through the invocation deadline, and report an unknown effect even
though no adapter is answering.

The live root cause was a connection-creation race in the extension. Bootstrap, installation,
startup, and reconnect signals could enter `connectNative` concurrently. Its `nativePort` guard
ran before an awaited local-state initialization, so two callers could both create a native host.
Only the last port remained active in extension state, while both opaque relays could attach to the
service. Service connection replacement and extension port ownership could then select different
pipes. The selected service pipe stayed structurally open but its extension listener deliberately
ignored every frame because that port was no longer current.

The unknown classification is correct once dispatch may have happened. The failed premise is the
earlier availability decision: accepting bytes into a local socket proves relay attachment, not
that the browser endpoint received them.

Operation silence cannot repair that premise. A healthy `browser_wait` may produce no operation
receipt for 30 seconds. Detaching an adapter because one operation is quiet would turn supported
browser work into a false disconnect. The signal must be independent of product work and must
terminate at the extension.

## Decision

### 1. Adapter liveness is an independently negotiated physical mechanism

Adapter protocol 2 gains the additive `adapter_liveness` capability at revision 1 and two typed
frames:

- `heartbeat { sequence }` from the service; and
- `heartbeat_ack { sequence }` from the extension.

The extension answers a valid heartbeat immediately, before browser negotiation or primitive
execution. The frame contains only a bounded sequence number. It carries no workspace, URL, tab,
page content, operation, authority, or presentation fact.

The browser connector remains an opaque bounded relay. It neither recognizes nor originates the
heartbeat.

### 2. Attachment and availability are separate service facts

For an adapter that advertises `adapter_liveness`, the service sends a heartbeat every 20 seconds.
An acknowledgement refreshes availability. Forty-five seconds without one makes the adapter
unavailable even when the relay socket remains attached.

The service keeps the socket and continues the bounded probes. A later valid acknowledgement can
restore availability without killing the connector or creating a reconnect loop. A real EOF or
read error still detaches the connection through the existing structural transport path.

Adapters that do not advertise the additive capability keep the prior attachment semantics. A new
service therefore continues to use every physical capability an older compatible adapter actually
advertises. The source adapter remains at 1.0.0 because it is still unreleased; the adapter
protocol major does not change.

### 3. Every physical dispatch carries its own liveness probe

After writing a physical request, the service writes one correlated heartbeat on the same ordered
stream. If the operation reaches its deadline without an acknowledgement for that post-dispatch
probe, the terminal result remains the truthful unknown effect and the adapter is marked
unavailable. The next invocation then fails before dispatch with the existing actionable browser
disconnected result.

If the heartbeat is acknowledged while the operation remains silent, the adapter stays available.
This is the required distinction for a legitimate long-running wait: browser work and connection
health have independent receipts.

### 4. Unknown effects are never replayed

Heartbeat state changes availability only. It cannot turn a timed-out physical effect into success,
failure, or no effect, and it never authorizes a retry. Existing completion truth and extension-side
duplicate suppression remain unchanged.

### 5. Native-host connection creation is single-flight

The extension owns one in-progress native connection attempt. Every bootstrap, installation,
startup, and reconnect signal joins that attempt instead of opening another native port. Local
state initialization occurs inside the same attempt, and connection ownership is checked again
after its asynchronous boundary before Chrome is asked to start a native host.

This is the root fix for the observed split ownership. Heartbeats remain the independent defense
that proves the single selected pipe still reaches the extension after attachment.

## Consequences

- Periodic end-to-end traffic prevents an otherwise idle extension shore from becoming silently
  unreachable and detects it when it does.
- A request dispatched at the instant the adapter disappears can still be unknown. That is
  unavoidable. Later requests stop before dispatch instead of repeating the same uncertainty.
- A healthy silent browser operation remains connected because the extension can acknowledge
  liveness while the operation is pending.
- Concurrent extension wake signals cannot create competing native hosts for one worker epoch.
- Three small content-free probes per minute are local to the existing native connection. No
  network request, telemetry, storage entry, or audit payload is added.
- Older compatible adapters continue to work without the stronger availability guarantee.

## Rejected alternatives

### Detach on every operation deadline

Rejected because an operation deadline says nothing about adapter liveness. It would disconnect a
healthy adapter during supported long-running waits.

### Use TCP keepalive or a service-to-connector ping

Rejected because either proves only that the relay process is alive. The missing fact is whether
Chrome and the extension consume the adapter stream.

### Teach the browser connector to parse adapter frames

Rejected because the existing end-to-end adapter protocol already reaches the correct endpoint.
Parsing it in the relay would violate the opaque fringe without improving the signal.

### Replay a request after liveness returns

Rejected because a lost action may already have taken effect. Recovery cannot weaken the existing
unknown-effect rule.

## Evidence

- Bridge tests round-trip both liveness frames.
- Browser-port tests keep a relay socket attached without acknowledgements and prove availability
  expires, then prove an unanswered dispatch probe quarantines the adapter at the operation
  deadline.
- A browser-port test withholds an operation receipt beyond the liveness timeout while answering
  heartbeats, then proves the operation succeeds and the adapter stays available.
- Extension tests prove capability advertisement, bounded acknowledgement behavior, and
  single-flight native-host startup.
- The real process journey uses the capability through the unchanged opaque browser connector.
