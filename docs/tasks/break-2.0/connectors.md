# Connectors lane ledger

Lane rules are in [BOOTSTRAP.md](BOOTSTRAP.md). Target: ADR-0197 Decisions 1-4, 10, 11, 13-16.

## RESUME HERE

Next slice: connectors-3 onward, per the lane plan. No blockers.

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

## Blockers

None.
