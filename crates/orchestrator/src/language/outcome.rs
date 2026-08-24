//! Ghostlight-authored outcome language and its content-minimized measurement projection.

use serde::{Deserialize, Serialize};

use crate::workspace::WorkspaceError;

/// What a controller is told when a person has paused Ghostlight (ADR-0126 Decision 4).
///
/// A pause refuses the next browser effect rather than suspending the invocation, because a
/// human-scale pause outlives an MCP request timeout. The refusal is reversible: a resume restores
/// ordinary work without the caller doing anything.
pub const HUMAN_PAUSE_DIRECTIVE: &str =
    "The user paused Ghostlight. Wait for further instructions.";

/// What a controller is told when a person has stopped the session (ADR-0126 Decision 5).
///
/// Terminal. Effect facts follow this sentence where they exist, and no retry is ever suggested.
pub const HUMAN_STOP_DIRECTIVE: &str =
    "The user asked to interrupt the process. Wait for further instructions.";

/// The noun named by a target-listing outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetNoun {
    /// A semantic search match.
    Match,
    /// An inspected control.
    Control,
    /// An inspected structural or mixed page item.
    Item,
}

/// The user-visible scope of one screenshot capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureKind {
    /// Current visual viewport.
    Viewport,
    /// Full document surface.
    FullPage,
    /// One semantic target.
    Target,
    /// One magnified region from a current screenshot view.
    Region,
}

/// What kind of thing an action touched, in words Ghostlight is willing to put in an audit.
///
/// A page authors its own `role` attribute, so the string that arrives is page content and can
/// say anything at all. Narrowing it here, once, at the boundary where it arrives, is what lets
/// a completed action say "Clicked a button" instead of "Clicked a target": the closed value is
/// what gets stored and spoken. A separate, governed label may identify the physical target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetRole {
    Button,
    Link,
    Checkbox,
    Radio,
    Textbox,
    Combobox,
    Slider,
    Tab,
    MenuItem,
    Option,
    Heading,
    Image,
    /// Anything Ghostlight does not recognize, including whatever a page invented.
    Control,
}

impl TargetRole {
    /// Narrow a page-authored role attribute to the closed vocabulary.
    #[must_use]
    pub fn classify(role: &str) -> Self {
        match role.trim().to_ascii_lowercase().as_str() {
            "button" => Self::Button,
            "link" => Self::Link,
            "checkbox" | "switch" => Self::Checkbox,
            "radio" => Self::Radio,
            "textbox" | "searchbox" => Self::Textbox,
            "combobox" | "listbox" | "select" => Self::Combobox,
            "slider" | "spinbutton" => Self::Slider,
            "tab" => Self::Tab,
            "menuitem" | "menuitemcheckbox" | "menuitemradio" => Self::MenuItem,
            "option" => Self::Option,
            "heading" => Self::Heading,
            "img" | "image" => Self::Image,
            _ => Self::Control,
        }
    }

    /// Render the noun for a sentence.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::Link => "link",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio button",
            Self::Textbox => "text field",
            Self::Combobox => "dropdown",
            Self::Slider => "slider",
            Self::Tab => "tab control",
            Self::MenuItem => "menu item",
            Self::Option => "option",
            Self::Heading => "heading",
            Self::Image => "image",
            Self::Control => "control",
        }
    }
}

/// Maximum number of visible characters retained from one browser-observed action label.
const TARGET_LABEL_MAX_CHARS: usize = 80;

/// The bounded human identity of the element an action actually used.
///
/// The role is always narrowed to Ghostlight's closed vocabulary. The optional name is page
/// content retained only when the effective governance snapshot permits it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSubject {
    role: TargetRole,
    label: Option<String>,
}

impl ActionSubject {
    /// Narrow and normalize one browser-observed physical subject.
    #[must_use]
    pub fn from_page(role: &str, name: &str, preserve_name: bool) -> Self {
        Self {
            role: TargetRole::classify(role),
            label: if preserve_name {
                normalized_target_label(name)
            } else {
                None
            },
        }
    }

    /// Construct the safe unnamed fallback for an already narrowed semantic target.
    #[must_use]
    pub const fn unnamed(role: TargetRole) -> Self {
        Self { role, label: None }
    }

    fn noun_phrase(&self) -> String {
        match &self.label {
            Some(label) => format!("the \"{label}\" {}", self.role.noun()),
            None => format!("{} {}", article(self.role.noun()), self.role.noun()),
        }
    }
}

/// Why configured authority refused a job, in the words a person needs to act on it.
///
/// This mirrors the governance reason code rather than importing it, so the model-facing voice
/// stays independent of the governance module it reports on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockedReason {
    /// The host was not granted by every authority layer.
    Host,
    /// The host or scheme is independently protected.
    ProtectedHost,
    /// The capability was not granted by every authority layer.
    Capability,
    /// Model-driven tab closure was refused.
    TabClose,
    /// Configured policy could not be validated.
    InvalidAuthority,
    /// A runtime control is holding browser work.
    Hold,
    /// A runtime control ended the session.
    SessionEnded,
    /// An authority layer does not admit this intake channel.
    Channel,
    /// Authority refused for a reason with no more specific wording.
    Unspecified,
}

/// Where a saved replay ended up.
///
/// Two of these never leave the browser, which is why they are separate outcomes rather than a
/// detail inside one: the sentence a reader gets should say where their replay actually is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SavedTo {
    /// Attached to a file input on a page Ghostlight controls.
    PageTarget,
    /// Written by the browser's own download mechanism.
    Download,
    /// Returned to the client that asked for it.
    Client,
}

/// What one completed browser action did in Ghostlight's product language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The workspace's bound tabs were listed, read live from the browser.
    TabsListed { count: usize },
    /// A semantic selector matched zero or several visible controls.
    SelectorUnresolved { matched: usize },
    /// One governed result-aware flow finished.
    FlowRan {
        completed: usize,
        total: usize,
        stopped: bool,
    },
    /// One flow decoded and classified without dispatching.
    FlowDecoded { steps: usize },
    /// One bounded document-tree observation was recorded.
    DocumentInspected {
        nodes: usize,
        truncated: bool,
        compared: bool,
    },
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
    TextRead { words: usize, host: Option<String> },
    /// Semantic targets were inspected or found.
    TargetsListed {
        noun: TargetNoun,
        count: usize,
        host: Option<String>,
    },
    /// A screenshot was captured.
    Captured {
        scope: CaptureKind,
        width: u32,
        height: u32,
    },
    /// A semantic target was clicked.
    TargetClicked {
        host: Option<String>,
        subject: ActionSubject,
    },
    /// A current screenshot point was clicked.
    PointClicked {
        host: Option<String>,
        x: u32,
        y: u32,
        subject: Option<ActionSubject>,
    },
    /// A page was scrolled.
    PageScrolled {
        host: Option<String>,
        direction: String,
    },
    /// A semantic target was revealed.
    TargetRevealed {
        host: Option<String>,
        subject: ActionSubject,
    },
    /// Visible tab zoom was set.
    ZoomSet { percent: u16, host: Option<String> },
    /// The browser window was resized.
    WindowResized { width: u32, height: u32 },
    /// A semantic target or current screenshot point was hovered.
    Hovered {
        host: Option<String>,
        subject: Option<ActionSubject>,
    },
    /// Ordinary form controls were filled.
    FormFilled {
        fields: usize,
        submitted: bool,
        host: Option<String>,
    },
    /// Ordinary text was typed through browser input events.
    TextTyped {
        host: Option<String>,
        subject: ActionSubject,
        characters: usize,
    },
    /// An explicit keyboard action was sent.
    ///
    /// `key` is present only for a named key. A single literal character is the caller's own
    /// input and stays out of a sentence that is written to the audit.
    KeyboardSent {
        host: Option<String>,
        key: Option<String>,
        subject: Option<ActionSubject>,
    },
    /// A drag completed.
    Dragged {
        host: Option<String>,
        source: Option<ActionSubject>,
        destination: Option<ActionSubject>,
    },
    /// Explicitly named local files were uploaded.
    FilesUploaded {
        count: usize,
        host: Option<String>,
        subject: Option<ActionSubject>,
    },
    /// A bounded page script was evaluated.
    ScriptEvaluated { host: Option<String> },
    /// An explicit observable condition was awaited.
    Waited {
        condition: String,
        elapsed_ms: u64,
        satisfied: bool,
        host: Option<String>,
    },
    /// A short sequence ran until completion or its first non-success.
    SequenceRan { completed: usize, total: usize },
    /// A browser dialog was resolved.
    DialogHandled { accepted: bool },
    /// Current JavaScript-dialog state was observed.
    DialogObserved { present: bool },
    /// Bounded console and network diagnostics were read.
    DiagnosticsRead {
        count: usize,
        capture_started: bool,
        problems_only: bool,
        host: Option<String>,
    },
    /// A memory-only recording began.
    RecordingStarted { host: Option<String> },
    /// Memory-only recording state was read.
    RecordingObserved { frames: usize, duration_ms: u64 },
    /// An active memory-only recording stopped.
    RecordingStopped { duration_ms: u64 },
    /// A recording was encoded as an animated GIF and delivered.
    RecordingSaved {
        /// How long the replay plays.
        duration_ms: u64,
        delivery: SavedTo,
    },
    /// Captured recording bytes were erased.
    RecordingDiscarded,
}

impl Outcome {
    /// Render the bounded Ghostlight-authored account of what happened.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::DocumentInspected {
                nodes,
                truncated: _,
                compared,
            } => {
                let counted = counted(*nodes, "node", "nodes");
                if *compared {
                    format!("Compared the document tree: {counted}.")
                } else {
                    format!("Recorded the document tree: {counted}.")
                }
            }
            Self::FlowRan {
                completed,
                total,
                stopped,
            } => {
                if *stopped {
                    format!("Stopped at step {completed} of {total}.")
                } else {
                    format!("Completed {total} flow steps.")
                }
            }
            Self::FlowDecoded { steps } => {
                format!("Decoded {steps} flow steps; nothing was dispatched.")
            }
            Self::SelectorUnresolved { matched } => {
                if *matched == 0 {
                    "No visible control matched the semantic selector.".into()
                } else {
                    format!(
                        "{matched} visible controls matched the semantic selector; none was chosen."
                    )
                }
            }
            Self::TabsListed { count } => {
                format!("Listed {}.", counted(*count, "bound tab", "bound tabs"))
            }
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
            Self::TextRead { words, host } => format!(
                "Read {} from {}.",
                counted(*words, "word", "words"),
                place(host, "the page")
            ),
            Self::TargetsListed {
                noun: TargetNoun::Match,
                count,
                host,
            } => format!(
                "Found {} on {}.",
                counted(*count, "match", "matches"),
                place(host, "the page")
            ),
            Self::TargetsListed {
                noun: TargetNoun::Control,
                count,
                host,
            } => format!(
                "Found {} on {}.",
                counted(*count, "control", "controls"),
                place(host, "the page")
            ),
            Self::TargetsListed {
                noun: TargetNoun::Item,
                count,
                host,
            } => format!(
                "Found {} on {}.",
                counted(*count, "item", "items"),
                place(host, "the page")
            ),
            Self::Captured {
                scope,
                width,
                height,
            } => format!(
                "Captured the {} at {width}x{height}.",
                match scope {
                    CaptureKind::Viewport => "viewport",
                    CaptureKind::FullPage => "full page",
                    CaptureKind::Target => "target",
                    CaptureKind::Region => "magnified region",
                }
            ),
            Self::TargetClicked { host, subject } => {
                format!(
                    "Clicked {} on {}.",
                    subject.noun_phrase(),
                    place(host, "the page")
                )
            }
            Self::PointClicked {
                host,
                subject: Some(subject),
                ..
            } => {
                format!(
                    "Clicked {} on {}.",
                    subject.noun_phrase(),
                    place(host, "the page")
                )
            }
            Self::PointClicked {
                host,
                x,
                y,
                subject: None,
            } => {
                format!("Clicked at {x},{y} on {}.", place(host, "the page"))
            }
            Self::PageScrolled { host, direction } => {
                format!("Scrolled {direction} on {}.", place(host, "the page"))
            }
            Self::TargetRevealed { host, subject } => format!(
                "Scrolled {} into view on {}.",
                subject.noun_phrase(),
                place(host, "the page")
            ),
            Self::ZoomSet { percent, host } => {
                format!("Set zoom to {percent}% on {}.", place(host, "the page"))
            }
            Self::WindowResized { width, height } => {
                format!("Resized the browser window to {width}x{height}.")
            }
            Self::Hovered {
                host,
                subject: Some(subject),
            } => format!(
                "Hovered {} on {}.",
                subject.noun_phrase(),
                place(host, "the page")
            ),
            Self::Hovered {
                host,
                subject: None,
            } => format!("Hovered a point on {}.", place(host, "the page")),
            Self::FormFilled {
                fields,
                submitted: false,
                host,
            } => format!(
                "Filled {} on {}.",
                counted(*fields, "field", "fields"),
                place(host, "the page")
            ),
            Self::FormFilled {
                fields,
                submitted: true,
                host,
            } => format!(
                "Filled {} on {} and submitted the form.",
                counted(*fields, "field", "fields"),
                place(host, "the page")
            ),
            Self::TextTyped {
                host,
                subject,
                characters,
            } => format!(
                "Typed {} into {} on {}.",
                counted(*characters, "character", "characters"),
                subject.noun_phrase(),
                place(host, "the page")
            ),
            Self::KeyboardSent {
                host,
                key: Some(key),
                subject,
            } => match subject {
                Some(subject) => format!(
                    "Pressed {key} in {} on {}.",
                    subject.noun_phrase(),
                    place(host, "the page")
                ),
                None => format!("Pressed {key} on {}.", place(host, "the page")),
            },
            Self::KeyboardSent {
                host,
                key: None,
                subject,
            } => match subject {
                Some(subject) => format!(
                    "Pressed a key in {} on {}.",
                    subject.noun_phrase(),
                    place(host, "the page")
                ),
                None => format!("Pressed a key on {}.", place(host, "the page")),
            },
            Self::Dragged {
                host,
                source: Some(source),
                destination: Some(destination),
            } => format!(
                "Dragged {} onto {} on {}.",
                source.noun_phrase(),
                destination.noun_phrase(),
                place(host, "the page")
            ),
            Self::Dragged { host, .. } => format!(
                "Dragged one point to another on {}.",
                place(host, "the page")
            ),
            Self::FilesUploaded {
                count,
                host,
                subject,
            } => match subject {
                Some(subject) => format!(
                    "Uploaded {} through {} on {}.",
                    counted(*count, "file", "files"),
                    subject.noun_phrase(),
                    place(host, "the page")
                ),
                None => format!(
                    "Uploaded {} to {}.",
                    counted(*count, "file", "files"),
                    place(host, "the page")
                ),
            },
            Self::ScriptEvaluated { host } => {
                format!("Executed JavaScript on {}.", place(host, "the page"))
            }
            Self::Waited {
                condition,
                elapsed_ms,
                satisfied,
                host,
            } => waited(condition, *elapsed_ms, *satisfied, host),
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
            Self::DiagnosticsRead {
                count,
                problems_only,
                host,
                ..
            } => format!(
                "Found {} on {}.",
                if *problems_only {
                    counted(*count, "problem", "problems")
                } else {
                    counted(*count, "observation", "observations")
                },
                place(host, "the page")
            ),
            Self::RecordingStarted { host } => {
                format!("Started recording {}.", place(host, "the page"))
            }
            Self::RecordingObserved {
                frames,
                duration_ms,
            } => format!(
                "Recording for {}, {} so far.",
                spanned(*duration_ms),
                counted(*frames, "frame", "frames")
            ),
            Self::RecordingStopped { duration_ms } => {
                format!("Stopped recording after {}.", spanned(*duration_ms))
            }
            Self::RecordingSaved {
                duration_ms,
                delivery,
            } => {
                let replay = format!("a replay of {} of page changes", spanned(*duration_ms));
                match delivery {
                    SavedTo::Client => format!("Saved {replay}."),
                    SavedTo::PageTarget => format!("Attached {replay} to the page."),
                    SavedTo::Download => format!("Downloaded {replay}."),
                }
            }
            Self::RecordingDiscarded => "Erased the recording.".into(),
        }
    }

    /// Render zero or one safe contextual recovery actions for this outcome.
    #[must_use]
    pub fn next_steps(&self) -> Vec<String> {
        match self {
            Self::DocumentInspected {
                truncated: true, ..
            } => vec!["Narrow the subtree root or depth to capture the rest.".into()],
            Self::SelectorUnresolved { .. } => vec![
                "Use browser_find with text visible on the page, inspect for fresh handles, or narrow the selector with role and exact.".into(),
            ],
            Self::FlowRan {
                completed,
                total,
                ..
            } if completed < total => vec![
                "Use the per-step results to find what went wrong, fix that step, and run the flow again."
                    .into(),
            ],
            Self::SequenceRan { completed, total } if completed < total => vec![
                "Find the step that stopped the sequence in the results, fix it, and run the sequence again."
                    .into(),
            ],
            Self::Waited {
                satisfied: false, ..
            } => vec![
                "Read or inspect the page to see its current state before choosing another action."
                    .into(),
            ],
            Self::DiagnosticsRead {
                count: 0,
                capture_started: true,
                ..
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
            | Self::SequenceRan {
                completed: count, ..
            } => Observed {
                count: measured(*count),
                ..Observed::default()
            },
            Self::DocumentInspected { nodes, .. } => Observed {
                count: measured(*nodes),
                ..Observed::default()
            },
            Self::FlowRan { completed, .. } | Self::FlowDecoded { steps: completed } => Observed {
                count: measured(*completed),
                ..Observed::default()
            },
            Self::SelectorUnresolved { matched: count } => Observed {
                count: measured(*count),
                ..Observed::default()
            },
            // Both halves of the sentence, so the record says as much as the words beside it.
            Self::TextRead { words: count, host }
            | Self::TargetsListed { count, host, .. }
            | Self::FilesUploaded { count, host, .. }
            | Self::DiagnosticsRead { count, host, .. }
            | Self::FormFilled {
                fields: count,
                host,
                ..
            } => Observed {
                count: measured(*count),
                host: host.clone(),
                ..Observed::default()
            },
            Self::TextTyped {
                characters: count,
                host,
                ..
            } => Observed {
                count: measured(*count),
                host: host.clone(),
                ..Observed::default()
            },
            Self::RecordingObserved { frames: count, .. } => Observed {
                count: measured(*count),
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
            // The audit line and its sentence must name the same number. The exact milliseconds
            // stay in the facts, where precision belongs.
            Self::Waited {
                elapsed_ms, host, ..
            } => Observed {
                count: measured(whole_seconds(*elapsed_ms)),
                host: host.clone(),
                ..Observed::default()
            },
            Self::RecordingStopped {
                duration_ms: measure,
            }
            | Self::RecordingSaved {
                duration_ms: measure,
                ..
            } => Observed {
                count: measured(whole_seconds(*measure)),
                ..Observed::default()
            },
            Self::TabActivated { host }
            | Self::PageOpened { host }
            | Self::PageNavigated { host }
            | Self::HistoryTraversed { host, .. }
            | Self::PageReloaded { host }
            | Self::TargetClicked { host, .. }
            | Self::PointClicked { host, .. }
            | Self::PageScrolled { host, .. }
            | Self::TargetRevealed { host, .. }
            | Self::ZoomSet { host, .. }
            | Self::Hovered { host, .. }
            | Self::KeyboardSent { host, .. }
            | Self::Dragged { host, .. }
            | Self::ScriptEvaluated { host }
            | Self::RecordingStarted { host } => Observed {
                host: host.clone(),
                ..Observed::default()
            },
            Self::TabClosed
            | Self::DialogHandled { .. }
            | Self::DialogObserved { .. }
            | Self::RecordingDiscarded => Observed::default(),
        }
    }
}

/// Closed product-language reason for a browser-readiness recovery failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserRecoveryReason {
    /// No supported browser installation was found.
    BrowserAbsent,
    /// Starting the selected browser failed.
    LaunchFailed,
    /// Only an unsupported sandboxed package was found.
    SandboxedPackage,
    /// The selected browser does not have the Ghostlight extension.
    ExtensionAbsent,
    /// The native messaging registration cannot be used safely.
    NativeHostUnavailable,
    /// The opened browser is not the profile this session belongs to.
    WrongProfile,
    /// The selected adapter did not arrive within the bounded wait.
    HandshakeTimeout,
    /// More than one installed browser is equally plausible.
    Ambiguous,
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
    ///
    /// The host is present whenever the refused work named one. It is the same deliberate
    /// exception the audit already makes for a governed host: it answers "where was the agent
    /// trying to go", and the identifying detail of a URL lives after it.
    AuthorityBlocked {
        reason: BlockedReason,
        host: Option<String>,
    },
    /// Runtime control requires the user.
    AttentionRequired,
    /// The browser-local physical safety setting refused the action.
    LocalInterlock,
    /// A credential-class field requires visible user handoff.
    CredentialHandoff,
    /// The browser returned a receipt outside the negotiated contract.
    IncompatibleReceipt,
    /// The job ran out of time.
    DeadlineExpired {
        /// True when the deadline fired before any dispatch could happen.
        before_dispatch: bool,
    },
    /// The browser answered the job with its own bounded refusal.
    BrowserPrimitive { detail: String },
    /// The browser stopped before a physical effect.
    BrowserStopped { reconnect: bool },
    /// Several browsers are connected and the call did not say which one it meant.
    BrowserAmbiguous,
    /// The named browser is not connected.
    BrowserUnknown,
    /// This session already works in a different browser.
    BrowserPinned,
    /// The configured posture leaves browser startup to the person.
    BrowserStartupManual { browser: Option<String> },
    /// Automatic readiness recovery failed before any browser effect.
    BrowserRecoveryFailed { reason: BrowserRecoveryReason },
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
            Self::AuthorityBlocked { reason, host } => return blocked(*reason, host),
            Self::AttentionRequired => "The browser job requires user attention.",
            Self::LocalInterlock => "Kept the tab open: Ghostlight's preserve-tabs setting is on.",
            Self::CredentialHandoff => {
                "A credential-class field requires user handoff in the visible browser."
            }
            Self::IncompatibleReceipt => {
                "The browser answered in a form Ghostlight does not recognize."
            }
            Self::BrowserPrimitive { detail } => {
                return format!("The browser refused this job: {detail}.");
            }
            Self::DeadlineExpired { before_dispatch } => {
                if *before_dispatch {
                    "The job ran out of time before reaching the browser."
                } else {
                    "Sent, but the job ran out of time before the browser confirmed."
                }
            }
            Self::BrowserStopped { .. } => "The browser disconnected before anything happened.",
            Self::BrowserAmbiguous => {
                "More than one browser is connected, so there is no single place to open this."
            }
            Self::BrowserUnknown => "That browser is not connected.",
            Self::BrowserPinned => "This session is already working in a different browser.",
            Self::BrowserStartupManual { browser } => {
                return browser.as_ref().map_or_else(
                    || "No browser is connected. Start a supported Chromium browser with the Ghostlight extension installed.".into(),
                    |name| format!("No browser is connected. Start {name} to continue."),
                );
            }
            Self::BrowserRecoveryFailed { reason } => match reason {
                BrowserRecoveryReason::BrowserAbsent => {
                    "No supported Chromium browser is installed."
                }
                BrowserRecoveryReason::LaunchFailed => {
                    "Ghostlight could not start the selected browser."
                }
                BrowserRecoveryReason::SandboxedPackage => {
                    "The installed browser is sandboxed and cannot start Ghostlight's native connector."
                }
                BrowserRecoveryReason::ExtensionAbsent => {
                    "The selected browser does not have the Ghostlight extension installed."
                }
                BrowserRecoveryReason::NativeHostUnavailable => {
                    "The browser cannot use Ghostlight's native messaging registration."
                }
                BrowserRecoveryReason::WrongProfile => {
                    "This session belongs to a browser profile that is not connected."
                }
                BrowserRecoveryReason::HandshakeTimeout => {
                    "The browser started, but its Ghostlight adapter did not connect in time."
                }
                BrowserRecoveryReason::Ambiguous => {
                    "More than one installed browser could handle this work, so Ghostlight did not choose one."
                }
            },
            Self::EffectUnknown => "Sent, but the browser never confirmed what happened.",
            Self::LandingDeniedUnknown => {
                "Blocked the landing, but the new tab's final state is unknown."
            }
            Self::WorkspaceUnusable { reason } => return reason.summary(),
            Self::FilesUnreadable => "The selected local files could not be prepared safely.",
            Self::CaptureTooLarge => "The screenshot was too large to return.",
            Self::NoDialogVisible => "No JavaScript dialog is currently visible.",
            Self::RecordingUnavailable => "That recording is no longer available.",
            Self::RecordingExportFailed => "The recording could not be turned into a replay.",
        }
        .into()
    }

    /// Render zero or one safe contextual recovery actions for this refusal.
    #[must_use]
    pub fn next_steps(&self) -> Vec<String> {
        match self {
            Self::InvalidRequest => {
                vec!["Match the call to the advertised schema; the invalid_input detail states exactly what to change.".into()]
            }
            Self::DeadlineBeforeStart => vec![
                "Repeat the call when the current Ghostlight action has finished.".into(),
            ],
            Self::LocalInterlock => vec![
                "The user can change the relevant Ghostlight extension setting or perform the action directly."
                    .into(),
            ],
            Self::CredentialHandoff => vec![
                "Complete the credential field in the visible browser, then inspect the page again."
                    .into(),
            ],
            Self::IncompatibleReceipt => vec![
                "Reload or update the Ghostlight extension in that browser, then repeat the call."
                    .into(),
            ],
            Self::BrowserStopped { reconnect: true } => {
                vec!["Reconnect the Ghostlight browser adapter.".into()]
            }
            Self::BrowserPrimitive { .. } => vec![
                "Read the browser's stated reason, adjust the call or the page, then repeat."
                    .into(),
            ],
            Self::DeadlineExpired { .. } => vec![
                "Repeat with a longer timeout_ms when the page genuinely needs more time."
                    .into(),
            ],
            Self::BrowserAmbiguous | Self::BrowserUnknown => vec![
                "Call browser_tabs with action list to see the connected browsers.".into(),
                "Repeat the call with the browser handle you want it to open in.".into(),
            ],
            Self::BrowserPinned => vec![
                "Omit browser to continue in the browser this session already works in.".into(),
            ],
            Self::BrowserStartupManual { .. } => vec![
                "Start the browser you normally use with the Ghostlight extension installed, then repeat the call."
                    .into(),
            ],
            Self::BrowserRecoveryFailed { reason } => match reason {
                BrowserRecoveryReason::BrowserAbsent => vec![
                    "Install Chrome, Edge, Brave, or Chromium as a native package, then run ghostlight install."
                        .into(),
                ],
                BrowserRecoveryReason::SandboxedPackage => vec![
                    "Install a supported native browser package; Snap and Flatpak browsers cannot start a native messaging host."
                        .into(),
                ],
                BrowserRecoveryReason::NativeHostUnavailable => {
                    vec!["Run ghostlight doctor, then repair the named browser registration.".into()]
                }
                BrowserRecoveryReason::ExtensionAbsent => vec![
                    "Install Ghostlight in the selected browser profile, then repeat the call."
                        .into(),
                ],
                BrowserRecoveryReason::WrongProfile => vec![
                    "Open the browser profile this Ghostlight session already uses, then repeat the call."
                        .into(),
                ],
                BrowserRecoveryReason::Ambiguous => vec![
                    "Start the browser you want to use, then repeat the call with its browser handle."
                        .into(),
                ],
                BrowserRecoveryReason::LaunchFailed | BrowserRecoveryReason::HandshakeTimeout => {
                    vec!["Start the selected browser yourself, then repeat the call.".into()]
                }
            },
            Self::WorkspaceUnusable { reason } => reason.next_steps(),
            Self::FilesUnreadable => vec![
                "Confirm each path is an existing file within Ghostlight's upload limits.".into(),
            ],
            Self::CaptureTooLarge => vec![
                "Capture the viewport or a smaller region instead of the full page.".into(),
            ],
            Self::RecordingUnavailable => vec![
                "Use browser_record with action status and an explicit recording handle when more than one exists."
                    .into(),
            ],
            Self::RecordingExportFailed => vec![
                "Inspect recording status, then discard it or start a shorter recording."
                    .into(),
            ],
            Self::EffectUnknown => vec![
                "If a JavaScript dialog may be open on the page, handle it with browser_dialog; handling checks the page directly."
                    .into(),
                "Then observe the page with browser_read or browser_inspect to learn what happened."
                    .into(),
            ],
            _ => vec![],
        }
    }

    /// Project the content-free facts named by this refusal into the audit vocabulary.
    #[must_use]
    pub fn observed(&self) -> Observed {
        match self {
            Self::AuthorityBlocked { host, .. } => Observed {
                host: host.clone(),
                ..Observed::default()
            },
            _ => Observed::default(),
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
    /// Say which handle went stale and how, rather than that something is "not usable".
    #[must_use]
    pub fn summary(self) -> String {
        match self {
            Self::TabUnavailable => "That tab is no longer open.",
            Self::StaleTarget => "That target belongs to an older version of the page.",
            Self::StaleView => "The page has moved since that screenshot was taken.",
            Self::TabHeld => "Ghostlight is paused on that tab.",
            Self::WorkspaceBusy => "Another Ghostlight action is already using this session.",
            Self::OwnershipMismatch => "That handle belongs to a different Ghostlight session.",
            Self::WorkspaceClosed => "This Ghostlight session has ended.",
        }
        .into()
    }

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
            Self::TabHeld => vec![
                "Resume browser work from the Ghostlight window or tray, then repeat the call."
                    .into(),
            ],
            Self::WorkspaceBusy => {
                vec!["Wait for the active Ghostlight invocation to finish.".into()]
            }
            Self::OwnershipMismatch => {
                vec!["Collect fresh handles from this session, then continue with those.".into()]
            }
            Self::WorkspaceClosed => vec![
                "Start over; the next call opens a fresh session and old handles will not work."
                    .into(),
            ],
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
            WorkspaceError::StaleView
            | WorkspaceError::ViewPointOutOfBounds
            | WorkspaceError::ViewRegionOutOfBounds => Self::StaleView,
            WorkspaceError::Held => Self::TabHeld,
            WorkspaceError::Busy => Self::WorkspaceBusy,
            WorkspaceError::NotOwnedTab
            | WorkspaceError::NotOwnedTarget
            | WorkspaceError::NotOwnedView
            | WorkspaceError::TargetTabMismatch
            | WorkspaceError::ViewTabMismatch
            | WorkspaceError::PhysicalTabOwned
            | WorkspaceError::BrowserPinned => Self::OwnershipMismatch,
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
    /// Governed host the action attempted or landed on, lowercased.
    ///
    /// Never the path, query, or fragment. The host answers "where did the agent go or try to go"
    /// and is already visible in the user's own request or tab strip; the path is where a record
    /// number would sit.
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

/// Name what authority refused and where, instead of that authority refused.
///
/// "Authority blocked the browser job" is true of every denial there is, which makes it useless
/// to the person reading it. The reason code and the attempted host were both already on hand.
fn blocked(reason: BlockedReason, host: &Option<String>) -> String {
    match (reason, host.as_deref()) {
        (BlockedReason::Host, Some(host)) => format!("Blocked: {host} is not an allowed host."),
        (BlockedReason::Host, None) => "Blocked: that host is not allowed.".into(),
        (BlockedReason::ProtectedHost, Some(host)) => {
            format!("Blocked: {host} is protected and is never automated.")
        }
        (BlockedReason::ProtectedHost, None) => {
            "Blocked: that host is protected and is never automated.".into()
        }
        (BlockedReason::Capability, _) => {
            "Blocked: this session may not take that kind of action.".into()
        }
        (BlockedReason::TabClose, _) => "Blocked: this session may not close tabs.".into(),
        (BlockedReason::InvalidAuthority, _) => {
            "Blocked: the Ghostlight policy could not be read.".into()
        }
        // The two human controls are the only refusals that speak to the controller rather than
        // about the request. A model that reads "Blocked: Ghostlight is paused." has been told a
        // fact and left to guess; these tell it what to do, and are pinned by ADR-0126 Decisions 4
        // and 5. Pause is reversible and says so by not being terminal; stop ends the session.
        (BlockedReason::Hold, _) => HUMAN_PAUSE_DIRECTIVE.into(),
        (BlockedReason::SessionEnded, _) => HUMAN_STOP_DIRECTIVE.into(),
        (BlockedReason::Channel, _) => "Blocked: this intake channel is disabled.".into(),
        (BlockedReason::Unspecified, Some(host)) => format!("Blocked by policy on {host}."),
        (BlockedReason::Unspecified, None) => "Blocked by Ghostlight policy.".into(),
    }
}

/// Say what the page did, not which condition constant was evaluated.
///
/// "Wait condition text_present was satisfied after 1687 ms" describes the mechanism doing the
/// looking. A reader wants to know that the text showed up, and roughly when.
fn waited(condition: &str, elapsed_ms: u64, satisfied: bool, host: &Option<String>) -> String {
    let (subject, arrived, missing) = match condition {
        "load_ready" => (None, "finished loading", "never finished loading"),
        "text_present" => (Some("Text"), "appeared", "never appeared"),
        "text_absent" => (Some("Text"), "disappeared", "never disappeared"),
        "url_contains" => (Some("The address"), "matched", "never matched"),
        "target_present" => (Some("The target"), "appeared", "never appeared"),
        "target_absent" => (Some("The target"), "disappeared", "never disappeared"),
        _ => (Some("The condition"), "was met", "was never met"),
    };
    let location = place(host, "the page");
    let clause = |verb: &str| {
        subject.map_or_else(
            || format!("{location} {verb}"),
            |subject| format!("{subject} {verb} on {location}"),
        )
    };
    if satisfied {
        format!("{} in {}.", clause(arrived), spanned(elapsed_ms))
    } else {
        format!("{} within {}.", clause(missing), spanned(elapsed_ms))
    }
}

/// "30 seconds", the way someone watching would say it.
///
/// A replay is worth as much time as it plays for. How many frames survived encoding and how
/// many bytes they became are mechanism: real, worth keeping in the facts, and of no interest
/// to whoever asked for the recording.
fn spanned(duration_ms: u64) -> String {
    match whole_seconds(duration_ms) {
        0 => "under a second".into(),
        seconds => counted(seconds, "second", "seconds"),
    }
}

/// The rounded second count the summary says, so the measurement beside it agrees.
fn whole_seconds(duration_ms: u64) -> usize {
    if duration_ms < 500 {
        return 0;
    }
    usize::try_from(duration_ms.saturating_add(500) / 1_000).unwrap_or(usize::MAX)
}

fn normalized_target_label(value: &str) -> Option<String> {
    let cleaned: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                ' '
            } else if character == '"' {
                '\''
            } else {
                character
            }
        })
        .collect();
    let one_line = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        return None;
    }
    let count = one_line.chars().count();
    if count <= TARGET_LABEL_MAX_CHARS {
        return Some(one_line);
    }
    let mut bounded: String = one_line.chars().take(TARGET_LABEL_MAX_CHARS - 3).collect();
    bounded.push_str("...");
    Some(bounded)
}

const fn article(noun: &str) -> &'static str {
    match noun.as_bytes().first() {
        Some(b'a' | b'e' | b'i' | b'o' | b'u') => "an",
        _ => "a",
    }
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

    use super::{
        ActionSubject, BlockedReason, CaptureKind, Observed, Outcome, Refusal, SavedTo, TargetNoun,
        TargetRole, WorkspaceReason, HUMAN_PAUSE_DIRECTIVE, HUMAN_STOP_DIRECTIVE,
        TARGET_LABEL_MAX_CHARS,
    };
    use crate::workspace::WorkspaceError;

    #[test]
    fn outcome_summaries_transcribe_the_product_oracles() {
        let examples = vec![
            (Outcome::TabsListed { count: 4 }, "Listed 4 bound tabs."),
            (Outcome::TabsListed { count: 1 }, "Listed 1 bound tab."),
            (
                Outcome::TabActivated {
                    host: Some("example.com".into()),
                },
                "Brought example.com into view.",
            ),
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
                Outcome::TextRead {
                    words: 1_240,
                    host: Some("example.com".into()),
                },
                "Read 1,240 words from example.com.",
            ),
            (
                Outcome::TargetsListed {
                    noun: TargetNoun::Match,
                    count: 7,
                    host: Some("example.com".into()),
                },
                "Found 7 matches on example.com.",
            ),
            (
                Outcome::TargetsListed {
                    noun: TargetNoun::Control,
                    count: 29,
                    host: Some("example.com".into()),
                },
                "Found 29 controls on example.com.",
            ),
            (
                Outcome::TargetsListed {
                    noun: TargetNoun::Item,
                    count: 3,
                    host: Some("example.com".into()),
                },
                "Found 3 items on example.com.",
            ),
            (
                Outcome::Captured {
                    scope: CaptureKind::Viewport,
                    width: 1280,
                    height: 720,
                },
                "Captured the viewport at 1280x720.",
            ),
            (
                Outcome::TargetClicked {
                    host: Some("example.com".into()),
                    subject: ActionSubject::from_page("button", "Save", true),
                },
                "Clicked the \"Save\" button on example.com.",
            ),
            (
                Outcome::PointClicked {
                    host: Some("example.com".into()),
                    x: 640,
                    y: 360,
                    subject: None,
                },
                "Clicked at 640,360 on example.com.",
            ),
            (
                Outcome::PageScrolled {
                    host: Some("example.com".into()),
                    direction: "down".into(),
                },
                "Scrolled down on example.com.",
            ),
            (
                Outcome::TargetRevealed {
                    host: Some("example.com".into()),
                    subject: ActionSubject::from_page("checkbox", "Publish", true),
                },
                "Scrolled the \"Publish\" checkbox into view on example.com.",
            ),
            (
                Outcome::ZoomSet {
                    percent: 125,
                    host: Some("example.com".into()),
                },
                "Set zoom to 125% on example.com.",
            ),
            (
                Outcome::WindowResized {
                    width: 1280,
                    height: 800,
                },
                "Resized the browser window to 1280x800.",
            ),
            (
                Outcome::Hovered {
                    host: Some("example.com".into()),
                    subject: Some(ActionSubject::from_page("link", "Details", true)),
                },
                "Hovered the \"Details\" link on example.com.",
            ),
            (
                Outcome::Hovered {
                    host: Some("example.com".into()),
                    subject: None,
                },
                "Hovered a point on example.com.",
            ),
            (
                Outcome::FormFilled {
                    fields: 1,
                    submitted: false,
                    host: Some("example.com".into()),
                },
                "Filled 1 field on example.com.",
            ),
            (
                Outcome::FormFilled {
                    fields: 3,
                    submitted: true,
                    host: Some("example.com".into()),
                },
                "Filled 3 fields on example.com and submitted the form.",
            ),
            (
                Outcome::TextTyped {
                    host: Some("example.com".into()),
                    subject: ActionSubject::from_page("textbox", "Email", true),
                    characters: 54,
                },
                "Typed 54 characters into the \"Email\" text field on example.com.",
            ),
            (
                Outcome::KeyboardSent {
                    host: Some("example.com".into()),
                    key: Some("Enter".into()),
                    subject: None,
                },
                "Pressed Enter on example.com.",
            ),
            (
                Outcome::KeyboardSent {
                    host: Some("example.com".into()),
                    key: None,
                    subject: None,
                },
                "Pressed a key on example.com.",
            ),
            (
                Outcome::Dragged {
                    host: Some("example.com".into()),
                    source: Some(ActionSubject::from_page("button", "Ticket", true)),
                    destination: Some(ActionSubject::from_page("link", "Ready", true)),
                },
                "Dragged the \"Ticket\" button onto the \"Ready\" link on example.com.",
            ),
            (
                Outcome::Dragged {
                    host: Some("example.com".into()),
                    source: None,
                    destination: None,
                },
                "Dragged one point to another on example.com.",
            ),
            (
                Outcome::FilesUploaded {
                    count: 2,
                    host: Some("example.com".into()),
                    subject: None,
                },
                "Uploaded 2 files to example.com.",
            ),
            (
                Outcome::ScriptEvaluated {
                    host: Some("example.com".into()),
                },
                "Executed JavaScript on example.com.",
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
                    host: Some("example.com".into()),
                },
                "example.com finished loading in 2 seconds.",
            ),
            (
                Outcome::Waited {
                    condition: "text_present".into(),
                    elapsed_ms: 8000,
                    satisfied: false,
                    host: Some("example.com".into()),
                },
                "Text never appeared on example.com within 8 seconds.",
            ),
            (
                Outcome::DialogHandled { accepted: true },
                "Accepted the browser dialog.",
            ),
            (
                Outcome::DialogHandled { accepted: false },
                "Dismissed the browser dialog.",
            ),
            (
                Outcome::DialogObserved { present: true },
                "A JavaScript dialog is currently visible.",
            ),
            (
                Outcome::DialogObserved { present: false },
                "No JavaScript dialog is currently visible.",
            ),
            (
                Outcome::DiagnosticsRead {
                    count: 3,
                    capture_started: false,
                    problems_only: true,
                    host: Some("example.com".into()),
                },
                "Found 3 problems on example.com.",
            ),
            (
                Outcome::DiagnosticsRead {
                    count: 2,
                    capture_started: false,
                    problems_only: false,
                    host: Some("example.com".into()),
                },
                "Found 2 observations on example.com.",
            ),
            (
                Outcome::RecordingStarted {
                    host: Some("example.com".into()),
                },
                "Started recording example.com.",
            ),
            (
                Outcome::RecordingObserved {
                    frames: 3,
                    duration_ms: 1200,
                },
                "Recording for 1 second, 3 frames so far.",
            ),
            (
                Outcome::RecordingStopped {
                    duration_ms: 30_400,
                },
                "Stopped recording after 30 seconds.",
            ),
            (
                Outcome::RecordingSaved {
                    duration_ms: 30_400,
                    delivery: SavedTo::Client,
                },
                "Saved a replay of 30 seconds of page changes.",
            ),
            (
                Outcome::RecordingSaved {
                    duration_ms: 30_400,
                    delivery: SavedTo::PageTarget,
                },
                "Attached a replay of 30 seconds of page changes to the page.",
            ),
            (
                Outcome::RecordingSaved {
                    duration_ms: 30_400,
                    delivery: SavedTo::Download,
                },
                "Downloaded a replay of 30 seconds of page changes.",
            ),
            (Outcome::RecordingDiscarded, "Erased the recording."),
        ];
        for (outcome, expected) in examples {
            assert_eq!(outcome.summary(), expected);
        }
    }

    #[test]
    fn page_authored_roles_collapse_to_a_closed_audit_vocabulary() {
        assert_eq!(TargetRole::classify("button"), TargetRole::Button);
        assert_eq!(TargetRole::classify("SWITCH"), TargetRole::Checkbox);
        assert_eq!(
            TargetRole::classify("Save my document"),
            TargetRole::Control
        );
        assert_eq!(
            TargetRole::classify("button onclick=steal"),
            TargetRole::Control
        );
        assert_eq!(TargetRole::Control.noun(), "control");
    }

    #[test]
    fn action_labels_are_bounded_normalized_and_governed() {
        let subject = ActionSubject::from_page(
            "button onclick=steal",
            "  Save\n\"patient\"\u{202e}   record  ",
            true,
        );
        assert_eq!(
            Outcome::TargetClicked {
                host: Some("example.com".into()),
                subject,
            }
            .summary(),
            "Clicked the \"Save 'patient' record\" control on example.com."
        );

        let hidden = ActionSubject::from_page("img", "private image name", false);
        assert_eq!(
            Outcome::TargetClicked {
                host: Some("example.com".into()),
                subject: hidden,
            }
            .summary(),
            "Clicked an image on example.com."
        );

        let long = "x".repeat(TARGET_LABEL_MAX_CHARS + 20);
        let bounded = ActionSubject::from_page("button", &long, true);
        let summary = Outcome::TargetClicked {
            host: None,
            subject: bounded,
        }
        .summary();
        assert!(summary.contains(&format!("{}...", "x".repeat(77))));
        assert!(!summary.contains(&"x".repeat(81)));
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
                subject: ActionSubject::unnamed(TargetRole::Button),
            },
            Outcome::PointClicked {
                host: Some("example.com".into()),
                x: 10,
                y: 20,
                subject: None,
            },
            Outcome::PageScrolled {
                host: Some("example.com".into()),
                direction: "down".into(),
            },
            Outcome::TargetRevealed {
                host: Some("example.com".into()),
                subject: ActionSubject::unnamed(TargetRole::Checkbox),
            },
            Outcome::ZoomSet {
                percent: 100,
                host: Some("example.com".into()),
            },
            Outcome::Hovered {
                host: Some("example.com".into()),
                subject: Some(ActionSubject::unnamed(TargetRole::Button)),
            },
            Outcome::TextTyped {
                host: Some("example.com".into()),
                subject: ActionSubject::unnamed(TargetRole::Textbox),
                characters: 3,
            },
            Outcome::KeyboardSent {
                host: Some("example.com".into()),
                key: Some("Enter".into()),
                subject: None,
            },
            Outcome::Dragged {
                host: Some("example.com".into()),
                source: Some(ActionSubject::unnamed(TargetRole::Button)),
                destination: Some(ActionSubject::unnamed(TargetRole::Link)),
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
            (
                Outcome::TextRead {
                    words: 3,
                    host: Some("example.com".into()),
                },
                3,
            ),
            (
                Outcome::TargetsListed {
                    noun: TargetNoun::Match,
                    count: 3,
                    host: Some("example.com".into()),
                },
                3,
            ),
            (
                Outcome::FormFilled {
                    fields: 3,
                    submitted: false,
                    host: Some("example.com".into()),
                },
                3,
            ),
            (
                Outcome::FilesUploaded {
                    count: 3,
                    host: Some("example.com".into()),
                    subject: None,
                },
                3,
            ),
            (
                Outcome::Waited {
                    condition: "load_ready".into(),
                    elapsed_ms: 3_000,
                    satisfied: true,
                    host: Some("example.com".into()),
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
            scope: CaptureKind::Viewport,
            width: 1280,
            height: 720,
        };
        assert!(capture.summary().contains("1280x720"));
        assert_eq!(
            (capture.observed().width, capture.observed().height),
            (Some(1280), Some(720))
        );
    }

    /// The stop directive is exactly what ADR-0126 Decision 5 pins, to the character.
    #[test]
    fn stop_outcome_begins_with_the_pinned_directive() {
        let refusal = Refusal::AuthorityBlocked {
            reason: BlockedReason::SessionEnded,
            host: None,
        };
        assert_eq!(
            refusal.summary(),
            "The user asked to interrupt the process. Wait for further instructions."
        );
        assert!(refusal
            .summary()
            .starts_with("The user asked to interrupt the process."));
    }

    /// The pause directive is exactly what ADR-0126 Decision 4 pins, and is not the stop one.
    #[test]
    fn pause_outcome_uses_its_own_pinned_directive() {
        let refusal = Refusal::AuthorityBlocked {
            reason: BlockedReason::Hold,
            host: None,
        };
        assert_eq!(
            refusal.summary(),
            "The user paused Ghostlight. Wait for further instructions."
        );
        assert_ne!(refusal.summary(), HUMAN_STOP_DIRECTIVE);
    }

    /// Neither human control suggests trying again. A retry would fight the person who asked.
    #[test]
    fn stop_recommends_no_automatic_retry() {
        for reason in [BlockedReason::SessionEnded, BlockedReason::Hold] {
            let refusal = Refusal::AuthorityBlocked { reason, host: None };
            assert!(
                refusal.next_steps().is_empty(),
                "{reason:?} suggested a next step"
            );
        }
    }

    /// A policy attention hold keeps its own words rather than borrowing the person's pause.
    #[test]
    fn attention_is_not_worded_as_a_human_pause() {
        let attention = Refusal::AttentionRequired;
        assert_ne!(attention.summary(), HUMAN_PAUSE_DIRECTIVE);
        assert_ne!(attention.summary(), HUMAN_STOP_DIRECTIVE);
    }

    #[test]
    fn refusal_wording_and_recovery_stay_exact() {
        let summaries = vec![
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
                Refusal::AuthorityBlocked {
                    reason: BlockedReason::Host,
                    host: Some("example.com".into()),
                },
                "Blocked: example.com is not an allowed host.",
            ),
            (
                Refusal::AuthorityBlocked {
                    reason: BlockedReason::Hold,
                    host: None,
                },
                HUMAN_PAUSE_DIRECTIVE,
            ),
            (
                Refusal::AuthorityBlocked {
                    reason: BlockedReason::SessionEnded,
                    host: None,
                },
                HUMAN_STOP_DIRECTIVE,
            ),
            (
                Refusal::AttentionRequired,
                "The browser job requires user attention.",
            ),
            (
                Refusal::LocalInterlock,
                "Kept the tab open: Ghostlight's preserve-tabs setting is on.",
            ),
            (
                Refusal::CredentialHandoff,
                "A credential-class field requires user handoff in the visible browser.",
            ),
            (
                Refusal::IncompatibleReceipt,
                "The browser answered in a form Ghostlight does not recognize.",
            ),
            (
                Refusal::BrowserStopped { reconnect: false },
                "The browser disconnected before anything happened.",
            ),
            (
                Refusal::EffectUnknown,
                "Sent, but the browser never confirmed what happened.",
            ),
            (
                Refusal::LandingDeniedUnknown,
                "Blocked the landing, but the new tab's final state is unknown.",
            ),
            (
                Refusal::WorkspaceUnusable {
                    reason: WorkspaceReason::TabUnavailable,
                },
                "That tab is no longer open.",
            ),
            (
                Refusal::FilesUnreadable,
                "The selected local files could not be prepared safely.",
            ),
            (
                Refusal::CaptureTooLarge,
                "The screenshot was too large to return.",
            ),
            (
                Refusal::NoDialogVisible,
                "No JavaScript dialog is currently visible.",
            ),
            (
                Refusal::RecordingUnavailable,
                "That recording is no longer available.",
            ),
            (
                Refusal::RecordingExportFailed,
                "The recording could not be turned into a replay.",
            ),
            (
                Refusal::DeadlineExpired {
                    before_dispatch: true,
                },
                "The job ran out of time before reaching the browser.",
            ),
            (
                Refusal::DeadlineExpired {
                    before_dispatch: false,
                },
                "Sent, but the job ran out of time before the browser confirmed.",
            ),
        ];
        for (refusal, expected) in summaries {
            assert_eq!(refusal.summary(), expected);
        }
        assert_eq!(
            Refusal::InvalidRequest.next_steps(),
            vec!["Match the call to the advertised schema; the invalid_input detail states exactly what to change."]
        );
        assert_eq!(
            Refusal::DeadlineBeforeStart.next_steps(),
            vec!["Repeat the call when the current Ghostlight action has finished."]
        );
        assert_eq!(
            Refusal::IncompatibleReceipt.next_steps(),
            vec![
                "Reload or update the Ghostlight extension in that browser, then repeat the call."
            ]
        );
        assert_eq!(
            Refusal::FilesUnreadable.next_steps(),
            vec!["Confirm each path is an existing file within Ghostlight's upload limits."]
        );
        assert_eq!(
            Refusal::CaptureTooLarge.next_steps(),
            vec!["Capture the viewport or a smaller region instead of the full page."]
        );
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
        assert_eq!(
            Refusal::EffectUnknown.next_steps(),
            vec![
                "If a JavaScript dialog may be open on the page, handle it with browser_dialog; handling checks the page directly.",
                "Then observe the page with browser_read or browser_inspect to learn what happened.",
            ]
        );

        let refusal = Refusal::AuthorityBlocked {
            reason: BlockedReason::ProtectedHost,
            host: Some("localhost".into()),
        };
        assert_eq!(
            refusal.summary(),
            "Blocked: localhost is protected and is never automated."
        );
        assert_eq!(refusal.observed().host.as_deref(), Some("localhost"));
        assert!(Refusal::InvalidRequest.observed().host.is_none());
    }

    /// A primitive adapter refusal speaks the browser's own reason and never masquerades as a
    /// disconnection.
    #[test]
    fn primitive_refusal_carries_the_browser_reason_without_claiming_a_disconnection() {
        let refusal = Refusal::BrowserPrimitive {
            detail: "target is not visible for focus".into(),
        };
        let summary = refusal.summary();
        assert!(summary.contains("target is not visible for focus"));
        assert!(!summary.to_lowercase().contains("disconnected"));
        assert_eq!(
            refusal.next_steps(),
            vec!["Read the browser's stated reason, adjust the call or the page, then repeat."]
        );
    }

    /// Every workspace reason names its own way back, or deliberately none.
    #[test]
    fn workspace_reasons_teach_their_own_recovery() {
        assert_eq!(
            WorkspaceReason::TabUnavailable.next_steps(),
            vec!["Call browser_tabs with action list to obtain current tab handles."]
        );
        assert_eq!(
            WorkspaceReason::StaleTarget.next_steps(),
            vec!["Call browser_inspect or browser_find to obtain current target handles."]
        );
        assert_eq!(
            WorkspaceReason::StaleView.next_steps(),
            vec!["Call browser_screenshot to obtain a current view handle."]
        );
        assert_eq!(
            WorkspaceReason::TabHeld.next_steps(),
            vec!["Resume browser work from the Ghostlight window or tray, then repeat the call."]
        );
        assert_eq!(
            WorkspaceReason::WorkspaceBusy.next_steps(),
            vec!["Wait for the active Ghostlight invocation to finish."]
        );
        assert_eq!(
            WorkspaceReason::OwnershipMismatch.next_steps(),
            vec!["Collect fresh handles from this session, then continue with those."]
        );
        assert_eq!(
            WorkspaceReason::WorkspaceClosed.next_steps(),
            vec!["Start over; the next call opens a fresh session and old handles will not work."]
        );
    }

    /// Outcome guidance leads with the recovery action and stays truthful about partial work.
    #[test]
    fn outcome_next_steps_teach_the_fix() {
        assert_eq!(
            Outcome::SelectorUnresolved { matched: 0 }.next_steps(),
            vec!["Use browser_find with text visible on the page, inspect for fresh handles, or narrow the selector with role and exact."]
        );
        assert_eq!(
            Outcome::FlowRan {
                completed: 2,
                total: 5,
                stopped: true,
            }
            .next_steps(),
            vec!["Use the per-step results to find what went wrong, fix that step, and run the flow again."]
        );
        assert!(Outcome::FlowRan {
            completed: 5,
            total: 5,
            stopped: false,
        }
        .next_steps()
        .is_empty());
        assert_eq!(
            Outcome::SequenceRan {
                completed: 2,
                total: 5,
            }
            .next_steps(),
            vec!["Find the step that stopped the sequence in the results, fix it, and run the sequence again."]
        );
        assert_eq!(
            Outcome::Waited {
                condition: "text_present".into(),
                elapsed_ms: 8_000,
                satisfied: false,
                host: None,
            }
            .next_steps(),
            vec![
                "Read or inspect the page to see its current state before choosing another action."
            ]
        );
        assert_eq!(
            Outcome::DiagnosticsRead {
                count: 0,
                capture_started: true,
                problems_only: true,
                host: None,
            }
            .next_steps(),
            vec!["Reproduce the problem or reload the page, then call browser_diagnose again."]
        );
        assert_eq!(
            Outcome::DocumentInspected {
                nodes: 200,
                truncated: true,
                compared: false,
            }
            .next_steps(),
            vec!["Narrow the subtree root or depth to capture the rest."]
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
