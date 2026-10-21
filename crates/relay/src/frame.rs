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

/// One frame read by [`read_frame_or_skip`].
#[derive(Debug, Eq, PartialEq)]
pub enum RawFrame {
    /// A frame within the bound, its payload uninterpreted.
    Payload(Vec<u8>),
    /// A frame that declared this many bytes, over the bound. Its bytes were read and dropped, so
    /// the stream is still aligned on the next frame.
    Skipped(usize),
}

/// Read one frame like [`read_frame_raw`], except that a frame over the bound is read through
/// and dropped instead of ending the stream, or `None` at a clean end of stream.
///
/// Only a reader whose peer may send more than the bound uses it: Chrome sends an extension's
/// native messages of up to 64 MiB, and one oversized answer must not end the port that carries
/// every session's browser (S-D8). Nothing is allocated for a skipped frame.
pub fn read_frame_or_skip(reader: &mut impl Read) -> Result<Option<RawFrame>, FrameError> {
    let mut header = [0_u8; 4];
    if reader.read(&mut header[..1])? == 0 {
        return Ok(None);
    }
    reader
        .read_exact(&mut header[1..])
        .map_err(truncated_or_io)?;
    let length = u32::from_le_bytes(header) as usize;
    if length > MAX_FRAME_BYTES {
        let declared = u64::from(u32::from_le_bytes(header));
        let skipped = io::copy(&mut reader.take(declared), &mut io::sink())?;
        if skipped < declared {
            return Err(FrameError::Truncated);
        }
        return Ok(Some(RawFrame::Skipped(length)));
    }
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload).map_err(truncated_or_io)?;
    Ok(Some(RawFrame::Payload(payload)))
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
mod tests;
