//! Pure, bounded GIF rendering for frames supplied by the Chromium extension.

use std::io::{self, Write};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ghostlight_bridge::browser::PhysicalRecordingFrame;
use gif::{Encoder, Frame, Repeat};
use thiserror::Error;
use zeroize::Zeroizing;

/// Maximum returned GIF bytes, including encoder overhead.
pub const MAX_GIF_BYTES: usize = 5 * 1024 * 1024;
const MAX_DECODED_PIXELS: usize = 8_000_000;
const MIN_FRAME_DELAY_MS: u64 = 20;
const MAX_FRAME_DELAY_MS: u64 = (u16::MAX as u64) * 10;

/// A decisive recording-encoding failure.
#[derive(Debug, Error)]
pub enum GifError {
    /// No captured frame exists.
    #[error("recording has no frames")]
    Empty,
    /// A JPEG was invalid or outside the pixel bound.
    #[error("recording frame {index} could not be decoded: {reason}")]
    Decode { index: usize, reason: String },
    /// GIF dimensions exceed the format or product bounds.
    #[error("recording dimensions are unsupported")]
    Dimensions,
    /// The encoded GIF exceeded its output bound.
    #[error("recording GIF exceeds {MAX_GIF_BYTES} bytes")]
    TooLarge,
    /// The GIF encoder failed decisively.
    #[error("recording GIF encoding failed: {0}")]
    Encode(String),
}

/// Encode captured JPEGs into one repeatable animated GIF.
pub fn encode(frames: &[PhysicalRecordingFrame]) -> Result<Zeroizing<Vec<u8>>, GifError> {
    let Some(first) = frames.first() else {
        return Err(GifError::Empty);
    };
    let first_bytes = decode_frame(first, 0)?;
    let (width, height) = jpeg_dimensions(&first_bytes, 0)?;
    let width_u16 = u16::try_from(width).map_err(|_| GifError::Dimensions)?;
    let height_u16 = u16::try_from(height).map_err(|_| GifError::Dimensions)?;

    let mut output = BoundedOutput::new(MAX_GIF_BYTES);
    {
        let mut encoder =
            Encoder::new(&mut output, width_u16, height_u16, &[]).map_err(map_encode_error)?;
        encoder
            .set_repeat(Repeat::Infinite)
            .map_err(map_encode_error)?;
        for (index, recorded) in frames.iter().enumerate() {
            let bytes = decode_frame(recorded, index)?;
            let (mut pixels, _, _) = decode_normalized(&bytes, index, width, height)?;
            let mut frame = Frame::from_rgba_speed(width_u16, height_u16, &mut pixels, 10);
            frame.delay = delay_centiseconds(recorded);
            encoder.write_frame(&frame).map_err(map_encode_error)?;
        }
    }
    output.finish()
}

fn decode_frame(
    frame: &PhysicalRecordingFrame,
    index: usize,
) -> Result<Zeroizing<Vec<u8>>, GifError> {
    if frame.mime_type != "image/jpeg" {
        return Err(GifError::Decode {
            index,
            reason: format!("unsupported recording MIME type {}", frame.mime_type),
        });
    }
    BASE64
        .decode(frame.data.as_bytes())
        .map(Zeroizing::new)
        .map_err(|error| GifError::Decode {
            index,
            reason: format!("invalid base64: {error}"),
        })
}

fn jpeg_dimensions(bytes: &[u8], index: usize) -> Result<(usize, usize), GifError> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
    decoder.read_info().map_err(|error| GifError::Decode {
        index,
        reason: error.to_string(),
    })?;
    let info = decoder.info().ok_or_else(|| GifError::Decode {
        index,
        reason: "missing JPEG dimensions".into(),
    })?;
    checked_dimensions(info.width, info.height, index)
}

fn decode_jpeg(bytes: &[u8], index: usize) -> Result<(Zeroizing<Vec<u8>>, usize, usize), GifError> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
    decoder.read_info().map_err(|error| GifError::Decode {
        index,
        reason: error.to_string(),
    })?;
    let info = decoder.info().ok_or_else(|| GifError::Decode {
        index,
        reason: "missing JPEG dimensions".into(),
    })?;
    let (width, height) = checked_dimensions(info.width, info.height, index)?;
    let pixels = Zeroizing::new(decoder.decode().map_err(|error| GifError::Decode {
        index,
        reason: error.to_string(),
    })?);
    let expected_pixels = width * height;
    let mut rgba = Zeroizing::new(Vec::with_capacity(expected_pixels * 4));
    match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 if pixels.len() == expected_pixels * 3 => {
            for pixel in pixels.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        jpeg_decoder::PixelFormat::L8 if pixels.len() == expected_pixels => {
            for value in pixels.iter().copied() {
                rgba.extend_from_slice(&[value, value, value, 255]);
            }
        }
        jpeg_decoder::PixelFormat::RGB24 | jpeg_decoder::PixelFormat::L8 => {
            return Err(GifError::Decode {
                index,
                reason: "decoded JPEG byte count does not match its dimensions".into(),
            });
        }
        other => {
            return Err(GifError::Decode {
                index,
                reason: format!("unsupported JPEG pixel format {other:?}"),
            });
        }
    }
    Ok((rgba, width, height))
}

fn checked_dimensions(width: u16, height: u16, index: usize) -> Result<(usize, usize), GifError> {
    let width = usize::from(width);
    let height = usize::from(height);
    if width == 0
        || height == 0
        || width
            .checked_mul(height)
            .is_none_or(|pixels| pixels > MAX_DECODED_PIXELS)
    {
        return Err(GifError::Decode {
            index,
            reason: "decoded pixel count exceeds the recording bound".into(),
        });
    }
    Ok((width, height))
}

fn decode_normalized(
    bytes: &[u8],
    index: usize,
    width: usize,
    height: usize,
) -> Result<(Zeroizing<Vec<u8>>, usize, usize), GifError> {
    let (pixels, source_width, source_height) = decode_jpeg(bytes, index)?;
    let normalized = if (source_width, source_height) == (width, height) {
        pixels
    } else {
        resize_nearest(&pixels, source_width, source_height, width, height)
    };
    Ok((normalized, source_width, source_height))
}

fn resize_nearest(
    source: &[u8],
    source_width: usize,
    source_height: usize,
    width: usize,
    height: usize,
) -> Zeroizing<Vec<u8>> {
    let mut resized = Zeroizing::new(vec![0_u8; width * height * 4]);
    for y in 0..height {
        let source_y = y * source_height / height;
        for x in 0..width {
            let source_x = x * source_width / width;
            let source_offset = (source_y * source_width + source_x) * 4;
            let destination_offset = (y * width + x) * 4;
            resized[destination_offset..destination_offset + 4]
                .copy_from_slice(&source[source_offset..source_offset + 4]);
        }
    }
    resized
}

fn delay_centiseconds(frame: &PhysicalRecordingFrame) -> u16 {
    let milliseconds = frame
        .duration_ms
        .clamp(MIN_FRAME_DELAY_MS, MAX_FRAME_DELAY_MS);
    u16::try_from(milliseconds.div_ceil(10)).unwrap_or(u16::MAX)
}

fn map_encode_error(error: gif::EncodingError) -> GifError {
    if matches!(&error, gif::EncodingError::Io(io_error) if io_error.kind() == io::ErrorKind::Other)
    {
        GifError::TooLarge
    } else {
        GifError::Encode(error.to_string())
    }
}

struct BoundedOutput {
    bytes: Zeroizing<Vec<u8>>,
    limit: usize,
}

impl BoundedOutput {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Zeroizing::new(Vec::new()),
            limit,
        }
    }

    fn finish(self) -> Result<Zeroizing<Vec<u8>>, GifError> {
        if self.bytes.len() > self.limit {
            Err(GifError::TooLarge)
        } else {
            Ok(self.bytes)
        }
    }
}

impl Write for BoundedOutput {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(buffer.len())
            .is_none_or(|length| length > self.limit)
        {
            return Err(io::Error::other("animated GIF exceeded its byte bound"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use ghostlight_bridge::browser::{PhysicalRecordingFrame, RecordingFrameKind};

    use super::{delay_centiseconds, encode, BoundedOutput, GifError};

    fn frame(data: &str, duration_ms: u64) -> PhysicalRecordingFrame {
        PhysicalRecordingFrame {
            frame_kind: RecordingFrameKind::Screencast,
            duration_ms,
            mime_type: "image/jpeg".into(),
            data: data.into(),
        }
    }

    #[test]
    fn empty_recording_is_decisive() {
        assert!(matches!(encode(&[]), Err(GifError::Empty)));
    }

    #[test]
    fn invalid_jpeg_is_decisive() {
        let frame = frame("AQID", 1_000);
        assert!(matches!(
            encode(&[frame]),
            Err(GifError::Decode { index: 0, .. })
        ));
    }

    #[test]
    fn frame_delays_use_extension_authored_visual_spans() {
        assert_eq!(delay_centiseconds(&frame("", 55)), 6);
        assert_eq!(delay_centiseconds(&frame("", 5_945)), 595);
        assert_eq!(delay_centiseconds(&frame("", 125)), 13);
        assert_eq!(delay_centiseconds(&frame("", 1_000)), 100);
        assert_eq!(delay_centiseconds(&frame("", 0)), 2);
    }

    #[test]
    fn bounded_writer_refuses_the_first_over_limit_write() {
        let mut output = BoundedOutput::new(3);
        assert_eq!(output.write(&[1, 2, 3]).expect("within bound"), 3);
        assert_eq!(
            output.write(&[4]).expect_err("over bound").kind(),
            std::io::ErrorKind::Other
        );
    }
}
