// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Integration tests for the G12 manifest engine's public API: the three example manifests
//! under `examples/` parse and validate, and the all-open invariant holds. The exhaustive
//! invalid-field matrix, the hash pins, and the source-grammar/selection tests live as inline
//! unit tests in `governance::manifest::document`/`governance::manifest::source` (pure
//! functions, no real files or environment touched); this file exercises the public API against
//! real example files on disk, the one thing inline unit tests cannot do without reaching
//! outside the crate.

use ghostlight::browser::pattern;
use ghostlight::governance::manifest::document::parse_manifest;

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
    let org_path = ghostlight::governance::config::load::org_policy_path();
    if org_path.exists() {
        eprintln!(
            "skipping the strict all-open assertion: a real org policy file exists at {}",
            org_path.display()
        );
        return;
    }

    let loaded =
        ghostlight::governance::manifest::source::load_policy(None, pattern::is_valid_pattern)
            .expect("no sources present: loading must not fail");
    assert_eq!(loaded.manifest, None);
    assert_eq!(loaded.origin, None);
    assert!(!loaded.user_manifest_ignored);
}
