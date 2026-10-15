//! The Linux endpoint: a Unix socket in a private run directory under the home.

use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{run_directory, socket_path, ConnectError};
use crate::os::{current_uid, peer_uid};
use crate::private;

/// Mode of the run directory: the owner only.
const RUN_DIRECTORY_MODE: u32 = 0o700;

/// Mode of the socket: read and write for the owner only.
const SOCKET_MODE: u32 = 0o600;

/// The group and other permission bits a trusted run directory has none of.
const GROUP_AND_OTHER_BITS: u32 = 0o077;

/// The permission bits of a mode.
const PERMISSION_BITS: u32 = 0o777;

/// The listening socket, with the path it is bound at.
#[derive(Debug)]
pub(super) struct Listener {
    inner: UnixListener,
    path: PathBuf,
    stopping: AtomicBool,
}

impl Listener {
    /// Accept the next peer owned by this user. A peer of another user is dropped unread.
    pub(super) fn accept(&self) -> io::Result<Stream> {
        loop {
            if self.stopping.load(Ordering::SeqCst) {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
            let (stream, _) = self.inner.accept()?;
            if self.stopping.load(Ordering::SeqCst) {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
            if peer_uid(&stream).is_ok_and(|uid| uid == current_uid()) {
                return Ok(Stream(stream));
            }
        }
    }

    /// Wake a blocked accept by connecting to the socket once the stop flag is up.
    pub(super) fn shutdown(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(&self.path);
    }
}

/// Create the run directory and bind the socket.
pub(super) fn bind_first(home: &Path) -> io::Result<Listener> {
    prepare_run_directory(&run_directory(home))?;
    let path = socket_path(home);
    remove_stale_socket(&path)?;
    let inner = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(SOCKET_MODE))?;
    Ok(Listener {
        inner,
        path,
        stopping: AtomicBool::new(false),
    })
}

/// Make `run` a directory owned by this user and closed to everyone else, creating it if needed.
fn prepare_run_directory(run: &Path) -> io::Result<()> {
    match fs::symlink_metadata(run) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "the run path is not a directory",
                ));
            }
            if metadata.uid() != current_uid() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "the run directory belongs to another user",
                ));
            }
            if metadata.mode() & PERMISSION_BITS == RUN_DIRECTORY_MODE {
                return Ok(());
            }
        }
        Err(failure) if failure.kind() == io::ErrorKind::NotFound => private::create_dir_all(run)?,
        Err(failure) => return Err(failure),
    }
    fs::set_permissions(run, fs::Permissions::from_mode(RUN_DIRECTORY_MODE))
}

/// Remove a socket file that nobody listens on.
///
/// A socket that still accepts connections belongs to a live listener, and is reported as
/// [`io::ErrorKind::AddrInUse`] rather than removed.
fn remove_stale_socket(path: &Path) -> io::Result<()> {
    match UnixStream::connect(path) {
        Ok(_) => Err(io::ErrorKind::AddrInUse.into()),
        Err(failure) if failure.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(failure) if failure.kind() == io::ErrorKind::ConnectionRefused => {
            match fs::remove_file(path) {
                Err(failure) if failure.kind() != io::ErrorKind::NotFound => Err(failure),
                _ => Ok(()),
            }
        }
        Err(failure) => Err(failure),
    }
}

/// Connect to the socket of `home` after checking its run directory.
pub(super) fn connect(home: &Path, _deadline: Instant) -> Result<Stream, ConnectError> {
    match fs::symlink_metadata(run_directory(home)) {
        Ok(metadata) => {
            let trusted = metadata.is_dir()
                && metadata.uid() == current_uid()
                && metadata.mode() & GROUP_AND_OTHER_BITS == 0;
            if !trusted {
                return Err(ConnectError::ForeignServer);
            }
        }
        Err(failure) if failure.kind() == io::ErrorKind::NotFound => {
            return Err(ConnectError::NoService);
        }
        Err(failure) => return Err(failure.into()),
    }
    match UnixStream::connect(socket_path(home)) {
        Ok(stream) => Ok(Stream(stream)),
        Err(failure) => Err(match failure.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => ConnectError::NoService,
            io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe => ConnectError::Closed,
            _ => ConnectError::Io(failure),
        }),
    }
}

/// Two connected ends.
pub(super) fn pair() -> io::Result<(Stream, Stream)> {
    let (first, second) = UnixStream::pair()?;
    Ok((Stream(first), Stream(second)))
}

/// One end of a Unix socket connection, with timeouts reported as [`io::ErrorKind::TimedOut`].
#[derive(Debug)]
pub(super) struct Stream(UnixStream);

impl Stream {
    pub(super) fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }

    pub(super) fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(timeout)
    }

    pub(super) fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(timeout)
    }

    pub(super) fn shutdown(&self) {
        let _ = self.0.shutdown(std::net::Shutdown::Both);
    }
}

/// A socket reports an expired timeout as would-block; the endpoint reports it as timed out.
fn timed_out(failure: io::Error) -> io::Error {
    if failure.kind() == io::ErrorKind::WouldBlock {
        io::ErrorKind::TimedOut.into()
    } else {
        failure
    }
}

impl Read for &Stream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        (&self.0).read(buffer).map_err(timed_out)
    }
}

impl Write for &Stream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        (&self.0).write(buffer).map_err(timed_out)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.0).flush()
    }
}
