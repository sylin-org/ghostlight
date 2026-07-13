# T5: Add Crush as an installer target

**Goal.** Register Ghostlight under Crush's `mcp` key (the `type:"stdio"` shape). Depends on T2 (the
`CrushMcp` dialect). Normative: ADR-0071, `PINS.md` P1 (Crush row) + P2.3 (CrushMcp == Servers shape
under key `mcp`).

## Tree facts (RE-READ)

Same `clients.rs` structure as T1/T3/T4. T2 has added `Dialect::CrushMcp`. STOP if it does not exist
(run T2 first) or client structs changed.

## Edits (clients.rs only)

1. `ClientId`: add `Crush`.
2. `CLIENTS`: append
   ```rust
   ClientSpec {
       id: ClientId::Crush,
       cli_id: "crush",
       display: "Crush",
       // crush.json's `mcp` entry uses type:"stdio" + command/args/env (T2 dialect). A commented
       // file degrades to a printed manual step (T2 JSONC-safe fallback).
       add_via: AddVia::JsonFileMerge(Dialect::CrushMcp),
   },
   ```
3. `config_path` arm -- Crush uses `~/.config` literally on all OSes:
   ```rust
   ClientId::Crush => ctx.home.join(".config").join("crush").join("crush.json"),
   ```
4. `detect` arm:
   ```rust
   ClientId::Crush => on_path("crush") || ctx.home.join(".config").join("crush").is_dir(),
   ```

## Test (BY NAME, pinned)

```rust
/// Crush registers under the mcp (type:stdio) dialect at ~/.config/crush/crush.json (ADR-0071).
#[test]
fn crush_uses_the_mcp_stdio_dialect_at_config_crush() {
    let ctx = PlanCtx {
        current_exe: PathBuf::from("/opt/gl/ghostlight"),
        home: PathBuf::from("/home/tester"),
        config: PathBuf::from("/config"),
        local: PathBuf::from("/local"),
    };
    let crush = client_by_id("crush").expect("Crush is a supported client");
    assert_eq!(crush.display, "Crush");
    assert!(matches!(crush.add_via, AddVia::JsonFileMerge(Dialect::CrushMcp)));
    assert_eq!(
        config_path(crush, &ctx),
        PathBuf::from("/home/tester/.config/crush/crush.json")
    );
}
```

## Note

Crush's JSON-vs-JSONC format needs no separate resolution: T2's JSONC-safe fallback auto-merges a
comment-free file and degrades a commented one to a printed manual step, so either format is handled
correctly.

## Verify

```
CARGO_TARGET_DIR=target-check cargo fmt --check
CARGO_TARGET_DIR=target-check cargo clippy -p ghostlight-core --all-targets -- -D warnings
CARGO_TARGET_DIR=target-check cargo test -p ghostlight-core --lib install::clients
```

## Out of scope

merge.rs (T2); other clients; docs/prose. Commit:
`feat(install): add Crush as an installer target (ADR-0071)`, then update the LEDGER.
