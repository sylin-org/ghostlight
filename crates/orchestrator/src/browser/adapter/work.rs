//! The service's internal browser work vocabulary.
//!
//! The executor, workspaces, presentation and recovery describe browser work in these types:
//! a [`BrowserCommand`] in, a [`BrowserOutcome`] back, and [`BrowserEvent`]s between calls. They
//! carry mechanism only. The Chromium adapter translates them into adapter protocol 4 primitives
//! and back, so nothing here is a wire format.

use serde::{Deserialize, Serialize};

use super::documents;
use crate::language::area::LeftOut;
use crate::language::field_echo::ChoiceMatch;

/// The service's internal browser work vocabulary, by its executor name.
pub type Work = BrowserCommand;

/// Browser-local readiness observed by the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserReadiness {
    /// A document is still loading.
    Loading,
    /// The document is interactive.
    Interactive,
    /// The document reported complete.
    Complete,
    /// The adapter cannot determine readiness.
    Unknown,
}

/// A physical Chromium tab fact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalTab {
    /// Chromium tab id.
    pub tab_id: u64,
    /// Bounded browser-supplied title.
    pub title: String,
    /// Observed absolute URL.
    pub url: String,
    /// Whether Chromium reports the tab active.
    pub active: bool,
    /// Current browser-local readiness.
    pub readiness: BrowserReadiness,
}

/// One semantic target observed by the browser adapter.
///
/// The five identity keys are always present. The rest describe a control and appear only when
/// they apply, so an absent key means the page offered nothing of that kind. A credential-class
/// control carries `value` (masked) and none of the others.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedTarget {
    /// Browser-local locator, never exposed to the model.
    pub locator: String,
    /// Semantic or accessibility role.
    pub role: String,
    /// Bounded accessible name.
    pub name: String,
    /// Bounded semantic state labels.
    #[serde(default)]
    pub state: Vec<String>,
    /// Whether the browser classifies this as a credential field.
    pub credential_class: bool,
    /// What the control holds: a string, or an array of strings for a multiple select.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// The full length of `value` when the page cut it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_chars: Option<usize>,
    /// The options of a native select, in document order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
    /// The full option count when the page cut the list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_total: Option<usize>,
    /// The input type, when it is one a role does not imply.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    /// The constraints the page declared, keyed by attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<serde_json::Value>,
    /// Page text that tells what the control expects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// The fieldset or labelled group a radio or checkbox belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Why the control is invalid, when it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The page area the item sits in, as the page words it. The service keeps only the words
    /// of the closed area list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    /// The ordinal of the form that owns the item, counted from 1 in the document it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<u32>,
}

/// The browser-observed identity of the element an action actually used.
///
/// This is deliberately smaller than [`ObservedTarget`]. It carries no locator, state, or
/// credential metadata, and it travels only in the receipt for the action it describes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalActionSubject {
    /// Semantic or accessibility role observed at the action boundary.
    pub role: String,
    /// Bounded accessible name observed at the action boundary.
    pub name: String,
}

/// What a fill gives one control, as the page receives it.
///
/// A JSON boolean stays a boolean so a radio can tell it from a label that reads `true`. A list
/// is for a multiple select. Text is everything else, numbers included.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PhysicalValue {
    /// Checkbox or radio truth.
    Flag(bool),
    /// Literal text, a select option, or a number written out.
    Text(String),
    /// The options a multiple select ends up holding.
    Choices(Vec<String>),
}

impl PhysicalValue {
    /// The text a control that takes one value is given: the text itself, or the word for a
    /// boolean. A list has none.
    #[must_use]
    pub fn text(&self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text.clone()),
            Self::Flag(flag) => Some(flag.to_string()),
            Self::Choices(_) => None,
        }
    }

    /// The part of the value a sentence about one entry quotes: the entry of a list at `index`,
    /// or the whole value written out, a list joined by commas.
    #[must_use]
    pub fn quoted(&self, index: Option<usize>) -> String {
        match (self, index) {
            (Self::Choices(items), Some(index)) => items.get(index).cloned().unwrap_or_default(),
            (Self::Choices(items), None) => items.join(", "),
            (other, _) => other.text().unwrap_or_default(),
        }
    }
}

impl From<&str> for PhysicalValue {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

/// A physical fill value sent only after credential preflight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalField {
    /// Browser-local locator.
    pub locator: String,
    /// Input value, never retained in receipts or diagnostics.
    pub value: PhysicalValue,
}

/// What the page holds in one filled field after the fill, as the page reports it.
///
/// Only choice and typed controls, and text fields the page reformatted, are reported. A
/// credential-class control never is.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldNow {
    /// The field's position in the call's list of fields.
    pub field: usize,
    /// The value the page holds, bounded by the page.
    pub now: String,
    /// Whether the page holds the given value in another format, such as a masked phone number.
    #[serde(default)]
    pub tolerated: bool,
    /// How the given value matched a choice when it was not the option's exact text, value or
    /// label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched: Option<ChoiceMatch>,
    /// The label of the sibling radio the fill picked in this radio's place, when the value
    /// named another radio of the group. The field's own `now` then reads `unchecked`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picked: Option<String>,
    /// The name of the choice group of a radio picked through a sibling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// One physical point in page CSS coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalPoint {
    /// Horizontal page coordinate.
    pub x: f64,
    /// Vertical page coordinate.
    pub y: f64,
}

/// One physical rectangle in page CSS coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalRectangle {
    /// Horizontal page coordinate of the rectangle origin.
    pub x: f64,
    /// Vertical page coordinate of the rectangle origin.
    pub y: f64,
    /// Rectangle width in page CSS pixels.
    pub width: f64,
    /// Rectangle height in page CSS pixels.
    pub height: f64,
}

/// The browser geometry used to render one screenshot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewportGeometry {
    /// Capture scope used to interpret the clip.
    pub scope: CaptureScope,
    /// Horizontal CSS page origin of the capture.
    pub page_x: f64,
    /// Vertical CSS page origin of the capture.
    pub page_y: f64,
    /// Captured CSS width before output scaling.
    pub css_width: f64,
    /// Captured CSS height before output scaling.
    pub css_height: f64,
    /// Visual viewport horizontal page origin at capture time.
    pub visual_page_x: f64,
    /// Visual viewport vertical page origin at capture time.
    pub visual_page_y: f64,
    /// Visual viewport CSS width at capture time.
    pub visual_css_width: f64,
    /// Visual viewport CSS height at capture time.
    pub visual_css_height: f64,
    /// Browser device scale factor.
    pub device_scale: f64,
    /// Current tab zoom factor.
    pub zoom: f64,
    /// Scale applied to produce the returned image dimensions.
    pub output_scale: f64,
}

/// Physical screenshot scope used for coordinate validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureScope {
    /// Current visual viewport.
    Viewport,
    /// Full document surface.
    FullPage,
    /// One semantic target rectangle.
    Target,
    /// One rectangle resolved from a current screenshot view.
    Region,
}

/// One bounded file payload selected by the orchestrator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalFile {
    /// Base filename without a local path.
    pub name: String,
    /// Conservative media type or `application/octet-stream`.
    pub media_type: String,
    /// Base64-encoded bytes.
    pub data: String,
    /// Decoded byte count for receipt validation.
    pub size: u64,
}

/// Browser diagnostic sources selected at the physical adapter boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSource {
    /// Return console and network evidence.
    Both,
    /// Return console evidence only.
    Console,
    /// Return network evidence only.
    Network,
}

/// Browser diagnostic detail selected at the physical adapter boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticDetail {
    /// Return warnings, errors, exceptions, and failed HTTP activity.
    Problems,
    /// Return every retained bounded diagnostic entry.
    All,
}

/// One bounded volatile diagnostic observation from Chromium.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "snake_case")]
pub enum DiagnosticEntry {
    /// One console call or uncaught exception.
    Console {
        /// Opaque cursor for this retained observation.
        cursor: String,
        /// Adapter-wall-clock observation time.
        timestamp_ms: u64,
        /// Bounded Chromium console level.
        level: String,
        /// Bounded untrusted console text.
        text: String,
        /// Sanitized source origin and path, or `invalid:` when Chromium cannot prove provenance.
        url: String,
    },
    /// One sanitized network request observation.
    Network {
        /// Opaque cursor for this retained observation.
        cursor: String,
        /// Adapter-wall-clock observation time.
        timestamp_ms: u64,
        /// Bounded HTTP method.
        method: String,
        /// URL reduced to origin and path, without userinfo, query, or fragment.
        url: String,
        /// Bounded Chromium resource kind.
        resource_type: String,
        /// HTTP response status when observed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
        /// Bounded physical failure category when loading failed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure: Option<String>,
    },
}

/// Extension-owned recording lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    /// Chromium capture is active.
    Recording,
    /// Capture stopped normally and frames remain temporarily available.
    Frozen,
    /// Capture stopped through a physical safety path and retained partial frames.
    Interrupted,
}

/// Why an extension-owned recording stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStopReason {
    /// The physical document scope changed before further imagery could be admitted.
    DocumentBoundary,
    /// The caller explicitly stopped capture.
    Explicit,
    /// The extension-owned absolute deadline elapsed.
    HardTimeout,
    /// The extension-owned recording memory ceiling was reached.
    MemoryLimit,
    /// The browser target or debugger attachment disappeared.
    BrowserDetached,
    /// A local runtime hold revoked ongoing capture.
    RuntimeHeld,
    /// The service connection disappeared while capture was active.
    ServiceDisconnected,
    /// One encoded JPEG exceeded the extension's per-frame ceiling.
    FrameTooLarge,
}

/// Content-free physical facts about one extension-owned recording.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalRecordingSummary {
    /// Physical Chromium tab captured by this recording.
    pub tab_id: u64,
    /// Current extension-owned lifecycle.
    pub state: RecordingState,
    /// Number of retained compressed JPEG frames.
    pub frame_count: usize,
    /// Total decoded JPEG bytes retained by the extension.
    pub bytes_held: usize,
    /// Elapsed time since capture started.
    pub duration_ms: u64,
    /// Absolute capture deadline while capture is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hard_expires_unix_ms: Option<u64>,
    /// Absolute memory-erasure deadline after capture stops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_expires_unix_ms: Option<u64>,
    /// Physical stop reason after capture ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<RecordingStopReason>,
    /// Bounded HTTP(S) document URLs encountered during capture for disclosure authorization.
    pub source_urls: Vec<String>,
    /// Whether the bounded source set covers every captured document, including embeds.
    #[serde(default)]
    pub source_urls_complete: bool,
}

/// Output budget for a recording GIF that never leaves the browser.
///
/// The browser holds no more encoded bytes than it already allows itself to hold in retained
/// frames, and nothing on this path is serialized across a process boundary.
pub const RECORDING_LOCAL_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Output budget for a recording GIF that must cross to a caller outside the browser.
///
/// Base64 inflates by a third, so this is what fits inside the protocol-4 transfer bound with
/// room for the receipt around it. Only a client-return save is bounded by the transfer ceiling;
/// a save that stays in the browser is not (ADR-0109 Decision 3).
pub const RECORDING_TRANSFER_MAX_BYTES: usize = 5 * 1024 * 1024;

/// Where the browser delivers a finished recording GIF.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "destination", rename_all = "snake_case")]
pub enum RecordingDestination {
    /// Attach the GIF to a file input in a page the browser already controls.
    Target {
        tab_id: u64,
        locator: String,
        file_name: String,
    },
    /// Write the GIF through the browser's own download mechanism. The browser chooses where
    /// downloads land; no caller names a path.
    Download { file_name: String },
    /// Hand the GIF back once. This is the only destination whose bytes leave the browser.
    Client,
}

/// Content-free measurements of one encoded recording GIF.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncodedRecording {
    /// Frames kept in the animation after fidelity was traded to fit the budget.
    pub frame_count: usize,
    /// Frames the recording captured.
    pub captured_frame_count: usize,
    /// How long the animation plays.
    pub duration_ms: u64,
    /// Animation width in pixels.
    pub width: u32,
    /// Animation height in pixels.
    pub height: u32,
    /// Encoded GIF size.
    pub byte_count: usize,
}

/// How the browser delivered a finished recording GIF.
///
/// Bytes appear in exactly one variant. A destination that stays inside the browser cannot carry
/// them, because there is nowhere in its shape to put them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "delivery", rename_all = "snake_case")]
pub enum RecordingDelivery {
    /// The browser attached the GIF to a page file input itself.
    Attached { tab_id: u64 },
    /// The browser wrote the GIF through its download mechanism.
    Downloaded,
    /// The GIF crossed once, for a caller outside the browser.
    Returned {
        /// Exact media type of the returned artifact.
        mime_type: String,
        /// Base64-encoded GIF bytes.
        data: String,
    },
}

/// Human intent originating from the local extension toolbar.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeControlIntent {
    /// Toggle between active and held.
    ToggleHold,
    /// Enter a hold.
    Hold,
    /// Resume active work.
    Resume,
    /// End the current admitted session.
    EndSession,
    /// Begin a new local runtime session after an ended state.
    StartSession,
}

/// Authoritative content-free runtime state published by the service.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeControlState {
    /// Effects may proceed subject to invocation authority.
    Active,
    /// Effects are held.
    Held,
    /// The admitted session has ended.
    Ended,
}

/// A content-free browser presentation signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationSignal {
    /// Opaque invocation correlation handle.
    pub invocation: String,
    /// Closed content-free signal kind.
    pub signal: PresentationKind,
    /// Closed content-free activity treatment.
    pub activity: PresentationActivity,
    /// Fixed Ghostlight-authored phase label.
    pub phase: String,
    /// Fixed Ghostlight-authored label of the activity, for the browser's activity list.
    pub label: String,
    /// Optional fixed Ghostlight-authored supporting detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Optional Chromium tab id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<u64>,
    /// Optional browser-local target locator used only for indication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    /// Optional content-free shape of the click being confirmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub click: Option<ClickShape>,
}

/// Content-free shape of one governed click, carried only so its confirmation can be drawn.
///
/// An adapter that does not understand this field renders one primary ring, which is what every
/// click drew before it existed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClickShape {
    /// How many clicks landed, as bounded by the catalog.
    pub clicks: u8,
    /// Which button produced them.
    pub button: String,
}

/// Established Ghostlight activity treatments rendered by the browser adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationActivity {
    /// Deliberately no visual treatment.
    Quiet,
    /// Governed navigation.
    Navigate,
    /// Pointer activation.
    Click,
    /// Pointer hover.
    Hover,
    /// Pointer drag.
    Drag,
    /// Browser input typing.
    Type,
    /// Keyboard action.
    Key,
    /// Page scrolling.
    Scroll,
    /// Page reading or inspection.
    Read,
    /// Semantic find.
    Find,
    /// Screenshot capture.
    Screenshot,
    /// Visible zoom change.
    Zoom,
    /// Form field write.
    Fill,
    /// Local file upload.
    Upload,
    /// Page script evaluation.
    Script,
    /// Explicit waiting.
    Wait,
    /// Native browser dialog handling.
    Dialog,
}

/// Closed content-free presentation kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationKind {
    /// Operation began.
    Start,
    /// A target is being indicated.
    Target,
    /// Operation made progress.
    Progress,
    /// Operation completed.
    Completion,
    /// Authority denied work.
    Denial,
}

/// A closed physical primitive request.
#[derive(Clone, Debug, PartialEq)]
pub enum BrowserCommand {
    /// Observe document identity and routing without extracting page content.
    DescribeDocuments {
        tab_id: u64,
        locators: Vec<String>,
        points: Vec<PhysicalPoint>,
        focused: bool,
        /// Optional bounded geometry preparation before binding current subjects.
        settle_ms: u64,
    },
    /// Run one primitive inside an application-selected physical document scope.
    InDocuments {
        scope: documents::DocumentScope,
        primitive: Box<BrowserCommand>,
    },
    /// List physical tabs visible to the adapter.
    ListTabs,
    /// Bring a physical tab and its window into view.
    FocusTab { tab_id: u64 },
    /// Open and group one URL as a single physical browser effect.
    ///
    /// `reuse` is `domain` when the edge may adopt an existing unbound same-host tab instead of
    /// creating one, and `never` when a fresh tab is required. `focus_window` is true when the
    /// workspace holds no tab yet, so its first tab comes into view.
    OpenTab {
        url: String,
        group_title: String,
        reuse: String,
        focus_window: bool,
    },
    /// Navigate a physical tab.
    Navigate { tab_id: u64, url: String },
    /// Move through browser history.
    TraverseHistory { tab_id: u64, direction: String },
    /// Reload a physical tab.
    Reload { tab_id: u64, bypass_cache: bool },
    /// Close a physical tab.
    CloseTab {
        tab_id: u64,
        /// True when this close is a dead-workspace release; a refused close then unbinds.
        released: bool,
    },
    /// Navigate, accepting only this navigation's own beforeunload prompt.
    NavigateDiscardingBeforeUnload { tab_id: u64, url: String },
    /// Read bounded useful text.
    ReadText {
        tab_id: u64,
        locator: Option<String>,
        max_chars: usize,
    },
    /// Inspect semantic controls or structure.
    Inspect {
        tab_id: u64,
        kind: String,
        max_items: usize,
    },
    /// Read one composed document with visible-text or explicit article extraction.
    ReadDocument {
        tab_id: u64,
        mode: String,
        max_chars: usize,
    },
    /// Observe one bounded hierarchical document tree.
    InspectTree {
        tab_id: u64,
        locator: Option<String>,
        max_depth: usize,
    },
    /// Find semantic targets.
    Find {
        tab_id: u64,
        text: String,
        kind: String,
        max_results: usize,
    },
    /// Capture a screenshot.
    Screenshot {
        tab_id: u64,
        locator: Option<String>,
        full_page: bool,
        visual_settle: Option<bool>,
    },
    /// Capture and magnify one page rectangle resolved from a governed view handle.
    ScreenshotRegion {
        tab_id: u64,
        region: PhysicalRectangle,
        expected_viewport: ViewportGeometry,
        visual_settle: Option<bool>,
    },
    /// Recheck credential classification immediately before a fill.
    DescribeTargets { tab_id: u64, locators: Vec<String> },
    /// Resolve one typed semantic selector against visible targets.
    QuerySemantic {
        tab_id: u64,
        /// Required accessible-name text.
        name: String,
        /// Optional closed role filter.
        role: Option<String>,
        /// Require the whole accessible name to equal the text.
        exact: bool,
        /// Restrict to controls associated with a form.
        form_scope: bool,
    },
    /// Activate one target.
    Activate {
        tab_id: u64,
        locator: String,
        button: String,
        click_count: u8,
    },
    /// Activate one page point resolved from a governed view handle.
    ActivatePoint {
        tab_id: u64,
        point: PhysicalPoint,
        expected_viewport: ViewportGeometry,
        button: String,
        click_count: u8,
    },
    /// Activate one target with held keyboard modifiers.
    ActivateModified {
        tab_id: u64,
        locator: String,
        button: String,
        click_count: u8,
        modifiers: Vec<String>,
    },
    /// Activate one page point with held keyboard modifiers.
    ActivatePointModified {
        tab_id: u64,
        point: PhysicalPoint,
        expected_viewport: ViewportGeometry,
        button: String,
        click_count: u8,
        modifiers: Vec<String>,
    },
    /// Wheel-scroll at one page point resolved from a governed view handle.
    WheelAt {
        tab_id: u64,
        point: PhysicalPoint,
        expected_viewport: ViewportGeometry,
        direction: String,
        ticks: u8,
    },
    /// Scroll the page or reveal a locator.
    Scroll {
        tab_id: u64,
        locator: Option<String>,
        direction: Option<String>,
        amount: Option<String>,
    },
    /// Set tab zoom as a browser scale factor.
    SetZoom { tab_id: u64, zoom: f64 },
    /// Resize the normal browser window containing a physical tab.
    ResizeWindow {
        tab_id: u64,
        width: u32,
        height: u32,
    },
    /// Hover one browser-local locator.
    Hover { tab_id: u64, locator: String },
    /// Hover one page point resolved from a governed view handle.
    HoverPoint {
        tab_id: u64,
        point: PhysicalPoint,
        expected_viewport: ViewportGeometry,
    },
    /// Fill fields after service preflight and optionally submit.
    Fill {
        tab_id: u64,
        fields: Vec<PhysicalField>,
        submit_locator: Option<String>,
        /// Adapter-side budget for the complete physical fill and its terminal verification.
        timeout_ms: u64,
        /// Whether service preflight allows credential-class fields in this request.
        allow_credentials: bool,
    },
    /// Type text through browser input events after credential preflight.
    TypeText {
        tab_id: u64,
        locator: String,
        text: String,
        clear_first: bool,
        /// Whether service preflight allows credential-class input in this request.
        allow_credentials: bool,
    },
    /// Describe the currently focused editable control before typing.
    DescribeFocused { tab_id: u64 },
    /// Type into the currently focused editable control after credential preflight.
    TypeFocused {
        tab_id: u64,
        text: String,
        clear_first: bool,
        /// Whether service preflight allows credential-class input in this request.
        allow_credentials: bool,
    },
    /// Send one physical keyboard action.
    PressKey {
        tab_id: u64,
        locator: Option<String>,
        key: String,
        modifiers: Vec<String>,
    },
    /// Drag between two browser-local locators.
    Drag {
        tab_id: u64,
        source_locator: String,
        destination_locator: String,
    },
    /// Drag between two page points resolved from a governed view handle.
    DragPoints {
        tab_id: u64,
        start: PhysicalPoint,
        end: PhysicalPoint,
        expected_viewport: ViewportGeometry,
    },
    /// Upload already selected and bounded file bytes.
    UploadFiles {
        tab_id: u64,
        locator: String,
        files: Vec<PhysicalFile>,
    },
    /// Drop one bounded image at a page point resolved from a governed view handle.
    DropImageAt {
        tab_id: u64,
        point: PhysicalPoint,
        expected_viewport: ViewportGeometry,
        file: PhysicalFile,
    },
    /// Evaluate an explicit script in the page's main world.
    EvaluateScript {
        tab_id: u64,
        script: String,
        max_result_chars: usize,
    },
    /// Observe one condition.
    Observe {
        tab_id: u64,
        condition: String,
        value: Option<String>,
        locator: Option<String>,
        timeout_ms: u64,
    },
    /// Inspect the current JavaScript dialog.
    InspectDialog { tab_id: u64 },
    /// Handle the current JavaScript dialog.
    HandleDialog {
        tab_id: u64,
        accept: bool,
        text: Option<String>,
    },
    /// Enable and read bounded volatile browser diagnostics.
    ReadDiagnostics {
        tab_id: u64,
        source: DiagnosticSource,
        detail: DiagnosticDetail,
        match_text: Option<String>,
        after: Option<String>,
        limit: u16,
    },
    /// Erase volatile diagnostics for tabs released by the owning workspace.
    ClearDiagnostics { tab_ids: Vec<u64> },
    /// Start the workspace's one bounded recording on a physical tab.
    StartRecording { tab_id: u64 },
    /// Return the workspace's recording summary.
    StatusRecording,
    /// Capture a final screenshot and freeze the workspace's recording.
    StopRecording,
    /// Encode the workspace's recording as an animated GIF and deliver it.
    ExportRecording {
        destination: RecordingDestination,
        /// Output budget the browser meets by trading fidelity, never coverage.
        max_output_bytes: usize,
    },
    /// Erase the workspace's recording immediately.
    DiscardRecording,
    /// Render content-free feedback.
    Present { signal: PresentationSignal },
}

impl BrowserCommand {
    /// Read the physical primitive carried by a single document execution constraint.
    #[must_use]
    pub fn primitive(&self) -> &Self {
        if let Self::InDocuments { primitive, .. } = self {
            primitive
        } else {
            self
        }
    }
}

/// Per-call facts about a target listing, carried beside its items.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listing {
    /// The page's forms index, one content-free entry per form.
    pub forms: Vec<serde_json::Value>,
    /// What the listing left out of its items, by key.
    pub left_out: LeftOut,
    /// Whether a document of the tab has an open modal dialog.
    pub modal_open: bool,
}

/// The observed outcome of a physical primitive.
#[derive(Clone, Debug, PartialEq)]
pub enum BrowserOutcome {
    /// Content-free current document and subject evidence.
    Documents {
        tab_id: u64,
        inventory: documents::DocumentInventory,
    },
    /// A primitive receipt accompanied by factual document coverage.
    InDocuments {
        observation: documents::DocumentObservation,
        result: Box<BrowserOutcome>,
    },
    /// Physical tab list.
    Tabs { tabs: Vec<PhysicalTab> },
    /// A physical tab and its window were focused.
    TabFocused {
        tab_id: u64,
        active: bool,
        window_focused: bool,
    },
    /// A new physical tab reached its observed landing.
    ///
    /// `reused` is true when the adapter satisfied the open by adopting an existing unbound
    /// same-host tab instead of creating one (ADR-0137).
    TabOpened {
        tab: PhysicalTab,
        committed_urls: Vec<String>,
        reused: bool,
    },
    /// Navigation completed or reached useful readiness.
    Navigated {
        tab: PhysicalTab,
        committed_urls: Vec<String>,
    },
    /// A tab was decisively closed.
    TabClosed { tab_id: u64 },
    /// Bounded page text.
    Text {
        tab_id: u64,
        text: String,
        truncated: bool,
        title: String,
        url: String,
    },
    /// Semantic targets.
    Targets {
        tab_id: u64,
        targets: Vec<ObservedTarget>,
        /// Per-call facts about the listing beside its items.
        listing: Listing,
    },
    /// One bounded hierarchical document tree.
    DocumentTree {
        tab_id: u64,
        /// JSON-encoded semantic tree, bounded and structure-only.
        tree: String,
        truncated: bool,
    },
    /// JPEG screenshot bytes encoded as base64.
    Screenshot {
        tab_id: u64,
        mime_type: String,
        data: String,
        width: u32,
        height: u32,
        /// Exact capture transform used by the adapter.
        viewport: ViewportGeometry,
    },
    /// Credential classifications refreshed at the final boundary.
    TargetsDescribed {
        tab_id: u64,
        targets: Vec<ObservedTarget>,
    },
    /// Target activation receipt.
    Activated {
        tab: PhysicalTab,
        subject: Option<PhysicalActionSubject>,
        committed_urls: Vec<String>,
    },
    /// Scroll or reveal receipt.
    Scrolled {
        tab_id: u64,
        x: f64,
        y: f64,
        subject: Option<PhysicalActionSubject>,
    },
    /// Zoom receipt.
    Zoomed { tab_id: u64, zoom: f64 },
    /// Browser-window resize receipt with Chromium's observed dimensions.
    WindowResized {
        tab_id: u64,
        width: u32,
        height: u32,
        /// Every physical tab whose viewport transform may have changed.
        affected_tab_ids: Vec<u64>,
    },
    /// Hover receipt.
    Hovered {
        tab_id: u64,
        subject: Option<PhysicalActionSubject>,
    },
    /// Form fill receipt.
    Filled {
        tab: PhysicalTab,
        filled_count: usize,
        /// What the page holds in the fields it reports, in the call's field order.
        held: Vec<FieldNow>,
        submitted: bool,
        committed_urls: Vec<String>,
    },
    /// Text typing receipt.
    Typed {
        tab: PhysicalTab,
        character_count: usize,
        subject: Option<PhysicalActionSubject>,
        committed_urls: Vec<String>,
    },
    /// Keyboard receipt.
    KeyPressed {
        tab: PhysicalTab,
        key: String,
        subject: Option<PhysicalActionSubject>,
        committed_urls: Vec<String>,
    },
    /// Drag receipt.
    Dragged {
        tab: PhysicalTab,
        source_subject: Option<PhysicalActionSubject>,
        destination_subject: Option<PhysicalActionSubject>,
        committed_urls: Vec<String>,
    },
    /// File upload receipt.
    FilesUploaded {
        tab_id: u64,
        uploaded_count: usize,
        uploaded_bytes: u64,
        subject: Option<PhysicalActionSubject>,
    },
    /// Script evaluation receipt.
    ScriptEvaluated {
        tab: PhysicalTab,
        value: String,
        truncated: bool,
        committed_urls: Vec<String>,
    },
    /// Condition observation receipt.
    Observed {
        tab_id: u64,
        satisfied: bool,
        elapsed_ms: u64,
        readiness: BrowserReadiness,
    },
    /// Dialog observation, with the dialog's bounded message and the url of the document that
    /// raised it when the adapter reports them.
    Dialog {
        tab_id: u64,
        present: bool,
        dialog_type: String,
        message: Option<String>,
        url: Option<String>,
    },
    /// Dialog handling receipt, with the url of the document that raised the dialog.
    DialogHandled {
        tab_id: u64,
        dialog_type: String,
        accepted: bool,
        url: Option<String>,
    },
    /// No dialog was showing when the handling attempt landed.
    DialogAbsent { tab_id: u64 },
    /// Bounded, non-destructive diagnostic read.
    DiagnosticsRead {
        tab_id: u64,
        entries: Vec<DiagnosticEntry>,
        cursor: Option<String>,
        truncated: bool,
        evicted: bool,
        capture_started: bool,
        omitted_count: usize,
    },
    /// Volatile diagnostics were erased for the reported number of tabs.
    DiagnosticsCleared { cleared_count: usize },
    /// Chromium started one extension-owned recording.
    RecordingStarted {
        summary: PhysicalRecordingSummary,
        existing: bool,
    },
    /// Current extension-owned recording state.
    RecordingStatus { summary: PhysicalRecordingSummary },
    /// Chromium stopped one extension-owned recording after its final-frame barrier.
    RecordingStopped {
        summary: PhysicalRecordingSummary,
        /// True only when this request changed active capture into a stopped state.
        changed: bool,
    },
    /// The browser encoded one recording and delivered it to the requested destination.
    RecordingExported {
        summary: PhysicalRecordingSummary,
        encoded: EncodedRecording,
        delivery: RecordingDelivery,
    },
    /// The browser could not encode or deliver the recording.
    RecordingExportFailed { reason: String },
    /// Extension memory was decisively erased.
    RecordingDiscarded { released_bytes: usize },
    /// The workspace already holds a recording, so none was started.
    RecordingHeld,
    /// The workspace holds no recording.
    RecordingNotFound,
    /// Presentation was attempted without affecting product work.
    Presented { rendered: bool },
    /// Dispatch occurred but the effect cannot be determined.
    EffectUnknown { reason: String },
}

impl BrowserOutcome {
    /// Semantic targets with no listing facts beyond the items.
    pub fn targets(tab_id: u64, targets: Vec<ObservedTarget>) -> Self {
        Self::Targets {
            tab_id,
            targets,
            listing: Listing::default(),
        }
    }
}

/// An asynchronous physical browser event.
#[derive(Clone, Debug, PartialEq)]
pub enum BrowserEvent {
    /// A top-level document committed.
    DocumentCommitted {
        tab_id: u64,
        url: String,
        /// Active primitive correlation when the commit belongs to dispatched work.
        correlation: Option<String>,
    },
    /// Readiness changed.
    ReadinessChanged {
        tab_id: u64,
        readiness: BrowserReadiness,
    },
    /// JavaScript dialog state changed.
    DialogChanged {
        tab_id: u64,
        present: bool,
        dialog_type: String,
    },
    /// A window of this browser gained attention.
    ///
    /// Only the gain is reported. Losing attention tells the resolver nothing that recency order
    /// does not already say, and no adapter can prove the gain came from a human rather than from
    /// the browser or a page, so this is an ergonomic hint and never an authorization fact.
    Attended,
    /// A local human requested a runtime-control change.
    RuntimeControlRequested { intent: RuntimeControlIntent },
    /// A tab closed outside an invocation.
    TabClosed { tab_id: u64 },
    /// The adapter connection ended.
    Disconnected,
}

#[cfg(test)]
mod tests {
    use super::{RecordingDelivery, RECORDING_LOCAL_MAX_BYTES, RECORDING_TRANSFER_MAX_BYTES};
    use crate::browser::adapter::chromium::protocol4::wire::TRANSFER_MAX_BYTES;

    #[test]
    fn only_a_client_return_can_carry_recording_bytes() {
        // The destinations that stay inside the browser have nowhere to put bytes, so "frames
        // never cross" is a property of the shape rather than a rule someone has to remember.
        for delivery in [
            RecordingDelivery::Attached { tab_id: 7 },
            RecordingDelivery::Downloaded,
        ] {
            let encoded = serde_json::to_string(&delivery).expect("delivery serializes");
            assert!(
                !encoded.contains("data"),
                "a browser-local delivery carried bytes: {encoded}"
            );
        }
        // The negative control: the one destination outside the browser does carry them, so the
        // assertion above is testing the shape rather than a missing field everywhere.
        let returned = serde_json::to_string(&RecordingDelivery::Returned {
            mime_type: "image/gif".into(),
            data: "AA==".into(),
        })
        .expect("delivery serializes");
        assert!(returned.contains("\"data\":\"AA==\""), "{returned}");
    }

    #[test]
    fn the_transfer_ceiling_binds_only_the_destination_that_transfers() {
        // Base64 inflates by a third, and the receipt travels with it. A returned GIF has to fit
        // through the native boundary; one that stays in the browser never touches it.
        const {
            assert!(4 * RECORDING_TRANSFER_MAX_BYTES.div_ceil(3) < TRANSFER_MAX_BYTES);
            assert!(RECORDING_LOCAL_MAX_BYTES > RECORDING_TRANSFER_MAX_BYTES);
        }
    }
}
