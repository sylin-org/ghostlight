//! Ghostlight 1.0 orchestrator and integrated desktop workbench process.

// The mandatory npm launcher retains CLI stdio and waits for this child. Release desktop launches
// therefore use the native Windows application subsystem without flashing a console window.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::ffi::OsString;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const ACTIVATION_RETRY_COUNT: usize = 20;
const ACTIVATION_RETRY_DELAY: Duration = Duration::from_millis(50);

/// Every subcommand a person is offered, in help order.
///
/// This is the list the shell completions must match. `native-host` is deliberately absent: it is
/// the package-facing registration seam, not something to suggest at a prompt.
const SUBCOMMANDS: &[&str] = &[
    "open",
    "install",
    "uninstall",
    "doctor",
    "status",
    "call",
    "diagnostics",
    "policy",
];

#[derive(Clone, Debug, Eq, PartialEq)]
enum LaunchMode {
    Desktop,
    /// Explicit local-human intent to make the workbench visible.
    Open,
    /// The command-line intake. A script asked for work, not for a window (ADR-0105).
    Call,
    /// Process diagnostics: read the shared log and actuate the marker (ADR-0145).
    Diagnostics(ghostlight::cli::diagnostics::Command),
    /// Local policy validation, explanation, and audit-free simulation.
    Policy(ghostlight::governance::inspection::Command),
    /// The narrow package-facing Chromium registration seam (ADR-0115).
    NativeHost(NativeHostCommand),
    /// Install the browser and selected MCP-client integrations.
    Install(SetupOptions),
    /// Remove only Ghostlight-owned browser and MCP-client integrations.
    Uninstall(SetupOptions),
    /// Inspect the complete local connection chain without changing it.
    Doctor {
        fix: bool,
        json: bool,
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
        LaunchMode::Desktop => start_or_activate_desktop(),
        LaunchMode::Open => open_desktop(),
        LaunchMode::Call => run_call(),
        LaunchMode::Diagnostics(command) => ghostlight::cli::diagnostics::run(&command),
        LaunchMode::Policy(command) => run_policy(&command),
        LaunchMode::NativeHost(command) => run_native_host(command),
        LaunchMode::Install(options) => run_setup(true, &options),
        LaunchMode::Uninstall(options) => run_setup(false, &options),
        LaunchMode::Doctor { fix, json } => run_doctor(fix, json),
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
    use ghostlight::install::desktop_entry::DesktopIntegration;
    use ghostlight::install::native_host::{NativeHostRegistry, NativeHostState};
    use ghostlight::install::{HarnessAction, HarnessRegistry};

    let native_hosts = NativeHostRegistry::discover();
    let mut install_usable = false;
    let initial_browser_report = native_hosts.check()?;
    let install_browser_ids = if install {
        select_install_browsers(&initial_browser_report, options)?
    } else {
        None
    };
    if options.dry_run {
        println!(
            "Ghostlight {} dry run -- no machine state will change.",
            if install { "install" } else { "uninstall" }
        );
        print_native_host_report(&initial_browser_report);
    } else if install {
        let result = match &install_browser_ids {
            None => native_hosts.install()?,
            Some(browser_ids) => native_hosts.install_selected(browser_ids)?,
        };
        println!("Browser connection installed; changed: {}", result.changed);
        print_native_host_report(&result.report);
        install_usable = result.report.browsers.iter().any(|browser| {
            (install_browser_ids
                .as_ref()
                .is_none_or(|browser_ids| browser_ids.contains(&browser.id)))
                && browser.state == NativeHostState::Current
                && (browser.package.native_messaging_usable() || options.all_browsers)
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

    let command_path = ghostlight::install::command_path::CommandPath::discover();
    if options.dry_run {
        print_command_path_report(&command_path.check()?);
    } else {
        let result = if install {
            command_path.install()?
        } else {
            command_path.uninstall()?
        };
        if result.report.state != ghostlight::install::command_path::CommandPathState::NotApplicable
        {
            print_command_path_report(&result.report);
        }
    }

    let user_assets = ghostlight::install::user_assets::UserAssets::discover();
    if !options.dry_run {
        let result = if install {
            user_assets.install()?
        } else {
            user_assets.uninstall()?
        };
        if result.report.state != ghostlight::install::user_assets::UserAssetState::NotApplicable {
            println!(
                "Documentation: {} -- {}",
                result.report.state.label(),
                result.report.detail
            );
        }
    }

    let desktop = DesktopIntegration::discover();
    if options.dry_run {
        print_desktop_integration_report(&desktop.check()?);
    } else {
        let result = if install {
            desktop.install()?
        } else {
            desktop.uninstall()?
        };
        if result.report.state
            != ghostlight::install::desktop_entry::DesktopIntegrationState::NotApplicable
        {
            println!(
                "Applications entry {}; changed: {}",
                if install { "installed" } else { "removed" },
                result.changed
            );
            print_desktop_integration_report(&result.report);
        }
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

fn select_install_browsers(
    report: &ghostlight::install::native_host::NativeHostReport,
    options: &SetupOptions,
) -> anyhow::Result<Option<Vec<String>>> {
    use ghostlight::install::browser_package::BrowserPackage;

    if options.all_browsers {
        return Ok(None);
    }
    if !options.browser_ids.is_empty() {
        for id in &options.browser_ids {
            let browser = report
                .browsers
                .iter()
                .find(|browser| browser.id == *id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown browser '{id}'; expected chrome, edge, brave, or chromium"
                    )
                })?;
            if browser.package.sandboxed() {
                anyhow::bail!("{}", browser.package_detail);
            }
            if browser.package == BrowserPackage::NotDetected {
                anyhow::bail!(
                    "{} Install Chrome, Edge, Brave, or Chromium as a native package, then run Ghostlight install again.",
                    browser.package_detail
                );
            }
        }
        return Ok(Some(options.browser_ids.clone()));
    }
    if report
        .browsers
        .iter()
        .all(|browser| browser.package == BrowserPackage::NotChecked)
    {
        return Ok(None);
    }
    let native = report
        .browsers
        .iter()
        .filter(|browser| browser.package == BrowserPackage::Native)
        .map(|browser| browser.id.clone())
        .collect::<Vec<_>>();
    if !native.is_empty() {
        return Ok(Some(native));
    }
    let sandboxed = report
        .browsers
        .iter()
        .filter(|browser| browser.package.sandboxed())
        .map(|browser| browser.package_detail.as_str())
        .collect::<Vec<_>>();
    if sandboxed.is_empty() {
        anyhow::bail!(
            "No supported native browser was detected. Install Chrome, Edge, Brave, or Chromium as a native package, then run Ghostlight install again."
        );
    }
    anyhow::bail!("{}", sandboxed.join(" "))
}

fn finish_setup(install: bool, options: &SetupOptions, install_usable: bool) -> anyhow::Result<()> {
    use ghostlight::install::handoff::{self, HandoffOutcome, EXTENSION_INSTALL_URL};

    if !install || options.dry_run {
        return Ok(());
    }
    if !install_usable {
        anyhow::bail!("Ghostlight could not establish a usable browser registration");
    }

    use ghostlight::language::environment;

    println!();
    println!("Ghostlight's local connection is ready.");
    let here = environment::current();
    println!("{}", here.location());
    println!("{}", environment::BACKGROUND_POSTURE);
    if let Some(caveat) = here.caveat() {
        println!("{caveat}");
    }
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

/// Everything `doctor` observed, gathered once.
///
/// The text and JSON renderings read the same values, so a script and a person cannot be told
/// different things about the same machine.
struct DoctorObservation {
    environment: ghostlight::language::environment::Environment,
    binaries: Vec<(PathBuf, bool)>,
    sibling_set_ready: bool,
    native_host: ghostlight::install::native_host::NativeHostReport,
    command_path: ghostlight::install::command_path::CommandPathReport,
    user_assets: ghostlight::install::user_assets::UserAssetReport,
    desktop: ghostlight::install::desktop_entry::DesktopIntegrationReport,
    harnesses: Vec<ghostlight::install::HarnessSummary>,
    runtime: RuntimeObservation,
    readiness: Option<ghostlight::workbench::ReadinessSummary>,
    diagnostics: ghostlight::diagnostics::DiagnosticsReport,
}

fn observe_doctor() -> anyhow::Result<DoctorObservation> {
    use ghostlight::install::desktop_entry::DesktopIntegration;
    use ghostlight::install::native_host::NativeHostRegistry;
    use ghostlight::install::HarnessRegistry;
    use ghostlight::language::environment;

    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("the Ghostlight executable has no parent directory"))?;
    let mut sibling_set_ready = true;
    let mut binaries = Vec::new();
    for name in [
        executable_name("ghostlight"),
        executable_name("ghostlight-mcp-connector"),
        executable_name("ghostlight-browser-connector"),
    ] {
        let path = directory.join(name);
        let ready = path.is_file();
        sibling_set_ready &= ready;
        binaries.push((path, ready));
    }
    let runtime = observe_runtime();
    let readiness = if runtime.running {
        ghostlight::service::request_readiness(&runtime.path).ok()
    } else {
        None
    };
    let diagnostics = ghostlight::diagnostics::observe(&ghostlight_bridge::runtime::runtime_file());
    Ok(DoctorObservation {
        // Same module as the install summary, so the two can never describe this machine
        // differently.
        environment: environment::current(),
        binaries,
        sibling_set_ready,
        native_host: NativeHostRegistry::discover().check()?,
        command_path: ghostlight::install::command_path::CommandPath::discover().check()?,
        user_assets: ghostlight::install::user_assets::UserAssets::discover().check()?,
        desktop: DesktopIntegration::discover().check()?,
        harnesses: HarnessRegistry::discover().refresh()?,
        runtime,
        readiness,
        diagnostics,
    })
}

fn run_doctor(fix: bool, json: bool) -> anyhow::Result<()> {
    let observation = observe_doctor()?;
    if json {
        println!("{}", serde_json::to_string(&doctor_document(&observation))?);
        return Ok(());
    }
    println!("Ghostlight {} diagnostics", env!("CARGO_PKG_VERSION"));
    println!(
        "Environment: {} -- {}",
        observation.environment.label(),
        observation.environment.location()
    );
    if let Some(caveat) = observation.environment.caveat() {
        println!("Environment: {caveat}");
    }
    if let Some(readiness) = &observation.readiness {
        println!("{}", readiness_line(readiness));
    } else if observation.runtime.running {
        println!(
            "Readiness: unavailable -- The running Ghostlight authority did not report its current state."
        );
    }
    for (path, ready) in &observation.binaries {
        println!(
            "Binary: {} -- {}",
            path.display(),
            if *ready { "ready" } else { "missing" }
        );
    }
    print_native_host_report(&observation.native_host);
    print_command_path_report(&observation.command_path);
    if observation.user_assets.state
        != ghostlight::install::user_assets::UserAssetState::NotApplicable
    {
        println!(
            "Documentation: {} -- {}",
            observation.user_assets.state.label(),
            observation.user_assets.detail
        );
    }
    print_desktop_integration_report(&observation.desktop);
    for harness in &observation.harnesses {
        println!(
            "MCP client: {} -- {} -- {}",
            harness.name,
            harness.state.label(),
            harness.detail
        );
    }
    render_runtime_status(&observation.runtime, false, observation.sibling_set_ready);
    let report = &observation.diagnostics;
    match (&report.layer[..], &report.directory) {
        (layer, Some(directory)) => println!(
            "Process diagnostics: {layer} -- {} bytes of log in {directory}",
            report.used_bytes
        ),
        (layer, None) => println!(
            "Process diagnostics: {layer} -- set GHOSTLIGHT_DIAGNOSTICS_DIR or create diagnostics.on beside the runtime file to turn them on"
        ),
    }
    if fix {
        println!("Applying ownership-safe repairs.");
        run_setup(true, &SetupOptions::default())?;
    }
    Ok(())
}

/// The JSON document `doctor --json` prints, built from the same observation the text path uses.
fn doctor_document(observation: &DoctorObservation) -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "environment": {
            "label": observation.environment.label(),
            "location": observation.environment.location(),
            "caveat": observation.environment.caveat(),
        },
        "binaries": observation
            .binaries
            .iter()
            .map(|(path, ready)| serde_json::json!({
                "path": path.display().to_string(),
                "ready": ready,
            }))
            .collect::<Vec<_>>(),
        "browser_connector": observation.native_host.connector.display().to_string(),
        "browsers": observation.native_host.browsers,
        "command": observation.command_path,
        "documentation": observation.user_assets,
        "applications": observation.desktop,
        "mcp_clients": observation.harnesses,
        "service": runtime_document(&observation.runtime),
        "readiness": observation.readiness,
        "process_diagnostics": observation.diagnostics,
    })
}

fn readiness_line(readiness: &ghostlight::workbench::ReadinessSummary) -> String {
    format!("Readiness: {} -- {}", readiness.word, readiness.detail)
}

fn run_status(json: bool) -> anyhow::Result<()> {
    if !json {
        println!("Ghostlight {}", env!("CARGO_PKG_VERSION"));
    }
    render_runtime_status(&observe_runtime(), json, false);
    Ok(())
}

/// What the local service endpoint looks like right now.
struct RuntimeObservation {
    path: PathBuf,
    version: Option<String>,
    service_bridge_major: Option<u16>,
    browser_relay_major: Option<u16>,
    running: bool,
}

fn observe_runtime() -> RuntimeObservation {
    let path = ghostlight_bridge::runtime::runtime_file();
    match ghostlight_bridge::runtime::read_runtime(&path) {
        Ok(runtime) => {
            let running = TcpStream::connect_timeout(
                &SocketAddrV4::new(Ipv4Addr::LOCALHOST, runtime.service_port).into(),
                Duration::from_millis(250),
            )
            .is_ok();
            RuntimeObservation {
                path,
                version: Some(runtime.service_version),
                service_bridge_major: Some(runtime.service_bridge_major),
                browser_relay_major: Some(runtime.browser_relay_major),
                running,
            }
        }
        Err(_) => RuntimeObservation {
            path,
            version: None,
            service_bridge_major: None,
            browser_relay_major: None,
            running: false,
        },
    }
}

/// The `status --json` document. Its shape is consumed by scripts and does not change here.
fn runtime_document(observation: &RuntimeObservation) -> serde_json::Value {
    let Some(version) = observation.version.as_deref() else {
        return serde_json::json!({ "running": false });
    };
    serde_json::json!({
        "version": version,
        "service_bridge_major": observation.service_bridge_major,
        "browser_relay_major": observation.browser_relay_major,
        "running": observation.running,
    })
}

fn render_runtime_status(observation: &RuntimeObservation, json: bool, idle_is_ready: bool) {
    if json {
        println!("{}", runtime_document(observation));
        return;
    }
    match observation.version.as_deref() {
        Some(version) => println!(
            "Service: {} -- version {} -- bridge {} -- {}",
            observation.path.display(),
            version,
            observation
                .service_bridge_major
                .map_or_else(String::new, |major| major.to_string()),
            if observation.running {
                "running"
            } else {
                "not reachable"
            }
        ),
        None if idle_is_ready => println!(
            "Service: ready on demand -- it starts when Chromium or an MCP client connects."
        ),
        None => println!(
            "Service: not running (no readable endpoint at {})",
            observation.path.display()
        ),
    }
}

fn print_native_host_report(report: &ghostlight::install::native_host::NativeHostReport) {
    println!("Browser connector: {}", report.connector.display());
    for browser in &report.browsers {
        println!(
            "Browser: {} -- {} -- {} -- {}",
            browser.name,
            browser.package_detail,
            browser.state.label(),
            browser.detail
        );
    }
}

fn print_command_path_report(report: &ghostlight::install::command_path::CommandPathReport) {
    use ghostlight::install::command_path::CommandPathState;

    if report.state == CommandPathState::NotApplicable {
        // Still name the executable: a person on a system package or on Windows needs the path as
        // much as anyone else does.
        println!("Command: {}", report.executable.display());
        return;
    }
    println!(
        "Command: {} -- {}",
        report.executable.display(),
        report.detail
    );
}

fn print_desktop_integration_report(
    report: &ghostlight::install::desktop_entry::DesktopIntegrationReport,
) {
    println!(
        "Applications: {} -- {} -- {}",
        report.state.label(),
        report.detail,
        report.desktop_entry.display()
    );
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

fn help_text() -> String {
    format!(
        "Ghostlight {version}\n\nUsage:\n  ghostlight open                    Open the desktop workbench\n  ghostlight install [options]       Connect browsers and detected MCP clients\n  ghostlight uninstall [options]     Remove only Ghostlight-owned registrations\n  ghostlight doctor [--json]         Check the complete local installation\n  ghostlight status [--json]         Check the local service endpoint\n  ghostlight call <tool> [json]      Run one browser tool\n  ghostlight diagnostics path|show|prune|on|off\n                                     Read the shared diagnostics log, or turn it on and off\n  ghostlight policy validate <file>  Validate one schema-3 policy\n  ghostlight policy explain <file>   Explain policy and the RAWX capability map\n  ghostlight policy simulate <file> <audit.jsonl>\n                                     Preview denials against existing audit\n  ghostlight policy keygen <dir>     Create customer-owned policy signing keys\n  ghostlight policy pubkey ...       Print public bootstrap verification keys\n  ghostlight policy sign ...         Sign a policy at an explicit sequence\n  ghostlight policy publish ...      Advance sequence and prepare deployment\n\nInstall options:\n  --dry-run                          Show changes without writing them\n  --browser <id>                     Select Chrome, Edge, Brave, or Chromium\n  --all-browsers                     Select every supported Chromium browser\n  --client <id>                      Select an MCP client (repeatable)\n  --all-clients                      Include clients not currently detected\n  --no-clients                       Leave every MCP client configuration unchanged\n  --no-open                          Do not open the browser-extension walkthrough\n\nUse 'ghostlight call --catalog' to list browser tools.",
        version = env!("CARGO_PKG_VERSION")
    )
}

fn print_help() {
    println!("{}", help_text());
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

fn run_policy(command: &ghostlight::governance::inspection::Command) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    ghostlight::governance::inspection::run(command, &mut out)
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

fn open_desktop() -> anyhow::Result<()> {
    use ghostlight_bridge::lifecycle::StartDisposition;

    let runtime = ghostlight_bridge::runtime::runtime_file();
    match ghostlight::service::request_workbench_activation(&runtime) {
        Ok(true) => return Ok(()),
        Ok(false) => return finish_activation(wait_for_workbench_activation(&runtime), None),
        Err(_) => {}
    }
    if ghostlight_bridge::lifecycle::request_orchestrator_start()?
        == StartDisposition::DeploymentInProgress
    {
        anyhow::bail!("Ghostlight is being updated; open it again when the update finishes");
    }
    finish_activation(wait_for_workbench_activation(&runtime), None)
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
            "the running Ghostlight authority has no desktop workbench; stop it before opening Ghostlight again"
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
        Ok(LaunchMode::Diagnostics(
            ghostlight::cli::diagnostics::parse(&values)?,
        ))
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
        Ok(LaunchMode::Policy(
            ghostlight::governance::inspection::parse(&values)?,
        ))
    } else {
        // Naming what is available beats sending someone to --help to find out.
        anyhow::bail!(
            "unknown Ghostlight command; expected one of: {}",
            SUBCOMMANDS.join(", ")
        )
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
    use std::path::PathBuf;

    use ghostlight::install::browser_package::BrowserPackage;
    use ghostlight::install::native_host::{
        BrowserRegistration, NativeHostReport, NativeHostState,
    };

    use super::{
        launch_mode, readiness_line, select_install_browsers, LaunchMode, NativeHostCommand,
        SetupOptions,
    };

    #[test]
    fn doctor_renders_every_workbench_readiness_state_in_the_same_words() {
        use ghostlight::language::readiness::Readiness;
        use ghostlight::workbench::ReadinessSummary;

        for state in Readiness::ALL {
            let readiness = ReadinessSummary {
                state: *state,
                word: state.word().into(),
                detail: state.detail().into(),
                tone: state.tone().into(),
                invites_control: state.invites_control(),
            };
            assert_eq!(
                readiness_line(&readiness),
                format!("Readiness: {} -- {}", state.word(), state.detail()),
                "{state:?}"
            );
        }
    }

    /// Every state `doctor` can report has plain words, and they are pinned here.
    ///
    /// Before this, `doctor` printed Rust identifiers such as `NeedsAttention` through `{:?}`,
    /// which is a debugger's vocabulary rather than a person's. The workbench renders the same
    /// states from the same serialized values; S6 makes it consume these exact words.
    #[test]
    fn every_reportable_state_has_plain_words() {
        use ghostlight::install::command_path::CommandPathState;
        use ghostlight::install::desktop_entry::DesktopIntegrationState;
        use ghostlight::install::native_host::NativeHostState;
        use ghostlight::install::user_assets::UserAssetState;
        use ghostlight::install::HarnessState;

        assert_eq!(HarnessState::NotDetected.label(), "not detected");
        assert_eq!(HarnessState::Available.label(), "detected, not connected");
        assert_eq!(HarnessState::Installed.label(), "connected");
        assert_eq!(
            HarnessState::Updatable.label(),
            "connected, needs an update"
        );
        assert_eq!(HarnessState::NeedsAttention.label(), "needs attention");

        assert_eq!(NativeHostState::Missing.label(), "not registered");
        assert_eq!(NativeHostState::Current.label(), "registered");
        assert_eq!(
            NativeHostState::Updatable.label(),
            "registered, needs an update"
        );
        assert_eq!(NativeHostState::NeedsAttention.label(), "needs attention");

        for label in [
            DesktopIntegrationState::NotApplicable.label(),
            DesktopIntegrationState::Missing.label(),
            DesktopIntegrationState::Current.label(),
            DesktopIntegrationState::Updatable.label(),
            DesktopIntegrationState::NeedsAttention.label(),
            CommandPathState::NotApplicable.label(),
            CommandPathState::Missing.label(),
            CommandPathState::Current.label(),
            CommandPathState::Updatable.label(),
            CommandPathState::NeedsAttention.label(),
            UserAssetState::NotApplicable.label(),
            UserAssetState::Missing.label(),
            UserAssetState::Current.label(),
            UserAssetState::NeedsAttention.label(),
        ] {
            assert!(!label.is_empty());
            // A person's word, not an identifier: no capitals and no camel case.
            assert_eq!(
                label,
                label.to_lowercase(),
                "{label} reads as an identifier"
            );
        }
    }

    /// `doctor` renders those words rather than Rust's debug formatting.
    #[test]
    fn doctor_prints_no_debug_formatted_state() {
        let source =
            std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"))
                .expect("main.rs is readable");
        for line in source.lines() {
            let reports_state = line.contains("Browser: {")
                || line.contains("Applications: {")
                || line.contains("MCP client: {")
                || line.contains("Command: {")
                || line.contains("Documentation: {");
            assert!(
                !(reports_state && line.contains("{:?}")),
                "doctor line uses debug formatting for a state: {line}"
            );
        }
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
        let expected = super::SUBCOMMANDS.join(" ");
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
        let help = super::help_text();
        for subcommand in super::SUBCOMMANDS {
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
                    // Skip this file: it necessarily contains the patterns it searches for.
                    let is_this_file = path.file_name().is_some_and(|name| name == "main.rs");
                    if !is_this_file && (source.contains(ESCAPE) || escaped_forms) {
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

    fn browser(id: &str, package: BrowserPackage) -> BrowserRegistration {
        BrowserRegistration {
            id: id.into(),
            name: id.into(),
            package,
            package_detail: format!("{id} package detail"),
            state: NativeHostState::Missing,
            detail: "registration detail".into(),
        }
    }

    fn browser_report(browsers: Vec<BrowserRegistration>) -> NativeHostReport {
        NativeHostReport {
            connector: PathBuf::from("/ghostlight-browser-connector"),
            browsers,
        }
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
            LaunchMode::Policy(ghostlight::governance::inspection::Command::Validate(
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

    #[test]
    fn ordinary_install_selects_only_detected_native_browsers() {
        let report = browser_report(vec![
            browser("chrome", BrowserPackage::Native),
            browser("chromium", BrowserPackage::Snap),
            browser("brave", BrowserPackage::NotDetected),
        ]);
        assert_eq!(
            select_install_browsers(&report, &SetupOptions::default()).unwrap(),
            Some(vec!["chrome".into()])
        );
        assert!(select_install_browsers(
            &report,
            &SetupOptions {
                browser_ids: vec!["chromium".into()],
                ..SetupOptions::default()
            }
        )
        .unwrap_err()
        .to_string()
        .contains("chromium package detail"));
    }

    #[test]
    fn all_browsers_and_windows_keep_deliberate_pre_registration() {
        let missing = browser_report(vec![browser("chrome", BrowserPackage::NotDetected)]);
        assert!(select_install_browsers(&missing, &SetupOptions::default()).is_err());
        assert_eq!(
            select_install_browsers(
                &missing,
                &SetupOptions {
                    all_browsers: true,
                    ..SetupOptions::default()
                }
            )
            .unwrap(),
            None
        );
        let windows = browser_report(vec![browser("chrome", BrowserPackage::NotChecked)]);
        assert_eq!(
            select_install_browsers(&windows, &SetupOptions::default()).unwrap(),
            None
        );
    }
}
