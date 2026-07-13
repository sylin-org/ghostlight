// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Lossless TOML merge for Codex's `config.toml` MCP table.
//!
//! Codex shares one TOML configuration file across its CLI, desktop app, and IDE extension. This
//! module changes only the active Ghostlight server table, retaining unrelated settings, comments,
//! formatting, project entries, and sibling MCP servers. It deliberately mirrors the JSON merge
//! module's add, match, presence, and removal operations so install planning stays idempotent.

use super::merge::ServerEntry;
use std::collections::BTreeMap;
use toml_edit::{Array, DocumentMut, Item, Table, Value};

/// The top-level table Codex uses for MCP server declarations.
const MCP_SERVERS_TABLE: &str = "mcp_servers";

/// A TOML merge failure. Every variant means the original file remains untouched.
#[derive(Debug, thiserror::Error)]
pub enum TomlMergeError {
    #[error("config is not valid TOML: {0}")]
    Parse(String),
    #[error("config key '{MCP_SERVERS_TABLE}' is present but is not a table")]
    ServersNotTable,
    #[error("server entry '{0}' is present but is not a table")]
    ServerNotTable(String),
}

/// Parse `existing`, allowing an absent or empty config to begin as a new document.
fn parse(existing: &str) -> Result<DocumentMut, TomlMergeError> {
    if existing.trim().is_empty() {
        return Ok(DocumentMut::new());
    }
    existing
        .parse::<DocumentMut>()
        .map_err(|e| TomlMergeError::Parse(e.to_string()))
}

/// The exact TOML table Ghostlight owns for one Codex MCP server.
fn server_table(entry: &ServerEntry) -> Table {
    let mut table = Table::new();
    table.insert("command", Item::Value(Value::from(entry.command.clone())));

    let mut args = Array::new();
    for arg in &entry.args {
        args.push(arg.as_str());
    }
    table.insert("args", Item::Value(Value::Array(args)));

    if !entry.env.is_empty() {
        table.insert("env", Item::Table(env_table(&entry.env)));
    }
    table
}

/// Render the optional environment map in Codex's nested-table form.
fn env_table(env: &BTreeMap<String, String>) -> Table {
    let mut table = Table::new();
    for (name, value) in env {
        table.insert(name, Item::Value(Value::from(value.clone())));
    }
    table
}

/// Return the mutable `mcp_servers` table, creating it only when absent.
fn servers_table(doc: &mut DocumentMut) -> Result<&mut Table, TomlMergeError> {
    let root = doc.as_table_mut();
    if !root.contains_key(MCP_SERVERS_TABLE) {
        root.insert(MCP_SERVERS_TABLE, Item::Table(Table::new()));
    }
    match root.get_mut(MCP_SERVERS_TABLE) {
        Some(Item::Table(table)) => Ok(table),
        Some(_) => Err(TomlMergeError::ServersNotTable),
        None => unreachable!("the mcp_servers table was inserted above"),
    }
}

/// Return the existing `mcp_servers` table without creating it.
fn existing_servers_table(doc: &DocumentMut) -> Result<Option<&Table>, TomlMergeError> {
    match doc.as_table().get(MCP_SERVERS_TABLE) {
        None => Ok(None),
        Some(Item::Table(table)) => Ok(Some(table)),
        Some(_) => Err(TomlMergeError::ServersNotTable),
    }
}

/// Upsert Ghostlight's server table. Untouched TOML retains its original formatting and comments.
pub fn merge_server(existing: &str, entry: &ServerEntry) -> Result<String, TomlMergeError> {
    let mut doc = parse(existing)?;
    let servers = servers_table(&mut doc)?;
    servers.insert(&entry.name, Item::Table(server_table(entry)));
    Ok(doc.to_string())
}

/// True when a server table with `name` exists under Codex's `mcp_servers` table.
pub fn has_server(existing: &str, name: &str) -> Result<bool, TomlMergeError> {
    let doc = parse(existing)?;
    Ok(existing_servers_table(&doc)?.is_some_and(|servers| servers.contains_key(name)))
}

/// True only when the owned server table exactly matches the entry Ghostlight would write.
pub fn server_matches(existing: &str, entry: &ServerEntry) -> Result<bool, TomlMergeError> {
    let doc = parse(existing)?;
    let Some(servers) = existing_servers_table(&doc)? else {
        return Ok(false);
    };
    match servers.get(&entry.name) {
        None => Ok(false),
        Some(Item::Table(table)) => Ok(table_matches(table, entry)),
        Some(_) => Err(TomlMergeError::ServerNotTable(entry.name.clone())),
    }
}

/// Exact semantic equality for the table Ghostlight owns. `toml_edit::Table` intentionally does
/// not implement `PartialEq` because it preserves formatting metadata, so compare its values while
/// requiring that no stale Ghostlight-owned keys remain.
fn table_matches(table: &Table, entry: &ServerEntry) -> bool {
    let command_matches = matches!(
        table.get("command"),
        Some(Item::Value(value)) if value.as_str() == Some(entry.command.as_str())
    );
    let args_match = match table.get("args") {
        Some(Item::Value(value)) => value.as_array().is_some_and(|args| {
            args.len() == entry.args.len()
                && args
                    .iter()
                    .map(Value::as_str)
                    .eq(entry.args.iter().map(|arg| Some(arg.as_str())))
        }),
        _ => false,
    };
    let env_matches = match (table.get("env"), entry.env.is_empty()) {
        (None, true) => true,
        (Some(Item::Table(env)), false) => {
            env.len() == entry.env.len()
                && entry.env.iter().all(|(name, expected)| {
                    matches!(
                        env.get(name),
                        Some(Item::Value(value)) if value.as_str() == Some(expected.as_str())
                    )
                })
        }
        _ => false,
    };
    let expected_keys = if entry.env.is_empty() { 2 } else { 3 };
    command_matches && args_match && env_matches && table.len() == expected_keys
}

/// Remove only the named server table. Absence is a semantic no-op.
pub fn remove_server(existing: &str, name: &str) -> Result<String, TomlMergeError> {
    let mut doc = parse(existing)?;
    if existing_servers_table(&doc)?.is_some() {
        let root = doc.as_table_mut();
        let Some(Item::Table(servers)) = root.get_mut(MCP_SERVERS_TABLE) else {
            return Err(TomlMergeError::ServersNotTable);
        };
        servers.remove(name);
    }
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> ServerEntry {
        ServerEntry {
            name: "ghostlight".into(),
            command: "/abs/ghostlight-relay".into(),
            args: vec!["--role".into(), "agent".into()],
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn creates_the_codex_mcp_table_from_empty() {
        let out = merge_server("", &entry()).unwrap();
        let doc = parse(&out).unwrap();
        let server = existing_servers_table(&doc)
            .unwrap()
            .unwrap()
            .get("ghostlight");
        assert!(matches!(server, Some(Item::Table(_))));
        assert!(server_matches(&out, &entry()).unwrap());
    }

    #[test]
    fn preserves_unrelated_codex_configuration_and_comments() {
        let existing = "# personal defaults\nmodel = \"gpt-5\"\n\n[projects.\"repo\"]\ntrust_level = \"trusted\"\n\n[mcp_servers.other]\ncommand = \"other\"\n";
        let out = merge_server(existing, &entry()).unwrap();
        assert!(out.contains("# personal defaults"));
        assert!(out.contains("model = \"gpt-5\""));
        assert!(out.contains("[projects.\"repo\"]"));
        assert!(out.contains("[mcp_servers.other]"));
        assert!(server_matches(&out, &entry()).unwrap());
    }

    #[test]
    fn updates_our_entry_without_duplicate_and_is_idempotent() {
        let existing = "[mcp_servers.ghostlight]\ncommand = \"/old\"\nargs = []\n";
        let once = merge_server(existing, &entry()).unwrap();
        let twice = merge_server(&once, &entry()).unwrap();
        assert_eq!(once, twice);
        assert!(server_matches(&once, &entry()).unwrap());
    }

    #[test]
    fn environment_is_nested_only_when_present() {
        let mut entry = entry();
        entry.env.insert("GHOSTLIGHT_DEBUG".into(), "1".into());
        let out = merge_server("", &entry).unwrap();
        assert!(out.contains("[mcp_servers.ghostlight.env]"));
        assert!(server_matches(&out, &entry).unwrap());
    }

    #[test]
    fn rejects_non_table_mcp_servers_without_clobbering() {
        let existing = "mcp_servers = []\n";
        assert!(matches!(
            merge_server(existing, &entry()),
            Err(TomlMergeError::ServersNotTable)
        ));
    }

    #[test]
    fn remove_drops_only_our_server_table() {
        let existing =
            "[mcp_servers.ghostlight]\ncommand = \"x\"\n\n[mcp_servers.other]\ncommand = \"y\"\n";
        let out = remove_server(existing, "ghostlight").unwrap();
        assert!(!has_server(&out, "ghostlight").unwrap());
        assert!(has_server(&out, "other").unwrap());
    }
}
