# S2: Move the leaf utilities into ghostlight-transport

Goal: move the dependency-light foundation modules (error, proc, instance, observability,
watchdog, role) plus `init_tracing` and `build_debug_sink` into the transport crate, with the root
crate re-exporting them so every existing path keeps compiling.

## STOP preconditions

- S1 not logged complete in the LEDGER -> STOP.
- Any of these files absent at its listed path -> STOP: `src/error.rs`, `src/proc.rs`,
  `src/instance.rs`, `src/observability.rs`, `src/transport/watchdog.rs`, `src/hub/role.rs`.

## Required changes

1. `git mv` per the SPEC section 2 table rows for: `error`, `proc`, `instance`, `observability`,
   `watchdog`, `role` (destinations `crates/transport/src/<module>.rs`).
2. Transport `lib.rs`: declare exactly those six `pub mod`s, add
   `pub use error::{Error, Result, ToolError};`, and add the `init_tracing` fn CUT from the root
   `src/lib.rs` (SPEC section 2 listing).
3. MOVE `build_debug_sink` from `src/hub/mod.rs` into `crates/transport/src/observability.rs`
   (same body; its ONE internal `crate::observability::log_dir()` call becomes `log_dir()`;
   the `DebugSink` references need nothing). Re-point every call site:
   - root `src/hub/mod.rs` (`run_mcp_server`, `run_service`) ->
     `crate::observability::build_debug_sink` (resolves via the facade re-export below);
   - root `src/main.rs` in `run_native_host_role` (currently
     `ghostlight::hub::build_debug_sink(debug, "native-host")`) ->
     `ghostlight::observability::build_debug_sink(debug, "native-host")`.
4. Intra-transport path fixes per SPEC section 2's rewrite list (e.g. observability's
   `crate::proc::creation_time` keeps working -- same crate; watchdog's `crate::proc` keeps
   working; role/instance need no changes).
5. Root `src/lib.rs` becomes an interim facade: delete the moved module declarations and add
   `pub use ghostlight_transport::{error, instance, observability, proc};`,
   `pub use ghostlight_transport::error::{Error, Result, ToolError};`,
   `pub use ghostlight_transport::init_tracing;`, and inside the existing `pub use transport::...`
   area ensure `ghostlight::transport::watchdog` still resolves: change `src/transport/mod.rs` to
   `pub use ghostlight_transport::watchdog;` (replacing `pub mod watchdog;`). For `role`: add in
   `src/hub/mod.rs` a `pub use ghostlight_transport::role;` replacing its `pub mod role;`.
6. Transport `Cargo.toml` gains ONLY the deps this code needs (from SPEC section 2's dependency
   listing: tokio, serde, serde_json, tracing, tracing-subscriber, thiserror, dirs, plus the
   cfg(windows) windows-sys trio and cfg(unix) libc). Leave sha2/hmac/getrandom/uuid for S3.
7. Root `Cargo.toml`: REMOVE deps that are now unused by the root crate ONLY IF the build proves
   them unused; when unsure, leave them (S4 does the final pruning).

## Tests

None added; every moved file carries its own unit tests along. Oracle: the full suite passes with
IDENTICAL test names (the moved unit tests now run inside `ghostlight-transport`).

## Verify (literal)

SPEC section 12. Additionally:
`cargo test -p ghostlight-transport 2>&1 | grep "test result"` shows the moved unit tests running
(instance, observability, watchdog, role, error tool_error_tests) with 0 failed.

## Out of scope

host/ipc/handshake/antisquat/session/supervisor (S3). Any behavior change. The mcp/native trees.

## Commit

`refactor(transport): move error/proc/instance/observability/watchdog/role into ghostlight-transport (S2)`
