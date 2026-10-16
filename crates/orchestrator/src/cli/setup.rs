//! Browser and client setup, installation, and registration workflows.
//!
//! This module coordinates machine-level integration: registering the native
//! messaging host with Chromium browsers, updating MCP client configurations,
//! and handling uninstallation.
//!
//! Inside a Windows app container the registry and `AppData` writes reach only that app, so an
//! install there leaves those registrations unwritten, says which, and names the one command that
//! finishes them from a regular terminal (see `crate::install::container`).

use crate::cli::parse::{NativeHostCommand, SetupOptions};
use crate::install::container::{self, AppContainer};
use crate::install::deployment::{stop_found_strays, StrayStop, SystemProcessControl};
use crate::install::layout::Layout;
use crate::install::version_directories;
use crate::language::deployment as deployment_words;
use crate::language::workbench as stray_words;

/// Registrations an install left unwritten because the app container it ran in keeps them private.
#[derive(Default)]
struct KeptPrivate {
    container: Option<AppContainer>,
    registrations: Vec<String>,
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
    use crate::install::container::Residence;
    use crate::install::desktop_entry::DesktopIntegration;
    use crate::install::native_host::{NativeHostRegistry, NativeHostState};
    use crate::install::{HarnessAction, HarnessRegistry};
    use crate::language::environment;

    ghostlight_relay::home::home()?;
    if install && !options.dry_run {
        stop_older_installation_programs()?;
    }
    let native_hosts = NativeHostRegistry::discover();
    let mut kept_private = KeptPrivate::default();
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
        println!(
            "Browser connection {}; changed: {}",
            if result.private.is_empty() {
                "installed"
            } else {
                "prepared"
            },
            result.changed
        );
        print_native_host_report(&result.report);
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
        if result.report.state != CommandPathState::NotApplicable {
            print_command_path_report(&result.report);
        }
        if install && result.report.state == CommandPathState::PrivateView {
            if kept_private.container.is_none() {
                kept_private.container = Residence::detect().container().cloned();
            }
            kept_private
                .registrations
                .push(PATH_ENTRY_REGISTRATION.to_owned());
        }
        if result.needs_new_terminal {
            println!("{}", environment::OPEN_NEW_TERMINAL);
        }
    }

    let user_assets = crate::install::user_assets::UserAssets::discover();
    if !options.dry_run {
        let result = if install {
            user_assets.install()?
        } else {
            user_assets.uninstall()?
        };
        if result.report.state != crate::install::user_assets::UserAssetState::NotApplicable {
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
            != crate::install::desktop_entry::DesktopIntegrationState::NotApplicable
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
        return conclude(install, options, install_usable, &kept_private);
    }

    if options.no_clients {
        println!("MCP client configuration was left unchanged.");
        return conclude(install, options, install_usable, &kept_private);
    }

    let harnesses = HarnessRegistry::discover();
    let summaries = harnesses.refresh()?;
    let attention_count = summaries
        .iter()
        .filter(|summary| summary.state == crate::install::HarnessState::NeedsAttention)
        .inspect(|summary| {
            eprintln!(
                "MCP client needs attention: {} -- {}",
                summary.label, summary.detail
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
        return conclude(install, options, install_usable, &kept_private);
    }
    let mut failures = Vec::new();
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
            Ok(result) => println!("{}", result.message),
            Err(error) => {
                eprintln!("{}: {error}", summary.label);
                failures.push(summary.label);
            }
        }
    }
    conclude(install, options, install_usable, &kept_private)?;
    if !failures.is_empty() {
        anyhow::bail!(
            "Ghostlight could not update {} MCP client integration(s)",
            failures.len()
        );
    }
    Ok(())
}

/// Stop what an older Ghostlight installation left running, so one install leaves nothing behind
/// that `doctor` would name. The service that answers is spared unless it runs from the retired
/// development directory or from a 1.x version directory, which an install removes, so that one
/// is stopped too. A program that will not stop is said so and does not fail the install.
fn stop_older_installation_programs() -> anyhow::Result<()> {
    let layout = Layout::current()?;
    let service_pid = crate::cli::client::status_at(layout.home())
        .ok()
        .map(|status| status.pid);
    let stop = stop_found_strays(&SystemProcessControl, &layout, service_pid);
    for line in stray_stop_lines(&stop) {
        println!("{line}");
    }
    Ok(())
}

/// Archive the audit log of each 1.x version directory in `layout`'s `bin`, remove the directory,
/// and say what became of each. A directory that cannot be removed is said so and does not fail
/// the install: the next one tries again.
fn remove_version_directories(layout: &Layout) {
    let directories = version_directories::find(layout);
    let archives = version_directories::archives(layout, &directories);
    let retired = version_directories::retire(&directories, &archives);
    for directory in &retired.removed {
        println!(
            "{}",
            deployment_words::removed_version_directory(&directory.display().to_string())
        );
        if let Some(archive) = version_directories::archive_of(&retired.archived, directory) {
            println!(
                "{}",
                deployment_words::archived_version_audit(&archive.to.display().to_string())
            );
        }
    }
    for directory in &retired.kept {
        eprintln!(
            "{}",
            deployment_words::kept_entry(&directory.display().to_string())
        );
    }
}

/// One line for each stray `stop` stopped and each it could not.
fn stray_stop_lines(stop: &StrayStop) -> Vec<String> {
    let image =
        |stray: &crate::install::deployment::StrayProcess| stray.image.display().to_string();
    stop.stopped
        .iter()
        .map(|stray| stray_words::stray_stopped_line(&image(stray)))
        .chain(
            stop.stuck
                .iter()
                .map(|stray| stray_words::stray_not_stopped_line(&image(stray))),
        )
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
/// private, name them and the command that finishes them; otherwise present the ready summary.
fn conclude(
    install: bool,
    options: &SetupOptions,
    install_usable: bool,
    kept_private: &KeptPrivate,
) -> anyhow::Result<()> {
    let container = match &kept_private.container {
        Some(container)
            if install && !options.dry_run && !kept_private.registrations.is_empty() =>
        {
            container
        }
        _ => return finish_setup(install, options, install_usable),
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

/// Finish the setup sequence by presenting environment status and opening the walkthrough.
pub fn finish_setup(
    install: bool,
    options: &SetupOptions,
    install_usable: bool,
) -> anyhow::Result<()> {
    use crate::install::handoff::{self, HandoffOutcome, EXTENSION_INSTALL_URL};

    if !install || options.dry_run {
        return Ok(());
    }
    if !install_usable {
        anyhow::bail!("Ghostlight could not establish a usable browser registration");
    }

    use crate::language::environment;

    println!();
    println!("Ghostlight's local connection is ready.");
    let here = environment::current();
    println!("{}", here.location());
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
pub(crate) fn print_native_host_report(report: &crate::install::native_host::NativeHostReport) {
    println!("Browser connector: {}", report.connector.display());
    for browser in &report.browsers {
        match &report.private_view {
            Some(container) => println!(
                "Browser: {} -- {} -- {} -- Windows gives {} its own view of the registry.",
                browser.name,
                browser.package_detail,
                container::NOT_VISIBLE,
                container.app
            ),
            None => println!(
                "Browser: {} -- {} -- {} -- {}",
                browser.name,
                browser.package_detail,
                browser.state.label(),
                browser.detail
            ),
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
    use crate::install::native_host::{NativeHostRegistry, NativeHostState};

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
    println!("Connector: {}", report.connector.display());
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

    use super::{private_browser_registrations, select_install_browsers, stray_stop_lines};

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
    fn an_install_says_what_it_stopped_and_what_it_could_not() {
        use crate::install::deployment::{StrayKind, StrayProcess, StrayStop};

        let stray = |image: &str| StrayProcess {
            pid: 3,
            image: PathBuf::from(image),
            kind: StrayKind::RetiredConnector,
        };
        assert!(stray_stop_lines(&StrayStop::default()).is_empty());
        let lines = stray_stop_lines(&StrayStop {
            stopped: vec![stray("a.exe")],
            stuck: vec![stray("b.exe")],
        });
        assert_eq!(
            lines,
            ["Stopped a.exe.", "Could not stop b.exe. Close it yourself."]
        );
    }
}
