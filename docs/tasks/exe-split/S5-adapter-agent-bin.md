# S5: The ghostlight-adapter-agent executable

Goal: add the MCP-side pass-through binary, point client registrations and the test harness at
it, and switch CI to workspace scope. After this task MCP clients launch the adapter bin; the
bare `ghostlight` adapter role still exists (retired in S7).

## STOP preconditions

- S4 not logged complete -> STOP.
- `crates/core/src/hub/mod.rs` does not contain `pub fn run_mcp_server` and `async fn
  run_as_adapter` -> STOP (the reference bodies for the new main are missing).

## Required changes

1. Workspace: add `"crates/adapter-agent"` member. Create `crates/adapter-agent/Cargo.toml` per
   SPEC section 5.1 (package + bin `ghostlight-adapter-agent`, version 0.3.0, publish false,
   license Apache-2.0 OR MIT, deps: ghostlight-transport path dep, tokio {rt-multi-thread,
   macros}, tracing).
2. Write `crates/adapter-agent/src/main.rs` implementing SPEC section 5.1 EXACTLY (instance
   resolution order + exit-2 message, init_tracing, set_role(Adapter), build_debug_sink(debug,
   "adapter"), parent watchdog select against `ipc::relay_adapter`, sink.flush,
   process::exit(code), the --manifest warning). Transcribe the select/exit structure from the
   CURRENT `run_as_adapter` in `crates/core/src/hub/mod.rs` (re-read it first) -- adjusted to
   transport-crate paths -- rather than inventing a new shape. NO dependency on ghostlight-core.
3. `crates/core/src/install/native_host.rs`: add `pub fn sibling_bin` EXACTLY as SPEC section 5.3.
4. `crates/core/src/install/clients.rs` `server_entry`: command becomes
   `sibling_bin(exe, "ghostlight-adapter-agent")` (string-lossy, as today); args/name logic
   unchanged.
5. `tests/support/mod.rs`: add
   `pub fn adapter_bin() -> std::path::PathBuf` = sibling of `env!("CARGO_BIN_EXE_ghostlight")`
   named `ghostlight-adapter-agent` (+`.exe` when `cfg!(windows)`); switch `spawn_adapter` to
   spawn it. Update the direct bare-bin ADAPTER spawn the harness does not cover:
   `tests/hub_lifecycle.rs` (re-read it; replace the adapter-role `Command::new(bin())` with the
   same sibling derivation -- a tiny local helper or support's).
   CAUTION: `tests/peer_death.rs` spawns the NATIVE-HOST role (its spawn passes a
   `chrome-extension://...` positional argument) -- it is NOT an adapter; DO NOT touch it in this
   task (S6 re-points it to the browser adapter). Find any other adapter spawns:
   `grep -rn "CARGO_BIN_EXE_ghostlight" tests/ | grep -v support` and update ONLY spawns whose
   process acts as the ADAPTER role (service spawns and the peer_death native-host spawn stay).
6. `tests/adapter_reconnect.rs`: its `spawn_adapter` switches to the sibling adapter bin (same
   env vars as today).
7. `.github/workflows/ci.yml` (sanctioned exception): the test job's two cargo lines become
   `cargo clippy --workspace --all-targets --locked -- -D warnings` and
   `cargo test --locked --no-fail-fast --workspace`; the e2e job's build line becomes
   `cargo build --locked --workspace`.
8. The new main must respect SPEC 5.1's NAMING FENCE (no fn named run_mcp_server /
   run_as_adapter / run_native_host_role; use `relay_with_watchdog` if a helper is wanted).

## Tests (pinned)

- NEW in `crates/core/src/install/clients.rs` tests module:
  `server_entry_points_at_the_agent_adapter_sibling`: with
  `exe = Path::new("/opt/gl/ghostlight")`, assert `server_entry(exe).command` ends with
  `"ghostlight-adapter-agent.exe"` on windows / `"ghostlight-adapter-agent"` elsewhere, and
  starts with the parent dir (`/opt/gl` -- use `.contains("gl")` for windows-path tolerance:
  pin: `assert!(cmd.contains("ghostlight-adapter-agent"))` plus the suffix assertion).
- Existing `tests/adapter_reconnect.rs` and `tests/mcp_protocol.rs` pass UNCHANGED in their
  assertions (only the spawn target changed) -- they are the real oracle that the new bin relays
  correctly.

## Verify (literal)

SPEC section 12. Plus:
- `cargo build -p ghostlight-adapter-agent` succeeds.
- `! cargo tree -p ghostlight-adapter-agent | grep -q ghostlight-core` exits 0 (the load-bearing
  dependency rule: core must be absent from the adapter's tree).

## Out of scope

Retiring the adapter role from `ghostlight` (S7). The browser adapter (S6). release.yml (S10).

## Commit

`feat(adapter-agent): the MCP-side pass-through executable; clients + tests launch it (S5)`
