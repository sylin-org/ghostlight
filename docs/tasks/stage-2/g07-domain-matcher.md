# G07: Domain pattern matcher with bypass-class tests

## Goal

Implement the pure domain pattern matcher described in
`docs/tasks/stage-2/00-shared-format.md` section 5: the pattern grammar (exact host and
single leading `*.` wildcard), host normalization (lowercase, trailing dot, IDN/punycode
A-label space, IP-literal canonicalization), port and scheme handling, and URL parsing
that is immune to the userinfo bypass class. `https://allowed.com@evil.com/` must match
patterns for `evil.com` and must never match patterns for `allowed.com`; this is the
CVE-2025-47241 class and it ships as named test cases. IP-literal forms, embedded
credentials, and malformed URLs are covered by tests. The module does no I/O and makes no
policy decision: it turns URLs into matchable hosts and answers "does this host match this
pattern?". Later stage-2 tasks consume it: G08 (sacred domains), G13 (grants), and G06
(the audit record's `domain` field).

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` (sections 4.3 grant resolution note, 5.1,
  5.2, 5.3, 6.1 `domain` field, 7.1 rule strings). Read it before writing any code; its
  names and semantics are authoritative.
- All release-1 (stage-1) tasks in `docs/tasks/release-1/` are assumed landed. No other
  stage-2 task is a prerequisite; G08 (sacred domains) and G13 (grants) depend on THIS
  task.

## Project context

Browser MCP is governed browser automation. A single Rust binary is both the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) and the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

Stage 1 (docs/tasks/release-1/) hardened the engine. Stage 2 is the governance layer per
ADR-0013 (separable overlay; all-open stays first-class), ADR-0018 (observe-then-enforce
sequencing), ADR-0019 (layered configuration and typed key registry), and ADR-0020 (org
policy experience). This task is ADR-0018 step 3 groundwork: every enforcement decision
that involves a domain (sacred domains, grant resolution, the `unmatched_domain` and
`scheme` denial rules, the audit `domain` field) needs one hardened answer to "what host
is this URL really pointing at, and does it match this pattern?". Prior art in this space
has been bypassed by exactly the tricks this module must be immune to: userinfo smuggling
(`https://allowed.com@evil.com/`), string-suffix stitching (`evilallowed.com`),
IP-literal respelling (`0x7f.0.0.1`), homoglyph IDN hosts, and trailing-dot variants.

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc `docs/tasks/stage-2/00-shared-format.md` is the reconciled single source for
formats and names; SPEC 4.2's wildcard rule is retained by it, and its section 5 adds
the normalization rules and required negative test classes that the SPEC lacks. Do not
derive behavior from the SPEC alone; sections 5.1-5.3 of the shared format doc plus this
prompt are the complete specification.

Key files for this task:

- `src/policy/mod.rs` -- the governance module. Declares `pub mod redact;` (and, if task
  G05 has landed, `pub mod classify;`). Holds the typed key registry seed (`KeyDef`,
  `KEYS`, `Config`). Your new module is declared here; nothing else in this file changes.
- `src/policy/redact.rs` -- existing overlay example (style reference only; do not
  modify).
- `src/origin.rs` -- doc-only placeholder for committed-origin tracking. Its module doc
  already states that the raw committed URL is the authoritative reported value and that
  "any canonicalized matching-key is overlay-only". This task builds that overlay-side
  canonicalization; do not modify `origin.rs`.
- `src/dispatch.rs` -- the single dispatch chokepoint; `policy_check` and `audit` are
  documented no-ops today. You do NOT touch this file.
- `Cargo.toml` -- gains exactly one dependency (see Required behavior, part 1).
- `src/mcp/schemas/tools.json` -- the SACRED tool schema fixture; read-only forever;
  irrelevant to this task except that it must show no diff.

## Current behavior

- No URL parsing or domain matching exists anywhere in the Rust source. Grep for
  `Url::parse`, `url::`, `host_str`, or `hostname` under `src/` finds nothing.
- `src/policy/` contains `mod.rs` (key registry seed: `KeyDef { key, description,
  minimal_default }`, the `KEYS` table with one entry for
  `content.security.secrets.redact`, and `Config` with `Config::minimal()`) and
  `redact.rs` (the read_page secret-value redaction overlay). If G05 has landed there is
  also `classify.rs`; leave it alone either way.
- `Cargo.toml` dependencies are exactly: `tokio`, `serde`, `serde_json` (with
  `preserve_order`), `clap`, `tracing`, `tracing-subscriber`, `thiserror`, `anyhow`,
  `dirs`, plus Windows-only `winreg` and `windows-sys`. There is no `url`, `uuid`,
  `chrono`, or `sha2` crate.
- The crate is a library (`src/lib.rs` declares `pub mod policy;` among others) with a
  thin binary shell (`src/main.rs`), so integration tests could import
  `browser_mcp::policy`; this task nevertheless uses inline unit tests per repo
  convention for pure functions.
- `src/mcp/server.rs` calls the no-op seams at lines 132-133 of `handle_tools_call`:
  `let _decision = dispatch::policy_check(name);` then `dispatch::audit(name);`. Every
  call is allowed; no domain is ever consulted.

## Required behavior

### 1. One new dependency: the `url` crate

Add to `[dependencies]` in `Cargo.toml`:

```toml
url = "2"
```

with a one-line ASCII comment above it: `# WHATWG URL parser -- domain matching must
never hand-roll URL/host parsing (CVE-2025-47241 class).`

Justification (this is the "strongest justification" the project's dependency rule
demands, and it is already settled; do not re-litigate it): the shared format doc
section 5.2 mandates that "matching operates on the HOST produced by a real,
WHATWG-compliant URL parser" and that "the matcher never substring-searches the raw URL
string". The `url` crate is the Rust reference implementation of the WHATWG URL Standard
(the Servo project's rust-url). Hand-rolled URL handling is the direct cause of the
userinfo bypass class this task exists to kill, and IDNA/punycode (UTS 46) conversion
requires Unicode tables that must not be hand-rolled. `url` is the ONLY new dependency
this task may add. Do not add `idna` directly (it arrives transitively), and do not add
`publicsuffix`, `addr`, `regex`, `glob`, or anything else.

### 2. New module `src/policy/domain.rs`

Create `src/policy/domain.rs` and declare it in `src/policy/mod.rs` by adding the single
line `pub mod domain;` among the existing `pub mod` declarations, in alphabetical order
(after `pub mod classify;` if G05 has landed, otherwise before `pub mod redact;`). That
one line is the ONLY change to `mod.rs`.

Module-level doc comment must state: this is the domain pattern language of the shared
format doc section 5 (ADR-0018 step 3); it is pure (no I/O, no policy decisions); hosts
are produced only by the WHATWG parser, never by substring inspection of raw URLs;
matching applies to FINAL URLs handed in by callers (redirect interception and
re-checking the current tab URL are enforcement concerns, not matcher concerns); the
`about:blank` always-allow carve-out is a caller-side policy rule (shared format doc
section 5.2), not implemented here; consumers are sacred domains (G08), grants (G13),
and the audit `domain` field (G06). Doc comments are prose; do not include fenced code
examples (no doctests to maintain).

### 3. Public API

Exactly these public items, each with a doc comment:

```rust
/// A parser-normalized host, safe to hand to `DomainPattern::matches`.
/// Constructible only via `host_for_matching`, so a raw URL string can never
/// be passed to the matcher by mistake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchHost(String); // field private

impl MatchHost {
    /// The normalized host as a string. This is the exact value the audit
    /// record's `domain` field carries (shared format doc section 6.1).
    pub fn as_str(&self) -> &str
}

/// Outcome of extracting a matchable host from a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostOutcome {
    /// An http(s) URL with a host, normalized for matching.
    Host(MatchHost),
    /// The URL parsed but its scheme is not http or https. Carries the
    /// lowercase scheme without the trailing colon (the exact token the
    /// `scheme/<scheme>` denial rule needs; shared format doc section 7.1).
    NonHttpScheme(String),
    /// The input is not a parseable absolute URL, or it has no usable host.
    /// Callers must fail closed on this variant.
    Unparseable,
}

/// Parse a URL with the WHATWG parser and extract the normalized host.
pub fn host_for_matching(url: &str) -> HostOutcome

/// A validated domain pattern: an exact host or a single leading `*.` wildcard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPattern { /* private fields */ }

impl DomainPattern {
    /// Validate and canonicalize a pattern per the section 5.1 grammar.
    pub fn parse(pattern: &str) -> Result<DomainPattern, PatternError>
    /// The canonical pattern string, e.g. "example.com" or "*.example.com".
    /// This is the token the `sacred/<pattern>` denial rule renders.
    pub fn as_str(&self) -> &str
    /// True for `*.suffix` patterns.
    pub fn is_wildcard(&self) -> bool
    /// Does this pattern match the given normalized host?
    pub fn matches(&self, host: &MatchHost) -> bool
}

/// First pattern in slice order that matches, or None. Slice order is
/// authoring order; first match wins (mirrors grant resolution, shared
/// format doc section 4.3).
pub fn first_match<'a>(patterns: &'a [DomainPattern], host: &MatchHost)
    -> Option<&'a DomainPattern>

/// Why a pattern failed validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PatternError {
    #[error("pattern is empty")]
    Empty,
    #[error("pattern contains non-ASCII characters; author IDN domains in punycode (A-label) form")]
    NonAscii,
    #[error("pattern must not contain a scheme")]
    HasScheme,
    #[error("pattern must not contain userinfo ('@')")]
    HasUserinfo,
    #[error("pattern must not contain a path, query, or fragment")]
    HasPath,
    #[error("pattern must not contain a port")]
    HasPort,
    #[error("'*' is only legal as a single leading '*.' label over a domain suffix")]
    BadWildcard,
    #[error("pattern is not a valid host: {0}")]
    InvalidHost(String),
}
```

Nothing else is public. No `Display`, `Serialize`, `Deserialize`, `FromStr`, or `TryFrom`
impls; `as_str` is the only string conversion until a consumer proves the need.

### 4. `host_for_matching` exact algorithm

1. `url::Url::parse(url)`; on error return `HostOutcome::Unparseable`.
2. If the parsed scheme is not `"http"` and not `"https"`, return
   `HostOutcome::NonHttpScheme(scheme.to_string())`. The `url` crate already lowercases
   schemes; do not re-lowercase.
3. Match on `parsed.host()` (the `url::Host` enum; do NOT use `host_str` and do NOT
   strip brackets by string surgery):
   - `None` -> `Unparseable`.
   - `Some(Host::Domain(d))` -> strip AT MOST ONE trailing `.` from `d`; if the result
     is empty or still ends with `.`, return `Unparseable` (fail closed); otherwise
     `Host(MatchHost(result))`. The parser already emits lowercase A-label (punycode)
     domains; do not re-lowercase and do not run IDNA yourself.
   - `Some(Host::Ipv4(a))` -> `Host(MatchHost(a.to_string()))` (canonical dotted
     decimal).
   - `Some(Host::Ipv6(a))` -> `Host(MatchHost(a.to_string()))` (canonical RFC 5952
     form, NO brackets; `std::net::Ipv6Addr`'s `Display` provides it).

The parser is what defeats the bypass classes: userinfo is consumed before the host
(`https://allowed.com@evil.com/` has host `evil.com`), IPv4 respellings (`0x7f.0.0.1`,
`2130706433`) normalize to `127.0.0.1`, and IDN input normalizes to A-label form. Trust
it; add no ad-hoc string checks on the raw URL.

### 5. `DomainPattern::parse` exact algorithm

Apply these checks in this exact order so the error variant for any given input is
deterministic:

1. If the input is empty -> `Empty`.
2. If any byte is non-ASCII -> `NonAscii`. (Section 5.1: patterns are authored in
   lowercase ASCII; IDN domains must be authored in punycode A-label form. Rejecting
   hard here keeps sacred-domain protection truthful: a pattern that could silently
   never match must not be accepted. The friendlier `policy explain` warning is a later
   task layered on this variant.)
3. ASCII-lowercase a working copy (section 5.2 case rule; `Allowed.COM` is accepted and
   stored as `allowed.com`).
4. If it contains `://` -> `HasScheme`.
5. If it contains `@` -> `HasUserinfo`.
6. If it contains `/`, `?`, or `#` -> `HasPath`.
7. Wildcard split: if it starts with `*.`, set `wildcard = true` and let `body` be the
   remainder; if `body` is empty or contains `*` -> `BadWildcard`. Otherwise, if the
   input contains `*` anywhere -> `BadWildcard` (covers bare `*`, `*.`, `ex*mple.com`,
   `foo.*.com`, `*.*.com`). Otherwise `wildcard = false` and `body` is the whole input.
8. Body validation and canonicalization:
   a. If NOT wildcard: strip one pair of surrounding square brackets from `body` when it
      both starts with `[` and ends with `]`; if the (possibly unbracketed) result
      parses as `std::net::Ipv6Addr`, this is an exact IPv6 pattern; canonical body is
      the `Ipv6Addr` `Display` form (no brackets). Skip to step 9. (Both `::1` and
      `[::1]` are accepted authored forms; canonical is `::1`.)
   b. If `body` contains `:` -> `HasPort`.
   c. Strip AT MOST ONE trailing `.` from `body`; if the result is empty ->
      `InvalidHost` carrying the original body.
   d. `url::Host::parse(body)` (NOT `parse_opaque`; `Host::parse` applies the WHATWG
      special-scheme host rules, the same rules `Url::parse` applies to http(s) hosts,
      so patterns and hosts canonicalize identically):
      - `Err(_)` -> `InvalidHost` carrying the body (covers interior whitespace,
        forbidden host code points, empty labels).
      - `Ok(Host::Domain(d))` -> if `d` is empty or ends with `.` -> `InvalidHost`;
        otherwise `d` is the canonical body.
      - `Ok(Host::Ipv4(a))` -> if wildcard -> `BadWildcard` (a wildcard over an IP
        literal could never match anything; accepting it would be a silent no-op
        protection, which the truthfulness rule forbids); otherwise canonical body is
        `a.to_string()`. (This also means an author who writes `0x7f.0.0.1` or
        `2130706433` gets the canonical `127.0.0.1` stored; the parser normalizes
        patterns exactly as it normalizes hosts.)
      - `Ok(Host::Ipv6(a))` -> if wildcard -> `BadWildcard`; otherwise canonical body is
        `a.to_string()`.
9. Store: the canonical pattern string is `"*."` + canonical body for wildcards, else
   the canonical body. `as_str` returns exactly this string.

### 6. `matches` exact semantics

Operating on `MatchHost` only (never a raw URL):

- Exact pattern: `host.as_str() == canonical_body`. Nothing else. A grant for
  `example.com` covers `example.com:8443` because the port never reaches the matcher
  (it is dropped by `host_for_matching`).
- Wildcard pattern `*.suffix`:
  - If the host parses as `std::net::Ipv4Addr` or `std::net::Ipv6Addr`, return `false`
    (section 5.2: wildcard patterns NEVER match IP literals; this match-time guard is
    kept even though parse-time already rejects wildcard-over-IP patterns; defense in
    depth).
  - Otherwise return true if and only if the host string ends with `"."` + suffix
    (string concatenation of a dot and the canonical suffix). The leading dot enforces
    the label boundary (`evilallowed.com` does not end with `.allowed.com`) and strict
    subdomain-ness (`allowed.com` does not end with `.allowed.com`), and it matches at
    any depth (`a.b.allowed.com` ends with `.allowed.com`).
- No regex, no glob crate, no per-call allocation requirements are imposed; a simple
  `ends_with` over `&str` is the expected shape. To cover a domain AND its subdomains a
  caller lists both patterns (`["example.com", "*.example.com"]`, section 5.1); the
  matcher must not merge these semantics.

`first_match` is a linear scan calling `matches`, returning the first hit.

### 7. Required unit tests

Inline `#[cfg(test)] mod tests` in `src/policy/domain.rs`. The section 5.3 table of
negative test classes ships here as NAMED tests; keep these exact test names so the
table rows are findable by name. Where a test needs a host, obtain it through
`host_for_matching` and unwrap the `Host` variant; never construct hosts by hand.

Positive semantics:

1. `exact_pattern_matches_only_the_exact_host`: pattern `allowed.com` matches the host
   of `https://allowed.com/`; does NOT match the host of `https://foo.allowed.com/`.
2. `wildcard_matches_strict_subdomains_at_any_depth`: `*.allowed.com` matches hosts of
   `https://foo.allowed.com/` and `https://a.b.allowed.com/`.
3. `case_and_port_are_normalized_away`: host of `HTTPS://ALLOWED.COM:8443/PATH` matches
   pattern `allowed.com`; `DomainPattern::parse("Allowed.COM")` succeeds and matches the
   host of `https://allowed.com/`.
4. `ip_literal_exact_patterns_match_canonically`: pattern `127.0.0.1` matches the host
   of `http://127.0.0.1:8080/`; patterns `::1` and `[::1]` both parse, both have
   `as_str() == "::1"`, and match the hosts of `http://[::1]/` and
   `http://[0:0:0:0:0:0:0:1]/` (canonical compression).

Bypass classes (each must FAIL to match patterns `allowed.com` and `*.allowed.com`
unless stated otherwise; assert through both patterns):

5. `userinfo_bypass_cve_2025_47241`: `https://allowed.com@evil.com/` yields
   `HostOutcome::Host` with `as_str() == "evil.com"`; it does NOT match `allowed.com`
   or `*.allowed.com`; it DOES match pattern `evil.com` (the host must match evil.com,
   never allowed.com).
6. `embedded_credentials_never_reach_matching`: `https://user:pass@evil.com/` and
   `https://allowed.com:token@evil.com/` both yield host `evil.com`; neither matches
   the allowed patterns.
7. `wildcard_never_matches_ip_literals`: hosts of `http://127.0.0.1/` and
   `http://[::1]/` do not match `*.allowed.com` (nor any wildcard); additionally
   `DomainPattern::parse("*.0.0.1")` and `DomainPattern::parse("*.127.0.0.1")` return
   `Err(PatternError::BadWildcard)`.
8. `ip_literal_alternate_forms_normalize_to_canonical`: `http://0x7f.0.0.1/` and
   `http://2130706433/` both yield `HostOutcome::Host` with `as_str() == "127.0.0.1"`
   and both match pattern `127.0.0.1` (this pins the WHATWG parser normalization; the
   shared format doc requires the test to pin the parser behavior either way, and with
   the `url` crate the behavior is normalization to `127.0.0.1`).
9. `trailing_dot_strips_without_creating_a_bypass`: host of `https://evil.com./` is
   `evil.com` and does not match `allowed.com`; positive twin: host of
   `https://allowed.com./` is `allowed.com` and MUST match pattern `allowed.com`;
   fail-closed twin: `https://allowed.com../` either yields `Unparseable` or yields a
   host that does NOT match `allowed.com` (assert the disjunction; only one trailing
   dot is ever stripped).
10. `punycode_homoglyph_does_not_match_ascii`: host of `https://xn--llowed-vx9c.com/`
    does not match `allowed.com` or `*.allowed.com`; the escaped-Unicode URL
    `"https://\u{0430}llowed.com/"` (Cyrillic a; keep the source ASCII via the escape)
    yields either `Unparseable` or a host that does not match either pattern (assert
    the disjunction; comparison happens in A-label space only); and
    `DomainPattern::parse("\u{0430}llowed.com")` returns
    `Err(PatternError::NonAscii)`.
11. `apex_does_not_match_wildcard_alone`: host of `https://allowed.com/` does not match
    `*.allowed.com` (the wildcard excludes the apex by definition; covering both takes
    both patterns).
12. `suffix_stitching_requires_a_label_boundary`: hosts of `https://evilallowed.com/`
    and `https://allowed.com.evil.com/` match neither `allowed.com` nor
    `*.allowed.com`.
13. `non_http_schemes_yield_no_matchable_host`: `file:///etc/passwd` ->
    `NonHttpScheme("file")`; `javascript:alert(1)` -> `NonHttpScheme("javascript")`;
    `chrome://settings/` -> `NonHttpScheme("chrome")`; `about:blank` ->
    `NonHttpScheme("about")`; `data:text/html,hi` -> `NonHttpScheme("data")`;
    `chrome-extension://abcdefghijklmnop/page.html` ->
    `NonHttpScheme("chrome-extension")`. (The always-allow carve-out for `about:blank`
    is the caller's rule, applied in enforcement, not here.)
14. `malformed_urls_fail_closed`: `""`, `"not a url"`, `"http://"`, `"http:///path"`,
    `"https://exa mple.com/"`, `"https://:8080/"`, and `"//no-scheme.example/"` all
    yield `HostOutcome::Unparseable`.

Grammar and helpers:

15. `pattern_grammar_rejections`, asserting the exact `Err` variant for each input:
    `""` -> `Empty`; `"https://example.com"` -> `HasScheme`;
    `"user@example.com"` -> `HasUserinfo`; `"example.com/path"` -> `HasPath`;
    `"example.com:443"` -> `HasPort`; `"*"` -> `BadWildcard`; `"*."` -> `BadWildcard`;
    `"ex*mple.com"` -> `BadWildcard`; `"foo.*.com"` -> `BadWildcard`;
    `"*.*.com"` -> `BadWildcard`; `"exa mple.com"` -> `InvalidHost(_)`;
    `"."` -> `InvalidHost(_)`.
16. `pattern_canonical_form_via_as_str`: `parse("Allowed.COM")` -> `"allowed.com"`;
    `parse("*.Allowed.COM")` -> `"*.allowed.com"`; `parse("example.com.")` ->
    `"example.com"`; `parse("[::1]")` -> `"::1"`; `parse("0x7f.0.0.1")` ->
    `"127.0.0.1"` (patterns canonicalize through the same parser as hosts). Also assert
    `is_wildcard()` is true for `*.allowed.com` and false for `allowed.com`.
17. `first_match_returns_the_first_hit_in_order`: with patterns
    `["a.example.com", "*.example.com", "example.com"]` in that order, the host of
    `https://a.example.com/` resolves to the `a.example.com` pattern (index 0, not the
    wildcard); the host of `https://b.example.com/` resolves to `*.example.com`; the
    host of `https://example.com/` resolves to `example.com`; the host of
    `https://other.org/` resolves to `None`.

The redirect row of the section 5.3 table (navigate to an allowed URL that 302s to
`https://evil.com/`, final-URL check denies and parks on `about:blank`) is NOT
implementable in a pure matcher and is explicitly deferred to the enforcement wiring
task; the module doc comment must say that matching applies to final URLs handed in by
callers.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or descriptions.
   `tests/tool_schema_fidelity.rs` must pass unchanged. This task does not touch the
   tool surface at all.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task changes no extension file.
3. All-open stays first-class: with no manifest and default config, behavior is
   byte-identical to today. This task guarantees that trivially by not wiring anything:
   only `Cargo.toml` (+ `Cargo.lock`), `src/policy/mod.rs` (one added line), and the new
   `src/policy/domain.rs` change. `src/dispatch.rs`, `src/mcp/server.rs`, and all tool
   code stay untouched.
4. ASCII only in ALL code, comments, and docs: no em-dashes, arrows, or curly quotes.
   Non-ASCII test inputs must be written as `\u{...}` escapes (see test 10).
5. The engine is truthful: patterns that could never match are rejected at parse time
   (`NonAscii`, wildcard-over-IP) rather than silently accepted; `Unparseable` is a
   distinct outcome callers must fail closed on, never a silent allow.
6. `url = "2"` is the ONLY new dependency. No `idna` (transitive only), `publicsuffix`,
   `addr`, `regex`, `glob`, `once_cell`, `lazy_static`, or map crates. Matching is
   hand-rolled string comparison over parser-normalized hosts.
7. Rust 2021 edition; `thiserror` for `PatternError`; doc comments on every public item
   (module, both structs, both enums and their variants, every method and function);
   `cargo fmt` clean; `cargo clippy --all-targets -- -D warnings` clean. Unit tests
   inline in `domain.rs`; no new integration test file.
8. Do NOT copy code from other projects; implement from the behavior described here.
9. The matcher never inspects the raw URL string for policy signals: no substring
   checks, no manual splitting on `@` or `//`, no scheme allowlists applied before
   parsing. All structure comes from `url::Url::parse` / `url::Host::parse`.
10. Use the shared format doc's vocabulary: "host" is the parser-normalized hostname;
    patterns are "exact" or "wildcard"; the scheme token has no trailing colon. Do not
    invent alternative names for concepts the doc already names.

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green, including:
   - all 17 named tests in `src/policy/domain.rs` listed above;
   - `tests/tool_schema_fidelity.rs` unchanged and passing;
   - every pre-existing test unchanged and passing.
4. `git status` / `git diff --stat` shows changes ONLY to: `Cargo.toml` (the `url`
   dependency and its comment), `Cargo.lock` (regenerated), `src/policy/mod.rs` (exactly
   one added line: `pub mod domain;`), and the new file `src/policy/domain.rs`.
   `src/mcp/schemas/tools.json` shows no diff.
5. Grep the changed files for non-ASCII bytes (for example
   `rg -n "[^\x00-\x7F]" src/policy/domain.rs src/policy/mod.rs Cargo.toml`); there must
   be none.

Build note: if `target/debug/browser-mcp.exe` is locked by a running MCP session, rename
it aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and
rebuild. No extension reload or MCP client restart is needed for this task since no
runtime behavior changes.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Enforcement or config wiring of any kind. Nothing is blocked, denied, or held.
  `PolicyDecision` in `src/dispatch.rs` keeps its single `Allow` variant; do not edit
  `src/dispatch.rs` or `src/mcp/server.rs` at all.
- Redirect interception or current-tab URL tracking. Matching applies to final URLs
  handed in by callers; the redirect negative-test row lands with enforcement (G13 and
  the navigate enforcement wiring), not here. Do not modify `src/origin.rs`.
- Sacred domains (G08): no `content.security.sacred_domains` `KeyDef` entry, no
  `Config` field, no registry changes of any kind, no `sacred/<pattern>` denial
  formatting. This module only provides `as_str` so G08 can render that token later.
- Grants (G13): no manifest parsing, no grant structs, no `access` mapping, no
  first-matching-grant resolution across grants. `first_match` operates on one flat
  pattern list only.
- Denial ids, denial messages, audit records, or rule strings (`unmatched_domain`,
  `scheme/<scheme>`, `sacred/<pattern>`); `HostOutcome::NonHttpScheme` merely carries
  the scheme token those later tasks will need.
- The `about:blank` always-allow carve-out: that is caller-side policy (shared format
  doc section 5.2); the matcher reports it as `NonHttpScheme("about")` like any other
  non-http scheme.
- `policy explain` lint warnings (non-ASCII pattern warning, bare-`write` grant lint):
  later tasks. Here non-ASCII patterns are a hard `PatternError::NonAscii`.
- Public-suffix-list logic, DNS resolution, percent-decoding utilities, or any network
  or filesystem I/O.
- Any change under `extension/`, `tests/`, or `docs/` (including the SPEC; the SPEC
  amendment for domain-matching detail is tracked in the shared format doc's "SPEC
  updates needed" list, item 11, and is a separate docs task).
- Any `Display`, `Serialize`, `Deserialize`, `FromStr`, or `TryFrom` impl, caching,
  interning, or precomputed lookup structures beyond plain struct fields.
