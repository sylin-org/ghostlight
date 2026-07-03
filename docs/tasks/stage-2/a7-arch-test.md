# A7: Fail-closed architecture-invariant test for the governance core

## Goal

Add one normal cargo integration test (`tests/architecture.rs`) that walks the
`src/governance/` source tree and FAILS the build if any file under it contains a forbidden
dependency edge: `crate::browser`, `crate::transport`, `crate::mcp`, `crate::native`, or any
reference to the `url` crate. The test is pure Rust over `std::fs` (read the directory tree,
scan file contents as text); it adds no dependency. It is the load-bearing, machine-checked
guarantee that the domain-agnostic core has no back-edges into the plugin (`browser/`) or the
infrastructure (`transport/`, `mcp/`, `native/`), which is exactly what makes the future
crate extraction of `governance/` a mechanical move rather than a hunt for hidden couplings.
Because it is a `#[test]`, `cargo test` (and therefore CI) runs it automatically; no extra
wiring is required. This task also adds a couple of unit assertions on the scanner itself, so
the guard is trusted to catch a real violation and to not cry wolf on clean code.

This task builds ONLY the arch-test. It does not add or change any port, config, audit, or
enforcement code, and it does not move any module.

## Depends on

- **A1 (module reorg).** The `src/governance/` directory MUST already exist (it is created by
  A1, which regroups the tree into `governance/` core, `browser/` plugin, and `transport/`
  infra). If `src/governance/` does not exist in your tree, A1 has not landed: STOP and do not
  invent a directory. The test asserts the directory exists and fails loudly if it is missing
  (this is deliberate, fail-closed behavior; see Required behavior part 3).
- `docs/tasks/stage-2/PLAN.md`, Phase A item A7 and the "Build into the architecture seams"
  principle. Read it before writing code.
- `docs/design/ghostlight-service-architecture.md` sections 3 (bounded contexts) and 4 (the
  seam trait sketches). The whole point of this test is to keep section 3's boundary honest:
  `governance/` is the relocatable, domain-agnostic core; `browser/` is the swappable plugin;
  the pure serializable `DecisionRequest` travels between them without dragging live browser
  state along.
- No other stage-2 task is a prerequisite, and no later task depends on this one for its
  types; A7 only observes the tree. Landing A7 early (right after A1) means every subsequent
  governance task is guarded from the first line it writes.

## Project context

Browser MCP is governed browser automation. A single Rust binary is both the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled on tokio, no MCP SDK crate) and the Chrome
native-messaging host; a thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

Stage 1 (`docs/tasks/release-1/`) hardened the engine and merged to `main`. Stage 2 is the
governance layer per ADR-0013 (separable overlay; all-open stays first-class), ADR-0018
(observe-then-enforce sequencing), ADR-0019 (layered typed config), and ADR-0021 (the seam
architecture). Stage 2 groups the source into three bounded contexts:

- `governance/` -- the domain-agnostic core (config registry, ports, the policy decision
  point, audit records). It knows nothing about browsers, CDP, MCP, or URLs. It is written so
  it can later be lifted into its own crate with no code change. Its ONLY inputs are the pure,
  serializable seam types (`DecisionRequest`, `Grant`, `GoverningResource`, `RwClass`,
  `EffectiveMode`, `Decision`, `Denial`).
- `browser/` -- the domain plugin: tool implementations, CDP-facing code, the concrete
  `DomainPolicy`/`ResourceResolver` impls, and the domain matcher. The `url` crate lives HERE
  and only here (G07).
- `transport/` -- infrastructure: `native/`, `mcp/`, and the `Browser` executor handle.

The seam contract (from the architecture doc; A2 defines these, this task never touches them,
it only forbids `governance/` from reaching around them):

```rust
// PURE + serializable decision (relocatable). This is the whole decision.
#[derive(Serialize, Deserialize)]
pub struct DecisionRequest { pub grants: Vec<Grant>, pub tool: String, pub rw: RwClass,
    pub resource: GoverningResource, pub mode: EffectiveMode }
pub enum Decision { Allow { grant_id: Option<String> }, Deny(Denial), ShadowDeny(Denial) }
pub trait PolicyDecisionPoint: Send + Sync { fn decide(&self, req: &DecisionRequest) -> Decision; }
// PLUGIN pure half travels WITH the decision (DomainPolicy); PLUGIN impure half stays PEP-side
// (ResourceResolver, async, needs live state, never relocates). governance/ depends on the
// TRAITS, never on the browser/ or transport/ impls.
```

All-open stays first-class and byte-identical: with no manifest and default config, every tool
result is exactly what stage 1 produced (STEP-0 short-circuit). This test is fully additive to
runtime behavior: it changes no production code and no tool output. It only compiles as a test.

## Current behavior

Verified against the working tree; trust this prose over any line numbers, which drift.

- `tests/` contains exactly three integration tests: `mcp_protocol.rs`, `peer_death.rs`,
  `tool_schema_fidelity.rs`. There is NO architecture or layering test today. `tests/architecture.rs`
  does not exist; you create it.
- The existing integration tests locate the built binary via `env!("CARGO_BIN_EXE_browser-mcp")`.
  This task instead needs `env!("CARGO_MANIFEST_DIR")` (the crate root, i.e. the directory that
  contains `Cargo.toml`) to find source files. Both env vars are set by cargo for integration
  tests; `CARGO_MANIFEST_DIR` is the robust anchor for reading source, independent of the
  current working directory the test is launched from.
- Post-A1 the source tree is grouped as `src/governance/`, `src/browser/`, `src/transport/`
  (plus `src/main.rs`, `src/lib.rs`, and whatever A1 left at the root). This task reads only
  `src/governance/` and its subdirectories, recursively.
- `Cargo.toml` already has `serde_json`, `thiserror`, `tokio`, `clap`, `tracing`. This task
  needs none of them and adds nothing: the scanner is `std::fs` plus byte comparisons. No
  `walkdir`, no `regex`, no `syn`.
- The tool schemas (`src/mcp/schemas/tools.json`) and `tests/tool_schema_fidelity.rs` are the
  sacred surface and are untouched here.

## Required behavior

Everything lands in one new file, `tests/architecture.rs`, with a module-level doc comment
explaining that it is the fail-closed guard on the `governance/` core boundary (per ADR-0021
and PLAN A7): `governance/` may never name `browser`, `transport`, `mcp`, `native`, or the
`url` crate, so the core stays relocatable. Keep the file ASCII only.

### 1. What counts as a violation (be precise)

A violation is any of these tokens appearing in the raw text of any `.rs` file under
`src/governance/` (recursively), on any line, whether in a `use` path, a path-qualified
reference, a macro, a doc comment, or a string literal:

- `crate::browser` -- matched as a path token: the preceding character (if any) is not an
  identifier character (`[A-Za-z0-9_]`), AND the character after `crate::browser` is not an
  identifier character. This flags `use crate::browser::Cdp;`, `crate::browser::foo()`, and
  `crate::browser;`, but NOT `crate::browser_stats::X` (a different, hypothetical top-level
  module whose name merely starts with `browser`) and NOT a longer crate name such as
  `mycrate::browser`.
- `crate::transport` -- same rule.
- `crate::mcp` -- same rule.
- `crate::native` -- same rule. Note the `crate::` prefix is what scopes this: a local
  variable named `native`, or the prose phrase "native messaging" in a comment, is NOT a
  violation, because neither is preceded by `crate::`.
- A reference to the `url` crate, matched as either:
  - `url::` appearing as a path token (the character before `url` is not an identifier
    character), which catches `use url::Url;`, `use url::{Host, Url};`, and
    `url::Url::parse(s)`; or
  - a bare crate import `use url` or `extern crate url` where the name terminates immediately
    (`use url;`, `use url ;`, `use url as u;`, `extern crate url;`).
  This deliberately does NOT flag identifiers that merely contain the letters `url`, such as
  `full_url`, `build_url()`, or a struct field named `url`, because none of those is `url::`
  as a leading path token or a bare `url` import.

Rationale for scanning raw text including comments and strings (not stripping them): the guard
is fail-closed, so it prefers to over-report. The invariant it enforces is stronger and simpler
than "no compiled dependency": the domain-agnostic core must not even NAME the plugin or the
infra, in code OR in prose. If a `governance/` doc comment needs to refer to the plugin, it
says "the domain plugin", not `crate::browser`. This eliminates the false-negative risk of a
comment-stripper hiding a real `use` that shares a line with a `//` inside a string literal.

The scanner is line-oriented (report the 1-based line number of each hit) so failures point at
the exact offending line.

### 2. The scanner (pure, testable, ASCII)

Implement the scanner as free functions operating on `&str`, so the unit assertions in part 4
can exercise them directly on synthetic input without touching the filesystem.

```rust
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
```

### 3. Locating the governance tree and walking it (robust, fail-closed)

```rust
use std::fs;
use std::path::{Path, PathBuf};

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
```

The main test:

```rust
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
                violations.push(format!("{}:{}: forbidden edge `{}`", file.display(), idx + 1, edge));
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
```

Two fail-closed properties are load-bearing and MUST be kept:

- A missing `src/governance/` directory FAILS the test (it does not skip). A silently-absent
  core would make the guard vacuous.
- An empty `src/governance/` (zero `.rs` files) FAILS the test, for the same reason: a scan
  with nothing to scan must not report success.

### 4. Unit assertions on the scanner itself

Add these `#[test]` functions in the same file (they are plain integration-test functions;
no `#[cfg(test)]` module is needed in a `tests/*.rs` file). They prove the scanner catches a
real violation and does not false-positive on clean code.

```rust
#[test]
fn scanner_detects_forbidden_crate_edges() {
    assert_eq!(scan_line("use crate::browser::Cdp;"), vec!["crate::browser".to_string()]);
    assert_eq!(
        scan_line("    let h = crate::transport::Handle::new();"),
        vec!["crate::transport".to_string()]
    );
    assert_eq!(scan_line("use crate::mcp::types::Foo;"), vec!["crate::mcp".to_string()]);
    assert_eq!(scan_line("crate::native::host::send();"), vec!["crate::native".to_string()]);
}

#[test]
fn scanner_detects_url_crate_reference() {
    assert_eq!(scan_line("use url::Url;"), vec!["url".to_string()]);
    assert_eq!(scan_line("let u = url::Url::parse(s)?;"), vec!["url".to_string()]);
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
```

## Constraints

1. ASCII only in all code and docs: no em-dashes, no arrows, no curly quotes, anywhere
   (comments, tests, strings). Use Rust `\u{..}` escapes if a test ever needs a non-ASCII
   input (this task does not).
2. All-open stays first-class and byte-identical: with no manifest and default config, every
   tool result is exactly what stage 1 produced (STEP-0 short-circuit). This task adds only a
   test; it changes no production code and no tool output.
3. NEVER modify the tool schemas (`src/mcp/schemas/tools.json`), tool names, params, or
   descriptions; `tests/tool_schema_fidelity.rs` must pass unchanged (ADR-0007, the sacred
   surface).
4. The extension holds mechanism only; no policy/access/redaction decisions in extension JS.
   This task touches no extension file.
5. Rust 2021, `thiserror` for typed errors, doc comments on all public items and modules,
   rustfmt clean, `cargo clippy --all-targets -- -D warnings` clean. (The scanner functions in
   an integration test file are private to that file; still give each a doc comment, and keep
   the file's module doc comment.)
6. One task = one commit (code + tests + ledger/browser-test updates). Keep the tree green
   between tasks (full suite + clippy + fmt).
7. Windows dev gotcha: if `target/debug/browser-mcp.exe` is locked by a running session, rename
   it aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and rebuild,
   or stop the MCP client first.

Task-specific:

8. No new dependency and no `Cargo.toml` change. The scanner is `std::fs` plus byte
   comparisons only. Do NOT add `walkdir`, `glob`, `regex`, `syn`, or any crate.
9. Scan ONLY `src/governance/` (recursively). Do not scan `browser/`, `transport/`, `mcp/`,
   `native/`, `tools/`, `main.rs`, or `lib.rs`; those modules legitimately name each other and
   the `url` crate. Enforcing the rule elsewhere is out of scope.
10. Keep the fail-closed properties: a missing or empty `src/governance/` fails the test, never
    passes vacuously.
11. Anchor all paths on `env!("CARGO_MANIFEST_DIR")`, never on the current working directory or
    a relative path, so the test passes regardless of where cargo launches it from.
12. Do not modify any file other than the new `tests/architecture.rs` (and the ledger /
    browser-test docs the commit convention requires). In particular do not touch
    `src/governance/**` to make the test pass; if it fails on the reorged tree, the fix belongs
    to whichever governance task introduced the edge, not here.

## Verification

1. `cargo fmt` then `cargo clippy --all-targets -- -D warnings` from the repo root: clean.
2. `cargo test` from the repo root: all tests pass, including the three new tests in
   `tests/architecture.rs` (`governance_core_has_no_forbidden_back_edges`,
   `scanner_detects_forbidden_crate_edges`, `scanner_detects_url_crate_reference`,
   `scanner_ignores_clean_lines`), and the existing `tests/tool_schema_fidelity.rs`,
   `tests/mcp_protocol.rs`, `tests/peer_death.rs` unchanged.
3. `cargo test --test architecture` runs just this file and passes.
4. Negative check (prove the guard actually bites, then REVERT): temporarily add a line
   `use crate::browser::Foo;` to any `.rs` file under `src/governance/`, run
   `cargo test --test architecture`, and confirm
   `governance_core_has_no_forbidden_back_edges` FAILS with a message naming that file, its
   line number, and `crate::browser`. Remove the temporary line and confirm the test is green
   again. Do not commit the temporary edit.
5. Robustness check: run `cargo test --test architecture` from a subdirectory (for example
   `cd src && cargo test --test architecture`) to confirm the `CARGO_MANIFEST_DIR` anchor makes
   the test independent of the working directory. (If `target/debug/browser-mcp.exe` is locked,
   rename it aside per constraint 7.)

## Out of scope

- Enforcing the no-back-edges rule on any module other than `governance/`. `browser/`,
  `transport/`, `mcp/`, `native/`, and `tools/` are allowed to depend on each other and on the
  `url` crate; only the core is guarded.
- AST-level analysis. This is a text scan, not a `syn`/`rustc` pass. It does not resolve
  re-exports, `pub use` chains, type aliases, or transitive edges, and it does not need to: the
  invariant is the simpler "the core does not NAME these", which a text scan enforces exactly.
- Comment/string stripping or any attempt to distinguish code from prose. Scanning raw lines is
  intentional and fail-closed (see Required behavior part 1); a `governance/` comment must not
  spell `crate::browser` any more than the code may.
- Catching `super::`/`self::` escapes, cross-module leakage other than the five listed edges,
  or a general layering linter. Only the five named edges (`crate::browser`, `crate::transport`,
  `crate::mcp`, `crate::native`, and the `url` crate) are in scope.
- Defining or modifying any port, config key, audit record, or enforcement logic (A2-A6 and the
  g-docs). This task only observes the tree.
- Any `Cargo.toml`, CI-config, or workflow-file change. The test is a normal `#[test]`, so the
  existing `cargo test` invocation in CI already covers it; no new CI job is added.
