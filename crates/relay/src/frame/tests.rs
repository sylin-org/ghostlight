//! Tests of how values are framed and read back.

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
