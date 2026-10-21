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
    use std::collections::BTreeMap;
    use std::thread;

    use super::*;
    use crate::intake::mcp::{McpSession, OwnedTab, ResumeState, LATEST_REVISION};
    use crate::intake::session::SessionMarker;
    use ghostlight_relay::endpoint::pair;
    use ghostlight_relay::frame::write_frame;
    use ghostlight_relay::protocol::{Edge, Program, Role, MCP_CHANNEL};
    use serde_json::json;

    /// Defect SERV-1 (3db83135, arch-1 second sweep) and ADR-0203 C11 (8c216920), the one limit
    /// both ends of a reconnect must agree on: the largest resume token the service mints comes
    /// back in the longest opening an edge sends, and the service reads that opening. A session
    /// with about 270 tabs used to mint a token whose opening went over the opening cap, so it
    /// could never reconnect. The token keeps as many of the session's first tabs as fit, its
    /// claim, and the lives of the kept tabs' browsers only.
    #[test]
    fn the_largest_token_the_service_mints_comes_back_in_an_opening_it_reads() {
        let mut session = McpSession::new();
        session.accept(
            &json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": LATEST_REVISION,
                    "clientInfo": {"name": "c".repeat(150), "version": "1"},
                    "capabilities": {}
                }
            })
            .to_string(),
        );
        let browser = format!("browser_{}", "b".repeat(60));
        let tab = |index: u64| OwnedTab {
            browser_id: browser.clone(),
            tab_id: index,
            handle: format!("tab_{index}"),
            url_digest: Some("d".repeat(64)),
        };
        let lives = BTreeMap::from([
            (browser.clone(), "life_b".to_owned()),
            ("browser_without_tabs".to_owned(), "life_c".to_owned()),
        ]);
        let state = session
            .resume_state((0..5_000).map(tab).collect(), lives)
            .expect("an initialized session has a resume state");
        let token = state.encode();
        assert!(token.len() <= MAX_RESUME_TOKEN_BYTES, "{}", token.len());

        // The opening an MCP edge sends with its token, with every other optional field an edge
        // sets: the browser edge's extension origin and the command line's session marker.
        let mut sent = Opening::new(
            Role::Mcp,
            Edge::new(Program::Edge, env!("CARGO_PKG_VERSION").to_owned()),
        );
        sent.channel = Some(MCP_CHANNEL.to_owned());
        sent.resume = Some(token.clone());
        sent.origin = Some(format!("chrome-extension://{}/", "a".repeat(32)));
        sent.session = Some(
            serde_json::to_value(SessionMarker::Declared {
                key: "s".repeat(512),
            })
            .unwrap(),
        );
        let size = serde_json::to_vec(&sent).unwrap().len();
        assert!(
            size > MAX_RESUME_TOKEN_BYTES && size <= MAX_OPENING_BYTES,
            "{size}"
        );
        let expected = sent.clone();
        let (mut client, server) = pair().unwrap();
        // The frame is larger than a pipe's buffer, so the edge writes it while it is read.
        let writer = thread::spawn(move || write_frame(&mut client, &sent).unwrap());
        let read = read_opening(&server, Duration::from_secs(5)).unwrap();
        writer.join().unwrap();
        assert_eq!(read, expected);

        let decoded = ResumeState::decode(read.resume.as_deref().unwrap()).expect("it decodes");
        let kept = decoded.owned_tabs.len();
        assert!(kept > 0 && kept < state.owned_tabs.len(), "{kept}");
        assert_eq!(decoded.owned_tabs[..], state.owned_tabs[..kept]);
        assert!(state.claim.is_some());
        assert_eq!(decoded.claim, state.claim);
        assert_eq!(decoded.lives.keys().collect::<Vec<_>>(), [&browser]);
        let one_more = ResumeState {
            owned_tabs: state.owned_tabs[..=kept].to_vec(),
            ..state.clone()
        };
        assert_eq!(
            one_more.encode(),
            token,
            "one tab more than the token keeps does not fit"
        );
    }
}
