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
//! A real uninstall reads the same way: it prints [`crate::language::setup::uninstall_lines`],
//! which name what it removed and what it kept, plus the lines that need a person. With
//! `--dry-run` it lists what it would remove instead, the `Path` entry among it.
//!
//! Inside a Windows app container the registry and `AppData` writes reach only that app, so an
//! install there leaves those registrations unwritten, says which, and names the one command that
//! finishes them from a regular terminal (see `crate::install::container`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ghostlight_relay::protocol::{Control, QuitReason};

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
use crate::install::{client_ids, setup_state, version_directories};
use crate::install::{HarnessAction, HarnessActionResult, HarnessState, HarnessSummary};
use crate::language::deployment as deployment_words;
use crate::language::environment::{self, Environment};
use crate::language::setup::{
    self as setup_words, BrowserFact, InstallAgent, InstallFacts, RunningAfterUninstall,
    UninstallFacts,
};
use crate::language::workbench as stray_words;
use crate::service::lock::ServiceLock;

/// Registrations an install left unwritten because the app container it ran in keeps them private.
#[derive(Default)]
struct KeptPrivate {
    container: Option<AppContainer>,
    registrations: Vec<String>,
}

/// What a real install or uninstall learned that its closing summary needs.
#[derive(Default)]
struct Collected {
    /// Whether the user `Path` was written, so a terminal that is already open lacks it.
    path_written: bool,
    /// The `ghostlight` command's full path, when its directory is not on the `PATH` the install
    /// ran with.
    command_off_path: Option<PathBuf>,
    /// The agents on this computer as the install left them.
    agents: Vec<InstallAgent>,
    /// The command an agent starts to run Ghostlight's MCP server.
    mcp_command: String,
    /// The browsers a real uninstall removed Ghostlight's connection from, by name.
    removed_browsers: Vec<String>,
    /// The agents a real uninstall removed Ghostlight's entry from, by name.
    removed_agents: Vec<String>,
    /// Whether a real uninstall removed the `ghostlight` command.
    command_removed: bool,
}

/// Run installation or uninstallation across detected browsers and MCP clients.
///
/// An install that wrote its registrations also removes the 1.x version directories from `bin`,
/// with the programs running from them already stopped. A whole uninstall ends the running
/// Ghostlight, and a real uninstall then prints its summary.
pub fn run_setup(install: bool, options: &SetupOptions) -> anyhow::Result<()> {
    let collected = run_setup_steps(install, options)?;
    let layout = Layout::current()?;
    if install {
        if !options.dry_run {
            remove_version_directories(&layout);
        }
        return Ok(());
    }
    let whole = stops_running(options);
    if options.dry_run {
        if whole && !ServiceLock::is_free(&layout.service_lock()) {
            println!("{}", deployment_words::RUNNING_WOULD_STOP);
        }
        return Ok(());
    }
    let running = if whole {
        stop_running_for_uninstall(&layout)
    } else if ServiceLock::is_free(&layout.service_lock()) {
        RunningAfterUninstall::Unchanged
    } else {
        RunningAfterUninstall::LeftRunning
    };
    let facts = UninstallFacts {
        whole,
        agents: collected.removed_agents,
        browsers: collected.removed_browsers,
        command_removed: collected.command_removed,
        running,
        home: layout.home().display().to_string(),
    };
    for line in setup_words::uninstall_lines(&facts) {
        println!("{line}");
    }
    Ok(())
}

/// How long a whole uninstall waits for the running service to say goodbye and leave.
const UNINSTALL_QUIT_WAIT: Duration = Duration::from_secs(10);

/// How often that wait looks at the service lock.
const UNINSTALL_QUIT_POLL: Duration = Duration::from_millis(50);

/// Whether an uninstall removes Ghostlight as a whole, and so ends the Ghostlight that runs. One
/// that names browsers or clients, or leaves every client alone, removes only part of it.
fn stops_running(options: &SetupOptions) -> bool {
    options.browser_ids.is_empty() && options.client_ids.is_empty() && !options.no_clients
}

/// End the running Ghostlight after a whole uninstall: the service sends the `uninstall` goodbye,
/// on which the edges and the shell exit (ADR-0199 Decision 4). The answer to the quit only says
/// the service heard; the free service lock says it has gone. When nothing runs, nothing changes.
fn stop_running_for_uninstall(layout: &Layout) -> RunningAfterUninstall {
    let lock = layout.service_lock();
    if ServiceLock::is_free(&lock) {
        return RunningAfterUninstall::Unchanged;
    }
    let _ = crate::cli::client::control_at(
        layout.home(),
        Control::Quit {
            reason: QuitReason::Uninstall,
        },
    );
    let deadline = Instant::now() + UNINSTALL_QUIT_WAIT;
    while !ServiceLock::is_free(&lock) {
        if Instant::now() >= deadline {
            return RunningAfterUninstall::StillRunning;
        }
        std::thread::sleep(UNINSTALL_QUIT_POLL);
    }
    RunningAfterUninstall::Stopped
}

/// The registration steps of [`run_setup`], returning what the closing summary needs.
fn run_setup_steps(install: bool, options: &SetupOptions) -> anyhow::Result<Collected> {
    use crate::install::command_path::{CommandPath, CommandPathState, PATH_ENTRY_REGISTRATION};
    use crate::install::desktop_entry::{DesktopIntegration, DesktopIntegrationState};
    use crate::install::native_host::NativeHostRegistry;
    use crate::install::user_assets::{UserAssetState, UserAssets};
    use crate::install::HarnessRegistry;

    ghostlight_relay::home::home()?;
    // A real install says only what the summary says, and what needs a person.
    let real_install = install && !options.dry_run;
    let native_hosts = NativeHostRegistry::discover();
    let mut kept_private = KeptPrivate::default();
    let mut collected = Collected::default();
    let mut install_usable = false;
    // Every browser and client the person named is checked before anything stops, changes or
    // prints, so a mistyped id is refused at no cost. Both looks only read.
    let initial_browser_report = native_hosts.check()?;
    let install_browser_ids = if install {
        select_install_browsers(&initial_browser_report, options)?
    } else {
        None
    };
    if !options.client_ids.is_empty() && !options.no_clients {
        select_harnesses(&HarnessRegistry::discover().refresh()?, options, install)?;
    }
    if real_install {
        stop_older_installation_programs()?;
    }
    if options.dry_run {
        println!(
            "Ghostlight {} dry run -- no machine state will change.",
            if install { "install" } else { "uninstall" }
        );
        print_native_host_report(&initial_browser_report);
        if !install {
            for name in would_remove_browsers(&initial_browser_report, &options.browser_ids) {
                println!("{}", deployment_words::browser_would_remove(&name));
            }
        }
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
                    || result.private.contains(&browser.name)
                    || not_checkable(&result.report, browser))
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
        collected.removed_browsers = removed_browsers(&initial_browser_report, &result.report);
        for browser in &result.report.browsers {
            if browser.state == NativeHostState::NeedsAttention {
                println!("{}", browser_line(browser));
            }
        }
    }

    let command_path = CommandPath::discover();
    if options.dry_run {
        match command_path.removable()? {
            Some(entry) if !install => {
                println!("{}", deployment_words::command_entry_would_remove(&entry));
            }
            _ => print_command_path_report(&command_path.check()?),
        }
    } else {
        let result = if install {
            command_path.install()?
        } else {
            command_path.uninstall()?
        };
        let state = result.report.state;
        if state == CommandPathState::NeedsAttention {
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
        collected.command_off_path = result.off_path.clone();
        collected.command_removed = !install && result.changed;
    }

    let user_assets = UserAssets::discover();
    if !options.dry_run {
        let result = if install {
            user_assets.install()?
        } else {
            user_assets.uninstall()?
        };
        let state = result.report.state;
        if state == UserAssetState::NeedsAttention {
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
        if result.report.state == DesktopIntegrationState::NeedsAttention {
            print_desktop_integration_report(&result.report);
        }
    }

    if install {
        reconcile_windows_entries(options, &mut kept_private)?;
    } else {
        remove_windows_entries(options)?;
    }

    if real_install && !install_usable {
        return conclude(install, options, install_usable, &kept_private, &collected)
            .map(|()| collected);
    }

    if options.no_clients && !real_install {
        if options.dry_run {
            println!("MCP client configuration was left unchanged.");
        }
        return conclude(install, options, install_usable, &kept_private, &collected)
            .map(|()| collected);
    }

    let harnesses = HarnessRegistry::discover();
    if real_install && !options.no_clients {
        harnesses.remove_stray_temporaries();
    }
    // The scan only reads. A real install reads it even with `--no-clients`, so the summary can
    // name the agents that are here.
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
        if options.dry_run {
            if attention_count == 0 {
                println!("No MCP client configuration needs to change.");
            } else {
                println!("No MCP client configuration can be changed automatically.");
            }
        }
        collected.agents = install_agents(&summaries, &[]);
        return conclude(install, options, install_usable, &kept_private, &collected)
            .map(|()| collected);
    }
    let mut failures = Vec::new();
    let mut applied = Vec::new();
    let container = Residence::detect().container().cloned();
    for summary in selected {
        let action = match client_step(&summary, install, options.dry_run, container.as_ref()) {
            ClientStep::Apply(action) => action,
            ClientStep::LeavePrivate => {
                if kept_private.container.is_none() {
                    kept_private.container.clone_from(&container);
                }
                kept_private
                    .registrations
                    .push(format!("{} MCP client", summary.label));
                continue;
            }
            ClientStep::Say(line) => {
                println!("{line}");
                continue;
            }
        };
        match harnesses.apply(&summary.id, action) {
            Ok(result) => {
                if !install && result.changed {
                    collected.removed_agents.push(result.summary.label.clone());
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
    Ok(collected)
}

/// The browsers a real uninstall would remove Ghostlight's connection from, by name: each one
/// whose registration is Ghostlight's, among `browser_ids` when the person named any. Inside an
/// app container a real uninstall is refused, so it would remove none.
fn would_remove_browsers(report: &NativeHostReport, browser_ids: &[String]) -> Vec<String> {
    if report.private_view.is_some() {
        return Vec::new();
    }
    report
        .browsers
        .iter()
        .filter(|browser| browser.state.owned())
        .filter(|browser| browser_ids.is_empty() || browser_ids.contains(&browser.id))
        .map(|browser| browser.name.clone())
        .collect()
}

/// The browsers whose Ghostlight registration was there `before` an uninstall and is gone `after`
/// it, by name.
fn removed_browsers(before: &NativeHostReport, after: &NativeHostReport) -> Vec<String> {
    before
        .browsers
        .iter()
        .filter(|browser| browser.state.owned())
        .filter(|browser| {
            after
                .browsers
                .iter()
                .any(|now| now.id == browser.id && now.state == NativeHostState::Missing)
        })
        .map(|browser| browser.name.clone())
        .collect()
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
/// report is that app's own view: a registration the install could not judge there is named as
/// one that cannot be checked from here, and the closing lines name what it kept private.
fn browser_attention_lines(
    report: &NativeHostReport,
    selected: Option<&[String]>,
    install_usable: bool,
) -> Vec<String> {
    if let Some(container) = &report.private_view {
        return report
            .browsers
            .iter()
            .filter(|browser| not_checkable(report, browser))
            .map(|browser| not_visible_line(browser, container))
            .collect();
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
/// each one left because it names something else, and, with `--dry-run`, each one that would be
/// removed. A failure is reported and does not stop the rest of the uninstall.
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

/// One line for each Windows entry kept, after, with `dry_run`, one for each that would be removed.
/// What a real uninstall removed is in its summary's words, not named entry by entry.
fn removal_lines(removal: &WindowsRemoval, dry_run: bool) -> Vec<String> {
    let removed: &[String] = if dry_run { &removal.removed } else { &[] };
    removed
        .iter()
        .map(|entry| deployment_words::platform_entry_would_remove(entry))
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
/// what the computer shows now. `scan_browsers` reads the browsers and their profiles. It runs
/// only while setup is incomplete: a complete setup's summary names no browser step, and
/// docs/trust/data-flows.md says the profiles are read only then.
fn install_facts(
    collected: &Collected,
    version: String,
    environment: Environment,
    scan_browsers: impl FnOnce() -> Vec<BrowserFact>,
    setup_complete: bool,
    window_shown: bool,
) -> InstallFacts {
    InstallFacts {
        version,
        environment,
        path_written: collected.path_written,
        command_off_path: collected.command_off_path.clone(),
        agents: collected.agents.clone(),
        mcp_command: collected.mcp_command.clone(),
        browsers: if setup_complete {
            Vec::new()
        } else {
            scan_browsers()
        },
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
        || crate::workbench::setup::browsers_on_this_computer().facts,
        setup_complete,
        window_shown,
    );
    for line in setup_words::install_lines(&facts) {
        println!("{line}");
    }
    Ok(())
}

/// What a run does with one MCP client it selected.
#[derive(Debug, PartialEq, Eq)]
enum ClientStep {
    /// Change the client's configuration.
    Apply(HarnessAction),
    /// Leave the entry an install would write to the closing lines, which name the command that
    /// finishes it from a regular terminal.
    LeavePrivate,
    /// Change nothing and print this line.
    Say(String),
}

/// Decide what a run does with `summary`. Inside `container`, a client whose configuration it
/// keeps private is never changed: a real install leaves it to the closing lines, and a dry run or
/// a removal says that its entry is left and which command run elsewhere changes it. A dry run
/// says what the real run would change, read from the same inspection the real run acts on: a
/// removal names each other copy of the client's settings Uninstall would clean.
fn client_step(
    summary: &HarnessSummary,
    install: bool,
    dry_run: bool,
    container: Option<&AppContainer>,
) -> ClientStep {
    if summary.private_view {
        if install && !dry_run {
            return ClientStep::LeavePrivate;
        }
        if let Some(container) = container {
            return ClientStep::Say(deployment_words::client_left_private(
                &summary.label,
                &container.app,
                install,
            ));
        }
    }
    if dry_run {
        return ClientStep::Say(if install {
            deployment_words::client_would_install(&summary.label, summary.state, summary.attention)
        } else {
            deployment_words::client_would_remove(
                &summary.label,
                summary.state,
                summary.attention,
                &summary.owned_copies,
            )
        });
    }
    ClientStep::Apply(if install {
        HarnessAction::Install
    } else {
        HarnessAction::Uninstall
    })
}

/// Filter candidate MCP harness summaries based on user selection.
///
/// Each `--client` value selects the hosts [`HarnessSummary::is_selected_by`] says, in the order
/// the values were given and each host once: a harness id selects that one host, and an agent's
/// id the hosts of it that are detected, or every host of it with `--all-clients`. A value that
/// names no host is refused with every valid id, and an agent id none of whose hosts is detected
/// is refused with the agent's host ids.
///
/// Without `--client`, a run also selects each client it would change if the app container it
/// runs in did not keep that client's configuration private, so what it leaves is said rather than
/// passed over.
pub fn select_harnesses(
    summaries: &[HarnessSummary],
    options: &SetupOptions,
    install: bool,
) -> anyhow::Result<Vec<HarnessSummary>> {
    if !options.client_ids.is_empty() {
        let mut selected: Vec<HarnessSummary> = Vec::new();
        for id in &options.client_ids {
            let named: Vec<&HarnessSummary> = summaries
                .iter()
                .filter(|summary| summary.named_by(id).is_some())
                .collect();
            let Some(first) = named.first() else {
                let valid = client_ids();
                anyhow::bail!(deployment_words::unknown_client(
                    id,
                    &valid.agents,
                    &valid.hosts
                ));
            };
            let chosen: Vec<&HarnessSummary> = named
                .iter()
                .copied()
                .filter(|summary| summary.is_selected_by(id, options.all_clients))
                .collect();
            if chosen.is_empty() {
                let hosts: Vec<&str> = named.iter().map(|summary| summary.id.as_str()).collect();
                anyhow::bail!(deployment_words::no_client_host_found(
                    id,
                    &first.agent.name,
                    &hosts
                ));
            }
            for summary in chosen {
                if !selected.iter().any(|kept| kept.id == summary.id) {
                    selected.push(summary.clone());
                }
            }
        }
        return Ok(selected);
    }
    Ok(summaries
        .iter()
        .filter(|summary| {
            if install {
                (summary.can_install || left_private(summary, install))
                    && (options.all_clients || summary.state != HarnessState::NotDetected)
            } else {
                summary.can_uninstall || left_private(summary, install)
            }
        })
        .cloned()
        .collect())
}

/// Whether an install would write `summary`'s entry, or a removal would remove it, but the run
/// leaves it to a regular terminal, because the app container this process runs in keeps the
/// client's configuration private.
fn left_private(summary: &crate::install::HarnessSummary, install: bool) -> bool {
    summary.private_view
        && !summary.copy_only
        && if install {
            summary.state.takes_install()
        } else {
            summary.has_entry_to_remove()
        }
}

/// Print the line that names the Ghostlight program browsers start.
fn print_program_line(program: &std::path::Path) {
    println!("Ghostlight program: {}", program.display());
}

/// Print the native host registration status report to stdout.
pub(crate) fn print_native_host_report(report: &NativeHostReport) {
    print_program_line(&report.connector);
    for line in browser_report_lines(report) {
        println!("{line}");
    }
}

/// One line per browser of `report`. Inside an app container each line says the registration
/// cannot be checked from here, because the registry this process reads is that app's own.
pub(crate) fn browser_report_lines(report: &NativeHostReport) -> Vec<String> {
    report
        .browsers
        .iter()
        .map(|browser| match &report.private_view {
            Some(container) => not_visible_line(browser, container),
            None => browser_line(browser),
        })
        .collect()
}

/// One browser's line inside `container`: its package, and in place of its registration state
/// that the state cannot be checked from here, because the registry read is that app's own.
fn not_visible_line(browser: &BrowserRegistration, container: &AppContainer) -> String {
    format!(
        "Browser: {} -- {} -- {} -- {}",
        browser.name,
        browser.package_detail,
        container::NOT_VISIBLE,
        container::registry_view_detail(container)
    )
}

/// Whether an install inside an app container left `browser` unjudged: the value in that app's
/// own view of the registry is not one Ghostlight can read as its own, which says nothing about
/// the value the browser reads, so the row is neither a failure nor a registration kept private.
fn not_checkable(report: &NativeHostReport, browser: &BrowserRegistration) -> bool {
    report.private_view.is_some() && browser.state == NativeHostState::NeedsAttention
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
    print_program_line(&report.connector);
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
        client_step, private_browser_registrations, select_harnesses, select_install_browsers,
        stray_not_stopped_lines, ClientStep, Collected,
    };
    use crate::install::{HarnessAction, HarnessActionResult, HarnessState, HarnessSummary};
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
    fn only_a_whole_uninstall_ends_the_running_ghostlight() {
        assert!(super::stops_running(&SetupOptions::default()));
        for scoped in [
            SetupOptions {
                browser_ids: vec!["chromium".into()],
                ..SetupOptions::default()
            },
            SetupOptions {
                client_ids: vec!["zed".into()],
                ..SetupOptions::default()
            },
            SetupOptions {
                no_clients: true,
                ..SetupOptions::default()
            },
        ] {
            assert!(!super::stops_running(&scoped));
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
        use crate::scratch::Scratch;

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
    fn an_uninstall_names_each_entry_it_kept_and_its_dry_run_each_it_would_remove() {
        use super::removal_lines;
        use crate::install::reconcile::WindowsRemoval;

        let removal = WindowsRemoval {
            removed: vec!["Start-menu shortcut".to_owned()],
            kept: vec!["installed-apps entry".to_owned()],
            failures: Vec::new(),
        };
        let kept = "Kept the installed-apps entry; it does not name this Ghostlight installation.";
        assert_eq!(removal_lines(&removal, false), [kept]);
        assert_eq!(
            removal_lines(&removal, true),
            ["Would remove the Start-menu shortcut.", kept]
        );
        assert!(removal_lines(&WindowsRemoval::default(), false).is_empty());
    }

    #[test]
    fn an_uninstall_names_the_browsers_whose_registration_it_removed() {
        use super::removed_browsers;

        let with_state = |id: &str, state| BrowserRegistration {
            state,
            ..browser(id, BrowserPackage::Native)
        };
        let before = browser_report(vec![
            with_state("chrome", NativeHostState::Current),
            with_state("edge", NativeHostState::OwnedElsewhere),
            with_state("brave", NativeHostState::NeedsAttention),
            with_state("chromium", NativeHostState::Missing),
        ]);
        let after = browser_report(vec![
            with_state("chrome", NativeHostState::Missing),
            with_state("edge", NativeHostState::Missing),
            with_state("brave", NativeHostState::NeedsAttention),
            with_state("chromium", NativeHostState::Missing),
        ]);
        assert_eq!(removed_browsers(&before, &after), ["chrome", "edge"]);
        assert!(removed_browsers(&before, &before).is_empty());
    }

    #[test]
    fn a_dry_run_names_only_the_browsers_the_uninstall_would_act_on() {
        use super::would_remove_browsers;

        let with_state = |id: &str, state| BrowserRegistration {
            state,
            ..browser(id, BrowserPackage::Native)
        };
        let mut report = browser_report(vec![
            with_state("chrome", NativeHostState::Current),
            with_state("edge", NativeHostState::OwnedElsewhere),
            with_state("brave", NativeHostState::NeedsAttention),
            with_state("chromium", NativeHostState::Missing),
        ]);
        assert_eq!(would_remove_browsers(&report, &[]), ["chrome", "edge"]);
        assert_eq!(
            would_remove_browsers(&report, &["chrome".to_owned(), "brave".to_owned()]),
            ["chrome"]
        );
        report.private_view = Some(crate::install::container::AppContainer {
            app: "Claude_pzs8sxrjxfjjc".into(),
        });
        assert!(would_remove_browsers(&report, &[]).is_empty());
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
            agent: crate::agent::Agent::listed(id, label, crate::agent::image::Monogram::FIXTURE),
            state,
            detail: String::new(),
            evidence: None,
            attention: None,
            can_install: true,
            can_fix: false,
            can_uninstall: false,
            can_download: false,
            download_url: None,
            can_locate: false,
            copy_only: false,
            private_view: false,
            config_path: String::new(),
            packaged: false,
            other_copy: None,
            owned_copies: Vec::new(),
            connector_command: "/home/ghostlight mcp".into(),
            manual_setup: String::new(),
        }
    }

    #[test]
    fn a_run_inside_a_container_selects_the_private_clients_it_leaves() {
        let private = |id: &str, state: HarnessState| HarnessSummary {
            can_install: false,
            private_view: true,
            ..harness(id, id, state)
        };
        let summaries = [
            harness("cursor", "Cursor", HarnessState::Available),
            private("claude-desktop", HarnessState::Available),
            private("zed", HarnessState::Updatable),
            private("windsurf", HarnessState::Installed),
            private("kiro", HarnessState::NeedsAttention),
            HarnessSummary {
                copy_only: true,
                ..private("goose", HarnessState::Available)
            },
            private("vscode", HarnessState::NotDetected),
        ];
        let ids = |selected: Vec<HarnessSummary>| -> Vec<String> {
            selected.into_iter().map(|summary| summary.id).collect()
        };
        let options = SetupOptions::default();
        assert_eq!(
            ids(select_harnesses(&summaries, &options, true).unwrap()),
            ["cursor", "claude-desktop", "zed"]
        );
        let every = SetupOptions {
            all_clients: true,
            ..SetupOptions::default()
        };
        assert_eq!(
            ids(select_harnesses(&summaries, &every, true).unwrap()),
            ["cursor", "claude-desktop", "zed", "vscode"]
        );
        // A removal selects each private client that holds Ghostlight's entry, current or not, so
        // it can say so.
        assert_eq!(
            ids(select_harnesses(&summaries, &options, false).unwrap()),
            ["zed", "windsurf"]
        );
    }

    #[test]
    fn a_client_id_selects_an_agents_detected_hosts_or_one_host_in_the_order_given() {
        let cline = |id: &str, host: &str, state: HarnessState| HarnessSummary {
            agent: crate::agent::Agent::listed(
                "cline",
                "Cline",
                crate::agent::image::Monogram::FIXTURE,
            ),
            ..harness(id, &format!("Cline ({host})"), state)
        };
        let summaries = [
            harness("codex", "Codex", HarnessState::Installed),
            cline("cline-cli", "CLI", HarnessState::Available),
            cline(
                "cline-vscode",
                "Visual Studio Code",
                HarnessState::NotDetected,
            ),
            cline("cline-cursor", "Cursor", HarnessState::NotDetected),
            cline("cline-windsurf", "Windsurf", HarnessState::NotDetected),
            harness("zed", "Zed", HarnessState::NotDetected),
        ];
        let select = |summaries: &[HarnessSummary],
                      values: &[&str],
                      all_clients: bool,
                      install: bool|
         -> anyhow::Result<Vec<String>> {
            let options = SetupOptions {
                client_ids: values.iter().map(|value| (*value).into()).collect(),
                all_clients,
                ..SetupOptions::default()
            };
            Ok(select_harnesses(summaries, &options, install)?
                .into_iter()
                .map(|summary| summary.id)
                .collect())
        };
        // An agent id selects only the hosts of it that are found, so naming Cline writes no
        // file into an editor the person does not have.
        assert_eq!(
            select(&summaries, &["cline"], false, true).unwrap(),
            ["cline-cli"]
        );
        assert_eq!(
            select(&summaries, &["cline"], false, false).unwrap(),
            ["cline-cli"]
        );
        assert_eq!(
            select(&summaries, &["cline"], true, true).unwrap(),
            [
                "cline-cli",
                "cline-vscode",
                "cline-cursor",
                "cline-windsurf"
            ]
        );
        // A harness id selects exactly the host it selected before, whatever its state.
        assert_eq!(
            select(&summaries, &["CLINE-Cursor"], false, true).unwrap(),
            ["cline-cursor"]
        );
        assert_eq!(
            select(&summaries, &["zed", "codex"], false, true).unwrap(),
            ["zed", "codex"]
        );
        assert_eq!(
            select(&summaries, &["codex"], false, false).unwrap(),
            ["codex"]
        );
        // A host named twice, by its agent and by its own id, is set up once.
        assert_eq!(
            select(
                &summaries,
                &["cline-cli", "cline-vscode", "cline", "codex"],
                false,
                false
            )
            .unwrap(),
            ["cline-cli", "cline-vscode", "codex"]
        );
        let refusal = select(&summaries, &["cline", "Cline (Cursor)"], false, true)
            .unwrap_err()
            .to_string();
        assert!(
            refusal.starts_with("unknown MCP client 'Cline (Cursor)'; expected an agent id ("),
            "{refusal}"
        );
        let valid = crate::install::client_ids();
        for id in valid.all() {
            assert!(refusal.contains(id), "{refusal} does not list {id}");
        }
        // An agent none of whose hosts is found is refused, naming each host and --all-clients.
        let absent: Vec<HarnessSummary> = summaries
            .iter()
            .map(|summary| HarnessSummary {
                state: HarnessState::NotDetected,
                ..summary.clone()
            })
            .collect();
        let refusal = select(&absent, &["Cline"], false, true)
            .unwrap_err()
            .to_string();
        assert_eq!(
            refusal,
            "no host of Cline was found for 'Cline'; name one by its host id (cline-cli, \
             cline-vscode, cline-cursor, cline-windsurf), or add --all-clients to select every \
             host of Cline"
        );
        assert_eq!(select(&absent, &["cline"], true, true).unwrap().len(), 4);
        assert_eq!(
            select(&absent, &["cline-windsurf"], false, true).unwrap(),
            ["cline-windsurf"]
        );
    }

    #[test]
    fn a_run_inside_a_container_changes_no_private_client_and_says_what_it_left() {
        let container = crate::install::container::AppContainer {
            app: "OpenAI.Codex".into(),
        };
        let desktop = HarnessSummary {
            can_install: false,
            private_view: true,
            ..harness("claude-desktop", "Claude Desktop", HarnessState::Installed)
        };
        for dry_run in [false, true] {
            let ClientStep::Say(line) = client_step(&desktop, false, dry_run, Some(&container))
            else {
                panic!("a removal inside a container must not remove a private entry");
            };
            assert!(line.starts_with("Claude Desktop: "), "{line}");
            assert!(line.contains("private to OpenAI.Codex"), "{line}");
            assert!(line.contains("entry stays in place"), "{line}");
            assert!(line.contains("Ghostlight's uninstall"), "{line}");
        }
        assert_eq!(
            client_step(&desktop, true, false, Some(&container)),
            ClientStep::LeavePrivate
        );
        let ClientStep::Say(line) = client_step(&desktop, true, true, Some(&container)) else {
            panic!("a dry run inside a container must say what it would leave");
        };
        assert!(line.contains("not written from here") && line.contains("Ghostlight's install"));

        let cursor = harness("cursor", "Cursor", HarnessState::Installed);
        assert_eq!(
            client_step(&cursor, false, false, Some(&container)),
            ClientStep::Apply(HarnessAction::Uninstall)
        );
        assert_eq!(
            client_step(&cursor, true, false, None),
            ClientStep::Apply(HarnessAction::Install)
        );
    }

    #[test]
    fn a_dry_run_removal_says_what_uninstall_would_change_in_every_copy() {
        use crate::install::Attention;

        let plain = r"C:\Users\Pat\AppData\Roaming\Claude\claude_desktop_config.json";
        let said = |summary: &HarnessSummary| match client_step(summary, false, true, None) {
            ClientStep::Say(line) => line,
            other => panic!("a dry run changes nothing: {other:?}"),
        };
        let elsewhere = format!(
            "Would remove Ghostlight from another copy of Claude Desktop's settings, at {plain}."
        );

        // The chosen file has no entry and another copy holds Ghostlight's own: Uninstall cleans
        // that copy, so the dry run names it.
        let available = HarnessSummary {
            can_uninstall: true,
            other_copy: Some(plain.into()),
            owned_copies: vec![plain.into()],
            ..harness("claude-desktop", "Claude Desktop", HarnessState::Available)
        };
        assert_eq!(said(&available), elsewhere);

        // The chosen file holds an entry Ghostlight does not own, and another copy holds
        // Ghostlight's: Uninstall leaves the chosen file and cleans the other copy.
        let foreign = HarnessSummary {
            state: HarnessState::NeedsAttention,
            attention: Some(Attention::ForeignEntry),
            ..available.clone()
        };
        assert_eq!(said(&foreign), elsewhere);

        // No copy holds Ghostlight's entry: nothing would change, and a file that could not be
        // parsed is said as that, not as a foreign entry.
        let none = HarnessSummary {
            other_copy: Some(plain.into()),
            ..harness("claude-desktop", "Claude Desktop", HarnessState::Available)
        };
        assert_eq!(
            said(&none),
            "Claude Desktop has no Ghostlight entry; nothing would change."
        );
        let malformed = HarnessSummary {
            attention: Some(Attention::Malformed),
            ..harness(
                "claude-desktop",
                "Claude Desktop",
                HarnessState::NeedsAttention,
            )
        };
        assert_eq!(
            said(&malformed),
            "Would leave Claude Desktop as it is: its settings file could not be parsed."
        );
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
            || browsers,
            setup_complete,
            window_shown,
        ))
    }

    fn two_agents_changed() -> Collected {
        Collected {
            path_written: false,
            agents: vec![agent("Claude Code", true, true), agent("Codex", true, true)],
            mcp_command: "/home/ghostlight mcp".into(),
            ..Collected::default()
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
    fn an_extension_already_in_a_profile_asks_for_that_browser_as_the_window_does() {
        use crate::language::setup::ProfileFact;

        let mut chrome = native_chrome();
        chrome.extension = Some(ProfileFact {
            directory: "Default".into(),
            name: None,
            profiles: 1,
        });
        let lines = lines_of(&two_agents_changed(), vec![chrome], false, false);
        assert_eq!(
            lines.last().unwrap(),
            "Next: open Chrome, where Ghostlight is already added."
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
    fn a_complete_setup_reads_no_browser_profile() {
        use super::install_facts;
        use crate::language::environment::Environment;

        let facts = install_facts(
            &two_agents_changed(),
            "2.0.0".into(),
            Environment::Windows,
            || panic!("a complete setup scanned the browser profiles"),
            true,
            false,
        );
        assert!(facts.browsers.is_empty());
        let facts = install_facts(
            &two_agents_changed(),
            "2.0.0".into(),
            Environment::Windows,
            || vec![native_chrome()],
            false,
            false,
        );
        assert_eq!(facts.browsers, vec![native_chrome()]);
    }

    #[test]
    fn a_command_off_the_path_is_named_by_its_full_path() {
        use crate::language::environment::run_by_full_path;

        let command = PathBuf::from("/home/p/.local/bin/ghostlight");
        let collected = Collected {
            command_off_path: Some(command.clone()),
            ..two_agents_changed()
        };
        let lines = lines_of(&collected, vec![native_chrome()], false, true);
        assert_eq!(lines[2], run_by_full_path(&command));
        assert!(!lines.iter().any(|line| line.contains("new terminal")));
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

        // Inside an app container the report is that app's own view: a registration it holds
        // is never blamed, and one the install could not judge cannot be checked from here.
        report.private_view = Some(crate::install::container::AppContainer {
            app: "Claude".into(),
        });
        assert!(browser_attention_lines(&report, None, false).is_empty());
        report.browsers[2].state = NativeHostState::NeedsAttention;
        let lines = browser_attention_lines(&report, None, true);
        assert_eq!(
            lines,
            [format!(
                "Browser: brave -- {} -- cannot be checked from here -- Claude's own view of the \
                 registry; run ghostlight doctor from a regular terminal.",
                report.browsers[2].package_detail
            )]
        );
        assert!(super::not_checkable(&report, &report.browsers[2]));
        assert!(!super::not_checkable(&report, &report.browsers[0]));
        report.private_view = None;
        assert!(!super::not_checkable(&report, &report.browsers[2]));
    }
}
