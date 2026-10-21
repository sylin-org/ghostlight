//! The agent list of the installation guide, generated from the roster.
//!
//! The roster is the one catalog of agents. The guide's table names every agent, each host it runs
//! on, the entry Ghostlight keeps in that host's configuration, and where the configuration lives
//! on Windows and on Linux: a variable that names it first, then the files the agent reads, in the
//! order it prefers them. The table sits between [`BEGIN`] and [`END`] in [`GUIDE`]. The test
//! fails when the guide differs from the roster, and rewrites the table when [`UPDATE_VARIABLE`]
//! is set.

use std::path::{Path, PathBuf};

use serde_json::Value;
use toml_edit::{DocumentMut, Table};

use super::user_dirs::{APP_DATA_VARIABLE, USER_PROFILE_VARIABLE, XDG_CONFIG_HOME_VARIABLE};
use super::{
    harness_roots, manual_json_setup, manual_setup, AgentRow, ConfigDialect, HarnessContext,
    JsonDialect, Packages, CLAUDE_CONFIG_VARIABLE, CODEX_HOME_VARIABLE, CRUSH_CONFIG_VARIABLE,
    ROSTER, SERVER_NAME,
};
use crate::language::self_check::golden::{check_golden_with, repository_path};

/// The environment variable that rewrites the guide's table instead of comparing it.
const UPDATE_VARIABLE: &str = "GHOSTLIGHT_AGENT_LIST_UPDATE";

/// What the table pins, for the failure message.
const SUBJECT: &str = "the agent catalog";

/// The guide that holds the table, relative to the repository root.
const GUIDE: &str = "docs/guides/installation.md";

/// The line before the table.
const BEGIN: &str = "<!-- BEGIN agent list -->";

/// The line after the table.
const END: &str = "<!-- END agent list -->";

/// How the table shows the person's home on Linux.
const LINUX_HOME: &str = "~";

/// The variables that name one agent's own file or directory, in place of its default.
const OVERRIDES: [&str; 3] = [
    CLAUDE_CONFIG_VARIABLE,
    CODEX_HOME_VARIABLE,
    CRUSH_CONFIG_VARIABLE,
];

/// The format of a JSON configuration; Ghostlight reads it as JSONC.
const JSON: &str = "JSON";

/// The format of Codex's configuration.
const TOML: &str = "TOML";

/// The entry of an agent whose file Ghostlight never edits: the person pastes the YAML entry.
const COPY_SETUP: &str = "Copy setup (YAML)";

/// A computer the table describes.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Platform {
    Windows,
    Linux,
}

impl Platform {
    fn windows(self) -> bool {
        self == Self::Windows
    }

    fn separator(self) -> char {
        if self.windows() {
            '\\'
        } else {
            '/'
        }
    }

    /// The environment variable `name` as a person writes it on this computer.
    fn variable(self, name: &str) -> String {
        if self.windows() {
            format!("%{name}%")
        } else {
            format!("${name}")
        }
    }

    fn home(self) -> PathBuf {
        if self.windows() {
            PathBuf::from(self.variable(USER_PROFILE_VARIABLE))
        } else {
            PathBuf::from(LINUX_HOME)
        }
    }

    /// The configuration root's variable on this computer: `APPDATA` on Windows and
    /// `XDG_CONFIG_HOME` on Linux.
    fn config_variable(self) -> &'static str {
        if self.windows() {
            APP_DATA_VARIABLE
        } else {
            XDG_CONFIG_HOME_VARIABLE
        }
    }

    /// A context whose roots are the variables a person reads. With `overrides`, the variables
    /// that name one agent's own file or directory are set too.
    fn context(self, overrides: bool) -> HarnessContext {
        let windows = self.windows();
        let home = self.home();
        let config = Some(PathBuf::from(self.variable(self.config_variable())));
        let (xdg_config_home, app_data) = if windows {
            (None, config)
        } else {
            (config, None)
        };
        let codex_home = overrides.then(|| PathBuf::from(self.variable(CODEX_HOME_VARIABLE)));
        let roots = harness_roots(&home, xdg_config_home, app_data, codex_home, windows);
        HarnessContext {
            home,
            config_root: roots.config_root,
            codex_config: roots.codex_config,
            claude_config_dir: overrides
                .then(|| PathBuf::from(self.variable(CLAUDE_CONFIG_VARIABLE))),
            crush_config: overrides.then(|| PathBuf::from(self.variable(CRUSH_CONFIG_VARIABLE))),
            edge: PathBuf::new(),
            path_entries: Vec::new(),
            locations: PathBuf::new(),
            windows,
            linux: !windows,
            local_app_data: None,
            private_view: None,
        }
    }

    /// A path as a person writes it on this computer.
    fn shown(self, path: &Path) -> String {
        path.to_string_lossy()
            .chars()
            .map(|character| {
                if matches!(character, '/' | '\\') {
                    self.separator()
                } else {
                    character
                }
            })
            .collect()
    }

    /// Where the configuration root is when its variable is not set.
    fn unset_config(self) -> String {
        let roots = harness_roots(&self.home(), None, None, None, self.windows());
        self.shown(&roots.config_root)
    }
}

/// Where the configuration of `row` lives on `platform`: the file a variable names when it is
/// set, then the files the agent reads.
fn location(row: &AgentRow, platform: Platform) -> String {
    let unset = platform.context(false);
    let found = files(row, &unset, platform);
    let named = (row.path)(&platform.context(true));
    if named == (row.path)(&unset) {
        return found;
    }
    let named = platform.shown(&named);
    let variable = OVERRIDES
        .into_iter()
        .find(|name| named.starts_with(&platform.variable(name)))
        .expect("a path that changes with a variable starts with it");
    format!("`{named}` when {variable} is set, else {found}")
}

/// The files `row` reads on the computer `context` describes, in the order it prefers them, and
/// the one a new configuration is written to.
fn files(row: &AgentRow, context: &HarnessContext, platform: Platform) -> String {
    let default = (row.path)(context);
    if row.candidates.is_empty() {
        return format!("`{}`", platform.shown(&default));
    }
    let directory = default.parent().expect("a configuration has a directory");
    let new = default
        .file_name()
        .expect("a configuration has a file name")
        .to_string_lossy();
    let names: Vec<String> = row
        .candidates
        .iter()
        .map(|name| format!("`{name}`"))
        .collect();
    format!(
        "`{}{}`: the first found of {}; else a new `{new}`",
        platform.shown(directory),
        platform.separator(),
        names.join(", ")
    )
}

/// The entry Ghostlight keeps for `row`, by the path to it in the file.
fn entry(row: &AgentRow) -> String {
    match row.dialect {
        ConfigDialect::Json(dialect) => format!("{} ({JSON})", json_key(dialect)),
        ConfigDialect::OpenCode => format!(
            "{} or {} ({JSON})",
            json_key(JsonDialect::OpenCodeV1),
            json_key(JsonDialect::OpenCodeV2)
        ),
        ConfigDialect::CodexToml => format!("{} ({TOML})", toml_key(row)),
        ConfigDialect::Manual(_) => COPY_SETUP.into(),
    }
}

/// The path to Ghostlight's entry in the JSON document a person would paste for `dialect`.
fn json_key(dialect: JsonDialect) -> String {
    let document = manual_json_setup(Path::new(SERVER_NAME), dialect)
        .expect("every JSON dialect has a setup document");
    let document: Value = serde_json::from_str(&document).expect("the setup document is JSON");
    dotted(&json_path(&document).expect("the setup document holds Ghostlight's entry"))
}

fn json_path(value: &Value) -> Option<Vec<String>> {
    for (key, child) in value.as_object()? {
        if key == SERVER_NAME {
            return Some(vec![key.clone()]);
        }
        if let Some(mut path) = json_path(child) {
            path.insert(0, key.clone());
            return Some(path);
        }
    }
    None
}

/// The path to Ghostlight's entry in the TOML document a person would paste for `row`.
fn toml_key(row: &AgentRow) -> String {
    let context = Platform::Linux.context(false);
    let document = manual_setup(&context, &row.on(&context, &Packages::none()))
        .expect("a TOML dialect has a setup document");
    let document: DocumentMut = document.parse().expect("the setup document is TOML");
    dotted(&toml_path(document.as_table()).expect("the setup document holds Ghostlight's entry"))
}

fn toml_path(table: &Table) -> Option<Vec<String>> {
    for (key, item) in table {
        if key == SERVER_NAME {
            return Some(vec![key.to_owned()]);
        }
        if let Some(mut path) = item.as_table().and_then(toml_path) {
            path.insert(0, key.to_owned());
            return Some(path);
        }
    }
    None
}

fn dotted(path: &[String]) -> String {
    format!("`{}`", path.join("."))
}

/// The agent's name, with its catalog id when no host of the roster has that id.
fn agent(row: &AgentRow) -> String {
    if ROSTER.iter().any(|other| other.id == row.agent_id) {
        row.name.to_owned()
    } else {
        format!("{} (`{}`)", row.name, row.agent_id)
    }
}

/// The table, from the line after [`BEGIN`] to the line before [`END`].
fn table() -> String {
    let mut agents: Vec<&str> = ROSTER.iter().map(|row| row.agent_id).collect();
    agents.sort_unstable();
    agents.dedup();
    let mut out = format!(
        "<!-- Generated from the agent catalog by install::agent_list. Rewrite it with \
         {UPDATE_VARIABLE}=1 cargo test -p ghostlight-service agent_list -->\n\n\
         The catalog holds {} agents on {} hosts.\n\n\
         | Agent | Host id | Target | Entry | Windows | Linux |\n\
         | --- | --- | --- | --- | --- | --- |\n",
        agents.len(),
        ROSTER.len()
    );
    for row in ROSTER {
        out.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} |\n",
            agent(row),
            row.id,
            row.target,
            entry(row),
            location(row, Platform::Windows),
            location(row, Platform::Linux)
        ));
    }
    out.push_str(&format!(
        "\nWhere {} is not set it is `{}`, and where {} is not set it is `{}`.\n",
        Platform::Linux.config_variable(),
        Platform::Linux.unset_config(),
        Platform::Windows.config_variable(),
        Platform::Windows.unset_config()
    ));
    out
}

#[test]
fn the_installation_guide_lists_every_agent_of_the_catalog() {
    let guide = std::fs::read_to_string(repository_path(GUIDE)).expect("the guide is readable");
    let (before, rest) = guide
        .split_once(&format!("{BEGIN}\n"))
        .expect("the guide marks where the agent list begins");
    let (_, after) = rest
        .split_once(&format!("{END}\n"))
        .expect("the guide marks where the agent list ends");
    let actual = format!("{before}{BEGIN}\n{}{END}\n{after}", table());
    check_golden_with(GUIDE, &actual, UPDATE_VARIABLE, SUBJECT);
}
