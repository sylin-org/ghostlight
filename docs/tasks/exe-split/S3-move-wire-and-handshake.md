# S3: Move the wire + handshake into ghostlight-transport

Goal: move the native-messaging framing, the hub handshake/anti-squat/session-guid/supervisor
modules, and the ADAPTER half of ipc.rs into transport. The SERVICE half of ipc.rs stays in the
root crate (it moves to core in S4). After this task the transport crate is functionally complete.

## STOP preconditions

- S2 not logged complete -> STOP.
- `src/transport/native/ipc.rs` does not contain BOTH `pub async fn relay_adapter` and
  `pub async fn serve_adapters` -> STOP (the adapter/service halves are not where authoring
  assumed).

## Required changes

1. `git mv` per SPEC section 2 rows: `host`, `handshake`, `antisquat`, `supervisor`
   (destinations `crates/transport/src/<module>.rs`).
2. SPLIT `src/hub/session.rs`: move the `SessionGuid` struct + impls + its Display/Debug + the
   SessionGuid-only unit tests into NEW `crates/transport/src/session_guid.rs` (SPDX header
   Apache-2.0 OR MIT; module doc: one sentence naming ADR-0030 Decision 4). EVERYTHING ELSE in
   the file stays (PeerCred, PeerUser, SessionRegistry, Admission, TabClaim, claim_tab,
   owned_tab_ids, group_title, SessionSummary, live_session_summaries, and all remaining tests);
   at its top add `pub use ghostlight_transport::session_guid::SessionGuid;` so every
   `crate::hub::session::SessionGuid` path keeps resolving.
   SANCTIONED two-line body edit (private field crosses the crate boundary): the staying code
   accesses the guid's private tuple field for the 8-char group-title prefix -- replace
   `&guid.0[..8]` with `&guid.as_str()[..8]` in `group_title`, and the same `.0` -> `.as_str()`
   form at its test's usage. No other body edits.
3. SPLIT `src/transport/native/ipc.rs` per SPEC section 2's two item lists:
   - Adapter half -> NEW `crates/transport/src/ipc.rs` (SPDX header + carry the module doc
     comment's first paragraph; adjust intra-crate paths per SPEC section 2's rewrite list, e.g.
     `crate::hub::supervisor::start_service` -> `crate::supervisor::start_service`,
     `crate::hub::session::SessionGuid` -> `crate::session_guid::SessionGuid`,
     `crate::hub::handshake` -> `crate::handshake`, `host::` -> `crate::host::`,
     `crate::observability::DebugSink` -> `crate::observability::DebugSink`).
     Make `pipe_path`/`socket_path`/`adapter_endpoint_name` `pub` (SPEC pins this).
   - Service half stays in `src/transport/native/ipc.rs`, which now ALSO does
     `pub use ghostlight_transport::ipc::*;` at its top so the historical
     `ghostlight::native::ipc::{connect, probe_endpoint, default_endpoint, relay_adapter, ...}`
     paths still resolve; its own service items now call the moved helpers via
     `ghostlight_transport::ipc::{pipe_path, socket_path, set_mode, adapter_endpoint_name, default_endpoint}`
     and `ghostlight_transport::{handshake, antisquat, session_guid}` as needed.
   - `src/transport/native/mod.rs`: replace `pub mod host;` with
     `pub use ghostlight_transport::host;` (the framing move would otherwise break
     `ghostlight::native::host` and `crate::transport::native::host` importers).
4. Root `src/hub/mod.rs`: replace `pub mod handshake; pub mod antisquat; pub mod supervisor;` with
   `pub use ghostlight_transport::{antisquat, handshake, supervisor};`. Fix root-crate callers of
   the moved items ONLY where the compiler demands (the re-exports should cover nearly all).
5. `src/install/supervisor.rs` and `src/install/native_host.rs` imports keep working via the
   re-exports; if the compiler asks, point them at `ghostlight_transport::supervisor` /
   `ghostlight_transport::host` directly.
6. Transport `Cargo.toml` gains: sha2, hmac, getrandom, uuid (SPEC section 2 dependency listing;
   final state must MATCH that listing exactly).

## Tests

No new tests; moved unit tests travel with their files (host framing tests, antisquat tests,
session_guid tests, supervisor pin test, the three adapter-side ipc tests SPEC names). Oracle:
full suite green; `cargo test -p ghostlight-transport` runs the travelled tests with 0 failed.

SANCTIONED test edit (source-path scan; SPEC section 13): `tests/hub_lifecycle.rs` has a test
that reads `src/hub/supervisor.rs` as TEXT via CARGO_MANIFEST_DIR
(`supervisor_start_asserts_adapter_role`); re-point that path string to
`crates/transport/src/supervisor.rs`. No assertion changes.

## Verify (literal)

SPEC section 12. Plus:
`grep -n "pub use ghostlight_transport::ipc" src/transport/native/ipc.rs` (the merge shim exists).

## Out of scope

Moving the service half anywhere (S4). browser/governance/install/mcp trees. Any reconnect
behavior change (S8).

## Commit

`refactor(transport): move wire framing, handshake, anti-squat, session-guid, supervisor, and the adapter ipc half (S3)`
