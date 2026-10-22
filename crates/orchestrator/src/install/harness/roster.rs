//! The roster: every harness Ghostlight can set up, one row each, as it stands on a
//! computer, and the agent-catalog queries the rest of the service asks of it.

use super::*;

/// The package name of the Claude desktop app, which reads the Claude Desktop configuration.
pub(super) const CLAUDE_DESKTOP_PACKAGE: &str = "Claude";
/// The package name of the Codex desktop app.
pub(super) const CODEX_PACKAGE: &str = "OpenAI.Codex";

/// One roster row as it stands on one computer: the row, and what this look resolved for it.
/// The row's fields read through it ([`std::ops::Deref`]).
#[derive(Clone)]
pub(in crate::install) struct ResolvedHarness {
    pub(in crate::install) row: &'static HarnessRow,
    /// The name a person reads: `name`, with the host added when several targets share the name.
    pub(in crate::install) label: String,
    pub(in crate::install) path: PathBuf,
    pub(in crate::install) default_path: PathBuf,
    pub(in crate::install) located_executable: Option<PathBuf>,
    pub(in crate::install) located_stale: bool,
    /// Every place this look found the agent's settings may be, the one it reads first leading
    /// ([`Packages::copies`]): the Store packages' private copies, then the plain path.
    pub(in crate::install) copies: Vec<SettingsCopy>,
}

impl std::ops::Deref for ResolvedHarness {
    type Target = HarnessRow;

    fn deref(&self) -> &HarnessRow {
        self.row
    }
}

impl ResolvedHarness {
    /// Whether an act may write this client's file from here: Ghostlight writes it, and it is not
    /// private to the app container this process runs in.
    pub(super) fn is_writable_from(&self, context: &HarnessContext) -> bool {
        !matches!(self.dialect, ConfigDialect::Manual(_)) && context.private_to(self).is_none()
    }

    /// Whether `path` is the private copy of the agent's settings kept by its own Store app, on
    /// Windows (`windows`) or another system.
    pub(super) fn packaged(&self, windows: bool) -> bool {
        self.copies.iter().any(|copy| {
            copy.place == CopyPlace::OwnPackage
                && paths::spelled_alike(&copy.file, &self.path, windows)
        })
    }

    /// The configuration files of the agent's other copies of its settings that exist: what
    /// `path` is not, but an earlier install or another package may still hold, on Windows
    /// (`windows`) or another system.
    pub(super) fn other_copies(&self, windows: bool) -> impl Iterator<Item = &Path> {
        self.copies
            .iter()
            .filter(move |copy| {
                !paths::spelled_alike(&copy.file, &self.path, windows) && copy.file.is_file()
            })
            .map(|copy| copy.file.as_path())
    }

    pub(super) fn accepts_config_path(&self, path: &Path) -> bool {
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            return false;
        };
        self.default_path.file_name() == Some(OsStr::new(name)) || self.candidates.contains(&name)
    }
}

/// One harness: one agent Ghostlight can set up, on one host. It says who the agent is, how the
/// window shows it (its monogram, and where the app's own icon may be found), and where its MCP
/// configuration lives on a computer. The roster is the one record of all of it, and the catalog
/// of agents beside the command line and an unknown agent ([`crate::agent`]).
pub(in crate::install) struct HarnessRow {
    /// The harness id: one per host the agent runs on.
    pub(in crate::install) id: &'static str,
    /// The agent's catalog id: one per agent, shared by every host it runs on, and the id every
    /// place the window names the agent carries ([`Agent::id`]). It is the catalog's own, never
    /// the product id, which two agents can share.
    pub(in crate::install) agent_id: &'static str,
    /// The product id, shared by every host of one product.
    pub(in crate::install) product_id: &'static str,
    /// The product name, as the install summary says it.
    pub(in crate::install) name: &'static str,
    /// The names the agent's client is known to report for itself on an MCP connection besides
    /// its harness id and product name.
    pub(in crate::install) aliases: &'static [&'static str],
    pub(in crate::install) target: &'static str,
    /// The agent's packaged monogram tile, the image the window serves for it when no better one
    /// is found ([`crate::agent::image`]): the same for every host of one agent.
    pub(in crate::install) monogram: Monogram,
    pub(in crate::install) download_url: Option<&'static str>,
    /// Where the configuration file is on the computer a context describes when the agent has
    /// none yet: the file a new configuration is written to.
    pub(in crate::install) path: fn(&HarnessContext) -> PathBuf,
    /// The file names the agent reads beside `path`, in the order it prefers them, when it reads
    /// more than one: the first that exists is its file. Empty when it reads only `path`.
    pub(in crate::install) candidates: &'static [&'static str],
    pub(in crate::install) executables: &'static [&'static str],
    pub(in crate::install) dialect: ConfigDialect,
    /// The package name of the packaged app that reads this configuration, when the client is one.
    pub(in crate::install) own_package: Option<&'static str>,
    /// Where this host's own app icon may be found, in order ([`icons`]). Empty when the host has
    /// no app of its own or none is known with confidence.
    pub(in crate::install) app_places: &'static [AppPlace],
    /// The catalog id of the agent whose icon stands for this one when none of its own is found:
    /// a terminal tool wears its vendor's desktop app's icon. `None` shows the agent's monogram
    /// instead.
    pub(in crate::install) icon_family: Option<&'static str>,
    /// The catalog ids of the agents whose apps this host is known to run inside: Claude Code
    /// inside Claude Desktop, a Cline extension inside its editor. A connection is said to be the
    /// agent "in" an app only when this list names that app ([`runs_inside`]). Empty when the host
    /// runs on its own or no host app is known with confidence.
    pub(in crate::install) runs_inside: &'static [&'static str],
}

/// Where every Cline host keeps its MCP settings below its own storage directory.
pub(super) const CLINE_SETTINGS: &str = "settings/cline_mcp_settings.json";

/// The id of the Cline extension, which names its storage directory in every editor.
pub(super) const CLINE_EXTENSION: &str = "saoudrizwan.claude-dev";

/// Where Cline's installation guide is, for the editors whose own store does not list it.
pub(super) const CLINE_INSTALL_URL: &str =
    "https://docs.cline.bot/getting-started/installing-cline";

/// Cline's settings file inside `editor`'s configuration, written as one join so the path keeps
/// the separators the editors' own paths use.
pub(super) fn cline_settings(context: &HarnessContext, editor: &str) -> PathBuf {
    context
        .config_root
        .join(format!("{editor}/User/globalStorage/{CLINE_EXTENSION}"))
        .join(CLINE_SETTINGS)
}

/// The files OpenCode reads in its configuration directory, in the order it prefers them.
pub(super) const OPENCODE_FILES: &[&str] = &["opencode.jsonc", "opencode.json"];

/// The files Kilo Code reads in its configuration directory, in the order it prefers them: its
/// own, then OpenCode's.
pub(super) const KILO_FILES: &[&str] =
    &["kilo.jsonc", "kilo.json", "opencode.jsonc", "opencode.json"];

/// Where the Codex desktop app's icon may be found: its Store package. The Codex CLI shares the
/// agent, so it wears the app's icon.
pub(super) const CODEX_APP_PLACES: &[AppPlace] = &[AppPlace::WindowsPackage(CODEX_PACKAGE)];

/// Where Claude Desktop's icon may be found: its Store package, then the per-user installer's
/// program.
pub(super) const CLAUDE_DESKTOP_APP_PLACES: &[AppPlace] = &[
    AppPlace::WindowsPackage(CLAUDE_DESKTOP_PACKAGE),
    AppPlace::WindowsExe(&[ProgramPlace::Below(
        KnownFolder::LocalAppData,
        "AnthropicClaude/claude.exe",
    )]),
];

/// Folders a catalog agent's own programs live in that none of its icon sources names, by agent id.
/// The Claude desktop app downloads Claude Code into
/// `%APPDATA%\Claude\claude-code\<version>\<build>\claude.exe`, which lies in no folder of the
/// desktop app's own, so without this entry the caller walk passed it and stopped at the desktop
/// app's `claude.exe` (ADR-0106, amendment of 2026-10-21). The desktop app from the Microsoft Store
/// keeps it in `%LOCALAPPDATA%\Claude-Data\claude_pzs8sxrjxfjjc\claude-code`, its package family's
/// data folder, found on leo-desktop-02 by the 2.0.1 candidate check.
pub(super) const AGENT_APP_FOLDERS: &[(&str, KnownFolder, &str)] = &[
    (
        "claude-code",
        KnownFolder::RoamingAppData,
        "Claude/claude-code",
    ),
    (
        "claude-code",
        KnownFolder::LocalAppData,
        "Claude-Data/claude_pzs8sxrjxfjjc/claude-code",
    ),
];

/// Where Cursor's icon may be found: its per-user install, the launcher on `PATH`, and its Linux
/// desktop entry.
pub(super) const CURSOR_APP_PLACES: &[AppPlace] = &[
    AppPlace::WindowsExe(&[
        ProgramPlace::Below(KnownFolder::LocalAppData, "Programs/cursor/Cursor.exe"),
        ProgramPlace::OnPath("cursor"),
    ]),
    AppPlace::LinuxDesktop(&["cursor"]),
];

/// Where Visual Studio Code's icon may be found: the program its `code` launcher starts, its
/// per-user and machine-wide installs, and its desktop entry from a package, the Snap or the
/// Flatpak.
pub(super) const VSCODE_APP_PLACES: &[AppPlace] = &[
    AppPlace::WindowsExe(&[
        ProgramPlace::OnPath("code"),
        ProgramPlace::Below(
            KnownFolder::LocalAppData,
            "Programs/Microsoft VS Code/Code.exe",
        ),
        ProgramPlace::Below(KnownFolder::ProgramFiles, "Microsoft VS Code/Code.exe"),
    ]),
    AppPlace::LinuxDesktop(&["code", "code_code", "com.visualstudio.code"]),
];

/// Where Windsurf's icon may be found: its per-user install, the launcher on `PATH`, and its
/// Linux desktop entry.
pub(super) const WINDSURF_APP_PLACES: &[AppPlace] = &[
    AppPlace::WindowsExe(&[
        ProgramPlace::Below(KnownFolder::LocalAppData, "Programs/Windsurf/Windsurf.exe"),
        ProgramPlace::OnPath("windsurf"),
    ]),
    AppPlace::LinuxDesktop(&["windsurf"]),
];

/// Where Zed's icon may be found: its per-user install, the program on `PATH`, and its Linux
/// desktop entry.
pub(super) const ZED_APP_PLACES: &[AppPlace] = &[
    AppPlace::WindowsExe(&[
        ProgramPlace::Below(KnownFolder::LocalAppData, "Programs/Zed/Zed.exe"),
        ProgramPlace::OnPath("zed"),
    ]),
    AppPlace::LinuxDesktop(&["dev.zed.Zed", "zed"]),
];

/// Where Kiro's icon may be found: its per-user install, the launcher on `PATH`, and its Linux
/// desktop entry.
pub(super) const KIRO_APP_PLACES: &[AppPlace] = &[
    AppPlace::WindowsExe(&[
        ProgramPlace::Below(KnownFolder::LocalAppData, "Programs/Kiro/Kiro.exe"),
        ProgramPlace::OnPath("kiro"),
    ]),
    AppPlace::LinuxDesktop(&["kiro"]),
];

/// Where Antigravity's icon may be found: its per-user install and its Linux desktop entry.
pub(super) const ANTIGRAVITY_APP_PLACES: &[AppPlace] = &[
    AppPlace::WindowsExe(&[ProgramPlace::Below(
        KnownFolder::LocalAppData,
        "Programs/Antigravity/Antigravity.exe",
    )]),
    AppPlace::LinuxDesktop(&["antigravity"]),
];

/// Every agent Ghostlight can set up, one row per host.
pub(in crate::install) const ROSTER: &[HarnessRow] = &[
    HarnessRow {
        aliases: &["codex-mcp-client"],
        download_url: Some("https://developers.openai.com/codex/cli/"),
        executables: &["codex"],
        app_places: CODEX_APP_PLACES,
        ..HarnessRow::new(
            "codex",
            "Codex",
            "User",
            monogram!("codex.svg"),
            |context| context.codex_config.clone(),
            ConfigDialect::CodexToml,
        )
    },
    HarnessRow {
        download_url: Some("https://claude.com/product/claude-code"),
        executables: &["claude"],
        icon_family: Some("claude-desktop"),
        runs_inside: &["claude-desktop"],
        ..HarnessRow::new(
            "claude-code",
            "Claude Code",
            "User",
            monogram!("claude-code.svg"),
            |context| {
                context
                    .claude_config_dir
                    .as_ref()
                    .unwrap_or(&context.home)
                    .join(CLAUDE_CODE_FILE)
            },
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        aliases: &["claude-ai"],
        own_package: Some(CLAUDE_DESKTOP_PACKAGE),
        app_places: CLAUDE_DESKTOP_APP_PLACES,
        ..HarnessRow::new(
            "claude-desktop",
            "Claude Desktop",
            "User",
            monogram!("claude-desktop.svg"),
            |context| {
                context
                    .config_root
                    .join("Claude/claude_desktop_config.json")
            },
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        aliases: &["cursor-vscode"],
        download_url: Some("https://cursor.com/downloads"),
        executables: &["cursor", "cursor-agent"],
        app_places: CURSOR_APP_PLACES,
        ..HarnessRow::new(
            "cursor",
            "Cursor",
            "User",
            monogram!("cursor.svg"),
            |context| context.home.join(".cursor/mcp.json"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        download_url: Some("https://code.visualstudio.com/download"),
        executables: &["code"],
        app_places: VSCODE_APP_PLACES,
        ..HarnessRow::new(
            "vscode",
            "Visual Studio Code",
            "User",
            monogram!("vscode.svg"),
            |context| context.config_root.join("Code/User/mcp.json"),
            ConfigDialect::Json(JsonDialect::Servers),
        )
    },
    HarnessRow {
        download_url: Some("https://windsurf.com/editor/download"),
        executables: &["windsurf"],
        app_places: WINDSURF_APP_PLACES,
        ..HarnessRow::new(
            "windsurf",
            "Windsurf",
            "User",
            monogram!("windsurf.svg"),
            |context| context.home.join(".codeium/windsurf/mcp_config.json"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        product_id: "windsurf",
        download_url: Some("https://windsurf.com/editor/download"),
        executables: &["devin", "devin-desktop", "surf"],
        ..HarnessRow::new(
            "devin",
            "Devin",
            "Devin user",
            monogram!("windsurf.svg"),
            |context| context.config_root.join("devin/mcp_config.json"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        download_url: Some("https://zed.dev/download"),
        executables: &["zed", "zeditor"],
        app_places: ZED_APP_PLACES,
        ..HarnessRow::new(
            "zed",
            "Zed",
            "User",
            monogram!("zed.svg"),
            |context| {
                if context.windows {
                    context.config_root.join("Zed/settings.json")
                } else {
                    context.config_root.join("zed/settings.json")
                }
            },
            ConfigDialect::Json(JsonDialect::ContextServers),
        )
    },
    HarnessRow {
        executables: &["zcode"],
        ..HarnessRow::new(
            "zcode",
            "ZCode",
            "User",
            monogram!("zcode.svg"),
            |context| context.home.join(".zcode/cli/config.json"),
            ConfigDialect::Json(JsonDialect::ZCode),
        )
    },
    HarnessRow {
        download_url: Some("https://opencode.ai/docs"),
        candidates: OPENCODE_FILES,
        executables: &["opencode", "opencode2"],
        ..HarnessRow::new(
            "opencode",
            "OpenCode",
            "User",
            monogram!("opencode.svg"),
            |context| context.home.join(".config/opencode/opencode.json"),
            ConfigDialect::OpenCode,
        )
    },
    HarnessRow {
        download_url: Some("https://github.com/charmbracelet/crush"),
        executables: &["crush"],
        ..HarnessRow::new(
            "crush",
            "Crush",
            "User",
            monogram!("crush.svg"),
            |context| {
                context
                    .crush_config
                    .clone()
                    .unwrap_or_else(|| context.home.join(".config/crush/crush.json"))
            },
            ConfigDialect::Json(JsonDialect::Crush),
        )
    },
    HarnessRow {
        download_url: Some("https://github.com/github/copilot-cli"),
        executables: &["copilot"],
        ..HarnessRow::new(
            "copilot-cli",
            "GitHub Copilot CLI",
            "CLI",
            monogram!("copilot-cli.svg"),
            |context| context.home.join(".copilot/mcp-config.json"),
            ConfigDialect::Json(JsonDialect::Copilot),
        )
    },
    HarnessRow {
        agent_id: "cline",
        product_id: "cline",
        download_url: Some("https://docs.cline.bot/cline-cli/overview"),
        executables: &["cline"],
        ..HarnessRow::new(
            "cline-cli",
            "Cline",
            "CLI",
            monogram!("cline.svg"),
            |context| context.home.join(".cline/data").join(CLINE_SETTINGS),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        agent_id: "cline",
        product_id: "cline",
        download_url: Some(
            "https://marketplace.visualstudio.com/items?itemName=saoudrizwan.claude-dev",
        ),
        runs_inside: &["vscode"],
        ..HarnessRow::new(
            "cline-vscode",
            "Cline",
            "Visual Studio Code",
            monogram!("cline.svg"),
            |context| cline_settings(context, "Code"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        agent_id: "cline",
        product_id: "cline",
        download_url: Some(CLINE_INSTALL_URL),
        runs_inside: &["cursor"],
        ..HarnessRow::new(
            "cline-cursor",
            "Cline",
            "Cursor",
            monogram!("cline.svg"),
            |context| cline_settings(context, "Cursor"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        agent_id: "cline",
        product_id: "cline",
        download_url: Some(CLINE_INSTALL_URL),
        runs_inside: &["windsurf"],
        ..HarnessRow::new(
            "cline-windsurf",
            "Cline",
            "Windsurf",
            monogram!("cline.svg"),
            |context| cline_settings(context, "Windsurf"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        download_url: Some("https://kiro.dev/downloads/"),
        executables: &["kiro", "kiro-cli"],
        app_places: KIRO_APP_PLACES,
        ..HarnessRow::new(
            "kiro",
            "Kiro",
            "CLI and IDE",
            monogram!("kiro.svg"),
            |context| context.home.join(".kiro/settings/mcp.json"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        download_url: Some("https://qwenlm.github.io/qwen-code-docs/en/users/installation/"),
        executables: &["qwen"],
        ..HarnessRow::new(
            "qwen-code",
            "Qwen Code",
            "CLI",
            monogram!("qwen-code.svg"),
            |context| context.home.join(".qwen/settings.json"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        download_url: Some("https://junie.jetbrains.com/docs/get-started-with-junie-cli.html"),
        executables: &["junie"],
        ..HarnessRow::new(
            "junie",
            "Junie",
            "CLI and JetBrains",
            monogram!("junie.svg"),
            |context| context.home.join(".junie/mcp/mcp.json"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
    HarnessRow {
        download_url: Some("https://kilo.ai/"),
        candidates: KILO_FILES,
        executables: &["kilo"],
        ..HarnessRow::new(
            "kilo-code",
            "Kilo Code",
            "CLI",
            monogram!("kilo-code.svg"),
            |context| context.config_root.join("kilo").join("kilo.json"),
            ConfigDialect::Json(JsonDialect::OpenCodeV1),
        )
    },
    HarnessRow {
        download_url: Some("https://block.github.io/goose/docs/getting-started/installation/"),
        executables: &["goose"],
        ..HarnessRow::new(
            "goose",
            "goose",
            "CLI and desktop",
            monogram!("goose.svg"),
            |context| context.config_root.join("goose/config.yaml"),
            ConfigDialect::Manual(ManualDialect::Goose),
        )
    },
    HarnessRow {
        download_url: Some("https://docs.continue.dev/cli/quickstart"),
        executables: &["cn"],
        ..HarnessRow::new(
            "continue",
            "Continue",
            "CLI and IDE",
            monogram!("continue.svg"),
            |context| context.home.join(".continue/config.yaml"),
            ConfigDialect::Manual(ManualDialect::Continue),
        )
    },
    HarnessRow {
        download_url: Some("https://antigravity.google/download?platform=linux"),
        executables: &["agy"],
        app_places: ANTIGRAVITY_APP_PLACES,
        ..HarnessRow::new(
            "antigravity",
            "Antigravity",
            "CLI",
            monogram!("antigravity.svg"),
            |context| context.home.join(".gemini/config/mcp_config.json"),
            ConfigDialect::Json(JsonDialect::McpServers),
        )
    },
];

impl HarnessRow {
    /// The row of agent `id` on one host, named `name` there: the agent and its product share the
    /// id, and it has no aliases, candidates, programs, package, icons or host app. A row states
    /// what differs from this with struct update.
    pub(super) const fn new(
        id: &'static str,
        name: &'static str,
        target: &'static str,
        monogram: Monogram,
        path: fn(&HarnessContext) -> PathBuf,
        dialect: ConfigDialect,
    ) -> Self {
        Self {
            id,
            agent_id: id,
            product_id: id,
            name,
            aliases: &[],
            target,
            monogram,
            download_url: None,
            path,
            candidates: &[],
            executables: &[],
            dialect,
            own_package: None,
            app_places: &[],
            icon_family: None,
            runs_inside: &[],
        }
    }

    /// Whether the agent's client calls itself `reported`: its catalog id, its harness id, its
    /// product name or one of its aliases, ignoring case. The product id is not matched, because
    /// two products can share one: Devin's product id is Windsurf's id.
    pub(super) fn is_called(&self, reported: &str) -> bool {
        [self.agent_id, self.id, self.name]
            .iter()
            .chain(self.aliases)
            .any(|known| known.eq_ignore_ascii_case(reported))
    }

    /// The agent of this row, as the window names and shows it: the same for every host of one
    /// agent.
    pub(super) fn agent(&self) -> Agent {
        Agent::listed(self.agent_id, self.name, self.monogram)
    }

    /// The names of the Store packages of this host's own app: its packaged app, and the
    /// packages its icon sources name.
    pub(super) fn store_packages(&self) -> Vec<&'static str> {
        self.own_package
            .into_iter()
            .chain(self.app_places.iter().filter_map(|source| match source {
                AppPlace::WindowsPackage(name) => Some(*name),
                AppPlace::WindowsExe(_) | AppPlace::LinuxDesktop(_) => None,
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
    pub(super) fn copies_on(
        &self,
        context: &HarnessContext,
        packages: &Packages,
    ) -> Vec<SettingsCopy> {
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
    pub(in crate::install) fn on(
        &'static self,
        context: &HarnessContext,
        packages: &Packages,
    ) -> ResolvedHarness {
        let copies = self.copies_on(context, packages);
        let path = copies
            .first()
            .map_or_else(|| (self.path)(context), |copy| copy.file.clone());
        ResolvedHarness {
            row: self,
            label: self.name.to_owned(),
            default_path: path.clone(),
            path,
            located_executable: None,
            located_stale: false,
            copies,
        }
    }
}

/// Every roster row as this look finds it: the Store packages are listed once for all of them.
pub(super) fn definitions(context: &HarnessContext) -> Vec<ResolvedHarness> {
    let packages = context.packages();
    let mut rows: Vec<ResolvedHarness> = ROSTER
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
        .map(HarnessRow::agent)
}

/// The catalog agent whose catalog id is `agent_id`, as the window names and shows it; none for an
/// id outside the catalog.
#[must_use]
pub fn catalog_agent(agent_id: &str) -> Option<Agent> {
    ROSTER
        .iter()
        .find(|row| row.agent_id == agent_id)
        .map(HarnessRow::agent)
}

/// Whether the catalog knows that the agent `agent_id` runs inside the app of the agent `host_id`
/// on one of its hosts ([`HarnessRow::runs_inside`]). False for any id outside the catalog.
#[must_use]
pub fn runs_inside(agent_id: &str, app_id: &str) -> bool {
    ROSTER
        .iter()
        .filter(|row| row.agent_id == agent_id)
        .any(|row| row.runs_inside.contains(&app_id))
}

/// Whether the roster row `harness_id`, one host of an agent, is known to run inside the app of
/// the agent `host_id` ([`HarnessRow::runs_inside`]). False for any id outside the roster.
#[must_use]
pub fn host_runs_inside(harness_id: &str, app_id: &str) -> bool {
    ROSTER
        .iter()
        .any(|row| row.id == harness_id && row.runs_inside.contains(&app_id))
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

/// One catalog agent's app as the roster gives it: every host's install places and commands in
/// roster order, and the first icon family a host names.
pub(super) struct AgentApp {
    pub(super) agent_id: &'static str,
    pub(super) name: &'static str,
    pub(super) places: Vec<AppPlace>,
    pub(super) commands: Vec<&'static str>,
    pub(super) family: Option<&'static str>,
}

/// The roster grouped by agent, one entry per agent in roster order.
pub(super) fn agent_apps() -> Vec<AgentApp> {
    let mut apps: Vec<AgentApp> = Vec::new();
    for row in ROSTER {
        if let Some(app) = apps.iter_mut().find(|app| app.agent_id == row.agent_id) {
            app.places.extend_from_slice(row.app_places);
            app.commands.extend_from_slice(row.executables);
            app.family = app.family.or(row.icon_family);
        } else {
            apps.push(AgentApp {
                agent_id: row.agent_id,
                name: row.name,
                places: row.app_places.to_vec(),
                commands: row.executables.to_vec(),
                family: row.icon_family,
            });
        }
    }
    apps
}

/// Where each catalog agent's own icon may be found, one entry per agent in roster order: its name,
/// its hosts' app places in roster order, and the first family a host names.
#[must_use]
pub fn icon_catalog() -> Vec<AgentIconSources> {
    agent_apps()
        .into_iter()
        .map(|app| AgentIconSources {
            agent_id: app.agent_id,
            name: app.name,
            sources: app.places,
            family: app.family,
        })
        .collect()
}

/// Where each catalog agent's app may be installed, one entry per agent in roster order: its
/// hosts' app places and the commands they are started by, in roster order ([`apps`]).
#[must_use]
pub fn app_catalog() -> Vec<apps::AgentAppSources> {
    agent_apps()
        .into_iter()
        .map(|app| apps::AgentAppSources {
            agent_id: app.agent_id,
            sources: app.places,
            commands: app.commands,
            folders: AGENT_APP_FOLDERS
                .iter()
                .filter(|(agent_id, ..)| *agent_id == app.agent_id)
                .map(|(_, known, relative)| (*known, *relative))
                .collect(),
        })
        .collect()
}
