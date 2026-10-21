//! Ghostlight-authored outcome language and its content-minimized measurement projection.

use serde::{Deserialize, Serialize};

use super::area::{LeftOut, LeftOutKey};
use super::choices::ScrollDirection;
use super::field_echo::{self, HeldField};
use super::field_problem::{self, ControlRef, RejectedField};
use super::page_refusal::{self, PageGesture, PageRefusal};
use super::phrase::{article, counted, counted_or_none, one_line_bounded};
use super::wait::WaitCondition;
use super::{adapter, deployment, setup, MAX_INSPECT_ITEMS};
use crate::governance::ReasonCode;
use crate::intake::session::IntakeChannel;
use crate::workspace::{StaleReason, WorkspaceError};

/// The most characters a result's summary keeps. Longer summaries are cut at this bound, so a
/// summary that lists many things measures itself against it.
pub const SUMMARY_MAX_CHARS: usize = 500;

/// A summary followed by one closed note, within `SUMMARY_MAX_CHARS`. The summary is cut, never
/// the note, so the note survives a summary that already fills the bound.
#[must_use]
pub fn with_note(summary: &str, note: &str) -> String {
    if note.is_empty() {
        return summary.into();
    }
    let room = SUMMARY_MAX_CHARS.saturating_sub(note.chars().count() + 1);
    let kept: String = summary.chars().take(room).collect();
    format!("{kept} {note}")
}

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

/// Why Ghostlight's own control page is refused, after the address that was asked for.
///
/// The rule covers one origin, not the machine: a model testing a local server must not conclude
/// that every loopback address is closed, and no policy lifts this refusal (ADR-0199 amendment of
/// 2026-10-15).
const OWN_CONTROL_PAGE_REFUSED: &str =
    "Ghostlight's own control page and is never automated. Other ports on this computer are not affected.";

/// The boundary line for Ghostlight's own control page, authored once.
///
/// The compiled authority ends its ceilings with it, so the person's Policy page, the watch view
/// and `policy_explain` say the same thing.
pub const OWN_CONTROL_PAGE_CEILING: &str =
    "Ghostlight's own control page on this computer is never automated.";

/// The one way to give an empty session its first page, shared by every result that finds none.
const OPEN_FIRST_TAB: &str = "Call browser_navigate with url to open a page.";

fn browser_choices(browsers: &[String]) -> String {
    match browsers {
        [] => "a supported Chromium".into(),
        [browser] => format!("a {browser}"),
        [first, second] => format!("a {first} or {second}"),
        [head @ .., last] => format!("a {}, or {last}", head.join(", ")),
    }
}

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
    /// An always-open list of options, such as a multi-select.
    ListBox,
    Slider,
    /// A numeric input, which the page reports as a spin button.
    NumberField,
    /// A file input. The page reports it as a text field; only an upload, which knows it attached
    /// files there, names it so.
    FileField,
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
            "combobox" => Self::Combobox,
            "listbox" => Self::ListBox,
            "slider" => Self::Slider,
            "spinbutton" => Self::NumberField,
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
            Self::ListBox => "list box",
            Self::Slider => "slider",
            Self::NumberField => "number field",
            Self::FileField => "file field",
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

/// Maximum number of visible characters retained from one JavaScript dialog's message.
const DIALOG_MESSAGE_MAX_CHARS: usize = 500;

/// The bounded human identity of the element an action actually used.
///
/// The role is always narrowed to Ghostlight's closed vocabulary. The optional name is page
/// content retained only when the effective governance snapshot permits it. `rebound` says the
/// action reached the element through a handle that followed its control across a re-render the
/// page proved (ADR-0198 Decision 17).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSubject {
    role: TargetRole,
    label: Option<String>,
    rebound: bool,
}

impl ActionSubject {
    /// Narrow and normalize one browser-observed physical subject.
    #[must_use]
    pub fn from_page(role: &str, name: &str, preserve_name: bool) -> Self {
        Self::with_role(TargetRole::classify(role), name, preserve_name)
    }

    /// Construct the safe unnamed fallback for an already narrowed semantic target.
    #[must_use]
    pub const fn unnamed(role: TargetRole) -> Self {
        Self {
            role,
            label: None,
            rebound: false,
        }
    }

    /// Say whether the action reached this element by following its handle through a re-render.
    #[must_use]
    pub const fn rebound(mut self, rebound: bool) -> Self {
        self.rebound = rebound;
        self
    }

    /// Add that an earlier call of the same operation followed this element's handle. Never
    /// clears a fact the action already holds.
    #[must_use]
    pub const fn also_rebound(mut self, rebound: bool) -> Self {
        self.rebound = self.rebound || rebound;
        self
    }

    /// Name an already narrowed role. The name is kept only when the governed snapshot
    /// preserves target names.
    #[must_use]
    pub fn with_role(role: TargetRole, name: &str, preserve_name: bool) -> Self {
        Self {
            role,
            label: if preserve_name {
                one_line_bounded(name, TARGET_LABEL_MAX_CHARS)
            } else {
                None
            },
            rebound: false,
        }
    }

    /// The same element as the file input an upload attached its files to, whatever role the page
    /// reported for it.
    #[must_use]
    pub const fn file_field(mut self) -> Self {
        self.role = TargetRole::FileField;
        self
    }

    /// The closed role of the element.
    pub(super) const fn role(&self) -> TargetRole {
        self.role
    }

    /// The bounded name, when the snapshot lets it be kept.
    pub(super) fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Whether both name one control, by role and name. Whether a handle followed it to get
    /// there does not make it another control.
    fn names_same_control(&self, other: &Self) -> bool {
        self.role == other.role && self.label == other.label
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
/// It groups the governance reason codes by the sentence each reads as ([`BlockedReason::of`]).
/// Durable history keeps this group, never the code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedReason {
    /// Saving history is unavailable while policy requires it.
    AuditUnavailable,
    /// The host was not granted by every authority layer.
    Host,
    /// The host or scheme is independently protected.
    ProtectedHost,
    /// The address is Ghostlight's own control page on this computer.
    OwnWorkbench,
    /// The capability was not granted by every authority layer.
    Capability,
    /// Model-driven tab closure was refused.
    TabClose,
    /// Binary screenshot permission was refused.
    Screenshot,
    /// Configured policy could not be validated.
    InvalidAuthority,
    /// A runtime control is holding browser work.
    Hold,
    /// A runtime control ended the session.
    SessionEnded,
    /// Authority refused for a reason with no more specific wording.
    Unspecified,
}

#[cfg(test)]
impl BlockedReason {
    /// One governance reason code that reads as this group, for tests that build refusals.
    pub(crate) const fn code(self) -> ReasonCode {
        match self {
            Self::Host => ReasonCode::HostDenied,
            Self::ProtectedHost => ReasonCode::ProtectedHost,
            Self::OwnWorkbench => ReasonCode::OwnWorkbench,
            Self::Capability => ReasonCode::CapabilityDenied,
            Self::TabClose => ReasonCode::TabCloseDenied,
            Self::Screenshot => ReasonCode::ScreenshotDenied,
            Self::InvalidAuthority => ReasonCode::InvalidAuthority,
            Self::Hold => ReasonCode::RuntimeHold,
            Self::SessionEnded => ReasonCode::SessionEnded,
            Self::AuditUnavailable => ReasonCode::AuditUnavailable,
            Self::Unspecified => ReasonCode::InvalidRequest,
        }
    }
}

#[cfg(test)]
impl CheckFailure {
    /// The reason code of the decision a result with this check failure reports, for tests.
    pub(crate) const fn decision_code(self) -> ReasonCode {
        match self {
            Self::Blocked(reason) => reason.code(),
            _ => ReasonCode::Permitted,
        }
    }
}

impl BlockedReason {
    /// The sentence group a governance reason code reads as.
    #[must_use]
    pub const fn of(code: ReasonCode) -> Self {
        match code {
            ReasonCode::HostDenied => Self::Host,
            ReasonCode::ProtectedHost => Self::ProtectedHost,
            ReasonCode::OwnWorkbench => Self::OwnWorkbench,
            ReasonCode::CapabilityDenied => Self::Capability,
            ReasonCode::TabCloseDenied => Self::TabClose,
            ReasonCode::ScreenshotDenied => Self::Screenshot,
            ReasonCode::InvalidAuthority => Self::InvalidAuthority,
            ReasonCode::RuntimeHold => Self::Hold,
            ReasonCode::SessionEnded => Self::SessionEnded,
            ReasonCode::AuditUnavailable => Self::AuditUnavailable,
            ReasonCode::Permitted | ReasonCode::InvalidRequest | ReasonCode::ChannelDenied => {
                Self::Unspecified
            }
        }
    }
}

/// Whose policy layer refused a request, in the words its refusal names it by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyAuthor {
    /// The organization managing this computer.
    Organization,
    /// The person using this computer.
    User,
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

/// The kind of JavaScript dialog a page has open, narrowed to the browser's own closed names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogKind {
    /// `alert`: a message with one button.
    Alert,
    /// `confirm`: a question to accept or dismiss.
    Confirm,
    /// `prompt`: a question that takes typed text.
    Prompt,
    /// `beforeunload`: the leave-this-page question.
    BeforeUnload,
    /// A kind the browser reported that this build does not name.
    Other,
}

impl DialogKind {
    /// Narrow the kind the browser reported.
    #[must_use]
    pub fn classify(reported: &str) -> Self {
        match reported {
            "alert" => Self::Alert,
            "confirm" => Self::Confirm,
            "prompt" => Self::Prompt,
            "beforeunload" => Self::BeforeUnload,
            _ => Self::Other,
        }
    }

    /// The name the `dialog_opened` fact carries.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Confirm => "confirm",
            Self::Prompt => "prompt",
            Self::BeforeUnload => "beforeunload",
            Self::Other => "other",
        }
    }

    /// The sentence that says this dialog is open.
    const fn sentence(self) -> &'static str {
        match self {
            Self::Alert => "An alert dialog is now open.",
            Self::Confirm => "A confirm dialog is now open.",
            Self::Prompt => "A prompt dialog is now open.",
            Self::BeforeUnload => "A leave-page dialog is now open.",
            Self::Other => "A JavaScript dialog is now open.",
        }
    }
}

/// The step that clears a dialog once its status has been read.
const HANDLE_DIALOG_FIRST: &str =
    "Call browser_dialog with action accept or dismiss (or respond with text for a prompt) before continuing.";

/// The step that answers a prompt once its status has been read. Every 2.0 extension answers an
/// accept with the text in the prompt's field, as OK does, whether or not it reports that text.
const ANSWER_PROMPT: &str = "Call browser_dialog with action accept to answer with the text in its field, as OK does, respond with your own text, or dismiss, before continuing.";

/// What the status of a prompt says about the text in its field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromptField {
    /// The text the field holds, empty when the field is empty, as the extension reported it.
    Holds(String),
    /// The extension did not say what the field holds: one older than 2.0.1 does not report it.
    Unreported,
}

/// The step for a dialog the agent has not read yet: its status first, because accept answers a
/// prompt with its default text, as OK does, and only the message says whether that is wanted.
const READ_DIALOG_FIRST: &str = "Call browser_dialog with action status to read it, then accept, dismiss, or respond with text for a prompt.";

/// The step after a script whose value is `{}` and that was sent awaited. A promise has resolved
/// by then, so what remains is a value JSON cannot show.
const EMPTY_OBJECT_AWAITED: &str = "The value {} is also how a DOM node, a Map or a class instance reads as JSON. Return plain fields, such as el.textContent or Array.from(map), to read them.";

/// The step after a script whose value is `{}` and that was sent as written. A promise the script
/// ends in was not awaited, and says nothing in JSON.
const EMPTY_OBJECT_NOT_AWAITED: &str = "The value {} is also how an unawaited promise, a DOM node or a Map reads as JSON. To wait for a promise, end the script with return await <promise>.";

/// Whether a script's value was `{}`, which several different things read as.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmptyObject {
    /// The value was something else, or was cut.
    Absent,
    /// The value was exactly `{}` and the script was sent awaited.
    Awaited,
    /// The value was exactly `{}` and the script was sent as written.
    NotAwaited,
}

/// What one completed browser action did in Ghostlight's product language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The workspace's bound tabs were listed, read live from the browser.
    TabsListed { count: usize },
    /// The authority in force was explained to the model (ADR-0136).
    PolicyExplained { capabilities: usize, layers: usize },
    /// A semantic selector matched zero or several visible controls. `more` says the page held
    /// more matches than the listing kept, so `matched` is a lower bound.
    SelectorUnresolved { matched: usize, more: bool },
    /// One governed flow finished with an explicit progress account.
    CompositionRan(super::composition::CompositionProgress),
    /// Invalid child inputs prevented entering the ordinary executor.
    StepNotStarted,

    /// One bounded document tree was returned to the caller and recorded as a snapshot.
    ///
    /// `differences` is present only when the previous snapshot covered the same root and depth,
    /// because a tree of another scope differs without the page changing.
    DocumentInspected {
        nodes: usize,
        truncated: bool,
        differences: Option<usize>,
    },
    /// One controlled tab was brought into view.
    TabActivated { host: Option<String> },
    /// A requested page was opened.
    PageOpened { host: Option<String> },
    /// A same-host tab Ghostlight opened and a released session of the same client left open was
    /// reused for the requested page (ADR-0137, ADR-0196 Decision 9).
    PageReused { host: Option<String> },
    /// A manually closed page was restored in a new controlled tab.
    PageRecovered { host: Option<String> },
    /// The requested controlled tab was already absent.
    TabAlreadyClosed,
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
        /// What the listing left out, by key (ADR-0198 Decision 20).
        left_out: LeftOut,
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
        direction: ScrollDirection,
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
        /// What the page holds in the fields worth reading back. Audit never holds it.
        held: Vec<HeldField>,
        /// Whether a field or the submit control was reached by a handle that followed its
        /// control through a re-render (ADR-0198 Decision 17).
        rebound: bool,
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
    ScriptEvaluated {
        host: Option<String>,
        empty_object: EmptyObject,
    },
    /// An explicit observable condition was awaited.
    Waited {
        condition: WaitCondition,
        elapsed_ms: u64,
        satisfied: bool,
        host: Option<String>,
    },
    /// A browser dialog was resolved.
    DialogHandled { accepted: bool },
    /// Current JavaScript-dialog state was observed, with the dialog's message when the browser
    /// reported one.
    DialogObserved {
        present: bool,
        message: Option<String>,
        /// `Some` for a prompt, with what its field holds when the extension reported it.
        prompt: Option<PromptField>,
    },
    /// Bounded console and network diagnostics were read.
    DiagnosticsRead {
        count: usize,
        capture_started: bool,
        problems_only: bool,
        host: Option<String>,
    },
    /// A memory-only recording began.
    RecordingStarted { host: Option<String> },
    /// Memory-only recording state was read. `active` is whether capture is still running; a
    /// recording that stopped keeps its frames for a save.
    RecordingObserved {
        frames: usize,
        duration_ms: u64,
        active: bool,
    },
    /// An active memory-only recording stopped.
    RecordingStopped { duration_ms: u64 },
    /// Recording ended when its admitted document scope could no longer be verified.
    ///
    /// The adapter stops for any frame, not only the top page: a navigation or reload in any
    /// frame, a frame added or removed, or a document set it cannot list. The sentence says
    /// exactly that much and never claims the tab itself moved.
    RecordingBoundary { duration_ms: u64 },
    /// A recording was encoded as an animated GIF and delivered.
    RecordingSaved {
        /// How long the replay plays.
        duration_ms: u64,
        delivery: SavedTo,
        /// Whether describing the page target the replay was attached to followed its handle
        /// through a re-render (ADR-0198 Decision 17). Only a page target has a handle.
        rebound: bool,
    },
    /// Captured recording bytes were erased.
    RecordingDiscarded,
    /// An action completed and left a JavaScript dialog open on its page.
    ///
    /// The action happened, so its own sentence stands; the dialog is the page's answer to it and
    /// blocks the page until it is handled.
    DialogOpened {
        /// The completed action, in its own words.
        action: Box<Outcome>,
        /// The kind of dialog it left open.
        dialog: DialogKind,
    },
}

impl Outcome {
    /// Render the bounded Ghostlight-authored account of what happened.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::DocumentInspected {
                nodes,
                truncated: _,
                differences,
            } => {
                let returned = format!(
                    "Returned the document outline: {}",
                    counted(*nodes, "node", "nodes")
                );
                match differences {
                    None => format!("{returned}."),
                    Some(0) => format!("{returned}, matching the previous outline."),
                    Some(differences) => format!(
                        "{returned}, {} from the previous outline.",
                        counted(*differences, "difference", "differences")
                    ),
                }
            }
            Self::CompositionRan(progress) => progress.summary(),
            Self::StepNotStarted => "This step could not start.".into(),

            Self::SelectorUnresolved { matched, more } => {
                if *matched == 0 {
                    "No visible control matched the semantic selector in the inspected content."
                        .into()
                } else if *more {
                    format!(
                        "At least {matched} visible controls matched the semantic selector; none was chosen."
                    )
                } else {
                    format!(
                        "{matched} visible controls matched the semantic selector; none was chosen."
                    )
                }
            }
            Self::TabsListed { count } => {
                format!("Listed {} in this session.", counted(*count, "tab", "tabs"))
            }
            Self::PolicyExplained {
                capabilities,
                layers,
            } => format!(
                "Explained what agents may do: {}, {}.",
                counted(*capabilities, "capability area", "capability areas"),
                counted_or_none(*layers, "policy layer", "policy layers")
            ),
            Self::TabActivated { host } => {
                format!("Brought {} into view.", place(host, "the controlled tab"))
            }
            Self::PageOpened { host } => {
                format!("Opened {}.", place(host, "the requested page"))
            }
            Self::PageReused { host } => match host {
                Some(host) => format!("Reused the {host} tab."),
                None => "Reused the existing tab.".to_string(),
            },
            Self::PageRecovered { host } => page_recovered_summary(host.as_deref()),
            Self::TabAlreadyClosed => tab_already_closed_summary(),
            Self::PageNavigated { host } => {
                format!("Navigated to {}.", place(host, "the requested page"))
            }
            Self::HistoryTraversed { direction, host } => {
                format!("Went {direction} to {}.", place(host, "the previous page"))
            }
            Self::PageReloaded { host } => {
                format!("Reloaded {}.", on_page(host))
            }
            Self::TabClosed => "Closed the controlled tab.".into(),
            Self::TextRead { words, host } => format!(
                "Read {} from {}.",
                counted(*words, "word", "words"),
                on_page(host)
            ),
            Self::TargetsListed {
                noun: TargetNoun::Match,
                count,
                host,
                ..
            } => format!(
                "Found {} on {}.",
                counted(*count, "match", "matches"),
                on_page(host)
            ),
            Self::TargetsListed {
                noun: TargetNoun::Control,
                count,
                host,
                ..
            } => format!(
                "Found {} on {}.",
                counted(*count, "control", "controls"),
                on_page(host)
            ),
            Self::TargetsListed {
                noun: TargetNoun::Item,
                count,
                host,
                ..
            } => format!(
                "Found {} on {}.",
                counted(*count, "item", "items"),
                on_page(host)
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
                    on_page(host)
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
                    on_page(host)
                )
            }
            Self::PointClicked {
                host,
                x,
                y,
                subject: None,
            } => {
                format!("Clicked at {x},{y} on {}.", on_page(host))
            }
            Self::PageScrolled { host, direction } => {
                format!("Scrolled {} on {}.", direction.as_str(), on_page(host))
            }
            Self::TargetRevealed { host, subject } => format!(
                "Scrolled {} into view on {}.",
                subject.noun_phrase(),
                on_page(host)
            ),
            Self::ZoomSet { percent, host } => {
                format!("Set zoom to {percent}% on {}.", on_page(host))
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
                on_page(host)
            ),
            Self::Hovered {
                host,
                subject: None,
            } => format!("Hovered a point on {}.", on_page(host)),
            Self::FormFilled {
                fields,
                submitted,
                host,
                held,
                ..
            } => {
                let readings = field_echo::readings(held);
                let mut summary = format!(
                    "Filled {} on {}",
                    counted(*fields, "field", "fields"),
                    on_page(host)
                );
                if *submitted {
                    summary.push_str(" and submitted the form");
                }
                if let Some(after) = readings.after_clause() {
                    summary.push_str(": ");
                    summary.push_str(after);
                }
                summary.push('.');
                summary.push_str(&readings.trailing());
                summary
            }
            Self::TextTyped {
                host,
                subject,
                characters,
            } => format!(
                "Typed {} into {} on {}.",
                counted(*characters, "character", "characters"),
                subject.noun_phrase(),
                on_page(host)
            ),
            Self::KeyboardSent {
                host,
                key: Some(key),
                subject,
            } => match subject {
                Some(subject) => format!(
                    "Pressed {key} in {} on {}.",
                    subject.noun_phrase(),
                    on_page(host)
                ),
                None => format!("Pressed {key} on {}.", on_page(host)),
            },
            Self::KeyboardSent {
                host,
                key: None,
                subject,
            } => match subject {
                Some(subject) => format!(
                    "Pressed a key in {} on {}.",
                    subject.noun_phrase(),
                    on_page(host)
                ),
                None => format!("Pressed a key on {}.", on_page(host)),
            },
            // Both ends landing on one named control is a drag inside it, such as a slider or a
            // canvas, and "onto itself" would misdescribe it.
            Self::Dragged {
                host,
                source: Some(source),
                destination: Some(destination),
            } if source.names_same_control(destination) && source.label.is_some() => format!(
                "Dragged across {} on {}.",
                source.noun_phrase(),
                on_page(host)
            ),
            Self::Dragged {
                host,
                source: Some(source),
                destination: Some(destination),
            } => format!(
                "Dragged {} onto {} on {}.",
                source.noun_phrase(),
                destination.noun_phrase(),
                on_page(host)
            ),
            Self::Dragged { host, .. } => format!(
                "Dragged one point to another on {}.",
                on_page(host)
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
                    on_page(host)
                ),
                None => format!(
                    "Uploaded {} to {}.",
                    counted(*count, "file", "files"),
                    on_page(host)
                ),
            },
            Self::ScriptEvaluated { host, .. } => {
                format!("Executed JavaScript on {}.", on_page(host))
            }
            Self::Waited {
                condition,
                elapsed_ms,
                satisfied,
                host,
            } => waited(*condition, *elapsed_ms, *satisfied, host),
            Self::DialogHandled { accepted: true } => "Accepted the browser dialog.".into(),
            Self::DialogHandled { accepted: false } => "Dismissed the browser dialog.".into(),
            Self::DialogObserved {
                present: true,
                message,
                prompt,
            } => dialog_observed(message.as_deref(), prompt.as_ref()),
            Self::DialogObserved { present: false, .. } => {
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
                on_page(host)
            ),
            Self::RecordingStarted { host } => {
                format!("Started recording {}.", on_page(host))
            }
            Self::RecordingObserved {
                frames,
                duration_ms,
                active: true,
            } => format!(
                "Recording for {}, {} so far.",
                spanned(*duration_ms),
                counted(*frames, "frame", "frames")
            ),
            Self::RecordingObserved {
                frames,
                duration_ms,
                active: false,
            } => format!(
                "A stopped recording of {} with {} is ready to save.",
                spanned(*duration_ms),
                counted(*frames, "frame", "frames")
            ),
            Self::RecordingStopped { duration_ms } => {
                format!("Stopped recording after {}.", spanned(*duration_ms))
            }
            Self::RecordingBoundary { duration_ms } => format!(
                "Recording stopped after {} because the page, or a frame inside it, loaded new content or went away. The captured portion is available.",
                spanned(*duration_ms)
            ),
            Self::RecordingSaved {
                duration_ms,
                delivery,
                ..
            } => {
                let replay = format!("a replay of {} of page changes", spanned(*duration_ms));
                match delivery {
                    SavedTo::Client => format!("Saved {replay}."),
                    SavedTo::PageTarget => format!("Attached {replay} to the page."),
                    SavedTo::Download => format!("Downloaded {replay}."),
                }
            }
            Self::RecordingDiscarded => "Erased the recording.".into(),
            Self::DialogOpened { action, dialog } => {
                format!("{} {}", action.summary(), dialog.sentence())
            }
        }
    }

    /// Whether this completed action can leave a dialog open on its page.
    ///
    /// Reading, listing, waiting and the dialog and recording tools never open one; the actions
    /// that drive the page do.
    #[must_use]
    pub const fn can_open_dialog(&self) -> bool {
        matches!(
            self,
            Self::TargetClicked { .. }
                | Self::PointClicked { .. }
                | Self::PageScrolled { .. }
                | Self::TargetRevealed { .. }
                | Self::Hovered { .. }
                | Self::FormFilled { .. }
                | Self::TextTyped { .. }
                | Self::KeyboardSent { .. }
                | Self::Dragged { .. }
                | Self::FilesUploaded { .. }
                | Self::ScriptEvaluated { .. }
                | Self::PageNavigated { .. }
                | Self::PageReloaded { .. }
                | Self::HistoryTraversed { .. }
        )
    }

    /// Say that this action left a dialog open.
    #[must_use]
    pub fn with_open_dialog(self, dialog: DialogKind) -> Self {
        Self::DialogOpened {
            action: Box::new(self),
            dialog,
        }
    }

    /// Render at most two safe contextual recovery actions for this outcome.
    #[must_use]
    pub fn next_steps(&self) -> Vec<String> {
        match self {
            Self::TabsListed { count: 0 } => vec![OPEN_FIRST_TAB.into()],
            Self::TabAlreadyClosed => vec![
                "Call browser_tabs with action list to see remaining open tabs.".into(),
            ],
            Self::TargetsListed { count: 0, .. } => vec![
                "Try a different search term, scroll to reveal content, or use browser_inspect to view available page controls.".into(),
            ],
            Self::TargetsListed {
                noun, left_out, ..
            } => left_out_step(*noun, left_out).into_iter().collect(),
            Self::TextRead { words: 0, .. } => vec![
                "Wait for the page to finish rendering with browser_wait, or inspect controls with browser_inspect.".into(),
            ],
            Self::DocumentInspected { nodes: 0, .. } => vec![
                "Wait for the page to finish loading with browser_wait, or inspect without a root selector.".into(),
            ],
            Self::DocumentInspected {
                truncated: true, ..
            } => vec!["Narrow the subtree root or depth to capture the rest.".into()],
            Self::SelectorUnresolved { .. } => vec![
                "Use browser_find with text visible on the page, inspect for fresh handles, or narrow the selector with role and exact.".into(),
            ],
            Self::CompositionRan(progress) => progress.next_steps(),
            Self::Waited {
                satisfied: false, ..
            } => vec![
                "Read or inspect the page to see its current state before choosing another action."
                    .into(),
            ],
            Self::DialogObserved {
                present: true,
                prompt: Some(_),
                ..
            } => vec![ANSWER_PROMPT.into()],
            Self::DialogObserved { present: true, .. } => vec![HANDLE_DIALOG_FIRST.into()],
            Self::ScriptEvaluated {
                empty_object: EmptyObject::Awaited,
                ..
            } => vec![EMPTY_OBJECT_AWAITED.into()],
            Self::ScriptEvaluated {
                empty_object: EmptyObject::NotAwaited,
                ..
            } => vec![EMPTY_OBJECT_NOT_AWAITED.into()],
            Self::DialogOpened { action, .. } => {
                let mut steps = vec![READ_DIALOG_FIRST.to_owned()];
                steps.extend(action.next_steps());
                steps.truncate(2);
                steps
            }
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

    /// Whether the action used a handle that followed its control through a re-render.
    ///
    /// Only the closed fact is kept. The handle, the row and the link stay in the page.
    #[must_use]
    pub fn rebound(&self) -> bool {
        match self {
            Self::TargetClicked { subject, .. }
            | Self::TargetRevealed { subject, .. }
            | Self::TextTyped { subject, .. } => subject.rebound,
            Self::FormFilled { rebound, .. } | Self::RecordingSaved { rebound, .. } => *rebound,
            Self::PointClicked { subject, .. }
            | Self::Hovered { subject, .. }
            | Self::KeyboardSent { subject, .. }
            | Self::FilesUploaded { subject, .. } => {
                subject.as_ref().is_some_and(|subject| subject.rebound)
            }
            Self::Dragged {
                source,
                destination,
                ..
            } => [source, destination]
                .into_iter()
                .flatten()
                .any(|subject| subject.rebound),
            Self::DialogOpened { action, .. } => action.rebound(),
            _ => false,
        }
    }

    /// Project the measurements named by the sentence, and whether a handle followed its control,
    /// into the audit vocabulary.
    #[must_use]
    pub fn observed(&self) -> Observed {
        Observed {
            rebound: self.rebound(),
            ..self.measurements()
        }
    }

    /// The measurements the sentence names.
    fn measurements(&self) -> Observed {
        match self {
            Self::TabsListed { count } => Observed {
                count: measured(*count),
                ..Observed::default()
            },
            Self::PolicyExplained {
                capabilities: count,
                ..
            } => Observed {
                count: measured(*count),
                ..Observed::default()
            },
            Self::DocumentInspected { nodes, .. } => Observed {
                count: measured(*nodes),
                ..Observed::default()
            },
            Self::CompositionRan(progress) => Observed {
                count: measured(progress.counts.succeeded),
                ..Observed::default()
            },
            Self::SelectorUnresolved { matched: count, .. } => Observed {
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
            | Self::RecordingBoundary {
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
            | Self::PageReused { host }
            | Self::PageRecovered { host }
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
            | Self::ScriptEvaluated { host, .. }
            | Self::RecordingStarted { host } => Observed {
                host: host.clone(),
                ..Observed::default()
            },
            Self::DialogOpened { action, .. } => action.measurements(),
            Self::TabClosed
            | Self::TabAlreadyClosed
            | Self::DialogHandled { .. }
            | Self::DialogObserved { .. }
            | Self::StepNotStarted
            | Self::RecordingDiscarded => Observed::default(),
        }
    }
}

/// Closed product-language reason for a browser-readiness recovery failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// Another Ghostlight installation owns the native messaging registration.
    OwnedElsewhere,
    /// The selected adapter did not arrive within the bounded wait.
    HandshakeTimeout,
}

impl BrowserRecoveryReason {
    /// The reason's model code.
    #[must_use]
    pub const fn fact(self) -> &'static str {
        match self {
            Self::BrowserAbsent => "browser_absent",
            Self::LaunchFailed => "browser_launch_failed",
            Self::SandboxedPackage => "browser_sandboxed",
            Self::ExtensionAbsent => "browser_extension_absent",
            Self::NativeHostUnavailable => "native_host_unavailable",
            Self::OwnedElsewhere => "native_host_owned_elsewhere",
            Self::HandshakeTimeout => "browser_handshake_timeout",
        }
    }
}

/// Closed product-language reason the browser a call needed could not take it. In every case
/// nothing was sent, and the call waited first when there was evidence the browser was returning
/// (ADR-0196 Decision 12).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserUnavailableReason {
    /// The browser still holds a connection but did not answer within the wait.
    NotAnswering,
    /// The browser is not connected, and no other browser is.
    NotConnected,
    /// The browser is not connected, but a different browser is.
    WrongProfile,
    /// The browser connection stayed busy with other work.
    Busy,
}

impl BrowserUnavailableReason {
    /// The reason's model code.
    #[must_use]
    pub const fn fact(self) -> &'static str {
        match self {
            Self::NotAnswering => "browser_not_answering",
            Self::NotConnected => "browser_disconnected",
            Self::WrongProfile => "browser_wrong_profile",
            Self::Busy => "browser_busy",
        }
    }
}

/// Why the condition a caller asked Ghostlight to check after an applied action was not
/// confirmed.
///
/// The action already happened in every case. The cause only says why the check that was meant
/// to follow it did not confirm the page state the caller expected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckFailure {
    /// The page was checked, and the expected condition did not hold.
    Unmet,
    /// A human control or configured authority refused the check.
    Blocked(BlockedReason),
    /// The caller cancelled before the check finished.
    Cancelled,
    /// The call ran out of time before the check finished.
    DeadlineExpired,
    /// The browser disconnected before the check finished.
    Disconnected,
    /// Ghostlight could not reach a browser to run the check.
    BrowserUnavailable,
    /// Access to the page could not be verified for the check.
    DocumentUnavailable,
    /// The browser could not run the check or answered outside the contract.
    BrowserRefused,
}

/// The one recovery step when the extension and the service disagree about the bridge contract:
/// only the person can update the extension, and the way to do it now is the one the extension
/// itself is told ([`adapter::UPDATE_NOW`]).
fn update_extension_then_repeat() -> String {
    format!(
        "Ask the user to update Ghostlight in Browser (to update now, {}), then repeat the call.",
        adapter::UPDATE_NOW
    )
}

/// The one recovery step when the selected browser did not start or its adapter did not arrive.
/// It speaks to the model, which cannot open a browser for the person (ADR-0147 Decision 3).
const ASK_FOR_THAT_BROWSER: &str = "Ask the user to open a window of that browser with the Ghostlight extension installed, then repeat the call.";

/// Whether a refusal that sends the person to the browser also says where the extension comes
/// from.
///
/// While setup is incomplete and no browser profile holds the extension, an agent that only
/// asks the user to open a browser leaves them with nothing to open it for. Once the extension
/// is in some profile, or setup is complete, the refusal is the plain one.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExtensionAdvice {
    /// The extension is installed somewhere or setup is done; the refusal adds nothing.
    #[default]
    Plain,
    /// Setup is incomplete and no profile has the extension, so the refusal names the store
    /// listing and the Ghostlight window.
    AddFromStore,
}

impl ExtensionAdvice {
    /// The advice for the facts the executor read when it built the refusal.
    ///
    /// Both facts are read at the moment of the refusal, and the profile scan is only needed
    /// while setup is incomplete.
    #[must_use]
    pub const fn resolve(setup_complete: bool, extension_in_a_profile: bool) -> Self {
        if setup_complete || extension_in_a_profile {
            Self::Plain
        } else {
            Self::AddFromStore
        }
    }

    /// The one safe next step the advice adds, when it adds one.
    fn step(self) -> Option<String> {
        match self {
            Self::Plain => None,
            Self::AddFromStore => Some(format!(
                "Ask the user to add Ghostlight in Browser from {}, or to open the Ghostlight window on this computer for the setup steps, then repeat the call.",
                setup::STORE_LISTING_URL
            )),
        }
    }
}

/// The one recovery step after an action whose check did not confirm it.
///
/// A repeat may apply the action twice, so the step is to look first.
const CHECK_BEFORE_REPEATING: &str =
    "Check the page with browser_read or browser_inspect before repeating; the action already happened.";

/// Sentence for an applied action whose check never ran.
const CHECK_NOT_RUN: &str = "The expected condition was not checked";

/// Render why an applied action's requested check did not confirm it, then the action itself.
///
/// The failure leads, so a sentence cut short anywhere still says the check fell short. The
/// action sentence always follows, because the action really happened. Pause and Stop keep
/// their fixed directives first (ADR-0126).
fn unconfirmed_summary(action: &Outcome, cause: CheckFailure) -> String {
    let done = action.summary();
    match cause {
        CheckFailure::Unmet => {
            format!("The expected condition did not hold after the action. {done}")
        }
        CheckFailure::Blocked(BlockedReason::Hold) => {
            format!("{HUMAN_PAUSE_DIRECTIVE} {CHECK_NOT_RUN}. {done}")
        }
        CheckFailure::Blocked(BlockedReason::SessionEnded) => {
            format!("{HUMAN_STOP_DIRECTIVE} {CHECK_NOT_RUN}. {done}")
        }
        CheckFailure::Blocked(reason) => {
            format!("{CHECK_NOT_RUN}. {} {done}", blocked(reason, &None, &None))
        }
        CheckFailure::Cancelled => {
            format!("{CHECK_NOT_RUN} because the call was cancelled. {done}")
        }
        CheckFailure::DeadlineExpired => {
            format!("{CHECK_NOT_RUN} because the call ran out of time. {done}")
        }
        CheckFailure::Disconnected => {
            format!("{CHECK_NOT_RUN} because the browser disconnected. {done}")
        }
        CheckFailure::BrowserUnavailable => {
            format!("{CHECK_NOT_RUN} because Ghostlight could not reach the browser. {done}")
        }
        CheckFailure::DocumentUnavailable => {
            format!("{CHECK_NOT_RUN} because access to the page could not be verified. {done}")
        }
        CheckFailure::BrowserRefused => {
            format!("{CHECK_NOT_RUN} because the browser could not run the check. {done}")
        }
    }
}

/// Recovery for a refusal of configured authority, by the sentence group of its code.
fn blocked_next_steps(reason: BlockedReason) -> Vec<String> {
    match reason {
        // Policy is the one thing the caller can ask about. The human controls are pinned
        // step-free by ADR-0126, a storage outage recovers by itself, and an unreadable policy
        // has nothing to explain.
        BlockedReason::Host
        | BlockedReason::ProtectedHost
        | BlockedReason::Capability
        | BlockedReason::TabClose
        | BlockedReason::Screenshot
        | BlockedReason::Unspecified => {
            vec!["Call policy_explain to see what this session may do.".into()]
        }
        // No policy lifts it, and the sentence already says other ports are open.
        BlockedReason::OwnWorkbench
        | BlockedReason::AuditUnavailable
        | BlockedReason::InvalidAuthority
        | BlockedReason::Hold
        | BlockedReason::SessionEnded => vec![],
    }
}

/// Recovery for an applied action whose check did not confirm it.
///
/// Human controls and the history requirement stay step-free, as their own refusals are. A
/// policy refusal points to the policy. Everything else says to look before repeating.
fn unconfirmed_next_steps(cause: CheckFailure) -> Vec<String> {
    match cause {
        CheckFailure::Blocked(
            BlockedReason::Hold
            | BlockedReason::SessionEnded
            | BlockedReason::AuditUnavailable
            | BlockedReason::InvalidAuthority,
        ) => vec![],
        CheckFailure::Blocked(reason) => blocked_next_steps(reason),
        CheckFailure::Unmet
        | CheckFailure::Cancelled
        | CheckFailure::DeadlineExpired
        | CheckFailure::Disconnected
        | CheckFailure::BrowserUnavailable
        | CheckFailure::DocumentUnavailable
        | CheckFailure::BrowserRefused => vec![CHECK_BEFORE_REPEATING.into()],
    }
}

/// Human-only status for an admitted job still waiting for earlier work.
pub const WAITING_FOR_WORK: &str = "Waiting for earlier browser work";

/// Who cancelled a call: the agent that sent it, or the end of its session or of Ghostlight.
///
/// A call's cancellation records its first cause and keeps it, so a later cancel never rewrites
/// who stopped the call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelCause {
    /// The agent cancelled the call (`notifications/cancelled`).
    Client,
    /// The agent's session ended while the call waited or ran: its connection closed.
    SessionEnded,
    /// Ghostlight quit while the call waited or ran, or before it was admitted, for this reason.
    Quit(QuitCause),
}

/// Why Ghostlight was ending when it cancelled a call. It decides whether repeating the call
/// helps: the edge reconnects after a quit, a replacement or a reason this build does not know,
/// and leaves after an uninstall or the end of the person's sign-in session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuitCause {
    /// The person quit Ghostlight. The next call starts it again.
    Quit,
    /// A newer Ghostlight is taking over, and it serves the next call.
    Replace,
    /// Ghostlight is being removed, so nothing answers a repeated call.
    Uninstall,
    /// The person's sign-in session on this computer is ending, so nothing answers a repeated
    /// call.
    SignOut,
    /// No stream of any role was open for the idle limit. The next call starts Ghostlight again.
    Idle,
    /// The quit named a reason this build does not know. The edge reconnects, as after a quit.
    Unknown,
}

impl QuitCause {
    /// Every cause, for tests that cover each.
    pub const ALL: [Self; 6] = [
        Self::Quit,
        Self::Replace,
        Self::Uninstall,
        Self::SignOut,
        Self::Idle,
        Self::Unknown,
    ];

    /// Whether a Ghostlight answers the same call repeated in a moment.
    #[must_use]
    pub const fn repeat_helps(self) -> bool {
        matches!(
            self,
            Self::Quit | Self::Replace | Self::Idle | Self::Unknown
        )
    }
}

/// The step of work that never started because Ghostlight was ending, when a Ghostlight answers
/// the call repeated in a moment.
pub(crate) const REPEAT_IN_A_MOMENT: &str = "Try the call again in a moment.";

/// What kind of refusal the browser answered a job with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimitiveCause {
    /// The browser refused the job in words of its own.
    Refused,
    /// Another debugger, such as the browser's developer tools, holds the tab.
    DebuggerUnavailable,
    /// The browser ended Ghostlight's control of the tab before the job ran.
    SessionLost,
}

impl PrimitiveCause {
    /// The cause's model code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Refused => "browser_primitive_failed",
            Self::DebuggerUnavailable => "debugger_unavailable",
            Self::SessionLost => "session_lost",
        }
    }
}

/// What kept the files of an upload from being prepared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileProblem {
    /// A path could not be opened or read for its size.
    Unavailable,
    /// A path names something that is not a file.
    NotAFile,
    /// One file is over the upload ceiling.
    FileTooLarge,
    /// The files together are over the upload ceiling.
    UploadTooLarge,
    /// A file could not be read to its end.
    ReadFailed,
    /// A file changed while it was read.
    Changed,
    /// A path has no usable file name.
    InvalidName,
    /// An inline file's data is not standard base64.
    NotBase64,
    /// The captured image the call named is stale or belongs to another tab.
    StaleCapture,
}

impl FileProblem {
    /// The problem's model code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "file_unavailable",
            Self::NotAFile => "not_a_file",
            Self::FileTooLarge => "file_too_large",
            Self::UploadTooLarge => "upload_too_large",
            Self::ReadFailed => "file_read_failed",
            Self::Changed => "file_changed",
            Self::InvalidName => "invalid_file_name",
            Self::NotBase64 => "inline_file_not_base64",
            Self::StaleCapture => "capture_stale",
        }
    }
}

/// Why a browser job did not complete in Ghostlight's product language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The bounded admission queue could not accept another request.
    Capacity,
    /// Model-facing input did not match the catalog.
    InvalidRequest,
    /// Cancellation won before workspace admission.
    CancelledBeforeStart { by: CancelCause },
    /// The invocation deadline expired before workspace admission.
    DeadlineBeforeStart,
    /// Configured authority blocked the job.
    ///
    /// The host is present whenever the refused work named one. It is the same deliberate
    /// exception the audit already makes for a governed host: it answers "where was the agent
    /// trying to go", and the identifying detail of a URL lives after it.
    ///
    /// The origin is the address as the caller wrote it, scheme and port included, and never a
    /// path, query or fragment. Only the refusal of Ghostlight's own control page says it,
    /// because the rest of that sentence is about which port is refused. The observed facts and
    /// the audit summary never carry it: they keep the host alone.
    AuthorityBlocked {
        /// The governance reason code, the refusal's model code.
        code: ReasonCode,
        host: Option<String>,
        origin: Option<String>,
    },
    /// A policy's switch turned off the channel the request arrived on, so nothing ran
    /// (ADR-0202 Decision 22a). The connection stays open, so the caller hears why.
    ChannelOff {
        /// The channel the request's connection declared.
        channel: IntakeChannel,
        /// Whose layer turned it off.
        author: PolicyAuthor,
    },
    /// The browser-local physical safety setting refused the action.
    LocalInterlock,
    /// Credential input requires the caller to acknowledge the user's explicit instruction. It
    /// names the control that asked.
    CredentialAuthorization { control: ControlRef },
    /// The browser returned a receipt outside the negotiated contract.
    IncompatibleReceipt,
    /// The browser adapter predates the physical command meaning the service requires.
    BrowserAdapterOutdated,
    /// The job ran out of time.
    DeadlineExpired {
        /// True when the deadline fired before any dispatch could happen.
        before_dispatch: bool,
        /// True when the call's own `timeout_ms` set this deadline and is still below the
        /// maximum, so raising it is advice the decoder will take. A call already at the maximum,
        /// and one whose deadline an edge supplied, never get that advice.
        timeout_adjustable: bool,
    },
    /// The browser answered the job with its own bounded refusal.
    BrowserPrimitive {
        /// The browser's own words; never retained.
        detail: String,
        /// What kind of refusal the browser's answer was.
        cause: PrimitiveCause,
    },
    /// The page refused the call's control before acting on it, so nothing happened.
    PageRefused {
        /// Why the page refused the control.
        refusal: PageRefusal,
        /// What the call would have done to it.
        gesture: PageGesture,
    },
    /// A form fill was applied, but the page changed some fields back before the values settled.
    FieldsReverted {
        /// How many of the call's fields held their values.
        set: usize,
        /// Every field the page changed, with what it holds now.
        reverted: Vec<RejectedField>,
    },
    /// A form fill was refused before any edit because fields cannot take their values.
    FieldValuesRejected {
        /// Every field the page refused, with what would succeed.
        fields: Vec<RejectedField>,
        /// How many fields the call asked to fill.
        total: usize,
    },
    /// A required browser document could not be bound to current authority.
    DocumentUnavailable,
    /// The job was cancelled after it started but before any browser command was sent.
    ///
    /// The cause is absent when the browser reported the cancellation and the call's own
    /// cancellation was never requested.
    CancelledBeforeDispatch { by: Option<CancelCause> },
    /// The browser this session works in could not take the call, so nothing was sent.
    BrowserUnavailable { reason: BrowserUnavailableReason },
    /// Ghostlight and the browser extension disagree about the bridge contract: a protocol,
    /// authentication, or incompatible-version failure. The disagreement can surface in the
    /// browser's answer to a request it already received, so this says nothing about whether the
    /// request was sent.
    BrowserContract,
    /// Several browsers are connected and the call did not say which one it meant.
    BrowserAmbiguous,
    /// The named browser is not connected.
    BrowserUnknown,
    /// This session already works in a different browser.
    BrowserPinned,
    /// Startup is left to the person: the posture is manual, or more than one browser could
    /// serve and none of them is a usable browser that last connected.
    BrowserStartupManual {
        browsers: Vec<String>,
        /// Whether the person also needs to hear where the extension comes from.
        extension: ExtensionAdvice,
    },
    /// Automatic readiness recovery failed before any browser effect.
    BrowserRecoveryFailed {
        reason: BrowserRecoveryReason,
        /// Whether the person also needs to hear where the extension comes from. It only
        /// changes the advice for [`BrowserRecoveryReason::ExtensionAbsent`] and
        /// [`BrowserRecoveryReason::HandshakeTimeout`]: a browser Ghostlight started whose
        /// adapter never arrived is, while no profile has the extension, a browser without it.
        extension: ExtensionAdvice,
    },
    /// The connection was lost after dispatch, before a completion receipt.
    ConnectionLost,
    /// Cancellation arrived after dispatch, before a completion receipt.
    ///
    /// The cause is absent when the browser reported the cancellation and the call's own
    /// cancellation was never requested.
    CancelledAfterDispatch { by: Option<CancelCause> },
    /// A dispatched effect cannot be determined.
    EffectUnknown,
    /// A denied new-tab landing has an unknown final state.
    LandingDeniedUnknown {
        /// The governance reason code that denied the landing, the refusal's model code.
        code: ReasonCode,
    },
    /// The selected workspace resource is unusable.
    WorkspaceUnusable { reason: WorkspaceReason },
    /// The selected tab shows a page governance refused, on `host` when its address has one
    /// (ADR-0203 Decisions 2.4 and 5.5).
    ///
    /// The host is for the model and the audit's observed host. The closed audit facts keep the
    /// workspace cause `tab_held` alone, as audit v2 has always written it.
    TabHeld { host: Option<String> },
    /// Explicit local files could not be prepared safely.
    FilesUnreadable {
        /// What kept the files from being prepared.
        problem: FileProblem,
    },
    /// A screenshot exceeded the result bound.
    CaptureTooLarge,
    /// A capture of a tab its window is not showing: the browser does not draw it.
    TabNotShowing,
    /// No JavaScript dialog was visible.
    NoDialogVisible,
    /// A JavaScript dialog blocks the page, so the browser could not take the work.
    DialogOpen,
    /// Back (or forward) found no earlier (or later) page in the tab's history, so nothing was
    /// sent.
    NoHistoryEntry { forward: bool },
    /// This session has no recording to act on.
    RecordingUnavailable,
    /// This session already holds a recording, so another cannot start.
    RecordingHeld,
    /// Retained recording frames could not produce a bounded GIF.
    RecordingExportFailed,
    /// The page did not take a saved replay at the target the call named; the recording is kept.
    RecordingNotAttached,
    /// The action was applied, but the condition the caller asked to check was not confirmed.
    ///
    /// The job did complete its effect. What it could not do is confirm the expected page state
    /// afterward, so the account names both the action and why the check fell short.
    ActionUnconfirmed {
        /// The applied action, in its own words.
        action: Box<Outcome>,
        /// Why the check did not confirm it.
        cause: CheckFailure,
        /// The reason code of the decision the result reports: the refusing one when governance
        /// refused the check, which is then the refusal's model code.
        code: ReasonCode,
    },
}

/// The model code of a call cancelled before the browser confirmed anything.
const CANCELLED_CODE: &str = "cancelled";
/// The model code of a call whose deadline passed.
const DEADLINE_CODE: &str = "deadline";
/// The model code of a call whose page access could not be verified.
const DOCUMENT_UNAVAILABLE_CODE: &str = "document_unavailable";
/// The model code of a disagreement about the bridge contract.
const BROWSER_CONTRACT_CODE: &str = "browser_contract_failed";
/// The model code of a dispatched effect that cannot be determined.
const EFFECT_UNKNOWN_CODE: &str = "browser_effect_unknown";

impl Refusal {
    /// The refusal's closed model code: the `reason` fact its result carries (ADR-0203 Decision
    /// 5.1). It is distinct from the tag durable history keeps, which `audit()` writes.
    #[must_use]
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Capacity => "capacity",
            Self::InvalidRequest => "invalid_input",
            Self::CancelledBeforeStart { .. } | Self::CancelledBeforeDispatch { .. } => {
                CANCELLED_CODE
            }
            Self::DeadlineBeforeStart | Self::DeadlineExpired { .. } => DEADLINE_CODE,
            Self::AuthorityBlocked { code, .. } | Self::LandingDeniedUnknown { code } => {
                code.as_str()
            }
            Self::ChannelOff { .. } => ReasonCode::ChannelDenied.as_str(),
            Self::LocalInterlock => "browser_local_interlock",
            Self::CredentialAuthorization { .. } => "credential_authorization_required",
            Self::IncompatibleReceipt => "incompatible_browser_receipt",
            Self::BrowserAdapterOutdated | Self::BrowserContract => BROWSER_CONTRACT_CODE,
            Self::BrowserPrimitive { cause, .. } => cause.as_str(),
            Self::PageRefused { refusal, .. } => refusal.as_str(),
            Self::FieldsReverted { .. } => field_problem::REVERTED_REASON,
            Self::FieldValuesRejected { .. } => field_problem::REJECTED_REASON,
            Self::DocumentUnavailable => DOCUMENT_UNAVAILABLE_CODE,
            Self::BrowserUnavailable { reason } => reason.fact(),
            Self::BrowserAmbiguous => "browser_ambiguous",
            Self::BrowserUnknown => "browser_unknown",
            Self::BrowserPinned => BROWSER_PINNED_FACT,
            Self::BrowserStartupManual { .. } => "browser_startup_manual",
            Self::BrowserRecoveryFailed { reason, .. } => reason.fact(),
            Self::ConnectionLost | Self::CancelledAfterDispatch { .. } | Self::EffectUnknown => {
                EFFECT_UNKNOWN_CODE
            }
            Self::WorkspaceUnusable { reason } => reason.as_fact(),
            Self::TabHeld { .. } => WorkspaceReason::TabHeld.as_fact(),
            Self::FilesUnreadable { problem } => problem.as_str(),
            Self::CaptureTooLarge => "screenshot_too_large",
            Self::TabNotShowing => "tab_not_showing",
            Self::NoDialogVisible => "no_dialog_visible",
            Self::DialogOpen => "dialog_open",
            Self::NoHistoryEntry { .. } => "no_history_entry",
            Self::RecordingUnavailable => "not_found",
            Self::RecordingHeld => "held",
            Self::RecordingExportFailed => "recording_export_failed",
            Self::RecordingNotAttached => "recording_not_attached",
            Self::ActionUnconfirmed { cause, code, .. } => match cause {
                CheckFailure::Unmet => "expectation_unmet",
                CheckFailure::Blocked(_) => code.as_str(),
                CheckFailure::Cancelled => CANCELLED_CODE,
                CheckFailure::DeadlineExpired => DEADLINE_CODE,
                CheckFailure::Disconnected => BrowserUnavailableReason::NotConnected.fact(),
                CheckFailure::BrowserUnavailable => "browser_unavailable",
                CheckFailure::DocumentUnavailable => DOCUMENT_UNAVAILABLE_CODE,
                CheckFailure::BrowserRefused => "browser_refused",
            },
        }
    }

    /// Render the bounded Ghostlight-authored account of the refusal.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Capacity => "The work queue is full. This request was not started.",
            Self::InvalidRequest => "The call does not match the Ghostlight catalog.",
            Self::CancelledBeforeStart { by } => match by {
                CancelCause::Client => "The agent cancelled the browser job before it started.",
                CancelCause::SessionEnded => {
                    "The agent's session ended before the browser job started."
                }
                CancelCause::Quit(_) => "Ghostlight quit before the browser job started.",
            },
            Self::DeadlineBeforeStart => {
                "The browser job deadline expired while waiting for the workspace."
            }
            Self::AuthorityBlocked { code, host, origin } => {
                return blocked(BlockedReason::of(*code), host, origin)
            }
            Self::ChannelOff { channel, author } => return channel_off(*channel, *author),
            Self::LocalInterlock => {
                "Kept the tab open: Preserve Ghostlight tabs is on. To let agents close tabs, turn it off in the Ghostlight in Browser settings."
            }
            Self::CredentialAuthorization { control } => {
                return format!("Nothing was typed. {} is credential-class.", control.phrase());
            }
            Self::IncompatibleReceipt => {
                "The browser answered in a form Ghostlight does not recognize."
            }
            Self::BrowserAdapterOutdated => {
                "This browser command needs a newer Ghostlight extension."
            }
            Self::BrowserPrimitive {
                detail,
                cause: PrimitiveCause::Refused,
            } => {
                return format!("The browser refused this job: {detail}.");
            }
            Self::BrowserPrimitive {
                cause: PrimitiveCause::DebuggerUnavailable,
                ..
            } => "The browser would not let Ghostlight control this tab: another debugger holds it.",
            Self::BrowserPrimitive {
                cause: PrimitiveCause::SessionLost,
                ..
            } => "The browser ended Ghostlight's control of this tab before the job ran.",
            Self::PageRefused { refusal, gesture } => {
                return page_refusal::summary(*refusal, *gesture);
            }
            Self::FieldValuesRejected { fields, total } => {
                return field_problem::rejected_summary(fields, *total, SUMMARY_MAX_CHARS);
            }
            Self::FieldsReverted { set, reverted } => {
                return field_problem::reverted_summary(*set, reverted, SUMMARY_MAX_CHARS);
            }
            Self::DocumentUnavailable => "Document access could not be verified.",
            Self::DeadlineExpired {
                before_dispatch, ..
            } => {
                if *before_dispatch {
                    "The job ran out of time before anything changed in the browser."
                } else {
                    "The job ran out of time before the browser confirmed what the sent command did."
                }
            }
            Self::CancelledBeforeDispatch { by } => match by {
                None => "Cancelled before anything reached the browser.",
                Some(CancelCause::Client) => {
                    "The agent cancelled before anything reached the browser."
                }
                Some(CancelCause::SessionEnded) => {
                    "The agent's session ended before anything reached the browser."
                }
                Some(CancelCause::Quit(_)) => "Ghostlight quit before anything reached the browser.",
            },
            Self::BrowserUnavailable { reason } => match reason {
                BrowserUnavailableReason::NotAnswering => {
                    "The browser this session works in stopped answering. Nothing was sent."
                }
                BrowserUnavailableReason::NotConnected => {
                    "The browser this session works in is not connected. Nothing was sent."
                }
                BrowserUnavailableReason::WrongProfile => {
                    "This session works in a browser that is not connected; a different browser is."
                }
                BrowserUnavailableReason::Busy => {
                    "The browser connection was busy, so nothing was sent."
                }
            },
            Self::BrowserContract => {
                "Ghostlight and the browser extension do not agree on this request."
            }
            Self::BrowserAmbiguous => {
                "More than one browser is connected, so there is no single place to open this."
            }
            Self::BrowserUnknown => "That browser is not connected.",
            Self::BrowserPinned => "This session is already working in a different browser.",
            Self::BrowserStartupManual { browsers, .. } => {
                let choices = browser_choices(browsers);
                return format!(
                    "No browser is connected. Ask the user to open {choices} browser window with the Ghostlight extension installed, then repeat the call."
                );
            }
            Self::BrowserRecoveryFailed { reason, .. } => match reason {
                BrowserRecoveryReason::BrowserAbsent => {
                    "No supported Chromium browser is installed."
                }
                BrowserRecoveryReason::LaunchFailed => {
                    "Ghostlight could not start the selected browser."
                }
                BrowserRecoveryReason::SandboxedPackage => {
                    "The installed browser is sandboxed, so it cannot connect to Ghostlight."
                }
                BrowserRecoveryReason::ExtensionAbsent => {
                    "The selected browser does not have the Ghostlight extension installed."
                }
                BrowserRecoveryReason::NativeHostUnavailable => {
                    "The browser cannot use Ghostlight's native messaging registration."
                }
                BrowserRecoveryReason::OwnedElsewhere => {
                    "Another Ghostlight installation owns the browser registration."
                }
                BrowserRecoveryReason::HandshakeTimeout => {
                    "The Ghostlight adapter did not connect in time after the browser started."
                }
            },
            Self::ConnectionLost => "Connection lost before the browser confirmed completion.",
            Self::EffectUnknown | Self::CancelledAfterDispatch { by: None } => "The browser never confirmed what the sent command did.",
            Self::CancelledAfterDispatch { by: Some(CancelCause::Client) } => {
                "The agent cancelled before the browser confirmed what the sent command did."
            }
            Self::CancelledAfterDispatch { by: Some(CancelCause::SessionEnded) } => {
                "The agent's session ended before the browser confirmed what the sent command did."
            }
            Self::CancelledAfterDispatch { by: Some(CancelCause::Quit(_)) } => {
                "Ghostlight quit before the browser confirmed what the sent command did."
            }
            Self::LandingDeniedUnknown { .. } => {
                "The new tab's final state is unknown after its landing was blocked."
            }
            Self::WorkspaceUnusable { reason } => return reason.summary(),
            Self::TabHeld { host } => return held_tab_summary(host.as_deref()),
            Self::FilesUnreadable { .. } => {
                "The selected local files could not be prepared safely."
            }
            Self::CaptureTooLarge => "The screenshot was too large to return.",
            Self::TabNotShowing => {
                "Nothing was captured: another tab is showing in this tab's window, and the browser draws only the tab it shows."
            }
            Self::NoDialogVisible => "No JavaScript dialog is currently visible.",
            Self::DialogOpen => "A JavaScript dialog is open on this page, so nothing was sent.",
            Self::NoHistoryEntry { forward: false } => {
                "This tab has no earlier page in its history, so nothing was sent."
            }
            Self::NoHistoryEntry { forward: true } => {
                "This tab has no later page in its history, so nothing was sent."
            }
            Self::RecordingUnavailable => "This session has no recording.",
            Self::RecordingHeld => "This session already has a recording.",
            Self::RecordingExportFailed => "The recording could not be turned into a replay.",
            Self::RecordingNotAttached => {
                "The page did not take the replay at that target, so nothing was attached; the recording is kept."
            }
            Self::ActionUnconfirmed { action, cause, .. } => {
                return unconfirmed_summary(action, *cause);
            }
        }
        .into()
    }

    /// Render at most two safe contextual recovery actions for this refusal.
    #[must_use]
    pub fn next_steps(&self) -> Vec<String> {
        match self {

            Self::InvalidRequest => {
                vec!["Match the call to the advertised schema; the invalid_input detail states exactly what to change.".into()]
            }
            Self::Capacity | Self::DeadlineBeforeStart => vec![
                "Repeat the call when the current Ghostlight action has finished.".into(),
            ],
            Self::CancelledBeforeStart {
                by: CancelCause::Quit(quit),
            } if quit.repeat_helps() => vec![REPEAT_IN_A_MOMENT.into()],
            Self::AuthorityBlocked { code, .. } => blocked_next_steps(BlockedReason::of(*code)),
            Self::LocalInterlock => vec![
                "The tab stays open because the user chose to keep Ghostlight's tabs; continue without closing it."
                    .into(),
            ],
            Self::CredentialAuthorization { .. } => vec![
                "If the user explicitly authorized it, repeat with user_authorized_credentials true; otherwise ask the user to type it."
                    .into(),
            ],
            // The extension and the service disagree about the bridge contract, whichever side
            // noticed it; only the person can reload or update the extension.
            Self::IncompatibleReceipt | Self::BrowserAdapterOutdated | Self::BrowserContract => {
                vec![update_extension_then_repeat()]
            }
            Self::BrowserUnavailable { reason } => vec![match reason {
                BrowserUnavailableReason::NotAnswering => {
                    "Check that the browser window is open and responsive, then repeat the call."
                }
                BrowserUnavailableReason::NotConnected => {
                    "Ask the user to reopen that browser, then repeat the call."
                }
                BrowserUnavailableReason::WrongProfile => {
                    "Ask the user to reopen the browser profile this session started in, then repeat the call."
                }
                BrowserUnavailableReason::Busy => "Repeat the call.",
            }
            .into()],
            Self::BrowserPrimitive {
                cause: PrimitiveCause::Refused,
                ..
            } => vec![
                "Read the browser's stated reason, adjust the call or the page, then repeat."
                    .into(),
            ],
            Self::BrowserPrimitive {
                cause: PrimitiveCause::DebuggerUnavailable,
                ..
            } => vec![
                "Ask the user to close the browser's developer tools for this tab, then repeat the call."
                    .into(),
            ],
            Self::BrowserPrimitive {
                cause: PrimitiveCause::SessionLost,
                ..
            } => vec!["Repeat the call; Ghostlight takes control of the tab again.".into()],
            Self::PageRefused { refusal, .. } => page_refusal::next_steps(*refusal),
            Self::FieldValuesRejected { fields, .. } => field_problem::rejected_next_steps(fields),
            Self::FieldsReverted { reverted, .. } => field_problem::rejected_next_steps(reverted),
            Self::DocumentUnavailable => vec!["Inspect the current page before trying again.".into()],
            // Only advice the decoder will take: timeout_ms never grows past its maximum. After
            // dispatch the first job is learning whether the action already happened.
            Self::DeadlineExpired {
                before_dispatch: false,
                ..
            } => vec![
                "Check the page with browser_read or browser_screenshot before repeating; the action may already have happened."
                    .into(),
            ],
            Self::DeadlineExpired {
                before_dispatch: true,
                timeout_adjustable: true,
            } => vec![
                "Repeat with a longer timeout_ms when the page genuinely needs more time."
                    .into(),
            ],
            Self::DeadlineExpired {
                before_dispatch: true,
                timeout_adjustable: false,
            } => vec![
                "Repeat the call; if the page is very large, narrow it first (a target, a smaller max_chars, or a smaller inspect scope)."
                    .into(),
            ],
            Self::BrowserAmbiguous | Self::BrowserUnknown => vec![
                "Call browser_tabs with action list to see the connected browsers.".into(),
                "Repeat the call with the browser handle you want it to open in.".into(),
            ],
            Self::BrowserPinned => vec![
                "Omit browser to continue in the browser this session already works in.".into(),
            ],
            Self::BrowserStartupManual { extension, .. } => extension.step().into_iter().collect(),
            Self::BrowserRecoveryFailed { reason, extension } => match reason {
                BrowserRecoveryReason::BrowserAbsent => vec![format!(
                    "{}, then run ghostlight install.",
                    deployment::install_a_native_browser()
                )],
                BrowserRecoveryReason::SandboxedPackage => vec![
                    "Install a supported native browser package; Snap and Flatpak browsers cannot start a native messaging host."
                        .into(),
                ],
                BrowserRecoveryReason::NativeHostUnavailable => {
                    vec!["Run ghostlight doctor, then repair the named browser registration.".into()]
                }
                BrowserRecoveryReason::OwnedElsewhere => vec![
                    "Ask the user to run ghostlight install from the Ghostlight installation that should own the browsers, then repeat the call."
                        .into(),
                ],
                BrowserRecoveryReason::ExtensionAbsent => vec![extension.step().unwrap_or_else(|| {
                    "Install Ghostlight in the selected browser profile, then repeat the call."
                        .into()
                })],
                BrowserRecoveryReason::HandshakeTimeout => vec![extension.step().unwrap_or_else(|| {
                    ASK_FOR_THAT_BROWSER.into()
                })],
                BrowserRecoveryReason::LaunchFailed => vec![ASK_FOR_THAT_BROWSER.into()],
            },
            Self::WorkspaceUnusable { reason } => reason.next_steps(),
            Self::TabHeld { .. } => vec![LEAVE_HELD_TAB.into()],
            Self::FilesUnreadable { .. } => vec![
                "Confirm each path is an existing file within Ghostlight's upload limits.".into(),
            ],
            Self::TabNotShowing => vec![
                "Call browser_tabs with action focus for this tab, then repeat the screenshot; browser_read and browser_inspect read the page without it."
                    .into(),
            ],
            Self::CaptureTooLarge => vec![
                "Capture the viewport or a smaller region instead of the full page.".into(),
            ],
            Self::DialogOpen => vec![READ_DIALOG_FIRST.into()],
            Self::NoHistoryEntry { .. } => vec![OPEN_FIRST_TAB.into()],
            Self::RecordingUnavailable => vec![
                "Call browser_record with action start to begin a recording.".into(),
            ],
            Self::RecordingHeld => vec![
                "Call browser_record with action save or discard to finish the current recording, then start again."
                    .into(),
            ],
            Self::RecordingExportFailed => vec![
                "Inspect recording status, then discard it or start a shorter recording."
                    .into(),
            ],
            Self::RecordingNotAttached => vec!["Save again with target set to a file input that takes images (browser_inspect lists inputs of type file), or with download true.".into()],
            Self::EffectUnknown | Self::ConnectionLost | Self::CancelledAfterDispatch { .. } => vec![
                "If a JavaScript dialog may be open on the page, handle it with browser_dialog; handling checks the page directly."
                    .into(),
                "Then observe the page with browser_read or browser_inspect to learn what happened."
                    .into(),
            ],
            Self::ActionUnconfirmed { cause, .. } => unconfirmed_next_steps(*cause),
            _ => vec![],
        }
    }

    /// Project the content-free facts named by this refusal into the audit vocabulary.
    #[must_use]
    pub fn observed(&self) -> Observed {
        match self {
            Self::AuthorityBlocked { host, .. } | Self::TabHeld { host } => Observed {
                host: host.clone(),
                ..Observed::default()
            },
            // The action happened, so its measurements stand beside the sentence that names it.
            Self::ActionUnconfirmed { action, .. } => action.observed(),
            _ => Observed::default(),
        }
    }
}

/// The fact that carries the word of a stale target handle, beside the reason `stale_target`.
pub const STALE_FACT: &str = "stale";
/// The fact that lists the controls a stale-target refusal offers, each with a fresh handle.
pub const RECOVERY_CANDIDATES_FACT: &str = "recovery_candidates";
/// The fact that names the one control the page proved is the stale handle's control.
pub const REPLACEMENT_FACT: &str = "replacement";
/// The fact that lists this session's tabs, each with its handle, title and URL, in an
/// `ambiguous_tab` refusal, so the caller can repeat the call naming its own tab.
pub const CANDIDATE_TABS_FACT: &str = "tabs";
/// The fact that names the tab a `handle_on_other_tab` refusal's handle belongs to.
pub const HANDLE_TAB_FACT: &str = "handle_tab";
/// The fact that gives the `width` and `height` of the image an `outside_image` refusal's view
/// shows.
pub const IMAGE_SIZE_FACT: &str = "image";
/// The fact that names, as the window does, the agent of the session an `ownership_mismatch`
/// refusal's handle belongs to, when an agent is named.
pub const OWNER_FACT: &str = "owner_workspace";

/// What a stale-target refusal offers beyond the word (ADR-0198 Decision 28).
///
/// It is recovery help for the model and never reaches the audit: the serialized reason keeps
/// only the stale word.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum StaleHelp {
    /// Nothing the page found is worth naming.
    #[default]
    None,
    /// The facts list candidate controls, each with a handle, for the model to pick from.
    /// `named` is whether each carries its name and row context, which governance can withhold:
    /// without them a candidate is told apart by its role and place alone.
    Candidates {
        /// Whether the candidates carry their name and row context.
        named: bool,
    },
    /// Exactly one control on the page is the one the handle named, and the call can be repeated
    /// with it because nothing was sent.
    Replacement {
        /// The catalog tool the refused call used.
        tool: &'static str,
        /// The handle of the replacement control.
        target: String,
    },
}

/// The one step out of a held tab: the two calls that take it away from the refused page
/// (ADR-0203 Decision 2.4). Each is authorized on its own, never on the page the tab shows.
const LEAVE_HELD_TAB: &str = "Call browser_navigate with the tab and an allowed url to leave the page, or browser_tabs with action close to close the tab.";

/// Say what a held tab shows and what Ghostlight will not do there, naming the refused host when
/// it is known.
fn held_tab_summary(host: Option<&str>) -> String {
    match host {
        Some(host) => format!("That tab shows {host}, which this session may not use, so Ghostlight will not read or act on it there."),
        None => "That tab shows a page this session may not use, so Ghostlight will not read or act on it there.".into(),
    }
}

/// Stable language reason for an unusable workspace resource.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceReason {
    /// The call named no tab and this session has none yet.
    NoTab,
    /// The call named no tab and several of this session's tabs could be meant.
    AmbiguousTab,
    /// The named tab handle names no open tab of this session: its tab closed, its binding was
    /// forgotten, or the session never issued it.
    TabUnavailable,
    /// The target handle no longer names a control, for the reason given.
    StaleTarget {
        reason: StaleReason,
        #[serde(skip)]
        help: StaleHelp,
    },
    /// The view no longer matches current rendered coordinates.
    StaleView,
    /// A target or view handle belongs to another of this session's tabs than the one the call
    /// named. The model hears that tab's handle; the audit keeps the cause word alone.
    HandleOnOtherTab(#[serde(skip)] Option<String>),
    /// One fill named fields, or a submit target, on more than one tab.
    FieldsOnSeveralTabs,
    /// Coordinates reach outside the image of the view they name, whose size the sentence gives.
    /// The audit keeps the cause word alone.
    OutsideImage(#[serde(skip)] Option<ImageSize>),
    /// The selected tab shows a page governance refused. [`Refusal::TabHeld`] names its host.
    TabHeld,
    /// Another invocation owns the workspace lease.
    WorkspaceBusy,
    /// The selected resource belongs elsewhere.
    OwnershipMismatch { owner: Option<String> },
    /// The workspace is no longer admitted.
    WorkspaceClosed,
}

impl WorkspaceReason {
    /// Say which handle went stale and how, rather than that something is "not usable".
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::NoTab => "This session has no open tab yet.".into(),
            Self::AmbiguousTab => {
                "More than one tab is open in this session, so the call must name one.".into()
            }
            Self::TabUnavailable => "That tab handle names no open tab in this session.".into(),
            Self::StaleTarget { reason, help } => match (reason, help) {
                (StaleReason::Removed, StaleHelp::Replacement { .. }) => {
                    "That target is no longer on the page. One control on the page has the same role, name and row."
                }
                (StaleReason::Removed, _) => "That target is no longer on the page.",
                (StaleReason::Navigated, _) => "That target belongs to an older version of the page.",
                (StaleReason::TabClosed, _) => "That target's tab is closed.",
                (StaleReason::Unknown, _) => "Ghostlight does not know that target handle.",
            }
            .into(),
            Self::StaleView => "The page has moved since that screenshot was taken.".into(),
            Self::HandleOnOtherTab(Some(tab)) => {
                format!("That handle belongs to {tab}, another of this session's tabs.")
            }
            Self::HandleOnOtherTab(None) => {
                "That handle belongs to another of this session's tabs.".into()
            }
            Self::FieldsOnSeveralTabs => {
                "A fill works in one tab, and these targets are on more than one.".into()
            }
            Self::OutsideImage(Some(ImageSize { width, height })) => format!(
                "Those coordinates are outside the view's image, which is {width} by {height} pixels."
            ),
            Self::OutsideImage(None) => "Those coordinates are outside the view's image.".into(),
            Self::TabHeld => held_tab_summary(None),
            Self::WorkspaceBusy => {
                "Another Ghostlight action is already using this session.".into()
            }
            Self::OwnershipMismatch { owner: Some(owner) } => {
                format!("That handle belongs to another Ghostlight session ({owner}).")
            }
            Self::OwnershipMismatch { owner: None } => {
                "That handle belongs to a different Ghostlight session.".into()
            }
            Self::WorkspaceClosed => "This Ghostlight session has ended.".into(),
        }
    }

    /// Render the stable structured fact value.
    #[must_use]
    pub const fn as_fact(&self) -> &'static str {
        match self {
            Self::NoTab => "no_tab",
            Self::AmbiguousTab => "ambiguous_tab",
            Self::TabUnavailable => "tab_unavailable",
            Self::StaleTarget { .. } => "stale_target",
            Self::StaleView => "stale_view",
            Self::HandleOnOtherTab(_) => "handle_on_other_tab",
            Self::FieldsOnSeveralTabs => "fields_on_several_tabs",
            Self::OutsideImage(_) => "outside_image",
            Self::TabHeld => "tab_held",
            Self::WorkspaceBusy => "workspace_busy",
            Self::OwnershipMismatch { .. } => "ownership_mismatch",
            Self::WorkspaceClosed => "workspace_closed",
        }
    }

    /// Render zero or one safe contextual recovery actions for this reason.
    #[must_use]
    pub fn next_steps(&self) -> Vec<String> {
        match self {
            Self::NoTab => vec![OPEN_FIRST_TAB.into()],
            Self::AmbiguousTab => {
                vec!["Repeat the call with tab set to the handle of the tab you mean, listed in tabs.".into()]
            }
            Self::TabUnavailable => {
                vec!["Call browser_tabs with action list to obtain current tab handles.".into()]
            }
            Self::StaleTarget { reason, help } => vec![match (reason, help) {
                (StaleReason::Removed, StaleHelp::Replacement { tool, target }) => {
                    format!("Repeat {tool} with target {target}.")
                }
                (StaleReason::Removed, StaleHelp::Candidates { named: true }) => {
                    "Pick a target from recovery_candidates by its context, or call browser_find for current target handles."
                        .into()
                }
                (StaleReason::Removed, StaleHelp::Candidates { named: false }) => {
                    "Pick a target from recovery_candidates by its role and nth, or call browser_find for current target handles."
                        .into()
                }
                (StaleReason::Removed | StaleReason::Unknown, _) => {
                    "Call browser_find or browser_inspect to obtain current target handles.".into()
                }
                (StaleReason::Navigated, _) => {
                    "Call browser_inspect to obtain current target handles.".into()
                }
                (StaleReason::TabClosed, _) => "Call browser_tabs to choose an open tab.".into(),
            }],
            Self::StaleView => {
                vec!["Call browser_screenshot to obtain a current view handle.".into()]
            }
            Self::HandleOnOtherTab(_) => vec![
                "Repeat the call with handles from one tab, and with tab left out or set to that tab."
                    .into(),
            ],
            Self::FieldsOnSeveralTabs => {
                vec!["Make one browser_fill_form call for each tab.".into()]
            }
            Self::OutsideImage(_) => vec![
                "Repeat with coordinates inside the image, or call browser_screenshot for a new view."
                    .into(),
            ],
            Self::TabHeld => vec![LEAVE_HELD_TAB.into()],
            Self::WorkspaceBusy => {
                vec!["Wait for the active Ghostlight invocation to finish.".into()]
            }
            Self::OwnershipMismatch { owner: Some(_) } => {
                vec!["Call browser_tabs with action list to see this session's tabs, then continue with those handles.".into()]
            }
            Self::OwnershipMismatch { owner: None } => {
                vec!["Collect fresh handles from this session, then continue with those.".into()]
            }
            Self::WorkspaceClosed => vec![
                "Start over; the next call opens a fresh session and old handles will not work."
                    .into(),
            ],
        }
    }
}

/// The size of a view's image, in pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageSize {
    /// The image's width.
    pub width: u32,
    /// The image's height.
    pub height: u32,
}

/// The `reason` fact of a refusal because this session already works in another browser.
pub const BROWSER_PINNED_FACT: &str = "browser_pinned";

/// The refusal that says what is true of a workspace error, and the `reason` fact word that names
/// it in the result.
///
/// A held tab names the host of the page that held it. A tab of another browser than the one the
/// session works in is the session's browser binding, so it is refused as the browser choice is.
/// Every other error is an unusable workspace resource, for its own reason.
#[must_use]
pub fn workspace_refusal(error: WorkspaceError) -> (Refusal, &'static str) {
    let reason = match error {
        WorkspaceError::Held(hold) => {
            return (
                Refusal::TabHeld { host: hold.host() },
                WorkspaceReason::TabHeld.as_fact(),
            )
        }
        WorkspaceError::BrowserPinned => return (Refusal::BrowserPinned, BROWSER_PINNED_FACT),
        WorkspaceError::NoTab => WorkspaceReason::NoTab,
        WorkspaceError::AmbiguousTab => WorkspaceReason::AmbiguousTab,
        WorkspaceError::StaleTab => WorkspaceReason::TabUnavailable,
        WorkspaceError::StaleTarget { reason } => WorkspaceReason::StaleTarget {
            reason,
            help: StaleHelp::None,
        },
        WorkspaceError::StaleView => WorkspaceReason::StaleView,
        WorkspaceError::HandleOnOtherTab { tab } => {
            WorkspaceReason::HandleOnOtherTab(Some(tab.as_str().to_owned()))
        }
        WorkspaceError::FieldsOnSeveralTabs => WorkspaceReason::FieldsOnSeveralTabs,
        WorkspaceError::OutsideImage { width, height } => {
            WorkspaceReason::OutsideImage(Some(ImageSize { width, height }))
        }
        WorkspaceError::Busy => WorkspaceReason::WorkspaceBusy,
        WorkspaceError::NotOwnedTab
        | WorkspaceError::NotOwnedTarget
        | WorkspaceError::NotOwnedView
        | WorkspaceError::PhysicalTabOwned => WorkspaceReason::OwnershipMismatch { owner: None },
        WorkspaceError::UnknownWorkspace => WorkspaceReason::WorkspaceClosed,
    };
    let fact = reason.as_fact();
    (Refusal::WorkspaceUnusable { reason }, fact)
}

/// The label that introduces the safe next steps in a result's model-facing text.
const NEXT_STEPS_LABEL: &str = "Next:";

/// Render a result's model-facing text: the summary, then its safe next steps, if any.
#[must_use]
pub fn model_text<S: AsRef<str>>(summary: &str, next_steps: &[S]) -> String {
    let mut text = summary.to_owned();
    if !next_steps.is_empty() {
        text.push(' ');
        text.push_str(NEXT_STEPS_LABEL);
        for step in next_steps {
            text.push(' ');
            text.push_str(step.as_ref());
        }
    }
    text
}

/// Authored sentence for a navigation that recreated a dead tab binding.
#[must_use]
fn page_recovered_summary(host: Option<&str>) -> String {
    match host {
        Some(host) => format!("That tab was gone; opened {host} in a new tab."),
        None => "That tab was gone; opened the requested page in a new tab.".into(),
    }
}

/// Authored sentence for closing what was already closed.
#[must_use]
fn tab_already_closed_summary() -> String {
    "That tab was already closed.".into()
}

/// Content-free observations about one action.
///
/// This is deliberately not `InvocationResult::facts`. Facts legitimately carry page text and
/// full URLs for the model; an audit record carries measurements and metadata only. Keeping the
/// two as separate closed types makes copying one into the other impossible rather than merely
/// discouraged.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
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
    /// Whether the action used a handle that followed its control through a re-render the page
    /// proved. A closed fact: it names no handle, row or link.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rebound: bool,
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
            rebound: later.rebound || self.rebound,
        }
    }
}

fn place<'a>(host: &'a Option<String>, fallback: &'static str) -> &'a str {
    host.as_deref().unwrap_or(fallback)
}

/// The page's host, or "the page" when its address has none.
fn on_page(host: &Option<String>) -> &str {
    place(host, "the page")
}

/// Name what authority refused and where, instead of that authority refused.
///
/// "Authority blocked the browser job" is true of every denial there is, which makes it useless
/// to the person reading it. The reason code and the attempted host were both already on hand.
fn blocked(reason: BlockedReason, host: &Option<String>, origin: &Option<String>) -> String {
    match (reason, host.as_deref()) {
        (BlockedReason::AuditUnavailable, _) => "Browser work stopped because policy requires saved history. Ghostlight will check storage automatically; no action will be replayed.".into(),
        (BlockedReason::Host, Some(host)) => format!("Blocked: {host} is not an allowed host."),
        (BlockedReason::Host, None) => "Blocked: that host is not allowed.".into(),
        (BlockedReason::ProtectedHost, Some(host)) => {
            format!("Blocked: {host} is protected and is never automated.")
        }
        (BlockedReason::ProtectedHost, None) => {
            "Blocked: that host is protected and is never automated.".into()
        }
        (BlockedReason::OwnWorkbench, _) => match origin {
            Some(origin) => format!("Blocked: {origin} is {OWN_CONTROL_PAGE_REFUSED}"),
            None => format!("Blocked: that address is {OWN_CONTROL_PAGE_REFUSED}"),
        },
        (BlockedReason::Capability, _) => {
            "Blocked: this session may not take that kind of action.".into()
        }
        (BlockedReason::TabClose, _) => "Blocked: this session may not close tabs.".into(),
        (BlockedReason::Screenshot, _) => "Blocked: screenshots are disabled by policy.".into(),
        (BlockedReason::InvalidAuthority, _) => {
            "Blocked: the Ghostlight policy could not be read.".into()
        }
        // The two human controls are the only refusals that speak to the controller rather than
        // about the request. A model that reads "Blocked: Ghostlight is paused." has been told a
        // fact and left to guess; these tell it what to do, and are pinned by ADR-0126 Decisions 4
        // and 5. Pause is reversible and says so by not being terminal; stop ends the session.
        (BlockedReason::Hold, _) => HUMAN_PAUSE_DIRECTIVE.into(),
        (BlockedReason::SessionEnded, _) => HUMAN_STOP_DIRECTIVE.into(),
        (BlockedReason::Unspecified, Some(host)) => format!("Blocked by policy on {host}."),
        (BlockedReason::Unspecified, None) => "Blocked by Ghostlight policy.".into(),
    }
}

/// Say which channel switch is off and whose policy turned it off.
///
/// No next step goes with it: the caller cannot turn the switch back on, and nothing it calls on
/// this connection will run until a person does.
fn channel_off(channel: IntakeChannel, author: PolicyAuthor) -> String {
    let whose = match author {
        PolicyAuthor::Organization => "the organization's",
        PolicyAuthor::User => "the user's",
    };
    let from = super::channel_switch::work_from(channel);
    format!("Blocked: {whose} Ghostlight policy turns off work from {from}.")
}

/// Say what the page did, not which condition constant was evaluated.
///
/// "Wait condition text_present was satisfied after 1687 ms" describes the mechanism doing the
/// looking. A reader wants to know that the text showed up, and roughly when.
fn waited(
    condition: WaitCondition,
    elapsed_ms: u64,
    satisfied: bool,
    host: &Option<String>,
) -> String {
    let location = on_page(host);
    let (subject, arrived, missing) = match condition {
        WaitCondition::LoadReady => (None, "finished loading", "never finished loading"),
        WaitCondition::TextPresent => (Some("Text"), "appeared", "never appeared"),
        WaitCondition::TextAbsent => (Some("Text"), "disappeared", "never disappeared"),
        WaitCondition::UrlContains => (Some("The address"), "matched", "never matched"),
        WaitCondition::TargetPresent => (Some("The target"), "appeared", "never appeared"),
        WaitCondition::TargetAbsent => (Some("The target"), "disappeared", "never disappeared"),
        WaitCondition::SelectorPresent => (
            Some("The selector"),
            "matched a control",
            "matched no control",
        ),
        WaitCondition::VisualSettle => (
            Some("The view"),
            "settled visually",
            "never settled visually",
        ),
        // A duration watches nothing on the page; it only lets time pass.
        WaitCondition::Duration => return format!("Waited {} on {location}.", spanned(elapsed_ms)),
    };
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

/// The sentence for a dialog that is showing: its message when the browser reported one and, for
/// a prompt, what its field holds when the extension reported it.
fn dialog_observed(message: Option<&str>, prompt: Option<&PromptField>) -> String {
    let shown = if prompt.is_some() {
        "A prompt dialog is currently visible"
    } else {
        "A JavaScript dialog is currently visible"
    };
    let opening =
        match message.and_then(|message| one_line_bounded(message, DIALOG_MESSAGE_MAX_CHARS)) {
            Some(message) => format!("{shown}: \"{message}\"."),
            None => format!("{shown}."),
        };
    let field = match prompt {
        Some(PromptField::Holds(text)) => match one_line_bounded(text, DIALOG_MESSAGE_MAX_CHARS) {
            Some(text) => format!(" Its field holds \"{text}\"."),
            None if text.is_empty() => " Its field is empty.".to_owned(),
            None => " Its field holds only spaces.".to_owned(),
        },
        Some(PromptField::Unreported) | None => String::new(),
    };
    format!("{opening}{field}")
}

/// The step after a listing the cap cut: how many controls were left out, and the two ways to
/// reach one. It names the tier that matters most among those cut, because that is the one the
/// caller most likely wants. Hidden controls never get a step, and a find has no `max_items`.
fn left_out_step(noun: TargetNoun, left_out: &LeftOut) -> Option<String> {
    let (key, place) = [
        (LeftOutKey::Content, "page content"),
        (LeftOutKey::Aside, "aside"),
        (LeftOutKey::Chrome, "header, nav and footer"),
    ]
    .into_iter()
    .find(|(key, _)| left_out.count(*key) > 0)?;
    let (singular, plural) = match noun {
        TargetNoun::Match => return None,
        TargetNoun::Control => ("control", "controls"),
        TargetNoun::Item => ("item", "items"),
    };
    let amount = left_out.count(key);
    let verb = if amount == 1 { "was" } else { "were" };
    Some(format!(
        "{} {verb} left out to fit; raise max_items (up to {MAX_INSPECT_ITEMS}) or use browser_find for one by name.",
        counted(amount, &format!("{place} {singular}"), &format!("{place} {plural}")),
    ))
}

fn measured<T: TryInto<u32>>(value: T) -> Option<u32> {
    Some(value.try_into().unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_observed_fact_is_documented_for_the_collectors_that_read_it() {
        // The audit record's reader is a person configuring a collector, so the guide is where
        // the vocabulary has to stay honest. The literal names every field, so a new one must be
        // set here, and fails until the guide documents it.
        let guide = include_str!("../../../../docs/guides/siem-integration.md");
        let every_field = super::Observed {
            host: Some("example.com".into()),
            readiness: Some("complete".into()),
            count: Some(1240),
            width: Some(1280),
            height: Some(720),
            rebound: true,
        };
        let encoded = serde_json::to_value(every_field).expect("observations serialize");
        let fields = encoded.as_object().expect("an observation is an object");
        for field in fields.keys() {
            assert!(
                guide.contains(&format!("`{field}`")),
                "siem-integration.md documents no {field} field, so collectors cannot expect it"
            );
        }
    }

    #[test]
    fn number_fields_and_list_boxes_keep_their_own_nouns() {
        assert_eq!(TargetRole::classify("spinbutton"), TargetRole::NumberField);
        assert_eq!(TargetRole::NumberField.noun(), "number field");
        assert_eq!(TargetRole::classify("slider"), TargetRole::Slider);
        assert_eq!(TargetRole::classify("listbox"), TargetRole::ListBox);
        assert_eq!(TargetRole::ListBox.noun(), "list box");
        assert_eq!(TargetRole::classify("combobox"), TargetRole::Combobox);
        assert_ne!(TargetRole::classify("spinbutton").noun(), "slider");
    }

    fn composition(completed: usize, total: usize) -> super::Outcome {
        use crate::language::composition::{CompositionProgress, StepCause, StepCounts, StepIssue};
        let incomplete = completed < total;
        super::Outcome::CompositionRan(CompositionProgress {
            counts: StepCounts {
                total,
                succeeded: completed,
                failed: usize::from(incomplete),
                not_run: total - completed - usize::from(incomplete),
                ..StepCounts::default()
            },
            stopped: incomplete,
            issue: incomplete.then_some(StepIssue {
                step: completed + 1,
                cause: StepCause::Failed,
            }),
            ..CompositionProgress::default()
        })
    }

    use serde_json::json;

    use super::{
        workspace_refusal, ActionSubject, BlockedReason, BrowserRecoveryReason,
        BrowserUnavailableReason, CancelCause, CaptureKind, CheckFailure, DialogKind, EmptyObject,
        ExtensionAdvice, FileProblem, ImageSize, Observed, Outcome, PolicyAuthor, PrimitiveCause,
        PromptField, QuitCause, Refusal, SavedTo, StaleHelp, TargetNoun, TargetRole,
        WorkspaceReason, HUMAN_PAUSE_DIRECTIVE, HUMAN_STOP_DIRECTIVE, REPEAT_IN_A_MOMENT,
        TARGET_LABEL_MAX_CHARS,
    };
    use crate::intake::session::IntakeChannel;
    use crate::language::area::{LeftOut, LeftOutKey};
    use crate::language::field_problem::ControlRef;
    use crate::language::WaitCondition;
    use crate::workspace::{StaleReason, WorkspaceError};

    #[test]
    fn a_duration_wait_has_its_own_sentence() {
        let waited = |elapsed_ms, host: Option<&str>| {
            Outcome::Waited {
                condition: WaitCondition::Duration,
                elapsed_ms,
                satisfied: true,
                host: host.map(str::to_owned),
            }
            .summary()
        };
        assert_eq!(
            waited(1_200, Some("example.com")),
            "Waited 1 second on example.com."
        );
        assert_eq!(waited(300, None), "Waited under a second on the page.");
        for condition in WaitCondition::ALL {
            let summary = Outcome::Waited {
                condition,
                elapsed_ms: 1_200,
                satisfied: true,
                host: None,
            }
            .summary();
            assert!(!summary.contains("The condition"), "{summary}");
        }
    }

    #[test]
    fn outcome_summaries_transcribe_the_product_oracles() {
        let examples = vec![
            (
                Outcome::TabsListed { count: 4 },
                "Listed 4 tabs in this session.",
            ),
            (
                Outcome::TabsListed { count: 1 },
                "Listed 1 tab in this session.",
            ),
            (
                Outcome::TabsListed { count: 0 },
                "Listed 0 tabs in this session.",
            ),
            (
                Outcome::DocumentInspected {
                    nodes: 283,
                    truncated: false,
                    differences: None,
                },
                "Returned the document outline: 283 nodes.",
            ),
            (
                Outcome::DocumentInspected {
                    nodes: 283,
                    truncated: false,
                    differences: Some(0),
                },
                "Returned the document outline: 283 nodes, matching the previous outline.",
            ),
            (
                Outcome::DocumentInspected {
                    nodes: 1,
                    truncated: false,
                    differences: Some(6),
                },
                "Returned the document outline: 1 node, 6 differences from the previous outline.",
            ),
            (
                Outcome::PolicyExplained {
                    capabilities: 8,
                    layers: 2,
                },
                "Explained what agents may do: 8 capability areas, 2 policy layers.",
            ),
            (
                Outcome::PolicyExplained {
                    capabilities: 1,
                    layers: 1,
                },
                "Explained what agents may do: 1 capability area, 1 policy layer.",
            ),
            (
                Outcome::PolicyExplained {
                    capabilities: 4,
                    layers: 0,
                },
                "Explained what agents may do: 4 capability areas, no policy layers.",
            ),
            (
                Outcome::RecordingBoundary {
                    duration_ms: 12_000,
                },
                "Recording stopped after 12 seconds because the page, or a frame inside it, loaded new content or went away. The captured portion is available.",
            ),
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
                Outcome::PageReused {
                    host: Some("example.com".into()),
                },
                "Reused the example.com tab.",
            ),
            (
                Outcome::PageReused { host: None },
                "Reused the existing tab.",
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
                    left_out: LeftOut::default(),
                },
                "Found 7 matches on example.com.",
            ),
            (
                Outcome::TargetsListed {
                    noun: TargetNoun::Control,
                    count: 29,
                    host: Some("example.com".into()),
                    left_out: LeftOut::default(),
                },
                "Found 29 controls on example.com.",
            ),
            (
                Outcome::TargetsListed {
                    noun: TargetNoun::Item,
                    count: 3,
                    host: Some("example.com".into()),
                    left_out: LeftOut::default(),
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
                    direction: crate::language::ScrollDirection::Down,
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
                    held: Vec::new(),
                    rebound: false,
                },
                "Filled 1 field on example.com.",
            ),
            (
                Outcome::FormFilled {
                    fields: 3,
                    submitted: true,
                    host: Some("example.com".into()),
                    held: Vec::new(),
                    rebound: false,
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
                    source: Some(ActionSubject::from_page("slider", "Volume", true)),
                    destination: Some(ActionSubject::from_page("slider", "Volume", true)),
                },
                "Dragged across the \"Volume\" slider on example.com.",
            ),
            (
                // Without names there is no telling one control from another of its kind.
                Outcome::Dragged {
                    host: Some("example.com".into()),
                    source: Some(ActionSubject::unnamed(TargetRole::Control)),
                    destination: Some(ActionSubject::unnamed(TargetRole::Control)),
                },
                "Dragged a control onto a control on example.com.",
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
                Outcome::FilesUploaded {
                    count: 1,
                    host: Some("example.com".into()),
                    subject: Some(
                        ActionSubject::from_page("textbox", "Screenshot file", true).file_field(),
                    ),
                },
                "Uploaded 1 file through the \"Screenshot file\" file field on example.com.",
            ),
            (
                Outcome::ScriptEvaluated {
                    host: Some("example.com".into()),
                    empty_object: EmptyObject::Absent,
                },
                "Executed JavaScript on example.com.",
            ),
            (composition(2, 5), "Completed 2 of 5 steps. Step 3 failed."),
            (composition(5, 5), "Completed all 5 steps."),
            (
                Outcome::Waited {
                    condition: WaitCondition::LoadReady,
                    elapsed_ms: 1830,
                    satisfied: true,
                    host: Some("example.com".into()),
                },
                "example.com finished loading in 2 seconds.",
            ),
            (
                Outcome::Waited {
                    condition: WaitCondition::TextPresent,
                    elapsed_ms: 8000,
                    satisfied: false,
                    host: Some("example.com".into()),
                },
                "Text never appeared on example.com within 8 seconds.",
            ),
            (
                Outcome::TargetClicked {
                    host: Some("example.com".into()),
                    subject: ActionSubject::from_page("button", "Delete", true),
                }
                .with_open_dialog(DialogKind::Confirm),
                "Clicked the \"Delete\" button on example.com. A confirm dialog is now open.",
            ),
            (
                Outcome::ScriptEvaluated {
                    host: Some("example.com".into()),
                    empty_object: EmptyObject::Absent,
                }
                .with_open_dialog(DialogKind::classify("a kind from the future")),
                "Executed JavaScript on example.com. A JavaScript dialog is now open.",
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
                Outcome::DialogObserved {
                    present: true,
                    message: None,
                    prompt: None,
                },
                "A JavaScript dialog is currently visible.",
            ),
            (
                Outcome::DialogObserved {
                    present: true,
                    message: Some("Leave this page?
Changes are \"unsaved\".".into()),
                    prompt: None,
                },
                "A JavaScript dialog is currently visible: \"Leave this page? Changes are 'unsaved'.\".",
            ),
            (
                Outcome::DialogObserved {
                    present: true,
                    message: Some("What should it say?".into()),
                    prompt: Some(PromptField::Holds("hello from the desk".into())),
                },
                "A prompt dialog is currently visible: \"What should it say?\". Its field holds \"hello from the desk\".",
            ),
            (
                Outcome::DialogObserved {
                    present: true,
                    message: Some("Name?".into()),
                    prompt: Some(PromptField::Holds(String::new())),
                },
                "A prompt dialog is currently visible: \"Name?\". Its field is empty.",
            ),
            (
                Outcome::DialogObserved {
                    present: true,
                    message: None,
                    prompt: Some(PromptField::Holds("  \t ".into())),
                },
                "A prompt dialog is currently visible. Its field holds only spaces.",
            ),
            (
                Outcome::DialogObserved {
                    present: true,
                    message: Some("Name?".into()),
                    prompt: Some(PromptField::Unreported),
                },
                "A prompt dialog is currently visible: \"Name?\".",
            ),
            (
                Outcome::DialogObserved {
                    present: false,
                    message: None,
                    prompt: None,
                },
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
                    active: true,
                },
                "Recording for 1 second, 3 frames so far.",
            ),
            (
                Outcome::RecordingObserved {
                    frames: 35,
                    duration_ms: 20_598,
                    active: false,
                },
                "A stopped recording of 21 seconds with 35 frames is ready to save.",
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
                    rebound: false,
                },
                "Saved a replay of 30 seconds of page changes.",
            ),
            (
                Outcome::RecordingSaved {
                    duration_ms: 30_400,
                    delivery: SavedTo::PageTarget,
                    rebound: false,
                },
                "Attached a replay of 30 seconds of page changes to the page.",
            ),
            (
                Outcome::RecordingSaved {
                    duration_ms: 30_400,
                    delivery: SavedTo::Download,
                    rebound: false,
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

    /// A followed handle changes no sentence. The one fact reaches the measurements of every
    /// action that names a subject, and of an action that left a dialog open.
    #[test]
    fn a_followed_handle_marks_the_measurements_and_not_the_sentence() {
        let host = Some("example.com".to_owned());
        let followed = || ActionSubject::from_page("button", "Save", true).rebound(true);
        let plain = || ActionSubject::from_page("button", "Save", true);
        let pairs = [
            (
                Outcome::TargetClicked {
                    host: host.clone(),
                    subject: followed(),
                },
                Outcome::TargetClicked {
                    host: host.clone(),
                    subject: plain(),
                },
            ),
            (
                Outcome::Hovered {
                    host: host.clone(),
                    subject: Some(followed()),
                },
                Outcome::Hovered {
                    host: host.clone(),
                    subject: Some(plain()),
                },
            ),
            (
                Outcome::TextTyped {
                    host: host.clone(),
                    subject: followed(),
                    characters: 3,
                },
                Outcome::TextTyped {
                    host: host.clone(),
                    subject: plain(),
                    characters: 3,
                },
            ),
            (
                Outcome::Dragged {
                    host: host.clone(),
                    source: Some(plain()),
                    destination: Some(followed()),
                },
                Outcome::Dragged {
                    host: host.clone(),
                    source: Some(plain()),
                    destination: Some(plain()),
                },
            ),
        ];
        for (followed, plain) in pairs {
            assert_eq!(followed.summary(), plain.summary());
            assert_eq!(followed.next_steps(), plain.next_steps());
            assert_eq!(followed.audit().summary(), plain.audit().summary());
            assert!(followed.observed().rebound && !plain.observed().rebound);
            let opened = followed.with_open_dialog(DialogKind::Confirm);
            assert!(opened.observed().rebound, "a dialog keeps the fact");
        }
        assert!(!Outcome::TabsListed { count: 1 }.observed().rebound);
        let saved = |rebound| Outcome::RecordingSaved {
            duration_ms: 2_000,
            delivery: SavedTo::PageTarget,
            rebound,
        };
        assert_eq!(saved(true).summary(), saved(false).summary());
        assert!(saved(true).observed().rebound && !saved(false).observed().rebound);
        let merged = Observed {
            rebound: true,
            ..Observed::default()
        }
        .merged(Observed::default());
        assert!(merged.rebound, "a later account does not erase the fact");
        let line = serde_json::to_value(Observed::default()).unwrap();
        assert!(
            line.get("rebound").is_none(),
            "an ordinary line is unchanged"
        );
        assert_eq!(
            serde_json::to_value(merged).unwrap()["rebound"],
            serde_json::json!(true)
        );
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
                direction: crate::language::ScrollDirection::Down,
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
                empty_object: EmptyObject::Absent,
            },
        ];
        for outcome in host_outcomes {
            assert!(outcome.summary().contains("example.com"));
            assert_eq!(outcome.observed().host.as_deref(), Some("example.com"));
        }

        let counted = [
            (Outcome::TabsListed { count: 3 }, 3),
            (
                Outcome::PolicyExplained {
                    capabilities: 3,
                    layers: 1,
                },
                3,
            ),
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
                    left_out: LeftOut::default(),
                },
                3,
            ),
            (
                Outcome::FormFilled {
                    fields: 3,
                    submitted: false,
                    host: Some("example.com".into()),
                    held: Vec::new(),
                    rebound: false,
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
                    condition: WaitCondition::LoadReady,
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

        let sequence = composition(3, 5);
        assert_eq!(sequence.summary(), "Completed 3 of 5 steps. Step 4 failed.");
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
            code: crate::governance::ReasonCode::SessionEnded,
            host: None,
            origin: None,
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
            code: crate::governance::ReasonCode::RuntimeHold,
            host: None,
            origin: None,
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
            let refusal = Refusal::AuthorityBlocked {
                code: reason.code(),
                host: None,
                origin: None,
            };
            assert!(
                refusal.next_steps().is_empty(),
                "{reason:?} suggested a next step"
            );
        }
    }

    /// An action that happened stays in the account when its check falls short, and nothing in
    /// that account says the action did not happen or invites a blind repeat.
    #[test]
    fn unconfirmed_actions_keep_the_action_and_say_why_the_check_fell_short() {
        let action = Outcome::TargetClicked {
            host: Some("example.com".into()),
            subject: ActionSubject::from_page("button", "Save", true),
        };
        let check = "Check the page with browser_read or browser_inspect before repeating; the action already happened.";
        let cases = [
            (
                CheckFailure::Unmet,
                "The expected condition did not hold after the action. Clicked the \"Save\" button on example.com.",
                vec![check],
            ),
            (
                CheckFailure::Cancelled,
                "The expected condition was not checked because the call was cancelled. Clicked the \"Save\" button on example.com.",
                vec![check],
            ),
            (
                CheckFailure::Disconnected,
                "The expected condition was not checked because the browser disconnected. Clicked the \"Save\" button on example.com.",
                vec![check],
            ),
            (
                CheckFailure::BrowserUnavailable,
                "The expected condition was not checked because Ghostlight could not reach the browser. Clicked the \"Save\" button on example.com.",
                vec![check],
            ),
            (
                CheckFailure::DeadlineExpired,
                "The expected condition was not checked because the call ran out of time. Clicked the \"Save\" button on example.com.",
                vec![check],
            ),
            (
                CheckFailure::DocumentUnavailable,
                "The expected condition was not checked because access to the page could not be verified. Clicked the \"Save\" button on example.com.",
                vec![check],
            ),
            (
                CheckFailure::BrowserRefused,
                "The expected condition was not checked because the browser could not run the check. Clicked the \"Save\" button on example.com.",
                vec![check],
            ),
            (
                CheckFailure::Blocked(BlockedReason::Hold),
                "The user paused Ghostlight. Wait for further instructions. The expected condition was not checked. Clicked the \"Save\" button on example.com.",
                vec![],
            ),
            (
                CheckFailure::Blocked(BlockedReason::SessionEnded),
                "The user asked to interrupt the process. Wait for further instructions. The expected condition was not checked. Clicked the \"Save\" button on example.com.",
                vec![],
            ),
            (
                CheckFailure::Blocked(BlockedReason::Host),
                "The expected condition was not checked. Blocked: that host is not allowed. Clicked the \"Save\" button on example.com.",
                vec!["Call policy_explain to see what this session may do."],
            ),
        ];
        for (cause, summary, steps) in cases {
            let refusal = Refusal::ActionUnconfirmed {
                action: Box::new(action.clone()),
                cause,
                code: cause.decision_code(),
            };
            assert_eq!(refusal.summary(), summary, "{cause:?}");
            assert_eq!(refusal.next_steps(), steps, "{cause:?}");
            for false_claim in ["before anything happened", "repeat the call", "nothing"] {
                assert!(
                    !refusal.summary().contains(false_claim),
                    "{cause:?} says {false_claim}"
                );
            }
            assert_eq!(refusal.observed(), action.observed(), "{cause:?}");
            let audit = refusal.audit();
            assert_eq!(audit.summary(), summary, "{cause:?}");
            assert_eq!(
                audit.refusal(),
                Some(&crate::language::audit::AuditRefusal::ActionUnconfirmed { cause }),
                "{cause:?}"
            );
        }
        assert_eq!(
            serde_json::to_value(
                Refusal::ActionUnconfirmed {
                    action: Box::new(action.clone()),
                    cause: CheckFailure::Blocked(BlockedReason::Hold),
                    code: CheckFailure::Blocked(BlockedReason::Hold).decision_code(),
                }
                .audit()
                .refusal()
            )
            .unwrap(),
            json!({"reason":"action_unconfirmed","cause":{"blocked":"hold"}})
        );
    }

    #[test]
    fn refusal_wording_and_recovery_stay_exact() {
        let summaries = vec![
            (
                Refusal::InvalidRequest,
                "The call does not match the Ghostlight catalog.",
            ),
            (
                Refusal::CancelledBeforeStart {
                    by: CancelCause::Client,
                },
                "The agent cancelled the browser job before it started.",
            ),
            (
                Refusal::CancelledBeforeStart {
                    by: CancelCause::SessionEnded,
                },
                "The agent's session ended before the browser job started.",
            ),
            (
                Refusal::CancelledBeforeStart {
                    by: CancelCause::Quit(QuitCause::Quit),
                },
                "Ghostlight quit before the browser job started.",
            ),
            (
                Refusal::DeadlineBeforeStart,
                "The browser job deadline expired while waiting for the workspace.",
            ),
            (
                Refusal::AuthorityBlocked {
                    code: crate::governance::ReasonCode::HostDenied,
                    host: Some("example.com".into()),
                    origin: None,
                },
                "Blocked: example.com is not an allowed host.",
            ),
            (
                Refusal::AuthorityBlocked {
                    code: crate::governance::ReasonCode::RuntimeHold,
                    host: None,
                    origin: None,
                },
                HUMAN_PAUSE_DIRECTIVE,
            ),
            (
                Refusal::AuthorityBlocked {
                    code: crate::governance::ReasonCode::SessionEnded,
                    host: None,
                    origin: None,
                },
                HUMAN_STOP_DIRECTIVE,
            ),
            (
                Refusal::LocalInterlock,
                "Kept the tab open: Preserve Ghostlight tabs is on. To let agents close tabs, turn it off in the Ghostlight in Browser settings.",
            ),
            (
                Refusal::CredentialAuthorization {
                    control: ControlRef::new(TargetRole::Textbox, "Password", Some("target_7k2nb5xe"), true),
                },
                "Nothing was typed. The \"Password\" text field (target_7k2nb5xe) is credential-class.",
            ),
            (
                Refusal::IncompatibleReceipt,
                "The browser answered in a form Ghostlight does not recognize.",
            ),
            (
                Refusal::BrowserAdapterOutdated,
                "This browser command needs a newer Ghostlight extension.",
            ),
            (
                Refusal::BrowserUnavailable {
                    reason: BrowserUnavailableReason::NotAnswering,
                },
                "The browser this session works in stopped answering. Nothing was sent.",
            ),
            (
                Refusal::BrowserUnavailable {
                    reason: BrowserUnavailableReason::NotConnected,
                },
                "The browser this session works in is not connected. Nothing was sent.",
            ),
            (
                Refusal::BrowserUnavailable {
                    reason: BrowserUnavailableReason::WrongProfile,
                },
                "This session works in a browser that is not connected; a different browser is.",
            ),
            (
                Refusal::BrowserUnavailable {
                    reason: BrowserUnavailableReason::Busy,
                },
                "The browser connection was busy, so nothing was sent.",
            ),
            (
                Refusal::BrowserContract,
                "Ghostlight and the browser extension do not agree on this request.",
            ),
            (
                Refusal::CancelledBeforeDispatch { by: None },
                "Cancelled before anything reached the browser.",
            ),
            (
                Refusal::CancelledBeforeDispatch {
                    by: Some(CancelCause::Client),
                },
                "The agent cancelled before anything reached the browser.",
            ),
            (
                Refusal::CancelledBeforeDispatch {
                    by: Some(CancelCause::SessionEnded),
                },
                "The agent's session ended before anything reached the browser.",
            ),
            (
                Refusal::CancelledBeforeDispatch {
                    by: Some(CancelCause::Quit(QuitCause::Quit)),
                },
                "Ghostlight quit before anything reached the browser.",
            ),
            (
                Refusal::CancelledAfterDispatch { by: None },
                "The browser never confirmed what the sent command did.",
            ),
            (
                Refusal::CancelledAfterDispatch {
                    by: Some(CancelCause::Client),
                },
                "The agent cancelled before the browser confirmed what the sent command did.",
            ),
            (
                Refusal::CancelledAfterDispatch {
                    by: Some(CancelCause::SessionEnded),
                },
                "The agent's session ended before the browser confirmed what the sent command did.",
            ),
            (
                Refusal::CancelledAfterDispatch {
                    by: Some(CancelCause::Quit(QuitCause::Quit)),
                },
                "Ghostlight quit before the browser confirmed what the sent command did.",
            ),
            (
                Refusal::EffectUnknown,
                "The browser never confirmed what the sent command did.",
            ),
            (
                Refusal::LandingDeniedUnknown {
            code: crate::governance::ReasonCode::HostDenied,
        },
                "The new tab's final state is unknown after its landing was blocked.",
            ),
            (
                Refusal::WorkspaceUnusable {
                    reason: WorkspaceReason::TabUnavailable,
                },
                "That tab handle names no open tab in this session.",
            ),
            (
                Refusal::WorkspaceUnusable {
                    reason: WorkspaceReason::NoTab,
                },
                "This session has no open tab yet.",
            ),
            (
                Refusal::WorkspaceUnusable {
                    reason: WorkspaceReason::AmbiguousTab,
                },
                "More than one tab is open in this session, so the call must name one.",
            ),
            (
                Refusal::FilesUnreadable {
            problem: crate::language::outcome::FileProblem::Unavailable,
        },
                "The selected local files could not be prepared safely.",
            ),
            (
                Refusal::CaptureTooLarge,
                "The screenshot was too large to return.",
            ),
            (
                Refusal::TabNotShowing,
                "Nothing was captured: another tab is showing in this tab's window, and the browser draws only the tab it shows.",
            ),
            (
                Refusal::NoDialogVisible,
                "No JavaScript dialog is currently visible.",
            ),
            (
                Refusal::DialogOpen,
                "A JavaScript dialog is open on this page, so nothing was sent.",
            ),
            (
                Refusal::NoHistoryEntry { forward: false },
                "This tab has no earlier page in its history, so nothing was sent.",
            ),
            (
                Refusal::NoHistoryEntry { forward: true },
                "This tab has no later page in its history, so nothing was sent.",
            ),
            (
                Refusal::RecordingUnavailable,
                "This session has no recording.",
            ),
            (
                Refusal::RecordingHeld,
                "This session already has a recording.",
            ),
            (
                Refusal::RecordingExportFailed,
                "The recording could not be turned into a replay.",
            ),
            (
                Refusal::RecordingNotAttached,
                "The page did not take the replay at that target, so nothing was attached; the recording is kept.",
            ),
            (
                Refusal::DeadlineExpired {
                    before_dispatch: true,
                    timeout_adjustable: true,
                },
                "The job ran out of time before anything changed in the browser.",
            ),
            (
                Refusal::DeadlineExpired {
                    before_dispatch: false,
                    timeout_adjustable: true,
                },
                "The job ran out of time before the browser confirmed what the sent command did.",
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
            vec![UPDATE_EXTENSION_STEP]
        );
        assert_eq!(
            Refusal::BrowserAdapterOutdated.next_steps(),
            vec![UPDATE_EXTENSION_STEP]
        );
        assert_eq!(
            Refusal::FilesUnreadable {
                problem: FileProblem::Unavailable
            }
            .next_steps(),
            vec!["Confirm each path is an existing file within Ghostlight's upload limits."]
        );
        assert_eq!(
            Refusal::CaptureTooLarge.next_steps(),
            vec!["Capture the viewport or a smaller region instead of the full page."]
        );
        assert_eq!(
            Refusal::TabNotShowing.next_steps(),
            vec!["Call browser_tabs with action focus for this tab, then repeat the screenshot; browser_read and browser_inspect read the page without it."]
        );
        assert_eq!(
            Refusal::LocalInterlock.next_steps(),
            vec!["The tab stays open because the user chose to keep Ghostlight's tabs; continue without closing it."]
        );
        for by in [
            None,
            Some(CancelCause::Client),
            Some(CancelCause::Quit(QuitCause::Quit)),
        ] {
            assert!(Refusal::CancelledBeforeDispatch { by }
                .next_steps()
                .is_empty());
        }
        for quit in QuitCause::ALL {
            let by = CancelCause::Quit(quit);
            let steps = Refusal::CancelledBeforeStart { by }.next_steps();
            if matches!(quit, QuitCause::Uninstall | QuitCause::SignOut) {
                assert!(steps.is_empty(), "{quit:?}");
            } else {
                assert_eq!(steps, vec![REPEAT_IN_A_MOMENT], "{quit:?}");
            }
        }
        for by in [CancelCause::Client, CancelCause::SessionEnded] {
            assert!(Refusal::CancelledBeforeStart { by }.next_steps().is_empty());
        }
        for (reason, step) in [
            (
                BrowserUnavailableReason::NotAnswering,
                "Check that the browser window is open and responsive, then repeat the call.",
            ),
            (
                BrowserUnavailableReason::NotConnected,
                "Ask the user to reopen that browser, then repeat the call.",
            ),
            (
                BrowserUnavailableReason::WrongProfile,
                "Ask the user to reopen the browser profile this session started in, then repeat the call.",
            ),
            (BrowserUnavailableReason::Busy, "Repeat the call."),
        ] {
            assert_eq!(
                Refusal::BrowserUnavailable { reason }.next_steps(),
                vec![step]
            );
        }
        assert_eq!(
            Refusal::BrowserContract.next_steps(),
            vec![UPDATE_EXTENSION_STEP]
        );
        assert_eq!(
            Refusal::EffectUnknown.next_steps(),
            vec![
                "If a JavaScript dialog may be open on the page, handle it with browser_dialog; handling checks the page directly.",
                "Then observe the page with browser_read or browser_inspect to learn what happened.",
            ]
        );

        // Localhost is protected only when an authored policy marks it never-touch.
        let refusal = Refusal::AuthorityBlocked {
            code: crate::governance::ReasonCode::ProtectedHost,
            host: Some("localhost".into()),
            origin: None,
        };
        assert_eq!(
            refusal.summary(),
            "Blocked: localhost is protected and is never automated."
        );
        assert_eq!(refusal.observed().host.as_deref(), Some("localhost"));
        assert!(Refusal::InvalidRequest.observed().host.is_none());
    }

    #[test]
    fn a_channel_refusal_names_the_switch_and_whose_policy_turned_it_off() {
        for (channel, author, expected) in [
            (
                IntakeChannel::Mcp,
                PolicyAuthor::User,
                "Blocked: the user's Ghostlight policy turns off work from MCP agents.",
            ),
            (
                IntakeChannel::Mcp,
                PolicyAuthor::Organization,
                "Blocked: the organization's Ghostlight policy turns off work from MCP agents.",
            ),
            (
                IntakeChannel::Cli,
                PolicyAuthor::User,
                "Blocked: the user's Ghostlight policy turns off work from the command line.",
            ),
            (
                IntakeChannel::Cli,
                PolicyAuthor::Organization,
                "Blocked: the organization's Ghostlight policy turns off work from the command line.",
            ),
        ] {
            let refusal = Refusal::ChannelOff { channel, author };
            assert_eq!(refusal.summary(), expected);
            assert!(refusal.next_steps().is_empty());
            let audit = refusal.audit();
            assert_eq!(audit.summary(), expected);
            assert_eq!(
                serde_json::to_value(audit.refusal()).unwrap(),
                serde_json::json!({"reason": "channel_off", "channel": channel.as_str()})
            );
        }
    }

    /// A primitive adapter refusal speaks the browser's own reason and never masquerades as a
    /// disconnection.
    #[test]
    fn primitive_refusal_carries_the_browser_reason_without_claiming_a_disconnection() {
        let refusal = Refusal::BrowserPrimitive {
            detail: "target is not visible for focus".into(),
            cause: crate::language::outcome::PrimitiveCause::Refused,
        };
        let summary = refusal.summary();
        assert!(summary.contains("target is not visible for focus"));
        assert!(!summary.to_lowercase().contains("disconnected"));
        assert_eq!(
            refusal.next_steps(),
            vec!["Read the browser's stated reason, adjust the call or the page, then repeat."]
        );
    }

    /// A deadline suggests only what the call can actually change: more `timeout_ms` below the
    /// maximum, a narrower request at it, and a look at the page when the action may already have
    /// happened.
    #[test]
    fn deadline_advice_follows_what_the_call_can_change() {
        assert_eq!(
            Refusal::DeadlineExpired {
                before_dispatch: true,
                timeout_adjustable: true,
            }
            .next_steps(),
            vec!["Repeat with a longer timeout_ms when the page genuinely needs more time."]
        );
        let fixed = Refusal::DeadlineExpired {
            before_dispatch: true,
            timeout_adjustable: false,
        }
        .next_steps();
        assert_eq!(
            fixed,
            vec!["Repeat the call; if the page is very large, narrow it first (a target, a smaller max_chars, or a smaller inspect scope)."]
        );
        assert!(!fixed.join(" ").contains("timeout_ms"));
        for timeout_adjustable in [true, false] {
            let after = Refusal::DeadlineExpired {
                before_dispatch: false,
                timeout_adjustable,
            }
            .next_steps();
            assert_eq!(
                after,
                vec!["Check the page with browser_read or browser_screenshot before repeating; the action may already have happened."]
            );
        }
    }

    /// Ghostlight's own control page is named by the address the caller wrote, says other ports
    /// are untouched, and sends no one to policy, which cannot lift it.
    #[test]
    fn the_own_control_page_refusal_names_the_address_and_no_policy_call() {
        for origin in [
            "http://127.0.0.1:41259",
            "http://localhost:41259",
            "http://[::1]:41259",
        ] {
            let refusal = Refusal::AuthorityBlocked {
                code: crate::governance::ReasonCode::OwnWorkbench,
                host: Some("127.0.0.1".into()),
                origin: Some(origin.into()),
            };
            assert_eq!(
                refusal.summary(),
                format!("Blocked: {origin} is Ghostlight's own control page and is never automated. Other ports on this computer are not affected.")
            );
            assert!(refusal.next_steps().is_empty());
            assert_eq!(refusal.observed().host.as_deref(), Some("127.0.0.1"));
        }
        let unnamed = Refusal::AuthorityBlocked {
            code: crate::governance::ReasonCode::OwnWorkbench,
            host: None,
            origin: None,
        };
        assert_eq!(
            unnamed.summary(),
            "Blocked: that address is Ghostlight's own control page and is never automated. Other ports on this computer are not affected."
        );
        let protected = Refusal::AuthorityBlocked {
            code: crate::governance::ReasonCode::ProtectedHost,
            host: Some("localhost".into()),
            origin: None,
        };
        assert_eq!(
            protected.summary(),
            "Blocked: localhost is protected and is never automated."
        );
    }

    /// A policy denial points to the one tool that explains policy. The human controls stay
    /// step-free (ADR-0126), and so do the outages Ghostlight resolves without the caller.
    #[test]
    fn policy_denials_point_to_policy_explain() {
        let explain = vec!["Call policy_explain to see what this session may do.".to_string()];
        for reason in [
            BlockedReason::Host,
            BlockedReason::ProtectedHost,
            BlockedReason::Capability,
            BlockedReason::TabClose,
            BlockedReason::Screenshot,
            BlockedReason::Unspecified,
        ] {
            let refusal = Refusal::AuthorityBlocked {
                code: reason.code(),
                host: Some("example.com".into()),
                origin: None,
            };
            assert_eq!(refusal.next_steps(), explain, "{reason:?}");
        }
        for reason in [
            BlockedReason::AuditUnavailable,
            BlockedReason::InvalidAuthority,
            BlockedReason::Hold,
            BlockedReason::SessionEnded,
        ] {
            let refusal = Refusal::AuthorityBlocked {
                code: reason.code(),
                host: None,
                origin: None,
            };
            assert!(refusal.next_steps().is_empty(), "{reason:?}");
        }
    }

    /// Governance vocabulary stays out of what the model reads.
    #[test]
    fn model_facing_sentences_use_plain_words() {
        let sentences = [
            Outcome::TabsListed { count: 3 }.summary(),
            Outcome::PolicyExplained {
                capabilities: 3,
                layers: 2,
            }
            .summary(),
            Outcome::RecordingBoundary { duration_ms: 3_000 }.summary(),
            Refusal::AuthorityBlocked {
                code: crate::governance::ReasonCode::ScreenshotDenied,
                host: None,
                origin: None,
            }
            .summary(),
        ];
        for sentence in sentences {
            let lower = sentence.to_lowercase();
            for jargon in [
                "authority",
                "admitted",
                "bound",
                "custody",
                "document scope",
                "document boundary",
                "intake",
            ] {
                assert!(!lower.contains(jargon), "{sentence:?} says {jargon:?}");
            }
        }
    }

    /// Every workspace reason names its own way back, or deliberately none.
    #[test]
    fn workspace_reasons_teach_their_own_recovery() {
        assert_eq!(
            WorkspaceReason::NoTab.next_steps(),
            vec!["Call browser_navigate with url to open a page."]
        );
        assert_eq!(
            WorkspaceReason::AmbiguousTab.next_steps(),
            vec!["Repeat the call with tab set to the handle of the tab you mean, listed in tabs."]
        );
        assert_eq!(
            WorkspaceReason::TabUnavailable.next_steps(),
            vec!["Call browser_tabs with action list to obtain current tab handles."]
        );
        for (reason, step) in [
            (
                StaleReason::Removed,
                "Call browser_find or browser_inspect to obtain current target handles.",
            ),
            (
                StaleReason::Navigated,
                "Call browser_inspect to obtain current target handles.",
            ),
            (
                StaleReason::TabClosed,
                "Call browser_tabs to choose an open tab.",
            ),
            (
                StaleReason::Unknown,
                "Call browser_find or browser_inspect to obtain current target handles.",
            ),
        ] {
            assert_eq!(
                WorkspaceReason::StaleTarget {
                    reason,
                    help: StaleHelp::None,
                }
                .next_steps(),
                vec![step]
            );
        }
        assert_eq!(
            WorkspaceReason::StaleView.next_steps(),
            vec!["Call browser_screenshot to obtain a current view handle."]
        );
        assert_eq!(
            WorkspaceReason::TabHeld.next_steps(),
            vec!["Call browser_navigate with the tab and an allowed url to leave the page, or browser_tabs with action close to close the tab."]
        );
        assert_eq!(
            WorkspaceReason::WorkspaceBusy.next_steps(),
            vec!["Wait for the active Ghostlight invocation to finish."]
        );
        assert_eq!(
            WorkspaceReason::OwnershipMismatch { owner: None }.next_steps(),
            vec!["Collect fresh handles from this session, then continue with those."]
        );
        assert_eq!(
            WorkspaceReason::OwnershipMismatch {
                owner: Some("Workspace B".into())
            }
            .next_steps(),
            vec!["Call browser_tabs with action list to see this session's tabs, then continue with those handles."]
        );
        assert_eq!(
            WorkspaceReason::OwnershipMismatch {
                owner: Some("Workspace B".into())
            }
            .summary(),
            "That handle belongs to another Ghostlight session (Workspace B)."
        );
        assert_eq!(
            WorkspaceReason::WorkspaceClosed.next_steps(),
            vec!["Start over; the next call opens a fresh session and old handles will not work."]
        );
    }

    /// An empty-object script value adds one step, worded for whether the script was awaited, and
    /// no measurement or sentence.
    #[test]
    fn an_empty_object_value_adds_one_step_and_nothing_else() {
        let evaluated = |empty_object| Outcome::ScriptEvaluated {
            host: Some("example.com".into()),
            empty_object,
        };
        assert!(evaluated(EmptyObject::Absent).next_steps().is_empty());
        assert_eq!(
            evaluated(EmptyObject::Awaited).next_steps(),
            vec!["The value {} is also how a DOM node, a Map or a class instance reads as JSON. Return plain fields, such as el.textContent or Array.from(map), to read them."]
        );
        assert_eq!(
            evaluated(EmptyObject::NotAwaited).next_steps(),
            vec!["The value {} is also how an unawaited promise, a DOM node or a Map reads as JSON. To wait for a promise, end the script with return await <promise>."]
        );
        for empty_object in [EmptyObject::Awaited, EmptyObject::NotAwaited] {
            assert_eq!(
                evaluated(empty_object).summary(),
                "Executed JavaScript on example.com."
            );
            assert_eq!(
                evaluated(empty_object).observed(),
                evaluated(EmptyObject::Absent).observed()
            );
        }
    }

    /// Outcome guidance leads with the recovery action and stays truthful about partial work.
    #[test]
    fn outcome_next_steps_teach_the_fix() {
        assert_eq!(
            Outcome::TabsListed { count: 0 }.next_steps(),
            vec!["Call browser_navigate with url to open a page."]
        );
        assert!(Outcome::TabsListed { count: 2 }.next_steps().is_empty());
        assert_eq!(
            Outcome::TabAlreadyClosed.next_steps(),
            vec!["Call browser_tabs with action list to see remaining open tabs."]
        );
        let opened = Outcome::PageScrolled {
            host: None,
            direction: crate::language::ScrollDirection::Down,
        }
        .with_open_dialog(DialogKind::Prompt);
        assert_eq!(
            opened.next_steps(),
            vec!["Call browser_dialog with action status to read it, then accept, dismiss, or respond with text for a prompt."]
        );
        assert_eq!(opened.observed(), Observed::default());
        assert_eq!(
            Outcome::TargetsListed {
                noun: TargetNoun::Control,
                count: 0,
                host: None,
                left_out: LeftOut::default(),
            }
            .next_steps(),
            vec!["Try a different search term, scroll to reveal content, or use browser_inspect to view available page controls."]
        );
        assert!(Outcome::TargetsListed {
            noun: TargetNoun::Control,
            count: 3,
            host: None,
            left_out: LeftOut::default(),
        }
        .next_steps()
        .is_empty());
        assert_eq!(
            Outcome::TextRead {
                words: 0,
                host: None,
            }
            .next_steps(),
            vec!["Wait for the page to finish rendering with browser_wait, or inspect controls with browser_inspect."]
        );
        assert!(Outcome::TextRead {
            words: 42,
            host: None,
        }
        .next_steps()
        .is_empty());
        assert_eq!(
            Outcome::DialogObserved {
            present: true,
            message: None,
            prompt: None,
        }.next_steps(),
            vec!["Call browser_dialog with action accept or dismiss (or respond with text for a prompt) before continuing."]
        );
        for field in [PromptField::Holds("hello".into()), PromptField::Unreported] {
            assert_eq!(
                Outcome::DialogObserved {
                    present: true,
                    message: None,
                    prompt: Some(field),
                }
                .next_steps(),
                vec!["Call browser_dialog with action accept to answer with the text in its field, as OK does, respond with your own text, or dismiss, before continuing."]
            );
        }
        assert!(Outcome::DialogObserved {
            present: false,
            message: None,
            prompt: None,
        }
        .next_steps()
        .is_empty());
        assert_eq!(
            Outcome::DocumentInspected {
                nodes: 0,
                truncated: false,
                differences: None,
            }
            .next_steps(),
            vec!["Wait for the page to finish loading with browser_wait, or inspect without a root selector."]
        );
        assert_eq!(
            Outcome::SelectorUnresolved {
                matched: 0,
                more: false
            }
            .next_steps(),
            vec!["Use browser_find with text visible on the page, inspect for fresh handles, or narrow the selector with role and exact."]
        );
        assert_eq!(
            composition(2, 5)
            .next_steps(),
            vec!["Use the per-step results to prepare only unfinished work with current page references."]
        );
        assert!(composition(5, 5).next_steps().is_empty());
        assert_eq!(
            composition(2, 5)
            .next_steps(),
            vec!["Use the per-step results to prepare only unfinished work with current page references."]
        );
        assert_eq!(
            Outcome::Waited {
                condition: WaitCondition::TextPresent,
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
                differences: None,
            }
            .next_steps(),
            vec!["Narrow the subtree root or depth to capture the rest."]
        );
    }

    #[test]
    fn a_stale_handle_says_why_in_one_sentence_per_word() {
        for (reason, summary) in [
            (
                StaleReason::Removed,
                "That target is no longer on the page.",
            ),
            (
                StaleReason::Navigated,
                "That target belongs to an older version of the page.",
            ),
            (StaleReason::TabClosed, "That target's tab is closed."),
            (
                StaleReason::Unknown,
                "Ghostlight does not know that target handle.",
            ),
        ] {
            let stale = WorkspaceReason::StaleTarget {
                reason,
                help: StaleHelp::None,
            };
            assert_eq!(stale.summary(), summary);
            assert_eq!(stale.as_fact(), "stale_target");
            assert_eq!(stale.next_steps().len(), 1);
            assert_eq!(
                Refusal::WorkspaceUnusable { reason: stale }.summary(),
                summary
            );
        }
    }

    #[test]
    fn a_removed_handle_names_its_replacement_or_asks_for_a_pick() {
        let replacement = WorkspaceReason::StaleTarget {
            reason: StaleReason::Removed,
            help: StaleHelp::Replacement {
                tool: "browser_click",
                target: "target_7k2nb5xe".into(),
            },
        };
        assert_eq!(
            replacement.summary(),
            "That target is no longer on the page. One control on the page has the same role, name and row."
        );
        assert_eq!(
            replacement.next_steps(),
            vec!["Repeat browser_click with target target_7k2nb5xe."]
        );
        let candidates = WorkspaceReason::StaleTarget {
            reason: StaleReason::Removed,
            help: StaleHelp::Candidates { named: true },
        };
        assert_eq!(
            candidates.summary(),
            "That target is no longer on the page."
        );
        assert_eq!(
            candidates.next_steps(),
            vec!["Pick a target from recovery_candidates by its context, or call browser_find for current target handles."]
        );
        // Only a removed control has a replacement: every other word keeps its own sentence.
        for reason in [
            StaleReason::Navigated,
            StaleReason::TabClosed,
            StaleReason::Unknown,
        ] {
            let offered = WorkspaceReason::StaleTarget {
                reason,
                help: StaleHelp::Candidates { named: true },
            };
            let bare = WorkspaceReason::StaleTarget {
                reason,
                help: StaleHelp::None,
            };
            assert_eq!(offered.summary(), bare.summary());
            assert_eq!(offered.next_steps(), bare.next_steps());
        }
    }

    #[test]
    fn candidates_without_names_are_picked_by_role_and_nth_not_by_context() {
        let unnamed = WorkspaceReason::StaleTarget {
            reason: StaleReason::Removed,
            help: StaleHelp::Candidates { named: false },
        };
        assert_eq!(unnamed.summary(), "That target is no longer on the page.");
        assert_eq!(
            unnamed.next_steps(),
            vec!["Pick a target from recovery_candidates by its role and nth, or call browser_find for current target handles."]
        );
    }

    #[test]
    fn workspace_errors_map_to_the_refusal_that_says_what_is_true() {
        let unusable = |reason: WorkspaceReason| Refusal::WorkspaceUnusable { reason };
        let examples = [
            (
                WorkspaceError::StaleTab,
                unusable(WorkspaceReason::TabUnavailable),
            ),
            (WorkspaceError::NoTab, unusable(WorkspaceReason::NoTab)),
            (
                WorkspaceError::AmbiguousTab,
                unusable(WorkspaceReason::AmbiguousTab),
            ),
            (
                WorkspaceError::StaleTarget {
                    reason: StaleReason::Removed,
                },
                unusable(WorkspaceReason::StaleTarget {
                    reason: StaleReason::Removed,
                    help: StaleHelp::None,
                }),
            ),
            (
                WorkspaceError::StaleView,
                unusable(WorkspaceReason::StaleView),
            ),
            (
                WorkspaceError::OutsideImage {
                    width: 400,
                    height: 300,
                },
                unusable(WorkspaceReason::OutsideImage(Some(ImageSize {
                    width: 400,
                    height: 300,
                }))),
            ),
            (
                WorkspaceError::FieldsOnSeveralTabs,
                unusable(WorkspaceReason::FieldsOnSeveralTabs),
            ),
            (
                WorkspaceError::Held(crate::workspace::TabHold {
                    denial: crate::governance::Decision::refused(
                        crate::governance::ReasonCode::HostDenied,
                    ),
                    authority: "authority_test".into(),
                    requirements: crate::governance::CapabilitySet::READ,
                    url: "https://Elsewhere.test/path".into(),
                }),
                Refusal::TabHeld {
                    host: Some("elsewhere.test".into()),
                },
            ),
            (WorkspaceError::BrowserPinned, Refusal::BrowserPinned),
            (
                WorkspaceError::Busy,
                unusable(WorkspaceReason::WorkspaceBusy),
            ),
            (
                WorkspaceError::NotOwnedTab,
                unusable(WorkspaceReason::OwnershipMismatch { owner: None }),
            ),
            (
                WorkspaceError::NotOwnedTarget,
                unusable(WorkspaceReason::OwnershipMismatch { owner: None }),
            ),
            (
                WorkspaceError::NotOwnedView,
                unusable(WorkspaceReason::OwnershipMismatch { owner: None }),
            ),
            (
                WorkspaceError::PhysicalTabOwned,
                unusable(WorkspaceReason::OwnershipMismatch { owner: None }),
            ),
            (
                WorkspaceError::UnknownWorkspace,
                unusable(WorkspaceReason::WorkspaceClosed),
            ),
        ];
        for (error, expected) in examples {
            let (refusal, fact) = workspace_refusal(error);
            let expected_fact = match &expected {
                Refusal::WorkspaceUnusable { reason } => reason.as_fact(),
                Refusal::TabHeld { .. } => "tab_held",
                _ => "browser_pinned",
            };
            assert_eq!(refusal, expected);
            assert_eq!(fact, expected_fact);
        }
        assert_eq!(WorkspaceReason::TabUnavailable.as_fact(), "tab_unavailable");
        assert_eq!(WorkspaceReason::NoTab.as_fact(), "no_tab");
        assert_eq!(WorkspaceReason::AmbiguousTab.as_fact(), "ambiguous_tab");
        // History stores the same names the facts use, and none of the carried tab or size.
        for reason in [
            WorkspaceReason::NoTab,
            WorkspaceReason::AmbiguousTab,
            WorkspaceReason::TabUnavailable,
            WorkspaceReason::HandleOnOtherTab(Some("tab_7k2nb5xe".into())),
            WorkspaceReason::FieldsOnSeveralTabs,
            WorkspaceReason::OutsideImage(Some(ImageSize {
                width: 400,
                height: 300,
            })),
        ] {
            assert_eq!(serde_json::to_value(&reason).unwrap(), reason.as_fact());
        }
    }

    #[test]
    fn a_handle_on_another_tab_a_split_fill_and_a_point_off_the_image_each_say_so() {
        let other = WorkspaceReason::HandleOnOtherTab(Some("tab_7k2nb5xe".into()));
        assert_eq!(
            other.summary(),
            "That handle belongs to tab_7k2nb5xe, another of this session's tabs."
        );
        assert_eq!(
            other.next_steps(),
            vec!["Repeat the call with handles from one tab, and with tab left out or set to that tab."]
        );
        assert_eq!(
            WorkspaceReason::HandleOnOtherTab(None).summary(),
            "That handle belongs to another of this session's tabs."
        );
        assert_eq!(
            WorkspaceReason::FieldsOnSeveralTabs.summary(),
            "A fill works in one tab, and these targets are on more than one."
        );
        assert_eq!(
            WorkspaceReason::FieldsOnSeveralTabs.next_steps(),
            vec!["Make one browser_fill_form call for each tab."]
        );
        let outside = WorkspaceReason::OutsideImage(Some(ImageSize {
            width: 400,
            height: 300,
        }));
        assert_eq!(
            outside.summary(),
            "Those coordinates are outside the view's image, which is 400 by 300 pixels."
        );
        assert_eq!(
            outside.next_steps(),
            vec!["Repeat with coordinates inside the image, or call browser_screenshot for a new view."]
        );
        assert_eq!(
            WorkspaceReason::TabUnavailable.summary(),
            "That tab handle names no open tab in this session."
        );
    }

    const UPDATE_EXTENSION_STEP: &str = "Ask the user to update Ghostlight in Browser (to update now, open chrome://extensions, turn on Developer mode and click Update), then repeat the call.";

    fn store_step() -> String {
        format!(
            "Ask the user to add Ghostlight in Browser from {}, or to open the Ghostlight window on this computer for the setup steps, then repeat the call.",
            crate::language::setup::STORE_LISTING_URL
        )
    }

    #[test]
    fn advice_is_given_only_while_setup_is_incomplete_and_no_profile_has_the_extension() {
        assert_eq!(
            ExtensionAdvice::resolve(false, false),
            ExtensionAdvice::AddFromStore
        );
        for (complete, present) in [(false, true), (true, false), (true, true)] {
            assert_eq!(
                ExtensionAdvice::resolve(complete, present),
                ExtensionAdvice::Plain
            );
        }
    }

    #[test]
    fn the_no_browser_answer_adds_the_store_step_only_when_advised() {
        let plain = Refusal::BrowserStartupManual {
            browsers: vec!["Chrome".into()],
            extension: ExtensionAdvice::Plain,
        };
        let advised = Refusal::BrowserStartupManual {
            browsers: vec!["Chrome".into()],
            extension: ExtensionAdvice::AddFromStore,
        };
        assert!(plain.next_steps().is_empty());
        assert_eq!(advised.next_steps(), vec![store_step()]);
        assert_eq!(plain.summary(), advised.summary());
    }

    #[test]
    fn an_absent_extension_sends_the_person_to_the_store_only_when_advised() {
        let absent = |extension| Refusal::BrowserRecoveryFailed {
            reason: BrowserRecoveryReason::ExtensionAbsent,
            extension,
        };
        assert_eq!(
            absent(ExtensionAdvice::Plain).next_steps(),
            vec!["Install Ghostlight in the selected browser profile, then repeat the call."]
        );
        assert_eq!(
            absent(ExtensionAdvice::AddFromStore).next_steps(),
            vec![store_step()]
        );
        assert_eq!(
            absent(ExtensionAdvice::Plain).summary(),
            absent(ExtensionAdvice::AddFromStore).summary()
        );
    }

    #[test]
    fn a_launched_browser_whose_adapter_never_arrived_sends_the_person_to_the_store_only_when_advised(
    ) {
        let timed_out = |extension| Refusal::BrowserRecoveryFailed {
            reason: BrowserRecoveryReason::HandshakeTimeout,
            extension,
        };
        assert_eq!(
            timed_out(ExtensionAdvice::Plain).next_steps(),
            vec!["Ask the user to open a window of that browser with the Ghostlight extension installed, then repeat the call."]
        );
        assert_eq!(
            timed_out(ExtensionAdvice::AddFromStore).next_steps(),
            vec![store_step()]
        );
        assert_eq!(
            timed_out(ExtensionAdvice::Plain).summary(),
            timed_out(ExtensionAdvice::AddFromStore).summary()
        );
    }

    #[test]
    fn advice_changes_no_other_recovery_failure() {
        for reason in [
            BrowserRecoveryReason::BrowserAbsent,
            BrowserRecoveryReason::LaunchFailed,
            BrowserRecoveryReason::SandboxedPackage,
            BrowserRecoveryReason::NativeHostUnavailable,
            BrowserRecoveryReason::OwnedElsewhere,
        ] {
            let with =
                |extension| Refusal::BrowserRecoveryFailed { reason, extension }.next_steps();
            assert_eq!(
                with(ExtensionAdvice::Plain),
                with(ExtensionAdvice::AddFromStore)
            );
        }
    }

    #[test]
    fn cross_tree_ownership_refusal_teaches_the_deliberate_install() {
        let refusal = Refusal::BrowserRecoveryFailed {
            reason: super::BrowserRecoveryReason::OwnedElsewhere,
            extension: ExtensionAdvice::AddFromStore,
        };
        assert_eq!(
            refusal.summary(),
            "Another Ghostlight installation owns the browser registration."
        );
        assert_eq!(
            refusal.next_steps(),
            vec![
                "Ask the user to run ghostlight install from the Ghostlight installation that should own the browsers, then repeat the call."
                    .to_owned()
            ]
        );
    }

    #[test]
    fn observed_json_shape_round_trips_unchanged() {
        let observed = Observed {
            host: Some("example.com".into()),
            readiness: Some("complete".into()),
            count: Some(3),
            width: Some(1280),
            height: Some(720),
            rebound: false,
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

    fn listed(noun: TargetNoun, cut: &[(LeftOutKey, usize)]) -> Outcome {
        let mut left_out = LeftOut::default();
        for (key, amount) in cut {
            left_out.add(*key, *amount);
        }
        Outcome::TargetsListed {
            noun,
            count: 80,
            host: Some("news.example".into()),
            left_out,
        }
    }

    #[test]
    fn a_cut_listing_keeps_its_summary_and_adds_one_step() {
        let outcome = listed(TargetNoun::Control, &[(LeftOutKey::Chrome, 96)]);
        assert_eq!(outcome.summary(), "Found 80 controls on news.example.");
        assert_eq!(
            outcome.next_steps(),
            vec![
                "96 header, nav and footer controls were left out to fit; raise max_items (up to 200) or use browser_find for one by name."
            ]
        );
    }

    #[test]
    fn the_step_names_the_tier_that_matters_most_among_those_cut() {
        let both = listed(
            TargetNoun::Item,
            &[(LeftOutKey::Chrome, 4), (LeftOutKey::Content, 1)],
        );
        assert_eq!(
            both.next_steps(),
            vec!["1 page content item was left out to fit; raise max_items (up to 200) or use browser_find for one by name."]
        );
        let aside = listed(TargetNoun::Control, &[(LeftOutKey::Aside, 12)]);
        assert_eq!(
            aside.next_steps(),
            vec!["12 aside controls were left out to fit; raise max_items (up to 200) or use browser_find for one by name."]
        );
    }

    #[test]
    fn hidden_controls_a_dialog_and_a_find_get_no_cut_step() {
        for outcome in [
            listed(TargetNoun::Control, &[(LeftOutKey::Hidden, 30)]),
            listed(TargetNoun::Control, &[(LeftOutKey::BehindDialog, 30)]),
            listed(TargetNoun::Match, &[(LeftOutKey::Content, 30)]),
            listed(TargetNoun::Control, &[]),
        ] {
            assert!(outcome.next_steps().is_empty(), "{outcome:?}");
        }
    }

    #[test]
    fn a_cut_listing_audits_the_same_count_as_a_whole_one() {
        let cut = listed(TargetNoun::Control, &[(LeftOutKey::Chrome, 96)]);
        let whole = listed(TargetNoun::Control, &[]);
        assert_eq!(cut.observed(), whole.observed());
    }

    /// One refusal of every variant, and of every closed cause that changes its model code.
    fn every_refusal() -> Vec<Refusal> {
        use crate::governance::ReasonCode;
        use crate::language::page_refusal::{PageGesture, PageRefusal};
        let mut all = vec![
            Refusal::Capacity,
            Refusal::InvalidRequest,
            Refusal::CancelledBeforeStart {
                by: CancelCause::Client,
            },
            Refusal::DeadlineBeforeStart,
            Refusal::AuthorityBlocked {
                code: ReasonCode::HostDenied,
                host: None,
                origin: None,
            },
            Refusal::AuthorityBlocked {
                code: ReasonCode::ChannelDenied,
                host: None,
                origin: None,
            },
            Refusal::ChannelOff {
                channel: IntakeChannel::Mcp,
                author: PolicyAuthor::User,
            },
            Refusal::LocalInterlock,
            Refusal::CredentialAuthorization {
                control: ControlRef::new(TargetRole::Textbox, "Password", None, true),
            },
            Refusal::IncompatibleReceipt,
            Refusal::BrowserAdapterOutdated,
            Refusal::DeadlineExpired {
                before_dispatch: true,
                timeout_adjustable: false,
            },
            Refusal::PageRefused {
                refusal: PageRefusal::Disabled,
                gesture: PageGesture::Other,
            },
            Refusal::FieldsReverted {
                set: 1,
                reverted: vec![],
            },
            Refusal::FieldValuesRejected {
                fields: vec![],
                total: 1,
            },
            Refusal::DocumentUnavailable,
            Refusal::CancelledBeforeDispatch { by: None },
            Refusal::BrowserContract,
            Refusal::BrowserAmbiguous,
            Refusal::BrowserUnknown,
            Refusal::BrowserPinned,
            Refusal::BrowserStartupManual {
                browsers: vec![],
                extension: ExtensionAdvice::Plain,
            },
            Refusal::ConnectionLost,
            Refusal::CancelledAfterDispatch { by: None },
            Refusal::EffectUnknown,
            Refusal::LandingDeniedUnknown {
                code: ReasonCode::HostDenied,
            },
            Refusal::WorkspaceUnusable {
                reason: WorkspaceReason::NoTab,
            },
            Refusal::TabHeld { host: None },
            Refusal::CaptureTooLarge,
            Refusal::TabNotShowing,
            Refusal::NoDialogVisible,
            Refusal::DialogOpen,
            Refusal::NoHistoryEntry { forward: false },
            Refusal::RecordingUnavailable,
            Refusal::RecordingHeld,
            Refusal::RecordingExportFailed,
            Refusal::RecordingNotAttached,
        ];
        for cause in [
            PrimitiveCause::Refused,
            PrimitiveCause::DebuggerUnavailable,
            PrimitiveCause::SessionLost,
        ] {
            all.push(Refusal::BrowserPrimitive {
                detail: "Frame gone".into(),
                cause,
            });
        }
        for reason in [
            BrowserUnavailableReason::NotAnswering,
            BrowserUnavailableReason::NotConnected,
            BrowserUnavailableReason::WrongProfile,
            BrowserUnavailableReason::Busy,
        ] {
            all.push(Refusal::BrowserUnavailable { reason });
        }
        for reason in [
            BrowserRecoveryReason::BrowserAbsent,
            BrowserRecoveryReason::LaunchFailed,
            BrowserRecoveryReason::SandboxedPackage,
            BrowserRecoveryReason::ExtensionAbsent,
            BrowserRecoveryReason::NativeHostUnavailable,
            BrowserRecoveryReason::OwnedElsewhere,
            BrowserRecoveryReason::HandshakeTimeout,
        ] {
            all.push(Refusal::BrowserRecoveryFailed {
                reason,
                extension: ExtensionAdvice::Plain,
            });
        }
        for problem in [
            FileProblem::Unavailable,
            FileProblem::NotAFile,
            FileProblem::FileTooLarge,
            FileProblem::UploadTooLarge,
            FileProblem::ReadFailed,
            FileProblem::Changed,
            FileProblem::InvalidName,
            FileProblem::NotBase64,
            FileProblem::StaleCapture,
        ] {
            all.push(Refusal::FilesUnreadable { problem });
        }
        for cause in [
            CheckFailure::Unmet,
            CheckFailure::Blocked(BlockedReason::Capability),
            CheckFailure::Cancelled,
            CheckFailure::DeadlineExpired,
            CheckFailure::Disconnected,
            CheckFailure::BrowserUnavailable,
            CheckFailure::DocumentUnavailable,
            CheckFailure::BrowserRefused,
        ] {
            all.push(Refusal::ActionUnconfirmed {
                action: Box::new(Outcome::DialogHandled { accepted: true }),
                cause,
                code: cause.decision_code(),
            });
        }
        all
    }

    /// The name of a refusal's variant. The match is exhaustive, so a new variant fails to
    /// compile here until the table below gives it a model code and an audit tag.
    const fn variant_name(refusal: &Refusal) -> &'static str {
        match refusal {
            Refusal::Capacity => "Capacity",
            Refusal::InvalidRequest => "InvalidRequest",
            Refusal::CancelledBeforeStart { .. } => "CancelledBeforeStart",
            Refusal::DeadlineBeforeStart => "DeadlineBeforeStart",
            Refusal::AuthorityBlocked { .. } => "AuthorityBlocked",
            Refusal::ChannelOff { .. } => "ChannelOff",
            Refusal::LocalInterlock => "LocalInterlock",
            Refusal::CredentialAuthorization { .. } => "CredentialAuthorization",
            Refusal::IncompatibleReceipt => "IncompatibleReceipt",
            Refusal::BrowserAdapterOutdated => "BrowserAdapterOutdated",
            Refusal::DeadlineExpired { .. } => "DeadlineExpired",
            Refusal::BrowserPrimitive { .. } => "BrowserPrimitive",
            Refusal::PageRefused { .. } => "PageRefused",
            Refusal::FieldsReverted { .. } => "FieldsReverted",
            Refusal::FieldValuesRejected { .. } => "FieldValuesRejected",
            Refusal::DocumentUnavailable => "DocumentUnavailable",
            Refusal::CancelledBeforeDispatch { .. } => "CancelledBeforeDispatch",
            Refusal::BrowserUnavailable { .. } => "BrowserUnavailable",
            Refusal::BrowserContract => "BrowserContract",
            Refusal::BrowserAmbiguous => "BrowserAmbiguous",
            Refusal::BrowserUnknown => "BrowserUnknown",
            Refusal::BrowserPinned => "BrowserPinned",
            Refusal::BrowserStartupManual { .. } => "BrowserStartupManual",
            Refusal::BrowserRecoveryFailed { .. } => "BrowserRecoveryFailed",
            Refusal::ConnectionLost => "ConnectionLost",
            Refusal::CancelledAfterDispatch { .. } => "CancelledAfterDispatch",
            Refusal::EffectUnknown => "EffectUnknown",
            Refusal::LandingDeniedUnknown { .. } => "LandingDeniedUnknown",
            Refusal::WorkspaceUnusable { .. } => "WorkspaceUnusable",
            Refusal::TabHeld { .. } => "TabHeld",
            Refusal::FilesUnreadable { .. } => "FilesUnreadable",
            Refusal::CaptureTooLarge => "CaptureTooLarge",
            Refusal::TabNotShowing => "TabNotShowing",
            Refusal::NoDialogVisible => "NoDialogVisible",
            Refusal::DialogOpen => "DialogOpen",
            Refusal::NoHistoryEntry { .. } => "NoHistoryEntry",
            Refusal::RecordingUnavailable => "RecordingUnavailable",
            Refusal::RecordingHeld => "RecordingHeld",
            Refusal::RecordingExportFailed => "RecordingExportFailed",
            Refusal::RecordingNotAttached => "RecordingNotAttached",
            Refusal::ActionUnconfirmed { .. } => "ActionUnconfirmed",
        }
    }

    /// How many variants `Refusal` has; `variant_name` names each once.
    const REFUSAL_VARIANTS: usize = 41;

    fn audit_tag(refusal: &Refusal) -> String {
        let tag = serde_json::to_value(refusal.audit().refusal()).expect("an audit tag encodes");
        tag["reason"]
            .as_str()
            .expect("an audit tag has a reason")
            .to_owned()
    }

    /// Each refusal has one closed model code, the `reason` fact its result carries, and one
    /// audit tag, the closed fact durable history keeps. The two spell some refusals differently;
    /// aligning them is a separate, visible choice (ADR-0203 Decision 5.1).
    #[test]
    fn every_refusal_has_one_model_code_and_one_audit_tag() {
        let expected = [
            ("Capacity", "capacity", "capacity"),
            ("InvalidRequest", "invalid_input", "invalid_request"),
            (
                "CancelledBeforeStart",
                "cancelled",
                "cancelled_before_start",
            ),
            ("DeadlineBeforeStart", "deadline", "deadline_before_start"),
            ("AuthorityBlocked", "host_denied", "authority_blocked"),
            ("AuthorityBlocked", "channel_denied", "authority_blocked"),
            ("ChannelOff", "channel_denied", "channel_off"),
            (
                "LocalInterlock",
                "browser_local_interlock",
                "local_interlock",
            ),
            (
                "CredentialAuthorization",
                "credential_authorization_required",
                "credential_authorization",
            ),
            (
                "IncompatibleReceipt",
                "incompatible_browser_receipt",
                "incompatible_receipt",
            ),
            (
                "BrowserAdapterOutdated",
                "browser_contract_failed",
                "browser_adapter_outdated",
            ),
            ("DeadlineExpired", "deadline", "deadline_expired"),
            ("PageRefused", "disabled", "page_refused"),
            ("FieldsReverted", "fields_reverted", "fields_reverted"),
            (
                "FieldValuesRejected",
                "field_value_rejected",
                "field_values_rejected",
            ),
            (
                "DocumentUnavailable",
                "document_unavailable",
                "document_unavailable",
            ),
            (
                "CancelledBeforeDispatch",
                "cancelled",
                "cancelled_before_dispatch",
            ),
            (
                "BrowserContract",
                "browser_contract_failed",
                "browser_contract",
            ),
            ("BrowserAmbiguous", "browser_ambiguous", "browser_ambiguous"),
            ("BrowserUnknown", "browser_unknown", "browser_unknown"),
            ("BrowserPinned", "browser_pinned", "browser_pinned"),
            (
                "BrowserStartupManual",
                "browser_startup_manual",
                "browser_startup_manual",
            ),
            (
                "ConnectionLost",
                "browser_effect_unknown",
                "connection_lost",
            ),
            (
                "CancelledAfterDispatch",
                "browser_effect_unknown",
                "cancelled_after_dispatch",
            ),
            ("EffectUnknown", "browser_effect_unknown", "effect_unknown"),
            (
                "LandingDeniedUnknown",
                "host_denied",
                "landing_denied_unknown",
            ),
            ("WorkspaceUnusable", "no_tab", "workspace_unusable"),
            ("TabHeld", "tab_held", "workspace_unusable"),
            (
                "CaptureTooLarge",
                "screenshot_too_large",
                "capture_too_large",
            ),
            ("TabNotShowing", "tab_not_showing", "tab_not_showing"),
            ("NoDialogVisible", "no_dialog_visible", "no_dialog_visible"),
            ("DialogOpen", "dialog_open", "dialog_open"),
            ("NoHistoryEntry", "no_history_entry", "no_history_entry"),
            ("RecordingUnavailable", "not_found", "recording_unavailable"),
            ("RecordingHeld", "held", "recording_held"),
            (
                "RecordingExportFailed",
                "recording_export_failed",
                "recording_export_failed",
            ),
            (
                "RecordingNotAttached",
                "recording_not_attached",
                "recording_not_attached",
            ),
            (
                "BrowserPrimitive",
                "browser_primitive_failed",
                "browser_primitive_failed",
            ),
            (
                "BrowserPrimitive",
                "debugger_unavailable",
                "browser_primitive_failed",
            ),
            (
                "BrowserPrimitive",
                "session_lost",
                "browser_primitive_failed",
            ),
            (
                "BrowserUnavailable",
                "browser_not_answering",
                "browser_unavailable",
            ),
            (
                "BrowserUnavailable",
                "browser_disconnected",
                "browser_unavailable",
            ),
            (
                "BrowserUnavailable",
                "browser_wrong_profile",
                "browser_unavailable",
            ),
            ("BrowserUnavailable", "browser_busy", "browser_unavailable"),
            (
                "BrowserRecoveryFailed",
                "browser_absent",
                "browser_recovery_failed",
            ),
            (
                "BrowserRecoveryFailed",
                "browser_launch_failed",
                "browser_recovery_failed",
            ),
            (
                "BrowserRecoveryFailed",
                "browser_sandboxed",
                "browser_recovery_failed",
            ),
            (
                "BrowserRecoveryFailed",
                "browser_extension_absent",
                "browser_recovery_failed",
            ),
            (
                "BrowserRecoveryFailed",
                "native_host_unavailable",
                "browser_recovery_failed",
            ),
            (
                "BrowserRecoveryFailed",
                "native_host_owned_elsewhere",
                "browser_recovery_failed",
            ),
            (
                "BrowserRecoveryFailed",
                "browser_handshake_timeout",
                "browser_recovery_failed",
            ),
            ("FilesUnreadable", "file_unavailable", "files_unreadable"),
            ("FilesUnreadable", "not_a_file", "files_unreadable"),
            ("FilesUnreadable", "file_too_large", "files_unreadable"),
            ("FilesUnreadable", "upload_too_large", "files_unreadable"),
            ("FilesUnreadable", "file_read_failed", "files_unreadable"),
            ("FilesUnreadable", "file_changed", "files_unreadable"),
            ("FilesUnreadable", "invalid_file_name", "files_unreadable"),
            (
                "FilesUnreadable",
                "inline_file_not_base64",
                "files_unreadable",
            ),
            ("FilesUnreadable", "capture_stale", "files_unreadable"),
            (
                "ActionUnconfirmed",
                "expectation_unmet",
                "action_unconfirmed",
            ),
            (
                "ActionUnconfirmed",
                "capability_denied",
                "action_unconfirmed",
            ),
            ("ActionUnconfirmed", "cancelled", "action_unconfirmed"),
            ("ActionUnconfirmed", "deadline", "action_unconfirmed"),
            (
                "ActionUnconfirmed",
                "browser_disconnected",
                "action_unconfirmed",
            ),
            (
                "ActionUnconfirmed",
                "browser_unavailable",
                "action_unconfirmed",
            ),
            (
                "ActionUnconfirmed",
                "document_unavailable",
                "action_unconfirmed",
            ),
            ("ActionUnconfirmed", "browser_refused", "action_unconfirmed"),
        ];
        let refusals = every_refusal();
        assert_eq!(refusals.len(), expected.len());
        for (refusal, (variant, code, tag)) in refusals.iter().zip(expected) {
            assert_eq!(variant_name(refusal), variant);
            assert_eq!(refusal.reason(), code, "{variant}");
            assert_eq!(audit_tag(refusal), tag, "{variant}");
        }
        let named: std::collections::HashSet<&str> = refusals.iter().map(variant_name).collect();
        assert_eq!(named.len(), REFUSAL_VARIANTS, "every variant has a row");
    }

    /// A browser refusal with a closed cause says what happened in Ghostlight's words; only the
    /// open refusal repeats the browser's text, and history keeps none of it.
    #[test]
    fn a_closed_primitive_cause_reads_its_own_sentence() {
        for cause in [
            PrimitiveCause::DebuggerUnavailable,
            PrimitiveCause::SessionLost,
        ] {
            let refusal = Refusal::BrowserPrimitive {
                detail: "Cannot access a chrome:// URL".into(),
                cause,
            };
            assert!(!refusal.summary().contains("chrome://"), "{cause:?}");
            assert_eq!(refusal.next_steps().len(), 1, "{cause:?}");
            assert!(
                !refusal.audit().summary().contains("chrome://"),
                "{cause:?}"
            );
        }
        let open = Refusal::BrowserPrimitive {
            detail: "Frame gone".into(),
            cause: PrimitiveCause::Refused,
        };
        assert_eq!(open.summary(), "The browser refused this job: Frame gone.");
    }

    /// The recovery words are the browser recovery's own fact words.
    #[test]
    fn recovery_reasons_say_the_recovery_fact_words() {
        use crate::browser::recovery::RecoveryFailure;
        for (reason, failure) in [
            (
                BrowserRecoveryReason::BrowserAbsent,
                RecoveryFailure::BrowserAbsent,
            ),
            (
                BrowserRecoveryReason::LaunchFailed,
                RecoveryFailure::LaunchFailed,
            ),
            (
                BrowserRecoveryReason::SandboxedPackage,
                RecoveryFailure::SandboxedPackage,
            ),
            (
                BrowserRecoveryReason::ExtensionAbsent,
                RecoveryFailure::ExtensionAbsent,
            ),
            (
                BrowserRecoveryReason::NativeHostUnavailable,
                RecoveryFailure::NativeHostUnavailable,
            ),
            (
                BrowserRecoveryReason::OwnedElsewhere,
                RecoveryFailure::OwnedElsewhere,
            ),
            (
                BrowserRecoveryReason::HandshakeTimeout,
                RecoveryFailure::HandshakeTimeout,
            ),
        ] {
            assert_eq!(reason.fact(), failure.as_str());
        }
    }
}
