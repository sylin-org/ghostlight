# Connectors lane ledger

Lane rules are in [BOOTSTRAP.md](BOOTSTRAP.md). Target: ADR-0197 Decisions 1-4, 10, 11, 13-16.

## RESUME HERE

Next slice: connectors-5 onward, per the lane plan. No blockers.

## Landed

- connectors-0: ADR-0197 recorded; the ADR index status cells and ADR header notes mark what it
  supersedes or amends; this batch opened.

- connectors-1: `crates/relay` (package `ghostlight-relay` 3.0.0): framing, the fixed home and
  file names, the runtime file, the opening and its replies, failure templates. Fixtures under
  `tests/golden/relay3/`, bound by `crates/relay/tests/golden.rs`. Workspace `rust-version` is
  now 1.89. Gate: 35 unit tests and 9 fixture tests pass.

- connectors-2: relay mechanics in `crates/relay`: `spawn` (backoff, detached `serve` launch),
  `watchdog` (parent identity and polling), `log` (marker-gated diagnostics lines), `client`
  (connect and read the reply under a deadline), `server` (bounded opening reader, constant-time
  token compare, the lease). Gate: 64 unit tests and 9 fixture tests pass. Not covered by a test:
  the null stdio of the `serve` command, which `std::process::Command` does not expose.

- connectors-3: `ghostlight-mcp-connector` 3.0.0 rebuilt on `ghostlight-relay` (`main.rs`,
  `forward.rs`, `hold.rs`); its MCP handling (`mcp_2025_11_25.rs`, `mcp_2026_07_28.rs`,
  `service_session.rs`) deleted for the service lane to port. It forwards lines unchanged, tracks
  request ids per direction (batches per element), answers `ping` itself, holds lines while the
  service is away (4 MiB, 30 s each), starts the service only at its own start and for a held
  request, presents the resume token on reconnect, and answers cut-off and expired requests from
  the template file. Gate: 16 connector tests, 64 relay unit tests and 9 fixture tests pass.
  Known interim break: the bridge test
  `both_connectors_recover_through_the_shared_lifecycle_seam` reads the deleted
  `service_session.rs`; it goes with the bridge in the switch.

- connectors-4: `ghostlight-browser-connector` 3.0.0 rebuilt on `ghostlight-relay` (`main.rs`,
  `port.rs`); it depends on the relay crate and `anyhow` only. It holds the extension's hello,
  drops later Chrome frames until a service accepts a `browser` stream, starts the service on
  the backoff schedule only before that first connection, then pumps raw frames both ways on two
  threads without parsing. When the stream ends it writes `backend_unavailable` once, drops
  Chrome's frames, never starts the service, and probes with a read-only control `readiness`
  opening on the backoff schedule; the first accepted probe ends the run so the extension
  reconnects with a fresh hello. Gate: 11 connector tests, 19 MCP connector tests, 64 relay unit
  tests and 9 fixture tests pass. The bridge test that reads `browser-connector/src/main.rs`
  stays broken until the bridge goes in the switch, like the MCP one.

## Blockers

None.
