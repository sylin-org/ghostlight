//! Comprehensive local installation health check and diagnosis.
//!
//! This module gathers environmental observations across binaries, package registrations,
//! MCP harness configs, the agents' cached icons, the service endpoint and status, and
//! diagnostics, and can optionally apply non-destructive repairs.

use std::path::PathBuf;

use crate::cli::parse::SetupOptions;
use crate::cli::setup::{
    print_command_path_report, print_desktop_integration_report, print_native_host_report,
    run_setup,
};
use crate::cli::status::{
    observe_runtime, render_runtime_status, runtime_document, RuntimeObservation,
};
use crate::install::container;
use crate::install::deployment::{stray_processes, StrayProcess};
use crate::install::icons::{AgentIcon, IconCache, IconPlaces};
use crate::install::layout::Layout;
use crate::install::record::{Channel, InstallRecord};
use crate::install::version_directories;
use crate::language::agent_icon;
use crate::language::deployment as deployment_words;
use crate::language::workbench as words;

/// Everything `doctor` observed, gathered once.
///
/// The text and JSON renderings read the same values, so a script and a person cannot be told
/// different things about the same machine.
pub struct DoctorObservation {
    home: PathBuf,
    installed: Option<InstallRecord>,
    environment: crate::language::environment::Environment,
    binaries: Vec<(PathBuf, bool)>,
    sibling_set_ready: bool,
    native_host: crate::install::native_host::NativeHostReport,
    command_path: crate::install::command_path::CommandPathReport,
    user_assets: crate::install::user_assets::UserAssetReport,
    desktop: crate::install::desktop_entry::DesktopIntegrationReport,
    harnesses: Vec<crate::install::HarnessSummary>,
    /// Every catalog agent and the icon the service's cache holds for it. Doctor reads the cache
    /// and looks at no app.
    icons: Vec<AgentIcon>,
    runtime: RuntimeObservation,
    strays: Vec<StrayProcess>,
    version_directories: Vec<PathBuf>,
    readiness: Option<crate::workbench::ReadinessSummary>,
    diagnostics: crate::diagnostics::DiagnosticsReport,
}

impl DoctorObservation {
    /// The sentence that names the way back to a release, when the record says a development
    /// build is installed and one of its executables is gone. No command works from such an
    /// installation, so this is the line a person needs.
    fn missing_development_executable(&self) -> Option<String> {
        let development = self
            .installed
            .as_ref()
            .is_some_and(|record| record.channel == Channel::Development);
        let (missing, _) = self.binaries.iter().find(|(_, ready)| !ready)?;
        development.then(|| {
            deployment_words::development_executable_missing(&missing.display().to_string())
        })
    }
}

/// Observe complete local system state for diagnostic reporting.
pub fn observe_doctor() -> anyhow::Result<DoctorObservation> {
    use crate::install::desktop_entry::DesktopIntegration;
    use crate::install::native_host::NativeHostRegistry;
    use crate::install::HarnessRegistry;
    use crate::language::environment;

    let layout = Layout::current()?;
    let mut sibling_set_ready = true;
    let mut binaries = Vec::new();
    for path in [layout.service_executable(), layout.edge()] {
        let ready = path.is_file();
        sibling_set_ready &= ready;
        binaries.push((path, ready));
    }
    let runtime = observe_runtime()?;
    let readiness = if runtime.running() {
        crate::cli::client::readiness().ok()
    } else {
        None
    };
    let diagnostics = crate::diagnostics::observe(layout.home());
    let strays = stray_processes(&layout, runtime.status.as_ref().map(|status| status.pid));
    Ok(DoctorObservation {
        installed: InstallRecord::read(&layout.install_record())?,
        home: layout.home().to_path_buf(),
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
        icons: IconCache::open(layout.icons(), IconPlaces::system(), None).agents(),
        runtime,
        strays,
        version_directories: version_directories::find(&layout),
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
    println!("Home: {}", observation.home.display());
    println!("{}", words::endpoint_line(&observation.runtime.endpoint));
    match &observation.installed {
        Some(record) => println!(
            "Installed: Ghostlight {} -- relay {}",
            record.service_version, record.relay_revision
        ),
        None => println!("Installed: nothing recorded in the Ghostlight home"),
    }
    if observation
        .installed
        .as_ref()
        .is_some_and(|record| record.channel == Channel::Development)
    {
        println!("{}", words::DEVELOPMENT_CHANNEL_LINE);
    }
    if let Some(line) = observation.missing_development_executable() {
        println!("{line}");
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
    } else if observation.runtime.running() {
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
    if let Some(container) = &observation.native_host.private_view {
        println!(
            "Registrations: {}",
            container::cannot_see_line(container, &Layout::new(observation.home.clone()))
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
        if harness.private_view {
            println!("{}", container::client_cannot_see_line(&harness.label));
            continue;
        }
        println!(
            "MCP client: {} -- {} -- {}",
            harness.label,
            harness.state.label(),
            harness.detail
        );
        if let Some(evidence) = &harness.evidence {
            println!("  {evidence}");
        }
        if let Some(settings) = deployment_words::client_settings_line(
            &harness.config_path,
            harness.packaged,
            harness.other_copy.as_deref(),
        ) {
            println!("  {settings}");
        }
    }
    for line in agent_icon::doctor_lines(&observation.icons) {
        println!("{line}");
    }
    render_runtime_status(&observation.runtime, false, observation.sibling_set_ready);
    for stray in &observation.strays {
        println!("{}", words::stray_line(&stray.image.display().to_string()));
    }
    for directory in &observation.version_directories {
        println!(
            "{}",
            words::version_directory_line(&directory.display().to_string())
        );
    }
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
        run_setup(
            true,
            &SetupOptions {
                no_open: true,
                ..SetupOptions::default()
            },
        )?;
    }
    Ok(())
}

/// The JSON document `doctor --json` prints, built from the same observation the text path uses.
pub fn doctor_document(observation: &DoctorObservation) -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "installation": {
            "home": observation.home,
            "endpoint": observation.runtime.endpoint,
            "installed": observation.installed,
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
        "browser_program": observation.native_host.connector.display().to_string(),
        "browsers": observation.native_host.browsers,
        "private_view": observation.native_host.private_view,
        "command": observation.command_path,
        "documentation": observation.user_assets,
        "applications": observation.desktop,
        "mcp_clients": observation.harnesses,
        "agent_icons": observation.icons.iter().map(icon_document).collect::<Vec<_>>(),
        "service": runtime_document(&observation.runtime),
        "strays": observation.strays,
        "version_directories": observation.version_directories,
        "readiness": observation.readiness,
        "process_diagnostics": observation.diagnostics,
    })
}

/// What `doctor --json` says about one agent's icon: whether one was found and, when it was, the
/// catalog id of the agent whose app gave it and the kind of place it was read from.
fn icon_document(agent: &AgentIcon) -> serde_json::Value {
    match &agent.found {
        Some(found) => serde_json::json!({
            "agent": agent.agent_id,
            "found": true,
            "app": found.app_id,
            "source": found.found_in,
        }),
        None => serde_json::json!({
            "agent": agent.agent_id,
            "found": false,
        }),
    }
}

/// Format a readiness summary line for output.
pub fn readiness_line(readiness: &crate::workbench::ReadinessSummary) -> String {
    format!("Readiness: {} -- {}", readiness.word, readiness.detail)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{doctor_document, readiness_line, DoctorObservation};

    /// An observation of a machine where the service answers and one retired connector runs.
    fn observation(strays: Vec<crate::install::deployment::StrayProcess>) -> DoctorObservation {
        use crate::install::command_path::{CommandPathReport, CommandPathState};
        use crate::install::desktop_entry::{DesktopIntegrationReport, DesktopIntegrationState};
        use crate::install::native_host::NativeHostReport;
        use crate::install::user_assets::{UserAssetReport, UserAssetState};

        DoctorObservation {
            home: PathBuf::from("home"),
            installed: None,
            environment: crate::language::environment::current(),
            binaries: Vec::new(),
            sibling_set_ready: true,
            native_host: NativeHostReport {
                connector: PathBuf::from("ghostlight"),
                browsers: Vec::new(),
                private_view: None,
            },
            command_path: CommandPathReport {
                state: CommandPathState::NotApplicable,
                detail: String::new(),
                link: PathBuf::new(),
                executable: PathBuf::new(),
            },
            user_assets: UserAssetReport {
                state: UserAssetState::NotApplicable,
                detail: String::new(),
                directory: PathBuf::new(),
            },
            desktop: DesktopIntegrationReport {
                state: DesktopIntegrationState::NotApplicable,
                detail: String::new(),
                desktop_entry: None,
                icon: None,
            },
            harnesses: Vec::new(),
            icons: Vec::new(),
            runtime: crate::cli::status::RuntimeObservation {
                endpoint: "endpoint".into(),
                status: Some(crate::intake::listener::control::Status {
                    service: "2.0.0".into(),
                    instance: "instance_a".into(),
                    pid: 42,
                    started_by: ghostlight_relay::protocol::StartReason::Agent,
                    connections: Some(crate::language::workbench::ConnectionCounts {
                        agents: 1,
                        mcp_clients: 1,
                        command_line: 0,
                        browsers: 1,
                    }),
                    workbench: Some(crate::intake::listener::control::WorkbenchStatus {
                        url: "http://127.0.0.1:47600/".into(),
                        usual_port: true,
                        watching: false,
                    }),
                    window: None,
                }),
            },
            strays,
            version_directories: Vec::new(),
            readiness: None,
            diagnostics: crate::diagnostics::DiagnosticsReport {
                layer: "off",
                directory: None,
                used_bytes: 0,
            },
        }
    }

    #[test]
    fn a_development_build_with_a_missing_executable_gets_the_way_back() {
        use crate::install::record::{Channel, InstallRecord};

        let mut observed = observation(Vec::new());
        observed.binaries = vec![
            (PathBuf::from("bin/ghostlight-service"), false),
            (PathBuf::from("bin/ghostlight"), true),
        ];
        assert_eq!(observed.missing_development_executable(), None);
        observed.installed = Some(InstallRecord {
            service_version: "2.0.0".into(),
            relay_revision: "4".into(),
            service_sha256: "a".into(),
            edge_sha256: None,
            channel: Channel::Release,
        });
        assert_eq!(observed.missing_development_executable(), None);
        if let Some(record) = observed.installed.as_mut() {
            record.channel = Channel::Development;
        }
        let line = observed.missing_development_executable().unwrap();
        assert!(line.contains("bin/ghostlight-service is missing"), "{line}");
        assert!(line.ends_with(&crate::language::deployment::restore_route()));
        observed.binaries[0].1 = true;
        assert_eq!(observed.missing_development_executable(), None);
    }

    #[test]
    fn doctor_says_why_there_is_no_window_in_text_and_in_json() {
        use crate::language::workbench::WindowReason;

        for reason in [
            WindowReason::NoDisplay,
            WindowReason::NoInteractiveDesktop,
            WindowReason::NoShellFile,
            WindowReason::NoWebview,
            WindowReason::NoTray,
            WindowReason::DidNotStay,
        ] {
            let mut observed = observation(Vec::new());
            observed.runtime.status.as_mut().unwrap().window = Some(reason);
            assert_eq!(
                doctor_document(&observed)["service"]["window"],
                reason.as_str()
            );
            assert!(crate::cli::status::runtime_lines(&observed.runtime, false)
                .contains(&reason.sentence().unwrap().to_owned()));
        }
        assert!(doctor_document(&observation(Vec::new()))["service"]
            .get("window")
            .is_none());
    }

    #[test]
    fn doctor_json_carries_the_strays_and_the_service_fields() {
        use crate::install::deployment::{StrayKind, StrayProcess};

        let document = doctor_document(&observation(vec![StrayProcess {
            pid: 99,
            image: PathBuf::from("bin/ghostlight-mcp-connector"),
            kind: StrayKind::RetiredConnector,
        }]));
        assert_eq!(
            document["strays"],
            serde_json::json!([{
                "pid": 99,
                "image": "bin/ghostlight-mcp-connector",
                "kind": "retired_connector",
            }])
        );
        assert_eq!(document["service"]["running"], true);
        assert_eq!(
            document["service"]["connections"],
            serde_json::json!({"agents": 1, "mcp_clients": 1, "command_line": 0, "browsers": 1})
        );
        assert_eq!(
            document["service"]["workbench"],
            serde_json::json!({
                "url": "http://127.0.0.1:47600/",
                "watching": false,
                "usual_port": true,
            })
        );
    }

    #[test]
    fn doctor_json_lists_the_1x_version_directories_left_in_bin() {
        let mut observed = observation(Vec::new());
        assert_eq!(
            doctor_document(&observed)["version_directories"],
            serde_json::json!([])
        );
        observed.version_directories =
            vec![PathBuf::from("bin/v1.3.5"), PathBuf::from("bin/v1.3.6")];
        assert_eq!(
            doctor_document(&observed)["version_directories"],
            serde_json::json!(["bin/v1.3.5", "bin/v1.3.6"])
        );
    }

    #[test]
    fn doctor_json_says_per_agent_whether_an_icon_was_found_and_from_where() {
        use crate::install::icons::{AgentIcon, FoundIcon, IconSourceKind};

        let mut observed = observation(Vec::new());
        observed.icons = vec![
            AgentIcon {
                agent_id: "claude-code",
                name: "Claude Code",
                found: Some(FoundIcon {
                    sha256: "0".repeat(64),
                    app_id: "claude-desktop",
                    app_name: "Claude Desktop",
                    found_in: IconSourceKind::StoreApp,
                }),
            },
            AgentIcon {
                agent_id: "cline",
                name: "Cline",
                found: None,
            },
        ];
        assert_eq!(
            doctor_document(&observed)["agent_icons"],
            serde_json::json!([
                { "agent": "claude-code", "found": true, "app": "claude-desktop", "source": "store_app" },
                { "agent": "cline", "found": false },
            ])
        );
    }

    #[test]
    fn doctor_json_lists_no_strays_when_nothing_is_left_running() {
        assert_eq!(
            doctor_document(&observation(Vec::new()))["strays"],
            serde_json::json!([])
        );
    }

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

    /// Every state doctor reports has plain words, never a Rust identifier; the workbench renders
    /// the same serialized values.
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
            CommandPathState::PrivateView.label(),
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
}
