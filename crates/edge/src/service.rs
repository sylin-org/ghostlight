//! The installed service as the edge's relays reach it.
//!
//! Both relays connect to the service under the fixed home and start it the same way: through
//! [`ghostlight_relay::spawn::start_service`], which refuses while the session is ending. They are
//! woken the same way too, through [`ghostlight_relay::wake`]. This module is that one
//! implementation behind each relay's own `Service` trait.

use std::io;
use std::thread;
use std::time::Instant;

use ghostlight_relay::endpoint::{ConnectError, Connection};
use ghostlight_relay::protocol::{self, Opening, Reply, StartReason};
use ghostlight_relay::wake::Waiting;
use ghostlight_relay::{home, spawn, OPENING_DEADLINE};

use crate::{forward, port};

/// The installed service under the fixed home.
pub struct InstalledService;

/// Connect to the service under the fixed home and present `opening`.
fn connect(opening: &Opening) -> Result<(Connection, Reply), ConnectError> {
    protocol::connect(&home::home()?, opening, Instant::now() + OPENING_DEADLINE)
}

/// Register this process's wake endpoint under the fixed home and call `ready` on each wake,
/// from a thread of its own, for as long as the process runs.
fn watch(ready: Box<dyn Fn() + Send>) -> io::Result<()> {
    let waiting = Waiting::register(&home::home()?)?;
    thread::spawn(move || {
        while waiting.next().is_ok() {
            ready();
        }
    });
    Ok(())
}

impl forward::Service for InstalledService {
    fn connect(&self, opening: &Opening) -> Result<(Connection, Reply), ConnectError> {
        connect(opening)
    }

    fn start(&self, reason: StartReason) -> io::Result<()> {
        spawn::start_service(reason)
    }

    fn watch(&self, ready: Box<dyn Fn() + Send>) -> io::Result<()> {
        watch(ready)
    }
}

impl port::Service for InstalledService {
    fn connect(&self, opening: &Opening) -> Result<(Connection, Reply), ConnectError> {
        connect(opening)
    }

    fn start(&self, reason: StartReason) -> io::Result<()> {
        spawn::start_service(reason)
    }

    fn watch(&self, ready: Box<dyn Fn() + Send>) -> io::Result<()> {
        watch(ready)
    }
}
