//! Tests of how values are framed and read back.

use std::io::Cursor;

use serde_json::{json, Value};

use super::*;

/// A reader that hands out one byte per read, so every frame arrives split.
struct OneByteAtATime(Cursor<Vec<u8>>);

impl Read for OneByteAtATime {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let end = buffer.len().min(1);
        self.0.read(&mut buffer[..end])
    }
}

/// ACCEPTANCE C6: framing survives coalesced reads, split reads and disconnects. A frame is a
/// little-endian `u32` length and that many bytes, the same bytes as Chrome native messaging.
/// Frames written back to back are read one at a time, whether a read returns them all at once
/// or one byte at a time; the raw reader hands a payload back unparsed; the end of the stream
/// between frames is a clean end; and a stream cut inside a frame is truncated.
#[test]
fn framing_survives_coalesced_reads_split_reads_and_a_cut() {
    let mut bytes = Vec::new();
    write_frame_raw(&mut bytes, b"{}").unwrap();
    assert_eq!(
        bytes,
        [2, 0, 0, 0, b'{', b'}'],
        "a little-endian length, then the bytes"
    );
    let value = json!({"kind": "message", "line": "{\"jsonrpc\":\"2.0\"}"});
    write_frame(&mut bytes, &value).unwrap();
    write_frame_raw(&mut bytes, b"not json").unwrap();

    let readers: [(&str, Box<dyn Read>); 2] = [
        ("coalesced", Box::new(Cursor::new(bytes.clone()))),
        ("split", Box::new(OneByteAtATime(Cursor::new(bytes)))),
    ];
    for (reads, mut reader) in readers {
        assert_eq!(
            read_frame::<Value>(&mut reader).unwrap(),
            Some(json!({})),
            "{reads}"
        );
        assert_eq!(
            read_frame::<Value>(&mut reader).unwrap(),
            Some(value.clone()),
            "{reads}"
        );
        assert_eq!(
            read_frame_raw(&mut reader).unwrap().as_deref(),
            Some(&b"not json"[..]),
            "{reads}: the raw reader does not parse"
        );
        assert!(
            read_frame_raw(&mut reader).unwrap().is_none(),
            "{reads}: a clean end"
        );
    }

    for cut in [vec![9, 0], vec![9, 0, 0, 0, b'{']] {
        assert!(
            matches!(
                read_frame_raw(&mut Cursor::new(cut.clone())),
                Err(FrameError::Truncated)
            ),
            "{cut:?}"
        );
    }
}

/// Relay-protocol-4 section 10: the relay vocabulary's largest frame holds both ways. A frame
/// of exactly the limit crosses, a larger payload is never written, and a larger declared
/// length is refused before anything is allocated for it, so a peer cannot make a reader grow
/// by lying about a length.
#[test]
fn the_frame_limit_holds_both_ways_and_is_checked_before_allocation() {
    let at_limit = vec![b' '; MAX_FRAME_BYTES];
    let mut bytes = Vec::new();
    write_frame_raw(&mut bytes, &at_limit).unwrap();
    let read = read_frame_raw(&mut Cursor::new(bytes)).unwrap().unwrap();
    assert_eq!(read.len(), MAX_FRAME_BYTES);

    let over = vec![b' '; MAX_FRAME_BYTES + 1];
    let mut unwritten = Vec::new();
    assert!(matches!(
        write_frame_raw(&mut unwritten, &over),
        Err(FrameError::TooLarge)
    ));
    assert!(unwritten.is_empty(), "nothing of a larger frame is written");

    let declared = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
    assert!(
        matches!(
            read_frame_raw(&mut Cursor::new(declared.to_le_bytes().to_vec())),
            Err(FrameError::TooLarge)
        ),
        "only the header is there, so reading the length first would be truncated instead"
    );
}
