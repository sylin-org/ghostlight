//! Retained outcome language: closed failure metadata and summaries independent of client payloads.

use serde::{Deserialize, Deserializer, Serialize};

use super::composition::CompositionProgress;
use super::outcome::{
    counted, BlockedReason, BrowserRecoveryReason, BrowserUnavailableReason, CheckFailure, Outcome,
    Refusal, WorkspaceReason,
};
use super::page_refusal::{PageGesture, PageRefusal};
use crate::intake::session::IntakeChannel;

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
    /// Say which kind of act a browser failure could not do, in history's content-free words. Any
    /// other projection is returned unchanged.
    #[must_use]
    pub fn for_gesture(mut self, gesture: PageGesture) -> Self {
        if self.refusal == Some(AuditRefusal::BrowserPrimitiveFailed) {
            self.summary = gesture.could_not().into();
        }
        self
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
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum AuditRefusal {
    InvalidRequest,
    CancelledBeforeStart,
    DeadlineBeforeStart,
    Capacity,
    AuthorityBlocked { cause: BlockedReason },
    ChannelOff { channel: IntakeChannel },
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
    /// This outcome as durable history may keep it (ADR-0198 rule 3, ADR-0203 Decision 4.1).
    ///
    /// What a fill read back and a dialog's message are page-authored text, so they go. A dialog
    /// the action left open retains the action it wraps, so a wrapper never carries past this
    /// rule what the bare outcome could not. Every other outcome's sentence is Ghostlight's own,
    /// with a page name only where the governed snapshot kept it.
    #[must_use]
    pub fn retained(&self) -> Self {
        match self {
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
            },
            Self::DialogObserved { present, .. } => Self::DialogObserved {
                present: *present,
                message: None,
            },
            Self::DialogOpened { action, dialog } => Self::DialogOpened {
                action: Box::new(action.retained()),
                dialog: *dialog,
            },
            Self::TabsListed { .. }
            | Self::PolicyExplained { .. }
            | Self::SelectorUnresolved { .. }
            | Self::CompositionRan(_)
            | Self::CompositionProgress { .. }
            | Self::CompositionUnrecorded
            | Self::StepNotStarted
            | Self::DocumentInspected { .. }
            | Self::TabActivated { .. }
            | Self::PageOpened { .. }
            | Self::PageReused { .. }
            | Self::PageRecovered { .. }
            | Self::TabAlreadyClosed
            | Self::PageNavigated { .. }
            | Self::HistoryTraversed { .. }
            | Self::PageReloaded { .. }
            | Self::TabClosed
            | Self::TextRead { .. }
            | Self::TargetsListed { .. }
            | Self::Captured { .. }
            | Self::TargetClicked { .. }
            | Self::PointClicked { .. }
            | Self::PageScrolled { .. }
            | Self::TargetRevealed { .. }
            | Self::ZoomSet { .. }
            | Self::WindowResized { .. }
            | Self::Hovered { .. }
            | Self::TextTyped { .. }
            | Self::KeyboardSent { .. }
            | Self::Dragged { .. }
            | Self::FilesUploaded { .. }
            | Self::ScriptEvaluated { .. }
            | Self::Waited { .. }
            | Self::DialogHandled { .. }
            | Self::DiagnosticsRead { .. }
            | Self::RecordingStarted { .. }
            | Self::RecordingObserved { .. }
            | Self::RecordingStopped { .. }
            | Self::RecordingBoundary { .. }
            | Self::RecordingSaved { .. }
            | Self::RecordingDiscarded => self.clone(),
        }
    }

    /// Project a completed operation into durable history: the sentence of its retained form,
    /// and never its result facts.
    #[must_use]
    pub fn audit(&self) -> AuditProjection {
        let retained = self.retained();
        AuditProjection {
            unconfirmed_history_steps: 0,
            coverage: None,
            summary: retained.summary(),
            refusal: None,
            tools: vec![],
            composition: match retained {
                Self::CompositionRan(progress) => Some(progress),
                _ => None,
            },
        }
    }
}

impl WorkspaceReason {
    /// This reason as durable history may keep it. The name another session's client claimed
    /// is that client's word, not Ghostlight's, so an ownership refusal names no one.
    #[must_use]
    pub fn retained(&self) -> Self {
        match self {
            Self::OwnershipMismatch { .. } => Self::OwnershipMismatch { owner: None },
            Self::NoTab
            | Self::AmbiguousTab
            | Self::TabUnavailable
            | Self::StaleTarget { .. }
            | Self::StaleView
            | Self::TabHeld { .. }
            | Self::WorkspaceBusy
            | Self::WorkspaceClosed => self.clone(),
        }
    }
}

impl Refusal {
    /// This refusal as durable history may keep it (ADR-0198 rule 3, ADR-0203 Decision 4.1).
    ///
    /// The browser's own words, a credential control's name and handle, the address of
    /// Ghostlight's own control page and the name another session's client claimed all go. An
    /// unconfirmed action retains the action it wraps, so a wrapper never carries past this rule
    /// what the bare outcome could not. A fill the page refused or undid keeps its fields only so
    /// that its durable sentence can count them.
    #[must_use]
    pub fn retained(&self) -> Self {
        match self {
            Self::AuthorityBlocked { reason, host, .. } => Self::AuthorityBlocked {
                reason: *reason,
                host: host.clone(),
                origin: None,
            },
            Self::CredentialAuthorization { control } => Self::CredentialAuthorization {
                control: control.unnamed(),
            },
            Self::BrowserPrimitive { .. } => Self::BrowserPrimitive {
                detail: String::new(),
            },
            Self::WorkspaceUnusable { reason } => Self::WorkspaceUnusable {
                reason: reason.retained(),
            },
            Self::ActionUnconfirmed { action, cause } => Self::ActionUnconfirmed {
                action: Box::new(action.retained()),
                cause: *cause,
            },
            Self::Capacity
            | Self::InvalidRequest
            | Self::CancelledBeforeStart { .. }
            | Self::DeadlineBeforeStart
            | Self::ChannelOff { .. }
            | Self::LocalInterlock
            | Self::IncompatibleReceipt
            | Self::BrowserAdapterOutdated
            | Self::DeadlineExpired { .. }
            | Self::PageRefused { .. }
            | Self::FieldsReverted { .. }
            | Self::FieldValuesRejected { .. }
            | Self::DocumentUnavailable
            | Self::CancelledBeforeDispatch { .. }
            | Self::BrowserUnavailable { .. }
            | Self::BrowserContract
            | Self::BrowserAmbiguous
            | Self::BrowserUnknown
            | Self::BrowserPinned
            | Self::BrowserStartupManual { .. }
            | Self::BrowserRecoveryFailed { .. }
            | Self::ConnectionLost
            | Self::CancelledAfterDispatch { .. }
            | Self::EffectUnknown
            | Self::LandingDeniedUnknown
            | Self::FilesUnreadable
            | Self::CaptureTooLarge
            | Self::TabNotShowing
            | Self::NoDialogVisible
            | Self::DialogOpen
            | Self::RecordingUnavailable
            | Self::RecordingHeld
            | Self::RecordingExportFailed
            | Self::RecordingNotAttached => self.clone(),
        }
    }

    /// Project a refusal into durable history: the closed facts and the sentence of its retained
    /// form.
    #[must_use]
    pub fn audit(&self) -> AuditProjection {
        let retained = self.retained();
        AuditProjection {
            unconfirmed_history_steps: 0,
            coverage: None,
            summary: retained.durable_summary(),
            refusal: Some(retained.closed_facts()),
            tools: vec![],
            composition: None,
        }
    }

    /// The sentence durable history keeps for a retained refusal.
    ///
    /// It is the refusal's own sentence, apart from the refusals whose sentence is made of what
    /// history never keeps: the browser's own words, the fields a fill could not set and the
    /// credential control. Those say what kind of thing was refused, and how many.
    fn durable_summary(&self) -> String {
        match self {
            Self::BrowserPrimitive { .. } => PageGesture::Other.could_not().into(),
            Self::FieldValuesRejected { fields, .. } => format!(
                "Refused a form fill before any edit: {} could not take {}.",
                counted(fields.len(), "field", "fields"),
                if fields.len() == 1 {
                    "its value"
                } else {
                    "their values"
                },
            ),
            Self::FieldsReverted { set, .. } => format!(
                "A form fill was partly applied: {} held {}.",
                counted(*set, "field", "fields"),
                if *set == 1 {
                    "its value"
                } else {
                    "their values"
                },
            ),
            Self::CredentialAuthorization { .. } => {
                "Refused credential input that the user had not authorized.".into()
            }
            retained => retained.summary(),
        }
    }

    /// The closed failure facts of a retained refusal.
    fn closed_facts(&self) -> AuditRefusal {
        match self {
            Self::InvalidRequest => AuditRefusal::InvalidRequest,
            Self::CancelledBeforeStart { .. } => AuditRefusal::CancelledBeforeStart,
            Self::DeadlineBeforeStart => AuditRefusal::DeadlineBeforeStart,
            Self::Capacity => AuditRefusal::Capacity,
            Self::AuthorityBlocked { reason, .. } => {
                AuditRefusal::AuthorityBlocked { cause: *reason }
            }
            Self::ChannelOff { channel, .. } => AuditRefusal::ChannelOff { channel: *channel },
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
            Self::CancelledBeforeDispatch { .. } => AuditRefusal::CancelledBeforeDispatch,
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
            Self::CancelledAfterDispatch { .. } => AuditRefusal::CancelledAfterDispatch,
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
    use crate::language::field_echo::HeldField;
    use crate::language::field_problem::ControlRef;
    use crate::language::outcome::{ActionSubject, DialogKind, StaleHelp, TargetRole};
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

    /// A fill whose page read back a choice and a reformatted text, with names preserved, so the
    /// agent's sentence carries both readings.
    fn read_back_fill() -> Outcome {
        Outcome::FormFilled {
            fields: 2,
            submitted: true,
            host: Some("example.com".into()),
            held: vec![
                HeldField::new(
                    "target_7k2nb5xe",
                    TargetRole::Combobox,
                    "Plan",
                    "PRIVATE_CHOICE",
                    false,
                    true,
                ),
                HeldField::new(
                    "target_8m3pc6yf",
                    TargetRole::Textbox,
                    "Phone",
                    "PRIVATE_READING",
                    true,
                    true,
                ),
            ],
            rebound: false,
        }
    }

    /// Every way a check after an action can fall short.
    const CHECK_FAILURES: [CheckFailure; 10] = [
        CheckFailure::Unmet,
        CheckFailure::Blocked(BlockedReason::Host),
        CheckFailure::Blocked(BlockedReason::Hold),
        CheckFailure::Blocked(BlockedReason::SessionEnded),
        CheckFailure::Cancelled,
        CheckFailure::DeadlineExpired,
        CheckFailure::Disconnected,
        CheckFailure::BrowserUnavailable,
        CheckFailure::DocumentUnavailable,
        CheckFailure::BrowserRefused,
    ];

    #[test]
    fn no_page_text_reaches_the_audit_through_any_wrapper() {
        let cases = [
            (read_back_fill(), vec!["PRIVATE_CHOICE", "PRIVATE_READING"]),
            (
                Outcome::DialogObserved {
                    present: true,
                    message: Some("Delete PRIVATE_DIALOG?".into()),
                },
                vec!["PRIVATE_DIALOG"],
            ),
        ];
        for (bare, sentinels) in cases {
            let wrapped = bare.clone().with_open_dialog(DialogKind::Confirm);
            let mut accounts = vec![
                (bare.summary(), bare.audit()),
                (wrapped.summary(), wrapped.audit()),
            ];
            for cause in CHECK_FAILURES {
                for action in [&bare, &wrapped] {
                    let refusal = Refusal::ActionUnconfirmed {
                        action: Box::new(action.clone()),
                        cause,
                    };
                    accounts.push((refusal.summary(), refusal.audit()));
                }
            }
            for (said, audit) in accounts {
                let kept = serde_json::to_string(&audit.refusal()).unwrap();
                for sentinel in &sentinels {
                    assert!(said.contains(sentinel), "the agent reads it: {said}");
                    assert!(!audit.summary().contains(sentinel), "{}", audit.summary());
                    assert!(!kept.contains(sentinel), "{kept}");
                }
            }
        }
        let fill = read_back_fill();
        assert_eq!(
            fill.audit().summary(),
            "Filled 2 fields on example.com and submitted the form."
        );
        assert_eq!(
            fill.clone()
                .with_open_dialog(DialogKind::Confirm)
                .audit()
                .summary(),
            "Filled 2 fields on example.com and submitted the form. A confirm dialog is now open."
        );
        assert_eq!(
            Refusal::ActionUnconfirmed {
                action: Box::new(fill),
                cause: CheckFailure::Unmet,
            }
            .audit()
            .summary(),
            "The expected condition did not hold after the action. Filled 2 fields on example.com and submitted the form."
        );
    }

    #[test]
    fn an_ownership_refusal_writes_no_other_clients_name() {
        let refusal = Refusal::WorkspaceUnusable {
            reason: WorkspaceReason::OwnershipMismatch {
                owner: Some("PRIVATE_CLIENT".into()),
            },
        };
        assert!(refusal.summary().contains("PRIVATE_CLIENT"));
        let audit = refusal.audit();
        assert_eq!(
            audit.summary(),
            "That handle belongs to a different Ghostlight session."
        );
        assert_eq!(
            serde_json::to_value(audit.refusal()).unwrap(),
            serde_json::json!({
                "reason": "workspace_unusable",
                "cause": {"ownership_mismatch": {"owner": null}}
            })
        );
    }

    #[test]
    fn the_retained_refusal_drops_the_control_page_address_the_credential_name_and_the_browsers_words(
    ) {
        let own_page = Refusal::AuthorityBlocked {
            reason: BlockedReason::OwnWorkbench,
            host: Some("127.0.0.1".into()),
            origin: Some("http://127.0.0.1:41259".into()),
        };
        assert!(own_page.summary().contains(":41259"));
        assert!(!own_page.retained().summary().contains(":41259"));
        assert!(!own_page.audit().summary().contains(":41259"));
        assert_eq!(own_page.observed().host.as_deref(), Some("127.0.0.1"));

        let credential = Refusal::CredentialAuthorization {
            control: ControlRef::new(
                TargetRole::Textbox,
                "PRIVATE_NAME",
                Some("target_7k2nb5xe"),
                true,
            ),
        };
        assert!(credential.summary().contains("PRIVATE_NAME"));
        for kept in [
            credential.retained().summary(),
            credential.audit().summary().into(),
        ] {
            assert!(!kept.contains("PRIVATE_NAME"), "{kept}");
            assert!(!kept.contains("target_"), "{kept}");
        }

        let primitive = Refusal::BrowserPrimitive {
            detail: "PRIVATE_EXCEPTION".into(),
        };
        assert!(!primitive.retained().summary().contains("PRIVATE_EXCEPTION"));
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
    fn a_browser_failure_keeps_the_kind_of_act_it_could_not_do_and_nothing_of_the_page() {
        let refusal = Refusal::BrowserPrimitive {
            detail: "PRIVATE_EXCEPTION".into(),
        };
        let typed = refusal.audit().for_gesture(PageGesture::Type);
        assert_eq!(
            typed.summary(),
            "The browser could not type into that field."
        );
        assert_eq!(typed.refusal(), refusal.audit().refusal());
        assert_eq!(
            refusal.audit().for_gesture(PageGesture::Other).summary(),
            refusal.audit().summary()
        );
        let other = Refusal::BrowserContract.audit();
        assert_eq!(
            other.clone().for_gesture(PageGesture::Type).summary(),
            other.summary(),
            "only a browser failure takes the gesture's words"
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
