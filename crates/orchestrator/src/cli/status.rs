//! Local service status reporting.
//!
//! This module asks the service for its status over a control stream. It never changes machine
//! state and never starts a service.

use ghostlight_relay::endpoint::Endpoint;

use crate::intake::listener::control::Status;
use crate::language::workbench as words;

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
/// A service that did not report its connections or its workbench carries neither field.
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
    document
}

/// The lines the text `status` and `doctor` print about the service, in order.
///
/// A running service gives one sentence counting its connections and, once it serves one, a line
/// for the workbench. When nothing answers, `idle_is_ready` adds that the service starts by itself.
#[must_use]
pub fn runtime_lines(observation: &RuntimeObservation, idle_is_ready: bool) -> Vec<String> {
    let Some(status) = &observation.status else {
        return vec![words::service_not_running_line(idle_is_ready)];
    };
    let mut lines = vec![match status.connections {
        Some(_) => words::service_line(status.connections),
        None => words::SERVICE_UP.to_owned(),
    }];
    if let Some(workbench) = &status.workbench {
        lines.push(words::workbench_line(
            &workbench.url,
            workbench.watching,
            workbench.usual_port,
        ));
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
            }),
        }
    }

    fn counts() -> Option<ConnectionCounts> {
        Some(ConnectionCounts {
            mcp_clients: 2,
            command_line: 0,
            browsers: 2,
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

    #[test]
    fn a_service_with_counts_and_a_workbench_that_others_watch() {
        let seen = observation(counts(), workbench(true, true));
        assert_eq!(
            runtime_lines(&seen, false),
            [
                "Ghostlight service is up; 2 MCP clients and 2 browsers connected.",
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
                "connections": {"mcp_clients": 2, "command_line": 0, "browsers": 2},
                "workbench": {"url": URL, "watching": true, "usual_port": true},
            })
        );
    }

    #[test]
    fn a_workbench_with_watching_off_says_so() {
        let seen = observation(counts(), workbench(false, true));
        assert_eq!(
            runtime_lines(&seen, false)[1],
            "Workbench: http://127.0.0.1:47600/ -- watching is off"
        );
        assert_eq!(runtime_document(&seen)["workbench"]["watching"], false);
    }

    #[test]
    fn a_workbench_on_another_port_says_the_usual_one_was_taken() {
        let seen = observation(counts(), workbench(true, false));
        assert_eq!(
            runtime_lines(&seen, false)[1],
            "Workbench: http://127.0.0.1:47600/ -- others on this computer can watch (the usual port was taken)"
        );
        assert_eq!(runtime_document(&seen)["workbench"]["usual_port"], false);
    }

    #[test]
    fn an_older_service_answers_without_counts_or_a_workbench() {
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
        assert_eq!(runtime_lines(&seen, true), ["Ghostlight service is up."]);
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

    #[test]
    fn no_service_answering_reads_as_not_running() {
        let seen = nothing_answers();
        assert_eq!(
            runtime_lines(&seen, false),
            ["Ghostlight service is not running."]
        );
        assert_eq!(
            runtime_document(&seen),
            json!({"running": false, "endpoint": ENDPOINT})
        );
    }

    #[test]
    fn an_idle_service_that_can_start_says_how_it_starts() {
        assert_eq!(
            runtime_lines(&nothing_answers(), true),
            [
                "Ghostlight service is not running. It starts when a browser or an MCP client connects."
            ]
        );
    }
}
