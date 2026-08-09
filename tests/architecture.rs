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

/// ADR-0101: product operations may reach the browser only through typed mechanisms.
/// String-keyed Browser call wrappers are test-fixture compatibility, never production dispatch.
#[test]
fn tool_browser_dispatches_use_typed_mechanisms() {
    let mut files = Vec::new();
    collect_rust_files(&tool_dir(), &mut files);
    let mut violations = Vec::new();
    for file in files {
        let contents = fs::read_to_string(&file).expect("read tool source");
        let compact: String = contents
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .collect();
        if compact.contains(".call(")
            || compact.contains(".call_with_context(")
            || compact.contains(".call_with_delivery_outcome(")
        {
            violations.push(file.display().to_string());
        }
    }
    assert!(
        violations.is_empty(),
        "tool execution regained a string-keyed browser dispatch path: {}",
        violations.join(", ")
    );
}

/// Response-dependent operation handlers may emit only mechanisms declared by their operation's
/// closed dynamic plan. Browser-wide instrumentation
/// has separate authority and therefore is not part of this handler-only guard.
#[test]
fn dynamic_handlers_enforce_their_canonical_physical_plans() {
    for relative in [
        "crates/core/src/tool/act_on.rs",
        "crates/core/src/tool/drag.rs",
        "crates/core/src/tool/form_fill.rs",
        "crates/core/src/tool/page_read.rs",
        "crates/core/src/tool/tab_navigation.rs",
        "crates/core/src/tool/target_screenshot.rs",
        "crates/core/src/tool/wait.rs",
    ] {
        let source = read_repo_file(relative);
        let production = production_source(&source);
        assert!(
            production.contains("for_operation(")
                || (relative.ends_with("tab_navigation.rs")
                    && production.contains("compile_navigation_transaction(")),
            "dynamic handler does not bind physical work to its operation plan: {relative}"
        );
        for escape in [
            "MechanismRequest::object(",
            "MechanismRequest::new(",
            "BrowserControl::object(",
            "BrowserControl::new(",
        ] {
            assert!(
                !production.contains(escape),
                "dynamic handler bypasses its operation plan with {escape}: {relative}"
            );
        }
    }
}

/// ADR-0101 R3: raw physical construction is private to the mechanism authority. Every
/// production caller must bind the request to a canonical operation or one closed auxiliary
/// purpose; a future handler cannot mint an arbitrary typed id and bypass those plans.
#[test]
fn raw_physical_construction_cannot_escape_the_mechanism_owner() {
    let root = repo_path("crates/core/src");
    let owner = repo_path("crates/core/src/browser/mechanism.rs");
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);
    let mut violations = Vec::new();
    for file in files {
        if file == owner {
            continue;
        }
        let source = fs::read_to_string(&file).expect("read core source");
        let production = production_source(&source);
        let compact = production
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .collect::<String>();
        for escape in [
            "MechanismRequest::object(",
            "MechanismRequest::new(",
            "BrowserControl::object(",
            "BrowserControl::new(",
            "MechanismRequest{id:",
            "MechanismRequest{input:",
            "BrowserControl{id:",
            "BrowserControl{input:",
        ] {
            if compact.contains(escape) {
                violations.push(format!("{}: {escape}", file.display()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "raw physical construction escaped the mechanism authority:\n{}",
        violations.join("\n")
    );

    let browser = read_repo_file("crates/core/src/hub/outbound/browser.rs");
    assert!(production_source(&browser).contains("send_and_await_delivery"));
    assert!(browser.contains("pub(crate) async fn execute_mechanism("));
    assert!(!browser.contains("pub async fn execute_mechanism("));
}

/// ADR-0101 R3: the auxiliary constructor is reserved for the exact cross-cutting owners named
/// by the closed auxiliary-purpose vocabulary.
#[test]
fn auxiliary_physical_authority_has_only_the_declared_callers() {
    let allowed = [
        "crates/core/src/browser/mechanism.rs",
        "crates/core/src/hub/outbound/browser.rs",
        "crates/core/src/tool/pipeline.rs",
    ];
    let mut files = Vec::new();
    collect_rust_files(&repo_path("crates/core/src"), &mut files);
    let mut violations = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("read core source");
        let production = production_source(&source);
        if production.contains("for_auxiliary(") {
            let relative = file
                .strip_prefix(repo_path("."))
                .expect("core source under repository")
                .to_string_lossy()
                .replace('\\', "/");
            if !allowed.contains(&relative.as_str()) {
                violations.push(relative);
            }
        }
    }
    assert!(
        violations.is_empty(),
        "auxiliary physical authority escaped its declared owners: {}",
        violations.join(", ")
    );
}

/// ADR-0101 R3: covered one-way controls and recording events cross production browser logic only
/// as typed ids. Their old adapter spellings belong to the bounded compatibility serializer.
#[test]
fn browser_control_and_event_aliases_are_adapter_only() {
    let adapter = read_repo_file("crates/core/src/hub/outbound/adapter_wire_v0.rs");
    let browser = read_repo_file("crates/core/src/hub/outbound/browser.rs");
    let production = production_source(&browser);
    assert!(production.contains("route_reply"));
    let production_logic = production
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n");

    for alias in [
        "gif_lease_renew",
        "gif_capture_cancel",
        "narration_clear",
        "notification",
        "attention_required",
        "attention_resolved",
        "gif_frame",
        "gif_capture_ended",
        "tool_response",
        "tool_error",
        "tab_url_response",
        "tool_accepted",
        "tool_terminal",
        "surface_destroyed",
        "session_killed",
        "debug_event",
        "get_hold",
        "set_hold",
        "toggle_hold",
        "get_attention",
        "attention_action",
        "hold_state",
        "hold_error",
        "attention_state",
        "attention_error",
    ] {
        let quoted = format!("\"{alias}\"");
        assert!(
            adapter.contains(&quoted),
            "legacy browser alias lost its one compatibility owner: {alias}"
        );
        assert!(
            !production_logic.contains(&quoted),
            "production Browser logic regained raw compatibility alias {alias}"
        );
    }
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_path(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn production_source(source: &str) -> &str {
    ["#[cfg(test)]\nmod tests", "#[cfg(test)]\r\nmod tests"]
        .into_iter()
        .filter_map(|marker| source.find(marker))
        .min()
        .map_or(source, |index| &source[..index])
}

#[test]
fn production_scanner_ignores_cfg_test_imports_without_truncating_live_code() {
    let source = "#[cfg(test)]\nuse crate::test_support;\nfn live() {}\n#[cfg(test)]\nmod tests {}";
    let production = production_source(source);
    assert!(production.contains("fn live()"));
    assert!(!production.contains("mod tests"));
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

/// ADR-0102: browser evidence is reduced inside the operation executor. The completion shore can
/// bind owned handles, but cannot see adapter JSON or recreate result meaning.
#[test]
fn operation_completion_is_typed_and_non_inferential() {
    let outcome = read_repo_file("crates/core/src/tool/outcome.rs");
    let reducer = read_repo_file("crates/core/src/operation/result.rs");
    let completion = read_repo_file("crates/core/src/hub/completion.rs");
    let bridge = read_repo_file("crates/core/src/hub/bridge.rs");
    let transport = read_repo_file("crates/transport/src/operation.rs");

    assert!(outcome.contains("pub struct OperationExecution"));
    assert!(outcome.contains("pub disposition: ExecutionDisposition"));
    assert!(outcome.contains("pub navigation: NavigationCompletion"));
    assert!(outcome.contains("pub audit: ExecutionAuditFacts"));
    assert!(outcome.contains("pub targets: ResolvedTargets"));
    assert!(!outcome.contains("OperationReceipt"));

    assert!(reducer.contains("fn reduce_operation_payload("));
    assert!(!reducer.contains("project_operation_result"));
    assert!(!reducer.contains("project_operation_data"));
    assert!(reducer.contains("OperationResult::BrowserGetStatus"));
    assert!(reducer.contains("OperationResult::BrowserHandleDialog"));

    assert!(!completion.contains("serde_json"));
    assert!(!completion.contains("canonicalize"));
    assert!(!completion.contains("reduce_operation"));
    assert!(!bridge.contains("canonicalize_operation_success"));
    assert!(!transport.contains("pub data: Value"));
}

/// ADR-0102: a sequence child enters the same public operation executor as a direct call. It may
/// not call a private raw executor or perform its own completion/projection pass.
#[test]
fn sequence_children_share_the_direct_operation_executor() {
    let flow = read_repo_file("crates/core/src/tool/flow.rs");
    let production = production_source(&flow);
    assert!(production.contains("use crate::tool::pipeline::run_work;"));
    assert!(production.contains("run_work("));
    for forbidden in [
        "run_work_execution(",
        "build_operation_completion(",
        "bind_operation_completion(",
        "canonicalize_operation_success(",
        "project_operation_result(",
    ] {
        assert!(
            !production.contains(forbidden),
            "sequence regained a second completion path through {forbidden}"
        );
    }
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

/// Operation-scoped evidence belongs to the typed receipt, never hidden in adapter JSON.
#[test]
fn operation_completion_has_no_private_json_marker_channel() {
    let mut files = Vec::new();
    collect_rust_files(&repo_path("crates/core/src"), &mut files);
    let forbidden = [
        "\"_operation_tab\"",
        "\"_batch_id\"",
        "\"_target_assurance\"",
        "\"_outcome_category\"",
        "\"_canonical_readiness\"",
        "\"_navigation_landing_denied\"",
        "\"_navigation_committed_partial\"",
        "\"_composed_effect\"",
        "\"_target_blocked\"",
        "\"_canonicalTarget\"",
        "\"_canonicalFrom\"",
        "\"_canonicalTo\"",
        "\"_canonicalNavigationFinalUrl\"",
    ];
    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        for marker in forbidden {
            assert!(
                !source.contains(marker),
                "private operation marker {marker} returned in {}",
                path.display()
            );
        }
    }
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
