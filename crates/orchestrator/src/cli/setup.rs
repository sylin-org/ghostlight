//! Browser and client setup, installation, and registration workflows.
//!
//! This module coordinates machine-level integration: registering the native
//! messaging host with Chromium browsers, updating MCP client configurations,
//! and handling uninstallation and migration.

use crate::cli::parse::{NativeHostCommand, SetupOptions};

/// Run installation or uninstallation across detected browsers and MCP clients.
pub fn run_setup(install: bool, options: &SetupOptions) -> anyhow::Result<()> {
    use crate::install::desktop_entry::DesktopIntegration;
    use crate::install::native_host::{NativeHostRegistry, NativeHostState};
    use crate::install::{HarnessAction, HarnessRegistry};

    if install && !options.dry_run {
        crate::install::deployment::select(ghostlight_bridge::installation::Selection::Release)?;
    } else if !install && !crate::install::deployment::owns_registration()? {
        println!("The active Ghostlight installation is preserved; this package owns no active registrations.");
        return Ok(());
    }
    ghostlight_bridge::runtime::runtime_discovery()?;
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
        let migration = crate::install::migration::retire_obsolete_supervisor();
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

    let command_path = crate::install::command_path::CommandPath::discover();
    if options.dry_run {
        print_command_path_report(&command_path.check()?);
    } else {
        let result = if install {
            command_path.install()?
        } else {
            command_path.uninstall()?
        };
        if result.report.state != crate::install::command_path::CommandPathState::NotApplicable {
            print_command_path_report(&result.report);
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
        .filter(|summary| summary.state == crate::install::HarnessState::NeedsAttention)
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
        println!(
            "Browser: {} -- {} -- {} -- {}",
            browser.name,
            browser.package_detail,
            browser.state.label(),
            browser.detail
        );
    }
}

/// Print the command path link status report to stdout.
pub(crate) fn print_command_path_report(report: &crate::install::command_path::CommandPathReport) {
    use crate::install::command_path::CommandPathState;

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

/// Print the desktop applications entry report to stdout.
pub(crate) fn print_desktop_integration_report(
    report: &crate::install::desktop_entry::DesktopIntegrationReport,
) {
    println!(
        "Applications: {} -- {} -- {}",
        report.state.label(),
        report.detail,
        report.desktop_entry.display()
    );
}

/// Execute a native host registration subcommand.
pub fn run_native_host(command: NativeHostCommand) -> anyhow::Result<()> {
    use crate::install::native_host::{NativeHostRegistry, NativeHostState};

    if command == NativeHostCommand::Install {
        crate::install::deployment::select(ghostlight_bridge::installation::Selection::Release)?;
    } else if command == NativeHostCommand::Uninstall
        && !crate::install::deployment::owns_registration()?
    {
        println!("The active Ghostlight browser registration is preserved.");
        return Ok(());
    }
    ghostlight_bridge::runtime::runtime_discovery()?;
    let registry = NativeHostRegistry::discover();
    let (verb, changed, report, migration) = match command {
        NativeHostCommand::Check => ("checked", false, registry.check()?, None),
        NativeHostCommand::Install => {
            let result = registry.install();
            let migration = crate::install::migration::retire_obsolete_supervisor();
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
            NativeHostState::OwnedElsewhere => "owned by another installation",
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

/// Manage Flatpak Chromium native messaging registration on Linux.
#[cfg(target_os = "linux")]
pub fn run_flatpak_native_host(
    command: NativeHostCommand,
    allow_activation: bool,
) -> anyhow::Result<()> {
    if command == NativeHostCommand::Install {
        crate::install::deployment::select(ghostlight_bridge::installation::Selection::Release)?;
    } else if command == NativeHostCommand::Uninstall
        && !crate::install::deployment::owns_registration()?
    {
        println!("The active Ghostlight Flatpak registration is preserved.");
        return Ok(());
    }
    let registry = crate::install::flatpak::FlatpakRegistry::discover()?;
    let report = match command {
        NativeHostCommand::Check => registry.check()?,
        NativeHostCommand::Install => {
            if allow_activation {
                eprintln!("Selected app-wide permission: Flatpak Chromium may start the one installed Ghostlight host application. No general host execution is granted. Existing browser sandboxes need a restart for this new permission; Ghostlight will not restart them.");
            }
            registry.install(allow_activation)?
        }
        NativeHostCommand::Uninstall => registry.uninstall()?,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::cli::parse::SetupOptions;
    use crate::install::browser_package::BrowserPackage;
    use crate::install::native_host::{BrowserRegistration, NativeHostReport, NativeHostState};

    use super::select_install_browsers;

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
