//! Bounded newline and Chromium native-message framing.

use std::io::{self, BufRead, Read, Write};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

use crate::MAX_FRAME_BYTES;

/// A bounded framing or serialization failure.
#[derive(Debug, Error)]
pub enum FrameError {
    /// Stream I/O failed.
    #[error("frame I/O failed: {0}")]
    Io(#[from] io::Error),
    /// JSON encoding or decoding failed.
    #[error("frame JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    /// The peer exceeded the fixed frame bound.
    #[error("frame exceeds {MAX_FRAME_BYTES} bytes")]
    TooLarge,
    /// The stream ended in the middle of a native frame.
    #[error("native message ended before its declared length")]
    Truncated,
}

/// Read one bounded newline-delimited JSON value, or `None` at clean EOF.
pub fn read_json_line<T: DeserializeOwned>(
    reader: &mut impl BufRead,
) -> Result<Option<T>, FrameError> {
    let mut bytes = Vec::new();
    let read = reader.read_until(b'\n', &mut bytes)?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge);
    }
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
    Ok(Some(serde_json::from_slice(&bytes)?))
}

/// Write one newline-delimited JSON value and flush it.
pub fn write_json_line<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<(), FrameError> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge);
    }
    writer.write_all(&bytes)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

/// Read one Chromium native-message frame, or `None` at clean EOF.
pub fn read_native<T: DeserializeOwned>(reader: &mut impl Read) -> Result<Option<T>, FrameError> {
    let Some(payload) = read_length_frame(reader)? else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_slice(&payload)?))
}

/// Read one bounded four-byte little-endian length frame without interpreting its payload.
pub fn read_length_frame(reader: &mut impl Read) -> Result<Option<Vec<u8>>, FrameError> {
    let mut length = [0_u8; 4];
    let first = reader.read(&mut length[..1])?;
    if first == 0 {
        return Ok(None);
    }
    if reader.read_exact(&mut length[1..]).is_err() {
        return Err(FrameError::Truncated);
    }
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge);
    }
    let mut payload = vec![0_u8; length];
    if reader.read_exact(&mut payload).is_err() {
        return Err(FrameError::Truncated);
    }
    Ok(Some(payload))
}

/// Write one Chromium native-message frame and flush it.
pub fn write_native<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<(), FrameError> {
    let payload = serde_json::to_vec(value)?;
    write_length_frame(writer, &payload)
}

/// Write one bounded four-byte little-endian length frame without interpreting its payload.
pub fn write_length_frame(writer: &mut impl Write, payload: &[u8]) -> Result<(), FrameError> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge);
    }
    #[allow(clippy::cast_possible_truncation)]
    let length = (payload.len() as u32).to_le_bytes();
    writer.write_all(&length)?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use serde_json::{json, Value};

    use super::{
        read_json_line, read_length_frame, read_native, write_json_line, write_length_frame,
        write_native,
    };

    #[test]
    fn newline_framing_handles_coalesced_values() {
        let mut bytes = Vec::new();
        write_json_line(&mut bytes, &json!({"a": 1})).expect("first frame writes");
        write_json_line(&mut bytes, &json!({"b": 2})).expect("second frame writes");
        let mut reader = BufReader::new(Cursor::new(bytes));
        assert_eq!(
            read_json_line::<Value>(&mut reader).unwrap(),
            Some(json!({"a": 1}))
        );
        assert_eq!(
            read_json_line::<Value>(&mut reader).unwrap(),
            Some(json!({"b": 2}))
        );
        assert_eq!(read_json_line::<Value>(&mut reader).unwrap(), None);
    }

    #[test]
    fn native_framing_handles_coalesced_values() {
        let mut bytes = Vec::new();
        write_native(&mut bytes, &json!({"a": 1})).expect("first frame writes");
        write_native(&mut bytes, &json!({"b": 2})).expect("second frame writes");
        let mut cursor = Cursor::new(bytes);
        assert_eq!(
            read_native::<Value>(&mut cursor).unwrap(),
            Some(json!({"a": 1}))
        );
        assert_eq!(
            read_native::<Value>(&mut cursor).unwrap(),
            Some(json!({"b": 2}))
        );
        assert_eq!(read_native::<Value>(&mut cursor).unwrap(), None);
    }

    #[test]
    fn opaque_length_frames_preserve_unknown_payloads() {
        let payload = br#"{"future_adapter_frame":{"unknown":true}}"#;
        let mut bytes = Vec::new();
        write_length_frame(&mut bytes, payload).expect("opaque frame writes");
        let mut cursor = Cursor::new(bytes);
        assert_eq!(
            read_length_frame(&mut cursor).unwrap().as_deref(),
            Some(payload.as_slice())
        );
        assert_eq!(read_length_frame(&mut cursor).unwrap(), None);
    }
}
