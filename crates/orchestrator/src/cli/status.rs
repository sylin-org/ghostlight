//! Local service status reporting.
//!
//! This module asks the service for its status over a control stream. It never changes machine
//! state and never starts a service.

use ghostlight_relay::endpoint::Endpoint;

use crate::intake::listener::control::Status;

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
pub fn runtime_document(observation: &RuntimeObservation) -> serde_json::Value {
    match &observation.status {
        Some(status) => serde_json::json!({
            "running": true,
            "version": status.service,
            "instance": status.instance,
            "endpoint": observation.endpoint,
        }),
        None => serde_json::json!({
            "running": false,
            "endpoint": observation.endpoint,
        }),
    }
}

/// Render the observed runtime status to standard output in text or JSON format.
pub fn render_runtime_status(observation: &RuntimeObservation, json: bool, idle_is_ready: bool) {
    if json {
        println!("{}", runtime_document(observation));
        return;
    }
    match &observation.status {
        Some(status) => println!(
            "Service: {} -- version {} -- running",
            observation.endpoint, status.service
        ),
        None if idle_is_ready => println!(
            "Service: ready on demand -- it starts when Chromium or an MCP client connects."
        ),
        None => println!(
            "Service: not running (nothing answers at {})",
            observation.endpoint
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
