// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The gif_creator encoding pipeline (ADR-0053): JPEG frames in, an annotated animated GIF out.
//!
//! This module is the service-side home of everything the thin-extension rule moved out of the
//! extension (ADR-0053 Decision 5): JPEG decode (`jpeg-decoder`), overlay compositing (overlay.rs,
//! reference geometry recolored to Ghostlight sky-blue), adaptive NeuQuant palette (quantize.rs via
//! `color_quant`), and the GIF89a writer (writer.rs, via the image-rs `gif`/weezl crate). Everything
//! here is pure computation over byte buffers -- deterministic,
//! cargo-tested, and safe to run under `spawn_blocking`; the extension's only remaining GIF role is
//! the screencast capture relay.

mod font;
pub(crate) mod overlay;
mod quantize;
mod writer;

pub use overlay::{describe_action, take_action_for_frame, ActionMeta};
use overlay::{Canvas, OverlayOptions};
use serde_json::Value;

/// One captured frame as recorded by the service: raw JPEG bytes plus overlay context.
pub struct RecordedFrame {
    /// The frame's JPEG bytes (a screencast frame or the seed screenshot).
    pub jpeg: Vec<u8>,
    /// Capture time (Unix ms) -- drives the per-frame GIF delay.
    pub ts_ms: i64,
    /// The CSS viewport width at capture; overlay scaleFactor = frame width / vp_w.
    pub vp_w: Option<f64>,
    /// The action this frame's paint shows, if one was tagged (ADR-0052 D4).
    pub action: Option<ActionMeta>,
}

/// Encoding failures. Everything else in the pipeline is total.
#[derive(Debug, thiserror::Error)]
pub enum GifError {
    #[error("recording has no frames")]
    Empty,
    #[error("frame {index} failed to decode: {reason}")]
    Decode { index: usize, reason: String },
    #[error("GIF assembly failed: {0}")]
    Encode(String),
}

/// Decode a JPEG into RGBA plus dimensions.
fn decode_jpeg_rgba(bytes: &[u8]) -> Result<(Vec<u8>, usize, usize), String> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
    let pixels = decoder.decode().map_err(|e| e.to_string())?;
    let info = decoder
        .info()
        .ok_or_else(|| "missing jpeg info".to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            let mut out = Vec::with_capacity(w * h * 4);
            for px in pixels.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        jpeg_decoder::PixelFormat::L8 => {
            let mut out = Vec::with_capacity(w * h * 4);
            for &v in &pixels {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        other => return Err(format!("unsupported jpeg pixel format {other:?}")),
    };
    Ok((rgba, w, h))
}

/// Nearest-neighbor resize (frames can differ slightly in dimensions when the viewport changes
/// mid-recording; everything is normalized to the first frame's grid, as the JS pipeline did).
fn resize_nearest(rgba: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    for y in 0..dh {
        let sy = (y * sh) / dh;
        for x in 0..dw {
            let sx = (x * sw) / dw;
            let (si, di) = ((sy * sw + sx) * 4, (y * dw + x) * 4);
            out[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
        }
    }
    out
}

/// Encode a recording into a complete animated GIF89a: decode each JPEG, composite the overlays
/// (gated by the tool's open `options` object -- all switches default on), learn one adaptive
/// global palette across all frames, and write the file with real per-frame timing.
pub fn encode_recording(frames: &[RecordedFrame], options: &Value) -> Result<Vec<u8>, GifError> {
    if frames.is_empty() {
        return Err(GifError::Empty);
    }
    let opts: OverlayOptions = overlay::resolve_overlay_options(options);

    let mut rgba_frames: Vec<Vec<u8>> = Vec::with_capacity(frames.len());
    let (mut w0, mut h0) = (0usize, 0usize);
    for (i, frame) in frames.iter().enumerate() {
        let (mut rgba, w, h) = decode_jpeg_rgba(&frame.jpeg)
            .map_err(|reason| GifError::Decode { index: i, reason })?;
        if i == 0 {
            (w0, h0) = (w, h);
        } else if (w, h) != (w0, h0) {
            rgba = resize_nearest(&rgba, w, h, w0, h0);
        }
        let mut canvas = Canvas {
            buf: &mut rgba,
            w: w0,
            h: h0,
        };
        let progress = (i + 1) as f64 / frames.len() as f64;
        overlay::composite_overlays(
            &mut canvas,
            frame.action.as_ref(),
            &opts,
            progress,
            frame.vp_w,
        );
        rgba_frames.push(rgba);
    }

    let refs: Vec<&[u8]> = rgba_frames.iter().map(|f| f.as_slice()).collect();
    let palette = quantize::build_global_palette(&refs, quantize::DEFAULT_SAMPLE_FAC);

    let stamps: Vec<i64> = frames.iter().map(|f| f.ts_ms).collect();
    let delays_ms = overlay::compute_frame_delays(&stamps);
    let indexed: Vec<writer::IndexedFrame> = rgba_frames
        .iter()
        .zip(delays_ms)
        .map(|(rgba, ms)| writer::IndexedFrame {
            indices: palette.quantize_frame(rgba),
            delay_cs: ((ms as f64 / 10.0).round() as u16).max(2),
        })
        .collect();

    writer::encode_gif(w0 as u16, h0 as u16, &palette.rgb, &indexed).map_err(GifError::Encode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const RED: &[u8] = include_bytes!("testdata/red4x4.jpg");
    const BLUE: &[u8] = include_bytes!("testdata/blue4x4.jpg");

    #[test]
    fn decodes_a_real_jpeg_fixture() {
        let (rgba, w, h) = decode_jpeg_rgba(RED).expect("fixture decodes");
        assert_eq!((w, h), (4, 4));
        assert_eq!(rgba.len(), 4 * 4 * 4);
        // Solid red at quality 90 stays near-red through JPEG artifacts.
        assert!(
            rgba[0] > 200 && rgba[1] < 80 && rgba[2] < 80,
            "top-left pixel is red-ish"
        );
        assert_eq!(rgba[3], 255, "opaque alpha");
    }

    #[test]
    fn encodes_a_two_frame_recording_end_to_end() {
        let frames = vec![
            RecordedFrame {
                jpeg: RED.to_vec(),
                ts_ms: 1000,
                vp_w: Some(4.0),
                action: None,
            },
            RecordedFrame {
                jpeg: BLUE.to_vec(),
                ts_ms: 1500,
                vp_w: Some(4.0),
                action: Some(ActionMeta {
                    kind: "left_click".to_string(),
                    coordinate: Some((2.0, 2.0)),
                    start_coordinate: None,
                    description: "left_click".to_string(),
                    ts_ms: 1400,
                }),
            },
        ];
        // Overlays off keeps the tiny 4x4 canvas readable; the pipeline still runs the full path.
        let gif = encode_recording(
            &frames,
            &json!({"showProgressBar": false, "showWatermark": false, "showClickIndicators": false, "showActionLabels": false}),
        )
        .expect("encodes");
        assert_eq!(&gif[0..6], b"GIF89a");
        assert_eq!(*gif.last().unwrap(), 0x3b);
        // Two image descriptors' GCE delays: 500ms -> 50cs, last frame hold 2800ms -> 280cs.
        // (Delay parsing is pinned in writer.rs's tests; here we assert the end-to-end size class.)
        assert!(gif.len() > 800, "header + GCT + two frames");
    }

    #[test]
    fn empty_recording_is_an_error() {
        assert!(matches!(
            encode_recording(&[], &Value::Null),
            Err(GifError::Empty)
        ));
    }

    #[test]
    fn resize_nearest_scales_dimensions() {
        // A 2x1 red|blue frame upscaled to 4x2 keeps left red, right blue.
        let src = [255, 0, 0, 255, 0, 0, 255, 255];
        let out = resize_nearest(&src, 2, 1, 4, 2);
        assert_eq!(out.len(), 4 * 2 * 4);
        assert_eq!(&out[0..4], &[255, 0, 0, 255], "left stays red");
        assert_eq!(&out[12..16], &[0, 0, 255, 255], "right stays blue");
    }
}
