//! The connector side of an opening: find the service, connect, present the token, read the
//! reply.
//!
//! [`connect`] never starts the service and never decides what a refusal means. It returns the
//! live stream with the service's [`Reply`], and the caller acts on it.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::frame::{self, FrameError};
use crate::home;
use crate::opening::{Opening, Reply, Stream};
use crate::runtime::RuntimeFile;

/// Default time the service has to accept the connection and answer the opening: 5 s.
pub const REPLY_DEADLINE: Duration = Duration::from_secs(5);

/// Why a connection to the service could not be made.
#[derive(Debug, Error)]
pub enum ConnectError {
    /// No runtime file names a service of this relay revision.
    #[error("no service has published a runtime file")]
    NoService,
    /// The connection or the exchange failed.
    #[error("connecting to the service failed: {0}")]
    Io(#[from] io::Error),
    /// The reply could not be framed or decoded.
    #[error("the service reply was not readable: {0}")]
    Frame(#[from] FrameError),
    /// The service closed the connection without replying.
    #[error("the service closed the connection without replying")]
    Closed,
    /// The service did not answer before the deadline.
    #[error("the service did not answer before the deadline")]
    Deadline,
}

/// Connect to the running service and open `stream`, waiting at most `deadline` in all.
///
/// The runtime file under the home names the port and the token. The returned stream has no read
/// timeout. A [`Reply::Refused`] or [`Reply::Unknown`] is returned like any other reply; only the
/// caller knows whether to retry.
pub fn connect(stream: Stream, deadline: Duration) -> Result<(TcpStream, Reply), ConnectError> {
    let runtime = RuntimeFile::read(&home::runtime_file()?)?.ok_or(ConnectError::NoService)?;
    open(runtime.port, runtime.token, stream, deadline)
}

/// Connect to `127.0.0.1:port` and run the opening exchange.
fn open(
    port: u16,
    token: String,
    stream: Stream,
    deadline: Duration,
) -> Result<(TcpStream, Reply), ConnectError> {
    let limit = Instant::now() + deadline;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut socket = TcpStream::connect_timeout(&address, remaining(limit)?).map_err(timed_out)?;
    socket.set_nodelay(true)?;
    socket.set_write_timeout(Some(remaining(limit)?))?;
    frame::write_frame(&mut socket, &Opening::new(token, stream)).map_err(|error| match error {
        FrameError::Io(error) => timed_out(error),
        other => ConnectError::Frame(other),
    })?;
    socket.set_write_timeout(None)?;
    socket.set_read_timeout(Some(remaining(limit)?))?;
    let reply = match frame::read_frame::<Reply>(&mut socket) {
        Ok(Some(reply)) => reply,
        Ok(None) => return Err(ConnectError::Closed),
        Err(FrameError::Io(error)) => return Err(timed_out(error)),
        Err(other) => return Err(other.into()),
    };
    socket.set_read_timeout(None)?;
    Ok((socket, reply))
}

/// Time left before `limit`, or [`ConnectError::Deadline`] when none is.
fn remaining(limit: Instant) -> Result<Duration, ConnectError> {
    limit
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(ConnectError::Deadline)
}

/// Map a socket timeout to [`ConnectError::Deadline`] and any other failure to I/O.
fn timed_out(error: io::Error) -> ConnectError {
    if matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ) {
        ConnectError::Deadline
    } else {
        ConnectError::Io(error)
    }
}

#[cfg(test)]
mod tests;
