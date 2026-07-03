//! Integration tests for the G12 manifest engine's public API: the three example manifests
//! under `examples/` parse and validate, and the all-open invariant holds. The exhaustive
//! invalid-field matrix, the hash pins, and the source-grammar/selection tests live as inline
//! unit tests in `governance::manifest::document`/`governance::manifest::source` (pure
//! functions, no real files or environment touched); this file exercises the public API against
//! real example files on disk, the one thing inline unit tests cannot do without reaching
//! outside the crate.

use browser_mcp::browser::pattern;
use browser_mcp::governance::manifest::document::parse_manifest;

fn read_example(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn assert_valid_hash(hash: &str) {
    assert_eq!(hash.len(), 64, "hash: {hash}");
    assert!(
        hash.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "hash: {hash}"
    );
}

#[test]
fn enterprise_healthcare_example_parses() {
    let text = read_example("enterprise-healthcare.json");
    let m = parse_manifest(
        &text,
        "enterprise-healthcare.json",
        pattern::is_valid_pattern,
    )
    .expect("enterprise-healthcare.json should parse and validate");
    assert_eq!(m.schema, 3);
    assert_eq!(m.name, "enterprise-healthcare");
    assert_eq!(m.grants.len(), 4);
    assert_valid_hash(&m.hash);
}

#[test]
fn developer_observe_example_parses() {
    let text = read_example("developer-observe.json");
    let m = parse_manifest(&text, "developer-observe.json", pattern::is_valid_pattern)
        .expect("developer-observe.json should parse and validate");
    assert_eq!(m.schema, 3);
    assert_eq!(m.name, "developer-observe");
    assert_eq!(m.grants.len(), 0);
    assert_valid_hash(&m.hash);
}

/// `qa-staging.json` was rewritten by G16 (Required behavior section 6) to exercise observe
/// mode, a per-grant enforce override, and a positive `tools` list for the `policy explain`
/// goldens; it no longer carries a `config` array (the G12-era Unix-shaped `audit.file.path`
/// that needed a `#[cfg(windows)]` split here is gone). Parses identically on every platform.
#[test]
fn qa_staging_example_parses() {
    let text = read_example("qa-staging.json");
    let m = parse_manifest(&text, "qa-staging.json", pattern::is_valid_pattern)
        .expect("qa-staging.json should parse and validate");
    assert_eq!(m.schema, 3);
    assert_eq!(m.name, "qa-staging");
    assert_eq!(m.grants.len(), 3);
    assert_valid_hash(&m.hash);
}

/// `developer-unrestricted.json` was added by G18 (Required behavior section 2) as the
/// `developer-unrestricted` embedded template. Distinct from the pre-existing
/// `developer-observe.json` above: same shape (empty grants, recommended-level audit config,
/// no domain restriction), different name/content per G18's own verbatim template text.
#[test]
fn developer_unrestricted_example_parses() {
    let text = read_example("developer-unrestricted.json");
    let m = parse_manifest(
        &text,
        "developer-unrestricted.json",
        pattern::is_valid_pattern,
    )
    .expect("developer-unrestricted.json should parse and validate");
    assert_eq!(m.schema, 3);
    assert_eq!(m.name, "developer-unrestricted");
    assert_eq!(m.grants.len(), 0);
    assert_valid_hash(&m.hash);
}

#[test]
fn research_read_only_example_parses() {
    let text = read_example("research-read-only.json");
    let m = parse_manifest(&text, "research-read-only.json", pattern::is_valid_pattern)
        .expect("research-read-only.json should parse and validate");
    assert_eq!(m.schema, 3);
    assert_eq!(m.name, "research-read-only");
    assert_eq!(m.grants.len(), 1);
    assert_valid_hash(&m.hash);
}

/// All-open invariant (g12 constraint 3): loading with no org file and no user source yields
/// `LoadedPolicy { manifest: None, origin: None, user_manifest_ignored: false }`.
/// `tests/mcp_protocol.rs` (unchanged by this task) already proves the binary's byte-identical
/// wire behavior end to end; this test proves the loader's own return value directly, through
/// the exact public entry point `server::run` uses. Confirms no real org policy file exists on
/// this machine first (as G02/G09's own manual-verification passes did) so the strict
/// assertion is never a false failure caused by unrelated local machine state.
#[test]
fn no_manifest_sources_yields_all_open() {
    let org_path = browser_mcp::governance::config::load::org_policy_path();
    if org_path.exists() {
        eprintln!(
            "skipping the strict all-open assertion: a real org policy file exists at {}",
            org_path.display()
        );
        return;
    }

    let loaded =
        browser_mcp::governance::manifest::source::load_policy(None, pattern::is_valid_pattern)
            .expect("no sources present: loading must not fail");
    assert_eq!(loaded.manifest, None);
    assert_eq!(loaded.origin, None);
    assert!(!loaded.user_manifest_ignored);
}

/// ADR-0023: the stage-3 outage regression test. Before this task, ANY policy file at the org
/// path was a fatal startup error (two parsers, mutually exclusive schema gates): a schema-3
/// org policy died in the now-deleted second org-file parser's schema-2-only gate. This spawns
/// the real binary with a schema-3 org policy (a read-only grant plus two mandatory config
/// entries) at a fake `ProgramData`-rooted org path, and proves the server answers
/// `initialize`/`tools/list` instead of exiting at startup, with the governed (filtered) tool
/// set for the read-only grant.
#[cfg(windows)]
#[test]
fn org_policy_file_with_config_boots_the_server() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();

    let program_data_dir =
        std::env::temp_dir().join(format!("browser-mcp-t01-program-data-{pid}-{seq}"));
    let policy_dir = program_data_dir.join("browser-mcp");
    std::fs::create_dir_all(&policy_dir).expect("create fake ProgramData\\browser-mcp");
    let policy_path = policy_dir.join("policy.json");

    let audit_path = std::env::temp_dir().join(format!("browser-mcp-t01-audit-{pid}-{seq}.jsonl"));
    // Windows JSON string values need forward slashes or escaped backslashes; forward slashes
    // are accepted as path separators on Windows and need no escaping here.
    let audit_path_str = audit_path.to_string_lossy().replace('\\', "/");

    let manifest = serde_json::json!({
        "schema": 3,
        "name": "t01-org-policy-boot",
        "version": "1",
        "grants": [
            { "id": "r", "hosts": {"allow": ["example.com"]}, "allowed": ["read"] },
        ],
        "config": [
            { "key": "audit.enabled", "value": true, "level": "mandatory" },
            { "key": "audit.file.path", "value": audit_path_str, "level": "mandatory" },
        ],
    });
    std::fs::write(&policy_path, serde_json::to_vec(&manifest).unwrap())
        .expect("write the org policy file");

    let endpoint = format!("browser-mcp-t01-{pid}-{seq}");
    let mut child = Command::new(env!("CARGO_BIN_EXE_browser-mcp"))
        .env("BROWSER_MCP_ENDPOINT", &endpoint)
        .env("ProgramData", &program_data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn browser-mcp");

    {
        let mut stdin = child.stdin.take().expect("stdin");
        for req in [
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        ] {
            stdin
                .write_all(serde_json::to_string(&req).unwrap().as_bytes())
                .unwrap();
            stdin.write_all(b"\n").unwrap();
        }
    } // drop stdin -> EOF -> the server loop ends

    let stdout = child.stdout.take().expect("stdout");
    let responses: Vec<serde_json::Value> = BufReader::new(stdout)
        .lines()
        .map(|l| serde_json::from_str(&l.unwrap()).expect("each stdout line is JSON"))
        .collect();
    child.wait().expect("wait for child");

    std::fs::remove_file(&policy_path).ok();
    std::fs::remove_dir_all(&program_data_dir).ok();
    std::fs::remove_file(&audit_path).ok();

    assert_eq!(
        responses.len(),
        2,
        "the outage regression: the server must answer both requests instead of exiting at \
         startup, got {responses:?}"
    );
    assert_eq!(responses[0]["id"], 1, "initialize response: {responses:?}");

    let list = &responses[1];
    assert_eq!(list["id"], 2);
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    // Transcribed verbatim from
    // `tests/tool_advertisement.rs::read_only_manifest_advertises_everything_except_write_and_execute_tools`.
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
}
