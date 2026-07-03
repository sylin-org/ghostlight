//! Debug / observability sink for the mcp-server role.
//!
//! When enabled (`--debug` or `GHOSTLIGHT_DEBUG=1`), the server records what it does at the three
//! process boundaries -- MCP request/response, tool-call begin/end, extension connect/disconnect --
//! into two per-process files under the log directory:
//! - `debug-state-<pid>.json`: a live snapshot (pid, uptime, extension connected?, in-flight calls,
//!   counters, recent events) that `ghostlight status` reads (newest session wins).
//! - `debug-events-<pid>.jsonl`: the append-only structured event stream (one JSON object per
//!   line), a full firehose for post-hoc inspection and a precursor to the v1.5 audit subsystem.
//!
//! Files are per-PID so two concurrent `--debug` servers never clobber each other's logs.
//!
//! This is **operational** observability, deliberately distinct from the governance-overlay audit
//! subsystem (see `lib.rs`). When disabled the sink is a cheap no-op: every method returns at once.
//! Enabled, it is best-effort and self-contained: a debug I/O failure or even a panic while holding
//! the lock must never disturb the server, so the lock is poison-recovering and I/O errors are
//! swallowed.

use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How many recent events to keep in the live snapshot (the JSONL log keeps everything).
const RECENT_CAP: usize = 64;

/// Max length of an event `detail` body before truncation (keeps screenshots out of the log).
const DETAIL_MAX: usize = 600;

/// Max length of a client-controlled identifier (tool / method / id) embedded in a summary.
const IDENT_MAX: usize = 120;

/// Minimum interval between full `debug-state.json` rewrites. The JSONL log is always appended (one
/// small line); only the full-snapshot rewrite -- the expensive part on the async hot path -- is
/// throttled. Connection transitions bypass this so the connected flag is always accurate.
const STATE_THROTTLE_MS: u128 = 200;

/// Session files older than this are best-effort cleaned up when a new session starts.
const STALE_AFTER: Duration = Duration::from_secs(24 * 3600);

/// Milliseconds since the Unix epoch (best-effort; 0 if the clock is before the epoch).
pub(crate) fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// The log directory: `GHOSTLIGHT_LOG_DIR`, else `<data-local>/ghostlight`.
pub fn log_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("GHOSTLIGHT_LOG_DIR") {
        return Some(PathBuf::from(dir));
    }
    dirs::data_local_dir().map(|d| d.join("ghostlight"))
}

/// Truncate `s` to at most `max` bytes on a UTF-8 char boundary (so non-ASCII never panics).
fn boundary_clip(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Format a millisecond duration compactly ("3m 12s", "800ms").
pub(crate) fn fmt_ms(ms: u128) -> String {
    let secs = ms / 1000;
    if secs == 0 {
        return format!("{ms}ms");
    }
    let (m, s) = (secs / 60, secs % 60);
    if m == 0 {
        format!("{s}s")
    } else {
        format!("{m}m {s}s")
    }
}

/// Session `debug-state-*.json` files under `dir`, newest (by mtime) first.
pub(crate) fn session_state_files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("debug-state-") && name.ends_with(".json") {
                let mtime = e.metadata().and_then(|m| m.modified()).ok()?;
                Some((mtime, e.path()))
            } else {
                None
            }
        })
        .collect();
    found.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));
    found.into_iter().map(|(_, p)| p).collect()
}

/// Best-effort removal of session files older than [`STALE_AFTER`] (bounds log-dir litter).
fn cleanup_stale(dir: &Path) {
    let now = SystemTime::now();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let ours = (name.starts_with("debug-state-") && name.ends_with(".json"))
            || (name.starts_with("debug-events-") && name.ends_with(".jsonl"));
        if !ours {
            continue;
        }
        if let Ok(mtime) = e.metadata().and_then(|m| m.modified()) {
            if now
                .duration_since(mtime)
                .map(|age| age > STALE_AFTER)
                .unwrap_or(false)
            {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// The raw newest `debug-state-<pid>.json` contents, if a debug session has written one.
pub fn raw_state() -> Option<String> {
    let dir = log_dir()?;
    let newest = session_state_files(&dir).into_iter().next()?;
    std::fs::read_to_string(newest).ok()
}

/// A human-readable `ghostlight status` report, or a hint when no debug session is found.
///
/// Role-aware: `status` describes the mcp-server role only. A state file is a *candidate* when it
/// parses as JSON and its `role` field is either absent (old-format files, written before the
/// `role` field existed) or equal to `"mcp-server"`; native-host state files (see `doctor`, which
/// does read those) are skipped here rather than misreported as a server session.
pub fn status_report() -> String {
    let Some(dir) = log_dir() else {
        return "no log directory available on this platform".to_string();
    };
    let files = session_state_files(&dir);
    if files.is_empty() {
        return format!(
            "no debug state under {}\nstart the server with --debug (or GHOSTLIGHT_DEBUG=1), then re-run status",
            dir.display()
        );
    }
    let candidates: Vec<(PathBuf, serde_json::Value)> = files
        .into_iter()
        .filter_map(|p| {
            let raw = std::fs::read_to_string(&p).ok()?;
            let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
            let role = v
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("mcp-server");
            (role == "mcp-server").then_some((p, v))
        })
        .collect();
    let Some((_path, v)) = candidates.first() else {
        return format!(
            "no mcp-server debug state under {} (state files exist for other roles or are unreadable)",
            dir.display()
        );
    };

    let now = now_ms();
    let get_ms = |k: &str| v.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0) as u128;
    let started = get_ms("started_ms");
    let updated = get_ms("updated_ms");
    let counters = &v["counters"];
    let cn = |k: &str| {
        counters
            .get(k)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    };

    let mut out = String::from("ghostlight status\n");
    out += &format!("  pid            {}\n", v["pid"].as_u64().unwrap_or(0));
    out += &format!("  uptime         {}\n", fmt_ms(now.saturating_sub(started)));
    out += &format!(
        "  updated        {} ago\n",
        fmt_ms(now.saturating_sub(updated))
    );
    out += &format!(
        "  extension      {}\n",
        if v["extension_connected"].as_bool().unwrap_or(false) {
            "connected"
        } else {
            "not connected"
        }
    );
    out += &format!("  mcp requests   {}\n", cn("mcp_requests"));
    out += &format!(
        "  tool calls     {} ({} error(s))\n",
        cn("tool_calls"),
        cn("tool_errors")
    );
    out += &format!(
        "  frames         out {} / in {}\n",
        cn("frames_out"),
        cn("frames_in")
    );
    out += &format!(
        "  connects       {} ({} disconnect(s))\n",
        cn("connects"),
        cn("disconnects")
    );

    out += "\n  in-flight:\n";
    match v["in_flight"].as_array() {
        Some(a) if !a.is_empty() => {
            for f in a {
                let since = f
                    .get("since_ms")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u128;
                out += &format!(
                    "    #{} {} ({})\n",
                    f["id"].as_str().unwrap_or("?"),
                    f["tool"].as_str().unwrap_or("?"),
                    fmt_ms(now.saturating_sub(since))
                );
            }
        }
        _ => out += "    (none)\n",
    }

    out += "\n  recent:\n";
    match v["recent"].as_array() {
        Some(a) if !a.is_empty() => {
            for e in a.iter().rev().take(12) {
                let ts = e
                    .get("ts_ms")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u128;
                out += &format!(
                    "    {:>6} ago  {:<4} {:<3} {}\n",
                    fmt_ms(now.saturating_sub(ts)),
                    e["kind"].as_str().unwrap_or("?"),
                    e["dir"].as_str().unwrap_or("-"),
                    e["summary"].as_str().unwrap_or("")
                );
            }
        }
        _ => out += "    (none)\n",
    }

    if candidates.len() > 1 {
        out += &format!(
            "\n  ({} debug sessions present; showing the most recently updated)\n",
            candidates.len()
        );
    }
    out
}

/// One recorded event at a process boundary.
#[derive(Clone, Serialize)]
pub struct Event {
    pub ts_ms: u128,
    /// "mcp" | "tool" | "ipc".
    pub kind: &'static str,
    /// "in" | "out" | "-".
    pub dir: &'static str,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Serialize)]
struct InFlight {
    id: String,
    tool: String,
    since_ms: u128,
}

#[derive(Default, Clone, Serialize)]
struct Counters {
    mcp_requests: u64,
    tool_calls: u64,
    tool_errors: u64,
    frames_out: u64,
    frames_in: u64,
    connects: u64,
    disconnects: u64,
}

/// The serialized live snapshot written to `debug-state-<pid>.json`.
#[derive(Serialize)]
struct Snapshot<'a> {
    pid: u32,
    /// "mcp-server" or "native-host". Absent in state files written before this field existed;
    /// consumers (see `doctor` / `status_report`) treat a missing field as `"mcp-server"`.
    role: &'static str,
    /// The MCP client's self-reported identity (from `initialize`'s `clientInfo`), if recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    client: Option<&'a str>,
    started_ms: u128,
    updated_ms: u128,
    extension_connected: bool,
    in_flight: Vec<&'a InFlight>,
    counters: &'a Counters,
    recent: Vec<&'a Event>,
}

/// Mutable inner state, guarded by the sink's mutex.
struct Inner {
    role: &'static str,
    client: Option<String>,
    started_ms: u128,
    last_state_ms: u128,
    extension_connected: bool,
    in_flight: BTreeMap<String, InFlight>,
    counters: Counters,
    recent: VecDeque<Event>,
    events_file: File,
    state_path: PathBuf,
}

impl Inner {
    /// Record an event: append it to the JSONL log, push it onto the recent ring, and rewrite the
    /// snapshot when forced or the throttle window has elapsed. Best-effort: I/O errors are swallowed
    /// (there is nowhere useful to report them from the hot path).
    fn record(&mut self, event: Event, force_state: bool) {
        if let Ok(mut line) = serde_json::to_string(&event) {
            line.push('\n');
            let _ = self.events_file.write_all(line.as_bytes());
        }
        if self.recent.len() == RECENT_CAP {
            self.recent.pop_front();
        }
        self.recent.push_back(event);
        let now = now_ms();
        if force_state || now.saturating_sub(self.last_state_ms) >= STATE_THROTTLE_MS {
            self.last_state_ms = now;
            self.write_state();
        }
    }

    /// Refresh `updated_ms` on the same throttle window `record` uses, without appending an event.
    /// Used by `frame_in`/`frame_out`: individual frames are too chatty for the event stream (see
    /// their doc comments) but `updated_ms` should still track frame traffic, not just requests.
    fn touch(&mut self) {
        let now = now_ms();
        if now.saturating_sub(self.last_state_ms) >= STATE_THROTTLE_MS {
            self.last_state_ms = now;
            self.write_state();
        }
    }

    /// Atomically rewrite the snapshot (temp sibling + rename) so a reader never sees a partial file.
    fn write_state(&self) {
        let snapshot = Snapshot {
            pid: std::process::id(),
            role: self.role,
            client: self.client.as_deref(),
            started_ms: self.started_ms,
            updated_ms: now_ms(),
            extension_connected: self.extension_connected,
            in_flight: self.in_flight.values().collect(),
            counters: &self.counters,
            recent: self.recent.iter().collect(),
        };
        let Ok(json) = serde_json::to_string_pretty(&snapshot) else {
            return;
        };
        let tmp = {
            let mut n = self.state_path.as_os_str().to_owned();
            n.push(".tmp");
            PathBuf::from(n)
        };
        if std::fs::write(&tmp, json.as_bytes()).is_ok() {
            let _ = std::fs::rename(&tmp, &self.state_path);
        }
    }
}

/// A cloneable observability sink. `None` inner == disabled == every method is a no-op.
#[derive(Clone)]
pub struct DebugSink {
    inner: Option<Arc<Mutex<Inner>>>,
}

impl DebugSink {
    /// A no-op sink (debug mode off).
    pub fn disabled() -> Self {
        Self { inner: None }
    }

    /// An enabled sink writing `debug-state-<pid>.json` and `debug-events-<pid>.jsonl` under `dir`
    /// (created if needed). Per-PID names avoid clobbering when two servers run with `--debug`.
    /// `role` ("mcp-server" or "native-host") is recorded in every snapshot so a reader (`status`,
    /// `doctor`) can tell which process wrote a given state file.
    pub fn enabled(dir: &Path, role: &'static str) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        cleanup_stale(dir);
        let pid = std::process::id();
        let events_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(dir.join(format!("debug-events-{pid}.jsonl")))?;
        let now = now_ms();
        let inner = Inner {
            role,
            client: None,
            started_ms: now,
            last_state_ms: now,
            extension_connected: false,
            in_flight: BTreeMap::new(),
            counters: Counters::default(),
            recent: VecDeque::new(),
            events_file,
            state_path: dir.join(format!("debug-state-{pid}.json")),
        };
        let sink = Self {
            inner: Some(Arc::new(Mutex::new(inner))),
        };
        // Initial snapshot so `status` works before any traffic.
        sink.with(|i| i.write_state());
        Ok(sink)
    }

    /// True when debug mode is on.
    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Run `f` under the lock. The lock is **poison-recovering**: a prior panic while holding it must
    /// not turn every later debug call into a crash (the whole point is to never disturb the server).
    fn with(&self, f: impl FnOnce(&mut Inner)) {
        if let Some(arc) = &self.inner {
            let mut guard = arc.lock().unwrap_or_else(|poison| poison.into_inner());
            f(&mut guard);
        }
    }

    /// Force the live snapshot to disk now (used on shutdown and in tests, bypassing the throttle).
    pub fn flush(&self) {
        self.with(|i| {
            i.last_state_ms = now_ms();
            i.write_state();
        });
    }

    /// Truncate a body for the `detail` field (avoids logging whole screenshots), on a char boundary.
    fn clip(body: &str) -> String {
        if body.len() <= DETAIL_MAX {
            body.to_string()
        } else {
            format!(
                "{}... ({} bytes total)",
                boundary_clip(body, DETAIL_MAX),
                body.len()
            )
        }
    }

    /// Clip a client-controlled identifier used in a summary (bounds `debug-state.json` growth).
    fn ident(s: &str) -> String {
        if s.len() <= IDENT_MAX {
            s.to_string()
        } else {
            format!("{}...", boundary_clip(s, IDENT_MAX))
        }
    }

    /// Record an incoming MCP JSON-RPC request.
    pub fn mcp_request(&self, method: &str, id: &str, body: &str) {
        self.with(|i| {
            i.counters.mcp_requests += 1;
            i.record(
                Event {
                    ts_ms: now_ms(),
                    kind: "mcp",
                    dir: "in",
                    summary: format!("{} (id={})", Self::ident(method), Self::ident(id)),
                    detail: Some(Self::clip(body)),
                },
                false,
            );
        });
    }

    /// Record an outgoing MCP JSON-RPC response.
    pub fn mcp_response(&self, id: &str, body: &str) {
        self.with(|i| {
            i.record(
                Event {
                    ts_ms: now_ms(),
                    kind: "mcp",
                    dir: "out",
                    summary: format!("response (id={})", Self::ident(id)),
                    detail: Some(Self::clip(body)),
                },
                false,
            );
        });
    }

    /// Record the start of a tool call (adds it to the in-flight set).
    pub fn tool_begin(&self, id: &str, tool: &str) {
        self.with(|i| {
            i.counters.tool_calls += 1;
            i.in_flight.insert(
                id.to_string(),
                InFlight {
                    id: Self::ident(id),
                    tool: Self::ident(tool),
                    since_ms: now_ms(),
                },
            );
            i.record(
                Event {
                    ts_ms: now_ms(),
                    kind: "tool",
                    dir: "in",
                    summary: format!("{} #{}", Self::ident(tool), Self::ident(id)),
                    detail: None,
                },
                false,
            );
        });
    }

    /// Record the end of a tool call (removes it from in-flight; `ok=false` counts a tool error).
    pub fn tool_end(&self, id: &str, ok: bool, detail: &str) {
        self.with(|i| {
            let tool = i.in_flight.remove(id).map(|f| f.tool).unwrap_or_default();
            if !ok {
                i.counters.tool_errors += 1;
            }
            i.record(
                Event {
                    ts_ms: now_ms(),
                    kind: "tool",
                    dir: "out",
                    summary: format!(
                        "{tool} #{} -> {}",
                        Self::ident(id),
                        if ok { "ok" } else { "error" }
                    ),
                    detail: Some(Self::clip(detail)),
                },
                false,
            );
        });
    }

    /// Record a frame sent to the extension (counter only; not an event -- too chatty). Refreshes
    /// `updated_ms` (throttled, see `Inner::touch`) so an idle-but-relaying session does not look
    /// stale in `status`/`doctor` just because no MCP request has arrived recently.
    pub fn frame_out(&self) {
        self.with(|i| {
            i.counters.frames_out += 1;
            i.touch();
        });
    }

    /// Record a frame received from the extension (counter only; see `frame_out`).
    pub fn frame_in(&self) {
        self.with(|i| {
            i.counters.frames_in += 1;
            i.touch();
        });
    }

    /// Record an extension connect / disconnect transition (forces a snapshot: the flag must be
    /// immediately accurate for `status`).
    pub fn set_connected(&self, connected: bool) {
        self.with(|i| {
            i.extension_connected = connected;
            if connected {
                i.counters.connects += 1;
            } else {
                i.counters.disconnects += 1;
                i.in_flight.clear();
            }
            i.record(
                Event {
                    ts_ms: now_ms(),
                    kind: "ipc",
                    dir: "-",
                    summary: format!(
                        "extension {}",
                        if connected {
                            "connected"
                        } else {
                            "disconnected"
                        }
                    ),
                    detail: None,
                },
                true,
            );
        });
    }

    /// Record the MCP client's self-reported identity (from the `initialize` params' `clientInfo`).
    /// Forces a snapshot so `doctor`/`status` see it immediately.
    pub fn set_client(&self, client: &str) {
        self.with(|i| {
            let clipped = Self::ident(client);
            i.client = Some(clipped.clone());
            i.record(
                Event {
                    ts_ms: now_ms(),
                    kind: "mcp",
                    dir: "-",
                    summary: format!("client {clipped}"),
                    detail: None,
                },
                true,
            );
        });
    }

    /// Record a one-line IPC lifecycle note (used by the native-host role, which has no MCP
    /// request/response boundary of its own to hang events off of). Forces a snapshot.
    pub fn ipc_note(&self, summary: &str) {
        self.with(|i| {
            i.record(
                Event {
                    ts_ms: now_ms(),
                    kind: "ipc",
                    dir: "-",
                    summary: Self::ident(summary),
                    detail: None,
                },
                true,
            );
        });
    }
}

impl Default for DebugSink {
    fn default() -> Self {
        Self::disabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ghostlight-debug-{tag}-{}", std::process::id()))
    }

    fn state_path(dir: &Path) -> PathBuf {
        dir.join(format!("debug-state-{}.json", std::process::id()))
    }
    fn events_path(dir: &Path) -> PathBuf {
        dir.join(format!("debug-events-{}.jsonl", std::process::id()))
    }
    fn read_snap(dir: &Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(state_path(dir)).unwrap()).unwrap()
    }

    #[test]
    fn disabled_sink_is_inert() {
        let sink = DebugSink::disabled();
        assert!(!sink.is_enabled());
        // None of these panic or write anything.
        sink.mcp_request("initialize", "1", "{}");
        sink.tool_begin("1", "navigate");
        sink.tool_end("1", true, "{}");
        sink.set_connected(true);
        sink.flush();
    }

    #[test]
    fn clip_truncates_on_a_char_boundary_without_panicking() {
        assert_eq!(DebugSink::clip("short"), "short");
        // 'a' + 400 two-byte chars = 801 bytes; the DETAIL_MAX offset lands mid-char, which a naive
        // byte slice would panic on. clip must step back to a boundary.
        let body = format!("a{}", "\u{e9}".repeat(400));
        let clipped = DebugSink::clip(&body);
        assert!(clipped.contains("bytes total"));
        assert!(clipped.len() < body.len());
    }

    #[test]
    fn ident_clips_oversized_identifiers() {
        let short = DebugSink::ident("navigate_mcp");
        assert_eq!(short, "navigate_mcp");
        let huge = DebugSink::ident(&"x".repeat(10_000));
        assert!(huge.ends_with("..."));
        assert!(huge.len() < 200);
    }

    #[test]
    fn enabled_sink_tracks_state_and_writes_files() {
        let dir = temp_dir("state");
        let sink = DebugSink::enabled(&dir, "mcp-server").unwrap();
        assert!(sink.is_enabled());
        assert!(state_path(&dir).is_file());
        assert!(events_path(&dir).is_file());

        sink.set_connected(true);
        sink.tool_begin("7", "navigate");
        sink.flush(); // throttle bypass so the snapshot reflects the in-flight call
        let snap = read_snap(&dir);
        assert_eq!(snap["extension_connected"], true);
        assert_eq!(snap["in_flight"][0]["tool"], "navigate");
        assert_eq!(snap["counters"]["tool_calls"], 1);

        sink.tool_end("7", false, "boom");
        sink.flush();
        let snap = read_snap(&dir);
        assert_eq!(snap["in_flight"].as_array().unwrap().len(), 0);
        assert_eq!(snap["counters"]["tool_errors"], 1);

        // The JSONL log has one line per recorded event.
        let lines = std::fs::read_to_string(events_path(&dir)).unwrap();
        assert!(lines.lines().count() >= 3, "connect + begin + end");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn enabled_sink_records_role_and_client() {
        let dir = temp_dir("role-client");
        let sink = DebugSink::enabled(&dir, "mcp-server").unwrap();
        sink.set_client("claude-code 1.2.3");
        sink.flush();
        let snap = read_snap(&dir);
        assert_eq!(snap["role"], "mcp-server");
        assert_eq!(snap["client"], "claude-code 1.2.3");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disconnect_clears_in_flight() {
        let dir = temp_dir("disc");
        let sink = DebugSink::enabled(&dir, "mcp-server").unwrap();
        sink.set_connected(true);
        sink.tool_begin("1", "read_page_mcp");
        sink.set_connected(false); // forces a snapshot
        let snap = read_snap(&dir);
        assert_eq!(snap["extension_connected"], false);
        assert_eq!(snap["in_flight"].as_array().unwrap().len(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
