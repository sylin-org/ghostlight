# T1: Add Windsurf as an installer target

**Goal.** `ghostlight install` detects Windsurf and registers the Ghostlight MCP server into its
config, idempotently, exactly as it already does for Cursor. Windsurf uses the SAME `mcpServers`
plain-JSON dialect, so this is a client-registry addition only -- no merge logic changes.

Normative: ADR-0071 D1 + D4 (Windsurf row). Windsurf (now Devin Desktop / Cascade) config verified
2026-07-13: `~/.codeium/windsurf/mcp_config.json`, top key `mcpServers`, entry `{command, args,
env}`, PLAIN JSON.

## Tree facts (AS OF AUTHORING 2026-07-13 -- RE-READ before editing)

All edits are in `crates/core/src/install/clients.rs`. As of authoring it contains:

- `pub enum ClientId { ClaudeCode, ClaudeDesktop, Cursor, VsCode, Codex }`
- `pub const CLIENTS: &[ClientSpec] = &[ ... ]` with five entries, each
  `ClientSpec { id, cli_id, display, add_via }`. Cursor's entry is:
  ```rust
  ClientSpec {
      id: ClientId::Cursor,
      cli_id: "cursor",
      display: "Cursor",
      add_via: AddVia::JsonFileMerge(Dialect::McpServers),
  },
  ```
- `fn config_path(spec, ctx) -> PathBuf { match spec.id { ... } }` -- an EXHAUSTIVE match; Cursor's
  arm is `ClientId::Cursor => ctx.home.join(".cursor").join("mcp.json"),`.
- `fn detect(spec, ctx) -> bool { match spec.id { ... } }` -- an EXHAUSTIVE match; Cursor's arm is
  `ClientId::Cursor => ctx.home.join(".cursor").is_dir(),`.
- `Dialect` and `AddVia` are already in scope (`use super::merge::{Dialect, ServerEntry};` and the
  local `AddVia` enum). `on_path` is imported (`use super::{on_path, PlanCtx};`).
- A `#[cfg(test)] mod tests` with `use super::*;` and `use std::path::PathBuf;` present, containing
  `codex_uses_the_shared_home_toml_config` (the pattern to mirror).

**STOP preconditions.** If any of these is false when you read the file, STOP and mark BLOCKED:
- `ClientId` is not the five-variant enum above (it was refactored).
- `config_path` or `detect` is no longer a `match spec.id` with per-client arms.
- `Dialect::McpServers` no longer exists in `merge.rs`, or `AddVia::JsonFileMerge(Dialect)` changed shape.

## Edits (exactly four, all in clients.rs)

1. **Enum** -- add `Windsurf` as the last variant:
   ```rust
   pub enum ClientId {
       ClaudeCode,
       ClaudeDesktop,
       Cursor,
       VsCode,
       Codex,
       Windsurf,
   }
   ```

2. **CLIENTS array** -- append this entry after the Codex entry (Windsurf reuses the Cursor/Claude
   `mcpServers` dialect):
   ```rust
   ClientSpec {
       id: ClientId::Windsurf,
       cli_id: "windsurf",
       display: "Windsurf",
       // ~/.codeium/windsurf/mcp_config.json is plain JSON with an `mcpServers` map, identical in
       // shape to Cursor's -- the value-level merge is idempotent and safe (ADR-0071 D1).
       add_via: AddVia::JsonFileMerge(Dialect::McpServers),
   },
   ```

3. **config_path arm** -- add before the closing brace of the `match spec.id`:
   ```rust
   ClientId::Windsurf => ctx.home.join(".codeium").join("windsurf").join("mcp_config.json"),
   ```

4. **detect arm** -- add before the closing brace of the `match spec.id`:
   ```rust
   ClientId::Windsurf => {
       on_path("windsurf") || ctx.home.join(".codeium").join("windsurf").is_dir()
   }
   ```

Nothing else changes. `server_registered` matches on `add_via` (not `id`), so it needs no arm.
`doctor` and the install plan iterate `CLIENTS`, so they pick up Windsurf with no edit.

## Test to add (BY NAME, pinned assertions -- transcribe verbatim)

Add to `#[cfg(test)] mod tests` in clients.rs, mirroring `codex_uses_the_shared_home_toml_config`:

```rust
/// Windsurf (Devin Desktop / Cascade) registers under the same plain-JSON `mcpServers` dialect as
/// Cursor, at ~/.codeium/windsurf/mcp_config.json (ADR-0071 D1).
#[test]
fn windsurf_uses_the_codeium_mcp_config_path() {
    let ctx = PlanCtx {
        current_exe: PathBuf::from("/opt/gl/ghostlight"),
        home: PathBuf::from("/home/tester"),
        config: PathBuf::from("/config"),
        local: PathBuf::from("/local"),
    };
    let windsurf = client_by_id("windsurf").expect("Windsurf is a supported client");
    assert_eq!(windsurf.display, "Windsurf");
    assert_eq!(
        config_path(windsurf, &ctx),
        PathBuf::from("/home/tester/.codeium/windsurf/mcp_config.json")
    );
    assert!(matches!(
        windsurf.add_via,
        AddVia::JsonFileMerge(Dialect::McpServers)
    ));
}
```

(If the `PlanCtx` literal's field set differs from the pinned four -- `current_exe, home, config,
local` -- STOP: copy the exact fields from the adjacent `codex_uses_the_shared_home_toml_config`
test instead, which is the live oracle for that struct.)

## Verify (literal commands)

```
CARGO_TARGET_DIR=target-check cargo fmt --check
CARGO_TARGET_DIR=target-check cargo clippy -p ghostlight-core --all-targets -- -D warnings
CARGO_TARGET_DIR=target-check cargo test -p ghostlight-core --lib install::clients
```
The third must show `windsurf_uses_the_codeium_mcp_config_path ... ok` and all sibling
`install::clients` tests still green.

If `cargo fmt --check` flags the new `detect` arm, run `CARGO_TARGET_DIR=target-check cargo fmt`
once and re-run the gate: exact whitespace is rustfmt's to decide; the LOGIC above (the enum
variant, the CLIENTS entry, the two match arms, the test assertions) is the oracle and must be
transcribed unchanged.

Optional manual confirmation (needs a built binary; not required for the green-tree gate):
`ghostlight install --client windsurf --dry-run` should plan an `mcpServers.ghostlight` entry
pointing at the `ghostlight-relay` sibling with `args ["--role","agent"]`, targeting
`~/.codeium/windsurf/mcp_config.json`.

## Out of scope (do NOT do these in T1)

- No `merge.rs` change. No new dialect. Windsurf is the existing `McpServers` dialect verbatim.
- No Zed / OpenCode / Crush work (T2-T4, blocked).
- No prose/doc edits (README, `llms-install.md`, `docs/guides/installation.md`) -- a separate
  follow-up syncs user-facing client lists. `doctor` already reports Windsurf via `CLIENTS`.
- Do not "verify" Windsurf's PATH binary name by judgment: the `on_path("windsurf")` check is the
  pinned value; the `.codeium/windsurf` dir check is the authoritative signal regardless.

## Commit

`feat(install): add Windsurf as an installer target (ADR-0071)` -- clients.rs only. Then update the
LEDGER.
