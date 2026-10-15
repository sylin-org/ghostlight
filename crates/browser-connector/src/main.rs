//! `ghostlight-browser-connector`: carries one Chrome native-messaging port to one service
//! connection (ADR-0197 Decisions 1, 3, 4 and 11).
//!
//! Chrome starts this process when the extension connects and speaks native messaging on its
//! stdin and stdout. The connector opens a relay protocol 4 `browser` stream and passes every
//! frame through in both directions without looking inside. It starts the service once, and only
//! before its first connection. When no service is there, at the start or later, it tells the
//! extension and waits, and when a service answers it exits so the extension reconnects with a
//! fresh hello. All adapter meaning lives in the service and the extension; [`port`] is the
//! whole relay.

mod port;

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

use crate::port::{extension_origin, Environment, Event, Service, ABSENT_AFTER};

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

/// Write one diagnostics line as the browser connector.
fn record(event: LogEvent, detail: &str) {
    log::record(Component::BrowserConnector, event, detail);
}

fn main() -> Result<()> {
    record(LogEvent::ProcessStarted, "");
    let environment = Environment {
        service: Arc::new(InstalledService),
        origin: extension_origin(std::env::args().nth(1)),
        absent_after: ABSENT_AFTER,
        log: record,
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
    let ended = port::run(environment, io::stdin(), io::stdout(), events, inbox)
        .context("carry the browser port")?;
    record(LogEvent::ProcessExited, ended.as_str());
    Ok(())
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
