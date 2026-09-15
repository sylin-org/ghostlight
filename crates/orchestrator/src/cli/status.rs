//! Local service runtime endpoint status reporting.
//!
//! This module inspects the local Ghostlight service runtime file and queries
//! endpoint reachability without modifying machine state or starting background services.

use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

/// What the local service endpoint looks like right now.
#[derive(Clone, Debug)]
pub struct RuntimeObservation {
    /// Path to the local runtime endpoint JSON file.
    pub path: PathBuf,
    /// Reported service version, if runtime file exists.
    pub version: Option<String>,
    /// Major protocol version of the service bridge.
    pub service_bridge_major: Option<u16>,
    /// Major protocol version of the browser relay.
    pub browser_relay_major: Option<u16>,
    /// Whether the TCP service port responds to probe connections.
    pub running: bool,
}

/// Observe the current status of the local Ghostlight service endpoint.
pub fn observe_runtime() -> anyhow::Result<RuntimeObservation> {
    let path = ghostlight_bridge::runtime::runtime_file()?;
    Ok(match ghostlight_bridge::runtime::read_runtime(&path) {
        Ok(runtime) => {
            let running = TcpStream::connect_timeout(
                &SocketAddrV4::new(Ipv4Addr::LOCALHOST, runtime.service_port).into(),
                Duration::from_millis(250),
            )
            .is_ok();
            RuntimeObservation {
                path,
                version: Some(runtime.service_version),
                service_bridge_major: Some(runtime.service_bridge_major),
                browser_relay_major: Some(runtime.browser_relay_major),
                running,
            }
        }
        Err(_) => RuntimeObservation {
            path,
            version: None,
            service_bridge_major: None,
            browser_relay_major: None,
            running: false,
        },
    })
}

/// The `status --json` document. Its shape is consumed by scripts and does not change here.
pub fn runtime_document(observation: &RuntimeObservation) -> serde_json::Value {
    let Some(version) = observation.version.as_deref() else {
        return serde_json::json!({ "running": false });
    };
    serde_json::json!({
        "version": version,
        "service_bridge_major": observation.service_bridge_major,
        "browser_relay_major": observation.browser_relay_major,
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
            "Service: {} -- version {} -- bridge {} -- {}",
            observation.path.display(),
            version,
            observation
                .service_bridge_major
                .map_or_else(String::new, |major| major.to_string()),
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
