//! The edge's diagnostics writer.
//!
//! Diagnostics are off unless the marker file `home/diagnostics.on` exists. While it does, each
//! call to [`record`] appends one JSON line to `home/diagnostics/<component>.jsonl`. The writer
//! checks the marker on every call, so there is no watcher and no state to keep in step. It
//! never reports a failure to its caller: a diagnostics fault must not disturb an edge.
//!
//! Lines are content-free: an event name from the closed [`Event`] vocabulary and a short
//! detail. A file stops growing at [`MAX_FILE_BYTES`] and the service prunes it.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::{home, private, RELAY};

/// Longest detail, in characters, a line carries.
pub const MAX_DETAIL_CHARS: usize = 500;

/// Size at which a file stops receiving lines: 1 MiB.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Which relay peer is writing. The name is the log file's stem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Component {
    /// The edge running as the MCP relay.
    EdgeMcp,
    /// The edge running as the browser relay.
    EdgeBrowser,
}

impl Component {
    /// The component's name in log lines and file names.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EdgeMcp => "edge-mcp",
            Self::EdgeBrowser => "edge-browser",
        }
    }
}

/// The closed vocabulary of edge diagnostics events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    /// The edge process began.
    ProcessStarted,
    /// The edge process is about to exit.
    ProcessExited,
    /// The parent process went away.
    ParentGone,
    /// The session is ending, so the edge is leaving.
    SessionEnded,
    /// The edge tried to start the service.
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
    /// A service that became ready woke the edge ([`crate::wake`]).
    ServiceReady,
    /// The edge could not register its wake endpoint, so only its scheduled looks find a service.
    WakeFailed,
    /// A browser frame over the relay bound was dropped; the detail is its declared byte count.
    FrameSkipped,
}

impl Event {
    /// The event's name in log lines.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProcessStarted => "process_started",
            Self::ProcessExited => "process_exited",
            Self::ParentGone => "parent_gone",
            Self::SessionEnded => "session_ended",
            Self::ServiceStartAttempted => "service_start_attempted",
            Self::ServiceStartFailed => "service_start_failed",
            Self::ConnectFailed => "connect_failed",
            Self::ServiceConnected => "service_connected",
            Self::OpeningRefused => "opening_refused",
            Self::ServiceDisconnected => "service_disconnected",
            Self::ServiceReady => "service_ready",
            Self::WakeFailed => "wake_failed",
            Self::FrameSkipped => "frame_skipped",
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

/// The one place the edge's diagnostics line is built: what `record` appends, and what the
/// service's `diagnostics show` reads as a `ghostlight_bridge::diagnostics::Record`.
#[must_use]
pub fn line(ts_ms: u64, component: Component, event: Event, detail: &str) -> serde_json::Value {
    json!({
        "ts_ms": ts_ms,
        "component": component.as_str(),
        "relay": RELAY,
        "pid": std::process::id(),
        "event": event.as_str(),
        "detail": detail.chars().take(MAX_DETAIL_CHARS).collect::<String>(),
    })
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
    private::create_dir_all(directory)?;
    let path = directory.join(format!("{}.jsonl", component.as_str()));
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() >= MAX_FILE_BYTES) {
        return Ok(());
    }
    let mut line =
        serde_json::to_vec(&line(ts_ms, component, event, detail)).map_err(io::Error::other)?;
    line.push(b'\n');
    private::file_options()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(&line)
}

#[cfg(test)]
mod tests;
