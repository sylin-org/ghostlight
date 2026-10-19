//! Bounded permission evidence captured by the same evaluation that decides authority.

use super::{AuthoritySnapshot, CapabilitySet, Decision, LayerOutcome, ReasonCode};
use serde::{Deserialize, Serialize};

const PERMISSION_CHECK_LIMIT: usize = 64;

/// The evaluated grant identities in one authority layer, without its policy payload.
///
/// A reader ignores fields it does not know, so this type does not deny unknown fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LayerEvidence {
    pub tier: String,
    pub grants: Vec<String>,
    pub allowed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{manifest, AuthorityTier, GovernanceFacade, PolicyLayer};
    use crate::intake::session::IntakeChannel;

    #[test]
    fn positive_evidence_preserves_ordered_grants_and_every_layer() {
        let mut snapshot = GovernanceFacade::new(None, None)
            .snapshot()
            .as_ref()
            .clone();
        for tier in [AuthorityTier::Managed, AuthorityTier::User] {
            snapshot.layers.push(PolicyLayer::new(tier, manifest::parse(r#"{"schema":4,"name":"test","version":"1","grants":[{"id":"read","hosts":{"allow":["example.com"]},"allowed":["read"]},{"id":"write","hosts":{"allow":["example.com"]},"allowed":["write"]},{"id":"combined","hosts":{"allow":["example.com"]},"allowed":["read","write"]}]}"#, "test").unwrap()));
        }
        let requirements = CapabilitySet::READ.union(CapabilitySet::WRITE);
        let (decision, evidence) = snapshot.authorize_with_evidence(
            requirements,
            Some("https://example.com/PRIVATE_PATH?PRIVATE_QUERY"),
        );
        assert!(decision.allowed);
        assert_eq!(evidence.host.as_deref(), Some("example.com"));
        assert_eq!(evidence.layers.len(), 2);
        assert!(evidence
            .layers
            .iter()
            .all(|layer| layer.grants == ["combined"]));
        let (_, resource_less) = snapshot.authorize_with_evidence(requirements, None);
        assert_eq!(
            resource_less.layers[0].grants,
            ["read", "write", "combined"]
        );
        let encoded = serde_json::to_string(&evidence).unwrap();
        assert!(!encoded.contains("PRIVATE_"));
        assert_eq!(
            serde_json::from_str::<PermissionCheck>(&encoded).unwrap(),
            evidence
        );
        // Replacing the current snapshot cannot change the retained permission story.
        snapshot.layers.clear();
        assert_eq!(evidence.layers[0].grants, ["combined"]);
    }

    #[test]
    fn all_open_and_protected_denials_keep_their_actual_meaning() {
        let facade = GovernanceFacade::new(None, None);
        let mut snapshot = facade.snapshot().as_ref().clone();
        let (decision, open) =
            snapshot.authorize_with_evidence(CapabilitySet::READ, Some("http://localhost:3000/"));
        assert!(decision.allowed);
        assert!(open.layers.is_empty());
        assert_eq!(
            crate::language::history::permission(&open, IntakeChannel::Mcp, None),
            "Allowed without configured policy."
        );
        snapshot.layers.push(PolicyLayer::new(
            AuthorityTier::User,
            manifest::parse(
                r#"{"schema":4,"name":"test","version":"1","grants":[]}"#,
                "test",
            )
            .unwrap(),
        ));
        let (decision, refused) =
            snapshot.authorize_with_evidence(CapabilitySet::WRITE, Some("https://example.com/"));
        assert!(!decision.allowed);
        assert!(!refused.layers[0].allowed);
        assert!(
            crate::language::history::permission(&refused, IntakeChannel::Mcp, Some("user"))
                .starts_with("Refused")
        );
        let (decision, protected) =
            snapshot.authorize_with_evidence(CapabilitySet::READ, Some("chrome://extensions"));
        assert!(!decision.allowed);
        assert!(
            protected.layers.is_empty(),
            "protected rejection never evaluated grants"
        );
        snapshot.layers[0].manifest = manifest::parse(
            r#"{"schema":4,"name":"enforced","version":"1","grants":[]}"#,
            "test",
        )
        .unwrap();
        let decision = snapshot.authorize_capability(super::super::Capability::Write);
        assert!(!decision.allowed);
        assert!(
            snapshot.attribution(decision).unwrap().0 == "user",
            "new decisions contain no caller restriction evidence"
        );
    }

    #[test]
    fn trace_bounds_and_deduplication_keep_omission_explicit() {
        let snapshot = GovernanceFacade::new(None, None).snapshot();
        let mut trace = PermissionTrace::default();
        for index in 0..=PERMISSION_CHECK_LIMIT {
            let (_, check) = snapshot.authorize_with_evidence(
                CapabilitySet::READ,
                Some(&format!("https://host{index}.test/")),
            );
            trace.record(check.clone());
            trace.record(check);
        }
        assert_eq!(trace.checks.len(), PERMISSION_CHECK_LIMIT);
        assert!(trace.truncated);
    }
}

/// One actual permission boundary; a host never includes its URL path or query.
///
/// A reader ignores fields it does not know, so this type does not deny unknown fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PermissionCheck {
    pub requirements: CapabilitySet,
    pub host: Option<String>,
    pub allowed: bool,
    pub reason: ReasonCode,
    pub layers: Vec<LayerEvidence>,
}

/// A bounded trace. Omission is explicit and never implies complete policy coverage.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PermissionTrace {
    pub checks: Vec<PermissionCheck>,
    pub truncated: bool,
}

impl PermissionTrace {
    /// Coalesce identical checks and bound retained permission evidence.
    pub fn record(&mut self, check: PermissionCheck) {
        if self.checks.contains(&check) {
            return;
        }
        if self.checks.len() == PERMISSION_CHECK_LIMIT {
            self.truncated = true;
        } else {
            self.checks.push(check);
        }
    }
}

impl AuthoritySnapshot {
    /// Decide once and retain the evidence from that same immutable evaluation.
    #[must_use]
    pub fn authorize_with_evidence(
        &self,
        requirements: CapabilitySet,
        url: Option<&str>,
    ) -> (Decision, PermissionCheck) {
        let (decision, outcomes) = match url {
            Some(url) => self.decide_landing(requirements, url),
            None => self.decide_requirements(requirements),
        };
        let mut evidence = self.decision_evidence(requirements, url, decision);
        evidence.layers = self
            .layers
            .iter()
            .zip(outcomes)
            .map(|(layer, outcome)| {
                let LayerOutcome { grants, denial } = outcome;
                let indices = if grants.is_empty() {
                    denial.and_then(|d| d.grant).into_iter().collect()
                } else {
                    grants
                };
                LayerEvidence {
                    tier: layer.tier.as_str().into(),
                    grants: indices
                        .into_iter()
                        .filter_map(|index| layer.manifest.grants.get(usize::from(index)))
                        .map(|grant| grant.id.clone())
                        .collect(),
                    allowed: denial.is_none(),
                }
            })
            .collect();
        (decision, evidence)
    }

    /// Project a runtime or special-boundary decision without inventing evaluated grants.
    #[must_use]
    pub fn decision_evidence(
        &self,
        requirements: CapabilitySet,
        url: Option<&str>,
        decision: Decision,
    ) -> PermissionCheck {
        PermissionCheck {
            requirements,
            host: url
                .and_then(|url| url::Url::parse(url).ok())
                .and_then(|url| url.host_str().map(str::to_ascii_lowercase)),
            allowed: decision.allowed,
            reason: decision.reason,
            layers: vec![],
        }
    }
}
