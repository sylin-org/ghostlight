# S7: Retire the adapter + native-host roles from the ghostlight bin

Goal: the `ghostlight` executable becomes CLI + service only. The role dispatch is deleted; a
bare invocation prints pinned guidance.

## STOP preconditions

- S6 not logged complete -> STOP.
- Any test still spawns the BARE bin in a retired role -- as an adapter (piped stdio MCP relay)
  or as the native host (`chrome-extension://` positional) -- after the S5/S6 re-points
  (`grep -rn "spawn_adapter\|CARGO_BIN_EXE_ghostlight\|chrome-extension" tests/` and inspect;
  service/CLI spawns of the bare bin are fine) -> STOP, list them.

## Required changes

1. Root `src/main.rs`:
   - Delete `run_native_host_role` and the `chrome-extension://` argv detection block.
   - The `Cli { command: None, .. }` arm: print the SPEC section 9 guidance (both lines,
     verbatim) to STDERR and `std::process::exit(2)`.
   - Keep everything else (install/uninstall/doctor/status/config/policy/service, the
     `resolve_instance` flag>env>argv0 fold, `--keep-warm`).
2. `crates/core/src/hub/mod.rs`: delete `run_mcp_server` and `run_as_adapter` (and now-unused
   imports the compiler names). `run_service`/`run_service_loop`/`idle_grace_watch`/
   `ServiceContext` stay untouched.
3. Doc comments in root `main.rs` and core `hub/mod.rs` that enumerate the roles: update the
   role lists to name the two adapter EXECUTABLES instead of in-process roles (surgical edits;
   keep every other sentence).

## Tests (pinned)

- NEW root integration test file `tests/bare_invocation.rs`:
  `bare_invocation_prints_guidance_and_exits_2` per SPEC section 9 (spawn
  `env!("CARGO_BIN_EXE_ghostlight")` with no args, `stdin(Stdio::null())`, capture stderr;
  assert `status.code() == Some(2)` and stderr contains
  `ghostlight no longer serves MCP directly`).
- The whole suite stays green (the S5/S6 re-pointing made every adapter-behavior test launch the
  adapter bins, so nothing else references the deleted roles).

## Verify (literal)

SPEC section 12. Plus: `grep -rn "run_mcp_server\|run_native_host_role\|run_as_adapter" src crates`
returns NO code hits (doc-comment prose mentions are acceptable; SPEC 5.1's naming fence keeps the
adapter mains clean, so any code hit is a defect). An empty grep exits 1 -- treat exit 1 with no
output as PASS.

## Out of scope

Reconnect constants (S8). Packaging (S10). Any install-plan change.

## Commit

`feat(ghostlight): retire the adapter and native-host roles from the main executable (S7)`
