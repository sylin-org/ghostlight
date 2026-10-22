//! Local service status reporting.
//!
//! This module asks the service for its status over a control stream. It never changes machine
//! state and never starts a service.

use ghostlight_relay::endpoint::Endpoint;

use crate::install::layout::Layout;
use crate::install::record::{Channel, InstallRecord};
use crate::intake::listener::control::Status;
use crate::language::workbench::{self as workbench_words, WindowReason};

/// What the local service looks like right now.
#[derive(Clone, Debug)]
pub struct RuntimeObservation {
    /// The endpoint the service listens on: a pipe name or a socket path.
    pub endpoint: String,
    /// What the service reported, when one answered.
    pub status: Option<Status>,
}

impl RuntimeObservation {
    /// Whether a service answered.
    #[must_use]
    pub fn running(&self) -> bool {
        self.status.is_some()
    }
}

/// Observe the current status of the local Ghostlight service.
pub fn observe_runtime() -> anyhow::Result<RuntimeObservation> {
    let endpoint = Endpoint::for_home(&ghostlight_relay::home::home()?)?.to_string();
    Ok(RuntimeObservation {
        endpoint,
        status: crate::cli::client::status().ok(),
    })
}

/// The `status --json` document scripts read.
///
/// A service that did not report its connections, its workbench or why it has no window carries
/// none of those fields.
pub fn runtime_document(observation: &RuntimeObservation) -> serde_json::Value {
    let Some(status) = &observation.status else {
        return serde_json::json!({
            "running": false,
            "endpoint": observation.endpoint,
        });
    };
    let mut document = serde_json::json!({
        "running": true,
        "version": status.service,
        "instance": status.instance,
        "endpoint": observation.endpoint,
    });
    if let Some(connections) = &status.connections {
        document["connections"] = serde_json::json!(connections);
    }
    if let Some(workbench) = &status.workbench {
        document["workbench"] = serde_json::json!(workbench);
    }
    if let Some(window) = status.window {
        document["window"] = serde_json::json!(window.as_str());
    }
    document
}

/// The lines the text `status` and `doctor` print about the service, in order.
///
/// A running service gives one sentence counting its connections, once it serves one a line for
/// the workbench, and, when there is no window or it is closed, one sentence that says why. When
/// nothing answers, `idle_is_ready` adds that the service starts by itself.
#[must_use]
pub fn runtime_lines(observation: &RuntimeObservation, idle_is_ready: bool) -> Vec<String> {
    let Some(status) = &observation.status else {
        return vec![workbench_words::service_not_running_line(idle_is_ready)];
    };
    let mut lines = vec![match status.connections {
        Some(_) => workbench_words::service_line(status.connections),
        None => workbench_words::SERVICE_UP.to_owned(),
    }];
    if let Some(workbench) = &status.workbench {
        lines.push(workbench_words::workbench_line(
            &workbench.url,
            workbench.watching,
            workbench.usual_port,
        ));
    }
    if let Some(sentence) = status.window.and_then(WindowReason::sentence) {
        lines.push(sentence.to_owned());
    }
    lines
}

/// Render the observed runtime status to standard output in text or JSON format.
pub fn render_runtime_status(observation: &RuntimeObservation, json: bool, idle_is_ready: bool) {
    if json {
        println!("{}", runtime_document(observation));
        return;
    }
    for line in runtime_lines(observation, idle_is_ready) {
        println!("{line}");
    }
}

/// Run the `ghostlight status` command.
pub fn run_status(json: bool) -> anyhow::Result<()> {
    if !json {
        println!("Ghostlight {}", env!("CARGO_PKG_VERSION"));
        let installed = Layout::current()
            .ok()
            .and_then(|layout| InstallRecord::read(&layout.install_record()).ok().flatten());
        if installed.is_some_and(|record| record.channel == Channel::Development) {
            println!("{}", workbench_words::DEVELOPMENT_CHANNEL_LINE);
        }
    }
    render_runtime_status(&observe_runtime()?, json, false);
    Ok(())
}

#[cfg(test)]
mod tests {
    use ghostlight_relay::protocol::StartReason;
    use serde_json::json;

    use super::*;
    use crate::intake::listener::control::WorkbenchStatus;
    use crate::language::workbench::ConnectionCounts;

    const ENDPOINT: &str = "ghostlight-test-endpoint";
    const URL: &str = "http://127.0.0.1:47600/";

    fn observation(
        connections: Option<ConnectionCounts>,
        workbench: Option<WorkbenchStatus>,
    ) -> RuntimeObservation {
        RuntimeObservation {
            endpoint: ENDPOINT.into(),
            status: Some(Status {
                service: "2.0.0".into(),
                instance: "instance_a".into(),
                pid: 42,
                started_by: StartReason::Agent,
                connections,
                workbench,
                window: None,
            }),
        }
    }

    fn counts() -> Option<ConnectionCounts> {
        Some(ConnectionCounts {
            agents: 2,
            mcp_clients: 3,
            command_line: 0,
            browsers: 1,
        })
    }

    fn workbench(watching: bool, usual_port: bool) -> Option<WorkbenchStatus> {
        Some(WorkbenchStatus {
            url: URL.into(),
            usual_port,
            watching,
        })
    }

    fn nothing_answers() -> RuntimeObservation {
        RuntimeObservation {
            endpoint: ENDPOINT.into(),
            status: None,
        }
    }

    /// Protects ghostlight status as the spec words it: a running service gives its counts and its
    /// workbench and who watches, watching off and a workbench on another port say so, an older
    /// service answers without them, each reason for the window gets its sentence and its word (an
    /// open window only a word, no display the spec's sentence), and no service reads as not running
    /// while an idle one that can start says how it starts.
    #[test]
    fn status_says_what_runs_who_watches_and_why_there_is_no_window() {
        {
            let seen = observation(counts(), workbench(true, true));
            assert_eq!(
                runtime_lines(&seen, false),
                [
                    "Ghostlight is running; 2 agents and 1 browser connected.",
                    "Workbench: http://127.0.0.1:47600/ -- others on this computer can watch",
                ]
            );
            assert_eq!(
                runtime_document(&seen),
                json!({
                    "running": true,
                    "version": "2.0.0",
                    "instance": "instance_a",
                    "endpoint": ENDPOINT,
                    "connections": {"agents": 2, "mcp_clients": 3, "command_line": 0, "browsers": 1},
                    "workbench": {"url": URL, "watching": true, "usual_port": true},
                })
            );
        }
        {
            let seen = observation(counts(), workbench(false, true));
            assert_eq!(
                runtime_lines(&seen, false)[1],
                "Workbench: http://127.0.0.1:47600/ -- watching is off"
            );
            assert_eq!(runtime_document(&seen)["workbench"]["watching"], false);
        }
        {
            let seen = observation(counts(), workbench(true, false));
            assert_eq!(
            runtime_lines(&seen, false)[1],
            "Workbench: http://127.0.0.1:47600/ -- others on this computer can watch (the usual port was taken)"
        );
            assert_eq!(runtime_document(&seen)["workbench"]["usual_port"], false);
        }
        {
            let older: Status = serde_json::from_value(json!({
                "service": "1.9.0",
                "instance": "instance_old",
                "pid": 7,
                "started_by": "agent",
            }))
            .unwrap();
            let seen = RuntimeObservation {
                endpoint: ENDPOINT.into(),
                status: Some(older),
            };
            assert_eq!(runtime_lines(&seen, true), ["Ghostlight is running."]);
            assert_eq!(
                runtime_document(&seen),
                json!({
                    "running": true,
                    "version": "1.9.0",
                    "instance": "instance_old",
                    "endpoint": ENDPOINT,
                })
            );
        }
        {
            for reason in [
                WindowReason::Tray,
                WindowReason::NoDisplay,
                WindowReason::NoInteractiveDesktop,
                WindowReason::NoShellFile,
                WindowReason::NoWebview,
                WindowReason::WindowFailed,
                WindowReason::NoTray,
                WindowReason::DidNotStay,
            ] {
                let mut seen = observation(counts(), workbench(true, true));
                seen.status.as_mut().unwrap().window = Some(reason);
                let lines = runtime_lines(&seen, false);
                assert_eq!(lines.len(), 3, "{reason:?}");
                assert_eq!(lines[2], reason.sentence().unwrap());
                assert_eq!(runtime_document(&seen)["window"], reason.as_str());
            }
            assert_eq!(
                runtime_lines(&nothing_answers(), false).len(),
                1,
                "no service, no window sentence"
            );
        }
        {
            let mut seen = observation(counts(), workbench(true, true));
            seen.status.as_mut().unwrap().window = Some(WindowReason::Shown);
            assert_eq!(runtime_lines(&seen, false).len(), 2);
            assert_eq!(runtime_document(&seen)["window"], "shown");
            seen.status.as_mut().unwrap().window = None;
            assert!(runtime_document(&seen).get("window").is_none());
        }
        {
            assert_eq!(
                WindowReason::NoDisplay.sentence(),
                Some("No Ghostlight window: no display is reachable from the service.")
            );
        }
        {
            let seen = nothing_answers();
            assert_eq!(runtime_lines(&seen, false), ["Ghostlight is not running."]);
            assert_eq!(
                runtime_document(&seen),
                json!({"running": false, "endpoint": ENDPOINT})
            );
        }
        {
            assert_eq!(
                runtime_lines(&nothing_answers(), true),
                ["Ghostlight is not running. It starts when a browser or an agent connects."]
            );
        }
    }
}
