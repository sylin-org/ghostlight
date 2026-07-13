# T4: Add OpenCode as an installer target

**Goal.** Register Ghostlight under OpenCode's `mcp` key (the `type:"local"`, command-array shape).
Depends on T2 (the `OpenCodeMcp` dialect). Normative: ADR-0071, `PINS.md` P1 (OpenCode row) + P2.3
(the OpenCode `to_value` shape).

## Tree facts (RE-READ)

Same `clients.rs` structure as T1/T3. T2 has added `Dialect::OpenCodeMcp`. STOP if it does not exist
(run T2 first) or client structs changed.

## Edits (clients.rs only)

1. `ClientId`: add `OpenCode`.
2. `CLIENTS`: append
   ```rust
   ClientSpec {
       id: ClientId::OpenCode,
       cli_id: "opencode",
       display: "OpenCode",
       // opencode.json is JSONC; the `mcp` entry uses type:"local" + a command array (T2 dialect).
       add_via: AddVia::JsonFileMerge(Dialect::OpenCodeMcp),
   },
   ```
3. `config_path` arm -- OpenCode uses `~/.config` literally on all OSes (NOT ctx.config):
   ```rust
   ClientId::OpenCode => ctx.home.join(".config").join("opencode").join("opencode.json"),
   ```
4. `detect` arm:
   ```rust
   ClientId::OpenCode => on_path("opencode") || ctx.home.join(".config").join("opencode").is_dir(),
   ```

## Test (BY NAME, pinned)

```rust
/// OpenCode registers under the mcp (type:local, command-array) dialect at ~/.config/opencode (ADR-0071).
#[test]
fn opencode_uses_the_mcp_dialect_at_config_opencode() {
    let ctx = PlanCtx {
        current_exe: PathBuf::from("/opt/gl/ghostlight"),
        home: PathBuf::from("/home/tester"),
        config: PathBuf::from("/config"),
        local: PathBuf::from("/local"),
    };
    let oc = client_by_id("opencode").expect("OpenCode is a supported client");
    assert_eq!(oc.display, "OpenCode");
    assert!(matches!(oc.add_via, AddVia::JsonFileMerge(Dialect::OpenCodeMcp)));
    assert_eq!(
        config_path(oc, &ctx),
        PathBuf::from("/home/tester/.config/opencode/opencode.json")
    );
}
```

## RESIDUAL confirm before shipping

OpenCode's Windows user config path was not documented (PINS P1). Pinned as
`~/.config/opencode/opencode.json` on all OSes. Confirm on a real Windows OpenCode install; if it
uses `%APPDATA%` instead, add a `cfg!(target_os = "windows")` branch to this arm (T4-local change).

## Verify

```
CARGO_TARGET_DIR=target-check cargo fmt --check
CARGO_TARGET_DIR=target-check cargo clippy -p ghostlight-core --all-targets -- -D warnings
CARGO_TARGET_DIR=target-check cargo test -p ghostlight-core --lib install::clients
```

## Out of scope

merge.rs (T2); other clients; docs/prose. Commit:
`feat(install): add OpenCode as an installer target (ADR-0071)`, then update the LEDGER.
