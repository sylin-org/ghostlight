//! Monotonic audit-availability requirement across the existing authority layers.

use super::effective::{layer_kind, LayerKind};
use super::manifest::PolicySetting;
use super::{AuthoritySnapshot, PolicyLayer};
use serde::{Deserialize, Serialize};

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
                .string_setting(PolicySetting::AuditAvailability)
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
        self.layer_denial(
            super::ReasonCode::AuditUnavailable,
            usize::from(index),
            None,
            super::PolicyRule::AuditAvailability,
        )
    }
}
