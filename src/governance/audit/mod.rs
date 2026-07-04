// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! The audit flight recorder (ADR-0018 step 1). Records are written by the binary only; the
//! extension never logs (SPEC section 7 trust boundary). One record per tool call, plus one
//! session-EVENT record per session event (g11: the panic kill switch); write failures never
//! break tool calls but are reported via `tracing`.
//!
//! [`Recorder`] is the concrete [`crate::governance::ports::AuditSink`] impl held by the
//! [`crate::governance::dispatch::Governance`] facade. Its destination (file, stderr, or
//! disabled) resolves from the `audit.enabled` / `audit.destination` / `audit.file.path`
//! config keys (G01). Per RECONCILIATION.md section 3, this is now live, not
//! resolve-once-at-startup: [`Recorder::reload`] re-resolves the destination from a fresh
//! config snapshot and swaps it in, so a hot-reloaded config change re-opens the sink (closing
//! the old destination, opening the new one) with no restart.

pub mod destinations;

use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use crate::governance::config::Config;
use crate::governance::ports::{AuditRecord, AuditSink, SessionEventRecord};

/// Where a `Recorder`'s lines currently go, or `None` when audit is disabled. Disabled
/// creates no file and holds no path.
enum Inner {
    File(PathBuf),
    Stderr,
    Syslog(SocketAddr),
}

/// The audit flight recorder. Cheap to share by reference from the server loop. A disabled
/// recorder does nothing (and creates no file).
pub struct Recorder {
    inner: Mutex<Option<Inner>>,
}

/// Resolve the destination `Recorder::from_config` and `Recorder::reload` both need, from a
/// config snapshot. Shared so the two never drift: reload must re-derive the exact same
/// destination a fresh construction would.
fn resolve_inner(config: &Config) -> Option<Inner> {
    if !config.audit_enabled() {
        return None;
    }
    match config.audit_destination() {
        "stderr" => Some(Inner::Stderr),
        "none" => None,
        "syslog" => match config.audit_syslog_address().to_socket_addrs() {
            Ok(mut addrs) => match addrs.next() {
                Some(addr) => Some(Inner::Syslog(addr)),
                None => {
                    tracing::warn!(
                        address = config.audit_syslog_address(),
                        "syslog address resolved to no addresses; audit disabled"
                    );
                    None
                }
            },
            Err(e) => {
                tracing::warn!(
                    address = config.audit_syslog_address(),
                    error = %e,
                    "invalid syslog address; audit disabled"
                );
                None
            }
        },
        _ => {
            let path = if !config.audit_file_path().is_empty() {
                Some(PathBuf::from(config.audit_file_path()))
            } else {
                destinations::default_audit_path()
            };
            match path {
                Some(p) => Some(Inner::File(p)),
                None => {
                    tracing::warn!("no data directory available; audit recording disabled");
                    None
                }
            }
        }
    }
}

impl Recorder {
    /// Build the recorder from the initial resolved config. Called once at mcp-server
    /// startup; [`Self::reload`] re-resolves it on every subsequent config change.
    pub fn from_config(config: &Config) -> Self {
        Self {
            inner: Mutex::new(resolve_inner(config)),
        }
    }

    /// A recorder that writes nothing (audit disabled).
    pub fn disabled() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// A recorder that appends JSON lines to `path`.
    pub fn to_file(path: PathBuf) -> Self {
        Self {
            inner: Mutex::new(Some(Inner::File(path))),
        }
    }

    /// A recorder that writes JSON lines to stderr.
    pub fn to_stderr() -> Self {
        Self {
            inner: Mutex::new(Some(Inner::Stderr)),
        }
    }

    /// True while this recorder has a live destination (audit enabled).
    pub fn is_enabled(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// Re-resolve the destination from a fresh config snapshot and swap it in. Called by the
    /// mcp-server's config-change watcher (A5's `ConfigStore::subscribe`) so an
    /// `audit.enabled` / `audit.destination` / `audit.file.path` edit takes effect with no
    /// restart (RECONCILIATION.md section 3): the old destination is simply replaced, which is
    /// "close old, open new" for a file sink (no handle is held open between records; see
    /// `destinations::append_line_to_file`'s one-open-per-record design).
    pub fn reload(&self, config: &Config) {
        *self.inner.lock().unwrap_or_else(PoisonError::into_inner) = resolve_inner(config);
    }
}

impl Recorder {
    /// Serialize and append one line to the resolved destination, or do nothing when disabled.
    /// Shared by [`AuditSink::record`] and [`AuditSink::record_session_event`]: same framing,
    /// same failure handling (a write failure never breaks the call path; it is reported via
    /// `tracing::warn!` and swallowed), different record TYPES.
    fn write_serialized(&self, record: &impl serde::Serialize, kind: &str) {
        let Some(inner) = &*self.inner.lock().unwrap_or_else(PoisonError::into_inner) else {
            return;
        };
        let line = match serde_json::to_string(record) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(error = %e, kind, "failed to serialize audit record");
                return;
            }
        };
        match inner {
            Inner::File(path) => {
                if let Err(e) = destinations::append_line_to_file(path, &line) {
                    tracing::warn!(
                        error = %e,
                        path = %path.display(),
                        kind,
                        "failed to write audit record"
                    );
                }
            }
            Inner::Stderr => destinations::write_line_to_stderr(&line),
            Inner::Syslog(addr) => {
                if let Err(e) = destinations::send_line_to_syslog(*addr, &line) {
                    tracing::warn!(
                        error = %e,
                        addr = %addr,
                        kind,
                        "failed to write audit record"
                    );
                }
            }
        }
    }
}

impl AuditSink for Recorder {
    fn record(&self, record: &AuditRecord) {
        self.write_serialized(record, "tool_call");
    }

    fn record_session_event(&self, record: &SessionEventRecord) {
        self.write_serialized(record, "session_event");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(tool: &str, action: Option<&str>, capability: &'static str) -> AuditRecord {
        AuditRecord {
            event_id: "00000000-0000-4000-8000-000000000000".to_string(),
            ts: "2026-07-02T00:00:00.000Z".to_string(),
            identity: None,
            client: None,
            tool: tool.to_string(),
            action: action.map(str::to_string),
            capability,
            domain: None,
            decision: "allow",
            grant_id: None,
            denial_id: None,
            duration_ms: 0,
            manifest: None,
            held: false,
        }
    }

    fn sample_session_event() -> SessionEventRecord {
        SessionEventRecord {
            event_id: "00000000-0000-4000-8000-000000000000".to_string(),
            ts: "2026-07-02T00:00:00.000Z".to_string(),
            identity: None,
            client: None,
            event: "session_killed",
            manifest: None,
        }
    }

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ghostlight-audit-test-{}-{tag}.jsonl",
            std::process::id()
        ))
    }

    #[test]
    fn file_destination_appends_one_line_per_record() {
        let path = temp_path("append");
        let _ = std::fs::remove_file(&path);
        let recorder = Recorder::to_file(path.clone());
        recorder.record(&sample_record("navigate", None, "read"));
        recorder.record(&sample_record("read_page", None, "read"));
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        for line in &lines {
            let _: serde_json::Value = serde_json::from_str(line).unwrap();
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn disabled_recorder_writes_nothing() {
        let path = temp_path("disabled");
        let _ = std::fs::remove_file(&path);
        let recorder = Recorder::disabled();
        assert!(!recorder.is_enabled());
        recorder.record(&sample_record("navigate", None, "read"));
        assert!(!path.exists());
    }

    #[test]
    fn session_event_appends_one_line_alongside_tool_call_records() {
        let path = temp_path("session-event");
        let _ = std::fs::remove_file(&path);
        let recorder = Recorder::to_file(path.clone());
        recorder.record(&sample_record("navigate", None, "read"));
        recorder.record_session_event(&sample_session_event());
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        let event_line: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(event_line["event"], "session_killed");
        assert!(event_line.get("tool").is_none());
        assert!(event_line.get("decision").is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn default_audit_path_ends_with_ghostlight_audit_jsonl() {
        if let Some(p) = destinations::default_audit_path() {
            let components: Vec<_> = p
                .components()
                .rev()
                .take(2)
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();
            assert_eq!(components[0], "audit.jsonl");
            assert_eq!(components[1], "ghostlight");
        }
    }

    #[test]
    fn reload_reopens_the_sink_on_a_config_change() {
        use crate::governance::config::layers::{self, LayerInputs};
        use serde_json::json;

        let file_path = temp_path("reload");
        let _ = std::fs::remove_file(&file_path);

        // Start disabled.
        let disabled_inputs = LayerInputs {
            user: serde_json::Map::from_iter([(
                crate::governance::config::AUDIT_ENABLED.to_string(),
                json!(false),
            )]),
            ..Default::default()
        };
        let config = Config::from_resolution(&layers::resolve(&disabled_inputs));
        let recorder = Recorder::from_config(&config);
        assert!(!recorder.is_enabled());

        // Reload to a config with audit enabled, destination file, and an explicit temp path
        // (never the real platform default, so this test never touches shared state).
        let enabled_inputs = LayerInputs {
            user: serde_json::Map::from_iter([
                (
                    crate::governance::config::AUDIT_ENABLED.to_string(),
                    json!(true),
                ),
                (
                    crate::governance::config::AUDIT_DESTINATION.to_string(),
                    json!("file"),
                ),
                (
                    crate::governance::config::AUDIT_FILE_PATH.to_string(),
                    json!(file_path.to_string_lossy()),
                ),
            ]),
            ..Default::default()
        };
        let config = Config::from_resolution(&layers::resolve(&enabled_inputs));
        recorder.reload(&config);
        assert!(recorder.is_enabled());

        recorder.record(&sample_record("navigate", None, "read"));
        assert!(file_path.exists());
        std::fs::remove_file(&file_path).ok();
    }

    #[test]
    fn syslog_destination_sends_one_rfc5424_datagram_per_record() {
        use crate::governance::config::layers::{self, LayerInputs};
        use serde_json::json;

        let listener = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        listener
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let addr = listener.local_addr().unwrap();

        let inputs = LayerInputs {
            user: serde_json::Map::from_iter([
                (
                    crate::governance::config::AUDIT_ENABLED.to_string(),
                    json!(true),
                ),
                (
                    crate::governance::config::AUDIT_DESTINATION.to_string(),
                    json!("syslog"),
                ),
                (
                    crate::governance::config::AUDIT_SYSLOG_ADDRESS.to_string(),
                    json!(addr.to_string()),
                ),
            ]),
            ..Default::default()
        };
        let config = Config::from_resolution(&layers::resolve(&inputs));
        let recorder = Recorder::from_config(&config);
        assert!(recorder.is_enabled());

        recorder.record(&sample_record("navigate", None, "read"));

        let mut buf = [0u8; 4096];
        let (n, _) = listener.recv_from(&mut buf).expect("expected one datagram");
        let payload = String::from_utf8_lossy(&buf[..n]).to_string();
        assert!(payload.starts_with("<134>1 "), "payload: {payload}");
        assert!(payload.contains(" ghostlight "), "payload: {payload}");
        assert!(payload.contains("\"event_id\""), "payload: {payload}");
    }

    #[test]
    fn none_destination_discards_records_and_reports_disabled() {
        use crate::governance::config::layers::{self, LayerInputs};
        use serde_json::json;

        let inputs = LayerInputs {
            user: serde_json::Map::from_iter([
                (
                    crate::governance::config::AUDIT_ENABLED.to_string(),
                    json!(true),
                ),
                (
                    crate::governance::config::AUDIT_DESTINATION.to_string(),
                    json!("none"),
                ),
            ]),
            ..Default::default()
        };
        let config = Config::from_resolution(&layers::resolve(&inputs));
        let recorder = Recorder::from_config(&config);
        assert!(!recorder.is_enabled());

        let path = temp_path("none-destination");
        let _ = std::fs::remove_file(&path);
        recorder.record(&sample_record("navigate", None, "read"));
        assert!(!path.exists());
    }

    #[test]
    fn invalid_syslog_address_disables_audit_with_a_warning() {
        use crate::governance::config::layers::{self, LayerInputs};
        use serde_json::json;

        let inputs = LayerInputs {
            user: serde_json::Map::from_iter([
                (
                    crate::governance::config::AUDIT_ENABLED.to_string(),
                    json!(true),
                ),
                (
                    crate::governance::config::AUDIT_DESTINATION.to_string(),
                    json!("syslog"),
                ),
                (
                    crate::governance::config::AUDIT_SYSLOG_ADDRESS.to_string(),
                    json!("not an address"),
                ),
            ]),
            ..Default::default()
        };
        let config = Config::from_resolution(&layers::resolve(&inputs));
        let recorder = Recorder::from_config(&config);
        assert!(!recorder.is_enabled());
    }

    #[test]
    fn reload_switches_file_to_syslog() {
        use crate::governance::config::layers::{self, LayerInputs};
        use serde_json::json;

        let file_path = temp_path("reload-to-syslog");
        let _ = std::fs::remove_file(&file_path);

        let file_inputs = LayerInputs {
            user: serde_json::Map::from_iter([
                (
                    crate::governance::config::AUDIT_ENABLED.to_string(),
                    json!(true),
                ),
                (
                    crate::governance::config::AUDIT_DESTINATION.to_string(),
                    json!("file"),
                ),
                (
                    crate::governance::config::AUDIT_FILE_PATH.to_string(),
                    json!(file_path.to_string_lossy()),
                ),
            ]),
            ..Default::default()
        };
        let config = Config::from_resolution(&layers::resolve(&file_inputs));
        let recorder = Recorder::from_config(&config);
        assert!(recorder.is_enabled());

        let listener = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        listener
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let addr = listener.local_addr().unwrap();

        let syslog_inputs = LayerInputs {
            user: serde_json::Map::from_iter([
                (
                    crate::governance::config::AUDIT_ENABLED.to_string(),
                    json!(true),
                ),
                (
                    crate::governance::config::AUDIT_DESTINATION.to_string(),
                    json!("syslog"),
                ),
                (
                    crate::governance::config::AUDIT_SYSLOG_ADDRESS.to_string(),
                    json!(addr.to_string()),
                ),
            ]),
            ..Default::default()
        };
        let config = Config::from_resolution(&layers::resolve(&syslog_inputs));
        recorder.reload(&config);
        assert!(recorder.is_enabled());

        recorder.record(&sample_record("navigate", None, "read"));

        let mut buf = [0u8; 4096];
        let (n, _) = listener.recv_from(&mut buf).expect("expected one datagram");
        let payload = String::from_utf8_lossy(&buf[..n]).to_string();
        assert!(payload.starts_with("<134>1 "), "payload: {payload}");

        std::fs::remove_file(&file_path).ok();
    }
}
