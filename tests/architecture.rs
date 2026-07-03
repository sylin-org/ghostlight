//! Fail-closed guard on the `governance/` core boundary (ADR-0021, PLAN A7).
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
    "crate::mcp",
    "crate::native",
];

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
    hits
}

/// The `src/governance/` directory, anchored at the crate root so the test is independent of
/// the current working directory. `CARGO_MANIFEST_DIR` is the directory holding `Cargo.toml`.
fn governance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("governance")
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
        scan_line("use crate::mcp::types::Foo;"),
        vec!["crate::mcp".to_string()]
    );
    assert_eq!(
        scan_line("crate::native::host::send();"),
        vec!["crate::native".to_string()]
    );
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
    assert!(scan_line("use mycrate::mcp_helpers::Y;").is_empty());
    // `url` letters inside identifiers are not the crate.
    assert!(scan_line("let full_url = build_url();").is_empty());
    assert!(scan_line("struct R { url: String }").is_empty());
    // The `crate::` prefix scopes the ban: a variable or prose `native` is fine.
    assert!(scan_line("let native = true; // native messaging path").is_empty());
}
