//! Monotonic audit-availability requirement across the existing authority layers.

use super::effective::{layer_kind, LayerKind};
use super::{AuthoritySnapshot, PolicyLayer};
use serde::{Deserialize, Serialize};

/// Registered choice for continuing work during a known history-storage failure.
pub const MODE_KEY: &str = "audit.availability";

/// Whether browser work continues while history cannot be saved.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditMode {
    #[default]
    KeepWorking,
    RequireAudit,
}

impl AuditMode {
    /// Decode the closed policy vocabulary.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "keep_working" => Some(Self::KeepWorking),
            "require_audit" => Some(Self::RequireAudit),
            _ => None,
        }
    }
}

/// Effective requirement and its author, shared by policy explain and the human editor.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AuditPolicy {
    pub mode: AuditMode,
    pub source: Option<LayerKind>,
    pub organization_mode: Option<AuditMode>,
}

/// The audit requirement of one publication, resolved once when its snapshot is assembled.
///
/// Enforcement and the compiled view both read this one answer, so a refusal is attributed to the
/// layer the Policy page names.
#[derive(Clone, Debug, Default)]
pub(super) struct AuditRequirement {
    /// What the compiled view reports.
    policy: AuditPolicy,
    /// The snapshot index of the layer whose choice is in force.
    layer: Option<u16>,
}

impl AuditRequirement {
    /// The strictest choice `layers` author, in authority order: a later layer can tighten the
    /// organization's choice and never relax it.
    pub(super) fn of(layers: &[PolicyLayer]) -> Self {
        let mut resolved = Self::default();
        for (index, layer) in layers.iter().enumerate() {
            let Some(mode) = layer
                .manifest
                .string_setting(MODE_KEY)
                .and_then(AuditMode::parse)
            else {
                continue;
            };
            let source = layer_kind(layer.tier);
            if source == LayerKind::Organization {
                resolved.policy.organization_mode = Some(mode);
            }
            if resolved.layer.is_none() || mode > resolved.policy.mode {
                resolved.policy.mode = mode;
                resolved.policy.source = Some(source);
                resolved.layer = Some(u16::try_from(index).expect("bounded authority layers"));
            }
        }
        resolved
    }
}

impl AuthoritySnapshot {
    /// The effective audit requirement and the layer that set it, as enforcement applies it.
    #[must_use]
    pub fn audit_policy(&self) -> &AuditPolicy {
        &self.audit.policy
    }

    /// Whether this invocation requires a currently working audit destination.
    #[must_use]
    pub fn requires_audit(&self) -> bool {
        self.audit.policy.mode == AuditMode::RequireAudit
    }

    /// Admit current storage health with attribution to the actual immutable policy requirement.
    #[must_use]
    pub fn authorize_audit(&self, available: bool) -> super::Decision {
        let Some(index) = self
            .audit
            .layer
            .filter(|_| self.requires_audit() && !available)
        else {
            return super::Decision::permitted();
        };
        let rule = super::PolicyRule::AuditAvailability;
        super::Decision::policy(
            super::ReasonCode::AuditUnavailable,
            super::PolicyAttribution {
                layer: index,
                grant: None,
                rule,
                denial: super::denial_bytes(
                    &self.layers[usize::from(index)].manifest.hash,
                    "",
                    rule,
                ),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::{assemble, manifest, AuthorityTier, OwnOrigin};
    use super::*;

    fn choosing(mode: &str) -> manifest::Manifest {
        manifest::parse(&format!(r#"{{"schema":4,"name":"test","version":"1","grants":[],"config":[{{"key":"{MODE_KEY}","value":"{mode}","level":"mandatory"}}]}}"#), "test").unwrap()
    }

    fn snapshot(
        organization: Option<manifest::Manifest>,
        user: Option<manifest::Manifest>,
    ) -> AuthoritySnapshot {
        assemble(
            [
                (organization, AuthorityTier::Managed),
                (user, AuthorityTier::User),
            ],
            None,
            None,
            true,
            &OwnOrigin::default(),
        )
    }

    #[test]
    fn either_layer_can_require_audit_and_user_cannot_relax_the_organization() {
        let strict = || Some(choosing("require_audit"));
        let permissive = || Some(choosing("keep_working"));
        assert_eq!(AuditPolicy::default().mode, AuditMode::KeepWorking);
        let policy = snapshot(strict(), permissive()).audit_policy().clone();
        assert_eq!(
            policy,
            AuditPolicy {
                mode: AuditMode::RequireAudit,
                source: Some(LayerKind::Organization),
                organization_mode: Some(AuditMode::RequireAudit),
            }
        );
        assert_eq!(
            snapshot(permissive(), strict()).audit_policy().source,
            Some(LayerKind::User)
        );
        assert_eq!(
            snapshot(None, strict()).audit_policy().mode,
            AuditMode::RequireAudit
        );
        assert_eq!(
            snapshot(None, permissive()).audit_policy().mode,
            AuditMode::KeepWorking
        );
        assert_eq!(snapshot(None, None).audit_policy(), &AuditPolicy::default());
        assert_eq!(
            serde_json::from_str::<AuditMode>("\"require_audit\"").unwrap(),
            AuditMode::RequireAudit
        );
        assert!(manifest::parse(&serde_json::to_string(&serde_json::json!({"schema":4,"name":"test","version":"1","grants":[],"config":[{"key":MODE_KEY,"value":"silent","level":"mandatory"}]})).unwrap(), "test").is_err());
    }
}
