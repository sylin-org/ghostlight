//! Tests for the edge's diagnostics writer.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::Value;

use super::*;

/// A scratch directory removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "ghostlight-relay-log-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn marker(&self) -> PathBuf {
        self.0.join("diagnostics.on")
    }

    fn directory(&self) -> PathBuf {
        self.0.join("diagnostics")
    }

    fn log(&self) -> PathBuf {
        self.directory().join("edge-mcp.jsonl")
    }

    fn write(&self, detail: &str) -> io::Result<()> {
        append(
            &self.marker(),
            &self.directory(),
            1234,
            Component::EdgeMcp,
            Event::ServiceConnected,
            detail,
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn nothing_is_written_without_the_marker() {
    let scratch = Scratch::new();
    scratch.write("hello").unwrap();
    assert!(!scratch.directory().exists());
}

#[test]
fn a_line_is_appended_while_the_marker_exists() {
    let scratch = Scratch::new();
    fs::write(scratch.marker(), "").unwrap();
    scratch.write("first").unwrap();
    scratch.write("second").unwrap();
    let text = fs::read_to_string(scratch.log()).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0],
        json!({
            "ts_ms": 1234,
            "component": "edge-mcp",
            "relay": crate::RELAY,
            "pid": std::process::id(),
            "event": "service_connected",
            "detail": "first",
        })
    );
    assert_eq!(lines[1]["detail"], "second");
}

#[test]
fn removing_the_marker_stops_the_writing() {
    let scratch = Scratch::new();
    fs::write(scratch.marker(), "").unwrap();
    scratch.write("kept").unwrap();
    fs::remove_file(scratch.marker()).unwrap();
    scratch.write("dropped").unwrap();
    assert_eq!(
        fs::read_to_string(scratch.log()).unwrap().lines().count(),
        1
    );
}

#[test]
fn the_detail_is_bounded() {
    let scratch = Scratch::new();
    fs::write(scratch.marker(), "").unwrap();
    scratch.write(&"x".repeat(MAX_DETAIL_CHARS * 3)).unwrap();
    let line: Value =
        serde_json::from_str(fs::read_to_string(scratch.log()).unwrap().trim()).unwrap();
    assert_eq!(
        line["detail"].as_str().unwrap().chars().count(),
        MAX_DETAIL_CHARS
    );
}

#[test]
fn a_full_file_stops_growing() {
    let scratch = Scratch::new();
    fs::write(scratch.marker(), "").unwrap();
    fs::create_dir_all(scratch.directory()).unwrap();
    let full = vec![b' '; usize::try_from(MAX_FILE_BYTES).unwrap()];
    fs::write(scratch.log(), &full).unwrap();
    scratch.write("refused").unwrap();
    assert_eq!(fs::metadata(scratch.log()).unwrap().len(), MAX_FILE_BYTES);
}

#[test]
fn a_write_fault_is_an_error_only_below_record() {
    // `record` returns nothing, so this internal error is all a fault ever becomes.
    let scratch = Scratch::new();
    fs::write(scratch.marker(), "").unwrap();
    fs::write(scratch.directory(), "not a directory").unwrap();
    assert!(scratch.write("x").is_err());
}

#[cfg(unix)]
#[test]
fn the_first_line_creates_a_private_file_in_a_private_directory() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = Scratch::new();
    fs::write(scratch.marker(), "").unwrap();
    scratch.write("first").unwrap();
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&scratch.log()), 0o600);
    assert_eq!(mode(&scratch.directory()), 0o700);
}
