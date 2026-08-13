//! Explicit, ownership-checked development-harness registration owned by the orchestrator.

pub mod migration;
pub mod native_host;

use std::env;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use jsonc_parser::{parse_to_serde_value, ParseOptions};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use toml_edit::{value, Array, DocumentMut, Item, Table};
use uuid::Uuid;

const SERVER_NAME: &str = "ghostlight";

/// Cached, explicit registry of supported development harness integrations.
#[derive(Clone)]
pub struct HarnessRegistry {
    inner: Arc<RegistryInner>,
}

struct RegistryInner {
    context: HarnessContext,
    summaries: Mutex<Vec<HarnessSummary>>,
    action: Mutex<()>,
}

impl HarnessRegistry {
    /// Discover the current user context and perform one initial read-only check.
    #[must_use]
    pub fn discover() -> Self {
        Self::with_context(HarnessContext::system())
    }

    fn with_context(context: HarnessContext) -> Self {
        let registry = Self {
            inner: Arc::new(RegistryInner {
                context,
                summaries: Mutex::new(Vec::new()),
                action: Mutex::new(()),
            }),
        };
        let _ = registry.refresh();
        registry
    }

    /// Return the last complete, immutable harness check.
    #[must_use]
    pub fn summaries(&self) -> Vec<HarnessSummary> {
        lock(&self.inner.summaries).clone()
    }

    /// Re-check every supported harness without changing its configuration.
    pub fn refresh(&self) -> Result<Vec<HarnessSummary>, HarnessError> {
        let summaries: Vec<HarnessSummary> = definitions(&self.inner.context)
            .into_iter()
            .map(|definition| inspect(&self.inner.context, &definition))
            .collect();
        *lock(&self.inner.summaries) = summaries.clone();
        Ok(summaries)
    }

    /// Apply one explicit check, install, or uninstall action.
    pub fn apply(
        &self,
        id: &str,
        action: HarnessAction,
    ) -> Result<HarnessActionResult, HarnessError> {
        let _action = lock(&self.inner.action);
        let definition = definitions(&self.inner.context)
            .into_iter()
            .find(|definition| definition.id == id)
            .ok_or_else(|| HarnessError::UnknownHarness(id.into()))?;
        let result = match action {
            HarnessAction::Check => HarnessActionResult {
                changed: false,
                summary: inspect(&self.inner.context, &definition),
                message: format!(
                    "{} was checked without changing configuration.",
                    definition.name
                ),
            },
            HarnessAction::Install => apply_install(&self.inner.context, &definition)?,
            HarnessAction::Uninstall => apply_uninstall(&self.inner.context, &definition)?,
        };
        let _ = self.refresh();
        Ok(result)
    }
}

/// User-visible harness management action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HarnessAction {
    /// Re-read current state only.
    Check,
    /// Add or update Ghostlight's owned registration.
    Install,
    /// Remove only Ghostlight's owned registration.
    Uninstall,
}

/// Current state of one supported harness integration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessState {
    /// The harness has not been detected in the current user context.
    NotDetected,
    /// The harness is detected without a Ghostlight registration.
    Available,
    /// The exact current Ghostlight registration is present.
    Installed,
    /// A Ghostlight-owned registration points at an older connector location.
    Updatable,
    /// A malformed or foreign entry requires deliberate manual attention.
    NeedsAttention,
}

/// Immutable user-facing integration summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HarnessSummary {
    /// Stable product-owned harness identifier.
    pub id: String,
    /// User-facing harness name.
    pub name: String,
    /// Current registration state.
    pub state: HarnessState,
    /// Fixed outcome-oriented detail.
    pub detail: String,
    /// Whether an explicit install can be attempted.
    pub can_install: bool,
    /// Whether an owned registration can be removed.
    pub can_uninstall: bool,
}

/// Definite result of one explicit harness action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HarnessActionResult {
    /// Whether a configuration file changed.
    pub changed: bool,
    /// State observed after the action.
    pub summary: HarnessSummary,
    /// Fixed user-facing outcome.
    pub message: String,
}

#[derive(Clone)]
struct HarnessContext {
    home: PathBuf,
    config: PathBuf,
    roaming: PathBuf,
    connector: PathBuf,
    path_entries: Vec<PathBuf>,
    windows: bool,
    macos: bool,
}

impl HarnessContext {
    fn system() -> Self {
        let home = env::var_os("USERPROFILE")
            .or_else(|| env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let windows = cfg!(target_os = "windows");
        let macos = cfg!(target_os = "macos");
        let config = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let roaming = if windows {
            env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData/Roaming"))
        } else if macos {
            home.join("Library/Application Support")
        } else {
            config.clone()
        };
        let connector = env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_default()
            .join(executable_name("ghostlight-mcp-connector", windows));
        let path_entries = env::var_os("PATH")
            .map(|path| env::split_paths(&path).collect())
            .unwrap_or_default();
        Self {
            home,
            config,
            roaming,
            connector,
            path_entries,
            windows,
            macos,
        }
    }
}

#[derive(Clone)]
struct HarnessDefinition {
    id: &'static str,
    name: &'static str,
    path: PathBuf,
    executables: &'static [&'static str],
    dialect: ConfigDialect,
}

#[derive(Clone, Copy)]
enum ConfigDialect {
    Json(JsonDialect),
    CodexToml,
    OpenCode,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum JsonDialect {
    McpServers,
    Servers,
    ContextServers,
    Crush,
    OpenCodeV1,
    OpenCodeV2,
}

fn definitions(context: &HarnessContext) -> Vec<HarnessDefinition> {
    let zed = if context.windows || context.macos {
        context.roaming.join("Zed/settings.json")
    } else {
        context.config.join("zed/settings.json")
    };
    let claude_desktop = context.roaming.join("Claude/claude_desktop_config.json");
    let vscode = context.roaming.join("Code/User/mcp.json");
    let opencode_jsonc = context.home.join(".config/opencode/opencode.jsonc");
    let opencode = if opencode_jsonc.exists() {
        opencode_jsonc
    } else {
        context.home.join(".config/opencode/opencode.json")
    };
    let crush = env::var_os("CRUSH_GLOBAL_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| context.home.join(".config/crush/crush.json"));
    vec![
        HarnessDefinition {
            id: "codex",
            name: "Codex",
            path: context.home.join(".codex/config.toml"),
            executables: &["codex"],
            dialect: ConfigDialect::CodexToml,
        },
        HarnessDefinition {
            id: "claude-code",
            name: "Claude Code",
            path: context.home.join(".claude.json"),
            executables: &["claude"],
            dialect: ConfigDialect::Json(JsonDialect::McpServers),
        },
        HarnessDefinition {
            id: "claude-desktop",
            name: "Claude Desktop",
            path: claude_desktop,
            executables: &[],
            dialect: ConfigDialect::Json(JsonDialect::McpServers),
        },
        HarnessDefinition {
            id: "cursor",
            name: "Cursor",
            path: context.home.join(".cursor/mcp.json"),
            executables: &["cursor", "cursor-agent"],
            dialect: ConfigDialect::Json(JsonDialect::McpServers),
        },
        HarnessDefinition {
            id: "vscode",
            name: "Visual Studio Code",
            path: vscode,
            executables: &["code"],
            dialect: ConfigDialect::Json(JsonDialect::Servers),
        },
        HarnessDefinition {
            id: "windsurf",
            name: "Windsurf",
            path: context.home.join(".codeium/windsurf/mcp_config.json"),
            executables: &["windsurf"],
            dialect: ConfigDialect::Json(JsonDialect::McpServers),
        },
        HarnessDefinition {
            id: "zed",
            name: "Zed",
            path: zed,
            executables: &["zed"],
            dialect: ConfigDialect::Json(JsonDialect::ContextServers),
        },
        HarnessDefinition {
            id: "opencode",
            name: "OpenCode",
            path: opencode,
            executables: &["opencode", "opencode2"],
            dialect: ConfigDialect::OpenCode,
        },
        HarnessDefinition {
            id: "crush",
            name: "Crush",
            path: crush,
            executables: &["crush"],
            dialect: ConfigDialect::Json(JsonDialect::Crush),
        },
    ]
}

fn inspect(context: &HarnessContext, definition: &HarnessDefinition) -> HarnessSummary {
    let detected = definition.path.exists()
        || definition.path.parent().is_some_and(Path::exists)
        || definition
            .executables
            .iter()
            .any(|name| executable_on_path(context, name));
    let connector_ready = context.connector.is_file();
    let state = match registration_state(context, definition) {
        Ok(RegistrationState::Missing) if detected => HarnessState::Available,
        Ok(RegistrationState::Missing) => HarnessState::NotDetected,
        Ok(RegistrationState::Current) => HarnessState::Installed,
        Ok(RegistrationState::Updatable) => HarnessState::Updatable,
        Ok(RegistrationState::Foreign) | Err(_) => HarnessState::NeedsAttention,
    };
    let detail = match state {
        HarnessState::Installed => "Ghostlight is registered for this user context.".into(),
        HarnessState::Updatable => {
            "Ghostlight is registered, but its connector path belongs to an older installation."
                .into()
        }
        HarnessState::Available if connector_ready => {
            "Detected and ready for an explicit Ghostlight registration.".into()
        }
        HarnessState::Available => "Detected, but the sibling MCP connector is missing.".into(),
        HarnessState::NotDetected if connector_ready => {
            "Not detected. You can prepare its user configuration before installing it.".into()
        }
        HarnessState::NotDetected => {
            "Not detected, and the sibling MCP connector is missing.".into()
        }
        HarnessState::NeedsAttention => {
            "The configuration is malformed or has a foreign ghostlight entry; it was left untouched.".into()
        }
    };
    HarnessSummary {
        id: definition.id.into(),
        name: definition.name.into(),
        state,
        detail,
        can_install: connector_ready
            && matches!(
                state,
                HarnessState::Available | HarnessState::NotDetected | HarnessState::Updatable
            ),
        can_uninstall: state == HarnessState::Installed,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistrationState {
    Missing,
    Current,
    Updatable,
    Foreign,
}

fn registration_state(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<RegistrationState, HarnessError> {
    if !definition.path.exists() {
        return Ok(RegistrationState::Missing);
    }
    let source = fs::read_to_string(&definition.path).map_err(HarnessError::Read)?;
    match definition.dialect {
        ConfigDialect::CodexToml => inspect_toml(&source, &context.connector, context.windows),
        ConfigDialect::Json(dialect) => {
            inspect_json(&source, dialect, &context.connector, context.windows)
        }
        ConfigDialect::OpenCode => inspect_json(
            &source,
            opencode_dialect(context, &source)?,
            &context.connector,
            context.windows,
        ),
    }
}

fn inspect_json(
    source: &str,
    dialect: JsonDialect,
    connector: &Path,
    windows: bool,
) -> Result<RegistrationState, HarnessError> {
    let parsed = parse_jsonc(source)?;
    let Some(entry) = json_entry(&parsed, dialect) else {
        return Ok(RegistrationState::Missing);
    };
    Ok(
        json_entry_command(entry, dialect).map_or(RegistrationState::Foreign, |command| {
            command_registration_state(command, connector, windows)
        }),
    )
}

fn inspect_toml(
    source: &str,
    connector: &Path,
    windows: bool,
) -> Result<RegistrationState, HarnessError> {
    let document = source
        .parse::<DocumentMut>()
        .map_err(|error| HarnessError::Malformed(error.to_string()))?;
    let Some(entry) = document
        .get("mcp_servers")
        .and_then(Item::as_table)
        .and_then(|servers| servers.get(SERVER_NAME))
    else {
        return Ok(RegistrationState::Missing);
    };
    let command = entry
        .get("command")
        .and_then(Item::as_str)
        .unwrap_or_default();
    Ok(command_registration_state(command, connector, windows))
}

fn apply_install(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<HarnessActionResult, HarnessError> {
    if !context.connector.is_file() {
        return Err(HarnessError::ConnectorMissing);
    }
    let changed = match definition.dialect {
        ConfigDialect::CodexToml => edit_toml(&definition.path, &context.connector, true)?,
        ConfigDialect::Json(dialect) => {
            edit_json(&definition.path, &context.connector, dialect, true)?
        }
        ConfigDialect::OpenCode => {
            let source = read_or_empty(&definition.path)?;
            let dialect = opencode_dialect(context, &source)?;
            edit_json(&definition.path, &context.connector, dialect, true)?
        }
    };
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        summary,
        message: if changed {
            format!(
                "Ghostlight was registered with {}. Restart or reconnect it to load the tools.",
                definition.name
            )
        } else {
            format!(
                "{} already has the current Ghostlight registration.",
                definition.name
            )
        },
    })
}

fn apply_uninstall(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<HarnessActionResult, HarnessError> {
    let changed = match definition.dialect {
        ConfigDialect::CodexToml => edit_toml(&definition.path, &context.connector, false)?,
        ConfigDialect::Json(dialect) => {
            edit_json(&definition.path, &context.connector, dialect, false)?
        }
        ConfigDialect::OpenCode => {
            let source = read_or_empty(&definition.path)?;
            let dialect = opencode_dialect(context, &source)?;
            edit_json(&definition.path, &context.connector, dialect, false)?
        }
    };
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        summary,
        message: if changed {
            format!(
                "Ghostlight was removed from {}. Restart or reconnect it to unload the tools.",
                definition.name
            )
        } else {
            format!(
                "{} had no owned Ghostlight registration to remove.",
                definition.name
            )
        },
    })
}

fn edit_json(
    path: &Path,
    connector: &Path,
    dialect: JsonDialect,
    install: bool,
) -> Result<bool, HarnessError> {
    let source = read_or_empty(path)?;
    let parsed = parse_jsonc(&source)?;
    let current = json_entry(&parsed, dialect);
    if let Some(entry) = current {
        if !json_entry_owned(entry, dialect) {
            return Err(HarnessError::ForeignEntry);
        }
        if install && entry == &expected_json_entry(connector, dialect) {
            return Ok(false);
        }
    } else if !install {
        return Ok(false);
    }
    let root = CstRootNode::parse(&source, &jsonc_options())
        .map_err(|error| HarnessError::Malformed(error.to_string()))?;
    let object = root
        .object_value_or_create()
        .ok_or_else(|| HarnessError::Malformed("configuration root is not an object".into()))?;
    let collection = json_collection(&object, dialect, install)?;
    if install {
        let collection = collection.ok_or_else(|| {
            HarnessError::Malformed("could not create the harness server collection".into())
        })?;
        let entry = cst_input(expected_json_entry(connector, dialect));
        if let Some(existing) = collection.get(SERVER_NAME) {
            existing.set_value(entry);
        } else {
            collection.append(SERVER_NAME, entry);
        }
    } else if let Some(entry) = collection.and_then(|collection| collection.get(SERVER_NAME)) {
        entry.remove();
    }
    let rendered = root.to_string();
    replace_with_backup(path, rendered.as_bytes())?;
    Ok(true)
}

fn edit_toml(path: &Path, connector: &Path, install: bool) -> Result<bool, HarnessError> {
    let source = read_or_empty(path)?;
    let mut document = source
        .parse::<DocumentMut>()
        .map_err(|error| HarnessError::Malformed(error.to_string()))?;
    let existing = document
        .get("mcp_servers")
        .and_then(Item::as_table)
        .and_then(|servers| servers.get(SERVER_NAME));
    if let Some(entry) = existing {
        let command = entry
            .get("command")
            .and_then(Item::as_str)
            .unwrap_or_default();
        if !command_owned(command) {
            return Err(HarnessError::ForeignEntry);
        }
        if install
            && command == connector.to_string_lossy()
            && entry
                .get("args")
                .and_then(Item::as_array)
                .is_some_and(Array::is_empty)
        {
            return Ok(false);
        }
    } else if !install {
        return Ok(false);
    }
    if install {
        if !document.contains_key("mcp_servers") {
            document["mcp_servers"] = Item::Table(Table::new());
        }
        let servers = document["mcp_servers"]
            .as_table_mut()
            .ok_or_else(|| HarnessError::Malformed("mcp_servers is not a TOML table".into()))?;
        let mut entry = Table::new();
        entry["command"] = value(connector.to_string_lossy().into_owned());
        entry["args"] = value(Array::new());
        servers[SERVER_NAME] = Item::Table(entry);
    } else if let Some(servers) = document["mcp_servers"].as_table_mut() {
        servers.remove(SERVER_NAME);
        if servers.is_empty() {
            document.remove("mcp_servers");
        }
    }
    replace_with_backup(path, document.to_string().as_bytes())?;
    Ok(true)
}

fn json_entry(root: &Value, dialect: JsonDialect) -> Option<&Value> {
    match dialect {
        JsonDialect::McpServers => root.get("mcpServers")?.get(SERVER_NAME),
        JsonDialect::Servers => root.get("servers")?.get(SERVER_NAME),
        JsonDialect::ContextServers => root.get("context_servers")?.get(SERVER_NAME),
        JsonDialect::Crush => root.get("mcp")?.get(SERVER_NAME),
        JsonDialect::OpenCodeV1 => root.get("mcp")?.get(SERVER_NAME),
        JsonDialect::OpenCodeV2 => root.get("mcp")?.get("servers")?.get(SERVER_NAME),
    }
}

fn expected_json_entry(connector: &Path, dialect: JsonDialect) -> Value {
    let command = connector.to_string_lossy().into_owned();
    match dialect {
        JsonDialect::Servers => json!({"type":"stdio","command":command,"args":[]}),
        JsonDialect::Crush => json!({"type":"stdio","command":command,"args":[]}),
        JsonDialect::McpServers | JsonDialect::ContextServers => {
            json!({"command":command,"args":[],"env":{}})
        }
        JsonDialect::OpenCodeV1 => {
            json!({"type":"local","command":[command],"enabled":true})
        }
        JsonDialect::OpenCodeV2 => json!({"type":"local","command":[command]}),
    }
}

fn json_collection(
    root: &CstObject,
    dialect: JsonDialect,
    create: bool,
) -> Result<Option<CstObject>, HarnessError> {
    if dialect == JsonDialect::OpenCodeV2 {
        let mcp = if create {
            root.object_value_or_create("mcp")
        } else {
            root.object_value("mcp")
        };
        let Some(mcp) = mcp else {
            return if create && root.get("mcp").is_some() {
                Err(HarnessError::Malformed("mcp is not an object".into()))
            } else {
                Ok(None)
            };
        };
        let servers = if create {
            mcp.object_value_or_create("servers")
        } else {
            mcp.object_value("servers")
        };
        return if create && servers.is_none() && mcp.get("servers").is_some() {
            Err(HarnessError::Malformed(
                "mcp.servers is not an object".into(),
            ))
        } else {
            Ok(servers)
        };
    }
    let key = match dialect {
        JsonDialect::McpServers => "mcpServers",
        JsonDialect::Servers => "servers",
        JsonDialect::ContextServers => "context_servers",
        JsonDialect::Crush | JsonDialect::OpenCodeV1 => "mcp",
        JsonDialect::OpenCodeV2 => unreachable!("handled above"),
    };
    let collection = if create {
        root.object_value_or_create(key)
    } else {
        root.object_value(key)
    };
    if create && collection.is_none() && root.get(key).is_some() {
        return Err(HarnessError::Malformed(format!("{key} is not an object")));
    }
    Ok(collection)
}

fn cst_input(value: Value) -> CstInputValue {
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

fn escape_cst_string(value: &str) -> String {
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

fn json_entry_owned(entry: &Value, dialect: JsonDialect) -> bool {
    json_entry_command(entry, dialect).is_some_and(command_owned)
}

fn json_entry_command(entry: &Value, dialect: JsonDialect) -> Option<&str> {
    match dialect {
        JsonDialect::McpServers
        | JsonDialect::Servers
        | JsonDialect::ContextServers
        | JsonDialect::Crush => entry.get("command").and_then(Value::as_str),
        JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => entry
            .get("command")
            .and_then(Value::as_array)
            .and_then(|command| command.first())
            .and_then(Value::as_str),
    }
}

fn command_owned(command: &str) -> bool {
    Path::new(command)
        .file_stem()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.eq_ignore_ascii_case("ghostlight-mcp-connector"))
}

fn command_registration_state(command: &str, connector: &Path, windows: bool) -> RegistrationState {
    if !command_owned(command) {
        return RegistrationState::Foreign;
    }
    let actual = fs::canonicalize(command).unwrap_or_else(|_| PathBuf::from(command));
    let expected = fs::canonicalize(connector).unwrap_or_else(|_| connector.to_path_buf());
    let actual = actual.to_string_lossy();
    let expected = expected.to_string_lossy();
    let current = if windows {
        actual.eq_ignore_ascii_case(&expected)
    } else {
        actual == expected
    };
    if current {
        RegistrationState::Current
    } else {
        RegistrationState::Updatable
    }
}

fn opencode_dialect(context: &HarnessContext, source: &str) -> Result<JsonDialect, HarnessError> {
    let parsed = parse_jsonc(if source.trim().is_empty() {
        "{}"
    } else {
        source
    })?;
    let mcp = parsed.get("mcp").and_then(Value::as_object);
    let v2 = mcp.is_some_and(|mcp| mcp.contains_key("servers"))
        || !mcp.is_some_and(|mcp| mcp.contains_key(SERVER_NAME))
            && executable_on_path(context, "opencode2")
            && !executable_on_path(context, "opencode");
    Ok(if v2 {
        JsonDialect::OpenCodeV2
    } else {
        JsonDialect::OpenCodeV1
    })
}

fn jsonc_options() -> ParseOptions {
    ParseOptions {
        allow_comments: true,
        allow_loose_object_property_names: false,
        allow_trailing_commas: true,
    }
}

fn parse_jsonc(source: &str) -> Result<Value, HarnessError> {
    let value = if source.trim().is_empty() {
        Value::Object(Default::default())
    } else {
        parse_to_serde_value(source, &jsonc_options())
            .map_err(|error| HarnessError::Malformed(error.to_string()))?
            .ok_or_else(|| HarnessError::Malformed("configuration has no value".into()))?
    };
    if !value.is_object() {
        return Err(HarnessError::Malformed(
            "configuration root is not an object".into(),
        ));
    }
    Ok(value)
}

fn replace_with_backup(path: &Path, bytes: &[u8]) -> Result<(), HarnessError> {
    let parent = path
        .parent()
        .ok_or_else(|| HarnessError::InvalidPath(path.to_path_buf()))?;
    fs::create_dir_all(parent).map_err(HarnessError::Write)?;
    let temporary = parent.join(format!(".ghostlight-{}.tmp", Uuid::new_v4().simple()));
    let backup = path.with_extension(format!(
        "{}.ghostlight-backup",
        path.extension().and_then(OsStr::to_str).unwrap_or("config")
    ));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    let mut file = options.open(&temporary).map_err(HarnessError::Write)?;
    file.write_all(bytes).map_err(HarnessError::Write)?;
    file.sync_all().map_err(HarnessError::Write)?;
    if path.exists() {
        fs::copy(path, &backup).map_err(HarnessError::Write)?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if path.exists() {
            fs::remove_file(path).map_err(HarnessError::Write)?;
            fs::rename(&temporary, path).map_err(HarnessError::Write)?;
        } else {
            let _ = fs::remove_file(&temporary);
            return Err(HarnessError::Write(error));
        }
    }
    Ok(())
}

fn read_or_empty(path: &Path) -> Result<String, HarnessError> {
    match fs::read_to_string(path) {
        Ok(source) => Ok(source),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(HarnessError::Read(error)),
    }
}

fn executable_on_path(context: &HarnessContext, name: &str) -> bool {
    let executable = executable_name(name, context.windows);
    context
        .path_entries
        .iter()
        .any(|directory| directory.join(&executable).is_file())
}

fn executable_name(name: &str, windows: bool) -> String {
    if windows {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Safe harness registration failure.
#[derive(Debug, Error)]
pub enum HarnessError {
    /// Unsupported identifier supplied by the presentation adapter.
    #[error("unknown supported harness `{0}`")]
    UnknownHarness(String),
    /// The packaged sibling connector is unavailable.
    #[error("the sibling Ghostlight MCP connector is missing")]
    ConnectorMissing,
    /// The harness config could not be read.
    #[error("could not read harness configuration: {0}")]
    Read(io::Error),
    /// The harness config could not be written.
    #[error("could not write harness configuration: {0}")]
    Write(io::Error),
    /// The config is not structurally safe to edit.
    #[error("harness configuration is malformed: {0}")]
    Malformed(String),
    /// A foreign server uses Ghostlight's registration name.
    #[error("a foreign `ghostlight` entry is present; Ghostlight left it untouched")]
    ForeignEntry,
    /// A path has no writable parent.
    #[error("harness configuration path has no parent: {0}")]
    InvalidPath(PathBuf),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use super::{
        edit_json, edit_toml, inspect_json, inspect_toml, HarnessError, JsonDialect,
        RegistrationState,
    };

    fn temporary(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "ghostlight-harness-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn connector(directory: &Path) -> PathBuf {
        let connector = directory.join(if cfg!(windows) {
            "ghostlight-mcp-connector.exe"
        } else {
            "ghostlight-mcp-connector"
        });
        fs::write(&connector, b"test").unwrap();
        connector
    }

    #[test]
    fn json_install_is_idempotent_and_preserves_siblings() {
        let directory = temporary("json");
        let path = directory.join("mcp.json");
        let connector = connector(&directory);
        fs::write(
            &path,
            r#"{"other":7,"mcpServers":{"sibling":{"command":"sibling"}}}"#,
        )
        .unwrap();
        assert!(edit_json(&path, &connector, JsonDialect::McpServers, true).unwrap());
        assert!(!edit_json(&path, &connector, JsonDialect::McpServers, true).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["other"], 7);
        assert_eq!(value["mcpServers"]["sibling"]["command"], "sibling");
        assert_eq!(
            inspect_json(
                &fs::read_to_string(&path).unwrap(),
                JsonDialect::McpServers,
                &connector,
                cfg!(windows)
            )
            .unwrap(),
            RegistrationState::Current
        );
        assert!(edit_json(&path, &connector, JsonDialect::McpServers, false).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(value["mcpServers"].get("ghostlight").is_none());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn jsonc_install_and_uninstall_preserve_comments_and_trailing_commas() {
        let directory = temporary("jsonc");
        let path = directory.join("settings.json");
        let connector = connector(&directory);
        let source = "{\n  // keep this thought\n  \"context_servers\": {},\n}\n";
        fs::write(&path, source).unwrap();
        assert!(edit_json(&path, &connector, JsonDialect::ContextServers, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        assert!(installed.contains("// keep this thought"));
        assert!(installed.contains("\"ghostlight\""));
        assert_eq!(
            inspect_json(
                &installed,
                JsonDialect::ContextServers,
                &connector,
                cfg!(windows)
            )
            .unwrap(),
            RegistrationState::Current
        );
        assert!(edit_json(&path, &connector, JsonDialect::ContextServers, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("// keep this thought"));
        assert!(!removed.contains("\"ghostlight\""));
        assert!(removed.contains("\"context_servers\": {},"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn foreign_entries_are_never_overwritten_or_removed() {
        let directory = temporary("foreign");
        let path = directory.join("mcp.json");
        let connector = connector(&directory);
        let source = r#"{"mcpServers":{"ghostlight":{"command":"some-other-tool"}}}"#;
        fs::write(&path, source).unwrap();
        for install in [true, false] {
            assert!(matches!(
                edit_json(&path, &connector, JsonDialect::McpServers, install),
                Err(HarnessError::ForeignEntry)
            ));
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn an_owned_old_connector_path_is_updatable_not_installed() {
        let directory = temporary("updatable");
        let current = connector(&directory);
        let old = directory.join("old").join(if cfg!(windows) {
            "ghostlight-mcp-connector.exe"
        } else {
            "ghostlight-mcp-connector"
        });
        let source = serde_json::json!({
            "mcpServers": {
                "ghostlight": {
                    "command": old,
                    "args": [],
                    "env": {}
                }
            }
        })
        .to_string();
        assert_eq!(
            inspect_json(&source, JsonDialect::McpServers, &current, cfg!(windows)).unwrap(),
            RegistrationState::Updatable
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn codex_toml_edit_preserves_unrelated_comments() {
        let directory = temporary("toml");
        let path = directory.join("config.toml");
        let connector = connector(&directory);
        fs::write(&path, "# keep me\nmodel = \"gpt-test\"\n").unwrap();
        assert!(edit_toml(&path, &connector, true).unwrap());
        assert!(!edit_toml(&path, &connector, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        assert!(installed.contains("# keep me"));
        assert_eq!(
            inspect_toml(&installed, &connector, cfg!(windows)).unwrap(),
            RegistrationState::Current
        );
        assert!(edit_toml(&path, &connector, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("# keep me"));
        assert!(!removed.contains("mcp_servers"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn opencode_v1_uses_the_local_command_array_contract() {
        let directory = temporary("opencode-v1");
        let path = directory.join("opencode.json");
        let connector = connector(&directory);
        fs::write(&path, r#"{"theme":"ghost","mcp":{}}"#).unwrap();

        assert!(edit_json(&path, &connector, JsonDialect::OpenCodeV1, true).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["theme"], "ghost");
        assert_eq!(value["mcp"]["ghostlight"]["type"], "local");
        assert_eq!(
            value["mcp"]["ghostlight"]["command"][0].as_str(),
            Some(connector.to_string_lossy().as_ref())
        );
        assert_eq!(value["mcp"]["ghostlight"]["enabled"], true);
        assert_eq!(
            inspect_json(
                &fs::read_to_string(&path).unwrap(),
                JsonDialect::OpenCodeV1,
                &connector,
                cfg!(windows)
            )
            .unwrap(),
            RegistrationState::Current
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn opencode_v2_preserves_the_nested_servers_contract() {
        let directory = temporary("opencode-v2");
        let path = directory.join("opencode.json");
        let connector = connector(&directory);
        fs::write(
            &path,
            r#"{"mcp":{"servers":{"sibling":{"type":"remote"}}}}"#,
        )
        .unwrap();

        assert!(edit_json(&path, &connector, JsonDialect::OpenCodeV2, true).unwrap());
        assert!(!edit_json(&path, &connector, JsonDialect::OpenCodeV2, true).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["mcp"]["servers"]["sibling"]["type"], "remote");
        assert_eq!(value["mcp"]["servers"]["ghostlight"]["type"], "local");
        assert!(value["mcp"]["servers"]["ghostlight"]
            .get("enabled")
            .is_none());
        assert!(edit_json(&path, &connector, JsonDialect::OpenCodeV2, false).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(value["mcp"]["servers"].get("ghostlight").is_none());
        assert_eq!(value["mcp"]["servers"]["sibling"]["type"], "remote");

        fs::remove_dir_all(directory).unwrap();
    }
}
