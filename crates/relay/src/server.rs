//! The service side of the relay that is not product policy: reading an opening safely,
//! comparing tokens, and holding the one lease.
//!
//! The listener is reachable by any local process, so everything read before the token is
//! checked is bounded in size and in time.

use std::fs::{File, TryLockError};
use std::io::{self, Read};
use std::net::TcpStream;
use std::path::Path;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::opening::Opening;
use crate::private;

/// Largest opening frame the service reads, in bytes: 64 KiB.
pub const MAX_OPENING_BYTES: usize = 64 * 1024;

/// Why an opening could not be read.
#[derive(Debug, Error)]
pub enum OpeningError {
    /// The socket failed.
    #[error("opening I/O failed: {0}")]
    Io(#[from] io::Error),
    /// The peer did not finish its opening before the deadline.
    #[error("the opening did not arrive before the deadline")]
    Deadline,
    /// The peer closed the connection before or inside its opening.
    #[error("the connection closed before a complete opening")]
    Closed,
    /// The declared opening is larger than [`MAX_OPENING_BYTES`].
    #[error("the opening exceeds {MAX_OPENING_BYTES} bytes")]
    TooLarge,
    /// The opening frame is not an opening.
    #[error("the opening is not valid JSON of the opening shape: {0}")]
    Malformed(#[from] serde_json::Error),
}

/// Compare two secrets without stopping at the first difference.
///
/// The time taken depends on the length of the inputs and not on where they differ. A length
/// difference returns early; the length of a token is not a secret.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// Read the one [`Opening`] a new connection must send, within `deadline` and
/// [`MAX_OPENING_BYTES`].
///
/// The deadline covers the whole frame, so a peer cannot extend it by sending slowly. The
/// socket's read timeout is cleared before returning, ready for the stream the opening names.
pub fn read_opening(stream: &mut TcpStream, deadline: Duration) -> Result<Opening, OpeningError> {
    let limit = Instant::now() + deadline;
    let mut header = [0_u8; 4];
    read_exact_until(stream, &mut header, limit)?;
    let length = u32::from_le_bytes(header) as usize;
    if length > MAX_OPENING_BYTES {
        return Err(OpeningError::TooLarge);
    }
    let mut payload = vec![0_u8; length];
    read_exact_until(stream, &mut payload, limit)?;
    stream.set_read_timeout(None)?;
    Ok(serde_json::from_slice(&payload)?)
}

/// Fill `buffer` from `stream` before `limit`.
fn read_exact_until(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    limit: Instant,
) -> Result<(), OpeningError> {
    let mut filled = 0;
    while filled < buffer.len() {
        let remaining = limit
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(OpeningError::Deadline)?;
        stream.set_read_timeout(Some(remaining))?;
        match stream.read(&mut buffer[filled..]) {
            Ok(0) => return Err(OpeningError::Closed),
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(OpeningError::Deadline)
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// The one lease: held by the running service for its whole life.
///
/// The lease is an exclusive lock on the lease file. The operating system releases it when the
/// holder exits, however it exits, so a crashed service never strands the lease.
#[derive(Debug)]
pub struct Lease {
    _file: File,
}

impl Lease {
    /// Try to take the lease at `path`, creating the file and its directory when missing.
    ///
    /// Returns `None` when another process holds it.
    pub fn try_acquire(path: &Path) -> io::Result<Option<Self>> {
        if let Some(parent) = path.parent() {
            private::create_dir_all(parent)?;
        }
        let file = private::file_options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;

    use super::*;
    use crate::frame::{write_frame, write_frame_raw};
    use crate::opening::Stream;

    /// A scratch directory removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "ghostlight-relay-server-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A connected pair: the client end and the accepted server end.
    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        (client, server)
    }

    #[test]
    fn equal_secrets_compare_equal_and_others_do_not() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(constant_time_eq(b"", b""));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secre"));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn an_opening_is_read_from_a_connection() {
        let (mut client, mut server) = pair();
        let sent = Opening::new("token".to_owned(), Stream::Browser);
        write_frame(&mut client, &sent).unwrap();
        let read = read_opening(&mut server, Duration::from_secs(5)).unwrap();
        assert_eq!(read, sent);
    }

    #[test]
    fn a_silent_peer_hits_the_deadline() {
        let (_client, mut server) = pair();
        let started = Instant::now();
        let error = read_opening(&mut server, Duration::from_millis(100)).unwrap_err();
        assert!(matches!(error, OpeningError::Deadline));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_peer_that_sends_half_an_opening_hits_the_deadline() {
        let (mut client, mut server) = pair();
        io::Write::write_all(&mut client, &[40, 0, 0, 0, b'{']).unwrap();
        let error = read_opening(&mut server, Duration::from_millis(100)).unwrap_err();
        assert!(matches!(error, OpeningError::Deadline));
    }

    #[test]
    fn an_oversize_opening_is_refused_before_it_is_read() {
        let (mut client, mut server) = pair();
        let declared = u32::try_from(MAX_OPENING_BYTES + 1).unwrap();
        io::Write::write_all(&mut client, &declared.to_le_bytes()).unwrap();
        let error = read_opening(&mut server, Duration::from_secs(5)).unwrap_err();
        assert!(matches!(error, OpeningError::TooLarge));
    }

    #[test]
    fn a_closed_connection_is_reported_as_closed() {
        let (client, mut server) = pair();
        drop(client);
        let error = read_opening(&mut server, Duration::from_secs(5)).unwrap_err();
        assert!(matches!(error, OpeningError::Closed));
    }

    #[test]
    fn a_frame_that_is_not_an_opening_is_malformed() {
        let (mut client, mut server) = pair();
        write_frame_raw(&mut client, b"[1, 2]").unwrap();
        let error = read_opening(&mut server, Duration::from_secs(5)).unwrap_err();
        assert!(matches!(error, OpeningError::Malformed(_)));
    }

    #[test]
    fn a_second_lease_fails_while_the_first_is_held_and_succeeds_after_it_drops() {
        let scratch = Scratch::new();
        let path = scratch.0.join("nested").join("ghostlight-runtime.lock");
        let first = Lease::try_acquire(&path).unwrap().expect("first lease");
        assert!(Lease::try_acquire(&path).unwrap().is_none());
        drop(first);
        assert!(Lease::try_acquire(&path).unwrap().is_some());
    }

    #[test]
    fn leases_on_different_files_do_not_exclude_each_other() {
        let scratch = Scratch::new();
        let first = Lease::try_acquire(&scratch.0.join("a.lock")).unwrap();
        let second = Lease::try_acquire(&scratch.0.join("b.lock")).unwrap();
        assert!(first.is_some() && second.is_some());
    }

    #[test]
    fn the_reader_stops_at_the_opening_and_leaves_later_bytes() {
        let (mut client, mut server) = pair();
        let sent = Opening::new("token".to_owned(), Stream::Browser);
        write_frame(&mut client, &sent).unwrap();
        write_frame_raw(&mut client, b"{\"next\":1}").unwrap();
        let handle = thread::spawn(move || {
            read_opening(&mut server, Duration::from_secs(5)).unwrap();
            crate::frame::read_frame_raw(&mut server).unwrap().unwrap()
        });
        assert_eq!(handle.join().unwrap(), b"{\"next\":1}");
    }
}
