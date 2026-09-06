//! Retained outcome language: closed failure metadata and summaries independent of client payloads.

use serde::{Deserialize, Deserializer, Serialize};

use super::outcome::{BlockedReason, BrowserRecoveryReason, Outcome, Refusal, WorkspaceReason};

/// The explicitly permitted language projection of one terminal result.
///
/// Only outcome/refusal constructors can author this value. There is no constructor from a
/// result envelope, arbitrary summary, or JSON. Measurements remain in `Observed`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditProjection {
    summary: String,
    refusal: Option<AuditRefusal>,
}

impl AuditProjection {
    /// Read the language-authored retained sentence.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
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
    AuthorityBlocked { cause: BlockedReason },
    AttentionRequired,
    LocalInterlock,
    CredentialHandoff,
    IncompatibleReceipt,
    BrowserAdapterOutdated,
    DeadlineExpired { before_dispatch: bool },
    BrowserPrimitiveFailed,
    BrowserStopped { reconnect: bool },
    BrowserAmbiguous,
    BrowserUnknown,
    BrowserPinned,
    BrowserStartupManual,
    BrowserRecoveryFailed { cause: BrowserRecoveryReason },
    EffectUnknown,
    LandingDeniedUnknown,
    WorkspaceUnusable { cause: WorkspaceReason },
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
            summary: self.summary(),
            refusal: None,
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
            Self::AuthorityBlocked { reason, .. } => {
                AuditRefusal::AuthorityBlocked { cause: *reason }
            }
            Self::AttentionRequired => AuditRefusal::AttentionRequired,
            Self::LocalInterlock => AuditRefusal::LocalInterlock,
            Self::CredentialHandoff => AuditRefusal::CredentialHandoff,
            Self::IncompatibleReceipt => AuditRefusal::IncompatibleReceipt,
            Self::BrowserAdapterOutdated => AuditRefusal::BrowserAdapterOutdated,
            Self::DeadlineExpired { before_dispatch } => AuditRefusal::DeadlineExpired {
                before_dispatch: *before_dispatch,
            },
            Self::BrowserPrimitive { .. } => AuditRefusal::BrowserPrimitiveFailed,
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
            Self::EffectUnknown => AuditRefusal::EffectUnknown,
            Self::LandingDeniedUnknown => AuditRefusal::LandingDeniedUnknown,
            Self::WorkspaceUnusable { reason } => {
                AuditRefusal::WorkspaceUnusable { cause: *reason }
            }
            Self::FilesUnreadable => AuditRefusal::FilesUnreadable,
            Self::CaptureTooLarge => AuditRefusal::CaptureTooLarge,
            Self::NoDialogVisible => AuditRefusal::NoDialogVisible,
            Self::RecordingUnavailable => AuditRefusal::RecordingUnavailable,
            Self::RecordingExportFailed => AuditRefusal::RecordingExportFailed,
        };
        AuditProjection {
            summary: match self {
                Self::BrowserPrimitive { .. } => {
                    "The browser could not complete this operation.".into()
                }
                _ => self.summary(),
            },
            refusal: Some(refusal),
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
    fn retained_refusal_has_no_browser_text_but_client_detail_survives() {
        let refusal = Refusal::BrowserPrimitive {
            detail: "PRIVATE_EXCEPTION".into(),
        };
        assert!(refusal.summary().contains("PRIVATE_EXCEPTION"));
        assert!(!refusal.audit().summary().contains("PRIVATE_EXCEPTION"));
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
