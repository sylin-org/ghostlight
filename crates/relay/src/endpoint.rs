//! The relay protocol 4 endpoint: where the service listens, how an edge connects, and the one
//! connection type both ends hold (ADR-0199 Decision 5).
//!
//! On Windows the endpoint is a named pipe whose name carries the user's security identifier and
//! a hash of the home, so two users never share a name and a test home stays apart from the real
//! one. On Linux it is a Unix socket in a private run directory under the home. Neither depends
//! on an environment variable that an MCP client might filter, and neither opens a network port.
//!
//! Each side checks the other before any payload byte. The service admits a peer that is the same
//! user (and, on Windows, at least medium integrity, which keeps sandboxed and AppContainer
//! processes out). The client requires a server that is the same user, so an account that took
//! the name first receives nothing.
//!
//! [`Connection`] is the same type for both ends and offers what `TcpStream` offered the code it
//! replaces: reading and writing, [`Connection::try_clone`], read and write timeouts shared by
//! clones, and [`Connection::shutdown`] to end a stream from any thread. An accepted connection
//! also keeps the process id the peer check read ([`Connection::peer_process_id`]). The platform-specific
//! parts live in [`crate::os`] (Windows pipes, user ids) and the private `windows` and `linux`
//! files here.
//!
//! A waiting edge serves a wake endpoint of its own, which [`crate::wake`] names by an id: on
//! Windows the service's pipe name with [`WAKE_PIPE_INFIX`] and the id, on Linux a socket named
//! by the id in `run/wake/`. The service connects to it as a client. Both ends make the same
//! checks as at the service's endpoint, with the roles of server and client swapped.

use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::home::{RELAY_SOCKET_NAME, RUN_DIRECTORY_NAME, WAKE_DIRECTORY_NAME};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(windows)]
use windows as platform;

/// Prefix of every endpoint pipe name on Windows.
const PIPE_NAME_PREFIX: &str = r"\\.\pipe\ghostlight-";

/// What follows the service's pipe name in a wake endpoint's pipe name, before the id.
pub const WAKE_PIPE_INFIX: &str = "-wake-";

/// FNV-1a 32-bit offset basis.
const FNV_OFFSET_BASIS: u32 = 0x811c_9dc5;

/// FNV-1a 32-bit prime.
const FNV_PRIME: u32 = 0x0100_0193;

/// Where the service listens and where an edge connects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Endpoint {
    /// A Windows named pipe, by its full name.
    NamedPipe(String),
    /// A Unix socket, by its path.
    UnixSocket(PathBuf),
}

impl Endpoint {
    /// The endpoint for `home` on this platform.
    ///
    /// On Windows this reads the current user's security identifier from the process token.
    pub fn for_home(home: &Path) -> io::Result<Self> {
        #[cfg(windows)]
        {
            Ok(Self::named_pipe(&crate::os::current_user_sid()?, home))
        }
        #[cfg(not(windows))]
        {
            Ok(Self::unix_socket(home))
        }
    }

    /// The pipe endpoint for the user `user_sid` and `home`:
    /// `\\.\pipe\ghostlight-<user_sid>-<home hash>`.
    #[must_use]
    pub fn named_pipe(user_sid: &str, home: &Path) -> Self {
        Self::NamedPipe(pipe_name(user_sid, home))
    }

    /// The socket endpoint for `home`: `<home>/run/relay.sock`.
    #[must_use]
    pub fn unix_socket(home: &Path) -> Self {
        Self::UnixSocket(socket_path(home))
    }
}

impl fmt::Display for Endpoint {
    /// The pipe name or the socket path, as `doctor` shows it.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedPipe(name) => formatter.write_str(name),
            Self::UnixSocket(path) => write!(formatter, "{}", path.display()),
        }
    }
}

/// The 32-bit FNV-1a hash of `bytes`.
#[must_use]
pub fn fnv1a32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, &byte| {
        (hash ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

/// The home's hash as it appears in a pipe name: eight lowercase hexadecimal digits of the
/// FNV-1a hash of the home path, ASCII-lowercased.
#[must_use]
pub fn home_hash(home: &Path) -> String {
    let lowered = home.to_string_lossy().to_ascii_lowercase();
    format!("{:08x}", fnv1a32(lowered.as_bytes()))
}

/// The pipe name for the user `user_sid` and `home`.
fn pipe_name(user_sid: &str, home: &Path) -> String {
    format!("{PIPE_NAME_PREFIX}{user_sid}-{}", home_hash(home))
}

/// The pipe name of the wake endpoint `id` for the user `user_sid` and `home`.
#[cfg_attr(
    not(windows),
    expect(dead_code, reason = "only Windows names a wake pipe")
)]
fn wake_pipe_name(user_sid: &str, home: &Path, id: &str) -> String {
    format!("{}{WAKE_PIPE_INFIX}{id}", pipe_name(user_sid, home))
}

/// The run directory under `home`.
fn run_directory(home: &Path) -> PathBuf {
    home.join(RUN_DIRECTORY_NAME)
}

/// The relay socket path under `home`.
fn socket_path(home: &Path) -> PathBuf {
    run_directory(home).join(RELAY_SOCKET_NAME)
}

/// The directory under `home` where waiting edges register: `<home>/run/wake`.
#[must_use]
pub fn wake_directory(home: &Path) -> PathBuf {
    run_directory(home).join(WAKE_DIRECTORY_NAME)
}

/// Why [`connect`] did not return a connection.
#[derive(Debug, Error)]
pub enum ConnectError {
    /// Nothing is listening at the endpoint.
    #[error("no service is listening")]
    NoService,
    /// Something is listening, but not a server running as this user.
    #[error("the endpoint is served by another account")]
    ForeignServer,
    /// The connection or a check on it failed.
    #[error("connecting to the service failed: {0}")]
    Io(#[from] io::Error),
    /// The deadline passed before the endpoint could be reached.
    #[error("the service was not reachable before the deadline")]
    Deadline,
    /// The server closed the connection while it was being checked.
    #[error("the service closed the connection")]
    Closed,
}

/// The listening side of the endpoint.
///
/// One thread calls [`Listener::accept`] in a loop while another may call
/// [`Listener::shutdown`].
#[derive(Debug)]
pub struct Listener {
    inner: platform::Listener,
}

impl Listener {
    /// Bind the endpoint of `home` as its first and only listener.
    ///
    /// An endpoint that is already served yields [`io::ErrorKind::AddrInUse`]. On Linux this
    /// creates the run directory (mode 0700; an existing one must belong to this user and is
    /// tightened to 0700), removes a socket file nobody listens on, binds the socket and makes
    /// it 0600. The caller holds the service lock, so a stale file is its own to remove.
    pub fn bind_first(home: &Path) -> io::Result<Self> {
        Ok(Self {
            inner: platform::bind_first(home)?,
        })
    }

    /// Bind the wake endpoint `id` of `home`, for an edge that waits for a service.
    ///
    /// An endpoint that is already served yields [`io::ErrorKind::AddrInUse`]. On Linux this
    /// creates the run and wake directories as [`Listener::bind_first`] creates the run
    /// directory, removes a socket file nobody listens on, binds the socket and makes it 0600.
    pub fn bind_wake(home: &Path, id: &str) -> io::Result<Self> {
        Ok(Self {
            inner: platform::bind_wake(home, id)?,
        })
    }

    /// Wait for the next peer that passes the same-user check and return its end, which knows
    /// the peer's process id ([`Connection::peer_process_id`]).
    ///
    /// A peer that fails the check is dropped before any byte moves and the wait goes on. After
    /// [`Listener::shutdown`] this fails with [`io::ErrorKind::ConnectionAborted`].
    pub fn accept(&self) -> io::Result<Connection> {
        let (inner, peer_process_id) = platform::accept(&self.inner)?;
        Ok(Connection {
            inner,
            peer_process_id,
        })
    }

    /// Wake an [`accept`](Listener::accept) blocked on another thread, and make every later one
    /// fail. Callable from any thread.
    pub fn shutdown(&self) {
        self.inner.shutdown();
    }
}

/// Connect to the service of `home`, waiting at most until `deadline` for a busy endpoint.
///
/// The server is checked before the connection is returned. On Linux the socket's run directory
/// must belong to this user with no group or other permission, and a connect to a Unix socket
/// completes or fails at once, so the deadline bounds nothing there.
pub fn connect(home: &Path, deadline: Instant) -> Result<Connection, ConnectError> {
    Ok(Connection {
        inner: platform::connect(home, deadline)?,
        peer_process_id: None,
    })
}

/// Connect to the wake endpoint `id` of `home`, waiting at most until `deadline` for a busy one.
///
/// The edge that serves it is checked as [`connect`] checks the service: the same user, and on
/// Linux a wake directory that belongs to this user with no group or other permission.
/// [`ConnectError::NoService`] means no edge serves the id any more.
pub fn connect_wake(home: &Path, id: &str, deadline: Instant) -> Result<Connection, ConnectError> {
    Ok(Connection {
        inner: platform::connect_wake(home, id, deadline)?,
        peer_process_id: None,
    })
}

/// Two connected ends, for tests. No peer is checked.
pub fn pair() -> io::Result<(Connection, Connection)> {
    let (first, second) = platform::pair()?;
    Ok((
        Connection {
            inner: first,
            peer_process_id: None,
        },
        Connection {
            inner: second,
            peer_process_id: None,
        },
    ))
}

/// One end of a relay connection, the same type on the service and on the edge.
///
/// Clones share the stream, its timeouts and its closed state, as clones of a socket do. A read
/// that waits longer than the read timeout fails with [`io::ErrorKind::TimedOut`] and leaves the
/// stream usable. [`Connection::shutdown`] ends the stream for every clone: pending and later
/// reads return end of file and writes fail with [`io::ErrorKind::BrokenPipe`].
#[derive(Debug)]
pub struct Connection {
    inner: platform::Stream,
    /// The peer's process id, as the operating system reported it when the listener admitted
    /// the peer.
    peer_process_id: Option<u32>,
}

impl Connection {
    /// Another handle to the same stream.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            inner: self.inner.try_clone()?,
            peer_process_id: self.peer_process_id,
        })
    }

    /// The process id of the peer [`Listener::accept`] admitted, as the operating system reported
    /// it then: a fact about who connected, read by the service for display and history and never
    /// for authority. None on the connecting end, on a [`pair`], and on Linux for a peer in
    /// another process id namespace.
    #[must_use]
    pub fn peer_process_id(&self) -> Option<u32> {
        self.peer_process_id
    }

    /// Limit how long a read waits, or `None` to wait without limit. Zero is refused.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.inner.set_read_timeout(timeout)
    }

    /// Limit how long a write waits, or `None` to wait without limit. Zero is refused.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.inner.set_write_timeout(timeout)
    }

    /// End the stream for every clone. Callable from any thread, and harmless after the peer has
    /// already gone.
    pub fn shutdown(&self) {
        self.inner.shutdown();
    }
}

impl Read for Connection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        (&self.inner).read(buffer)
    }
}

impl Read for &Connection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        (&self.inner).read(buffer)
    }
}

impl Write for Connection {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        (&self.inner).write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.inner).flush()
    }
}

impl Write for &Connection {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        (&self.inner).write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.inner).flush()
    }
}

#[cfg(test)]
mod tests;
