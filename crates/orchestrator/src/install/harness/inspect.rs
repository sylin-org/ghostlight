//! Inspection: what Ghostlight's entry in a harness's configuration is, whether Ghostlight
//! owns it, and the bounded evidence a summary shows of an entry it does not own.

use super::*;

pub(super) fn inspect(context: &HarnessContext, definition: &ResolvedHarness) -> HarnessSummary {
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
    // One reading of the registration gives the state, the words naming a blocked cause, the
    // evidence beside them, the attention cause and whether Fix may replace the entry. The blocked
    // causes are named from what the inspection actually saw, never conflated (ADR-0135).
    let reading = match &registration {
        Ok(RegistrationState::Missing) => Reading::plain(if detected {
            HarnessState::Available
        } else {
            HarnessState::NotDetected
        }),
        Ok(RegistrationState::Current) => Reading::plain(HarnessState::Installed),
        Ok(RegistrationState::Updatable | RegistrationState::Retired) => {
            Reading::plain(HarnessState::Updatable)
        }
        Ok(RegistrationState::Launcher(launcher)) => Reading {
            state: HarnessState::NeedsAttention,
            cause: Some(words::launcher_detail(launcher)),
            evidence: Some(words::launcher_evidence(launcher, &context.edge)),
            attention: Some(Attention::Launcher(launcher.pin)),
            fixable: true,
        },
        Ok(RegistrationState::Foreign(found_command)) => Reading {
            state: HarnessState::NeedsAttention,
            cause: Some(words::CLIENT_FOREIGN_ENTRY.into()),
            evidence: Some(match found_command {
                Some(found) => {
                    words::other_program_evidence(&found.program, found.arguments, &context.edge)
                }
                None => words::no_command_evidence(&context.edge),
            }),
            attention: Some(Attention::ForeignEntry),
            fixable: true,
        },
        Err(error @ HarnessError::Malformed(_)) => Reading {
            state: HarnessState::NeedsAttention,
            cause: Some(error.to_string()),
            evidence: None,
            attention: Some(Attention::Malformed),
            fixable: false,
        },
        Err(_) => Reading {
            state: HarnessState::NeedsAttention,
            cause: Some(words::CLIENT_UNREADABLE.into()),
            evidence: None,
            attention: Some(Attention::Unreadable),
            fixable: false,
        },
    };
    let Reading {
        state,
        cause,
        evidence,
        attention,
        fixable,
    } = reading;
    // A container's private view comes first: the cause behind it cannot be acted on here.
    let mut detail: String = match (private, cause) {
        (Some(container), _) if !manual => words::client_kept_private(&container.app),
        (_, Some(cause)) => cause,
        _ if manual => manual_setup_sentence(definition),
        _ => words::client_detail(state, edge_ready, retired.then_some(RETIRED_ENTRY)),
    };
    if definition.located_stale {
        detail = words::with_located_path_missing(&detail);
    }
    let changeable = private.is_none();
    let can_fix = changeable && edge_ready && fixable;
    let owned_copies = owned_other_copies(context, definition);
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

/// What one reading of a registration means for its summary.
pub(super) struct Reading {
    pub(super) state: HarnessState,
    /// The words naming a blocked cause, when the registration has one.
    pub(super) cause: Option<String>,
    pub(super) evidence: Option<String>,
    pub(super) attention: Option<Attention>,
    /// Whether Fix may replace the entry.
    pub(super) fixable: bool,
}

impl Reading {
    /// A state with no blocked cause.
    pub(super) const fn plain(state: HarnessState) -> Self {
        Self {
            state,
            cause: None,
            evidence: None,
            attention: None,
            fixable: false,
        }
    }
}

pub(super) fn product_config_parent_exists(context: &HarnessContext, path: &Path) -> bool {
    let Some(parent) = path.parent().filter(|parent| parent.exists()) else {
        return false;
    };
    // A product-specific settings directory is useful installation evidence. A generic home or
    // configuration root is not: treating `$HOME` as Claude Code detection made that harness
    // appear installed on every ordinary machine and would make aggregate setup overreach.
    ![&context.home, &context.config_root]
        .into_iter()
        .any(|root| paths::spelled_alike(parent, root, context.windows))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RegistrationState {
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

impl RegistrationState {
    /// Whether the entry is Ghostlight's own, whatever its age: the one owned-entry predicate
    /// (ADR-0203 Decision 10.2).
    pub(super) const fn is_owned(&self) -> bool {
        matches!(self, Self::Current | Self::Updatable | Self::Retired)
    }
}

/// What Ghostlight discloses about a foreign entry's command: the program and how many
/// arguments follow it, and nothing of their text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ForeignCommand {
    /// The bounded file name of the program.
    pub(super) program: String,
    /// The `args` array length plus every further word of a whole-command-line `command`.
    pub(super) arguments: usize,
}

impl ForeignCommand {
    /// Reduce a foreign entry's command to its program and argument count. A command that
    /// names an existing file is that program whole. Any other command is cut at its first
    /// whitespace before a file name is taken, so no word after the program is ever read as a
    /// path. A command with no file name part yields `None`.
    pub(super) fn from_entry(command: &str, args: Option<&[&str]>) -> Option<Self> {
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

/// The refusal of an install or an uninstall that finds an entry Ghostlight does not own under its
/// key, starting `command` with `args` when it names a command: Ghostlight's npm launcher is said
/// as itself, anything else as foreign.
pub(super) fn unowned_entry_refusal(command: Option<&str>, args: Option<&[&str]>) -> HarnessError {
    match command.and_then(|command| NpmLauncher::from_entry(command, args)) {
        Some(_) => HarnessError::LauncherEntry,
        None => HarnessError::ForeignEntry,
    }
}

/// The state of Ghostlight's entry in `definition`'s own configuration file.
pub(super) fn registration_state(
    context: &HarnessContext,
    definition: &ResolvedHarness,
) -> Result<RegistrationState, HarnessError> {
    registration_state_at(context, definition, &definition.path)
}

/// The state of Ghostlight's entry in the file at `path`, read as the configuration of
/// `definition`.
pub(super) fn registration_state_at(
    context: &HarnessContext,
    definition: &ResolvedHarness,
    path: &Path,
) -> Result<RegistrationState, HarnessError> {
    if !path.exists() {
        return Ok(RegistrationState::Missing);
    }
    let read = || fs::read_to_string(path).map_err(HarnessError::Read);
    match definition.dialect.format(context, read)? {
        Ok(format) => format.inspect(context, path),
        Err(_) => Ok(RegistrationState::Missing),
    }
}

pub(super) fn command_name(command: &str, expected: &str) -> bool {
    Path::new(command)
        .file_stem()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.eq_ignore_ascii_case(expected))
}

pub(super) fn command_owned(
    command: &str,
    args: Option<&[&str]>,
    edge: &Path,
    windows: bool,
) -> bool {
    command_name(command, EDGE_COMMAND_NAME)
        || command_name(command, RETIRED_MCP_CONNECTOR_NAME)
        || legacy_relay_owned(command, args, edge, windows)
}

pub(super) fn legacy_relay_owned(
    command: &str,
    args: Option<&[&str]>,
    edge: &Path,
    windows: bool,
) -> bool {
    if !command_name(command, "ghostlight-relay") || args != Some(&["--role", "agent"]) {
        return false;
    }
    let Some(actual_root) = Path::new(command).parent().and_then(Path::parent) else {
        return false;
    };
    let Some(expected_root) = edge.parent().and_then(Path::parent) else {
        return false;
    };
    paths::same_location_on(actual_root, expected_root, windows)
}

pub(super) fn command_registration_state(
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
        && paths::same_location_on(Path::new(command), edge, windows)
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
pub(super) const EVIDENCE_MAX_CHARS: usize = 200;

/// Bound one piece of found content for disclosure.
///
/// Control characters and bidi overrides are stripped, whitespace collapses to single spaces,
/// and the result is capped, so file-authored text can never smuggle formatting into a card.
pub(super) fn bounded_disclosure(value: &str) -> Option<String> {
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
