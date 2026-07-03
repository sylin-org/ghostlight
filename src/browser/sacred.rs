//! The sacred-domains never-touch list (browser plugin; RECONCILIATION.md section 1, g08).
//!
//! `content.security.sacred_domains` (ADR-0018 step 2) is a user-authored deny-list, always
//! enforced regardless of `governance.mode` or manifest presence (shared format doc section
//! 3.4). This module owns the browser-domain half: scanning the list for a match
//! ([`first_match`]), mirroring the extension's `navigate` URL normalization so the target-host
//! check never diverges from what the extension will actually navigate to
//! ([`navigate_target_host`]), and building the sacred-flavored [`Denial`]
//! ([`sacred`]). Matching itself is entirely delegated to [`crate::browser::pattern`]; this
//! module reimplements no matching semantics. Pure: no I/O, no policy-mode logic -- the
//! always-on wiring (calling this unconditionally, regardless of mode) lives at the dispatch
//! chokepoint (`transport::mcp::server`), not here.

use crate::browser::pattern::{self, DomainPattern, HostOutcome, MatchHost};
use crate::governance::denial;
use crate::governance::ports::Denial;

/// First sacred pattern, in authored list order, matching `host`; `None` when none match. List
/// order matters: it fixes which pattern the denial id derives from (shared format doc section
/// 7.1). Entries that fail to parse as a domain pattern (should not happen for a list already
/// validated at config load; see [`crate::governance::config::CONTENT_SECURITY_SACRED_DOMAINS`])
/// are skipped rather than treated as a match or a crash.
pub fn first_match<'a>(host: &MatchHost, patterns: &'a [String]) -> Option<&'a str> {
    patterns
        .iter()
        .find(|p| {
            DomainPattern::parse(p)
                .map(|dp| dp.matches(host))
                .unwrap_or(false)
        })
        .map(String::as_str)
}

/// The host the extension will navigate to for a given `navigate` `url` argument, mirroring
/// `extension/service-worker.js`'s normalization exactly. `None` when there is no http(s)
/// target: `"back"`/`"forward"`, `about:`/`chrome:`/`edge:`/`brave:` URLs, or anything the URL
/// parser rejects after normalization. This MUST agree with the extension, or a schemeless or
/// scheme-mangled URL bypasses the list. Returns a [`MatchHost`] (not a bare `String`): the
/// result comes only from [`pattern::host_for_matching`], so it composes directly with
/// [`first_match`] without a second, unnormalized round trip through a plain string.
pub fn navigate_target_host(url_arg: &str) -> Option<MatchHost> {
    let normalized = normalize_navigate_target(url_arg)?;
    host_of(&normalized)
}

fn host_of(url: &str) -> Option<MatchHost> {
    match pattern::host_for_matching(url) {
        HostOutcome::Host(h) => Some(h),
        HostOutcome::NonHttpScheme(_) | HostOutcome::Unparseable => None,
    }
}

/// Mirror the extension's `navigate` URL normalization exactly (its handler in
/// `extension/service-worker.js`), returning the string the extension will actually attempt to
/// parse and navigate to. `None` for `"back"`/`"forward"`: there is no URL to normalize, since
/// the extension replays browser history instead of parsing one. Shared by
/// [`navigate_target_host`] (g08, the sacred-domains check) and
/// [`crate::browser::resource::navigate_target_resource`] (g13, the grant-enforcement
/// pre-dispatch check): both MUST agree with the extension on the exact same string, or a
/// schemeless or scheme-mangled target could pass one check and fail the other.
pub(crate) fn normalize_navigate_target(url_arg: &str) -> Option<String> {
    if url_arg == "back" || url_arg == "forward" {
        return None;
    }

    let lower = url_arg.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(url_arg.to_string());
    }
    if ["about:", "chrome:", "edge:", "brave:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        return Some(url_arg.to_string());
    }

    let stripped = strip_one_leading_scheme(url_arg);
    Some(format!("https://{stripped}"))
}

/// Strip one leading scheme prefix if present: 1 to 6 ASCII alphabetic characters, then `:`,
/// then one or more `/` (mirrors the extension's `url.replace(/^[a-z]{1,6}:\/+/i, "")`, first
/// occurrence only, case-insensitive). Returns `url_arg` unchanged if no such prefix is found.
fn strip_one_leading_scheme(url_arg: &str) -> &str {
    let Some(colon_idx) = url_arg.find(':') else {
        return url_arg;
    };
    let scheme = &url_arg[..colon_idx];
    if scheme.is_empty() || scheme.len() > 6 || !scheme.chars().all(|c| c.is_ascii_alphabetic()) {
        return url_arg;
    }
    let after_colon = &url_arg[colon_idx + 1..];
    let slash_count = after_colon.chars().take_while(|&c| c == '/').count();
    if slash_count == 0 {
        return url_arg;
    }
    &after_colon[slash_count..]
}

/// Build the denial for a host matching a sacred-domains pattern (shared format doc section
/// 7.2). No manifest and no grant participate in the sacred rule, so the id preimage uses an
/// empty manifest hash and grant id.
pub fn sacred(host: &str, pattern: &str) -> Denial {
    let rule = format!("sacred/{pattern}");
    let denial_id = denial::denial_id("", "", &rule);
    let message = format!(
        "Denied ({denial_id}): {host} is on the user's never-touch list. Do not retry or work around this; choose a different approach or ask the user directly."
    );
    Denial {
        rule,
        grant_id: None,
        denial_id,
        domain: host.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_of_url(url: &str) -> MatchHost {
        match pattern::host_for_matching(url) {
            HostOutcome::Host(h) => h,
            other => panic!("expected a matchable host for {url}, got {other:?}"),
        }
    }

    #[test]
    fn sacred_message_is_exact_and_leaks_nothing() {
        let denial = sacred("www.mybank.com", "*.mybank.com");
        assert_eq!(denial.denial_id, "D-af6633ec");
        assert_eq!(
            denial.message,
            "Denied (D-af6633ec): www.mybank.com is on the user's never-touch list. \
             Do not retry or work around this; choose a different approach or ask the user directly."
        );
        assert!(!denial.message.contains("*.mybank.com"));
        assert!(!denial.message.contains("sacred/"));
        assert!(!denial.message.contains("config"));
    }

    #[test]
    fn first_match_honors_list_order() {
        let patterns = vec!["*.mybank.com".to_string(), "mybank.com".to_string()];
        assert_eq!(
            first_match(&host_of_url("https://a.mybank.com/"), &patterns),
            Some("*.mybank.com")
        );
        assert_eq!(
            first_match(&host_of_url("https://mybank.com/"), &patterns),
            Some("mybank.com")
        );

        let reversed = vec!["mybank.com".to_string(), "*.mybank.com".to_string()];
        assert_eq!(
            first_match(&host_of_url("https://a.mybank.com/"), &reversed),
            Some("*.mybank.com"),
            "the wildcard still wins for subdomains regardless of list order"
        );
        assert_eq!(
            first_match(&host_of_url("https://mybank.com/"), &reversed),
            Some("mybank.com")
        );
    }

    #[test]
    fn navigate_target_mirrors_the_extension() {
        let cases: &[(&str, Option<&str>)] = &[
            ("back", None),
            ("forward", None),
            ("https://mybank.com/x", Some("mybank.com")),
            ("HTTPS://MYBANK.COM", Some("mybank.com")),
            ("mybank.com/login", Some("mybank.com")),
            ("ftp://mybank.com/", Some("mybank.com")),
            ("about:blank", None),
            ("chrome://settings", None),
            ("javascript:alert(1)", None),
            ("https://mybank.com@evil.com/", Some("evil.com")),
            ("https://evil.com@mybank.com/", Some("mybank.com")),
            ("https://mybank.com./", Some("mybank.com")),
        ];
        for (input, expected) in cases {
            let got = navigate_target_host(input);
            assert_eq!(
                got.as_ref().map(MatchHost::as_str),
                *expected,
                "input: {input}"
            );
        }
    }

    #[test]
    fn sacred_bypass_classes() {
        let patterns = vec![
            "mybank.com".to_string(),
            "*.mybank.com".to_string(),
            "127.0.0.1".to_string(),
        ];

        for url in [
            "https://user:pass@mybank.com/",
            "https://mybank.com./",
            "https://sub.a.mybank.com/",
            "http://127.0.0.1/",
            "http://mybank.com:8443/",
        ] {
            assert!(
                first_match(&host_of_url(url), &patterns).is_some(),
                "must be denied: {url}"
            );
        }

        for url in [
            "https://mybank.com@evil.com/",
            "https://evilmybank.com/",
            "https://mybank.com.evil.com/",
            "http://[::1]/",
        ] {
            assert!(
                first_match(&host_of_url(url), &patterns).is_none(),
                "must not be denied: {url}"
            );
        }

        let homoglyph_host = host_of_url("https://myb\u{0430}nk.com/");
        assert!(homoglyph_host.as_str().starts_with("xn--"));
        assert!(first_match(&homoglyph_host, &patterns).is_none());

        let wildcard_only = vec!["*.mybank.com".to_string()];
        assert!(first_match(&host_of_url("https://mybank.com/"), &wildcard_only).is_none());
        assert!(first_match(&host_of_url("https://www.mybank.com/"), &wildcard_only).is_some());
    }
}
