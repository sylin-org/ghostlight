// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! `channels.webapi.from` -- the minimal flat channel-source allowlist (ADR-0030 "Governance
//! schema section (normative)"; Decision 5; Decision 9). This is the SINGLE sanctioned
//! `src/governance/**` addition in the Hub batch (H8; `tests/architecture.rs` a7 exception,
//! `docs/tasks/hub/H8-web-api-loopback-policy.md`).
//!
//! `channels.webapi.from` governs authenticated SOURCES connecting to the web API adapter; it
//! NEVER gates which tools exist (ADR-0030 Decision 6: "all tools free for everyone" is
//! preserved). The full recursive `grant := { id, channels, tools }` grammar (ADR-0030
//! "Governance schema section") is DEFERRED to its own core-only ADR; this batch realizes only
//! the minimal flat allowlist selector described there.
//!
//! This module stays inside the a7 boundary: it names none of the forbidden crate edges the
//! architecture test guards, and no bare tabId/token/socket identifier -- the allowlist is a
//! plain `Vec<String>` of source patterns and the resolved source is a plain `String`.

use crate::governance::denial;
use crate::governance::ports::{Decision, DecisionRequest, Denial, PolicyDecisionPoint};

/// Rule label for a `channels.webapi.from` denial (PINNED, `docs/tasks/hub/PINS.md` SS7).
pub const RULE_WEBAPI_FROM: &str = "channel/webapi_from";

/// Membership matcher (ADR-0030 "Governance schema section": "exact matcher ... for channels").
/// `"*"` matches any source; otherwise a pattern must equal `source` exactly (a bare host,
/// `"localhost"`, or a named principal -- the axis has no glob/wildcard grammar beyond the
/// literal `"*"` member, unlike the `hosts` axis).
pub fn is_member(allowlist: &[String], source: &str) -> bool {
    allowlist
        .iter()
        .any(|pattern| pattern == "*" || pattern == source)
}

/// Fail-closed load-time validation of a raw `channels.webapi.from` value (ADR-0030 "Governance
/// schema section": "each adapter validates its own refinement slice; fail-closed on an unknown
/// selector"). Accepts ONLY a flat JSON array of non-empty source-pattern strings; any other
/// shape (not an array, a non-string member, an empty-string member) is rejected rather than
/// silently defaulted or partially accepted.
pub fn validate_webapi_from(value: &serde_json::Value) -> Result<Vec<String>, String> {
    let arr = value
        .as_array()
        .ok_or_else(|| "channels.webapi.from must be an array of source patterns".to_string())?;
    arr.iter()
        .map(|v| {
            v.as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or_else(|| "channels.webapi.from entries must be non-empty strings".to_string())
        })
        .collect()
}

/// The channels denial (rule [`RULE_WEBAPI_FROM`]; `denial_id` via the existing
/// [`denial::denial_id`] scheme, PINS.md SS7). `domain` carries the refused source (mirroring
/// the existing denial shape's use of `domain` for "the thing that was checked and refused").
fn channel_denial(source: &str, manifest_hash: &str) -> Denial {
    let rule = RULE_WEBAPI_FROM.to_string();
    let denial_id = denial::denial_id(manifest_hash, "", &rule);
    let message = format!(
        "Denied ({denial_id}): '{source}' is not permitted to connect to the local web API. \
         Only sources named in the channels.webapi.from policy may connect."
    );
    Denial {
        rule,
        grant_id: None,
        denial_id,
        domain: source.to_string(),
        message,
    }
}

/// The pure `channels.webapi.from` PDP-side decision (ADR-0030 Decision 5/9, H8 Required
/// behavior item 4): `Allow` when `source` is a member of `allowlist`, `Deny` (rule
/// [`RULE_WEBAPI_FROM`]) otherwise. `manifest_hash` feeds the existing `denial::denial_id`
/// scheme so the id is fully reproducible from the inputs alone, exactly like every other
/// denial in the governance core.
pub fn decide_webapi_from(allowlist: &[String], source: &str, manifest_hash: &str) -> Decision {
    if is_member(allowlist, source) {
        Decision::Allow { grant_id: None }
    } else {
        Decision::Deny(channel_denial(source, manifest_hash))
    }
}

/// A [`PolicyDecisionPoint`] deciding ONLY the resolved connecting-source axis
/// (`DecisionRequest::channel_source`), constructed with the resolved `channels.webapi.from`
/// allowlist for the connection being decided. It never touches the tool/resource axes (those
/// remain `LocalPdp`/`NoopPdp`'s job elsewhere) -- so it cannot gate which tools exist (ADR-0030
/// Decision 6 is preserved by construction: this type has no notion of a tool at all).
///
/// Driven directly (no listener involved) by `tests/channels_policy.rs`, exactly as the task's
/// pinned assertion describes: "the decision is produced by `PolicyDecisionPoint::decide` (the
/// PDP), NOT by any transport-layer check."
pub struct ChannelsPdp {
    allowlist: Vec<String>,
}

impl ChannelsPdp {
    /// `allowlist` is the resolved `channels.webapi.from` value for this connection (the web
    /// adapter's builtin default, `[allow: "localhost"]`, absent any overlay -- ADR-0030
    /// Decision 5).
    pub fn new(allowlist: Vec<String>) -> Self {
        Self { allowlist }
    }
}

impl PolicyDecisionPoint for ChannelsPdp {
    fn decide(&self, req: &DecisionRequest) -> Decision {
        let source = req.channel_source.as_deref().unwrap_or("");
        decide_webapi_from(&self.allowlist, source, &req.manifest_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_is_a_member_of_the_builtin_default() {
        let allowlist = vec!["localhost".to_string()];
        assert!(is_member(&allowlist, "localhost"));
        assert!(!is_member(&allowlist, "203.0.113.7"));
    }

    #[test]
    fn star_matches_any_source() {
        let allowlist = vec!["*".to_string()];
        assert!(is_member(&allowlist, "localhost"));
        assert!(is_member(&allowlist, "203.0.113.7"));
        assert!(is_member(&allowlist, "alice"));
    }

    #[test]
    fn decide_allows_a_member_and_denies_a_non_member() {
        let allowlist = vec!["localhost".to_string()];
        assert_eq!(
            decide_webapi_from(&allowlist, "localhost", ""),
            Decision::Allow { grant_id: None }
        );
        match decide_webapi_from(&allowlist, "203.0.113.7", "") {
            Decision::Deny(denial) => {
                assert_eq!(denial.rule, RULE_WEBAPI_FROM);
                assert!(denial.denial_id.starts_with("D-"));
                assert_eq!(denial.denial_id.len(), 10);
            }
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    #[test]
    fn validate_accepts_a_flat_string_array_and_rejects_other_shapes() {
        assert_eq!(
            validate_webapi_from(&serde_json::json!(["localhost", "*"])).unwrap(),
            vec!["localhost".to_string(), "*".to_string()]
        );
        assert!(validate_webapi_from(&serde_json::json!("localhost")).is_err());
        assert!(validate_webapi_from(&serde_json::json!(["localhost", ""])).is_err());
        assert!(validate_webapi_from(&serde_json::json!([1, 2])).is_err());
    }
}
