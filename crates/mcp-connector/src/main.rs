//! `ghostlight-mcp-connector`: keeps one MCP stdio stream alive across service restarts
//! (ADR-0197 Decisions 1, 2, 11 and 12).
//!
//! An MCP client starts this process and speaks MCP on its stdin and stdout. The connector
//! forwards every line to the Ghostlight service over a relay protocol 4 `mcp` stream and every
//! service line back, holding lines while the service is away and starting it when a request
//! needs it. All MCP meaning lives in the service; [`forward`] is the whole relay.

mod forward;
mod hold;

use std::io;
use std::sync::{mpsc, Arc};
use std::time::Instant;

use anyhow::{Context, Result};
use ghostlight_relay::endpoint::{ConnectError, Connection};
use ghostlight_relay::home;
use ghostlight_relay::log::{self, Component, Event as LogEvent};
use ghostlight_relay::os::session;
use ghostlight_relay::protocol::{self, Opening, Reply, StartReason};
use ghostlight_relay::spawn;
use ghostlight_relay::watchdog::ParentWatchdog;
use ghostlight_relay::OPENING_DEADLINE;

use crate::forward::{Environment, Event, Service, SystemClock};

/// The installed service under the fixed home.
struct InstalledService;

impl Service for InstalledService {
    fn connect(&self, opening: &Opening) -> Result<(Connection, Reply), ConnectError> {
        protocol::connect(&home::home()?, opening, Instant::now() + OPENING_DEADLINE)
    }

    fn start(&self, reason: StartReason) -> io::Result<()> {
        spawn::start_service(reason)
    }
}

/// Write one diagnostics line as the MCP connector.
fn record(event: LogEvent, detail: &str) {
    log::record(Component::McpConnector, event, detail);
}

fn main() -> Result<()> {
    record(LogEvent::ProcessStarted, "");
    let environment = Environment {
        service: Arc::new(InstalledService),
        clock: Arc::new(SystemClock),
        log: record,
        poll: None,
    };
    let (events, inbox) = mpsc::channel();
    let parent = events.clone();
    let _watchdog = ParentWatchdog::start(move || {
        let _ = parent.send(Event::ParentGone);
    });
    // The listener is part of the shutdown contract, so a connector that cannot start it does
    // not run.
    let ending = events.clone();
    let _session = session::listen(move |signal| {
        session::stop_on_final(signal, || {
            let _ = ending.send(Event::SessionEnded);
        });
    })
    .context("listen for the end of the session")?;
    let result = forward::run(environment, io::stdin(), io::stdout(), events, inbox);
    record(LogEvent::ProcessExited, "");
    result.context("write to the MCP client")
}

#[cfg(test)]
mod version_tests {
    //! Checks that the crate version major tracks the relay revision.

    /// The crate version major is the relay revision, so a published connector names its revision.
    #[test]
    fn crate_version_major_equals_relay_revision() {
        let major: u16 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
        assert_eq!(major, ghostlight_relay::RELAY);
    }
}
