// SPDX-License-Identifier: Apache-2.0 OR MIT
//! H3 role-marker wiring test (ADR-0030 Decision 1 addendum; PINS.md SS8).
//!
//! A text-scan test (a7-style), NOT a live-process test: it guards the CALL SITE existing,
//! separately from `src/hub/role.rs`'s own `#[cfg(test)]` unit tests, which guard the assertion
//! LOGIC. Anchored the same way `tests/architecture.rs`'s `governance_dir()` is: join
//! `CARGO_MANIFEST_DIR` with the file's repo-relative path and read it directly, independent of
//! the current working directory.

use std::path::{Path, PathBuf};

fn repo_file(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// The governance chokepoint every transport calls (`transport::mcp::server::serve_session`)
/// must assert the SERVICE role as its first action (H3, PINS.md SS8).
#[test]
fn governance_chokepoint_asserts_service_role() {
    let path = repo_file("src/transport/mcp/server.rs");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        source.contains("assert_service_role"),
        "the governance chokepoint (serve_session, {}) must call assert_service_role",
        path.display()
    );
}
