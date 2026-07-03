//! Host polarity evaluation for schema-3 grants (ADR-0022 Decision 4). A grant's
//! `hosts.allow` grants coverage, `hosts.deny` carves holes out of it, and the default is
//! DENY: a host matched by neither list is [`HostRuleOutcome::Unmatched`]. Specificity
//! order: exact beats `*.suffix`, a longer wildcard suffix beats a shorter one, `*` loses
//! to everything, and an exact tie between allow and deny goes to deny. This module is
//! pure (no I/O, no grant walking -- composing grants in manifest order is enforcement's
//! job, s05); the outcome type lives in the core
//! ([`crate::governance::ports::HostRuleOutcome`]) while this module is injected into the
//! core as a plain `fn` pointer, mirroring [`crate::browser::pattern::pattern_matches_normalized_host`].

use crate::governance::ports::HostRuleOutcome;

/// True when `pattern` is a syntactically valid schema-3 `hosts` entry. Bare `"*"` is legal
/// ONLY in schema-3 grant `hosts` lists (ADR-0022 Decision 4 rule 1: the explicit
/// everything token), NEVER in `content.security.sacred_domains`, whose validation keeps
/// calling [`crate::browser::pattern::is_valid_pattern`].
pub fn is_valid_host_rule(pattern: &str) -> bool {
    pattern == "*" || crate::browser::pattern::is_valid_pattern(pattern)
}

/// Evaluate one grant's host rules against `host`, an ALREADY-NORMALIZED host string
/// (callers pass hosts produced for `GoverningResource::Resource` via
/// [`crate::browser::pattern::host_for_matching`]). Patterns were validated at manifest
/// load; invalid patterns never match (false, never a panic).
pub fn evaluate_host(host: &str, allow: &[String], deny: &[String]) -> HostRuleOutcome {
    if allow.is_empty() {
        return HostRuleOutcome::Unmatched;
    }

    let best_allow = best_specificity(host, allow);
    let best_deny = best_specificity(host, deny);

    match (best_allow, best_deny) {
        (None, None) => HostRuleOutcome::Unmatched,
        (Some(_), None) => HostRuleOutcome::Allowed,
        (None, Some(_)) => HostRuleOutcome::Denied,
        (Some(a), Some(d)) => {
            if d >= a {
                HostRuleOutcome::Denied
            } else {
                HostRuleOutcome::Allowed
            }
        }
    }
}

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

/// Specificity encoding: `(0, 0)` for `"*"`; `(1, suffix.len())` for a `"*."`-prefixed
/// pattern where `suffix` is the pattern with its two-byte `"*."` prefix stripped;
/// `(2, pattern.len())` otherwise. Tuples compare with Rust's derived lexicographic `Ord`;
/// larger is more specific.
fn specificity(pattern: &str) -> (u8, usize) {
    if pattern == "*" {
        (0, 0)
    } else if let Some(suffix) = pattern.strip_prefix("*.") {
        (1, suffix.len())
    } else {
        (2, pattern.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn allowlist_covers_only_listed_hosts() {
        let allow = rules(&["site1.com", "site2.com"]);
        let deny = rules(&[]);
        assert_eq!(
            evaluate_host("site1.com", &allow, &deny),
            HostRuleOutcome::Allowed
        );
        assert_eq!(
            evaluate_host("site2.com", &allow, &deny),
            HostRuleOutcome::Allowed
        );
        assert_eq!(
            evaluate_host("site3.com", &allow, &deny),
            HostRuleOutcome::Unmatched
        );
        assert_eq!(
            evaluate_host("sub.site1.com", &allow, &deny),
            HostRuleOutcome::Unmatched
        );
    }

    #[test]
    fn star_allow_with_deny_carveout() {
        let allow = rules(&["*"]);
        let deny = rules(&["site1.com"]);
        assert_eq!(
            evaluate_host("site1.com", &allow, &deny),
            HostRuleOutcome::Denied
        );
        assert_eq!(
            evaluate_host("site2.com", &allow, &deny),
            HostRuleOutcome::Allowed
        );
        assert_eq!(
            evaluate_host("sub.site1.com", &allow, &deny),
            HostRuleOutcome::Allowed
        );
    }

    #[test]
    fn star_allow_alone_allows_everything() {
        let allow = rules(&["*"]);
        let deny = rules(&[]);
        assert_eq!(
            evaluate_host("site1.com", &allow, &deny),
            HostRuleOutcome::Allowed
        );
        assert_eq!(
            evaluate_host("a.b.example.org", &allow, &deny),
            HostRuleOutcome::Allowed
        );
        assert_eq!(
            evaluate_host("127.0.0.1", &allow, &deny),
            HostRuleOutcome::Allowed
        );
    }

    #[test]
    fn empty_rules_are_unmatched() {
        let allow = rules(&[]);
        let deny = rules(&[]);
        assert_eq!(
            evaluate_host("site1.com", &allow, &deny),
            HostRuleOutcome::Unmatched
        );
        assert_eq!(
            evaluate_host("example.com", &allow, &deny),
            HostRuleOutcome::Unmatched
        );
    }

    #[test]
    fn deny_only_is_unmatched_for_everything() {
        let allow = rules(&[]);
        let deny = rules(&["site1.com"]);
        assert_eq!(
            evaluate_host("site1.com", &allow, &deny),
            HostRuleOutcome::Unmatched
        );
        assert_eq!(
            evaluate_host("site2.com", &allow, &deny),
            HostRuleOutcome::Unmatched
        );
    }

    #[test]
    fn exact_allow_beats_star_deny() {
        let allow = rules(&["site1.com"]);
        let deny = rules(&["*"]);
        assert_eq!(
            evaluate_host("site1.com", &allow, &deny),
            HostRuleOutcome::Allowed
        );
        assert_eq!(
            evaluate_host("site2.com", &allow, &deny),
            HostRuleOutcome::Denied
        );
    }

    #[test]
    fn exact_deny_beats_star_allow() {
        let allow = rules(&["*"]);
        let deny = rules(&["site1.com"]);
        assert_eq!(
            evaluate_host("site1.com", &allow, &deny),
            HostRuleOutcome::Denied
        );
    }

    #[test]
    fn longer_wildcard_beats_shorter() {
        let allow = rules(&["*.corp.example.com"]);
        let deny = rules(&["*.example.com"]);
        assert_eq!(
            evaluate_host("a.corp.example.com", &allow, &deny),
            HostRuleOutcome::Allowed
        );
        assert_eq!(
            evaluate_host("b.example.com", &allow, &deny),
            HostRuleOutcome::Denied
        );
    }

    #[test]
    fn identical_pattern_in_both_lists_is_denied() {
        let allow = rules(&["site1.com"]);
        let deny = rules(&["site1.com"]);
        assert_eq!(
            evaluate_host("site1.com", &allow, &deny),
            HostRuleOutcome::Denied
        );

        let allow_star = rules(&["*"]);
        let deny_star = rules(&["*"]);
        assert_eq!(
            evaluate_host("anything.example", &allow_star, &deny_star),
            HostRuleOutcome::Denied
        );
    }

    #[test]
    fn wildcard_never_matches_the_apex() {
        let allow = rules(&["*.example.com"]);
        let deny = rules(&[]);
        assert_eq!(
            evaluate_host("example.com", &allow, &deny),
            HostRuleOutcome::Unmatched
        );
        assert_eq!(
            evaluate_host("sub.example.com", &allow, &deny),
            HostRuleOutcome::Allowed
        );
    }

    #[test]
    fn is_valid_host_rule_accepts_star_and_delegates_the_rest() {
        for p in ["*", "example.com", "*.example.com", "127.0.0.1"] {
            assert!(is_valid_host_rule(p), "{p} should be valid");
        }
        for p in [
            "",
            "*.",
            "**.example.com",
            "site*",
            "*bank*",
            "Example.com",
            "https://example.com",
            "example.com:8443",
            "example.com/path",
        ] {
            assert!(!is_valid_host_rule(p), "{p} should be invalid");
        }
    }
}
