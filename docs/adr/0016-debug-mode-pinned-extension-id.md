# 0016. Debug/observability mode + pinned dev extension id

- Status: Accepted
- Date: 2026-07

## Context

The mcp-server role is a subprocess launched by the client with stdout reserved
for the JSON-RPC stream, so its inner state (is the extension connected, what
calls are in flight, what just happened) is opaque during development and
support. Separately, an unpacked Chromium extension derives its id from its
on-disk path, so the id changes across machines and checkouts; that makes the
native-host `allowed_origins` and any `install --extension-id` value
non-deterministic and breaks reload-driven dogfooding.

## Decision

(a) Opt-in observability (commit f566793). `--debug` or `BROWSER_MCP_DEBUG=1`
turns on a sink that records the three boundaries (MCP request/response,
tool-call begin/end, and extension connect/disconnect) into two per-PID files
under the log dir: `debug-state-<pid>.json` (a live snapshot: uptime, extension
connected, in-flight calls, counters, recent events) and
`debug-events-<pid>.jsonl` (the append-only event firehose). `browser-mcp
status` renders the newest session's snapshot, and `install --debug` registers
the server with `BROWSER_MCP_DEBUG=1` so a client-launched server runs
observably. It is deliberately distinct from the (v1.5) governance audit
subsystem. The path is best-effort and self-contained: a poison-recovering lock
and swallowed I/O errors mean a debug fault never disturbs the server; detail
bodies are clipped to 600 bytes on a UTF-8 boundary, identifiers to 120, and
recent events capped at 64; the full-snapshot rewrite is throttled to 200ms
while the JSONL append always writes; per-PID files avoid clobbering across two
concurrent --debug servers, and stale session files (>24h) are cleaned on start.
Off by default it is a no-op sink, and stdout stays pure JSON-RPC.

(b) Pinned dev extension id (commit 37dcf9e). A committed public `key` in
`extension/manifest.json` fixes the unpacked extension id to
`cjcmhepmagomefjggkcohdbfemacojoa` regardless of its path on disk, making the
native-host `allowed_origins` deterministic and `install --extension-id` a
fixed, repeatable value. The private half (`extension/.dev-key.pem`) is
gitignored: it is only needed to pack a .crx later, never to load unpacked, and
must not be committed.

## Consequences

- Positive: the server's inner state is inspectable without polluting stdout;
  `install --debug` makes a client-launched server observable (dogfooding).
- Positive: a stable dev id makes native-messaging registration and extension
  reloads deterministic across machines and checkouts.
- Negative: per-PID debug files accumulate (mitigated by the >24h cleanup on
  start); the committed public key is dev-only and does not replace a real
  Web Store / packaged-extension identity.
- Follow-up: the governance audit subsystem (v1.5) is a separate concern and
  does not reuse this debug sink.
