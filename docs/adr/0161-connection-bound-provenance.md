# ADR-0161: Connection-bound provenance reporting

- Status: Accepted; reporting foundation implemented and verified locally; not deployed
- Date: 2026-09-07
- Amends: ADR-0105's reporting stage
- Builds on: ADR-0106, ADR-0102, ADR-0103, ADR-0156, ADR-0159, ADR-0160

## Context

The C1 review found that workspace-level attribution could change while earlier work was still
running. When two connections shared a caller-owned session, admitting the later connection
replaced the executable identity used to record the earlier connection's work. The workbench also
resolved action labels through current session state, so reconnects could mislabel history.

A fresh-build process fixture then exposed a second attribution bug: the Windows observer matched
the accepted socket's local TCP-table row and identified the service itself. The existing test
connected both endpoints from one process, so either direction produced its expected PID. The
observer must match the inverse address tuple to identify the remote process.

The owner accepted option A: implement the reporting foundation first. It fixes attribution and
makes existing evidence understandable without adding a verifier. Signature and hash verification
remain a later C1 cycle with a concrete subject. This decision does not complete all of C1 or select
C2/C3 admission mechanisms.

## Decision

### Capture evidence once for each connection

The orchestrator captures immutable evidence at service admission. Each connection gets a fresh
service-issued opaque identifier and local observation time. Reconnecting or sharing an existing
workspace does not reuse another connection's evidence. Caller-owned workspace lifetime remains
ADR-0106's contract; a workspace may hold several active connections.

Every prepared invocation retains its originating connection's evidence before it enters a queue.
Direct work, composed children, preparation failures, and refusals carry that same evidence to the
existing completion path. Waiting, another connection's admission, and disconnection cannot
change an admitted action's attribution. Disconnect removes only that connection from the current
session details; completed and still-running actions retain their own evidence.

### Separate application claims from durable observations

The existing service hello's application label remains a claim. It is bounded to 100 characters
with control and direction-changing characters removed. Familiar names remain useful in current
sessions, operations, and the bounded live history projection. The raw claim is not written to
durable audit or process diagnostics.

Durable attribution contains only:

- the service-issued connection identifier and observation time;
- the declared intake channel, which remains attribution rather than authority;
- a bounded observed executable basename or an explicit observation state;
- the explicit signature state `not_checked`.

An observed executable basename is at most 120 characters, normalized and path-free. No PID,
full path, command line, raw operating-system error, caller-supplied label, or certificate dump is
added to durable records. These observations do not make the executable trusted or approved.
Neither the basename nor the claimed channel becomes an admission rule.

The live projection receives the bounded claim separately from the serializable audit record.
After a durable-history reload, that claim is absent. The workbench uses the retained executable
name or existing opaque workspace identity for compact attribution; it never borrows the label
from a currently connected application.

### Report the evidence the platform actually provides

Windows corrects the existing socket-owner observation in `ghostlight-win-peer`: given the
observer's local and peer addresses, the TCP-table lookup matches the peer as the row's local
endpoint and the observer as its remote endpoint. The observed executable belongs to the directly
connected process; an ordinary MCP connection normally identifies Ghostlight's connector. The
upstream application's identity and the instructions it supplies remain unverified. A signed
interpreter would likewise say nothing by itself about the script it runs.

Observation states are closed: `observed`, `unsupported_platform`, `unavailable`, and
`not_recorded`. The human surface explains unsupported observation as "Unavailable on this
platform", a failed supported observation as "Could not identify this connection", and missing
historical evidence as "Not recorded". Linux currently has no socket-peer observer in this seam
and reports the unsupported state explicitly.

Signature status is always "Not checked" in this cycle. There is no hash calculation, signature
verifier, verification cache, certificate approval, identity admission, or new policy setting.
Fresh observation on every connection replaces any need for a provenance cache in this scope.

Older receipts remain readable and retain their declared channel. A legacy `peer_image` field
cannot establish the remote executable because the prior observer selected the service's row.
Human projections therefore show its executable observation as "Not recorded"; they do not
promote that old basename to remote-peer evidence. Original historical files remain
untouched. Missing connection identifiers and observation times are not reconstructed as facts.

### Keep ordinary work quiet and details accessible

Keep the familiar client chips. A collapsed Connection details section exposes all active
connections in the existing session surface. Each action and recorded composition child has its
own collapsed details beside the existing permission and coverage details. Open details and
keyboard focus survive projection updates.

Details distinguish "Reported application", "Observed executable", and "Signature". The
orchestrator authors the observation explanation and connector/upstream distinction. No trusted
badge, routine popup, or all-open admission consequence follows from unchecked or unavailable
evidence. The UI escapes claims and renders evidence; it does not infer identity or policy.

### Preserve existing process boundaries

The orchestrator owns evidence capture, immutable invocation attribution, safe audit projection,
and human explanations. The existing service bridge already supplies the claimed label, intake,
and local socket; its contract is unchanged. The MCP connector only stops copying raw labels into
durable diagnostics. No browser connector or extension feature is required.

## Acceptance and remaining work

Regression evidence must show two connections sharing one workspace without rewriting each
other's direct, queued, composed, failed-preparation, or refused receipts. History must retain
observed evidence after disconnect and restart, omit raw claim sentinels from durable files, and
state unavailable observations accurately. Windows tests must use distinct processes so a tuple
direction error cannot pass by observing the same PID on both ends. The real bundled UI must
preserve collapsed/open behavior, escaped labels, focus, and narrow layout.

The [C1 task record](../tasks/security-hardening/c1-reporting-foundation.md) owns validation and
implementation status. Signature/hash verification remains deferred to a further C1 cycle.
C2/C3 still require their concrete subject and connection proof under ADR-0105. This reporting
foundation adds no host containment, endpoint-wide audit, or upstream application-authentication
claim. Source completion, local deployment, and publication remain separate states.
