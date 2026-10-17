//! The writer golden: every byte Ghostlight's installer writes into an agent's MCP configuration
//! and every sentence it says about it, pinned in `tests/golden/agents/writes/`.
//!
//! Each host of the roster is set up from fixed seed files on three computers: Windows, Linux,
//! and Windows seen from inside another app's container. For each seed the golden records what
//! inspection says, what the Check, Install, Fix and Uninstall acts say, and every file each act
//! wrote or removed, each act starting from a fresh copy of the seed. A setup of everything is
//! recorded once per computer. The files change only on purpose: run this test with
//! [`UPDATE_VARIABLE`] set, then read the diff line by line.
//!
//! The seeds are written out here as a person's files hold them, never built by the code under
//! test. Every path is built with forward slashes, and the run's directory and the edge are shown
//! as placeholders, so one file holds on every build system. How the build system's own
//! separators are written, backslashes and their escapes on Windows, is pinned per build system
//! in `tests/golden/agents/native/` (the `native` module).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use super::container::AppContainer;
use super::layout::{EDGE_COMMAND_NAME, RETIRED_MCP_CONNECTOR_NAME};
use super::{
    definition, executable_name, harness_roots, AgentRow, ConfigDialect, HarnessAction,
    HarnessContext, HarnessRegistry, HarnessState, HarnessSummary, JsonDialect, PrivateView,
    HARNESS_LOCATIONS_FILE, ROSTER,
};
use crate::language::self_check::golden::{check_golden_with, repository_path};

/// The environment variable that rewrites the writer golden instead of comparing it.
const UPDATE_VARIABLE: &str = "GHOSTLIGHT_AGENTS_GOLDEN_UPDATE";

/// What the golden pins, for the failure message.
const SUBJECT: &str = "what the installer writes and says";

/// The golden's directory, relative to the repository root: one file per host, plus the setup of
/// everything.
const GOLDEN_DIRECTORY: &str = "tests/golden/agents/writes";

/// The file that records setting up everything at once.
const SETUP_GOLDEN: &str = "setup-everything.txt";

/// The extension of every golden file.
const GOLDEN_EXTENSION: &str = "txt";

/// What the golden shows in place of the run's own directory.
const ROOT_PLACEHOLDER: &str = "<root>";

/// What the golden shows in place of the edge's path.
const EDGE_PLACEHOLDER: &str = "<edge>";

/// The person's home directory below the run's directory. It holds a space, as many Windows
/// profiles do, so the command line's quoting is recorded.
const PERSON: &str = "Pat Doe";

/// The home of an older install, whose edge a stale entry starts.
const OLD_PERSON: &str = "Old Home";

/// The app container the third computer runs Ghostlight from.
const CONTAINER_APP: &str = "OpenAI.Codex";

/// The argument a seed's entry gives the edge, written out as a person's file holds it.
const EDGE_ARGUMENT: &str = "mcp";

/// The program a 1.x entry started, with the arguments it gave it.
const LEGACY_RELAY: &str = "ghostlight-relay";
const LEGACY_RELAY_ARGUMENTS: [&str; 2] = ["--role", "agent"];

/// The opening of every seed that holds a person's own settings.
const KEPT_COMMENT: &str = "A person's own settings, kept as written.";

/// The desktop session variables a Linux Codex entry forwards, written out as a seed.
const FORWARDED_LIST: &str = "[\"DISPLAY\", \"WAYLAND_DISPLAY\", \"XDG_RUNTIME_DIR\", \
                              \"DBUS_SESSION_BUS_ADDRESS\", \"XDG_CURRENT_DESKTOP\", \"XAUTHORITY\"]";

/// A computer the installer runs on.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Computer {
    Windows,
    Linux,
    /// Windows, with Ghostlight run from inside another app's container.
    InsideAnotherApp,
}

/// Every computer the golden records, in its order.
const COMPUTERS: [Computer; 3] = [
    Computer::Windows,
    Computer::Linux,
    Computer::InsideAnotherApp,
];

impl Computer {
    fn heading(self) -> String {
        match self {
            Self::Windows => "Windows".into(),
            Self::Linux => "Linux".into(),
            Self::InsideAnotherApp => {
                format!("Windows, from inside the {CONTAINER_APP} app container")
            }
        }
    }

    fn windows(self) -> bool {
        !matches!(self, Self::Linux)
    }

    /// The seeds recorded on this computer for a host whose file has `shape`.
    fn seeds(self, shape: Shape) -> Vec<Seed> {
        let all = shape.seeds();
        match self {
            Self::InsideAnotherApp => all
                .into_iter()
                .filter(|seed| matches!(seed, Seed::Absent | Seed::Other | Seed::Current))
                .collect(),
            _ => all,
        }
    }
}

/// How a host's file is written, as far as its seeds go.
#[derive(Clone, Copy)]
enum Shape {
    Json(JsonDialect),
    /// OpenCode, whose file is read before its dialect is chosen.
    OpenCode,
    Toml,
    /// A file Ghostlight never parses.
    Yaml,
}

impl Shape {
    fn of(row: &AgentRow) -> Self {
        match row.dialect {
            ConfigDialect::Json(dialect) => Self::Json(dialect),
            ConfigDialect::OpenCode => Self::OpenCode,
            ConfigDialect::CodexToml => Self::Toml,
            ConfigDialect::Manual(_) => Self::Yaml,
        }
    }

    fn seeds(self) -> Vec<Seed> {
        const PARSED: [Seed; 10] = [
            Seed::Absent,
            Seed::Empty,
            Seed::Other,
            Seed::Foreign,
            Seed::Current,
            Seed::Stale,
            Seed::LegacyRelay,
            Seed::Retired,
            Seed::Malformed,
            Seed::Located,
        ];
        let mut seeds = PARSED.to_vec();
        match self {
            Self::Json(_) => {}
            Self::OpenCode => seeds.extend([Seed::OtherNested, Seed::CurrentNested]),
            Self::Toml => seeds.push(Seed::CurrentForwarded),
            Self::Yaml => {
                seeds = vec![Seed::Absent, Seed::Empty, Seed::Other, Seed::Located];
            }
        }
        seeds
    }
}

/// One starting file.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Seed {
    Absent,
    Empty,
    Other,
    Foreign,
    Current,
    Stale,
    LegacyRelay,
    Retired,
    Malformed,
    Located,
    /// OpenCode's nested `mcp.servers` file with another server.
    OtherNested,
    /// OpenCode's nested `mcp.servers` file with the current entry.
    CurrentNested,
    /// Codex's current entry with the Linux desktop session forwarded.
    CurrentForwarded,
}

impl Seed {
    fn heading(self) -> &'static str {
        match self {
            Self::Absent => "absent: no file",
            Self::Empty => "empty: a file of no bytes",
            Self::Other => "other: comments, a setting and another server",
            Self::Foreign => "foreign: another program under Ghostlight's key",
            Self::Current => "current: the entry Ghostlight writes for this edge",
            Self::Stale => "stale: Ghostlight's entry for an edge somewhere else",
            Self::LegacyRelay => "legacy relay: the 1.x relay entry in this install",
            Self::Retired => "retired: an entry starting the retired connector",
            Self::Malformed => "malformed: a file that does not parse",
            Self::Located => "located: the other seed at a path the person chose",
            Self::OtherNested => "other nested: another server under mcp.servers",
            Self::CurrentNested => "current nested: the current entry under mcp.servers",
            Self::CurrentForwarded => "current forwarded: the current entry forwarding the desktop",
        }
    }
}

/// How a run spells the paths it builds.
#[derive(Clone, Copy)]
enum Spelling {
    /// Forward slashes on every build system, so one golden file holds on all of them.
    Portable,
    /// The build system's own separator, as Ghostlight spells a person's paths.
    #[cfg(any(windows, target_os = "linux"))]
    Native,
}

impl Spelling {
    fn separator(self) -> char {
        match self {
            Self::Portable => '/',
            #[cfg(any(windows, target_os = "linux"))]
            Self::Native => std::path::MAIN_SEPARATOR,
        }
    }
}

/// `parts` below `root`, each after `separator`.
fn spelled(root: &str, separator: char, parts: &[&str]) -> String {
    let mut path = root.to_owned();
    for part in parts {
        path.push(separator);
        path.push_str(part);
    }
    path
}

/// One fresh directory standing for a computer, removed when dropped.
struct Run {
    directory: PathBuf,
    /// The directory as text, spelled with [`Run::separator`], the prefix of every path the run
    /// builds.
    root: String,
    separator: char,
    computer: Computer,
    context: HarnessContext,
    /// Each text the golden hides and the placeholder it shows instead, replaced in this order.
    masks: Vec<(String, &'static str)>,
}

impl Run {
    /// A run whose paths are spelled with forward slashes, with the edge and the directory hidden.
    fn new(computer: Computer) -> Self {
        Self::spelled(computer, Spelling::Portable)
    }

    /// A fresh run on `computer` whose paths are spelled as `spelling` says.
    fn spelled(computer: Computer, spelling: Spelling) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "ghostlight-writer-golden-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&directory).expect("the run's directory is writable");
        let separator = spelling.separator();
        let root = directory
            .to_string_lossy()
            .replace(['\\', '/'], &separator.to_string());
        assert!(
            !root.contains(['"', '\'']),
            "the temporary directory {root} holds a quote, which the seeds cannot write as a \
             plain string"
        );
        let windows = computer.windows();
        let home = PathBuf::from(spelled(&root, separator, &[PERSON]));
        let edge_name = executable_name(EDGE_COMMAND_NAME, windows);
        let edge = PathBuf::from(spelled(
            &root,
            separator,
            &[PERSON, ".ghostlight", "bin", &edge_name],
        ));
        fs::create_dir_all(edge.parent().expect("the edge has a directory"))
            .expect("the edge's directory is writable");
        fs::write(&edge, b"edge").expect("the edge is writable");
        let (app_data, masks) = match spelling {
            Spelling::Portable => (
                None,
                vec![
                    (edge.to_string_lossy().into_owned(), EDGE_PLACEHOLDER),
                    (root.clone(), ROOT_PLACEHOLDER),
                ],
            ),
            // The directory is hidden as a JSON or TOML string escapes it and as it is, so the
            // separators and escapes of every path below it stay in the golden.
            #[cfg(any(windows, target_os = "linux"))]
            Spelling::Native => (
                windows.then(|| {
                    PathBuf::from(spelled(&root, separator, &[PERSON, "AppData", "Roaming"]))
                }),
                vec![
                    (quoted(&root).trim_matches('"').to_owned(), ROOT_PLACEHOLDER),
                    (root.clone(), ROOT_PLACEHOLDER),
                ],
            ),
        };
        let roots = harness_roots(&home, None, app_data, None, windows);
        let private_view = (computer == Computer::InsideAnotherApp).then(|| PrivateView {
            container: AppContainer {
                app: CONTAINER_APP.into(),
            },
            roots: vec![roots.roaming.clone(), home.join("AppData/Local")],
        });
        let context = HarnessContext {
            home,
            config: roots.config,
            roaming: roots.roaming,
            codex_config: roots.codex_config,
            crush_config: None,
            edge,
            path_entries: Vec::new(),
            locations: PathBuf::from(spelled(
                &root,
                separator,
                &["state", HARNESS_LOCATIONS_FILE],
            )),
            windows,
            linux: computer == Computer::Linux,
            private_view,
        };
        Self {
            directory,
            root,
            separator,
            computer,
            context,
            masks,
        }
    }

    fn edge(&self) -> String {
        self.context.edge.to_string_lossy().into_owned()
    }

    /// A program of this run's install, by name.
    fn program(&self, directory: &str, name: &str) -> String {
        let name = executable_name(name, self.computer.windows());
        self.below(&[PERSON, ".ghostlight", directory, &name])
    }

    fn old_edge(&self) -> String {
        let name = executable_name(EDGE_COMMAND_NAME, self.computer.windows());
        self.below(&[OLD_PERSON, ".ghostlight", "bin", &name])
    }

    /// The path of `parts` below the run's directory, spelled as the run spells paths.
    fn below(&self, parts: &[&str]) -> String {
        spelled(&self.root, self.separator, parts)
    }

    /// `text` with every hidden text replaced by its placeholder.
    fn masked(&self, text: &str) -> String {
        self.masks
            .iter()
            .fold(text.to_owned(), |text, (hidden, placeholder)| {
                text.replace(hidden, placeholder)
            })
    }

    /// A path as the golden shows it: masked, with forward slashes.
    fn shown(&self, path: &Path) -> String {
        self.masked(&path.to_string_lossy()).replace('\\', "/")
    }

    /// Every file below the run's directory but the edge, by the path the golden shows.
    fn files(&self) -> BTreeMap<String, Vec<u8>> {
        let mut files = BTreeMap::new();
        let mut pending = vec![self.root.clone()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory).expect("the run's directory is readable") {
                let entry = entry.expect("a directory entry is readable");
                let path = spelled(
                    &directory,
                    self.separator,
                    &[&entry.file_name().to_string_lossy()],
                );
                if entry.file_type().expect("an entry has a type").is_dir() {
                    pending.push(path);
                } else if Path::new(&path) != self.context.edge {
                    let bytes = fs::read(&path).expect("a file is readable");
                    files.insert(self.shown(Path::new(&path)), bytes);
                }
            }
        }
        files
    }

    /// Write `seed` for the host `row` and point Ghostlight at it when the seed is located. The
    /// seed's path and bytes, and the registry over the run.
    fn seeded(&self, row: &AgentRow, seed: Seed) -> (PathBuf, Option<String>, HarnessRegistry) {
        let default = definition(&self.context, row.id)
            .expect("every roster row has a definition")
            .default_path;
        let path = if seed == Seed::Located {
            let name = default
                .file_name()
                .expect("a configuration has a file name");
            PathBuf::from(self.below(&["located", &name.to_string_lossy()]))
        } else {
            default
        };
        let bytes = seed_bytes(self, Shape::of(row), seed);
        if let Some(bytes) = &bytes {
            fs::create_dir_all(path.parent().expect("a configuration has a directory"))
                .expect("the seed's directory is writable");
            fs::write(&path, bytes).expect("the seed is writable");
        }
        (
            path,
            bytes,
            HarnessRegistry::with_context(self.context.clone()),
        )
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

/// The bytes of `seed` for a host whose file has `shape`, or none for no file.
fn seed_bytes(run: &Run, shape: Shape, seed: Seed) -> Option<String> {
    let edge = run.edge();
    let legacy = run.program("v0", LEGACY_RELAY);
    let retired = run.program("bin", RETIRED_MCP_CONNECTOR_NAME);
    let old_edge = run.old_edge();
    match seed {
        Seed::Absent => return None,
        Seed::Empty => return Some(String::new()),
        Seed::Located => return seed_bytes(run, shape, Seed::Other),
        _ => {}
    }
    Some(match shape {
        Shape::Yaml => format!("# {KEPT_COMMENT}\nextensions:\n  other:\n    cmd: other-server\n"),
        Shape::Toml => match seed {
            Seed::Malformed => {
                format!("# {KEPT_COMMENT}\n[mcp_servers.other\ncommand = \"other-server\"\n")
            }
            Seed::Foreign => toml_document(Some(toml_entry("node", &["server.js"], false))),
            Seed::Current => toml_document(Some(toml_entry(&edge, &[EDGE_ARGUMENT], false))),
            Seed::CurrentForwarded => {
                toml_document(Some(toml_entry(&edge, &[EDGE_ARGUMENT], true)))
            }
            Seed::Stale => toml_document(Some(toml_entry(&old_edge, &[EDGE_ARGUMENT], false))),
            Seed::LegacyRelay => {
                toml_document(Some(toml_entry(&legacy, &LEGACY_RELAY_ARGUMENTS, false)))
            }
            Seed::Retired => toml_document(Some(toml_entry(&retired, &[], false))),
            _ => toml_document(None),
        },
        Shape::Json(_) | Shape::OpenCode => {
            let dialect = match (shape, seed) {
                (Shape::Json(dialect), _) => dialect,
                (_, Seed::OtherNested | Seed::CurrentNested) => JsonDialect::OpenCodeV2,
                _ => JsonDialect::OpenCodeV1,
            };
            let ghostlight = match seed {
                Seed::Malformed => {
                    return Some(format!(
                        "{{\n  // {KEPT_COMMENT}\n  \"theme\": \"dark\",\n  \"broken\": [\n"
                    ))
                }
                Seed::Foreign => Some(json_entry(dialect, "node", &["server.js"])),
                Seed::Current | Seed::CurrentNested => Some(json_current(dialect, &edge)),
                Seed::Stale => Some(json_current(dialect, &old_edge)),
                Seed::LegacyRelay => Some(json_entry(dialect, &legacy, &LEGACY_RELAY_ARGUMENTS)),
                Seed::Retired => Some(json_entry(dialect, &retired, &[])),
                _ => None,
            };
            json_document(dialect, ghostlight)
        }
    })
}

/// Where a JSON dialect keeps its servers, written out as a person's file holds it.
fn json_collection(dialect: JsonDialect) -> &'static [&'static str] {
    match dialect {
        JsonDialect::McpServers | JsonDialect::Copilot => &["mcpServers"],
        JsonDialect::Servers => &["servers"],
        JsonDialect::ContextServers => &["context_servers"],
        JsonDialect::Crush | JsonDialect::OpenCodeV1 => &["mcp"],
        JsonDialect::OpenCodeV2 | JsonDialect::ZCode => &["mcp", "servers"],
    }
}

/// Whether a JSON dialect puts the program and its arguments in one `command` array.
fn command_array(dialect: JsonDialect) -> bool {
    matches!(dialect, JsonDialect::OpenCodeV1 | JsonDialect::OpenCodeV2)
}

fn quoted(text: &str) -> String {
    serde_json::to_string(text).expect("a string serializes")
}

fn quoted_list(items: &[&str]) -> String {
    let items: Vec<String> = items.iter().map(|item| quoted(item)).collect();
    format!("[{}]", items.join(", "))
}

/// A plain entry that starts `command` with `args`, in `dialect`.
fn json_entry(dialect: JsonDialect, command: &str, args: &[&str]) -> String {
    if command_array(dialect) {
        let mut words = vec![command];
        words.extend_from_slice(args);
        format!(
            "{{\"type\": \"local\", \"command\": {}}}",
            quoted_list(&words)
        )
    } else {
        format!(
            "{{\"command\": {}, \"args\": {}}}",
            quoted(command),
            quoted_list(args)
        )
    }
}

/// The whole entry Ghostlight writes in `dialect` for the edge `edge`, as a file holds it.
fn json_current(dialect: JsonDialect, edge: &str) -> String {
    let edge = quoted(edge);
    let argument = quoted(EDGE_ARGUMENT);
    match dialect {
        JsonDialect::McpServers | JsonDialect::ContextServers | JsonDialect::ZCode => {
            format!("{{\"command\": {edge}, \"args\": [{argument}], \"env\": {{}}}}")
        }
        JsonDialect::Copilot => format!(
            "{{\"type\": \"local\", \"command\": {edge}, \"args\": [{argument}], \"env\": {{}}, \
             \"tools\": [\"*\"]}}"
        ),
        JsonDialect::Servers | JsonDialect::Crush => {
            format!("{{\"type\": \"stdio\", \"command\": {edge}, \"args\": [{argument}]}}")
        }
        JsonDialect::OpenCodeV1 => {
            format!("{{\"type\": \"local\", \"command\": [{edge}, {argument}], \"enabled\": true}}")
        }
        JsonDialect::OpenCodeV2 => {
            format!("{{\"type\": \"local\", \"command\": [{edge}, {argument}]}}")
        }
    }
}

/// A JSON file with comments, a trailing comma, a setting and another server, plus `ghostlight`
/// under Ghostlight's key when given.
fn json_document(dialect: JsonDialect, ghostlight: Option<String>) -> String {
    let collection = json_collection(dialect);
    let mut text = format!("{{\n  // {KEPT_COMMENT}\n  \"theme\": \"dark\",\n");
    for (depth, key) in collection.iter().enumerate() {
        let indent = "  ".repeat(depth + 1);
        let _ = writeln!(text, "{indent}{}: {{", quoted(key));
    }
    let indent = "  ".repeat(collection.len() + 1);
    let _ = writeln!(
        text,
        "{indent}// Another server.\n{indent}\"other\": {},",
        json_entry(dialect, "other-server", &["--flag"])
    );
    if let Some(entry) = ghostlight {
        let _ = writeln!(text, "{indent}\"ghostlight\": {entry},");
    }
    for depth in (0..collection.len()).rev() {
        let _ = writeln!(text, "{}}},", "  ".repeat(depth + 1));
    }
    text.push_str("}\n");
    text
}

/// A Codex entry that starts `command` with `args`, forwarding the desktop session when asked.
fn toml_entry(command: &str, args: &[&str], forwarded: bool) -> String {
    let mut entry = format!(
        "\n[mcp_servers.ghostlight]\ncommand = {}\nargs = {}\n",
        quoted(command),
        quoted_list(args)
    );
    if forwarded {
        let _ = writeln!(entry, "env_vars = {FORWARDED_LIST}");
    }
    entry
}

/// A Codex file with a comment, a setting and another server, plus `ghostlight` when given.
fn toml_document(ghostlight: Option<String>) -> String {
    format!(
        "# {KEPT_COMMENT}\nmodel = \"o3\"\n\n[mcp_servers.other]\ncommand = \"other-server\"\nargs = [\"--flag\"]\n{}",
        ghostlight.unwrap_or_default()
    )
}

/// One line of a file as the golden shows it: masked, with every byte outside printable ASCII
/// written as an escape.
fn shown_line(run: &Run, line: &str) -> String {
    let mut shown = String::new();
    for character in run.masked(line).chars() {
        match character {
            '\r' => shown.push_str("\\r"),
            '\t' => shown.push_str("\\t"),
            ' '..='~' => shown.push(character),
            other => {
                let _ = write!(shown, "\\u{{{:04x}}}", u32::from(other));
            }
        }
    }
    shown
}

/// A file's bytes, one `| ` line per line.
fn write_block(out: &mut String, run: &Run, bytes: &[u8]) {
    if bytes.is_empty() {
        out.push_str("  (no bytes)\n");
        return;
    }
    let text = std::str::from_utf8(bytes).expect("every file the golden records is UTF-8");
    for line in text.split_inclusive('\n') {
        let line = shown_line(run, line.strip_suffix('\n').unwrap_or(line));
        let gap = if line.is_empty() { "" } else { " " };
        let _ = writeln!(out, "  |{gap}{line}");
    }
    if !text.ends_with('\n') {
        out.push_str("  (no newline at end)\n");
    }
}

/// Text that may span lines, such as the manual setup document.
fn write_text(out: &mut String, run: &Run, label: &str, text: &str) {
    let _ = writeln!(out, "{label}:");
    write_block(out, run, text.as_bytes());
}

fn yes(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn state_words(state: HarnessState) -> String {
    let wire = serde_json::to_value(state).expect("a state serializes");
    format!(
        "{} ({})",
        wire.as_str().expect("a state is a word"),
        state.label()
    )
}

/// Who the host is, once per computer.
fn write_identity(out: &mut String, summary: &HarnessSummary) {
    let _ = writeln!(out, "id: {}", summary.id);
    let _ = writeln!(out, "product: {}", summary.product_id);
    let _ = writeln!(out, "name: {}", summary.name);
    let _ = writeln!(out, "label: {}", summary.label);
    let _ = writeln!(out, "target: {}", summary.target);
    // The agent's id and name are what the installer and its words use. How the window draws the
    // agent is not this golden's subject, so its symbol stays out.
    let _ = writeln!(out, "agent: {} ({})", summary.agent.id, summary.agent.name);
}

/// Everything inspection says about the host. Every field is named, so a new one cannot be left
/// out of the golden unnoticed.
fn write_summary(
    out: &mut String,
    run: &Run,
    registry: &HarnessRegistry,
    summary: &HarnessSummary,
) {
    let HarnessSummary {
        id,
        product_id: _,
        name: _,
        label: _,
        target: _,
        agent: _,
        state,
        detail,
        evidence,
        can_install,
        can_fix,
        can_uninstall,
        can_download,
        download_url,
        can_locate,
        copy_only,
        private_view,
        config_path,
        connector_command,
        manual_setup,
    } = summary;
    let _ = writeln!(out, "state: {}", state_words(*state));
    let _ = writeln!(out, "detail: {}", run.masked(detail));
    if let Some(evidence) = evidence {
        let _ = writeln!(out, "evidence: {}", run.masked(evidence));
    }
    let acts: Vec<&str> = [
        (*can_install, "install"),
        (*can_fix, "fix"),
        (*can_uninstall, "uninstall"),
        (*can_download, "download"),
        (*can_locate, "locate"),
    ]
    .into_iter()
    .filter_map(|(offered, act)| offered.then_some(act))
    .collect();
    let _ = writeln!(out, "offers: {}", acts.join(", "));
    assert_eq!(
        download_url.as_deref(),
        registry.download_url(id).ok(),
        "{id}: the page the window opens is the one the summary names"
    );
    if *can_download {
        let download = download_url.as_deref().unwrap_or("none, although offered");
        let _ = writeln!(out, "download: {download}");
    }
    let _ = writeln!(out, "copy only: {}", yes(*copy_only));
    let _ = writeln!(out, "private view: {}", yes(*private_view));
    let _ = writeln!(out, "config path: {}", run.shown(Path::new(config_path)));
    let _ = writeln!(out, "connector command: {}", run.masked(connector_command));
    write_text(out, run, "manual setup", manual_setup);
}

fn summary_of(registry: &HarnessRegistry, id: &str) -> HarnessSummary {
    registry
        .summaries()
        .into_iter()
        .find(|summary| summary.id == id)
        .expect("every roster row has a summary")
}

/// Every file that differs between `before` and `after`, the seed's bytes said by name.
fn write_changes(
    out: &mut String,
    run: &Run,
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
    seed: Option<&str>,
) {
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    let mut changed = false;
    for path in paths {
        match (before.get(path), after.get(path)) {
            (old, Some(new)) if old != Some(new) => {
                changed = true;
                if seed.is_some_and(|seed| seed.as_bytes() == new.as_slice()) {
                    let _ = writeln!(out, "wrote {path}: the seed's bytes");
                } else {
                    let _ = writeln!(out, "wrote {path}:");
                    write_block(out, run, new);
                }
            }
            (Some(_), None) => {
                changed = true;
                let _ = writeln!(out, "removed {path}");
            }
            _ => {}
        }
    }
    if !changed {
        out.push_str("wrote nothing\n");
    }
}

/// The name of an act, as the golden heads it.
fn act_name(action: HarnessAction) -> &'static str {
    match action {
        HarnessAction::Check => "check",
        HarnessAction::Install => "install",
        HarnessAction::Fix => "fix",
        HarnessAction::Uninstall => "uninstall",
    }
}

/// Apply `action` to the host `id` and record what it says and writes.
fn write_act(
    out: &mut String,
    run: &Run,
    registry: &HarnessRegistry,
    id: &str,
    action: HarnessAction,
    seed: Option<&str>,
) {
    let _ = writeln!(out, "-- {}", act_name(action));
    let before = run.files();
    match registry.apply(id, action) {
        Ok(result) => {
            let _ = writeln!(out, "changed: {}", yes(result.changed));
            let _ = writeln!(out, "says: {}", run.masked(&result.message));
            let _ = writeln!(
                out,
                "after: {} | {}",
                state_words(result.summary.state),
                run.masked(&result.summary.detail)
            );
        }
        Err(error) => {
            let _ = writeln!(out, "refused: {}", run.masked(&error.to_string()));
        }
    }
    write_changes(out, run, &before, &run.files(), seed);
}

/// Everything the golden records for one host on one computer from one seed.
fn write_seed(out: &mut String, row: &AgentRow, computer: Computer, seed: Seed) {
    let _ = writeln!(out, "\n=== {} / {}", computer.heading(), seed.heading());
    let run = Run::new(computer);
    let (path, bytes, registry) = run.seeded(row, seed);
    match &bytes {
        Some(bytes) => {
            let _ = writeln!(out, "seed {}:", run.shown(&path));
            write_block(out, &run, bytes.as_bytes());
        }
        None => {
            let _ = writeln!(out, "seed {}: no file", run.shown(&path));
        }
    }
    if seed == Seed::Located {
        out.push_str("-- locate\n");
        let before = run.files();
        match registry.locate(row.id, &path) {
            Ok(result) => {
                let _ = writeln!(out, "says: {}", run.masked(&result.message));
            }
            Err(error) => {
                let _ = writeln!(out, "refused: {}", run.masked(&error.to_string()));
            }
        }
        write_changes(out, &run, &before, &run.files(), None);
    }
    out.push_str("-- inspect\n");
    write_summary(out, &run, &registry, &summary_of(&registry, row.id));
    write_act(
        out,
        &run,
        &registry,
        row.id,
        HarnessAction::Check,
        bytes.as_deref(),
    );
    drop(run);
    for action in [
        HarnessAction::Install,
        HarnessAction::Fix,
        HarnessAction::Uninstall,
    ] {
        let run = Run::new(computer);
        let (path, bytes, registry) = run.seeded(row, seed);
        if seed == Seed::Located {
            registry
                .locate(row.id, &path)
                .expect("the located seed was accepted above");
        }
        write_act(out, &run, &registry, row.id, action, bytes.as_deref());
    }
}

/// The header every golden file opens with.
fn header(subject: &str) -> String {
    format!(
        "# What Ghostlight's installer writes and says for {subject}.\n\
         # Recorded by install::writer_golden. Rewrite it only on purpose, with\n\
         # {UPDATE_VARIABLE}=1, and read the diff line by line.\n\
         # {ROOT_PLACEHOLDER} is the run's directory and {EDGE_PLACEHOLDER} the ghostlight program in it.\n"
    )
}

/// The golden of one host: every computer, every seed, every act.
fn host_golden(row: &AgentRow) -> String {
    let mut out = header(row.id);
    for computer in COMPUTERS {
        let run = Run::new(computer);
        let registry = HarnessRegistry::with_context(run.context.clone());
        let _ = writeln!(out, "\n## {}", computer.heading());
        write_identity(&mut out, &summary_of(&registry, row.id));
        drop(run);
        for seed in computer.seeds(Shape::of(row)) {
            write_seed(&mut out, row, computer, seed);
        }
    }
    out
}

/// The seed each host starts from when everything is set up at once: another server for most,
/// and one host for each blocked or stale state.
fn setup_seed(row: &AgentRow) -> Seed {
    match row.id {
        "claude-code" => Seed::Stale,
        "cursor" => Seed::Foreign,
        "zed" => Seed::Malformed,
        _ => Seed::Other,
    }
}

/// The golden of setting up everything at once, on Windows and on Linux.
fn setup_golden() -> String {
    let mut out = header("every host at once");
    for computer in [Computer::Windows, Computer::Linux] {
        let _ = writeln!(out, "\n## {}", computer.heading());
        let run = Run::new(computer);
        for row in ROSTER {
            let seed = setup_seed(row);
            let path = definition(&run.context, row.id)
                .expect("every roster row has a definition")
                .default_path;
            let bytes = seed_bytes(&run, Shape::of(row), seed).expect("every setup seed is a file");
            fs::create_dir_all(path.parent().expect("a configuration has a directory"))
                .expect("the seed's directory is writable");
            fs::write(&path, bytes).expect("the seed is writable");
        }
        let registry = HarnessRegistry::with_context(run.context.clone());
        let before = registry.summaries();
        for pass in ["first", "second"] {
            let _ = writeln!(out, "-- set up everything, {pass} time");
            let written = run.files();
            match registry.setup_detected() {
                Ok(result) => {
                    let _ = writeln!(out, "says: {}", run.masked(&result.message));
                    let _ = writeln!(
                        out,
                        "set up: {}, updated: {}, needs attention: {}",
                        result.set_up, result.updated, result.needs_attention
                    );
                    for failure in &result.failures {
                        let _ = writeln!(
                            out,
                            "failed: {} ({}): {}",
                            failure.id,
                            failure.name,
                            run.masked(&failure.detail)
                        );
                    }
                }
                Err(error) => {
                    let _ = writeln!(out, "refused: {}", run.masked(&error.to_string()));
                }
            }
            let after = run.files();
            let changed: Vec<&String> = after
                .iter()
                .filter(|(path, bytes)| written.get(*path) != Some(*bytes))
                .map(|(path, _)| path)
                .collect();
            for path in changed {
                let _ = writeln!(out, "wrote {path}");
            }
        }
        out.push_str("-- states\n");
        let after = registry.summaries();
        for (old, new) in before.iter().zip(&after) {
            let _ = writeln!(
                out,
                "{} ({}): {} -> {}",
                old.id,
                setup_seed_name(&old.id),
                state_words(old.state),
                state_words(new.state)
            );
        }
    }
    out
}

fn setup_seed_name(id: &str) -> &'static str {
    let row = ROSTER
        .iter()
        .find(|row| row.id == id)
        .expect("every summary is a roster row");
    let heading = setup_seed(row).heading();
    heading.split(':').next().unwrap_or(heading)
}

/// The golden file of a name in the golden directory.
fn golden_file(name: &str) -> String {
    format!("{GOLDEN_DIRECTORY}/{name}")
}

#[test]
fn every_host_writes_and_says_what_the_writer_golden_records() {
    // Every host runs in directories of its own, so the hosts are recorded side by side.
    let goldens: Vec<(String, String)> = std::thread::scope(|scope| {
        let hosts: Vec<_> = ROSTER
            .iter()
            .map(|row| {
                scope.spawn(move || (format!("{}.{GOLDEN_EXTENSION}", row.id), host_golden(row)))
            })
            .collect();
        hosts
            .into_iter()
            .map(|host| host.join().expect("a host's golden is recorded"))
            .collect()
    });
    let mut expected = BTreeSet::new();
    for (name, golden) in goldens {
        check_golden_with(&golden_file(&name), &golden, UPDATE_VARIABLE, SUBJECT);
        expected.insert(name);
    }
    check_golden_with(
        &golden_file(SETUP_GOLDEN),
        &setup_golden(),
        UPDATE_VARIABLE,
        SUBJECT,
    );
    expected.insert(SETUP_GOLDEN.to_owned());

    let directory = repository_path(GOLDEN_DIRECTORY);
    let present: BTreeSet<String> = fs::read_dir(&directory)
        .expect("the golden directory exists")
        .map(|entry| {
            entry
                .expect("a golden entry is readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    if std::env::var_os(UPDATE_VARIABLE).is_some() {
        for stale in present.difference(&expected) {
            fs::remove_file(directory.join(stale)).expect("a stale golden is removable");
        }
    } else {
        assert_eq!(
            present, expected,
            "{GOLDEN_DIRECTORY} holds exactly one file per roster host and {SETUP_GOLDEN}; run \
             with {UPDATE_VARIABLE}=1 to rewrite it when the roster changes on purpose"
        );
    }
}

/// The native golden: how this build system's own paths are spelled in what the installer writes,
/// pinned in `tests/golden/agents/native/`, one file per build system.
///
/// A person's edge is a native path, so on Windows every file Ghostlight writes holds backslashes,
/// escaped as JSON or TOML escapes them. The portable golden builds every path with forward
/// slashes and so cannot see that. Here the run spells its paths with the build system's
/// separator and hides only its directory, in every form a file writes it, so the separators and
/// escapes of the edge below it stay in the file. Each build system checks its own file and leaves
/// the other alone.
#[cfg(any(windows, target_os = "linux"))]
mod native {
    use super::*;

    /// The directory of the native goldens, relative to the repository root.
    const NATIVE_DIRECTORY: &str = "tests/golden/agents/native";

    /// This build system's native golden file and the computer it records.
    #[cfg(windows)]
    const SYSTEM: (&str, Computer) = ("windows.txt", Computer::Windows);
    #[cfg(target_os = "linux")]
    const SYSTEM: (&str, Computer) = ("linux.txt", Computer::Linux);

    /// What the native golden pins, for the failure message.
    const NATIVE_SUBJECT: &str = "how the installer spells this computer's own paths";

    /// The seed every recorded host starts from: a person's file with another server in it.
    const NATIVE_SEED: Seed = Seed::Other;

    /// The way a host's file is written, or none for a file Ghostlight never writes.
    fn written_as(dialect: ConfigDialect) -> Option<String> {
        match dialect {
            ConfigDialect::Json(dialect) => Some(format!("JSON {dialect:?}")),
            ConfigDialect::OpenCode => Some("OpenCode".into()),
            ConfigDialect::CodexToml => Some("Codex TOML".into()),
            ConfigDialect::Manual(_) => None,
        }
    }

    /// The first roster host of each way a file is written, with that way's name.
    fn hosts() -> Vec<(String, &'static AgentRow)> {
        let mut hosts: Vec<(String, &'static AgentRow)> = Vec::new();
        for row in ROSTER {
            if let Some(way) = written_as(row.dialect) {
                if hosts.iter().all(|(seen, _)| *seen != way) {
                    hosts.push((way, row));
                }
            }
        }
        hosts
    }

    /// The native golden of `computer`: for each recorded host, its seed, what inspection says
    /// (the manual setup document among it) and what Install says and writes.
    fn native_golden(computer: Computer) -> String {
        let mut out = format!(
            "# How Ghostlight's installer spells this computer's own paths, on {}.\n\
             # Recorded by install::writer_golden on this build system only. Rewrite it only on\n\
             # purpose, with {UPDATE_VARIABLE}=1, and read the diff line by line.\n\
             # {ROOT_PLACEHOLDER} is the run's directory, as each file writes it; every path below it\n\
             # keeps its own separators and escapes.\n",
            computer.heading()
        );
        for (way, row) in hosts() {
            let _ = writeln!(out, "\n=== {} ({way}) / {}", row.id, NATIVE_SEED.heading());
            let run = Run::spelled(computer, Spelling::Native);
            let (path, bytes, registry) = run.seeded(row, NATIVE_SEED);
            let bytes = bytes.expect("the native seed is a file");
            let _ = writeln!(out, "seed {}:", run.shown(&path));
            write_block(&mut out, &run, bytes.as_bytes());
            out.push_str("-- inspect\n");
            write_summary(&mut out, &run, &registry, &summary_of(&registry, row.id));
            write_act(
                &mut out,
                &run,
                &registry,
                row.id,
                HarnessAction::Install,
                Some(bytes.as_str()),
            );
        }
        out
    }

    #[test]
    fn this_computer_s_own_paths_are_written_as_the_native_golden_records() {
        let (name, computer) = SYSTEM;
        check_golden_with(
            &format!("{NATIVE_DIRECTORY}/{name}"),
            &native_golden(computer),
            UPDATE_VARIABLE,
            NATIVE_SUBJECT,
        );
    }
}
