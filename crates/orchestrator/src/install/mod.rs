//! Explicit, ownership-checked development-harness registration owned by the orchestrator.

pub mod browser_package;
pub mod desktop_entry;
pub mod handoff;
pub mod migration;
pub mod native_host;

use std::collections::BTreeMap;
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

    /// Return product-owned text for the desktop clipboard boundary.
    pub fn copy_text(&self, id: &str, kind: HarnessCopyKind) -> Result<String, HarnessError> {
        let definition = definition(&self.inner.context, id)?;
        Ok(match kind {
            HarnessCopyKind::Command => self.inner.context.connector.to_string_lossy().into_owned(),
            HarnessCopyKind::Setup => manual_setup(&self.inner.context, &definition)?,
        })
    }

    /// Return the closed official download destination for one supported product.
    pub fn download_url(&self, product_id: &str) -> Result<&'static str, HarnessError> {
        definitions(&self.inner.context)
            .into_iter()
            .find(|definition| definition.product_id == product_id)
            .and_then(|definition| definition.download_url)
            .ok_or_else(|| HarnessError::NoDownload(product_id.into()))
    }

    /// Persist one user-selected executable or configuration path after validating it.
    pub fn locate(&self, id: &str, path: &Path) -> Result<HarnessActionResult, HarnessError> {
        let _action = lock(&self.inner.action);
        let definition = definition(&self.inner.context, id)?;
        if !path.is_file() {
            return Err(HarnessError::LocatedPathInvalid(path.to_path_buf()));
        }
        let mut overrides = read_location_overrides(&self.inner.context)?;
        let located = overrides.entry(id.into()).or_default();
        if definition.accepts_config_path(path) {
            validate_located_config(path, definition.dialect)?;
            located.config = Some(path.to_path_buf());
        } else if is_executable_file(path, self.inner.context.windows) {
            located.executable = Some(path.to_path_buf());
        } else {
            return Err(HarnessError::LocatedPathInvalid(path.to_path_buf()));
        }
        write_location_overrides(&self.inner.context, &overrides)?;
        let summaries = self.refresh()?;
        let summary = summaries
            .into_iter()
            .find(|summary| summary.id == id)
            .ok_or_else(|| HarnessError::UnknownHarness(id.into()))?;
        Ok(HarnessActionResult {
            changed: true,
            summary,
            message: format!(
                "Ghostlight will use the located {} path after you choose Set up.",
                definition.name
            ),
        })
    }
}

fn definition(context: &HarnessContext, id: &str) -> Result<HarnessDefinition, HarnessError> {
    definitions(context)
        .into_iter()
        .find(|definition| definition.id == id)
        .ok_or_else(|| HarnessError::UnknownHarness(id.into()))
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

/// Product-owned clipboard material exposed by the desktop adapter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HarnessCopyKind {
    /// The absolute installed MCP connector path.
    Command,
    /// The smallest complete target-specific configuration document.
    Setup,
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
    /// Stable product id shared by coexisting targets.
    pub product_id: String,
    /// User-facing harness name.
    pub name: String,
    /// User-facing concrete target label.
    pub target: String,
    /// Bundled integration artwork file name.
    pub icon: String,
    /// Current registration state.
    pub state: HarnessState,
    /// Fixed outcome-oriented detail.
    pub detail: String,
    /// Whether an explicit install can be attempted.
    pub can_install: bool,
    /// Whether an owned registration can be removed.
    pub can_uninstall: bool,
    /// Whether the product has a closed official download destination on this platform.
    pub can_download: bool,
    /// Whether the native Locate action is available.
    pub can_locate: bool,
    /// Exact configuration destination currently resolved for this target.
    pub config_path: String,
    /// Exact installed connector command available for manual setup.
    pub connector_command: String,
    /// Smallest complete manual configuration document for this target.
    pub manual_setup: String,
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
    codex_config: PathBuf,
    connector: PathBuf,
    path_entries: Vec<PathBuf>,
    locations: PathBuf,
    windows: bool,
}

impl HarnessContext {
    fn system() -> Self {
        let windows = cfg!(target_os = "windows");
        let home = if windows {
            env::var_os("USERPROFILE").or_else(|| env::var_os("HOME"))
        } else {
            env::var_os("HOME")
        }
        .map(PathBuf::from)
        .unwrap_or_default();
        let roots = harness_roots(
            &home,
            env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            env::var_os("APPDATA").map(PathBuf::from),
            env::var_os("CODEX_HOME").map(PathBuf::from),
            windows,
        );
        let connector = env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_default()
            .join(executable_name("ghostlight-mcp-connector", windows));
        let path_entries = env::var_os("PATH")
            .map(|path| env::split_paths(&path).collect())
            .unwrap_or_default();
        let locations = roots.config.join("ghostlight/harness-locations.json");
        Self {
            home,
            config: roots.config,
            roaming: roots.roaming,
            codex_config: roots.codex_config,
            connector,
            path_entries,
            locations,
            windows,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HarnessRoots {
    config: PathBuf,
    roaming: PathBuf,
    codex_config: PathBuf,
}

fn harness_roots(
    home: &Path,
    xdg_config_home: Option<PathBuf>,
    app_data: Option<PathBuf>,
    codex_home: Option<PathBuf>,
    windows: bool,
) -> HarnessRoots {
    let config = if windows {
        app_data
            .clone()
            .unwrap_or_else(|| home.join("AppData/Roaming"))
    } else {
        xdg_config_home.unwrap_or_else(|| home.join(".config"))
    };
    let roaming = if windows {
        app_data.unwrap_or_else(|| home.join("AppData/Roaming"))
    } else {
        config.clone()
    };
    let codex_config = codex_home
        .unwrap_or_else(|| home.join(".codex"))
        .join("config.toml");
    HarnessRoots {
        config,
        roaming,
        codex_config,
    }
}

#[derive(Clone)]
struct HarnessDefinition {
    id: &'static str,
    product_id: &'static str,
    name: &'static str,
    target: &'static str,
    icon: &'static str,
    download_url: Option<&'static str>,
    path: PathBuf,
    default_path: PathBuf,
    located_executable: Option<PathBuf>,
    located_stale: bool,
    executables: &'static [&'static str],
    dialect: ConfigDialect,
}

impl HarnessDefinition {
    fn accepts_config_path(&self, path: &Path) -> bool {
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            return false;
        };
        match self.id {
            "opencode" => matches!(name, "opencode.json" | "opencode.jsonc"),
            "kilo-code" => matches!(
                name,
                "kilo.json" | "kilo.jsonc" | "opencode.json" | "opencode.jsonc"
            ),
            _ => self.default_path.file_name() == Some(OsStr::new(name)),
        }
    }
}

#[derive(Clone, Copy)]
enum ConfigDialect {
    Json(JsonDialect),
    Yaml(YamlDialect),
    CodexToml,
    OpenCode,
}

#[derive(Clone, Copy)]
enum YamlDialect {
    Goose,
    Continue,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum JsonDialect {
    McpServers,
    Copilot,
    Servers,
    ContextServers,
    Crush,
    OpenCodeV1,
    OpenCodeV2,
}

fn definitions(context: &HarnessContext) -> Vec<HarnessDefinition> {
    let zed = if context.windows {
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
    let kilo = ["kilo.jsonc", "kilo.json", "opencode.jsonc", "opencode.json"]
        .into_iter()
        .map(|name| context.config.join("kilo").join(name))
        .find(|path| path.exists())
        .unwrap_or_else(|| context.config.join("kilo").join("kilo.json"));
    let cline_file = "settings/cline_mcp_settings.json";
    let mut rows = vec![
        row(
            "codex",
            "codex",
            "Codex",
            "User",
            "codex.svg",
            Some("https://developers.openai.com/codex/cli/"),
            context.codex_config.clone(),
            &["codex"],
            ConfigDialect::CodexToml,
        ),
        row(
            "claude-code",
            "claude-code",
            "Claude Code",
            "User",
            "claude-code.svg",
            Some("https://claude.com/product/claude-code"),
            context.home.join(".claude.json"),
            &["claude"],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "claude-desktop",
            "claude-desktop",
            "Claude Desktop",
            "User",
            "claude-desktop.svg",
            None,
            claude_desktop,
            &[],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "cursor",
            "cursor",
            "Cursor",
            "User",
            "cursor.svg",
            Some("https://cursor.com/downloads"),
            context.home.join(".cursor/mcp.json"),
            &["cursor", "cursor-agent"],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "vscode",
            "vscode",
            "Visual Studio Code",
            "User",
            "vscode.svg",
            Some("https://code.visualstudio.com/download"),
            vscode,
            &["code"],
            ConfigDialect::Json(JsonDialect::Servers),
        ),
        row(
            "windsurf",
            "windsurf",
            "Windsurf",
            "User",
            "windsurf.svg",
            Some("https://windsurf.com/editor/download"),
            context.home.join(".codeium/windsurf/mcp_config.json"),
            &["windsurf"],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "zed",
            "zed",
            "Zed",
            "User",
            "zed.svg",
            Some("https://zed.dev/download"),
            zed,
            &["zed", "zeditor"],
            ConfigDialect::Json(JsonDialect::ContextServers),
        ),
        row(
            "opencode",
            "opencode",
            "OpenCode",
            "User",
            "opencode.svg",
            Some("https://opencode.ai/docs"),
            opencode,
            &["opencode", "opencode2"],
            ConfigDialect::OpenCode,
        ),
        row(
            "crush",
            "crush",
            "Crush",
            "User",
            "crush.svg",
            Some("https://github.com/charmbracelet/crush"),
            crush,
            &["crush"],
            ConfigDialect::Json(JsonDialect::Crush),
        ),
        row(
            "copilot-cli",
            "copilot-cli",
            "GitHub Copilot CLI",
            "CLI",
            "copilot-cli.svg",
            Some("https://github.com/github/copilot-cli"),
            context.home.join(".copilot/mcp-config.json"),
            &["copilot"],
            ConfigDialect::Json(JsonDialect::Copilot),
        ),
        row(
            "cline-cli",
            "cline",
            "Cline",
            "CLI",
            "cline.svg",
            Some("https://docs.cline.bot/cline-cli/overview"),
            context
                .home
                .join(".cline/data/settings/cline_mcp_settings.json"),
            &["cline"],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "cline-vscode",
            "cline",
            "Cline",
            "Visual Studio Code",
            "cline.svg",
            Some("https://marketplace.visualstudio.com/items?itemName=saoudrizwan.claude-dev"),
            context
                .roaming
                .join("Code/User/globalStorage/saoudrizwan.claude-dev")
                .join(cline_file),
            &[],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "cline-cursor",
            "cline",
            "Cline",
            "Cursor",
            "cline.svg",
            Some("https://docs.cline.bot/getting-started/installing-cline"),
            context
                .roaming
                .join("Cursor/User/globalStorage/saoudrizwan.claude-dev")
                .join(cline_file),
            &[],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "cline-windsurf",
            "cline",
            "Cline",
            "Windsurf",
            "cline.svg",
            Some("https://docs.cline.bot/getting-started/installing-cline"),
            context
                .roaming
                .join("Windsurf/User/globalStorage/saoudrizwan.claude-dev")
                .join(cline_file),
            &[],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "kiro",
            "kiro",
            "Kiro",
            "CLI and IDE",
            "kiro.svg",
            Some("https://kiro.dev/downloads/"),
            context.home.join(".kiro/settings/mcp.json"),
            &["kiro", "kiro-cli"],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "qwen-code",
            "qwen-code",
            "Qwen Code",
            "CLI",
            "qwen-code.svg",
            Some("https://qwenlm.github.io/qwen-code-docs/en/users/installation/"),
            context.home.join(".qwen/settings.json"),
            &["qwen"],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "junie",
            "junie",
            "Junie",
            "CLI and JetBrains",
            "junie.svg",
            Some("https://junie.jetbrains.com/docs/get-started-with-junie-cli.html"),
            context.home.join(".junie/mcp/mcp.json"),
            &["junie"],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
        row(
            "kilo-code",
            "kilo-code",
            "Kilo Code",
            "CLI",
            "kilo-code.svg",
            Some("https://kilo.ai/"),
            kilo,
            &["kilo"],
            ConfigDialect::Json(JsonDialect::OpenCodeV1),
        ),
        row(
            "goose",
            "goose",
            "goose",
            "CLI and desktop",
            "goose.svg",
            Some("https://block.github.io/goose/docs/getting-started/installation/"),
            context.config.join("goose/config.yaml"),
            &["goose"],
            ConfigDialect::Yaml(YamlDialect::Goose),
        ),
        row(
            "continue",
            "continue",
            "Continue",
            "CLI and IDE",
            "continue.svg",
            Some("https://docs.continue.dev/cli/quickstart"),
            context.home.join(".continue/config.yaml"),
            &["cn"],
            ConfigDialect::Yaml(YamlDialect::Continue),
        ),
        row(
            "antigravity",
            "antigravity",
            "Antigravity",
            "CLI",
            "antigravity.svg",
            Some("https://antigravity.google/download?platform=linux"),
            context.home.join(".gemini/config/mcp_config.json"),
            &["agy"],
            ConfigDialect::Json(JsonDialect::McpServers),
        ),
    ];
    let overrides = read_location_overrides(context).unwrap_or_default();
    for definition in &mut rows {
        if let Some(located) = overrides.get(definition.id) {
            if let Some(path) = &located.config {
                if path.is_file() {
                    definition.path = path.clone();
                } else {
                    definition.located_stale = true;
                }
            }
            if let Some(path) = &located.executable {
                if is_executable_file(path, context.windows) {
                    definition.located_executable = Some(path.clone());
                } else {
                    definition.located_stale = true;
                }
            }
        }
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn row(
    id: &'static str,
    product_id: &'static str,
    name: &'static str,
    target: &'static str,
    icon: &'static str,
    download_url: Option<&'static str>,
    path: PathBuf,
    executables: &'static [&'static str],
    dialect: ConfigDialect,
) -> HarnessDefinition {
    HarnessDefinition {
        id,
        product_id,
        name,
        target,
        icon,
        download_url,
        default_path: path.clone(),
        path,
        located_executable: None,
        located_stale: false,
        executables,
        dialect,
    }
}

fn inspect(context: &HarnessContext, definition: &HarnessDefinition) -> HarnessSummary {
    let detected = definition.path.exists()
        || definition.path.parent().is_some_and(Path::exists)
        || definition
            .located_executable
            .as_ref()
            .is_some_and(|path| path.is_file())
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
    let mut detail: String = match state {
        HarnessState::Installed => "Ghostlight is registered for this user context.".into(),
        HarnessState::Updatable => {
            "Ghostlight is registered through an older Ghostlight installation or executable."
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
    if definition.located_stale {
        detail.push_str(" A previously located path is missing; the normal location is shown.");
    }
    HarnessSummary {
        id: definition.id.into(),
        product_id: definition.product_id.into(),
        name: definition.name.into(),
        target: definition.target.into(),
        icon: definition.icon.into(),
        state,
        detail,
        can_install: connector_ready
            && matches!(
                state,
                HarnessState::Available | HarnessState::NotDetected | HarnessState::Updatable
            ),
        can_uninstall: state == HarnessState::Installed,
        can_download: definition.download_url.is_some(),
        can_locate: true,
        config_path: definition.path.to_string_lossy().into_owned(),
        connector_command: context.connector.to_string_lossy().into_owned(),
        manual_setup: manual_setup(context, definition).unwrap_or_else(|_| {
            format!(
                "Use this command as a local stdio MCP server:\n{}",
                context.connector.display()
            )
        }),
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
        ConfigDialect::Yaml(dialect) => {
            inspect_yaml(&source, dialect, &context.connector, context.windows)
        }
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
            let args = json_entry_args(entry, dialect);
            command_registration_state(command, args.as_deref(), connector, windows)
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
    let args = toml_entry_args(entry);
    Ok(command_registration_state(
        command,
        args.as_deref(),
        connector,
        windows,
    ))
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
        ConfigDialect::Yaml(dialect) => {
            edit_yaml(&definition.path, &context.connector, dialect, true)?
        }
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
        ConfigDialect::Yaml(dialect) => {
            edit_yaml(&definition.path, &context.connector, dialect, false)?
        }
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
        if !json_entry_owned(entry, dialect, connector, cfg!(target_os = "windows")) {
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
        let args = toml_entry_args(entry);
        if !command_owned(
            command,
            args.as_deref(),
            connector,
            cfg!(target_os = "windows"),
        ) {
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

fn inspect_yaml(
    source: &str,
    dialect: YamlDialect,
    connector: &Path,
    windows: bool,
) -> Result<RegistrationState, HarnessError> {
    if source.trim().is_empty() {
        return Ok(RegistrationState::Missing);
    }
    let root: serde_yaml::Value =
        serde_yaml::from_str(source).map_err(|error| HarnessError::Malformed(error.to_string()))?;
    let entry = match dialect {
        YamlDialect::Goose => yaml_value(&root, "extensions")
            .and_then(|extensions| yaml_value(extensions, SERVER_NAME)),
        YamlDialect::Continue => yaml_value(&root, "mcpServers")
            .and_then(serde_yaml::Value::as_sequence)
            .and_then(|servers| {
                servers.iter().find(|entry| {
                    yaml_value(entry, "name")
                        .and_then(serde_yaml::Value::as_str)
                        .is_some_and(|name| name.eq_ignore_ascii_case("Ghostlight"))
                })
            }),
    };
    let Some(entry) = entry else {
        return Ok(RegistrationState::Missing);
    };
    let command_key = match dialect {
        YamlDialect::Goose => "cmd",
        YamlDialect::Continue => "command",
    };
    let command = yaml_value(entry, command_key)
        .and_then(serde_yaml::Value::as_str)
        .unwrap_or_default();
    let args = yaml_value(entry, "args")
        .and_then(serde_yaml::Value::as_sequence)
        .and_then(|args| {
            args.iter()
                .map(serde_yaml::Value::as_str)
                .collect::<Option<Vec<_>>>()
        });
    Ok(command_registration_state(
        command,
        args.as_deref(),
        connector,
        windows,
    ))
}

fn edit_yaml(
    path: &Path,
    connector: &Path,
    dialect: YamlDialect,
    install: bool,
) -> Result<bool, HarnessError> {
    let original = read_or_empty(path)?;
    let state = inspect_yaml(&original, dialect, connector, cfg!(target_os = "windows"))?;
    if state == RegistrationState::Foreign {
        return Err(HarnessError::ForeignEntry);
    }
    if (install && state == RegistrationState::Current)
        || (!install && state == RegistrationState::Missing)
    {
        return Ok(false);
    }
    if state != RegistrationState::Missing && owned_yaml_range(&original, dialect)?.is_none() {
        return Err(HarnessError::Malformed(
            "the Ghostlight YAML entry uses a shape that cannot be edited losslessly".into(),
        ));
    }
    let rendered = edit_yaml_text(&original, connector, dialect, install)?;
    replace_with_backup(path, rendered.as_bytes())?;
    Ok(true)
}

fn yaml_value<'a>(root: &'a serde_yaml::Value, key: &str) -> Option<&'a serde_yaml::Value> {
    root.as_mapping()?
        .get(serde_yaml::Value::String(key.into()))
}

#[derive(Clone, Copy)]
struct YamlLine<'a> {
    start: usize,
    end: usize,
    indent: usize,
    text: &'a str,
}

fn edit_yaml_text(
    source: &str,
    connector: &Path,
    dialect: YamlDialect,
    install: bool,
) -> Result<String, HarnessError> {
    if source.lines().any(|line| line.starts_with('\t')) {
        return Err(HarnessError::Malformed(
            "tab-indented YAML is not safe to edit automatically".into(),
        ));
    }
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut rendered = source.to_owned();
    if let Some((start, end)) = owned_yaml_range(source, dialect)? {
        rendered.replace_range(start..end, "");
    }
    if !install {
        return Ok(rendered);
    }
    match dialect {
        YamlDialect::Goose => insert_goose_yaml(&rendered, connector, newline),
        YamlDialect::Continue => insert_continue_yaml(&rendered, connector, newline),
    }
}

fn yaml_lines(source: &str) -> Vec<YamlLine<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for segment in source.split_inclusive('\n') {
        let text = segment.trim_end_matches(['\r', '\n']);
        lines.push(YamlLine {
            start,
            end: start + segment.len(),
            indent: text.len() - text.trim_start_matches(' ').len(),
            text,
        });
        start += segment.len();
    }
    if start < source.len() || source.is_empty() {
        let text = &source[start..];
        lines.push(YamlLine {
            start,
            end: source.len(),
            indent: text.len() - text.trim_start_matches(' ').len(),
            text,
        });
    }
    lines
}

fn top_level_yaml_key(source: &str, key: &str) -> Option<usize> {
    yaml_lines(source)
        .iter()
        .position(|line| line.indent == 0 && line.text.trim_end() == format!("{key}:"))
}

fn yaml_parent_end(lines: &[YamlLine<'_>], parent: usize) -> usize {
    lines
        .iter()
        .skip(parent + 1)
        .find(|line| {
            let trimmed = line.text.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && line.indent == 0
        })
        .map_or_else(
            || lines.last().map_or(0, |line| line.end),
            |line| line.start,
        )
}

fn owned_yaml_range(
    source: &str,
    dialect: YamlDialect,
) -> Result<Option<(usize, usize)>, HarnessError> {
    let lines = yaml_lines(source);
    let key = match dialect {
        YamlDialect::Goose => "extensions",
        YamlDialect::Continue => "mcpServers",
    };
    let Some(parent) = top_level_yaml_key(source, key) else {
        return Ok(None);
    };
    let parent_end = yaml_parent_end(&lines, parent);
    let child_indent = lines
        .iter()
        .skip(parent + 1)
        .take_while(|line| line.start < parent_end)
        .filter(|line| {
            let trimmed = line.text.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && line.indent > lines[parent].indent
        })
        .map(|line| line.indent)
        .min();
    let candidate = lines.iter().enumerate().skip(parent + 1).find(|(_, line)| {
        if line.start >= parent_end {
            return false;
        }
        if Some(line.indent) != child_indent {
            return false;
        }
        let trimmed = line.text.trim();
        match dialect {
            YamlDialect::Goose => trimmed == "ghostlight:",
            YamlDialect::Continue => trimmed
                .strip_prefix("- name:")
                .map(str::trim)
                .map(|name| name.trim_matches(['\'', '"']))
                .is_some_and(|name| name.eq_ignore_ascii_case("Ghostlight")),
        }
    });
    let Some((index, line)) = candidate else {
        return Ok(None);
    };
    let end = lines
        .iter()
        .skip(index + 1)
        .find(|next| {
            let trimmed = next.text.trim();
            if trimmed.is_empty() {
                return false;
            }
            next.indent <= line.indent
                && (next.indent == 0
                    || matches!(dialect, YamlDialect::Goose)
                    || trimmed.starts_with("- "))
        })
        .map_or(parent_end, |next| next.start);
    Ok(Some((line.start, end)))
}

fn insert_goose_yaml(
    source: &str,
    connector: &Path,
    newline: &str,
) -> Result<String, HarnessError> {
    let block = indent_yaml(
        &manual_yaml_setup(connector, YamlDialect::Goose),
        0,
        newline,
    );
    let lines = yaml_lines(source);
    if let Some(parent) = top_level_yaml_key(source, "extensions") {
        let mut child = manual_yaml_setup(connector, YamlDialect::Goose);
        child = child
            .strip_prefix("extensions:\n")
            .expect("manual goose setup has one extensions root")
            .to_owned();
        let child = indent_yaml(&child, 0, newline);
        return Ok(insert_yaml_block(
            source,
            yaml_parent_end(&lines, parent),
            &child,
            newline,
        ));
    }
    if !source.trim().is_empty() {
        let parsed: serde_yaml::Value = serde_yaml::from_str(source)
            .map_err(|error| HarnessError::Malformed(error.to_string()))?;
        if !parsed.is_mapping() {
            return Err(HarnessError::Malformed(
                "configuration root is not a YAML mapping".into(),
            ));
        }
        if yaml_value(&parsed, "extensions").is_some() {
            return Err(HarnessError::Malformed(
                "extensions uses a YAML shape that cannot be edited losslessly".into(),
            ));
        }
    }
    Ok(insert_yaml_block(source, source.len(), &block, newline))
}

fn insert_continue_yaml(
    source: &str,
    connector: &Path,
    newline: &str,
) -> Result<String, HarnessError> {
    let mut rendered = source.to_owned();
    if rendered.trim().is_empty() {
        return Ok(indent_yaml(
            &manual_yaml_setup(connector, YamlDialect::Continue),
            0,
            newline,
        ));
    }
    let parsed: serde_yaml::Value = serde_yaml::from_str(&rendered)
        .map_err(|error| HarnessError::Malformed(error.to_string()))?;
    if !parsed.is_mapping() {
        return Err(HarnessError::Malformed(
            "configuration root is not a YAML mapping".into(),
        ));
    }
    if yaml_value(&parsed, "mcpServers").is_some()
        && top_level_yaml_key(&rendered, "mcpServers").is_none()
    {
        return Err(HarnessError::Malformed(
            "mcpServers uses a YAML shape that cannot be edited losslessly".into(),
        ));
    }
    for (key, value) in [
        ("name", "Local Config"),
        ("version", "1.0.0"),
        ("schema", "v1"),
    ] {
        if yaml_value(&parsed, key).is_none() {
            rendered = insert_yaml_block(
                &rendered,
                rendered.len(),
                &format!("{key}: {value}\n"),
                newline,
            );
        }
    }
    let lines = yaml_lines(&rendered);
    if let Some(parent) = top_level_yaml_key(&rendered, "mcpServers") {
        let command = serde_json::to_string(&connector.to_string_lossy())
            .expect("a path is representable as a YAML string");
        let item = format!("  - name: Ghostlight\n    command: {command}\n    args: []\n");
        return Ok(insert_yaml_block(
            &rendered,
            yaml_parent_end(&lines, parent),
            &indent_yaml(&item, 0, newline),
            newline,
        ));
    }
    let command = serde_json::to_string(&connector.to_string_lossy())
        .expect("a path is representable as a YAML string");
    let block =
        format!("mcpServers:\n  - name: Ghostlight\n    command: {command}\n    args: []\n");
    Ok(insert_yaml_block(
        &rendered,
        rendered.len(),
        &indent_yaml(&block, 0, newline),
        newline,
    ))
}

fn indent_yaml(source: &str, spaces: usize, newline: &str) -> String {
    let prefix = " ".repeat(spaces);
    source
        .lines()
        .map(|line| format!("{prefix}{line}{newline}"))
        .collect()
}

fn insert_yaml_block(source: &str, at: usize, block: &str, newline: &str) -> String {
    let mut rendered = String::with_capacity(source.len() + block.len() + newline.len());
    rendered.push_str(&source[..at]);
    if at > 0 && !rendered.ends_with('\n') {
        rendered.push_str(newline);
    }
    rendered.push_str(block);
    rendered.push_str(&source[at..]);
    rendered
}

fn json_entry(root: &Value, dialect: JsonDialect) -> Option<&Value> {
    match dialect {
        JsonDialect::McpServers | JsonDialect::Copilot => root.get("mcpServers")?.get(SERVER_NAME),
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
        JsonDialect::Copilot => {
            json!({"type":"local","command":command,"args":[],"env":{},"tools":["*"]})
        }
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
        JsonDialect::McpServers | JsonDialect::Copilot => "mcpServers",
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

fn json_entry_owned(entry: &Value, dialect: JsonDialect, connector: &Path, windows: bool) -> bool {
    json_entry_command(entry, dialect).is_some_and(|command| {
        let args = json_entry_args(entry, dialect);
        command_owned(command, args.as_deref(), connector, windows)
    })
}

fn json_entry_command(entry: &Value, dialect: JsonDialect) -> Option<&str> {
    match dialect {
        JsonDialect::McpServers
        | JsonDialect::Copilot
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

fn json_entry_args(entry: &Value, dialect: JsonDialect) -> Option<Vec<&str>> {
    let values: &[Value] = match dialect {
        JsonDialect::McpServers
        | JsonDialect::Copilot
        | JsonDialect::Servers
        | JsonDialect::ContextServers
        | JsonDialect::Crush => entry.get("args")?.as_array()?,
        JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => {
            let command = entry.get("command")?.as_array()?;
            command.get(1..)?
        }
    };
    values.iter().map(Value::as_str).collect()
}

fn manual_setup(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<String, HarnessError> {
    match definition.dialect {
        ConfigDialect::CodexToml => Ok(format!(
            "[mcp_servers.{SERVER_NAME}]\ncommand = {}\nargs = []\n",
            toml_edit::Value::from(context.connector.to_string_lossy().into_owned())
        )),
        ConfigDialect::Json(dialect) => manual_json_setup(&context.connector, dialect),
        ConfigDialect::Yaml(dialect) => Ok(manual_yaml_setup(&context.connector, dialect)),
        ConfigDialect::OpenCode => {
            let source = read_or_empty(&definition.path)?;
            manual_json_setup(&context.connector, opencode_dialect(context, &source)?)
        }
    }
}

fn manual_json_setup(connector: &Path, dialect: JsonDialect) -> Result<String, HarnessError> {
    let entry = expected_json_entry(connector, dialect);
    let document = match dialect {
        JsonDialect::McpServers | JsonDialect::Copilot => {
            json!({"mcpServers": {SERVER_NAME: entry}})
        }
        JsonDialect::Servers => json!({"servers": {SERVER_NAME: entry}}),
        JsonDialect::ContextServers => json!({"context_servers": {SERVER_NAME: entry}}),
        JsonDialect::Crush | JsonDialect::OpenCodeV1 => json!({"mcp": {SERVER_NAME: entry}}),
        JsonDialect::OpenCodeV2 => json!({"mcp": {"servers": {SERVER_NAME: entry}}}),
    };
    serde_json::to_string_pretty(&document)
        .map_err(|error| HarnessError::Malformed(error.to_string()))
}

fn manual_yaml_setup(connector: &Path, dialect: YamlDialect) -> String {
    let command = serde_json::to_string(&connector.to_string_lossy())
        .expect("a filesystem path is always representable as a JSON/YAML string");
    match dialect {
        YamlDialect::Goose => format!(
            "extensions:\n  ghostlight:\n    type: stdio\n    name: ghostlight\n    display_name: Ghostlight\n    enabled: true\n    cmd: {command}\n    args: []\n    envs: {{}}\n    timeout: 300\n"
        ),
        YamlDialect::Continue => format!(
            "name: Local Config\nversion: 1.0.0\nschema: v1\nmcpServers:\n  - name: Ghostlight\n    command: {command}\n    args: []\n"
        ),
    }
}

fn toml_entry_args(entry: &Item) -> Option<Vec<&str>> {
    entry
        .get("args")?
        .as_array()?
        .iter()
        .map(toml_edit::Value::as_str)
        .collect()
}

fn command_name(command: &str, expected: &str) -> bool {
    Path::new(command)
        .file_stem()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.eq_ignore_ascii_case(expected))
}

fn command_owned(command: &str, args: Option<&[&str]>, connector: &Path, windows: bool) -> bool {
    command_name(command, "ghostlight-mcp-connector")
        || legacy_relay_owned(command, args, connector, windows)
}

fn legacy_relay_owned(
    command: &str,
    args: Option<&[&str]>,
    connector: &Path,
    windows: bool,
) -> bool {
    if !command_name(command, "ghostlight-relay") || args != Some(&["--role", "agent"]) {
        return false;
    }
    let Some(actual_root) = Path::new(command).parent().and_then(Path::parent) else {
        return false;
    };
    let Some(expected_root) = connector.parent().and_then(Path::parent) else {
        return false;
    };
    paths_equal(actual_root, expected_root, windows)
}

fn command_registration_state(
    command: &str,
    args: Option<&[&str]>,
    connector: &Path,
    windows: bool,
) -> RegistrationState {
    if !command_owned(command, args, connector, windows) {
        return RegistrationState::Foreign;
    }
    let actual = fs::canonicalize(command).unwrap_or_else(|_| PathBuf::from(command));
    let expected = fs::canonicalize(connector).unwrap_or_else(|_| connector.to_path_buf());
    let current = command_name(command, "ghostlight-mcp-connector")
        && paths_equal(&actual, &expected, windows)
        && args.is_some_and(|args| args.is_empty());
    if current {
        RegistrationState::Current
    } else {
        RegistrationState::Updatable
    }
}

fn paths_equal(actual: &Path, expected: &Path, windows: bool) -> bool {
    let actual = actual.to_string_lossy();
    let expected = expected.to_string_lossy();
    if windows {
        actual.eq_ignore_ascii_case(&expected)
    } else {
        actual == expected
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct LocationOverride {
    executable: Option<PathBuf>,
    config: Option<PathBuf>,
}

type LocationOverrides = BTreeMap<String, LocationOverride>;

fn read_location_overrides(context: &HarnessContext) -> Result<LocationOverrides, HarnessError> {
    match fs::read_to_string(&context.locations) {
        Ok(source) => serde_json::from_str(&source)
            .map_err(|error| HarnessError::MalformedLocationState(error.to_string())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(HarnessError::Read(error)),
    }
}

fn write_location_overrides(
    context: &HarnessContext,
    overrides: &LocationOverrides,
) -> Result<(), HarnessError> {
    let bytes = serde_json::to_vec_pretty(overrides)
        .map_err(|error| HarnessError::MalformedLocationState(error.to_string()))?;
    replace_with_backup(&context.locations, &bytes)
}

fn validate_located_config(path: &Path, dialect: ConfigDialect) -> Result<(), HarnessError> {
    let source = fs::read_to_string(path).map_err(HarnessError::Read)?;
    match dialect {
        ConfigDialect::CodexToml => source
            .parse::<DocumentMut>()
            .map(|_| ())
            .map_err(|error| HarnessError::Malformed(error.to_string())),
        ConfigDialect::Json(_) | ConfigDialect::OpenCode => parse_jsonc(&source).map(|_| ()),
        ConfigDialect::Yaml(_) => serde_yaml::from_str::<serde_yaml::Value>(&source)
            .map(|_| ())
            .map_err(|error| HarnessError::Malformed(error.to_string())),
    }
}

fn is_executable_file(path: &Path, windows: bool) -> bool {
    if !path.is_file() {
        return false;
    }
    if windows {
        return path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0);
    }
    #[allow(unreachable_code)]
    false
}

/// If `path` is itself a symlink, resolve it to the real file it points at, so a subsequent
/// atomic write lands on that file instead of unlinking the symlink and leaving a plain file in
/// its place. A symlinked client config (for example, one tracked through a synced dotfiles repo)
/// is a deliberate choice; silently replacing the link with an ordinary file would orphan
/// whatever it pointed to without telling anyone.
pub(crate) fn resolve_through_symlink(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::canonicalize(path),
        _ => Ok(path.to_path_buf()),
    }
}

fn replace_with_backup(path: &Path, bytes: &[u8]) -> Result<(), HarnessError> {
    let path = &resolve_through_symlink(path).map_err(HarnessError::Write)?;
    let permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());
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
    if let Some(permissions) = permissions {
        fs::set_permissions(&temporary, permissions).map_err(HarnessError::Write)?;
    }
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
    /// The selected product does not have an official download on this platform.
    #[error("supported harness `{0}` has no official download for this platform")]
    NoDownload(String),
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
    /// A located path is neither a matching configuration nor an executable.
    #[error("the selected path is not a usable harness executable or configuration: {0}")]
    LocatedPathInvalid(PathBuf),
    /// Ghostlight's own located-path state is malformed.
    #[error("Ghostlight's harness location state is malformed: {0}")]
    MalformedLocationState(String),
    /// A path has no writable parent.
    #[error("harness configuration path has no parent: {0}")]
    InvalidPath(PathBuf),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use jsonc_parser::parse_to_serde_value;

    use super::{
        command_registration_state, definitions, edit_json, edit_toml, edit_yaml, harness_roots,
        inspect_json, inspect_toml, inspect_yaml, jsonc_options, manual_json_setup,
        replace_with_backup, resolve_through_symlink, HarnessAction, HarnessContext, HarnessError,
        HarnessRegistry, HarnessRoots, HarnessState, JsonDialect, RegistrationState, YamlDialect,
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

    fn context(directory: &Path) -> HarnessContext {
        HarnessContext {
            home: directory.join("home"),
            config: directory.join("config"),
            roaming: directory.join("config"),
            codex_config: directory.join("codex/config.toml"),
            connector: connector(directory),
            path_entries: Vec::new(),
            locations: directory.join("config/ghostlight/harness-locations.json"),
            windows: false,
        }
    }

    #[test]
    fn resolve_through_symlink_leaves_an_ordinary_path_alone() {
        let directory = temporary("resolve-plain");
        let path = directory.join("config.json");
        assert_eq!(resolve_through_symlink(&path).unwrap(), path);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_config_is_written_through_to_its_real_target() {
        use std::os::unix::fs::symlink;

        let directory = temporary("symlink-write-through-unix");
        let real = directory.join("real-config.json");
        fs::write(&real, br#"{"old":true}"#).unwrap();
        let link = directory.join("config.json");
        symlink(&real, &link).unwrap();

        replace_with_backup(&link, br#"{"new":true}"#).unwrap();

        // The link itself is untouched: still a symlink, still pointing at the same real file.
        // Before this fix, the rename in replace_with_backup unlinked it and left a plain file
        // in its place, silently orphaning whatever the link pointed to.
        let metadata = fs::symlink_metadata(&link).unwrap();
        assert!(metadata.file_type().is_symlink());
        assert_eq!(fs::read_link(&link).unwrap(), real);

        // The write landed on the real file behind the link.
        assert_eq!(fs::read_to_string(&real).unwrap(), r#"{"new":true}"#);
        assert_eq!(fs::read_to_string(&link).unwrap(), r#"{"new":true}"#);
    }

    #[cfg(windows)]
    #[test]
    fn a_symlinked_config_is_written_through_to_its_real_target() {
        use std::os::windows::fs::symlink_file;

        let directory = temporary("symlink-write-through-windows");
        let real = directory.join("real-config.json");
        fs::write(&real, br#"{"old":true}"#).unwrap();
        let link = directory.join("config.json");
        if symlink_file(&real, &link).is_err() {
            // No symlink privilege (Developer Mode / an elevated shell) in this environment.
            // The write-through logic itself is exercised by the equivalent Unix test; skip
            // rather than fail on an environment limitation this test cannot control.
            return;
        }

        replace_with_backup(&link, br#"{"new":true}"#).unwrap();

        let metadata = fs::symlink_metadata(&link).unwrap();
        assert!(metadata.file_type().is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), r#"{"new":true}"#);
        assert_eq!(fs::read_to_string(&link).unwrap(), r#"{"new":true}"#);
    }

    #[test]
    fn platform_roots_honor_effective_linux_and_windows_configuration() {
        let linux = harness_roots(
            Path::new("/home/test"),
            Some(PathBuf::from("/mnt/config")),
            Some(PathBuf::from("/ignored/appdata")),
            Some(PathBuf::from("/workbench/codex")),
            false,
        );
        assert_eq!(
            linux,
            HarnessRoots {
                config: PathBuf::from("/mnt/config"),
                roaming: PathBuf::from("/mnt/config"),
                codex_config: PathBuf::from("/workbench/codex/config.toml"),
            }
        );

        let windows = harness_roots(
            Path::new("C:/Users/test"),
            Some(PathBuf::from("C:/ignored/xdg")),
            Some(PathBuf::from("D:/Profiles/Roaming")),
            None,
            true,
        );
        assert_eq!(
            windows,
            HarnessRoots {
                config: PathBuf::from("D:/Profiles/Roaming"),
                roaming: PathBuf::from("D:/Profiles/Roaming"),
                codex_config: PathBuf::from("C:/Users/test/.codex/config.toml"),
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn zed_is_detected_by_the_arch_linux_executable_name() {
        let directory = temporary("zed-zeditor");
        let binary_directory = directory.join("bin");
        fs::create_dir_all(&binary_directory).unwrap();
        fs::write(binary_directory.join("zeditor"), b"test").unwrap();
        let connector = connector(&directory);
        let registry = HarnessRegistry::with_context(HarnessContext {
            home: directory.join("home"),
            config: directory.join("config"),
            roaming: directory.join("config"),
            codex_config: directory.join("codex/config.toml"),
            connector,
            path_entries: vec![binary_directory],
            locations: directory.join("locations.json"),
            windows: false,
        });

        let zed = registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "zed")
            .unwrap();
        assert_eq!(zed.state, HarnessState::Available);
        assert!(zed.can_install);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn expanded_roster_is_fixed_and_cline_targets_are_plural() {
        let directory = temporary("expanded-roster");
        let context = context(&directory);
        let rows = definitions(&context);
        for id in [
            "copilot-cli",
            "cline-cli",
            "kiro",
            "qwen-code",
            "junie",
            "kilo-code",
            "goose",
            "continue",
            "antigravity",
        ] {
            assert!(rows.iter().any(|row| row.id == id), "missing {id}");
        }
        assert_eq!(
            rows.iter().filter(|row| row.product_id == "cline").count(),
            4
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn every_registry_row_has_offline_artwork_and_the_exact_manual_command() {
        let directory = temporary("roster-artwork-manual");
        let context = context(&directory);
        let connector = context.connector.to_string_lossy().into_owned();
        let serialized_connector = serde_json::to_string(&connector).unwrap();
        let escaped_connector = serialized_connector
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap();
        let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/integrations");
        let registry = HarnessRegistry::with_context(context);
        for summary in registry.summaries() {
            assert!(
                asset_root.join(&summary.icon).is_file(),
                "missing packaged artwork for {}",
                summary.id
            );
            assert_eq!(summary.connector_command, connector);
            assert!(
                summary.manual_setup.contains(escaped_connector),
                "manual setup for {} omitted the connector",
                summary.id
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn every_download_destination_is_closed_and_https() {
        let directory = temporary("download-destinations");
        let context = context(&directory);
        let registry = HarnessRegistry::with_context(context.clone());
        for product_id in definitions(&context)
            .into_iter()
            .filter(|row| row.download_url.is_some())
            .map(|row| row.product_id)
        {
            assert!(registry
                .download_url(product_id)
                .unwrap()
                .starts_with("https://"));
        }
        assert!(matches!(
            registry.download_url("not-a-product"),
            Err(HarnessError::NoDownload(_))
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn kilo_prefers_current_config_names_and_accepts_a_located_jsonc_file() {
        let directory = temporary("kilo-current-config");
        let context = context(&directory);
        let current = context.config.join("kilo").join("kilo.jsonc");
        fs::create_dir_all(current.parent().unwrap()).unwrap();
        fs::write(&current, "{}\n").unwrap();
        let registry = HarnessRegistry::with_context(context.clone());
        let kilo = registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "kilo-code")
            .unwrap();
        assert_eq!(kilo.config_path, current.to_string_lossy());

        let located = directory.join("portable/kilo.jsonc");
        fs::create_dir_all(located.parent().unwrap()).unwrap();
        fs::write(&located, "{}\n").unwrap();
        let result = registry.locate("kilo-code", &located).unwrap();
        assert_eq!(result.summary.config_path, located.to_string_lossy());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn manual_json_setup_uses_the_real_ghostlight_key() {
        let directory = temporary("manual-json");
        let connector = connector(&directory);
        let document = manual_json_setup(&connector, JsonDialect::Copilot).unwrap();
        let value: Value = serde_json::from_str(&document).unwrap();
        assert!(value["mcpServers"].get("ghostlight").is_some());
        assert!(value["mcpServers"].get("SERVER_NAME").is_none());
        assert_eq!(
            value["mcpServers"]["ghostlight"]["command"].as_str(),
            Some(connector.to_string_lossy().as_ref())
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn located_executable_is_persistent_detection_evidence() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temporary("located-executable");
        let context = context(&directory);
        let executable = directory.join("odd-place/custom-qwen");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"test").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let registry = HarnessRegistry::with_context(context.clone());
        let located = registry.locate("qwen-code", &executable).unwrap();
        assert_eq!(located.summary.state, HarnessState::Available);
        let rediscovered = HarnessRegistry::with_context(context);
        let qwen = rediscovered
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "qwen-code")
            .unwrap();
        assert_eq!(qwen.state, HarnessState::Available);
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_missing_located_executable_is_visible_and_falls_back_safely() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temporary("stale-located-executable");
        let context = context(&directory);
        let executable = directory.join("portable/qwen");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"test").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        HarnessRegistry::with_context(context.clone())
            .locate("qwen-code", &executable)
            .unwrap();
        fs::remove_file(executable).unwrap();

        let summary = HarnessRegistry::with_context(context)
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "qwen-code")
            .unwrap();
        assert_eq!(summary.state, HarnessState::NotDetected);
        assert!(summary
            .detail
            .contains("previously located path is missing"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_located_config_requires_the_target_filename_and_valid_shape() {
        let directory = temporary("located-config-validation");
        let context = context(&directory);
        let registry = HarnessRegistry::with_context(context);
        let wrong_name = directory.join("portable/not-qwen.json");
        fs::create_dir_all(wrong_name.parent().unwrap()).unwrap();
        fs::write(&wrong_name, "{}\n").unwrap();
        assert!(matches!(
            registry.locate("qwen-code", &wrong_name),
            Err(HarnessError::LocatedPathInvalid(_))
        ));

        let malformed = directory.join("portable/settings.json");
        fs::write(&malformed, "[]\n").unwrap();
        assert!(matches!(
            registry.locate("qwen-code", &malformed),
            Err(HarnessError::Malformed(_))
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn cline_targets_install_and_remove_independently() {
        let directory = temporary("cline-plural");
        let context = context(&directory);
        fs::create_dir_all(context.home.join(".cline/data/settings")).unwrap();
        fs::create_dir_all(
            context
                .roaming
                .join("Code/User/globalStorage/saoudrizwan.claude-dev/settings"),
        )
        .unwrap();
        let registry = HarnessRegistry::with_context(context.clone());
        registry.apply("cline-cli", HarnessAction::Install).unwrap();
        registry
            .apply("cline-vscode", HarnessAction::Install)
            .unwrap();
        registry
            .apply("cline-cli", HarnessAction::Uninstall)
            .unwrap();
        assert_eq!(
            registry
                .refresh()
                .unwrap()
                .into_iter()
                .find(|summary| summary.id == "cline-vscode")
                .unwrap()
                .state,
            HarnessState::Installed
        );
        let vscode = fs::read_to_string(context.roaming.join(
            "Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
        ))
        .unwrap();
        assert!(vscode.contains("ghostlight"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn goose_yaml_is_lossless_idempotent_and_mode_preserving() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temporary("goose-yaml");
        let path = directory.join("config.yaml");
        let connector = connector(&directory);
        let source =
            "# keep goose notes\nprovider: local\nextensions:\n  sibling:\n    type: builtin\n";
        fs::write(&path, source).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(edit_yaml(&path, &connector, YamlDialect::Goose, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        assert!(installed.contains("# keep goose notes"));
        assert!(installed.contains("sibling:"));
        assert_eq!(
            inspect_yaml(&installed, YamlDialect::Goose, &connector, false).unwrap(),
            RegistrationState::Current
        );
        assert!(!edit_yaml(&path, &connector, YamlDialect::Goose, true).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), installed);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert!(edit_yaml(&path, &connector, YamlDialect::Goose, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("# keep goose notes"));
        assert!(removed.contains("sibling:"));
        assert!(!removed.contains("ghostlight:"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn continue_yaml_preserves_siblings_comments_and_exact_no_op_bytes() {
        let directory = temporary("continue-yaml");
        let path = directory.join("config.yaml");
        let connector = connector(&directory);
        let source = "# keep continue notes\nname: Mine\nversion: 2.0.0\nschema: v1\nmcpServers:\n  - name: Sibling\n    command: sibling\n    args: []\n";
        fs::write(&path, source).unwrap();
        assert!(edit_yaml(&path, &connector, YamlDialect::Continue, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        assert!(installed.contains("# keep continue notes"));
        assert!(installed.contains("name: Sibling"));
        assert!(installed.contains("name: Ghostlight"));
        assert_eq!(
            inspect_yaml(&installed, YamlDialect::Continue, &connector, false).unwrap(),
            RegistrationState::Current
        );
        assert!(!edit_yaml(&path, &connector, YamlDialect::Continue, true).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), installed);
        assert!(edit_yaml(&path, &connector, YamlDialect::Continue, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("# keep continue notes"));
        assert!(removed.contains("name: Sibling"));
        assert!(!removed.contains("name: Ghostlight"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn yaml_flow_shapes_are_refused_without_changing_bytes() {
        let directory = temporary("yaml-flow-refusal");
        let connector = connector(&directory);
        for (name, source, dialect) in [
            (
                "goose.yaml",
                "provider: local\nextensions: { sibling: { type: builtin } }\n",
                YamlDialect::Goose,
            ),
            (
                "continue.yaml",
                "name: Mine\nversion: 1.0.0\nschema: v1\nmcpServers: []\n",
                YamlDialect::Continue,
            ),
        ] {
            let path = directory.join(name);
            fs::write(&path, source).unwrap();
            assert!(matches!(
                edit_yaml(&path, &connector, dialect, true),
                Err(HarnessError::Malformed(_))
            ));
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn flow_style_owned_yaml_entry_is_not_rewritten_or_removed() {
        let directory = temporary("yaml-flow-owned");
        let path = directory.join("config.yaml");
        let connector = connector(&directory);
        let source = format!(
            "extensions: {{ ghostlight: {{ type: stdio, cmd: {}, args: [] }} }}\n",
            serde_json::to_string(&connector.to_string_lossy()).unwrap()
        );
        fs::write(&path, &source).unwrap();
        assert!(matches!(
            edit_yaml(&path, &connector, YamlDialect::Goose, false),
            Err(HarnessError::Malformed(_))
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn nested_ghostlight_yaml_names_belong_to_their_sibling() {
        let directory = temporary("yaml-nested-sibling");
        let connector = connector(&directory);
        for (name, source, dialect, nested) in [
            (
                "goose.yaml",
                "extensions:\n  sibling:\n    ghostlight:\n      keep: true\n",
                YamlDialect::Goose,
                "    ghostlight:\n      keep: true",
            ),
            (
                "continue.yaml",
                "mcpServers:\n  - name: Sibling\n    options:\n      children:\n        - name: Ghostlight\n          keep: true\n",
                YamlDialect::Continue,
                "        - name: Ghostlight\n          keep: true",
            ),
        ] {
            let path = directory.join(name);
            fs::write(&path, source).unwrap();
            assert!(edit_yaml(&path, &connector, dialect, true).unwrap());
            assert!(edit_yaml(&path, &connector, dialect, false).unwrap());
            let removed = fs::read_to_string(&path).unwrap();
            assert!(removed.contains(nested), "{name}: {removed}");
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn legacy_relay_requires_the_exact_agent_signature_and_install_root() {
        let directory = temporary("legacy-relay");
        let install_root = directory.join("bin");
        let current = install_root.join("v1").join(if cfg!(windows) {
            "ghostlight-mcp-connector.exe"
        } else {
            "ghostlight-mcp-connector"
        });
        let legacy = install_root.join("v0").join(if cfg!(windows) {
            "ghostlight-relay.exe"
        } else {
            "ghostlight-relay"
        });
        assert_eq!(
            command_registration_state(
                legacy.to_string_lossy().as_ref(),
                Some(&["--role", "agent"]),
                &current,
                cfg!(windows),
            ),
            RegistrationState::Updatable
        );
        assert_eq!(
            command_registration_state(
                legacy.to_string_lossy().as_ref(),
                Some(&["--role", "browser"]),
                &current,
                cfg!(windows),
            ),
            RegistrationState::Foreign
        );
        assert_eq!(
            command_registration_state(
                directory
                    .join("foreign/v0")
                    .join(legacy.file_name().unwrap())
                    .to_string_lossy()
                    .as_ref(),
                Some(&["--role", "agent"]),
                &current,
                cfg!(windows),
            ),
            RegistrationState::Foreign
        );
        fs::remove_dir_all(directory).unwrap();
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
        // Pinned against Zed's own current source (crates/settings_content/src/project.rs,
        // ContextServerCommand): the stdio variant has no `source` field at all, only command,
        // args, and env. ADR-0071's amendment records the verification; this is where that
        // verification stops being re-litigable.
        let written: Value = parse_to_serde_value(&installed, &jsonc_options())
            .unwrap()
            .unwrap();
        let entry = &written["context_servers"]["ghostlight"];
        assert!(entry.get("source").is_none(), "{entry}");
        assert!(entry["command"].is_string());
        assert!(entry["args"].is_array());
        assert!(entry["env"].is_object());
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
