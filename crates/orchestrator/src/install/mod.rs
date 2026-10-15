//! Explicit, ownership-checked development-harness registration owned by the orchestrator.

pub mod browser_package;
pub mod command_path;
pub mod container;
pub mod deployment;
pub mod desktop_entry;
pub mod entry;
pub mod handoff;
pub mod layout;
pub mod migration;
pub mod native_host;
pub mod reconcile;
pub mod record;
#[cfg(test)]
pub(crate) mod scratch;
pub mod swap;
pub mod user_assets;
pub mod user_path;
#[cfg(windows)]
pub mod windows_entry;

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use ghostlight_relay::route::MCP_ARGUMENT;
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use jsonc_parser::{parse_to_serde_value, ParseOptions};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use toml_edit::{value, Array, DocumentMut, Item, Table};
use uuid::Uuid;

use container::{AppContainer, Residence};
use layout::{EDGE_COMMAND_NAME, RETIRED_MCP_CONNECTOR_NAME};

const SERVER_NAME: &str = "ghostlight";
/// The arguments that make the edge the MCP relay, in every client's entry.
const MCP_ARGUMENTS: [&str; 1] = [MCP_ARGUMENT];
/// The package name of the Claude desktop app, which reads the Claude Desktop configuration.
const CLAUDE_DESKTOP_PACKAGE: &str = "Claude";
/// File in the state directory recording where each MCP client's configuration was found.
const HARNESS_LOCATIONS_FILE: &str = "harness-locations.json";
// Codex filters its stdio child's environment. Linux demand-start needs the invoking
// desktop session, not captured machine-local values or a second service-only route.
const CODEX_DESKTOP_ENVIRONMENT: &[&str] = &[
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_CURRENT_DESKTOP",
    "XAUTHORITY",
];

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

    /// A registry over client configurations below `directory` that start `edge` and keeps
    /// its located paths in `state`, for tests that must not touch this user's clients.
    #[cfg(test)]
    pub(crate) fn isolated(directory: &Path, edge: &Path, state: &Path) -> Self {
        Self::with_context(isolated_context(directory, edge, state))
    }

    /// Like [`Self::isolated`], run from inside `container`: the configurations below
    /// `directory/config` are that app's private view.
    #[cfg(test)]
    pub(crate) fn isolated_inside(
        directory: &Path,
        edge: &Path,
        state: &Path,
        container: AppContainer,
    ) -> Self {
        let mut context = isolated_context(directory, edge, state);
        context.private_view = Some(PrivateView {
            container,
            roots: vec![directory.join("config")],
        });
        Self::with_context(context)
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

    /// Apply one explicit check, install, fix, or uninstall action.
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
                message: format!(
                    "{} was checked without changing configuration.",
                    definition.label
                ),
                summary: inspect(&self.inner.context, &definition),
            },
            HarnessAction::Install => apply_install(&self.inner.context, &definition)?,
            HarnessAction::Fix => apply_fix(&self.inner.context, &definition)?,
            HarnessAction::Uninstall => apply_uninstall(&self.inner.context, &definition)?,
        };
        let _ = self.refresh();
        Ok(result)
    }

    /// Set up or update every detected target Ghostlight can change safely.
    ///
    /// Current registrations and targets that are not detected are left alone. Malformed and
    /// foreign registrations remain blocked by the same per-target ownership check used by
    /// [`Self::apply`]. One target failing does not prevent an independent target from becoming
    /// ready; the result names the failures and the refreshed roster carries their evidence.
    pub fn setup_detected(&self) -> Result<HarnessSetupResult, HarnessError> {
        let _action = lock(&self.inner.action);
        let initial = self.refresh()?;
        let states: BTreeMap<&str, HarnessState> = initial
            .iter()
            .map(|summary| (summary.id.as_str(), summary.state))
            .collect();
        let eligible: Vec<HarnessDefinition> = definitions(&self.inner.context)
            .into_iter()
            .filter(|definition| {
                !matches!(definition.dialect, ConfigDialect::Manual(_))
                    && matches!(
                        states.get(definition.id),
                        Some(HarnessState::Available | HarnessState::Updatable)
                    )
            })
            .collect();

        let mut set_up = 0;
        let mut updated = 0;
        let mut failures = Vec::new();
        for definition in eligible {
            let state = states
                .get(definition.id)
                .copied()
                .unwrap_or(HarnessState::NotDetected);
            match apply_install(&self.inner.context, &definition) {
                Ok(result) if result.changed => match state {
                    HarnessState::Available => set_up += 1,
                    HarnessState::Updatable => updated += 1,
                    _ => {}
                },
                Ok(_) => {}
                Err(error) => failures.push(HarnessSetupFailure {
                    id: definition.id.into(),
                    name: definition.label,
                    detail: bounded_disclosure(&error.to_string())
                        .unwrap_or_else(|| "The target could not be changed.".into()),
                }),
            }
        }

        let summaries = self.refresh()?;
        let needs_attention = summaries
            .iter()
            .filter(|summary| summary.state == HarnessState::NeedsAttention)
            .count();
        let message = setup_message(set_up, updated, needs_attention, failures.len());
        Ok(HarnessSetupResult {
            set_up,
            updated,
            needs_attention,
            failures,
            message,
        })
    }

    /// Return product-owned text for the desktop clipboard boundary.
    pub fn copy_text(&self, id: &str, kind: HarnessCopyKind) -> Result<String, HarnessError> {
        let definition = definition(&self.inner.context, id)?;
        Ok(match kind {
            HarnessCopyKind::Command => mcp_command_line(&self.inner.context.edge),
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
        let message = if matches!(definition.dialect, ConfigDialect::Manual(_)) {
            manual_setup_sentence(&definition)
        } else {
            format!(
                "Ghostlight will use the located {} path after you choose Set up.",
                summary.label
            )
        };
        Ok(HarnessActionResult {
            changed: true,
            message,
            summary,
        })
    }
}

/// A context over client configurations below `directory`, for tests that must not touch this
/// user's clients.
#[cfg(test)]
fn isolated_context(directory: &Path, edge: &Path, state: &Path) -> HarnessContext {
    HarnessContext {
        home: directory.join("home"),
        config: directory.join("config"),
        roaming: directory.join("config"),
        codex_config: directory.join("codex").join("config.toml"),
        edge: edge.to_path_buf(),
        path_entries: Vec::new(),
        locations: state.join(HARNESS_LOCATIONS_FILE),
        windows: false,
        private_view: None,
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
    /// Replace one foreign entry occupying Ghostlight's registration key.
    Fix,
    /// Remove only Ghostlight's owned registration.
    Uninstall,
}

/// Product-owned clipboard material exposed by the desktop adapter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HarnessCopyKind {
    /// The installed edge's command line for a local stdio MCP server: its absolute path and
    /// the `mcp` argument.
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
    /// A Ghostlight-owned registration starts something other than the installed edge.
    Updatable,
    /// A malformed or foreign entry requires deliberate manual attention.
    NeedsAttention,
}

impl HarnessState {
    /// The plain words a person reads for this state.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotDetected => "not detected",
            Self::Available => "detected, not connected",
            Self::Installed => "connected",
            Self::Updatable => "connected, needs an update",
            Self::NeedsAttention => "needs attention",
        }
    }
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
    /// What a person reads for this target: the name, plus the host in parentheses when
    /// several targets share the name, such as `Cline (Cursor)`.
    pub label: String,
    /// User-facing concrete target label.
    pub target: String,
    /// Bundled integration artwork file name.
    pub icon: String,
    /// Current registration state.
    pub state: HarnessState,
    /// Fixed outcome-oriented detail.
    pub detail: String,
    /// Optional precomposed proof behind a blocked state, authored by the orchestrator
    /// (ADR-0135).
    ///
    /// Present only for a foreign entry: it names the program found where Ghostlight's entry
    /// belongs and its argument count, never its arguments (ADR-0135 as narrowed by the
    /// ADR-0199 amendment of 2026-10-15). The surface renders it verbatim and authors no
    /// words of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    /// Whether an explicit install can be attempted.
    pub can_install: bool,
    /// Whether an explicit fix can replace a foreign entry under Ghostlight's key.
    pub can_fix: bool,
    /// Whether an owned registration can be removed.
    pub can_uninstall: bool,
    /// Whether the product has a closed official download destination on this platform.
    pub can_download: bool,
    /// Whether the native Locate action is available.
    pub can_locate: bool,
    /// Whether this process runs inside an app container that keeps this target's configuration
    /// private: what it reads here is that app's view, and a change would reach only that app.
    pub private_view: bool,
    /// Exact configuration destination currently resolved for this target.
    pub config_path: String,
    /// Exact installed command line available for manual setup.
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

/// Definite aggregate result of setting up every safely changeable detected target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HarnessSetupResult {
    /// Missing registrations added for detected targets.
    pub set_up: usize,
    /// Stale Ghostlight-owned registrations updated to start the installed edge.
    pub updated: usize,
    /// Refused malformed or foreign targets still requiring a person's attention.
    pub needs_attention: usize,
    /// Independent target writes that failed for an environmental reason.
    pub failures: Vec<HarnessSetupFailure>,
    /// Product-owned plain-language outcome for the workbench.
    pub message: String,
}

/// One target the aggregate setup could not change.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HarnessSetupFailure {
    /// Stable target id.
    pub id: String,
    /// User-facing product name.
    pub name: String,
    /// Bounded typed failure rendered by the existing harness boundary.
    pub detail: String,
}

fn setup_message(set_up: usize, updated: usize, needs_attention: usize, failed: usize) -> String {
    let changed = set_up + updated;
    let mut message = if changed == 0 {
        "Everything Ghostlight can set up is already ready.".to_owned()
    } else {
        format!(
            "Ghostlight set up {set_up} and updated {updated} MCP integration{}.",
            if changed == 1 { "" } else { "s" }
        )
    };
    if changed > 0 {
        message.push_str(" Restart or reconnect those clients to load the tools.");
    }
    if needs_attention > 0 {
        message.push_str(&format!(
            " {needs_attention} target{} still need{} attention.",
            if needs_attention == 1 { "" } else { "s" },
            if needs_attention == 1 { "s" } else { "" }
        ));
    }
    if failed > 0 {
        message.push_str(&format!(
            " {failed} target{} could not be changed.",
            if failed == 1 { "" } else { "s" }
        ));
    }
    message
}

#[derive(Clone)]
struct HarnessContext {
    home: PathBuf,
    config: PathBuf,
    roaming: PathBuf,
    codex_config: PathBuf,
    edge: PathBuf,
    path_entries: Vec<PathBuf>,
    locations: PathBuf,
    windows: bool,
    /// Set when this process runs inside an app container: configuration below its `AppData`
    /// roots is that app's private view, which no other app reads.
    private_view: Option<PrivateView>,
}

/// The app container this process runs in and the directories it redirects.
#[derive(Clone)]
struct PrivateView {
    container: AppContainer,
    roots: Vec<PathBuf>,
}

impl HarnessContext {
    /// The app whose private view holds the configuration of `definition`, when this process
    /// writes there and the client is not that app itself: an app reads the very view it
    /// writes, so its own configuration is written like any other.
    fn private_to(&self, definition: &HarnessDefinition) -> Option<&AppContainer> {
        let view = self.private_view.as_ref()?;
        if definition
            .own_package
            .is_some_and(|package| view.container.is_app(package))
        {
            return None;
        }
        let path = comparable(&definition.path);
        view.roots
            .iter()
            .map(|root| comparable(root))
            .any(|root| path == root || path.starts_with(&format!("{root}/")))
            .then_some(&view.container)
    }

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
        let layout = layout::Layout::current();
        let edge = layout
            .as_ref()
            .map(layout::Layout::bin)
            .unwrap_or_default()
            .join(executable_name(EDGE_COMMAND_NAME, windows));
        let path_entries = env::var_os("PATH")
            .map(|path| env::split_paths(&path).collect())
            .unwrap_or_default();
        let locations = layout
            .map(|layout| layout.state())
            .unwrap_or_default()
            .join(HARNESS_LOCATIONS_FILE);
        let private_view = if windows {
            Residence::detect()
                .container()
                .map(|container| PrivateView {
                    container: container.clone(),
                    roots: vec![
                        roots.roaming.clone(),
                        env::var_os("LOCALAPPDATA")
                            .map_or_else(|| home.join("AppData/Local"), PathBuf::from),
                    ],
                })
        } else {
            None
        };
        Self {
            home,
            config: roots.config,
            roaming: roots.roaming,
            codex_config: roots.codex_config,
            edge,
            path_entries,
            locations,
            windows,
            private_view,
        }
    }
}

/// A path as one lowercase string with forward slashes, for comparing Windows paths.
fn comparable(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
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
    /// The name a person reads: `name`, with the host added when several targets share the name.
    label: String,
    target: &'static str,
    icon: &'static str,
    download_url: Option<&'static str>,
    path: PathBuf,
    default_path: PathBuf,
    located_executable: Option<PathBuf>,
    located_stale: bool,
    executables: &'static [&'static str],
    dialect: ConfigDialect,
    /// The package name of the packaged app that reads this configuration, when the client is
    /// one. Inside that app's own container the private view is the one the client reads.
    own_package: Option<&'static str>,
}

impl HarnessDefinition {
    /// The same definition, for a client that is the packaged app `package`.
    fn packaged_as(mut self, package: &'static str) -> Self {
        self.own_package = Some(package);
        self
    }

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
    /// A client whose file Ghostlight never parses or writes: a person pastes the setup.
    Manual(ManualDialect),
    CodexToml,
    OpenCode,
}

#[derive(Clone, Copy)]
enum ManualDialect {
    Goose,
    Continue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JsonDialect {
    McpServers,
    Copilot,
    Servers,
    ContextServers,
    Crush,
    OpenCodeV1,
    OpenCodeV2,
    ZCode,
}

fn definitions(context: &HarnessContext) -> Vec<HarnessDefinition> {
    let zed = if context.windows {
        context.roaming.join("Zed/settings.json")
    } else {
        context.config.join("zed/settings.json")
    };
    let zcode = context.home.join(".zcode/cli/config.json");
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
        )
        .packaged_as(CLAUDE_DESKTOP_PACKAGE),
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
        // Windsurf's current official distribution is named Devin and moved its user MCP
        // configuration. Keep the historical target above so either installed generation can be
        // managed without guessing one path from the other (ADR-0148).
        row(
            "devin",
            "windsurf",
            "Devin",
            "Devin user",
            "windsurf.svg",
            Some("https://windsurf.com/editor/download"),
            context.config.join("devin/mcp_config.json"),
            &["devin", "devin-desktop", "surf"],
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
        // ZCode's MCP registration lives in its shared CLI configuration under `mcp.servers`
        // (ADR-0141). No official download destination is pinned until one is verified.
        row(
            "zcode",
            "zcode",
            "ZCode",
            "User",
            "zcode.svg",
            None,
            zcode,
            &["zcode"],
            ConfigDialect::Json(JsonDialect::ZCode),
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
            ConfigDialect::Manual(ManualDialect::Goose),
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
            ConfigDialect::Manual(ManualDialect::Continue),
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
    let shared: Vec<&'static str> = rows
        .iter()
        .filter(|row| rows.iter().filter(|other| other.name == row.name).count() > 1)
        .map(|row| row.name)
        .collect();
    for definition in &mut rows {
        if shared.contains(&definition.name) {
            definition.label = format!("{} ({})", definition.name, definition.target);
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
        label: name.to_owned(),
        target,
        icon,
        download_url,
        default_path: path.clone(),
        path,
        located_executable: None,
        located_stale: false,
        executables,
        dialect,
        own_package: None,
    }
}

fn inspect(context: &HarnessContext, definition: &HarnessDefinition) -> HarnessSummary {
    let detected = definition.path.exists()
        || product_config_parent_exists(context, &definition.path)
        || definition
            .located_executable
            .as_ref()
            .is_some_and(|path| path.is_file())
        || definition
            .executables
            .iter()
            .any(|name| executable_on_path(context, name));
    let edge_ready = context.edge.is_file();
    let manual = matches!(definition.dialect, ConfigDialect::Manual(_));
    let registration = registration_state(context, definition);
    let retired = matches!(registration, Ok(RegistrationState::Retired));
    let state = match &registration {
        Ok(RegistrationState::Missing) if detected => HarnessState::Available,
        Ok(RegistrationState::Missing) => HarnessState::NotDetected,
        Ok(RegistrationState::Current) => HarnessState::Installed,
        Ok(RegistrationState::Updatable | RegistrationState::Retired) => HarnessState::Updatable,
        Ok(RegistrationState::Foreign(_)) | Err(_) => HarnessState::NeedsAttention,
    };
    // The blocked causes are named from what the inspection actually saw, never conflated
    // (ADR-0135). The evidence line carries the specifics beside the claim.
    let mut detail: String = match &registration {
        Ok(RegistrationState::Foreign(_)) => {
            "A foreign entry holds this target's Ghostlight registration; it was left untouched."
                .into()
        }
        Err(error @ HarnessError::Malformed(_)) => error.to_string(),
        Err(_) => "This configuration could not be read; it was left untouched.".into(),
        _ if manual => manual_setup_sentence(definition),
        _ => match state {
            HarnessState::Installed => "Ghostlight is registered for this user context.".into(),
            HarnessState::Updatable if retired => format!(
                "This entry starts {RETIRED_MCP_CONNECTOR_NAME}, which Ghostlight no longer \
                 installs. Set up points it at {EDGE_COMMAND_NAME} {MCP_ARGUMENT}."
            ),
            HarnessState::Updatable => "Ghostlight's saved registration needs an update.".into(),
            HarnessState::Available if edge_ready => {
                "Detected and ready for an explicit Ghostlight registration.".into()
            }
            HarnessState::Available => "Detected, but the Ghostlight edge is missing.".into(),
            HarnessState::NotDetected if edge_ready => {
                "Not detected. You can prepare its user configuration before installing it.".into()
            }
            HarnessState::NotDetected => "Not detected, and the Ghostlight edge is missing.".into(),
            HarnessState::NeedsAttention => String::new(),
        },
    };
    let evidence: Option<String> = match &registration {
        Ok(RegistrationState::Foreign(found_command)) => Some(match found_command {
            Some(found) => format!(
                "Found another program, `{}` with {}, where Ghostlight's entry belongs; \
                 Ghostlight maintains `{}` there and changed nothing.",
                found.program,
                argument_count_phrase(found.arguments),
                context.edge.display()
            ),
            None => format!(
                "Found an entry without a usable command where Ghostlight's entry belongs; \
                 Ghostlight maintains `{}` there and changed nothing.",
                context.edge.display()
            ),
        }),
        _ => None,
    };
    if definition.located_stale {
        detail.push_str(" A previously located path is missing; the normal location is shown.");
    }
    let can_fix = edge_ready && matches!(registration, Ok(RegistrationState::Foreign(_)));
    HarnessSummary {
        id: definition.id.into(),
        product_id: definition.product_id.into(),
        name: definition.name.into(),
        label: definition.label.clone(),
        target: definition.target.into(),
        icon: definition.icon.into(),
        state,
        detail,
        evidence,
        can_install: edge_ready
            && !manual
            && matches!(
                state,
                HarnessState::Available | HarnessState::NotDetected | HarnessState::Updatable
            ),
        can_fix,
        can_uninstall: state == HarnessState::Installed,
        can_download: definition.download_url.is_some(),
        can_locate: true,
        private_view: context.private_to(definition).is_some(),
        config_path: definition.path.to_string_lossy().into_owned(),
        connector_command: mcp_command_line(&context.edge),
        manual_setup: manual_setup(context, definition).unwrap_or_else(|_| {
            format!(
                "Use this command as a local stdio MCP server:\n{}",
                mcp_command_line(&context.edge)
            )
        }),
    }
}

fn product_config_parent_exists(context: &HarnessContext, path: &Path) -> bool {
    let Some(parent) = path.parent().filter(|parent| parent.exists()) else {
        return false;
    };
    // A product-specific settings directory is useful installation evidence. A generic home or
    // configuration root is not: treating `$HOME` as Claude Code detection made that harness
    // appear installed on every ordinary machine and would make aggregate setup overreach.
    ![&context.home, &context.config, &context.roaming]
        .into_iter()
        .any(|root| paths_equal(parent, root, context.windows))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RegistrationState {
    Missing,
    Current,
    Updatable,
    /// An owned entry starts the retired `ghostlight-mcp-connector`, which no longer exists.
    Retired,
    /// An entry sits under Ghostlight's key that Ghostlight does not own. The payload names
    /// the program found there, when one could be read (ADR-0135); never its arguments.
    Foreign(Option<ForeignCommand>),
}

/// What Ghostlight discloses about a foreign entry's command: the program and how many
/// arguments follow it, and nothing of their text.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ForeignCommand {
    /// The bounded file name of the program.
    program: String,
    /// The `args` array length plus every further word of a whole-command-line `command`.
    arguments: usize,
}

impl ForeignCommand {
    /// Reduce a foreign entry's command to its program and argument count. A command that
    /// names an existing file is that program whole. Any other command is cut at its first
    /// whitespace before a file name is taken, so no word after the program is ever read as a
    /// path. A command with no file name part yields `None`.
    fn from_entry(command: &str, args: Option<&[&str]>) -> Option<Self> {
        let (program_path, extra_words) = if Path::new(command).is_file() {
            (command, 0)
        } else {
            let mut words = command.split_whitespace();
            let first = words.next()?;
            (first, words.count())
        };
        let file_name = Path::new(program_path).file_name()?.to_string_lossy();
        let program = bounded_disclosure(&file_name)?;
        Some(Self {
            program,
            arguments: args.map_or(0, <[&str]>::len) + extra_words,
        })
    }
}

/// "no arguments", "1 argument" or "<n> arguments".
fn argument_count_phrase(count: usize) -> String {
    match count {
        0 => "no arguments".into(),
        1 => "1 argument".into(),
        many => format!("{many} arguments"),
    }
}

fn registration_state(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<RegistrationState, HarnessError> {
    if !definition.path.exists() || matches!(definition.dialect, ConfigDialect::Manual(_)) {
        return Ok(RegistrationState::Missing);
    }
    let source = fs::read_to_string(&definition.path).map_err(HarnessError::Read)?;
    match definition.dialect {
        ConfigDialect::CodexToml => inspect_toml(&source, &context.edge, context.windows),
        ConfigDialect::Manual(_) => Ok(RegistrationState::Missing),
        ConfigDialect::Json(dialect) => {
            inspect_json(&source, dialect, &context.edge, context.windows)
        }
        ConfigDialect::OpenCode => inspect_json(
            &source,
            opencode_dialect(context, &source)?,
            &context.edge,
            context.windows,
        ),
    }
}

fn inspect_json(
    source: &str,
    dialect: JsonDialect,
    edge: &Path,
    windows: bool,
) -> Result<RegistrationState, HarnessError> {
    let parsed = parse_jsonc(source)?;
    let Some(entry) = json_entry(&parsed, dialect) else {
        return Ok(RegistrationState::Missing);
    };
    Ok(
        json_entry_command(entry, dialect).map_or(RegistrationState::Foreign(None), |command| {
            let args = json_entry_args(entry, dialect);
            command_registration_state(command, args.as_deref(), edge, windows)
        }),
    )
}

fn inspect_toml(
    source: &str,
    edge: &Path,
    windows: bool,
) -> Result<RegistrationState, HarnessError> {
    let document = parse_toml(source)?;
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
    let state = command_registration_state(command, args.as_deref(), edge, windows);
    if matches!(
        state,
        RegistrationState::Current | RegistrationState::Updatable
    ) && !missing_codex_desktop_environment(entry, windows)?.is_empty()
    {
        return Ok(RegistrationState::Updatable);
    }
    Ok(state)
}

fn missing_codex_desktop_environment(
    entry: &Item,
    windows: bool,
) -> Result<Vec<&'static str>, HarnessError> {
    if windows || !cfg!(target_os = "linux") {
        return Ok(Vec::new());
    }
    let forwarded = entry
        .get("env_vars")
        .map(|item| {
            let values = item.as_array().ok_or(HarnessError::Malformed(None))?;
            // Validate every member before deciding whether any handoff is missing.
            // Codex accepts a name or a closed name/source object, not arbitrary TOML.
            for value in values.iter() {
                let valid = value.as_str().is_some()
                    || value.as_inline_table().is_some_and(|table| {
                        table
                            .get("name")
                            .and_then(toml_edit::Value::as_str)
                            .is_some()
                            && table.get("source").is_none_or(|source| {
                                matches!(source.as_str(), Some("local" | "remote"))
                            })
                            && table
                                .iter()
                                .all(|(key, _)| matches!(key, "name" | "source"))
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
                .get("env")
                .and_then(|env| env.get(name))
                .and_then(Item::as_str)
                .is_some();
            let local = forwarded.is_some_and(|values| {
                values.iter().any(|value| {
                    value.as_str() == Some(name)
                        || value.as_inline_table().is_some_and(|table| {
                            table.get("name").and_then(toml_edit::Value::as_str) == Some(name)
                                && table
                                    .get("source")
                                    .and_then(toml_edit::Value::as_str)
                                    .is_none_or(|source| source == "local")
                        })
                })
            });
            !explicit && !local
        })
        .collect())
}

fn forward_codex_desktop_environment(entry: &mut Item, windows: bool) -> Result<(), HarnessError> {
    let missing = missing_codex_desktop_environment(entry, windows)?;
    if !missing.is_empty() {
        if entry.get("env_vars").is_none() {
            entry["env_vars"] = value(Array::new());
        }
        let forwarded = entry["env_vars"]
            .as_array_mut()
            .expect("the environment array was validated above");
        for name in missing {
            forwarded.push(name);
        }
    }
    Ok(())
}

fn apply_install(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<HarnessActionResult, HarnessError> {
    refuse_private(context, definition)?;
    if !context.edge.is_file() {
        return Err(HarnessError::EdgeMissing);
    }
    let changed = match definition.dialect {
        ConfigDialect::CodexToml => edit_toml(&definition.path, &context.edge, true)?,
        ConfigDialect::Manual(_) => return Err(manual_refusal(definition)),
        ConfigDialect::Json(dialect) => edit_json(&definition.path, &context.edge, dialect, true)?,
        ConfigDialect::OpenCode => {
            let source = read_or_empty(&definition.path)?;
            let dialect = opencode_dialect(context, &source)?;
            edit_json(&definition.path, &context.edge, dialect, true)?
        }
    };
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        message: if changed {
            format!(
                "Ghostlight was registered with {}. Restart or reconnect it to load the tools.",
                summary.label
            )
        } else {
            format!(
                "{} already has the current Ghostlight registration.",
                summary.label
            )
        },
        summary,
    })
}

fn apply_uninstall(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<HarnessActionResult, HarnessError> {
    refuse_private(context, definition)?;
    let changed = match definition.dialect {
        ConfigDialect::CodexToml => edit_toml(&definition.path, &context.edge, false)?,
        ConfigDialect::Manual(_) => return Err(manual_refusal(definition)),
        ConfigDialect::Json(dialect) => edit_json(&definition.path, &context.edge, dialect, false)?,
        ConfigDialect::OpenCode => {
            let source = read_or_empty(&definition.path)?;
            let dialect = opencode_dialect(context, &source)?;
            edit_json(&definition.path, &context.edge, dialect, false)?
        }
    };
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        message: if changed {
            format!(
                "Ghostlight was removed from {}. Restart or reconnect it to unload the tools.",
                summary.label
            )
        } else {
            format!(
                "{} had no owned Ghostlight registration to remove.",
                summary.label
            )
        },
        summary,
    })
}

fn apply_fix(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<HarnessActionResult, HarnessError> {
    refuse_private(context, definition)?;
    if !context.edge.is_file() {
        return Err(HarnessError::EdgeMissing);
    }
    let changed = match definition.dialect {
        ConfigDialect::CodexToml => fix_toml(&definition.path, &context.edge)?,
        ConfigDialect::Manual(_) => return Err(manual_refusal(definition)),
        ConfigDialect::Json(dialect) => fix_json(&definition.path, &context.edge, dialect)?,
        ConfigDialect::OpenCode => {
            let source = read_or_empty(&definition.path)?;
            let dialect = opencode_dialect(context, &source)?;
            fix_json(&definition.path, &context.edge, dialect)?
        }
    };
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        message: format!(
            "Ghostlight replaced the incorrect entry for {}. Restart or reconnect it to load the tools.",
            summary.label
        ),
        summary,
    })
}

/// Refuse a change that would reach only the private view of the app container this process runs
/// in.
fn refuse_private(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<(), HarnessError> {
    match context.private_to(definition) {
        Some(container) => Err(HarnessError::PrivateView(container.app.clone())),
        None => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistrationEdit {
    Install,
    Uninstall,
    Fix,
}

impl RegistrationEdit {
    const fn installs(self) -> bool {
        !matches!(self, Self::Uninstall)
    }

    const fn replaces_foreign(self) -> bool {
        matches!(self, Self::Fix)
    }
}

fn edit_json(
    path: &Path,
    edge: &Path,
    dialect: JsonDialect,
    install: bool,
) -> Result<bool, HarnessError> {
    edit_json_with(
        path,
        edge,
        dialect,
        if install {
            RegistrationEdit::Install
        } else {
            RegistrationEdit::Uninstall
        },
    )
}

fn fix_json(path: &Path, edge: &Path, dialect: JsonDialect) -> Result<bool, HarnessError> {
    edit_json_with(path, edge, dialect, RegistrationEdit::Fix)
}

fn edit_json_with(
    path: &Path,
    edge: &Path,
    dialect: JsonDialect,
    intent: RegistrationEdit,
) -> Result<bool, HarnessError> {
    let source = read_or_empty(path)?;
    let parsed = parse_jsonc(&source)?;
    let current = json_entry(&parsed, dialect);
    if let Some(entry) = current {
        let foreign = !json_entry_owned(entry, dialect, edge, cfg!(target_os = "windows"));
        if foreign && !intent.replaces_foreign() {
            return Err(HarnessError::ForeignEntry);
        }
        if !foreign && intent.replaces_foreign() {
            return Err(HarnessError::NotFixable);
        }
        if intent == RegistrationEdit::Install && entry == &expected_json_entry(edge, dialect) {
            return Ok(false);
        }
    } else if intent.replaces_foreign() {
        return Err(HarnessError::NotFixable);
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
        let entry = cst_input(expected_json_entry(edge, dialect));
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

fn edit_toml(path: &Path, edge: &Path, install: bool) -> Result<bool, HarnessError> {
    edit_toml_with(
        path,
        edge,
        if install {
            RegistrationEdit::Install
        } else {
            RegistrationEdit::Uninstall
        },
    )
}

fn fix_toml(path: &Path, edge: &Path) -> Result<bool, HarnessError> {
    edit_toml_with(path, edge, RegistrationEdit::Fix)
}

fn edit_toml_with(
    path: &Path,
    edge: &Path,
    intent: RegistrationEdit,
) -> Result<bool, HarnessError> {
    let source = read_or_empty(path)?;
    let mut document = parse_toml(&source)?;
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
        let foreign = !command_owned(command, args.as_deref(), edge, cfg!(target_os = "windows"));
        if foreign && !intent.replaces_foreign() {
            return Err(HarnessError::ForeignEntry);
        }
        if !foreign && intent.replaces_foreign() {
            return Err(HarnessError::NotFixable);
        }
        if intent == RegistrationEdit::Install
            && command == edge.to_string_lossy()
            && args.as_deref() == Some(&MCP_ARGUMENTS[..])
            && missing_codex_desktop_environment(entry, cfg!(windows))?.is_empty()
        {
            return Ok(false);
        }
    } else if intent.replaces_foreign() {
        return Err(HarnessError::NotFixable);
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
        updated["command"] = value(edge.to_string_lossy().into_owned());
        updated["args"] = value(mcp_arguments_array());
        forward_codex_desktop_environment(&mut updated, cfg!(windows))?;
        if !document.contains_key("mcp_servers") {
            document["mcp_servers"] = Item::Table(Table::new());
        }
        let servers = document["mcp_servers"]
            .as_table_mut()
            .ok_or(HarnessError::Malformed(None))?;
        servers[SERVER_NAME] = updated;
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
        JsonDialect::McpServers | JsonDialect::Copilot => root.get("mcpServers")?.get(SERVER_NAME),
        JsonDialect::Servers => root.get("servers")?.get(SERVER_NAME),
        JsonDialect::ContextServers => root.get("context_servers")?.get(SERVER_NAME),
        JsonDialect::Crush => root.get("mcp")?.get(SERVER_NAME),
        JsonDialect::OpenCodeV1 => root.get("mcp")?.get(SERVER_NAME),
        JsonDialect::OpenCodeV2 | JsonDialect::ZCode => {
            root.get("mcp")?.get("servers")?.get(SERVER_NAME)
        }
    }
}

fn expected_json_entry(edge: &Path, dialect: JsonDialect) -> Value {
    let command = edge.to_string_lossy().into_owned();
    let args = MCP_ARGUMENTS;
    match dialect {
        JsonDialect::Copilot => {
            json!({"type":"local","command":command,"args":args,"env":{},"tools":["*"]})
        }
        JsonDialect::Servers => json!({"type":"stdio","command":command,"args":args}),
        JsonDialect::Crush => json!({"type":"stdio","command":command,"args":args}),
        JsonDialect::McpServers | JsonDialect::ContextServers | JsonDialect::ZCode => {
            json!({"command":command,"args":args,"env":{}})
        }
        JsonDialect::OpenCodeV1 => {
            json!({"type":"local","command":[command, MCP_ARGUMENT],"enabled":true})
        }
        JsonDialect::OpenCodeV2 => json!({"type":"local","command":[command, MCP_ARGUMENT]}),
    }
}

fn json_collection(
    root: &CstObject,
    dialect: JsonDialect,
    create: bool,
) -> Result<Option<CstObject>, HarnessError> {
    if matches!(dialect, JsonDialect::OpenCodeV2 | JsonDialect::ZCode) {
        let mcp = if create {
            root.object_value_or_create("mcp")
        } else {
            root.object_value("mcp")
        };
        let Some(mcp) = mcp else {
            return if create && root.get("mcp").is_some() {
                Err(HarnessError::Malformed(None))
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
            Err(HarnessError::Malformed(None))
        } else {
            Ok(servers)
        };
    }
    let key = match dialect {
        JsonDialect::McpServers | JsonDialect::Copilot => "mcpServers",
        JsonDialect::Servers => "servers",
        JsonDialect::ContextServers => "context_servers",
        JsonDialect::Crush | JsonDialect::OpenCodeV1 => "mcp",
        JsonDialect::OpenCodeV2 | JsonDialect::ZCode => unreachable!("handled above"),
    };
    let collection = if create {
        root.object_value_or_create(key)
    } else {
        root.object_value(key)
    };
    if create && collection.is_none() && root.get(key).is_some() {
        return Err(HarnessError::Malformed(None));
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

fn json_entry_owned(entry: &Value, dialect: JsonDialect, edge: &Path, windows: bool) -> bool {
    json_entry_command(entry, dialect).is_some_and(|command| {
        let args = json_entry_args(entry, dialect);
        command_owned(command, args.as_deref(), edge, windows)
    })
}

fn json_entry_command(entry: &Value, dialect: JsonDialect) -> Option<&str> {
    match dialect {
        JsonDialect::McpServers
        | JsonDialect::Copilot
        | JsonDialect::Servers
        | JsonDialect::ContextServers
        | JsonDialect::Crush
        | JsonDialect::ZCode => entry.get("command").and_then(Value::as_str),
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
        | JsonDialect::Crush
        | JsonDialect::ZCode => entry.get("args")?.as_array()?,
        JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => {
            let command = entry.get("command")?.as_array()?;
            command.get(1..)?
        }
    };
    values.iter().map(Value::as_str).collect()
}

/// The sentence a person reads for a client whose file Ghostlight never edits.
fn manual_setup_sentence(definition: &HarnessDefinition) -> String {
    let file = definition.path.file_name().map_or_else(
        || "its configuration file".into(),
        |name| name.to_string_lossy(),
    );
    format!("Ghostlight does not edit this client's file. Copy setup and paste it into {file}.")
}

fn manual_refusal(definition: &HarnessDefinition) -> HarnessError {
    HarnessError::ManualSetup(manual_setup_sentence(definition))
}

fn manual_setup(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<String, HarnessError> {
    match definition.dialect {
        ConfigDialect::CodexToml => {
            let mut entry = Item::Table(Table::new());
            entry["command"] = value(context.edge.to_string_lossy().into_owned());
            entry["args"] = value(mcp_arguments_array());
            forward_codex_desktop_environment(&mut entry, context.windows)?;
            let mut document = DocumentMut::new();
            document["mcp_servers"] = Item::Table(Table::new());
            document["mcp_servers"][SERVER_NAME] = entry;
            Ok(document.to_string())
        }
        ConfigDialect::Json(dialect) => manual_json_setup(&context.edge, dialect),
        ConfigDialect::Manual(dialect) => Ok(manual_yaml_setup(&context.edge, dialect)),
        ConfigDialect::OpenCode => {
            let source = read_or_empty(&definition.path)?;
            manual_json_setup(&context.edge, opencode_dialect(context, &source)?)
        }
    }
}

fn manual_json_setup(edge: &Path, dialect: JsonDialect) -> Result<String, HarnessError> {
    let entry = expected_json_entry(edge, dialect);
    let document = match dialect {
        JsonDialect::McpServers | JsonDialect::Copilot => {
            json!({"mcpServers": {SERVER_NAME: entry}})
        }
        JsonDialect::Servers => json!({"servers": {SERVER_NAME: entry}}),
        JsonDialect::ContextServers => json!({"context_servers": {SERVER_NAME: entry}}),
        JsonDialect::Crush | JsonDialect::OpenCodeV1 => json!({"mcp": {SERVER_NAME: entry}}),
        JsonDialect::OpenCodeV2 | JsonDialect::ZCode => {
            json!({"mcp": {"servers": {SERVER_NAME: entry}}})
        }
    };
    serde_json::to_string_pretty(&document).map_err(|_| HarnessError::Malformed(None))
}

fn manual_yaml_setup(edge: &Path, dialect: ManualDialect) -> String {
    let command = serde_json::to_string(&edge.to_string_lossy())
        .expect("a filesystem path is always representable as a JSON/YAML string");
    let arguments = serde_json::to_string(&MCP_ARGUMENTS)
        .expect("fixed arguments are always representable as a JSON/YAML string");
    match dialect {
        ManualDialect::Goose => format!(
            "extensions:\n  ghostlight:\n    type: stdio\n    name: ghostlight\n    display_name: Ghostlight\n    enabled: true\n    cmd: {command}\n    args: {arguments}\n    envs: {{}}\n    timeout: 300\n"
        ),
        ManualDialect::Continue => format!(
            "name: Local Config\nversion: 1.0.0\nschema: v1\nmcpServers:\n  - name: Ghostlight\n    command: {command}\n    args: {arguments}\n"
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

fn command_owned(command: &str, args: Option<&[&str]>, edge: &Path, windows: bool) -> bool {
    command_name(command, EDGE_COMMAND_NAME)
        || command_name(command, RETIRED_MCP_CONNECTOR_NAME)
        || legacy_relay_owned(command, args, edge, windows)
}

fn legacy_relay_owned(command: &str, args: Option<&[&str]>, edge: &Path, windows: bool) -> bool {
    if !command_name(command, "ghostlight-relay") || args != Some(&["--role", "agent"]) {
        return false;
    }
    let Some(actual_root) = Path::new(command).parent().and_then(Path::parent) else {
        return false;
    };
    let Some(expected_root) = edge.parent().and_then(Path::parent) else {
        return false;
    };
    paths_equal(actual_root, expected_root, windows)
}

fn command_registration_state(
    command: &str,
    args: Option<&[&str]>,
    edge: &Path,
    windows: bool,
) -> RegistrationState {
    if !command_owned(command, args, edge, windows) {
        return RegistrationState::Foreign(ForeignCommand::from_entry(command, args));
    }
    let actual = fs::canonicalize(command).unwrap_or_else(|_| PathBuf::from(command));
    let expected = fs::canonicalize(edge).unwrap_or_else(|_| edge.to_path_buf());
    let current = command_name(command, EDGE_COMMAND_NAME)
        && paths_equal(&actual, &expected, windows)
        && args == Some(&MCP_ARGUMENTS[..]);
    if current {
        RegistrationState::Current
    } else if command_name(command, RETIRED_MCP_CONNECTOR_NAME) {
        RegistrationState::Retired
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

/// Maximum visible characters disclosed from a blocked target's existing content (ADR-0135).
const EVIDENCE_MAX_CHARS: usize = 200;

/// Bound one piece of found content for disclosure.
///
/// Control characters and bidi overrides are stripped, whitespace collapses to single spaces,
/// and the result is capped, so file-authored text can never smuggle formatting into a card.
fn bounded_disclosure(value: &str) -> Option<String> {
    let cleaned: String = value
        .chars()
        .filter(|character| {
            (!character.is_control() || character.is_whitespace())
                && !matches!(
                    character,
                    '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                )
        })
        .collect();
    let one_line = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        return None;
    }
    let count = one_line.chars().count();
    if count <= EVIDENCE_MAX_CHARS {
        return Some(one_line);
    }
    let mut bounded: String = one_line.chars().take(EVIDENCE_MAX_CHARS - 3).collect();
    bounded.push_str("...");
    Some(bounded)
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

/// What a person reads when a harness configuration cannot be parsed. It names the line of
/// the parse error and nothing the file says.
fn malformed_sentence(line: Option<usize>) -> String {
    match line {
        Some(line) => {
            format!("This configuration could not be parsed at line {line}; it was left untouched.")
        }
        None => "This configuration could not be parsed; it was left untouched.".into(),
    }
}

/// A JSON-with-comments parse error reduced to its line.
fn malformed_json_at(error: jsonc_parser::errors::ParseError) -> HarnessError {
    HarnessError::Malformed(Some(error.line_display()))
}

/// Parse a TOML configuration. A parse error is reduced to the line its span starts on.
fn parse_toml(source: &str) -> Result<DocumentMut, HarnessError> {
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

fn parse_jsonc(source: &str) -> Result<Value, HarnessError> {
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
    if matches!(dialect, ConfigDialect::Manual(_)) {
        return Ok(());
    }
    let source = fs::read_to_string(path).map_err(HarnessError::Read)?;
    match dialect {
        ConfigDialect::CodexToml => parse_toml(&source).map(|_| ()),
        ConfigDialect::Json(_) | ConfigDialect::OpenCode => parse_jsonc(&source).map(|_| ()),
        ConfigDialect::Manual(_) => Ok(()),
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
        fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        false
    }
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

/// The command line that starts the MCP relay of the edge at `edge`: its path, quoted when the
/// path holds a space, then the `mcp` argument.
fn mcp_command_line(edge: &Path) -> String {
    let path = edge.to_string_lossy();
    let path = if path.contains(char::is_whitespace) {
        format!("\"{path}\"")
    } else {
        path.into_owned()
    };
    format!("{path} {}", MCP_ARGUMENTS.join(" "))
}

/// The arguments of an edge entry as a TOML array.
fn mcp_arguments_array() -> Array {
    MCP_ARGUMENTS.iter().copied().collect()
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
    /// The installed edge is unavailable.
    #[error("the Ghostlight edge is missing")]
    EdgeMissing,
    /// The harness config could not be read.
    #[error("could not read harness configuration: {0}")]
    Read(io::Error),
    /// The harness config could not be written.
    #[error("could not write harness configuration: {0}")]
    Write(io::Error),
    /// The config is not structurally safe to edit. The payload is the 1-indexed line of the
    /// parse error, when the parser located one. Nothing the file says is ever carried.
    #[error("{}", malformed_sentence(*.0))]
    Malformed(Option<usize>),
    /// The client's file is never edited by Ghostlight; the person pastes the setup.
    #[error("{0}")]
    ManualSetup(String),
    /// A foreign server uses Ghostlight's registration name.
    #[error("a foreign `ghostlight` entry is present; Ghostlight left it untouched")]
    ForeignEntry,
    /// The explicit fix no longer points at one foreign entry.
    #[error(
        "the Ghostlight entry changed and is no longer eligible for Fix; re-check integrations"
    )]
    NotFixable,
    /// A located path is neither a matching configuration nor an executable.
    #[error("the selected path is not a usable harness executable or configuration: {0}")]
    LocatedPathInvalid(PathBuf),
    /// Ghostlight's own located-path state is malformed.
    #[error("Ghostlight's harness location state is malformed: {0}")]
    MalformedLocationState(String),
    /// A path has no writable parent.
    #[error("harness configuration path has no parent: {0}")]
    InvalidPath(PathBuf),
    /// The change would reach only the private view of an app container.
    #[error(
        "Windows keeps this configuration private to {0}, so a change made from here would not \
         reach other apps; run Ghostlight's install from a regular terminal"
    )]
    PrivateView(String),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use jsonc_parser::parse_to_serde_value;

    use super::{
        command_registration_state, definitions, edit_json, edit_toml, executable_name,
        expected_json_entry, fix_toml, harness_roots, inspect, inspect_json, inspect_toml,
        json_entry_args, json_entry_command, jsonc_options, manual_json_setup, mcp_command_line,
        replace_with_backup, resolve_through_symlink, AppContainer, ForeignCommand, HarnessAction,
        HarnessContext, HarnessCopyKind, HarnessError, HarnessRegistry, HarnessRoots, HarnessState,
        HarnessSummary, JsonDialect, PrivateView, RegistrationState, EDGE_COMMAND_NAME,
        RETIRED_MCP_CONNECTOR_NAME,
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

    fn edge(directory: &Path) -> PathBuf {
        let edge = directory.join(executable_name(EDGE_COMMAND_NAME, cfg!(windows)));
        fs::write(&edge, b"test").unwrap();
        edge
    }

    fn context(directory: &Path) -> HarnessContext {
        HarnessContext {
            home: directory.join("home"),
            config: directory.join("config"),
            roaming: directory.join("config"),
            codex_config: directory.join("codex/config.toml"),
            edge: edge(directory),
            path_entries: Vec::new(),
            locations: directory.join("config/ghostlight/harness-locations.json"),
            windows: false,
            private_view: None,
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
        let edge = edge(&directory);
        let registry = HarnessRegistry::with_context(HarnessContext {
            home: directory.join("home"),
            config: directory.join("config"),
            roaming: directory.join("config"),
            codex_config: directory.join("codex/config.toml"),
            edge,
            path_entries: vec![binary_directory],
            locations: directory.join("locations.json"),
            windows: false,
            private_view: None,
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
            "zcode",
            "devin",
        ] {
            assert!(rows.iter().any(|row| row.id == id), "missing {id}");
        }
        assert_eq!(
            rows.iter().filter(|row| row.product_id == "cline").count(),
            4
        );
        let devin = rows.iter().find(|row| row.id == "devin").unwrap();
        assert_eq!(devin.product_id, "windsurf");
        assert_eq!(devin.path, context.config.join("devin/mcp_config.json"));
        assert_eq!(devin.executables, ["devin", "devin-desktop", "surf"]);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn every_registry_row_has_offline_artwork_and_the_exact_manual_command() {
        let directory = temporary("roster-artwork-manual");
        let context = context(&directory);
        let edge = context.edge.to_string_lossy().into_owned();
        let serialized_edge = serde_json::to_string(&edge).unwrap();
        let escaped_edge = serialized_edge
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
            assert_eq!(summary.connector_command, format!("{edge} mcp"));
            assert!(
                summary.manual_setup.contains(&edge) || summary.manual_setup.contains(escaped_edge),
                "manual setup for {} omitted the edge",
                summary.id
            );
            assert!(
                summary.manual_setup.contains("\"mcp\""),
                "manual setup for {} omitted the mcp argument",
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
    fn locating_a_manual_clients_file_points_to_copy_setup_and_leaves_it_untouched() {
        let directory = temporary("goose-locate");
        let context = context(&directory);
        let registry = HarnessRegistry::with_context(context);
        let located = directory.join("portable/config.yaml");
        fs::create_dir_all(located.parent().unwrap()).unwrap();
        let source = "# keep goose notes\nprovider: local\n";
        fs::write(&located, source).unwrap();
        let result = registry.locate("goose", &located).unwrap();
        assert_eq!(result.summary.config_path, located.to_string_lossy());
        assert!(result.message.contains("Copy setup"));
        assert!(!result.message.contains("Set up"));
        assert_eq!(fs::read_to_string(&located).unwrap(), source);
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn json_edits_preserve_the_files_permission_bits() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temporary("json-mode");
        let path = directory.join("config.json");
        let edge = edge(&directory);
        fs::write(&path, "{\"theme\":\"dark\"}\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(edit_json(&path, &edge, JsonDialect::Copilot, true).unwrap());
        let installed = fs::read(&path).unwrap();
        assert!(!edit_json(&path, &edge, JsonDialect::Copilot, true).unwrap());
        assert_eq!(fs::read(&path).unwrap(), installed);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert!(edit_json(&path, &edge, JsonDialect::Copilot, false).unwrap());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn manual_json_setup_uses_the_real_ghostlight_key() {
        let directory = temporary("manual-json");
        let edge = edge(&directory);
        let document = manual_json_setup(&edge, JsonDialect::Copilot).unwrap();
        let value: Value = serde_json::from_str(&document).unwrap();
        assert!(value["mcpServers"].get("ghostlight").is_some());
        assert!(value["mcpServers"].get("SERVER_NAME").is_none());
        assert_eq!(
            value["mcpServers"]["ghostlight"]["command"].as_str(),
            Some(edge.to_string_lossy().as_ref())
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

    #[test]
    fn a_generic_home_directory_is_not_product_detection_evidence() {
        let directory = temporary("generic-home-detection");
        let mut context = context(&directory);
        fs::create_dir_all(&context.home).unwrap();
        let binaries = directory.join("bin");
        fs::create_dir_all(&binaries).unwrap();
        context.path_entries = vec![binaries.clone()];

        let absent = HarnessRegistry::with_context(context.clone())
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "claude-code")
            .unwrap();
        assert_eq!(absent.state, HarnessState::NotDetected);

        fs::write(binaries.join("claude"), b"test").unwrap();
        let detected = HarnessRegistry::with_context(context)
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "claude-code")
            .unwrap();
        assert_eq!(detected.state, HarnessState::Available);
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

    #[test]
    fn devin_setup_does_not_touch_the_historical_windsurf_target() {
        let directory = temporary("devin-windsurf-continuity");
        let context = context(&directory);
        let devin = context.config.join("devin/mcp_config.json");
        let windsurf = context.home.join(".codeium/windsurf/mcp_config.json");
        fs::create_dir_all(devin.parent().unwrap()).unwrap();
        fs::create_dir_all(windsurf.parent().unwrap()).unwrap();
        fs::write(&windsurf, "{\n  \"mcpServers\": {}\n}\n").unwrap();

        let registry = HarnessRegistry::with_context(context.clone());
        registry.apply("devin", HarnessAction::Install).unwrap();

        let configured: Value = serde_json::from_str(&fs::read_to_string(&devin).unwrap()).unwrap();
        assert_eq!(
            configured["mcpServers"]["ghostlight"]["command"].as_str(),
            Some(context.edge.to_string_lossy().as_ref())
        );
        assert_eq!(
            fs::read_to_string(&windsurf).unwrap(),
            "{\n  \"mcpServers\": {}\n}\n"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn setup_detected_adds_missing_updates_owned_and_preserves_blocked_targets() {
        let directory = temporary("setup-detected");
        let mut context = context(&directory);
        let binaries = directory.join("bin");
        fs::create_dir_all(&binaries).unwrap();
        fs::write(binaries.join("qwen"), b"test").unwrap();
        fs::write(binaries.join("zcode"), b"test").unwrap();
        context.path_entries = vec![binaries];

        let codex = context.codex_config.clone();
        fs::create_dir_all(codex.parent().unwrap()).unwrap();
        let old_connector = directory.join("old/ghostlight-mcp-connector");
        fs::write(
            &codex,
            format!(
                "[mcp_servers.ghostlight]\ncommand = {}\nargs = []\n",
                serde_json::to_string(&old_connector.to_string_lossy()).unwrap()
            ),
        )
        .unwrap();

        let blocked = context
            .home
            .join(".cline/data/settings/cline_mcp_settings.json");
        fs::create_dir_all(blocked.parent().unwrap()).unwrap();
        fs::write(
            &blocked,
            r#"{"mcpServers":{"ghostlight":{"command":"alien-agent","args":[]}}}"#,
        )
        .unwrap();

        let registry = HarnessRegistry::with_context(context);
        let actionable: Vec<_> = registry
            .summaries()
            .into_iter()
            .filter(|summary| {
                matches!(
                    summary.state,
                    HarnessState::Available | HarnessState::Updatable
                )
            })
            .map(|summary| (summary.id, summary.state))
            .collect();
        assert_eq!(
            actionable,
            vec![
                ("codex".into(), HarnessState::Updatable),
                ("zcode".into(), HarnessState::Available),
                ("qwen-code".into(), HarnessState::Available),
            ]
        );
        let result = registry.setup_detected().unwrap();
        assert_eq!(result.set_up, 2);
        assert_eq!(result.updated, 1);
        assert_eq!(result.needs_attention, 1);
        assert!(result.failures.is_empty());
        assert!(result.message.contains("set up 2 and updated 1"));

        let summaries = registry.summaries();
        for id in ["codex", "qwen-code", "zcode"] {
            assert_eq!(
                summaries
                    .iter()
                    .find(|summary| summary.id == id)
                    .unwrap()
                    .state,
                HarnessState::Installed,
                "{id}"
            );
        }
        assert_eq!(
            summaries
                .iter()
                .find(|summary| summary.id == "cline-cli")
                .unwrap()
                .state,
            HarnessState::NeedsAttention
        );
        assert_eq!(
            summaries
                .iter()
                .find(|summary| summary.id == "cursor")
                .unwrap()
                .state,
            HarnessState::NotDetected
        );

        let repeated = registry.setup_detected().unwrap();
        assert_eq!(repeated.set_up, 0);
        assert_eq!(repeated.updated, 0);
        assert!(repeated.message.contains("already ready"));
        assert_eq!(
            fs::read_to_string(&blocked).unwrap(),
            r#"{"mcpServers":{"ghostlight":{"command":"alien-agent","args":[]}}}"#
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn goose_and_continue_get_copy_setup_and_their_files_are_never_touched() {
        for (id, relative, source) in [
            (
                "goose",
                "config/goose/config.yaml",
                "provider: keep\nextensions: {a: 1}\n",
            ),
            (
                "continue",
                "home/.continue/config.yaml",
                "name: x\nmcpServers: [broken\n",
            ),
        ] {
            let directory = temporary("manual-clients");
            let context = context(&directory);
            let path = directory.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, source).unwrap();

            let registry = HarnessRegistry::with_context(context.clone());
            let summary = registry
                .summaries()
                .into_iter()
                .find(|summary| summary.id == id)
                .expect("the client stays in the closed roster");
            assert_eq!(summary.state, HarnessState::Available, "{id}");
            assert!(!summary.can_install && !summary.can_fix && !summary.can_uninstall);
            assert!(summary.detail.contains("Copy setup"), "{}", summary.detail);
            assert!(summary
                .manual_setup
                .contains(&*context.edge.to_string_lossy().replace('\\', "\\\\")));

            let result = registry.setup_detected().unwrap();
            assert_eq!(result.set_up, 0);
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
            for action in [
                HarnessAction::Install,
                HarnessAction::Fix,
                HarnessAction::Uninstall,
            ] {
                assert!(matches!(
                    registry.apply(id, action),
                    Err(HarnessError::ManualSetup(_))
                ));
                assert_eq!(fs::read_to_string(&path).unwrap(), source);
            }
            fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn goose_and_continue_with_nothing_present_are_not_detected() {
        let directory = temporary("manual-absent");
        let registry = HarnessRegistry::with_context(context(&directory));
        for id in ["goose", "continue"] {
            let summary = registry
                .summaries()
                .into_iter()
                .find(|summary| summary.id == id)
                .unwrap();
            assert_eq!(summary.state, HarnessState::NotDetected, "{id}");
            assert!(!summary.can_install);
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn legacy_relay_requires_the_exact_agent_signature_and_install_root() {
        let directory = temporary("legacy-relay");
        let install_root = directory.join("bin");
        let current = install_root
            .join("v1")
            .join(executable_name(EDGE_COMMAND_NAME, cfg!(windows)));
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
        assert!(matches!(
            command_registration_state(
                legacy.to_string_lossy().as_ref(),
                Some(&["--role", "browser"]),
                &current,
                cfg!(windows),
            ),
            RegistrationState::Foreign(Some(_))
        ));
        assert!(matches!(
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
            RegistrationState::Foreign(Some(_))
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn json_install_is_idempotent_and_preserves_siblings() {
        let directory = temporary("json");
        let path = directory.join("mcp.json");
        let edge = edge(&directory);
        fs::write(
            &path,
            r#"{"other":7,"mcpServers":{"sibling":{"command":"sibling"}}}"#,
        )
        .unwrap();
        assert!(edit_json(&path, &edge, JsonDialect::McpServers, true).unwrap());
        assert!(!edit_json(&path, &edge, JsonDialect::McpServers, true).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["other"], 7);
        assert_eq!(value["mcpServers"]["sibling"]["command"], "sibling");
        assert_eq!(
            inspect_json(
                &fs::read_to_string(&path).unwrap(),
                JsonDialect::McpServers,
                &edge,
                cfg!(windows)
            )
            .unwrap(),
            RegistrationState::Current
        );
        assert!(edit_json(&path, &edge, JsonDialect::McpServers, false).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(value["mcpServers"].get("ghostlight").is_none());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn jsonc_install_and_uninstall_preserve_comments_and_trailing_commas() {
        let directory = temporary("jsonc");
        let path = directory.join("settings.json");
        let edge = edge(&directory);
        let source = "{\n  // keep this thought\n  \"context_servers\": {},\n}\n";
        fs::write(&path, source).unwrap();
        assert!(edit_json(&path, &edge, JsonDialect::ContextServers, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        assert!(installed.contains("// keep this thought"));
        assert!(installed.contains("\"ghostlight\""));
        assert_eq!(
            inspect_json(
                &installed,
                JsonDialect::ContextServers,
                &edge,
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
        assert!(edit_json(&path, &edge, JsonDialect::ContextServers, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("// keep this thought"));
        assert!(!removed.contains("\"ghostlight\""));
        assert!(removed.contains("\"context_servers\": {},"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn zcode_nested_install_and_uninstall_preserve_sibling_servers() {
        let directory = temporary("zcode");
        let path = directory.join("config.json");
        let edge = edge(&directory);
        let source = "{\n  \"mcp\": {\n    \"servers\": {\n      \"other\": {\"command\": \"keep\"}\n    }\n  }\n}\n";
        fs::write(&path, source).unwrap();
        assert!(edit_json(&path, &edge, JsonDialect::ZCode, true).unwrap());
        assert!(!edit_json(&path, &edge, JsonDialect::ZCode, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        assert!(installed.contains("\"other\""));
        assert_eq!(
            inspect_json(&installed, JsonDialect::ZCode, &edge, cfg!(windows)).unwrap(),
            RegistrationState::Current
        );
        // Pinned against ZCode's own written shape, observed live in ~/.zcode/cli/config.json
        // on 2026-08-27 (ADR-0141): the entry is a plain string command with args and env,
        // nested under `mcp.servers`, exactly like the McpServers dialect but one level deeper.
        let written: Value = parse_to_serde_value(&installed, &jsonc_options())
            .unwrap()
            .unwrap();
        let entry = &written["mcp"]["servers"]["ghostlight"];
        assert!(entry["command"].is_string());
        assert!(entry["args"].is_array());
        assert!(entry["env"].is_object());
        assert!(edit_json(&path, &edge, JsonDialect::ZCode, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("\"other\""));
        assert!(!removed.contains("\"ghostlight\""));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn zcode_is_detected_by_its_cli_configuration_directory() {
        let directory = temporary("zcode-detection");
        let context = context(&directory);
        let config = context.home.join(".zcode/cli/config.json");
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, "{}\n").unwrap();
        let registry = HarnessRegistry::with_context(context);
        let zcode = registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "zcode")
            .expect("zcode stays in the closed roster");
        assert_eq!(zcode.state, HarnessState::Available);
        assert!(zcode.can_install);
        assert!(
            !zcode.can_download,
            "no verified download destination exists"
        );
        assert_eq!(zcode.config_path, config.to_string_lossy());
        assert!(zcode.manual_setup.contains("\"mcp\""));
        assert!(zcode.manual_setup.contains("\"servers\""));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_hand_registered_orchestrator_command_stays_foreign() {
        // Pinned against the live 2026-08-27 incident (ADR-0141): ZCode was registered by hand
        // with the orchestrator binary, which has no MCP stdio mode. Ghostlight must report the
        // entry as foreign evidence. Ordinary install and remove must never overwrite it; only
        // the separately confirmed Fix action may do that.
        let directory = temporary("zcode-foreign-orchestrator");
        let path = directory.join("config.json");
        let edge = edge(&directory);
        let orchestrator = directory
            .join(executable_name("ghostlight-service", cfg!(windows)))
            .to_string_lossy()
            .replace('\\', "\\\\");
        let source = format!(
            "{{\"mcp\":{{\"servers\":{{\"ghostlight\":{{\"command\":\"{orchestrator}\",\"args\":[],\"env\":{{}}}}}}}}}}"
        );
        fs::write(&path, &source).unwrap();
        for install in [true, false] {
            assert!(matches!(
                edit_json(&path, &edge, JsonDialect::ZCode, install),
                Err(HarnessError::ForeignEntry)
            ));
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        let state = inspect_json(&source, JsonDialect::ZCode, &edge, cfg!(windows)).unwrap();
        assert!(
            matches!(state, RegistrationState::Foreign(Some(_))),
            "{state:?}"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn foreign_entries_are_never_overwritten_or_removed() {
        let directory = temporary("foreign");
        let path = directory.join("mcp.json");
        let edge = edge(&directory);
        let source = r#"{"mcpServers":{"ghostlight":{"command":"some-other-tool"}}}"#;
        fs::write(&path, source).unwrap();
        for install in [true, false] {
            assert!(matches!(
                edit_json(&path, &edge, JsonDialect::McpServers, install),
                Err(HarnessError::ForeignEntry)
            ));
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn explicit_fix_replaces_only_the_foreign_entry_and_keeps_a_backup() {
        let directory = temporary("fix-foreign");
        let context = context(&directory);
        let path = context
            .home
            .join(".cline/data/settings/cline_mcp_settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source = r#"{
  "keep": true,
  "mcpServers": {
    "sibling": {"command": "keep-me"},
    "ghostlight": {"command": "npx", "args": ["-y", "ghostlight"]}
  }
}
"#;
        fs::write(&path, source).unwrap();
        let registry = HarnessRegistry::with_context(context.clone());
        let before = registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "cline-cli")
            .expect("Cline CLI stays in the closed roster");
        assert_eq!(before.state, HarnessState::NeedsAttention);
        assert!(before.can_fix);
        assert!(!before.can_install);

        let result = registry.apply("cline-cli", HarnessAction::Fix).unwrap();
        assert!(result.changed);
        assert_eq!(result.summary.state, HarnessState::Installed);
        assert!(!result.summary.can_fix);
        assert!(result.message.contains("replaced the incorrect entry"));

        let fixed = fs::read_to_string(&path).unwrap();
        let parsed = parse_to_serde_value(&fixed, &jsonc_options())
            .unwrap()
            .unwrap();
        assert_eq!(parsed["keep"], true);
        assert_eq!(parsed["mcpServers"]["sibling"]["command"], "keep-me");
        assert_eq!(
            inspect_json(
                &fixed,
                JsonDialect::McpServers,
                &context.edge,
                context.windows
            )
            .unwrap(),
            RegistrationState::Current
        );
        assert_eq!(
            fs::read_to_string(path.with_extension("json.ghostlight-backup")).unwrap(),
            source
        );

        let unchanged = fixed.clone();
        assert!(matches!(
            registry.apply("cline-cli", HarnessAction::Fix),
            Err(HarnessError::NotFixable)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), unchanged);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn explicit_fix_is_lossless_for_the_toml_dialect() {
        let directory = temporary("fix-foreign-toml");
        let edge = edge(&directory);

        let toml = directory.join("config.toml");
        let toml_source = "title = \"keep\"\n\n[mcp_servers.sibling]\ncommand = \"keep-me\"\n\n[mcp_servers.ghostlight]\ncommand = \"alien\"\nargs = [\"serve\"]\n";
        fs::write(&toml, toml_source).unwrap();
        assert!(fix_toml(&toml, &edge).unwrap());
        let fixed_toml = fs::read_to_string(&toml).unwrap();
        assert!(fixed_toml.contains("title = \"keep\""));
        assert!(fixed_toml.contains("[mcp_servers.sibling]"));
        assert_eq!(
            inspect_toml(&fixed_toml, &edge, cfg!(windows)).unwrap(),
            RegistrationState::Current
        );
        assert_eq!(
            fs::read_to_string(toml.with_extension("toml.ghostlight-backup")).unwrap(),
            toml_source
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn fix_has_one_closed_desktop_action_name() {
        assert_eq!(
            serde_json::from_value::<HarnessAction>(serde_json::json!("fix")).unwrap(),
            HarnessAction::Fix
        );
        assert!(serde_json::from_value::<HarnessAction>(serde_json::json!("force")).is_err());
    }

    /// A blocked card names what was actually found beside the claim (ADR-0135).
    fn inspected_summary(directory: &Path) -> HarnessSummary {
        inspected_summary_of(directory, "claude-desktop")
    }

    fn inspected_summary_of(directory: &Path, id: &str) -> HarnessSummary {
        let context = context(directory);
        let definition = definitions(&context)
            .into_iter()
            .find(|definition| definition.id == id)
            .expect("the target stays in the closed roster");
        inspect(&context, &definition)
    }

    fn claude_desktop_config(directory: &Path) -> PathBuf {
        let path = directory
            .join("config")
            .join("Claude/claude_desktop_config.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        path
    }

    #[test]
    fn coexisting_hosts_of_one_client_carry_distinct_labels() {
        let directory = temporary("host-labels");
        let summaries = HarnessRegistry::with_context(context(&directory))
            .refresh()
            .unwrap();
        let label_of = |id: &str| {
            summaries
                .iter()
                .find(|summary| summary.id == id)
                .unwrap()
                .label
                .clone()
        };
        assert_eq!(label_of("cline-cursor"), "Cline (Cursor)");
        assert_eq!(label_of("cline-cli"), "Cline (CLI)");
        assert_eq!(label_of("claude-desktop"), "Claude Desktop");
        assert_eq!(label_of("windsurf"), "Windsurf");
        assert_eq!(label_of("devin"), "Devin");
        let mut labels: Vec<_> = summaries.iter().map(|summary| &summary.label).collect();
        let total = labels.len();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), total);
    }

    #[test]
    fn foreign_entries_name_only_the_program_and_its_argument_count() {
        let directory = temporary("foreign-evidence");
        let path = claude_desktop_config(&directory);
        let expected_edge = edge(&directory);
        let command = if cfg!(windows) {
            "C:/tools/other\u{202e}-server.exe"
        } else {
            "/opt/tools/other\u{202e}-server"
        };
        let entry = serde_json::json!({
            "mcpServers": {
                "ghostlight": {
                    "command": command,
                    "args": ["--token", "sk-test-secret", "launch-it"],
                }
            }
        });
        fs::write(&path, entry.to_string()).unwrap();

        let summary = inspected_summary(&directory);

        assert_eq!(summary.state, HarnessState::NeedsAttention);
        assert!(
            summary.detail.starts_with("A foreign entry holds"),
            "{}",
            summary.detail
        );
        let evidence = summary.evidence.expect("foreign entries carry evidence");
        assert!(evidence.contains("other-server"), "{evidence}");
        assert!(!evidence.contains('\u{202e}'), "{evidence}");
        assert!(evidence.contains("3 arguments"), "{evidence}");
        assert!(
            evidence.contains(&expected_edge.display().to_string()),
            "{evidence}"
        );
        assert!(evidence.contains("changed nothing"), "{evidence}");
        for hidden in ["sk-test-secret", "--token", "launch-it"] {
            assert!(!evidence.contains(hidden), "{evidence}");
        }
        assert!(!summary.can_install);
        assert!(summary.can_fix);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn argument_counts_read_in_plain_words() {
        let directory = temporary("foreign-count");
        let path = claude_desktop_config(&directory);
        edge(&directory);
        let write = |entry: serde_json::Value| {
            fs::write(
                &path,
                serde_json::json!({ "mcpServers": { "ghostlight": entry } }).to_string(),
            )
            .unwrap();
            inspected_summary(&directory).evidence.expect("evidence")
        };

        let one = write(serde_json::json!({ "command": "/opt/tools/other", "args": ["a"] }));
        assert!(one.contains("with 1 argument,"), "{one}");
        let none = write(serde_json::json!({ "command": "/opt/tools/other" }));
        assert!(none.contains("with no arguments,"), "{none}");

        let line = write(serde_json::json!({
            "command": "C:/tools/other-server.exe --token sk-test-secret"
        }));
        assert!(line.contains("`other-server.exe`"), "{line}");
        assert!(line.contains("2 arguments"), "{line}");
        assert!(
            !line.contains("--token") && !line.contains("sk-test-secret"),
            "{line}"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn malformed_configurations_name_the_line_and_disclose_nothing_from_the_file() {
        let directory = temporary("malformed-evidence");
        let path = claude_desktop_config(&directory);
        edge(&directory);
        fs::write(&path, "{\n  \"token\" \"sk-test-secret\"\n}\n").unwrap();

        let summary = inspected_summary(&directory);

        assert_eq!(summary.state, HarnessState::NeedsAttention);
        assert_eq!(
            summary.detail,
            "This configuration could not be parsed at line 2; it was left untouched."
        );
        assert!(summary.evidence.is_none(), "{:?}", summary.evidence);
        assert!(!summary.can_fix);
        let serialized = serde_json::to_string(&summary).unwrap();
        assert!(!serialized.contains("sk-test-secret"), "{serialized}");

        let codex = context(&directory).codex_config;
        fs::create_dir_all(codex.parent().unwrap()).unwrap();
        fs::write(&codex, "[mcp_servers.ghostlight]\ntoken = sk-test-secret\n").unwrap();

        let summary = inspected_summary_of(&directory, "codex");

        assert_eq!(summary.state, HarnessState::NeedsAttention);
        assert_eq!(
            summary.detail,
            "This configuration could not be parsed at line 2; it was left untouched."
        );
        assert!(summary.evidence.is_none(), "{:?}", summary.evidence);
        let serialized = serde_json::to_string(&summary).unwrap();
        assert!(!serialized.contains("sk-test-secret"), "{serialized}");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_structural_fault_names_no_line() {
        let directory = temporary("malformed-structure");
        let path = claude_desktop_config(&directory);
        edge(&directory);
        fs::write(
            &path, "[]
",
        )
        .unwrap();

        let summary = inspected_summary(&directory);

        assert_eq!(
            summary.detail,
            "This configuration could not be parsed; it was left untouched."
        );
        assert!(summary.evidence.is_none());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_command_line_command_discloses_only_its_first_word() {
        let line =
            ForeignCommand::from_entry("C:/tools/other-server.exe --token sk/test-secret", None)
                .expect("a program is named");
        assert_eq!(line.program, "other-server.exe");
        assert_eq!(line.arguments, 2);

        let npx = ForeignCommand::from_entry("npx -y @scope/server-x --api-key abc", None)
            .expect("a program is named");
        assert_eq!(npx.program, "npx");
        assert_eq!(npx.arguments, 4);

        let both = ForeignCommand::from_entry("npx -y @scope/server-x", Some(&["--port", "9"]))
            .expect("a program is named");
        assert_eq!(both.program, "npx");
        assert_eq!(both.arguments, 4);

        let directory = temporary("foreign-spaced-file");
        let spaced = directory.join("tools with spaces").join("other-server");
        fs::create_dir_all(spaced.parent().unwrap()).unwrap();
        fs::write(&spaced, "").unwrap();
        let whole = ForeignCommand::from_entry(&spaced.to_string_lossy(), Some(&["serve"]))
            .expect("a program is named");
        assert_eq!(whole.program, "other-server");
        assert_eq!(whole.arguments, 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn unblocked_targets_and_commandless_foreign_entries_stay_bounded() {
        let directory = temporary("unblocked-evidence");
        let path = claude_desktop_config(&directory);
        let expected_edge = edge(&directory);
        let installed = serde_json::json!({
            "mcpServers": {
                "ghostlight": {
                    "command": expected_edge,
                    "args": ["mcp"],
                    "env": {},
                }
            }
        });
        fs::write(&path, installed.to_string()).unwrap();
        let summary = inspected_summary(&directory);
        assert_eq!(summary.state, HarnessState::Installed);
        assert!(summary.evidence.is_none(), "{:?}", summary.evidence);

        fs::write(&path, r#"{"mcpServers":{"ghostlight":{"env":{}}}}"#).unwrap();
        let summary = inspected_summary(&directory);
        assert_eq!(summary.state, HarnessState::NeedsAttention);
        let evidence = summary.evidence.clone().expect("commandless foreign entry");
        assert!(evidence.contains("without a usable command"), "{evidence}");

        // With the configuration root gone, nothing about the target remains blocked or
        // disclosed.
        fs::remove_dir_all(directory.join("config")).unwrap();
        let summary = inspected_summary(&directory);
        assert_eq!(summary.state, HarnessState::NotDetected);
        assert!(summary.evidence.is_none());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn every_blocked_dialect_names_the_found_program() {
        let directory = temporary("dialect-evidence");
        let expected_edge = edge(&directory);

        let toml = directory.join("codex.toml");
        std::fs::write(
            &toml,
            "[mcp_servers.ghostlight]\ncommand = \"alien-tool\"\nargs = [\"serve\"]\n",
        )
        .unwrap();
        let toml_source = fs::read_to_string(&toml).unwrap();
        assert!(matches!(
            inspect_toml(&toml_source, &expected_edge, false),
            Ok(RegistrationState::Foreign(Some(ForeignCommand { program, arguments: 1 })))
                if program == "alien-tool"
        ));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn an_owned_entry_naming_the_retired_connector_is_retired_not_installed() {
        let directory = temporary("retired");
        let current = edge(&directory);
        let old = directory
            .join("old")
            .join(executable_name(RETIRED_MCP_CONNECTOR_NAME, cfg!(windows)));
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
            RegistrationState::Retired
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn the_edge_with_other_arguments_or_another_path_is_updatable() {
        let directory = temporary("updatable");
        let current = edge(&directory);
        let elsewhere = directory
            .join("elsewhere")
            .join(executable_name(EDGE_COMMAND_NAME, cfg!(windows)));
        for (command, args) in [
            (current.clone(), serde_json::json!([])),
            (current.clone(), serde_json::json!(["serve"])),
            (elsewhere, serde_json::json!(["mcp"])),
        ] {
            let source = serde_json::json!({
                "mcpServers": {"ghostlight": {"command": command, "args": args, "env": {}}}
            })
            .to_string();
            assert_eq!(
                inspect_json(&source, JsonDialect::McpServers, &current, cfg!(windows)).unwrap(),
                RegistrationState::Updatable
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn every_dialect_starts_the_edge_with_the_mcp_argument() {
        let directory = temporary("arguments");
        let current = edge(&directory);
        for dialect in [
            JsonDialect::McpServers,
            JsonDialect::Copilot,
            JsonDialect::Servers,
            JsonDialect::ContextServers,
            JsonDialect::Crush,
            JsonDialect::OpenCodeV1,
            JsonDialect::OpenCodeV2,
            JsonDialect::ZCode,
        ] {
            let entry = expected_json_entry(&current, dialect);
            assert_eq!(
                json_entry_command(&entry, dialect),
                Some(current.to_string_lossy().as_ref()),
                "{dialect:?}"
            );
            assert_eq!(
                json_entry_args(&entry, dialect).as_deref(),
                Some(&["mcp"][..]),
                "{dialect:?}"
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn the_copied_command_is_the_edge_and_its_argument() {
        let directory = temporary("copy-command");
        let registry = HarnessRegistry::with_context(context(&directory));
        let path = edge(&directory);
        let id = registry.summaries()[0].id.clone();
        assert_eq!(
            registry.copy_text(&id, HarnessCopyKind::Command).unwrap(),
            format!("{} mcp", path.display())
        );
        assert_eq!(
            mcp_command_line(Path::new("/with space/ghostlight")),
            "\"/with space/ghostlight\" mcp"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_hand_written_entry_naming_the_retired_connector_is_named_in_the_summary() {
        let directory = temporary("hand-written");
        let mut context = context(&directory);
        let codex = context.codex_config.clone();
        fs::create_dir_all(codex.parent().unwrap()).unwrap();
        fs::write(
            &codex,
            "[mcp_servers.ghostlight]\ncommand = \"ghostlight-mcp-connector\"\nargs = []\n",
        )
        .unwrap();
        context.path_entries = Vec::new();
        let registry = HarnessRegistry::with_context(context);
        let summary = registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "codex")
            .unwrap();
        assert_eq!(summary.state, HarnessState::Updatable);
        assert!(summary.detail.contains("ghostlight-mcp-connector"));
        assert!(summary.detail.contains("ghostlight mcp"));
        registry.apply("codex", HarnessAction::Install).unwrap();
        let rewritten = fs::read_to_string(&codex).unwrap();
        assert!(!rewritten.contains("ghostlight-mcp-connector"));
        assert!(rewritten.contains("args = [\"mcp\"]"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn codex_toml_edit_preserves_unrelated_comments() {
        let directory = temporary("toml");
        let path = directory.join("config.toml");
        let edge = edge(&directory);
        fs::write(&path, "# keep me\nmodel = \"gpt-test\"\n").unwrap();
        assert!(edit_toml(&path, &edge, true).unwrap());
        assert!(!edit_toml(&path, &edge, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        assert!(installed.contains("# keep me"));
        assert_eq!(
            inspect_toml(&installed, &edge, cfg!(windows)).unwrap(),
            RegistrationState::Current
        );
        assert!(edit_toml(&path, &edge, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("# keep me"));
        assert!(!removed.contains("mcp_servers"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn codex_owned_registration_gains_desktop_forwarding_without_losing_custom_fields() {
        let directory = temporary("codex-desktop");
        let path = directory.join("config.toml");
        let edge = edge(&directory);
        let source = format!(
            "# keep me\n[mcp_servers.ghostlight]\ncommand = {:?}\nargs = []\nstartup_timeout_sec = 45\nenv_vars = [\"CUSTOM_SETTING\", {{ name = \"DISPLAY\", source = \"local\" }}, {{ name = \"WAYLAND_DISPLAY\" }}, {{ name = \"CUSTOM_REMOTE\", source = \"remote\" }}]\n[mcp_servers.ghostlight.env]\nXAUTHORITY = \"/explicit/test-authority\"\n[mcp_servers.other]\ncommand = \"other-server\"\n",
            edge.to_string_lossy()
        );
        fs::write(&path, &source).unwrap();
        assert_eq!(
            inspect_toml(&source, &edge, false).unwrap(),
            RegistrationState::Updatable
        );
        assert!(edit_toml(&path, &edge, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        let document = installed.parse::<toml_edit::DocumentMut>().unwrap();
        let entry = &document["mcp_servers"]["ghostlight"];
        assert_eq!(entry["startup_timeout_sec"].as_integer(), Some(45));
        assert_eq!(
            entry["env"]["XAUTHORITY"].as_str(),
            Some("/explicit/test-authority")
        );
        assert_eq!(entry["env_vars"].as_array().unwrap().len(), 7);
        let original = source.parse::<toml_edit::DocumentMut>().unwrap();
        for (before, after) in original["mcp_servers"]["ghostlight"]["env_vars"]
            .as_array()
            .unwrap()
            .iter()
            .zip(entry["env_vars"].as_array().unwrap().iter())
        {
            assert_eq!(before.to_string(), after.to_string());
        }
        assert_eq!(
            document["mcp_servers"]["other"]["command"].as_str(),
            Some("other-server")
        );
        assert!(installed.contains("# keep me"));
        assert!(super::missing_codex_desktop_environment(entry, false)
            .unwrap()
            .is_empty());
        assert!(!edit_toml(&path, &edge, true).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), installed);
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn codex_malformed_environment_is_preserved_and_windows_needs_no_desktop_forwarding() {
        let directory = temporary("codex-malformed-env");
        let path = directory.join("config.toml");
        let edge = edge(&directory);
        let source = format!(
            "[mcp_servers.ghostlight]\ncommand = {:?}\nargs = []\nenv_vars = \"not-an-array\"\n",
            edge.to_string_lossy()
        );
        fs::write(&path, &source).unwrap();
        assert!(inspect_toml(&source, &edge, false).is_err());
        assert!(edit_toml(&path, &edge, true).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        assert_eq!(
            inspect_toml(&source, &edge, true).unwrap(),
            RegistrationState::Current
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn codex_malformed_environment_members_are_preserved_before_migration() {
        let directory = temporary("codex-malformed-env-members");
        let path = directory.join("config.toml");
        let edge = edge(&directory);
        for members in [
            "123",
            "true",
            "[]",
            "{}",
            "{ name = 123 }",
            "{ source = \"local\" }",
            "{ name = \"DISPLAY\", source = 123 }",
            "{ name = \"DISPLAY\", source = \"bogus\" }",
            "{ name = \"DISPLAY\", extra = true }",
            "\"DISPLAY\", \"WAYLAND_DISPLAY\", \"XDG_RUNTIME_DIR\", \"DBUS_SESSION_BUS_ADDRESS\", \"XDG_CURRENT_DESKTOP\", \"XAUTHORITY\", 123",
        ] {
            for command in [&edge, &directory.join("old/ghostlight-mcp-connector")] {
                for source in [
                    format!(
                        "# preserve exactly\n[mcp_servers.ghostlight]\ncommand = {:?}\nargs = []\nenv_vars = [{members}]\n",
                        command.to_string_lossy()
                    ),
                    format!(
                        "# preserve exactly\n[mcp_servers]\nghostlight = {{ command = {:?}, args = [], env_vars = [{members}] }}\n",
                        command.to_string_lossy()
                    ),
                ] {
                    fs::write(&path, &source).unwrap();
                    assert!(
                        inspect_toml(&source, &edge, false).is_err(),
                        "inspection accepted {source}"
                    );
                    assert!(edit_toml(&path, &edge, true).is_err());
                    assert_eq!(fs::read_to_string(&path).unwrap(), source);
                }
            }
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn codex_inline_owned_registration_keeps_custom_fields() {
        let directory = temporary("codex-inline-env");
        let path = directory.join("config.toml");
        let edge = edge(&directory);
        fs::write(&path, format!(
            "[mcp_servers]\nghostlight = {{ command = {:?}, args = [], startup_timeout_sec = 45 }}\n",
            edge.to_string_lossy()
        )).unwrap();
        assert!(edit_toml(&path, &edge, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        let document = installed.parse::<toml_edit::DocumentMut>().unwrap();
        assert_eq!(
            document["mcp_servers"]["ghostlight"]["startup_timeout_sec"].as_integer(),
            Some(45)
        );
        assert_eq!(
            inspect_toml(&installed, &edge, false).unwrap(),
            RegistrationState::Current
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn codex_manual_fragment_matches_automatic_desktop_environment() {
        let directory = temporary("codex-manual-env");
        let context = context(&directory);
        let definition = definitions(&context)
            .into_iter()
            .find(|row| row.id == "codex")
            .unwrap();
        let manual = super::manual_setup(&context, &definition).unwrap();
        assert!(edit_toml(&definition.path, &context.edge, true).unwrap());
        assert_eq!(manual, fs::read_to_string(&definition.path).unwrap());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn opencode_v1_uses_the_local_command_array_contract() {
        let directory = temporary("opencode-v1");
        let path = directory.join("opencode.json");
        let edge = edge(&directory);
        fs::write(&path, r#"{"theme":"ghost","mcp":{}}"#).unwrap();

        assert!(edit_json(&path, &edge, JsonDialect::OpenCodeV1, true).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["theme"], "ghost");
        assert_eq!(value["mcp"]["ghostlight"]["type"], "local");
        assert_eq!(
            value["mcp"]["ghostlight"]["command"][0].as_str(),
            Some(edge.to_string_lossy().as_ref())
        );
        assert_eq!(value["mcp"]["ghostlight"]["enabled"], true);
        assert_eq!(
            inspect_json(
                &fs::read_to_string(&path).unwrap(),
                JsonDialect::OpenCodeV1,
                &edge,
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
        let edge = edge(&directory);
        fs::write(
            &path,
            r#"{"mcp":{"servers":{"sibling":{"type":"remote"}}}}"#,
        )
        .unwrap();

        assert!(edit_json(&path, &edge, JsonDialect::OpenCodeV2, true).unwrap());
        assert!(!edit_json(&path, &edge, JsonDialect::OpenCodeV2, true).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["mcp"]["servers"]["sibling"]["type"], "remote");
        assert_eq!(value["mcp"]["servers"]["ghostlight"]["type"], "local");
        assert!(value["mcp"]["servers"]["ghostlight"]
            .get("enabled")
            .is_none());
        assert!(edit_json(&path, &edge, JsonDialect::OpenCodeV2, false).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(value["mcp"]["servers"].get("ghostlight").is_none());
        assert_eq!(value["mcp"]["servers"]["sibling"]["type"], "remote");

        fs::remove_dir_all(directory).unwrap();
    }

    /// A context run from inside the container of the app `package`, whose `AppData` is the
    /// directory `config`, with a Claude Desktop configuration present.
    fn inside_container(directory: &Path, package: &str) -> (HarnessContext, PathBuf) {
        let mut context = context(directory);
        context.private_view = Some(PrivateView {
            container: AppContainer {
                app: package.into(),
            },
            roots: vec![directory.join("config")],
        });
        let desktop = context.roaming.join("Claude/claude_desktop_config.json");
        fs::create_dir_all(desktop.parent().unwrap()).unwrap();
        fs::write(&desktop, r#"{"mcpServers":{}}"#).unwrap();
        (context, desktop)
    }

    #[test]
    fn inside_another_apps_container_an_appdata_client_is_private_and_a_profile_client_is_not() {
        let directory = temporary("private-view");
        let (context, desktop) = inside_container(&directory, "OpenAI.Codex");
        let before = fs::read(&desktop).unwrap();
        let registry = HarnessRegistry::with_context(context);

        let summaries = registry.summaries();
        let claude = summaries
            .iter()
            .find(|summary| summary.id == "claude-desktop")
            .unwrap();
        assert!(claude.private_view);
        let codex = summaries
            .iter()
            .find(|summary| summary.id == "codex")
            .unwrap();
        assert!(!codex.private_view);

        let refused = registry
            .apply("claude-desktop", HarnessAction::Install)
            .unwrap_err();
        assert!(matches!(refused, HarnessError::PrivateView(ref app) if app == "OpenAI.Codex"));
        assert_eq!(fs::read(&desktop).unwrap(), before);
        assert!(
            registry
                .apply("codex", HarnessAction::Install)
                .unwrap()
                .changed
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn inside_its_own_container_a_packaged_client_is_written_like_any_other() {
        let directory = temporary("own-container");
        let (context, desktop) = inside_container(&directory, "Claude");
        let registry = HarnessRegistry::with_context(context);

        let claude = registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "claude-desktop")
            .unwrap();
        assert!(!claude.private_view);
        assert!(
            registry
                .apply("claude-desktop", HarnessAction::Install)
                .unwrap()
                .changed
        );
        let value: Value = serde_json::from_str(&fs::read_to_string(&desktop).unwrap()).unwrap();
        assert!(value["mcpServers"].get("ghostlight").is_some());

        fs::remove_dir_all(directory).unwrap();
    }
}
