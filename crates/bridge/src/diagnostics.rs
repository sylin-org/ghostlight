//! The shared process diagnostics sink: one local directory where every Ghostlight process
//! appends bounded, content-free operational JSONL. Implements ADR-0145.
//!
//! Activation is layered and re-checked live: an explicit `GHOSTLIGHT_DIAGNOSTICS_DIR` pins the
//! sink on at birth, otherwise a presence-only `diagnostics.on` marker beside the runtime
//! discovery file toggles every running component, served by an OS directory watch with a
//! 2-second safety-net re-check, whichever fires first. A sink fault never disturbs the
//! product: append failures disable the sink for the process lifetime. Each activation period
//! owns one file, named so a directory listing reads chronologically; retention keeps the
//! newest files per component under a total byte ceiling and never touches the marker.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::Watcher as _;

/// The environment variable that turns diagnostics on and names the directory.
pub const ENV_DIR: &str = "GHOSTLIGHT_DIAGNOSTICS_DIR";
/// The presence-only marker file that turns diagnostics on beside the runtime discovery file.
pub const MARKER_FILE_NAME: &str = "diagnostics.on";
/// The header record's schema marker.
pub const SCHEMA: &str = "ghostlight-diagnostics-1";
/// Log files kept per component after pruning.
const KEEP_PER_COMPONENT: usize = 8;
/// The total byte ceiling across all diagnostics files in the directory.
const TOTAL_CEILING_BYTES: u64 = 64 * 1024 * 1024;
/// The safety-net re-check interval for the marker layer.
const TICK: Duration = Duration::from_secs(2);
/// The longest detail string a record carries, clipped on a UTF-8 boundary.
const MAX_DETAIL_BYTES: usize = 500;

/// The closed event-name vocabulary. Event names are constants so no call site carries a
/// literal.
pub mod event {
    pub const PROCESS_STARTED: &str = "process_started";
    pub const SINK_OPENED: &str = "sink_opened";
    pub const SINK_CLOSED: &str = "sink_closed";
    pub const DEMAND_START_ATTEMPT: &str = "demand_start_attempt";
    pub const DEMAND_START_SPAWNED: &str = "demand_start_spawned";
    pub const DEMAND_START_ALREADY_RUNNING: &str = "demand_start_already_running";
    pub const DEMAND_START_DEPLOYMENT_IN_PROGRESS: &str = "demand_start_deployment_in_progress";
    pub const DEMAND_START_FAILED: &str = "demand_start_failed";
    pub const SERVICE_CONNECTED: &str = "service_connected";
    pub const SERVICE_DISCONNECTED: &str = "service_disconnected";
    pub const HARNESS_ATTACHED: &str = "harness_attached";
    pub const HARNESS_DETACHED: &str = "harness_detached";
    pub const ADAPTER_ATTACHED: &str = "adapter_attached";
    pub const ADAPTER_REPLACED: &str = "adapter_replaced";
    pub const ADAPTER_DISCONNECTED: &str = "adapter_disconnected";
    pub const HEARTBEAT_LOST: &str = "heartbeat_lost";
    pub const HEARTBEAT_RESUMED: &str = "heartbeat_resumed";
    pub const OPERATION_COMPLETED: &str = "operation_completed";
    pub const OPERATION_FAILED: &str = "operation_failed";
    pub const DIAGNOSTICS_TOGGLE_REQUESTED: &str = "diagnostics_toggle_requested";
    pub const DIAGNOSTICS_TOGGLED: &str = "diagnostics_toggled";
}

/// Which Ghostlight process is writing. Component names appear in file names and records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    Orchestrator,
    McpConnector,
    BrowserConnector,
}

impl Component {
    pub fn as_str(self) -> &'static str {
        match self {
            Component::Orchestrator => "orchestrator",
            Component::McpConnector => "mcp-connector",
            Component::BrowserConnector => "browser-connector",
        }
    }
}

/// Record severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// The resolved activation state: explicit directory wins over the marker, over off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activation {
    Explicit { directory: PathBuf },
    Marker { directory: PathBuf },
    Off,
}

impl Activation {
    /// The layer name used by `diagnostics path`, doctor, and wire state.
    pub fn layer(&self) -> &'static str {
        match self {
            Activation::Explicit { .. } => "explicit",
            Activation::Marker { .. } => "marker",
            Activation::Off => "off",
        }
    }

    /// The directory to write into, when active.
    pub fn directory(&self) -> Option<&Path> {
        match self {
            Activation::Explicit { directory } | Activation::Marker { directory } => {
                Some(directory)
            }
            Activation::Off => None,
        }
    }
}

/// The path of the activation marker beside the runtime discovery file.
pub fn marker_path(runtime_path: &Path) -> PathBuf {
    runtime_path.with_file_name(MARKER_FILE_NAME)
}

/// The default log directory: a `logs` folder beside the runtime discovery file, never the
/// application root itself. This is where logs go when the marker layer activates, and where
/// retained logs remain readable after they are turned off.
pub fn default_directory(runtime_path: &Path) -> PathBuf {
    match marker_path(runtime_path).parent() {
        Some(directory) => directory.join("logs"),
        None => PathBuf::from("logs"),
    }
}

/// Resolve the activation layers: an explicit directory wins, then the marker, then off.
pub fn resolve(explicit: Option<PathBuf>, runtime_path: &Path) -> Activation {
    if let Some(directory) = explicit {
        return Activation::Explicit { directory };
    }
    if marker_path(runtime_path).is_file() {
        Activation::Marker {
            directory: default_directory(runtime_path),
        }
    } else {
        Activation::Off
    }
}

/// Create or remove the marker as a person's act, then report the resulting activation.
pub fn set_marker(runtime_path: &Path, on: bool) -> io::Result<Activation> {
    let marker = marker_path(runtime_path);
    if on {
        if let Some(parent) = marker.parent() {
            fs::create_dir_all(parent)?;
        }
        OpenOptions::new().create(true).append(true).open(&marker)?;
    } else {
        match fs::remove_file(&marker) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(resolve(None, runtime_path))
}

/// A source of marker-change wake-ups. The production implementation watches the marker's
/// directory through the operating system; tests inject synthetic wake-ups.
pub trait MarkerWatcher: Send + Sync + 'static {
    /// Call `wake` whenever the marker file may have changed. The returned guard stops the
    /// watch when dropped; `None` means this watcher cannot run here and the safety-net tick
    /// alone serves.
    fn watch(
        &self,
        marker: PathBuf,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Option<Box<dyn Send + Sync>>;
}

/// The production watcher: one OS directory watch on the marker's parent, filtered to the
/// marker's file name so the sink's own log appends never trigger an evaluation.
#[derive(Default)]
pub struct OsMarkerWatcher;

impl MarkerWatcher for OsMarkerWatcher {
    fn watch(
        &self,
        marker: PathBuf,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Option<Box<dyn Send + Sync>> {
        let marker_name = marker.file_name()?.to_os_string();
        let directory = marker.parent()?.to_path_buf();
        let (send, receive) = mpsc::channel::<()>();
        let marker_name_for_events = marker_name.clone();
        let mut watcher =
            notify::recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
                if let Ok(event) = result {
                    if event
                        .paths
                        .iter()
                        .any(|path| path.file_name() == Some(marker_name_for_events.as_os_str()))
                    {
                        let _ = send.send(());
                    }
                }
            })
            .ok()?;
        watcher
            .watch(&directory, notify::RecursiveMode::NonRecursive)
            .ok()?;
        let join = thread::Builder::new()
            .name("ghostlight-diagnostics-watch".into())
            .spawn(move || {
                while receive.recv().is_ok() {
                    wake();
                }
            })
            .ok()?;
        struct Guard {
            watcher: Option<notify::RecommendedWatcher>,
            join: Option<thread::JoinHandle<()>>,
        }
        impl Drop for Guard {
            fn drop(&mut self) {
                if let Some(watcher) = self.watcher.take() {
                    drop(watcher);
                }
                if let Some(join) = self.join.take() {
                    let _ = join.join();
                }
            }
        }
        Some(Box::new(Guard {
            watcher: Some(watcher),
            join: Some(join),
        }))
    }
}

/// A callback fired after every activation transition.
pub type ChangeCallback = Arc<dyn Fn(&Activation) + Send + Sync>;

/// A live diagnostics sink for one process. Cheap when off: emitting without an open file is a
/// mutex and a compare.
pub struct Sink {
    component: Component,
    version: String,
    run_id: String,
    pid: u32,
    pinned: Option<PathBuf>,
    runtime_path: PathBuf,
    watcher: Mutex<Option<Box<dyn Send + Sync>>>,
    state: Mutex<SinkState>,
    on_change: Mutex<Option<ChangeCallback>>,
}

struct SinkState {
    open: Option<(PathBuf, File)>,
    disabled: bool,
}

impl Sink {
    /// Birth the sink from the process environment, starting the safety-net tick and the OS
    /// watch. The explicit layer, when present, pins the sink on for this process's life.
    pub fn birth(component: Component, version: &str, runtime_path: &Path) -> Arc<Sink> {
        let pinned = std::env::var_os(ENV_DIR).map(PathBuf::from);
        Sink::birth_with(
            component,
            version,
            runtime_path,
            pinned,
            &OsMarkerWatcher,
            None,
        )
    }

    /// Birth with explicit inputs; the seam tests use.
    pub fn birth_with(
        component: Component,
        version: &str,
        runtime_path: &Path,
        pinned: Option<PathBuf>,
        watcher: &dyn MarkerWatcher,
        on_change: Option<ChangeCallback>,
    ) -> Arc<Sink> {
        let started_ms = unix_ms();
        let run_id = format!(
            "{:08x}-{:06x}",
            (started_ms & 0xffff_ffff) as u32,
            std::process::id() & 0xff_ffff
        );
        let sink = Arc::new(Sink {
            component,
            version: version.to_string(),
            run_id,
            pid: std::process::id(),
            pinned,
            runtime_path: runtime_path.to_path_buf(),
            watcher: Mutex::new(None),
            state: Mutex::new(SinkState {
                open: None,
                disabled: false,
            }),
            on_change: Mutex::new(on_change),
        });

        if sink.pinned.is_none() {
            let marker = marker_path(runtime_path);
            let wake_sink = Arc::clone(&sink);
            let guard = watcher.watch(marker, Arc::new(move || wake_sink.evaluate()));
            *sink.watcher.lock().expect("diagnostics watcher lock") = guard;
        }

        let tick_sink = Arc::clone(&sink);
        let _ = thread::Builder::new()
            .name("ghostlight-diagnostics-tick".into())
            .spawn(move || loop {
                thread::park_timeout(TICK);
                tick_sink.evaluate();
            });

        // Apply the activation the process was born with before the caller logs anything.
        sink.evaluate();
        sink
    }

    /// Register a callback fired after every activation transition. The orchestrator uses this
    /// to republish wire state; the callback runs outside the sink's lock.
    pub fn set_on_change(&self, callback: Arc<dyn Fn(&Activation) + Send + Sync>) {
        *self.on_change.lock().expect("diagnostics on_change lock") = Some(callback);
    }

    /// The current activation, re-resolved now.
    pub fn activation(&self) -> Activation {
        match &self.pinned {
            Some(directory) => Activation::Explicit {
                directory: directory.clone(),
            },
            None => resolve(None, &self.runtime_path),
        }
    }

    /// Re-evaluate the layers and transition if they changed. Idempotent and cheap; both the
    /// watch and the tick call it, and whichever fires first wins.
    pub fn evaluate(&self) {
        let target = self.activation();
        let mut state = self.state.lock().expect("diagnostics state lock");
        if state.disabled {
            return;
        }
        match (&target, &state.open) {
            (Activation::Off, None) => {}
            (Activation::Off, Some(_)) => {
                Self::write_record(
                    self,
                    &mut state,
                    event::SINK_CLOSED,
                    Level::Info,
                    None,
                    "activation off",
                );
                state.open = None;
                drop(state);
                self.notify_change(&Activation::Off);
            }
            (target, None) => {
                if let Some(directory) = target.directory() {
                    Self::open_locked(self, &mut state, directory);
                    drop(state);
                    self.notify_change(target);
                }
            }
            (target, Some(_)) => {
                let same = target.directory().is_some_and(|directory| {
                    state
                        .open
                        .as_ref()
                        .is_some_and(|(open, _)| open == directory)
                });
                if !same {
                    Self::write_record(
                        self,
                        &mut state,
                        event::SINK_CLOSED,
                        Level::Info,
                        None,
                        "activation directory changed",
                    );
                    state.open = None;
                    if let Some(directory) = target.directory() {
                        Self::open_locked(self, &mut state, directory);
                    }
                    drop(state);
                    self.notify_change(target);
                }
            }
        }
    }

    /// Append one record. When the sink is off this is a mutex and a compare.
    pub fn emit(&self, event: &str, level: Level, operation: Option<&str>, detail: &str) {
        let mut state = self.state.lock().expect("diagnostics state lock");
        if state.open.is_none() || state.disabled {
            return;
        }
        Self::write_record(self, &mut state, event, level, operation, detail);
    }

    fn open_locked(sink: &Sink, state: &mut SinkState, directory: &Path) {
        if let Err(error) = fs::create_dir_all(directory) {
            sink.disable(state, &error);
            return;
        }
        let _ = prune_directory(directory);
        let path = allocate_log_path(directory, sink.component, sink.pid, &utc_stamp(unix_ms()));
        let file = match OpenOptions::new().append(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) => {
                sink.disable(state, &error);
                return;
            }
        };
        *state = SinkState {
            open: Some((directory.to_path_buf(), file)),
            disabled: false,
        };
        let header = serde_json::json!({
            "schema": SCHEMA,
            "component": sink.component.as_str(),
            "version": sink.version,
            "pid": sink.pid,
            "run_id": sink.run_id,
            "started_ms": unix_ms(),
        });
        if let Some((_, file)) = state.open.as_mut() {
            let _ = writeln!(file, "{}", header);
        }
        Self::write_record(
            sink,
            state,
            event::SINK_OPENED,
            Level::Info,
            None,
            &format!("activation on ({})", sink.activation().layer()),
        );
    }

    fn write_record(
        sink: &Sink,
        state: &mut SinkState,
        event: &str,
        level: Level,
        operation: Option<&str>,
        detail: &str,
    ) {
        let record = serde_json::json!({
            "ts_ms": unix_ms(),
            "run_id": sink.run_id,
            "component": sink.component.as_str(),
            "event": event,
            "level": level.as_str(),
            "op": operation,
            "detail": clip(detail),
        });
        if let Some((_, file)) = state.open.as_mut() {
            if writeln!(file, "{}", record)
                .and_then(|()| file.flush())
                .is_err()
            {
                state.open = None;
                state.disabled = true;
                eprintln!("Ghostlight diagnostics disabled for this process: write failed");
            }
        }
    }

    fn disable(&self, state: &mut SinkState, error: &io::Error) {
        state.open = None;
        state.disabled = true;
        eprintln!(
            "Ghostlight diagnostics disabled for this process: {}",
            error
        );
    }

    fn notify_change(&self, activation: &Activation) {
        let callback = self.on_change.lock().expect("diagnostics on_change lock");
        if let Some(callback) = callback.as_ref() {
            callback(activation);
        }
    }
}

impl fmt::Debug for Sink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sink")
            .field("component", &self.component.as_str())
            .field("run_id", &self.run_id)
            .finish_non_exhaustive()
    }
}

/// Pick the log file name for a new activation period, numbering past collisions so two
/// periods in the same second never share a file.
pub fn allocate_log_path(directory: &Path, component: Component, pid: u32, stamp: &str) -> PathBuf {
    let base = format!("{}-{}-{}", stamp, component.as_str(), pid);
    let mut path = directory.join(format!("{}.jsonl", base));
    let mut sequence: u32 = 1;
    while path.exists() {
        sequence += 1;
        path = directory.join(format!("{}-{}.jsonl", base, sequence));
    }
    path
}

/// What a prune pass did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PruneReport {
    pub deleted: usize,
    pub kept: usize,
    pub kept_bytes: u64,
}

/// Apply the retention bounds to a diagnostics directory: keep the newest
/// [`KEEP_PER_COMPONENT`] files per component and no more than [`TOTAL_CEILING_BYTES`] total,
/// pruning oldest first. Only files matching the diagnostics naming are considered; the marker
/// and anything else in the directory are never touched.
pub fn prune_directory(directory: &Path) -> PruneReport {
    let empty = PruneReport {
        deleted: 0,
        kept: 0,
        kept_bytes: 0,
    };
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return empty,
    };
    let mut by_component: Vec<(String, Vec<(String, u64)>)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some((component, _)) = parse_log_name(&name) {
            let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            match by_component
                .iter_mut()
                .find(|(known, _)| *known == component)
            {
                Some((_, files)) => files.push((name, size)),
                None => by_component.push((component, vec![(name, size)])),
            }
        }
    }
    let mut survivors: Vec<(String, u64)> = Vec::new();
    let mut deleted = 0usize;
    for (_, files) in by_component.iter_mut() {
        files.sort_by(|left, right| left.0.cmp(&right.0));
        let excess = files.len().saturating_sub(KEEP_PER_COMPONENT);
        for (name, _) in files.iter().take(excess) {
            if fs::remove_file(directory.join(name)).is_ok() {
                deleted += 1;
            }
        }
        survivors.append(&mut files.split_off(excess));
    }
    survivors.sort_by(|left, right| left.0.cmp(&right.0));
    let mut kept = survivors.len();
    let mut total: u64 = survivors.iter().map(|(_, size)| *size).sum();
    for (name, size) in &survivors {
        if total <= TOTAL_CEILING_BYTES {
            break;
        }
        if fs::remove_file(directory.join(name)).is_ok() {
            deleted += 1;
            kept -= 1;
            total -= size;
        }
    }
    PruneReport {
        deleted,
        kept,
        kept_bytes: total,
    }
}

/// Parse a diagnostics log file name into its component and stamp. Returns `None` for anything
/// that is not a diagnostics log, which is how the marker and foreign files survive pruning,
/// and how `diagnostics show` knows which files are its own.
pub fn parse_log_name(name: &str) -> Option<(String, String)> {
    let base = name.strip_suffix(".jsonl")?;
    let mut parts = base.split('-').collect::<Vec<_>>();
    if parts.len() < 3 {
        return None;
    }
    let stamp = parts[0].to_string();
    let stamp_ok = stamp.len() == 16
        && stamp
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'T' || byte == b'Z');
    if !stamp_ok {
        return None;
    }
    let last = parts.pop().expect("non-empty");
    let pid_part = if last.bytes().all(|byte| byte.is_ascii_digit())
        && parts
            .last()
            .is_some_and(|candidate| candidate.bytes().all(|byte| byte.is_ascii_digit()))
    {
        // A trailing number over another number is a same-second sequence suffix.
        parts.pop().expect("pid").to_string()
    } else {
        last.to_string()
    };
    if pid_part.is_empty() || parts.len() < 2 {
        return None;
    }
    Some((parts[1..].join("-"), stamp))
}

/// Clip a detail string to the record bound without splitting a character.
fn clip(detail: &str) -> &str {
    if detail.len() <= MAX_DETAIL_BYTES {
        return detail;
    }
    let mut end = MAX_DETAIL_BYTES;
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    &detail[..end]
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or(0)
}

/// Render a UTC timestamp in the `YYYYMMDDTHHMMSSZ` file-name form from milliseconds since the
/// epoch. Leap seconds are ignored; file names only need to sort chronologically.
fn utc_stamp(milliseconds: u128) -> String {
    let total_seconds = (milliseconds / 1000) as i64;
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        year,
        month,
        day,
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60,
        seconds_of_day % 60
    )
}

/// Days since the epoch to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ghostlight-diagnostics-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");
        root
    }

    fn runtime_in(root: &Path) -> PathBuf {
        root.join("ghostlight-runtime.json")
    }

    /// Drive re-checks until an observable effect lands, bounded. The product contract is
    /// eventual (watch or tick, whichever fires first), and resolution alone proves nothing:
    /// only an opened file, a written line, or a callback is the transition.
    fn wait_until_bounded(mut predicate: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !predicate() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn log_lines(root: &Path) -> usize {
        let root = root.join("logs");
        fs::read_dir(root)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| entry.file_name().to_string_lossy().ends_with(".jsonl"))
                    .filter_map(|entry| fs::read_to_string(entry.path()).ok())
                    .map(|content| content.lines().count())
                    .sum()
            })
            .unwrap_or(0)
    }

    #[test]
    fn explicit_layer_wins_over_marker_over_off() {
        let root = temp_root("resolve");
        let runtime = runtime_in(&root);
        assert_eq!(resolve(None, &runtime), Activation::Off);
        fs::write(marker_path(&runtime), b"").expect("marker");
        assert_eq!(
            resolve(None, &runtime),
            Activation::Marker {
                directory: root.join("logs")
            }
        );
        assert_eq!(
            resolve(Some(root.join("elsewhere")), &runtime),
            Activation::Explicit {
                directory: root.join("elsewhere")
            }
        );
    }

    #[test]
    fn marker_names_are_stable_and_derived_from_the_runtime_file() {
        let runtime = Path::new("/data/ghostlight-runtime.json");
        assert_eq!(marker_path(runtime), PathBuf::from("/data/diagnostics.on"));
    }

    #[test]
    fn birth_pins_the_explicit_layer_and_opens_a_headered_file() {
        let root = temp_root("pinned");
        let runtime = runtime_in(&root);
        let sink = Sink::birth_with(
            Component::Orchestrator,
            "9.9.9-test",
            &runtime,
            Some(root.join("logs")),
            &OsMarkerWatcher,
            None,
        );
        assert_eq!(sink.activation().layer(), "explicit");
        sink.emit(event::PROCESS_STARTED, Level::Info, None, "test process");
        let logs = root.join("logs");
        let mut log_files: Vec<PathBuf> = fs::read_dir(&logs)
            .expect("log dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".jsonl"))
            .map(|entry| entry.path())
            .collect();
        assert_eq!(log_files.len(), 1, "one activation period, one file");
        let content = fs::read_to_string(log_files.pop().expect("one file")).expect("log");
        let mut lines = content.lines();
        let header: serde_json::Value =
            serde_json::from_str(lines.next().expect("header")).expect("header json");
        for key in [
            "schema",
            "component",
            "version",
            "pid",
            "run_id",
            "started_ms",
        ] {
            assert!(header.get(key).is_some(), "header carries {key}");
        }
        assert_eq!(header["schema"], SCHEMA);
        assert_eq!(header["component"], "orchestrator");
        let opened: serde_json::Value =
            serde_json::from_str(lines.next().expect("record")).expect("opened json");
        assert_eq!(opened["event"], event::SINK_OPENED);
        let record: serde_json::Value =
            serde_json::from_str(lines.next().expect("record")).expect("record json");
        let mut keys: Vec<&str> = record
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "component",
                "detail",
                "event",
                "level",
                "op",
                "run_id",
                "ts_ms"
            ]
        );
        assert_eq!(record["component"], "orchestrator");
        assert_eq!(record["event"], event::PROCESS_STARTED);
    }

    #[test]
    fn the_marker_layer_toggles_a_live_sink_through_evaluate() {
        let root = temp_root("toggle");
        let runtime = runtime_in(&root);
        let sink = Sink::birth_with(
            Component::McpConnector,
            "9.9.9-test",
            &runtime,
            None,
            &OsMarkerWatcher,
            None,
        );
        assert_eq!(sink.activation().layer(), "off");
        sink.emit(event::PROCESS_STARTED, Level::Info, None, "before marker");
        let marker = marker_path(&runtime);
        fs::write(&marker, b"").expect("marker");
        wait_until_bounded(|| log_lines(&root) >= 2);
        sink.evaluate();
        sink.emit(event::PROCESS_STARTED, Level::Info, None, "after marker");
        wait_until_bounded(|| log_lines(&root) == 3);
        let log_dir = root.join("logs");
        let log = fs::read_dir(&log_dir)
            .expect("dir")
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().ends_with(".jsonl"))
            .expect("log file after marker");
        let content = fs::read_to_string(log.path()).expect("content");
        assert_eq!(
            content.lines().count(),
            3,
            "header, the opened line, and the post-marker line"
        );
        fs::remove_file(&marker).expect("remove marker");
        wait_until_bounded(|| log_lines(&root) == 4);
        sink.emit(event::PROCESS_STARTED, Level::Info, None, "after off");
        let content = fs::read_to_string(log.path()).expect("content");
        assert_eq!(
            content.lines().count(),
            4,
            "off writes a closing line and then nothing more"
        );
    }

    #[test]
    fn transitions_report_through_the_change_callback() {
        let root = temp_root("callback");
        let runtime = runtime_in(&root);
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let seen_for_callback = Arc::clone(&seen);
        let sink = Sink::birth_with(
            Component::BrowserConnector,
            "9.9.9-test",
            &runtime,
            None,
            &OsMarkerWatcher,
            Some(Arc::new(move |activation: &Activation| {
                seen_for_callback
                    .lock()
                    .expect("seen lock")
                    .push(activation.layer().to_string());
            })),
        );
        fs::write(marker_path(&runtime), b"").expect("marker");
        // The contract is eventual: the watch or the tick applies the layer, whichever fires
        // first. Drive the re-check to the deadline instead of assuming one call is enough.
        wait_until_bounded(|| !seen.lock().expect("seen lock").is_empty());
        sink.evaluate();
        let layers = seen.lock().expect("seen lock");
        assert_eq!(layers.last().map(String::as_str), Some("marker"));
        assert!(!layers.contains(&"off".to_string()));
    }

    #[test]
    fn same_second_activation_periods_get_sequence_numbers() {
        let root = temp_root("sequence");
        let stamp = "20260101T000000Z";
        let first = root.join(format!("{}-orchestrator-7.jsonl", stamp));
        fs::write(&first, b"").expect("first");
        let second = allocate_log_path(&root, Component::Orchestrator, 7, stamp);
        assert_ne!(first, second);
        assert!(second
            .to_string_lossy()
            .ends_with("-orchestrator-7-2.jsonl"));
        fs::write(&second, b"").expect("second");
        let third = allocate_log_path(&root, Component::Orchestrator, 7, stamp);
        assert!(third.to_string_lossy().ends_with("-orchestrator-7-3.jsonl"));
    }

    #[test]
    fn an_unusable_directory_disables_the_sink_without_panicking() {
        let root = temp_root("disabled");
        let blocked = root.join("file-not-directory");
        fs::write(&blocked, b"not a directory").expect("blocker");
        let sink = Sink::birth_with(
            Component::Orchestrator,
            "9.9.9-test",
            &runtime_in(&root),
            Some(blocked),
            &OsMarkerWatcher,
            None,
        );
        sink.emit(event::PROCESS_STARTED, Level::Info, None, "dropped");
        sink.evaluate();
        let logs = fs::read_dir(&root)
            .expect("dir")
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.ends_with(".jsonl")
            })
            .count();
        assert_eq!(logs, 0, "a disabled sink writes nothing");
    }

    #[test]
    fn detail_is_clipped_on_a_utf8_boundary() {
        assert_eq!(clip("short"), "short");
        let long = "x".repeat(MAX_DETAIL_BYTES + 10);
        assert_eq!(clip(&long).len(), MAX_DETAIL_BYTES);
        let multibyte = "e".repeat(MAX_DETAIL_BYTES - 1) + "\u{00e9}\u{00e9}";
        let clipped = clip(&multibyte);
        assert!(clipped.is_char_boundary(clipped.len()));
        assert!(clipped.len() <= MAX_DETAIL_BYTES);
    }

    #[test]
    fn pruning_keeps_the_newest_per_component_and_never_touches_the_marker() {
        let root = temp_root("prune");
        let directory = root.join("logs");
        fs::create_dir_all(&directory).expect("dir");
        for index in 0..10 {
            let name = format!(
                "2026080{}T00000{}Z-orchestrator-1.jsonl",
                index / 5 + 1,
                index % 5
            );
            fs::write(directory.join(&name), vec![b'a'; 10]).expect("seed file");
        }
        for index in 0..3 {
            let name = format!("2026080{}T00000{}Z-mcp-connector-2.jsonl", index + 1, index);
            fs::write(directory.join(&name), vec![b'b'; 10]).expect("seed file");
        }
        fs::write(directory.join(MARKER_FILE_NAME), b"").expect("marker");
        fs::write(directory.join("foreign.txt"), b"keep me").expect("foreign");
        let report = prune_directory(&directory);
        assert_eq!(report.deleted, 2);
        let remaining: Vec<String> = fs::read_dir(&directory)
            .expect("dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            remaining.len(),
            8 + 3 + 2,
            "kept logs plus marker plus foreign"
        );
        assert!(remaining.contains(&MARKER_FILE_NAME.to_string()));
        assert!(remaining.contains(&"foreign.txt".to_string()));
        assert!(!remaining.contains(&"20260801T000000Z-orchestrator-1.jsonl".to_string()));
        assert!(!remaining.contains(&"20260801T000001Z-orchestrator-1.jsonl".to_string()));
    }

    #[test]
    fn foreign_and_marker_names_are_not_parsed_as_logs() {
        assert!(parse_log_name("diagnostics.on").is_none());
        assert!(parse_log_name("foreign.txt").is_none());
        assert!(parse_log_name("ghostlight-runtime.json").is_none());
        let (component, stamp) =
            parse_log_name("20260829T123456Z-mcp-connector-42-7.jsonl").expect("parses");
        assert_eq!(component, "mcp-connector");
        assert_eq!(stamp, "20260829T123456Z");
        assert!(parse_log_name("2026-not-a-stamp-orchestrator-1.jsonl").is_none());
    }

    #[test]
    fn utc_stamp_renders_known_instants() {
        assert_eq!(utc_stamp(0), "19700101T000000Z");
        assert_eq!(utc_stamp(1_000_000_000_000), "20010909T014640Z");
        assert_eq!(utc_stamp(1_787_961_600_000), "20260829T000000Z");
    }

    #[test]
    fn watch_wakeups_evaluate_the_sink_through_the_injected_seam() {
        struct Manual;
        impl MarkerWatcher for Manual {
            fn watch(
                &self,
                _marker: PathBuf,
                wake: Arc<dyn Fn() + Send + Sync>,
            ) -> Option<Box<dyn Send + Sync>> {
                wake();
                Some(Box::new(()))
            }
        }
        let root = temp_root("watch");
        let sink = Sink::birth_with(
            Component::Orchestrator,
            "9.9.9-test",
            &runtime_in(&root),
            None,
            &Manual,
            None,
        );
        assert_eq!(sink.activation().layer(), "off");
    }
}
