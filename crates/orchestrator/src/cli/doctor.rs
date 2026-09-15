//! Comprehensive local installation health check and diagnosis.
//!
//! This module gathers environmental observations across binaries, package registrations,
//! MCP harness configs, service runtime status, and diagnostics, and can optionally apply
//! non-destructive repairs.

use std::path::PathBuf;

use crate::cli::parse::SetupOptions;
use crate::cli::setup::{
    print_command_path_report, print_desktop_integration_report, print_native_host_report,
    run_setup,
};
use crate::cli::status::{
    observe_runtime, render_runtime_status, runtime_document, RuntimeObservation,
};

/// Everything `doctor` observed, gathered once.
///
/// The text and JSON renderings read the same values, so a script and a person cannot be told
/// different things about the same machine.
pub struct DoctorObservation {
    installation: Option<ghostlight_bridge::installation::Installation>,
    environment: crate::language::environment::Environment,
    binaries: Vec<(PathBuf, bool)>,
    sibling_set_ready: bool,
    native_host: crate::install::native_host::NativeHostReport,
    command_path: crate::install::command_path::CommandPathReport,
    user_assets: crate::install::user_assets::UserAssetReport,
    desktop: crate::install::desktop_entry::DesktopIntegrationReport,
    harnesses: Vec<crate::install::HarnessSummary>,
    runtime: RuntimeObservation,
    readiness: Option<crate::workbench::ReadinessSummary>,
    diagnostics: crate::diagnostics::DiagnosticsReport,
}

/// Observe complete local system state for diagnostic reporting.
pub fn observe_doctor() -> anyhow::Result<DoctorObservation> {
    use crate::install::desktop_entry::DesktopIntegration;
    use crate::install::native_host::NativeHostRegistry;
    use crate::install::HarnessRegistry;
    use crate::language::environment;

    let executable = ghostlight_bridge::installation::selected_executable()?;
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
    let runtime = observe_runtime()?;
    let readiness = if runtime.running {
        crate::service::request_readiness(&runtime.path).ok()
    } else {
        None
    };
    let diagnostics = crate::diagnostics::observe(&ghostlight_bridge::runtime::runtime_file()?);
    Ok(DoctorObservation {
        installation: ghostlight_bridge::installation::read(&runtime.path)?,
        // Same module as the install summary, so the two can never describe this machine
        // differently.
        environment: environment::current(),
        binaries,
        sibling_set_ready,
        native_host: NativeHostRegistry::discover().check()?,
        command_path: crate::install::command_path::CommandPath::discover().check()?,
        user_assets: crate::install::user_assets::UserAssets::discover().check()?,
        desktop: DesktopIntegration::discover().check()?,
        harnesses: HarnessRegistry::discover().refresh()?,
        runtime,
        readiness,
        diagnostics,
    })
}

/// Run the `ghostlight doctor` health check command.
pub fn run_doctor(fix: bool, json: bool) -> anyhow::Result<()> {
    let observation = observe_doctor()?;
    if json {
        println!("{}", serde_json::to_string(&doctor_document(&observation))?);
        return Ok(());
    }
    println!("Ghostlight {} diagnostics", env!("CARGO_PKG_VERSION"));
    if let Some(installation) = &observation.installation {
        println!(
            "Selected authority: {} -- {}",
            installation.directory().display(),
            installation.source()
        );
        println!("Shared runtime: {}", observation.runtime.path.display());
    }
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
    if observation.user_assets.state != crate::install::user_assets::UserAssetState::NotApplicable {
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
    match (report.layer, &report.directory) {
        (layer, Some(directory)) => println!(
            "Process diagnostics: {layer} -- {} bytes of log in {directory}",
            report.used_bytes
        ),
        (layer, None) => println!(
            "Process diagnostics: {layer} -- run ghostlight diagnostics on to turn them on"
        ),
    }
    if fix {
        println!("Applying ownership-safe repairs.");
        run_setup(true, &SetupOptions::default())?;
    }
    Ok(())
}

/// The JSON document `doctor --json` prints, built from the same observation the text path uses.
pub fn doctor_document(observation: &DoctorObservation) -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "installation": {
            "runtime_file": observation.runtime.path,
            "selection": observation.installation,
            "source": observation.installation.as_ref().map(|selection| selection.source()),
            "invoking_executable": std::env::current_exe().ok(),
        },
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

/// Format a readiness summary line for output.
pub fn readiness_line(readiness: &crate::workbench::ReadinessSummary) -> String {
    format!("Readiness: {} -- {}", readiness.word, readiness.detail)
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::readiness_line;

    #[test]
    fn doctor_renders_every_workbench_readiness_state_in_the_same_words() {
        use crate::language::readiness::Readiness;
        use crate::workbench::ReadinessSummary;

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
        use crate::install::command_path::CommandPathState;
        use crate::install::desktop_entry::DesktopIntegrationState;
        use crate::install::native_host::NativeHostState;
        use crate::install::user_assets::UserAssetState;
        use crate::install::HarnessState;

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
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for file in ["src/cli/doctor.rs", "src/cli/setup.rs"] {
            let source = std::fs::read_to_string(manifest_dir.join(file))
                .unwrap_or_else(|_| panic!("{file} is readable"));
            for line in source.lines() {
                let reports_state = line.contains("Browser: {")
                    || line.contains("Applications: {")
                    || line.contains("MCP client: {")
                    || line.contains("Command: {")
                    || line.contains("Documentation: {");
                assert!(
                    !(reports_state && line.contains("{:?}")),
                    "{file} line uses debug formatting for a state: {line}"
                );
            }
        }
    }
}
