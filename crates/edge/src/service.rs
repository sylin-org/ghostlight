//! The installed service as the edge's relays reach it.
//!
//! Both relays connect to the service under the fixed home and start it the same way: through
//! [`ghostlight_relay::spawn::start_service`], which refuses while the session is ending. This
//! module is that one implementation behind each relay's own `Service` trait.

use std::io;
use std::time::Instant;

use ghostlight_relay::endpoint::{ConnectError, Connection};
use ghostlight_relay::protocol::{self, Opening, Reply, StartReason};
use ghostlight_relay::{home, spawn, OPENING_DEADLINE};

use crate::{forward, port};

/// The installed service under the fixed home.
pub struct InstalledService;

/// Connect to the service under the fixed home and present `opening`.
fn connect(opening: &Opening) -> Result<(Connection, Reply), ConnectError> {
    protocol::connect(&home::home()?, opening, Instant::now() + OPENING_DEADLINE)
}

impl forward::Service for InstalledService {
    fn connect(&self, opening: &Opening) -> Result<(Connection, Reply), ConnectError> {
        connect(opening)
    }

    fn start(&self, reason: StartReason) -> io::Result<()> {
        spawn::start_service(reason)
    }
}

impl port::Service for InstalledService {
    fn connect(&self, opening: &Opening) -> Result<(Connection, Reply), ConnectError> {
        connect(opening)
    }

    fn start(&self, reason: StartReason) -> io::Result<()> {
        spawn::start_service(reason)
    }
}
