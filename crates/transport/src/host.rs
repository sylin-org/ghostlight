// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Chrome native-messaging host protocol.
//!
//! Framing: a 4-byte little-endian `u32` length prefix followed by exactly that many bytes of
//! UTF-8 JSON. Chrome speaks this framing on the process's stdin/stdout when it launches the
//! executable as a native-messaging host (Chrome native-messaging protocol; the harvested
//! technique is recorded in docs/research/12 and ADR-0050 Decision 1).

use crate::{Error, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The Chrome native-messaging host name for the active instance (ADR-0044): `org.sylin.ghostlight`
/// for the default instance, `org.sylin.ghostlight.<n>` for a named one. MUST match the host
/// manifest and every registration the installer writes for that instance. Distinct from the IPC
/// endpoint name, which carries a `.v1` version suffix (`super::ipc`).
pub fn host_name() -> String {
    crate::instance::Instance::resolve().host_name()
}

/// Human-readable description written into the native-messaging host manifest.
pub const HOST_DESCRIPTION: &str = "Ghostlight native messaging host";

/// Upper bound on generic framed IPC input. This is a corruption guard, not Chrome's directional
/// contract: the core browser adapter separately keeps host-to-extension messages below Chrome's
/// 1 MiB limit and uses bounded negotiated chunks for larger ordinary requests (ADR-0074).
pub const MAX_MESSAGE_LEN: u32 = 128 * 1024 * 1024;

/// Frame a payload as a native message: a 4-byte little-endian length prefix + the payload bytes.
pub fn encode(payload: &[u8]) -> Result<Vec<u8>> {
    let len: u32 = payload
        .len()
        .try_into()
        .map_err(|_| Error::NativeMessaging("message exceeds u32 length".into()))?;
    let mut framed = Vec::with_capacity(4 + payload.len());
    framed.extend_from_slice(&len.to_le_bytes());
    framed.extend_from_slice(payload);
    Ok(framed)
}

/// Read one native message. Returns `Ok(None)` on a clean end-of-stream (the peer closed), or the
/// payload bytes otherwise. A partial length prefix or truncated body is a framing error.
pub async fn read_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(Error::Io(e)),
    }
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_MESSAGE_LEN {
        return Err(Error::NativeMessaging(format!(
            "framed length {len} exceeds MAX_MESSAGE_LEN {MAX_MESSAGE_LEN}"
        )));
    }
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload).await.map_err(Error::Io)?;
    Ok(Some(payload))
}

/// Write one native message (length-prefixed) and flush.
pub async fn write_message<W: AsyncWriteExt + Unpin>(writer: &mut W, payload: &[u8]) -> Result<()> {
    let framed = encode(payload)?;
    writer.write_all(&framed).await.map_err(Error::Io)?;
    writer.flush().await.map_err(Error::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_prefixes_little_endian_length() {
        let framed = encode(b"hi").unwrap();
        assert_eq!(&framed[0..4], &[2, 0, 0, 0], "4-byte little-endian length");
        assert_eq!(&framed[4..], b"hi");
    }

    #[test]
    fn encode_empty_payload_is_just_a_zero_length() {
        assert_eq!(encode(b"").unwrap(), vec![0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn reads_back_an_encoded_message() {
        let msg = br#"{"jsonrpc":"2.0","method":"ping"}"#;
        let framed = encode(msg).unwrap();
        let mut reader: &[u8] = &framed;
        assert_eq!(
            read_message(&mut reader).await.unwrap().as_deref(),
            Some(&msg[..])
        );
    }

    #[tokio::test]
    async fn clean_eof_yields_none() {
        let mut reader: &[u8] = &[];
        assert!(read_message(&mut reader).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn empty_but_present_message_is_some_not_none() {
        // A zero-length message (Some(vec![])) must be distinguished from clean EOF (None).
        let framed = encode(b"").unwrap();
        let mut reader: &[u8] = &framed;
        assert_eq!(
            read_message(&mut reader).await.unwrap().as_deref(),
            Some(&b""[..])
        );
        assert!(read_message(&mut reader).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn reads_two_consecutive_messages_then_eof() {
        let mut buf = Vec::new();
        buf.extend(encode(b"one").unwrap());
        buf.extend(encode(b"two").unwrap());
        let mut reader: &[u8] = &buf;
        assert_eq!(
            read_message(&mut reader).await.unwrap().as_deref(),
            Some(&b"one"[..])
        );
        assert_eq!(
            read_message(&mut reader).await.unwrap().as_deref(),
            Some(&b"two"[..])
        );
        assert!(read_message(&mut reader).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn truncated_body_is_a_framing_error() {
        // Length prefix claims 10 bytes; only 3 are provided.
        let mut framed = 10u32.to_le_bytes().to_vec();
        framed.extend_from_slice(b"abc");
        let mut reader: &[u8] = &framed;
        assert!(read_message(&mut reader).await.is_err());
    }
}
