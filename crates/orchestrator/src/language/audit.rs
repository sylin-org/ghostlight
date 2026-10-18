//! Retained outcome language: closed failure metadata and summaries independent of client payloads.

use serde::{Deserialize, Deserializer, Serialize};

use super::composition::CompositionProgress;
use super::outcome::{
    counted, BlockedReason, BrowserRecoveryReason, BrowserUnavailableReason, CheckFailure, Outcome,
    Refusal, WorkspaceReason,
};
use super::page_refusal::PageRefusal;

/// The explicitly permitted language projection of one terminal result.
///
/// Only outcome/refusal constructors can author this value. There is no constructor from a
/// result envelope, arbitrary summary, or JSON. Measurements remain in `Observed`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditProjection {
    unconfirmed_history_steps: u32,
    coverage: Option<super::coverage::Coverage>,
    summary: String,
    refusal: Option<AuditRefusal>,
    composition: Option<CompositionProgress>,
    tools: Vec<String>,
}

impl AuditProjection {
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
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum AuditRefusal {
    InvalidRequest,
    CancelledBeforeStart,
    DeadlineBeforeStart,
    Capacity,
    AuthorityBlocked { cause: BlockedReason },
    LocalInterlock,
    CredentialHandoff,
    CredentialAuthorization,
    IncompatibleReceipt,
    BrowserAdapterOutdated,
    DeadlineExpired { before_dispatch: bool },
    BrowserPrimitiveFailed,
    PageRefused { cause: PageRefusal },
    FieldValuesRejected,
    FieldsReverted,
    DocumentUnavailable,
    CancelledBeforeDispatch,
    BrowserUnavailable { cause: BrowserUnavailableReason },
    BrowserContract,
    BrowserAmbiguous,
    BrowserUnknown,
    BrowserPinned,
    BrowserStartupManual,
    BrowserRecoveryFailed { cause: BrowserRecoveryReason },
    ConnectionLost,
    CancelledAfterDispatch,
    EffectUnknown,
    LandingDeniedUnknown,
    WorkspaceUnusable { cause: WorkspaceReason },
    FilesUnreadable,
    CaptureTooLarge,
    TabNotShowing,
    NoDialogVisible,
    DialogOpen,
    RecordingUnavailable,
    RecordingHeld,
    RecordingExportFailed,
    RecordingNotAttached,
    ActionUnconfirmed { cause: CheckFailure },
}

impl Outcome {
    /// Project a completed operation's authored sentence without copying its result facts.
    ///
    /// A dialog message is page-authored text, so the durable sentence omits it. So are the
    /// values and names a fill read back.
    #[must_use]
    pub fn audit(&self) -> AuditProjection {
        let summary = match self {
            Self::DialogObserved { present, .. } => Self::DialogObserved {
                present: *present,
                message: None,
            }
            .summary(),
            Self::FormFilled {
                fields,
                submitted,
                host,
                rebound,
                ..
            } => Self::FormFilled {
                fields: *fields,
                submitted: *submitted,
                host: host.clone(),
                held: Vec::new(),
                rebound: *rebound,
            }
            .summary(),
            _ => self.summary(),
        };
        AuditProjection {
            unconfirmed_history_steps: 0,
            coverage: None,
            summary,
            refusal: None,
            tools: vec![],
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
            Self::LocalInterlock => AuditRefusal::LocalInterlock,
            Self::CredentialAuthorization { .. } => AuditRefusal::CredentialAuthorization,
            Self::IncompatibleReceipt => AuditRefusal::IncompatibleReceipt,
            Self::BrowserAdapterOutdated => AuditRefusal::BrowserAdapterOutdated,
            Self::DeadlineExpired {
                before_dispatch, ..
            } => AuditRefusal::DeadlineExpired {
                before_dispatch: *before_dispatch,
            },
            Self::BrowserPrimitive { .. } => AuditRefusal::BrowserPrimitiveFailed,
            Self::PageRefused { refusal, .. } => AuditRefusal::PageRefused { cause: *refusal },
            Self::FieldValuesRejected { .. } => AuditRefusal::FieldValuesRejected,
            Self::FieldsReverted { .. } => AuditRefusal::FieldsReverted,
            Self::DocumentUnavailable => AuditRefusal::DocumentUnavailable,
            Self::CancelledBeforeDispatch => AuditRefusal::CancelledBeforeDispatch,
            Self::BrowserUnavailable { reason } => {
                AuditRefusal::BrowserUnavailable { cause: *reason }
            }
            Self::BrowserContract => AuditRefusal::BrowserContract,
            Self::BrowserAmbiguous => AuditRefusal::BrowserAmbiguous,
            Self::BrowserUnknown => AuditRefusal::BrowserUnknown,
            Self::BrowserPinned => AuditRefusal::BrowserPinned,
            Self::BrowserStartupManual { .. } => AuditRefusal::BrowserStartupManual,
            Self::BrowserRecoveryFailed { reason, .. } => {
                AuditRefusal::BrowserRecoveryFailed { cause: *reason }
            }
            Self::ConnectionLost => AuditRefusal::ConnectionLost,
            Self::CancelledAfterDispatch => AuditRefusal::CancelledAfterDispatch,
            Self::EffectUnknown => AuditRefusal::EffectUnknown,
            Self::LandingDeniedUnknown => AuditRefusal::LandingDeniedUnknown,
            Self::WorkspaceUnusable { reason } => AuditRefusal::WorkspaceUnusable {
                cause: reason.clone(),
            },
            Self::FilesUnreadable => AuditRefusal::FilesUnreadable,
            Self::CaptureTooLarge => AuditRefusal::CaptureTooLarge,
            Self::TabNotShowing => AuditRefusal::TabNotShowing,
            Self::NoDialogVisible => AuditRefusal::NoDialogVisible,
            Self::DialogOpen => AuditRefusal::DialogOpen,
            Self::RecordingUnavailable => AuditRefusal::RecordingUnavailable,
            Self::RecordingHeld => AuditRefusal::RecordingHeld,
            Self::RecordingExportFailed => AuditRefusal::RecordingExportFailed,
            Self::RecordingNotAttached => AuditRefusal::RecordingNotAttached,
            Self::ActionUnconfirmed { cause, .. } => {
                AuditRefusal::ActionUnconfirmed { cause: *cause }
            }
        };
        AuditProjection {
            unconfirmed_history_steps: 0,
            coverage: None,
            summary: match self {
                Self::BrowserPrimitive { .. } => {
                    "The browser could not complete this operation.".into()
                }
                // The count is the whole of it: names, options and values stay out of history.
                Self::FieldValuesRejected { fields, .. } => format!(
                    "Refused a form fill before any edit: {} could not take {}.",
                    counted(fields.len(), "field", "fields"),
                    if fields.len() == 1 {
                        "its value"
                    } else {
                        "their values"
                    },
                ),
                // Only how many fields held: which ones changed, and to what, stays out of history.
                Self::FieldsReverted { set, .. } => format!(
                    "A form fill was partly applied: {} held {}.",
                    counted(*set, "field", "fields"),
                    if *set == 1 {
                        "its value"
                    } else {
                        "their values"
                    },
                ),
                // A credential refusal names the control; history keeps only that one was refused.
                Self::CredentialAuthorization { .. } => {
                    "Refused credential input that the user had not authorized.".into()
                }
                // The control page is named without its address: history keeps the reason, and the
                // host the refusal already records, never the scheme or the port.
                Self::AuthorityBlocked {
                    reason: BlockedReason::OwnWorkbench,
                    host,
                    ..
                } => Self::AuthorityBlocked {
                    reason: BlockedReason::OwnWorkbench,
                    host: host.clone(),
                    origin: None,
                }
                .summary(),
                _ => self.summary(),
            },
            refusal: Some(refusal),
            tools: vec![],
            composition: None,
        }
    }
}

/// Read audit failure facts without keeping arbitrary payloads.
///
/// Retain recognized closed metadata and discard the rest. A fact this release does not know
/// omits this optional field and preserves the record.
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
    use crate::language::outcome::{ActionSubject, StaleHelp, TargetRole};
    use crate::workspace::StaleReason;

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
    fn retained_dialog_observation_omits_the_page_authored_message() {
        let outcome = Outcome::DialogObserved {
            present: true,
            message: Some("Delete account PRIVATE_DIALOG_TEXT?".into()),
        };
        assert!(outcome.summary().contains("PRIVATE_DIALOG_TEXT"));
        let audit = outcome.audit();
        assert!(!audit.summary().contains("PRIVATE_DIALOG_TEXT"));
        assert!(audit
            .summary()
            .contains("JavaScript dialog is currently visible"));
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
    fn a_page_refusal_audits_its_word_and_the_sentence_that_quotes_no_page() {
        use crate::language::page_refusal::{PageGesture, PageRefusal};
        for refusal in PageRefusal::ALL {
            let refused = Refusal::PageRefused {
                refusal,
                gesture: PageGesture::Click,
            };
            let audit = refused.audit();
            assert_eq!(audit.summary(), refused.summary());
            assert_eq!(
                serde_json::to_value(audit.refusal()).unwrap(),
                serde_json::json!({"reason":"page_refused","cause":refusal.as_str()})
            );
        }
    }

    #[test]
    fn a_stale_target_audits_its_word_and_the_history_holds_the_sentence_only() {
        for reason in StaleReason::ALL {
            let refusal = Refusal::WorkspaceUnusable {
                reason: WorkspaceReason::StaleTarget {
                    reason,
                    help: StaleHelp::None,
                },
            };
            let audit = refusal.audit();
            assert_eq!(audit.summary(), refusal.summary());
            assert_eq!(
                serde_json::to_value(audit.refusal()).unwrap(),
                serde_json::json!({
                    "reason": "workspace_unusable",
                    "cause": {"stale_target": {"reason": reason.word()}}
                })
            );
        }
    }

    #[test]
    fn a_stale_target_audits_no_replacement_and_no_handle() {
        for help in [
            StaleHelp::Candidates { named: true },
            StaleHelp::Candidates { named: false },
            StaleHelp::Replacement {
                tool: "browser_click",
                target: "target_7k2nb5xe".into(),
            },
        ] {
            let refusal = Refusal::WorkspaceUnusable {
                reason: WorkspaceReason::StaleTarget {
                    reason: StaleReason::Removed,
                    help,
                },
            };
            let audit = refusal.audit();
            assert_eq!(
                serde_json::to_value(audit.refusal()).unwrap(),
                serde_json::json!({
                    "reason": "workspace_unusable",
                    "cause": {"stale_target": {"reason": "removed"}}
                })
            );
            assert!(!serde_json::to_value(audit.refusal())
                .unwrap()
                .to_string()
                .contains("target_"));
            assert!(!audit.summary().contains("target_"));
        }
    }

    #[test]
    fn every_catalog_tool_has_a_stable_audit_name() {
        for tool in super::super::catalog() {
            assert_eq!(tool_name(&tool.name), tool.name);
        }
        assert_eq!(tool_name("PRIVATE_UNKNOWN_TOOL"), "unknown_tool");
    }
}
