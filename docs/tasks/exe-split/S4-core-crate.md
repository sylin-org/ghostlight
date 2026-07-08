# S4: Create ghostlight-core; the root becomes a facade

Goal: move everything that is not the transport substrate or the binary shell into a new
`ghostlight-core` crate. The root package keeps `main.rs`, `tests/`, and a pure facade `lib.rs`.
This is the largest, most mechanical task; it changes NO behavior.

## STOP preconditions

- S3 not logged complete -> STOP.
- `tests/tool_schema_fidelity.rs` imports anything other than `ghostlight::`-rooted paths
  (check: `grep -n "use " tests/tool_schema_fidelity.rs`) -> STOP (facade plan would not cover it).

## Required changes

1. Workspace: add `"crates/core"` to members. Create `crates/core/Cargo.toml` per SPEC sections
   1 and 3 (package `ghostlight-core`, version 0.3.0, publish false,
   `license-file = "../../LICENSE"`, deps per SPEC section 3 incl.
   `ghostlight-transport = { path = "../transport" }`).
2. `git mv` per the SPEC section 3 table: browser/, governance/, hub/ (what remains of it),
   install/, `src/transport/mcp/` -> `crates/core/src/mcp/`,
   `src/transport/native/messages.rs` -> `crates/core/src/messages.rs`, `src/origin.rs`.
3. The service half of `src/transport/native/ipc.rs` moves to NEW
   `crates/core/src/hub/endpoint.rs` (module doc: one line, "The service-side endpoint owners
   (ADR-0030): serve, claim, serve_adapters; split from the old ipc module by ADR-0046.").
   Delete the now-empty `src/transport/` tree (mod.rs, native/mod.rs, native/ipc.rs shims).
4. Write `crates/core/src/lib.rs` EXACTLY as SPEC section 3 pins it. Add `pub mod endpoint;` to
   `crates/core/src/hub/mod.rs`.
5. Apply the SPEC section 3 path-rewrite table across every moved core file (mechanical; sed-like,
   then fix stragglers by compiler error). session.rs keeps its S3 `pub use` of SessionGuid.
6. Replace root `src/lib.rs` with the SPEC section 6 facade EXACTLY.
7. Root `Cargo.toml` final dependency prune per SPEC section 4 (keep only what `main.rs` needs;
   the compiler is the referee; log every kept-but-questionable dep as a deviation note).
8. `src/main.rs`: imports keep resolving via the facade; where it referenced
   `ghostlight::hub::manage::doctor` etc., no change should be needed. Fix only what the
   compiler names.

## Tests

None added. Oracle: the ENTIRE suite (root package integration tests + moved unit tests now in
core/transport) passes with zero failures, and `tests/tool_schema_fidelity.rs` +
`tests/all_open_golden.rs` pass UNTOUCHED.

SANCTIONED test edits (source-path scans; SPEC section 13), and ONLY these two:
- `tests/architecture.rs`: the path it asserts for the governance tree becomes
  `crates/core/src/governance` (re-point the CARGO_MANIFEST_DIR join; assertions unchanged).
- `tests/hub_role_wiring.rs`: the source path it reads becomes
  `crates/core/src/mcp/server.rs` (re-point only; assertions unchanged).

## Verify (literal)

SPEC section 12. Plus:
- `cargo test -p ghostlight-core 2>&1 | grep "test result"` (core unit tests run, 0 failed)
- `git diff --name-only HEAD~1..HEAD -- tests/` lists EXACTLY `tests/architecture.rs` and
  `tests/hub_role_wiring.rs` and nothing else.

## Out of scope

New binaries (S5/S6). main.rs role retirement (S7). Anything behavioral.

## Commit

`refactor(core): move the churny brain into ghostlight-core; the root crate becomes facade + shell (S4)`
