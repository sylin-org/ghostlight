//! Local service status reporting.
//!
//! This module reads the relay runtime file the service publishes and asks the service for its
//! readiness over a control stream. It never changes machine state and never starts a service.

use std::path::PathBuf;

/// What the local service looks like right now.
#[derive(Clone, Debug)]
pub struct RuntimeObservation {
    /// Path to the relay runtime file.
    pub path: PathBuf,
    /// Reported service version, if a runtime file names a service.
    pub version: Option<String>,
    /// Relay revision the service speaks, if a runtime file names a service.
    pub relay: Option<u16>,
    /// Whether the service answered a readiness control opening.
    pub running: bool,
}

/// Observe the current status of the local Ghostlight service.
pub fn observe_runtime() -> anyhow::Result<RuntimeObservation> {
    let path = ghostlight_relay::home::runtime_file()?;
    Ok(
        match ghostlight_relay::runtime::RuntimeFile::read(&path)
            .ok()
            .flatten()
        {
            Some(runtime) => RuntimeObservation {
                path,
                version: Some(runtime.service_version),
                relay: Some(runtime.relay),
                running: crate::cli::client::readiness().is_ok(),
            },
            None => RuntimeObservation {
                path,
                version: None,
                relay: None,
                running: false,
            },
        },
    )
}

/// The `status --json` document scripts read.
pub fn runtime_document(observation: &RuntimeObservation) -> serde_json::Value {
    let Some(version) = observation.version.as_deref() else {
        return serde_json::json!({ "running": false });
    };
    serde_json::json!({
        "version": version,
        "relay": observation.relay,
        "running": observation.running,
    })
}

/// Render the observed runtime status to standard output in text or JSON format.
pub fn render_runtime_status(observation: &RuntimeObservation, json: bool, idle_is_ready: bool) {
    if json {
        println!("{}", runtime_document(observation));
        return;
    }
    match observation.version.as_deref() {
        Some(version) => println!(
            "Service: {} -- version {} -- relay {} -- {}",
            observation.path.display(),
            version,
            observation
                .relay
                .map_or_else(String::new, |relay| relay.to_string()),
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

/// Run the `ghostlight status` command.
pub fn run_status(json: bool) -> anyhow::Result<()> {
    if !json {
        println!("Ghostlight {}", env!("CARGO_PKG_VERSION"));
    }
    render_runtime_status(&observe_runtime()?, json, false);
    Ok(())
}
