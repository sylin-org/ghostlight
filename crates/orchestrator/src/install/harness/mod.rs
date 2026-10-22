//! The harness feature: Ghostlight's explicit, ownership-checked registration in each agent's MCP
//! configuration on each host, and the agent catalog it is read from.
//!
//! A harness is one agent's MCP configuration on one host: one row of [`ROSTER`]. The registry
//! inspects each, installs, fixes and removes Ghostlight's own entry in the dialect the client
//! reads, and leaves every entry Ghostlight does not own as it is.
//!
//! This module holds the registry, its public types and errors, the dialects and the format a
//! client's file resolves to, and the file helpers the parts share. The parts: [`context`] (the
//! computer a harness is read on), [`roster`] (the rows and the agent catalog), [`inspect`]
//! (what an entry is and whose), [`launcher`] (Ghostlight's npm launcher), [`json`] and
//! [`codex`] (the two writers), [`acts`] (install, fix and uninstall) and [`locations`] (the
//! paths a person located).

use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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
use crate::sync::lock;

use super::{apps, files, layout, paths, user_dirs, user_path};

use super::container::{AppContainer, Residence};
use super::files::{Access, Link, Replace};
use super::icons::{AgentIconSources, AppPlace, KnownFolder, ProgramPlace};
use super::layout::{
    program_file_name, EDGE_COMMAND_NAME, RETIRED_MCP_CONNECTOR_NAME, WINDOWS_PROGRAM_SUFFIX,
};
use super::packaged::{CopyPlace, DataRoots, Packages, SettingsCopy};
use super::user_dirs::UserDirs;

mod acts;
mod codex;
mod context;
mod inspect;
mod json;
mod launcher;
mod locations;
mod roster;

use acts::*;
use codex::*;
pub(in crate::install) use context::*;
use inspect::*;
pub(in crate::install) use json::*;
pub use launcher::*;
use locations::*;
pub use roster::*;

pub(super) const SERVER_NAME: &str = "ghostlight";
/// The key of the program an entry starts, in every dialect.
const COMMAND_KEY: &str = "command";
/// The key of the program's arguments, in the dialects that keep them apart from the command.
const ARGS_KEY: &str = "args";

/// The arguments that make the edge the MCP relay, in every client's entry.
const MCP_ARGUMENTS: [&str; 1] = [MCP_ARGUMENT];
/// How a client's detail names an owned entry that starts the retired MCP connector.
const RETIRED_ENTRY: words::RetiredEntry<'static> = words::RetiredEntry {
    connector: RETIRED_MCP_CONNECTOR_NAME,
    edge: EDGE_COMMAND_NAME,
    argument: MCP_ARGUMENT,
};

/// The age past which a replace's temporary file beside a configuration is no write in flight
/// but one an interrupted write left behind. A write takes milliseconds.
const STRAY_TEMPORARY_AGE: Duration = Duration::from_secs(60);

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
}

impl HarnessRegistry {
    /// Discover the current user context and perform one initial read-only check.
    #[must_use]
    pub fn discover() -> Self {
        Self::with_context(HarnessContext::system())
    }

    pub(super) fn with_context(context: HarnessContext) -> Self {
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
        let summaries: Vec<HarnessSummary> = definitions(&self.inner.context)
            .into_iter()
            .map(|definition| inspect(&self.inner.context, &definition))
            .collect();
        *lock(&self.inner.summaries) = summaries.clone();
        summaries
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
                message: words::harness_checked(&definition.label),
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
        let eligible: Vec<ResolvedHarness> = definitions(&self.inner.context)
            .into_iter()
            .filter(|definition| {
                definition.is_writable_from(&self.inner.context)
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
            .filter(|definition| definition.is_writable_from(context))
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
        config_root: directory.join("config"),
        codex_config: directory.join("codex").join("config.toml"),
        claude_config_dir: None,
        crush_config: None,
        edge: edge.to_path_buf(),
        path_entries: Vec::new(),
        locations: state.join(layout::HARNESS_LOCATIONS_NAME),
        windows: false,
        linux: cfg!(target_os = "linux"),
        local_app_data: None,
        private_view: None,
    }
}

pub(super) fn definition(
    context: &HarnessContext,
    id: &str,
) -> Result<ResolvedHarness, HarnessError> {
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

/// How a `--client` value names a harness: by its own harness id, or by its agent's id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedBy {
    /// The value is the harness's own id. It selects that harness in any state.
    Harness,
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
    pub fn named_by(&self, id: &str) -> Option<NamedBy> {
        if self.id.eq_ignore_ascii_case(id) {
            Some(NamedBy::Harness)
        } else if self.agent.id.eq_ignore_ascii_case(id) {
            Some(NamedBy::Agent)
        } else {
            None
        }
    }

    /// Whether the `--client` value `id` selects this host. Its harness id always does. Its
    /// agent's id does when the host is detected, or whatever its state when `all_clients` is set.
    #[must_use]
    pub fn is_selected_by(&self, id: &str, all_clients: bool) -> bool {
        match self.named_by(id) {
            Some(NamedBy::Harness) => true,
            Some(NamedBy::Agent) => all_clients || self.state != HarnessState::NotDetected,
            None => false,
        }
    }
}

/// Every value `ghostlight install --client` and `ghostlight uninstall --client` accept, in roster
/// order, for the help text, the refusal of an unknown value and the shell completions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarnessIds {
    /// Every agent's catalog id, once. Each selects the agent's detected harnesses, or every
    /// harness of it with [`ALL_CLIENTS_OPTION`].
    pub agents: Vec<&'static str>,
    /// The id of every harness that no agent id names: the harnesses of an agent that has more
    /// than one. Each selects that one harness.
    pub harnesses: Vec<&'static str>,
}

impl HarnessIds {
    /// The agent ids, then the harness ids.
    pub fn all(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.agents.iter().chain(&self.harnesses).copied()
    }
}

/// The `--client` values the roster offers. A harness id is a persisted key (the person's located
/// paths are stored under it), so every one of them stays a value even when its agent's id is
/// also listed.
#[must_use]
pub fn harness_ids() -> HarnessIds {
    let mut agents: Vec<&'static str> = Vec::new();
    for row in ROSTER {
        if !agents.contains(&row.agent_id) {
            agents.push(row.agent_id);
        }
    }
    let harnesses = ROSTER
        .iter()
        .map(|row| row.id)
        .filter(|id| !agents.contains(id))
        .collect();
    HarnessIds { agents, harnesses }
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

#[derive(Clone, Copy)]
pub(super) enum ConfigDialect {
    Json(JsonDialect),
    /// A client whose file Ghostlight never parses or writes: a person pastes the setup.
    Manual(ManualDialect),
    CodexToml,
    OpenCode,
}

#[derive(Clone, Copy)]
pub(super) enum ManualDialect {
    Goose,
    Continue,
}

/// How Ghostlight reads and writes one client's file once its dialect is resolved: JSON in one of
/// its dialects, or Codex's TOML. OpenCode resolves to the JSON form its file holds.
#[derive(Clone, Copy)]
enum Format {
    Json(JsonDialect),
    Codex,
}

impl ConfigDialect {
    /// The format of a client's file, or the manual dialect of a client whose file Ghostlight
    /// never reads or writes. Only OpenCode reads `source`, its file's text, so each copy an act
    /// edits resolves its own form.
    fn format(
        self,
        context: &HarnessContext,
        source: impl FnOnce() -> Result<String, HarnessError>,
    ) -> Result<Result<Format, ManualDialect>, HarnessError> {
        Ok(match self {
            Self::Json(dialect) => Ok(Format::Json(dialect)),
            Self::CodexToml => Ok(Format::Codex),
            Self::OpenCode => Ok(Format::Json(opencode_dialect(context, &source()?)?)),
            Self::Manual(dialect) => Err(dialect),
        })
    }
}

impl Format {
    /// The state of Ghostlight's entry in the file at `path`, which exists.
    fn inspect(
        self,
        context: &HarnessContext,
        path: &Path,
    ) -> Result<RegistrationState, HarnessError> {
        let source = fs::read_to_string(path).map_err(HarnessError::Read)?;
        match self {
            Self::Json(dialect) => inspect_json(&source, dialect, &context.edge, context.windows),
            Self::Codex => inspect_toml(&source, context),
        }
    }

    /// Make the edit `intent` to Ghostlight's entry in the file at `path`. Whether it changed.
    fn edit(
        self,
        context: &HarnessContext,
        path: &Path,
        intent: RegistrationEdit,
    ) -> Result<bool, HarnessError> {
        match self {
            Self::Json(dialect) => edit_json_with(context, path, dialect, intent),
            Self::Codex => edit_toml_with(context, path, intent),
        }
    }

    /// The setup a person pastes into a file of this format.
    fn manual_setup(self, context: &HarnessContext) -> Result<String, HarnessError> {
        match self {
            Self::Codex => {
                let mut entry = Item::Table(Table::new());
                entry[COMMAND_KEY] = value(context.edge.to_string_lossy().into_owned());
                entry[ARGS_KEY] = value(mcp_arguments_array());
                forward_codex_desktop_environment(&mut entry, context.linux)?;
                let mut document = DocumentMut::new();
                document[CODEX_SERVERS_KEY] = Item::Table(Table::new());
                document[CODEX_SERVERS_KEY][SERVER_NAME] = entry;
                Ok(document.to_string())
            }
            Self::Json(dialect) => manual_json_setup(&context.edge, dialect),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum JsonDialect {
    McpServers,
    Copilot,
    Servers,
    ContextServers,
    Crush,
    OpenCodeV1,
    OpenCodeV2,
    ZCode,
}

/// The sentence a person reads for a client whose file Ghostlight never edits.
fn manual_setup_sentence(definition: &ResolvedHarness) -> String {
    let file = definition
        .path
        .file_name()
        .map(|name| name.to_string_lossy());
    words::manual_setup(file.as_deref())
}

fn manual_refusal(definition: &ResolvedHarness) -> HarnessError {
    HarnessError::ManualSetup(manual_setup_sentence(definition))
}

pub(super) fn manual_setup(
    context: &HarnessContext,
    definition: &ResolvedHarness,
) -> Result<String, HarnessError> {
    match definition
        .dialect
        .format(context, || read_or_empty(&definition.path))?
    {
        Ok(format) => format.manual_setup(context),
        Err(dialect) => Ok(manual_yaml_setup(&context.edge, dialect)),
    }
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

/// How Ghostlight replaces a client's configuration: the client's access kept, the old file
/// copied beside it first, and a link written through. A configuration tracked through a link (a
/// synced dotfiles repository, say) is a deliberate choice, so it stays a link to the file it
/// names.
const CONFIGURATION_WRITE: Replace = Replace {
    access: Access::Kept,
    backup: true,
    link: Link::Follow,
};

/// Remove from `directory` each replace temporary ([`files::is_temporary`]) that is at least
/// [`STRAY_TEMPORARY_AGE`] old at `now`. Returns how many were removed.
fn remove_stray_temporaries_in(directory: &Path, now: SystemTime) -> usize {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| files::is_temporary(&entry.file_name()))
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
    let executable = program_file_name(name, context.windows);
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
    /// A Fix found Ghostlight's own entry where it expected a foreign one.
    #[error("{}", words::NOT_FIXABLE)]
    NotFixable,
    /// A Fix found no entry under Ghostlight's name.
    #[error("{}", words::NOTHING_TO_FIX)]
    NoEntryToFix,
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
    use super::user_dirs::{self, UserDirs};
    use crate::install::packaged;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::{json, Value};

    use crate::install::files::assert_owner_only;
    use crate::scratch::Scratch;

    use jsonc_parser::parse_to_serde_value;

    use super::{
        agent_called, command_registration_state, definition, definitions, environment_roots,
        expected_json_entry, harness_roots, holds_owned_entry, inspect, inspect_json, inspect_toml,
        json_entry_defaults, program_file_name, remove_stray_temporaries_in, replace_with_backup,
        AppContainer, Attention, ForeignCommand, Format, HarnessAction, HarnessContext,
        HarnessError, HarnessRegistry, HarnessRoots, HarnessState, HarnessSummary, JsonDialect,
        LauncherPin, NpmLauncher, Packages, PrivateView, RegistrationEdit, RegistrationState,
        CODEX_HOME_VARIABLE, EDGE_COMMAND_NAME, MCP_ARGUMENT, RETIRED_MCP_CONNECTOR_NAME, ROSTER,
        SERVER_NAME,
    };

    /// The name of the roster's agent whose client calls itself `reported`.
    fn named(reported: &str) -> Option<String> {
        agent_called(reported).map(|agent| agent.name)
    }

    fn temporary(name: &str) -> Scratch {
        Scratch::named(&format!("harness-{name}"))
    }

    fn edge(directory: &Path) -> PathBuf {
        let edge = directory.join(program_file_name(EDGE_COMMAND_NAME, cfg!(windows)));
        fs::write(&edge, b"test").unwrap();
        edge
    }

    fn context(directory: &Path) -> HarnessContext {
        HarnessContext {
            home: directory.join("home"),
            config_root: directory.join("config"),
            codex_config: directory.join("codex/config.toml"),
            claude_config_dir: None,
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
        let desktop = context
            .config_root
            .join("Claude/claude_desktop_config.json");
        fs::create_dir_all(desktop.parent().unwrap()).unwrap();
        fs::write(&desktop, r#"{"mcpServers":{}}"#).unwrap();
        (context, desktop)
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
        context
            .config_root
            .join("Claude/claude_desktop_config.json")
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

    /// Protects workbench gates 8 and 10 and arch-1 slices I2 and I4: an entry is Ghostlight's only
    /// when it starts this edge or a retired Ghostlight program with the exact signature and install
    /// root, under any spelling Windows allows. The edge with other arguments or another path is
    /// updatable, an entry naming the retired connector is retired rather than installed, a
    /// hand-registered service command stays foreign, and every state shown as owned holds an owned
    /// entry.
    #[test]
    fn an_entry_is_ghostlights_only_when_it_starts_this_edge_or_a_retired_ghostlight() {
        {
            let directory = temporary("legacy-relay");
            let install_root = directory.join("bin");
            let current = install_root
                .join("v1")
                .join(program_file_name(EDGE_COMMAND_NAME, cfg!(windows)));
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
        #[cfg(windows)]
        {
            let directory = temporary("legacy-relay-slashes");
            let install_root = directory.join("bin");
            let current = install_root
                .join("v1")
                .join(program_file_name(EDGE_COMMAND_NAME, true));
            let legacy = install_root.join("v0").join("ghostlight-relay.exe");
            let respelled = legacy.to_string_lossy().replace('\\', "/").to_uppercase();
            assert_eq!(
                command_registration_state(&respelled, Some(&["--role", "agent"]), &current, true),
                RegistrationState::Updatable
            );
        }
        {
            // A hand-written entry that runs the service image, which has no MCP stdio mode, is
            // foreign evidence (ADR-0141): install and remove never overwrite it; only the
            // confirmed Fix may.
            let directory = temporary("zcode-foreign-orchestrator");
            let path = directory.join("config.json");
            let context = context(&directory);
            let edge = context.edge.clone();
            let orchestrator = directory
                .join(program_file_name("ghostlight-service", cfg!(windows)))
                .to_string_lossy()
                .replace('\\', "\\\\");
            let source = format!(
            "{{\"mcp\":{{\"servers\":{{\"ghostlight\":{{\"command\":\"{orchestrator}\",\"args\":[],\"env\":{{}}}}}}}}}}"
        );
            fs::write(&path, &source).unwrap();
            for install in [true, false] {
                assert!(matches!(
                    Format::Json(JsonDialect::ZCode).edit(
                        &context,
                        &path,
                        if install {
                            RegistrationEdit::Install
                        } else {
                            RegistrationEdit::Uninstall
                        }
                    ),
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
        {
            let directory = temporary("retired");
            let current = edge(&directory);
            let old = directory
                .join("old")
                .join(program_file_name(RETIRED_MCP_CONNECTOR_NAME, cfg!(windows)));
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
        {
            let directory = temporary("updatable");
            let current = edge(&directory);
            let elsewhere = directory
                .join("elsewhere")
                .join(program_file_name(EDGE_COMMAND_NAME, cfg!(windows)));
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
                    inspect_json(&source, JsonDialect::McpServers, &current, cfg!(windows))
                        .unwrap(),
                    RegistrationState::Updatable
                );
            }
        }
        {
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
    }

    /// Protects workbench gate 8: setting up everything adds what is missing and updates what is owned
    /// on detected agents only, and leaves not-detected, blocked and container-private targets as
    /// they are.
    #[test]
    fn set_up_everything_touches_only_detected_changeable_targets() {
        {
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
        {
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
                assert!(
                    matches!(refused, HarnessError::PrivateView(ref app) if app == "OpenAI.Codex")
                );
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
    }

    /// Protects defects 567cf210 and 218d386f: what a scan reports about a foreign or malformed entry
    /// names only the program found and its argument count, or the line of a fault, and nothing else
    /// from the file, so no secret, argument or control character leaves it.
    #[test]
    fn evidence_names_only_the_found_program_and_nothing_from_the_file() {
        {
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
        {
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
        {
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
        {
            let line = ForeignCommand::from_entry(
                "C:/tools/other-server.exe --token sk/test-secret",
                None,
            )
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
        {
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
    }

    /// Protects arch-1 slice I3 and defects 5d589e1d, 4af49885 and f51130d0: an update sets only the
    /// keys Ghostlight owns and keeps everything else the client wrote, in every dialect: a current
    /// entry under another spelling is left byte for byte, a dialect that needs a type gains it, and
    /// Codex keeps its custom, inline and malformed environment members while gaining desktop
    /// forwarding on Linux. A write never leaves a temporary behind, and an install sweeps the ones an
    /// interrupted write left.
    #[test]
    fn an_update_sets_only_ghostlights_keys_and_keeps_the_clients() {
        {
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
        {
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
        {
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
                    !Format::Json(dialect)
                        .edit(&context, &path, RegistrationEdit::Install)
                        .unwrap(),
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
            assert!(!Format::Codex
                .edit(&context, &path, RegistrationEdit::Install)
                .unwrap());
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
        {
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
                assert!(Format::Json(dialect)
                    .edit(&context, &path, RegistrationEdit::Install)
                    .unwrap());
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
        #[cfg(target_os = "linux")]
        {
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
            assert!(Format::Codex
                .edit(&context, &path, RegistrationEdit::Install)
                .unwrap());
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
            assert!(!Format::Codex
                .edit(&context, &path, RegistrationEdit::Install)
                .unwrap());
            assert_eq!(fs::read_to_string(&path).unwrap(), installed);
        }
        #[cfg(target_os = "linux")]
        {
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
            assert!(Format::Codex
                .edit(&context, &path, RegistrationEdit::Install)
                .is_err());
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
        {
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
                    assert!(Format::Codex.edit(&context, &path, RegistrationEdit::Install).is_err());
                    assert_eq!(fs::read_to_string(&path).unwrap(), source);
                }
            }
        }
        }
        #[cfg(target_os = "linux")]
        {
            let directory = temporary("codex-inline-env");
            let path = directory.join("config.toml");
            let context = context(&directory);
            let edge = context.edge.clone();
            // The edge command with no arguments is an updatable registration on purpose.
            fs::write(&path, format!(
            "[mcp_servers]\nghostlight = {{ command = {:?}, args = [], startup_timeout_sec = 45 }}\n",
            edge.to_string_lossy()
        )).unwrap();
            assert!(Format::Codex
                .edit(&context, &path, RegistrationEdit::Install)
                .unwrap());
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
    }

    /// Protects arch-1 slice I4 and defects 765736e1 and 41a3f8bf: uninstall removes Ghostlight's entry
    /// from every copy of an agent's settings that holds one, the plain copy and a stale entry
    /// included, names each removal in the dry run, and leaves everything else.
    #[test]
    fn uninstall_removes_every_owned_entry_from_every_copy_and_nothing_else() {
        {
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
            let foreign_text =
                r#"{"mcpServers":{"ghostlight":{"command":"node","args":["x.js"]}}}"#;
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
        {
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
                crate::cli::setup::Direction::Uninstall,
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
        {
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
                crate::cli::setup::Direction::Uninstall,
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
    }

    /// Protects workbench gate 10, arch-1 slice I1 and defects c51204ca, 1d220361 and 0a0ef063:
    /// Ghostlight writes the file each agent reads in the effective environment. Empty or relative
    /// root variables resolve below the home; XDG, APPDATA, CODEX_HOME, CRUSH_GLOBAL_CONFIG and
    /// CLAUDE_CONFIG_DIR are honored, the last read from the person's environment when the process
    /// has none; a host reads the first of its candidates that exists. A located path is validated,
    /// kept owner-only, persists as detection evidence and is reported when it goes missing.
    #[test]
    fn each_agents_file_is_the_one_it_reads_in_the_effective_environment() {
        {
            let home = if cfg!(windows) {
                "C:/Users/test"
            } else {
                "/home/test"
            };
            let lookup = |name: &str| match name {
                user_dirs::USER_PROFILE_VARIABLE | user_dirs::HOME_VARIABLE => {
                    Some(OsString::from(home))
                }
                user_dirs::XDG_CONFIG_HOME_VARIABLE | CODEX_HOME_VARIABLE => Some(OsString::new()),
                user_dirs::APP_DATA_VARIABLE => Some(OsString::from("Roaming")),
                _ => None,
            };
            let only_home =
                |name: &str| (name == user_dirs::HOME_VARIABLE).then(|| OsString::from(home));
            assert_eq!(
                UserDirs::from_lookup(&only_home, true).home(),
                Path::new("")
            );
            let home = Path::new(home);

            for windows in [false, true] {
                let dirs = UserDirs::from_lookup(&lookup, windows);
                let config = if windows {
                    home.join("AppData").join("Roaming")
                } else {
                    home.join(".config")
                };
                assert_eq!(
                    environment_roots(&dirs, &lookup, windows),
                    HarnessRoots {
                        config_root: config,
                        codex_config: home.join(".codex").join("config.toml"),
                    }
                );
            }
        }
        {
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
                    config_root: PathBuf::from("/mnt/config"),
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
                    config_root: PathBuf::from("D:/Profiles/Roaming"),
                    codex_config: PathBuf::from("C:/Users/test/.codex/config.toml"),
                }
            );
        }
        {
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
                [super::layout::HARNESS_LOCATIONS_NAME]
            );
        }
        {
            let directory = temporary("kilo-current-config");
            let context = context(&directory);
            let current = context.config_root.join("kilo").join("kilo.jsonc");
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
        {
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
        {
            for named in [false, true] {
                let directory = temporary(&format!("claude-config-dir-{named}"));
                let own = directory.join("claude-profile");
                let context = HarnessContext {
                    claude_config_dir: named.then(|| own.clone()),
                    ..context(&directory)
                };
                let (read, unread) = if named {
                    (own.join(".claude.json"), context.home.join(".claude.json"))
                } else {
                    (context.home.join(".claude.json"), own.join(".claude.json"))
                };
                fs::create_dir_all(read.parent().unwrap()).unwrap();
                fs::write(&read, "{\"theme\": \"dark\"}\n").unwrap();
                let registry = HarnessRegistry::with_context(context);

                let installed = registry
                    .apply("claude-code", HarnessAction::Install)
                    .unwrap();
                assert!(installed.changed, "{named}");
                assert!(has_entry(&read), "{named}");
                assert!(fs::read_to_string(&read).unwrap().contains("dark"));
                let found = registry
                    .summaries()
                    .into_iter()
                    .find(|summary| summary.id == "claude-code")
                    .unwrap();
                assert_eq!(found.state, HarnessState::Installed, "{named}");
                assert!(found.can_uninstall, "{named}");

                let removed = registry
                    .apply("claude-code", HarnessAction::Uninstall)
                    .unwrap();
                assert!(removed.changed, "{named}");
                assert!(!has_entry(&read), "{named}");
                assert!(!unread.exists(), "{named}");
            }
        }
        {
            use std::ffi::OsString;

            use super::user_dirs::USER_PROFILE_VARIABLE;
            use super::{claude_config_dir, user_path};

            let lookup =
                |name: &str| (name == USER_PROFILE_VARIABLE).then(|| r"C:\Users\a".to_owned());
            let stored = |text: &str, expandable| {
                Some(user_path::PathValue {
                    text: text.to_owned(),
                    expandable,
                })
            };

            assert_eq!(
                claude_config_dir(
                    Some(OsString::from(r"D:\process")),
                    stored(r"D:\person", false),
                    &lookup
                ),
                Some(PathBuf::from(r"D:\process"))
            );
            for unset in [None, Some(OsString::new())] {
                assert_eq!(
                    claude_config_dir(
                        unset.clone(),
                        stored(r"%USERPROFILE%\claude", true),
                        &lookup
                    ),
                    Some(PathBuf::from(r"C:\Users\a\claude"))
                );
                assert_eq!(
                    claude_config_dir(
                        unset.clone(),
                        stored(r"%USERPROFILE%\claude", false),
                        &lookup
                    ),
                    Some(PathBuf::from(r"%USERPROFILE%\claude"))
                );
                assert_eq!(
                    claude_config_dir(unset.clone(), stored("", true), &lookup),
                    None
                );
                assert_eq!(claude_config_dir(unset, None, &lookup), None);
            }
        }
        {
            let directory = temporary("crush-variable");
            let named = directory.join("elsewhere/crush.json");
            let context = HarnessContext {
                crush_config: Some(named.clone()),
                ..context(&directory)
            };
            assert_eq!(definition(&context, "crush").unwrap().path, named);
        }
        #[cfg(unix)]
        {
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
        #[cfg(unix)]
        {
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
        {
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
    }

    /// Protects the ADR-0197 container amendments and defects 4fac15a7 and 236485f4: a Store app's own
    /// copy of its settings is the file its view shows and is written once from inside its container,
    /// another app's container leaves it, and an agent absent from its plain path is never found in
    /// another app's copy.
    #[test]
    fn inside_a_store_apps_own_container_its_copy_is_written_once_and_another_container_leaves_it()
    {
        {
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
            let value: Value =
                serde_json::from_str(&fs::read_to_string(&desktop).unwrap()).unwrap();
            assert!(value["mcpServers"].get("ghostlight").is_some());
        }
        {
            // As on leo-desktop-02 on 2026-10-20: Visual Studio Code had never been started, so its
            // plain settings folder did not exist, and an install had written Ghostlight's entry into
            // the Codex app's private copy of that folder, which Visual Studio Code never reads.
            let directory = temporary("store-other-only");
            let context = windows_context(&directory);
            let plain = context.config_root.join("Code/User/mcp.json");
            // Spelled as the look spells it: the settings folder as the row names it, then the file.
            let leftover = packaged::private_root(
                context.local_app_data.as_deref().unwrap(),
                "OpenAI.Codex_2p2nqsd0c76g0",
                packaged::DataRoot::Roaming,
            )
            .join("Code/User")
            .join("mcp.json");
            fs::create_dir_all(leftover.parent().unwrap()).unwrap();
            fs::write(
            &leftover,
            format!(
                "{{\"servers\":{{\"ghostlight\":{{\"type\":\"stdio\",\"command\":{},\"args\":[\"mcp\"]}}}}}}",
                serde_json::to_string(&context.edge.to_string_lossy()).unwrap()
            ),
        )
        .unwrap();
            let registry = HarnessRegistry::with_context(context.clone());
            let editor = || {
                registry
                    .summaries()
                    .into_iter()
                    .find(|summary| summary.id == "vscode")
                    .unwrap()
            };

            let summary = editor();
            assert_eq!(PathBuf::from(&summary.config_path), plain);
            assert_eq!(
                summary.state,
                HarnessState::NotDetected,
                "another app's copy is no evidence of the agent: {}",
                summary.detail
            );
            assert_eq!(summary.owned_copies, [leftover.to_string_lossy()]);

            let removed = registry.apply("vscode", HarnessAction::Uninstall).unwrap();
            assert!(removed.changed, "{}", removed.message);
            let left = fs::read_to_string(&leftover).unwrap();
            assert!(!left.contains(SERVER_NAME), "{left}");
            assert!(
                !plain.exists(),
                "nothing is written where the agent is absent"
            );
            assert!(editor().owned_copies.is_empty());
        }
        {
            let directory = temporary("store-own-container");
            let mut context = windows_context(&directory);
            let local = context.local_app_data.clone().unwrap();
            context.private_view = Some(PrivateView {
                container: AppContainer {
                    app: "Claude".into(),
                },
                roots: vec![context.config_root.clone(), local.clone()],
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
                .strip_prefix(&context.config_root)
                .unwrap()
                .to_path_buf();
            let redirected =
                packaged::private_root(&local, STORE_FAMILY, packaged::DataRoot::Roaming)
                    .join(rest);
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
                roots: vec![context.config_root.clone(), local],
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
    }

    /// Protects arch-1 slice I5: an entry that starts Ghostlight's own npm package through npx, in JSON
    /// or Codex TOML, is the launcher whatever version it pins; it is never current, install and
    /// uninstall leave it, and only Fix replaces it.
    #[test]
    fn the_npm_launcher_entry_is_never_current_and_only_fix_replaces_it() {
        {
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
        {
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
        {
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
        {
            let launcher =
                |command: &str, args: &[&str]| NpmLauncher::from_entry(command, Some(args));
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
        {
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
                Format::Codex.edit(&context, &path, RegistrationEdit::Install),
                Err(HarnessError::LauncherEntry)
            ));
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
    }

    /// Protects the agent catalog contract and defect d4bdaf9c: every agent is known once by its id,
    /// name and aliases on every host it runs on, a product id two agents share names only its own
    /// agent, reserved entries are no row, the apps an agent runs inside and every icon family are
    /// catalog agents, and coexisting hosts of one client carry distinct labels.
    #[test]
    fn the_agent_catalog_names_each_agent_once_and_its_references_resolve() {
        {
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
        {
            let devin = ROSTER.iter().find(|row| row.id == "devin").unwrap();
            assert_eq!(devin.product_id, "windsurf");
            assert_eq!(named("windsurf").as_deref(), Some("Windsurf"));
            assert_eq!(named("devin").as_deref(), Some("Devin"));
            assert_eq!(named("Devin").as_deref(), Some("Devin"));
        }
        {
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
        {
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
        {
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
        {
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
        {
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
}
