//! CLI argument parsing and launch mode dispatch resolution.
//!
//! This module parses command-line arguments into typed launch modes without
//! performing execution or environmental mutation.

use std::ffi::OsString;

/// Every subcommand a person is offered, in help order.
///
/// This is the list the shell completions must match. `native-host` is deliberately absent: it is
/// the package-facing registration seam, not something to suggest at a prompt.
pub const SUBCOMMANDS: &[&str] = &[
    "open",
    "install",
    "uninstall",
    "doctor",
    "status",
    "call",
    "diagnostics",
    "policy",
];

/// The parsed execution intent of a Ghostlight process invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchMode {
    /// Local package/development lifecycle seam, never a model-facing tool.
    Deployment(ghostlight_bridge::installation::Selection),
    /// Start or activate the desktop workbench.
    Desktop,
    /// Explicit local-human intent to make the workbench visible.
    Open,
    /// The command-line intake. A script asked for work, not for a window (ADR-0105).
    Call,
    /// Process diagnostics: read the shared log and actuate the marker (ADR-0145).
    Diagnostics(crate::cli::diagnostics::Command),
    /// Local policy validation, explanation, and audit-free simulation.
    Policy(crate::governance::inspection::Command),
    /// The narrow package-facing Chromium registration seam (ADR-0115).
    NativeHost(NativeHostCommand),
    /// Flatpak Chromium registration on Linux.
    #[cfg(target_os = "linux")]
    FlatpakNativeHost {
        /// Native host action.
        command: NativeHostCommand,
        /// Explicit app-wide permission for Flatpak Chromium.
        allow_activation: bool,
    },
    /// Install the browser and selected MCP-client integrations.
    Install(SetupOptions),
    /// Remove only Ghostlight-owned browser and MCP-client integrations.
    Uninstall(SetupOptions),
    /// Inspect the complete local connection chain without changing it.
    Doctor {
        /// Apply ownership-safe repairs.
        fix: bool,
        /// Render structured JSON document.
        json: bool,
    },
    /// Report the local engine endpoint without starting it.
    Status {
        /// Render structured JSON document.
        json: bool,
    },
    /// Render stable command-line help.
    Help,
    /// Render the exact package version.
    Version,
}

/// Options configured for install and uninstall subcommands.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SetupOptions {
    /// Show changes without writing them.
    pub dry_run: bool,
    /// Select every supported Chromium browser.
    pub all_browsers: bool,
    /// Specific browser identifiers to configure.
    pub browser_ids: Vec<String>,
    /// Include MCP clients not currently detected.
    pub all_clients: bool,
    /// Leave every MCP client configuration unchanged.
    pub no_clients: bool,
    /// Do not open the browser-extension walkthrough.
    pub no_open: bool,
    /// Specific MCP client identifiers to configure.
    pub client_ids: Vec<String>,
}

/// The command sent to the native host registration manager.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHostCommand {
    /// Check registration status without modifying state.
    Check,
    /// Register the native messaging host.
    Install,
    /// Remove the native messaging host registration.
    Uninstall,
}

/// Parse command-line arguments into a typed `LaunchMode`.
pub fn launch_mode(arguments: impl IntoIterator<Item = OsString>) -> anyhow::Result<LaunchMode> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments
        .first()
        .is_some_and(|argument| argument == "deployment")
    {
        use ghostlight_bridge::installation::Selection;
        let selection = match arguments.get(1).and_then(|argument| argument.to_str()) {
            Some("development") if arguments.len() == 2 => Selection::Development,
            Some("release") if arguments.len() == 2 => Selection::Release,
            Some("restore") if arguments.len() == 2 => Selection::Restore,
            _ => anyhow::bail!("usage: ghostlight deployment <development|release|restore>"),
        };
        return Ok(LaunchMode::Deployment(selection));
    }
    if arguments.is_empty() {
        return Ok(LaunchMode::Desktop);
    }
    if arguments.len() == 1
        && arguments
            .first()
            .is_some_and(|argument| argument == "--version" || argument == "-V")
    {
        return Ok(LaunchMode::Version);
    }
    if arguments.len() == 1
        && arguments
            .first()
            .is_some_and(|argument| argument == "--help" || argument == "-h" || argument == "help")
    {
        return Ok(LaunchMode::Help);
    }
    if arguments.len() == 2
        && arguments
            .first()
            .is_some_and(|argument| argument == "install" || argument == "uninstall")
        && arguments
            .get(1)
            .is_some_and(|argument| argument == "--help" || argument == "-h")
    {
        return Ok(LaunchMode::Help);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "native-host")
    {
        let command = match arguments.get(1).and_then(|argument| argument.to_str()) {
            Some("check") => NativeHostCommand::Check,
            Some("install") => NativeHostCommand::Install,
            Some("uninstall") => NativeHostCommand::Uninstall,
            _ => anyhow::bail!("usage: ghostlight native-host <check|install|uninstall>"),
        };
        #[cfg(target_os = "linux")]
        if arguments
            .get(2)
            .is_some_and(|value| value == "--flatpak-chromium")
        {
            let allow_activation = arguments
                .get(3)
                .is_some_and(|value| value == "--allow-flatpak-activation");
            if arguments.len() != if allow_activation { 4 } else { 3 }
                || (allow_activation && command != NativeHostCommand::Install)
            {
                anyhow::bail!("usage: ghostlight native-host <check|install|uninstall> --flatpak-chromium [--allow-flatpak-activation (install only)]");
            }
            return Ok(LaunchMode::FlatpakNativeHost {
                command,
                allow_activation,
            });
        }
        if arguments.len() != 2 {
            anyhow::bail!("usage: ghostlight native-host <check|install|uninstall>");
        }
        Ok(LaunchMode::NativeHost(command))
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "install")
    {
        Ok(LaunchMode::Install(parse_setup_options(&arguments[1..])?))
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "uninstall")
    {
        Ok(LaunchMode::Uninstall(parse_setup_options(&arguments[1..])?))
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "doctor")
    {
        let mut fix = false;
        let mut json = false;
        for argument in &arguments[1..] {
            match argument.to_str() {
                Some("--fix") => fix = true,
                Some("--json") => json = true,
                Some("--verbose") => {}
                Some(other) => anyhow::bail!("unknown doctor option {other}"),
                None => anyhow::bail!("Ghostlight command options must be valid UTF-8"),
            }
        }
        // --fix writes; --json is for a script reading the result. Combining them would print a
        // document describing a state that the repair has already replaced.
        if fix && json {
            anyhow::bail!("ghostlight doctor --json reports state; use it without --fix");
        }
        Ok(LaunchMode::Doctor { fix, json })
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "status")
    {
        match arguments.as_slice() {
            [_] => Ok(LaunchMode::Status { json: false }),
            [_, option] if option == "--json" => Ok(LaunchMode::Status { json: true }),
            _ => anyhow::bail!("usage: ghostlight status [--json]"),
        }
    } else if arguments.len() == 1 && arguments.first().is_some_and(|argument| argument == "open") {
        Ok(LaunchMode::Open)
    } else if arguments.first().is_some_and(|argument| argument == "call") {
        Ok(LaunchMode::Call)
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "diagnostics")
    {
        let values = arguments[1..]
            .iter()
            .map(|argument| {
                argument
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow::anyhow!("diagnostics arguments must be valid UTF-8"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(LaunchMode::Diagnostics(crate::cli::diagnostics::parse(
            &values,
        )?))
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "policy")
    {
        let values = arguments[1..]
            .iter()
            .map(|argument| {
                argument
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow::anyhow!("policy paths must be valid UTF-8"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(LaunchMode::Policy(crate::governance::inspection::parse(
            &values,
        )?))
    } else {
        // Naming what is available beats sending someone to --help to find out.
        anyhow::bail!(
            "unknown Ghostlight command; expected one of: {}",
            SUBCOMMANDS.join(", ")
        )
    }
}

/// Parse arguments for install and uninstall commands into `SetupOptions`.
pub fn parse_setup_options(arguments: &[OsString]) -> anyhow::Result<SetupOptions> {
    let mut options = SetupOptions::default();
    let mut remaining = arguments.iter();
    while let Some(argument) = remaining.next() {
        let argument = argument
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Ghostlight command options must be valid UTF-8"))?;
        match argument {
            "--dry-run" => options.dry_run = true,
            "--all-browsers" => options.all_browsers = true,
            "--all-clients" => options.all_clients = true,
            "--no-clients" => options.no_clients = true,
            "--no-open" => options.no_open = true,
            "--browser" => {
                let id = remaining
                    .next()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| anyhow::anyhow!("--browser needs a browser id"))?;
                options.browser_ids.push(id.into());
            }
            value if value.starts_with("--browser=") => {
                let id = &value["--browser=".len()..];
                if id.is_empty() {
                    anyhow::bail!("--browser needs a browser id");
                }
                options.browser_ids.push(id.into());
            }
            "--client" => {
                let id = remaining
                    .next()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| anyhow::anyhow!("--client needs a client id"))?;
                options.client_ids.push(id.into());
            }
            value if value.starts_with("--client=") => {
                let id = &value["--client=".len()..];
                if id.is_empty() {
                    anyhow::bail!("--client needs a client id");
                }
                options.client_ids.push(id.into());
            }
            other => anyhow::bail!("unknown setup option {other}"),
        }
    }
    if options.no_clients && (options.all_clients || !options.client_ids.is_empty()) {
        anyhow::bail!("--no-clients cannot be combined with a client selection");
    }
    if options.all_browsers && !options.browser_ids.is_empty() {
        anyhow::bail!("--all-browsers cannot be combined with a browser selection");
    }
    Ok(options)
}

/// Render the CLI help text.
pub fn help_text() -> String {
    format!(
        "Ghostlight {version}\n\nUsage:\n  ghostlight open                    Open the desktop workbench\n  ghostlight install [options]       Connect browsers and detected MCP clients\n  ghostlight uninstall [options]     Remove only Ghostlight-owned registrations\n  ghostlight doctor [--json]         Check the complete local installation\n  ghostlight status [--json]         Check the local service endpoint\n  ghostlight call <tool> [json]      Run one browser tool\n  ghostlight diagnostics path|show|prune|on|off\n                                     Read the shared diagnostics log, or turn it on and off\n  ghostlight policy validate <file>  Validate one schema-3 policy\n  ghostlight policy explain <file>   Explain policy and the RAWX capability map\n  ghostlight policy simulate <file> <audit.jsonl>\n                                     Preview denials against existing audit\n  ghostlight policy keygen <dir>     Create customer-owned policy signing keys\n  ghostlight policy pubkey ...       Print public bootstrap verification keys\n  ghostlight policy sign ...         Sign a policy at an explicit sequence\n  ghostlight policy publish ...      Advance sequence and prepare deployment\n\nInstall options:\n  --dry-run                          Show changes without writing them\n  --browser <id>                     Select Chrome, Edge, Brave, or Chromium\n  --all-browsers                     Select every supported Chromium browser\n  --client <id>                      Select an MCP client (repeatable)\n  --all-clients                      Include clients not currently detected\n  --no-clients                       Leave every MCP client configuration unchanged\n  --no-open                          Do not open the browser-extension walkthrough\n\nUse 'ghostlight call --catalog' to list browser tools.",
        version = env!("CARGO_PKG_VERSION")
    )
}

/// Print the CLI help text to standard output.
pub fn print_help() {
    println!("{}", help_text());
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{help_text, launch_mode, LaunchMode, NativeHostCommand, SetupOptions, SUBCOMMANDS};

    #[cfg(target_os = "linux")]
    #[test]
    fn flatpak_permission_selection_is_explicit_and_install_only() {
        assert!(matches!(
            launch_mode(
                [
                    "native-host",
                    "install",
                    "--flatpak-chromium",
                    "--allow-flatpak-activation"
                ]
                .map(Into::into)
            )
            .unwrap(),
            LaunchMode::FlatpakNativeHost {
                allow_activation: true,
                ..
            }
        ));
        for command in ["check", "uninstall"] {
            assert!(launch_mode(
                [
                    "native-host",
                    command,
                    "--flatpak-chromium",
                    "--allow-flatpak-activation"
                ]
                .map(Into::into)
            )
            .is_err());
        }
        assert!(launch_mode(
            ["native-host", "install", "--flatpak-chromium", "typo"].map(Into::into)
        )
        .is_err());
    }

    /// The shell completions offer exactly the subcommands the command line has.
    ///
    /// Each completion file declares its command list on one line with a fixed shape, so this reads
    /// the real list rather than searching for words that might appear in a comment.
    #[test]
    fn completion_subcommands_match_the_parser() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packaging/linux/completions")
            .canonicalize()
            .expect("completion directory exists");
        let expected = SUBCOMMANDS.join(" ");
        for (file, prefix, suffix) in [
            ("ghostlight.bash", "local commands=\"", "\""),
            ("_ghostlight", "local -a ghostlight_commands=(", ")"),
            ("ghostlight.fish", "set -l ghostlight_commands ", "\n"),
        ] {
            let source = std::fs::read_to_string(root.join(file)).expect("completion is readable");
            let start = source
                .find(prefix)
                .unwrap_or_else(|| panic!("{file} declares no command list"))
                + prefix.len();
            let end = start
                + source[start..]
                    .find(suffix)
                    .unwrap_or_else(|| panic!("{file} command list is unterminated"));
            assert_eq!(
                source[start..end].trim(),
                expected,
                "{file} offers a different command list than the command line has"
            );
        }
    }

    /// Zsh selects option completions from the subcommand rather than the executable word.
    #[test]
    fn zsh_options_are_selected_from_the_subcommand_word() {
        let source = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../packaging/linux/completions/_ghostlight"),
        )
        .expect("zsh completion is readable");
        assert!(source.contains("local command=$words[2]"));
        assert!(source.contains("words=(\"${(@)words[2,-1]}\")"));
        assert!(source.contains("case $command in"));
        assert!(!source.contains("case $words[1] in"));
        assert!(!source.contains("_arguments -C"));
    }

    /// Every offered subcommand is documented in help, so the two surfaces cannot drift.
    #[test]
    fn help_documents_every_subcommand() {
        let help = help_text();
        for subcommand in SUBCOMMANDS {
            assert!(
                help.contains(&format!("ghostlight {subcommand}")),
                "help does not document {subcommand}"
            );
        }
    }

    /// Ghostlight's command line emits plain text on purpose.
    ///
    /// `NO_COLOR` has nothing to suppress today because nothing is styled, and that is worth
    /// keeping rather than rediscovering later. If styling is ever added, this test fails, and
    /// whoever adds it has to honor `NO_COLOR` in the same change and say so here.
    #[test]
    fn the_command_line_emits_no_terminal_styling() {
        const ESCAPE: char = '\u{1b}';

        fn visit(directory: &std::path::Path, offenders: &mut Vec<String>) {
            for entry in std::fs::read_dir(directory).expect("orchestrator source is readable") {
                let path = entry.expect("directory entry is readable").path();
                if path.is_dir() {
                    visit(&path, offenders);
                    continue;
                }
                if path.extension().is_some_and(|extension| extension == "rs") {
                    let source = std::fs::read_to_string(&path).expect("source file is UTF-8");
                    let escaped_forms = source.contains("\\x1b[") || source.contains("\\u{1b}[");
                    // Skip files that declare escape definitions for tests.
                    let is_exempt = path
                        .file_name()
                        .is_some_and(|name| name == "main.rs" || name == "parse.rs");
                    if !is_exempt && (source.contains(ESCAPE) || escaped_forms) {
                        offenders.push(path.display().to_string());
                    }
                }
            }
        }

        let mut offenders = Vec::new();
        visit(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut offenders,
        );
        assert!(
            offenders.is_empty(),
            "terminal styling was added in {offenders:?}; honor NO_COLOR and update this test"
        );
    }

    #[test]
    fn launch_modes_keep_desktop_and_call_intents_distinct() {
        assert_eq!(launch_mode(Vec::new()).unwrap(), LaunchMode::Desktop);
        assert_eq!(launch_mode(["open".into()]).unwrap(), LaunchMode::Open);
        assert!(launch_mode(["--headless".into()]).is_err());
        assert!(launch_mode(["service".into()]).is_err());
        assert_eq!(launch_mode(["call".into()]).unwrap(), LaunchMode::Call);
        assert_eq!(
            launch_mode(["policy".into(), "validate".into(), "policy.json".into()]).unwrap(),
            LaunchMode::Policy(crate::governance::inspection::Command::Validate(
                "policy.json".into()
            ))
        );
        assert_eq!(
            launch_mode(["doctor".into()]).unwrap(),
            LaunchMode::Doctor {
                fix: false,
                json: false
            }
        );
        assert_eq!(
            launch_mode(["doctor".into(), "--verbose".into(), "--fix".into()]).unwrap(),
            LaunchMode::Doctor {
                fix: true,
                json: false
            }
        );
        assert_eq!(
            launch_mode(["doctor".into(), "--json".into()]).unwrap(),
            LaunchMode::Doctor {
                fix: false,
                json: true
            }
        );
        // --fix writes and --json reports; together they would describe a state the repair has
        // already replaced.
        assert!(launch_mode(["doctor".into(), "--json".into(), "--fix".into()]).is_err());
        assert_eq!(
            launch_mode(["status".into()]).unwrap(),
            LaunchMode::Status { json: false }
        );
        assert_eq!(
            launch_mode(["status".into(), "--json".into()]).unwrap(),
            LaunchMode::Status { json: true }
        );
        assert_eq!(launch_mode(["--help".into()]).unwrap(), LaunchMode::Help);
        assert_eq!(
            launch_mode(["install".into(), "--help".into()]).unwrap(),
            LaunchMode::Help
        );
        assert_eq!(
            launch_mode(["--version".into()]).unwrap(),
            LaunchMode::Version
        );
        assert_eq!(
            launch_mode(["native-host".into(), "check".into()]).unwrap(),
            LaunchMode::NativeHost(NativeHostCommand::Check)
        );
        assert!(launch_mode(["native-host".into(), "guess".into()]).is_err());
        assert!(launch_mode(["nonsense".into()]).is_err());
    }

    #[test]
    fn setup_options_preserve_safe_package_compatibility() {
        assert_eq!(
            launch_mode([
                "install".into(),
                "--dry-run".into(),
                "--client".into(),
                "codex".into(),
                "--no-open".into(),
            ])
            .unwrap(),
            LaunchMode::Install(SetupOptions {
                dry_run: true,
                all_browsers: false,
                browser_ids: Vec::new(),
                all_clients: false,
                no_clients: false,
                no_open: true,
                client_ids: vec!["codex".into()],
            })
        );
        assert_eq!(
            launch_mode([
                "uninstall".into(),
                "--browser=brave".into(),
                "--all-clients".into(),
            ])
            .unwrap(),
            LaunchMode::Uninstall(SetupOptions {
                browser_ids: vec!["brave".into()],
                all_clients: true,
                ..SetupOptions::default()
            })
        );
        assert!(launch_mode([
            "install".into(),
            "--no-clients".into(),
            "--client=codex".into(),
        ])
        .is_err());
    }
}
