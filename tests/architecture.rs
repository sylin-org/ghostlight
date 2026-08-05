// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Fail-closed guards on the governance and protocol-shore boundaries.
//!
//! `governance/` is the domain-agnostic core: it is written so it can later be lifted into its
//! own crate with no code change. This test walks every `.rs` file under `src/governance/`
//! (recursively) and fails the build if any file names `browser`, `transport`, `mcp`, `native`,
//! or the `url` crate, in code, a doc comment, or a string literal. Scanning raw text (not just
//! compiled code) is intentional: the invariant is "the core does not even NAME these", which a
//! text scan enforces exactly, and it never has a false negative from a comment-stripping pass.

use std::fs;
use std::path::{Path, PathBuf};

/// Forbidden path edges: a `governance/` source file may never contain any of these as a path
/// token. Each is matched with both a leading and a trailing identifier boundary, so
/// `crate::native` matches but a hypothetical `crate::native_helpers` does not.
const FORBIDDEN_CRATE_EDGES: &[&str] = &[
    "crate::browser",
    "crate::transport",
    "crate::tool",
    "crate::native",
];

/// H3 (ADR-0030 "Preserved invariants" as amended; PINS.md-adjacent, the sanctioned scanner
/// extension): bare identifiers a `governance/` source may never name, on top of the crate-edge
/// and `url` checks above. Matched the same way as a crate edge (leading AND trailing identifier
/// boundary), so e.g. `tabIdString` or `fetch_token` do not false-positive.
const FORBIDDEN_IDENTIFIERS: &[&str] = &["tabId", "token", "socket"];

/// True when `line` (after trimming leading whitespace) is a rustdoc comment (`///` or `//!`).
/// H3's [`FORBIDDEN_IDENTIFIERS`] check is scoped to CODE lines only (unlike the pre-existing
/// crate-edge/`url` checks above, which intentionally scan doc comments too): `tabId`/`token`/
/// `socket` are ordinary English words that already appear, incidentally and correctly, in
/// unrelated governance prose (e.g. a grammar/HTML "token", or a network "socket" in an unrelated
/// doc comment) -- the concern this check exists for (ADR-0030: "the core additionally names no
/// tabId/token/socket TYPE") is a code-level fact, and a bare-word prose scan cannot distinguish
/// that from incidental English vocabulary the way the rare, specific `crate::` qualified paths
/// can.
fn is_doc_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("///") || trimmed.starts_with("//!")
}

/// True when `b` is an ASCII identifier character (`[A-Za-z0-9_]`).
fn is_ident_char(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

/// True when `needle` occurs in `hay` as a path token: preceded by a non-identifier boundary,
/// and (when `require_trailing_boundary`) followed by a non-identifier boundary. ASCII needle.
fn contains_path_token(hay: &str, needle: &str, require_trailing_boundary: bool) -> bool {
    let bytes = hay.as_bytes();
    let mut start = 0usize;
    while let Some(rel) = hay[start..].find(needle) {
        let i = start + rel;
        let end = i + needle.len();
        let before_ok = i == 0 || !is_ident_char(bytes[i - 1]);
        let after_ok =
            !require_trailing_boundary || end >= bytes.len() || !is_ident_char(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        start = i + 1;
    }
    false
}

/// True when `line` references the `url` crate: `url::` as a leading path token, or a bare
/// `use url` / `extern crate url` import that terminates immediately.
fn references_url_crate(line: &str) -> bool {
    // Path-qualified use: `url::...`. Leading boundary only; it is inherently a path continuation.
    if contains_path_token(line, "url::", false) {
        return true;
    }
    for kw in ["use url", "extern crate url"] {
        if let Some(pos) = line.find(kw) {
            let before_ok = pos == 0 || !is_ident_char(line.as_bytes()[pos - 1]);
            let rest = line[pos + kw.len()..].trim_start();
            let terminates = rest.is_empty()
                || rest.starts_with(';')
                || rest.starts_with("as ")
                || rest.starts_with("as\t");
            if before_ok && terminates {
                return true;
            }
        }
    }
    false
}

/// Scan one source line and return every forbidden edge it contains, in a stable order
/// (crate edges first, in `FORBIDDEN_CRATE_EDGES` order, then `"url"`).
fn scan_line(line: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for edge in FORBIDDEN_CRATE_EDGES {
        if contains_path_token(line, edge, true) {
            hits.push((*edge).to_string());
        }
    }
    if references_url_crate(line) {
        hits.push("url".to_string());
    }
    // H3 (sanctioned addition): bare tabId/token/socket identifiers, same boundary matching,
    // scoped to code lines only (see `is_doc_comment`).
    if !is_doc_comment(line) {
        for ident in FORBIDDEN_IDENTIFIERS {
            if contains_path_token(line, ident, true) {
                hits.push((*ident).to_string());
            }
        }
    }
    hits
}

/// The `src/governance/` directory, anchored at the crate root so the test is independent of
/// the current working directory. `CARGO_MANIFEST_DIR` is the directory holding `Cargo.toml`.
fn governance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates")
        .join("core")
        .join("src")
        .join("governance")
}

fn tool_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates")
        .join("core")
        .join("src")
        .join("tool")
}

/// Recursively collect every `.rs` file under `dir` into `out`. Hand-rolled, no `walkdir`.
fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display())) {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Fail-closed guard: no file under `src/governance/` may depend on `browser`, `transport`,
/// `mcp`, `native`, or the `url` crate. This is what keeps the domain-agnostic core
/// relocatable (ADR-0021, PLAN A7).
#[test]
fn governance_core_has_no_forbidden_back_edges() {
    let dir = governance_dir();
    assert!(
        dir.is_dir(),
        "src/governance/ not found at {} -- A1 (module reorg) must create it before A7 runs",
        dir.display()
    );

    let mut files = Vec::new();
    collect_rust_files(&dir, &mut files);
    assert!(
        !files.is_empty(),
        "no .rs files found under {}; the scan would be vacuously green",
        dir.display()
    );

    let mut violations: Vec<String> = Vec::new();
    for file in &files {
        let contents =
            fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        for (idx, line) in contents.lines().enumerate() {
            for edge in scan_line(line) {
                violations.push(format!(
                    "{}:{}: forbidden edge `{}`",
                    file.display(),
                    idx + 1,
                    edge
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "governance/ core must not name browser/transport/mcp/native or the url crate.\n\
         The core is relocatable ONLY while it has no back-edges. Move the coupling behind a \
         port (A2) or into browser/. Violations:\n{}",
        violations.join("\n")
    );
}

/// ADR-0080: production tool code may not use the legacy safety-protocol Browser::call wrapper.
/// Every ordinary browser dispatch must name call_with_context and carry an ExecutionContext.
#[test]
fn tool_browser_dispatches_require_an_execution_context() {
    let mut files = Vec::new();
    collect_rust_files(&tool_dir(), &mut files);
    let mut violations = Vec::new();
    for file in files {
        let contents = fs::read_to_string(&file).expect("read tool source");
        let production = contents.split("#[cfg(test)]").next().unwrap_or(&contents);
        let compact: String = production
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .collect();
        if compact.contains("browser.call(") {
            violations.push(file.display().to_string());
        }
    }
    assert!(
        violations.is_empty(),
        "ordinary tool browser sends must use call_with_context; legacy call found in: {}",
        violations.join(", ")
    );
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_path(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// ADR-0096: the protocol edge may depend on transport mechanics, never the service engine.
#[test]
fn protocol_edge_has_no_service_engine_dependency() {
    let manifest = read_repo_file("crates/mcp-connector/Cargo.toml");
    for forbidden in ["ghostlight-core", "path = \"../core\""] {
        assert!(
            !manifest.contains(forbidden),
            "crates/mcp-connector must not depend on the service engine: found {forbidden}"
        );
    }
    assert!(manifest.contains("ghostlight-transport"));

    let mut files = Vec::new();
    collect_rust_files(&repo_path("crates/mcp-connector/src"), &mut files);
    for file in files {
        let source = fs::read_to_string(&file).expect("read protocol-edge source");
        assert!(
            !source.contains("ghostlight_core") && !source.contains("ghostlight::browser"),
            "protocol edge imports service code: {}",
            file.display()
        );
    }
}

/// ADR-0096: exact revision and client-wire vocabulary stops at the service shore.
#[test]
fn service_execution_is_protocol_revision_agnostic() {
    let mut files = Vec::new();
    collect_rust_files(&tool_dir(), &mut files);
    files.push(repo_path("crates/core/src/hub/bridge.rs"));
    files.push(repo_path("crates/core/src/work.rs"));
    let forbidden = [
        "2025-11-25",
        "2026-07-28",
        "json-rpc",
        "json_rpc",
        "stdio",
        "mcp tasks",
    ];
    let mut violations = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("read neutral service source");
        let lower = source.to_ascii_lowercase();
        for term in forbidden {
            if lower.contains(term) {
                violations.push(format!("{}: {term}", file.display()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "protocol-specific vocabulary crossed into service execution:\n{}",
        violations.join("\n")
    );
}

/// ADR-0096: process identity is diagnostic only, never application authority or routing state.
#[test]
fn work_and_workspace_routing_do_not_name_pid() {
    for relative in [
        "crates/core/src/work.rs",
        "crates/core/src/hub/workspace.rs",
        "crates/core/src/hub/scheduling.rs",
    ] {
        let lower = read_repo_file(relative).to_ascii_lowercase();
        assert!(
            !contains_path_token(&lower, "pid", true)
                && !lower.contains("process_id")
                && !lower.contains("processid"),
            "process identity entered authority or routing state: {relative}"
        );
    }
}

/// ADR-0096: the relay is browser-only; the MCP role cannot grow back as a flag.
#[test]
fn browser_connector_has_no_agent_or_mcp_role() {
    let source = read_repo_file("crates/browser-connector/src/main.rs");
    for forbidden in ["--role", "Role::McpEdge", "relay_adapter", "ROLE_MCP"] {
        assert!(
            !source.contains(forbidden),
            "browser connector regained an MCP role: {forbidden}"
        );
    }
}

/// ADR-0096: releases have exactly three product executables; Lightbox remains dev-only.
#[test]
fn shipped_executable_topology_is_exact() {
    let root = read_repo_file("Cargo.toml");
    let mcp = read_repo_file("crates/mcp-connector/Cargo.toml");
    let browser = read_repo_file("crates/browser-connector/Cargo.toml");
    let lightbox = read_repo_file("crates/lightbox/Cargo.toml");
    assert!(root.contains("name = \"ghostlight\""));
    assert!(mcp.contains("name = \"ghostlight-mcp-connector\""));
    assert!(browser.contains("name = \"ghostlight-browser-connector\""));
    assert!(lightbox.contains("publish = false"));
    assert!(repo_path("src/main.rs").is_file());
    assert!(repo_path("crates/mcp-connector/src/main.rs").is_file());
    assert!(repo_path("crates/browser-connector/src/main.rs").is_file());
    assert!(!repo_path("crates/core/src/mcp").exists());
}

#[test]
fn scanner_detects_forbidden_crate_edges() {
    assert_eq!(
        scan_line("use crate::browser::Cdp;"),
        vec!["crate::browser".to_string()]
    );
    assert_eq!(
        scan_line("    let h = crate::transport::Handle::new();"),
        vec!["crate::transport".to_string()]
    );
    assert_eq!(
        scan_line("use crate::tool::result::Foo;"),
        vec!["crate::tool".to_string()]
    );
    assert_eq!(
        scan_line("crate::native::host::send();"),
        vec!["crate::native".to_string()]
    );
}

/// H3 (`docs/tasks/hub/H3-session-identity-guid.md` item 5, ADR-0030 "Preserved invariants" as
/// amended): proves the `tabId`/`token`/`socket` extension is live, not dead code, without
/// weakening any existing rule -- a synthetic source naming each is flagged, and one naming none
/// of the three passes.
#[test]
fn governance_core_rejects_tabid_token_socket_identifiers() {
    assert_eq!(scan_line("let tabId: i64 = 12;"), vec!["tabId".to_string()]);
    assert_eq!(
        scan_line("let token = fetch_token();"),
        vec!["token".to_string()]
    );
    assert_eq!(
        scan_line("let socket = accept();"),
        vec!["socket".to_string()]
    );
    assert!(scan_line("use crate::config::registry::KeyDef;").is_empty());
}

#[test]
fn scanner_detects_url_crate_reference() {
    assert_eq!(scan_line("use url::Url;"), vec!["url".to_string()]);
    assert_eq!(
        scan_line("let u = url::Url::parse(s)?;"),
        vec!["url".to_string()]
    );
    assert_eq!(scan_line("use url as u;"), vec!["url".to_string()]);
    assert_eq!(scan_line("extern crate url;"), vec!["url".to_string()]);
}

#[test]
fn scanner_ignores_clean_lines() {
    // Legitimate intra-core and std paths.
    assert!(scan_line("use crate::config::registry::KeyDef;").is_empty());
    assert!(scan_line("use super::ports::Decision;").is_empty());
    assert!(scan_line("use std::collections::HashMap;").is_empty());
    // Trailing boundary: a different module whose name merely starts with a forbidden one.
    assert!(scan_line("use crate::browser_stats::X;").is_empty());
    // Leading boundary: a longer crate name.
    assert!(scan_line("use mycrate::tool_helpers::Y;").is_empty());
    // `url` letters inside identifiers are not the crate.
    assert!(scan_line("let full_url = build_url();").is_empty());
    assert!(scan_line("struct R { url: String }").is_empty());
    // The `crate::` prefix scopes the ban: a variable or prose `native` is fine.
    assert!(scan_line("let native = true; // native messaging path").is_empty());
}
