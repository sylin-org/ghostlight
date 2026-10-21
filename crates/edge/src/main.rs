//! `ghostlight`: the edge (ADR-0199 Decisions 1 and 2).
//!
//! One small executable is the MCP relay, the browser relay and the front door for typed
//! commands. It parses no payload, authors no sentence, stores nothing and knows no verb.
//! [`ghostlight_relay::route::route`] decides what an invocation is, and this file only acts on
//! the answer:
//!
//! - a copy that does not run from `<home>/bin` is a carrier, and runs the service image beside
//!   it with the same arguments;
//! - `mcp` is the MCP relay: it keeps one MCP stdio stream alive across service restarts, holding
//!   lines while the service is away and starting it when a request needs it ([`forward`]);
//! - an argument that is a Chrome extension origin is the browser relay: it carries one native
//!   messaging port to one service connection and passes every frame through ([`port`]);
//! - anything else runs `<home>/bin/ghostlight-service` with the same arguments ([`hand_over`]).
//!
//! The edge registers the session listener in every role, so the end of a session never waits for
//! it and nothing starts once the session is ending. All MCP and adapter meaning lives in the
//! service and the extension.

mod forward;
mod hand_over;
mod hold;
mod port;
mod service;

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{mpsc, Arc};

use anyhow::{Context, Result};
use ghostlight_relay::home::{self, EDGE_EXECUTABLE_NAME, SERVICE_EXECUTABLE_NAME};
use ghostlight_relay::log::{self, Component, Event as LogEvent};
use ghostlight_relay::os::session::{self, SessionListener};
use ghostlight_relay::route::{route, Route};
use ghostlight_relay::watchdog::ParentWatchdog;

use crate::service::InstalledService;

// The edge's version major is the relay revision it speaks, so a published edge names its
// revision in the build every opening carries. Checked when the edge compiles.
const _: () = assert!(
    decimal(env!("CARGO_PKG_VERSION_MAJOR")) == ghostlight_relay::RELAY,
    "the edge's version major is the relay revision"
);

/// The number the decimal digits of `text` spell, for the compile-time check above.
const fn decimal(text: &str) -> u16 {
    let digits = text.as_bytes();
    let mut value = 0_u16;
    let mut index = 0;
    while index < digits.len() {
        assert!(
            digits[index].is_ascii_digit(),
            "a version number is decimal"
        );
        value = value * 10 + (digits[index] - b'0') as u16;
        index += 1;
    }
    value
}

/// Write one diagnostics line as the MCP relay.
fn record_mcp(event: LogEvent, detail: &str) {
    log::record(Component::EdgeMcp, event, detail);
}

/// Write one diagnostics line as the browser relay.
fn record_browser(event: LogEvent, detail: &str) {
    log::record(Component::EdgeBrowser, event, detail);
}

/// `path` with its links resolved, or `path` itself when it does not exist yet: a copy cannot run
/// from a directory that is not there.
fn canonical_or_same(path: PathBuf) -> PathBuf {
    home::canonical(&path).unwrap_or(path)
}

/// Register the listener of a role that has nothing to say goodbye to: it leaves at once on the
/// final signal. A process that cannot listen still does its job.
fn leave_on_final() -> Option<SessionListener> {
    session::listen(|event| session::stop_on_final(event, || std::process::exit(0))).ok()
}

/// Run the service image `executable` with `arguments` and exit with its code.
fn hand_over(executable: &Path, arguments: &[OsString]) -> Result<ExitCode> {
    let _session = leave_on_final();
    let code = hand_over::run(executable, arguments)?;
    std::process::exit(code)
}

/// Keep one MCP stdio stream alive across service restarts.
fn run_mcp() -> Result<ExitCode> {
    record_mcp(LogEvent::ProcessStarted, "");
    let environment = forward::Environment {
        service: Arc::new(InstalledService),
        clock: Arc::new(forward::SystemClock),
        log: record_mcp,
        poll: None,
    };
    let (events, inbox) = mpsc::channel();
    let parent = events.clone();
    let _watchdog = ParentWatchdog::start(move || {
        let _ = parent.send(forward::Event::ParentGone);
    });
    // The listener is part of the shutdown contract, so a relay that cannot start it does not
    // run.
    let ending = events.clone();
    let _session = session::listen(move |signal| {
        session::stop_on_final(signal, || {
            let _ = ending.send(forward::Event::SessionEnded);
        });
    })
    .context("listen for the end of the session")?;
    let result = forward::run(environment, io::stdin(), io::stdout(), events, inbox);
    record_mcp(LogEvent::ProcessExited, "");
    result.context("write to the MCP client")?;
    Ok(ExitCode::SUCCESS)
}

/// Carry the native-messaging port of the extension at `origin` to one service connection.
fn run_browser(origin: String) -> Result<ExitCode> {
    record_browser(LogEvent::ProcessStarted, "");
    let environment = port::Environment {
        service: Arc::new(InstalledService),
        origin,
        absent_after: port::ABSENT_AFTER,
        log: record_browser,
    };
    let (events, inbox) = mpsc::channel();
    let parent = events.clone();
    let _watchdog = ParentWatchdog::start(move || {
        let _ = parent.send(port::Event::ParentGone);
    });
    let ending = events.clone();
    let _session = session::listen(move |signal| {
        session::stop_on_final(signal, || {
            let _ = ending.send(port::Event::SessionEnded);
        });
    })
    .context("listen for the end of the session")?;
    let ended = port::run(environment, io::stdin(), io::stdout(), events, inbox)
        .context("carry the browser port")?;
    record_browser(LogEvent::ProcessExited, ended.as_str());
    Ok(ExitCode::SUCCESS)
}

fn main() -> Result<ExitCode> {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let this_exe = home::canonical(&std::env::current_exe()?)?;
    let bin = canonical_or_same(home::bin()?);
    match route(
        &this_exe,
        &bin,
        OsStr::new(EDGE_EXECUTABLE_NAME),
        &arguments,
    ) {
        Route::Carrier { directory } => {
            hand_over(&directory.join(SERVICE_EXECUTABLE_NAME), &arguments)
        }
        Route::Mcp => run_mcp(),
        Route::Browser { origin } => run_browser(origin),
        Route::Service => hand_over(&home::service_executable()?, &arguments),
    }
}
