# PINS: installer-targets batch (ADR-0071)

Authoritative, code-level oracles for T2-T5. The frontier author resolved these; the executor
TRANSCRIBES them and never derives them. Where a fact could not be fully resolved from vendor docs
it is marked RESIDUAL PIN with the confirm step -- a standing re-verify order, not a blocker.

Research verified 2026-07-13 against: Zed docs `raw.githubusercontent.com/zed-industries/zed/main/
docs/src/ai/mcp.md`; OpenCode `opencode.ai/docs/mcp-servers` + `/docs/config`; Crush
`github.com/charmbracelet/crush` README; Windsurf `docs.devin.ai/desktop/cascade/mcp`. Installer
shapes read from `crates/core/src/install/{merge.rs,clients.rs,mod.rs}` at authoring.

## P0. Design summary (why this is small)

- **The JSONC-safe merge is already the architecture.** `install/merge.rs::merge_server` already
  refuses to touch a file it cannot parse (returns `MergeError::Parse`, never a partial write). The
  install planner (`mod.rs plan_client_install`) already has `Op::Manual` ("print the manual hint")
  and a `Tally.manual` kept distinct from `failed`. So "JSONC-safe" = route a JSON `MergeError::Parse`
  (a config with comments/trailing commas that serde cannot parse) to `Op::Manual`, not
  `Op::Blocked`. A JSONC file with NO comments parses fine and merges normally; a commented one
  degrades to a printed manual step. Zero heuristics, zero new deps.
- **Zed's entry shape == `mcpServers`** (Zed docs confirmed NO `source` field, `command` is a plain
  string, fields `command`/`args`/`env`). So `context_servers` is a different top-key with the
  existing shape.
- **Crush's entry shape == the existing `Servers` dialect** (`type:"stdio"` + command/args/env),
  under key `mcp`.
- **OpenCode is the only genuinely new shape**: `type:"local"`, `command` is an ARRAY combining
  command+args, `enabled:true`, env under `environment`.

## P1. Per-client pinned facts

Our registered entry is always: `name = "ghostlight"` (instance-resolved), `command = <path to
ghostlight-relay>`, `args = ["--role","agent"]`, `env = {}` (see `clients::server_entry`).

| Client | cli_id | display | config_path (per OS) | detect | Dialect |
|---|---|---|---|---|---|
| Zed | `zed` | `Zed` | mac `ctx.config/Zed/settings.json`; win `ctx.config/Zed/settings.json`; **linux `ctx.home/.config/zed/settings.json`** (lowercase!) | `on_path("zed")` or the settings dir exists | `ContextServers` |
| OpenCode | `opencode` | `OpenCode` | `ctx.home/.config/opencode/opencode.json` (all OSes) [RESIDUAL: Windows] | `on_path("opencode")` or `ctx.home/.config/opencode` dir | `OpenCodeMcp` |
| Crush | `crush` | `Crush` | `ctx.home/.config/crush/crush.json` (all OSes) | `on_path("crush")` or `ctx.home/.config/crush` dir | `CrushMcp` |

Path notes (PIN):
- **Zed dir casing is per-OS**: `Zed` under the OS config base on macOS/Windows, but literal
  `~/.config/zed` (lowercase) on Linux. `ctx.config` is `~/Library/Application Support` (mac) /
  `%APPDATA%` (win) / `~/.config` (linux), so Linux would give `~/.config/Zed` -- WRONG casing.
  Therefore Zed's `config_path` arm MUST branch: `if cfg!(target_os = "linux") { ctx.home.join(".config").join("zed").join("settings.json") } else { ctx.config.join("Zed").join("settings.json") }`.
- **OpenCode + Crush use `~/.config/...` literally on every OS** (XDG-style apps), NOT `ctx.config`.
  So use `ctx.home.join(".config").join(...)`, not `ctx.config`.
- **RESIDUAL PIN (OpenCode, Windows)**: docs did not state the Windows user config path. Pin
  `~/.config/opencode/opencode.json` on all OSes; before shipping T4, confirm OpenCode's Windows
  location on a real install. If it differs, update only the Windows branch of the OpenCode arm.
- **RESIDUAL PIN (Zed source field)**: current Zed docs (main) show NO `"source": "custom"`. Pinned
  accordingly. If a specific installed Zed rejects a sourceless custom server, add `"source":
  "custom"` to the `ContextServers` `to_value` arm (and its oracle). Low risk; confirm against the
  running Zed.

## P2. merge.rs changes (T2)

### P2.1 Add three `Dialect` variants

```rust
pub enum Dialect {
    McpServers,
    Servers,
    ContextServers, // Zed: key "context_servers", same entry shape as McpServers
    OpenCodeMcp,    // OpenCode: key "mcp", { type:"local", command:[cmd,...args], enabled:true }
    CrushMcp,       // Crush: key "mcp", same entry shape as Servers ({ type:"stdio", ... })
}
```

### P2.2 `top_key`

```rust
pub fn top_key(self) -> &'static str {
    match self {
        Dialect::McpServers => "mcpServers",
        Dialect::Servers => "servers",
        Dialect::ContextServers => "context_servers",
        Dialect::OpenCodeMcp | Dialect::CrushMcp => "mcp",
    }
}
```

### P2.3 `ServerEntry::to_value` (replace the body)

```rust
pub fn to_value(&self, dialect: Dialect) -> Value {
    let mut obj = Map::new();
    match dialect {
        // OpenCode: command + args COMBINED into one array; type/enabled required; env -> "environment".
        Dialect::OpenCodeMcp => {
            obj.insert("type".into(), json!("local"));
            let mut command = vec![self.command.clone()];
            command.extend(self.args.iter().cloned());
            obj.insert("command".into(), json!(command));
            obj.insert("enabled".into(), json!(true));
            if !self.env.is_empty() {
                obj.insert("environment".into(), json!(self.env));
            }
        }
        // Everything else uses a command string + args + env; Servers/Crush add type:"stdio".
        _ => {
            if matches!(dialect, Dialect::Servers | Dialect::CrushMcp) {
                obj.insert("type".into(), json!("stdio"));
            }
            obj.insert("command".into(), json!(self.command));
            obj.insert("args".into(), json!(self.args));
            obj.insert("env".into(), json!(self.env));
        }
    }
    Value::Object(obj)
}
```

`merge_server`, `has_server`, `server_matches`, `remove_server` are dialect-generic (they call
`top_key`/`to_value`); they need NO change.

### P2.4 merge.rs tests to add (BY NAME, pinned oracles)

The existing `fn entry()` has `args: vec![]`. Use it where args do not matter; build a local entry
with args for the OpenCode array oracle.

```rust
#[test]
fn context_servers_dialect_is_mcpservers_shape_under_a_different_key() {
    let out = parse(&merge_server("", Dialect::ContextServers, &entry()).unwrap());
    assert_eq!(out["context_servers"]["ghostlight"]["command"], "/abs/ghostlight");
    assert!(out["context_servers"]["ghostlight"].get("type").is_none());
}

#[test]
fn crush_mcp_dialect_adds_type_stdio_under_mcp() {
    let out = parse(&merge_server("", Dialect::CrushMcp, &entry()).unwrap());
    assert_eq!(out["mcp"]["ghostlight"]["type"], "stdio");
    assert_eq!(out["mcp"]["ghostlight"]["command"], "/abs/ghostlight");
}

#[test]
fn opencode_mcp_dialect_uses_command_array_type_local_and_omits_empty_env() {
    let e = ServerEntry {
        name: "ghostlight".into(),
        command: "/abs/ghostlight".into(),
        args: vec!["--role".into(), "agent".into()],
        env: BTreeMap::new(),
    };
    let out = parse(&merge_server("", Dialect::OpenCodeMcp, &e).unwrap());
    assert_eq!(out["mcp"]["ghostlight"]["type"], "local");
    assert_eq!(out["mcp"]["ghostlight"]["enabled"], true);
    assert_eq!(
        out["mcp"]["ghostlight"]["command"],
        json!(["/abs/ghostlight", "--role", "agent"])
    );
    assert!(out["mcp"]["ghostlight"].get("environment").is_none());
}
```

## P3. mod.rs change (T2): route JSONC parse-failure to Manual

In `plan_client_install`, the `AddVia::JsonFileMerge(dialect)` arm (the `match merge::merge_server(
...).and_then(...)` block), split the single `Err(e) => blocked(...)` arm:

```rust
// A config we cannot parse as JSON is almost always JSONC-with-comments (Zed/OpenCode/Crush). Do
// NOT reformat it (that would strip the user's comments); degrade to a printed manual step, which
// the tally counts as `manual`, never `failed`.
Err(merge::MergeError::Parse(_)) => Action {
    label,
    detail: target,
    noop: None,
    manual,
    op: Op::Manual,
},
Err(e) => blocked(label, target, e.to_string(), manual),
```

`MergeError` is `pub` in merge.rs and reachable as `merge::MergeError`. `NotAnObject`/`KeyNotObject`
stay `blocked` (a structurally wrong file we must not touch). This arm is shared by ALL JSON clients;
the plain-JSON ones (Windsurf/Cursor/Claude) never reach it because their files parse.

RESIDUAL (optional polish, not required): enrich the `manual` string to include the literal entry
JSON (`serde_json::to_string_pretty(&entry.to_value(dialect))`). Out of scope for T2 unless trivial.

## P4. clients.rs change (T2): tolerant `server_registered` for JSONC

`server_registered`'s `AddVia::JsonFileMerge(dialect)` arm currently returns
`merge::has_server(...).unwrap_or(false)`, which is `false` on a JSONC-with-comments file (serde
cannot parse it) -- so `doctor` would wrongly report "not registered". Add a substring fallback on
parse failure, mirroring the existing VS Code JSONC check:

```rust
AddVia::JsonFileMerge(dialect) => {
    merge::has_server(contents, dialect, name)
        .unwrap_or_else(|_| contents.contains(&format!("\"{name}\"")))
}
```

Pinned test (clients.rs tests):

```rust
#[test]
fn jsonc_config_with_comments_is_detected_by_substring_fallback() {
    let cursor = client_by_id("cursor").unwrap(); // any JsonFileMerge client
    let jsonc = "{\n  // a comment makes this unparseable as strict JSON\n  \"mcpServers\": { \"ghostlight\": {} }\n}";
    assert!(server_registered(cursor, jsonc, "ghostlight"));
    assert!(!server_registered(cursor, jsonc, "other"));
}
```

## P5. Sequence and independence

T2 (merge.rs + mod.rs + clients.rs foundation) MUST land before T3-T5. Each of T3-T5 then adds one
`ClientId` + its `config_path`/`detect` arms + one path test, exactly like T1-windsurf but with the
PINS shapes above. Every task leaves a green tree. T2 changes no client wiring, so T2 alone is a
coherent, green, shippable commit (new dialects exist but are unused until T3-T5).
