// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Minimal standard base64 (RFC 4648, with padding) -- encode + decode, std-only.
//!
//! Used by the gif_creator recording path (ADR-0053): screencast frames arrive from the extension
//! as base64 JPEG and the encoded GIF returns to MCP clients as base64. Hand-rolled per the
//! project's dependency posture (the codebase already hand-rolls the encode half for the
//! WebSocket accept key); both directions are oracle-tested below.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode bytes as standard base64 with padding.
pub(crate) fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn value_of(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some((c - b'A') as u32),
        b'a'..=b'z' => Some((c - b'a') as u32 + 26),
        b'0'..=b'9' => Some((c - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decode standard base64 (padding optional; whitespace rejected). None on any invalid input.
pub(crate) fn decode(s: &str) -> Option<Vec<u8>> {
    let trimmed = s.trim_end_matches('=');
    let mut out = Vec::with_capacity(trimmed.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for &c in trimmed.as_bytes() {
        acc = (acc << 6) | value_of(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    // Leftover bits must be zero-padding remnants of a valid final quantum.
    if bits > 0 && acc & ((1 << bits) - 1) != 0 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4648_vectors_round_trip() {
        // The RFC 4648 section 10 oracle vectors.
        let vectors: [(&str, &str); 7] = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, encoded) in vectors {
            assert_eq!(encode(plain.as_bytes()), encoded);
            assert_eq!(decode(encoded).unwrap(), plain.as_bytes());
        }
    }

    #[test]
    fn binary_round_trip_and_unpadded_decode() {
        let bytes: Vec<u8> = (0u16..=255).map(|b| b as u8).collect();
        let enc = encode(&bytes);
        assert_eq!(decode(&enc).unwrap(), bytes);
        assert_eq!(decode("Zm9vYg").unwrap(), b"foob", "unpadded input decodes");
    }

    #[test]
    fn invalid_input_is_none() {
        assert!(decode("not base64!").is_none());
        assert!(decode("Zm9v\n").is_none(), "whitespace rejected");
    }
}
