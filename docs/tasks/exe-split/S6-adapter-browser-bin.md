# S6: The ghostlight-adapter-browser executable

Goal: add the browser-side pass-through binary, rework the native-host install to point at it
(default = the sibling bin; non-default instance = a tiny per-instance copy of IT), and add the
base-parameterized exe-stem resolution. Also re-point the e2e harness.

## STOP preconditions

- S5 not logged complete -> STOP.
- `crates/core/src/install/native_host.rs` does not contain `pub fn instance_launcher` -> STOP.

## Required changes

1. Workspace member + `crates/adapter-browser/` crate: Cargo.toml mirrors S5's (package + bin
   `ghostlight-adapter-browser`; same deps).
2. `crates/transport/src/instance.rs`: add `from_exe_stem_with_base` EXACTLY per SPEC section 7;
   re-implement `from_exe_stem` as the base="ghostlight" call.
3. Write `crates/adapter-browser/src/main.rs` per SPEC section 5.2 EXACTLY (env-then-argv0
   instance resolution with WARN-not-exit on invalid; role string "native-host"; the direct
   `process::exit(0)` with the parked-stdin comment carried over from the current
   `run_native_host_role` in root `src/main.rs` -- re-read it first and transcribe).
4. `crates/core/src/install/native_host.rs` rework `instance_launcher`:
   - default instance -> `(sibling_bin(&ctx.current_exe, "ghostlight-adapter-browser"), false)`;
   - named instance `<n>` -> copy target
     `ctx.local.join(dir_leaf).join("ghostlight-adapter-browser-<n>[.exe]")`, `true`.
   In `install/mod.rs`'s copy action, EVERY reference to the copy source becomes the
   adapter-browser sibling (`sibling_bin(&ctx.current_exe, "ghostlight-adapter-browser")`), not
   `ctx.current_exe`: the `CopyBinary.from`, the `up_to_date` size comparison's second metadata
   read, AND the `manual:` hint string. Update the doc comments to match (they currently
   describe copying the running binary).
5. `tests/e2e/run-smoke.mjs` (re-read first): the wrapper/manifest `path` wraps the
   adapter-browser bin; the stdio MCP spawn uses the adapter-agent bin; the `service` spawn stays
   on the `ghostlight` bin; its internal `cargo build` invocation gains `--workspace`. Derive
   both adapter paths as siblings of the existing `binaryPath`.
6. `tests/peer_death.rs` (deferred here from S5; re-read first): its native-host-role spawn
   (the one passing a `chrome-extension://...` positional argument) switches from the bare
   `ghostlight` bin to the `ghostlight-adapter-browser` sibling (same env, same args -- the bin
   tolerates and ignores them). Assertions unchanged.

## Tests (pinned)

- NEW in `crates/transport/src/instance.rs`:
  `from_exe_stem_with_base_resolves_the_browser_adapter_family` with the four pinned cases from
  SPEC section 7.
- NEW in `crates/core/src/install/native_host.rs` tests:
  `instance_launcher_default_is_the_adapter_browser_sibling` -- with the existing test `ctx()`,
  assert the DEFAULT case only: `(path, false)` where the path string ends with
  `ghostlight-adapter-browser.exe` on windows / `ghostlight-adapter-browser` elsewhere.
  DO NOT set `GHOSTLIGHT_INSTANCE` inside a unit test (env mutation races the parallel tests
  that call `Instance::resolve`); the NAMED-instance path is covered by the subprocess dry-run
  assertion below, which isolates the env per process.
- `tests/install_instance.rs` EDIT (sanctioned):
  `dev_install_plan_copies_a_named_binary_and_suffixes_the_whole_stack` -- the "instance binary"
  assertion becomes `plan.contains("ghostlight-adapter-browser-dev")`.
- Verification-by-node: `node --check tests/e2e/run-smoke.mjs` passes.

## Verify (literal)

SPEC section 12; `! cargo tree -p ghostlight-adapter-browser | grep -q ghostlight-core` exits 0;
the node check above.

## Out of scope

Retiring `run_native_host_role` from the root bin (S7). Any extension/ file. Packaging (S10).

## Commit

`feat(adapter-browser): the browser-side pass-through executable; host install points at it (S6)`
