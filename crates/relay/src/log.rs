//! The connectors' diagnostics writer.
//!
//! Diagnostics are off unless the marker file `home/diagnostics.on` exists. While it does, each
//! call to [`record`] appends one JSON line to `home/diagnostics/<component>.jsonl`. The writer
//! checks the marker on every call, so there is no watcher and no state to keep in step. It
//! never reports a failure to its caller: a diagnostics fault must not disturb a connector.
//!
//! Lines are content-free: an event name from the closed [`Event`] vocabulary and a short
//! detail. A file stops growing at [`MAX_FILE_BYTES`] and the service prunes it.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::{home, RELAY};

/// Longest detail, in characters, a line carries.
pub const MAX_DETAIL_CHARS: usize = 500;

/// Size at which a file stops receiving lines: 1 MiB.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Which relay peer is writing. The name is the log file's stem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Component {
    /// The MCP stdio connector.
    McpConnector,
    /// The browser native-messaging connector.
    BrowserConnector,
}

impl Component {
    /// The component's name in log lines and file names.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::McpConnector => "mcp-connector",
            Self::BrowserConnector => "browser-connector",
        }
    }
}

/// The closed vocabulary of connector diagnostics events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    /// The connector process began.
    ProcessStarted,
    /// The connector process is about to exit.
    ProcessExited,
    /// The parent process went away.
    ParentGone,
    /// The connector tried to start the service.
    ServiceStartAttempted,
    /// Starting the service failed.
    ServiceStartFailed,
    /// Connecting to the service failed.
    ConnectFailed,
    /// The service accepted an opening.
    ServiceConnected,
    /// The service refused an opening.
    OpeningRefused,
    /// The service connection ended.
    ServiceDisconnected,
}

impl Event {
    /// The event's name in log lines.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProcessStarted => "process_started",
            Self::ProcessExited => "process_exited",
            Self::ParentGone => "parent_gone",
            Self::ServiceStartAttempted => "service_start_attempted",
            Self::ServiceStartFailed => "service_start_failed",
            Self::ConnectFailed => "connect_failed",
            Self::ServiceConnected => "service_connected",
            Self::OpeningRefused => "opening_refused",
            Self::ServiceDisconnected => "service_disconnected",
        }
    }
}

/// Append one line for `event` when diagnostics are on. Failures are swallowed.
pub fn record(component: Component, event: Event, detail: &str) {
    let _ = try_record(component, event, detail);
}

/// [`record`] with the home lookup and the clock, returning what went wrong.
fn try_record(component: Component, event: Event, detail: &str) -> io::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(0));
    append(
        &home::diagnostics_marker()?,
        &home::diagnostics_dir()?,
        now,
        component,
        event,
        detail,
    )
}

/// Append one line to `directory/<component>.jsonl` when `marker` exists and the file has room.
fn append(
    marker: &Path,
    directory: &Path,
    ts_ms: u64,
    component: Component,
    event: Event,
    detail: &str,
) -> io::Result<()> {
    if !marker.exists() {
        return Ok(());
    }
    fs::create_dir_all(directory)?;
    let path = directory.join(format!("{}.jsonl", component.as_str()));
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() >= MAX_FILE_BYTES) {
        return Ok(());
    }
    let mut line = serde_json::to_vec(&json!({
        "ts_ms": ts_ms,
        "component": component.as_str(),
        "relay": RELAY,
        "pid": std::process::id(),
        "event": event.as_str(),
        "detail": detail.chars().take(MAX_DETAIL_CHARS).collect::<String>(),
    }))
    .map_err(io::Error::other)?;
    line.push(b'\n');
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(&line)
}

#[cfg(test)]
mod tests {
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
            self.directory().join("mcp-connector.jsonl")
        }

        fn write(&self, detail: &str) -> io::Result<()> {
            append(
                &self.marker(),
                &self.directory(),
                1234,
                Component::McpConnector,
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
                "component": "mcp-connector",
                "relay": 3,
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
}
