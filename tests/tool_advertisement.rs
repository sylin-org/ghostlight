//! Integration test for G14 tool advertisement filtering: proves the wiring end to end (a
//! restrictive manifest's grants actually reach `tools/list` through the real server loop), not
//! just the pure filtering logic (`browser::advertise`'s own exhaustive inline unit tests cover
//! that). No extension is ever connected; `tools/list` never touches it.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

/// Mirrors `tests/tool_enforcement.rs`'s helper of the same name: the `file://` source-string
/// form `governance::manifest::source::parse_source_string` expects, on either platform.
fn file_uri(path: &Path) -> String {
    let forward = path.to_string_lossy().replace('\\', "/");
    match forward.strip_prefix('/') {
        Some(rest) => format!("file:///{rest}"),
        None => format!("file:///{forward}"),
    }
}

fn write_manifest(tag: &str, value: &Value) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ghostlight-tool-advertisement-{}-{tag}-{}.json",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}

fn drive(manifest_path: &Path, requests: &[Value]) -> Vec<Value> {
    let endpoint = format!(
        "ghostlight-ad-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_ghostlight"))
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .arg("--manifest")
        .arg(file_uri(manifest_path))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight");

    let mut stdin = child.stdin.take().expect("stdin");
    for req in requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    drop(stdin);

    let stdout = child.stdout.take().expect("stdout");
    let responses: Vec<Value> = BufReader::new(stdout)
        .lines()
        .map(|l| serde_json::from_str(&l.unwrap()).expect("each stdout line is JSON"))
        .collect();
    child.wait().expect("wait for child");
    responses
}

/// A read-only manifest (`allowed: ["read"]`). Per ADR-0022 Decision 8, a read-only grant
/// advertises every tool with a directory variant that is `requires: []` or a subset of `read`
/// -- everything except `form_input` (requires `write`) and `javascript_tool` (requires
/// `execute`).
#[test]
fn read_only_manifest_advertises_everything_except_write_and_execute_tools() {
    let manifest = write_manifest(
        "read-only",
        &json!({
            "schema": 3,
            "name": "g14-read-only",
            "version": "1",
            "grants": [
                { "id": "r", "hosts": {"allow": ["example.com"]}, "allowed": ["read"] },
            ],
        }),
    );

    let responses = drive(
        &manifest,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        ],
    );
    let list = responses
        .iter()
        .find(|r| r["id"] == 2)
        .expect("a tools/list response");
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        vec![
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
        ]
    );

    std::fs::remove_file(&manifest).ok();
}

/// An empty `grants` array advertises exactly the requires-empty set (ADR-0022 Decision 5 step
/// 2: those actions need no grant at all), not the full surface and not nothing.
#[test]
fn empty_grants_manifest_advertises_exactly_the_requires_empty_set() {
    let manifest = write_manifest(
        "empty-grants",
        &json!({
            "schema": 3,
            "name": "g14-empty-grants",
            "version": "1",
            "grants": [],
        }),
    );

    let responses = drive(
        &manifest,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        ],
    );
    let list = responses
        .iter()
        .find(|r| r["id"] == 2)
        .expect("a tools/list response");
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        vec![
            "tabs_create_mcp",
            "computer",
            "resize_window",
            "update_plan",
            "explain",
        ]
    );

    std::fs::remove_file(&manifest).ok();
}
