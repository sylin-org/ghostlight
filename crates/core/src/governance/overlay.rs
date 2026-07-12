// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Per-session tighten-only policy overlay (ADR-0060): the bottom tier of the policy
//! composition model.
//!
//! A client may declare an overlay policy at session `initialize`. For that session's calls
//! only, the service intersects the overlay's decision with the active service policy's
//! decision -- deny-overrides: a call is allowed only if BOTH allow. This mirrors AWS IAM
//! session policies ("can only reduce permissions"): composition is pure intersection, so an
//! overlay can never grant capability the service policy withholds. The service policy is always
//! the ceiling, automatically -- there is nothing to validate for escalation, because the
//! algebra makes escalation impossible.
//!
//! Maximum reuse, minimum new parts: an overlay IS a schema-3 manifest (same
//! [`parse_manifest`] path, same [`Grant`](crate::governance::manifest::document::Grant) type),
//! and it decides through the SAME audit-free [`Governance::decide`] the service policy uses --
//! just against the overlay's own grants. Only `.decide()` is ever called on the overlay's
//! `Governance`, never `.authorize()`, so the overlay's audit sink is never touched (it holds a
//! [`NullSink`]); the ONE audit record for a call is always the service's.

use std::sync::Arc;

use crate::governance::config::CONTENT_SECURITY_SACRED_DOMAINS;
use crate::governance::dispatch::Governance;
use crate::governance::enforcement::LocalPdp;
use crate::governance::manifest::document::{parse_manifest, ConfigEntry, ManifestError};
use crate::governance::ports::{
    Capability, Decision, EffectiveMode, GoverningResource, HostRuleOutcome, NullSink,
};

/// A parsed, ready-to-evaluate session overlay: the tighten-only bottom policy tier for one
/// session. Built once when the session declares it (at `initialize`), then consulted per call.
pub struct SessionOverlay {
    /// A governed facade over the overlay's own grants. Only [`Governance::decide`] (audit-free)
    /// is called on it; its [`NullSink`] audit is never used.
    governance: Governance,
    /// The overlay's own `content.security.sacred_domains`, unioned into the pipeline's always-on
    /// sacred check (a deny ceiling composes by union: any tier's sacred entry denies).
    sacred_domains: Vec<String>,
}

impl SessionOverlay {
    /// Parse a client-supplied overlay -- a schema-3 manifest, validated exactly as a service
    /// manifest is -- into a tighten-only session tier. `is_valid_pattern` (the host-syntax checker,
    /// `browser::pattern::is_valid_pattern`) and `evaluate_host` (the polarity evaluator,
    /// `browser::polarity::evaluate_host`) are INJECTED by the caller (`mcp/`), exactly as the
    /// service policy path injects them: the relocatable governance core must not name `browser`
    /// (the A7 architecture guard). A parse or shape error is returned verbatim so the client learns
    /// precisely what was malformed; the caller declines the overlay rather than silently proceeding
    /// without it.
    pub fn parse(
        text: &str,
        is_valid_pattern: fn(&str) -> bool,
        evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome,
    ) -> Result<Self, ManifestError> {
        let manifest = parse_manifest(text, "session-overlay", is_valid_pattern)?;
        let sacred_domains = extract_sacred_domains(&manifest.config);
        let governance = Governance::governed(
            Box::new(LocalPdp::new(evaluate_host)),
            Arc::new(NullSink),
            manifest.grants,
            manifest.hash,
            manifest.mode,
        );
        Ok(Self {
            governance,
            sacred_domains,
        })
    }

    /// The overlay's decision for one already-classified call: the SAME audit-free
    /// [`Governance::decide`] the service policy runs, against the overlay's grants. The caller
    /// intersects this with the service decision (deny-overrides); this method itself records
    /// nothing and mutates nothing.
    pub fn decide(
        &self,
        tool: &str,
        action: Option<&str>,
        requires: &[Capability],
        resource: GoverningResource,
        config_mode: EffectiveMode,
    ) -> Decision {
        self.governance
            .decide(tool, action, requires, resource, config_mode)
    }

    /// The overlay's sacred (never-touch) domains, for the pipeline to union into its always-on
    /// sacred check. Empty when the overlay declares none.
    pub fn sacred_domains(&self) -> &[String] {
        &self.sacred_domains
    }
}

/// Pull the string list out of the overlay's `content.security.sacred_domains` config entry, if
/// present. Any non-string array members are skipped (parse-time validation already rejected a
/// malformed value; this is a defensive read).
fn extract_sacred_domains(config: &[ConfigEntry]) -> Vec<String> {
    config
        .iter()
        .find(|e| e.key == CONTENT_SECURITY_SACRED_DOMAINS)
        .and_then(|e| e.value.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A permissive syntactic checker for these pure tests (the authoritative grammar lives in
    /// `browser::pattern`'s own tests). The governance core -- including this test module -- must
    /// not name `browser`, so an injected stub stands in, exactly as `enforcement.rs` /
    /// `document.rs` tests do.
    fn stub_valid_pattern(_p: &str) -> bool {
        true
    }

    /// A simplified host matcher for these pure tests: exact, `*.suffix`, or `*` (mirrors
    /// `enforcement.rs`'s `stub_evaluate_host`). Never the authoritative grammar.
    fn stub_evaluate_host(host: &str, allow: &[String], deny: &[String]) -> HostRuleOutcome {
        fn matches(pattern: &str, host: &str) -> bool {
            pattern == "*"
                || match pattern.strip_prefix("*.") {
                    Some(suffix) => host.ends_with(&format!(".{suffix}")),
                    None => pattern == host,
                }
        }
        let allowed = allow.iter().any(|p| matches(p, host));
        let denied = deny.iter().any(|p| matches(p, host));
        match (allowed, denied) {
            (true, false) => HostRuleOutcome::Allowed,
            (_, true) => HostRuleOutcome::Denied,
            (false, false) => HostRuleOutcome::Unmatched,
        }
    }

    /// Parse the overlay with the test stubs injected.
    fn parse(text: &str) -> Result<SessionOverlay, ManifestError> {
        SessionOverlay::parse(text, stub_valid_pattern, stub_evaluate_host)
    }

    /// An overlay granting only sylin.org, read+action, enforce mode.
    fn sylin_only_overlay() -> &'static str {
        r#"{
          "schema": 3,
          "name": "test-overlay",
          "version": "1",
          "mode": "enforce",
          "grants": [
            { "id": "sylin", "hosts": { "allow": ["sylin.org", "*.sylin.org"] },
              "allowed": ["read", "action"], "description": "the stage" }
          ],
          "config": [
            { "key": "content.security.sacred_domains", "value": ["sacred-bank.invalid"], "level": "mandatory" }
          ]
        }"#
    }

    fn decide_host(overlay: &SessionOverlay, host: &str) -> Decision {
        overlay.decide(
            "navigate",
            None,
            &[Capability::Action],
            GoverningResource::Resource(host.to_string()),
            EffectiveMode::Enforce,
        )
    }

    #[test]
    fn parses_a_schema_3_overlay() {
        let overlay = parse(sylin_only_overlay()).expect("valid overlay parses");
        assert_eq!(
            overlay.sacred_domains(),
            &["sacred-bank.invalid".to_string()]
        );
    }

    #[test]
    fn allows_a_granted_host() {
        let overlay = parse(sylin_only_overlay()).unwrap();
        assert!(matches!(
            decide_host(&overlay, "sylin.org"),
            Decision::Allow { .. }
        ));
    }

    #[test]
    fn denies_an_ungranted_host() {
        // The finale's essence: a host the overlay does not grant is denied, so the effective
        // (intersected) decision is Deny regardless of what the service policy says.
        let overlay = parse(sylin_only_overlay()).unwrap();
        assert!(matches!(
            decide_host(&overlay, "example.com"),
            Decision::Deny(_)
        ));
    }

    #[test]
    fn a_malformed_overlay_is_rejected_not_ignored() {
        // schema 2 (or anything but 3) must error, so a caller declines the overlay rather than
        // proceeding as if the client had asked for no restriction at all.
        let err = parse(r#"{"schema": 2, "name": "x", "version": "1", "grants": []}"#);
        assert!(err.is_err());
    }

    #[test]
    fn an_overlay_without_sacred_domains_reports_none() {
        let text = r#"{
          "schema": 3, "name": "n", "version": "1", "mode": "enforce",
          "grants": [ { "id": "g", "hosts": { "allow": ["sylin.org"] }, "allowed": ["read"], "description": "d" } ]
        }"#;
        let overlay = parse(text).unwrap();
        assert!(overlay.sacred_domains().is_empty());
    }
}
