//! Pure JSON merge for adding/removing the `ghostlight` server entry in an MCP client's config.
//!
//! No I/O -- the whole point is a unit-testable core. `serde_json`'s `preserve_order` feature keeps
//! sibling servers and key order intact across a merge. Never clobbers: a non-object root or a
//! non-object servers key is an error, not an overwrite.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// The top-level config-key dialect a client uses (docs/research/11-install-detection.md B.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// `mcpServers` (Claude Code/Desktop, Cursor). Entry is `{ command, args, env }`.
    McpServers,
    /// `servers` (VS Code). Entry additionally carries `"type": "stdio"`.
    Servers,
}

impl Dialect {
    /// The top-level object key this dialect stores servers under.
    pub fn top_key(self) -> &'static str {
        match self {
            Dialect::McpServers => "mcpServers",
            Dialect::Servers => "servers",
        }
    }
}

/// The stdio MCP server entry the installer registers.
#[derive(Debug, Clone)]
pub struct ServerEntry {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

impl ServerEntry {
    /// The JSON value for this entry under `dialect` (VS Code's `servers` adds `type: "stdio"`).
    pub fn to_value(&self, dialect: Dialect) -> Value {
        let mut obj = Map::new();
        if dialect == Dialect::Servers {
            obj.insert("type".into(), json!("stdio"));
        }
        obj.insert("command".into(), json!(self.command));
        obj.insert("args".into(), json!(self.args));
        obj.insert("env".into(), json!(self.env));
        Value::Object(obj)
    }
}

/// A merge failure. Every variant means "leave the file untouched", never a partial write.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("config root is not a JSON object")]
    NotAnObject,
    #[error("config key '{0}' is present but is not an object")]
    KeyNotObject(&'static str),
    #[error("config is not valid JSON: {0}")]
    Parse(String),
}

fn root_object(existing: &str) -> Result<Map<String, Value>, MergeError> {
    if existing.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(existing).map_err(|e| MergeError::Parse(e.to_string()))? {
        Value::Object(map) => Ok(map),
        _ => Err(MergeError::NotAnObject),
    }
}

fn serialize(root: Map<String, Value>) -> Result<String, MergeError> {
    serde_json::to_string_pretty(&Value::Object(root))
        .map(|s| s + "\n")
        .map_err(|e| MergeError::Parse(e.to_string()))
}

/// Upsert our server entry under the dialect's top-level key, preserving every other key and
/// sibling server. `existing` may be empty (absent/new file). Returns pretty JSON + trailing `\n`.
pub fn merge_server(
    existing: &str,
    dialect: Dialect,
    entry: &ServerEntry,
) -> Result<String, MergeError> {
    let mut root = root_object(existing)?;
    let key = dialect.top_key();
    let slot = root
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(servers) = slot else {
        return Err(MergeError::KeyNotObject(key));
    };
    servers.insert(entry.name.clone(), entry.to_value(dialect));
    serialize(root)
}

/// True if a server named `name` is already present under the dialect's key. Used for *semantic*
/// no-op detection so we never rewrite (and reformat) a file that already has the intended state.
pub fn has_server(existing: &str, dialect: Dialect, name: &str) -> Result<bool, MergeError> {
    let root = root_object(existing)?;
    Ok(matches!(root.get(dialect.top_key()), Some(Value::Object(m)) if m.contains_key(name)))
}

/// True if our entry is present under the dialect's key *and* equal to what we would write. A
/// semantic no-op check for install: avoids reformatting a config whose entry is already correct.
pub fn server_matches(
    existing: &str,
    dialect: Dialect,
    entry: &ServerEntry,
) -> Result<bool, MergeError> {
    let root = root_object(existing)?;
    Ok(match root.get(dialect.top_key()) {
        Some(Value::Object(m)) => m.get(&entry.name) == Some(&entry.to_value(dialect)),
        _ => false,
    })
}

/// Remove only our server entry (by `name`) under the dialect's key. No-op if absent. Preserves the
/// key and all siblings.
pub fn remove_server(existing: &str, dialect: Dialect, name: &str) -> Result<String, MergeError> {
    let mut root = root_object(existing)?;
    if let Some(Value::Object(servers)) = root.get_mut(dialect.top_key()) {
        servers.remove(name);
    }
    serialize(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> ServerEntry {
        ServerEntry {
            name: "ghostlight".into(),
            command: "/abs/ghostlight".into(),
            args: vec![],
            env: BTreeMap::new(),
        }
    }
    fn parse(s: &str) -> Value {
        serde_json::from_str(s).expect("output is valid JSON")
    }

    #[test]
    fn creates_from_empty() {
        let out = parse(&merge_server("", Dialect::McpServers, &entry()).unwrap());
        assert_eq!(
            out["mcpServers"]["ghostlight"]["command"],
            "/abs/ghostlight"
        );
        assert_eq!(out.as_object().unwrap().len(), 1);
    }

    #[test]
    fn preserves_sibling_servers() {
        let existing = r#"{"mcpServers":{"other":{"command":"x"}}}"#;
        let out = parse(&merge_server(existing, Dialect::McpServers, &entry()).unwrap());
        assert_eq!(out["mcpServers"]["other"]["command"], "x");
        assert_eq!(
            out["mcpServers"]["ghostlight"]["command"],
            "/abs/ghostlight"
        );
    }

    #[test]
    fn updates_our_entry_not_duplicate() {
        let existing = r#"{"mcpServers":{"ghostlight":{"command":"/old"}}}"#;
        let out = parse(&merge_server(existing, Dialect::McpServers, &entry()).unwrap());
        assert_eq!(
            out["mcpServers"]["ghostlight"]["command"],
            "/abs/ghostlight"
        );
        assert_eq!(out["mcpServers"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn servers_dialect_adds_type_stdio() {
        let existing = r#"{"servers":{"foo":{"command":"y"}}}"#;
        let out = parse(&merge_server(existing, Dialect::Servers, &entry()).unwrap());
        assert_eq!(out["servers"]["ghostlight"]["type"], "stdio");
        assert_eq!(out["servers"]["foo"]["command"], "y");
    }

    #[test]
    fn preserves_unrelated_top_level_keys() {
        let existing = r#"{"someTop":true,"mcpServers":{}}"#;
        let out = parse(&merge_server(existing, Dialect::McpServers, &entry()).unwrap());
        assert_eq!(out["someTop"], true);
        assert_eq!(
            out["mcpServers"]["ghostlight"]["command"],
            "/abs/ghostlight"
        );
    }

    #[test]
    fn non_object_servers_key_errors_without_clobber() {
        let existing = r#"{"mcpServers":[]}"#;
        assert!(matches!(
            merge_server(existing, Dialect::McpServers, &entry()),
            Err(MergeError::KeyNotObject("mcpServers"))
        ));
    }

    #[test]
    fn non_object_root_errors() {
        assert!(matches!(
            merge_server("[]", Dialect::McpServers, &entry()),
            Err(MergeError::NotAnObject)
        ));
    }

    #[test]
    fn merge_is_idempotent() {
        let once = merge_server("", Dialect::McpServers, &entry()).unwrap();
        let twice = merge_server(&once, Dialect::McpServers, &entry()).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn has_server_detects_presence() {
        let with = r#"{"mcpServers":{"ghostlight":{"command":"x"}}}"#;
        let without = r#"{"mcpServers":{"other":{}}}"#;
        assert!(has_server(with, Dialect::McpServers, "ghostlight").unwrap());
        assert!(!has_server(without, Dialect::McpServers, "ghostlight").unwrap());
        assert!(!has_server("", Dialect::McpServers, "ghostlight").unwrap());
    }

    #[test]
    fn server_matches_is_true_only_for_our_exact_entry() {
        // A file we just wrote round-trips to a semantic match (drives install no-op).
        let written = merge_server("", Dialect::McpServers, &entry()).unwrap();
        assert!(server_matches(&written, Dialect::McpServers, &entry()).unwrap());
        // Present but different command -> not a match (we should rewrite).
        let stale = r#"{"mcpServers":{"ghostlight":{"command":"/old","args":[],"env":{}}}}"#;
        assert!(!server_matches(stale, Dialect::McpServers, &entry()).unwrap());
        // Absent -> not a match.
        assert!(!server_matches("", Dialect::McpServers, &entry()).unwrap());
    }

    #[test]
    fn remove_drops_only_our_entry() {
        let existing = r#"{"mcpServers":{"ghostlight":{"command":"x"},"other":{"command":"y"}}}"#;
        let out = parse(&remove_server(existing, Dialect::McpServers, "ghostlight").unwrap());
        assert!(out["mcpServers"].get("ghostlight").is_none());
        assert_eq!(out["mcpServers"]["other"]["command"], "y");
    }

    #[test]
    fn remove_is_noop_when_absent() {
        let existing = r#"{"mcpServers":{"other":{}}}"#;
        let out = parse(&remove_server(existing, Dialect::McpServers, "ghostlight").unwrap());
        assert!(out["mcpServers"]["other"].is_object());
        // empty input round-trips to an empty object without panic
        assert_eq!(
            parse(&remove_server("", Dialect::McpServers, "ghostlight").unwrap()),
            json!({})
        );
    }
}
