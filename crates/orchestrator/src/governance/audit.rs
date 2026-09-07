//! Monotonic audit-availability requirement across the existing authority layers.

use super::{effective::LayerKind, manifest::Manifest, AuthoritySnapshot, AuthorityTier};
use serde::{Deserialize, Serialize};

/// Registered choice for continuing work during a known history-storage failure.
pub const MODE_KEY: &str = "audit.availability";

/// A storage requirement is independent of ordinary grant observe mode.
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

impl AuditPolicy {
    /// Resolve the strictest authored choice without relaxing an organization requirement.
    #[must_use]
    pub fn resolve(organization: Option<&Manifest>, user: Option<&Manifest>) -> Self {
        let mut resolved = Self::default();
        for (manifest, source) in [
            (organization, LayerKind::Organization),
            (user, LayerKind::User),
        ] {
            let Some(mode) = manifest
                .and_then(|m| m.string_setting(MODE_KEY))
                .and_then(AuditMode::parse)
            else {
                continue;
            };
            if source == LayerKind::Organization {
                resolved.organization_mode = Some(mode);
            }
            if resolved.source.is_none() || mode > resolved.mode {
                resolved.mode = mode;
                resolved.source = Some(source);
            }
        }
        resolved
    }
}

impl AuthoritySnapshot {
    fn requiring_layer(&self) -> Option<usize> {
        self.layers.iter().position(|layer| {
            matches!(layer.tier, AuthorityTier::Managed | AuthorityTier::User)
                && layer
                    .manifest
                    .string_setting(MODE_KEY)
                    .and_then(AuditMode::parse)
                    == Some(AuditMode::RequireAudit)
        })
    }

    /// Whether this invocation requires a currently working audit destination.
    #[must_use]
    pub fn requires_audit(&self) -> bool {
        self.requiring_layer().is_some()
    }

    /// Admit current storage health with attribution to the actual immutable policy requirement.
    #[must_use]
    pub fn authorize_audit(&self, available: bool) -> super::Decision {
        let Some(index) = self.requiring_layer().filter(|_| !available) else {
            return super::Decision::permitted();
        };
        let rule = super::PolicyRule::AuditAvailability;
        super::Decision::policy(
            super::ReasonCode::AuditUnavailable,
            super::PolicyAttribution {
                layer: u16::try_from(index).expect("bounded authority layers"),
                grant: None,
                rule,
                denial: super::denial_bytes(&self.layers[index].manifest.hash, "", rule),
                mode: super::manifest::PolicyMode::Enforce,
            },
            super::manifest::PolicyMode::Enforce,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest(mode: &str) -> Manifest {
        super::super::manifest::parse(&format!(r#"{{"schema":3,"name":"test","version":"1","grants":[],"config":[{{"key":"{MODE_KEY}","value":"{mode}","level":"mandatory"}}]}}"#), "test").unwrap()
    }
    #[test]
    fn either_layer_can_require_audit_and_user_cannot_relax_the_organization() {
        let strict = manifest("require_audit");
        let permissive = manifest("keep_working");
        assert_eq!(AuditPolicy::default().mode, AuditMode::KeepWorking);
        let policy = AuditPolicy::resolve(Some(&strict), Some(&permissive));
        assert_eq!(policy.mode, AuditMode::RequireAudit);
        assert_eq!(policy.source, Some(LayerKind::Organization));
        assert_eq!(
            AuditPolicy::resolve(Some(&permissive), Some(&strict)).source,
            Some(LayerKind::User)
        );
        assert_eq!(
            AuditPolicy::resolve(None, Some(&strict)).mode,
            AuditMode::RequireAudit
        );
        assert_eq!(
            AuditPolicy::resolve(None, Some(&permissive)).mode,
            AuditMode::KeepWorking
        );
        assert_eq!(
            serde_json::from_str::<AuditMode>("\"require_audit\"").unwrap(),
            AuditMode::RequireAudit
        );
        assert!(super::super::manifest::parse(&serde_json::to_string(&serde_json::json!({"schema":3,"name":"test","version":"1","grants":[],"config":[{"key":MODE_KEY,"value":"silent","level":"mandatory"}]})).unwrap(), "test").is_err());
    }
}
