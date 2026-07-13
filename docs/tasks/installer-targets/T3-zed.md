# T3: Add Zed as an installer target

**Goal.** `ghostlight install` registers Ghostlight under Zed's `context_servers` key. Depends on
T2 (the `ContextServers` dialect + JSONC-safe fallback must already exist). Normative: ADR-0071,
`PINS.md` P1 (Zed row + path notes) and the fact that `context_servers` uses the same entry shape
as `mcpServers` (no `source`, command string).

## Tree facts (RE-READ)

Same `clients.rs` structure as T1 (five..N-variant `ClientId`, `CLIENTS`, `config_path`/`detect`
exhaustive matches, `#[cfg(test)] mod tests`). T2 has added `Dialect::ContextServers` to `merge.rs`.
STOP if `Dialect::ContextServers` does not exist (run T2 first) or the client structs changed shape.

## Edits (clients.rs only)

1. `ClientId`: add `Zed`.
2. `CLIENTS`: append
   ```rust
   ClientSpec {
       id: ClientId::Zed,
       cli_id: "zed",
       display: "Zed",
       // settings.json is JSONC; a commented file degrades to a printed manual step (T2).
       add_via: AddVia::JsonFileMerge(Dialect::ContextServers),
   },
   ```
3. `config_path` arm -- per-OS casing (PINS P1: `Zed` on mac/win, lowercase `zed` on linux):
   ```rust
   ClientId::Zed => {
       if cfg!(target_os = "linux") {
           ctx.home.join(".config").join("zed").join("settings.json")
       } else {
           ctx.config.join("Zed").join("settings.json")
       }
   }
   ```
4. `detect` arm:
   ```rust
   ClientId::Zed => on_path("zed") || config_path(spec, ctx).parent().is_some_and(std::path::Path::is_dir),
   ```

## Test (BY NAME, pinned)

```rust
/// Zed registers under the context_servers dialect at its per-OS settings.json (ADR-0071).
#[test]
fn zed_uses_context_servers_at_the_per_os_settings_path() {
    let ctx = PlanCtx {
        current_exe: PathBuf::from("/opt/gl/ghostlight"),
        home: PathBuf::from("/home/tester"),
        config: PathBuf::from("/config"),
        local: PathBuf::from("/local"),
    };
    let zed = client_by_id("zed").expect("Zed is a supported client");
    assert_eq!(zed.display, "Zed");
    assert!(matches!(zed.add_via, AddVia::JsonFileMerge(Dialect::ContextServers)));
    #[cfg(not(target_os = "linux"))]
    assert_eq!(config_path(zed, &ctx), PathBuf::from("/config/Zed/settings.json"));
    #[cfg(target_os = "linux")]
    assert_eq!(config_path(zed, &ctx), PathBuf::from("/home/tester/.config/zed/settings.json"));
}
```

## RESIDUAL confirm before shipping

Confirm against a running Zed that a custom `context_servers` entry does NOT require `"source":
"custom"` (current docs say it does not; PINS P1). If it does, add that field in T2's
`ContextServers` `to_value` arm and its oracle -- that is a T2 amendment, not a T3 change.

## Verify

```
CARGO_TARGET_DIR=target-check cargo fmt --check
CARGO_TARGET_DIR=target-check cargo clippy -p ghostlight-core --all-targets -- -D warnings
CARGO_TARGET_DIR=target-check cargo test -p ghostlight-core --lib install::clients
```

## Out of scope

merge.rs (done in T2); other clients; docs/prose. Commit:
`feat(install): add Zed as an installer target (ADR-0071)`, then update the LEDGER.
