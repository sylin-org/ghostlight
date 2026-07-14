// SPDX-License-Identifier: Apache-2.0 OR MIT
//! MCP-client detection and config targets: which clients are installed, where their config lives,
//! how we add our server entry (CLI vs safe JSON merge), and the dialect each uses (doc 11 B.*).

use super::merge::{Dialect, ServerEntry};
use super::{merge, toml_merge};
use super::{on_path, PlanCtx};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The v1 client set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientId {
    ClaudeCode,
    ClaudeDesktop,
    Cursor,
    VsCode,
    Codex,
    Windsurf,
    Zed,
    OpenCode,
    Crush,
}

/// How we register with a client. `FileMerge` is the idempotent value-level merge used for every
/// plain-JSON config; `VsCodeCli` drives VS Code's `code --add-mcp` (its config is JSONC, which a
/// value-level merge would strip of comments).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddVia {
    VsCodeCli,
    JsonFileMerge(Dialect),
    TomlFileMerge,
}

pub struct ClientSpec {
    pub id: ClientId,
    pub cli_id: &'static str,
    pub display: &'static str,
    pub add_via: AddVia,
}

pub const CLIENTS: &[ClientSpec] = &[
    ClientSpec {
        id: ClientId::ClaudeCode,
        cli_id: "claude-code",
        display: "Claude Code",
        // ~/.claude.json is plain JSON; a value-level merge is idempotent and safe even while
        // Claude Code is running (the merge re-reads at apply time -- see install::apply_merge).
        add_via: AddVia::JsonFileMerge(Dialect::McpServers),
    },
    ClientSpec {
        id: ClientId::ClaudeDesktop,
        cli_id: "claude-desktop",
        display: "Claude Desktop",
        add_via: AddVia::JsonFileMerge(Dialect::McpServers),
    },
    ClientSpec {
        id: ClientId::Cursor,
        cli_id: "cursor",
        display: "Cursor",
        add_via: AddVia::JsonFileMerge(Dialect::McpServers),
    },
    ClientSpec {
        id: ClientId::VsCode,
        cli_id: "vscode",
        display: "VS Code",
        add_via: AddVia::VsCodeCli,
    },
    ClientSpec {
        id: ClientId::Codex,
        cli_id: "codex",
        display: "Codex",
        add_via: AddVia::TomlFileMerge,
    },
    ClientSpec {
        id: ClientId::Windsurf,
        cli_id: "windsurf",
        display: "Windsurf",
        // ~/.codeium/windsurf/mcp_config.json is plain JSON with an `mcpServers` map, identical in
        // shape to Cursor's -- the value-level merge is idempotent and safe (ADR-0071 D1).
        add_via: AddVia::JsonFileMerge(Dialect::McpServers),
    },
    ClientSpec {
        id: ClientId::Zed,
        cli_id: "zed",
        display: "Zed",
        // settings.json is JSONC; a commented file degrades to a printed manual step (T2).
        add_via: AddVia::JsonFileMerge(Dialect::ContextServers),
    },
    ClientSpec {
        id: ClientId::OpenCode,
        cli_id: "opencode",
        display: "OpenCode",
        // opencode.json is JSONC; the `mcp` entry uses type:"local" + a command array (T2 dialect).
        add_via: AddVia::JsonFileMerge(Dialect::OpenCodeMcp),
    },
    ClientSpec {
        id: ClientId::Crush,
        cli_id: "crush",
        display: "Crush",
        // crush.json's `mcp` entry uses type:"stdio" + command/args/env (T2 dialect). A commented
        // file degrades to a printed manual step (T2 JSONC-safe fallback).
        add_via: AddVia::JsonFileMerge(Dialect::CrushMcp),
    },
];

pub fn client_by_id(id: &str) -> Option<&'static ClientSpec> {
    CLIENTS.iter().find(|c| c.cli_id == id)
}

/// The user-scope config file for a client. Uniform across OSes because [`PlanCtx::config`] is the
/// per-OS base (`%APPDATA%` / `~/Library/Application Support` / `~/.config`).
pub fn config_path(spec: &ClientSpec, ctx: &PlanCtx) -> PathBuf {
    match spec.id {
        ClientId::ClaudeCode => ctx.home.join(".claude.json"),
        ClientId::ClaudeDesktop => ctx.config.join("Claude").join("claude_desktop_config.json"),
        ClientId::Cursor => ctx.home.join(".cursor").join("mcp.json"),
        ClientId::VsCode => ctx.config.join("Code").join("User").join("mcp.json"),
        ClientId::Codex => ctx.home.join(".codex").join("config.toml"),
        ClientId::Windsurf => ctx
            .home
            .join(".codeium")
            .join("windsurf")
            .join("mcp_config.json"),
        ClientId::Zed => {
            if cfg!(target_os = "linux") {
                ctx.home.join(".config").join("zed").join("settings.json")
            } else {
                ctx.config.join("Zed").join("settings.json")
            }
        }
        ClientId::OpenCode => ctx
            .home
            .join(".config")
            .join("opencode")
            .join("opencode.json"),
        ClientId::Crush => ctx.home.join(".config").join("crush").join("crush.json"),
    }
}

/// Multi-signal detection (doc 11 C.2).
pub fn detect(spec: &ClientSpec, ctx: &PlanCtx) -> bool {
    match spec.id {
        ClientId::ClaudeCode => on_path("claude") || ctx.home.join(".claude.json").is_file(),
        ClientId::ClaudeDesktop => config_path(spec, ctx).is_file(),
        ClientId::Cursor => ctx.home.join(".cursor").is_dir(),
        ClientId::VsCode => {
            on_path("code")
                || config_path(spec, ctx)
                    .parent()
                    .is_some_and(std::path::Path::is_dir)
        }
        ClientId::Codex => on_path("codex") || ctx.home.join(".codex").is_dir(),
        ClientId::Windsurf => {
            on_path("windsurf") || ctx.home.join(".codeium").join("windsurf").is_dir()
        }
        ClientId::Zed => {
            on_path("zed")
                || config_path(spec, ctx)
                    .parent()
                    .is_some_and(std::path::Path::is_dir)
        }
        ClientId::OpenCode => {
            on_path("opencode") || ctx.home.join(".config").join("opencode").is_dir()
        }
        ClientId::Crush => on_path("crush") || ctx.home.join(".config").join("crush").is_dir(),
    }
}

/// True when this client's configuration contains the active Ghostlight MCP server.
pub fn server_registered(spec: &ClientSpec, contents: &str, name: &str) -> bool {
    match spec.add_via {
        // VS Code's JSONC is deliberately not parsed by the installer; retain its prior
        // conservative quoted-key check for doctor reporting.
        AddVia::VsCodeCli => contents.contains(&format!("\"{name}\"")),
        AddVia::JsonFileMerge(dialect) => merge::has_server(contents, dialect, name)
            .unwrap_or_else(|_| contents.contains(&format!("\"{name}\""))),
        AddVia::TomlFileMerge => toml_merge::has_server(contents, name).unwrap_or(false),
    }
}

/// The server entry we register: absolute binary path, never npx (doc 11 B.7/C.4).
pub fn server_entry(exe: &Path) -> ServerEntry {
    let instance = ghostlight_transport::instance::Instance::resolve();
    // The single relay binary carries both roles (ADR-0051 Phase 3); the client launches it with an
    // explicit `--role agent`. A non-default instance also carries `--instance <n>` so the client
    // launches the right stack. The command stays the bare (stable) binary path, so a dev rebuild is
    // picked up with no reinstall (the adapter is a dumb pipe; ADR-0044 Decision 4 / ADR-0045).
    let mut args = vec!["--role".to_string(), "agent".to_string()];
    if let Some(name) = instance.name() {
        args.push("--instance".to_string());
        args.push(name.to_string());
    }
    ServerEntry {
        name: instance.mcp_server_name(),
        command: super::native_host::sibling_bin(exe, "ghostlight-relay")
            .to_string_lossy()
            .into_owned(),
        args,
        env: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The client entry launches the RELAY sibling in the agent role (ADR-0046 + ADR-0051 Phase 3),
    /// never the `ghostlight` binary itself: MCP clients speak to `ghostlight-relay --role agent`,
    /// which relays to the service.
    #[test]
    fn server_entry_points_at_the_relay_sibling_in_agent_role() {
        let exe = Path::new("/opt/gl/ghostlight");
        let entry = server_entry(exe);
        let cmd = entry.command;
        assert!(
            cmd.contains("ghostlight-relay"),
            "command names the relay binary: {cmd}"
        );
        assert_eq!(
            &entry.args[..2],
            &["--role".to_string(), "agent".to_string()],
            "the agent role is passed explicitly: {:?}",
            entry.args
        );
        let suffix = if cfg!(windows) {
            "ghostlight-relay.exe"
        } else {
            "ghostlight-relay"
        };
        assert!(cmd.ends_with(suffix), "command ends with {suffix}: {cmd}");
        assert!(
            cmd.contains("gl"),
            "command retains the parent dir /opt/gl: {cmd}"
        );
    }

    /// Codex is a first-class global TOML client: its CLI, desktop app, and IDE extension share
    /// this one config file, so it must use the home-scoped Codex path rather than VS Code's JSONC.
    #[test]
    fn codex_uses_the_shared_home_toml_config() {
        let ctx = PlanCtx {
            current_exe: PathBuf::from("/opt/gl/ghostlight"),
            home: PathBuf::from("/home/tester"),
            config: PathBuf::from("/config"),
            local: PathBuf::from("/local"),
        };
        let codex = client_by_id("codex").expect("Codex is a supported client");
        assert_eq!(codex.display, "Codex");
        assert_eq!(
            config_path(codex, &ctx),
            PathBuf::from("/home/tester/.codex/config.toml")
        );
        assert!(matches!(codex.add_via, AddVia::TomlFileMerge));
    }

    /// Doctor's format-aware check recognizes Codex's TOML table and never relies on a JSON-key
    /// substring that would miss the unquoted `ghostlight` table segment.
    #[test]
    fn codex_registration_check_parses_the_toml_server_table() {
        let codex = client_by_id("codex").unwrap();
        let configured =
            "[mcp_servers.ghostlight]\ncommand = \"relay\"\nargs = [\"--role\", \"agent\"]\n";
        assert!(server_registered(codex, configured, "ghostlight"));
        assert!(!server_registered(codex, configured, "other"));
    }

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

    #[test]
    fn jsonc_config_with_comments_is_detected_by_substring_fallback() {
        let cursor = client_by_id("cursor").unwrap(); // any JsonFileMerge client
        let jsonc = "{\n  // a comment makes this unparseable as strict JSON\n  \"mcpServers\": { \"ghostlight\": {} }\n}";
        assert!(server_registered(cursor, jsonc, "ghostlight"));
        assert!(!server_registered(cursor, jsonc, "other"));
    }

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
        assert!(matches!(
            zed.add_via,
            AddVia::JsonFileMerge(Dialect::ContextServers)
        ));
        #[cfg(not(target_os = "linux"))]
        assert_eq!(
            config_path(zed, &ctx),
            PathBuf::from("/config/Zed/settings.json")
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            config_path(zed, &ctx),
            PathBuf::from("/home/tester/.config/zed/settings.json")
        );
    }

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
        assert!(matches!(
            oc.add_via,
            AddVia::JsonFileMerge(Dialect::OpenCodeMcp)
        ));
        assert_eq!(
            config_path(oc, &ctx),
            PathBuf::from("/home/tester/.config/opencode/opencode.json")
        );
    }

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
        assert!(matches!(
            crush.add_via,
            AddVia::JsonFileMerge(Dialect::CrushMcp)
        ));
        assert_eq!(
            config_path(crush, &ctx),
            PathBuf::from("/home/tester/.config/crush/crush.json")
        );
    }
}
