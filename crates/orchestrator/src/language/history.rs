//! Closed composition receipt identities, each record's outcome class, and readable
//! retained-history explanations.
//!
//! History is read by a person in the window. Each record has one closed [`OutcomeClass`],
//! computed here from its typed status and refusal, and a flow's account of each step derives
//! its cause from the same class ([`OutcomeClass::step_cause`]). A record keeps the summary the
//! model was told, and a few of those speak to the model about "the user": the human controls'
//! directives, the channel refusal, the refusal that leaves opening a browser to the person, and
//! the credential refusal. [`for_the_person`] gives the window its own sentence for them, and the
//! permission explanations name a refusal's reason in words rather than as a code.

use crate::audit::AuditRecord;
use crate::governance::evidence::PermissionCheck;
use crate::governance::{ReasonCode, MANAGED_TIER};
use crate::intake::session::IntakeChannel;
use crate::language::audit::AuditRefusal;
use crate::language::channel_switch;
use crate::language::composition::StepCause;
use crate::language::outcome::{
    BlockedReason, BrowserUnavailableReason, CheckFailure, PolicyAuthor, HUMAN_PAUSE_DIRECTIVE,
    HUMAN_STOP_DIRECTIVE,
};
use crate::work::result::Status;
use serde::{Deserialize, Serialize};

/// What a settled call came to, as history shows it (ADR-0203 Decision 12.2).
///
/// The window draws each row from this class and never re-derives it from the decision and the
/// status words.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeClass {
    /// It did what it was asked.
    Succeeded,
    /// Policy or the person's runtime controls refused it.
    Refused,
    /// The person's own browser setting kept a tab open that the call would have closed.
    Kept,
    /// It waits on something only the person can give, such as a secret they had not allowed
    /// Ghostlight to type.
    NeedsYou,
    /// It could not do what it was asked, or could not start.
    Failed,
    /// What it did cannot be confirmed.
    Unknown,
    /// It was cancelled before it finished.
    Cancelled,
}

impl OutcomeClass {
    /// The class of a call that settled with `status` and, when it did not succeed, the closed
    /// `refusal` its record keeps.
    #[must_use]
    pub const fn of(status: Status, refusal: Option<&AuditRefusal>) -> Self {
        match refusal {
            Some(AuditRefusal::CredentialHandoff | AuditRefusal::CredentialAuthorization) => {
                Self::NeedsYou
            }
            Some(AuditRefusal::LocalInterlock) => Self::Kept,
            _ => match status {
                Status::Succeeded => Self::Succeeded,
                Status::Blocked => Self::Refused,
                Status::Failed => Self::Failed,
                Status::Cancelled => Self::Cancelled,
                Status::Unknown => Self::Unknown,
            },
        }
    }

    /// The class of a durable record, read from its stored status word, its decision, its
    /// refusal and, for a flow, its step account.
    ///
    /// A status word no settled call ends with is a step that could not start, so it failed. A
    /// flow keeps no refusal of its own. A flow its steps held back is refused when its decision
    /// or its deciding step was refused, and otherwise takes the class of what held it back
    /// ([`held_back_by`]).
    #[must_use]
    pub fn recorded(record: &AuditRecord) -> Self {
        let Some(status) = Status::from_stored_name(&record.status) else {
            return Self::Failed;
        };
        let refusal = record.refusal_facts.as_ref();
        let issue = record.composition.and_then(|progress| progress.issue);
        match (Self::of(status, refusal), issue) {
            (Self::Refused, Some(issue)) if refusal.is_none() && record.allowed => {
                held_back_by(issue.cause)
            }
            (class, _) => class,
        }
    }

    /// The cause a flow keeps for a step of this class, refined by the step's own refusal.
    ///
    /// The person's controls, the deadline, a cancellation and a lost connection keep their own
    /// cause whatever the class, because they decide whether the flow goes on.
    #[must_use]
    pub const fn step_cause(self, refusal: Option<&AuditRefusal>) -> Option<StepCause> {
        let fallback = match self {
            Self::Succeeded => return None,
            // The person's setting kept the tab, and the flow stops for them as for any other
            // request for their attention.
            Self::Kept => return Some(StepCause::AttentionRequired),
            // A refused secret was never typed, so the flow goes on as past a failed step.
            Self::NeedsYou => return Some(StepCause::Failed),
            Self::Refused => StepCause::PolicyBlocked,
            Self::Failed => StepCause::Failed,
            Self::Unknown => StepCause::Unconfirmed,
            Self::Cancelled => StepCause::Cancelled,
        };
        if let Some(refusal) = refusal {
            if let Some(cause) = interruption(refusal) {
                return Some(cause);
            }
        }
        Some(fallback)
    }
}

/// The class of a blocked flow that authority admitted, by the cause its account keeps for the
/// step that decided it.
///
/// Such a flow was held back by a step whose check was refused, by a kept tab, or by a step that
/// needs the person, which is the credential refusal: the only steps blocked without a refusing
/// decision.
const fn held_back_by(cause: StepCause) -> OutcomeClass {
    match cause {
        StepCause::PolicyBlocked
        | StepCause::Paused
        | StepCause::SessionEnded
        | StepCause::AuditUnavailable => OutcomeClass::Refused,
        StepCause::AttentionRequired => OutcomeClass::Kept,
        StepCause::InvalidArguments
        | StepCause::MissingReference
        | StepCause::Failed
        | StepCause::ConnectionLost
        | StepCause::Deadline
        | StepCause::Cancelled
        | StepCause::Unconfirmed => OutcomeClass::NeedsYou,
    }
}

/// The cause a refusal gives a step whatever its class: the person's controls, the history
/// requirement, the deadline, a cancellation, a lost connection, or the check an applied action
/// did not pass.
const fn interruption(refusal: &AuditRefusal) -> Option<StepCause> {
    use AuditRefusal as A;
    Some(match refusal {
        A::AuthorityBlocked {
            cause: BlockedReason::AuditUnavailable,
        } => StepCause::AuditUnavailable,
        A::AuthorityBlocked {
            cause: BlockedReason::Hold,
        } => StepCause::Paused,
        A::AuthorityBlocked {
            cause: BlockedReason::SessionEnded,
        } => StepCause::SessionEnded,
        A::DeadlineBeforeStart | A::DeadlineExpired { .. } => StepCause::Deadline,
        A::CancelledBeforeStart | A::CancelledBeforeDispatch | A::CancelledAfterDispatch => {
            StepCause::Cancelled
        }
        A::ConnectionLost
        | A::BrowserUnavailable {
            cause: BrowserUnavailableReason::NotConnected | BrowserUnavailableReason::NotAnswering,
        } => StepCause::ConnectionLost,
        A::ActionUnconfirmed { cause } => unconfirmed_cause(*cause),
        _ => return None,
    })
}

/// The flow cause of an applied action whose requested check did not confirm it.
///
/// It is the cause the check would have had on its own, so a Pause, Stop, cancellation, or
/// deadline during a check still stops the flow the way it stops any other step.
const fn unconfirmed_cause(cause: CheckFailure) -> StepCause {
    match cause {
        CheckFailure::Blocked(BlockedReason::AuditUnavailable) => StepCause::AuditUnavailable,
        CheckFailure::Blocked(BlockedReason::Hold) => StepCause::Paused,
        CheckFailure::Blocked(BlockedReason::SessionEnded) => StepCause::SessionEnded,
        CheckFailure::Blocked(_) => StepCause::PolicyBlocked,
        CheckFailure::Cancelled => StepCause::Cancelled,
        CheckFailure::DeadlineExpired => StepCause::Deadline,
        CheckFailure::Disconnected => StepCause::ConnectionLost,
        CheckFailure::Unmet
        | CheckFailure::BrowserUnavailable
        | CheckFailure::DocumentUnavailable
        | CheckFailure::BrowserRefused => StepCause::Failed,
    }
}

/// What the window says in place of the pause directive the model was told.
const YOU_PAUSED: &str = "You paused Ghostlight.";

/// What the window says in place of the stop directive the model was told.
const YOU_ENDED: &str = "You ended the session.";

/// What the window says in place of the refusal that asks the model to have the user open a
/// browser.
const NO_BROWSER_CONNECTED: &str = "No browser was connected, so nothing ran. Open a browser window with Ghostlight in Browser installed.";

/// What the window says in place of the refusal of credential input the user had not authorized.
const SECRET_NOT_ALLOWED: &str = "Refused to type a password or other secret you had not allowed.";

/// Whose layer a record's `policy_tier` names: the organization's for the managed tier, and the
/// person's own otherwise.
#[must_use]
pub fn author_of(tier: Option<&str>) -> PolicyAuthor {
    if tier == Some(MANAGED_TIER) {
        PolicyAuthor::Organization
    } else {
        PolicyAuthor::User
    }
}

/// The sentence the window shows for a recorded action whose `summary` speaks to the model, or
/// `None` when the summary already reads right for a person.
///
/// A channel refusal says whose policy turned off which work: "Your policy turns off work from MCP
/// agents.". A record keeps no organization name, so the organization's refusal says "Your
/// organization's policy", and the refusal line beneath it names the organization. A pause or a
/// stop says what the person did, followed by whatever the model's sentence went on to say about
/// the action. A refusal that left opening a browser to the person, or that kept a secret from
/// being typed, is read from the record's closed `refusal` facts, never from its prose.
#[must_use]
pub fn for_the_person(
    summary: &str,
    reason: ReasonCode,
    refusal: Option<&AuditRefusal>,
    channel: IntakeChannel,
    tier: Option<&str>,
) -> Option<String> {
    if reason == ReasonCode::ChannelDenied {
        return Some(channel_switch::refused(channel, author_of(tier), None));
    }
    match refusal {
        Some(AuditRefusal::BrowserStartupManual) => return Some(NO_BROWSER_CONNECTED.into()),
        Some(AuditRefusal::CredentialAuthorization) => return Some(SECRET_NOT_ALLOWED.into()),
        _ => {}
    }
    [
        (HUMAN_PAUSE_DIRECTIVE, YOU_PAUSED),
        (HUMAN_STOP_DIRECTIVE, YOU_ENDED),
    ]
    .into_iter()
    .find_map(|(directive, said)| {
        summary
            .strip_prefix(directive)
            .map(|rest| format!("{said}{rest}"))
    })
}

/// Why a check no policy layer decided was refused, as the end of "Refused: ...".
const fn refused_because(reason: ReasonCode) -> &'static str {
    match reason {
        ReasonCode::Permitted => "nothing refused it",
        ReasonCode::CapabilityDenied => "policy does not allow this kind of work",
        ReasonCode::TabCloseDenied => "policy does not let agents close tabs",
        ReasonCode::ScreenshotDenied => "policy turns off screenshots",
        ReasonCode::ChannelDenied => "policy turns off work from this channel",
        ReasonCode::HostDenied => "policy does not allow this site",
        ReasonCode::ProtectedHost => "this site is protected and never automated",
        ReasonCode::OwnWorkbench => "Ghostlight's own control page is never automated",
        ReasonCode::InvalidAuthority => "a policy that applies is not valid",
        ReasonCode::InvalidRequest => "the call did not match the Ghostlight catalog",
        ReasonCode::RuntimeHold => "you paused Ghostlight",
        ReasonCode::SessionEnded => "you ended the session",
        ReasonCode::AuditUnavailable => "policy requires saved history, and it could not be saved",
    }
}

/// The composition identity; caller labels never supply one.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionKind {
    Flow,
}

impl CompositionKind {
    /// Return the canonical catalog name.
    #[must_use]
    pub const fn tool(self) -> &'static str {
        match self {
            Self::Flow => super::Tool::Flow.name(),
        }
    }
}

/// Parent invocation plus position identifies one child receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StepReceipt {
    pub parent: CompositionKind,
    pub position: usize,
    pub total: usize,
    pub preparation_failed: bool,
}

/// Explain actual permission evidence, without reading today's policy or guessing grants.
///
/// `channel` and `tier` are the record's own: a check its channel switch refused takes the words
/// the window's row says ([`for_the_person`]).
#[must_use]
pub fn permission(check: &PermissionCheck, channel: IntakeChannel, tier: Option<&str>) -> String {
    let verdict = if check.allowed { "Allowed" } else { "Refused" };
    if check.reason == ReasonCode::ChannelDenied {
        let said = channel_switch::refused(channel, author_of(tier), None);
        return format!("{verdict}. {said}");
    }
    if check.reason != ReasonCode::Permitted && check.layers.is_empty() {
        return format!("{verdict}: {}.", refused_because(check.reason));
    }
    if check.requirements.is_empty() {
        return format!("{verdict}; no capability grant was required.");
    }
    if check.layers.is_empty() {
        return format!("{verdict} without configured policy.");
    }
    let sources: Vec<String> = check
        .layers
        .iter()
        .map(|layer| {
            let source = if layer.tier == MANAGED_TIER {
                "organization policy"
            } else {
                "local policy"
            };
            let rules = if layer.grants.is_empty() {
                "no matching grant".into()
            } else {
                layer.grants.join(", ")
            };
            let verdict = if layer.allowed {
                "permits this work"
            } else {
                "refuses this work"
            };
            format!("{source} {verdict} ({rules})")
        })
        .collect();
    format!("{verdict}: {}.", sources.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::CapabilitySet;
    use crate::language::composition::CompositionProgress;
    use crate::language::field_problem::ControlRef;
    use crate::language::outcome::{ExtensionAdvice, Refusal, TargetRole};

    fn refused(reason: ReasonCode) -> PermissionCheck {
        PermissionCheck {
            requirements: CapabilitySet::EMPTY,
            host: None,
            allowed: false,
            reason,
            layers: vec![],
        }
    }

    /// The window's sentence for a refusal the model was told, read as the window reads it.
    fn in_the_window(refusal: &Refusal) -> Option<String> {
        let audit = refusal.audit();
        for_the_person(
            audit.summary(),
            ReasonCode::Permitted,
            audit.refusal(),
            IntakeChannel::Mcp,
            None,
        )
    }

    /// ACCEPTANCE Governance 4 and LANGUAGE (channel switch, human controls): a history row whose
    /// summary speaks to the model about the user, and a permission explanation, read for the
    /// person: whose policy turned off which channel, what the person did, what to open, that a
    /// secret stayed untyped, and a refusal reason in words, never as a code. A summary already
    /// written for a person is kept.
    #[test]
    fn the_window_says_to_the_person_what_the_model_was_told() {
        let model = "Blocked: the user's Ghostlight policy turns off work from MCP agents.";
        let person = |summary: &str, reason, channel, tier| {
            for_the_person(summary, reason, None, channel, tier)
        };
        assert_eq!(
            person(
                model,
                ReasonCode::ChannelDenied,
                IntakeChannel::Mcp,
                Some("user")
            ),
            Some(channel_switch::refused(
                IntakeChannel::Mcp,
                PolicyAuthor::User,
                None
            ))
        );
        assert_eq!(
            person(
                model,
                ReasonCode::ChannelDenied,
                IntakeChannel::Cli,
                Some(MANAGED_TIER)
            ),
            Some(channel_switch::refused(
                IntakeChannel::Cli,
                PolicyAuthor::Organization,
                None
            ))
        );
        assert_eq!(
            person(
                HUMAN_PAUSE_DIRECTIVE,
                ReasonCode::RuntimeHold,
                IntakeChannel::Mcp,
                None
            )
            .as_deref(),
            Some(YOU_PAUSED)
        );
        let rest = " The expected condition was not checked.";
        assert_eq!(
            person(
                &format!("{HUMAN_STOP_DIRECTIVE}{rest}"),
                ReasonCode::SessionEnded,
                IntakeChannel::Mcp,
                None
            ),
            Some(format!("{YOU_ENDED}{rest}"))
        );
        assert_eq!(
            person(
                "Opened example.com.",
                ReasonCode::Permitted,
                IntakeChannel::Mcp,
                None
            ),
            None
        );
        let manual = Refusal::BrowserStartupManual {
            browsers: vec!["Google Chrome".into()],
            extension: ExtensionAdvice::Plain,
        };
        assert_eq!(
            in_the_window(&manual).as_deref(),
            Some(NO_BROWSER_CONNECTED)
        );
        let credential = Refusal::CredentialAuthorization {
            control: ControlRef::new(TargetRole::Textbox, "Password", None, false),
        };
        assert_eq!(
            in_the_window(&credential).as_deref(),
            Some(SECRET_NOT_ALLOWED)
        );
        assert_eq!(
            permission(
                &refused(ReasonCode::ChannelDenied),
                IntakeChannel::Mcp,
                Some("user")
            ),
            format!(
                "Refused. {}",
                channel_switch::refused(IntakeChannel::Mcp, PolicyAuthor::User, None)
            )
        );
        for reason in [
            ReasonCode::CapabilityDenied,
            ReasonCode::TabCloseDenied,
            ReasonCode::ScreenshotDenied,
            ReasonCode::HostDenied,
            ReasonCode::ProtectedHost,
            ReasonCode::OwnWorkbench,
            ReasonCode::InvalidAuthority,
            ReasonCode::InvalidRequest,
            ReasonCode::RuntimeHold,
            ReasonCode::SessionEnded,
            ReasonCode::AuditUnavailable,
        ] {
            let said = permission(&refused(reason), IntakeChannel::Mcp, None);
            assert!(!said.contains('_') && said.ends_with('.'), "{said}");
        }
    }

    /// A durable record of one settled call, as the audit file keeps it.
    fn record(
        status: Status,
        decision: crate::governance::Decision,
        audit: &crate::language::audit::AuditProjection,
    ) -> AuditRecord {
        AuditRecord::now(
            "invocation_class",
            "workspace_class",
            "browser_type_text",
            CapabilitySet::WRITE,
            "authority_class",
            decision,
            &status.stored_name(),
            "none",
            audit,
            1,
        )
    }

    /// A blocked flow's record whose deciding step had `cause`.
    fn flow(allowed: bool, cause: StepCause) -> AuditRecord {
        use crate::language::composition::{StepCounts, StepIssue};
        use crate::language::outcome::Outcome;
        let progress = CompositionProgress {
            counts: StepCounts {
                total: 2,
                succeeded: 1,
                blocked: 1,
                ..StepCounts::default()
            },
            stopped: false,
            issue: Some(StepIssue { step: 2, cause }),
            ..CompositionProgress::default()
        };
        let decision = if allowed {
            crate::governance::Decision::permitted()
        } else {
            crate::governance::Decision::refused(ReasonCode::CapabilityDenied)
        };
        record(
            Status::Blocked,
            decision,
            &Outcome::CompositionRan(progress).audit(),
        )
    }

    /// ADR-0203 Decision 12.2: each settled record has one closed class, computed from its status
    /// and closed refusal, a flow taking the class of the step that held it back, and a flow
    /// step's cause derives from it. Contract with the window's script: the class crosses as one
    /// closed word (`tests/workbench-settings.test.mjs` reads "kept" and "needs_you").
    #[test]
    fn every_record_has_one_outcome_class_that_crosses_to_the_window_as_one_word() {
        let permitted = crate::governance::Decision::permitted();
        let quiet = crate::language::outcome::Outcome::TabsListed { count: 0 }.audit();
        for (status, class, word) in [
            (Status::Succeeded, OutcomeClass::Succeeded, "succeeded"),
            (Status::Blocked, OutcomeClass::Refused, "refused"),
            (Status::Failed, OutcomeClass::Failed, "failed"),
            (Status::Cancelled, OutcomeClass::Cancelled, "cancelled"),
            (Status::Unknown, OutcomeClass::Unknown, "unknown"),
        ] {
            assert_eq!(OutcomeClass::of(status, None), class);
            assert_eq!(
                OutcomeClass::recorded(&record(status, permitted, &quiet)),
                class
            );
            assert_eq!(serde_json::to_value(class).unwrap(), word);
        }
        let credential = Refusal::CredentialAuthorization {
            control: ControlRef::new(TargetRole::Textbox, "Password", None, false),
        }
        .audit();
        let kept = Refusal::LocalInterlock.audit();
        let policy = Refusal::AuthorityBlocked {
            code: ReasonCode::CapabilityDenied,
            host: None,
            origin: None,
        }
        .audit();
        for (audit, decision, class, word) in [
            (&credential, permitted, OutcomeClass::NeedsYou, "needs_you"),
            (&kept, permitted, OutcomeClass::Kept, "kept"),
            (
                &policy,
                crate::governance::Decision::refused(ReasonCode::CapabilityDenied),
                OutcomeClass::Refused,
                "refused",
            ),
        ] {
            assert_eq!(OutcomeClass::of(Status::Blocked, audit.refusal()), class);
            assert_eq!(
                OutcomeClass::recorded(&record(Status::Blocked, decision, audit)),
                class
            );
            assert_eq!(serde_json::to_value(class).unwrap(), word);
        }
        // A flow keeps no refusal of its own: it is classed by the step that held it back, and a
        // refusing decision decides it whatever came after the refused step.
        for (allowed, cause, class) in [
            (true, StepCause::Failed, OutcomeClass::NeedsYou),
            (true, StepCause::AttentionRequired, OutcomeClass::Kept),
            (true, StepCause::Paused, OutcomeClass::Refused),
            (false, StepCause::Deadline, OutcomeClass::Refused),
        ] {
            assert_eq!(OutcomeClass::recorded(&flow(allowed, cause)), class);
        }
        let never_started = AuditRecord::not_started(
            "invocation_x",
            "workspace_x",
            "browser_read",
            &crate::governance::GovernanceFacade::new(None).snapshot(),
            &crate::provenance::Attribution::accepted(IntakeChannel::Mcp),
            StepReceipt {
                parent: CompositionKind::Flow,
                position: 1,
                total: 1,
                preparation_failed: true,
            },
        );
        assert_eq!(OutcomeClass::recorded(&never_started), OutcomeClass::Failed);
        // A flow goes on past a refused secret and stops for a kept tab.
        assert_eq!(OutcomeClass::Succeeded.step_cause(None), None);
        assert_eq!(
            OutcomeClass::NeedsYou.step_cause(credential.refusal()),
            Some(StepCause::Failed)
        );
        assert_eq!(
            OutcomeClass::Kept.step_cause(kept.refusal()),
            Some(StepCause::AttentionRequired)
        );
    }
}
