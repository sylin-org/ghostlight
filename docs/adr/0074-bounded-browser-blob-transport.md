# ADR-0074: Bounded browser blob transport

Date: 2026-07-13
Status: Accepted
Builds on: ADR-0045, ADR-0053, ADR-0058, ADR-0061, ADR-0062

## Context

Chrome native messaging is asymmetric: a message from the native host to the extension is limited
to 1 MiB, while a message from the extension to the host may be as large as 64 MiB. The shared
framing helper currently accepts 128 MiB in either direction. That is a useful corruption guard for
Ghostlight's internal framed IPC, but it is not a valid Chrome-boundary contract.

A seven-frame GIF produced about 1.98 MiB of base64 before its JSON envelope. Sending it through
`upload_image_exec` disconnected the native host within milliseconds. Four frames already exceeded
Chrome's host-to-extension limit. The same root defect affects sufficiently large `file_upload` and
`upload_image` calls.

Changing the GIF quantizer cannot fix this boundary. Large browser-bound values need one generic,
bounded transport.

## Decision

### 1. Directional limits are explicit

`transport::host::MAX_MESSAGE_LEN` remains the generic framed-IPC corruption ceiling. The browser
outbound adapter separately enforces Chrome's 1 MiB host-to-extension ceiling before a frame is
queued. No serialized request larger than that ceiling is ever written toward Chrome as one frame.

### 2. The extension advertises transport features

The existing `browser_hello` identity frame gains an optional feature list. The initial feature is
`chunkedHostMessagesV1`. `BrowserSession` retains the negotiated feature set for that browser slot.
Unknown features are ignored.

Small requests keep their existing byte shape. If an oversized request targets an extension that
does not advertise chunking, the service fails before sending any part with an update-required
error. There is no optimistic oversized write and no disconnect-as-version-probe.

### 3. Oversized requests use bounded chunks

The service serializes the ordinary request once, assigns a transfer id, computes total bytes and a
SHA-256 digest, and emits ordered `wire_chunk` messages whose own serialized sizes stay below the
Chrome ceiling. Each chunk carries:

```json
{
  "type": "wire_chunk",
  "transferId": "...",
  "requestId": "42",
  "index": 0,
  "count": 4,
  "totalBytes": 1800000,
  "sha256": "...",
  "data": "<base64 chunk>"
}
```

The extension keeps a small, memory-only reassembly table bounded by concurrent transfers, total
bytes, chunk count, and a short expiry. Duplicate, out-of-range, inconsistent, expired, oversized,
or hash-mismatched transfers are rejected and erased. Once complete, the extension parses the
reassembled request and hands it to the existing dispatcher exactly once.

The relay remains an opaque byte pipe. It does not parse chunks, retain blobs, or gain a second
protocol.

### 4. Cancellation and disconnect erase partial transfers

Every transfer is tied to its original request id and the live native port. Transfer expiry,
native-port disconnect, extension-worker teardown, or replacement of the browser session erases
partial chunks. Chunk buffers are never written to `chrome.storage`, IndexedDB, files, logs, or
telemetry.

An outgoing browser effect is not automatically replayed after an uncertain disconnect. The
service may retry transport only when it can prove the extension never completed reassembly and
dispatch. Otherwise the result is `outcome_unknown`.

### 5. The mechanism is generic

The selector lives below tool orchestration in the browser outbound adapter. It covers every
host-to-extension request, including GIF ref/coordinate delivery, `file_upload`, `upload_image`, and
future binary-bearing tools. No tool grows its own chunk protocol.

## Consequences

- GIF delivery no longer crashes the extension connection at four or more ordinary frames.
- Small tool calls remain unchanged and pay only a size comparison.
- Old-extension skew produces a fast corrective error rather than a 60-second timeout.
- Reassembly consumes bounded transient memory in the thin extension but introduces no policy or
  persistent state.
- The 128 MiB generic framing cap no longer falsely documents itself as the Chrome boundary.

## Rejected alternatives

- Reduce GIF quality until every output fits 1 MiB. Rejected because it is not a reliable bound and
  leaves file tools broken.
- Raise the existing framing ceiling. Rejected because Chrome enforces the smaller boundary.
- Put the GIF encoder or durable blob store in the extension. Rejected by the thin-extension rule
  and the ephemeral-content requirement.
- Add per-tool chunking. Rejected because the transport defect is shared and would create multiple
  incompatible protocols.
