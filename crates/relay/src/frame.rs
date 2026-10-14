//! Length-prefixed JSON framing for every relay stream.
//!
//! A frame is a `u32` little-endian byte count followed by that many bytes of UTF-8 JSON. These
//! are the same bytes as Chrome native messaging on Windows and Linux x86_64, which is what lets
//! the browser connector pump the extension's port to the service without parsing.

use std::io::{self, Read, Write};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

use crate::MAX_FRAME_BYTES;

/// A framing or serialization failure.
#[derive(Debug, Error)]
pub enum FrameError {
    /// Stream I/O failed.
    #[error("frame I/O failed: {0}")]
    Io(#[from] io::Error),
    /// JSON encoding or decoding failed.
    #[error("frame JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    /// The frame, as written or as declared by the peer, exceeds [`MAX_FRAME_BYTES`].
    #[error("frame exceeds {MAX_FRAME_BYTES} bytes")]
    TooLarge,
    /// The stream ended inside a frame.
    #[error("stream ended before the declared frame length")]
    Truncated,
}

/// Read one frame's payload without interpreting it, or `None` at a clean end of stream.
///
/// The declared length is checked before any allocation, so a peer cannot force memory growth by
/// lying about it.
pub fn read_frame_raw(reader: &mut impl Read) -> Result<Option<Vec<u8>>, FrameError> {
    let mut header = [0_u8; 4];
    if reader.read(&mut header[..1])? == 0 {
        return Ok(None);
    }
    reader
        .read_exact(&mut header[1..])
        .map_err(truncated_or_io)?;
    let length = u32::from_le_bytes(header) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge);
    }
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload).map_err(truncated_or_io)?;
    Ok(Some(payload))
}

/// Write one frame around already-encoded bytes and flush it.
pub fn write_frame_raw(writer: &mut impl Write, payload: &[u8]) -> Result<(), FrameError> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge);
    }
    let length = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge)?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// Read one frame and decode it, or `None` at a clean end of stream.
///
/// `T` may be `serde_json::Value` to read a frame without committing to a shape.
pub fn read_frame<T: DeserializeOwned>(reader: &mut impl Read) -> Result<Option<T>, FrameError> {
    match read_frame_raw(reader)? {
        Some(payload) => Ok(Some(serde_json::from_slice(&payload)?)),
        None => Ok(None),
    }
}

/// Encode a value as JSON and write it as one frame.
///
/// `T` may be `serde_json::Value` to write a frame without committing to a shape.
pub fn write_frame<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<(), FrameError> {
    write_frame_raw(writer, &serde_json::to_vec(value)?)
}

/// Map an early end of stream inside a frame to [`FrameError::Truncated`].
fn truncated_or_io(error: io::Error) -> FrameError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        FrameError::Truncated
    } else {
        FrameError::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::{json, Value};

    use super::*;

    #[test]
    fn a_value_round_trips_through_a_frame() {
        let value = json!({"kind": "message", "line": "{\"jsonrpc\":\"2.0\"}"});
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &value).unwrap();
        let decoded: Value = read_frame(&mut Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn the_header_is_a_little_endian_u32_length() {
        let mut bytes = Vec::new();
        write_frame_raw(&mut bytes, b"{}").unwrap();
        assert_eq!(bytes, [2, 0, 0, 0, b'{', b'}']);
    }

    #[test]
    fn a_clean_end_of_stream_is_none() {
        let mut empty = Cursor::new(Vec::new());
        assert!(read_frame_raw(&mut empty).unwrap().is_none());
        assert!(read_frame::<Value>(&mut empty).unwrap().is_none());
    }

    #[test]
    fn frames_are_read_one_at_a_time() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &json!(1)).unwrap();
        write_frame(&mut bytes, &json!(2)).unwrap();
        let mut reader = Cursor::new(bytes);
        assert_eq!(read_frame::<Value>(&mut reader).unwrap(), Some(json!(1)));
        assert_eq!(read_frame::<Value>(&mut reader).unwrap(), Some(json!(2)));
        assert!(read_frame::<Value>(&mut reader).unwrap().is_none());
    }

    #[test]
    fn the_raw_variant_returns_the_payload_unparsed() {
        let mut bytes = Vec::new();
        write_frame_raw(&mut bytes, b"not json").unwrap();
        let payload = read_frame_raw(&mut Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(payload, b"not json");
    }

    #[test]
    fn an_oversize_declared_length_is_rejected_before_allocation() {
        let declared = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
        let mut reader = Cursor::new(declared.to_le_bytes().to_vec());
        assert!(matches!(
            read_frame_raw(&mut reader),
            Err(FrameError::TooLarge)
        ));
    }

    #[test]
    fn an_oversize_payload_is_not_written() {
        let payload = vec![b' '; MAX_FRAME_BYTES + 1];
        let mut bytes = Vec::new();
        assert!(matches!(
            write_frame_raw(&mut bytes, &payload),
            Err(FrameError::TooLarge)
        ));
        assert!(bytes.is_empty());
    }

    #[test]
    fn a_frame_at_the_limit_is_accepted() {
        let payload = vec![b' '; MAX_FRAME_BYTES];
        let mut bytes = Vec::new();
        write_frame_raw(&mut bytes, &payload).unwrap();
        let read = read_frame_raw(&mut Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(read.len(), MAX_FRAME_BYTES);
    }

    #[test]
    fn a_stream_cut_inside_a_frame_is_truncated() {
        let mut header_only = Cursor::new(vec![9, 0]);
        assert!(matches!(
            read_frame_raw(&mut header_only),
            Err(FrameError::Truncated)
        ));
        let mut short_payload = Cursor::new(vec![9, 0, 0, 0, b'{']);
        assert!(matches!(
            read_frame_raw(&mut short_payload),
            Err(FrameError::Truncated)
        ));
    }

    #[test]
    fn malformed_json_is_a_json_error() {
        let mut bytes = Vec::new();
        write_frame_raw(&mut bytes, b"{").unwrap();
        assert!(matches!(
            read_frame::<Value>(&mut Cursor::new(bytes)),
            Err(FrameError::Json(_))
        ));
    }
}
