// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Adaptive 256-color global palette for GIF frames (ADR-0053 Decision 5).
//!
//! Quantization is NeuQuant via the `color_quant` crate -- the image-rs port of the SAME Dekker
//! algorithm the official extension ships in gif.js and that ADR-0050 vendored in JS; relocated
//! here by the thin-extension rule. One global palette is trained across ALL frames (bounded,
//! deterministically strided sample) so the animation shares a single global color table; every
//! pixel then maps to its nearest palette entry through the network's own search, memoized per
//! distinct color (screenshots hold thousands of distinct colors, not millions -- the memo is the
//! measured ~34x difference between an encode that stalls and one that does not, ADR-0052 D3).

use std::collections::HashMap;

use color_quant::NeuQuant;

/// gif.js's default quality/sampling factor (1 = best/slowest .. 30 = coarsest/fastest).
pub(crate) const DEFAULT_SAMPLE_FAC: i32 = 10;

/// Cap on pixels fed to NeuQuant training (bounds memory + time; deterministic stride).
const TRAIN_PIXEL_BUDGET: usize = 500_000;

/// The trained global palette: 256 RGB triples plus the network for nearest-color lookups.
pub(crate) struct GlobalPalette {
    nq: NeuQuant,
    /// 768 bytes: 256 * RGB.
    pub rgb: Vec<u8>,
}

/// Train one adaptive 256-color palette from all frames' pixels. `frames` are RGBA buffers of
/// equal length. Training pixels are sub-sampled with a deterministic stride so long recordings
/// stay bounded; NeuQuant sub-samples that buffer again per `sample_fac`.
pub(crate) fn build_global_palette(frames: &[&[u8]], sample_fac: i32) -> GlobalPalette {
    let total_px: usize = frames.iter().map(|f| f.len() / 4).sum();
    let stride = if total_px > TRAIN_PIXEL_BUDGET {
        (total_px / TRAIN_PIXEL_BUDGET).max(1)
    } else {
        1
    };

    let mut train: Vec<u8> = Vec::with_capacity((total_px / stride + 1) * 4);
    let mut g = 0usize;
    for frame in frames {
        for px in frame.chunks_exact(4) {
            if g.is_multiple_of(stride) {
                // Alpha is forced opaque: screenshots are opaque, and a constant alpha keeps the
                // 4-channel network's color placement driven by RGB alone.
                train.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            g += 1;
        }
    }
    if train.is_empty() {
        train.extend_from_slice(&[0, 0, 0, 255]);
    }

    let nq = NeuQuant::new(sample_fac, 256, &train);
    let rgba = nq.color_map_rgba();
    let mut rgb = Vec::with_capacity(768);
    for entry in rgba.chunks_exact(4) {
        rgb.extend_from_slice(&entry[..3]);
    }
    GlobalPalette { nq, rgb }
}

impl GlobalPalette {
    /// Map a whole RGBA frame to palette indices via the network's nearest-color search, memoized
    /// per distinct color.
    pub(crate) fn quantize_frame(&self, rgba: &[u8]) -> Vec<u8> {
        let mut cache: HashMap<u32, u8> = HashMap::new();
        let mut out = Vec::with_capacity(rgba.len() / 4);
        for px in rgba.chunks_exact(4) {
            let key = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
            let idx = *cache
                .entry(key)
                .or_insert_with(|| self.nq.index_of(&[px[0], px[1], px[2], 255]) as u8);
            out.push(idx);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_frame(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
        let mut f = Vec::with_capacity(w * h * 4);
        for _ in 0..w * h {
            f.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
        f
    }

    #[test]
    fn deterministic_and_primaries_converge() {
        // The ported JS oracle: frames must clear NeuQuant's minpicturebytes floor for real
        // learning; 32x32x3 frames converge to within ~1 of each primary (measured in the JS
        // port), so 16 is safe slack. Identical input must yield an identical palette.
        let r = solid_frame(32, 32, [255, 0, 0]);
        let g = solid_frame(32, 32, [0, 255, 0]);
        let b = solid_frame(32, 32, [0, 0, 255]);
        let frames: Vec<&[u8]> = vec![&r, &g, &b];
        let a1 = build_global_palette(&frames, DEFAULT_SAMPLE_FAC);
        let a2 = build_global_palette(&frames, DEFAULT_SAMPLE_FAC);
        assert_eq!(a1.rgb, a2.rgb, "same frames -> identical palette");

        let near = |pal: &GlobalPalette, rgb: [u8; 3]| -> f64 {
            let idx = pal.nq.index_of(&[rgb[0], rgb[1], rgb[2], 255]);
            let p = &pal.rgb[idx * 3..idx * 3 + 3];
            let (dr, dg, db) = (
                p[0] as f64 - rgb[0] as f64,
                p[1] as f64 - rgb[1] as f64,
                p[2] as f64 - rgb[2] as f64,
            );
            (dr * dr + dg * dg + db * db).sqrt()
        };
        assert!(near(&a1, [255, 0, 0]) < 16.0, "red maps near red");
        assert!(near(&a1, [0, 255, 0]) < 16.0, "green maps near green");
        assert!(near(&a1, [0, 0, 255]) < 16.0, "blue maps near blue");
    }

    #[test]
    fn quantize_maps_a_solid_frame_to_identical_indices() {
        let f = solid_frame(8, 8, [10, 200, 40]);
        let frames: Vec<&[u8]> = vec![&f];
        let pal = build_global_palette(&frames, DEFAULT_SAMPLE_FAC);
        let idx = pal.quantize_frame(&f);
        assert_eq!(idx.len(), 64);
        assert!(
            idx.windows(2).all(|w| w[0] == w[1]),
            "solid frame -> identical indices"
        );
    }

    #[test]
    fn empty_input_yields_a_valid_palette() {
        let frames: Vec<&[u8]> = vec![];
        let pal = build_global_palette(&frames, DEFAULT_SAMPLE_FAC);
        assert_eq!(pal.rgb.len(), 768);
    }
}
