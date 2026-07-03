//! Domain pattern syntax and matching (browser plugin; RECONCILIATION.md section 1, g07).
//!
//! This module owns the domain-pattern grammar of the shared format doc section 5.1: an exact
//! host (`example.com`) or a single leading `*.` wildcard (`*.example.com`). It has two halves:
//! the SYNTACTIC checker ([`is_valid_pattern`]), used to validate authored patterns (the
//! `content.security.sacred_domains` governance key, and later manifest grant domains); and the
//! MATCHING half ([`host_for_matching`], [`DomainPattern`], [`first_match`]) -- host
//! normalization via the WHATWG `url` crate, wildcard matching, and immunity to the section 5.3
//! bypass classes (userinfo smuggling, IP-literal respelling, punycode homoglyphs, suffix
//! stitching). The matcher is pure: no I/O, no policy decisions. It turns URLs into matchable
//! hosts and answers "does this host match this pattern?"; consumers (sacred domains, grants,
//! the audit record's `domain` field) decide what to do with that answer. Matching applies to
//! FINAL URLs handed in by callers -- redirect interception and re-checking the current tab URL
//! are enforcement concerns, not matcher concerns; the `about:blank` always-allow carve-out is
//! likewise a caller-side policy rule (shared format doc section 5.2), reported here as an
//! ordinary [`HostOutcome::NonHttpScheme`].
//!
//! This lives in the browser plugin, not the governance core: the governance registry
//! ([`crate::governance::config`]) constrains `content.security.sacred_domains` values to
//! valid patterns, but validates them through an injected function pointer rather than naming
//! this module directly, so the core never depends on the plugin (the a7 arch-test). The `url`
//! crate is a dependency of this module only, for the same reason (RECONCILIATION.md section 1:
//! "the `url` crate lives ONLY here").

use std::net::Ipv6Addr;

/// True when `pattern` is a syntactically valid domain pattern (shared format doc 5.1): an
/// exact host (`example.com`, `127.0.0.1`) or a single leading `*.` wildcard
/// (`*.example.com`). Lowercase ASCII only; IDN domains must be authored in punycode (A-label)
/// form. IPv6-literal patterns are not accepted by this syntactic check.
pub fn is_valid_pattern(pattern: &str) -> bool {
    if pattern.is_empty() || !pattern.is_ascii() {
        return false;
    }

    let host = match pattern.strip_prefix("*.") {
        // A `*` anywhere else (bare `*`, `*.` with an empty remainder, `**.example.com` which
        // leaves `*.example.com` containing `*`, or `foo.*.com`) is invalid.
        Some(rest) if !rest.is_empty() && !rest.contains('*') => rest,
        Some(_) => return false,
        None if pattern.contains('*') => return false,
        None => pattern,
    };

    // One or more labels separated by single `.` characters: no leading dot, no trailing dot,
    // no empty label.
    if host.starts_with('.') || host.ends_with('.') {
        return false;
    }

    host.split('.').all(is_valid_label)
}

/// A single label is 1 to 63 characters, each one of `a-z`, `0-9`, or `-`, and the label
/// neither starts nor ends with `-`. Uppercase ASCII letters are invalid (patterns are
/// authored lowercase). This grammar rejects schemes, ports, paths, userinfo, and whitespace
/// by construction, and naturally accepts IPv4 dotted literals such as `127.0.0.1` (digits are
/// valid label characters).
fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A parser-normalized host, safe to hand to [`DomainPattern::matches`]. Constructible only via
/// [`host_for_matching`], so a raw URL string can never be passed to the matcher by mistake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchHost(String);

impl MatchHost {
    /// The normalized host as a string. This is the exact value the audit record's `domain`
    /// field carries (shared format doc section 6.1).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Outcome of extracting a matchable host from a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostOutcome {
    /// An http(s) URL with a host, normalized for matching.
    Host(MatchHost),
    /// The URL parsed but its scheme is not http or https. Carries the lowercase scheme
    /// without the trailing colon (the exact token the `scheme/<scheme>` denial rule needs;
    /// shared format doc section 7.1).
    NonHttpScheme(String),
    /// The input is not a parseable absolute URL, or it has no usable host. Callers must fail
    /// closed on this variant.
    Unparseable,
}

/// Parse a URL with the WHATWG parser and extract the normalized host (shared format doc
/// section 5.2). This is what defeats the bypass classes: userinfo is consumed before the host
/// (`https://allowed.com@evil.com/` has host `evil.com`), IPv4 respellings (`0x7f.0.0.1`,
/// `2130706433`) normalize to `127.0.0.1`, and IDN input normalizes to A-label form. No ad-hoc
/// string inspection of the raw URL is ever performed; all structure comes from `url::Url`.
pub fn host_for_matching(url: &str) -> HostOutcome {
    let parsed = match url::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return HostOutcome::Unparseable,
    };

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return HostOutcome::NonHttpScheme(scheme.to_string());
    }

    match parsed.host() {
        None => HostOutcome::Unparseable,
        Some(url::Host::Domain(d)) => {
            // At most one trailing dot is ever stripped; a host that still ends in `.`
            // afterward (a double-trailing-dot input) fails closed rather than silently
            // collapsing further dots.
            let stripped = d.strip_suffix('.').unwrap_or(d);
            if stripped.is_empty() || stripped.ends_with('.') {
                HostOutcome::Unparseable
            } else {
                HostOutcome::Host(MatchHost(stripped.to_string()))
            }
        }
        Some(url::Host::Ipv4(a)) => HostOutcome::Host(MatchHost(a.to_string())),
        Some(url::Host::Ipv6(a)) => HostOutcome::Host(MatchHost(a.to_string())),
    }
}

/// A validated domain pattern: an exact host or a single leading `*.` wildcard (shared format
/// doc section 5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPattern {
    wildcard: bool,
    /// The full canonical pattern string returned by [`Self::as_str`], e.g. `"example.com"` or
    /// `"*.example.com"`. For a wildcard pattern the suffix used by [`Self::matches`] is this
    /// string with its two-byte `"*."` prefix stripped.
    canonical: String,
}

impl DomainPattern {
    /// Validate and canonicalize a pattern per the section 5.1 grammar. Checks are applied in a
    /// fixed order so the error variant for any given input is deterministic.
    pub fn parse(pattern: &str) -> Result<DomainPattern, PatternError> {
        if pattern.is_empty() {
            return Err(PatternError::Empty);
        }
        // Patterns are authored in lowercase ASCII; IDN domains must be authored in punycode
        // (A-label) form. Rejecting non-ASCII hard here keeps sacred-domain protection
        // truthful: a pattern that could silently never match must not be accepted.
        if !pattern.is_ascii() {
            return Err(PatternError::NonAscii);
        }
        let lower = pattern.to_ascii_lowercase();
        if lower.contains("://") {
            return Err(PatternError::HasScheme);
        }
        if lower.contains('@') {
            return Err(PatternError::HasUserinfo);
        }
        if lower.contains('/') || lower.contains('?') || lower.contains('#') {
            return Err(PatternError::HasPath);
        }

        let (wildcard, body) = match lower.strip_prefix("*.") {
            Some(rest) => {
                if rest.is_empty() || rest.contains('*') {
                    return Err(PatternError::BadWildcard);
                }
                (true, rest)
            }
            None => {
                if lower.contains('*') {
                    return Err(PatternError::BadWildcard);
                }
                (false, lower.as_str())
            }
        };

        let canonical_body = canonicalize_body(body, wildcard)?;
        let canonical = if wildcard {
            format!("*.{canonical_body}")
        } else {
            canonical_body
        };
        Ok(DomainPattern {
            wildcard,
            canonical,
        })
    }

    /// The canonical pattern string, e.g. `"example.com"` or `"*.example.com"`. This is the
    /// token the `sacred/<pattern>` denial rule renders.
    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    /// True for `*.suffix` patterns.
    pub fn is_wildcard(&self) -> bool {
        self.wildcard
    }

    /// Does this pattern match the given normalized host?
    pub fn matches(&self, host: &MatchHost) -> bool {
        let h = host.as_str();
        if !self.wildcard {
            return h == self.canonical;
        }
        // Wildcard patterns never match IP literals (section 5.2); kept as a match-time guard
        // even though parse-time already rejects wildcard-over-IP patterns (defense in depth).
        if h.parse::<std::net::Ipv4Addr>().is_ok() || h.parse::<Ipv6Addr>().is_ok() {
            return false;
        }
        let suffix = &self.canonical[2..]; // strip the leading "*."
        h.ends_with(&format!(".{suffix}"))
    }
}

/// Validate and canonicalize the body of a pattern (the part after any `*.` wildcard prefix has
/// already been split off). `wildcard` gates the two wildcard-specific rules: no bracketed/bare
/// IPv6 literal check (a wildcard body is never bracket-stripped), and IP-literal bodies are
/// rejected outright (`BadWildcard`) since a wildcard over an IP literal could never match
/// anything.
fn canonicalize_body(body: &str, wildcard: bool) -> Result<String, PatternError> {
    if !wildcard {
        let unbracketed = body
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .unwrap_or(body);
        if let Ok(v6) = unbracketed.parse::<Ipv6Addr>() {
            return Ok(v6.to_string());
        }
    }
    if body.contains(':') {
        return Err(PatternError::HasPort);
    }
    let stripped = body.strip_suffix('.').unwrap_or(body);
    if stripped.is_empty() {
        return Err(PatternError::InvalidHost(body.to_string()));
    }
    match url::Host::parse(stripped) {
        Err(_) => Err(PatternError::InvalidHost(body.to_string())),
        Ok(url::Host::Domain(d)) => {
            if d.is_empty() || d.ends_with('.') {
                Err(PatternError::InvalidHost(body.to_string()))
            } else {
                Ok(d)
            }
        }
        Ok(url::Host::Ipv4(a)) => {
            if wildcard {
                Err(PatternError::BadWildcard)
            } else {
                Ok(a.to_string())
            }
        }
        Ok(url::Host::Ipv6(a)) => {
            if wildcard {
                Err(PatternError::BadWildcard)
            } else {
                Ok(a.to_string())
            }
        }
    }
}

/// First pattern in slice order that matches, or `None`. Slice order is authoring order; first
/// match wins (mirrors grant resolution, shared format doc section 4.3).
pub fn first_match<'a>(
    patterns: &'a [DomainPattern],
    host: &MatchHost,
) -> Option<&'a DomainPattern> {
    patterns.iter().find(|p| p.matches(host))
}

/// Whether `pattern` (an already-validated grant or sacred-domain pattern string) matches
/// `host` (an ALREADY-NORMALIZED host string, e.g. from
/// [`crate::governance::ports::GoverningResource::Resource`]). Unlike [`first_match`]/
/// [`DomainPattern::matches`], this takes a raw host string directly rather than a
/// [`MatchHost`]: the governance core cannot hold or construct browser-specific types across
/// its port boundary (the a7 arch-test), so [`crate::governance::enforcement::check_call`]
/// consumes matching through this function, injected as a plain `fn` pointer. `host` is
/// trusted to already be parser-normalized -- it was, by whoever produced the
/// `GoverningResource` in the first place, via [`host_for_matching`]. Returns `false` (never
/// panics) if `pattern` fails to parse; a `Grant`'s domains are validated at manifest-load
/// time, so this should not happen in practice.
pub fn pattern_matches_normalized_host(pattern: &str, host: &str) -> bool {
    DomainPattern::parse(pattern)
        .map(|dp| dp.matches(&MatchHost(host.to_string())))
        .unwrap_or(false)
}

/// Why a pattern failed validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PatternError {
    #[error("pattern is empty")]
    Empty,
    #[error(
        "pattern contains non-ASCII characters; author IDN domains in punycode (A-label) form"
    )]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_patterns() {
        for p in [
            "example.com",
            "*.example.com",
            "localhost",
            "127.0.0.1",
            "a-b.example.com",
            "xn--pple-43d.com",
        ] {
            assert!(is_valid_pattern(p), "{p} should be valid");
        }
    }

    #[test]
    fn invalid_patterns() {
        let sixty_four_char_label = "a".repeat(64);
        let cases: Vec<String> = vec![
            "".to_string(),
            "*".to_string(),
            "*.".to_string(),
            "**.example.com".to_string(),
            "foo.*.com".to_string(),
            "Example.com".to_string(),
            "https://example.com".to_string(),
            "example.com/path".to_string(),
            "example.com:8443".to_string(),
            "user@example.com".to_string(),
            ".example.com".to_string(),
            "example.com.".to_string(),
            "example..com".to_string(),
            "-foo.example.com".to_string(),
            "foo-.example.com".to_string(),
            sixty_four_char_label,
            "b\u{fc}cher.de".to_string(),
        ];
        for p in cases {
            assert!(!is_valid_pattern(&p), "{p} should be invalid");
        }
    }

    /// Extract the matched host from a URL, panicking if it is not a plain `Host` outcome.
    /// Never construct a `MatchHost` by hand in a test; always go through the real parser.
    fn host_of(url: &str) -> MatchHost {
        match host_for_matching(url) {
            HostOutcome::Host(h) => h,
            other => panic!("expected a matchable host for {url}, got {other:?}"),
        }
    }

    fn pat(s: &str) -> DomainPattern {
        DomainPattern::parse(s).unwrap_or_else(|e| panic!("{s} should parse: {e}"))
    }

    #[test]
    fn exact_pattern_matches_only_the_exact_host() {
        let p = pat("allowed.com");
        assert!(p.matches(&host_of("https://allowed.com/")));
        assert!(!p.matches(&host_of("https://foo.allowed.com/")));
    }

    #[test]
    fn wildcard_matches_strict_subdomains_at_any_depth() {
        let p = pat("*.allowed.com");
        assert!(p.matches(&host_of("https://foo.allowed.com/")));
        assert!(p.matches(&host_of("https://a.b.allowed.com/")));
    }

    #[test]
    fn case_and_port_are_normalized_away() {
        let p = pat("allowed.com");
        assert!(p.matches(&host_of("HTTPS://ALLOWED.COM:8443/PATH")));
        let p2 = DomainPattern::parse("Allowed.COM").expect("parses");
        assert!(p2.matches(&host_of("https://allowed.com/")));
    }

    #[test]
    fn ip_literal_exact_patterns_match_canonically() {
        assert!(pat("127.0.0.1").matches(&host_of("http://127.0.0.1:8080/")));

        let p1 = DomainPattern::parse("::1").expect("parses");
        let p2 = DomainPattern::parse("[::1]").expect("parses");
        assert_eq!(p1.as_str(), "::1");
        assert_eq!(p2.as_str(), "::1");
        assert!(p1.matches(&host_of("http://[::1]/")));
        assert!(p1.matches(&host_of("http://[0:0:0:0:0:0:0:1]/")));
        assert!(p2.matches(&host_of("http://[::1]/")));
    }

    #[test]
    fn userinfo_bypass_cve_2025_47241() {
        let host = match host_for_matching("https://allowed.com@evil.com/") {
            HostOutcome::Host(h) => h,
            other => panic!("expected a host, got {other:?}"),
        };
        assert_eq!(host.as_str(), "evil.com");
        assert!(!pat("allowed.com").matches(&host));
        assert!(!pat("*.allowed.com").matches(&host));
        assert!(pat("evil.com").matches(&host));
    }

    #[test]
    fn embedded_credentials_never_reach_matching() {
        for url in [
            "https://user:pass@evil.com/",
            "https://allowed.com:token@evil.com/",
        ] {
            let host = host_of(url);
            assert_eq!(host.as_str(), "evil.com", "url: {url}");
            assert!(!pat("allowed.com").matches(&host), "url: {url}");
            assert!(!pat("*.allowed.com").matches(&host), "url: {url}");
        }
    }

    #[test]
    fn wildcard_never_matches_ip_literals() {
        let wildcard = pat("*.allowed.com");
        assert!(!wildcard.matches(&host_of("http://127.0.0.1/")));
        assert!(!wildcard.matches(&host_of("http://[::1]/")));
        assert!(matches!(
            DomainPattern::parse("*.0.0.1"),
            Err(PatternError::BadWildcard)
        ));
        assert!(matches!(
            DomainPattern::parse("*.127.0.0.1"),
            Err(PatternError::BadWildcard)
        ));
    }

    #[test]
    fn ip_literal_alternate_forms_normalize_to_canonical() {
        for url in ["http://0x7f.0.0.1/", "http://2130706433/"] {
            let host = host_of(url);
            assert_eq!(host.as_str(), "127.0.0.1", "url: {url}");
            assert!(pat("127.0.0.1").matches(&host), "url: {url}");
        }
    }

    #[test]
    fn trailing_dot_strips_without_creating_a_bypass() {
        let evil = host_of("https://evil.com./");
        assert_eq!(evil.as_str(), "evil.com");
        assert!(!pat("allowed.com").matches(&evil));

        let allowed = host_of("https://allowed.com./");
        assert_eq!(allowed.as_str(), "allowed.com");
        assert!(pat("allowed.com").matches(&allowed));

        match host_for_matching("https://allowed.com../") {
            HostOutcome::Unparseable => {}
            HostOutcome::Host(h) => assert!(!pat("allowed.com").matches(&h)),
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn punycode_homoglyph_does_not_match_ascii() {
        let punycode_host = host_of("https://xn--llowed-vx9c.com/");
        assert!(!pat("allowed.com").matches(&punycode_host));
        assert!(!pat("*.allowed.com").matches(&punycode_host));

        match host_for_matching("https://\u{0430}llowed.com/") {
            HostOutcome::Unparseable => {}
            HostOutcome::Host(h) => {
                assert!(!pat("allowed.com").matches(&h));
                assert!(!pat("*.allowed.com").matches(&h));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }

        assert!(matches!(
            DomainPattern::parse("\u{0430}llowed.com"),
            Err(PatternError::NonAscii)
        ));
    }

    #[test]
    fn apex_does_not_match_wildcard_alone() {
        assert!(!pat("*.allowed.com").matches(&host_of("https://allowed.com/")));
    }

    #[test]
    fn suffix_stitching_requires_a_label_boundary() {
        for url in ["https://evilallowed.com/", "https://allowed.com.evil.com/"] {
            let host = host_of(url);
            assert!(!pat("allowed.com").matches(&host), "url: {url}");
            assert!(!pat("*.allowed.com").matches(&host), "url: {url}");
        }
    }

    #[test]
    fn non_http_schemes_yield_no_matchable_host() {
        let cases = [
            ("file:///etc/passwd", "file"),
            ("javascript:alert(1)", "javascript"),
            ("chrome://settings/", "chrome"),
            ("about:blank", "about"),
            ("data:text/html,hi", "data"),
            (
                "chrome-extension://abcdefghijklmnop/page.html",
                "chrome-extension",
            ),
        ];
        for (url, scheme) in cases {
            assert_eq!(
                host_for_matching(url),
                HostOutcome::NonHttpScheme(scheme.to_string()),
                "url: {url}"
            );
        }
    }

    #[test]
    fn malformed_urls_fail_closed() {
        // `"http:///path"` is deliberately excluded from this list (it appears in the g07 task
        // doc's example set): verified directly against the real `url` crate (2.5.x), the WHATWG
        // "special authority slashes" state slurps the redundant extra slash and parses this as
        // `http://path` (host "path", path "/"), not as a malformed/empty-host URL. That is
        // correct WHATWG behavior, not a bug in `host_for_matching`, so this input is not
        // actually malformed and does not belong in a fail-closed test.
        for url in [
            "",
            "not a url",
            "http://",
            "https://exa mple.com/",
            "https://:8080/",
            "//no-scheme.example/",
        ] {
            assert_eq!(
                host_for_matching(url),
                HostOutcome::Unparseable,
                "url: {url}"
            );
        }
    }

    #[test]
    fn pattern_grammar_rejections() {
        assert!(matches!(DomainPattern::parse(""), Err(PatternError::Empty)));
        assert!(matches!(
            DomainPattern::parse("https://example.com"),
            Err(PatternError::HasScheme)
        ));
        assert!(matches!(
            DomainPattern::parse("user@example.com"),
            Err(PatternError::HasUserinfo)
        ));
        assert!(matches!(
            DomainPattern::parse("example.com/path"),
            Err(PatternError::HasPath)
        ));
        assert!(matches!(
            DomainPattern::parse("example.com:443"),
            Err(PatternError::HasPort)
        ));
        for bad_wildcard in ["*", "*.", "ex*mple.com", "foo.*.com", "*.*.com"] {
            assert!(
                matches!(
                    DomainPattern::parse(bad_wildcard),
                    Err(PatternError::BadWildcard)
                ),
                "{bad_wildcard} should be BadWildcard"
            );
        }
        assert!(matches!(
            DomainPattern::parse("exa mple.com"),
            Err(PatternError::InvalidHost(_))
        ));
        assert!(matches!(
            DomainPattern::parse("."),
            Err(PatternError::InvalidHost(_))
        ));
    }

    #[test]
    fn pattern_canonical_form_via_as_str() {
        assert_eq!(
            DomainPattern::parse("Allowed.COM").unwrap().as_str(),
            "allowed.com"
        );
        assert_eq!(
            DomainPattern::parse("*.Allowed.COM").unwrap().as_str(),
            "*.allowed.com"
        );
        assert_eq!(
            DomainPattern::parse("example.com.").unwrap().as_str(),
            "example.com"
        );
        assert_eq!(DomainPattern::parse("[::1]").unwrap().as_str(), "::1");
        assert_eq!(
            DomainPattern::parse("0x7f.0.0.1").unwrap().as_str(),
            "127.0.0.1"
        );
        assert!(DomainPattern::parse("*.allowed.com").unwrap().is_wildcard());
        assert!(!DomainPattern::parse("allowed.com").unwrap().is_wildcard());
    }

    #[test]
    fn first_match_returns_the_first_hit_in_order() {
        let patterns = [
            pat("a.example.com"),
            pat("*.example.com"),
            pat("example.com"),
        ];
        assert_eq!(
            first_match(&patterns, &host_of("https://a.example.com/")).map(DomainPattern::as_str),
            Some("a.example.com")
        );
        assert_eq!(
            first_match(&patterns, &host_of("https://b.example.com/")).map(DomainPattern::as_str),
            Some("*.example.com")
        );
        assert_eq!(
            first_match(&patterns, &host_of("https://example.com/")).map(DomainPattern::as_str),
            Some("example.com")
        );
        assert!(first_match(&patterns, &host_of("https://other.org/")).is_none());
    }
}
