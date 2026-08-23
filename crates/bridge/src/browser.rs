//! Typed physical-browser relay messages with no product-facing operations.

use serde::{Deserialize, Serialize};

/// Adapter protocol major negotiated end to end by the extension and orchestrator.
pub const ADAPTER_PROTOCOL_MAJOR: u16 = 2;

/// Maximum decoded bytes carried by one host-to-extension command chunk.
pub const COMMAND_CHUNK_PAYLOAD_BYTES: usize = 512 * 1024;
/// Maximum serialized request bytes accepted by one chunked command transfer.
pub const COMMAND_TRANSFER_MAX_BYTES: usize = 8 * 1024 * 1024;
/// Maximum parts accepted for one chunked command transfer.
pub const COMMAND_TRANSFER_MAX_CHUNKS: u16 = 64;

/// Stable names for independently negotiable physical browser capabilities.
pub mod adapter_capability {
    /// Physical tab, window, grouping, and zoom mechanisms.
    pub const TABS: &str = "tabs";
    /// Atomic creation, navigation, and grouping of a new physical tab.
    pub const ATOMIC_TAB_OPEN: &str = "atomic_tab_open";
    /// Navigation, history, and reload mechanisms.
    pub const NAVIGATION: &str = "navigation";
    /// Semantic document observation and target description.
    pub const SEMANTIC_DOCUMENT: &str = "semantic_document";
    /// Screenshot capture and geometry reporting.
    pub const CAPTURE: &str = "capture";
    /// Pointer, hover, scroll, and drag input.
    pub const POINTER_INPUT: &str = "pointer_input";
    /// Form, text, and keyboard input.
    pub const KEYBOARD_INPUT: &str = "keyboard_input";
    /// Browser file-input materialization.
    pub const FILES: &str = "files";
    /// Explicit page script evaluation.
    pub const SCRIPT: &str = "script";
    /// Script revision implementing REPL-grade evaluation: top-level await,
    /// promise waiting, user gesture, and bare-return recovery.
    pub const SCRIPT_REVISION_REPL: u16 = 2;
    /// Pointer revision adding held-modifier activation and coordinate wheel input.
    pub const POINTER_INPUT_REVISION_PRECISION: u16 = 2;
    /// Keyboard revision adding focused-control description and typing.
    pub const KEYBOARD_INPUT_REVISION_FOCUSED: u16 = 2;
    /// Semantic-document revision adding typed semantic-selector queries.
    pub const SEMANTIC_DOCUMENT_REVISION_SELECTOR: u16 = 2;
    /// Browser-local condition observation.
    pub const OBSERVATION: &str = "observation";
    /// JavaScript dialog observation and handling.
    pub const DIALOGS: &str = "dialogs";
    /// Duplicate suppression, cancellation, and operation disposition.
    pub const OPERATION_RECOVERY: &str = "operation_recovery";
    /// Content-free Ghostlight presentation.
    pub const PRESENTATION: &str = "presentation";
    /// Physical browser-window geometry changes.
    pub const WINDOW_GEOMETRY: &str = "window_geometry";
    /// Opt-in bounded browser console and network observation.
    pub const DIAGNOSTICS: &str = "diagnostics";
    /// Extension-owned bounded browser recording lifecycle.
    pub const RECORDING: &str = "recording";
    /// Bounded host-to-adapter command reassembly.
    pub const CHUNKED_COMMANDS: &str = "chunked_commands";
    /// End-to-end adapter availability probes independent of browser work.
    pub const ADAPTER_LIVENESS: &str = "adapter_liveness";
    /// Reported browser-level attention, so bootstrap routing never guesses.
    pub const ADAPTER_ATTENTION: &str = "adapter_attention";
}

/// One physical capability and the highest compatible revision implemented by the adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterCapability {
    /// Stable physical capability name.
    pub name: String,
    /// Highest supported revision of this capability.
    pub revision: u16,
}

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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

/// A physical fill value sent only after credential preflight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalField {
    /// Browser-local locator.
    pub locator: String,
    /// Non-credential value.
    pub value: String,
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
    /// Opaque extension-minted recording identity.
    pub recording_id: String,
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
}

/// Output budget for a recording GIF that never leaves the browser.
///
/// The browser holds no more encoded bytes than it already allows itself to hold in retained
/// frames, and nothing on this path is serialized across a process boundary.
pub const RECORDING_LOCAL_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Output budget for a recording GIF that must cross to a caller outside the browser.
///
/// Base64 inflates by a third, so this is what fits inside [`COMMAND_TRANSFER_MAX_BYTES`] with
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
    /// Visible user attention is required.
    Attention,
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
    /// User attention is required.
    Attention,
}

/// A closed physical primitive request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum BrowserCommand {
    /// List physical tabs visible to the adapter.
    ListTabs,
    /// Bring a physical tab and its window into view.
    FocusTab { tab_id: u64 },
    /// Open and group one URL as a single physical browser effect.
    OpenTab { url: String, group_title: String },
    /// Navigate a physical tab.
    Navigate { tab_id: u64, url: String },
    /// Move through browser history.
    TraverseHistory { tab_id: u64, direction: String },
    /// Reload a physical tab.
    Reload { tab_id: u64, bypass_cache: bool },
    /// Close a physical tab.
    CloseTab { tab_id: u64 },
    /// Read bounded useful text.
    ReadText {
        tab_id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locator: Option<String>,
        max_chars: usize,
    },
    /// Inspect semantic controls or structure.
    Inspect {
        tab_id: u64,
        kind: String,
        max_items: usize,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locator: Option<String>,
        full_page: bool,
    },
    /// Capture and magnify one page rectangle resolved from a governed view handle.
    ScreenshotRegion {
        tab_id: u64,
        region: PhysicalRectangle,
        expected_viewport: ViewportGeometry,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locator: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// Fill non-credential fields and optionally submit.
    Fill {
        tab_id: u64,
        fields: Vec<PhysicalField>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        submit_locator: Option<String>,
    },
    /// Type text through browser input events after credential preflight.
    TypeText {
        tab_id: u64,
        locator: String,
        text: String,
        clear_first: bool,
    },
    /// Describe the currently focused editable control before typing.
    DescribeFocused { tab_id: u64 },
    /// Type into the currently focused editable control after credential preflight.
    TypeFocused {
        tab_id: u64,
        text: String,
        clear_first: bool,
    },
    /// Send one physical keyboard action.
    PressKey {
        tab_id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locator: Option<String>,
        timeout_ms: u64,
    },
    /// Inspect the current JavaScript dialog.
    InspectDialog { tab_id: u64 },
    /// Handle the current JavaScript dialog.
    HandleDialog {
        tab_id: u64,
        accept: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// Enable and read bounded volatile browser diagnostics.
    ReadDiagnostics {
        tab_id: u64,
        source: DiagnosticSource,
        detail: DiagnosticDetail,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        match_text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<String>,
        limit: u16,
    },
    /// Erase volatile diagnostics for tabs released by the owning workspace.
    ClearDiagnostics { tab_ids: Vec<u64> },
    /// Start one extension-owned bounded recording on a physical tab.
    StartRecording { tab_id: u64 },
    /// Return one extension-owned recording summary.
    StatusRecording { recording_id: Option<String> },
    /// Capture a final screenshot and freeze one extension-owned recording.
    StopRecording { recording_id: Option<String> },
    /// Encode one extension-owned recording as an animated GIF and deliver it.
    ExportRecording {
        recording_id: Option<String>,
        destination: RecordingDestination,
        /// Output budget the browser meets by trading fidelity, never coverage.
        max_output_bytes: usize,
    },
    /// Erase one extension-owned recording immediately.
    DiscardRecording { recording_id: Option<String> },
    /// Forward cancellation to the adapter.
    Cancel { correlation: String },
    /// Render content-free feedback.
    Present { signal: PresentationSignal },
}

impl BrowserCommand {
    /// Return the physical adapter capability required to dispatch this command.
    #[must_use]
    pub const fn required_capability(&self) -> &'static str {
        use adapter_capability as capability;
        match self {
            Self::ListTabs
            | Self::FocusTab { .. }
            | Self::CloseTab { .. }
            | Self::SetZoom { .. } => capability::TABS,
            Self::ResizeWindow { .. } => capability::WINDOW_GEOMETRY,
            Self::OpenTab { .. } => capability::ATOMIC_TAB_OPEN,
            Self::Navigate { .. } | Self::TraverseHistory { .. } | Self::Reload { .. } => {
                capability::NAVIGATION
            }
            Self::ReadText { .. }
            | Self::Inspect { .. }
            | Self::Find { .. }
            | Self::DescribeTargets { .. }
            | Self::QuerySemantic { .. } => capability::SEMANTIC_DOCUMENT,
            Self::Screenshot { .. } | Self::ScreenshotRegion { .. } => capability::CAPTURE,
            Self::Activate { .. }
            | Self::ActivatePoint { .. }
            | Self::ActivateModified { .. }
            | Self::ActivatePointModified { .. }
            | Self::WheelAt { .. }
            | Self::Scroll { .. }
            | Self::Hover { .. }
            | Self::HoverPoint { .. }
            | Self::Drag { .. }
            | Self::DragPoints { .. } => capability::POINTER_INPUT,
            Self::Fill { .. }
            | Self::TypeText { .. }
            | Self::DescribeFocused { .. }
            | Self::TypeFocused { .. }
            | Self::PressKey { .. } => capability::KEYBOARD_INPUT,
            Self::UploadFiles { .. } => capability::FILES,
            Self::EvaluateScript { .. } => capability::SCRIPT,
            Self::Observe { .. } => capability::OBSERVATION,
            Self::InspectDialog { .. } | Self::HandleDialog { .. } => capability::DIALOGS,
            Self::ReadDiagnostics { .. } | Self::ClearDiagnostics { .. } => capability::DIAGNOSTICS,
            Self::StartRecording { .. }
            | Self::StatusRecording { .. }
            | Self::StopRecording { .. }
            | Self::ExportRecording { .. }
            | Self::DiscardRecording { .. } => capability::RECORDING,
            Self::Cancel { .. } => capability::OPERATION_RECOVERY,
            Self::Present { .. } => capability::PRESENTATION,
        }
    }

    /// Return the minimum advertised revision of [`Self::required_capability`]
    /// this command accepts. Families without a stated upgrade remain at 1.
    #[must_use]
    pub const fn required_revision(&self) -> u16 {
        match self {
            Self::EvaluateScript { .. } => adapter_capability::SCRIPT_REVISION_REPL,
            Self::QuerySemantic { .. } => adapter_capability::SEMANTIC_DOCUMENT_REVISION_SELECTOR,
            Self::ActivateModified { .. }
            | Self::ActivatePointModified { .. }
            | Self::WheelAt { .. } => adapter_capability::POINTER_INPUT_REVISION_PRECISION,
            Self::DescribeFocused { .. } | Self::TypeFocused { .. } => {
                adapter_capability::KEYBOARD_INPUT_REVISION_FOCUSED
            }
            _ => 1,
        }
    }
}

/// A correlated physical primitive request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserRequest {
    /// Opaque bridge correlation handle.
    pub correlation: String,
    /// Opaque owning workspace handle.
    pub workspace: String,
    /// Physical primitive.
    pub command: BrowserCommand,
}

/// The observed outcome of a physical primitive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum BrowserOutcome {
    /// Physical tab list.
    Tabs { tabs: Vec<PhysicalTab> },
    /// A physical tab and its window were focused.
    TabFocused {
        tab_id: u64,
        active: bool,
        window_focused: bool,
    },
    /// A new physical tab reached its observed landing.
    TabOpened {
        tab: PhysicalTab,
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// Navigation completed or reached useful readiness.
    Navigated {
        tab: PhysicalTab,
        #[serde(default)]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<PhysicalActionSubject>,
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// Scroll or reveal receipt.
    Scrolled {
        tab_id: u64,
        x: f64,
        y: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<PhysicalActionSubject>,
    },
    /// Form fill receipt.
    Filled {
        tab: PhysicalTab,
        filled_count: usize,
        submitted: bool,
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// Text typing receipt.
    Typed {
        tab: PhysicalTab,
        character_count: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<PhysicalActionSubject>,
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// Keyboard receipt.
    KeyPressed {
        tab: PhysicalTab,
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<PhysicalActionSubject>,
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// Drag receipt.
    Dragged {
        tab: PhysicalTab,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_subject: Option<PhysicalActionSubject>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_subject: Option<PhysicalActionSubject>,
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// File upload receipt.
    FilesUploaded {
        tab_id: u64,
        uploaded_count: usize,
        uploaded_bytes: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<PhysicalActionSubject>,
    },
    /// Script evaluation receipt.
    ScriptEvaluated {
        tab: PhysicalTab,
        value: String,
        truncated: bool,
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// Condition observation receipt.
    Observed {
        tab_id: u64,
        satisfied: bool,
        elapsed_ms: u64,
        readiness: BrowserReadiness,
    },
    /// Dialog observation.
    Dialog {
        tab_id: u64,
        present: bool,
        dialog_type: String,
    },
    /// Dialog handling receipt.
    DialogHandled {
        tab_id: u64,
        dialog_type: String,
        accepted: bool,
    },
    /// Bounded, non-destructive diagnostic read.
    DiagnosticsRead {
        tab_id: u64,
        entries: Vec<DiagnosticEntry>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
    RecordingDiscarded {
        recording_id: String,
        released_bytes: usize,
    },
    /// Omission selected more than one recording in the caller's opaque namespace.
    RecordingAmbiguous { recording_ids: Vec<String> },
    /// No recording in the caller's opaque namespace matched the request.
    RecordingNotFound,
    /// Presentation was attempted without affecting product work.
    Presented { rendered: bool },
    /// The adapter received cancellation.
    Cancelled,
    /// Dispatch occurred but the effect cannot be determined.
    EffectUnknown { reason: String },
}

/// A correlated browser receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserReceipt {
    /// Correlation handle from the request.
    pub correlation: String,
    /// Observed physical outcome.
    pub result: BrowserOutcome,
}

/// An asynchronous physical browser event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BrowserEvent {
    /// A top-level document committed.
    DocumentCommitted {
        tab_id: u64,
        url: String,
        /// Active primitive correlation when the commit belongs to dispatched work.
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// A physical child tab opened from a known parent tab.
    ChildTabOpened {
        tab: PhysicalTab,
        opener_tab_id: u64,
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

/// A frame on the browser bridge or native-message boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrowserFrame {
    /// Adapter announces its contract.
    Hello {
        major: u16,
        adapter_version: String,
        /// Persistent opaque extension installation id.
        browser_id: String,
        /// Restart-local browser engine epoch.
        adapter_epoch: String,
        /// Bounded product name the person would recognize, such as `Chrome` or `Edge`.
        ///
        /// Absent from an adapter that predates browser plurality. Routing never depends on it;
        /// it exists so a human or a model can tell two connected browsers apart.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        browser_name: Option<String>,
        /// Whether this browser holds a focused window at the moment it connects.
        ///
        /// Attention is reported, never inferred from connection order (ADR-0084 D2). A browser
        /// that is already in front when it attaches says so here; every other browser attaches
        /// without disturbing established attention order.
        #[serde(default)]
        attended: bool,
        /// Versioned physical capabilities implemented by the adapter.
        capabilities: Vec<AdapterCapability>,
    },
    /// Service accepted the adapter.
    HelloAccepted {
        major: u16,
        service_version: String,
        /// Restart-local orchestrator service epoch.
        service_epoch: String,
        control_state: RuntimeControlState,
    },
    /// Service asks the adapter to prove that Chrome is consuming native messages.
    Heartbeat { sequence: u32 },
    /// Adapter confirms receipt of one service heartbeat.
    HeartbeatAck { sequence: u32 },
    /// Service sends a primitive request.
    Request { request: BrowserRequest },
    /// One bounded part of a serialized service-to-adapter request frame.
    CommandChunk {
        transfer_id: String,
        correlation: String,
        index: u16,
        count: u16,
        total_bytes: u32,
        sha256: String,
        data: String,
    },
    /// Adapter returns a receipt.
    Receipt { receipt: BrowserReceipt },
    /// Service confirms that a correlated terminal response is safely received.
    Acknowledge { correlation: String },
    /// Adapter emits an asynchronous event.
    Event { event: BrowserEvent },
    /// Service publishes authoritative content-free control state.
    ControlState { state: RuntimeControlState },
    /// Either side reports a bridge or primitive error.
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        correlation: Option<String>,
        code: String,
        message: String,
        effect_unknown: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::{
        adapter_capability, AdapterCapability, BrowserCommand, BrowserEvent, BrowserFrame,
        BrowserOutcome, BrowserReceipt, BrowserRequest, CaptureScope, DiagnosticDetail,
        DiagnosticEntry, DiagnosticSource, EncodedRecording, PhysicalActionSubject,
        PhysicalRecordingSummary, PhysicalRectangle, PhysicalTab, PresentationActivity,
        PresentationKind, PresentationSignal, RecordingDelivery, RecordingDestination,
        RecordingState, RecordingStopReason, ViewportGeometry, ADAPTER_PROTOCOL_MAJOR,
        COMMAND_CHUNK_PAYLOAD_BYTES, COMMAND_TRANSFER_MAX_BYTES, COMMAND_TRANSFER_MAX_CHUNKS,
        RECORDING_LOCAL_MAX_BYTES, RECORDING_TRANSFER_MAX_BYTES,
    };

    #[test]
    fn browser_messages_round_trip() {
        let frame = BrowserFrame::Request {
            request: BrowserRequest {
                correlation: "physical-1".into(),
                workspace: "workspace-1".into(),
                command: BrowserCommand::OpenTab {
                    url: "https://example.com".into(),
                    group_title: "Ghostlight - test".into(),
                },
            },
        };
        let encoded = serde_json::to_vec(&frame).expect("frame serializes");
        let decoded: BrowserFrame = serde_json::from_slice(&encoded).expect("frame deserializes");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn presentation_detail_round_trips_as_an_optional_compatible_field() {
        let frame = BrowserFrame::Request {
            request: BrowserRequest {
                correlation: "physical-2".into(),
                workspace: "workspace-1".into(),
                command: BrowserCommand::Present {
                    signal: PresentationSignal {
                        invocation: "invocation-1".into(),
                        signal: PresentationKind::Denial,
                        activity: PresentationActivity::Quiet,
                        phase: "Ghostlight kept this tab open".into(),
                        detail: Some("Closing tabs is blocked by policy.".into()),
                        tab_id: Some(7),
                        locator: None,
                        click: None,
                    },
                },
            },
        };
        let encoded = serde_json::to_vec(&frame).expect("frame serializes");
        let decoded: BrowserFrame = serde_json::from_slice(&encoded).expect("frame deserializes");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn action_receipts_carry_the_subject_observed_at_the_effect_boundary() {
        let frame = BrowserFrame::Receipt {
            receipt: BrowserReceipt {
                correlation: "physical-click".into(),
                result: BrowserOutcome::Activated {
                    tab: PhysicalTab {
                        tab_id: 7,
                        title: "Example".into(),
                        url: "https://example.com".into(),
                        active: true,
                        readiness: super::BrowserReadiness::Complete,
                    },
                    subject: Some(PhysicalActionSubject {
                        role: "button".into(),
                        name: "Save".into(),
                    }),
                    committed_urls: vec![],
                },
            },
        };
        let encoded = serde_json::to_vec(&frame).expect("action receipt serializes");
        let decoded: BrowserFrame =
            serde_json::from_slice(&encoded).expect("action receipt deserializes");
        assert_eq!(decoded, frame);
        assert!(
            serde_json::from_value::<PhysicalActionSubject>(serde_json::json!({
                "role": "button"
            }))
            .is_err()
        );
    }

    #[test]
    fn adapter_hello_capabilities_round_trip() {
        let frame = BrowserFrame::Hello {
            major: ADAPTER_PROTOCOL_MAJOR,
            adapter_version: "1.0.0".into(),
            browser_id: "browser_test".into(),
            adapter_epoch: "adapter_test".into(),
            browser_name: Some("Chrome".into()),
            attended: true,
            capabilities: vec![AdapterCapability {
                name: adapter_capability::OPERATION_RECOVERY.into(),
                revision: 1,
            }],
        };
        let encoded = serde_json::to_vec(&frame).unwrap();
        assert_eq!(
            serde_json::from_slice::<BrowserFrame>(&encoded).unwrap(),
            frame
        );
    }

    #[test]
    fn adapter_hello_without_plurality_fields_still_negotiates() {
        let hello = serde_json::json!({
            "kind": "hello",
            "major": ADAPTER_PROTOCOL_MAJOR,
            "adapter_version": "1.0.0",
            "browser_id": "browser_test",
            "adapter_epoch": "adapter_test",
            "capabilities": []
        });
        assert_eq!(
            serde_json::from_value::<BrowserFrame>(hello).expect("older hello deserializes"),
            BrowserFrame::Hello {
                major: ADAPTER_PROTOCOL_MAJOR,
                adapter_version: "1.0.0".into(),
                browser_id: "browser_test".into(),
                adapter_epoch: "adapter_test".into(),
                browser_name: None,
                attended: false,
                capabilities: vec![],
            }
        );
    }

    #[test]
    fn attention_event_round_trips() {
        let frame = BrowserFrame::Event {
            event: BrowserEvent::Attended,
        };
        let encoded = serde_json::to_vec(&frame).expect("attention event serializes");
        assert_eq!(
            serde_json::from_slice::<BrowserFrame>(&encoded).expect("attention event deserializes"),
            frame
        );
    }

    #[test]
    fn adapter_liveness_frames_round_trip() {
        let frames = [
            BrowserFrame::Heartbeat { sequence: 7 },
            BrowserFrame::HeartbeatAck { sequence: 7 },
        ];
        for frame in frames {
            let encoded = serde_json::to_vec(&frame).expect("liveness frame serializes");
            assert_eq!(
                serde_json::from_slice::<BrowserFrame>(&encoded)
                    .expect("liveness frame deserializes"),
                frame
            );
        }
    }

    #[test]
    fn protocol_two_mechanisms_round_trip() {
        let frames = [
            BrowserFrame::Request {
                request: BrowserRequest {
                    correlation: "physical-diagnostics".into(),
                    workspace: "workspace-1".into(),
                    command: BrowserCommand::ReadDiagnostics {
                        tab_id: 7,
                        source: DiagnosticSource::Both,
                        detail: DiagnosticDetail::Problems,
                        match_text: Some("failed".into()),
                        after: Some("diag_4".into()),
                        limit: 50,
                    },
                },
            },
            BrowserFrame::Receipt {
                receipt: BrowserReceipt {
                    correlation: "physical-diagnostics".into(),
                    result: BrowserOutcome::DiagnosticsRead {
                        tab_id: 7,
                        entries: vec![
                            DiagnosticEntry::Console {
                                cursor: "diag_4".into(),
                                timestamp_ms: 4,
                                level: "error".into(),
                                text: "request failed".into(),
                                url: "https://example.com/app.js".into(),
                            },
                            DiagnosticEntry::Network {
                                cursor: "diag_5".into(),
                                timestamp_ms: 5,
                                method: "GET".into(),
                                url: "https://example.com/path".into(),
                                resource_type: "fetch".into(),
                                status: Some(503),
                                failure: None,
                            },
                        ],
                        cursor: Some("diag_5".into()),
                        truncated: false,
                        evicted: false,
                        capture_started: false,
                        omitted_count: 0,
                    },
                },
            },
            BrowserFrame::Request {
                request: BrowserRequest {
                    correlation: "physical-diagnostics-clear".into(),
                    workspace: "workspace-1".into(),
                    command: BrowserCommand::ClearDiagnostics {
                        tab_ids: vec![7, 11],
                    },
                },
            },
            BrowserFrame::Receipt {
                receipt: BrowserReceipt {
                    correlation: "physical-diagnostics-clear".into(),
                    result: BrowserOutcome::DiagnosticsCleared { cleared_count: 2 },
                },
            },
            BrowserFrame::Receipt {
                receipt: BrowserReceipt {
                    correlation: "physical-recording-export".into(),
                    result: BrowserOutcome::RecordingExported {
                        summary: PhysicalRecordingSummary {
                            recording_id: "recording_1".into(),
                            tab_id: 7,
                            state: RecordingState::Interrupted,
                            frame_count: 1,
                            bytes_held: 1,
                            duration_ms: 1_000,
                            hard_expires_unix_ms: None,
                            retention_expires_unix_ms: Some(10_000),
                            stop_reason: Some(RecordingStopReason::HardTimeout),
                            source_urls: vec!["https://example.com/path".into()],
                        },
                        encoded: EncodedRecording {
                            frame_count: 1,
                            captured_frame_count: 4,
                            duration_ms: 1_000,
                            width: 1_280,
                            height: 720,
                            byte_count: 4_096,
                        },
                        delivery: RecordingDelivery::Downloaded,
                    },
                },
            },
            BrowserFrame::Request {
                request: BrowserRequest {
                    correlation: "physical-recording-export".into(),
                    workspace: "workspace-1".into(),
                    command: BrowserCommand::ExportRecording {
                        recording_id: Some("recording_1".into()),
                        destination: RecordingDestination::Target {
                            tab_id: 7,
                            locator: "locator-1".into(),
                            file_name: "ghostlight-recording.gif".into(),
                        },
                        max_output_bytes: RECORDING_LOCAL_MAX_BYTES,
                    },
                },
            },
            BrowserFrame::CommandChunk {
                transfer_id: "chunk_1".into(),
                correlation: "physical-upload".into(),
                index: 0,
                count: 2,
                total_bytes: 1_000_000,
                sha256: "0".repeat(64),
                data: "AA==".into(),
            },
        ];

        for frame in frames {
            let encoded = serde_json::to_vec(&frame).expect("frame serializes");
            let decoded: BrowserFrame =
                serde_json::from_slice(&encoded).expect("frame deserializes");
            assert_eq!(decoded, frame);
        }
    }

    #[test]
    fn new_commands_require_independent_physical_capabilities() {
        assert_eq!(
            BrowserCommand::ResizeWindow {
                tab_id: 1,
                width: 1280,
                height: 720,
            }
            .required_capability(),
            adapter_capability::WINDOW_GEOMETRY
        );
        assert_eq!(
            BrowserCommand::StartRecording { tab_id: 1 }.required_capability(),
            adapter_capability::RECORDING
        );
        assert_eq!(
            BrowserCommand::ClearDiagnostics {
                tab_ids: vec![1, 2]
            }
            .required_capability(),
            adapter_capability::DIAGNOSTICS
        );
        assert_eq!(
            BrowserCommand::ScreenshotRegion {
                tab_id: 1,
                region: PhysicalRectangle {
                    x: 10.0,
                    y: 20.0,
                    width: 300.0,
                    height: 200.0,
                },
                expected_viewport: ViewportGeometry {
                    scope: CaptureScope::Viewport,
                    page_x: 0.0,
                    page_y: 0.0,
                    css_width: 800.0,
                    css_height: 600.0,
                    visual_page_x: 0.0,
                    visual_page_y: 0.0,
                    visual_css_width: 800.0,
                    visual_css_height: 600.0,
                    device_scale: 1.0,
                    zoom: 1.0,
                    output_scale: 1.0,
                },
            }
            .required_capability(),
            adapter_capability::CAPTURE
        );
    }

    #[test]
    fn commands_declare_the_minimum_revision_that_implements_them() {
        assert_eq!(
            BrowserCommand::EvaluateScript {
                tab_id: 1,
                script: "1+1".into(),
                max_result_chars: 1000,
            }
            .required_revision(),
            adapter_capability::SCRIPT_REVISION_REPL
        );
        assert_eq!(
            BrowserCommand::ListTabs.required_revision(),
            1,
            "families without a stated upgrade stay at revision 1"
        );
        assert_eq!(
            BrowserCommand::Navigate {
                tab_id: 1,
                url: "https://example.com/".into(),
            }
            .required_revision(),
            1
        );
    }

    #[test]
    fn screenshot_region_command_round_trips_without_weakening_the_old_primitive() {
        let command = BrowserCommand::ScreenshotRegion {
            tab_id: 7,
            region: PhysicalRectangle {
                x: 120.5,
                y: 80.25,
                width: 400.0,
                height: 300.0,
            },
            expected_viewport: ViewportGeometry {
                scope: CaptureScope::Viewport,
                page_x: 10.0,
                page_y: 20.0,
                css_width: 800.0,
                css_height: 600.0,
                visual_page_x: 10.0,
                visual_page_y: 20.0,
                visual_css_width: 800.0,
                visual_css_height: 600.0,
                device_scale: 2.0,
                zoom: 1.0,
                output_scale: 0.5,
            },
        };
        let encoded = serde_json::to_value(&command).expect("region command serializes");
        assert_eq!(encoded["command"], "screenshot_region");
        assert_eq!(
            serde_json::from_value::<BrowserCommand>(encoded).expect("region command deserializes"),
            command
        );
    }

    #[test]
    fn command_chunk_bounds_fit_the_directional_chrome_boundary() {
        assert_eq!(COMMAND_CHUNK_PAYLOAD_BYTES, 512 * 1024);
        assert_eq!(COMMAND_TRANSFER_MAX_BYTES, 8 * 1024 * 1024);
        assert_eq!(COMMAND_TRANSFER_MAX_CHUNKS, 64);
        assert!(4 * COMMAND_CHUNK_PAYLOAD_BYTES.div_ceil(3) < 1024 * 1024);
    }

    #[test]
    fn console_diagnostics_require_source_provenance() {
        let missing_url = serde_json::json!({
            "entry": "console",
            "cursor": "diag_1_deadbeef",
            "timestamp_ms": 1,
            "level": "error",
            "text": "failed"
        });
        assert!(serde_json::from_value::<DiagnosticEntry>(missing_url).is_err());
    }

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
            assert!(4 * RECORDING_TRANSFER_MAX_BYTES.div_ceil(3) < COMMAND_TRANSFER_MAX_BYTES);
            assert!(RECORDING_LOCAL_MAX_BYTES > RECORDING_TRANSFER_MAX_BYTES);
        }
    }

    #[test]
    fn encoded_recordings_report_both_kept_and_captured_frames() {
        // Fidelity traded to fit is a fact about the artifact, so the receipt has to state both
        // numbers rather than let a thinned replay look like a complete one.
        let partial = serde_json::json!({
            "frame_count": 8,
            "duration_ms": 30_000,
            "width": 1_280,
            "height": 720,
            "byte_count": 4_096
        });
        assert!(serde_json::from_value::<EncodedRecording>(partial).is_err());
    }
}
