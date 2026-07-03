# S04: host polarity evaluation in the browser plugin

## Goal

Implement ADR-0022 Decision 4 (per-grant host allow/deny lists with a pinned DENY default)
as a pure function in the browser plugin, plus its outcome type in the governance core.
Purely ADDITIVE: nothing consumes the new items until s05; behavior is unchanged.

## Authority

ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) is normative; Decision 4 defines
every semantic here. Where this prompt and the ADR disagree, THE ADR WINS; record the
deviation in the ledger per `docs/tasks/stage-3/BOOTSTRAP.md`.

## Depends on

- Nothing functionally: only the stage-2 tree (`src/browser/pattern.rs` and
  `src/governance/ports.rs`). It uses neither s02's `Capability` nor s03's directory.
  Sequence-wise it runs after s03 per BOOTSTRAP.md; if either file above is missing, STOP.
- s05 consumes this task's output as `fn(&str, &[String], &[String]) -> HostRuleOutcome`
  and `is_valid_host_rule(pattern)`; the signatures below are load-bearing for it.

## Current behavior (verify against the tree before editing)

- `src/browser/pattern.rs` owns the section 5.1 grammar. `is_valid_pattern(pattern: &str)
  -> bool` accepts exactly: an exact host (`example.com`, `127.0.0.1`) or a single leading
  `*.` wildcard (`*.example.com`), lowercase ASCII only. It REJECTS bare `*`, `*.`, partial
  globs (`site*`, `foo.*.com`, `**.example.com`), schemes, ports, paths, and userinfo.
- `pattern_matches_normalized_host(pattern: &str, host: &str) -> bool` takes an
  ALREADY-NORMALIZED host string, parses the pattern on each call, and returns `false`
  (never panics) when the pattern fails to parse. Consequence: bare `*` returns `false`
  from it today, so the polarity evaluator MUST special-case `*` before delegating.
- Matching semantics (pinned by pattern.rs inline tests): an exact pattern matches only the
  identical host; `*.suffix` matches strict subdomains at any depth on a label boundary
  (`h.ends_with(".suffix")`) and NEVER matches the apex (`*.example.com` does not match
  `example.com`; test `apex_does_not_match_wildcard_alone`) nor IP literals.
- `src/governance/ports.rs` holds the axis types (`RwClass`, `EffectiveMode`) at the top
  under the `// --- Supporting placeholder and axis types ---` banner. No host-polarity
  outcome type exists anywhere.
- `src/browser/mod.rs` declares seven modules: `advertise`, `classify`, `pattern`,
  `redact`, `resource`, `sacred`, `tools`. There is no `polarity` module.
- `tests/architecture.rs` scans the RAW TEXT (doc comments and string literals included) of
  every `.rs` file under `src/governance/` and fails on the path tokens `crate::browser`,
  `crate::transport`, `crate::mcp`, `crate::native`, or `url` crate references.
- `src/lib.rs` declares `pub mod browser;` and `pub mod governance;`, so new `pub` items
  are library API and trip no dead-code lints while unconsumed.

## Required behavior

### 1. `HostRuleOutcome` in `src/governance/ports.rs`

Insert immediately after the `EffectiveMode` impl block and before `ToolId`, verbatim:

    /// Outcome of evaluating one grant's host rules (`hosts.allow` / `hosts.deny`) against a
    /// normalized host (ADR-0022 Decision 4). `Unmatched` means the grant does not cover the
    /// host at all: the grant-level default is DENY (Decision 4 rule 1), so an unmatched
    /// grant simply never resolves the call. Produced by the domain plugin's polarity
    /// evaluator and consumed by enforcement (s05) through an injected function pointer.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HostRuleOutcome {
        /// An allow pattern is the winning match: the grant covers the host.
        Allowed,
        /// A deny pattern matched and won: the grant explicitly excludes the host.
        Denied,
        /// Neither list matched (or the allow list is empty): the grant does not cover the host.
        Unmatched,
    }

Do NOT add serde derives (the type crosses the boundary only via a `fn` pointer); the doc
comment is pinned above and deliberately names no `crate::browser` path (constraint 3).

### 2. New module `src/browser/polarity.rs`

Module doc comment must state, in this order: host polarity evaluation for schema-3 grants
(ADR-0022 Decision 4); `hosts.allow` grants coverage, `hosts.deny` carves holes out of it,
and the default is DENY (a host matched by neither list is `Unmatched`); the specificity
order (exact beats `*.suffix`, longer wildcard suffix beats shorter, `*` loses to
everything, exact tie goes to deny); the module is pure (no I/O, no grant walking --
composing grants in manifest order is enforcement's job, s05); the outcome type lives in
the core (`crate::governance::ports::HostRuleOutcome`) while this module is injected into
the core as a plain `fn` pointer, mirroring `pattern_matches_normalized_host`.

Public functions, signatures verbatim:

    pub fn is_valid_host_rule(pattern: &str) -> bool

Body exactly: `pattern == "*" || crate::browser::pattern::is_valid_pattern(pattern)`.
Doc comment must state: bare `"*"` is legal ONLY in schema-3 grant `hosts` lists (ADR-0022
Decision 4 rule 1: the explicit everything token), NEVER in
`content.security.sacred_domains`, whose validation keeps calling `is_valid_pattern`.

    pub fn evaluate_host(host: &str, allow: &[String], deny: &[String]) -> HostRuleOutcome

Doc comment must state: `host` is an ALREADY-NORMALIZED host string (callers pass hosts
produced for `GoverningResource::Resource` via `pattern::host_for_matching`); patterns were
validated at manifest load; invalid patterns never match (false, never a panic).

Semantics (ADR Decision 4 rules 1 and 2), implemented exactly as:

1. If `allow.is_empty()`, return `Unmatched` for every host (rule 1: deny only carves holes
   out of allow; a grant with only deny entries covers nothing).
2. A pattern matches when it is `"*"` (matches EVERY host, including IP literals; it is the
   universal token, not a domain wildcard) or when
   `crate::browser::pattern::pattern_matches_normalized_host(pattern, host)` is true.
3. Specificity encoding, pinned: `fn specificity(pattern: &str) -> (u8, usize)` returns
   `(0, 0)` for `"*"`; `(1, suffix.len())` for a `"*."`-prefixed pattern where `suffix` is
   the pattern with its two-byte `"*."` prefix stripped; `(2, pattern.len())` otherwise.
   Tuples compare with Rust's derived lexicographic `Ord`; larger is more specific.
4. Compute the best (maximum) specificity among matching patterns in `allow` and, separately,
   in `deny` (each an `Option<(u8, usize)>`). Then: `(None, None)` is `Unmatched`;
   `(Some(_), None)` is `Allowed`; `(None, Some(_))` is `Denied`; `(Some(a), Some(d))` is
   `Denied` when `d >= a`, else `Allowed`. The `>=` encodes rule 2's tie-to-deny; equal
   specificity between two matching patterns is only possible when they are the identical
   pattern (or `"*"` on both sides), since same-length suffixes of one host are identical.

Private helpers, pinned (reference implementation; keep names, signatures, and semantics;
let `cargo fmt` settle layout):

    fn rule_matches(pattern: &str, host: &str) -> bool {
        pattern == "*" || crate::browser::pattern::pattern_matches_normalized_host(pattern, host)
    }

    fn best_specificity(host: &str, patterns: &[String]) -> Option<(u8, usize)> {
        patterns
            .iter()
            .map(String::as_str)
            .filter(|p| rule_matches(p, host))
            .map(specificity)
            .max()
    }

Import `HostRuleOutcome` with `use crate::governance::ports::HostRuleOutcome;`.

### 3. Register the module in `src/browser/mod.rs`

- Add `pub mod polarity;` between `pub mod pattern;` and `pub mod redact;`.
- In the module doc comment, after the `pattern` clause (`...the WHATWG-parser-backed
  matcher),`) and before `the sacred never-touch list`, insert exactly:
  `the host-polarity evaluator ([`polarity`], ADR-0022 Decision 4: per-grant
  hosts.allow/hosts.deny evaluation over already-normalized hosts, consumed by grant
  enforcement from s05 on),` (rewrap the paragraph to the file's existing line width).

## Constraints

1. Touch ONLY these source files: `src/governance/ports.rs`, `src/browser/polarity.rs`
   (new), `src/browser/mod.rs` (plus the LEDGER.md update required by BOOTSTRAP.md). No
   changes to enforcement, dispatch, manifest, advertise, classify, resource, or sacred.
2. Pure: no I/O, no live state, no policy composition. `evaluate_host` must coerce to the
   `fn(&str, &[String], &[String]) -> HostRuleOutcome` pointer shape s05 injects.
3. `tests/architecture.rs` must stay green: the ports.rs addition must contain no
   `crate::browser` (or other forbidden) token, including in its doc comment.
4. Do not touch `src/transport/mcp/schemas/tools.json` or `tests/tool_schema_fidelity.rs`.
5. ASCII only in every touched file; no new dependencies.

## Tests (minimum)

Inline `#[cfg(test)] mod tests` at the bottom of `src/browser/polarity.rs` (house style of
`pattern.rs`). Hosts in tests are already-normalized lowercase strings passed directly.
Pin this helper: `fn rules(items: &[&str]) -> Vec<String> { items.iter().map(|s| s.to_string()).collect() }`.

The ADR's five canonical postures (transcribed verbatim from Decision 4) are the backbone:

    { "allow": ["site1.com", "site2.com"] }          allowlist: those two, nothing else
    { "allow": ["*"], "deny": ["site1.com"] }        denylist: everything except site1
    { "allow": ["*"] }                                everything
    { }  or  { "allow": [] }                          nothing
    { "deny": ["site1.com"] }                         nothing (deny carves from allow; no allow, nothing to carve)

Test names and cases, pinned (assert with `assert_eq!` against `HostRuleOutcome` variants):

- `allowlist_covers_only_listed_hosts` -- allow `["site1.com", "site2.com"]`, deny `[]`:
  `site1.com` Allowed; `site2.com` Allowed; `site3.com` Unmatched; `sub.site1.com`
  Unmatched (exact patterns never match subdomains).
- `star_allow_with_deny_carveout` -- allow `["*"]`, deny `["site1.com"]`: `site1.com`
  Denied; `site2.com` Allowed; `sub.site1.com` Allowed (an exact deny does not carve
  subdomains).
- `star_allow_alone_allows_everything` -- allow `["*"]`, deny `[]`: `site1.com` Allowed;
  `a.b.example.org` Allowed; `127.0.0.1` Allowed (`*` matches IP literals; `*.suffix` never does).
- `empty_rules_are_unmatched` -- allow `[]`, deny `[]` (covers both the `{}` and
  `{ "allow": [] }` postures): `site1.com` Unmatched; `example.com` Unmatched.
- `deny_only_is_unmatched_for_everything` -- allow `[]`, deny `["site1.com"]`: `site1.com`
  Unmatched; `site2.com` Unmatched (deny carves from allow; nothing to carve).
- `exact_allow_beats_star_deny` -- allow `["site1.com"]`, deny `["*"]`: `site1.com`
  Allowed; `site2.com` Denied.
- `exact_deny_beats_star_allow` -- allow `["*"]`, deny `["site1.com"]`: `site1.com` Denied.
- `longer_wildcard_beats_shorter` -- allow `["*.corp.example.com"]`, deny
  `["*.example.com"]`: `a.corp.example.com` Allowed (suffix `corp.example.com`, 16 bytes,
  beats `example.com`, 11 bytes); `b.example.com` Denied.
- `identical_pattern_in_both_lists_is_denied` -- allow `["site1.com"]`, deny
  `["site1.com"]`: `site1.com` Denied. Also allow `["*"]`, deny `["*"]`:
  `anything.example` Denied.
- `wildcard_never_matches_the_apex` -- allow `["*.example.com"]`, deny `[]`: `example.com`
  Unmatched; `sub.example.com` Allowed.
- `is_valid_host_rule_accepts_star_and_delegates_the_rest` -- true for `*`, `example.com`,
  `*.example.com`, `127.0.0.1`; false for the empty string, `*.`, `**.example.com`,
  `site*`, `*bank*`, `Example.com`, `https://example.com`, `example.com:8443`,
  `example.com/path`.

## Verification

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean; `cargo test`
  fully green including the eleven new polarity tests, with `tests/architecture.rs`,
  `tests/all_open_golden.rs`, `tests/mcp_protocol.rs`, and `tests/tool_schema_fidelity.rs`
  passing unmodified.
- `rg -n "HostRuleOutcome|evaluate_host|is_valid_host_rule" src/` hits only
  `src/governance/ports.rs` and `src/browser/polarity.rs` (proving the task is additive).
- ASCII scan: `rg -n "[^\x00-\x7F]" src/governance/ports.rs src/browser/polarity.rs
  src/browser/mod.rs` produces no output.
- `git status --short` shows only the three source files above plus the ledger.
- No browser check is needed (pure function); append nothing to BROWSER-TESTS.md.

## Out of scope

- Manifest wiring: the `HostRules` grant field, schema-3 parsing, and calling
  `is_valid_host_rule` from validation are s05, as are enforcement, dispatch,
  advertisement, simulate, and explain.
- Any change to `content.security.sacred_domains` semantics or validation. The sacred list
  never accepts bare `*` and its semantics are unchanged forever in this stage.
- Any change to `src/browser/pattern.rs` (grammar and matcher stay exactly as they are).
- Audit changes (s06), tools.json / fidelity test (s07), docs outside code comments (s08).
