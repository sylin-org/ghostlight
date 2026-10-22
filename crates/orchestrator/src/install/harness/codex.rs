//! Codex's TOML: Ghostlight's entry under `mcp_servers`, the desktop session variables it
//! forwards on Linux, and the edit that keeps every other key of the file.

use super::*;

/// The Codex table that holds every MCP server.
pub(super) const CODEX_SERVERS_KEY: &str = "mcp_servers";
/// The key of a Codex entry's own fixed environment.
pub(super) const CODEX_ENV_KEY: &str = "env";
/// The key of the variables a Codex entry forwards to its program.
pub(super) const CODEX_ENV_VARS_KEY: &str = "env_vars";
/// The key of a forwarded variable's name, when it is written as a table.
pub(super) const CODEX_FORWARD_NAME_KEY: &str = "name";
/// The key of where a forwarded variable is read from, when it is written as a table.
pub(super) const CODEX_FORWARD_SOURCE_KEY: &str = "source";
/// A forwarded variable read from Codex's own process, the default.
pub(super) const CODEX_SOURCE_LOCAL: &str = "local";
/// A forwarded variable read from Codex's remote session.
pub(super) const CODEX_SOURCE_REMOTE: &str = "remote";

// Codex filters its stdio child's environment. Linux demand-start needs the invoking
// desktop session, not captured machine-local values or a second service-only route.
pub(super) const CODEX_DESKTOP_ENVIRONMENT: &[&str] = &[
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_CURRENT_DESKTOP",
    "XAUTHORITY",
];

pub(super) fn inspect_toml(
    source: &str,
    context: &HarnessContext,
) -> Result<RegistrationState, HarnessError> {
    let document = parse_toml(source)?;
    document
        .get(CODEX_SERVERS_KEY)
        .and_then(Item::as_table)
        .and_then(|servers| servers.get(SERVER_NAME))
        .map_or(Ok(RegistrationState::Missing), |entry| {
            toml_entry_state(entry, context)
        })
}

/// The state of `entry`, Ghostlight's key in Codex's file, for the edge and the computer `context`
/// describes: the one reading inspection reports and an install acts on. Current means the
/// command and its arguments name this edge and, on Linux, the forwarding list holds every
/// desktop variable Ghostlight needs; every other key is the client's (ADR-0203 Decision 10.1).
/// Refuses an owned entry whose forwarding list Codex would not accept.
pub(super) fn toml_entry_state(
    entry: &Item,
    context: &HarnessContext,
) -> Result<RegistrationState, HarnessError> {
    let command = entry
        .get(COMMAND_KEY)
        .and_then(Item::as_str)
        .unwrap_or_default();
    let args = toml_entry_args(entry);
    let state =
        command_registration_state(command, args.as_deref(), &context.edge, context.windows);
    if !state.is_owned() {
        return Ok(state);
    }
    // Every entry this install owns is validated, so inspection refuses the same malformed
    // members an edit of that entry refuses.
    let missing = missing_codex_desktop_environment(entry, context.linux)?;
    if state == RegistrationState::Current && !missing.is_empty() {
        return Ok(RegistrationState::Updatable);
    }
    Ok(state)
}

/// The desktop session variables a Codex entry on a Linux computer (`linux`) does not yet hand
/// to its stdio child; none on any other computer. Refuses an entry whose forwarding list is not
/// one Codex accepts.
pub(super) fn missing_codex_desktop_environment(
    entry: &Item,
    linux: bool,
) -> Result<Vec<&'static str>, HarnessError> {
    if !linux {
        return Ok(Vec::new());
    }
    let forwarded = entry
        .get(CODEX_ENV_VARS_KEY)
        .map(|item| {
            let values = item.as_array().ok_or(HarnessError::Malformed(None))?;
            // Validate every member before deciding whether any handoff is missing.
            // Codex accepts a name or a closed name/source object, not arbitrary TOML.
            for value in values.iter() {
                let valid = value.as_str().is_some()
                    || value.as_inline_table().is_some_and(|table| {
                        table
                            .get(CODEX_FORWARD_NAME_KEY)
                            .and_then(toml_edit::Value::as_str)
                            .is_some()
                            && table.get(CODEX_FORWARD_SOURCE_KEY).is_none_or(|source| {
                                matches!(
                                    source.as_str(),
                                    Some(CODEX_SOURCE_LOCAL | CODEX_SOURCE_REMOTE)
                                )
                            })
                            && table.iter().all(|(key, _)| {
                                matches!(key, CODEX_FORWARD_NAME_KEY | CODEX_FORWARD_SOURCE_KEY)
                            })
                    });
                if !valid {
                    return Err(HarnessError::Malformed(None));
                }
            }
            Ok(values)
        })
        .transpose()?;
    Ok(CODEX_DESKTOP_ENVIRONMENT
        .iter()
        .copied()
        .filter(|name| {
            let explicit = entry
                .get(CODEX_ENV_KEY)
                .and_then(|env| env.get(name))
                .and_then(Item::as_str)
                .is_some();
            let local = forwarded.is_some_and(|values| {
                values.iter().any(|value| {
                    value.as_str() == Some(name)
                        || value.as_inline_table().is_some_and(|table| {
                            table
                                .get(CODEX_FORWARD_NAME_KEY)
                                .and_then(toml_edit::Value::as_str)
                                == Some(name)
                                && table
                                    .get(CODEX_FORWARD_SOURCE_KEY)
                                    .and_then(toml_edit::Value::as_str)
                                    .is_none_or(|source| source == CODEX_SOURCE_LOCAL)
                        })
                })
            });
            !explicit && !local
        })
        .collect())
}

pub(super) fn forward_codex_desktop_environment(
    entry: &mut Item,
    linux: bool,
) -> Result<(), HarnessError> {
    let missing = missing_codex_desktop_environment(entry, linux)?;
    if !missing.is_empty() {
        if entry.get(CODEX_ENV_VARS_KEY).is_none() {
            entry[CODEX_ENV_VARS_KEY] = value(Array::new());
        }
        let forwarded = entry[CODEX_ENV_VARS_KEY]
            .as_array_mut()
            .expect("the environment array was validated above");
        for name in missing {
            forwarded.push(name);
        }
    }
    Ok(())
}

/// Make the edit `intent` to Ghostlight's entry in the Codex TOML file at `path`, for the edge and
/// the computer `context` describes. Whether the file changed.
pub(super) fn edit_toml_with(
    context: &HarnessContext,
    path: &Path,
    intent: RegistrationEdit,
) -> Result<bool, HarnessError> {
    let edge = context.edge.as_path();
    let source = read_or_empty(path)?;
    let mut document = parse_toml(&source)?;
    let existing = document
        .get(CODEX_SERVERS_KEY)
        .and_then(Item::as_table)
        .and_then(|servers| servers.get(SERVER_NAME));
    if let Some(entry) = existing {
        let command = entry
            .get(COMMAND_KEY)
            .and_then(Item::as_str)
            .unwrap_or_default();
        let args = toml_entry_args(entry);
        let foreign = !command_owned(command, args.as_deref(), edge, context.windows);
        if foreign && !intent.replaces_foreign() {
            return Err(unowned_entry_refusal(Some(command), args.as_deref()));
        }
        if !foreign && intent.replaces_foreign() {
            return Err(HarnessError::NotFixable);
        }
        if intent == RegistrationEdit::Install
            && toml_entry_state(entry, context)? == RegistrationState::Current
        {
            return Ok(false);
        }
    } else if intent.replaces_foreign() {
        return Err(HarnessError::NoEntryToFix);
    } else if !intent.installs() {
        return Ok(false);
    }
    let mut updated = if intent == RegistrationEdit::Fix {
        Item::Table(Table::new())
    } else {
        existing
            .cloned()
            .unwrap_or_else(|| Item::Table(Table::new()))
    };
    if intent.installs() {
        updated[COMMAND_KEY] = value(edge.to_string_lossy().into_owned());
        updated[ARGS_KEY] = value(mcp_arguments_array());
        forward_codex_desktop_environment(&mut updated, context.linux)?;
        if !document.contains_key(CODEX_SERVERS_KEY) {
            document[CODEX_SERVERS_KEY] = Item::Table(Table::new());
        }
        let servers = document[CODEX_SERVERS_KEY]
            .as_table_mut()
            .ok_or(HarnessError::Malformed(None))?;
        servers[SERVER_NAME] = updated;
    } else if let Some(servers) = document[CODEX_SERVERS_KEY].as_table_mut() {
        servers.remove(SERVER_NAME);
        if servers.is_empty() {
            document.remove(CODEX_SERVERS_KEY);
        }
    }
    replace_with_backup(path, document.to_string().as_bytes())?;
    Ok(true)
}

pub(super) fn toml_entry_args(entry: &Item) -> Option<Vec<&str>> {
    entry
        .get(ARGS_KEY)?
        .as_array()?
        .iter()
        .map(toml_edit::Value::as_str)
        .collect()
}

/// Parse a TOML configuration. A parse error is reduced to the line its span starts on.
pub(super) fn parse_toml(source: &str) -> Result<DocumentMut, HarnessError> {
    source.parse::<DocumentMut>().map_err(|error| {
        let line = error.span().map(|span| {
            let start = span.start.min(source.len());
            1 + source.as_bytes()[..start]
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
        });
        HarnessError::Malformed(line)
    })
}
