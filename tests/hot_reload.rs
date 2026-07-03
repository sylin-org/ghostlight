//! ADR-0025 flagship integration test: manifest hot-reload end to end, against the real binary
//! as a subprocess (reusing t01's temp-`ProgramData` technique, `tests/manifest_validation.rs`).
//!
//! ISOLATION: `ProgramData` (the org policy path) is overridden to a per-test temp directory,
//! exactly as t01's own test does -- `governance::config::load::org_policy_path`'s Windows
//! branch reads it via `std::env::var`, so this genuinely redirects. The policy under test
//! carries NO audit config entries at all (the deletion phase would otherwise remove them
//! mid-test); audit stays enabled at its built-in default (Minimal == Safe preset:
//! `audit.enabled: true`, `audit.destination: "file"`, empty `audit.file.path`), stable across
//! the governed -> all-open transition.
//!
//! DEVIATION from the task prompt's literal isolation pin: the prompt also directs overriding
//! `LOCALAPPDATA` to redirect the DEFAULT audit path. On this tree, `governance::audit::
//! destinations::default_audit_path` resolves via the `dirs` crate, whose Windows backend calls
//! `SHGetKnownFolderPath` (`dirs-sys` 0.5) rather than reading the `LOCALAPPDATA` environment
//! variable -- an env-var override of the CHILD process has no effect on it (confirmed: the
//! first version of this test, following the prompt literally, wrote no audit file at the
//! faked path at all). Since the audit destination is genuinely the real per-user default no
//! matter what env vars this test sets, and this file cannot add manifest config entries for it
//! (the whole point of the "no audit config" pin) or redirect it any other way, this test
//! instead takes over the REAL resolved default path for its own duration: it backs up any
//! pre-existing file there, restores it (or removes a freshly-created one) via a `Drop` guard
//! that runs even on a mid-test panic, and reads audit lines from that real path.

#![cfg(windows)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

static SEQ: AtomicU32 = AtomicU32::new(0);

/// Kills and reaps the child on drop unless [`Self::wait_normally`] already consumed it (the
/// success path). Without this, a mid-test panic (a `wait_for`/`assert_eq!` failure) leaks the
/// spawned `ghostlight.exe` process -- `std::process::Child` does not kill on drop.
struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    /// The success path: stdin is already closed, so the child should exit on its own. Consumes
    /// `self` so the `Drop` impl's force-kill branch is a no-op afterward.
    fn wait_normally(mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Takes over the REAL default audit path for the duration of one test (see the module doc's
/// DEVIATION note): backs up any pre-existing file there, and restores it (or removes a
/// freshly-created test file) on drop -- including on a mid-test panic, so a failed run never
/// leaves the developer's real audit history altered or a stray backup file behind.
struct AuditFileGuard {
    path: std::path::PathBuf,
    backup: Option<std::path::PathBuf>,
}

impl AuditFileGuard {
    fn take_over() -> Self {
        let path = ghostlight::governance::audit::destinations::default_audit_path()
            .expect("a local-data directory is resolvable on this machine");
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
        let backup = path.with_file_name(format!("{file_name}.t06-bak"));
        let _ = std::fs::remove_file(&backup);
        let backup = if path.exists() && std::fs::rename(&path, &backup).is_ok() {
            Some(backup)
        } else {
            None
        };
        Self { path, backup }
    }
}

impl Drop for AuditFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if let Some(backup) = &self.backup {
            let _ = std::fs::rename(backup, &self.path);
        }
    }
}

fn manifest_json(caps: &[&str]) -> Value {
    json!({
        "schema": 3,
        "name": "t06-hot-reload",
        "version": "1",
        "grants": [
            { "id": "r", "hosts": {"allow": ["example.com"]}, "allowed": caps },
        ],
    })
}

fn send(stdin: &mut ChildStdin, value: &Value) {
    stdin
        .write_all(serde_json::to_string(value).unwrap().as_bytes())
        .unwrap();
    stdin.write_all(b"\n").unwrap();
}

/// Poll the shared, continuously-appended `lines` buffer (fed by a background reader thread)
/// until a not-yet-consumed line satisfies `pred`, returning the parsed value and the index one
/// past it (so the caller can resume scanning from there). Panics after `timeout`.
fn wait_for(
    lines: &Arc<Mutex<Vec<String>>>,
    start_at: usize,
    timeout: Duration,
    pred: impl Fn(&Value) -> bool,
) -> (Value, usize) {
    let deadline = Instant::now() + timeout;
    loop {
        {
            let guard = lines.lock().unwrap();
            for (i, line) in guard.iter().enumerate().skip(start_at) {
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    if pred(&v) {
                        return (v, i + 1);
                    }
                }
            }
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for a matching line");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn tool_names(resp: &Value) -> Vec<String> {
    resp["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("name").to_string())
        .collect()
}

const NOTIFICATION_LINE: &str = r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#;

/// Poll `tools/list` (one request per attempt, a fresh id each time) until the advertised set
/// equals `expected`, or panic after `deadline_secs`. Returns the index just past the matching
/// response so the caller can resume scanning `lines` from there.
#[allow(clippy::too_many_arguments)]
fn poll_tools_list_until(
    stdin: &mut ChildStdin,
    lines: &Arc<Mutex<Vec<String>>>,
    start_at: usize,
    next_id: &mut i64,
    expected: &[&str],
    deadline_secs: u64,
) -> usize {
    let deadline = Instant::now() + Duration::from_secs(deadline_secs);
    let mut consumed = start_at;
    loop {
        let id = *next_id;
        *next_id += 1;
        send(
            stdin,
            &json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":{}}),
        );
        let (resp, idx) = wait_for(lines, consumed, Duration::from_secs(10), |v| v["id"] == id);
        consumed = idx;
        if tool_names(&resp) == expected {
            return consumed;
        }
        if Instant::now() > deadline {
            panic!(
                "the advertised set never matched {expected:?}; last seen: {:?}",
                tool_names(&resp)
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// ADR-0025 end to end: a governed session's advertised set expands live when the org policy
/// file is rewritten to grant more capabilities, with exactly one `list_changed` notification;
/// then, when the policy file is deleted, the set returns to the full all-open 14 with a second
/// notification. The audit stream carries exactly two `manifest_reload` session events (the
/// second with `manifest: null`), and a `tools/call` issued after the swap still carries the
/// `initialize` request's own `clientInfo` on its audit record (client identity survives the
/// swap, ADR-0025 Decision 2).
#[test]
fn org_policy_hot_swap_end_to_end() {
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();

    let program_data_dir =
        std::env::temp_dir().join(format!("ghostlight-t06-program-data-{pid}-{seq}"));
    let policy_dir = program_data_dir.join("ghostlight");
    std::fs::create_dir_all(&policy_dir).expect("create fake ProgramData\\ghostlight");
    let policy_path = policy_dir.join("policy.json");

    // See the module doc's DEVIATION note: the default audit path is not env-var redirectable
    // on this tree, so this test takes over the REAL one for its duration instead.
    let audit_guard = AuditFileGuard::take_over();
    let audit_path = audit_guard.path.clone();

    std::fs::write(
        &policy_path,
        serde_json::to_vec(&manifest_json(&["read"])).unwrap(),
    )
    .expect("write the initial org policy file");

    let endpoint = format!("ghostlight-t06-{pid}-{seq}");
    let mut child: Child = Command::new(env!("CARGO_BIN_EXE_ghostlight"))
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .env("ProgramData", &program_data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    // From here on, any panic (a failed assertion/timeout) force-kills the child instead of
    // leaking an orphaned ghostlight.exe process (`std::process::Child` does not kill on drop).
    let child = ChildGuard::new(child);

    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let reader_lines = Arc::clone(&lines);
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        loop {
            buf.clear();
            match reader.read_line(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let trimmed = buf.trim_end().to_string();
                    if !trimmed.is_empty() {
                        reader_lines.lock().unwrap().push(trimmed);
                    }
                }
            }
        }
    });

    // initialize, carrying a clientInfo we can later prove survives the swap.
    send(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "clientInfo": { "name": "t06-test-client", "version": "1.2.3" } }
        }),
    );
    let (init_resp, mut consumed) = wait_for(&lines, 0, Duration::from_secs(10), |v| v["id"] == 1);
    assert!(init_resp["result"].is_object(), "{init_resp:?}");

    // tools/list: the governed (read-only) set, transcribed from t01's own oracle
    // (`tests/manifest_validation.rs::org_policy_file_with_config_boots_the_server`).
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    );
    let (list_resp, idx) = wait_for(&lines, consumed, Duration::from_secs(10), |v| v["id"] == 2);
    consumed = idx;
    let governed_read_only = vec![
        "tabs_context_mcp",
        "tabs_create_mcp",
        "navigate",
        "computer",
        "find",
        "get_page_text",
        "read_console_messages",
        "read_network_requests",
        "read_page",
        "resize_window",
        "update_plan",
        "explain",
    ];
    assert_eq!(tool_names(&list_resp), governed_read_only);

    // Rewrite the policy: add "action" and "write" -- unlocks form_input (requires write) but
    // not javascript_tool (requires execute, still absent).
    std::fs::write(
        &policy_path,
        serde_json::to_vec(&manifest_json(&["read", "action", "write"])).unwrap(),
    )
    .expect("rewrite the policy file");

    let expanded: Vec<&str> = vec![
        "tabs_context_mcp",
        "tabs_create_mcp",
        "navigate",
        "computer",
        "find",
        "form_input",
        "get_page_text",
        "read_console_messages",
        "read_network_requests",
        "read_page",
        "resize_window",
        "update_plan",
        "explain",
    ];
    let mut next_id = 3i64;
    // Generous timeout: the a5 watcher polls every POLL_INTERVAL (750ms at authoring); >= 4x
    // that plus settle + rebuild + notification latency.
    consumed = poll_tools_list_until(&mut stdin, &lines, consumed, &mut next_id, &expanded, 20);

    // The exact list_changed notification line appeared somewhere in the stream.
    {
        let guard = lines.lock().unwrap();
        assert!(
            guard.iter().any(|l| l == NOTIFICATION_LINE),
            "the list_changed notification line must appear: {guard:?}"
        );
    }

    // A post-swap tools/call: no fake extension is attached, so this fails at "not connected"
    // after the first-call wait -- but it still runs through the full pipeline and produces
    // exactly one completed audit record carrying the retained client identity.
    let call_id = 9000i64;
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":call_id,"method":"tools/call",
                "params":{"name":"tabs_create_mcp","arguments":{}}}),
    );
    let (_call_resp, idx) = wait_for(&lines, consumed, Duration::from_secs(15), |v| {
        v["id"] == call_id
    });
    consumed = idx;

    // Delete the policy file: org removal is a legitimate transition back to all-open (14
    // tools), with a second notification.
    std::fs::remove_file(&policy_path).expect("delete the policy file");
    let full_set: Vec<&str> = vec![
        "tabs_context_mcp",
        "tabs_create_mcp",
        "navigate",
        "computer",
        "find",
        "form_input",
        "get_page_text",
        "javascript_tool",
        "read_console_messages",
        "read_network_requests",
        "read_page",
        "resize_window",
        "update_plan",
        "explain",
    ];
    let _ = poll_tools_list_until(&mut stdin, &lines, consumed, &mut next_id, &full_set, 20);

    {
        let guard = lines.lock().unwrap();
        let count = guard.iter().filter(|l| *l == NOTIFICATION_LINE).count();
        assert_eq!(
            count, 2,
            "exactly two list_changed notifications: {guard:?}"
        );
    }

    // Close stdin -> EOF -> the server loop ends.
    drop(stdin);
    child.wait_normally();
    reader.join().ok();

    // Audit assertions: two manifest_reload session events (the second carrying manifest:
    // null), and a post-swap tools/call record still carrying the initialize clientInfo.
    let audit_lines: Vec<Value> = std::fs::read_to_string(&audit_path)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).expect("each audit line is JSON"))
        .collect();

    let reload_events: Vec<&Value> = audit_lines
        .iter()
        .filter(|v| v["event"] == "manifest_reload")
        .collect();
    assert_eq!(
        reload_events.len(),
        2,
        "two manifest_reload session events: {audit_lines:?}"
    );
    assert!(
        reload_events[1]["manifest"].is_null(),
        "the second swap (to all-open) carries manifest: null: {:?}",
        reload_events[1]
    );

    let call_records: Vec<&Value> = audit_lines
        .iter()
        .filter(|v| v.get("event").is_none())
        .collect();
    assert!(
        call_records
            .iter()
            .any(|r| r["client"]["name"] == "t06-test-client"),
        "a post-swap tools/call record still carries the initialize clientInfo: {audit_lines:?}"
    );

    std::fs::remove_file(&policy_path).ok();
    std::fs::remove_dir_all(&program_data_dir).ok();
    drop(audit_guard);
}
