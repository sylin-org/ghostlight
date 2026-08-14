//! Ghostlight 1.0 orchestrator and integrated desktop workbench process.

// The mandatory npm launcher retains CLI stdio and waits for this child. Release desktop launches
// therefore use the native Windows application subsystem without flashing a console window.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::ffi::OsString;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::Path;
use std::thread;
use std::time::Duration;

const ACTIVATION_RETRY_COUNT: usize = 20;
const ACTIVATION_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, Eq, PartialEq)]
enum LaunchMode {
    Headless,
    Desktop,
    /// The command-line intake. A script asked for work, not for a window (ADR-0105).
    Call,
    /// The narrow package-facing Chromium registration seam (ADR-0115).
    NativeHost(NativeHostCommand),
    /// Install the browser and selected MCP-client integrations.
    Install(SetupOptions),
    /// Remove only Ghostlight-owned browser and MCP-client integrations.
    Uninstall(SetupOptions),
    /// Inspect the complete local connection chain without changing it.
    Doctor {
        fix: bool,
    },
    /// Report the local engine endpoint without starting it.
    Status {
        json: bool,
    },
    /// Render stable command-line help.
    Help,
    /// Render the exact package version.
    Version,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SetupOptions {
    dry_run: bool,
    all_browsers: bool,
    browser_ids: Vec<String>,
    all_clients: bool,
    no_clients: bool,
    no_open: bool,
    client_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeHostCommand {
    Check,
    Install,
    Uninstall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActivationState {
    Activated,
    Unavailable,
    Unreachable,
}

fn main() -> anyhow::Result<()> {
    match launch_mode(std::env::args_os().skip(1))? {
        LaunchMode::Headless => ghostlight::service::run_forever(),
        LaunchMode::Desktop => start_or_activate_desktop(),
        LaunchMode::Call => run_call(),
        LaunchMode::NativeHost(command) => run_native_host(command),
        LaunchMode::Install(options) => run_setup(true, &options),
        LaunchMode::Uninstall(options) => run_setup(false, &options),
        LaunchMode::Doctor { fix } => run_doctor(fix),
        LaunchMode::Status { json } => run_status(json),
        LaunchMode::Help => {
            print_help();
            Ok(())
        }
        LaunchMode::Version => {
            println!("ghostlight {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn run_setup(install: bool, options: &SetupOptions) -> anyhow::Result<()> {
    use ghostlight::install::native_host::{NativeHostRegistry, NativeHostState};
    use ghostlight::install::{HarnessAction, HarnessRegistry};

    let native_hosts = NativeHostRegistry::discover();
    let mut install_usable = false;
    if !options.browser_ids.is_empty() {
        let report = native_hosts.check()?;
        for id in &options.browser_ids {
            if !report.browsers.iter().any(|browser| browser.id == *id) {
                anyhow::bail!("unknown browser '{id}'; expected chrome, edge, brave, or chromium");
            }
        }
    }
    if options.dry_run {
        println!(
            "Ghostlight {} dry run -- no machine state will change.",
            if install { "install" } else { "uninstall" }
        );
        print_native_host_report(&native_hosts.check()?);
    } else if install {
        let result = if options.browser_ids.is_empty() {
            native_hosts.install()?
        } else {
            native_hosts.install_selected(&options.browser_ids)?
        };
        println!("Browser connection installed; changed: {}", result.changed);
        print_native_host_report(&result.report);
        install_usable = result.report.browsers.iter().any(|browser| {
            (options.browser_ids.is_empty() || options.browser_ids.contains(&browser.id))
                && browser.state == NativeHostState::Current
        });
        let migration = ghostlight::install::migration::retire_obsolete_supervisor();
        for removed in migration.removed {
            println!("Retired: {removed}");
        }
        for preserved in migration.preserved {
            println!("Preserved: {preserved}");
        }
        for warning in migration.warnings {
            eprintln!("Migration warning: {warning}");
        }
    } else {
        let result = if options.browser_ids.is_empty() {
            native_hosts.uninstall()?
        } else {
            native_hosts.uninstall_selected(&options.browser_ids)?
        };
        println!("Browser connection removed; changed: {}", result.changed);
        print_native_host_report(&result.report);
    }

    if install && !options.dry_run && !install_usable {
        return finish_setup(install, options, install_usable);
    }

    if options.no_clients {
        println!("MCP client configuration was left unchanged.");
        return finish_setup(install, options, install_usable);
    }

    let harnesses = HarnessRegistry::discover();
    let summaries = harnesses.refresh()?;
    let attention_count = summaries
        .iter()
        .filter(|summary| summary.state == ghostlight::install::HarnessState::NeedsAttention)
        .inspect(|summary| {
            eprintln!(
                "MCP client needs attention: {} -- {}",
                summary.name, summary.detail
            );
        })
        .count();
    let selected = select_harnesses(&summaries, options, install)?;
    if selected.is_empty() {
        if attention_count == 0 {
            println!("No MCP client configuration needs to change.");
        } else {
            println!("No MCP client configuration can be changed automatically.");
        }
        return finish_setup(install, options, install_usable);
    }
    let mut failures = Vec::new();
    for summary in selected {
        if options.dry_run {
            println!(
                "Would {} Ghostlight for {} ({:?}).",
                if install { "install" } else { "remove" },
                summary.name,
                summary.state
            );
            continue;
        }
        let action = if install {
            HarnessAction::Install
        } else {
            HarnessAction::Uninstall
        };
        match harnesses.apply(&summary.id, action) {
            Ok(result) => println!("{}", result.message),
            Err(error) => {
                eprintln!("{}: {error}", summary.name);
                failures.push(summary.name);
            }
        }
    }
    finish_setup(install, options, install_usable)?;
    if !failures.is_empty() {
        anyhow::bail!(
            "Ghostlight could not update {} MCP client integration(s)",
            failures.len()
        );
    }
    Ok(())
}

fn finish_setup(install: bool, options: &SetupOptions, install_usable: bool) -> anyhow::Result<()> {
    use ghostlight::install::handoff::{self, HandoffOutcome, EXTENSION_INSTALL_URL};

    if !install || options.dry_run {
        return Ok(());
    }
    if !install_usable {
        anyhow::bail!("Ghostlight could not establish a usable browser registration");
    }

    println!();
    println!("Ghostlight's local connection is ready.");
    println!("Browser extension: {EXTENSION_INSTALL_URL}");
    let automated = std::env::var_os("CI").is_some();
    match handoff::offer(options.dry_run, options.no_open, automated, install_usable) {
        Ok(HandoffOutcome::Opened) => println!("Opened the browser-extension walkthrough."),
        Ok(HandoffOutcome::AlreadyOffered) => {}
        Ok(HandoffOutcome::Suppressed) => {
            if options.no_open {
                println!("The walkthrough was not opened because --no-open was used.");
            }
        }
        Err(error) => eprintln!("Could not open the browser-extension walkthrough: {error}"),
    }
    println!("After adding the extension, restart or reconnect your MCP client. That is it.");
    Ok(())
}

fn select_harnesses(
    summaries: &[ghostlight::install::HarnessSummary],
    options: &SetupOptions,
    install: bool,
) -> anyhow::Result<Vec<ghostlight::install::HarnessSummary>> {
    use ghostlight::install::HarnessState;

    if !options.client_ids.is_empty() {
        let mut selected = Vec::new();
        for id in &options.client_ids {
            let summary = summaries
                .iter()
                .find(|summary| summary.id == *id)
                .ok_or_else(|| anyhow::anyhow!("unknown MCP client '{id}'"))?;
            selected.push(summary.clone());
        }
        return Ok(selected);
    }
    Ok(summaries
        .iter()
        .filter(|summary| {
            if install {
                summary.can_install
                    && (options.all_clients || summary.state != HarnessState::NotDetected)
            } else {
                summary.can_uninstall
            }
        })
        .cloned()
        .collect())
}

fn run_doctor(fix: bool) -> anyhow::Result<()> {
    use ghostlight::install::native_host::NativeHostRegistry;
    use ghostlight::install::HarnessRegistry;

    println!("Ghostlight {} diagnostics", env!("CARGO_PKG_VERSION"));
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("the Ghostlight executable has no parent directory"))?;
    let mut sibling_set_ready = true;
    for name in [
        executable_name("ghostlight"),
        executable_name("ghostlight-mcp-connector"),
        executable_name("ghostlight-browser-connector"),
    ] {
        let path = directory.join(name);
        let ready = path.is_file();
        sibling_set_ready &= ready;
        println!(
            "Binary: {} -- {}",
            path.display(),
            if ready { "ready" } else { "missing" }
        );
    }
    print_native_host_report(&NativeHostRegistry::discover().check()?);
    for harness in HarnessRegistry::discover().refresh()? {
        println!(
            "MCP client: {} -- {:?} -- {}",
            harness.name, harness.state, harness.detail
        );
    }
    print_runtime_status(false, sibling_set_ready);
    if fix {
        println!("Applying ownership-safe repairs.");
        run_setup(true, &SetupOptions::default())?;
    }
    Ok(())
}

fn run_status(json: bool) -> anyhow::Result<()> {
    if !json {
        println!("Ghostlight {}", env!("CARGO_PKG_VERSION"));
    }
    print_runtime_status(json, false);
    Ok(())
}

fn print_runtime_status(json: bool, idle_is_ready: bool) {
    let runtime_path = ghostlight_bridge::runtime::runtime_file();
    match ghostlight_bridge::runtime::read_runtime(&runtime_path) {
        Ok(runtime) => {
            let reachable = TcpStream::connect_timeout(
                &SocketAddrV4::new(Ipv4Addr::LOCALHOST, runtime.service_port).into(),
                Duration::from_millis(250),
            )
            .is_ok();
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "version": runtime.service_version,
                        "service_bridge_major": runtime.service_bridge_major,
                        "browser_relay_major": runtime.browser_relay_major,
                        "running": reachable,
                    })
                );
            } else {
                println!(
                    "Service: {} -- version {} -- bridge {} -- {}",
                    runtime_path.display(),
                    runtime.service_version,
                    runtime.service_bridge_major,
                    if reachable {
                        "running"
                    } else {
                        "not reachable"
                    }
                );
            }
        }
        Err(_) if json => println!("{}", serde_json::json!({ "running": false })),
        Err(_) if idle_is_ready => println!(
            "Service: ready on demand -- it starts when Chromium or an MCP client connects."
        ),
        Err(_) => println!(
            "Service: not running (no readable endpoint at {})",
            runtime_path.display()
        ),
    }
}

fn print_native_host_report(report: &ghostlight::install::native_host::NativeHostReport) {
    println!("Browser connector: {}", report.connector.display());
    for browser in &report.browsers {
        println!(
            "Browser: {} -- {:?} -- {}",
            browser.name, browser.state, browser.detail
        );
    }
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

fn print_help() {
    println!(
        "Ghostlight {version}\n\nUsage:\n  ghostlight                         Open the desktop workbench\n  ghostlight install [options]       Connect browsers and detected MCP clients\n  ghostlight uninstall [options]     Remove only Ghostlight-owned registrations\n  ghostlight doctor                  Check the complete local installation\n  ghostlight status [--json]         Check the local service endpoint\n  ghostlight service                 Run the local authority without a window\n  ghostlight call <tool> [json]      Run one browser tool\n  ghostlight --headless              Run the local authority without a window\n\nInstall options:\n  --dry-run                          Show changes without writing them\n  --browser <id>                     Select Chrome, Edge, Brave, or Chromium\n  --all-browsers                     Select every supported Chromium browser\n  --client <id>                      Select an MCP client (repeatable)\n  --all-clients                      Include clients not currently detected\n  --no-clients                       Leave every MCP client configuration unchanged\n  --no-open                          Do not open the browser-extension walkthrough\n\nUse 'ghostlight call --catalog' to list browser tools.",
        version = env!("CARGO_PKG_VERSION")
    );
}

fn run_native_host(command: NativeHostCommand) -> anyhow::Result<()> {
    use ghostlight::install::native_host::{NativeHostRegistry, NativeHostState};

    let registry = NativeHostRegistry::discover();
    let (verb, changed, report, migration) = match command {
        NativeHostCommand::Check => ("checked", false, registry.check()?, None),
        NativeHostCommand::Install => {
            let result = registry.install();
            let migration = ghostlight::install::migration::retire_obsolete_supervisor();
            let result = result?;
            ("installed", result.changed, result.report, Some(migration))
        }
        NativeHostCommand::Uninstall => {
            let result = registry.uninstall()?;
            ("uninstalled", result.changed, result.report, None)
        }
    };
    println!("Ghostlight native host {verb}; changed: {changed}");
    println!("Connector: {}", report.connector.display());
    for browser in report.browsers {
        let state = match browser.state {
            NativeHostState::Missing => "missing",
            NativeHostState::Current => "current",
            NativeHostState::Updatable => "updatable",
            NativeHostState::NeedsAttention => "needs attention",
        };
        println!("{}: {state} -- {}", browser.name, browser.detail);
    }
    if let Some(migration) = migration {
        for removed in migration.removed {
            println!("Retired: {removed}");
        }
        for preserved in migration.preserved {
            println!("Preserved: {preserved}");
        }
        for warning in migration.warnings {
            eprintln!("Migration warning: {warning}");
        }
    }
    Ok(())
}

/// Invoke one tool, or a batch of them, against the local authority.
///
/// Demand-start applies here exactly as it does to a connector: a script that runs before anything
/// else has started gets an authority rather than an error.
fn run_call() -> anyhow::Result<()> {
    let arguments: Vec<String> = std::env::args()
        .skip(1)
        .skip_while(|argument| argument != "call")
        .skip(1)
        .collect();
    let command = match ghostlight::cli::parse(&arguments) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let runtime = ghostlight_bridge::runtime::runtime_file();
    if ghostlight_bridge::runtime::read_runtime(&runtime).is_err() {
        let _ = ghostlight_bridge::lifecycle::request_orchestrator_start();
        wait_for_runtime(&runtime);
    }
    let mut out = std::io::stdout().lock();
    let code = ghostlight::cli::run(command, &runtime, &mut out);
    std::process::exit(code);
}

fn wait_for_runtime(runtime: &Path) {
    for _ in 0..ACTIVATION_RETRY_COUNT {
        if ghostlight_bridge::runtime::read_runtime(runtime).is_ok() {
            return;
        }
        thread::sleep(ACTIVATION_RETRY_DELAY);
    }
}

fn start_or_activate_desktop() -> anyhow::Result<()> {
    let runtime = ghostlight_bridge::runtime::runtime_file();
    match ghostlight::service::request_workbench_activation(&runtime) {
        Ok(true) => return Ok(()),
        Ok(false) => return finish_activation(wait_for_workbench_activation(&runtime), None),
        Err(_) => {}
    }
    match ghostlight::desktop::run() {
        Ok(()) => Ok(()),
        Err(start_error) => {
            finish_activation(wait_for_workbench_activation(&runtime), Some(start_error))
        }
    }
}

fn wait_for_workbench_activation(runtime: &Path) -> ActivationState {
    let mut presentation_seen = false;
    for _ in 0..ACTIVATION_RETRY_COUNT {
        match ghostlight::service::request_workbench_activation(runtime) {
            Ok(true) => return ActivationState::Activated,
            Ok(false) => presentation_seen = true,
            Err(_) => {}
        }
        thread::sleep(ACTIVATION_RETRY_DELAY);
    }
    if presentation_seen {
        ActivationState::Unavailable
    } else {
        ActivationState::Unreachable
    }
}

fn finish_activation(
    activation: ActivationState,
    start_error: Option<anyhow::Error>,
) -> anyhow::Result<()> {
    match activation {
        ActivationState::Activated => Ok(()),
        ActivationState::Unavailable => anyhow::bail!(
            "Ghostlight is running without a desktop workbench; stop the explicit headless engine before opening the desktop"
        ),
        ActivationState::Unreachable => Err(start_error.unwrap_or_else(|| {
            anyhow::anyhow!("the running Ghostlight authority could not be reached")
        })),
    }
}

fn launch_mode(arguments: impl IntoIterator<Item = OsString>) -> anyhow::Result<LaunchMode> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
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
        for argument in &arguments[1..] {
            match argument.to_str() {
                Some("--fix") => fix = true,
                Some("--verbose") => {}
                Some(other) => anyhow::bail!("unknown doctor option {other}"),
                None => anyhow::bail!("Ghostlight command options must be valid UTF-8"),
            }
        }
        Ok(LaunchMode::Doctor { fix })
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "status")
    {
        match arguments.as_slice() {
            [_] => Ok(LaunchMode::Status { json: false }),
            [_, option] if option == "--json" => Ok(LaunchMode::Status { json: true }),
            _ => anyhow::bail!("usage: ghostlight status [--json]"),
        }
    } else if arguments.len() == 1
        && arguments
            .first()
            .is_some_and(|argument| argument == "service")
    {
        Ok(LaunchMode::Headless)
    } else if arguments.first().is_some_and(|argument| argument == "call") {
        Ok(LaunchMode::Call)
    } else if arguments.len() == 1
        && arguments
            .first()
            .is_some_and(|argument| argument == "--headless")
    {
        Ok(LaunchMode::Headless)
    } else {
        anyhow::bail!("unknown Ghostlight command; run 'ghostlight --help'")
    }
}

fn parse_setup_options(arguments: &[OsString]) -> anyhow::Result<SetupOptions> {
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

#[cfg(test)]
mod tests {
    use super::{launch_mode, LaunchMode, NativeHostCommand, SetupOptions};

    #[test]
    fn launch_modes_keep_desktop_headless_and_call_intents_distinct() {
        assert_eq!(launch_mode(Vec::new()).unwrap(), LaunchMode::Desktop);
        assert_eq!(
            launch_mode(["--headless".into()]).unwrap(),
            LaunchMode::Headless
        );
        assert_eq!(launch_mode(["call".into()]).unwrap(), LaunchMode::Call);
        assert_eq!(
            launch_mode(["doctor".into()]).unwrap(),
            LaunchMode::Doctor { fix: false }
        );
        assert_eq!(
            launch_mode(["doctor".into(), "--verbose".into(), "--fix".into()]).unwrap(),
            LaunchMode::Doctor { fix: true }
        );
        assert_eq!(
            launch_mode(["status".into()]).unwrap(),
            LaunchMode::Status { json: false }
        );
        assert_eq!(
            launch_mode(["status".into(), "--json".into()]).unwrap(),
            LaunchMode::Status { json: true }
        );
        assert_eq!(
            launch_mode(["service".into()]).unwrap(),
            LaunchMode::Headless
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
