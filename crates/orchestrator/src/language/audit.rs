//! Retained outcome language: closed failure metadata and summaries independent of client payloads.

use serde::{Deserialize, Deserializer, Serialize};

use super::composition::CompositionProgress;
use super::outcome::{
    BlockedReason, BrowserRecoveryReason, BrowserUnavailableReason, CheckFailure, Outcome,
    PromptField, Refusal, WorkspaceReason,
};
use super::page_refusal::{PageGesture, PageRefusal};
use super::phrase::counted;
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

    /// Read the payload-free composition account, when this is a flow.
    #[must_use]
    pub fn composition(&self) -> Option<CompositionProgress> {
        self.composition
    }

    /// Attach only canonical catalog names, bounded by the composition contract.
    #[must_use]
    pub fn with_tools(mut self, tools: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.tools = tools
            .into_iter()
            .take(super::composition::COMPOSITION_STEP_LIMIT)
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
    NoHistoryEntry,
    RecordingUnavailable,
    RecordingHeld,
    RecordingExportFailed,
    RecordingNotAttached,
    ActionUnconfirmed { cause: CheckFailure },
}

impl Outcome {
    /// This outcome as durable history may keep it (ADR-0198 rule 3, ADR-0203 Decision 4.1).
    ///
    /// What a fill read back, a dialog's message and a prompt's field are page-authored text, so they
    /// go; a prompt stays a prompt. A dialog
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
            Self::DialogObserved {
                present, prompt, ..
            } => Self::DialogObserved {
                present: *present,
                message: None,
                prompt: prompt.as_ref().map(|_| PromptField::Unreported),
            },
            Self::DialogOpened { action, dialog } => Self::DialogOpened {
                action: Box::new(action.retained()),
                dialog: *dialog,
            },
            Self::TabsListed { .. }
            | Self::PolicyExplained { .. }
            | Self::SelectorUnresolved { .. }
            | Self::CompositionRan(_)
            | Self::StepNotStarted
            | Self::DocumentInspected { .. }
            | Self::TabActivated { .. }
            | Self::PageOpened { .. }
            | Self::PageReused { .. }
            | Self::PageRecovered { .. }
            | Self::TabNotHeld
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
    /// is that client's word, not Ghostlight's, so an ownership refusal names no one. A handle on
    /// another tab keeps its cause and not the tab, as a refusal on any tab names none.
    #[must_use]
    pub fn retained(&self) -> Self {
        match self {
            Self::OwnershipMismatch { .. } => Self::OwnershipMismatch { owner: None },
            Self::HandleOnOtherTab(_) => Self::HandleOnOtherTab(None),
            Self::NoTab
            | Self::AmbiguousTab
            | Self::TabUnavailable
            | Self::PageChanged
            | Self::StaleTarget { .. }
            | Self::StaleView
            | Self::FieldsOnSeveralTabs
            | Self::OutsideImage(_)
            | Self::TabHeld
            | Self::WorkspaceBusy
            | Self::WorkspaceClosed => self.clone(),
        }
    }
}

impl Refusal {
    /// This refusal as durable history may keep it (ADR-0198 rule 3, ADR-0203 Decision 4.1).
    ///
    /// The browser's own words, what a page script threw, a credential control's name and handle,
    /// the address of Ghostlight's own control page and the name another session's client claimed
    /// all go. An unconfirmed action retains the action it wraps, so a wrapper never carries past
    /// this rule what the bare outcome could not. A fill the page refused or undid keeps its
    /// fields only so that its durable sentence can count them.
    #[must_use]
    pub fn retained(&self) -> Self {
        match self {
            Self::AuthorityBlocked { code, host, .. } => Self::AuthorityBlocked {
                code: *code,
                host: host.clone(),
                origin: None,
            },
            Self::CredentialAuthorization { control } => Self::CredentialAuthorization {
                control: control.unnamed(),
            },
            Self::BrowserPrimitive { cause, .. } => Self::BrowserPrimitive {
                detail: String::new(),
                cause: *cause,
            },
            // What a script threw is the script's result, which history never keeps.
            Self::ScriptThrew { .. } => Self::ScriptThrew {
                exception: String::new(),
            },
            Self::WorkspaceUnusable { reason } => Self::WorkspaceUnusable {
                reason: reason.retained(),
            },
            Self::ActionUnconfirmed {
                action,
                cause,
                code,
            } => Self::ActionUnconfirmed {
                action: Box::new(action.retained()),
                cause: *cause,
                code: *code,
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
            | Self::LandingDeniedUnknown { .. }
            | Self::FilesUnreadable { .. }
            | Self::CaptureTooLarge
            | Self::TabNotShowing
            | Self::NoDialogVisible
            | Self::DialogOpen
            | Self::NoHistoryEntry { .. }
            | Self::RecordingUnavailable
            | Self::RecordingHeld
            | Self::RecordingExportFailed
            | Self::RecordingNotAttached
            | Self::TabHeld { .. } => self.clone(),
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
            Self::AuthorityBlocked { code, .. } => AuditRefusal::AuthorityBlocked {
                cause: BlockedReason::of(*code),
            },
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
            Self::EffectUnknown | Self::ScriptThrew { .. } => AuditRefusal::EffectUnknown,
            Self::LandingDeniedUnknown { .. } => AuditRefusal::LandingDeniedUnknown,
            Self::WorkspaceUnusable { reason } => AuditRefusal::WorkspaceUnusable {
                cause: reason.clone(),
            },
            Self::TabHeld { .. } => AuditRefusal::WorkspaceUnusable {
                cause: WorkspaceReason::TabHeld,
            },
            Self::FilesUnreadable { .. } => AuditRefusal::FilesUnreadable,
            Self::CaptureTooLarge => AuditRefusal::CaptureTooLarge,
            Self::TabNotShowing => AuditRefusal::TabNotShowing,
            Self::NoDialogVisible => AuditRefusal::NoDialogVisible,
            Self::DialogOpen => AuditRefusal::DialogOpen,
            Self::NoHistoryEntry { .. } => AuditRefusal::NoHistoryEntry,
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
    use crate::governance::ReasonCode;
    use crate::language::field_echo::HeldField;
    use crate::language::field_problem::ControlRef;
    use crate::language::outcome::{
        model_text, ActionSubject, DialogKind, PrimitiveCause, StaleHelp, TargetRole,
    };
    use crate::workspace::StaleReason;

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

    /// `action` inside the refusal of a check that fell short for `cause`.
    fn unconfirmed(action: &Outcome, cause: CheckFailure) -> Refusal {
        Refusal::ActionUnconfirmed {
            action: Box::new(action.clone()),
            cause,
            code: cause.decision_code(),
        }
    }

    /// ACCEPTANCE Governance 10 and ADR-0203 Decision 4.1: what the agent is told may hold page,
    /// browser or client text; durable history keeps none of it, through any wrapper, and keeps
    /// only a governed target name. Defects 9ce822e5 (the control page's address), 8f2241c0
    /// (another session's name), 0ca89f71 (a prompt's field), L-D6 (what a script threw) and
    /// a60cd481 window-10 (a browser failure names only the kind of act it could not do).
    #[test]
    fn durable_history_keeps_no_page_or_client_text_through_any_wrapper() {
        let outcomes = [
            (read_back_fill(), vec!["PRIVATE_CHOICE", "PRIVATE_READING"]),
            (
                Outcome::DialogObserved {
                    present: true,
                    message: Some("Delete PRIVATE_DIALOG?".into()),
                    prompt: None,
                },
                vec!["PRIVATE_DIALOG"],
            ),
            (
                Outcome::DialogObserved {
                    present: true,
                    message: Some("Your PRIVATE_DIALOG_TEXT?".into()),
                    prompt: Some(PromptField::Holds("PRIVATE_FIELD_TEXT".into())),
                },
                vec!["PRIVATE_DIALOG_TEXT", "PRIVATE_FIELD_TEXT"],
            ),
        ];
        let mut accounts: Vec<(String, AuditProjection, Vec<&str>)> = Vec::new();
        for (bare, sentinels) in outcomes {
            let wrapped = bare.clone().with_open_dialog(DialogKind::Confirm);
            for action in [&bare, &wrapped] {
                let said = model_text(&action.summary(), &action.next_steps());
                accounts.push((said, action.audit(), sentinels.clone()));
                for cause in CHECK_FAILURES {
                    let refusal = unconfirmed(action, cause);
                    let said = model_text(&refusal.summary(), &refusal.next_steps());
                    accounts.push((said, refusal.audit(), sentinels.clone()));
                }
            }
        }
        let own_page = Refusal::AuthorityBlocked {
            code: ReasonCode::OwnWorkbench,
            host: Some("127.0.0.1".into()),
            origin: Some("http://127.0.0.1:41259".into()),
        };
        assert_eq!(own_page.observed().host.as_deref(), Some("127.0.0.1"));
        let refusals = [
            (
                Refusal::ScriptThrew {
                    exception: "Error: PRIVATE_SCRIPT_TEXT".into(),
                },
                vec!["PRIVATE_SCRIPT_TEXT"],
            ),
            (
                Refusal::BrowserPrimitive {
                    detail: "PRIVATE_EXCEPTION".into(),
                    cause: PrimitiveCause::Refused,
                },
                vec!["PRIVATE_EXCEPTION"],
            ),
            (
                Refusal::CredentialAuthorization {
                    control: ControlRef::new(
                        TargetRole::Textbox,
                        "PRIVATE_NAME",
                        Some("target_7k2nb5xe"),
                        true,
                    ),
                },
                vec!["PRIVATE_NAME", "target_7k2nb5xe"],
            ),
            (
                Refusal::WorkspaceUnusable {
                    reason: WorkspaceReason::OwnershipMismatch {
                        owner: Some("PRIVATE_CLIENT".into()),
                    },
                },
                vec!["PRIVATE_CLIENT"],
            ),
            (
                Refusal::WorkspaceUnusable {
                    reason: WorkspaceReason::HandleOnOtherTab(Some("tab_7k2nb5xe".into())),
                },
                vec!["tab_7k2nb5xe"],
            ),
            (own_page, vec![":41259"]),
            (
                Refusal::WorkspaceUnusable {
                    reason: WorkspaceReason::StaleTarget {
                        reason: StaleReason::Removed,
                        help: StaleHelp::Replacement {
                            tool: "browser_click",
                            target: "target_7k2nb5xe".into(),
                        },
                    },
                },
                vec!["target_7k2nb5xe"],
            ),
        ];
        for (refusal, sentinels) in refusals {
            let said = model_text(&refusal.summary(), &refusal.next_steps());
            accounts.push((said, refusal.audit(), sentinels));
        }
        for (said, audit, sentinels) in accounts {
            let kept = serde_json::to_string(&audit.refusal()).unwrap();
            for sentinel in sentinels {
                assert!(
                    said.contains(sentinel),
                    "the agent reads {sentinel}: {said}"
                );
                assert!(!audit.summary().contains(sentinel), "{}", audit.summary());
                assert!(!kept.contains(sentinel), "{kept}");
            }
        }

        // The retained fill is the same fill without what the page read back, in every wrapper.
        let fill = read_back_fill();
        let Outcome::FormFilled {
            fields,
            submitted,
            host,
            rebound,
            ..
        } = fill.clone()
        else {
            unreachable!("read_back_fill is a fill")
        };
        let unread = Outcome::FormFilled {
            fields,
            submitted,
            host,
            held: Vec::new(),
            rebound,
        };
        assert_eq!(fill.audit().summary(), unread.summary());
        assert_eq!(
            fill.clone()
                .with_open_dialog(DialogKind::Confirm)
                .audit()
                .summary(),
            unread
                .clone()
                .with_open_dialog(DialogKind::Confirm)
                .summary()
        );
        assert_eq!(
            unconfirmed(&fill, CheckFailure::Unmet).audit().summary(),
            unconfirmed(&unread, CheckFailure::Unmet).summary()
        );

        for preserve in [true, false] {
            let clicked = Outcome::TargetClicked {
                host: Some("example.com".into()),
                subject: ActionSubject::from_page("button", "Save record", preserve),
            };
            let audit = clicked.audit();
            assert_eq!(audit.summary(), clicked.summary());
            assert_eq!(audit.summary().contains("Save record"), preserve);
        }

        let primitive = Refusal::BrowserPrimitive {
            detail: "PRIVATE_EXCEPTION".into(),
            cause: PrimitiveCause::Refused,
        }
        .audit();
        let typed = primitive.clone().for_gesture(PageGesture::Type);
        assert_eq!(typed.summary(), PageGesture::Type.could_not());
        assert_eq!(typed.refusal(), primitive.refusal());
        assert_eq!(
            primitive.clone().for_gesture(PageGesture::Other).summary(),
            primitive.summary()
        );
        let contract = Refusal::BrowserContract.audit();
        assert_eq!(
            contract.clone().for_gesture(PageGesture::Type).summary(),
            contract.summary(),
            "only a browser failure takes the gesture's words"
        );
    }
}
