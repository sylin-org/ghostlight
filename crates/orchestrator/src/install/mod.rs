//! Explicit, ownership-checked development-harness registration owned by the orchestrator.

#[cfg(test)]
mod agent_list;
pub mod apps;
pub mod browser_package;
pub mod browser_profiles;
pub mod command_path;
pub mod container;
pub mod deployment;
pub mod desktop_entry;
pub mod entry;
pub mod icons;
pub mod last_browser;
pub mod layout;
pub mod migration;
pub mod native_host;
mod packaged;
pub mod reconcile;
pub mod record;
pub mod setup_state;
pub mod swap;
pub mod user_assets;
pub mod user_path;
pub mod version_directories;
#[cfg(windows)]
pub mod windows_entry;
#[cfg(test)]
mod writer_golden;

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use ghostlight_relay::route::MCP_ARGUMENT;
use jsonc_parser::cst::{CstInputValue, CstObject, CstObjectProp, CstRootNode};
use jsonc_parser::{parse_to_serde_value, ParseOptions};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use toml_edit::{value, Array, DocumentMut, Item, Table};

use crate::agent::image::{monogram, Monogram};
use crate::agent::Agent;
use crate::language::deployment as words;

use container::{AppContainer, Residence};
use icons::{AgentIconSources, IconSource, KnownFolder, ProgramPlace};
use layout::{EDGE_COMMAND_NAME, RETIRED_MCP_CONNECTOR_NAME};
use packaged::{CopyPlace, DataRoots, Packages, SettingsCopy};
use record::{Access, Link, Replace};

const SERVER_NAME: &str = "ghostlight";
/// The key of the program an entry starts, in every dialect.
const COMMAND_KEY: &str = "command";
/// The key of the program's arguments, in the dialects that keep them apart from the command.
const ARGS_KEY: &str = "args";
/// The key of the kind of server an entry describes, in the dialects that need one.
const TYPE_KEY: &str = "type";
/// The arguments that make the edge the MCP relay, in every client's entry.
const MCP_ARGUMENTS: [&str; 1] = [MCP_ARGUMENT];
/// How a client's detail names an owned entry that starts the retired MCP connector.
const RETIRED_ENTRY: words::RetiredEntry<'static> = words::RetiredEntry {
    connector: RETIRED_MCP_CONNECTOR_NAME,
    edge: EDGE_COMMAND_NAME,
    argument: MCP_ARGUMENT,
};
/// The package name of the Claude desktop app, which reads the Claude Desktop configuration.
const CLAUDE_DESKTOP_PACKAGE: &str = "Claude";
/// The package name of the Codex desktop app.
const CODEX_PACKAGE: &str = "OpenAI.Codex";
/// File in the state directory recording where each MCP client's configuration was found.
const HARNESS_LOCATIONS_FILE: &str = "harness-locations.json";
/// The age past which a replace's temporary file beside a configuration is no write in flight
/// but one an interrupted write left behind. A write takes milliseconds.
const STRAY_TEMPORARY_AGE: Duration = Duration::from_secs(60);
/// The Windows variable that names the person's home, read before `HOME`.
const USER_PROFILE_VARIABLE: &str = "USERPROFILE";
/// The variable that names the person's home where `USERPROFILE` is not set, and on Linux.
const HOME_VARIABLE: &str = "HOME";
/// The Windows variable that names the roaming application data directory.
const APP_DATA_VARIABLE: &str = "APPDATA";
/// The Windows variable that names the local application data directory.
const LOCAL_APP_DATA_VARIABLE: &str = "LOCALAPPDATA";
/// The Linux variable that names the configuration directory (XDG base directories).
const XDG_CONFIG_HOME_VARIABLE: &str = "XDG_CONFIG_HOME";
/// The variable that names Codex's own directory, which holds its `config.toml`.
const CODEX_HOME_VARIABLE: &str = "CODEX_HOME";
/// The variable that names Crush's whole configuration file, in place of its default.
const CRUSH_CONFIG_VARIABLE: &str = "CRUSH_GLOBAL_CONFIG";
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
///
/// The cached roster is written only under the action lock: every act scans under the lock it
/// holds, and [`Self::refresh`] waits for it, so a scan that began before an act's write can never
/// store its older roster over the act's result.
#[derive(Clone)]
pub struct HarnessRegistry {
    inner: Arc<RegistryInner>,
}

struct RegistryInner {
    context: HarnessContext,
    summaries: Mutex<Vec<HarnessSummary>>,
    /// Held by every act and every scan, and always taken before `summaries`.
    action: Mutex<()>,
    /// How many scans this registry has run, for the tests that count them.
    #[cfg(test)]
    scans: std::sync::atomic::AtomicUsize,
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
                #[cfg(test)]
                scans: std::sync::atomic::AtomicUsize::new(0),
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
    ///
    /// It waits for an act in progress, so the roster it stores is never older than an act's.
    pub fn refresh(&self) -> Result<Vec<HarnessSummary>, HarnessError> {
        let _action = lock(&self.inner.action);
        Ok(self.scan())
    }

    /// Give every cached row the symbol its agent wears now, after the agents' icons changed,
    /// without reading any client configuration again. Returns the roster as it now stands.
    ///
    /// It waits for an act or scan in progress, so a scan that read the older icons never stores
    /// them over this.
    pub fn resymbol(&self) -> Vec<HarnessSummary> {
        let _action = lock(&self.inner.action);
        let mut summaries = lock(&self.inner.summaries);
        for summary in summaries.iter_mut() {
            if let Some(agent) = catalog_agent(&summary.agent.id) {
                summary.agent = agent;
            }
        }
        summaries.clone()
    }

    /// Inspect every supported harness and store the roster. The caller holds the action lock.
    fn scan(&self) -> Vec<HarnessSummary> {
        #[cfg(test)]
        self.inner
            .scans
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let summaries: Vec<HarnessSummary> = definitions(&self.inner.context)
            .into_iter()
            .map(|definition| inspect(&self.inner.context, &definition))
            .collect();
        *lock(&self.inner.summaries) = summaries.clone();
        summaries
    }

    /// How many scans this registry has run.
    #[cfg(test)]
    pub(crate) fn scans(&self) -> usize {
        self.inner.scans.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Apply one explicit check, install, fix, or uninstall action.
    pub fn apply(
        &self,
        id: &str,
        action: HarnessAction,
    ) -> Result<HarnessActionResult, HarnessError> {
        let _action = lock(&self.inner.action);
        let definition = definition(&self.inner.context, id)?;
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
        self.scan();
        Ok(result)
    }

    /// Set up or update every detected target Ghostlight can change safely.
    ///
    /// Current registrations, targets that are not detected and targets whose configuration the
    /// app container this process runs in keeps private are left alone. Malformed and foreign
    /// registrations remain blocked by the same per-target ownership check used by
    /// [`Self::apply`]. One target failing does not prevent an independent target from becoming
    /// ready; the result names the failures and the refreshed roster carries their evidence.
    pub fn setup_detected(&self) -> Result<HarnessSetupResult, HarnessError> {
        let _action = lock(&self.inner.action);
        let initial = self.scan();
        let states: BTreeMap<&str, HarnessState> = initial
            .iter()
            .map(|summary| (summary.id.as_str(), summary.state))
            .collect();
        let eligible: Vec<HarnessDefinition> = definitions(&self.inner.context)
            .into_iter()
            .filter(|definition| {
                !matches!(definition.dialect, ConfigDialect::Manual(_))
                    && self.inner.context.private_to(definition).is_none()
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
                        .unwrap_or_else(|| words::CLIENT_CHANGE_FAILED.into()),
                }),
            }
        }

        let summaries = self.scan();
        let needs_attention = summaries
            .iter()
            .filter(|summary| summary.state == HarnessState::NeedsAttention)
            .count();
        let message = words::setup_message(set_up, updated, needs_attention, failures.len());
        Ok(HarnessSetupResult {
            set_up,
            updated,
            needs_attention,
            failures,
            message,
        })
    }

    /// Remove the temporary files interrupted configuration writes left beside the
    /// configurations Ghostlight writes: the file of every agent it can change from here and its
    /// own location state. A temporary younger than [`STRAY_TEMPORARY_AGE`] may be a write in
    /// flight and stays. Returns how many were removed; one that cannot be removed is left.
    pub fn remove_stray_temporaries(&self) -> usize {
        self.remove_stray_temporaries_at(SystemTime::now())
    }

    /// [`Self::remove_stray_temporaries`], judging each temporary's age at `now`.
    fn remove_stray_temporaries_at(&self, now: SystemTime) -> usize {
        let _action = lock(&self.inner.action);
        let context = &self.inner.context;
        let mut directories: Vec<PathBuf> = definitions(context)
            .into_iter()
            .filter(|definition| {
                !matches!(definition.dialect, ConfigDialect::Manual(_))
                    && context.private_to(definition).is_none()
            })
            .map(|definition| {
                CONFIGURATION_WRITE
                    .target(&definition.path)
                    .unwrap_or(definition.path)
            })
            .chain(std::iter::once(context.locations.clone()))
            .filter_map(|path| path.parent().map(Path::to_path_buf))
            .collect();
        directories.sort();
        directories.dedup();
        directories
            .iter()
            .map(|directory| remove_stray_temporaries_in(directory, now))
            .sum()
    }

    /// Return the closed official download destination of one host, by its harness id.
    ///
    /// Fails with `UnknownHarness` for an id outside the roster and with `NoDownload` for a host
    /// that has no official download page.
    pub fn download_url(&self, id: &str) -> Result<&'static str, HarnessError> {
        definition(&self.inner.context, id)?
            .download_url
            .ok_or_else(|| HarnessError::NoDownload(id.into()))
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
        let summaries = self.scan();
        let summary = summaries
            .into_iter()
            .find(|summary| summary.id == id)
            .ok_or_else(|| HarnessError::UnknownHarness(id.into()))?;
        let message = if matches!(definition.dialect, ConfigDialect::Manual(_)) {
            manual_setup_sentence(&definition)
        } else {
            words::client_location_kept(&summary.label)
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
        crush_config: None,
        edge: edge.to_path_buf(),
        path_entries: Vec::new(),
        locations: state.join(HARNESS_LOCATIONS_FILE),
        windows: false,
        linux: cfg!(target_os = "linux"),
        local_app_data: None,
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
    /// Whether an install writes an entry for a target in this state: one that has none yet,
    /// detected or not, or one an older Ghostlight wrote.
    #[must_use]
    pub const fn takes_install(self) -> bool {
        matches!(self, Self::NotDetected | Self::Available | Self::Updatable)
    }

    /// Whether the configuration itself holds an entry Ghostlight owns, current or not, which an
    /// uninstall removes. Inspection shows every owned registration (current, updatable or
    /// retired) as one of these states and nothing else as them.
    #[must_use]
    pub const fn holds_owned_entry(self) -> bool {
        matches!(self, Self::Installed | Self::Updatable)
    }
}

/// What inspection saw that put a target in [`HarnessState::NeedsAttention`]. Each cause is
/// said as itself, never as another (ADR-0135).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Attention {
    /// Ghostlight's own npm launcher sits under Ghostlight's name, asking npm for the version
    /// its pin names.
    Launcher(LauncherPin),
    /// An entry Ghostlight does not own sits under Ghostlight's name.
    ForeignEntry,
    /// The configuration file could not be parsed.
    Malformed,
    /// The configuration file could not be read.
    Unreadable,
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
    /// The agent this target sets up, as the window names and shows it.
    pub agent: Agent,
    /// Current registration state.
    pub state: HarnessState,
    /// Fixed outcome-oriented detail.
    pub detail: String,
    /// Optional precomposed proof behind a blocked state, authored by the orchestrator
    /// (ADR-0135).
    ///
    /// Present only for a foreign entry or Ghostlight's npm launcher. For a foreign entry it names
    /// the program found where Ghostlight's entry belongs and its argument count, never its
    /// arguments (ADR-0135 as narrowed by the ADR-0199 amendment of 2026-10-15). For the
    /// launcher it names the package and the version the entry asks npm for. The surface renders
    /// it verbatim and authors no words of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    /// Why the target needs attention, present exactly when `state` is
    /// [`HarnessState::NeedsAttention`]. `detail` says it to the window; a dry run words its line
    /// from it.
    #[serde(skip)]
    pub attention: Option<Attention>,
    /// Whether an explicit install can be attempted.
    pub can_install: bool,
    /// Whether an explicit fix can replace a foreign entry under Ghostlight's key.
    pub can_fix: bool,
    /// Whether an owned registration, current or not, can be removed: Ghostlight's entry in
    /// `config_path`, or one an earlier install left in another copy of the agent's settings.
    pub can_uninstall: bool,
    /// Whether this host has a closed official download destination on this platform.
    pub can_download: bool,
    /// This host's official download page, present exactly when `can_download` is. It is public,
    /// so every audience is given it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// Whether the native Locate action is available.
    pub can_locate: bool,
    /// Whether Ghostlight never edits this target's file, so copying its setup and pasting it in
    /// is the only way to connect it.
    pub copy_only: bool,
    /// Whether this process runs inside an app container that keeps this target's configuration
    /// private: what it reads here is that app's view, and a change would reach only that app.
    /// Install, Fix and Uninstall are then never offered, and `detail` says why.
    pub private_view: bool,
    /// Exact configuration destination currently resolved for this target.
    pub config_path: String,
    /// Whether `config_path` is the private copy of the agent's settings kept by the agent's own
    /// Microsoft Store app below `%LOCALAPPDATA%\Packages`, which is the copy that app reads.
    pub packaged: bool,
    /// Another copy of the agent's settings file that exists besides `config_path`, such as a
    /// plain `%APPDATA%` file an earlier install left, or another package's copy. Ghostlight's
    /// entry goes in `config_path`; Uninstall also removes Ghostlight's own entry from this copy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_copy: Option<String>,
    /// Every copy of the agent's settings besides `config_path` that holds an entry Ghostlight
    /// owns: the copies Uninstall removes Ghostlight's entry from, besides `config_path` when that
    /// holds one too. A dry run names them, so it says what Uninstall changes.
    #[serde(skip)]
    pub owned_copies: Vec<String>,
    /// Exact installed command line available for manual setup.
    pub connector_command: String,
    /// Smallest complete manual configuration document for this target.
    pub manual_setup: String,
}

/// The install and uninstall option that includes clients not currently detected, and widens an
/// agent id given to `--client` to every host of the agent.
pub const ALL_CLIENTS_OPTION: &str = "--all-clients";

/// How a `--client` value names a host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientName {
    /// The value is the host's own harness id. It selects that host in any state.
    Host,
    /// The value is the catalog id of the host's agent and not the host's own id. It selects the
    /// host when the host is detected, or in any state with [`ALL_CLIENTS_OPTION`], so naming an
    /// agent never writes into a host the person does not have.
    Agent,
}

impl HarnessSummary {
    /// Whether an uninstall finds an entry Ghostlight owns here, current or not: in the
    /// configuration itself or in another copy of the agent's settings. `can_uninstall` is this
    /// where the configuration may be changed from this process; a run that may not change it
    /// reads this to say what it leaves.
    #[must_use]
    pub fn has_entry_to_remove(&self) -> bool {
        self.state.holds_owned_entry() || !self.owned_copies.is_empty()
    }

    /// How the `--client` value `id` names this host, ignoring case: by its harness id, by its
    /// agent's catalog id, or not at all. A harness id that is also its agent's id names the host.
    #[must_use]
    pub fn named_by(&self, id: &str) -> Option<ClientName> {
        if self.id.eq_ignore_ascii_case(id) {
            Some(ClientName::Host)
        } else if self.agent.id.eq_ignore_ascii_case(id) {
            Some(ClientName::Agent)
        } else {
            None
        }
    }

    /// Whether the `--client` value `id` selects this host. Its harness id always does. Its
    /// agent's id does when the host is detected, or whatever its state when `all_clients` is set.
    #[must_use]
    pub fn is_selected_by(&self, id: &str, all_clients: bool) -> bool {
        match self.named_by(id) {
            Some(ClientName::Host) => true,
            Some(ClientName::Agent) => all_clients || self.state != HarnessState::NotDetected,
            None => false,
        }
    }
}

/// Every value `ghostlight install --client` and `ghostlight uninstall --client` accept, in roster
/// order, for the help text, the refusal of an unknown value and the shell completions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientIds {
    /// Every agent's catalog id, once. Each selects the agent's detected hosts, or every host of
    /// it with [`ALL_CLIENTS_OPTION`].
    pub agents: Vec<&'static str>,
    /// The harness id of every host that no agent id names: the hosts of an agent that runs on
    /// more than one. Each selects that one host.
    pub hosts: Vec<&'static str>,
}

impl ClientIds {
    /// The agent ids, then the host ids.
    pub fn all(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.agents.iter().chain(&self.hosts).copied()
    }
}

/// The `--client` values the roster offers. A harness id is a persisted key (the person's located
/// paths are stored under it), so every one of them stays a value even when its agent's id is
/// also listed.
#[must_use]
pub fn client_ids() -> ClientIds {
    let mut agents: Vec<&'static str> = Vec::new();
    for row in ROSTER {
        if !agents.contains(&row.agent_id) {
            agents.push(row.agent_id);
        }
    }
    let hosts = ROSTER
        .iter()
        .map(|row| row.id)
        .filter(|id| !agents.contains(id))
        .collect();
    ClientIds { agents, hosts }
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

#[derive(Clone)]
struct HarnessContext {
    home: PathBuf,
    config: PathBuf,
    roaming: PathBuf,
    codex_config: PathBuf,
    /// The file [`CRUSH_CONFIG_VARIABLE`] names, when it is set: Crush reads it in place of its
    /// default file.
    crush_config: Option<PathBuf>,
    edge: PathBuf,
    path_entries: Vec<PathBuf>,
    locations: PathBuf,
    /// Whether the computer runs Windows: paths compare ignoring case and programs end in `.exe`.
    windows: bool,
    /// Whether the computer runs Linux, where Codex's entry forwards the desktop session
    /// ([`CODEX_DESKTOP_ENVIRONMENT`]).
    linux: bool,
    /// The person's `%LOCALAPPDATA%` on Windows, below which each Store app keeps its private
    /// copy of the data roots ([`packaged`]); none elsewhere, where no copy is looked for.
    local_app_data: Option<PathBuf>,
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
        if self.inside_own_app(definition.own_package) {
            return None;
        }
        view.roots
            .iter()
            .any(|root| entry::lies_within(&definition.path, root, self.windows))
            .then_some(&view.container)
    }

    /// Whether this process runs inside the container of `own_package`, the packaged app of a
    /// client, whose private view is the one that client reads.
    fn inside_own_app(&self, own_package: Option<&str>) -> bool {
        self.private_view
            .as_ref()
            .is_some_and(|view| own_package.is_some_and(|package| view.container.is_app(package)))
    }

    fn system() -> Self {
        let windows = cfg!(target_os = "windows");
        let home = if windows {
            env::var_os(USER_PROFILE_VARIABLE).or_else(|| env::var_os(HOME_VARIABLE))
        } else {
            env::var_os(HOME_VARIABLE)
        }
        .map(PathBuf::from)
        .unwrap_or_default();
        let roots = harness_roots(
            &home,
            env::var_os(XDG_CONFIG_HOME_VARIABLE).map(PathBuf::from),
            env::var_os(APP_DATA_VARIABLE).map(PathBuf::from),
            env::var_os(CODEX_HOME_VARIABLE).map(PathBuf::from),
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
        let local_app_data = windows.then(|| {
            env::var_os(LOCAL_APP_DATA_VARIABLE)
                .map_or_else(|| home.join("AppData/Local"), PathBuf::from)
        });
        let private_view = local_app_data.as_ref().and_then(|local| {
            Residence::detect()
                .container()
                .map(|container| PrivateView {
                    container: container.clone(),
                    roots: vec![roots.roaming.clone(), local.clone()],
                })
        });
        Self {
            home,
            config: roots.config,
            roaming: roots.roaming,
            codex_config: roots.codex_config,
            crush_config: env::var_os(CRUSH_CONFIG_VARIABLE).map(PathBuf::from),
            edge,
            path_entries,
            locations,
            windows,
            linux: cfg!(target_os = "linux"),
            local_app_data,
            private_view,
        }
    }

    /// The Store packages of this look that keep a private copy of a data root: one listing on
    /// Windows, none elsewhere.
    fn packages(&self) -> Packages {
        match &self.local_app_data {
            Some(local) if self.windows => Packages::listed(DataRoots {
                roaming: self.roaming.clone(),
                local: local.clone(),
            }),
            _ => Packages::none(),
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
    /// The agent's catalog id, shared by every host of one agent.
    agent_id: &'static str,
    product_id: &'static str,
    name: &'static str,
    /// The name a person reads: `name`, with the host added when several targets share the name.
    label: String,
    target: &'static str,
    /// The agent's packaged monogram tile.
    monogram: Monogram,
    download_url: Option<&'static str>,
    path: PathBuf,
    default_path: PathBuf,
    /// The file names the agent reads beside its configuration file ([`AgentRow::candidates`]).
    candidates: &'static [&'static str],
    located_executable: Option<PathBuf>,
    located_stale: bool,
    executables: &'static [&'static str],
    dialect: ConfigDialect,
    /// The package name of the packaged app that reads this configuration, when the client is
    /// one. Inside that app's own container the private view is the one the client reads.
    own_package: Option<&'static str>,
    /// Every place this look found the agent's settings may be, the one it reads first leading
    /// ([`Packages::copies`]): the Store packages' private copies, then the plain path.
    copies: Vec<SettingsCopy>,
}

impl HarnessDefinition {
    /// Whether `path` is the private copy of the agent's settings kept by its own Store app, on
    /// Windows (`windows`) or another system.
    fn packaged(&self, windows: bool) -> bool {
        self.copies.iter().any(|copy| {
            copy.place == CopyPlace::OwnPackage
                && entry::spelled_alike(&copy.file, &self.path, windows)
        })
    }

    /// The configuration files of the agent's other copies of its settings that exist: what
    /// `path` is not, but an earlier install or another package may still hold, on Windows
    /// (`windows`) or another system.
    fn other_copies(&self, windows: bool) -> impl Iterator<Item = &Path> {
        self.copies
            .iter()
            .filter(move |copy| {
                !entry::spelled_alike(&copy.file, &self.path, windows) && copy.file.is_file()
            })
            .map(|copy| copy.file.as_path())
    }

    fn accepts_config_path(&self, path: &Path) -> bool {
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            return false;
        };
        self.default_path.file_name() == Some(OsStr::new(name)) || self.candidates.contains(&name)
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

/// One agent Ghostlight can set up, on one host: who the agent is, how the window shows it (its
/// monogram, and where the app's own icon may be found), and where its MCP configuration lives on
/// a computer. The roster is the one record of all of it, and the catalog of agents beside the
/// command line and an unknown agent ([`crate::agent`]).
struct AgentRow {
    /// The harness id: one per host the agent runs on.
    id: &'static str,
    /// The agent's catalog id: one per agent, shared by every host it runs on, and the id every
    /// place the window names the agent carries ([`Agent::id`]). It is the catalog's own, never
    /// the product id, which two agents can share.
    agent_id: &'static str,
    /// The product id, shared by every host of one product.
    product_id: &'static str,
    /// The product name, as the install summary says it.
    name: &'static str,
    /// The names the agent's client is known to report for itself on an MCP connection besides
    /// its harness id and product name.
    aliases: &'static [&'static str],
    target: &'static str,
    /// The agent's packaged monogram tile, the image the window serves for it when no better one
    /// is found ([`crate::agent::image`]): the same for every host of one agent.
    monogram: Monogram,
    download_url: Option<&'static str>,
    /// Where the configuration file is on the computer a context describes when the agent has
    /// none yet: the file a new configuration is written to.
    path: fn(&HarnessContext) -> PathBuf,
    /// The file names the agent reads beside `path`, in the order it prefers them, when it reads
    /// more than one: the first that exists is its file. Empty when it reads only `path`.
    candidates: &'static [&'static str],
    executables: &'static [&'static str],
    dialect: ConfigDialect,
    /// The package name of the packaged app that reads this configuration, when the client is one.
    own_package: Option<&'static str>,
    /// Where this host's own app icon may be found, in order ([`icons`]). Empty when the host has
    /// no app of its own or none is known with confidence.
    icons: &'static [IconSource],
    /// The catalog id of the agent whose icon stands for this one when none of its own is found:
    /// a terminal tool wears its vendor's desktop app's icon. `None` shows the agent's monogram
    /// instead.
    icon_family: Option<&'static str>,
    /// The catalog ids of the agents whose apps this host is known to run inside: Claude Code
    /// inside Claude Desktop, a Cline extension inside its editor. A connection is said to be the
    /// agent "in" an app only when this list names that app ([`runs_inside`]). Empty when the host
    /// runs on its own or no host app is known with confidence.
    runs_inside: &'static [&'static str],
}

/// Where every Cline host keeps its MCP settings below its own storage directory.
const CLINE_SETTINGS: &str = "settings/cline_mcp_settings.json";

/// The files OpenCode reads in its configuration directory, in the order it prefers them.
const OPENCODE_FILES: &[&str] = &["opencode.jsonc", "opencode.json"];

/// The files Kilo Code reads in its configuration directory, in the order it prefers them: its
/// own, then OpenCode's.
const KILO_FILES: &[&str] = &["kilo.jsonc", "kilo.json", "opencode.jsonc", "opencode.json"];

/// Where the Codex desktop app's icon may be found: its Store package. The Codex CLI shares the
/// agent, so it wears the app's icon.
const CODEX_ICONS: &[IconSource] = &[IconSource::WindowsPackage(CODEX_PACKAGE)];

/// Where Claude Desktop's icon may be found: its Store package, then the per-user installer's
/// program.
const CLAUDE_DESKTOP_ICONS: &[IconSource] = &[
    IconSource::WindowsPackage(CLAUDE_DESKTOP_PACKAGE),
    IconSource::WindowsExe(&[ProgramPlace::Below(
        KnownFolder::LocalAppData,
        "AnthropicClaude/claude.exe",
    )]),
];

/// Where Cursor's icon may be found: its per-user install, the launcher on `PATH`, and its Linux
/// desktop entry.
const CURSOR_ICONS: &[IconSource] = &[
    IconSource::WindowsExe(&[
        ProgramPlace::Below(KnownFolder::LocalAppData, "Programs/cursor/Cursor.exe"),
        ProgramPlace::OnPath("cursor"),
    ]),
    IconSource::LinuxDesktop(&["cursor"]),
];

/// Where Visual Studio Code's icon may be found: the program its `code` launcher starts, its
/// per-user and machine-wide installs, and its desktop entry from a package, the Snap or the
/// Flatpak.
const VSCODE_ICONS: &[IconSource] = &[
    IconSource::WindowsExe(&[
        ProgramPlace::OnPath("code"),
        ProgramPlace::Below(
            KnownFolder::LocalAppData,
            "Programs/Microsoft VS Code/Code.exe",
        ),
        ProgramPlace::Below(KnownFolder::ProgramFiles, "Microsoft VS Code/Code.exe"),
    ]),
    IconSource::LinuxDesktop(&["code", "code_code", "com.visualstudio.code"]),
];

/// Where Windsurf's icon may be found: its per-user install, the launcher on `PATH`, and its
/// Linux desktop entry.
const WINDSURF_ICONS: &[IconSource] = &[
    IconSource::WindowsExe(&[
        ProgramPlace::Below(KnownFolder::LocalAppData, "Programs/Windsurf/Windsurf.exe"),
        ProgramPlace::OnPath("windsurf"),
    ]),
    IconSource::LinuxDesktop(&["windsurf"]),
];

/// Where Zed's icon may be found: its per-user install, the program on `PATH`, and its Linux
/// desktop entry.
const ZED_ICONS: &[IconSource] = &[
    IconSource::WindowsExe(&[
        ProgramPlace::Below(KnownFolder::LocalAppData, "Programs/Zed/Zed.exe"),
        ProgramPlace::OnPath("zed"),
    ]),
    IconSource::LinuxDesktop(&["dev.zed.Zed", "zed"]),
];

/// Where Kiro's icon may be found: its per-user install, the launcher on `PATH`, and its Linux
/// desktop entry.
const KIRO_ICONS: &[IconSource] = &[
    IconSource::WindowsExe(&[
        ProgramPlace::Below(KnownFolder::LocalAppData, "Programs/Kiro/Kiro.exe"),
        ProgramPlace::OnPath("kiro"),
    ]),
    IconSource::LinuxDesktop(&["kiro"]),
];

/// Where Antigravity's icon may be found: its per-user install and its Linux desktop entry.
const ANTIGRAVITY_ICONS: &[IconSource] = &[
    IconSource::WindowsExe(&[ProgramPlace::Below(
        KnownFolder::LocalAppData,
        "Programs/Antigravity/Antigravity.exe",
    )]),
    IconSource::LinuxDesktop(&["antigravity"]),
];

/// Every agent Ghostlight can set up, one row per host.
const ROSTER: &[AgentRow] = &[
    AgentRow {
        id: "codex",
        agent_id: "codex",
        product_id: "codex",
        name: "Codex",
        aliases: &["codex-mcp-client"],
        target: "User",
        monogram: monogram!("codex.svg"),
        download_url: Some("https://developers.openai.com/codex/cli/"),
        path: |context| context.codex_config.clone(),
        candidates: &[],
        executables: &["codex"],
        dialect: ConfigDialect::CodexToml,
        own_package: None,
        icons: CODEX_ICONS,
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "claude-code",
        agent_id: "claude-code",
        product_id: "claude-code",
        name: "Claude Code",
        aliases: &[],
        target: "User",
        monogram: monogram!("claude-code.svg"),
        download_url: Some("https://claude.com/product/claude-code"),
        path: |context| context.home.join(".claude.json"),
        candidates: &[],
        executables: &["claude"],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: &[],
        icon_family: Some("claude-desktop"),
        runs_inside: &["claude-desktop"],
    },
    AgentRow {
        id: "claude-desktop",
        agent_id: "claude-desktop",
        product_id: "claude-desktop",
        name: "Claude Desktop",
        aliases: &["claude-ai"],
        target: "User",
        monogram: monogram!("claude-desktop.svg"),
        download_url: None,
        path: |context| context.roaming.join("Claude/claude_desktop_config.json"),
        candidates: &[],
        executables: &[],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: Some(CLAUDE_DESKTOP_PACKAGE),
        icons: CLAUDE_DESKTOP_ICONS,
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "cursor",
        agent_id: "cursor",
        product_id: "cursor",
        name: "Cursor",
        aliases: &["cursor-vscode"],
        target: "User",
        monogram: monogram!("cursor.svg"),
        download_url: Some("https://cursor.com/downloads"),
        path: |context| context.home.join(".cursor/mcp.json"),
        candidates: &[],
        executables: &["cursor", "cursor-agent"],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: CURSOR_ICONS,
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "vscode",
        agent_id: "vscode",
        product_id: "vscode",
        name: "Visual Studio Code",
        aliases: &[],
        target: "User",
        monogram: monogram!("vscode.svg"),
        download_url: Some("https://code.visualstudio.com/download"),
        path: |context| context.roaming.join("Code/User/mcp.json"),
        candidates: &[],
        executables: &["code"],
        dialect: ConfigDialect::Json(JsonDialect::Servers),
        own_package: None,
        icons: VSCODE_ICONS,
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "windsurf",
        agent_id: "windsurf",
        product_id: "windsurf",
        name: "Windsurf",
        aliases: &[],
        target: "User",
        monogram: monogram!("windsurf.svg"),
        download_url: Some("https://windsurf.com/editor/download"),
        path: |context| context.home.join(".codeium/windsurf/mcp_config.json"),
        candidates: &[],
        executables: &["windsurf"],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: WINDSURF_ICONS,
        icon_family: None,
        runs_inside: &[],
    },
    // Windsurf's current official distribution is named Devin and moved its user MCP
    // configuration. Keep the historical target above so either installed generation can be
    // managed without guessing one path from the other (ADR-0148).
    AgentRow {
        id: "devin",
        agent_id: "devin",
        product_id: "windsurf",
        name: "Devin",
        aliases: &[],
        target: "Devin user",
        monogram: monogram!("windsurf.svg"),
        download_url: Some("https://windsurf.com/editor/download"),
        path: |context| context.config.join("devin/mcp_config.json"),
        candidates: &[],
        executables: &["devin", "devin-desktop", "surf"],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "zed",
        agent_id: "zed",
        product_id: "zed",
        name: "Zed",
        aliases: &[],
        target: "User",
        monogram: monogram!("zed.svg"),
        download_url: Some("https://zed.dev/download"),
        path: |context| {
            if context.windows {
                context.roaming.join("Zed/settings.json")
            } else {
                context.config.join("zed/settings.json")
            }
        },
        candidates: &[],
        executables: &["zed", "zeditor"],
        dialect: ConfigDialect::Json(JsonDialect::ContextServers),
        own_package: None,
        icons: ZED_ICONS,
        icon_family: None,
        runs_inside: &[],
    },
    // ZCode's MCP registration lives in its shared CLI configuration under `mcp.servers`
    // (ADR-0141). No official download destination is pinned until one is verified.
    AgentRow {
        id: "zcode",
        agent_id: "zcode",
        product_id: "zcode",
        name: "ZCode",
        aliases: &[],
        target: "User",
        monogram: monogram!("zcode.svg"),
        download_url: None,
        path: |context| context.home.join(".zcode/cli/config.json"),
        candidates: &[],
        executables: &["zcode"],
        dialect: ConfigDialect::Json(JsonDialect::ZCode),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "opencode",
        agent_id: "opencode",
        product_id: "opencode",
        name: "OpenCode",
        aliases: &[],
        target: "User",
        monogram: monogram!("opencode.svg"),
        download_url: Some("https://opencode.ai/docs"),
        path: |context| context.home.join(".config/opencode/opencode.json"),
        candidates: OPENCODE_FILES,
        executables: &["opencode", "opencode2"],
        dialect: ConfigDialect::OpenCode,
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "crush",
        agent_id: "crush",
        product_id: "crush",
        name: "Crush",
        aliases: &[],
        target: "User",
        monogram: monogram!("crush.svg"),
        download_url: Some("https://github.com/charmbracelet/crush"),
        path: |context| {
            context
                .crush_config
                .clone()
                .unwrap_or_else(|| context.home.join(".config/crush/crush.json"))
        },
        candidates: &[],
        executables: &["crush"],
        dialect: ConfigDialect::Json(JsonDialect::Crush),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "copilot-cli",
        agent_id: "copilot-cli",
        product_id: "copilot-cli",
        name: "GitHub Copilot CLI",
        aliases: &[],
        target: "CLI",
        monogram: monogram!("copilot-cli.svg"),
        download_url: Some("https://github.com/github/copilot-cli"),
        path: |context| context.home.join(".copilot/mcp-config.json"),
        candidates: &[],
        executables: &["copilot"],
        dialect: ConfigDialect::Json(JsonDialect::Copilot),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "cline-cli",
        agent_id: "cline",
        product_id: "cline",
        name: "Cline",
        aliases: &[],
        target: "CLI",
        monogram: monogram!("cline.svg"),
        download_url: Some("https://docs.cline.bot/cline-cli/overview"),
        path: |context| context.home.join(".cline/data").join(CLINE_SETTINGS),
        candidates: &[],
        executables: &["cline"],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "cline-vscode",
        agent_id: "cline",
        product_id: "cline",
        name: "Cline",
        aliases: &[],
        target: "Visual Studio Code",
        monogram: monogram!("cline.svg"),
        download_url: Some(
            "https://marketplace.visualstudio.com/items?itemName=saoudrizwan.claude-dev",
        ),
        path: |context| {
            context
                .roaming
                .join("Code/User/globalStorage/saoudrizwan.claude-dev")
                .join(CLINE_SETTINGS)
        },
        candidates: &[],
        executables: &[],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &["vscode"],
    },
    AgentRow {
        id: "cline-cursor",
        agent_id: "cline",
        product_id: "cline",
        name: "Cline",
        aliases: &[],
        target: "Cursor",
        monogram: monogram!("cline.svg"),
        download_url: Some("https://docs.cline.bot/getting-started/installing-cline"),
        path: |context| {
            context
                .roaming
                .join("Cursor/User/globalStorage/saoudrizwan.claude-dev")
                .join(CLINE_SETTINGS)
        },
        candidates: &[],
        executables: &[],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &["cursor"],
    },
    AgentRow {
        id: "cline-windsurf",
        agent_id: "cline",
        product_id: "cline",
        name: "Cline",
        aliases: &[],
        target: "Windsurf",
        monogram: monogram!("cline.svg"),
        download_url: Some("https://docs.cline.bot/getting-started/installing-cline"),
        path: |context| {
            context
                .roaming
                .join("Windsurf/User/globalStorage/saoudrizwan.claude-dev")
                .join(CLINE_SETTINGS)
        },
        candidates: &[],
        executables: &[],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &["windsurf"],
    },
    AgentRow {
        id: "kiro",
        agent_id: "kiro",
        product_id: "kiro",
        name: "Kiro",
        aliases: &[],
        target: "CLI and IDE",
        monogram: monogram!("kiro.svg"),
        download_url: Some("https://kiro.dev/downloads/"),
        path: |context| context.home.join(".kiro/settings/mcp.json"),
        candidates: &[],
        executables: &["kiro", "kiro-cli"],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: KIRO_ICONS,
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "qwen-code",
        agent_id: "qwen-code",
        product_id: "qwen-code",
        name: "Qwen Code",
        aliases: &[],
        target: "CLI",
        monogram: monogram!("qwen-code.svg"),
        download_url: Some("https://qwenlm.github.io/qwen-code-docs/en/users/installation/"),
        path: |context| context.home.join(".qwen/settings.json"),
        candidates: &[],
        executables: &["qwen"],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "junie",
        agent_id: "junie",
        product_id: "junie",
        name: "Junie",
        aliases: &[],
        target: "CLI and JetBrains",
        monogram: monogram!("junie.svg"),
        download_url: Some("https://junie.jetbrains.com/docs/get-started-with-junie-cli.html"),
        path: |context| context.home.join(".junie/mcp/mcp.json"),
        candidates: &[],
        executables: &["junie"],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "kilo-code",
        agent_id: "kilo-code",
        product_id: "kilo-code",
        name: "Kilo Code",
        aliases: &[],
        target: "CLI",
        monogram: monogram!("kilo-code.svg"),
        download_url: Some("https://kilo.ai/"),
        path: |context| context.config.join("kilo").join("kilo.json"),
        candidates: KILO_FILES,
        executables: &["kilo"],
        dialect: ConfigDialect::Json(JsonDialect::OpenCodeV1),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "goose",
        agent_id: "goose",
        product_id: "goose",
        name: "goose",
        aliases: &[],
        target: "CLI and desktop",
        monogram: monogram!("goose.svg"),
        download_url: Some("https://block.github.io/goose/docs/getting-started/installation/"),
        path: |context| context.config.join("goose/config.yaml"),
        candidates: &[],
        executables: &["goose"],
        dialect: ConfigDialect::Manual(ManualDialect::Goose),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "continue",
        agent_id: "continue",
        product_id: "continue",
        name: "Continue",
        aliases: &[],
        target: "CLI and IDE",
        monogram: monogram!("continue.svg"),
        download_url: Some("https://docs.continue.dev/cli/quickstart"),
        path: |context| context.home.join(".continue/config.yaml"),
        candidates: &[],
        executables: &["cn"],
        dialect: ConfigDialect::Manual(ManualDialect::Continue),
        own_package: None,
        icons: &[],
        icon_family: None,
        runs_inside: &[],
    },
    AgentRow {
        id: "antigravity",
        agent_id: "antigravity",
        product_id: "antigravity",
        name: "Antigravity",
        aliases: &[],
        target: "CLI",
        monogram: monogram!("antigravity.svg"),
        download_url: Some("https://antigravity.google/download?platform=linux"),
        path: |context| context.home.join(".gemini/config/mcp_config.json"),
        candidates: &[],
        executables: &["agy"],
        dialect: ConfigDialect::Json(JsonDialect::McpServers),
        own_package: None,
        icons: ANTIGRAVITY_ICONS,
        icon_family: None,
        runs_inside: &[],
    },
];

impl AgentRow {
    /// Whether the agent's client calls itself `reported`: its catalog id, its harness id, its
    /// product name or one of its aliases, ignoring case. The product id is not matched, because
    /// two products can share one: Devin's product id is Windsurf's id.
    fn is_called(&self, reported: &str) -> bool {
        [self.agent_id, self.id, self.name]
            .iter()
            .chain(self.aliases)
            .any(|known| known.eq_ignore_ascii_case(reported))
    }

    /// The agent of this row, as the window names and shows it: the same for every host of one
    /// agent.
    fn agent(&self) -> Agent {
        Agent::listed(self.agent_id, self.name, self.monogram)
    }

    /// The names of the Store packages of this host's own app: its packaged app, and the
    /// packages its icon sources name.
    fn store_packages(&self) -> Vec<&'static str> {
        self.own_package
            .into_iter()
            .chain(self.icons.iter().filter_map(|source| match source {
                IconSource::WindowsPackage(name) => Some(*name),
                IconSource::WindowsExe(_) | IconSource::LinuxDesktop(_) => None,
            }))
            .collect()
    }

    /// Every place the agent's settings may be on the computer `context` describes, among the
    /// Store `packages` of this look, the one it reads first leading; in each, the configuration
    /// file is the first of the candidates beside the file `path` names that exists, else that
    /// file.
    ///
    /// Inside the container of the agent's own app, the plain path is that app's merged view: it
    /// shows the app's own private copy when that copy leads, so it is not another copy then.
    fn copies_on(&self, context: &HarnessContext, packages: &Packages) -> Vec<SettingsCopy> {
        let mut copies = packages.copies(&(self.path)(context), &self.store_packages(), |file| {
            self.candidates
                .iter()
                .map(|candidate| file.with_file_name(candidate))
                .find(|candidate| candidate.exists())
                .unwrap_or_else(|| file.to_path_buf())
        });
        let own_leads = copies
            .first()
            .is_some_and(|copy| copy.place == CopyPlace::OwnPackage);
        if own_leads && context.inside_own_app(self.own_package) {
            copies.retain(|copy| copy.place != CopyPlace::Plain);
        }
        copies
    }

    /// The row as it stands on the computer `context` describes, among the Store `packages` of
    /// this look, before any location the person chose.
    fn on(&self, context: &HarnessContext, packages: &Packages) -> HarnessDefinition {
        let copies = self.copies_on(context, packages);
        let path = copies
            .first()
            .map_or_else(|| (self.path)(context), |copy| copy.file.clone());
        HarnessDefinition {
            id: self.id,
            agent_id: self.agent_id,
            product_id: self.product_id,
            name: self.name,
            label: self.name.to_owned(),
            target: self.target,
            monogram: self.monogram,
            download_url: self.download_url,
            default_path: path.clone(),
            path,
            candidates: self.candidates,
            located_executable: None,
            located_stale: false,
            executables: self.executables,
            dialect: self.dialect,
            own_package: self.own_package,
            copies,
        }
    }
}

/// Every roster row as this look finds it: the Store packages are listed once for all of them.
fn definitions(context: &HarnessContext) -> Vec<HarnessDefinition> {
    let packages = context.packages();
    let mut rows: Vec<HarnessDefinition> = ROSTER
        .iter()
        .map(|row| row.on(context, &packages))
        .collect();
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

/// The roster's agent whose client calls itself `reported`, ignoring case and surrounding space;
/// none for a name Ghostlight does not know. A roster row matches its catalog id, its harness id,
/// its product name and its aliases, never its product id. Every host of one agent names the same
/// agent, because the name a client reports does not say the host.
///
/// The name stays the client's own claim: it names the agent for a person and grants nothing.
#[must_use]
pub fn agent_called(reported: &str) -> Option<Agent> {
    let reported = reported.trim();
    ROSTER
        .iter()
        .find(|row| row.is_called(reported))
        .map(AgentRow::agent)
}

/// The catalog agent whose catalog id is `agent_id`, as the window names and shows it; none for an
/// id outside the catalog.
#[must_use]
pub fn catalog_agent(agent_id: &str) -> Option<Agent> {
    ROSTER
        .iter()
        .find(|row| row.agent_id == agent_id)
        .map(AgentRow::agent)
}

/// Whether the catalog knows that the agent `agent_id` runs inside the app of the agent `host_id`
/// on one of its hosts ([`AgentRow::runs_inside`]). False for any id outside the catalog.
#[must_use]
pub fn runs_inside(agent_id: &str, host_id: &str) -> bool {
    ROSTER
        .iter()
        .filter(|row| row.agent_id == agent_id)
        .any(|row| row.runs_inside.contains(&host_id))
}

/// The packaged monogram tile of the catalog agent `agent_id`; none for an id outside the catalog.
#[must_use]
pub fn monogram_of(agent_id: &str) -> Option<Monogram> {
    ROSTER
        .iter()
        .find(|row| row.agent_id == agent_id)
        .map(|row| row.monogram)
}

/// Every roster row's monogram tile, for tests that check the packaged tiles.
#[cfg(test)]
pub(crate) fn monograms() -> Vec<Monogram> {
    ROSTER.iter().map(|row| row.monogram).collect()
}

/// Where each catalog agent's own icon may be found, one entry per agent in roster order: its name,
/// its hosts' icon sources in roster order, and the first family a host names.
#[must_use]
pub fn icon_catalog() -> Vec<AgentIconSources> {
    let mut catalog: Vec<AgentIconSources> = Vec::new();
    for row in ROSTER {
        if let Some(agent) = catalog
            .iter_mut()
            .find(|agent| agent.agent_id == row.agent_id)
        {
            agent.sources.extend_from_slice(row.icons);
            agent.family = agent.family.or(row.icon_family);
        } else {
            catalog.push(AgentIconSources {
                agent_id: row.agent_id,
                name: row.name,
                sources: row.icons.to_vec(),
                family: row.icon_family,
            });
        }
    }
    catalog
}

/// Where each catalog agent's app may be installed, one entry per agent in roster order: its
/// hosts' icon sources and the commands they are started by, in roster order ([`apps`]).
#[must_use]
pub fn app_catalog() -> Vec<apps::AgentAppSources> {
    let mut catalog: Vec<apps::AgentAppSources> = Vec::new();
    for row in ROSTER {
        if let Some(agent) = catalog
            .iter_mut()
            .find(|agent| agent.agent_id == row.agent_id)
        {
            agent.sources.extend_from_slice(row.icons);
            agent.commands.extend_from_slice(row.executables);
        } else {
            catalog.push(apps::AgentAppSources {
                agent_id: row.agent_id,
                sources: row.icons.to_vec(),
                commands: row.executables.to_vec(),
            });
        }
    }
    catalog
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
    // A change made inside another app's container would reach only that app, so none is offered.
    let private = context.private_to(definition);
    let registration = registration_state(context, definition);
    let retired = matches!(registration, Ok(RegistrationState::Retired));
    let state = match &registration {
        Ok(RegistrationState::Missing) if detected => HarnessState::Available,
        Ok(RegistrationState::Missing) => HarnessState::NotDetected,
        Ok(RegistrationState::Current) => HarnessState::Installed,
        Ok(RegistrationState::Updatable | RegistrationState::Retired) => HarnessState::Updatable,
        Ok(RegistrationState::Launcher(_) | RegistrationState::Foreign(_)) | Err(_) => {
            HarnessState::NeedsAttention
        }
    };
    // The blocked causes are named from what the inspection actually saw, never conflated
    // (ADR-0135). The evidence line carries the specifics beside the claim.
    let mut detail: String = match (&registration, private) {
        (_, Some(container)) if !manual => words::client_kept_private(&container.app),
        (Ok(RegistrationState::Launcher(launcher)), _) => words::launcher_detail(launcher),
        (Ok(RegistrationState::Foreign(_)), _) => words::CLIENT_FOREIGN_ENTRY.into(),
        (Err(error @ HarnessError::Malformed(_)), _) => error.to_string(),
        (Err(_), _) => words::CLIENT_UNREADABLE.into(),
        _ if manual => manual_setup_sentence(definition),
        _ => words::client_detail(state, edge_ready, retired.then_some(RETIRED_ENTRY)),
    };
    let evidence: Option<String> = match &registration {
        Ok(RegistrationState::Launcher(launcher)) => {
            Some(words::launcher_evidence(launcher, &context.edge))
        }
        Ok(RegistrationState::Foreign(found_command)) => Some(match found_command {
            Some(found) => {
                words::other_program_evidence(&found.program, found.arguments, &context.edge)
            }
            None => words::no_command_evidence(&context.edge),
        }),
        _ => None,
    };
    if definition.located_stale {
        detail = words::with_located_path_missing(&detail);
    }
    let changeable = private.is_none();
    let can_fix = changeable
        && edge_ready
        && matches!(
            registration,
            Ok(RegistrationState::Launcher(_) | RegistrationState::Foreign(_))
        );
    let owned_copies = owned_other_copies(context, definition);
    let attention = match &registration {
        Ok(RegistrationState::Launcher(launcher)) => Some(Attention::Launcher(launcher.pin)),
        Ok(RegistrationState::Foreign(_)) => Some(Attention::ForeignEntry),
        Err(HarnessError::Malformed(_)) => Some(Attention::Malformed),
        Err(_) => Some(Attention::Unreadable),
        Ok(_) => None,
    };
    let mut summary = HarnessSummary {
        id: definition.id.into(),
        product_id: definition.product_id.into(),
        name: definition.name.into(),
        label: definition.label.clone(),
        target: definition.target.into(),
        agent: Agent::listed(definition.agent_id, definition.name, definition.monogram),
        state,
        detail,
        evidence,
        attention,
        can_install: changeable && edge_ready && !manual && state.takes_install(),
        can_fix,
        can_uninstall: false,
        can_download: definition.download_url.is_some(),
        download_url: definition.download_url.map(str::to_owned),
        can_locate: true,
        copy_only: manual,
        private_view: !changeable,
        config_path: definition.path.to_string_lossy().into_owned(),
        packaged: definition.packaged(context.windows),
        other_copy: definition
            .other_copies(context.windows)
            .next()
            .map(|path| path.to_string_lossy().into_owned()),
        owned_copies: owned_copies
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        connector_command: mcp_command_line(&context.edge),
        manual_setup: manual_setup(context, definition)
            .unwrap_or_else(|_| words::manual_setup_command(&mcp_command_line(&context.edge))),
    };
    summary.can_uninstall = changeable && summary.has_entry_to_remove();
    summary
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
        .any(|root| entry::spelled_alike(parent, root, context.windows))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RegistrationState {
    Missing,
    Current,
    Updatable,
    /// An owned entry starts `ghostlight-mcp-connector`, a program this release does not install.
    Retired,
    /// The entry starts Ghostlight's own npm launcher. Ghostlight did not write it and cannot ask
    /// npm what it runs (ADR-0028), so it is never current, and only Fix changes it (ADR-0203
    /// Decision 10.3).
    Launcher(NpmLauncher),
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

/// The program that runs an npm package.
const NPX_PROGRAM: &str = "npx";

/// The Windows command interpreter some clients start `npx` through, and its switch that runs the
/// command after it.
const WINDOWS_SHELL: (&str, &str) = ("cmd", "/c");

/// The `npx` options a launcher entry may carry before the package. They answer npm's own
/// prompts and choose nothing.
const NPX_QUIET_OPTIONS: [&str; 4] = ["-y", "--yes", "-q", "--quiet"];

/// The npm package Ghostlight's launcher is published as.
const NPM_PACKAGE: &str = "ghostlight";

/// The npm tag for whichever version npm serves as the newest.
const NPM_LATEST_TAG: &str = "latest";

/// The characters besides letters and digits a version, range or tag after `ghostlight@` is
/// written with. A spec with any other character, such as a URL or a path, is not the launcher.
const NPM_SPEC_MARKS: &str = ".-+^~=*<>";

/// The first major release whose npm launcher starts a Ghostlight that works with this one: 2.0
/// changed the relay and the extension's protocol.
pub const FIRST_LAUNCHER_MAJOR: u64 = 2;

/// Which version Ghostlight's npm launcher entry asks npm for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LauncherPin {
    /// No version, the `latest` tag or another name npm resolves: npm chooses the version at each
    /// launch.
    Unpinned,
    /// A release before 2.0, such as 1.x, whose launcher starts a Ghostlight that cannot work
    /// with this one.
    Earlier,
    /// A 2.0 or later version or range.
    Pinned,
}

impl LauncherPin {
    /// The pin the package `spec` after `ghostlight@` names, or none.
    fn of(spec: Option<&str>) -> Self {
        let Some(spec) = spec.filter(|spec| !spec.is_empty() && *spec != NPM_LATEST_TAG) else {
            return Self::Unpinned;
        };
        let version = spec.trim_start_matches(['^', '~', '=', 'v']);
        let major: String = version.chars().take_while(char::is_ascii_digit).collect();
        match major.parse::<u64>() {
            Ok(major) if major < FIRST_LAUNCHER_MAJOR => Self::Earlier,
            Ok(_) => Self::Pinned,
            Err(_) => Self::Unpinned,
        }
    }
}

/// Ghostlight's own npm launcher as an entry starts it: `npx` with only quiet options, the
/// `ghostlight` package, and no command or `mcp`, through `cmd /c` or not.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NpmLauncher {
    /// Which version the entry asks npm for.
    pub pin: LauncherPin,
    /// The package as the entry names it, `ghostlight` or `ghostlight@<version>`, bounded for
    /// disclosure.
    pub package: String,
}

impl NpmLauncher {
    /// The launcher an entry starting `command` with `args` names, or none when it starts anything
    /// else. A command that names an existing file is that program whole; any other command is
    /// read as a whole command line.
    fn from_entry(command: &str, args: Option<&[&str]>) -> Option<Self> {
        let mut words: Vec<&str> = if Path::new(command).is_file() {
            vec![command]
        } else {
            command.split_whitespace().collect()
        };
        words.extend(args.unwrap_or_default().iter().copied());
        let mut words = words.into_iter().peekable();
        if words
            .peek()
            .is_some_and(|word| command_name(word, WINDOWS_SHELL.0))
        {
            words.next();
            if !words
                .next()
                .is_some_and(|word| word.eq_ignore_ascii_case(WINDOWS_SHELL.1))
            {
                return None;
            }
        }
        if !words
            .next()
            .is_some_and(|word| command_name(word, NPX_PROGRAM))
        {
            return None;
        }
        let package = words.find(|word| !NPX_QUIET_OPTIONS.contains(word))?;
        let spec = match package.strip_prefix(NPM_PACKAGE)? {
            "" => None,
            rest => Some(rest.strip_prefix('@')?),
        };
        if spec.is_some_and(|spec| {
            !spec
                .chars()
                .all(|mark| mark.is_ascii_alphanumeric() || NPM_SPEC_MARKS.contains(mark))
        }) {
            return None;
        }
        let rest: Vec<&str> = words.collect();
        if !(rest.is_empty() || rest == [MCP_ARGUMENT]) {
            return None;
        }
        Some(Self {
            pin: LauncherPin::of(spec),
            package: bounded_disclosure(package)?,
        })
    }
}

/// The refusal of an install or an uninstall that finds an entry Ghostlight does not own under its
/// key, starting `command` with `args` when it names a command: Ghostlight's npm launcher is said
/// as itself, anything else as foreign.
fn unowned_entry_refusal(command: Option<&str>, args: Option<&[&str]>) -> HarnessError {
    match command.and_then(|command| NpmLauncher::from_entry(command, args)) {
        Some(_) => HarnessError::LauncherEntry,
        None => HarnessError::ForeignEntry,
    }
}

/// The state of Ghostlight's entry in `definition`'s own configuration file.
fn registration_state(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<RegistrationState, HarnessError> {
    registration_state_at(context, definition, &definition.path)
}

/// The state of Ghostlight's entry in the file at `path`, read as the configuration of
/// `definition`.
fn registration_state_at(
    context: &HarnessContext,
    definition: &HarnessDefinition,
    path: &Path,
) -> Result<RegistrationState, HarnessError> {
    if !path.exists() || matches!(definition.dialect, ConfigDialect::Manual(_)) {
        return Ok(RegistrationState::Missing);
    }
    let source = fs::read_to_string(path).map_err(HarnessError::Read)?;
    match definition.dialect {
        ConfigDialect::CodexToml => inspect_toml(&source, context),
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
fn json_entry_state(
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

fn inspect_toml(source: &str, context: &HarnessContext) -> Result<RegistrationState, HarnessError> {
    let document = parse_toml(source)?;
    document
        .get("mcp_servers")
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
fn toml_entry_state(
    entry: &Item,
    context: &HarnessContext,
) -> Result<RegistrationState, HarnessError> {
    let command = entry
        .get("command")
        .and_then(Item::as_str)
        .unwrap_or_default();
    let args = toml_entry_args(entry);
    let state =
        command_registration_state(command, args.as_deref(), &context.edge, context.windows);
    if !matches!(
        state,
        RegistrationState::Current | RegistrationState::Updatable | RegistrationState::Retired
    ) {
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
fn missing_codex_desktop_environment(
    entry: &Item,
    linux: bool,
) -> Result<Vec<&'static str>, HarnessError> {
    if !linux {
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

fn forward_codex_desktop_environment(entry: &mut Item, linux: bool) -> Result<(), HarnessError> {
    let missing = missing_codex_desktop_environment(entry, linux)?;
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
        ConfigDialect::CodexToml => edit_toml(context, &definition.path, true)?,
        ConfigDialect::Manual(_) => return Err(manual_refusal(definition)),
        ConfigDialect::Json(dialect) => edit_json(context, &definition.path, dialect, true)?,
        ConfigDialect::OpenCode => {
            let source = read_or_empty(&definition.path)?;
            let dialect = opencode_dialect(context, &source)?;
            edit_json(context, &definition.path, dialect, true)?
        }
    };
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        message: if changed {
            words::client_registered(&summary.label)
        } else {
            words::client_already_current(&summary.label)
        },
        summary,
    })
}

/// Remove Ghostlight's entry from its configuration and also from every other copy of the
/// agent's settings that holds an entry Ghostlight owns, since an earlier install may have written
/// either. A copy that holds no owned entry, or cannot be read, is left as it is. When only other
/// copies hold one, the configuration itself is left as it is too, so a foreign entry or an
/// unreadable text there does not stop their cleanup.
fn apply_uninstall(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<HarnessActionResult, HarnessError> {
    refuse_private(context, definition)?;
    let others = owned_other_copies(context, definition);
    let mut changed =
        if others.is_empty() || holds_owned_entry(context, definition, &definition.path) {
            uninstall_at(context, definition, &definition.path)?
        } else {
            false
        };
    for other in others {
        changed |= uninstall_at(context, definition, other)?;
    }
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        message: if changed {
            words::client_removed(&summary.label)
        } else {
            words::client_had_no_entry(&summary.label)
        },
        summary,
    })
}

/// The other copies of the agent's settings, besides the configuration itself, that hold an entry
/// Ghostlight owns. Uninstall removes Ghostlight's entry from each, and inspection names them, so
/// what a dry run says and what Uninstall does come from this one reading.
fn owned_other_copies<'a>(
    context: &HarnessContext,
    definition: &'a HarnessDefinition,
) -> Vec<&'a Path> {
    definition
        .other_copies(context.windows)
        .filter(|path| holds_owned_entry(context, definition, path))
        .collect()
}

/// Whether the file at `path`, read as the configuration of `definition`, holds an entry
/// Ghostlight owns, current or not.
fn holds_owned_entry(
    context: &HarnessContext,
    definition: &HarnessDefinition,
    path: &Path,
) -> bool {
    matches!(
        registration_state_at(context, definition, path),
        Ok(RegistrationState::Current | RegistrationState::Updatable | RegistrationState::Retired)
    )
}

/// Remove Ghostlight's own entry from the file at `path`, read as the configuration of
/// `definition`. Whether the file changed.
fn uninstall_at(
    context: &HarnessContext,
    definition: &HarnessDefinition,
    path: &Path,
) -> Result<bool, HarnessError> {
    match definition.dialect {
        ConfigDialect::CodexToml => edit_toml(context, path, false),
        ConfigDialect::Manual(_) => Err(manual_refusal(definition)),
        ConfigDialect::Json(dialect) => edit_json(context, path, dialect, false),
        ConfigDialect::OpenCode => {
            let source = read_or_empty(path)?;
            let dialect = opencode_dialect(context, &source)?;
            edit_json(context, path, dialect, false)
        }
    }
}

fn apply_fix(
    context: &HarnessContext,
    definition: &HarnessDefinition,
) -> Result<HarnessActionResult, HarnessError> {
    refuse_private(context, definition)?;
    if !context.edge.is_file() {
        return Err(HarnessError::EdgeMissing);
    }
    let launcher = matches!(
        registration_state(context, definition),
        Ok(RegistrationState::Launcher(_))
    );
    let changed = match definition.dialect {
        ConfigDialect::CodexToml => fix_toml(context, &definition.path)?,
        ConfigDialect::Manual(_) => return Err(manual_refusal(definition)),
        ConfigDialect::Json(dialect) => fix_json(context, &definition.path, dialect)?,
        ConfigDialect::OpenCode => {
            let source = read_or_empty(&definition.path)?;
            let dialect = opencode_dialect(context, &source)?;
            fix_json(context, &definition.path, dialect)?
        }
    };
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        message: if launcher {
            words::launcher_fixed(&summary.label)
        } else {
            words::client_fixed(&summary.label)
        },
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
    context: &HarnessContext,
    path: &Path,
    dialect: JsonDialect,
    install: bool,
) -> Result<bool, HarnessError> {
    edit_json_with(
        context,
        path,
        dialect,
        if install {
            RegistrationEdit::Install
        } else {
            RegistrationEdit::Uninstall
        },
    )
}

fn fix_json(
    context: &HarnessContext,
    path: &Path,
    dialect: JsonDialect,
) -> Result<bool, HarnessError> {
    edit_json_with(context, path, dialect, RegistrationEdit::Fix)
}

/// Make the edit `intent` to Ghostlight's entry in the JSON file at `path`, written in `dialect`,
/// for the edge and the computer `context` describes. Whether the file changed.
///
/// An install edits an entry Ghostlight owns in place and keeps every key that is the client's;
/// it writes the whole entry only where there is none. Fix replaces a foreign entry whole.
fn edit_json_with(
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

fn edit_toml(context: &HarnessContext, path: &Path, install: bool) -> Result<bool, HarnessError> {
    edit_toml_with(
        context,
        path,
        if install {
            RegistrationEdit::Install
        } else {
            RegistrationEdit::Uninstall
        },
    )
}

fn fix_toml(context: &HarnessContext, path: &Path) -> Result<bool, HarnessError> {
    edit_toml_with(context, path, RegistrationEdit::Fix)
}

/// Make the edit `intent` to Ghostlight's entry in the Codex TOML file at `path`, for the edge and
/// the computer `context` describes. Whether the file changed.
fn edit_toml_with(
    context: &HarnessContext,
    path: &Path,
    intent: RegistrationEdit,
) -> Result<bool, HarnessError> {
    let edge = context.edge.as_path();
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
        forward_codex_desktop_environment(&mut updated, context.linux)?;
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

/// The `type` Ghostlight's entry carries in `dialect`, where the dialect needs one to start a
/// local program. Ghostlight owns it beside the command and its arguments.
const fn json_entry_type(dialect: JsonDialect) -> Option<&'static str> {
    match dialect {
        JsonDialect::Copilot | JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => Some("local"),
        JsonDialect::Servers | JsonDialect::Crush => Some("stdio"),
        JsonDialect::McpServers | JsonDialect::ContextServers | JsonDialect::ZCode => None,
    }
}

/// The keys Ghostlight sets in its entry in `dialect` for `edge` at every install: the command,
/// its arguments and the dialect's `type`. Every key named neither here nor in
/// [`json_entry_defaults`] is the client's (ADR-0203 Decision 10.1).
fn json_owned_keys(edge: &Path, dialect: JsonDialect) -> Vec<(&'static str, Value)> {
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
fn json_entry_defaults(dialect: JsonDialect) -> Vec<(&'static str, Value)> {
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
fn expected_json_entry(edge: &Path, dialect: JsonDialect) -> Value {
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
fn update_owned_json_entry(
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
        | JsonDialect::ZCode => entry.get(COMMAND_KEY).and_then(Value::as_str),
        JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => entry
            .get(COMMAND_KEY)
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
        | JsonDialect::ZCode => entry.get(ARGS_KEY)?.as_array()?,
        JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2 => {
            let command = entry.get(COMMAND_KEY)?.as_array()?;
            command.get(1..)?
        }
    };
    values.iter().map(Value::as_str).collect()
}

/// The sentence a person reads for a client whose file Ghostlight never edits.
fn manual_setup_sentence(definition: &HarnessDefinition) -> String {
    let file = definition
        .path
        .file_name()
        .map(|name| name.to_string_lossy());
    words::manual_setup(file.as_deref())
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
            forward_codex_desktop_environment(&mut entry, context.linux)?;
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
    entry::same_location_on(actual_root, expected_root, windows)
}

fn command_registration_state(
    command: &str,
    args: Option<&[&str]>,
    edge: &Path,
    windows: bool,
) -> RegistrationState {
    if !command_owned(command, args, edge, windows) {
        return NpmLauncher::from_entry(command, args).map_or_else(
            || RegistrationState::Foreign(ForeignCommand::from_entry(command, args)),
            RegistrationState::Launcher,
        );
    }
    let current = command_name(command, EDGE_COMMAND_NAME)
        && entry::same_location_on(Path::new(command), edge, windows)
        && args == Some(&MCP_ARGUMENTS[..]);
    if current {
        RegistrationState::Current
    } else if command_name(command, RETIRED_MCP_CONNECTOR_NAME) {
        RegistrationState::Retired
    } else {
        RegistrationState::Updatable
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

/// Write the located paths owner-only: they are a record about the person, not the client's file,
/// so they take Ghostlight's own private replace and no backup copy beside them.
fn write_location_overrides(
    context: &HarnessContext,
    overrides: &LocationOverrides,
) -> Result<(), HarnessError> {
    let bytes = serde_json::to_vec_pretty(overrides)
        .map_err(|error| HarnessError::MalformedLocationState(error.to_string()))?;
    record::atomic_replace(&context.locations, &bytes).map_err(HarnessError::Write)
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

/// How Ghostlight replaces a client's configuration: the client's access kept, the old file
/// copied beside it first, and a link written through. A configuration tracked through a link (a
/// synced dotfiles repository, say) is a deliberate choice, so it stays a link to the file it
/// names.
const CONFIGURATION_WRITE: Replace = Replace {
    access: Access::Kept,
    backup: true,
    link: Link::Follow,
};

/// Remove from `directory` each replace temporary ([`record::is_temporary`]) that is at least
/// [`STRAY_TEMPORARY_AGE`] old at `now`. Returns how many were removed.
fn remove_stray_temporaries_in(directory: &Path, now: SystemTime) -> usize {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| record::is_temporary(&entry.file_name()))
        .filter(|entry| {
            entry
                .metadata()
                .ok()
                .filter(fs::Metadata::is_file)
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age >= STRAY_TEMPORARY_AGE)
        })
        .filter(|entry| fs::remove_file(entry.path()).is_ok())
        .count()
}

/// Replace a client's configuration at `path` with `bytes`, as [`CONFIGURATION_WRITE`] says.
fn replace_with_backup(path: &Path, bytes: &[u8]) -> Result<(), HarnessError> {
    CONFIGURATION_WRITE
        .write(path, bytes)
        .map_err(HarnessError::Write)
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
    #[error("{}", words::unknown_client_id(.0))]
    UnknownHarness(String),
    /// The selected host does not have an official download on this platform.
    #[error("{}", words::no_client_download(.0))]
    NoDownload(String),
    /// The installed edge is unavailable.
    #[error("{}", words::EDGE_MISSING)]
    EdgeMissing,
    /// The harness config could not be read.
    #[error("{}", words::configuration_unreadable(&.0.to_string()))]
    Read(io::Error),
    /// The harness config could not be written.
    #[error("{}", words::configuration_unwritable(&.0.to_string()))]
    Write(io::Error),
    /// The config is not structurally safe to edit. The payload is the 1-indexed line of the
    /// parse error, when the parser located one. Nothing the file says is ever carried.
    #[error("{}", words::configuration_malformed(*.0))]
    Malformed(Option<usize>),
    /// The client's file is never edited by Ghostlight; the person pastes the setup.
    #[error("{0}")]
    ManualSetup(String),
    /// A foreign server uses Ghostlight's registration name.
    #[error("{}", words::FOREIGN_ENTRY_LEFT)]
    ForeignEntry,
    /// Ghostlight's npm launcher holds Ghostlight's registration name; only Fix changes it.
    #[error("{}", words::LAUNCHER_ENTRY_LEFT)]
    LauncherEntry,
    /// The explicit fix no longer points at one foreign entry.
    #[error("{}", words::NOT_FIXABLE)]
    NotFixable,
    /// A located path is neither a matching configuration nor an executable.
    #[error("{}", words::located_path_invalid(.0))]
    LocatedPathInvalid(PathBuf),
    /// Ghostlight's own located-path state is malformed.
    #[error("{}", words::location_state_malformed(.0))]
    MalformedLocationState(String),
    /// The change would reach only the private view of the app container named here.
    #[error("{}", words::client_kept_private(.0))]
    PrivateView(String),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::{json, Value};

    use crate::agent::Symbol;
    use crate::install::record::assert_owner_only;
    use crate::scratch::Scratch;

    use jsonc_parser::parse_to_serde_value;

    use super::{
        agent_called, client_ids, command_registration_state, definition, definitions, edit_json,
        edit_toml, executable_name, expected_json_entry, fix_toml, harness_roots,
        holds_owned_entry, inspect, inspect_json, inspect_toml, json_entry_args,
        json_entry_command, json_entry_defaults, jsonc_options, manual_json_setup,
        mcp_command_line, packaged, remove_stray_temporaries_in, replace_with_backup, AppContainer,
        Attention, ClientName, ForeignCommand, HarnessAction, HarnessContext, HarnessError,
        HarnessRegistry, HarnessRoots, HarnessState, HarnessSummary, JsonDialect, LauncherPin,
        NpmLauncher, Packages, PrivateView, RegistrationState, CLINE_SETTINGS, EDGE_COMMAND_NAME,
        HARNESS_LOCATIONS_FILE, MCP_ARGUMENT, RETIRED_MCP_CONNECTOR_NAME, ROSTER, SERVER_NAME,
    };

    /// The name of the roster's agent whose client calls itself `reported`.
    fn named(reported: &str) -> Option<String> {
        agent_called(reported).map(|agent| agent.name)
    }

    fn temporary(name: &str) -> Scratch {
        Scratch::named(&format!("harness-{name}"))
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
            crush_config: None,
            edge: edge(directory),
            path_entries: Vec::new(),
            locations: directory.join("config/ghostlight/harness-locations.json"),
            windows: false,
            linux: cfg!(target_os = "linux"),
            local_app_data: None,
            private_view: None,
        }
    }

    /// The names in `directory`, sorted.
    fn names_in(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_configuration_write_leaves_no_temporary_when_it_fails() {
        let directory = temporary("write-fails");
        // A directory where the configuration should be: the backup copy fails, after the
        // temporary was written.
        let path = directory.join("settings.json");
        fs::create_dir_all(&path).unwrap();

        assert!(replace_with_backup(&path, b"{}").is_err());
        assert_eq!(names_in(&directory), ["settings.json"]);

        let written = directory.join("other.json");
        replace_with_backup(&written, b"{}").unwrap();
        assert_eq!(names_in(&directory), ["other.json", "settings.json"]);
    }

    #[test]
    fn an_install_removes_the_temporaries_an_interrupted_write_left_beside_a_configuration() {
        use std::time::{Duration, SystemTime};

        let directory = temporary("stray-temporaries");
        let context = context(&directory);
        let claude = definition(&context, "claude-code").unwrap();
        let settings = claude.path.parent().unwrap().to_path_buf();
        fs::create_dir_all(&settings).unwrap();
        let stray = ".ghostlight-0123456789abcdef0123456789abcdef.tmp";
        fs::write(settings.join(stray), b"{}").unwrap();
        fs::write(settings.join("notes.tmp"), b"theirs").unwrap();

        // Just written, it may be a write in flight.
        assert_eq!(remove_stray_temporaries_in(&settings, SystemTime::now()), 0);
        assert!(settings.join(stray).is_file());

        let later = SystemTime::now() + Duration::from_secs(3600);
        assert_eq!(remove_stray_temporaries_in(&settings, later), 1);
        assert!(!settings.join(stray).exists());
        assert!(settings.join("notes.tmp").is_file());

        // The registry looks beside each configuration it can change.
        fs::write(settings.join(stray), b"{}").unwrap();
        let registry = HarnessRegistry::with_context(context);
        assert_eq!(registry.remove_stray_temporaries(), 0);
        assert_eq!(registry.remove_stray_temporaries_at(later), 1);
        assert!(!settings.join(stray).exists());
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

        // replace_with_backup writes through a symlink: the link stays a link to the same file.
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
            crush_config: None,
            edge,
            path_entries: vec![binary_directory],
            locations: directory.join("locations.json"),
            windows: false,
            linux: cfg!(target_os = "linux"),
            private_view: None,
            local_app_data: None,
        });

        let zed = registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "zed")
            .unwrap();
        assert_eq!(zed.state, HarnessState::Available);
        assert!(zed.can_install);
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
    }

    #[test]
    fn every_agent_in_the_roster_is_known_by_its_id_its_name_and_its_aliases() {
        let directory = temporary("agent-names-roster");
        let rows = definitions(&context(&directory));
        for row in &rows {
            assert_eq!(named(row.id).as_deref(), Some(row.name), "{}", row.id);
            assert_eq!(named(row.name).as_deref(), Some(row.name), "{}", row.id);
        }
        for row in ROSTER {
            for alias in row.aliases {
                assert_eq!(named(alias).as_deref(), Some(row.name), "{alias}");
                let others = ROSTER.iter().filter(|other| other.name != row.name);
                for other in others {
                    assert!(!other.is_called(alias), "{alias} also calls {}", other.id);
                }
            }
        }
    }

    #[test]
    fn a_product_id_two_agents_share_names_only_the_agent_whose_id_it_is() {
        let devin = ROSTER.iter().find(|row| row.id == "devin").unwrap();
        assert_eq!(devin.product_id, "windsurf");
        assert_eq!(named("windsurf").as_deref(), Some("Windsurf"));
        assert_eq!(named("devin").as_deref(), Some("Devin"));
        assert_eq!(named("Devin").as_deref(), Some("Devin"));
    }

    #[test]
    fn every_agent_has_one_catalog_id_on_every_host_it_runs_on() {
        for row in ROSTER {
            for other in ROSTER {
                assert_eq!(
                    row.agent_id == other.agent_id,
                    row.name == other.name,
                    "{} and {}",
                    row.id,
                    other.id
                );
                if row.agent_id == other.agent_id {
                    assert_eq!(row.monogram, other.monogram, "{}", row.agent_id);
                }
            }
        }
        let cline: Vec<&str> = ROSTER
            .iter()
            .filter(|row| row.agent_id == "cline")
            .map(|row| row.id)
            .collect();
        assert_eq!(
            cline,
            [
                "cline-cli",
                "cline-vscode",
                "cline-cursor",
                "cline-windsurf"
            ]
        );
    }

    #[test]
    fn an_agent_runs_inside_only_the_catalog_apps_its_hosts_name() {
        for row in ROSTER {
            for host in row.runs_inside {
                assert!(super::catalog_agent(host).is_some(), "{}: {host}", row.id);
                assert_ne!(*host, row.agent_id, "{}", row.id);
            }
        }
        assert!(super::runs_inside("claude-code", "claude-desktop"));
        for editor in ["vscode", "cursor", "windsurf"] {
            assert!(super::runs_inside("cline", editor), "{editor}");
        }
        assert!(!super::runs_inside("cline", "claude-desktop"));
        assert!(!super::runs_inside("claude-desktop", "claude-code"));
        assert!(!super::runs_inside("not-an-agent", "claude-desktop"));
    }

    #[test]
    fn every_client_id_names_its_agents_hosts_or_its_one_host() {
        let directory = temporary("client-ids");
        let summaries = HarnessRegistry::with_context(context(&directory)).summaries();
        let ids = client_ids();
        let names = |id: &str| -> Vec<(String, ClientName)> {
            summaries
                .iter()
                .filter_map(|summary| Some((summary.id.clone(), summary.named_by(id)?)))
                .collect()
        };
        assert_eq!(
            ids.hosts,
            [
                "cline-cli",
                "cline-vscode",
                "cline-cursor",
                "cline-windsurf"
            ]
        );
        assert_eq!(ids.all().count(), ids.agents.len() + ids.hosts.len());
        for agent in &ids.agents {
            let hosts: Vec<(String, ClientName)> = ROSTER
                .iter()
                .filter(|row| row.agent_id == *agent)
                .map(|row| {
                    let name = if row.id == *agent {
                        ClientName::Host
                    } else {
                        ClientName::Agent
                    };
                    (row.id.to_owned(), name)
                })
                .collect();
            assert_eq!(names(agent), hosts, "{agent}");
            assert_eq!(names(&agent.to_ascii_uppercase()), hosts, "{agent}");
            assert_eq!(
                ids.agents.iter().filter(|other| *other == agent).count(),
                1,
                "{agent}"
            );
        }
        // Every harness id still names exactly its own host, whether or not an agent id names it.
        for row in ROSTER {
            assert_eq!(
                names(row.id),
                [(row.id.to_owned(), ClientName::Host)],
                "{}",
                row.id
            );
        }
        assert_eq!(names("cline").len(), 4);
        assert!(names("Cline (Cursor)").is_empty());
    }

    #[test]
    fn an_agent_id_selects_only_the_hosts_found_unless_every_client_is_asked_for() {
        let directory = temporary("client-ids-detected");
        let selected = |id: &str, all_clients: bool| -> Vec<String> {
            HarnessRegistry::with_context(context(&directory))
                .summaries()
                .into_iter()
                .filter(|summary| summary.is_selected_by(id, all_clients))
                .map(|summary| summary.id)
                .collect()
        };
        assert!(selected("cline", false).is_empty());
        assert_eq!(selected("cline-vscode", false), ["cline-vscode"]);
        assert_eq!(selected("zed", false), ["zed"]);
        let cli = directory.join("home/.cline/data").join(CLINE_SETTINGS);
        fs::create_dir_all(cli.parent().unwrap()).unwrap();
        fs::write(&cli, "{}").unwrap();
        assert_eq!(selected("cline", false), ["cline-cli"]);
        assert_eq!(
            selected("cline", true),
            [
                "cline-cli",
                "cline-vscode",
                "cline-cursor",
                "cline-windsurf"
            ]
        );
    }

    #[test]
    fn the_reserved_entries_are_entries_no_row_takes() {
        use crate::agent::{COMMAND_LINE_ID, EARLIER_ID, UNKNOWN_ID, UNKNOWN_NAME_SEPARATOR};
        use crate::language::provenance::{COMMAND_LINE_AGENT, EARLIER_SESSION, UNKNOWN_AGENT};

        for row in ROSTER {
            for id in [row.id, row.agent_id] {
                assert!(
                    ![COMMAND_LINE_ID, UNKNOWN_ID, EARLIER_ID].contains(&id),
                    "{id}"
                );
                assert!(!id.contains(UNKNOWN_NAME_SEPARATOR), "{id}");
            }
            for reserved in [
                COMMAND_LINE_ID,
                UNKNOWN_ID,
                COMMAND_LINE_AGENT,
                UNKNOWN_AGENT,
                EARLIER_ID,
                EARLIER_SESSION,
            ] {
                assert!(!row.is_called(reserved), "{} is called {reserved}", row.id);
            }
        }
    }

    #[test]
    fn an_agent_is_known_by_the_names_it_reports_in_any_case() {
        assert_eq!(named("codex-mcp-client").as_deref(), Some("Codex"));
        assert_eq!(named("claude-code").as_deref(), Some("Claude Code"));
        assert_eq!(named("CLAUDE-CODE").as_deref(), Some("Claude Code"));
        assert_eq!(named("  claude code ").as_deref(), Some("Claude Code"));
        assert_eq!(named("claude-ai").as_deref(), Some("Claude Desktop"));
        assert_eq!(named("cursor-vscode").as_deref(), Some("Cursor"));
        assert_eq!(
            named("Visual Studio Code").as_deref(),
            Some("Visual Studio Code")
        );
        assert_eq!(named("cline").as_deref(), Some("Cline"));
        assert_eq!(named("Antigravity").as_deref(), Some("Antigravity"));
        assert_eq!(named("my own agent").as_deref(), None);
        assert_eq!(named("").as_deref(), None);
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
        let registry = HarnessRegistry::with_context(context);
        for summary in registry.summaries() {
            let served = crate::agent::image::served(&summary.agent.id, None)
                .unwrap_or_else(|| panic!("missing packaged artwork for {}", summary.id));
            assert_eq!(served.media_type, "image/svg+xml", "{}", summary.id);
            assert_eq!(
                summary.agent.symbol,
                Symbol::Icon {
                    address: format!("agent-icons/{}?v={}", summary.agent.id, served.version),
                    about: None,
                },
                "every row names its agent's one image address: {}",
                summary.id
            );
            assert_eq!(
                summary.agent.name, summary.name,
                "every row's agent is its product"
            );
            assert_eq!(
                agent_called(&summary.id).as_ref(),
                Some(&summary.agent),
                "every harness row is in the catalog: {}",
                summary.id
            );
            assert_eq!(
                agent_called(&summary.agent.id).as_ref(),
                Some(&summary.agent),
                "every agent id names its agent: {}",
                summary.id
            );
            assert_eq!(summary.connector_command, format!("{edge} mcp"));
            assert_eq!(
                summary.copy_only,
                matches!(summary.id.as_str(), "goose" | "continue"),
                "only the clients whose files Ghostlight never edits are copy-only: {}",
                summary.id
            );
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
    }

    #[test]
    fn every_download_destination_is_closed_and_https() {
        let directory = temporary("download-destinations");
        let context = context(&directory);
        let registry = HarnessRegistry::with_context(context.clone());
        for summary in registry.summaries() {
            let row = ROSTER.iter().find(|row| row.id == summary.id).unwrap();
            assert_eq!(summary.download_url.as_deref(), row.download_url);
            assert_eq!(summary.can_download, row.download_url.is_some());
            match row.download_url {
                Some(url) => {
                    assert!(url.starts_with("https://"), "{}", row.id);
                    assert_eq!(registry.download_url(row.id).unwrap(), url);
                }
                None => assert!(matches!(
                    registry.download_url(row.id),
                    Err(HarnessError::NoDownload(_))
                )),
            }
        }
        assert!(matches!(
            registry.download_url("not-a-harness"),
            Err(HarnessError::UnknownHarness(_))
        ));
    }

    #[test]
    fn harness_locations_are_owner_only() {
        let directory = temporary("locations-private");
        let context = context(&directory);
        let registry = HarnessRegistry::with_context(context.clone());
        let located = directory.join("portable/kilo.jsonc");
        fs::create_dir_all(located.parent().unwrap()).unwrap();
        fs::write(&located, "{}\n").unwrap();
        registry.locate("kilo-code", &located).unwrap();
        assert_owner_only(&context.locations);
        registry.locate("kilo-code", &located).unwrap();
        assert_owner_only(&context.locations);
        assert_eq!(
            names_in(context.locations.parent().unwrap()),
            [HARNESS_LOCATIONS_FILE]
        );
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
    }

    #[test]
    fn a_host_reads_the_first_of_its_candidates_that_exists_and_otherwise_its_default() {
        for row in ROSTER.iter().filter(|row| !row.candidates.is_empty()) {
            let directory = temporary(&format!("candidates-{}", row.id));
            let context = context(&directory);
            let default = (row.path)(&context);
            let file_on = |context: &HarnessContext| {
                row.copies_on(context, &Packages::none())[0].file.clone()
            };
            assert_eq!(file_on(&context), default, "{}", row.id);
            fs::create_dir_all(default.parent().unwrap()).unwrap();
            // Each candidate, written after every later one, wins over all of them.
            for (index, name) in row.candidates.iter().enumerate().rev() {
                fs::write(default.with_file_name(name), "{}\n").unwrap();
                assert_eq!(
                    file_on(&context),
                    default.with_file_name(row.candidates[index]),
                    "{}",
                    row.id
                );
            }
            let definition = definition(&context, row.id).unwrap();
            for name in row.candidates {
                assert!(
                    definition.accepts_config_path(&directory.join(name)),
                    "{name}"
                );
            }
            assert!(!definition.accepts_config_path(&directory.join("settings.json")));
        }
    }

    #[test]
    fn crush_reads_the_file_its_variable_names_from_the_context() {
        let directory = temporary("crush-variable");
        let named = directory.join("elsewhere/crush.json");
        let context = HarnessContext {
            crush_config: Some(named.clone()),
            ..context(&directory)
        };
        assert_eq!(definition(&context, "crush").unwrap().path, named);
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
    }

    #[cfg(unix)]
    #[test]
    fn json_edits_preserve_the_files_permission_bits() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temporary("json-mode");
        let path = directory.join("config.json");
        let context = context(&directory);
        fs::write(&path, "{\"theme\":\"dark\"}\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(edit_json(&context, &path, JsonDialect::Copilot, true).unwrap());
        let installed = fs::read(&path).unwrap();
        assert!(!edit_json(&context, &path, JsonDialect::Copilot, true).unwrap());
        assert_eq!(fs::read(&path).unwrap(), installed);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert!(edit_json(&context, &path, JsonDialect::Copilot, false).unwrap());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
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
    }

    #[test]
    fn a_background_scan_never_overwrites_an_acts_result() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let directory = temporary("background-scan");
        let context = context(&directory);
        fs::create_dir_all(context.home.join(".cline/data/settings")).unwrap();
        let registry = HarnessRegistry::with_context(context.clone());
        let state = |registry: &HarnessRegistry| {
            registry
                .summaries()
                .into_iter()
                .find(|summary| summary.id == "cline-cli")
                .unwrap()
                .state
        };
        assert_eq!(state(&registry), HarnessState::Available);

        // An act is under way: it holds the action lock while it writes and scans.
        let act = super::lock(&registry.inner.action);
        let (finished, scanned) = mpsc::channel();
        let background = registry.clone();
        let scan = thread::spawn(move || {
            background.refresh().unwrap();
            finished.send(()).unwrap();
        });
        assert!(
            scanned.recv_timeout(Duration::from_millis(200)).is_err(),
            "a background scan waits for the act in progress"
        );
        super::apply_install(&context, &definition(&context, "cline-cli").unwrap()).unwrap();
        registry.scan();
        drop(act);

        scanned.recv().unwrap();
        scan.join().unwrap();
        assert_eq!(state(&registry), HarnessState::Installed);
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
            assert!(
                summary.copy_only,
                "{id} is connected only by pasting its setup"
            );
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
            assert!(summary.copy_only, "{id}");
        }
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
    }

    /// Windows reads `/` as `\` and ignores case, so a 1.x relay entry a client wrote with
    /// forward slashes is still Ghostlight's own, and Uninstall and Fix find it.
    #[cfg(windows)]
    #[test]
    fn a_legacy_relay_entry_spelled_with_forward_slashes_is_owned() {
        let directory = temporary("legacy-relay-slashes");
        let install_root = directory.join("bin");
        let current = install_root
            .join("v1")
            .join(executable_name(EDGE_COMMAND_NAME, true));
        let legacy = install_root.join("v0").join("ghostlight-relay.exe");
        let respelled = legacy.to_string_lossy().replace('\\', "/").to_uppercase();
        assert_eq!(
            command_registration_state(&respelled, Some(&["--role", "agent"]), &current, true),
            RegistrationState::Updatable
        );
    }

    #[test]
    fn json_install_is_idempotent_and_preserves_siblings() {
        let directory = temporary("json");
        let path = directory.join("mcp.json");
        let context = context(&directory);
        let edge = context.edge.clone();
        fs::write(
            &path,
            r#"{"other":7,"mcpServers":{"sibling":{"command":"sibling"}}}"#,
        )
        .unwrap();
        assert!(edit_json(&context, &path, JsonDialect::McpServers, true).unwrap());
        assert!(!edit_json(&context, &path, JsonDialect::McpServers, true).unwrap());
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
        assert!(edit_json(&context, &path, JsonDialect::McpServers, false).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(value["mcpServers"].get("ghostlight").is_none());
    }

    #[test]
    fn jsonc_install_and_uninstall_preserve_comments_and_trailing_commas() {
        let directory = temporary("jsonc");
        let path = directory.join("settings.json");
        let context = context(&directory);
        let edge = context.edge.clone();
        let source = "{\n  // keep this thought\n  \"context_servers\": {},\n}\n";
        fs::write(&path, source).unwrap();
        assert!(edit_json(&context, &path, JsonDialect::ContextServers, true).unwrap());
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
        // args, and env. ADR-0071's amendment records the verification.
        let written: Value = parse_to_serde_value(&installed, &jsonc_options())
            .unwrap()
            .unwrap();
        let entry = &written["context_servers"]["ghostlight"];
        assert!(entry.get("source").is_none(), "{entry}");
        assert!(entry["command"].is_string());
        assert!(entry["args"].is_array());
        assert!(entry["env"].is_object());
        assert!(edit_json(&context, &path, JsonDialect::ContextServers, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("// keep this thought"));
        assert!(!removed.contains("\"ghostlight\""));
        assert!(removed.contains("\"context_servers\": {},"));
    }

    #[test]
    fn zcode_nested_install_and_uninstall_preserve_sibling_servers() {
        let directory = temporary("zcode");
        let path = directory.join("config.json");
        let context = context(&directory);
        let edge = context.edge.clone();
        let source = "{\n  \"mcp\": {\n    \"servers\": {\n      \"other\": {\"command\": \"keep\"}\n    }\n  }\n}\n";
        fs::write(&path, source).unwrap();
        assert!(edit_json(&context, &path, JsonDialect::ZCode, true).unwrap());
        assert!(!edit_json(&context, &path, JsonDialect::ZCode, true).unwrap());
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
        assert!(edit_json(&context, &path, JsonDialect::ZCode, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("\"other\""));
        assert!(!removed.contains("\"ghostlight\""));
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
    }

    #[test]
    fn a_hand_registered_orchestrator_command_stays_foreign() {
        // A hand-written entry that runs the service image, which has no MCP stdio mode, is
        // foreign evidence (ADR-0141): install and remove never overwrite it; only the
        // confirmed Fix may.
        let directory = temporary("zcode-foreign-orchestrator");
        let path = directory.join("config.json");
        let context = context(&directory);
        let edge = context.edge.clone();
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
                edit_json(&context, &path, JsonDialect::ZCode, install),
                Err(HarnessError::ForeignEntry)
            ));
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        let state = inspect_json(&source, JsonDialect::ZCode, &edge, cfg!(windows)).unwrap();
        assert!(
            matches!(state, RegistrationState::Foreign(Some(_))),
            "{state:?}"
        );
    }

    #[test]
    fn foreign_entries_are_never_overwritten_or_removed() {
        let directory = temporary("foreign");
        let path = directory.join("mcp.json");
        let context = context(&directory);
        let source = r#"{"mcpServers":{"ghostlight":{"command":"some-other-tool"}}}"#;
        fs::write(&path, source).unwrap();
        for install in [true, false] {
            assert!(matches!(
                edit_json(&context, &path, JsonDialect::McpServers, install),
                Err(HarnessError::ForeignEntry)
            ));
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
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
    "ghostlight": {"command": "other-server", "args": ["--port", "9"]}
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
    }

    /// A registry over `context` whose Cline CLI settings hold `entry` under Ghostlight's key, and
    /// the path of that file.
    fn cline_with_entry(context: &HarnessContext, entry: &Value) -> (HarnessRegistry, PathBuf) {
        let path = context
            .home
            .join(".cline/data/settings/cline_mcp_settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({"mcpServers": {"ghostlight": entry}})).unwrap(),
        )
        .unwrap();
        (HarnessRegistry::with_context(context.clone()), path)
    }

    fn cline_summary(registry: &HarnessRegistry) -> HarnessSummary {
        registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "cline-cli")
            .expect("Cline CLI stays in the closed roster")
    }

    #[test]
    fn an_unpinned_npx_ghostlight_entry_says_npm_chooses_the_version() {
        let directory = temporary("launcher-unpinned");
        let context = context(&directory);
        for entry in [
            json!({"command": "npx", "args": ["-y", "ghostlight"]}),
            json!({"command": "npx", "args": ["--yes", "ghostlight@latest", "mcp"]}),
            json!({"command": "cmd", "args": ["/c", "npx", "-y", "ghostlight"]}),
            json!({"command": "npx -y ghostlight"}),
        ] {
            let (registry, path) = cline_with_entry(&context, &entry);
            let before = fs::read(&path).unwrap();
            let summary = cline_summary(&registry);
            assert_eq!(summary.state, HarnessState::NeedsAttention, "{entry}");
            assert_eq!(
                summary.attention,
                Some(Attention::Launcher(LauncherPin::Unpinned)),
                "{entry}"
            );
            assert!(
                summary
                    .detail
                    .contains("npm chooses the version at each launch"),
                "{}",
                summary.detail
            );
            assert!(!summary.detail.contains("foreign"), "{}", summary.detail);
            let evidence = summary.evidence.clone().expect("the launcher is named");
            assert!(evidence.contains("npm launcher"), "{evidence}");
            assert!(!evidence.contains("another program"), "{evidence}");
            assert!(summary.can_fix && !summary.can_install && !summary.can_uninstall);
            assert!(matches!(
                registry.apply("cline-cli", HarnessAction::Install),
                Err(HarnessError::LauncherEntry)
            ));
            assert_eq!(fs::read(&path).unwrap(), before);
        }
    }

    #[test]
    fn a_launcher_pinned_to_1x_needs_attention() {
        let directory = temporary("launcher-pinned-1x");
        let context = context(&directory);
        let (registry, path) = cline_with_entry(
            &context,
            &json!({"command": "npx", "args": ["-y", "ghostlight@1.3.5"]}),
        );
        let before = fs::read(&path).unwrap();
        let summary = cline_summary(&registry);
        assert_eq!(summary.state, HarnessState::NeedsAttention);
        assert_eq!(
            summary.attention,
            Some(Attention::Launcher(LauncherPin::Earlier))
        );
        assert!(
            summary.detail.contains("pinned to ghostlight@1.3.5")
                && summary.detail.contains("before 2.0"),
            "{}",
            summary.detail
        );
        assert!(summary
            .evidence
            .as_deref()
            .is_some_and(|evidence| evidence.contains("`npx ghostlight@1.3.5`")));
        for action in [HarnessAction::Install, HarnessAction::Uninstall] {
            assert!(matches!(
                registry.apply("cline-cli", action),
                Err(HarnessError::LauncherEntry)
            ));
        }
        assert_eq!(fs::read(&path).unwrap(), before);

        let fixed = registry.apply("cline-cli", HarnessAction::Fix).unwrap();
        assert!(fixed.changed);
        assert_eq!(fixed.summary.state, HarnessState::Installed);
        assert!(
            fixed.message.contains("instead of the npm launcher"),
            "{}",
            fixed.message
        );
    }

    #[test]
    fn a_launcher_pinned_to_2x_is_never_current() {
        let directory = temporary("launcher-pinned-2x");
        let context = context(&directory);
        let (registry, _) = cline_with_entry(
            &context,
            &json!({"command": "npx", "args": ["-y", "ghostlight@2.0.0"]}),
        );
        let summary = cline_summary(&registry);
        assert_eq!(summary.state, HarnessState::NeedsAttention);
        assert_eq!(
            summary.attention,
            Some(Attention::Launcher(LauncherPin::Pinned))
        );
        assert!(
            summary.detail.contains("cannot say it is current"),
            "{}",
            summary.detail
        );
    }

    #[test]
    fn only_ghostlights_own_package_through_npx_is_the_launcher() {
        let launcher = |command: &str, args: &[&str]| NpmLauncher::from_entry(command, Some(args));
        for (command, args) in [
            ("npx", &["-y", "ghostlight", "install"][..]),
            ("npx", &["-y", "@scope/server-x"][..]),
            ("npx", &["-y", "ghostlight-other"][..]),
            ("npx", &["-y", "ghostlight@https://example.test/x.tgz"][..]),
            ("npx", &["--registry", "r", "ghostlight"][..]),
            ("cmd", &["/k", "npx", "ghostlight"][..]),
            ("node", &["ghostlight"][..]),
            ("npx", &[][..]),
        ] {
            assert_eq!(launcher(command, args), None, "{command} {args:?}");
        }
        assert_eq!(
            launcher("npx.cmd", &["-y", "ghostlight@^1.2"]),
            Some(NpmLauncher {
                pin: LauncherPin::Earlier,
                package: "ghostlight@^1.2".into(),
            })
        );
        for (spec, pin) in [
            (None, LauncherPin::Unpinned),
            (Some(""), LauncherPin::Unpinned),
            (Some("latest"), LauncherPin::Unpinned),
            (Some("next"), LauncherPin::Unpinned),
            (Some("1.3.5"), LauncherPin::Earlier),
            (Some("~1"), LauncherPin::Earlier),
            (Some("0.9.0"), LauncherPin::Earlier),
            (Some("2.0.0"), LauncherPin::Pinned),
            (Some("2.0.0-rc.1"), LauncherPin::Pinned),
            (Some("^2"), LauncherPin::Pinned),
        ] {
            assert_eq!(LauncherPin::of(spec), pin, "{spec:?}");
        }
    }

    #[test]
    fn a_codex_launcher_entry_is_the_launcher_too() {
        let directory = temporary("launcher-codex");
        let context = context(&directory);
        let source =
            "[mcp_servers.ghostlight]\ncommand = \"npx\"\nargs = [\"-y\", \"ghostlight@1.3.5\"]\n";
        assert!(matches!(
            inspect_toml(source, &context).unwrap(),
            RegistrationState::Launcher(NpmLauncher {
                pin: LauncherPin::Earlier,
                ..
            })
        ));
        let path = directory.join("config.toml");
        fs::write(&path, source).unwrap();
        assert!(matches!(
            edit_toml(&context, &path, true),
            Err(HarnessError::LauncherEntry)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }

    #[test]
    fn explicit_fix_is_lossless_for_the_toml_dialect() {
        let directory = temporary("fix-foreign-toml");
        let context = context(&directory);

        let toml = directory.join("config.toml");
        let toml_source = "title = \"keep\"\n\n[mcp_servers.sibling]\ncommand = \"keep-me\"\n\n[mcp_servers.ghostlight]\ncommand = \"alien\"\nargs = [\"serve\"]\n";
        fs::write(&toml, toml_source).unwrap();
        assert!(fix_toml(&context, &toml).unwrap());
        let fixed_toml = fs::read_to_string(&toml).unwrap();
        assert!(fixed_toml.contains("title = \"keep\""));
        assert!(fixed_toml.contains("[mcp_servers.sibling]"));
        assert_eq!(
            inspect_toml(&fixed_toml, &context).unwrap(),
            RegistrationState::Current
        );
        assert_eq!(
            fs::read_to_string(toml.with_extension("toml.ghostlight-backup")).unwrap(),
            toml_source
        );
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
    }

    #[test]
    fn every_blocked_dialect_names_the_found_program() {
        let directory = temporary("dialect-evidence");
        let context = context(&directory);

        let toml = directory.join("codex.toml");
        std::fs::write(
            &toml,
            "[mcp_servers.ghostlight]\ncommand = \"alien-tool\"\nargs = [\"serve\"]\n",
        )
        .unwrap();
        let toml_source = fs::read_to_string(&toml).unwrap();
        assert!(matches!(
            inspect_toml(&toml_source, &context),
            Ok(RegistrationState::Foreign(Some(ForeignCommand { program, arguments: 1 })))
                if program == "alien-tool"
        ));
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
    }

    /// Every JSON dialect Ghostlight writes.
    const JSON_DIALECTS: [JsonDialect; 8] = [
        JsonDialect::McpServers,
        JsonDialect::Copilot,
        JsonDialect::Servers,
        JsonDialect::ContextServers,
        JsonDialect::Crush,
        JsonDialect::OpenCodeV1,
        JsonDialect::OpenCodeV2,
        JsonDialect::ZCode,
    ];

    /// A file in `dialect` that holds `entry` under Ghostlight's key.
    fn json_file_holding(dialect: JsonDialect, entry: Value) -> String {
        let file = match dialect {
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
        serde_json::to_string_pretty(&file).unwrap()
    }

    #[test]
    fn the_dry_run_and_the_install_agree_on_a_current_entry() {
        let directory = temporary("current-agrees");
        let context = context(&directory);
        let edge = context.edge.clone();
        // The edge spelled another way, with none of the defaults and a key of the client's own:
        // what a dry run reads as current, an install leaves byte for byte.
        let respelled = edge
            .parent()
            .unwrap()
            .join(".")
            .join(edge.file_name().unwrap());
        for dialect in JSON_DIALECTS {
            let mut entry = expected_json_entry(&respelled, dialect);
            let members = entry.as_object_mut().unwrap();
            for (key, _) in json_entry_defaults(dialect) {
                members.remove(key);
            }
            members.insert("disabled".into(), json!(true));
            let source = json_file_holding(dialect, entry);
            let path = directory.join(format!("{dialect:?}.json"));
            fs::write(&path, &source).unwrap();
            assert_eq!(
                inspect_json(&source, dialect, &edge, context.windows).unwrap(),
                RegistrationState::Current,
                "{dialect:?}"
            );
            assert!(
                !edit_json(&context, &path, dialect, true).unwrap(),
                "{dialect:?}"
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), source, "{dialect:?}");
        }
        let forwarded = if context.linux {
            format!("env_vars = {:?}\n", super::CODEX_DESKTOP_ENVIRONMENT)
        } else {
            String::new()
        };
        let source = format!(
            "[mcp_servers.ghostlight]\ncommand = {:?}\nargs = {:?}\nenabled = false\n{forwarded}",
            respelled.to_string_lossy(),
            super::MCP_ARGUMENTS
        );
        let path = directory.join("config.toml");
        fs::write(&path, &source).unwrap();
        assert_eq!(
            inspect_toml(&source, &context).unwrap(),
            RegistrationState::Current
        );
        assert!(!edit_toml(&context, &path, true).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }

    #[test]
    fn a_dialect_that_needs_a_type_is_updatable_without_it_and_the_update_keeps_the_rest() {
        let directory = temporary("type-owned");
        let context = context(&directory);
        let edge = context.edge.clone();
        for dialect in JSON_DIALECTS {
            let Some(kind) = super::json_entry_type(dialect) else {
                continue;
            };
            let mut entry = expected_json_entry(&edge, dialect);
            let members = entry.as_object_mut().unwrap();
            members.remove(super::TYPE_KEY);
            members.insert("timeout".into(), json!(5000));
            let source = json_file_holding(dialect, entry);
            let path = directory.join(format!("{dialect:?}.json"));
            fs::write(&path, &source).unwrap();
            assert_eq!(
                inspect_json(&source, dialect, &edge, context.windows).unwrap(),
                RegistrationState::Updatable,
                "{dialect:?}"
            );
            assert!(edit_json(&context, &path, dialect, true).unwrap());
            let written = fs::read_to_string(&path).unwrap();
            let parsed: Value = serde_json::from_str(&written).unwrap();
            let entry = super::json_entry(&parsed, dialect).unwrap();
            assert_eq!(entry[super::TYPE_KEY], kind, "{dialect:?}");
            assert_eq!(entry["timeout"], 5000, "{dialect:?}");
            assert_eq!(
                inspect_json(&written, dialect, &edge, context.windows).unwrap(),
                RegistrationState::Current,
                "{dialect:?}"
            );
        }
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
    }

    #[test]
    fn the_command_line_is_the_edge_and_its_argument() {
        let directory = temporary("copy-command");
        let registry = HarnessRegistry::with_context(context(&directory));
        let path = edge(&directory);
        assert_eq!(
            registry.summaries()[0].connector_command,
            format!("{} mcp", path.display())
        );
        assert_eq!(
            mcp_command_line(Path::new("/with space/ghostlight")),
            "\"/with space/ghostlight\" mcp"
        );
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
    }

    #[test]
    fn codex_toml_edit_preserves_unrelated_comments() {
        let directory = temporary("toml");
        let path = directory.join("config.toml");
        let context = context(&directory);
        fs::write(&path, "# keep me\nmodel = \"gpt-test\"\n").unwrap();
        assert!(edit_toml(&context, &path, true).unwrap());
        assert!(!edit_toml(&context, &path, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        assert!(installed.contains("# keep me"));
        assert_eq!(
            inspect_toml(&installed, &context).unwrap(),
            RegistrationState::Current
        );
        assert!(edit_toml(&context, &path, false).unwrap());
        let removed = fs::read_to_string(&path).unwrap();
        assert!(removed.contains("# keep me"));
        assert!(!removed.contains("mcp_servers"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn codex_owned_registration_gains_desktop_forwarding_without_losing_custom_fields() {
        let directory = temporary("codex-desktop");
        let path = directory.join("config.toml");
        let context = context(&directory);
        let edge = context.edge.clone();
        // The current registration lacking the Linux desktop forwarding: updatable for that
        // alone, so the edit adds the forwarding and keeps every custom field.
        let source = format!(
            "# keep me\n[mcp_servers.ghostlight]\ncommand = {:?}\nargs = {:?}\nstartup_timeout_sec = 45\nenv_vars = [\"CUSTOM_SETTING\", {{ name = \"DISPLAY\", source = \"local\" }}, {{ name = \"WAYLAND_DISPLAY\" }}, {{ name = \"CUSTOM_REMOTE\", source = \"remote\" }}]\n[mcp_servers.ghostlight.env]\nXAUTHORITY = \"/explicit/test-authority\"\n[mcp_servers.other]\ncommand = \"other-server\"\n",
            edge.to_string_lossy(),
            super::MCP_ARGUMENTS
        );
        fs::write(&path, &source).unwrap();
        assert_eq!(
            inspect_toml(&source, &context).unwrap(),
            RegistrationState::Updatable
        );
        assert!(edit_toml(&context, &path, true).unwrap());
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
        assert!(
            super::missing_codex_desktop_environment(entry, context.linux)
                .unwrap()
                .is_empty()
        );
        assert!(!edit_toml(&context, &path, true).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), installed);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn codex_malformed_environment_is_preserved_and_windows_needs_no_desktop_forwarding() {
        let directory = temporary("codex-malformed-env");
        let path = directory.join("config.toml");
        let context = context(&directory);
        let edge = context.edge.clone();
        let source = format!(
            "[mcp_servers.ghostlight]\ncommand = {:?}\nargs = {:?}\nenv_vars = \"not-an-array\"\n",
            edge.to_string_lossy(),
            super::MCP_ARGUMENTS
        );
        fs::write(&path, &source).unwrap();
        assert!(inspect_toml(&source, &context).is_err());
        assert!(edit_toml(&context, &path, true).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        assert_eq!(
            inspect_toml(
                &source,
                &HarnessContext {
                    windows: true,
                    linux: false,
                    ..context
                }
            )
            .unwrap(),
            RegistrationState::Current
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn codex_malformed_environment_members_are_preserved_before_migration() {
        let directory = temporary("codex-malformed-env-members");
        let path = directory.join("config.toml");
        let context = context(&directory);
        let edge = context.edge.clone();
        // The edge command with its current arguments is the current registration. The retired
        // connector command is a retired one. Both are owned, so both refuse malformed members.
        let arguments = format!("{:?}", super::MCP_ARGUMENTS);
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
                        "# preserve exactly\n[mcp_servers.ghostlight]\ncommand = {:?}\nargs = {arguments}\nenv_vars = [{members}]\n",
                        command.to_string_lossy()
                    ),
                    format!(
                        "# preserve exactly\n[mcp_servers]\nghostlight = {{ command = {:?}, args = {arguments}, env_vars = [{members}] }}\n",
                        command.to_string_lossy()
                    ),
                ] {
                    fs::write(&path, &source).unwrap();
                    assert!(
                        inspect_toml(&source, &context).is_err(),
                        "inspection accepted {source}"
                    );
                    assert!(edit_toml(&context, &path, true).is_err());
                    assert_eq!(fs::read_to_string(&path).unwrap(), source);
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn codex_inline_owned_registration_keeps_custom_fields() {
        let directory = temporary("codex-inline-env");
        let path = directory.join("config.toml");
        let context = context(&directory);
        let edge = context.edge.clone();
        // The edge command with no arguments is an updatable registration on purpose.
        fs::write(&path, format!(
            "[mcp_servers]\nghostlight = {{ command = {:?}, args = [], startup_timeout_sec = 45 }}\n",
            edge.to_string_lossy()
        )).unwrap();
        assert!(edit_toml(&context, &path, true).unwrap());
        let installed = fs::read_to_string(&path).unwrap();
        let document = installed.parse::<toml_edit::DocumentMut>().unwrap();
        assert_eq!(
            document["mcp_servers"]["ghostlight"]["startup_timeout_sec"].as_integer(),
            Some(45)
        );
        assert_eq!(
            inspect_toml(&installed, &context).unwrap(),
            RegistrationState::Current
        );
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
        assert!(edit_toml(&context, &definition.path, true).unwrap());
        assert_eq!(manual, fs::read_to_string(&definition.path).unwrap());
    }

    #[test]
    fn opencode_v1_uses_the_local_command_array_contract() {
        let directory = temporary("opencode-v1");
        let path = directory.join("opencode.json");
        let context = context(&directory);
        let edge = context.edge.clone();
        fs::write(&path, r#"{"theme":"ghost","mcp":{}}"#).unwrap();

        assert!(edit_json(&context, &path, JsonDialect::OpenCodeV1, true).unwrap());
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
    }

    #[test]
    fn opencode_v2_preserves_the_nested_servers_contract() {
        let directory = temporary("opencode-v2");
        let path = directory.join("opencode.json");
        let context = context(&directory);
        fs::write(
            &path,
            r#"{"mcp":{"servers":{"sibling":{"type":"remote"}}}}"#,
        )
        .unwrap();

        assert!(edit_json(&context, &path, JsonDialect::OpenCodeV2, true).unwrap());
        assert!(!edit_json(&context, &path, JsonDialect::OpenCodeV2, true).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["mcp"]["servers"]["sibling"]["type"], "remote");
        assert_eq!(value["mcp"]["servers"]["ghostlight"]["type"], "local");
        assert!(value["mcp"]["servers"]["ghostlight"]
            .get("enabled")
            .is_none());
        assert!(edit_json(&context, &path, JsonDialect::OpenCodeV2, false).unwrap());
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(value["mcp"]["servers"].get("ghostlight").is_none());
        assert_eq!(value["mcp"]["servers"]["sibling"]["type"], "remote");
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
        assert_eq!(claude.state, HarnessState::Available);
        assert!(!claude.can_install && !claude.can_fix && !claude.can_uninstall);
        assert!(
            claude.can_locate,
            "Locate writes only Ghostlight's own state"
        );
        let codex = summaries
            .iter()
            .find(|summary| summary.id == "codex")
            .unwrap();
        assert!(!codex.private_view);
        assert!(codex.can_install);

        // Every act inspection does not offer is refused, with the sentence that says why.
        for action in [
            HarnessAction::Install,
            HarnessAction::Fix,
            HarnessAction::Uninstall,
        ] {
            let refused = registry.apply("claude-desktop", action).unwrap_err();
            assert!(matches!(refused, HarnessError::PrivateView(ref app) if app == "OpenAI.Codex"));
            assert_eq!(refused.to_string(), claude.detail);
        }
        assert_eq!(fs::read(&desktop).unwrap(), before);
        assert!(
            registry
                .apply("codex", HarnessAction::Install)
                .unwrap()
                .changed
        );
        // Setting up everything leaves the private one out rather than failing on it.
        let everything = registry.setup_detected().unwrap();
        assert!(everything.failures.is_empty(), "{:?}", everything.failures);
        assert_eq!(fs::read(&desktop).unwrap(), before);
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
    }

    /// The Store Claude Desktop's package family, as the owner's computer has it.
    const STORE_FAMILY: &str = "Claude_pzs8sxrjxfjjc";

    /// A person's own settings in Claude Desktop's file, which every act keeps byte for byte.
    const PERSON_SETTINGS: &str =
        "{\n  // Kept as written.\n  \"theme\": \"dark\",\n  \"mcpServers\": {\n    \"other\": {\"command\": \"other-server\"}\n  }\n}\n";

    /// A Windows computer below `directory`: `%APPDATA%` is `config` and `%LOCALAPPDATA%` is
    /// `local`, where Store packages keep their private copies.
    fn windows_context(directory: &Path) -> HarnessContext {
        let mut context = context(directory);
        context.windows = true;
        context.local_app_data = Some(directory.join("local"));
        context
    }

    /// Claude Desktop's plain settings file on the computer `context` describes.
    fn plain_desktop(context: &HarnessContext) -> PathBuf {
        context.roaming.join("Claude/claude_desktop_config.json")
    }

    /// The Store edition's own Claude folder on the computer `context` describes, made.
    fn store_folder(context: &HarnessContext) -> PathBuf {
        package_folder(context, STORE_FAMILY)
    }

    /// The Claude folder in the private copy of `%APPDATA%` of package `family`, made.
    fn package_folder(context: &HarnessContext, family: &str) -> PathBuf {
        let folder = packaged::private_root(
            context.local_app_data.as_deref().unwrap(),
            family,
            packaged::DataRoot::Roaming,
        )
        .join("Claude");
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// Claude Desktop's settings holding only Ghostlight's current entry for `context`'s edge.
    fn current_settings(context: &HarnessContext) -> String {
        format!(
            "{{\"mcpServers\":{{\"ghostlight\":{{\"command\":{},\"args\":[\"mcp\"]}}}}}}",
            serde_json::to_string(&context.edge.to_string_lossy()).unwrap()
        )
    }

    fn desktop_summary(registry: &HarnessRegistry) -> HarnessSummary {
        registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "claude-desktop")
            .unwrap()
    }

    fn has_entry(path: &Path) -> bool {
        let text = fs::read_to_string(path).unwrap();
        parse_to_serde_value(&text, &super::jsonc_options())
            .unwrap()
            .expect("the file holds a document")["mcpServers"]
            .get(SERVER_NAME)
            .is_some()
    }

    #[test]
    fn a_fresh_store_install_is_found_by_its_own_folder_and_set_up_there() {
        let directory = temporary("store-fresh");
        let context = windows_context(&directory);
        let plain = plain_desktop(&context);

        // Without the Store edition nothing changes: Claude Desktop is not here and the plain
        // file is where it would look.
        let registry = HarnessRegistry::with_context(context.clone());
        let absent = desktop_summary(&registry);
        assert_eq!(absent.state, HarnessState::NotDetected);
        assert_eq!(PathBuf::from(&absent.config_path), plain);
        assert!(!absent.packaged && absent.other_copy.is_none());

        // The Store edition's own folder holds its other files but no settings file yet.
        let folder = store_folder(&context);
        fs::write(folder.join("config.json"), "{}").unwrap();
        registry.refresh().unwrap();
        let found = desktop_summary(&registry);
        let private = folder.join("claude_desktop_config.json");
        assert_eq!(found.state, HarnessState::Available, "{}", found.detail);
        assert_eq!(PathBuf::from(&found.config_path), private);
        assert!(found.packaged);
        assert!(found.other_copy.is_none());

        let installed = registry
            .apply("claude-desktop", HarnessAction::Install)
            .unwrap();
        assert!(installed.changed);
        assert_eq!(installed.summary.state, HarnessState::Installed);
        assert!(has_entry(&private));
        assert!(!plain.exists(), "the plain path is not written");
        assert!(!plain.parent().unwrap().exists());
    }

    #[test]
    fn the_store_copy_is_set_up_and_a_plain_copy_left_beside_it_is_not_touched() {
        let directory = temporary("store-plain");
        let context = windows_context(&directory);
        let plain = plain_desktop(&context);
        fs::create_dir_all(plain.parent().unwrap()).unwrap();
        let left = current_settings(&context);
        fs::write(&plain, &left).unwrap();
        let private = store_folder(&context).join("claude_desktop_config.json");
        fs::write(&private, PERSON_SETTINGS).unwrap();
        let registry = HarnessRegistry::with_context(context.clone());

        let summary = desktop_summary(&registry);
        assert_eq!(PathBuf::from(&summary.config_path), private);
        assert_eq!(
            summary.state,
            HarnessState::Available,
            "the plain copy's entry is not the one the Store edition reads"
        );
        assert!(summary.packaged);
        assert_eq!(
            summary.other_copy.as_deref().map(PathBuf::from),
            Some(plain.clone())
        );

        assert!(
            registry
                .apply("claude-desktop", HarnessAction::Install)
                .unwrap()
                .changed
        );
        assert!(has_entry(&private));
        let written = fs::read_to_string(&private).unwrap();
        assert!(written.starts_with("{\n  // Kept as written.\n  \"theme\": \"dark\",\n"));
        assert!(written.contains("\"other\": {\"command\": \"other-server\"}"));
        assert_eq!(
            fs::read_to_string(&plain).unwrap(),
            left,
            "the plain copy is left as it is"
        );
    }

    #[test]
    fn uninstall_removes_ghostlights_entry_from_every_copy_that_holds_it() {
        let directory = temporary("store-uninstall");
        let context = windows_context(&directory);
        let registry = HarnessRegistry::with_context(context.clone());
        let plain = plain_desktop(&context);
        fs::create_dir_all(plain.parent().unwrap()).unwrap();
        fs::write(&plain, PERSON_SETTINGS).unwrap();
        // An earlier non-Store install set up the plain copy.
        registry
            .apply("claude-desktop", HarnessAction::Install)
            .unwrap();
        let private = store_folder(&context).join("claude_desktop_config.json");
        fs::write(&private, PERSON_SETTINGS).unwrap();
        registry
            .apply("claude-desktop", HarnessAction::Install)
            .unwrap();
        assert!(has_entry(&plain) && has_entry(&private));
        // A foreign entry in a third copy is never removed.
        let foreign = packaged::private_root(
            context.local_app_data.as_deref().unwrap(),
            "Other.Claude_abc123",
            packaged::DataRoot::Roaming,
        )
        .join("Claude/claude_desktop_config.json");
        fs::create_dir_all(foreign.parent().unwrap()).unwrap();
        let foreign_text = r#"{"mcpServers":{"ghostlight":{"command":"node","args":["x.js"]}}}"#;
        fs::write(&foreign, foreign_text).unwrap();
        registry.refresh().unwrap();
        assert_eq!(
            desktop_summary(&registry).owned_copies,
            [plain.to_string_lossy()],
            "only the copy that holds Ghostlight's own entry is named"
        );

        let removed = registry
            .apply("claude-desktop", HarnessAction::Uninstall)
            .unwrap();
        assert!(removed.changed);
        assert!(!has_entry(&plain) && !has_entry(&private));
        for copy in [&plain, &private] {
            let text = fs::read_to_string(copy).unwrap();
            assert!(text.contains("// Kept as written."), "{text}");
            assert!(
                text.contains("\"other\": {\"command\": \"other-server\"}"),
                "{text}"
            );
        }
        assert_eq!(fs::read_to_string(&foreign).unwrap(), foreign_text);
    }

    #[test]
    fn a_plain_install_keeps_its_plain_path_beside_another_apps_leftover_copy() {
        let directory = temporary("store-leftover");
        let context = windows_context(&directory);
        let plain = plain_desktop(&context);
        fs::create_dir_all(plain.parent().unwrap()).unwrap();
        fs::write(&plain, PERSON_SETTINGS).unwrap();
        // An install run inside Codex's container once wrote Codex's private view.
        let leftover = package_folder(&context, "OpenAI.Codex_2p2nqsd0c76g0")
            .join("claude_desktop_config.json");
        fs::write(&leftover, current_settings(&context)).unwrap();
        let registry = HarnessRegistry::with_context(context.clone());

        let summary = desktop_summary(&registry);
        assert_eq!(PathBuf::from(&summary.config_path), plain);
        assert!(
            !summary.packaged,
            "another app's copy is no Store edition's own"
        );
        assert_eq!(summary.state, HarnessState::Available);
        assert_eq!(
            summary.other_copy.as_deref().map(PathBuf::from),
            Some(leftover.clone())
        );
        assert!(
            registry
                .apply("claude-desktop", HarnessAction::Install)
                .unwrap()
                .changed
        );
        assert!(has_entry(&plain));
        assert!(fs::read_to_string(&plain)
            .unwrap()
            .contains("\"other\": {\"command\": \"other-server\"}"));
        assert_eq!(
            fs::read_to_string(&leftover).unwrap(),
            current_settings(&context)
        );
    }

    #[test]
    fn a_store_folder_without_its_file_shows_the_plain_file_and_that_file_is_set_up() {
        let directory = temporary("store-merged");
        let context = windows_context(&directory);
        let plain = plain_desktop(&context);
        fs::create_dir_all(plain.parent().unwrap()).unwrap();
        fs::write(&plain, PERSON_SETTINGS).unwrap();
        let folder = store_folder(&context);
        fs::write(folder.join("config.json"), "{}").unwrap();
        let private = folder.join("claude_desktop_config.json");
        let registry = HarnessRegistry::with_context(context.clone());

        // While the Store edition has no file of its own, its merged view shows the plain one.
        let summary = desktop_summary(&registry);
        assert_eq!(PathBuf::from(&summary.config_path), plain);
        assert!(!summary.packaged && summary.other_copy.is_none());
        assert_eq!(summary.state, HarnessState::Available);
        assert!(
            registry
                .apply("claude-desktop", HarnessAction::Install)
                .unwrap()
                .changed
        );
        assert!(has_entry(&plain));
        let written = fs::read_to_string(&plain).unwrap();
        assert!(written.starts_with("{\n  // Kept as written.\n  \"theme\": \"dark\",\n"));
        assert!(written.contains("\"other\": {\"command\": \"other-server\"}"));
        assert!(
            !private.exists(),
            "no private file hides the person's servers"
        );

        // The entry the app loads is reported as it is, so nothing is added twice.
        registry.refresh().unwrap();
        let summary = desktop_summary(&registry);
        assert_eq!(summary.state, HarnessState::Installed);
        assert!(
            !registry
                .apply("claude-desktop", HarnessAction::Install)
                .unwrap()
                .changed
        );
    }

    #[test]
    fn removal_reaches_an_entry_left_only_in_the_plain_copy() {
        let directory = temporary("store-left-plain");
        let context = windows_context(&directory);
        let plain = plain_desktop(&context);
        fs::create_dir_all(plain.parent().unwrap()).unwrap();
        fs::write(&plain, current_settings(&context)).unwrap();
        let private = store_folder(&context).join("claude_desktop_config.json");
        fs::write(&private, PERSON_SETTINGS).unwrap();
        let registry = HarnessRegistry::with_context(context.clone());

        let summary = desktop_summary(&registry);
        assert_eq!(summary.state, HarnessState::Available);
        assert!(
            summary.can_uninstall,
            "the plain copy's entry can be removed"
        );
        assert_eq!(summary.owned_copies, [plain.to_string_lossy()]);
        let selected = crate::cli::setup::select_harnesses(
            &registry.summaries(),
            &crate::cli::parse::SetupOptions::default(),
            false,
        )
        .unwrap();
        assert!(selected
            .iter()
            .any(|summary| summary.id == "claude-desktop"));

        let removed = registry
            .apply("claude-desktop", HarnessAction::Uninstall)
            .unwrap();
        assert!(removed.changed);
        assert!(!has_entry(&plain));
        assert_eq!(fs::read_to_string(&private).unwrap(), PERSON_SETTINGS);
        assert!(!removed.summary.can_uninstall);
        assert!(removed.summary.owned_copies.is_empty());
    }

    #[test]
    fn a_whole_uninstall_removes_a_stale_owned_entry_and_names_it() {
        let directory = temporary("whole-uninstall-stale");
        let context = context(&directory);
        let claude = definition(&context, "claude-code").unwrap();
        fs::create_dir_all(claude.path.parent().unwrap()).unwrap();
        let older = directory
            .join("older")
            .join(context.edge.file_name().unwrap());
        fs::write(
            &claude.path,
            format!(
                "{{\"mcpServers\": {{\"{SERVER_NAME}\": {{\"command\": {}, \"args\": [\"{MCP_ARGUMENT}\"]}}}}}}",
                serde_json::to_string(&older.to_string_lossy()).unwrap()
            ),
        )
        .unwrap();
        let registry = HarnessRegistry::with_context(context);
        let stale = registry
            .summaries()
            .into_iter()
            .find(|summary| summary.id == "claude-code")
            .unwrap();
        assert_eq!(stale.state, HarnessState::Updatable);
        assert!(stale.can_uninstall, "a stale owned entry is one to remove");
        assert_eq!(
            crate::language::deployment::client_would_remove(
                &stale.label,
                stale.state,
                stale.attention,
                &stale.owned_copies
            ),
            format!("Would remove Ghostlight from {}.", stale.label)
        );

        // A whole uninstall, naming no client, selects it and takes the entry out.
        let selected = crate::cli::setup::select_harnesses(
            &registry.summaries(),
            &crate::cli::parse::SetupOptions::default(),
            false,
        )
        .unwrap();
        assert!(selected.iter().any(|summary| summary.id == "claude-code"));
        let removed = registry
            .apply("claude-code", HarnessAction::Uninstall)
            .unwrap();
        assert!(removed.changed);
        assert!(
            removed
                .message
                .starts_with(&format!("Ghostlight was removed from {}.", stale.label)),
            "{}",
            removed.message
        );
        assert!(!has_entry(&claude.path));
        assert!(!removed.summary.can_uninstall);
    }

    #[test]
    fn every_owned_registration_is_shown_as_a_state_that_holds_an_owned_entry() {
        let directory = temporary("owned-states");
        let context = context(&directory);
        let claude = definition(&context, "claude-code").unwrap();
        fs::create_dir_all(claude.path.parent().unwrap()).unwrap();
        let entry = |command: &Path, arguments: &str| {
            format!(
                "{{\"mcpServers\": {{\"{SERVER_NAME}\": {{\"command\": {}, \"args\": [{arguments}]}}}}}}",
                serde_json::to_string(&command.to_string_lossy()).unwrap()
            )
        };
        let mcp = format!("\"{MCP_ARGUMENT}\"");
        let older = directory
            .join("older")
            .join(context.edge.file_name().unwrap());
        let retired = directory.join("older").join(RETIRED_MCP_CONNECTOR_NAME);
        let seeds = [
            (entry(&context.edge, &mcp), true),
            (entry(&older, &mcp), true),
            (entry(&retired, ""), true),
            (entry(Path::new("alien-agent"), ""), false),
            ("{\"mcpServers\": {".to_owned(), false),
            ("{}".to_owned(), false),
        ];
        for (text, owned) in seeds {
            fs::write(&claude.path, &text).unwrap();
            let state = inspect(&context, &claude).state;
            assert_eq!(
                holds_owned_entry(&context, &claude, &claude.path),
                owned,
                "{text}"
            );
            assert_eq!(state.holds_owned_entry(), owned, "{text}: {state:?}");
        }
    }

    #[test]
    fn a_store_copy_that_needs_attention_names_its_cause_and_uninstall_still_cleans_the_plain_copy()
    {
        let directory = temporary("store-attention");
        let context = windows_context(&directory);
        let plain = plain_desktop(&context);
        fs::create_dir_all(plain.parent().unwrap()).unwrap();
        fs::write(&plain, current_settings(&context)).unwrap();
        let private = store_folder(&context).join("claude_desktop_config.json");
        let broken = "{\"mcpServers\": {";
        fs::write(&private, broken).unwrap();
        let registry = HarnessRegistry::with_context(context.clone());

        let summary = desktop_summary(&registry);
        assert_eq!(PathBuf::from(&summary.config_path), private);
        assert_eq!(summary.state, HarnessState::NeedsAttention);
        assert_eq!(summary.attention, Some(Attention::Malformed));
        assert!(summary.can_uninstall);
        assert_eq!(summary.owned_copies, [plain.to_string_lossy()]);

        let removed = registry
            .apply("claude-desktop", HarnessAction::Uninstall)
            .unwrap();
        assert!(removed.changed);
        assert!(!has_entry(&plain));
        assert_eq!(fs::read_to_string(&private).unwrap(), broken);
        assert!(removed.summary.owned_copies.is_empty());
        assert_eq!(removed.summary.attention, Some(Attention::Malformed));

        fs::write(
            &private,
            r#"{"mcpServers":{"ghostlight":{"command":"node"}}}"#,
        )
        .unwrap();
        registry.refresh().unwrap();
        assert_eq!(
            desktop_summary(&registry).attention,
            Some(Attention::ForeignEntry)
        );
    }

    #[test]
    fn when_the_store_copy_goes_away_the_next_look_falls_back_to_the_plain_path() {
        let directory = temporary("store-gone");
        let context = windows_context(&directory);
        let folder = store_folder(&context);
        let registry = HarnessRegistry::with_context(context.clone());
        assert!(desktop_summary(&registry).packaged);

        fs::remove_dir_all(context.local_app_data.as_deref().unwrap().join("Packages")).unwrap();
        assert!(!folder.exists());
        registry.refresh().unwrap();
        let summary = desktop_summary(&registry);
        assert!(!summary.packaged);
        assert_eq!(PathBuf::from(&summary.config_path), plain_desktop(&context));
        assert_eq!(summary.state, HarnessState::NotDetected);
    }

    #[test]
    fn inside_the_store_apps_own_container_its_copy_is_the_file_its_view_shows_and_is_written() {
        let directory = temporary("store-own-container");
        let mut context = windows_context(&directory);
        let local = context.local_app_data.clone().unwrap();
        context.private_view = Some(PrivateView {
            container: AppContainer {
                app: "Claude".into(),
            },
            roots: vec![context.roaming.clone(), local.clone()],
        });
        let private = store_folder(&context).join("claude_desktop_config.json");
        fs::write(&private, PERSON_SETTINGS).unwrap();
        // The container's merged view shows the same file at the plain path.
        let view = plain_desktop(&context);
        fs::create_dir_all(view.parent().unwrap()).unwrap();
        fs::write(&view, PERSON_SETTINGS).unwrap();
        let registry = HarnessRegistry::with_context(context.clone());

        let summary = desktop_summary(&registry);
        assert!(
            summary.other_copy.is_none(),
            "the view's plain path is the same file, not another copy"
        );
        // Inside the container a write to the plain path lands in the package's private copy of
        // `%APPDATA%`, the same mapping the container probe confirms for `%LOCALAPPDATA%`: the
        // plain path the view shows and the copy found here are one file.
        let rest = plain_desktop(&context)
            .strip_prefix(&context.roaming)
            .unwrap()
            .to_path_buf();
        let redirected =
            packaged::private_root(&local, STORE_FAMILY, packaged::DataRoot::Roaming).join(rest);
        assert_eq!(PathBuf::from(&summary.config_path), redirected);
        assert_eq!(redirected, private);
        assert!(!summary.private_view);
        assert!(summary.can_install);
        assert!(
            registry
                .apply("claude-desktop", HarnessAction::Install)
                .unwrap()
                .changed
        );
        assert!(has_entry(&private));
        assert_eq!(fs::read_to_string(&view).unwrap(), PERSON_SETTINGS);

        // Inside another app's container the same copy is that app's private view, and is left.
        context.private_view = Some(PrivateView {
            container: AppContainer {
                app: "OpenAI.Codex".into(),
            },
            roots: vec![context.roaming.clone(), local],
        });
        let other = HarnessRegistry::with_context(context);
        let summary = desktop_summary(&other);
        assert!(summary.private_view && !summary.can_uninstall);
        assert!(matches!(
            other.apply("claude-desktop", HarnessAction::Uninstall),
            Err(HarnessError::PrivateView(_))
        ));
        assert!(has_entry(&private));
    }

    #[test]
    fn the_icon_catalog_names_each_agent_once_and_every_family_is_another_catalog_agent() {
        let catalog = super::icon_catalog();
        let ids: Vec<&str> = catalog.iter().map(|agent| agent.agent_id).collect();
        let mut expected: Vec<&str> = Vec::new();
        for row in ROSTER {
            if !expected.contains(&row.agent_id) {
                expected.push(row.agent_id);
            }
        }
        assert_eq!(ids, expected, "one entry per agent, in roster order");
        for agent in &catalog {
            if let Some(family) = agent.family {
                assert_ne!(family, agent.agent_id, "{}", agent.agent_id);
                let stand_in = catalog
                    .iter()
                    .find(|other| other.agent_id == family)
                    .unwrap_or_else(|| panic!("{} names no catalog agent", agent.agent_id));
                assert!(
                    !stand_in.sources.is_empty(),
                    "{family} stands for {} but has no sources",
                    agent.agent_id
                );
            }
        }
        let family_of = |id: &str| catalog.iter().find(|agent| agent.agent_id == id).unwrap();
        assert_eq!(family_of("claude-code").family, Some("claude-desktop"));
        assert!(family_of("claude-code").sources.is_empty());
        assert!(!family_of("codex").sources.is_empty());
        assert!(family_of("cline").sources.is_empty());
    }
}
