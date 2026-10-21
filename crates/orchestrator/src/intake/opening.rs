//! The service side of an opening, bounded in size and time before anything is judged.
//!
//! The endpoint admits only the same user, but any program of that user can connect and say
//! nothing, or say something that is not an opening. Everything read before the opening is judged
//! is therefore bounded in size and in time. This module reads the one opening a connection must
//! send.

use std::io::{self, Read};
use std::time::{Duration, Instant};

use thiserror::Error;

use ghostlight_relay::endpoint::Connection;
use ghostlight_relay::protocol::Opening;
use ghostlight_relay::MAX_FRAME_BYTES;

use super::mcp::MAX_RESUME_TOKEN_BYTES;

/// Room for everything an opening carries besides its resume token, in bytes. The program and its
/// version, the role, the channel, the origin and the session marker are all short.
const OPENING_FIELDS_BYTES: usize = 8 * 1024;

/// Largest opening frame the service reads, in bytes: the largest resume token the service hands
/// out, and room for the opening's other fields. An opening that carries any token the service
/// minted is read, whatever else it says, so a long session can always reconnect.
pub const MAX_OPENING_BYTES: usize = MAX_RESUME_TOKEN_BYTES + OPENING_FIELDS_BYTES;

// The edge writes its opening as one relay frame, so the opening cap stays within the frame cap.
const _: () = assert!(MAX_OPENING_BYTES <= MAX_FRAME_BYTES);

/// Why an opening could not be read.
#[derive(Debug, Error)]
pub enum OpeningError {
    /// The connection failed.
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

/// Read the one [`Opening`] a new connection must send, within `deadline` and
/// [`MAX_OPENING_BYTES`].
///
/// The deadline covers the whole frame, so a peer cannot extend it by sending slowly. The
/// connection's read timeout is cleared before returning, ready for the stream the opening names.
pub fn read_opening(connection: &Connection, deadline: Duration) -> Result<Opening, OpeningError> {
    let limit = Instant::now() + deadline;
    let mut header = [0_u8; 4];
    read_exact_until(connection, &mut header, limit)?;
    let length = u32::from_le_bytes(header) as usize;
    if length > MAX_OPENING_BYTES {
        return Err(OpeningError::TooLarge);
    }
    let mut payload = vec![0_u8; length];
    read_exact_until(connection, &mut payload, limit)?;
    connection.set_read_timeout(None)?;
    Ok(serde_json::from_slice(&payload)?)
}

/// Fill `buffer` from `connection` before `limit`.
fn read_exact_until(
    connection: &Connection,
    buffer: &mut [u8],
    limit: Instant,
) -> Result<(), OpeningError> {
    let mut reader = connection;
    let mut filled = 0;
    while filled < buffer.len() {
        let remaining = limit
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(OpeningError::Deadline)?;
        connection.set_read_timeout(Some(remaining))?;
        match reader.read(&mut buffer[filled..]) {
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

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::thread;

    use super::*;
    use ghostlight_relay::endpoint::pair;
    use ghostlight_relay::frame::{write_frame, write_frame_raw};
    use ghostlight_relay::protocol::{Edge, Program, Role};

    fn opening() -> Opening {
        Opening::new(Role::Browser, Edge::new(Program::Edge, "2.0.0".into()))
    }

    #[test]
    fn an_opening_is_read_from_a_connection() {
        let (mut client, server) = pair().unwrap();
        let sent = opening();
        write_frame(&mut client, &sent).unwrap();
        let read = read_opening(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(read, sent);
    }

    #[test]
    fn a_silent_peer_hits_the_deadline() {
        let (_client, server) = pair().unwrap();
        let started = Instant::now();
        let error = read_opening(&server, Duration::from_millis(100)).unwrap_err();
        assert!(matches!(error, OpeningError::Deadline));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_peer_that_sends_half_an_opening_hits_the_deadline() {
        let (mut client, server) = pair().unwrap();
        client.write_all(&[40, 0, 0, 0, b'{']).unwrap();
        let error = read_opening(&server, Duration::from_millis(100)).unwrap_err();
        assert!(matches!(error, OpeningError::Deadline));
    }

    #[test]
    fn an_opening_that_carries_the_largest_resume_token_is_read() {
        // A session with a few hundred tabs mints a token at its cap. The opening around it, with
        // its channel, an extension origin and a session marker, is read all the same; it used to
        // be refused as too large, so such a session could never reconnect.
        let mut sent = opening();
        sent.resume = Some("t".repeat(MAX_RESUME_TOKEN_BYTES));
        sent.channel = Some("mcp".into());
        sent.origin = Some(format!("chrome-extension://{}/", "a".repeat(32)));
        sent.session = Some(serde_json::json!({ "marker": "s".repeat(512) }));
        let size = serde_json::to_vec(&sent).unwrap().len();
        assert!(
            size > MAX_RESUME_TOKEN_BYTES && size <= MAX_OPENING_BYTES,
            "{size}"
        );
        let expected = sent.clone();
        let (mut client, server) = pair().unwrap();
        // The frame is larger than a pipe's buffer, so it is written while it is read.
        let writer = thread::spawn(move || write_frame(&mut client, &sent).unwrap());
        let read = read_opening(&server, Duration::from_secs(5)).unwrap();
        writer.join().unwrap();
        assert_eq!(read, expected);
    }

    #[test]
    fn an_oversize_opening_is_refused_before_it_is_read() {
        let (mut client, server) = pair().unwrap();
        let declared = u32::try_from(MAX_OPENING_BYTES + 1).unwrap();
        client.write_all(&declared.to_le_bytes()).unwrap();
        let error = read_opening(&server, Duration::from_secs(5)).unwrap_err();
        assert!(matches!(error, OpeningError::TooLarge));
    }

    #[test]
    fn a_closed_connection_is_reported_as_closed() {
        let (client, server) = pair().unwrap();
        client.shutdown();
        drop(client);
        let error = read_opening(&server, Duration::from_secs(5)).unwrap_err();
        assert!(matches!(error, OpeningError::Closed));
    }

    #[test]
    fn a_frame_that_is_not_an_opening_is_malformed() {
        let (mut client, server) = pair().unwrap();
        write_frame_raw(&mut client, b"[1, 2]").unwrap();
        let error = read_opening(&server, Duration::from_secs(5)).unwrap_err();
        assert!(matches!(error, OpeningError::Malformed(_)));
    }

    #[test]
    fn the_reader_stops_at_the_opening_and_leaves_later_bytes() {
        let (mut client, mut server) = pair().unwrap();
        write_frame(&mut client, &opening()).unwrap();
        write_frame_raw(&mut client, b"{\"next\":1}").unwrap();
        let handle = thread::spawn(move || {
            read_opening(&server, Duration::from_secs(5)).unwrap();
            ghostlight_relay::frame::read_frame_raw(&mut server)
                .unwrap()
                .unwrap()
        });
        assert_eq!(handle.join().unwrap(), b"{\"next\":1}");
    }
}
