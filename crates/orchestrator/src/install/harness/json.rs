//! The JSON dialects: where each keeps its servers, Ghostlight's entry in each, and the
//! edit that sets only Ghostlight's keys and keeps every other byte of the file.

use super::*;

/// The key of the servers collection in the dialects that keep it at the top as `mcpServers`.
pub(super) const MCP_SERVERS_KEY: &str = "mcpServers";
/// The key of the servers collection in the dialects that call it `servers`, at the top or
/// below `mcp`.
pub(super) const SERVERS_KEY: &str = "servers";
/// The key of the servers collection in Zed's settings.
pub(super) const CONTEXT_SERVERS_KEY: &str = "context_servers";
/// The key of the `mcp` object that holds the servers, or holds `servers`, in its dialects.
pub(super) const MCP_KEY: &str = "mcp";

/// The key of the kind of server an entry describes, in the dialects that need one.
pub(super) const TYPE_KEY: &str = "type";

pub(super) fn inspect_json(
    source: &str,
    dialect: JsonDialect,
    edge: &Path,
    windows: bool,
) -> Result<RegistrationState, HarnessError> {
    let parsed = parse_jsonc(source)?;
    Ok(
        json_entry(&parsed, dialect).map_or(RegistrationState::Missing, |entry| {
            json_entry_state(entry, dialect, edge, windows)
        }),
    )
}

/// The state of `entry`, Ghostlight's key in a file written in `dialect`, for `edge`. It is the
/// one reading of whether the entry is current: inspection reports it and an install that finds
/// it current changes nothing. Current means every key Ghostlight owns holds its value: the
/// command and its arguments name this edge, and the dialect's `type` is set. The defaults and
/// every key the client added decide nothing (ADR-0203 Decision 10.1).
pub(super) fn json_entry_state(
    entry: &Value,
    dialect: JsonDialect,
    edge: &Path,
    windows: bool,
) -> RegistrationState {
    let Some(command) = json_entry_command(entry, dialect) else {
        return RegistrationState::Foreign(None);
    };
    let args = json_entry_args(entry, dialect);
    match command_registration_state(command, args.as_deref(), edge, windows) {
        RegistrationState::Current
            if json_entry_type(dialect)
                .is_some_and(|kind| entry.get(TYPE_KEY).and_then(Value::as_str) != Some(kind)) =>
        {
            RegistrationState::Updatable
        }
        state => state,
    }
}

/// Make the edit `intent` to Ghostlight's entry in the JSON file at `path`, written in `dialect`,
/// for the edge and the computer `context` describes. Whether the file changed.
///
/// An install edits an entry Ghostlight owns in place and keeps every key that is the client's;
/// it writes the whole entry only where there is none. Fix replaces a foreign entry whole.
pub(super) fn edit_json_with(
    context: &HarnessContext,
    path: &Path,
    dialect: JsonDialect,
    intent: RegistrationEdit,
) -> Result<bool, HarnessError> {
    let edge = context.edge.as_path();
    let source = read_or_empty(path)?;
    let parsed = parse_jsonc(&source)?;
    let current = json_entry(&parsed, dialect);
    if let Some(entry) = current {
        let foreign = !json_entry_owned(entry, dialect, edge, context.windows);
        if foreign && !intent.replaces_foreign() {
            return Err(unowned_entry_refusal(
                json_entry_command(entry, dialect),
                json_entry_args(entry, dialect).as_deref(),
            ));
        }
        if !foreign && intent.replaces_foreign() {
            return Err(HarnessError::NotFixable);
        }
        if intent == RegistrationEdit::Install
            && json_entry_state(entry, dialect, edge, context.windows) == RegistrationState::Current
        {
            return Ok(false);
        }
    } else if intent.replaces_foreign() {
        return Err(HarnessError::NoEntryToFix);
    } else if !intent.installs() {
        return Ok(false);
    }
    let root = CstRootNode::parse(&source, &jsonc_options()).map_err(malformed_json_at)?;
    let object = root
        .object_value_or_create()
        .ok_or(HarnessError::Malformed(None))?;
    let collection = json_collection(&object, dialect, intent.installs())?;
    if intent.installs() {
        let collection = collection.ok_or(HarnessError::Malformed(None))?;
        match (collection.get(SERVER_NAME), current) {
            (Some(existing), Some(found)) if intent == RegistrationEdit::Install => {
                update_owned_json_entry(&existing, found, edge, dialect)?;
            }
            (Some(existing), _) => {
                existing.set_value(cst_input(expected_json_entry(edge, dialect)))
            }
            (None, _) => {
                collection.append(SERVER_NAME, cst_input(expected_json_entry(edge, dialect)));
            }
        }
    } else if let Some(entry) = collection.and_then(|collection| collection.get(SERVER_NAME)) {
        entry.remove();
    }
    let rendered = root.to_string();
    replace_with_backup(path, rendered.as_bytes())?;
    Ok(true)
}

/// The keys from a client's root to the object that holds its servers, in `dialect`.
pub(super) const fn collection_path(dialect: JsonDialect) -> &'static [&'static str] {
    match dialect {
        JsonDialect::McpServers | JsonDialect::Copilot => &[MCP_SERVERS_KEY],
        JsonDialect::Servers => &[SERVERS_KEY],
        JsonDialect::ContextServers => &[CONTEXT_SERVERS_KEY],
        JsonDialect::Crush | JsonDialect::OpenCodeV1 => &[MCP_KEY],
        JsonDialect::OpenCodeV2 | JsonDialect::ZCode => &[MCP_KEY, SERVERS_KEY],
    }
}

pub(super) fn json_entry(root: &Value, dialect: JsonDialect) -> Option<&Value> {
    collection_path(dialect)
        .iter()
        .try_fold(root, |object, key| object.get(key))?
        .get(SERVER_NAME)
}

/// The `type` Ghostlight's entry carries in `dialect`, where the dialect needs one to start a
/// local program. Ghostlight owns it beside the command and its arguments.
pub(super) const fn json_entry_type(dialect: JsonDialect) -> Option<&'static str> {
    match dialect {
        JsonDialect::Copilot | JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => Some("local"),
        JsonDialect::Servers | JsonDialect::Crush => Some("stdio"),
        JsonDialect::McpServers | JsonDialect::ContextServers | JsonDialect::ZCode => None,
    }
}

/// The keys Ghostlight sets in its entry in `dialect` for `edge` at every install: the command,
/// its arguments and the dialect's `type`. Every key named neither here nor in
/// [`json_entry_defaults`] is the client's (ADR-0203 Decision 10.1).
pub(super) fn json_owned_keys(edge: &Path, dialect: JsonDialect) -> Vec<(&'static str, Value)> {
    let command = edge.to_string_lossy().into_owned();
    let mut keys: Vec<(&'static str, Value)> = json_entry_type(dialect)
        .map(|kind| (TYPE_KEY, json!(kind)))
        .into_iter()
        .collect();
    match dialect {
        JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => {
            keys.push((COMMAND_KEY, json!([command, MCP_ARGUMENT])));
        }
        JsonDialect::McpServers
        | JsonDialect::Copilot
        | JsonDialect::Servers
        | JsonDialect::ContextServers
        | JsonDialect::Crush
        | JsonDialect::ZCode => {
            keys.push((COMMAND_KEY, json!(command)));
            keys.push((ARGS_KEY, json!(MCP_ARGUMENTS)));
        }
    }
    keys
}

/// The keys Ghostlight writes into its entry in `dialect` only when the entry lacks them. The
/// client may change or remove them, and they never make an entry stale.
pub(super) fn json_entry_defaults(dialect: JsonDialect) -> Vec<(&'static str, Value)> {
    match dialect {
        JsonDialect::Copilot => vec![("env", json!({})), ("tools", json!(["*"]))],
        JsonDialect::McpServers | JsonDialect::ContextServers | JsonDialect::ZCode => {
            vec![("env", json!({}))]
        }
        JsonDialect::OpenCodeV1 => vec![("enabled", json!(true))],
        JsonDialect::Servers | JsonDialect::Crush | JsonDialect::OpenCodeV2 => Vec::new(),
    }
}

/// The whole entry Ghostlight writes in `dialect` for `edge` where the file holds none: every key
/// it owns and every default.
pub(super) fn expected_json_entry(edge: &Path, dialect: JsonDialect) -> Value {
    Value::Object(
        json_owned_keys(edge, dialect)
            .into_iter()
            .chain(json_entry_defaults(dialect))
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

/// Bring `existing`, the entry Ghostlight owns, whose value `found` was read, up to date for
/// `edge` in place: each owned key that differs is set, each missing owned key or default is
/// added, and every other key and comment stays as the client wrote it.
pub(super) fn update_owned_json_entry(
    existing: &CstObjectProp,
    found: &Value,
    edge: &Path,
    dialect: JsonDialect,
) -> Result<(), HarnessError> {
    let entry = existing
        .object_value()
        .ok_or(HarnessError::Malformed(None))?;
    for (key, value) in json_owned_keys(edge, dialect) {
        match entry.get(key) {
            Some(_) if found.get(key) == Some(&value) => {}
            Some(member) => member.set_value(cst_input(value)),
            None => {
                entry.append(key, cst_input(value));
            }
        }
    }
    for (key, value) in json_entry_defaults(dialect) {
        if entry.get(key).is_none() {
            entry.append(key, cst_input(value));
        }
    }
    Ok(())
}

pub(super) fn json_collection(
    root: &CstObject,
    dialect: JsonDialect,
    create: bool,
) -> Result<Option<CstObject>, HarnessError> {
    let mut object = root.clone();
    for key in collection_path(dialect) {
        let child = if create {
            object.object_value_or_create(key)
        } else {
            object.object_value(key)
        };
        match child {
            Some(child) => object = child,
            // A key that holds something other than an object cannot take Ghostlight's entry.
            None if create && object.get(key).is_some() => {
                return Err(HarnessError::Malformed(None))
            }
            None => return Ok(None),
        }
    }
    Ok(Some(object))
}

pub(super) fn cst_input(value: Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(value) => value.into(),
        Value::Number(value) => CstInputValue::Number(value.to_string()),
        Value::String(value) => CstInputValue::String(escape_cst_string(&value)),
        Value::Array(values) => CstInputValue::Array(values.into_iter().map(cst_input).collect()),
        Value::Object(values) => CstInputValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, cst_input(value)))
                .collect(),
        ),
    }
}

pub(super) fn escape_cst_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

pub(super) fn json_entry_owned(
    entry: &Value,
    dialect: JsonDialect,
    edge: &Path,
    windows: bool,
) -> bool {
    json_entry_command(entry, dialect).is_some_and(|command| {
        let args = json_entry_args(entry, dialect);
        command_owned(command, args.as_deref(), edge, windows)
    })
}

pub(super) fn json_entry_command(entry: &Value, dialect: JsonDialect) -> Option<&str> {
    match dialect {
        JsonDialect::McpServers
        | JsonDialect::Copilot
        | JsonDialect::Servers
        | JsonDialect::ContextServers
        | JsonDialect::Crush
        | JsonDialect::ZCode => entry.get(COMMAND_KEY).and_then(Value::as_str),
        JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => entry
            .get(COMMAND_KEY)
            .and_then(Value::as_array)
            .and_then(|command| command.first())
            .and_then(Value::as_str),
    }
}

pub(super) fn json_entry_args(entry: &Value, dialect: JsonDialect) -> Option<Vec<&str>> {
    let values: &[Value] = match dialect {
        JsonDialect::McpServers
        | JsonDialect::Copilot
        | JsonDialect::Servers
        | JsonDialect::ContextServers
        | JsonDialect::Crush
        | JsonDialect::ZCode => entry.get(ARGS_KEY)?.as_array()?,
        JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => {
            let command = entry.get(COMMAND_KEY)?.as_array()?;
            command.get(1..)?
        }
    };
    values.iter().map(Value::as_str).collect()
}

pub(in crate::install) fn manual_json_setup(
    edge: &Path,
    dialect: JsonDialect,
) -> Result<String, HarnessError> {
    let entry = expected_json_entry(edge, dialect);
    let document =
        collection_path(dialect)
            .iter()
            .rev()
            .fold(json!({ SERVER_NAME: entry }), |inner, key| {
                let key: &str = key;
                json!({ key: inner })
            });
    serde_json::to_string_pretty(&document).map_err(|_| HarnessError::Malformed(None))
}

pub(super) fn opencode_dialect(
    context: &HarnessContext,
    source: &str,
) -> Result<JsonDialect, HarnessError> {
    let parsed = parse_jsonc(if source.trim().is_empty() {
        "{}"
    } else {
        source
    })?;
    let mcp = parsed.get(MCP_KEY).and_then(Value::as_object);
    let v2 = mcp.is_some_and(|mcp| mcp.contains_key(SERVERS_KEY))
        || !mcp.is_some_and(|mcp| mcp.contains_key(SERVER_NAME))
            && executable_on_path(context, "opencode2")
            && !executable_on_path(context, "opencode");
    Ok(if v2 {
        JsonDialect::OpenCodeV2
    } else {
        JsonDialect::OpenCodeV1
    })
}

pub(super) fn jsonc_options() -> ParseOptions {
    ParseOptions {
        allow_comments: true,
        allow_loose_object_property_names: false,
        allow_trailing_commas: true,
    }
}

/// A JSON-with-comments parse error reduced to its line.
pub(super) fn malformed_json_at(error: jsonc_parser::errors::ParseError) -> HarnessError {
    HarnessError::Malformed(Some(error.line_display()))
}

pub(super) fn parse_jsonc(source: &str) -> Result<Value, HarnessError> {
    let value = if source.trim().is_empty() {
        Value::Object(Default::default())
    } else {
        parse_to_serde_value(source, &jsonc_options())
            .map_err(malformed_json_at)?
            .ok_or(HarnessError::Malformed(None))?
    };
    if !value.is_object() {
        return Err(HarnessError::Malformed(None));
    }
    Ok(value)
}
