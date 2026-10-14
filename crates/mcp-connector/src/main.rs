//! `ghostlight-mcp-connector`: keeps one MCP stdio stream alive across service restarts
//! (ADR-0197 Decisions 1, 2, 11 and 12).
//!
//! An MCP client starts this process and speaks MCP on its stdin and stdout. The connector
//! forwards every line to the Ghostlight service over a relay protocol 3 `mcp` stream and every
//! service line back, holding lines while the service is away and starting it when a request
//! needs it. All MCP meaning lives in the service; [`forward`] is the whole relay.

mod forward;
mod hold;

use std::io;
use std::net::TcpStream;
use std::sync::{mpsc, Arc};

use anyhow::{Context, Result};
use ghostlight_relay::client::{self, ConnectError, REPLY_DEADLINE};
use ghostlight_relay::home;
use ghostlight_relay::log::{self, Component, Event as LogEvent};
use ghostlight_relay::opening::{Reply, Stream};
use ghostlight_relay::spawn;
use ghostlight_relay::watchdog::ParentWatchdog;

use crate::forward::{Environment, Event, Service, SystemClock};

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

/// Write one diagnostics line as the MCP connector.
fn record(event: LogEvent, detail: &str) {
    log::record(Component::McpConnector, event, detail);
}

fn main() -> Result<()> {
    record(LogEvent::ProcessStarted, "");
    let environment = Environment {
        service: Arc::new(InstalledService),
        clock: Arc::new(SystemClock),
        templates: home::templates_file().ok(),
        log: record,
        poll: None,
    };
    let (events, inbox) = mpsc::channel();
    let parent = events.clone();
    let _watchdog = ParentWatchdog::start(move || {
        let _ = parent.send(Event::ParentGone);
    });
    let result = forward::run(environment, io::stdin(), io::stdout(), events, inbox);
    record(LogEvent::ProcessExited, "");
    result.context("write to the MCP client")
}
