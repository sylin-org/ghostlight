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
use crate::install::native_host::NativeHostReport;
use crate::install::record::{Channel, InstallRecord};
use crate::install::version_directories;
use crate::language::agent_icon;
use crate::language::deployment as deployment_words;
use crate::language::workbench as words;

/// How many of the first [`DoctorObservation`] binaries every command needs: the service and the
/// edge.
const COMMAND_EXECUTABLES: usize = 2;

/// Everything `doctor` observed, gathered once.
///
/// The text and JSON renderings read the same values, so a script and a person cannot be told
/// different things about the same machine.
pub struct DoctorObservation {
    home: PathBuf,
    installed: Option<InstallRecord>,
    environment: crate::language::environment::Environment,
    /// The installed executables, each with whether it is there: the service and the edge, which
    /// every command needs, then the window program.
    binaries: Vec<(PathBuf, bool)>,
    /// Whether the service and the edge are both there. A missing window program leaves every
    /// command working, so it does not count.
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
    /// build is installed and the service or the edge is gone. No command works from such an
    /// installation, so this is the line a person needs.
    fn missing_development_executable(&self) -> Option<String> {
        let development = self
            .installed
            .as_ref()
            .is_some_and(|record| record.channel == Channel::Development);
        let (missing, _) = self
            .binaries
            .iter()
            .take(COMMAND_EXECUTABLES)
            .find(|(_, ready)| !ready)?;
        development.then(|| {
            deployment_words::development_executable_missing(&missing.display().to_string())
        })
    }
}

/// The executables installed in `layout`'s `bin`, each with whether it is there: the service and
/// the edge, then the window program.
fn installed_binaries(layout: &Layout) -> Vec<(PathBuf, bool)> {
    [
        layout.service_executable(),
        layout.edge(),
        layout.desktop_executable(),
    ]
    .into_iter()
    .map(|path| {
        let ready = path.is_file();
        (path, ready)
    })
    .collect()
}

/// Observe complete local system state for diagnostic reporting.
pub fn observe_doctor() -> anyhow::Result<DoctorObservation> {
    use crate::install::desktop_entry::DesktopIntegration;
    use crate::install::native_host::NativeHostRegistry;
    use crate::install::HarnessRegistry;

    let layout = Layout::current()?;
    let binaries = installed_binaries(&layout);
    let sibling_set_ready = binaries
        .iter()
        .take(COMMAND_EXECUTABLES)
        .all(|(_, ready)| *ready);
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
        environment: super::machine::environment(),
        binaries,
        sibling_set_ready,
        native_host: NativeHostRegistry::discover().check()?,
        command_path: crate::install::command_path::CommandPath::discover().check()?,
        user_assets: crate::install::user_assets::UserAssets::discover().check()?,
        desktop: DesktopIntegration::discover().check()?,
        harnesses: HarnessRegistry::discover().summaries(),
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
    println!(
        "{}",
        deployment_words::doctor_heading(env!("CARGO_PKG_VERSION"))
    );
    println!("{}", deployment_words::home_line(&observation.home));
    println!("{}", words::endpoint_line(&observation.runtime.endpoint));
    println!(
        "{}",
        deployment_words::installed_line(observation.installed.as_ref())
    );
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
        "{}",
        deployment_words::environment_line(
            observation.environment.label(),
            observation.environment.location()
        )
    );
    if let Some(caveat) = observation.environment.caveat() {
        println!("{}", deployment_words::environment_caveat_line(caveat));
    }
    if let Some(readiness) = &observation.readiness {
        println!("{}", readiness_line(readiness));
    } else if observation.runtime.running() {
        println!("{}", deployment_words::READINESS_UNREPORTED);
    }
    for (path, ready) in &observation.binaries {
        println!("{}", deployment_words::binary_line(path, *ready));
    }
    if let Some(container) = &observation.native_host.private_view {
        println!(
            "{}",
            deployment_words::registrations_line(&deployment_words::cannot_see_line(
                container,
                &Layout::new(observation.home.clone())
            ))
        );
    }
    print_native_host_report(&observation.native_host);
    print_command_path_report(&observation.command_path);
    if observation.user_assets.state != crate::install::user_assets::UserAssetState::NotApplicable {
        println!(
            "{}",
            deployment_words::documentation_line(
                observation.user_assets.state,
                &observation.user_assets.detail
            )
        );
    }
    print_desktop_integration_report(&observation.desktop);
    for harness in &observation.harnesses {
        if harness.private_view {
            println!(
                "{}",
                deployment_words::client_cannot_see_line(&harness.label)
            );
            continue;
        }
        println!(
            "{}",
            deployment_words::client_line(&harness.label, harness.state, &harness.detail)
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
    println!(
        "{}",
        deployment_words::diagnostics_line(
            report.layer,
            report.used_bytes,
            report.directory.as_deref()
        )
    );
    if fix {
        println!("{}", deployment_words::APPLYING_REPAIRS);
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
        "browsers": browsers_document(&observation.native_host),
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

/// What `doctor --json` says about each browser: the report's rows, or, inside an app container,
/// each row in the `not_visible` state with the words the text prints, because the registry read
/// there is that app's own and says nothing about what the browser sees.
fn browsers_document(report: &NativeHostReport) -> serde_json::Value {
    let Some(container) = &report.private_view else {
        return serde_json::json!(report.browsers);
    };
    report
        .browsers
        .iter()
        .map(|browser| {
            let mut row = serde_json::json!(browser);
            row["state"] = container::NOT_VISIBLE_STATE.into();
            row["detail"] = deployment_words::registry_view_detail(container).into();
            row
        })
        .collect()
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
    deployment_words::readiness_line(&readiness.word, &readiness.detail)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{doctor_document, installed_binaries, readiness_line, DoctorObservation};

    /// An observation of a machine where the service answers and one retired connector runs.
    fn observation(strays: Vec<crate::install::deployment::StrayProcess>) -> DoctorObservation {
        use crate::install::browser_package::BrowserPackage;
        use crate::install::command_path::{CommandPathReport, CommandPathState};
        use crate::install::desktop_entry::{DesktopIntegrationReport, DesktopIntegrationState};
        use crate::install::native_host::{BrowserRegistration, NativeHostReport, NativeHostState};
        use crate::install::user_assets::{UserAssetReport, UserAssetState};

        DoctorObservation {
            home: PathBuf::from("home"),
            installed: None,
            environment: crate::cli::machine::environment(),
            binaries: Vec::new(),
            sibling_set_ready: true,
            native_host: NativeHostReport {
                connector: PathBuf::from("ghostlight"),
                browsers: vec![BrowserRegistration {
                    id: "chrome".into(),
                    name: "Google Chrome".into(),
                    package: BrowserPackage::Native,
                    package_detail: "Google Chrome is installed.".into(),
                    state: NativeHostState::NeedsAttention,
                    detail: "Malformed or foreign state was found and will not be changed.".into(),
                }],
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

    /// Protects doctor as the support surface, which names what is wrong and the way back. In text
    /// and in JSON it names the window program, why there is no window, an app container's view
    /// instead of blaming the browser, the strays and the service fields (none when nothing is left
    /// running), the 1.x version directories left in `bin`, per agent whether an icon was found and
    /// from where, every workbench readiness state in the same words, and the way back for a
    /// development build with a missing executable.
    #[test]
    fn doctor_names_every_state_a_person_must_act_on_in_text_and_json() {
        {
            use crate::install::layout::{Layout, DESKTOP_EXECUTABLE_NAME};
            use crate::scratch::Scratch;

            let root = Scratch::named("doctor-binaries");
            let layout = Layout::new(root.to_path_buf());
            std::fs::create_dir_all(layout.bin()).unwrap();
            std::fs::write(layout.service_executable(), b"service").unwrap();
            std::fs::write(layout.edge(), b"edge").unwrap();

            let binaries = installed_binaries(&layout);
            assert_eq!(
                binaries,
                vec![
                    (layout.service_executable(), true),
                    (layout.edge(), true),
                    (layout.desktop_executable(), false),
                ]
            );
            assert!(binaries[2].0.ends_with(DESKTOP_EXECUTABLE_NAME));
        }
        {
            use crate::install::record::{Channel, InstallRecord};

            let mut observed = observation(Vec::new());
            observed.binaries = vec![
                (PathBuf::from("bin/ghostlight-service"), false),
                (PathBuf::from("bin/ghostlight"), true),
                (PathBuf::from("bin/ghostlight-desktop"), false),
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
            assert_eq!(
                observed.missing_development_executable(),
                None,
                "every command works without the window program"
            );
        }
        {
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
        {
            use crate::install::container::AppContainer;

            let mut observed = observation(Vec::new());
            let open = doctor_document(&observed);
            assert_eq!(open["browsers"][0]["state"], "needs_attention");
            assert!(
                crate::cli::setup::browser_report_lines(&observed.native_host)[0]
                    .contains("needs attention")
            );

            observed.native_host.private_view = Some(AppContainer {
                app: "Claude".into(),
            });
            let words =
                "Claude's own view of the registry; run ghostlight doctor from a regular terminal.";
            let document = doctor_document(&observed);
            let row = &document["browsers"][0];
            assert_eq!(row["state"], "not_visible");
            assert_eq!(row["detail"], words);
            assert_eq!(row["name"], "Google Chrome");
            assert_eq!(document["private_view"]["app"], "Claude");
            let lines = crate::cli::setup::browser_report_lines(&observed.native_host);
            assert_eq!(
                lines,
                [format!(
                "Browser: Google Chrome -- Google Chrome is installed. -- cannot be checked from \
                 here -- {words}"
            )]
            );
            assert!(!lines[0].contains("Malformed"));
        }
        {
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
        {
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
        {
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
        {
            assert_eq!(
                doctor_document(&observation(Vec::new()))["strays"],
                serde_json::json!([])
            );
        }
        {
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
    }
}
