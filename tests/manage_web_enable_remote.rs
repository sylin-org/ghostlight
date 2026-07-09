// SPDX-License-Identifier: Apache-2.0 OR MIT
//! K5 (`docs/tasks/console/K5-enable-remote-connections.md`; PINS.md CS1, CS4, CS5): `POST
//! /api/v1/config/inbound-web-enable-remote`, the Console's ONE write action.
//!
//! Uses the `GHOSTLIGHT_USER_CONFIG_DIR` env override (`src/governance/config/load.rs`, added
//! during this task after `dirs::config_dir()` was found NOT to honor a platform env var
//! override for the current process the way `org_policy_path`'s `ProgramData` read does -- see
//! the LEDGER's K5 entry) so these tests never touch this machine's real user config file.

mod support;

use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static SEQ: AtomicU32 = AtomicU32::new(0);

fn http_post(port: u16, path: &str, body: &str) -> String {
    let mut stream = support::connect_webapi(port);
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn status_line(response: &str) -> &str {
    response.lines().next().unwrap_or_default()
}

fn body(response: &str) -> &str {
    // split_once: everything after the FIRST header/body delimiter, even when the body itself
    // contains a blank line (a "\r\n\r\n" run). A plain split(..).nth(1) would return only the
    // segment up to the body's first blank line and silently truncate it.
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default()
}

const ROUTE: &str = "/api/v1/config/inbound-web-enable-remote";

/// PINS.md CS5: a successful write returns the pinned `key`/`value`/`note` literals, and the
/// isolated user config file (never the real machine path) actually contains
/// `inbound.web.from: ["*"]` afterward.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn enable_remote_writes_the_pinned_value() {
    let pid = std::process::id();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let user_config_dir =
        std::env::temp_dir().join(format!("ghostlight-console-enable-remote-{pid}-{seq}"));
    std::fs::create_dir_all(&user_config_dir).unwrap();

    let endpoint = format!("ghostlight-console-enable-remote-{pid}-{seq}");
    let (mut service, port) =
        support::spawn_service_with_user_config_dir_and_webapi_port(&endpoint, &user_config_dir);

    let response = http_post(port, ROUTE, "");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
    let parsed: serde_json::Value = serde_json::from_str(body(&response)).expect("valid JSON");
    assert_eq!(parsed["key"], "inbound.web.from");
    assert_eq!(parsed["value"], serde_json::json!(["*"]));
    assert_eq!(
        parsed["note"],
        "takes effect the next time the Ghostlight service restarts"
    );
    assert!(!parsed["written_to"].as_str().unwrap_or_default().is_empty());

    let written = std::fs::read_to_string(user_config_dir.join("ghostlight").join("config.json"))
        .expect("the isolated user config file was written");
    let written: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(
        written["config"]["inbound.web.from"],
        serde_json::json!(["*"])
    );

    let _ = service.kill();
    let _ = service.wait();
    std::fs::remove_dir_all(&user_config_dir).ok();
}

/// PINS.md CS4: a successful write records exactly one `config_changed` session-event audit
/// record with the frozen 6-key shape and `identity`/`client`/`manifest` all `null`.
// Windows-only: this drives the mandatory `audit.*` config in through an injected org policy,
// and `org_policy_path()` only honors the `ProgramData` env override on Windows (macOS/Linux
// hardcode /Library/Application Support and /etc). Same platform constraint as
// `manifest_validation::org_policy_file_with_config_boots_the_server` and the whole
// `hot_reload` module. The config_changed audit path itself is platform-independent.
#[cfg(windows)]
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn enable_remote_records_one_config_changed_event() {
    let pid = std::process::id();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let user_config_dir = std::env::temp_dir().join(format!(
        "ghostlight-console-enable-remote-audit-{pid}-{seq}"
    ));
    std::fs::create_dir_all(&user_config_dir).unwrap();

    let program_data_dir =
        std::env::temp_dir().join(format!("ghostlight-console-enable-remote-pd-{pid}-{seq}"));
    let policy_dir = program_data_dir.join("ghostlight");
    std::fs::create_dir_all(&policy_dir).expect("create fake ProgramData\\ghostlight");
    let policy_path = policy_dir.join("policy.json");

    let audit_path = std::env::temp_dir().join(format!(
        "ghostlight-console-enable-remote-audit-{pid}-{seq}.jsonl"
    ));
    let audit_path_str = audit_path.to_string_lossy().replace('\\', "/");
    let manifest = serde_json::json!({
        "schema": 3,
        "name": "console-k5-audit",
        "version": "1",
        "grants": [],
        "config": [
            { "key": "audit.enabled", "value": true, "level": "mandatory" },
            { "key": "audit.destination", "value": "file", "level": "mandatory" },
            { "key": "audit.file.path", "value": audit_path_str, "level": "mandatory" },
        ],
    });
    std::fs::write(&policy_path, serde_json::to_vec(&manifest).unwrap())
        .expect("write the org policy file");

    let endpoint = format!("ghostlight-console-enable-remote-audit-{pid}-{seq}");
    let (mut service, port) =
        support::spawn_service_with_program_data_user_config_dir_and_webapi_port(
            &endpoint,
            &program_data_dir,
            &user_config_dir,
        );

    let response = http_post(port, ROUTE, "");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");

    // Give the recorder a brief moment to flush the line.
    std::thread::sleep(Duration::from_millis(200));
    let audit_content = std::fs::read_to_string(&audit_path).expect("audit file exists");
    let lines: Vec<&str> = audit_content.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "exactly one session-event line: {audit_content:?}"
    );
    let rec: serde_json::Value = serde_json::from_str(lines[0]).expect("line is JSON");
    let keys: Vec<&String> = rec.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        vec!["event_id", "ts", "identity", "client", "event", "manifest"],
        "field order matches the frozen 6-key SessionEventRecord order"
    );
    assert_eq!(rec["event"], "config_changed");
    assert_eq!(rec["identity"], serde_json::Value::Null);
    assert_eq!(rec["client"], serde_json::Value::Null);
    assert_eq!(rec["manifest"], serde_json::Value::Null);

    let _ = service.kill();
    let _ = service.wait();
    std::fs::remove_dir_all(&user_config_dir).ok();
    std::fs::remove_dir_all(&program_data_dir).ok();
    std::fs::remove_file(&audit_path).ok();
}

/// PINS.md CS5: an org-mandatory lock on `inbound.web.from` refuses the write with a `409`
/// and the exact transcribed lock-refusal message; the isolated user config file is never
/// created, and no audit event is recorded.
// Windows-only for the same reason as the audit test above: the lock is injected through an
// org policy, and only Windows honors the `ProgramData` env override in `org_policy_path()`.
// The lock-refusal logic (config `set` vs a mandatory layer) is exercised platform-independently
// by the `governance::config` unit tests.
#[cfg(windows)]
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn enable_remote_refuses_cleanly_under_an_org_mandatory_lock() {
    let pid = std::process::id();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let user_config_dir = std::env::temp_dir().join(format!(
        "ghostlight-console-enable-remote-locked-{pid}-{seq}"
    ));
    std::fs::create_dir_all(&user_config_dir).unwrap();

    let program_data_dir = std::env::temp_dir().join(format!(
        "ghostlight-console-enable-remote-locked-pd-{pid}-{seq}"
    ));
    let policy_dir = program_data_dir.join("ghostlight");
    std::fs::create_dir_all(&policy_dir).expect("create fake ProgramData\\ghostlight");
    let policy_path = policy_dir.join("policy.json");
    let manifest = serde_json::json!({
        "schema": 3,
        "name": "console-k5-locked",
        "version": "1",
        "grants": [],
        "config": [
            { "key": "inbound.web.from", "value": ["localhost"], "level": "mandatory" },
        ],
    });
    std::fs::write(&policy_path, serde_json::to_vec(&manifest).unwrap())
        .expect("write the org policy file");

    let endpoint = format!("ghostlight-console-enable-remote-locked-{pid}-{seq}");
    let (mut service, port) =
        support::spawn_service_with_program_data_user_config_dir_and_webapi_port(
            &endpoint,
            &program_data_dir,
            &user_config_dir,
        );

    let response = http_post(port, ROUTE, "");
    assert_eq!(status_line(&response), "HTTP/1.1 409 Conflict");
    let parsed: serde_json::Value = serde_json::from_str(body(&response)).expect("valid JSON");
    assert_eq!(
        parsed["error"],
        "inbound.web.from is managed by your organization (source: org_mandatory); \
         'config set' cannot override it"
    );

    assert!(
        !user_config_dir
            .join("ghostlight")
            .join("config.json")
            .exists(),
        "a refused write must never create the user config file"
    );

    let _ = service.kill();
    let _ = service.wait();
    std::fs::remove_dir_all(&user_config_dir).ok();
    std::fs::remove_dir_all(&program_data_dir).ok();
}

/// PINS.md CS5: the request body is NEVER read or honored -- the written value is always the
/// ONE pinned literal, never whatever the caller sends.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn enable_remote_ignores_the_request_body() {
    let pid = std::process::id();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let user_config_dir = std::env::temp_dir().join(format!(
        "ghostlight-console-enable-remote-ignore-body-{pid}-{seq}"
    ));
    std::fs::create_dir_all(&user_config_dir).unwrap();

    let endpoint = format!("ghostlight-console-enable-remote-ignore-body-{pid}-{seq}");
    let (mut service, port) =
        support::spawn_service_with_user_config_dir_and_webapi_port(&endpoint, &user_config_dir);

    let response = http_post(
        port,
        ROUTE,
        r#"{"key":"inbound.web.from","value":["evil.example.com"]}"#,
    );
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
    let parsed: serde_json::Value = serde_json::from_str(body(&response)).expect("valid JSON");
    assert_eq!(
        parsed["value"],
        serde_json::json!(["*"]),
        "the written value must ALWAYS be the pinned literal, never the caller's body"
    );

    let _ = service.kill();
    let _ = service.wait();
    std::fs::remove_dir_all(&user_config_dir).ok();
}
