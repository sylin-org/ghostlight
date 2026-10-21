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
    use crate::governance::{assemble, manifest, OwnOrigin, PolicyAuthor, MANAGED_TIER, USER_TIER};

    /// The snapshot enforcement reads for these layers.
    fn snapshot(organization: Option<&str>, user: Option<&str>) -> AuthoritySnapshot {
        let parse = |text: &str| manifest::parse(text, "test").expect("test policy parses");
        assemble(
            [
                (organization.map(parse), PolicyAuthor::Organization),
                (user.map(parse), PolicyAuthor::User),
            ],
            None,
            None,
            true,
            &OwnOrigin::default(),
        )
    }

    /// ACCEPTANCE Grouped history 2 (ADR-0156): the evidence a decision leaves records every layer
    /// it evaluated, with the grants that admitted the work there, tells all-open, granted and
    /// refused apart, and keeps the host without its path or query. A protected address or a
    /// never-touch host evaluates no grant, so it records no layer (arch-1 P2's note).
    #[test]
    fn evidence_records_every_layer_the_decision_evaluated_and_nothing_it_did_not() {
        let granted = r#"{"schema":4,"name":"test","version":"1","grants":[{"id":"read","hosts":{"allow":["example.com"]},"allowed":["read"]},{"id":"write","hosts":{"allow":["example.com"]},"allowed":["write"]},{"id":"combined","hosts":{"allow":["example.com"]},"allowed":["read","write"]}]}"#;
        let layered = snapshot(Some(granted), Some(granted));
        let requirements = CapabilitySet::READ.union(CapabilitySet::WRITE);
        let (decision, evidence) = layered.authorize_with_evidence(
            requirements,
            Some("https://example.com/PRIVATE_PATH?PRIVATE_QUERY"),
        );
        assert!(decision.allowed);
        assert!(evidence.allowed);
        assert_eq!(evidence.host.as_deref(), Some("example.com"));
        let combined = |tier: &str| LayerEvidence {
            tier: tier.into(),
            grants: vec!["combined".into()],
            allowed: true,
        };
        assert_eq!(
            evidence.layers,
            [combined(MANAGED_TIER), combined(USER_TIER)]
        );
        let encoded = serde_json::to_string(&evidence).unwrap();
        assert!(!encoded.contains("PRIVATE_"), "{encoded}");
        assert_eq!(
            serde_json::from_str::<PermissionCheck>(&encoded).unwrap(),
            evidence
        );
        // With no host, the layer's grants admit the set together.
        let (_, resource_less) = layered.authorize_with_evidence(requirements, None);
        assert_eq!(
            resource_less.layers[0].grants,
            ["read", "write", "combined"]
        );

        // All-open evaluates no layer.
        let (decision, open) = snapshot(None, None)
            .authorize_with_evidence(CapabilitySet::READ, Some("http://localhost:3000/"));
        assert!(decision.allowed);
        assert!(open.layers.is_empty());

        // A layer that refuses is recorded as refusing.
        let refusing = snapshot(
            None,
            Some(r#"{"schema":4,"name":"test","version":"1","grants":[]}"#),
        );
        let (decision, refused) =
            refusing.authorize_with_evidence(CapabilitySet::WRITE, Some("https://example.com/"));
        assert!(!decision.allowed);
        assert!(!refused.allowed);
        assert_eq!(refused.layers.len(), 1);
        assert_eq!(refused.layers[0].tier, USER_TIER);
        assert!(!refused.layers[0].allowed);

        // A protected address and a never-touch host are refused before any grant.
        let never_touch = snapshot(
            None,
            Some(&format!(
                r#"{{"schema":4,"name":"test","version":"1","grants":[{{"id":"all","hosts":{{"allow":["*"]}},"allowed":["read"]}}],"config":[{{"key":"{}","value":["bank.example"],"level":"mandatory"}}]}}"#,
                manifest::PolicySetting::NeverTouch.key()
            )),
        );
        for url in ["chrome://extensions", "https://bank.example/"] {
            let (decision, protected) =
                never_touch.authorize_with_evidence(CapabilitySet::READ, Some(url));
            assert!(!decision.allowed, "{url}");
            assert!(protected.layers.is_empty(), "{url} evaluated no grant");
        }
    }

    /// ACCEPTANCE Grouped history 2: a permission trace keeps each distinct check once, at most
    /// PERMISSION_CHECK_LIMIT of them, and says when it left some out.
    #[test]
    fn trace_bounds_and_deduplication_keep_omission_explicit() {
        let snapshot = snapshot(None, None);
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
                    tier: super::tier_word(layer.tier).into(),
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
                .and_then(|url| super::host_pattern::url_host(&url)),
            allowed: decision.allowed,
            reason: decision.reason,
            layers: vec![],
        }
    }
}
