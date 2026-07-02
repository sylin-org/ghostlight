# 0002. Dual-role binary bridged over local IPC

- Status: Accepted
- Date: 2026-07

## Context

The engine sits on two protocol boundaries at once. On one side the MCP client launches
the binary as a stdio subprocess and speaks JSON-RPC 2.0. On the other, Chrome launches a
native-messaging host on `connectNative` and speaks the 4-byte-length-prefixed framing
(SPEC 2.1, 2.2). Chrome always spawns its own host process, so a single OS process cannot
serve both boundaries: the "one process at the center" ideal is not literally
achievable (SPEC 2.1, "Process reality (corrected in Phase 0)").

## Decision

The same executable runs in two roles. The mcp-server role is launched by the MCP client
over stdio, is long-lived, and owns the browser. The native-host role is launched by
Chrome, is short-lived, and may relaunch whenever the service worker wakes; it relays
native-messaging frames both ways (SPEC 2.1, 2.2; src/native/ipc.rs module doc).

The two instances bridge over a local socket (a named pipe on Windows, a Unix domain
socket elsewhere) with no TCP and no network dependency (commit 0962f85; SPEC 2.2). The
mcp-server owns the endpoint and serves; the native-host connects with retry and relays.
`server::run` forwards `tools/call` through a `Browser` handle that correlates each
request to its framed response by id, times out, and turns tool failures into MCP tool
error results (`isError`), not JSON-RPC errors (commit 0962f85, src/browser.rs).

## Consequences

- Still a major simplification over the reference: one Rust executable plus a local pipe
  in place of two Node processes and a localhost-TCP relay (SPEC 2.1, 2.2).
- Startup ordering is irrelevant: the native-host retries the connect for ~30s, so either
  role may come up first (src/native/ipc.rs).
- The `Browser` handle is transport-agnostic and tested over an in-memory pipe, so tool
  routing is verifiable without a real browser (commit 0962f85).
- Trade-off: the two-instance model introduces a lifecycle hazard (a stranded native-host
  zombie) addressed in ADR-0003, and a single-active-session constraint addressed in
  ADR-0004.
