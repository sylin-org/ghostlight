//! URL-to-governing-resource classification (browser plugin; RECONCILIATION.md section 1, g13).
//!
//! Turns a URL string into the [`GoverningResource`] g13's decision core consumes: a `navigate`
//! target argument before the extension has acted on it ([`navigate_target_resource`]), or an
//! already-resolved URL such as a tab's current URL reported by the extension's tab-URL query
//! ([`resolved_url_resource`]). [`crate::browser::pattern`] only turns URLs into matchable hosts
//! and deliberately assigns no meaning to the result (its own module doc: "consumers ... decide
//! what to do with that answer"); this module is that consumer for the grant-enforcement
//! pre/post-dispatch checks specifically. Pure: no I/O, no policy decisions of its own -- it
//! reports what a URL IS, not whether it is allowed.

use crate::browser::pattern::{self, HostOutcome};
use crate::browser::sacred;
use crate::governance::ports::GoverningResource;

/// Classify an ALREADY-RESOLVED URL -- a tab's current URL from the extension's tab-URL query,
/// or a post-navigate re-query (g13 points 3 and 5) -- into a [`GoverningResource`]. The literal
/// parking page `about:blank` (case-insensitive) always allows; any other non-http(s) scheme is
/// out of scope by that scheme; an unparseable string is [`GoverningResource::Indeterminate`]
/// (fails closed, shared format doc section 4.5). Never applied to a raw `navigate` argument
/// (see [`navigate_target_resource`] instead): a tab's own URL needs no extension-normalization
/// mirror, since it is already the fully resolved string `chrome.tabs.get` reports.
pub fn resolved_url_resource(url: &str) -> GoverningResource {
    if url.eq_ignore_ascii_case("about:blank") {
        return GoverningResource::AlwaysAllow;
    }
    match pattern::host_for_matching(url) {
        HostOutcome::Host(h) => GoverningResource::Resource(h.as_str().to_string()),
        HostOutcome::NonHttpScheme(scheme) => GoverningResource::OutOfScope(scheme),
        HostOutcome::Unparseable => GoverningResource::Indeterminate,
    }
}

/// Classify a `navigate` `url` argument BEFORE the extension has acted on it, mirroring its own
/// normalization exactly via [`sacred::normalize_navigate_target`] (the same transform
/// [`sacred::navigate_target_host`] uses for the sacred-domains check). `None` means no
/// pre-dispatch check applies: `"back"`/`"forward"` (point 5 covers the landing instead), or a
/// normalized string the URL parser rejects (the extension itself refuses an invalid URL without
/// navigating, so there is nothing to govern). `Some` otherwise, with the same
/// about:blank/scheme/host classification [`resolved_url_resource`] uses.
pub fn navigate_target_resource(url_arg: &str) -> Option<GoverningResource> {
    let normalized = sacred::normalize_navigate_target(url_arg)?;
    if normalized.eq_ignore_ascii_case("about:blank") {
        return Some(GoverningResource::AlwaysAllow);
    }
    match pattern::host_for_matching(&normalized) {
        HostOutcome::Unparseable => None,
        HostOutcome::NonHttpScheme(scheme) => Some(GoverningResource::OutOfScope(scheme)),
        HostOutcome::Host(h) => Some(GoverningResource::Resource(h.as_str().to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_about_blank_always_allows() {
        assert_eq!(
            resolved_url_resource("about:blank"),
            GoverningResource::AlwaysAllow
        );
        assert_eq!(
            resolved_url_resource("ABOUT:BLANK"),
            GoverningResource::AlwaysAllow
        );
    }

    #[test]
    fn resolved_host_and_scheme_and_unparseable() {
        assert_eq!(
            resolved_url_resource("https://example.com/x"),
            GoverningResource::Resource("example.com".to_string())
        );
        assert_eq!(
            resolved_url_resource("chrome://settings"),
            GoverningResource::OutOfScope("chrome".to_string())
        );
        assert_eq!(
            resolved_url_resource("not a url"),
            GoverningResource::Indeterminate
        );
    }

    #[test]
    fn navigate_target_back_and_forward_have_no_pre_check() {
        assert_eq!(navigate_target_resource("back"), None);
        assert_eq!(navigate_target_resource("forward"), None);
    }

    #[test]
    fn navigate_target_about_blank_always_allows() {
        assert_eq!(
            navigate_target_resource("about:blank"),
            Some(GoverningResource::AlwaysAllow)
        );
    }

    #[test]
    fn navigate_target_http_host_and_schemeless_input() {
        assert_eq!(
            navigate_target_resource("https://example.com/x"),
            Some(GoverningResource::Resource("example.com".to_string()))
        );
        assert_eq!(
            navigate_target_resource("example.com/login"),
            Some(GoverningResource::Resource("example.com".to_string()))
        );
    }

    #[test]
    fn navigate_target_allowlisted_non_http_scheme_is_out_of_scope() {
        assert_eq!(
            navigate_target_resource("chrome://settings"),
            Some(GoverningResource::OutOfScope("chrome".to_string()))
        );
    }

    #[test]
    fn navigate_target_unparseable_after_normalization_has_no_pre_check() {
        assert_eq!(navigate_target_resource("javascript:alert(1)"), None);
    }
}
