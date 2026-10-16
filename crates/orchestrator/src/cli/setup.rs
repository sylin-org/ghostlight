//! Browser and client setup, installation, and registration workflows.
//!
//! This module coordinates machine-level integration: registering the native
//! messaging host with Chromium browsers, updating MCP client configurations,
//! and handling uninstallation.
//!
//! A real install says what the person needs and nothing about its mechanism: it prints the
//! summary [`crate::language::setup::install_lines`] composes, plus the lines that need a person
//! (errors, registrations that need attention, what an app container kept private). The detail of
//! what changed prints only with `--dry-run`. While setup is incomplete the install also shows the
//! Ghostlight window, so the next step is in front of the person (ADR-0200 Decisions 10 to 12).
//!
//! Inside a Windows app container the registry and `AppData` writes reach only that app, so an
//! install there leaves those registrations unwritten, says which, and names the one command that
//! finishes them from a regular terminal (see `crate::install::container`).

use crate::cli::desktop::{self, Detached};
use crate::cli::parse::{NativeHostCommand, SetupOptions};
use crate::install::container::{self, AppContainer, Residence};
use crate::install::deployment::{stop_found_strays, StrayStop, SystemProcessControl};
use crate::install::layout::Layout;
use crate::install::native_host::{BrowserRegistration, NativeHostReport, NativeHostState};
use crate::install::reconcile::{
    windows_entries_failure, PendingWindowsEntries, ReconcileReport, Registrations, WindowsRemoval,
};
use crate::install::record::InstallRecord;
use crate::install::{setup_state, version_directories};
use crate::install::{HarnessActionResult, HarnessState, HarnessSummary};
use crate::language::deployment as deployment_words;
use crate::language::environment::{self, Environment};
use crate::language::setup::{self as setup_words, BrowserFact, InstallAgent, InstallFacts};
use crate::language::workbench as stray_words;

/// Registrations an install left unwritten because the app container it ran in keeps them private.
#[derive(Default)]
struct KeptPrivate {
    container: Option<AppContainer>,
    registrations: Vec<String>,
}

/// What a real install learned that its closing summary needs.
#[derive(Default)]
struct Collected {
    /// Whether the user `Path` was written, so a terminal that is already open lacks it.
    path_written: bool,
    /// The agents on this computer as the install left them.
    agents: Vec<InstallAgent>,
    /// The command an agent starts to run Ghostlight's MCP server.
    mcp_command: String,
}

/// Run installation or uninstallation across detected browsers and MCP clients.
///
/// An install that wrote its registrations also removes the 1.x version directories from `bin`,
/// with the programs running from them already stopped.
pub fn run_setup(install: bool, options: &SetupOptions) -> anyhow::Result<()> {
    run_setup_steps(install, options)?;
    if install && !options.dry_run {
        remove_version_directories(&Layout::current()?);
    }
    Ok(())
}

/// The registration steps of [`run_setup`].
fn run_setup_steps(install: bool, options: &SetupOptions) -> anyhow::Result<()> {
    use crate::install::command_path::{CommandPath, CommandPathState, PATH_ENTRY_REGISTRATION};
    use crate::install::desktop_entry::{DesktopIntegration, DesktopIntegrationState};
    use crate::install::native_host::NativeHostRegistry;
    use crate::install::user_assets::{UserAssetState, UserAssets};
    use crate::install::{HarnessAction, HarnessRegistry};

    ghostlight_relay::home::home()?;
    // A real install says only what the summary says, and what needs a person.
    let real_install = install && !options.dry_run;
    if real_install {
        stop_older_installation_programs()?;
    }
    let native_hosts = NativeHostRegistry::discover();
    let mut kept_private = KeptPrivate::default();
    let mut collected = Collected::default();
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
        kept_private.container = result.report.private_view.clone();
        kept_private
            .registrations
            .extend(private_browser_registrations(&result.private));
        install_usable = result.report.browsers.iter().any(|browser| {
            (install_browser_ids
                .as_ref()
                .is_none_or(|browser_ids| browser_ids.contains(&browser.id)))
                && (browser.state == NativeHostState::Current
                    || result.private.contains(&browser.name))
                && (browser.package.native_messaging_usable() || options.all_browsers)
        });
        for line in browser_attention_lines(
            &result.report,
            install_browser_ids.as_deref(),
            install_usable,
        ) {
            println!("{line}");
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

    let command_path = CommandPath::discover();
    if options.dry_run {
        print_command_path_report(&command_path.check()?);
    } else {
        let result = if install {
            command_path.install()?
        } else {
            command_path.uninstall()?
        };
        let state = result.report.state;
        if state == CommandPathState::NeedsAttention
            || (!install && state != CommandPathState::NotApplicable)
        {
            print_command_path_report(&result.report);
        }
        if install && state == CommandPathState::PrivateView {
            if kept_private.container.is_none() {
                kept_private.container = Residence::detect().container().cloned();
            }
            kept_private
                .registrations
                .push(PATH_ENTRY_REGISTRATION.to_owned());
        }
        collected.path_written = result.needs_new_terminal;
    }

    let user_assets = UserAssets::discover();
    if !options.dry_run {
        let result = if install {
            user_assets.install()?
        } else {
            user_assets.uninstall()?
        };
        let state = result.report.state;
        if state == UserAssetState::NeedsAttention
            || (!install && state != UserAssetState::NotApplicable)
        {
            println!(
                "Documentation: {} -- {}",
                state.label(),
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
        let state = result.report.state;
        if state == DesktopIntegrationState::NeedsAttention {
            print_desktop_integration_report(&result.report);
        } else if !install && state != DesktopIntegrationState::NotApplicable {
            println!("Applications entry removed; changed: {}", result.changed);
            print_desktop_integration_report(&result.report);
        }
    }

    if install {
        reconcile_windows_entries(options, &mut kept_private)?;
    } else {
        remove_windows_entries(options)?;
    }

    if real_install && !install_usable {
        return conclude(install, options, install_usable, &kept_private, &collected);
    }

    if options.no_clients && !real_install {
        println!("MCP client configuration was left unchanged.");
        return conclude(install, options, install_usable, &kept_private, &collected);
    }

    // The scan only reads. A real install reads it even with `--no-clients`, so the summary can
    // name the agents that are here.
    let harnesses = HarnessRegistry::discover();
    let summaries = harnesses.refresh()?;
    collected.mcp_command = summaries
        .first()
        .map(|summary| summary.connector_command.clone())
        .unwrap_or_default();
    let attention_count = if options.no_clients {
        0
    } else {
        summaries
            .iter()
            .filter(|summary| summary.state == HarnessState::NeedsAttention)
            .inspect(|summary| {
                eprintln!(
                    "MCP client needs attention: {} -- {}",
                    summary.label, summary.detail
                );
            })
            .count()
    };
    let selected = if options.no_clients {
        Vec::new()
    } else {
        select_harnesses(&summaries, options, install)?
    };
    if selected.is_empty() {
        if !real_install {
            if attention_count == 0 {
                println!("No MCP client configuration needs to change.");
            } else {
                println!("No MCP client configuration can be changed automatically.");
            }
        }
        collected.agents = install_agents(&summaries, &[]);
        return conclude(install, options, install_usable, &kept_private, &collected);
    }
    let mut failures = Vec::new();
    let mut applied = Vec::new();
    for summary in selected {
        if options.dry_run {
            println!(
                "Would {} Ghostlight for {} ({:?}).",
                if install { "install" } else { "remove" },
                summary.label,
                summary.state
            );
            continue;
        }
        if install && summary.private_view {
            kept_private
                .registrations
                .push(format!("{} MCP client", summary.label));
            continue;
        }
        let action = if install {
            HarnessAction::Install
        } else {
            HarnessAction::Uninstall
        };
        match harnesses.apply(&summary.id, action) {
            Ok(result) => {
                if !install {
                    println!("{}", result.message);
                }
                applied.push(result);
            }
            Err(error) => {
                eprintln!("{}: {error}", summary.label);
                failures.push(summary.label);
            }
        }
    }
    collected.agents = install_agents(&summaries, &applied);
    conclude(install, options, install_usable, &kept_private, &collected)?;
    if !failures.is_empty() {
        anyhow::bail!(
            "Ghostlight could not update {} MCP client integration(s)",
            failures.len()
        );
    }
    Ok(())
}

/// The agents on this computer as an install left them: each one detected or just configured,
/// whether it now has Ghostlight's entry, and whether this run wrote or changed that entry.
fn install_agents(
    summaries: &[HarnessSummary],
    applied: &[HarnessActionResult],
) -> Vec<InstallAgent> {
    summaries
        .iter()
        .filter_map(|summary| {
            let result = applied
                .iter()
                .find(|result| result.summary.id == summary.id);
            let now = result.map_or(summary, |result| &result.summary);
            (now.state != HarnessState::NotDetected).then(|| InstallAgent {
                label: now.label.clone(),
                has_entry: now.state == HarnessState::Installed,
                changed: result.is_some_and(|result| result.changed),
            })
        })
        .collect()
}

/// The lines about browsers that a person has to read after a real install: a registration that
/// needs attention or belongs to another installation, and, when no browser ended up usable, every
/// browser the install chose, so the reason is in front of them. Inside an app container the
/// report is that app's own view and says nothing; the closing lines name what it kept private.
fn browser_attention_lines(
    report: &NativeHostReport,
    selected: Option<&[String]>,
    install_usable: bool,
) -> Vec<String> {
    if report.private_view.is_some() {
        return Vec::new();
    }
    report
        .browsers
        .iter()
        .filter(|browser| {
            let chosen = selected.is_none_or(|ids| ids.contains(&browser.id));
            matches!(
                browser.state,
                NativeHostState::NeedsAttention | NativeHostState::OwnedElsewhere
            ) || (chosen && !install_usable)
        })
        .map(browser_line)
        .collect()
}

/// One browser's package, registration state and detail, as the reports print it.
fn browser_line(browser: &BrowserRegistration) -> String {
    format!(
        "Browser: {} -- {} -- {} -- {}",
        browser.name,
        browser.package_detail,
        browser.state.label(),
        browser.detail
    )
}

/// Write the Windows Start-menu shortcut and Uninstall key the way a deploy does, or, with
/// `--dry-run`, say each one that would change. Inside an app container they would
/// land in that app's private view, so each out-of-date one is added to `kept_private` instead.
///
/// The version is the installed one from `bin/install.json`, or the running service's when no
/// record exists. A failure is reported and does not stop the rest of the install, as in a deploy.
fn reconcile_windows_entries(
    options: &SetupOptions,
    kept_private: &mut KeptPrivate,
) -> anyhow::Result<()> {
    let layout = Layout::current()?;
    let version = installed_version(&layout);
    let registrations = Registrations::discover();
    if options.dry_run {
        match registrations.pending_windows_entries(&layout, &version) {
            Ok(pending) => {
                for line in pending_entry_lines(&pending, &layout) {
                    println!("{line}");
                }
            }
            Err(error) => eprintln!(
                "{}",
                deployment_words::registration_failed(&windows_entries_failure(&error))
            ),
        }
        return Ok(());
    }
    carry_windows_report(
        registrations.reconcile_windows_entries(&layout, &version),
        kept_private,
    );
    Ok(())
}

/// What a dry run says of the Windows entries: each one it would update, or, inside an app
/// container, each one it would leave unwritten and the command that finishes them.
fn pending_entry_lines(pending: &PendingWindowsEntries, layout: &Layout) -> Vec<String> {
    let mut lines: Vec<String> = pending
        .would_update
        .iter()
        .map(|entry| deployment_words::platform_entry_would_update(entry))
        .collect();
    if let Some(container) = &pending.container {
        if !pending.kept_private.is_empty() {
            lines.extend(container::kept_private_lines(
                container,
                &pending.kept_private,
                layout,
            ));
        }
    }
    lines
}

/// Say what failed in a reconcile of the Windows entries, and carry the registrations it left to
/// an app container into `kept_private` for the closing lines. What it wrote is mechanism and is
/// not said.
fn carry_windows_report(report: ReconcileReport, kept_private: &mut KeptPrivate) {
    for failure in &report.failures {
        eprintln!("{}", deployment_words::registration_failed(failure));
    }
    if !report.kept_private.is_empty() {
        if kept_private.container.is_none() {
            kept_private.container = report.container;
        }
        kept_private.registrations.extend(report.kept_private);
    }
}

/// Remove the Windows Start-menu shortcut and Uninstall key that are this installation's, saying
/// each one removed, or, with `--dry-run`, each one that would be, and each one left because it
/// names something else. A failure is reported and does not stop the rest of the uninstall.
///
/// Inside an app container a real `uninstall` has already been refused by the browser connection
/// step, which comes first. A `--dry-run` uninstall does reach this, and prints nothing for the
/// Windows entries because no removal is planned inside a container.
fn remove_windows_entries(options: &SetupOptions) -> anyhow::Result<()> {
    let layout = Layout::current()?;
    let registrations = Registrations::discover();
    let removal = if options.dry_run {
        registrations.pending_windows_removals(&layout)
    } else {
        registrations.remove_windows_entries(&layout)
    };
    for line in removal_lines(&removal, options.dry_run) {
        println!("{line}");
    }
    for failure in &removal.failures {
        eprintln!("{}", deployment_words::removal_failed(failure));
    }
    Ok(())
}

/// One line for each Windows entry removed, or with `dry_run` each that would be, and each kept.
fn removal_lines(removal: &WindowsRemoval, dry_run: bool) -> Vec<String> {
    let removed = if dry_run {
        deployment_words::platform_entry_would_remove
    } else {
        deployment_words::platform_entry_removed
    };
    removal
        .removed
        .iter()
        .map(|entry| removed(entry))
        .chain(
            removal
                .kept
                .iter()
                .map(|entry| deployment_words::platform_entry_kept(entry)),
        )
        .collect()
}

/// The version the Windows entries record: the installed service's, from the install record, or
/// this running service's when `bin` holds no record.
fn installed_version(layout: &Layout) -> String {
    InstallRecord::read(&layout.install_record())
        .ok()
        .flatten()
        .map_or_else(
            || env!("CARGO_PKG_VERSION").to_owned(),
            |record| record.service_version,
        )
}

/// Stop what an older Ghostlight installation left running, so one install leaves nothing behind
/// that `doctor` would name. The service that answers is spared unless it runs from the retired
/// development directory or from a 1.x version directory, which an install removes, so that one
/// is stopped too. A program that will not stop is said so, because the person has to close it,
/// and does not fail the install.
fn stop_older_installation_programs() -> anyhow::Result<()> {
    let layout = Layout::current()?;
    let service_pid = crate::cli::client::status_at(layout.home())
        .ok()
        .map(|status| status.pid);
    let stop = stop_found_strays(&SystemProcessControl, &layout, service_pid);
    for line in stray_not_stopped_lines(&stop) {
        println!("{line}");
    }
    Ok(())
}

/// Archive the audit log of each 1.x version directory in `layout`'s `bin` and remove the
/// directory. A directory that cannot be removed is said so and does not fail the install: the
/// next one tries again.
fn remove_version_directories(layout: &Layout) {
    let directories = version_directories::find(layout);
    let archives = version_directories::archives(layout, &directories);
    let retired = version_directories::retire(&directories, &archives);
    for directory in &retired.kept {
        eprintln!(
            "{}",
            deployment_words::kept_entry(&directory.display().to_string())
        );
    }
}

/// One line for each stray `stop` could not stop. The ones it stopped need no word.
fn stray_not_stopped_lines(stop: &StrayStop) -> Vec<String> {
    stop.stuck
        .iter()
        .map(|stray| stray_words::stray_not_stopped_line(&stray.image.display().to_string()))
        .collect()
}

/// How the report names the browsers whose registry value an install left unwritten.
fn private_browser_registrations(browsers: &[String]) -> Vec<String> {
    browsers
        .iter()
        .map(|name| format!("{name} browser connection"))
        .collect()
}

/// Finish the setup: when registrations were left unwritten because an app container keeps them
/// private, name them and the command that finishes them; otherwise present the install summary.
fn conclude(
    install: bool,
    options: &SetupOptions,
    install_usable: bool,
    kept_private: &KeptPrivate,
    collected: &Collected,
) -> anyhow::Result<()> {
    let container = match &kept_private.container {
        Some(container)
            if install && !options.dry_run && !kept_private.registrations.is_empty() =>
        {
            container
        }
        _ => return finish_setup(install, options, install_usable, collected),
    };
    println!();
    for line in
        container::kept_private_lines(container, &kept_private.registrations, &Layout::current()?)
    {
        println!("{line}");
    }
    Ok(())
}

/// Select browser targets based on detected packages and user options.
pub(crate) fn select_install_browsers(
    report: &crate::install::native_host::NativeHostReport,
    options: &SetupOptions,
) -> anyhow::Result<Option<Vec<String>>> {
    use crate::install::browser_package::BrowserPackage;

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

/// Whether an install shows the Ghostlight window: not for a dry run, not when the person said
/// `--no-open`, not in an automated run (`CI` is set), only when a browser registration is
/// usable, not inside an app container (the window would be that app's), and only while setup is
/// incomplete.
fn should_show_window(
    dry_run: bool,
    no_open: bool,
    automated: bool,
    install_usable: bool,
    in_container: bool,
    setup_complete: bool,
) -> bool {
    !dry_run && !no_open && !automated && install_usable && !in_container && !setup_complete
}

/// Show the Ghostlight window the way `ghostlight open` does: start the service when none
/// answers, then ask it to show. While setup is incomplete the service opens the window at the
/// setup view. A start that fails counts as not shown; the install goes on and its exit code
/// stays.
fn show_window() -> bool {
    desktop::open_with(&Detached).is_ok()
}

/// The facts the install summary is worded from, put together from what the run collected and
/// what the computer shows now.
fn install_facts(
    collected: &Collected,
    version: String,
    environment: Environment,
    browsers: Vec<BrowserFact>,
    setup_complete: bool,
    window_shown: bool,
) -> InstallFacts {
    InstallFacts {
        version,
        environment,
        path_written: collected.path_written,
        agents: collected.agents.clone(),
        mcp_command: collected.mcp_command.clone(),
        browsers,
        complete: setup_complete,
        window_shown,
    }
}

/// Finish an install: show the window while setup is incomplete, then print the summary.
fn finish_setup(
    install: bool,
    options: &SetupOptions,
    install_usable: bool,
    collected: &Collected,
) -> anyhow::Result<()> {
    if !install || options.dry_run {
        return Ok(());
    }
    if !install_usable {
        anyhow::bail!("Ghostlight could not establish a usable browser registration");
    }
    let layout = Layout::current()?;
    let setup_complete = setup_state::is_complete(&layout.setup_record());
    let automated = std::env::var_os("CI").is_some();
    let in_container = Residence::detect().container().is_some();
    let window_shown = should_show_window(
        options.dry_run,
        options.no_open,
        automated,
        install_usable,
        in_container,
        setup_complete,
    ) && show_window();
    let facts = install_facts(
        collected,
        installed_version(&layout),
        environment::current(),
        crate::workbench::setup::browsers_on_this_computer().facts,
        setup_complete,
        window_shown,
    );
    for line in setup_words::install_lines(&facts) {
        println!("{line}");
    }
    Ok(())
}

/// Filter candidate MCP harness summaries based on user selection.
pub fn select_harnesses(
    summaries: &[crate::install::HarnessSummary],
    options: &SetupOptions,
    install: bool,
) -> anyhow::Result<Vec<crate::install::HarnessSummary>> {
    use crate::install::HarnessState;

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

/// Print the native host registration status report to stdout.
pub(crate) fn print_native_host_report(report: &NativeHostReport) {
    println!("Ghostlight program: {}", report.connector.display());
    for browser in &report.browsers {
        match &report.private_view {
            Some(container) => println!(
                "Browser: {} -- {} -- {} -- Windows gives {} its own view of the registry.",
                browser.name,
                browser.package_detail,
                container::NOT_VISIBLE,
                container.app
            ),
            None => println!("{}", browser_line(browser)),
        }
    }
}

/// Print the command path link status report to stdout.
pub(crate) fn print_command_path_report(report: &crate::install::command_path::CommandPathReport) {
    use crate::install::command_path::CommandPathState;

    if report.state == CommandPathState::NotApplicable {
        // Still name the executable: a person on a system package or on a test home needs the path
        // as much as anyone else does.
        println!("Command: {}", report.executable.display());
        return;
    }
    println!(
        "Command: {} -- {}",
        report.executable.display(),
        report.detail
    );
}

/// Print the desktop applications entry report to stdout.
///
/// A platform without an XDG application entry has nothing to report.
pub(crate) fn print_desktop_integration_report(
    report: &crate::install::desktop_entry::DesktopIntegrationReport,
) {
    if let Some(entry) = &report.desktop_entry {
        println!(
            "Applications: {} -- {} -- {}",
            report.state.label(),
            report.detail,
            entry.display()
        );
    }
}

/// Execute a native host registration subcommand.
pub fn run_native_host(command: NativeHostCommand) -> anyhow::Result<()> {
    use crate::install::native_host::NativeHostRegistry;

    ghostlight_relay::home::home()?;
    let registry = NativeHostRegistry::discover();
    let (verb, changed, report, private) = match command {
        NativeHostCommand::Check => ("checked", false, registry.check()?, Vec::new()),
        NativeHostCommand::Install => {
            let result = registry.install()?;
            let verb = if result.private.is_empty() {
                "installed"
            } else {
                "prepared"
            };
            (verb, result.changed, result.report, result.private)
        }
        NativeHostCommand::Uninstall => {
            let result = registry.uninstall()?;
            ("uninstalled", result.changed, result.report, Vec::new())
        }
    };
    println!("Ghostlight native host {verb}; changed: {changed}");
    if let Some(container) = &report.private_view {
        print_native_host_report(&report);
        if !private.is_empty() {
            let registrations = private_browser_registrations(&private);
            for line in
                container::kept_private_lines(container, &registrations, &Layout::current()?)
            {
                println!("{line}");
            }
        }
        return Ok(());
    }
    println!("Ghostlight program: {}", report.connector.display());
    for browser in report.browsers {
        let state = match browser.state {
            NativeHostState::Missing => "missing",
            NativeHostState::Current => "current",
            NativeHostState::Updatable => "updatable",
            NativeHostState::OwnedElsewhere => "owned by another installation",
            NativeHostState::NeedsAttention => "needs attention",
        };
        println!("{}: {state} -- {}", browser.name, browser.detail);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::cli::parse::SetupOptions;
    use crate::install::browser_package::BrowserPackage;
    use crate::install::native_host::{BrowserRegistration, NativeHostReport, NativeHostState};

    use super::{
        private_browser_registrations, select_install_browsers, stray_not_stopped_lines, Collected,
    };
    use crate::install::{HarnessActionResult, HarnessState, HarnessSummary};
    use crate::language::setup::{BrowserFact, BrowserForm, InstallAgent};

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
            connector: PathBuf::from("/ghostlight"),
            browsers,
            private_view: None,
        }
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

    #[test]
    fn an_install_archives_and_removes_the_1x_version_directories_and_nothing_else() {
        use std::fs;

        use super::remove_version_directories;
        use crate::install::layout::Layout;
        use crate::install::scratch::Scratch;

        let scratch = Scratch::new();
        let layout = Layout::new(scratch.path());
        let bin = layout.bin();
        for name in ["v1.3.5", "v1.3.6", "vnext"] {
            fs::create_dir_all(bin.join(name)).unwrap();
            fs::write(bin.join(name).join("ghostlight-mcp-connector"), "image").unwrap();
        }
        fs::write(
            bin.join("v1.3.5").join("audit.jsonl"),
            b"log
",
        )
        .unwrap();
        fs::write(bin.join("v1.3.7"), "a file").unwrap();

        remove_version_directories(&layout);

        assert!(!bin.join("v1.3.5").exists());
        assert!(!bin.join("v1.3.6").exists());
        assert!(bin.join("vnext").join("ghostlight-mcp-connector").is_file());
        assert!(bin.join("v1.3.7").is_file());
        assert_eq!(
            fs::read(layout.state().join("audit-v1.3.5.jsonl")).unwrap(),
            b"log
"
        );

        remove_version_directories(&layout);
        assert!(bin.join("vnext").is_dir());
    }

    #[test]
    fn a_browser_left_private_is_named_as_its_connection() {
        assert_eq!(
            private_browser_registrations(&["Google Chrome".to_owned()]),
            ["Google Chrome browser connection"]
        );
    }

    #[test]
    fn setup_carries_the_entries_a_container_keeps_private_into_the_closing_lines() {
        use super::{carry_windows_report, KeptPrivate};
        use crate::install::container::AppContainer;
        use crate::install::reconcile::ReconcileReport;

        let container = AppContainer {
            app: "Claude".into(),
        };
        let mut kept = KeptPrivate::default();
        kept.registrations.push("PATH entry".to_owned());
        carry_windows_report(ReconcileReport::default(), &mut kept);
        assert_eq!(kept.registrations, ["PATH entry"]);
        assert_eq!(kept.container, None);

        carry_windows_report(
            ReconcileReport {
                kept_private: vec![
                    "Start-menu shortcut".to_owned(),
                    "installed-apps entry".to_owned(),
                ],
                container: Some(container.clone()),
                ..ReconcileReport::default()
            },
            &mut kept,
        );
        assert_eq!(
            kept.registrations,
            ["PATH entry", "Start-menu shortcut", "installed-apps entry"]
        );
        assert_eq!(kept.container, Some(container));

        let other = AppContainer {
            app: "Codex".into(),
        };
        carry_windows_report(
            ReconcileReport {
                kept_private: vec!["browser".to_owned()],
                container: Some(other),
                ..ReconcileReport::default()
            },
            &mut kept,
        );
        assert_eq!(
            kept.container.map(|container| container.app),
            Some("Claude".to_owned()),
            "the container already named stays"
        );
    }

    #[test]
    fn a_dry_run_inside_a_container_says_what_it_would_leave_private_not_what_it_would_update() {
        use super::pending_entry_lines;
        use crate::install::container::AppContainer;
        use crate::install::layout::Layout;
        use crate::install::reconcile::PendingWindowsEntries;

        let layout = Layout::new("/home/.ghostlight");
        let outside = PendingWindowsEntries {
            would_update: vec!["Start-menu shortcut".to_owned()],
            ..PendingWindowsEntries::default()
        };
        assert_eq!(
            pending_entry_lines(&outside, &layout),
            ["Would update the Start-menu shortcut."]
        );
        let inside = PendingWindowsEntries {
            kept_private: vec!["Start-menu shortcut".to_owned()],
            container: Some(AppContainer {
                app: "Claude".into(),
            }),
            ..PendingWindowsEntries::default()
        };
        let lines = pending_entry_lines(&inside, &layout);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("private to Claude") && lines[0].contains("Start-menu shortcut"));
        assert!(!lines.iter().any(|line| line.contains("Would update")));
        assert!(pending_entry_lines(&PendingWindowsEntries::default(), &layout).is_empty());
    }

    #[test]
    fn an_uninstall_says_each_entry_it_removed_and_each_it_kept() {
        use super::removal_lines;
        use crate::install::reconcile::WindowsRemoval;

        let removal = WindowsRemoval {
            removed: vec!["Start-menu shortcut".to_owned()],
            kept: vec!["installed-apps entry".to_owned()],
            failures: Vec::new(),
        };
        assert_eq!(
            removal_lines(&removal, false),
            [
                "Removed the Start-menu shortcut.",
                "Kept the installed-apps entry; it does not name this Ghostlight installation."
            ]
        );
        assert_eq!(
            removal_lines(&removal, true)[0],
            "Would remove the Start-menu shortcut."
        );
        assert!(removal_lines(&WindowsRemoval::default(), false).is_empty());
    }

    #[test]
    fn an_install_says_only_the_programs_it_could_not_stop() {
        use crate::install::deployment::{StrayKind, StrayProcess, StrayStop};

        let stray = |image: &str| StrayProcess {
            pid: 3,
            image: PathBuf::from(image),
            kind: StrayKind::RetiredConnector,
        };
        assert!(stray_not_stopped_lines(&StrayStop::default()).is_empty());
        let lines = stray_not_stopped_lines(&StrayStop {
            stopped: vec![stray("a.exe")],
            stuck: vec![stray("b.exe")],
        });
        assert_eq!(lines, ["Could not stop b.exe. Close it yourself."]);
    }

    #[test]
    fn the_window_opens_only_for_a_usable_install_that_is_not_complete_and_not_held_back() {
        use super::should_show_window;

        for bits in 0u8..64 {
            let flag = |bit: u8| bits & (1 << bit) != 0;
            let (dry_run, no_open, automated) = (flag(0), flag(1), flag(2));
            let (install_usable, in_container, complete) = (flag(3), flag(4), flag(5));
            let expected =
                !dry_run && !no_open && !automated && install_usable && !in_container && !complete;
            assert_eq!(
                should_show_window(
                    dry_run,
                    no_open,
                    automated,
                    install_usable,
                    in_container,
                    complete
                ),
                expected,
                "{bits:06b}"
            );
        }
        assert!(should_show_window(false, false, false, true, false, false));
    }

    fn harness(id: &str, label: &str, state: HarnessState) -> HarnessSummary {
        HarnessSummary {
            id: id.into(),
            product_id: id.into(),
            name: label.into(),
            label: label.into(),
            target: label.into(),
            icon: format!("{id}.svg"),
            state,
            detail: String::new(),
            evidence: None,
            can_install: true,
            can_fix: false,
            can_uninstall: false,
            can_download: false,
            can_locate: false,
            private_view: false,
            config_path: String::new(),
            connector_command: "/home/ghostlight mcp".into(),
            manual_setup: String::new(),
        }
    }

    fn applied(id: &str, label: &str, changed: bool) -> HarnessActionResult {
        HarnessActionResult {
            changed,
            summary: harness(id, label, HarnessState::Installed),
            message: String::new(),
        }
    }

    fn agent(label: &str, has_entry: bool, changed: bool) -> InstallAgent {
        InstallAgent {
            label: label.into(),
            has_entry,
            changed,
        }
    }

    #[test]
    fn the_agents_of_an_install_are_those_here_with_what_this_run_did() {
        use super::install_agents;

        let summaries = vec![
            harness("claude-code", "Claude Code", HarnessState::Available),
            harness("codex", "Codex", HarnessState::Updatable),
            harness("cursor", "Cursor", HarnessState::Installed),
            harness("zed", "Zed", HarnessState::NeedsAttention),
            harness("cline", "Cline", HarnessState::NotDetected),
        ];
        // Nothing applied (for instance `--no-clients`): what each already has.
        assert_eq!(
            install_agents(&summaries, &[]),
            [
                agent("Claude Code", false, false),
                agent("Codex", false, false),
                agent("Cursor", true, false),
                agent("Zed", false, false),
            ]
        );
        // Two written, one of them was only brought up to date.
        assert_eq!(
            install_agents(
                &summaries,
                &[
                    applied("claude-code", "Claude Code", true),
                    applied("codex", "Codex", true),
                    applied("cursor", "Cursor", false),
                ]
            ),
            [
                agent("Claude Code", true, true),
                agent("Codex", true, true),
                agent("Cursor", true, false),
                agent("Zed", false, false),
            ]
        );
        // A client named on the command line counts once Ghostlight wrote its entry.
        assert_eq!(
            install_agents(&summaries[4..], &[applied("cline", "Cline", true)]),
            [agent("Cline", true, true)]
        );
    }

    fn native_chrome() -> BrowserFact {
        BrowserFact {
            id: "chrome".into(),
            name: "Google Chrome".into(),
            short: "Chrome".into(),
            form: BrowserForm::Native,
            connected: false,
            extension: None,
        }
    }

    fn lines_of(
        collected: &Collected,
        browsers: Vec<BrowserFact>,
        setup_complete: bool,
        window_shown: bool,
    ) -> Vec<String> {
        use super::install_facts;
        use crate::language::environment::Environment;
        use crate::language::setup::install_lines;

        install_lines(&install_facts(
            collected,
            "2.0.0".into(),
            Environment::Windows,
            browsers,
            setup_complete,
            window_shown,
        ))
    }

    fn two_agents_changed() -> Collected {
        Collected {
            path_written: false,
            agents: vec![agent("Claude Code", true, true), agent("Codex", true, true)],
            mcp_command: "/home/ghostlight mcp".into(),
        }
    }

    #[test]
    fn a_first_install_with_the_window_shown_points_at_the_window() {
        assert_eq!(
            lines_of(&two_agents_changed(), vec![native_chrome()], false, true),
            [
                "Ghostlight 2.0.0 is installed.",
                "Ghostlight is in your notification area, and in the Start menu.",
                "Agents: Claude Code and Codex (restart them before you ask).",
                "Next: add Ghostlight to Chrome in the Ghostlight window.",
            ]
        );
    }

    #[test]
    fn a_first_install_without_the_window_gives_the_listing() {
        use crate::language::setup::STORE_LISTING_URL;

        let lines = lines_of(&two_agents_changed(), vec![native_chrome()], false, false);
        assert_eq!(
            lines.last().unwrap(),
            &format!("Next: add Ghostlight to Chrome: {STORE_LISTING_URL}")
        );
        assert_eq!(lines.len(), 4, "{lines:?}");
    }

    #[test]
    fn the_path_just_written_says_to_open_a_new_terminal() {
        let collected = Collected {
            path_written: true,
            ..two_agents_changed()
        };
        let lines = lines_of(&collected, vec![native_chrome()], false, true);
        assert_eq!(lines[2], "Open a new terminal to use ghostlight.");
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.starts_with("Next:"))
                .count(),
            1
        );
    }

    #[test]
    fn an_extension_already_in_a_profile_goes_to_the_agent_step() {
        use crate::language::setup::{ProfileFact, PROMPT};

        let mut chrome = native_chrome();
        chrome.extension = Some(ProfileFact {
            directory: "Default".into(),
            name: None,
            profiles: 1,
        });
        let lines = lines_of(&two_agents_changed(), vec![chrome], false, false);
        assert_eq!(
            lines.last().unwrap(),
            &format!("Next: restart Claude Code and Codex, then ask one: {PROMPT}")
        );
        assert!(!lines.iter().any(|line| line.contains("Chrome Web Store")));
    }

    #[test]
    fn a_complete_setup_has_no_next_line_and_asks_for_a_restart_only_of_what_changed() {
        let lines = lines_of(&two_agents_changed(), vec![native_chrome()], true, false);
        assert!(!lines.iter().any(|line| line.starts_with("Next:")));
        assert_eq!(
            lines.last().unwrap(),
            "Restart Claude Code and Codex to use Ghostlight."
        );
        let unchanged = Collected {
            agents: vec![agent("Claude Code", true, false)],
            ..two_agents_changed()
        };
        let lines = lines_of(&unchanged, vec![native_chrome()], true, false);
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(lines[2].starts_with("Agents: Claude Code"));
    }

    #[test]
    fn no_agents_gives_the_command_to_add() {
        let collected = Collected {
            agents: Vec::new(),
            ..two_agents_changed()
        };
        let lines = lines_of(&collected, vec![native_chrome()], false, true);
        assert!(lines
            .iter()
            .any(|line| line.contains("No agents") && line.ends_with("/home/ghostlight mcp")));
    }

    #[test]
    fn only_a_browser_that_needs_a_person_is_named_after_an_install() {
        use super::browser_attention_lines;

        let mut report = browser_report(vec![
            browser("chrome", BrowserPackage::Native),
            browser("edge", BrowserPackage::Native),
            browser("brave", BrowserPackage::Native),
        ]);
        for registered in &mut report.browsers {
            registered.state = NativeHostState::Current;
        }
        assert!(browser_attention_lines(&report, None, true).is_empty());

        report.browsers[2].state = NativeHostState::OwnedElsewhere;
        report.browsers[1].state = NativeHostState::NeedsAttention;
        let lines = browser_attention_lines(&report, Some(&["chrome".to_owned()]), true);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].starts_with("Browser: edge -- "));
        assert!(lines[1].starts_with("Browser: brave -- "));

        // No usable browser: every browser the install chose, so the reason is on the screen.
        report.browsers[1].state = NativeHostState::Current;
        report.browsers[2].state = NativeHostState::Current;
        let lines = browser_attention_lines(&report, Some(&["chrome".to_owned()]), false);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("Browser: chrome -- "));

        // Inside an app container the report is that app's own view and says nothing.
        report.private_view = Some(crate::install::container::AppContainer {
            app: "Claude".into(),
        });
        assert!(browser_attention_lines(&report, None, false).is_empty());
    }
}
