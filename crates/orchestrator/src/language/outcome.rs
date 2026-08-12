//! Ghostlight-authored outcome language and its content-free measurement projection.

use serde::{Deserialize, Serialize};

use crate::workspace::WorkspaceError;

/// The noun named by a target-listing outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetNoun {
    /// A semantic search match.
    Match,
    /// An inspected page item.
    Item,
}

/// What one completed browser action did in Ghostlight's product language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// Controlled tabs were listed.
    TabsListed { count: usize },
    /// One controlled tab was brought into view.
    TabActivated { host: Option<String> },
    /// A requested page was opened.
    PageOpened { host: Option<String> },
    /// An existing controlled page was navigated.
    PageNavigated { host: Option<String> },
    /// Browser history was traversed.
    HistoryTraversed {
        direction: String,
        host: Option<String>,
    },
    /// A controlled page was reloaded.
    PageReloaded { host: Option<String> },
    /// A controlled tab was closed.
    TabClosed,
    /// Bounded page text was read.
    TextRead { words: usize },
    /// Semantic targets were inspected or found.
    TargetsListed { noun: TargetNoun, count: usize },
    /// A screenshot was captured.
    Captured {
        full_page: bool,
        width: u32,
        height: u32,
    },
    /// A semantic target was clicked.
    TargetClicked { host: Option<String> },
    /// A current screenshot point was clicked.
    PointClicked { host: Option<String> },
    /// A page was scrolled.
    PageScrolled { host: Option<String> },
    /// A semantic target was revealed.
    TargetRevealed { host: Option<String> },
    /// Visible tab zoom was set.
    ZoomSet { percent: u16, host: Option<String> },
    /// The browser window was resized.
    WindowResized { width: u32, height: u32 },
    /// A semantic target or current screenshot point was hovered.
    Hovered { host: Option<String> },
    /// Ordinary form controls were filled.
    FormFilled { fields: usize, submitted: bool },
    /// Ordinary text was typed through browser input events.
    TextTyped { host: Option<String> },
    /// An explicit keyboard action was sent.
    KeyboardSent { host: Option<String> },
    /// A drag completed.
    Dragged { host: Option<String> },
    /// Explicitly named local files were uploaded.
    FilesUploaded { count: usize },
    /// A bounded page script was evaluated.
    ScriptEvaluated { host: Option<String> },
    /// An explicit observable condition was awaited.
    Waited {
        condition: String,
        elapsed_ms: u64,
        satisfied: bool,
    },
    /// A short sequence ran until completion or its first non-success.
    SequenceRan { completed: usize, total: usize },
    /// A browser dialog was resolved.
    DialogHandled { accepted: bool },
    /// Current JavaScript-dialog state was observed.
    DialogObserved { present: bool },
    /// Bounded console and network diagnostics were read.
    DiagnosticsRead { count: usize, capture_started: bool },
    /// A memory-only recording began.
    RecordingStarted,
    /// Memory-only recording state was read.
    RecordingObserved { frames: usize },
    /// An active memory-only recording stopped.
    RecordingStopped { frames: usize },
    /// A recording was encoded as an animated GIF.
    RecordingSaved {
        frames: usize,
        bytes: usize,
        attached: bool,
    },
    /// Captured recording bytes were erased.
    RecordingDiscarded,
}

impl Outcome {
    /// Render the bounded Ghostlight-authored account of what happened.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::TabsListed { count } => format!(
                "Listed {}.",
                counted(*count, "controlled tab", "controlled tabs")
            ),
            Self::TabActivated { host } => {
                format!("Brought {} into view.", place(host, "the controlled tab"))
            }
            Self::PageOpened { host } => {
                format!("Opened {}.", place(host, "the requested page"))
            }
            Self::PageNavigated { host } => {
                format!("Navigated to {}.", place(host, "the requested page"))
            }
            Self::HistoryTraversed { direction, host } => {
                format!("Went {direction} to {}.", place(host, "the previous page"))
            }
            Self::PageReloaded { host } => {
                format!("Reloaded {}.", place(host, "the page"))
            }
            Self::TabClosed => "Closed the controlled tab.".into(),
            Self::TextRead { words } => {
                format!("Read {}.", counted(*words, "word", "words"))
            }
            Self::TargetsListed {
                noun: TargetNoun::Match,
                count,
            } => format!("Found {}.", counted(*count, "match", "matches")),
            Self::TargetsListed {
                noun: TargetNoun::Item,
                count,
            } => format!(
                "Inspected the page and found {}.",
                counted(*count, "item", "items")
            ),
            Self::Captured {
                full_page,
                width,
                height,
            } => format!(
                "Captured the {} at {width}x{height}.",
                if *full_page { "full page" } else { "viewport" }
            ),
            Self::TargetClicked { host } => {
                format!("Clicked a target on {}.", place(host, "the page"))
            }
            Self::PointClicked { host } => {
                format!("Clicked a point on {}.", place(host, "the page"))
            }
            Self::PageScrolled { host } => {
                format!("Scrolled {}.", place(host, "the page"))
            }
            Self::TargetRevealed { host } => {
                format!("Revealed a target on {}.", place(host, "the page"))
            }
            Self::ZoomSet { percent, host } => {
                format!("Set zoom to {percent}% on {}.", place(host, "the page"))
            }
            Self::WindowResized { width, height } => {
                format!("Resized the browser window to {width}x{height}.")
            }
            Self::Hovered { host } => {
                format!("Hovered a target on {}.", place(host, "the page"))
            }
            Self::FormFilled {
                fields,
                submitted: false,
            } => format!("Filled {}.", counted(*fields, "field", "fields")),
            Self::FormFilled {
                fields,
                submitted: true,
            } => format!(
                "Filled {} and submitted the form.",
                counted(*fields, "field", "fields")
            ),
            Self::TextTyped { host } => format!(
                "Typed text on {} through browser input events.",
                place(host, "the page")
            ),
            Self::KeyboardSent { host } => {
                format!("Sent a keyboard action to {}.", place(host, "the page"))
            }
            Self::Dragged { host } => {
                format!("Completed a drag on {}.", place(host, "the page"))
            }
            Self::FilesUploaded { count } => {
                format!("Uploaded {}.", counted(*count, "file", "files"))
            }
            Self::ScriptEvaluated { host } => {
                format!("Evaluated a script on {}.", place(host, "the page"))
            }
            Self::Waited {
                condition,
                elapsed_ms,
                satisfied: true,
            } => format!("Wait condition {condition} was satisfied after {elapsed_ms} ms."),
            Self::Waited {
                condition,
                elapsed_ms,
                satisfied: false,
            } => format!("Wait condition {condition} was not satisfied within {elapsed_ms} ms."),
            Self::SequenceRan { completed, total } if completed == total => {
                format!("Ran {}.", counted(*total, "step", "steps"))
            }
            Self::SequenceRan { completed, total } => {
                format!("Stopped at step {} of {total}.", completed + 1)
            }
            Self::DialogHandled { accepted: true } => "Accepted the browser dialog.".into(),
            Self::DialogHandled { accepted: false } => "Dismissed the browser dialog.".into(),
            Self::DialogObserved { present: true } => {
                "A JavaScript dialog is currently visible.".into()
            }
            Self::DialogObserved { present: false } => {
                "No JavaScript dialog is currently visible.".into()
            }
            Self::DiagnosticsRead { count, .. } => format!(
                "Read {}.",
                counted(*count, "diagnostic observation", "diagnostic observations")
            ),
            Self::RecordingStarted => "Started a memory-only browser recording.".into(),
            Self::RecordingObserved { frames } => format!(
                "The memory-only recording currently holds {}.",
                counted(*frames, "frame", "frames")
            ),
            Self::RecordingStopped { frames } => format!(
                "Stopped the browser recording with {}.",
                counted(*frames, "frame", "frames")
            ),
            Self::RecordingSaved {
                frames,
                bytes,
                attached: false,
            } => format!(
                "Saved {frames} recorded {} as an animated GIF of {bytes} bytes.",
                if *frames == 1 { "frame" } else { "frames" }
            ),
            Self::RecordingSaved {
                frames,
                bytes,
                attached: true,
            } => format!(
                "Prepared {frames} recorded {} as an animated GIF of {bytes} bytes and dispatched it to the page target without verified acceptance.",
                if *frames == 1 { "frame" } else { "frames" }
            ),
            Self::RecordingDiscarded => "Discarded the memory-only recording bytes.".into(),
        }
    }

    /// Render zero or one safe contextual recovery actions for this outcome.
    #[must_use]
    pub fn next_steps(&self) -> Vec<String> {
        match self {
            Self::Waited {
                satisfied: false, ..
            } => vec!["Inspect the current page before choosing another action.".into()],
            Self::DiagnosticsRead {
                count: 0,
                capture_started: true,
            } => vec![
                "Reproduce the problem or reload the page, then call browser_diagnose again."
                    .into(),
            ],
            _ => vec![],
        }
    }

    /// Project the measurements named by the sentence into the audit vocabulary.
    #[must_use]
    pub fn observed(&self) -> Observed {
        match self {
            Self::TabsListed { count }
            | Self::TextRead { words: count }
            | Self::TargetsListed { count, .. }
            | Self::FilesUploaded { count }
            | Self::DiagnosticsRead { count, .. }
            | Self::RecordingObserved { frames: count }
            | Self::RecordingStopped { frames: count }
            | Self::SequenceRan {
                completed: count, ..
            } => Observed {
                count: measured(*count),
                ..Observed::default()
            },
            Self::FormFilled { fields, .. } => Observed {
                count: measured(*fields),
                ..Observed::default()
            },
            Self::Waited { elapsed_ms, .. } => Observed {
                count: measured(*elapsed_ms),
                ..Observed::default()
            },
            Self::Captured { width, height, .. } => Observed {
                width: Some(*width),
                height: Some(*height),
                ..Observed::default()
            },
            Self::WindowResized { width, height } => Observed {
                width: Some(*width),
                height: Some(*height),
                ..Observed::default()
            },
            Self::RecordingSaved { frames, .. } => Observed {
                count: measured(*frames),
                ..Observed::default()
            },
            Self::TabActivated { host }
            | Self::PageOpened { host }
            | Self::PageNavigated { host }
            | Self::HistoryTraversed { host, .. }
            | Self::PageReloaded { host }
            | Self::TargetClicked { host }
            | Self::PointClicked { host }
            | Self::PageScrolled { host }
            | Self::TargetRevealed { host }
            | Self::ZoomSet { host, .. }
            | Self::Hovered { host }
            | Self::TextTyped { host }
            | Self::KeyboardSent { host }
            | Self::Dragged { host }
            | Self::ScriptEvaluated { host } => Observed {
                host: host.clone(),
                ..Observed::default()
            },
            Self::TabClosed
            | Self::DialogHandled { .. }
            | Self::DialogObserved { .. }
            | Self::RecordingStarted
            | Self::RecordingDiscarded => Observed::default(),
        }
    }
}

/// Why a browser job did not complete in Ghostlight's product language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// Model-facing input did not match the catalog.
    InvalidRequest,
    /// Cancellation won before workspace admission.
    CancelledBeforeStart,
    /// The invocation deadline expired before workspace admission.
    DeadlineBeforeStart,
    /// Configured authority blocked the job.
    AuthorityBlocked,
    /// Runtime control requires the user.
    AttentionRequired,
    /// The browser-local physical safety setting refused the action.
    LocalInterlock,
    /// A credential-class field requires visible user handoff.
    CredentialHandoff,
    /// The browser returned a receipt outside the negotiated contract.
    IncompatibleReceipt,
    /// The browser stopped before a physical effect.
    BrowserStopped { reconnect: bool },
    /// A dispatched effect cannot be determined.
    EffectUnknown,
    /// A denied new-tab landing has an unknown final state.
    LandingDeniedUnknown,
    /// The selected workspace resource is unusable.
    WorkspaceUnusable { reason: WorkspaceReason },
    /// Explicit local files could not be prepared safely.
    FilesUnreadable,
    /// A screenshot exceeded the result bound.
    CaptureTooLarge,
    /// No JavaScript dialog was visible.
    NoDialogVisible,
    /// A recording handle was absent, ambiguous, or in a conflicting transition.
    RecordingUnavailable,
    /// Retained recording frames could not produce a bounded GIF.
    RecordingExportFailed,
}

impl Refusal {
    /// Render the bounded Ghostlight-authored account of the refusal.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::InvalidRequest => "The call does not match the Ghostlight catalog.",
            Self::CancelledBeforeStart => "The browser job was cancelled before it started.",
            Self::DeadlineBeforeStart => {
                "The browser job deadline expired while waiting for the workspace."
            }
            Self::AuthorityBlocked => "Authority blocked the browser job.",
            Self::AttentionRequired => "The browser job requires user attention.",
            Self::LocalInterlock => "A local browser safety setting blocked this action.",
            Self::CredentialHandoff => {
                "A credential-class field requires user handoff in the visible browser."
            }
            Self::IncompatibleReceipt => {
                "The browser adapter returned an incompatible primitive receipt."
            }
            Self::BrowserStopped { .. } => "The browser job stopped before a physical effect.",
            Self::EffectUnknown => {
                "A browser effect was dispatched, but its final state cannot be determined."
            }
            Self::LandingDeniedUnknown => {
                "The landing was denied, but the new tab's final state cannot be determined."
            }
            Self::WorkspaceUnusable { .. } => {
                "The requested workspace target is not currently usable."
            }
            Self::FilesUnreadable => "The selected local files could not be prepared safely.",
            Self::CaptureTooLarge => "Screenshot exceeded the product result bound.",
            Self::NoDialogVisible => "No JavaScript dialog is currently visible.",
            Self::RecordingUnavailable => {
                "The requested memory-only recording is not currently available."
            }
            Self::RecordingExportFailed => {
                "The retained browser frames could not produce a bounded animated GIF."
            }
        }
        .into()
    }

    /// Render zero or one safe contextual recovery actions for this refusal.
    #[must_use]
    pub fn next_steps(&self) -> Vec<String> {
        match self {
            Self::InvalidRequest => {
                vec!["Correct the call using the advertised tool schema.".into()]
            }
            Self::LocalInterlock => vec![
                "The user can change the relevant Ghostlight extension setting or perform the action directly."
                    .into(),
            ],
            Self::CredentialHandoff => vec![
                "Complete the credential field in the visible browser, then inspect the page again."
                    .into(),
            ],
            Self::BrowserStopped { reconnect: true } => {
                vec!["Reconnect the Ghostlight browser adapter.".into()]
            }
            Self::WorkspaceUnusable { reason } => reason.next_steps(),
            Self::RecordingUnavailable => vec![
                "Use browser_record with action status and an explicit recording handle when more than one exists."
                    .into(),
            ],
            Self::RecordingExportFailed => vec![
                "Inspect recording status, then discard it or start a shorter recording."
                    .into(),
            ],
            _ => vec![],
        }
    }
}

/// Stable language reason for an unusable workspace resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceReason {
    /// No current unambiguous controlled tab matched.
    TabUnavailable,
    /// The target belongs to an old or unknown document.
    StaleTarget,
    /// The view no longer matches current rendered coordinates.
    StaleView,
    /// Runtime governance holds the selected tab.
    TabHeld,
    /// Another invocation owns the workspace lease.
    WorkspaceBusy,
    /// The selected resource belongs elsewhere.
    OwnershipMismatch,
    /// The workspace is no longer admitted.
    WorkspaceClosed,
}

impl WorkspaceReason {
    /// Render the stable structured fact value.
    #[must_use]
    pub const fn as_fact(self) -> &'static str {
        match self {
            Self::TabUnavailable => "tab_unavailable",
            Self::StaleTarget => "stale_target",
            Self::StaleView => "stale_view",
            Self::TabHeld => "tab_held",
            Self::WorkspaceBusy => "workspace_busy",
            Self::OwnershipMismatch => "ownership_mismatch",
            Self::WorkspaceClosed => "workspace_closed",
        }
    }

    /// Render zero or one safe contextual recovery actions for this reason.
    #[must_use]
    pub fn next_steps(self) -> Vec<String> {
        match self {
            Self::TabUnavailable => {
                vec!["Call browser_tabs with action list to obtain current tab handles.".into()]
            }
            Self::StaleTarget => vec![
                "Call browser_inspect or browser_find to obtain current target handles.".into(),
            ],
            Self::StaleView => {
                vec!["Call browser_screenshot to obtain a current view handle.".into()]
            }
            Self::WorkspaceBusy => {
                vec!["Wait for the active Ghostlight invocation to finish.".into()]
            }
            Self::TabHeld | Self::OwnershipMismatch | Self::WorkspaceClosed => vec![],
        }
    }
}

impl From<WorkspaceError> for WorkspaceReason {
    fn from(error: WorkspaceError) -> Self {
        match error {
            WorkspaceError::StaleTab | WorkspaceError::NoTab | WorkspaceError::AmbiguousTab => {
                Self::TabUnavailable
            }
            WorkspaceError::StaleTarget => Self::StaleTarget,
            WorkspaceError::StaleView | WorkspaceError::ViewPointOutOfBounds => Self::StaleView,
            WorkspaceError::Held => Self::TabHeld,
            WorkspaceError::Busy => Self::WorkspaceBusy,
            WorkspaceError::NotOwnedTab
            | WorkspaceError::NotOwnedTarget
            | WorkspaceError::NotOwnedView
            | WorkspaceError::TargetTabMismatch
            | WorkspaceError::ViewTabMismatch
            | WorkspaceError::PhysicalTabOwned => Self::OwnershipMismatch,
            WorkspaceError::UnknownWorkspace => Self::WorkspaceClosed,
        }
    }
}

/// Content-free observations about one action.
///
/// This is deliberately not `InvocationResult::facts`. Facts legitimately carry page text and
/// full URLs for the model; an audit record carries measurements and metadata only. Keeping the
/// two as separate closed types makes copying one into the other impossible rather than merely
/// discouraged.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observed {
    /// Host the action landed on, lowercased.
    ///
    /// Never the path, query, or fragment. The host answers "where did the agent go" and is
    /// already visible in the user's own tab strip; the path is where a record number would sit.
    #[serde(default)]
    pub host: Option<String>,
    /// Product readiness the browser reported, in the same vocabulary a result uses.
    #[serde(default)]
    pub readiness: Option<String>,
    /// However many things the action touched, named by the summary beside it.
    #[serde(default)]
    pub count: Option<u32>,
    /// Captured width in pixels.
    #[serde(default)]
    pub width: Option<u32>,
    /// Captured height in pixels.
    #[serde(default)]
    pub height: Option<u32>,
}

impl Observed {
    /// Fold a later account into the facts already gathered for an invocation.
    ///
    /// A fact absent from the later account leaves the earlier fact standing. Outcome language is
    /// merged after the browser seam, so its named measurements take precedence without erasing
    /// seam-owned host or readiness.
    #[must_use]
    pub fn merged(self, later: Self) -> Self {
        Self {
            host: later.host.or(self.host),
            readiness: later.readiness.or(self.readiness),
            count: later.count.or(self.count),
            width: later.width.or(self.width),
            height: later.height.or(self.height),
        }
    }
}

fn place<'a>(host: &'a Option<String>, fallback: &'static str) -> &'a str {
    host.as_deref().unwrap_or(fallback)
}

fn counted(count: usize, singular: &str, plural: &str) -> String {
    let noun = if count == 1 { singular } else { plural };
    format!("{} {noun}", grouped(count))
}

fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut rendered = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            rendered.push(',');
        }
        rendered.push(digit);
    }
    rendered
}

fn measured<T: TryInto<u32>>(value: T) -> Option<u32> {
    Some(value.try_into().unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Observed, Outcome, Refusal, TargetNoun, WorkspaceReason};
    use crate::workspace::WorkspaceError;

    #[test]
    fn outcome_summaries_transcribe_the_product_oracles() {
        let examples = [
            (
                Outcome::TabsListed { count: 4 },
                "Listed 4 controlled tabs.",
            ),
            (Outcome::TabsListed { count: 1 }, "Listed 1 controlled tab."),
            (Outcome::TextRead { words: 1_240 }, "Read 1,240 words."),
            (Outcome::TextRead { words: 5 }, "Read 5 words."),
            (Outcome::TextRead { words: 1 }, "Read 1 word."),
            (
                Outcome::PageOpened {
                    host: Some("example.com".into()),
                },
                "Opened example.com.",
            ),
            (
                Outcome::PageOpened { host: None },
                "Opened the requested page.",
            ),
            (
                Outcome::TabActivated {
                    host: Some("example.com".into()),
                },
                "Brought example.com into view.",
            ),
            (
                Outcome::PageNavigated {
                    host: Some("example.com".into()),
                },
                "Navigated to example.com.",
            ),
            (
                Outcome::HistoryTraversed {
                    direction: "back".into(),
                    host: Some("example.com".into()),
                },
                "Went back to example.com.",
            ),
            (
                Outcome::PageReloaded {
                    host: Some("example.com".into()),
                },
                "Reloaded example.com.",
            ),
            (Outcome::TabClosed, "Closed the controlled tab."),
            (
                Outcome::TargetsListed {
                    noun: TargetNoun::Match,
                    count: 7,
                },
                "Found 7 matches.",
            ),
            (
                Outcome::TargetsListed {
                    noun: TargetNoun::Item,
                    count: 1,
                },
                "Inspected the page and found 1 item.",
            ),
            (
                Outcome::Captured {
                    full_page: false,
                    width: 1280,
                    height: 720,
                },
                "Captured the viewport at 1280x720.",
            ),
            (
                Outcome::TargetClicked {
                    host: Some("example.com".into()),
                },
                "Clicked a target on example.com.",
            ),
            (
                Outcome::PointClicked {
                    host: Some("example.com".into()),
                },
                "Clicked a point on example.com.",
            ),
            (
                Outcome::PageScrolled {
                    host: Some("example.com".into()),
                },
                "Scrolled example.com.",
            ),
            (
                Outcome::TargetRevealed {
                    host: Some("example.com".into()),
                },
                "Revealed a target on example.com.",
            ),
            (
                Outcome::ZoomSet {
                    percent: 125,
                    host: Some("example.com".into()),
                },
                "Set zoom to 125% on example.com.",
            ),
            (
                Outcome::Hovered {
                    host: Some("example.com".into()),
                },
                "Hovered a target on example.com.",
            ),
            (
                Outcome::FormFilled {
                    fields: 1,
                    submitted: false,
                },
                "Filled 1 field.",
            ),
            (
                Outcome::FormFilled {
                    fields: 3,
                    submitted: true,
                },
                "Filled 3 fields and submitted the form.",
            ),
            (
                Outcome::TextTyped {
                    host: Some("example.com".into()),
                },
                "Typed text on example.com through browser input events.",
            ),
            (
                Outcome::KeyboardSent {
                    host: Some("example.com".into()),
                },
                "Sent a keyboard action to example.com.",
            ),
            (
                Outcome::Dragged {
                    host: Some("example.com".into()),
                },
                "Completed a drag on example.com.",
            ),
            (Outcome::FilesUploaded { count: 2 }, "Uploaded 2 files."),
            (
                Outcome::ScriptEvaluated {
                    host: Some("example.com".into()),
                },
                "Evaluated a script on example.com.",
            ),
            (
                Outcome::SequenceRan {
                    completed: 2,
                    total: 5,
                },
                "Stopped at step 3 of 5.",
            ),
            (
                Outcome::SequenceRan {
                    completed: 5,
                    total: 5,
                },
                "Ran 5 steps.",
            ),
            (
                Outcome::Waited {
                    condition: "load_ready".into(),
                    elapsed_ms: 1830,
                    satisfied: true,
                },
                "Wait condition load_ready was satisfied after 1830 ms.",
            ),
            (
                Outcome::Waited {
                    condition: "text_present".into(),
                    elapsed_ms: 8000,
                    satisfied: false,
                },
                "Wait condition text_present was not satisfied within 8000 ms.",
            ),
            (
                Outcome::DialogHandled { accepted: true },
                "Accepted the browser dialog.",
            ),
            (
                Outcome::DialogHandled { accepted: false },
                "Dismissed the browser dialog.",
            ),
        ];
        for (outcome, expected) in examples {
            assert_eq!(outcome.summary(), expected);
        }
    }

    #[test]
    fn whatever_the_sentence_names_the_observation_carries() {
        let host_outcomes = [
            Outcome::TabActivated {
                host: Some("example.com".into()),
            },
            Outcome::PageOpened {
                host: Some("example.com".into()),
            },
            Outcome::PageNavigated {
                host: Some("example.com".into()),
            },
            Outcome::HistoryTraversed {
                direction: "back".into(),
                host: Some("example.com".into()),
            },
            Outcome::PageReloaded {
                host: Some("example.com".into()),
            },
            Outcome::TargetClicked {
                host: Some("example.com".into()),
            },
            Outcome::PointClicked {
                host: Some("example.com".into()),
            },
            Outcome::PageScrolled {
                host: Some("example.com".into()),
            },
            Outcome::TargetRevealed {
                host: Some("example.com".into()),
            },
            Outcome::ZoomSet {
                percent: 100,
                host: Some("example.com".into()),
            },
            Outcome::Hovered {
                host: Some("example.com".into()),
            },
            Outcome::TextTyped {
                host: Some("example.com".into()),
            },
            Outcome::KeyboardSent {
                host: Some("example.com".into()),
            },
            Outcome::Dragged {
                host: Some("example.com".into()),
            },
            Outcome::ScriptEvaluated {
                host: Some("example.com".into()),
            },
        ];
        for outcome in host_outcomes {
            assert!(outcome.summary().contains("example.com"));
            assert_eq!(outcome.observed().host.as_deref(), Some("example.com"));
        }

        let counted = [
            (Outcome::TabsListed { count: 3 }, 3),
            (Outcome::TextRead { words: 3 }, 3),
            (
                Outcome::TargetsListed {
                    noun: TargetNoun::Match,
                    count: 3,
                },
                3,
            ),
            (
                Outcome::FormFilled {
                    fields: 3,
                    submitted: false,
                },
                3,
            ),
            (Outcome::FilesUploaded { count: 3 }, 3),
            (
                Outcome::Waited {
                    condition: "load_ready".into(),
                    elapsed_ms: 3,
                    satisfied: true,
                },
                3,
            ),
        ];
        for (outcome, expected) in counted {
            assert!(outcome.summary().contains('3'));
            assert_eq!(outcome.observed().count, Some(expected));
        }

        let sequence = Outcome::SequenceRan {
            completed: 3,
            total: 5,
        };
        assert_eq!(sequence.summary(), "Stopped at step 4 of 5.");
        assert_eq!(sequence.observed().count, Some(3));

        let capture = Outcome::Captured {
            full_page: false,
            width: 1280,
            height: 720,
        };
        assert!(capture.summary().contains("1280x720"));
        assert_eq!(
            (capture.observed().width, capture.observed().height),
            (Some(1280), Some(720))
        );
    }

    #[test]
    fn refusal_wording_and_recovery_stay_exact() {
        let summaries = [
            (
                Refusal::InvalidRequest,
                "The call does not match the Ghostlight catalog.",
            ),
            (
                Refusal::CancelledBeforeStart,
                "The browser job was cancelled before it started.",
            ),
            (
                Refusal::DeadlineBeforeStart,
                "The browser job deadline expired while waiting for the workspace.",
            ),
            (
                Refusal::AuthorityBlocked,
                "Authority blocked the browser job.",
            ),
            (
                Refusal::AttentionRequired,
                "The browser job requires user attention.",
            ),
            (
                Refusal::LocalInterlock,
                "A local browser safety setting blocked this action.",
            ),
            (
                Refusal::CredentialHandoff,
                "A credential-class field requires user handoff in the visible browser.",
            ),
            (
                Refusal::IncompatibleReceipt,
                "The browser adapter returned an incompatible primitive receipt.",
            ),
            (
                Refusal::BrowserStopped { reconnect: false },
                "The browser job stopped before a physical effect.",
            ),
            (
                Refusal::EffectUnknown,
                "A browser effect was dispatched, but its final state cannot be determined.",
            ),
            (
                Refusal::LandingDeniedUnknown,
                "The landing was denied, but the new tab's final state cannot be determined.",
            ),
            (
                Refusal::WorkspaceUnusable {
                    reason: WorkspaceReason::TabUnavailable,
                },
                "The requested workspace target is not currently usable.",
            ),
            (
                Refusal::FilesUnreadable,
                "The selected local files could not be prepared safely.",
            ),
            (
                Refusal::CaptureTooLarge,
                "Screenshot exceeded the product result bound.",
            ),
            (
                Refusal::NoDialogVisible,
                "No JavaScript dialog is currently visible.",
            ),
        ];
        for (refusal, expected) in summaries {
            assert_eq!(refusal.summary(), expected);
        }
        assert_eq!(
            Refusal::LocalInterlock.next_steps(),
            vec!["The user can change the relevant Ghostlight extension setting or perform the action directly."]
        );
        assert!(Refusal::BrowserStopped { reconnect: false }
            .next_steps()
            .is_empty());
        assert_eq!(
            Refusal::BrowserStopped { reconnect: true }.next_steps(),
            vec!["Reconnect the Ghostlight browser adapter."]
        );
    }

    #[test]
    fn workspace_errors_map_to_the_language_reason_oracle() {
        let examples = [
            (WorkspaceError::StaleTab, WorkspaceReason::TabUnavailable),
            (WorkspaceError::NoTab, WorkspaceReason::TabUnavailable),
            (
                WorkspaceError::AmbiguousTab,
                WorkspaceReason::TabUnavailable,
            ),
            (WorkspaceError::StaleTarget, WorkspaceReason::StaleTarget),
            (WorkspaceError::StaleView, WorkspaceReason::StaleView),
            (
                WorkspaceError::ViewPointOutOfBounds,
                WorkspaceReason::StaleView,
            ),
            (WorkspaceError::Held, WorkspaceReason::TabHeld),
            (WorkspaceError::Busy, WorkspaceReason::WorkspaceBusy),
            (
                WorkspaceError::NotOwnedTab,
                WorkspaceReason::OwnershipMismatch,
            ),
            (
                WorkspaceError::NotOwnedTarget,
                WorkspaceReason::OwnershipMismatch,
            ),
            (
                WorkspaceError::NotOwnedView,
                WorkspaceReason::OwnershipMismatch,
            ),
            (
                WorkspaceError::TargetTabMismatch,
                WorkspaceReason::OwnershipMismatch,
            ),
            (
                WorkspaceError::ViewTabMismatch,
                WorkspaceReason::OwnershipMismatch,
            ),
            (
                WorkspaceError::PhysicalTabOwned,
                WorkspaceReason::OwnershipMismatch,
            ),
            (
                WorkspaceError::UnknownWorkspace,
                WorkspaceReason::WorkspaceClosed,
            ),
        ];
        for (error, expected) in examples {
            assert_eq!(WorkspaceReason::from(error), expected);
        }
        assert_eq!(WorkspaceReason::TabUnavailable.as_fact(), "tab_unavailable");
    }

    #[test]
    fn observed_json_shape_round_trips_unchanged() {
        let observed = Observed {
            host: Some("example.com".into()),
            readiness: Some("complete".into()),
            count: Some(3),
            width: Some(1280),
            height: Some(720),
        };
        let encoded = serde_json::to_value(&observed).unwrap();
        assert_eq!(
            encoded,
            json!({
                "host":"example.com",
                "readiness":"complete",
                "count":3,
                "width":1280,
                "height":720
            })
        );
        assert_eq!(
            serde_json::from_value::<Observed>(encoded).unwrap(),
            observed
        );
    }
}
