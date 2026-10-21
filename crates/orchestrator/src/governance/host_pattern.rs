//! The host pattern grammar every policy layer authors: `*`, a `*.suffix` wildcard, or one exact
//! host.
//!
//! A pattern is parsed once, when its layer loads, into a lowercase [`HostPattern`]. Validation,
//! matching, the probe set and the Policy page all ask the parsed value, so they read one grammar
//! with one normalization.

use url::Url;

use super::manifest::HostRules;

/// The longest hostname DNS allows, in characters.
const HOST_MAX_CHARS: usize = 253;

/// The longest DNS label, in characters.
const LABEL_MAX_CHARS: usize = 63;

/// One parsed host pattern, lowercase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostPattern {
    /// `*`: every host.
    Any,
    /// `*.suffix`: every host below `suffix`, never `suffix` itself.
    Below(String),
    /// One exact host.
    Exact(String),
}

impl HostPattern {
    /// `pattern` in the grammar, or none when it is not one.
    ///
    /// An exact host or a wildcard's suffix is ASCII, at most 253 characters, has no `/`, `:` or
    /// `*`, does not start or end with a dot, and is made of non-empty labels of at most 63
    /// letters, digits and hyphens that neither start nor end with a hyphen. Case is ignored.
    #[must_use]
    pub fn parse(pattern: &str) -> Option<Self> {
        if pattern == "*" {
            return Some(Self::Any);
        }
        let (host, below) = match pattern.strip_prefix("*.") {
            Some(suffix) => (suffix, true),
            None => (pattern, false),
        };
        let valid = !host.is_empty()
            && host.len() <= HOST_MAX_CHARS
            && host.is_ascii()
            && !host.contains(['/', ':', '*'])
            && !host.starts_with('.')
            && !host.ends_with('.')
            && host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= LABEL_MAX_CHARS
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            });
        if !valid {
            return None;
        }
        let host = host.to_ascii_lowercase();
        Some(if below {
            Self::Below(host)
        } else {
            Self::Exact(host)
        })
    }

    /// Whether this pattern names `host`, which is lowercase.
    #[must_use]
    pub fn matches(&self, host: &str) -> bool {
        self.specificity(host).is_some()
    }

    /// How closely this pattern names `host`, which is lowercase, or none when it does not.
    ///
    /// Larger is closer: an exact host beats any wildcard, a longer suffix beats a shorter one,
    /// and `*` is the weakest.
    #[must_use]
    pub fn specificity(&self, host: &str) -> Option<(u8, usize)> {
        match self {
            Self::Exact(exact) => (exact == host).then_some((3, exact.len())),
            Self::Below(suffix) => is_below(host, suffix).then_some((2, suffix.len())),
            Self::Any => Some((1, 0)),
        }
    }

    /// Whether every host `narrow` names, this pattern names too.
    ///
    /// A suffix wildcard covers the hosts below its suffix only, so `*.example.com` does not
    /// cover `example.com`.
    #[must_use]
    pub fn covers(&self, narrow: &Self) -> bool {
        if self == narrow {
            return true;
        }
        match (self, narrow) {
            (Self::Any, _) => true,
            (Self::Below(suffix), Self::Exact(host) | Self::Below(host)) => is_below(host, suffix),
            _ => false,
        }
    }
}

/// Whether `host` lies below `suffix`: it ends with a dot and `suffix`, with at least one more
/// label in front.
fn is_below(host: &str, suffix: &str) -> bool {
    host.len() > suffix.len()
        && host.ends_with(suffix)
        && host.as_bytes().get(host.len() - suffix.len() - 1) == Some(&b'.')
}

/// The host of `url` as every host pattern compares it: lowercase and without the trailing dot of
/// a fully qualified name, since a browser loads `bank.example.` as `bank.example` (G-D1). None
/// when the address has no host, or a host of dots alone.
#[must_use]
pub fn url_host(url: &Url) -> Option<String> {
    let host = url.host_str()?.trim_end_matches('.');
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// A grant's host rules, parsed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ParsedHosts {
    pub(super) allow: Vec<HostPattern>,
    pub(super) deny: Vec<HostPattern>,
}

impl ParsedHosts {
    /// `rules` parsed. Validation refuses a layer with a pattern outside the grammar, so a loaded
    /// layer loses none here.
    pub(super) fn of(rules: &HostRules) -> Self {
        Self {
            allow: parse_all(&rules.allow),
            deny: parse_all(&rules.deny),
        }
    }
}

/// Every pattern of `patterns` that is in the grammar, in order.
pub(super) fn parse_all(patterns: &[String]) -> Vec<HostPattern> {
    patterns
        .iter()
        .filter_map(|pattern| HostPattern::parse(pattern))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::HostPattern;

    #[test]
    fn a_pattern_is_parsed_once_whatever_its_case() {
        assert_eq!(HostPattern::parse("*"), Some(HostPattern::Any));
        assert_eq!(
            HostPattern::parse("*.Example.COM"),
            Some(HostPattern::Below("example.com".into()))
        );
        assert_eq!(
            HostPattern::parse("Docs.Example.com"),
            Some(HostPattern::Exact("docs.example.com".into()))
        );
        let exact = HostPattern::parse("EXAMPLE.com").unwrap();
        assert!(exact.matches("example.com"));
        let below = HostPattern::parse("*.EXAMPLE.com").unwrap();
        assert!(below.matches("a.example.com"));
        assert!(!below.matches("example.com"));
        assert!(below.covers(&HostPattern::parse("A.Example.Com").unwrap()));
    }

    #[test]
    fn what_is_outside_the_grammar_does_not_parse() {
        for invalid in [
            "",
            "*.",
            "**",
            "*.*.example.com",
            " example.com",
            "example.com ",
            "a/b",
            "a:443",
            ".example.com",
            "example.com.",
            "-a.example.com",
            "a-.example.com",
            "a..example.com",
            "ex\u{e4}mple.com",
        ] {
            assert_eq!(HostPattern::parse(invalid), None, "{invalid:?}");
        }
    }

    #[test]
    fn closer_patterns_rank_higher() {
        let host = "a.docs.example.com";
        let exact = HostPattern::parse(host).unwrap().specificity(host);
        let long = HostPattern::parse("*.docs.example.com")
            .unwrap()
            .specificity(host);
        let short = HostPattern::parse("*.example.com")
            .unwrap()
            .specificity(host);
        let any = HostPattern::Any.specificity(host);
        assert!(exact > long && long > short && short > any);
    }
}
