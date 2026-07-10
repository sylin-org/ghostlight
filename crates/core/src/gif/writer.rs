// SPDX-License-Identifier: Apache-2.0 OR MIT
//! GIF89a writer (ADR-0053 Decision 5): assemble the frames into an animated GIF89a.
//!
//! LZW + file framing come from the image-rs `gif` crate (weezl LZW -- the reference
//! implementation). An earlier hand-port of the JS encoder carried a latent code-width-timing
//! off-by-one at the very first 9->10 bit transition: it round-tripped through its own matched
//! decoder (so every self-test passed) but produced a bitstream that strict third-party decoders
//! (Pillow/giflib, and browsers) reject with "broken data stream". The lesson is the project's own
//! rule -- do not hand-roll a codec; use the battle-tested library (ADR-0008, and the owner's
//! call during the ADR-0053 live test).

use std::borrow::Cow;

/// One encoded frame: quantized palette indices plus its GIF delay in centiseconds.
pub(crate) struct IndexedFrame {
    pub indices: Vec<u8>,
    pub delay_cs: u16,
}

/// Assemble a complete animated GIF89a (looping forever) from a shared 256-entry global color
/// table (`palette_rgb`, 768 bytes) and per-frame palette-index buffers with per-frame delays.
pub(crate) fn encode_gif(
    width: u16,
    height: u16,
    palette_rgb: &[u8],
    frames: &[IndexedFrame],
) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    {
        let mut encoder =
            gif::Encoder::new(&mut out, width, height, palette_rgb).map_err(|e| e.to_string())?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| e.to_string())?;
        for f in frames {
            let mut frame = gif::Frame {
                width,
                height,
                delay: f.delay_cs,
                dispose: gif::DisposalMethod::Keep,
                buffer: Cow::Borrowed(&f.indices),
                ..gif::Frame::default()
            };
            // Use the global color table, not a per-frame local one.
            frame.palette = None;
            encoder.write_frame(&frame).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray_palette() -> Vec<u8> {
        let mut p = Vec::with_capacity(768);
        for i in 0..256u16 {
            let v = i as u8;
            p.extend_from_slice(&[v, v, v]);
        }
        p
    }

    /// Decode a GIF with the `gif` crate: return each frame's (indices, delay_cs) plus the canvas
    /// dimensions. A different decode than any we wrote by hand (weezl), so a roundtrip is real
    /// evidence the stream is spec-decodable -- the property the hand-rolled encoder lacked.
    fn decode(gif_bytes: &[u8]) -> (Vec<(Vec<u8>, u16)>, u16, u16) {
        let mut opts = gif::DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::Indexed);
        let mut decoder = opts.read_info(std::io::Cursor::new(gif_bytes)).unwrap();
        let (w, h) = (decoder.width(), decoder.height());
        let mut frames = Vec::new();
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            frames.push((frame.buffer.to_vec(), frame.delay));
        }
        (frames, w, h)
    }

    #[test]
    fn header_and_frame_count() {
        let gif = encode_gif(
            2,
            2,
            &gray_palette(),
            &[IndexedFrame {
                indices: vec![0, 0, 0, 0],
                delay_cs: 10,
            }],
        )
        .unwrap();
        assert_eq!(&gif[0..6], b"GIF89a");
        assert_eq!(*gif.last().unwrap(), 0x3b, "ends with the GIF trailer");
        let (frames, w, h) = decode(&gif);
        assert_eq!((w, h), (2, 2));
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn solid_frame_roundtrips() {
        let gif = encode_gif(
            2,
            2,
            &gray_palette(),
            &[IndexedFrame {
                indices: vec![7, 7, 7, 7],
                delay_cs: 10,
            }],
        )
        .unwrap();
        let (frames, _, _) = decode(&gif);
        assert_eq!(frames[0].0, vec![7, 7, 7, 7]);
    }

    #[test]
    fn per_frame_delays_survive_encode() {
        let f = |cs: u16| IndexedFrame {
            indices: vec![0, 1, 2, 3],
            delay_cs: cs,
        };
        let gif = encode_gif(2, 2, &gray_palette(), &[f(25), f(10), f(280)]).unwrap();
        let (frames, _, _) = decode(&gif);
        assert_eq!(
            frames.iter().map(|(_, d)| *d).collect::<Vec<_>>(),
            vec![25, 10, 280]
        );
    }

    #[test]
    fn large_frame_crosses_every_code_width_and_roundtrips() {
        // The regression the hand-rolled LZW failed: a frame large enough to cross the 9->10 bit
        // transition (>512 codes) and beyond, round-tripped through the independent weezl decoder.
        let (w, h) = (600u16, 4u16);
        let n = w as usize * h as usize;
        let mut state: u32 = 0x1234_5678;
        let indices: Vec<u8> = (0..n)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 24) as u8
            })
            .collect();
        let gif = encode_gif(
            w,
            h,
            &gray_palette(),
            &[IndexedFrame {
                indices: indices.clone(),
                delay_cs: 5,
            }],
        )
        .unwrap();
        let (frames, dw, dh) = decode(&gif);
        assert_eq!((dw, dh), (w, h));
        assert_eq!(
            frames[0].0, indices,
            "large frame survives encode + independent decode"
        );
    }
}
