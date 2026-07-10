// SPDX-License-Identifier: Apache-2.0 OR MIT
//! GIF89a writer: variable-width LZW + file framing (ADR-0053 Decision 5).
//!
//! Hand-ported from the extension's oracle-tested JS encoder (ADR-0050 D5 / ADR-0052 D3), which the
//! thin-extension rule relocated into the binary. The LZW code-size / clear discipline follows the
//! GIF89a spec (and the omggif MIT approach): codes start at `MIN_CODE_SIZE + 1` bits, the width
//! grows when the next code index reaches `2^codeSize`, and a Clear code resets the table at 4096.
//! The tests pin the same oracles the JS tests pinned: a spec-decodable roundtrip through an
//! INDEPENDENT decoder (which grows its width one entry earlier than the encoder -- the off-by-one
//! this port inherited pre-debugged), and per-frame Graphic Control Extension delay bytes.

use std::collections::HashMap;

/// LZW minimum code size for a 256-entry global color table.
const MIN_CODE_SIZE: u32 = 8;

/// One encoded frame: quantized palette indices plus its GIF delay in centiseconds.
pub(crate) struct IndexedFrame {
    pub indices: Vec<u8>,
    pub delay_cs: u16,
}

/// LSB-first bit packer for LZW codes.
struct BitWriter {
    out: Vec<u8>,
    bit_buf: u32,
    bit_cnt: u32,
}

impl BitWriter {
    fn emit(&mut self, code: u32, code_size: u32) {
        self.bit_buf |= code << self.bit_cnt;
        self.bit_cnt += code_size;
        while self.bit_cnt >= 8 {
            self.out.push((self.bit_buf & 0xff) as u8);
            self.bit_buf >>= 8;
            self.bit_cnt -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bit_cnt > 0 {
            self.out.push((self.bit_buf & 0xff) as u8);
        }
        self.out
    }
}

/// GIF variable-width LZW encode of a palette-index stream.
pub(crate) fn lzw_encode(indices: &[u8]) -> Vec<u8> {
    let clear_code: u32 = 1 << MIN_CODE_SIZE;
    let eoi_code: u32 = clear_code + 1;
    let mut code_size: u32 = MIN_CODE_SIZE + 1;
    let mut next_code: u32 = eoi_code + 1;
    let mut table: HashMap<u32, u32> = HashMap::new();
    let mut w = BitWriter {
        out: Vec::new(),
        bit_buf: 0,
        bit_cnt: 0,
    };

    w.emit(clear_code, code_size);
    let Some((&first, rest)) = indices.split_first() else {
        w.emit(eoi_code, code_size);
        return w.finish();
    };
    let mut prefix: u32 = first as u32;
    for &k in rest {
        let k = k as u32;
        // prefix is a CODE (<= 4095), k a symbol (<= 255): the combination is a unique key.
        let map_key = prefix * 4096 + k;
        if let Some(&code) = table.get(&map_key) {
            prefix = code;
        } else {
            w.emit(prefix, code_size);
            table.insert(map_key, next_code);
            next_code += 1;
            if next_code == (1 << code_size) && code_size < 12 {
                code_size += 1;
            }
            if next_code > 4095 {
                w.emit(clear_code, code_size);
                table.clear();
                code_size = MIN_CODE_SIZE + 1;
                next_code = eoi_code + 1;
            }
            prefix = k;
        }
    }
    w.emit(prefix, code_size);
    w.emit(eoi_code, code_size);
    w.finish()
}

fn push_u16_le(bytes: &mut Vec<u8>, v: u16) {
    bytes.extend_from_slice(&v.to_le_bytes());
}

/// Split LZW bytes into GIF sub-blocks (<= 255 each, length-prefixed), terminated by a 0x00 block.
fn push_sub_blocks(bytes: &mut Vec<u8>, data: &[u8]) {
    for chunk in data.chunks(255) {
        bytes.push(chunk.len() as u8);
        bytes.extend_from_slice(chunk);
    }
    bytes.push(0x00);
}

/// Assemble a complete animated GIF89a (looping forever): header, logical screen descriptor, the
/// 256-entry global color table (`palette_rgb`, 768 bytes), a NETSCAPE2.0 infinite-loop extension,
/// then per frame a Graphic Control Extension (disposal=1, per-frame delay) + full-frame image.
pub(crate) fn encode_gif(
    width: u16,
    height: u16,
    palette_rgb: &[u8],
    frames: &[IndexedFrame],
) -> Vec<u8> {
    debug_assert_eq!(
        palette_rgb.len(),
        256 * 3,
        "global color table is 256 RGB triples"
    );
    let mut bytes: Vec<u8> = Vec::new();

    bytes.extend_from_slice(b"GIF89a");
    push_u16_le(&mut bytes, width);
    push_u16_le(&mut bytes, height);
    bytes.push(0xf7); // GCT flag=1, color res=7, sort=0, GCT size=7 (2^8 = 256)
    bytes.push(0x00); // background color index
    bytes.push(0x00); // pixel aspect ratio
    bytes.extend_from_slice(palette_rgb);

    // NETSCAPE2.0 application extension: loop forever.
    bytes.extend_from_slice(&[0x21, 0xff, 0x0b]);
    bytes.extend_from_slice(b"NETSCAPE2.0");
    bytes.extend_from_slice(&[0x03, 0x01, 0x00, 0x00, 0x00]);

    for frame in frames {
        // Graphic Control Extension: disposal=1 (leave in place), no transparency, per-frame delay.
        bytes.extend_from_slice(&[0x21, 0xf9, 0x04, 0x04]);
        push_u16_le(&mut bytes, frame.delay_cs);
        bytes.extend_from_slice(&[0x00, 0x00]);

        // Image Descriptor: full frame, no local color table.
        bytes.push(0x2c);
        push_u16_le(&mut bytes, 0);
        push_u16_le(&mut bytes, 0);
        push_u16_le(&mut bytes, width);
        push_u16_le(&mut bytes, height);
        bytes.push(0x00);

        bytes.push(MIN_CODE_SIZE as u8);
        push_sub_blocks(&mut bytes, &lzw_encode(&frame.indices));
    }

    bytes.push(0x3b); // trailer
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    // A standalone GIF89a parser + LZW decoder: extract the FIRST frame's palette indices.
    // Independent of the encoder (a different code path), so a roundtrip that reconstructs the
    // input indices is real evidence the stream is spec-decodable, not merely self-consistent.
    fn decode_first_frame_indices(gif: &[u8]) -> (Vec<u8>, u16, u16) {
        let mut pos = 6usize;
        let width = u16::from_le_bytes([gif[pos], gif[pos + 1]]);
        let height = u16::from_le_bytes([gif[pos + 2], gif[pos + 3]]);
        let packed = gif[pos + 4];
        pos += 7;
        if packed & 0x80 != 0 {
            pos += 3 * (1usize << ((packed & 0x07) + 1));
        }
        loop {
            let b = gif[pos];
            pos += 1;
            match b {
                0x3b => panic!("trailer before an image"),
                0x21 => {
                    pos += 1; // extension label
                    loop {
                        let n = gif[pos] as usize;
                        pos += 1;
                        if n == 0 {
                            break;
                        }
                        pos += n;
                    }
                }
                0x2c => {
                    let lp = gif[pos + 8];
                    pos += 9;
                    if lp & 0x80 != 0 {
                        pos += 3 * (1usize << ((lp & 0x07) + 1));
                    }
                    let min_code_size = gif[pos] as u32;
                    pos += 1;
                    let mut data: Vec<u8> = Vec::new();
                    loop {
                        let n = gif[pos] as usize;
                        pos += 1;
                        if n == 0 {
                            break;
                        }
                        data.extend_from_slice(&gif[pos..pos + n]);
                        pos += n;
                    }
                    return (lzw_decode(min_code_size, &data), width, height);
                }
                other => panic!("unexpected block 0x{other:02x}"),
            }
        }
    }

    fn lzw_decode(min_code_size: u32, data: &[u8]) -> Vec<u8> {
        let clear: u32 = 1 << min_code_size;
        let eoi: u32 = clear + 1;
        let mut code_size = min_code_size + 1;
        let reset = |dict: &mut Vec<Option<Vec<u8>>>| {
            dict.clear();
            for i in 0..clear {
                dict.push(Some(vec![i as u8]));
            }
            dict.push(None); // clear
            dict.push(None); // eoi
        };
        let mut dict: Vec<Option<Vec<u8>>> = Vec::new();
        reset(&mut dict);
        let mut out: Vec<u8> = Vec::new();
        let mut bit_buf: u32 = 0;
        let mut bit_cnt: u32 = 0;
        let mut p = 0usize;
        let mut prev: Option<Vec<u8>> = None;
        loop {
            if p >= data.len() && bit_cnt < code_size {
                break;
            }
            while bit_cnt < code_size {
                bit_buf |= (data.get(p).copied().unwrap_or(0) as u32) << bit_cnt;
                p += 1;
                bit_cnt += 8;
            }
            let code = bit_buf & ((1 << code_size) - 1);
            bit_buf >>= code_size;
            bit_cnt -= code_size;
            if code == clear {
                reset(&mut dict);
                code_size = min_code_size + 1;
                prev = None;
                continue;
            }
            if code == eoi {
                break;
            }
            let entry: Vec<u8> = match dict.get(code as usize) {
                Some(Some(e)) => e.clone(),
                _ if code as usize == dict.len() && prev.is_some() => {
                    let pv = prev.as_ref().unwrap();
                    let mut e = pv.clone();
                    e.push(pv[0]);
                    e
                }
                _ => panic!("bad LZW code {code}"),
            };
            out.extend_from_slice(&entry);
            if let Some(pv) = prev {
                let mut grown = pv.clone();
                grown.push(entry[0]);
                dict.push(Some(grown));
                // The decoder builds its table one entry BEHIND the encoder, so it must grow the
                // code width one entry EARLIER than the encoder's `next_code == 2^width` point.
                if dict.len() as u32 == (1 << code_size) - 1 && code_size < 12 {
                    code_size += 1;
                }
            }
            prev = Some(entry);
        }
        out
    }

    // Walk the block structure and collect each frame's GCE delay (centiseconds). Structure-aware,
    // so a delay-like byte pattern inside LZW data cannot be misread.
    fn read_gce_delays_cs(gif: &[u8]) -> Vec<u16> {
        let mut pos = 6usize;
        let packed = gif[pos + 4];
        pos += 7;
        if packed & 0x80 != 0 {
            pos += 3 * (1usize << ((packed & 0x07) + 1));
        }
        let mut delays = Vec::new();
        loop {
            let b = gif[pos];
            pos += 1;
            match b {
                0x3b => return delays,
                0x21 => {
                    let label = gif[pos];
                    pos += 1;
                    if label == 0xf9 {
                        // sub-block: size=0x04, packed, delayLo, delayHi, transparent index.
                        delays.push(u16::from_le_bytes([gif[pos + 2], gif[pos + 3]]));
                    }
                    loop {
                        let n = gif[pos] as usize;
                        pos += 1;
                        if n == 0 {
                            break;
                        }
                        pos += n;
                    }
                }
                0x2c => {
                    let lp = gif[pos + 8];
                    pos += 9;
                    if lp & 0x80 != 0 {
                        pos += 3 * (1usize << ((lp & 0x07) + 1));
                    }
                    pos += 1; // LZW minimum code size
                    loop {
                        let n = gif[pos] as usize;
                        pos += 1;
                        if n == 0 {
                            break;
                        }
                        pos += n;
                    }
                }
                other => panic!("unexpected block 0x{other:02x}"),
            }
        }
    }

    fn gray_palette() -> Vec<u8> {
        let mut p = Vec::with_capacity(768);
        for i in 0..256u16 {
            let v = i as u8;
            p.extend_from_slice(&[v, v, v]);
        }
        p
    }

    #[test]
    fn header_trailer_and_gct_shape() {
        let gif = encode_gif(
            2,
            2,
            &gray_palette(),
            &[IndexedFrame {
                indices: vec![0, 0, 0, 0],
                delay_cs: 10,
            }],
        );
        assert_eq!(&gif[0..6], b"GIF89a");
        assert_eq!(*gif.last().unwrap(), 0x3b, "ends with the GIF trailer");
        assert_eq!(
            (gif[10] & 0x07) + 1,
            8,
            "global color table is 2^8 = 256 entries"
        );
        assert!(
            gif.len() > 800,
            "non-trivial (header + 256-entry GCT + a frame)"
        );
    }

    #[test]
    fn solid_frame_roundtrips_through_an_independent_decoder() {
        // The ported JS oracle: indices [0,0,0,0] must decode back exactly via the independent
        // decoder (emitted 9-bit codes [clear=256, 0, 258, 0, eoi=257], LSB-packed).
        let gif = encode_gif(
            2,
            2,
            &gray_palette(),
            &[IndexedFrame {
                indices: vec![0, 0, 0, 0],
                delay_cs: 10,
            }],
        );
        let (indices, w, h) = decode_first_frame_indices(&gif);
        assert_eq!((w, h), (2, 2));
        assert_eq!(indices, vec![0, 0, 0, 0]);
    }

    #[test]
    fn table_growth_gradient_roundtrips() {
        // A 32x32 frame with many distinct indices forces the LZW table to grow and the code width
        // to bump past 9 bits -- the code-size growth path, not just the base case.
        let (w, h) = (32u16, 32u16);
        let indices: Vec<u8> = (0..(w as usize * h as usize))
            .map(|i| (i * 7 % 251) as u8)
            .collect();
        let gif = encode_gif(
            w,
            h,
            &gray_palette(),
            &[IndexedFrame {
                indices: indices.clone(),
                delay_cs: 6,
            }],
        );
        let (decoded, dw, dh) = decode_first_frame_indices(&gif);
        assert_eq!((dw, dh), (w, h));
        assert_eq!(
            decoded, indices,
            "the decoded indices equal the encoder's input"
        );
    }

    #[test]
    fn per_frame_delays_land_in_each_gce() {
        // The ported JS oracle: delays [250, 100, 2800] ms were written as [25, 10, 280] cs.
        let frames: Vec<IndexedFrame> = [25u16, 10, 280]
            .iter()
            .map(|&cs| IndexedFrame {
                indices: vec![0, 1, 2, 3],
                delay_cs: cs,
            })
            .collect();
        let gif = encode_gif(2, 2, &gray_palette(), &frames);
        assert_eq!(read_gce_delays_cs(&gif), vec![25, 10, 280]);
    }

    #[test]
    fn empty_index_stream_still_emits_a_valid_block() {
        let out = lzw_encode(&[]);
        assert!(!out.is_empty(), "clear + eoi at minimum");
    }
}
