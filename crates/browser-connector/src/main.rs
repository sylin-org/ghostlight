//! `ghostlight-browser-connector`: carries one Chrome native-messaging port to one service
//! connection (ADR-0197 Decisions 1, 3, 4 and 11).
//!
//! Chrome starts this process when the extension connects and speaks native messaging on its
//! stdin and stdout. The connector opens a relay protocol 3 `browser` stream and passes every
//! frame through in both directions without looking inside. It starts the service only before
//! its first connection. When the service goes away it tells the extension and waits, and when a
//! service answers again it exits so the extension reconnects with a fresh hello. All adapter
//! meaning lives in the service and the extension; [`port`] is the whole relay.

mod port;

use std::io;
use std::net::TcpStream;
use std::sync::{mpsc, Arc};

use anyhow::{Context, Result};
use ghostlight_relay::client::{self, ConnectError, REPLY_DEADLINE};
use ghostlight_relay::log::{self, Component, Event as LogEvent};
use ghostlight_relay::opening::{Reply, Stream};
use ghostlight_relay::spawn;
use ghostlight_relay::watchdog::ParentWatchdog;

use crate::port::{Environment, Event, Service};

/// The installed service under the fixed home.
struct InstalledService;

impl Service for InstalledService {
    fn connect(&self, stream: Stream) -> Result<(TcpStream, Reply), ConnectError> {
        client::connect(stream, REPLY_DEADLINE)
    }

    fn start(&self) -> io::Result<()> {
        spawn::start_service()
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
        log: record,
    };
    let (events, inbox) = mpsc::channel();
    let parent = events.clone();
    let _watchdog = ParentWatchdog::start(move || {
        let _ = parent.send(Event::ParentGone);
    });
    let ended = port::run(environment, io::stdin(), io::stdout(), events, inbox)
        .context("carry the browser port")?;
    record(LogEvent::ProcessExited, ended.as_str());
    Ok(())
}
