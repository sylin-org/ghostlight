//! Tests for the edge's diagnostics writer.

use std::path::PathBuf;

use serde_json::Value;

use super::*;
use crate::tests::Scratch;

/// A home in a scratch directory, with the diagnostics marker and directory the writer uses.
struct LogHome(Scratch);

impl LogHome {
    fn new() -> Self {
        Self(Scratch::named("log"))
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

#[test]
fn nothing_is_written_without_the_marker() {
    let home = LogHome::new();
    home.write("hello").unwrap();
    assert!(!home.directory().exists());
}

#[test]
fn a_line_is_appended_while_the_marker_exists() {
    let home = LogHome::new();
    fs::write(home.marker(), "").unwrap();
    home.write("first").unwrap();
    home.write("second").unwrap();
    let text = fs::read_to_string(home.log()).unwrap();
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
    let home = LogHome::new();
    fs::write(home.marker(), "").unwrap();
    home.write("kept").unwrap();
    fs::remove_file(home.marker()).unwrap();
    home.write("dropped").unwrap();
    assert_eq!(fs::read_to_string(home.log()).unwrap().lines().count(), 1);
}

#[test]
fn the_detail_is_bounded() {
    let home = LogHome::new();
    fs::write(home.marker(), "").unwrap();
    home.write(&"x".repeat(MAX_DETAIL_CHARS * 3)).unwrap();
    let line: Value = serde_json::from_str(fs::read_to_string(home.log()).unwrap().trim()).unwrap();
    assert_eq!(
        line["detail"].as_str().unwrap().chars().count(),
        MAX_DETAIL_CHARS
    );
}

#[test]
fn a_full_file_stops_growing() {
    let home = LogHome::new();
    fs::write(home.marker(), "").unwrap();
    fs::create_dir_all(home.directory()).unwrap();
    let full = vec![b' '; usize::try_from(MAX_FILE_BYTES).unwrap()];
    fs::write(home.log(), &full).unwrap();
    home.write("refused").unwrap();
    assert_eq!(fs::metadata(home.log()).unwrap().len(), MAX_FILE_BYTES);
}

#[test]
fn a_write_fault_is_an_error_only_below_record() {
    // `record` returns nothing, so this internal error is all a fault ever becomes.
    let home = LogHome::new();
    fs::write(home.marker(), "").unwrap();
    fs::write(home.directory(), "not a directory").unwrap();
    assert!(home.write("x").is_err());
}

#[cfg(unix)]
#[test]
fn the_first_line_creates_a_private_file_in_a_private_directory() {
    use std::os::unix::fs::PermissionsExt;

    let home = LogHome::new();
    fs::write(home.marker(), "").unwrap();
    home.write("first").unwrap();
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&home.log()), 0o600);
    assert_eq!(mode(&home.directory()), 0o700);
}
