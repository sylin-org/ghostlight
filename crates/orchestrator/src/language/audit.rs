//! Retained outcome language: closed failure metadata and summaries independent of client payloads.

use serde::{Deserialize, Deserializer, Serialize};

use super::composition::CompositionProgress;
use super::outcome::{BlockedReason, BrowserRecoveryReason, Outcome, Refusal, WorkspaceReason};

/// The explicitly permitted language projection of one terminal result.
///
/// Only outcome/refusal constructors can author this value. There is no constructor from a
/// result envelope, arbitrary summary, or JSON. Measurements remain in `Observed`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditProjection {
    unconfirmed_history_steps: u32,
    coverage: Option<super::coverage::Coverage>,
    pub(super) summary: String,
    pub(super) next_steps: Vec<String>,
    pub(super) refusal: Option<AuditRefusal>,
    pub(super) composition: Option<CompositionProgress>,
    tools: Vec<String>,
    pub(super) resolution: Option<super::resolution::RetainedResolution>,
}

impl AuditProjection {
    /// Read recovery language from the same typed outcome that authored the summary.
    #[must_use]
    pub fn next_steps(&self) -> &[String] {
        &self.next_steps
    }

    /// Read the canonical frozen account and its human presentation, when this is a new receipt.
    #[must_use]
    pub fn resolution(&self) -> Option<&super::resolution::RetainedResolution> {
        self.resolution.as_ref()
    }
    /// Retain child storage gaps without copying child payloads.
    pub fn with_unconfirmed_history(mut self, steps: u32) -> Self {
        self.unconfirmed_history_steps = steps;
        self
    }
    /// Number of child receipts whose storage was not confirmed.
    #[must_use]
    pub const fn unconfirmed_history_steps(&self) -> u32 {
        self.unconfirmed_history_steps
    }
    /// Qualify retained language with closed coverage facts, never arbitrary browser text.
    pub fn with_coverage(mut self, coverage: &super::coverage::Coverage) -> Self {
        self.summary = super::coverage::qualify(&self.summary, coverage);
        self.coverage = Some(coverage.clone());
        self
    }

    /// Read bounded document coverage without volatile human-only host details.
    #[must_use]
    pub fn coverage(&self) -> Option<&super::coverage::Coverage> {
        self.coverage.as_ref()
    }
    /// Read the language-authored retained sentence.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Read the payload-free composition account, when this is a flow or sequence.
    #[must_use]
    pub fn composition(&self) -> Option<CompositionProgress> {
        self.composition
    }

    /// Attach only canonical catalog names, bounded by the composition contract.
    #[must_use]
    pub fn with_tools(mut self, tools: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.tools = tools
            .into_iter()
            .take(super::history::COMPOSITION_STEP_LIMIT)
            .map(|name| tool_name(name.as_ref()).into())
            .collect();
        self
    }

    /// Read the retained composition plan, without caller step labels or arguments.
    #[must_use]
    pub fn tools(&self) -> &[String] {
        &self.tools
    }

    /// Read the closed refusal metadata, when this outcome is a refusal.
    #[must_use]
    pub fn refusal(&self) -> Option<&AuditRefusal> {
        self.refusal.as_ref()
    }
}

/// Failure facts permitted in durable history. No variant can hold arbitrary text or results.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuditRefusal {
    InvalidRequest,
    CancelledBeforeStart,
    DeadlineBeforeStart,
    Capacity,
    AuthorityBlocked {
        cause: BlockedReason,
    },
    RequestRestricted {
        cause: BlockedReason,
    },
    AttentionRequired,
    LocalInterlock,
    CredentialHandoff,
    CredentialAuthorization,
    IncompatibleReceipt,
    BrowserAdapterOutdated,
    BrowserAttentionProtected {
        cause: ghostlight_bridge::browser::BrowserAttentionReason,
    },
    DeadlineExpired {
        before_dispatch: bool,
    },
    BrowserPrimitiveFailed,
    DocumentUnavailable,
    BrowserStopped {
        reconnect: bool,
    },
    BrowserAmbiguous,
    BrowserUnknown,
    BrowserPinned,
    BrowserStartupManual,
    BrowserRecoveryFailed {
        cause: BrowserRecoveryReason,
    },
    ConnectionLost,
    CancelledAfterDispatch,
    EffectUnknown,
    ScriptException,
    ExpectedConditionNotMet,
    LandingDeniedUnknown,
    WorkspaceUnusable {
        cause: WorkspaceReason,
    },
    FilesUnreadable,
    CaptureTooLarge,
    NoDialogVisible,
    RecordingUnavailable,
    RecordingExportFailed,
}

impl Outcome {
    /// Project a completed operation's authored sentence without copying its result facts.
    #[must_use]
    pub fn audit(&self) -> AuditProjection {
        AuditProjection {
            unconfirmed_history_steps: 0,
            coverage: None,
            summary: self.summary(),
            next_steps: self.next_steps(),
            refusal: None,
            tools: vec![],
            resolution: None,
            composition: match self {
                Self::CompositionRan(progress) => Some(*progress),
                _ => None,
            },
        }
    }
}

impl Refusal {
    /// Project a refusal into durable history without retaining browser-authored error text.
    #[must_use]
    pub fn audit(&self) -> AuditProjection {
        let refusal = match self {
            Self::InvalidRequest => AuditRefusal::InvalidRequest,
            Self::CancelledBeforeStart => AuditRefusal::CancelledBeforeStart,
            Self::DeadlineBeforeStart => AuditRefusal::DeadlineBeforeStart,
            Self::Capacity => AuditRefusal::Capacity,
            Self::AuthorityBlocked { reason, .. } => {
                AuditRefusal::AuthorityBlocked { cause: *reason }
            }
            Self::AttentionRequired => AuditRefusal::AttentionRequired,
            Self::LocalInterlock => AuditRefusal::LocalInterlock,
            Self::CredentialAuthorization => AuditRefusal::CredentialAuthorization,
            Self::IncompatibleReceipt => AuditRefusal::IncompatibleReceipt,
            Self::BrowserAdapterOutdated => AuditRefusal::BrowserAdapterOutdated,
            Self::BrowserAttentionProtected { reason } => {
                AuditRefusal::BrowserAttentionProtected { cause: *reason }
            }
            Self::DeadlineExpired { before_dispatch } => AuditRefusal::DeadlineExpired {
                before_dispatch: *before_dispatch,
            },
            Self::BrowserPrimitive => AuditRefusal::BrowserPrimitiveFailed,
            Self::DocumentUnavailable => AuditRefusal::DocumentUnavailable,
            Self::BrowserStopped { reconnect } => AuditRefusal::BrowserStopped {
                reconnect: *reconnect,
            },
            Self::BrowserAmbiguous => AuditRefusal::BrowserAmbiguous,
            Self::BrowserUnknown => AuditRefusal::BrowserUnknown,
            Self::BrowserPinned => AuditRefusal::BrowserPinned,
            Self::BrowserStartupManual { .. } => AuditRefusal::BrowserStartupManual,
            Self::BrowserRecoveryFailed { reason } => {
                AuditRefusal::BrowserRecoveryFailed { cause: *reason }
            }
            Self::ConnectionLost => AuditRefusal::ConnectionLost,
            Self::CancelledAfterDispatch => AuditRefusal::CancelledAfterDispatch,
            Self::EffectUnknown => AuditRefusal::EffectUnknown,
            Self::ScriptException => AuditRefusal::ScriptException,
            Self::ExpectedConditionNotMet => AuditRefusal::ExpectedConditionNotMet,
            Self::LandingDeniedUnknown => AuditRefusal::LandingDeniedUnknown,
            Self::WorkspaceUnusable { reason } => AuditRefusal::WorkspaceUnusable {
                cause: reason.clone(),
            },
            Self::FilesUnreadable => AuditRefusal::FilesUnreadable,
            Self::CaptureTooLarge => AuditRefusal::CaptureTooLarge,
            Self::NoDialogVisible => AuditRefusal::NoDialogVisible,
            Self::RecordingUnavailable => AuditRefusal::RecordingUnavailable,
            Self::RecordingExportFailed => AuditRefusal::RecordingExportFailed,
        };
        AuditProjection {
            unconfirmed_history_steps: 0,
            coverage: None,
            summary: self.summary(),
            refusal: Some(refusal),
            next_steps: self.next_steps(),
            tools: vec![],
            composition: None,
            resolution: None,
        }
    }
}

/// Read legacy audit failure facts without keeping their arbitrary payloads.
///
/// Historical records predate the closed projection. Retain recognized closed metadata and
/// discard arbitrary details; unrecognized shapes omit this optional field, preserving the record.
pub fn read_refusal<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<AuditRefusal>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| serde_json::from_value(value).ok()))
}

/// Retain only names from the authored action directory, including on input decode failure.
#[must_use]
pub fn tool_name(requested: &str) -> &'static str {
    super::capability_map::DIRECTORY
        .iter()
        .find(|entry| entry.tool == requested)
        .map_or("unknown_tool", |entry| entry.tool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::outcome::{ActionSubject, TargetRole};

    #[test]
    fn retained_language_preserves_useful_context_and_governed_names() {
        for preserve in [true, false] {
            let outcome = Outcome::TargetClicked {
                host: Some("example.com".into()),
                subject: ActionSubject::from_page("button", "Save record", preserve),
            };
            let audit = outcome.audit();
            assert_eq!(audit.summary(), outcome.summary());
            assert_eq!(audit.summary().contains("Save record"), preserve);
            assert!(audit.summary().contains("button on example.com"));
        }
        let outcome = Outcome::TextTyped {
            host: Some("example.com".into()),
            subject: ActionSubject::unnamed(TargetRole::Textbox),
            characters: 12,
        };
        assert_eq!(outcome.audit().summary(), outcome.summary());
        assert_eq!(outcome.observed().count, Some(12));
    }

    #[test]
    fn retained_refusal_cannot_hold_browser_error_text() {
        let refusal = Refusal::BrowserPrimitive;
        assert_eq!(
            refusal.summary(),
            "The browser could not complete this operation."
        );
        assert_eq!(refusal.audit().summary(), refusal.summary());
        assert_eq!(refusal.audit().next_steps(), refusal.next_steps());
        assert_eq!(
            serde_json::to_value(refusal.audit().refusal()).unwrap(),
            serde_json::json!({"reason":"browser_primitive_failed"})
        );
    }

    #[test]
    fn every_catalog_tool_has_a_stable_audit_name() {
        for tool in super::super::catalog() {
            assert_eq!(tool_name(&tool.name), tool.name);
        }
        assert_eq!(tool_name("PRIVATE_UNKNOWN_TOOL"), "unknown_tool");
    }
}
