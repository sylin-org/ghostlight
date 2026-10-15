//! Adapter protocol 4: the frozen browser primitive vocabulary.
//!
//! Every frame, primitive, argument and outcome the service and the Chromium extension exchange
//! is a type here. The vocabulary carries mechanism only, with no product semantics: the service
//! classifies effects and owns every sentence, and the extension reports facts.
//!
//! Every reader is tolerant. No type uses `deny_unknown_fields`, and each closed enum the
//! extension sends has an `Unknown` or `Other` arm, so a newer extension never breaks an older
//! service. Within protocol 4 nothing is removed or renamed. The mirrors of the physical
//! vocabulary (`DiagnosticEntry`, `RecordingSummary`, `Encoded`, `Delivery`, the presentation
//! types and the runtime control state) convert to and from the types the rest of the service
//! uses, so the wire can stay tolerant while the service stays strict.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::browser::adapter as physical;

/// Adapter protocol major spoken by this module.
pub const PROTOCOL: u16 = 4;
/// Name of the isolated world that runs the service's page assets.
pub const WORLD_NAME: &str = "ghostlight";
/// Platform identity a Chromium extension names in its hello.
pub const PLATFORM: &str = "ghostlight/chromium";
/// Largest service-to-extension frame sent whole. Chromium caps host-to-extension messages at
/// 1 MiB, so anything larger travels as command chunks.
pub const DIRECT_FRAME_BYTES: usize = 768 * 1024;
/// Most decoded bytes one command chunk carries. Base64 keeps the chunk frame under 1 MiB.
pub const CHUNK_PAYLOAD_BYTES: usize = 512 * 1024;
/// Largest serialized request one chunked transfer carries.
pub const TRANSFER_MAX_BYTES: usize = 8 * 1024 * 1024;
/// Most chunks one transfer carries.
pub const TRANSFER_MAX_CHUNKS: u16 = 64;
/// Most world calls one `call` primitive carries, equal to the document limit so a route never
/// splits.
pub const MAX_WORLD_CALLS: usize = 256;
/// Most steps one `input` primitive carries.
pub const MAX_INPUT_STEPS: usize = 4096;
/// Most unseen frames a guard or inventory reports.
pub const MAX_UNSEEN: usize = 64;

/// Closed codes of the uncorrelated `error` frame the service sends.
pub mod error_code {
    /// The extension's hello names a protocol this service does not serve. The socket closes.
    pub const ADAPTER_INCOMPATIBLE: &str = "adapter_incompatible";
}

/// The closed names of the primitives, as the `name` tag of a request spells them.
pub mod primitive_name {
    /// `list_tabs`.
    pub const LIST_TABS: &str = "list_tabs";
    /// `open_tab`.
    pub const OPEN_TAB: &str = "open_tab";
    /// `navigate`.
    pub const NAVIGATE: &str = "navigate";
    /// `focus_tab`.
    pub const FOCUS_TAB: &str = "focus_tab";
    /// `close_tab`.
    pub const CLOSE_TAB: &str = "close_tab";
    /// `set_zoom`.
    pub const SET_ZOOM: &str = "set_zoom";
    /// `resize_window`.
    pub const RESIZE_WINDOW: &str = "resize_window";
    /// `frames`.
    pub const FRAMES: &str = "frames";
    /// `call`.
    pub const CALL: &str = "call";
    /// `evaluate_main`.
    pub const EVALUATE_MAIN: &str = "evaluate_main";
    /// `view`.
    pub const VIEW: &str = "view";
    /// `input`.
    pub const INPUT: &str = "input";
    /// `capture`.
    pub const CAPTURE: &str = "capture";
    /// `dialog`.
    pub const DIALOG: &str = "dialog";
    /// `diagnostics`.
    pub const DIAGNOSTICS: &str = "diagnostics";
    /// `recording`.
    pub const RECORDING: &str = "recording";
    /// `accessibility_snapshot`.
    pub const ACCESSIBILITY_SNAPSHOT: &str = "accessibility_snapshot";
}

// ---- frames, extension to service ----

/// A frame the extension sends to the service.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Inbound {
    /// The first frame on every port.
    Hello(Hello),
    /// The answer to one request.
    Receipt(Receipt),
    /// A fact about a controlled tab or the browser.
    Event {
        /// The event carried by the frame.
        event: Event,
    },
    /// The answer to a service heartbeat.
    HeartbeatAck {
        /// Sequence of the heartbeat being answered.
        sequence: u32,
    },
    /// An uncorrelated extension error.
    Error {
        /// Closed error code, or empty when absent.
        #[serde(default)]
        code: String,
        /// Bounded human-readable description, or empty when absent.
        #[serde(default)]
        message: String,
    },
    /// A frame kind this service does not know.
    #[serde(other)]
    Unknown,
}

/// The extension's opening frame: who it is and what it speaks.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Hello {
    /// Protocol major the extension speaks.
    pub protocol: u16,
    /// Stable browser-instance identity chosen by the extension.
    pub browser_id: String,
    /// Display name of the browser.
    #[serde(default)]
    pub browser_name: Option<String>,
    /// Platform identity, for example `ghostlight/chromium`.
    #[serde(default)]
    pub platform: Option<String>,
    /// Browser engine version, informational only.
    #[serde(default)]
    pub engine_version: Option<String>,
    /// Version of the extension build.
    pub adapter_version: String,
    /// Whether a person is attending the browser right now.
    #[serde(default)]
    pub attended: bool,
}

/// The extension's answer to one request: facts, never judgement.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Receipt {
    /// Correlation of the request this receipt answers.
    pub correlation: String,
    /// The primitive's outcome object, decoded by the caller against the primitive's outcome.
    #[serde(default)]
    pub ok: Option<Value>,
    /// The closed failure code. A receipt carries exactly one of `ok` and `failure`.
    #[serde(default)]
    pub failure: Option<Failure>,
    /// Bounded description that accompanies a failure.
    #[serde(default)]
    pub message: Option<String>,
    /// Whether the extension issued anything that can change browser or page state.
    #[serde(default)]
    pub dispatched: bool,
    /// Top-frame commits seen on the primitive's tab from just before dispatch to settle.
    #[serde(default)]
    pub committed_urls: Vec<String>,
    /// The tab read after the primitive, when it named one and dispatched.
    #[serde(default)]
    pub tab: Option<Tab>,
}

/// An extension event, sent only for tabs Ghostlight controls or for the browser as a whole.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A top frame committed a document.
    DocumentCommitted {
        /// Tab whose top frame committed.
        tab_id: u64,
        /// Committed URL.
        url: String,
    },
    /// A tab's readiness changed.
    ReadinessChanged {
        /// Tab whose readiness changed.
        tab_id: u64,
        /// The new readiness.
        readiness: Readiness,
    },
    /// A JavaScript dialog opened or closed.
    DialogChanged {
        /// Tab that holds the dialog.
        tab_id: u64,
        /// Whether a dialog is now showing.
        present: bool,
        /// Dialog kind reported by the browser.
        #[serde(default)]
        dialog_type: String,
    },
    /// A controlled tab closed.
    TabClosed {
        /// Tab that closed.
        tab_id: u64,
    },
    /// A person began attending the browser.
    Attended,
    /// The toolbar asked for a runtime control change.
    RuntimeControlRequested {
        /// The requested intent, parsed by [`Event::control_intent`].
        intent: String,
    },
    /// An event this service does not know.
    #[serde(other)]
    Unknown,
}

impl Event {
    /// The runtime control intent a `RuntimeControlRequested` event names.
    ///
    /// Returns `None` for any other event and for an intent this service does not know, which is
    /// ignored.
    #[must_use]
    pub fn control_intent(&self) -> Option<physical::RuntimeControlIntent> {
        let Self::RuntimeControlRequested { intent } = self else {
            return None;
        };
        serde_json::from_value(Value::String(intent.clone())).ok()
    }
}

// ---- frames, service to extension ----

/// A frame the service sends to the extension.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outbound {
    /// The answer to a hello: the service identity, control state and page assets.
    HelloAccepted {
        /// Identity of this service run; a change settles the extension's boundary state.
        service_epoch: String,
        /// Display-only service version.
        service_version: String,
        /// Runtime control state at acceptance.
        control_state: RuntimeControlState,
        /// The page assets the extension installs into its isolated world.
        assets: Assets,
    },
    /// One primitive for the extension to run.
    Request {
        /// Correlation minted per primitive and never resent.
        correlation: String,
        /// The primitive to run.
        primitive: Primitive,
    },
    /// One part of a request too large to send whole.
    CommandChunk {
        /// Identity of the transfer this part belongs to.
        transfer_id: String,
        /// Correlation of the request being transferred.
        correlation: String,
        /// Zero-based index of this part.
        index: u16,
        /// Number of parts in the transfer.
        count: u16,
        /// Decoded size of the whole transfer.
        total_bytes: u32,
        /// Lowercase hex SHA-256 of the whole transfer.
        sha256: String,
        /// Base64 bytes of this part.
        data: String,
    },
    /// Interrupt the request with this correlation. One-way.
    Cancel {
        /// Correlation of the request to interrupt.
        correlation: String,
    },
    /// A one-way presentation signal.
    Notice(Notice),
    /// A runtime control state change.
    ControlState {
        /// The new state.
        state: RuntimeControlState,
    },
    /// Periodic liveness, never per request.
    Heartbeat {
        /// Monotonic heartbeat sequence.
        sequence: u32,
    },
    /// An uncorrelated service error, for example `adapter_incompatible`.
    Error {
        /// Closed error code.
        code: String,
        /// Human-readable description.
        message: String,
    },
}

/// The page assets the service ships to the extension in `hello_accepted`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assets {
    /// Lowercase hex SHA-256 of `source`.
    pub id: String,
    /// The JavaScript that runs in the isolated world.
    pub source: String,
}

/// One primitive request: a frozen mechanism with typed arguments.
///
/// An unknown primitive name is answered `unsupported` by the extension. A new primitive gets a
/// new name and an existing one never changes meaning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", content = "args", rename_all = "snake_case")]
pub enum Primitive {
    /// List tabs.
    ListTabs(ListTabs),
    /// Open, group or adopt one tab.
    OpenTab(OpenTab),
    /// Navigate a tab.
    Navigate(Navigate),
    /// Bring a tab and its window into view.
    FocusTab(TabRef),
    /// Close a tab.
    CloseTab(CloseTab),
    /// Set a tab's zoom.
    SetZoom(SetZoom),
    /// Resize a tab's window.
    ResizeWindow(ResizeWindow),
    /// Inventory a tab's frames.
    Frames(TabRef),
    /// Run calls in the isolated world.
    Call(Call),
    /// Evaluate a script in the page's main world.
    EvaluateMain(EvaluateMain),
    /// Read the tab's view geometry.
    View(TabRef),
    /// Dispatch input steps.
    Input(Input),
    /// Capture an image.
    Capture(Capture),
    /// Inspect or handle a JavaScript dialog.
    Dialog(Dialog),
    /// Read or clear browser diagnostics.
    Diagnostics(DiagnosticsOp),
    /// Run a recording operation.
    Recording(RecordingOp),
    /// Read the accessibility tree of one frame.
    AccessibilitySnapshot(AccessibilitySnapshot),
}

// ---- document identity ----

/// One exact document: a frame and the loader that produced its current content.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct FrameRef {
    /// Browser frame identity.
    pub frame_id: String,
    /// Browser loader identity; it changes when the document is replaced.
    pub loader_id: String,
}

/// One document a guard names, with the URL the service admitted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuardDoc {
    /// Browser frame identity.
    pub frame_id: String,
    /// Browser loader identity.
    pub loader_id: String,
    /// URL the service admitted for this document.
    pub url: String,
}

/// Mechanical currency check the extension runs right before it acts.
///
/// When `exact` is set, the current inventory must equal `documents` (same set, same URLs), be
/// complete, and have exactly `unseen` unseen frames. Otherwise each listed document must be
/// current with the same origin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentGuard {
    /// The documents the service admitted.
    pub documents: Vec<GuardDoc>,
    /// Whether the inventory must equal `documents` exactly.
    pub exact: bool,
    /// Unseen frame count an exact guard requires.
    #[serde(default)]
    pub unseen: u16,
}

// ---- arguments ----

/// Arguments that name only a tab.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabRef {
    /// The tab.
    pub tab_id: u64,
}

/// Arguments of `list_tabs`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListTabs {
    /// Limit the listing to these tabs and the tabs they opened. When absent, every tab is listed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_ids: Option<Vec<u64>>,
}

/// Arguments of `open_tab`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenTab {
    /// URL to open; required unless adopting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Stable identity of the tab group.
    pub group_key: String,
    /// Display title of the tab group.
    pub group_title: String,
    /// An existing tab the service chose to adopt instead of opening one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adopt_tab_id: Option<u64>,
    /// Whether the new tab's window takes focus.
    pub focus_window: bool,
    /// Longest wait for the tab to become ready.
    pub ready_timeout_ms: u64,
}

/// Arguments of `navigate`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Navigate {
    /// The tab to navigate.
    pub tab_id: u64,
    /// Where to go.
    pub to: NavigationTarget,
    /// Whether to accept a `beforeunload` prompt.
    #[serde(default)]
    pub discard_beforeunload: bool,
    /// Longest wait for the tab to become ready.
    pub ready_timeout_ms: u64,
}

/// Where a `navigate` goes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum NavigationTarget {
    /// Load a URL.
    Url {
        /// The absolute URL.
        url: String,
    },
    /// Go back one history entry.
    Back,
    /// Go forward one history entry.
    Forward,
    /// Reload the document.
    Reload {
        /// Whether to bypass the cache.
        #[serde(default)]
        bypass_cache: bool,
    },
}

/// Arguments of `close_tab`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloseTab {
    /// The tab to close.
    pub tab_id: u64,
    /// This close releases a tab from an ended workspace. If the preserve-tabs veto refuses it,
    /// the extension marks the tab released.
    #[serde(default)]
    pub released: bool,
}

/// Arguments of `set_zoom`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SetZoom {
    /// The tab to zoom.
    pub tab_id: u64,
    /// The zoom factor.
    pub zoom: f64,
}

/// Arguments of `resize_window`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResizeWindow {
    /// A tab in the window to resize.
    pub tab_id: u64,
    /// Window width in pixels.
    pub width: u32,
    /// Window height in pixels.
    pub height: u32,
}

/// One call into the isolated world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldCall {
    /// The document to call into. Absent means the tab's current top document, without a loader
    /// check, and is for unscoped top reads only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// The asset receives the owner element of this child frame (inside `frame`) as `this`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_of: Option<String>,
    /// Message the extension passes through unread: `{kind, ...}`, built only by the world codec.
    pub message: Value,
}

/// How a `call` runs its calls.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallMode {
    /// In order. After the first answer that is not `ok:true`, the rest are skipped.
    Sequence,
    /// Concurrently. Complete when all answer.
    Parallel,
    /// Concurrently. Complete at the first `ok:true`, cancel the rest by token, and skip any
    /// still pending.
    Race,
}

/// Arguments of `call`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    /// The tab whose documents are called.
    pub tab_id: u64,
    /// How the calls run.
    pub mode: CallMode,
    /// The calls, 1 to [`MAX_WORLD_CALLS`].
    pub calls: Vec<WorldCall>,
    /// Currency check run right before the calls.
    #[serde(default)]
    pub guard: Option<DocumentGuard>,
    /// Bounded wait after the calls for the page to settle.
    #[serde(default)]
    pub settle_ms: u64,
    /// Longest the whole primitive may take.
    pub timeout_ms: u64,
}

/// Arguments of `evaluate_main`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvaluateMain {
    /// The tab to evaluate in.
    pub tab_id: u64,
    /// The script source.
    pub script: String,
    /// Longest result string returned.
    pub max_result_chars: usize,
    /// Currency check run right before the script.
    #[serde(default)]
    pub guard: Option<DocumentGuard>,
    /// Bounded wait after the script for the page to settle.
    #[serde(default)]
    pub settle_ms: u64,
    /// Longest the script may run before it is terminated.
    pub timeout_ms: u64,
}

/// Arguments of `input`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Input {
    /// The tab that receives the input.
    pub tab_id: u64,
    /// A world call run right before the first step, for example a focus check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precondition: Option<WorldCall>,
    /// The steps, 1 to [`MAX_INPUT_STEPS`].
    pub steps: Vec<InputStep>,
    /// Currency check run right before the steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard: Option<DocumentGuard>,
    /// Bounded wait after the steps for the page to settle.
    #[serde(default)]
    pub settle_ms: u64,
    /// Longest the whole primitive may take.
    pub timeout_ms: u64,
}

/// One step of an `input` primitive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum InputStep {
    /// One mouse event.
    Mouse {
        /// What the mouse does.
        action: MouseAction,
        /// Horizontal position in top-viewport CSS pixels.
        x: f64,
        /// Vertical position in top-viewport CSS pixels.
        y: f64,
        /// The button involved.
        #[serde(default)]
        button: MouseButton,
        /// Bitmask of buttons held.
        #[serde(default)]
        buttons: u8,
        /// Click count for press and release.
        #[serde(default)]
        click_count: u8,
        /// Bitmask of held modifier keys.
        #[serde(default)]
        modifiers: u8,
        /// Horizontal wheel delta.
        #[serde(default)]
        delta_x: f64,
        /// Vertical wheel delta.
        #[serde(default)]
        delta_y: f64,
        /// Pointer pressure.
        #[serde(default)]
        force: f64,
    },
    /// One key event.
    Key {
        /// What the key does.
        action: KeyAction,
        /// The key value.
        key: String,
        /// The physical key code.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        /// The legacy virtual key code.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key_code: Option<u16>,
        /// Text the key produces.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// Text the key produces without modifiers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unmodified_text: Option<String>,
        /// Bitmask of held modifier keys.
        #[serde(default)]
        modifiers: u8,
    },
    /// Insert text at the focus without key events.
    InsertText {
        /// The text.
        text: String,
    },
    /// Press at the first point, move through the path, release at the last point. An
    /// intercepted HTML5 drag is replayed as dragEnter, dragOver and drop at the last point.
    Drag {
        /// The path, in top-viewport CSS pixels.
        path: Vec<Point>,
        /// Bitmask of held modifier keys.
        #[serde(default)]
        modifiers: u8,
    },
    /// Wait.
    Pause {
        /// Milliseconds to wait, at most 1000.
        ms: u16,
    },
}

/// What a mouse step does.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseAction {
    /// Move the pointer.
    Moved,
    /// Press a button.
    Pressed,
    /// Release a button.
    Released,
    /// Turn the wheel.
    Wheel,
}

/// The button a mouse step involves.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    /// No button.
    #[default]
    None,
    /// The primary button.
    Left,
    /// The middle button.
    Middle,
    /// The secondary button.
    Right,
}

/// What a key step does.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    /// Key down with text.
    Down,
    /// Key up.
    Up,
    /// Key down without text.
    RawDown,
    /// Character input.
    Char,
}

/// A point in top-viewport CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
}

/// The region a `capture` covers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "clip", rename_all = "snake_case")]
pub enum CaptureClip {
    /// The visible viewport.
    Viewport,
    /// The whole page.
    FullPage,
    /// A rectangle in top-viewport CSS pixels. The extension adds the visual viewport's page
    /// offset from the same metrics read and clamps at zero.
    ViewportRect {
        /// Left edge.
        left: f64,
        /// Top edge.
        top: f64,
        /// Width.
        width: f64,
        /// Height.
        height: f64,
    },
    /// A rectangle in page coordinates.
    PageRect {
        /// Left edge.
        x: f64,
        /// Top edge.
        y: f64,
        /// Width.
        width: f64,
        /// Height.
        height: f64,
    },
}

/// The size limits a `capture` must fit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureBudget {
    /// Longest image side in pixels.
    pub max_side: u32,
    /// Most pixels in the image.
    pub max_pixels: u32,
    /// Smallest scale the extension may shrink to.
    pub min_scale: f64,
    /// Whether the extension may magnify a small clip.
    pub magnify: bool,
    /// JPEG qualities to try, best first.
    pub qualities: Vec<u8>,
    /// Longest base64 string the receipt may carry.
    pub max_base64_chars: usize,
}

/// Arguments of `capture`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Capture {
    /// The tab to capture.
    pub tab_id: u64,
    /// The region to capture.
    pub clip: CaptureClip,
    /// The size limits.
    pub budget: CaptureBudget,
}

/// Arguments of `dialog`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dialog {
    /// The tab that holds the dialog.
    pub tab_id: u64,
    /// What to do about it.
    pub op: DialogOp,
}

/// A dialog operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DialogOp {
    /// Report whether a dialog is showing, with its bounded message.
    Status,
    /// Accept or dismiss the showing dialog.
    Handle {
        /// Whether to accept.
        accept: bool,
        /// Text for a prompt dialog.
        #[serde(default)]
        text: Option<String>,
    },
}

/// A diagnostics operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DiagnosticsOp {
    /// Read retained console and network entries.
    Read {
        /// The tab to read.
        tab_id: u64,
        /// Which sources to read.
        source: DiagnosticSource,
        /// How much to return.
        detail: DiagnosticDetail,
        /// Text an entry must contain.
        #[serde(default)]
        match_text: Option<String>,
        /// Cursor after which to read.
        #[serde(default)]
        after: Option<String>,
        /// Most entries to return, 1 to 200.
        limit: u16,
        /// Opaque key of the admitted document set; a different key resets the tab's ring.
        #[serde(default)]
        binding: Option<String>,
        /// Currency check run right before the read.
        #[serde(default)]
        guard: Option<DocumentGuard>,
    },
    /// Clear retained entries.
    Clear {
        /// The tabs to clear.
        tab_ids: Vec<u64>,
    },
}

/// A recording operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RecordingOp {
    /// Start capturing a tab.
    Start {
        /// The tab to record.
        tab_id: u64,
        /// Stop with `document_boundary` when the inventory no longer equals this set.
        #[serde(default)]
        watch: Option<Vec<GuardDoc>>,
    },
    /// Report a recording's state.
    Status {
        /// The recording.
        recording_id: String,
    },
    /// Stop capturing.
    Stop {
        /// The recording.
        recording_id: String,
    },
    /// Encode a stopped recording and deliver it.
    Export {
        /// The recording.
        recording_id: String,
        /// Where the GIF goes.
        destination: RecordingTarget,
        /// Largest encoded GIF in bytes.
        max_output_bytes: usize,
    },
    /// Erase a recording.
    Discard {
        /// The recording.
        recording_id: String,
    },
}

/// Where an exported recording goes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "destination", rename_all = "snake_case")]
pub enum RecordingTarget {
    /// The browser's own download mechanism.
    Download {
        /// Base file name, without a path.
        file_name: String,
    },
    /// Hand the bytes back to the service once.
    Client,
    /// The extension runs `call` with the encoded GIF as the call's attachment.
    Page {
        /// The tab that receives the GIF.
        tab_id: u64,
        /// The call that consumes the GIF.
        call: WorldCall,
        /// Currency check run right before the call.
        #[serde(default)]
        guard: Option<DocumentGuard>,
    },
}

/// Arguments of `accessibility_snapshot`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccessibilitySnapshot {
    /// The tab to read.
    pub tab_id: u64,
    /// The exact document to read.
    pub frame: FrameRef,
    /// Most nodes to return, at most 5000.
    pub max_nodes: u32,
    /// Deepest level to return.
    #[serde(default)]
    pub max_depth: Option<u32>,
    /// Keep only nodes that match.
    #[serde(default)]
    pub query: Option<AxQuery>,
    /// Currency check run right before the read.
    #[serde(default)]
    pub guard: Option<DocumentGuard>,
}

/// A role and name filter for an accessibility snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AxQuery {
    /// Required role.
    #[serde(default)]
    pub role: Option<String>,
    /// Required accessible name.
    #[serde(default)]
    pub name: Option<String>,
}

/// A one-way presentation notice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notice {
    /// The tab to present on.
    pub tab_id: u64,
    /// The frame that holds the target, when the signal names one.
    #[serde(default)]
    pub frame: Option<FrameRef>,
    /// The content-free signal.
    pub signal: NoticeSignal,
    /// Whether to hold the signal until the tab is visible.
    pub defer_until_visible: bool,
}

/// A presentation signal without its tab; the locator is frame-local.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoticeSignal {
    /// Opaque invocation correlation handle.
    pub invocation: String,
    /// Closed content-free signal kind.
    pub signal: PresentationKind,
    /// Closed content-free activity treatment.
    pub activity: PresentationActivity,
    /// Fixed Ghostlight-authored phase label.
    pub phase: String,
    /// Fixed Ghostlight-authored supporting detail.
    #[serde(default)]
    pub detail: Option<String>,
    /// Frame-local locator used only for indication.
    #[serde(default)]
    pub locator: Option<String>,
    /// Content-free shape of the click being confirmed.
    #[serde(default)]
    pub click: Option<ClickShape>,
}

// ---- outcomes (the `ok` object of each primitive) ----

/// A physical tab fact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    /// Browser tab id.
    pub tab_id: u64,
    /// Bounded title.
    #[serde(default)]
    pub title: String,
    /// Current URL, or the pending URL while loading.
    #[serde(default)]
    pub url: String,
    /// Whether the browser reports the tab active.
    #[serde(default)]
    pub active: bool,
    /// Browser-local readiness.
    pub readiness: Readiness,
    /// The tab that opened this one, when Ghostlight controls or names it.
    #[serde(default)]
    pub opener_tab_id: Option<u64>,
}

impl From<Tab> for physical::PhysicalTab {
    /// The physical tab the rest of the service uses. The opener is a routing fact the
    /// physical tab does not carry.
    fn from(tab: Tab) -> Self {
        Self {
            tab_id: tab.tab_id,
            title: tab.title,
            url: tab.url,
            active: tab.active,
            readiness: tab.readiness.into(),
        }
    }
}

/// Browser-local readiness of a tab.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// A document is still loading.
    Loading,
    /// The document is interactive.
    Interactive,
    /// The document reported complete.
    Complete,
    /// A readiness this service does not know.
    #[serde(other)]
    Unknown,
}

/// Outcome of `list_tabs`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tabs {
    /// The tabs.
    pub tabs: Vec<Tab>,
}

/// Outcome of `open_tab`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabOpened {
    /// The opened or adopted tab.
    pub tab: Tab,
    /// Whether an existing tab was adopted.
    #[serde(default)]
    pub adopted: bool,
}

/// Outcome of `navigate`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Navigated {
    /// The tab after navigation.
    pub tab: Tab,
}

/// Outcome of `focus_tab`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabFocused {
    /// The focused tab.
    pub tab_id: u64,
    /// Whether the tab is now active.
    pub active: bool,
    /// Whether its window is now focused.
    pub window_focused: bool,
}

/// Outcome of `close_tab`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabClosed {
    /// The closed tab.
    pub tab_id: u64,
}

/// Outcome of `set_zoom`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zoomed {
    /// The zoomed tab.
    pub tab_id: u64,
    /// The zoom factor now in effect.
    pub zoom: f64,
}

/// Outcome of `resize_window`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowResized {
    /// The tab named by the request.
    pub tab_id: u64,
    /// Window width now in effect.
    pub width: u32,
    /// Window height now in effect.
    pub height: u32,
    /// Every tab that shares the resized window.
    pub affected_tab_ids: Vec<u64>,
}

/// One frame in a tab's inventory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameFact {
    /// Browser frame identity.
    pub frame_id: String,
    /// Browser loader identity.
    pub loader_id: String,
    /// The parent frame; absent for the top frame.
    #[serde(default)]
    pub parent_frame_id: Option<String>,
    /// The document's URL.
    pub url: String,
    /// Whether the frame shows an error page for a failed load.
    #[serde(default)]
    pub unreachable: bool,
}

/// Outcome of `frames`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameInventory {
    /// The frames the extension saw.
    pub frames: Vec<FrameFact>,
    /// Whether the inventory covers every frame.
    pub complete: bool,
    /// Frames the extension knows exist but could not describe, up to [`MAX_UNSEEN`].
    #[serde(default)]
    pub unseen: Vec<String>,
}

/// One answer within a `call` receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CallAnswer {
    /// The world answered `{ok:true,value}` or `{ok:false,error:{code,message}}`, unparsed by the
    /// extension.
    Returned {
        /// The world's answer.
        envelope: Value,
    },
    /// The call failed before or while running.
    Failed {
        /// The closed failure code.
        code: CallFailure,
        /// Bounded description.
        #[serde(default)]
        message: String,
        /// Whether the call started, so its effect is unknown.
        started: bool,
    },
    /// The call never ran.
    Skipped,
    /// A status this service does not know. Treat it as `Failed` with `Other` and `started`.
    #[serde(other)]
    Unknown,
}

/// Outcome of `call`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Called {
    /// One answer per call, in request order.
    pub answers: Vec<CallAnswer>,
}

/// Outcome of `evaluate_main`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evaluated {
    /// The bounded result string.
    #[serde(default)]
    pub value: String,
    /// Whether the result was cut at the limit.
    #[serde(default)]
    pub truncated: bool,
    /// When the page threw, the bounded exception description.
    #[serde(default)]
    pub exception: Option<String>,
}

/// Outcome of `view`: the tab's view geometry.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct ViewFacts {
    /// Visual viewport left offset in page CSS pixels.
    pub visual_page_x: f64,
    /// Visual viewport top offset in page CSS pixels.
    pub visual_page_y: f64,
    /// Visual viewport width in CSS pixels.
    pub visual_css_width: f64,
    /// Visual viewport height in CSS pixels.
    pub visual_css_height: f64,
    /// Content width in CSS pixels.
    pub content_css_width: f64,
    /// Content height in CSS pixels.
    pub content_css_height: f64,
    /// Device pixel ratio.
    pub device_scale: f64,
    /// Browser zoom factor.
    pub zoom: f64,
}

/// Why an `input` primitive stopped before its last step.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum InputStop {
    /// A JavaScript dialog opened.
    Dialog {
        /// Dialog kind reported by the browser.
        #[serde(default)]
        dialog_type: String,
    },
    /// A `cancel` frame interrupted the steps.
    Cancelled,
    /// A reason this service does not know.
    #[serde(other)]
    Other,
}

/// Outcome of `input`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputDone {
    /// The precondition's answer, when the request carried one.
    #[serde(default)]
    pub precondition: Option<CallAnswer>,
    /// Steps fully dispatched.
    pub completed_steps: u32,
    /// Why the steps stopped early, when they did.
    #[serde(default)]
    pub stopped: Option<InputStop>,
    /// Whether the browser began a native drag that the extension replayed.
    #[serde(default)]
    pub native_drag: bool,
}

/// Outcome of `capture`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Captured {
    /// Media type of `data`.
    pub mime_type: String,
    /// Base64 image bytes.
    pub data: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Scale from CSS pixels to image pixels.
    pub output_scale: f64,
    /// The captured region in page CSS pixels.
    pub clip: Rect,
    /// View geometry read with the capture.
    pub view: ViewFacts,
}

/// A rectangle in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// Outcome of `dialog`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DialogFacts {
    /// Whether a dialog was showing.
    pub present: bool,
    /// Dialog kind reported by the browser.
    #[serde(default)]
    pub dialog_type: String,
    /// The dialog's message, bounded to 500 characters; status only.
    #[serde(default)]
    pub message: Option<String>,
    /// The url of the frame that raised the dialog, bounded to the url limit; absent when the
    /// browser did not say.
    #[serde(default)]
    pub url: Option<String>,
    /// Whether the dialog was handled.
    #[serde(default)]
    pub handled: bool,
    /// Whether it was accepted.
    #[serde(default)]
    pub accepted: bool,
}

/// Outcome of `diagnostics`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticsFacts {
    /// The retained entries read.
    #[serde(default)]
    pub entries: Vec<DiagnosticEntry>,
    /// Cursor for the next read.
    #[serde(default)]
    pub cursor: Option<String>,
    /// Whether more entries remain than the limit returned.
    #[serde(default)]
    pub truncated: bool,
    /// Whether older entries were evicted from the ring.
    #[serde(default)]
    pub evicted: bool,
    /// Whether this read began capture for the tab.
    #[serde(default)]
    pub capture_started: bool,
    /// Network entries left out because their URL is not allowed.
    #[serde(default)]
    pub omitted_count: usize,
    /// Entries erased by a clear.
    #[serde(default)]
    pub cleared_count: usize,
}

/// Outcome of `recording`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum RecordingFacts {
    /// Capture started, or a recording already existed for the tab.
    Started {
        /// The recording's state.
        summary: RecordingSummary,
        /// Whether a recording already existed.
        existing: bool,
    },
    /// A recording's state.
    Status {
        /// The recording's state.
        summary: RecordingSummary,
    },
    /// Capture stopped.
    Stopped {
        /// The recording's state.
        summary: RecordingSummary,
        /// Whether this call changed the state.
        changed: bool,
    },
    /// The recording was encoded and delivered.
    Exported {
        /// The recording's state.
        summary: RecordingSummary,
        /// Measurements of the encoded GIF.
        encoded: Encoded,
        /// How the GIF was delivered.
        delivery: Delivery,
    },
    /// The export could not complete.
    ExportFailed {
        /// Bounded reason.
        reason: String,
    },
    /// The recording was erased.
    Discarded {
        /// The recording.
        recording_id: String,
        /// Bytes released.
        released_bytes: usize,
    },
    /// No such recording.
    NotFound,
    /// A result this service does not know.
    #[serde(other)]
    Unknown,
}

/// Outcome of `accessibility_snapshot`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AxSnapshot {
    /// The nodes, parents before children.
    pub nodes: Vec<AxNode>,
    /// Whether the node limit cut the tree.
    pub truncated: bool,
}

/// One accessibility node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AxNode {
    /// Node identity within the snapshot.
    pub node_id: String,
    /// The parent node; absent for a root.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Accessibility role.
    pub role: String,
    /// Accessible name.
    #[serde(default)]
    pub name: String,
    /// Current value.
    #[serde(default)]
    pub value: Option<String>,
    /// Name and value pairs of the node's properties.
    #[serde(default)]
    pub properties: Vec<(String, String)>,
}

// ---- failure vocabulary ----

/// Closed failure code of a receipt. A code this service does not know decodes as `Other`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    /// Unknown primitive name. Always `dispatched:false`.
    Unsupported,
    /// Arguments failed validation, or a chunk transfer was rejected.
    InvalidRequest,
    /// The named tab does not exist.
    TabMissing,
    /// Attach was refused: restricted page, another debugger, or a dismissed infobar.
    DebuggerUnavailable,
    /// The guard failed, or a named frame is gone or has a new loader.
    DocumentChanged,
    /// A JavaScript dialog blocks the page.
    DialogOpen,
    /// The primitive's own timeout elapsed.
    Timeout,
    /// A `cancel` frame interrupted it.
    Cancelled,
    /// The preserve-tabs veto.
    LocalInterlock,
    /// No quality in the budget fits the size limit.
    CaptureTooLarge,
    /// The browser rejected the call, or the CDP allowlist refused a method.
    BrowserRefused,
    /// The debugger session detached mid-primitive.
    SessionLost,
    /// Anything else, described by the receipt's message.
    #[serde(other)]
    Other,
}

/// Closed failure code of one world call. A code this service does not know decodes as `Other`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallFailure {
    /// The named document is gone or replaced.
    DocumentChanged,
    /// The named frame is gone.
    FrameMissing,
    /// The owner element of a child frame is gone.
    OwnerMissing,
    /// A JavaScript dialog blocks the page.
    DialogOpen,
    /// The call's time elapsed.
    Timeout,
    /// A `cancel` frame or a race interrupted it.
    Cancelled,
    /// The debugger session detached mid-call.
    SessionLost,
    /// The page threw.
    PageException,
    /// The world's answer could not be serialized.
    Unserializable,
    /// Anything else.
    #[serde(other)]
    Other,
}

// ---- mirrors of the physical vocabulary ----

/// A wire value that the physical vocabulary has no counterpart for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownVariant;

impl std::fmt::Display for UnknownVariant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the extension sent a value this service does not know")
    }
}

impl std::error::Error for UnknownVariant {}

/// Runtime control state as the extension understands it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeControlState {
    /// Effects may proceed.
    Active,
    /// Effects are held.
    Held,
    /// The admitted session has ended.
    Ended,
}

impl From<physical::RuntimeControlState> for RuntimeControlState {
    fn from(state: physical::RuntimeControlState) -> Self {
        match state {
            physical::RuntimeControlState::Active => Self::Active,
            physical::RuntimeControlState::Held => Self::Held,
            physical::RuntimeControlState::Ended => Self::Ended,
        }
    }
}

impl From<RuntimeControlState> for physical::RuntimeControlState {
    fn from(state: RuntimeControlState) -> Self {
        match state {
            RuntimeControlState::Active => Self::Active,
            RuntimeControlState::Held => Self::Held,
            RuntimeControlState::Ended => Self::Ended,
        }
    }
}

impl From<physical::BrowserReadiness> for Readiness {
    fn from(readiness: physical::BrowserReadiness) -> Self {
        match readiness {
            physical::BrowserReadiness::Loading => Self::Loading,
            physical::BrowserReadiness::Interactive => Self::Interactive,
            physical::BrowserReadiness::Complete => Self::Complete,
            physical::BrowserReadiness::Unknown => Self::Unknown,
        }
    }
}

impl From<Readiness> for physical::BrowserReadiness {
    fn from(readiness: Readiness) -> Self {
        match readiness {
            Readiness::Loading => Self::Loading,
            Readiness::Interactive => Self::Interactive,
            Readiness::Complete => Self::Complete,
            Readiness::Unknown => Self::Unknown,
        }
    }
}

/// Which diagnostic sources a read covers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSource {
    /// Console and network evidence.
    Both,
    /// Console evidence only.
    Console,
    /// Network evidence only.
    Network,
}

impl From<physical::DiagnosticSource> for DiagnosticSource {
    fn from(source: physical::DiagnosticSource) -> Self {
        match source {
            physical::DiagnosticSource::Both => Self::Both,
            physical::DiagnosticSource::Console => Self::Console,
            physical::DiagnosticSource::Network => Self::Network,
        }
    }
}

impl From<DiagnosticSource> for physical::DiagnosticSource {
    fn from(source: DiagnosticSource) -> Self {
        match source {
            DiagnosticSource::Both => Self::Both,
            DiagnosticSource::Console => Self::Console,
            DiagnosticSource::Network => Self::Network,
        }
    }
}

/// How much a diagnostics read returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticDetail {
    /// Warnings, errors, exceptions and failed HTTP activity.
    Problems,
    /// Every retained entry.
    All,
}

impl From<physical::DiagnosticDetail> for DiagnosticDetail {
    fn from(detail: physical::DiagnosticDetail) -> Self {
        match detail {
            physical::DiagnosticDetail::Problems => Self::Problems,
            physical::DiagnosticDetail::All => Self::All,
        }
    }
}

impl From<DiagnosticDetail> for physical::DiagnosticDetail {
    fn from(detail: DiagnosticDetail) -> Self {
        match detail {
            DiagnosticDetail::Problems => Self::Problems,
            DiagnosticDetail::All => Self::All,
        }
    }
}

/// One bounded volatile diagnostic observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "snake_case")]
pub enum DiagnosticEntry {
    /// One console call or uncaught exception.
    Console {
        /// Opaque cursor for this retained observation.
        cursor: String,
        /// Adapter wall-clock observation time.
        timestamp_ms: u64,
        /// Bounded console level.
        level: String,
        /// Bounded untrusted console text.
        text: String,
        /// Sanitized source origin and path, or `invalid:` when provenance cannot be proven.
        url: String,
    },
    /// One sanitized network request observation.
    Network {
        /// Opaque cursor for this retained observation.
        cursor: String,
        /// Adapter wall-clock observation time.
        timestamp_ms: u64,
        /// Bounded HTTP method.
        method: String,
        /// URL reduced to origin and path.
        url: String,
        /// Bounded resource kind.
        resource_type: String,
        /// HTTP response status when observed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
        /// Bounded failure category when loading failed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure: Option<String>,
    },
    /// An entry kind this service does not know.
    #[serde(other)]
    Unknown,
}

impl From<physical::DiagnosticEntry> for DiagnosticEntry {
    fn from(entry: physical::DiagnosticEntry) -> Self {
        match entry {
            physical::DiagnosticEntry::Console {
                cursor,
                timestamp_ms,
                level,
                text,
                url,
            } => Self::Console {
                cursor,
                timestamp_ms,
                level,
                text,
                url,
            },
            physical::DiagnosticEntry::Network {
                cursor,
                timestamp_ms,
                method,
                url,
                resource_type,
                status,
                failure,
            } => Self::Network {
                cursor,
                timestamp_ms,
                method,
                url,
                resource_type,
                status,
                failure,
            },
        }
    }
}

impl TryFrom<DiagnosticEntry> for physical::DiagnosticEntry {
    type Error = UnknownVariant;

    fn try_from(entry: DiagnosticEntry) -> Result<Self, UnknownVariant> {
        match entry {
            DiagnosticEntry::Console {
                cursor,
                timestamp_ms,
                level,
                text,
                url,
            } => Ok(Self::Console {
                cursor,
                timestamp_ms,
                level,
                text,
                url,
            }),
            DiagnosticEntry::Network {
                cursor,
                timestamp_ms,
                method,
                url,
                resource_type,
                status,
                failure,
            } => Ok(Self::Network {
                cursor,
                timestamp_ms,
                method,
                url,
                resource_type,
                status,
                failure,
            }),
            DiagnosticEntry::Unknown => Err(UnknownVariant),
        }
    }
}

/// Extension-owned recording lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    /// Capture is active.
    Recording,
    /// Capture stopped normally and frames remain temporarily available.
    Frozen,
    /// Capture stopped through a safety path and kept partial frames.
    Interrupted,
    /// A state this service does not know.
    #[serde(other)]
    Unknown,
}

impl TryFrom<RecordingState> for physical::RecordingState {
    type Error = UnknownVariant;

    fn try_from(state: RecordingState) -> Result<Self, UnknownVariant> {
        match state {
            RecordingState::Recording => Ok(Self::Recording),
            RecordingState::Frozen => Ok(Self::Frozen),
            RecordingState::Interrupted => Ok(Self::Interrupted),
            RecordingState::Unknown => Err(UnknownVariant),
        }
    }
}

/// Why an extension-owned recording stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStopReason {
    /// The document set changed before further imagery could be admitted.
    DocumentBoundary,
    /// The caller stopped capture.
    Explicit,
    /// The absolute deadline elapsed.
    HardTimeout,
    /// The memory ceiling was reached.
    MemoryLimit,
    /// The browser target or debugger attachment disappeared.
    BrowserDetached,
    /// A runtime hold revoked ongoing capture.
    RuntimeHeld,
    /// The service connection disappeared while capture was active.
    ServiceDisconnected,
    /// One encoded frame exceeded the per-frame ceiling.
    FrameTooLarge,
    /// A reason this service does not know.
    #[serde(other)]
    Unknown,
}

impl TryFrom<RecordingStopReason> for physical::RecordingStopReason {
    type Error = UnknownVariant;

    fn try_from(reason: RecordingStopReason) -> Result<Self, UnknownVariant> {
        match reason {
            RecordingStopReason::DocumentBoundary => Ok(Self::DocumentBoundary),
            RecordingStopReason::Explicit => Ok(Self::Explicit),
            RecordingStopReason::HardTimeout => Ok(Self::HardTimeout),
            RecordingStopReason::MemoryLimit => Ok(Self::MemoryLimit),
            RecordingStopReason::BrowserDetached => Ok(Self::BrowserDetached),
            RecordingStopReason::RuntimeHeld => Ok(Self::RuntimeHeld),
            RecordingStopReason::ServiceDisconnected => Ok(Self::ServiceDisconnected),
            RecordingStopReason::FrameTooLarge => Ok(Self::FrameTooLarge),
            RecordingStopReason::Unknown => Err(UnknownVariant),
        }
    }
}

/// Content-free physical facts about one extension-owned recording.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordingSummary {
    /// Opaque extension-minted recording identity.
    pub recording_id: String,
    /// Tab captured by this recording.
    pub tab_id: u64,
    /// Current lifecycle.
    pub state: RecordingState,
    /// Retained compressed frames.
    pub frame_count: usize,
    /// Decoded bytes retained by the extension.
    pub bytes_held: usize,
    /// Elapsed time since capture started.
    pub duration_ms: u64,
    /// Absolute capture deadline while capture is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hard_expires_unix_ms: Option<u64>,
    /// Absolute memory-erasure deadline after capture stops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_expires_unix_ms: Option<u64>,
    /// Why capture stopped, once it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<RecordingStopReason>,
    /// Bounded HTTP(S) document URLs encountered during capture.
    pub source_urls: Vec<String>,
    /// Whether the bounded set covers every captured document, embeds included.
    #[serde(default)]
    pub source_urls_complete: bool,
}

impl TryFrom<RecordingSummary> for physical::PhysicalRecordingSummary {
    type Error = UnknownVariant;

    fn try_from(summary: RecordingSummary) -> Result<Self, UnknownVariant> {
        Ok(Self {
            tab_id: summary.tab_id,
            state: summary.state.try_into()?,
            frame_count: summary.frame_count,
            bytes_held: summary.bytes_held,
            duration_ms: summary.duration_ms,
            hard_expires_unix_ms: summary.hard_expires_unix_ms,
            retention_expires_unix_ms: summary.retention_expires_unix_ms,
            stop_reason: summary.stop_reason.map(TryInto::try_into).transpose()?,
            source_urls: summary.source_urls,
            source_urls_complete: summary.source_urls_complete,
        })
    }
}

/// Content-free measurements of one encoded recording GIF.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Encoded {
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

impl From<physical::EncodedRecording> for Encoded {
    fn from(encoded: physical::EncodedRecording) -> Self {
        Self {
            frame_count: encoded.frame_count,
            captured_frame_count: encoded.captured_frame_count,
            duration_ms: encoded.duration_ms,
            width: encoded.width,
            height: encoded.height,
            byte_count: encoded.byte_count,
        }
    }
}

impl From<Encoded> for physical::EncodedRecording {
    fn from(encoded: Encoded) -> Self {
        Self {
            frame_count: encoded.frame_count,
            captured_frame_count: encoded.captured_frame_count,
            duration_ms: encoded.duration_ms,
            width: encoded.width,
            height: encoded.height,
            byte_count: encoded.byte_count,
        }
    }
}

/// How the extension delivered an exported recording GIF.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "delivery", rename_all = "snake_case")]
pub enum Delivery {
    /// The extension attached the GIF to a page file input.
    Attached {
        /// The tab that received it.
        tab_id: u64,
    },
    /// The extension wrote the GIF through the browser's download mechanism.
    Downloaded,
    /// The GIF crossed once, for the service.
    Returned {
        /// Exact media type of the GIF.
        mime_type: String,
        /// Base64 GIF bytes.
        data: String,
    },
    /// A delivery this service does not know.
    #[serde(other)]
    Unknown,
}

impl From<physical::RecordingDelivery> for Delivery {
    fn from(delivery: physical::RecordingDelivery) -> Self {
        match delivery {
            physical::RecordingDelivery::Attached { tab_id } => Self::Attached { tab_id },
            physical::RecordingDelivery::Downloaded => Self::Downloaded,
            physical::RecordingDelivery::Returned { mime_type, data } => {
                Self::Returned { mime_type, data }
            }
        }
    }
}

impl TryFrom<Delivery> for physical::RecordingDelivery {
    type Error = UnknownVariant;

    fn try_from(delivery: Delivery) -> Result<Self, UnknownVariant> {
        match delivery {
            Delivery::Attached { tab_id } => Ok(Self::Attached { tab_id }),
            Delivery::Downloaded => Ok(Self::Downloaded),
            Delivery::Returned { mime_type, data } => Ok(Self::Returned { mime_type, data }),
            Delivery::Unknown => Err(UnknownVariant),
        }
    }
}

/// Content-free shape of one governed click, carried so its confirmation can be drawn.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClickShape {
    /// How many clicks landed.
    pub clicks: u8,
    /// Which button produced them.
    pub button: String,
}

impl From<physical::ClickShape> for ClickShape {
    fn from(shape: physical::ClickShape) -> Self {
        Self {
            clicks: shape.clicks,
            button: shape.button,
        }
    }
}

impl From<ClickShape> for physical::ClickShape {
    fn from(shape: ClickShape) -> Self {
        Self {
            clicks: shape.clicks,
            button: shape.button,
        }
    }
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

impl From<physical::PresentationKind> for PresentationKind {
    fn from(kind: physical::PresentationKind) -> Self {
        match kind {
            physical::PresentationKind::Start => Self::Start,
            physical::PresentationKind::Target => Self::Target,
            physical::PresentationKind::Progress => Self::Progress,
            physical::PresentationKind::Completion => Self::Completion,
            physical::PresentationKind::Denial => Self::Denial,
        }
    }
}

impl From<PresentationKind> for physical::PresentationKind {
    fn from(kind: PresentationKind) -> Self {
        match kind {
            PresentationKind::Start => Self::Start,
            PresentationKind::Target => Self::Target,
            PresentationKind::Progress => Self::Progress,
            PresentationKind::Completion => Self::Completion,
            PresentationKind::Denial => Self::Denial,
        }
    }
}

/// Established Ghostlight activity treatments the extension renders.
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

impl From<physical::PresentationActivity> for PresentationActivity {
    fn from(activity: physical::PresentationActivity) -> Self {
        match activity {
            physical::PresentationActivity::Quiet => Self::Quiet,
            physical::PresentationActivity::Navigate => Self::Navigate,
            physical::PresentationActivity::Click => Self::Click,
            physical::PresentationActivity::Hover => Self::Hover,
            physical::PresentationActivity::Drag => Self::Drag,
            physical::PresentationActivity::Type => Self::Type,
            physical::PresentationActivity::Key => Self::Key,
            physical::PresentationActivity::Scroll => Self::Scroll,
            physical::PresentationActivity::Read => Self::Read,
            physical::PresentationActivity::Find => Self::Find,
            physical::PresentationActivity::Screenshot => Self::Screenshot,
            physical::PresentationActivity::Zoom => Self::Zoom,
            physical::PresentationActivity::Fill => Self::Fill,
            physical::PresentationActivity::Upload => Self::Upload,
            physical::PresentationActivity::Script => Self::Script,
            physical::PresentationActivity::Wait => Self::Wait,
            physical::PresentationActivity::Dialog => Self::Dialog,
        }
    }
}

impl From<PresentationActivity> for physical::PresentationActivity {
    fn from(activity: PresentationActivity) -> Self {
        match activity {
            PresentationActivity::Quiet => Self::Quiet,
            PresentationActivity::Navigate => Self::Navigate,
            PresentationActivity::Click => Self::Click,
            PresentationActivity::Hover => Self::Hover,
            PresentationActivity::Drag => Self::Drag,
            PresentationActivity::Type => Self::Type,
            PresentationActivity::Key => Self::Key,
            PresentationActivity::Scroll => Self::Scroll,
            PresentationActivity::Read => Self::Read,
            PresentationActivity::Find => Self::Find,
            PresentationActivity::Screenshot => Self::Screenshot,
            PresentationActivity::Zoom => Self::Zoom,
            PresentationActivity::Fill => Self::Fill,
            PresentationActivity::Upload => Self::Upload,
            PresentationActivity::Script => Self::Script,
            PresentationActivity::Wait => Self::Wait,
            PresentationActivity::Dialog => Self::Dialog,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn decode<T: serde::de::DeserializeOwned>(value: Value) -> T {
        serde_json::from_value(value).expect("decodes")
    }

    fn encode<T: Serialize>(value: &T) -> Value {
        serde_json::to_value(value).expect("encodes")
    }

    fn tab_json() -> Value {
        json!({"tab_id": 12, "title": "Example", "url": "https://example.com/",
               "active": true, "readiness": "complete"})
    }

    #[test]
    fn the_constants_are_the_ones_the_spec_names() {
        assert_eq!(PROTOCOL, 4);
        assert_eq!(WORLD_NAME, "ghostlight");
        assert_eq!(DIRECT_FRAME_BYTES, 786_432);
        assert_eq!(MAX_WORLD_CALLS, 256);
        assert_eq!(MAX_INPUT_STEPS, 4096);
        assert_eq!(MAX_UNSEEN, 64);
        assert_eq!(PLATFORM, "ghostlight/chromium");
        assert_eq!(TRANSFER_MAX_BYTES, 8 * 1024 * 1024);
        assert_eq!(TRANSFER_MAX_CHUNKS, 64);
        assert!(4 * CHUNK_PAYLOAD_BYTES.div_ceil(3) < 1024 * 1024);
        assert!(TRANSFER_MAX_BYTES <= CHUNK_PAYLOAD_BYTES * usize::from(TRANSFER_MAX_CHUNKS));
    }

    #[test]
    fn a_hello_decodes_and_serializes_back_to_itself() {
        let frame = json!({"kind": "hello", "protocol": 4, "browser_id": "browser_3f2a9c",
            "browser_name": "Chrome", "platform": "ghostlight/chromium",
            "engine_version": "131.0.6778.86", "adapter_version": "2.0.0", "attended": true});
        let Inbound::Hello(hello) = decode::<Inbound>(frame.clone()) else {
            panic!("a hello frame decodes as a hello");
        };
        assert_eq!(hello.protocol, PROTOCOL);
        assert_eq!(hello.browser_id, "browser_3f2a9c");
        assert!(hello.attended);
        assert_eq!(encode(&Inbound::Hello(hello)), frame);
    }

    #[test]
    fn a_hello_accepted_decodes_and_serializes_back_to_itself() {
        let frame = json!({"kind": "hello_accepted", "service_epoch": "service_7b1e",
            "service_version": "2.0.0", "control_state": "active",
            "assets": {"id": "ab".repeat(32), "source": "\"use strict\";\nconst ID = 1;"}});
        let outbound = decode::<Outbound>(frame.clone());
        let Outbound::HelloAccepted {
            control_state,
            ref assets,
            ..
        } = outbound
        else {
            panic!("a hello_accepted frame decodes as hello_accepted");
        };
        assert_eq!(control_state, RuntimeControlState::Active);
        assert_eq!(assets.id.len(), 64);
        assert_eq!(encode(&outbound), frame);
    }

    #[test]
    fn the_navigate_request_serializes_to_the_example() {
        let frame = Outbound::Request {
            correlation: "p4_5d0c".into(),
            primitive: Primitive::Navigate(Navigate {
                tab_id: 12,
                to: NavigationTarget::Url {
                    url: "https://example.com/".into(),
                },
                discard_beforeunload: false,
                ready_timeout_ms: 8000,
            }),
        };
        let example = json!({"kind": "request", "correlation": "p4_5d0c", "primitive": {
            "name": "navigate", "args": {"tab_id": 12,
                "to": {"to": "url", "url": "https://example.com/"},
                "discard_beforeunload": false, "ready_timeout_ms": 8000}}});
        assert_eq!(encode(&frame), example);
        assert_eq!(decode::<Outbound>(example), frame);
    }

    #[test]
    fn an_ok_receipt_carries_the_outcome_and_the_facts() {
        let frame = json!({"kind": "receipt", "correlation": "p4_5d0c",
            "ok": {"tab": tab_json()}, "dispatched": true,
            "committed_urls": ["https://example.com/"], "tab": tab_json()});
        let Inbound::Receipt(receipt) = decode::<Inbound>(frame) else {
            panic!("a receipt frame decodes as a receipt");
        };
        assert!(receipt.dispatched);
        assert_eq!(receipt.failure, None);
        assert_eq!(receipt.committed_urls, ["https://example.com/"]);
        let navigated: Navigated = decode(receipt.ok.expect("ok outcome"));
        assert_eq!(navigated.tab.tab_id, 12);
        assert_eq!(navigated.tab.readiness, Readiness::Complete);
        assert_eq!(receipt.tab, Some(navigated.tab));
    }

    #[test]
    fn a_failure_receipt_carries_a_closed_code_and_no_dispatch() {
        let frame = json!({"kind": "receipt", "correlation": "p4_77aa", "failure": "tab_missing",
            "message": "No tab with id: 99.", "dispatched": false, "committed_urls": []});
        let Inbound::Receipt(receipt) = decode::<Inbound>(frame) else {
            panic!("a receipt frame decodes as a receipt");
        };
        assert_eq!(receipt.failure, Some(Failure::TabMissing));
        assert_eq!(receipt.message.as_deref(), Some("No tab with id: 99."));
        assert!(receipt.ok.is_none());
        assert!(!receipt.dispatched);
        assert!(receipt.committed_urls.is_empty());
        assert!(receipt.tab.is_none());
    }

    #[test]
    fn an_unsupported_receipt_decodes_with_no_message() {
        let frame = json!({"kind": "receipt", "correlation": "p4_9e01", "failure": "unsupported",
            "dispatched": false, "committed_urls": []});
        let Inbound::Receipt(receipt) = decode::<Inbound>(frame) else {
            panic!("a receipt frame decodes as a receipt");
        };
        assert_eq!(receipt.failure, Some(Failure::Unsupported));
        assert_eq!(receipt.message, None);
    }

    #[test]
    fn fields_the_service_does_not_know_are_ignored_at_every_depth() {
        let hello = decode::<Inbound>(json!({"kind": "hello", "protocol": 4,
            "browser_id": "browser_a", "adapter_version": "2.1.0", "future": [1, 2]}));
        assert!(matches!(hello, Inbound::Hello(ref hello) if !hello.attended));
        let receipt = decode::<Inbound>(json!({"kind": "receipt", "correlation": "p4_1",
            "ok": {}, "dispatched": true, "future": {"a": 1},
            "tab": {"tab_id": 1, "readiness": "loading", "future": true}}));
        let Inbound::Receipt(receipt) = receipt else {
            panic!("a receipt frame decodes as a receipt");
        };
        assert_eq!(receipt.tab.expect("tab").readiness, Readiness::Loading);
        let event = decode::<Inbound>(json!({"kind": "event",
            "event": {"event": "tab_closed", "tab_id": 3, "future": 1}}));
        assert_eq!(
            event,
            Inbound::Event {
                event: Event::TabClosed { tab_id: 3 }
            }
        );
        let tabs: Tabs = decode(json!({"tabs": [{"tab_id": 1, "readiness": "complete",
            "future": 1}], "future": 2}));
        assert_eq!(tabs.tabs.len(), 1);
    }

    #[test]
    fn unknown_frame_kinds_events_failures_and_answers_decode_as_unknown_or_other() {
        assert_eq!(
            decode::<Inbound>(json!({"kind": "telemetry", "x": 1})),
            Inbound::Unknown
        );
        assert_eq!(
            decode::<Inbound>(json!({"kind": "event", "event": {"event": "tab_teleported",
                "tab_id": 1}})),
            Inbound::Event {
                event: Event::Unknown
            }
        );
        assert_eq!(decode::<Failure>(json!("from_the_future")), Failure::Other);
        assert_eq!(decode::<Failure>(json!("other")), Failure::Other);
        assert_eq!(
            decode::<CallFailure>(json!("from_the_future")),
            CallFailure::Other
        );
        assert_eq!(
            decode::<CallAnswer>(json!({"status": "from_the_future", "x": 1})),
            CallAnswer::Unknown
        );
        assert_eq!(
            decode::<Readiness>(json!("from_the_future")),
            Readiness::Unknown
        );
        assert_eq!(
            decode::<InputStop>(json!({"reason": "from_the_future"})),
            InputStop::Other
        );
        assert_eq!(
            decode::<RecordingFacts>(json!({"result": "from_the_future"})),
            RecordingFacts::Unknown
        );
        assert_eq!(
            decode::<DiagnosticEntry>(json!({"entry": "from_the_future"})),
            DiagnosticEntry::Unknown
        );
        assert_eq!(
            decode::<Delivery>(json!({"delivery": "from_the_future"})),
            Delivery::Unknown
        );
    }

    #[test]
    fn every_failure_code_in_the_spec_decodes() {
        let codes = [
            ("unsupported", Failure::Unsupported),
            ("invalid_request", Failure::InvalidRequest),
            ("tab_missing", Failure::TabMissing),
            ("debugger_unavailable", Failure::DebuggerUnavailable),
            ("document_changed", Failure::DocumentChanged),
            ("dialog_open", Failure::DialogOpen),
            ("timeout", Failure::Timeout),
            ("cancelled", Failure::Cancelled),
            ("local_interlock", Failure::LocalInterlock),
            ("capture_too_large", Failure::CaptureTooLarge),
            ("browser_refused", Failure::BrowserRefused),
            ("session_lost", Failure::SessionLost),
            ("other", Failure::Other),
        ];
        for (name, code) in codes {
            assert_eq!(decode::<Failure>(json!(name)), code);
            assert_eq!(encode(&code), json!(name));
        }
        let calls = [
            ("document_changed", CallFailure::DocumentChanged),
            ("frame_missing", CallFailure::FrameMissing),
            ("owner_missing", CallFailure::OwnerMissing),
            ("dialog_open", CallFailure::DialogOpen),
            ("timeout", CallFailure::Timeout),
            ("cancelled", CallFailure::Cancelled),
            ("session_lost", CallFailure::SessionLost),
            ("page_exception", CallFailure::PageException),
            ("unserializable", CallFailure::Unserializable),
            ("other", CallFailure::Other),
        ];
        for (name, code) in calls {
            assert_eq!(decode::<CallFailure>(json!(name)), code);
            assert_eq!(encode(&code), json!(name));
        }
    }

    #[test]
    fn a_primitive_serializes_as_a_name_and_its_arguments() {
        let primitive = Primitive::FocusTab(TabRef { tab_id: 7 });
        assert_eq!(
            encode(&primitive),
            json!({"name": "focus_tab", "args": {"tab_id": 7}})
        );
        let names = [
            (Primitive::ListTabs(ListTabs { tab_ids: None }), "list_tabs"),
            (Primitive::Frames(TabRef { tab_id: 1 }), "frames"),
            (Primitive::View(TabRef { tab_id: 1 }), "view"),
            (
                Primitive::EvaluateMain(EvaluateMain {
                    tab_id: 1,
                    script: "1".into(),
                    max_result_chars: 10,
                    guard: None,
                    settle_ms: 0,
                    timeout_ms: 100,
                }),
                "evaluate_main",
            ),
            (
                Primitive::AccessibilitySnapshot(AccessibilitySnapshot {
                    tab_id: 1,
                    frame: FrameRef {
                        frame_id: "F".into(),
                        loader_id: "L".into(),
                    },
                    max_nodes: 10,
                    max_depth: None,
                    query: None,
                    guard: None,
                }),
                "accessibility_snapshot",
            ),
        ];
        for (primitive, name) in names {
            assert_eq!(encode(&primitive)["name"], json!(name));
            assert_eq!(decode::<Primitive>(encode(&primitive)), primitive);
        }
    }

    #[test]
    fn optional_arguments_are_left_out_when_absent() {
        let list = encode(&Primitive::ListTabs(ListTabs { tab_ids: None }));
        assert_eq!(list, json!({"name": "list_tabs", "args": {}}));
        let open = encode(&Primitive::OpenTab(OpenTab {
            url: None,
            group_key: "g".into(),
            group_title: "Group".into(),
            adopt_tab_id: Some(9),
            focus_window: false,
            ready_timeout_ms: 5000,
        }));
        assert_eq!(
            open["args"],
            json!({"group_key": "g", "group_title": "Group", "adopt_tab_id": 9,
                "focus_window": false, "ready_timeout_ms": 5000})
        );
    }

    #[test]
    fn each_closed_argument_enum_carries_its_documented_tag() {
        let targets = [
            (NavigationTarget::Back, json!({"to": "back"})),
            (NavigationTarget::Forward, json!({"to": "forward"})),
            (
                NavigationTarget::Reload { bypass_cache: true },
                json!({"to": "reload", "bypass_cache": true}),
            ),
        ];
        for (target, expected) in targets {
            assert_eq!(encode(&target), expected);
            assert_eq!(decode::<NavigationTarget>(expected), target);
        }

        let steps = [
            (
                InputStep::InsertText { text: "hi".into() },
                json!({"step": "insert_text", "text": "hi"}),
            ),
            (
                InputStep::Pause { ms: 40 },
                json!({"step": "pause", "ms": 40}),
            ),
            (
                InputStep::Drag {
                    path: vec![Point { x: 1.0, y: 2.0 }, Point { x: 3.0, y: 4.0 }],
                    modifiers: 0,
                },
                json!({"step": "drag", "path": [{"x": 1.0, "y": 2.0}, {"x": 3.0, "y": 4.0}],
                    "modifiers": 0}),
            ),
        ];
        for (step, expected) in steps {
            assert_eq!(encode(&step), expected);
            assert_eq!(decode::<InputStep>(expected), step);
        }
        let mouse = decode::<InputStep>(json!({"step": "mouse", "action": "pressed",
            "x": 10.0, "y": 20.0, "button": "left", "click_count": 1}));
        assert!(matches!(
            mouse,
            InputStep::Mouse {
                action: MouseAction::Pressed,
                button: MouseButton::Left,
                click_count: 1,
                buttons: 0,
                ..
            }
        ));
        let key = decode::<InputStep>(json!({"step": "key", "action": "raw_down", "key": "Enter"}));
        assert!(matches!(
            key,
            InputStep::Key {
                action: KeyAction::RawDown,
                code: None,
                ..
            }
        ));

        let clips = [
            (CaptureClip::Viewport, json!({"clip": "viewport"})),
            (CaptureClip::FullPage, json!({"clip": "full_page"})),
            (
                CaptureClip::ViewportRect {
                    left: 1.0,
                    top: 2.0,
                    width: 3.0,
                    height: 4.0,
                },
                json!({"clip": "viewport_rect", "left": 1.0, "top": 2.0, "width": 3.0,
                    "height": 4.0}),
            ),
            (
                CaptureClip::PageRect {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                },
                json!({"clip": "page_rect", "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0}),
            ),
        ];
        for (clip, expected) in clips {
            assert_eq!(encode(&clip), expected);
            assert_eq!(decode::<CaptureClip>(expected), clip);
        }
    }

    #[test]
    fn dialog_diagnostics_and_recording_operations_are_tagged_by_op() {
        let dialog = Dialog {
            tab_id: 4,
            op: DialogOp::Handle {
                accept: true,
                text: None,
            },
        };
        assert_eq!(
            encode(&dialog),
            json!({"tab_id": 4, "op": {"op": "handle", "accept": true, "text": null}})
        );
        assert_eq!(encode(&DialogOp::Status), json!({"op": "status"}));

        let clear = DiagnosticsOp::Clear {
            tab_ids: vec![1, 2],
        };
        assert_eq!(encode(&clear), json!({"op": "clear", "tab_ids": [1, 2]}));
        let read = decode::<DiagnosticsOp>(json!({"op": "read", "tab_id": 1,
            "source": "console", "detail": "problems", "limit": 50}));
        assert!(matches!(
            read,
            DiagnosticsOp::Read {
                source: DiagnosticSource::Console,
                detail: DiagnosticDetail::Problems,
                limit: 50,
                guard: None,
                ..
            }
        ));

        let ops = [
            (
                RecordingOp::Start {
                    tab_id: 3,
                    watch: None,
                },
                "start",
            ),
            (
                RecordingOp::Status {
                    recording_id: "r".into(),
                },
                "status",
            ),
            (
                RecordingOp::Stop {
                    recording_id: "r".into(),
                },
                "stop",
            ),
            (
                RecordingOp::Discard {
                    recording_id: "r".into(),
                },
                "discard",
            ),
        ];
        for (op, name) in ops {
            assert_eq!(encode(&op)["op"], json!(name));
            assert_eq!(decode::<RecordingOp>(encode(&op)), op);
        }
    }

    #[test]
    fn a_recording_export_names_its_destination_by_tag() {
        let targets = [
            (
                RecordingTarget::Download {
                    file_name: "demo.gif".into(),
                },
                json!({"destination": "download", "file_name": "demo.gif"}),
            ),
            (RecordingTarget::Client, json!({"destination": "client"})),
            (
                RecordingTarget::Page {
                    tab_id: 2,
                    call: WorldCall {
                        frame: None,
                        owner_of: None,
                        message: json!({"kind": "attach"}),
                    },
                    guard: None,
                },
                json!({"destination": "page", "tab_id": 2,
                    "call": {"message": {"kind": "attach"}}, "guard": null}),
            ),
        ];
        for (target, expected) in targets {
            assert_eq!(encode(&target), expected);
            assert_eq!(decode::<RecordingTarget>(expected), target);
        }
        let export = RecordingOp::Export {
            recording_id: "r".into(),
            destination: RecordingTarget::Client,
            max_output_bytes: 1024,
        };
        assert_eq!(
            encode(&export),
            json!({"op": "export", "recording_id": "r",
                "destination": {"destination": "client"}, "max_output_bytes": 1024})
        );
    }

    #[test]
    fn a_world_call_names_its_document_and_passes_its_message_through() {
        let call = Call {
            tab_id: 5,
            mode: CallMode::Race,
            calls: vec![WorldCall {
                frame: Some(FrameRef {
                    frame_id: "F1".into(),
                    loader_id: "L1".into(),
                }),
                owner_of: Some("F2".into()),
                message: json!({"kind": "find", "needle": [1, 2]}),
            }],
            guard: Some(DocumentGuard {
                documents: vec![GuardDoc {
                    frame_id: "F1".into(),
                    loader_id: "L1".into(),
                    url: "https://example.com/".into(),
                }],
                exact: true,
                unseen: 0,
            }),
            settle_ms: 50,
            timeout_ms: 2000,
        };
        let value = encode(&Primitive::Call(call.clone()));
        assert_eq!(value["name"], "call");
        assert_eq!(value["args"]["mode"], "race");
        assert_eq!(value["args"]["calls"][0]["owner_of"], "F2");
        assert_eq!(value["args"]["guard"]["exact"], true);
        assert_eq!(decode::<Primitive>(value), Primitive::Call(call));
    }

    #[test]
    fn a_call_receipt_decodes_every_answer_status() {
        let called: Called = decode(json!({"answers": [
            {"status": "returned", "envelope": {"ok": true, "value": 1}},
            {"status": "failed", "code": "frame_missing", "message": "gone", "started": false},
            {"status": "skipped"},
            {"status": "from_the_future"}
        ]}));
        assert_eq!(
            called.answers,
            vec![
                CallAnswer::Returned {
                    envelope: json!({"ok": true, "value": 1})
                },
                CallAnswer::Failed {
                    code: CallFailure::FrameMissing,
                    message: "gone".into(),
                    started: false
                },
                CallAnswer::Skipped,
                CallAnswer::Unknown,
            ]
        );
    }

    #[test]
    fn a_recording_result_is_tagged_by_result() {
        let summary = json!({"recording_id": "r1", "tab_id": 3, "state": "frozen",
            "frame_count": 4, "bytes_held": 100, "duration_ms": 900,
            "stop_reason": "explicit", "source_urls": ["https://example.com/"],
            "source_urls_complete": true, "future": 1});
        let started: RecordingFacts = decode(json!({"result": "started", "summary": summary,
            "existing": false}));
        let RecordingFacts::Started { summary, existing } = started else {
            panic!("a started result decodes as started");
        };
        assert!(!existing);
        assert_eq!(summary.state, RecordingState::Frozen);
        assert_eq!(summary.stop_reason, Some(RecordingStopReason::Explicit));
        assert_eq!(
            decode::<RecordingFacts>(json!({"result": "not_found"})),
            RecordingFacts::NotFound
        );
        assert_eq!(
            decode::<RecordingFacts>(json!({"result": "export_failed", "reason": "too big"})),
            RecordingFacts::ExportFailed {
                reason: "too big".into()
            }
        );
        assert_eq!(
            decode::<RecordingFacts>(json!({"result": "discarded", "recording_id": "r1",
                "released_bytes": 100})),
            RecordingFacts::Discarded {
                recording_id: "r1".into(),
                released_bytes: 100
            }
        );
    }

    #[test]
    fn the_runtime_control_state_round_trips_the_wire() {
        use physical::RuntimeControlState as Internal;
        for (internal, wire, name) in [
            (Internal::Active, RuntimeControlState::Active, "active"),
            (Internal::Held, RuntimeControlState::Held, "held"),
            (Internal::Ended, RuntimeControlState::Ended, "ended"),
        ] {
            assert_eq!(RuntimeControlState::from(internal), wire);
            assert_eq!(Internal::from(wire), internal);
            assert_eq!(encode(&wire), json!(name));
        }
        assert_eq!(
            encode(&Outbound::ControlState {
                state: Internal::Held.into()
            }),
            json!({"kind": "control_state", "state": "held"})
        );
    }

    #[test]
    fn a_runtime_control_request_names_a_known_intent_or_none() {
        let known = Event::RuntimeControlRequested {
            intent: "toggle_hold".into(),
        };
        assert_eq!(
            known.control_intent(),
            Some(physical::RuntimeControlIntent::ToggleHold)
        );
        let unknown = Event::RuntimeControlRequested {
            intent: "from_the_future".into(),
        };
        assert_eq!(unknown.control_intent(), None);
        assert_eq!(Event::Attended.control_intent(), None);
    }

    #[test]
    fn the_physical_mirrors_convert_both_ways() {
        let console = physical::DiagnosticEntry::Console {
            cursor: "c1".into(),
            timestamp_ms: 5,
            level: "error".into(),
            text: "boom".into(),
            url: "https://example.com/a.js".into(),
        };
        let network = physical::DiagnosticEntry::Network {
            cursor: "c2".into(),
            timestamp_ms: 6,
            method: "GET".into(),
            url: "https://example.com/x".into(),
            resource_type: "xhr".into(),
            status: Some(500),
            failure: None,
        };
        for entry in [console, network] {
            let wire = DiagnosticEntry::from(entry.clone());
            assert_eq!(
                physical::DiagnosticEntry::try_from(wire.clone()),
                Ok(entry.clone())
            );
            assert_eq!(
                encode(&wire),
                serde_json::to_value(&entry).expect("physical encodes")
            );
        }
        assert_eq!(
            physical::DiagnosticEntry::try_from(DiagnosticEntry::Unknown),
            Err(UnknownVariant)
        );

        let summary = physical::PhysicalRecordingSummary {
            tab_id: 3,
            state: physical::RecordingState::Interrupted,
            frame_count: 4,
            bytes_held: 100,
            duration_ms: 900,
            hard_expires_unix_ms: Some(7),
            retention_expires_unix_ms: None,
            stop_reason: Some(physical::RecordingStopReason::MemoryLimit),
            source_urls: vec!["https://example.com/".into()],
            source_urls_complete: true,
        };
        let wire = RecordingSummary {
            recording_id: "r1".into(),
            tab_id: 3,
            state: RecordingState::Interrupted,
            frame_count: 4,
            bytes_held: 100,
            duration_ms: 900,
            hard_expires_unix_ms: Some(7),
            retention_expires_unix_ms: None,
            stop_reason: Some(RecordingStopReason::MemoryLimit),
            source_urls: vec!["https://example.com/".into()],
            source_urls_complete: true,
        };
        assert_eq!(
            physical::PhysicalRecordingSummary::try_from(wire.clone()),
            Ok(summary)
        );
        let mut unknown_state = wire.clone();
        unknown_state.state = RecordingState::Unknown;
        assert_eq!(
            physical::PhysicalRecordingSummary::try_from(unknown_state),
            Err(UnknownVariant)
        );
        let mut unknown_reason = wire;
        unknown_reason.stop_reason = Some(RecordingStopReason::Unknown);
        assert_eq!(
            physical::PhysicalRecordingSummary::try_from(unknown_reason),
            Err(UnknownVariant)
        );

        let encoded = physical::EncodedRecording {
            frame_count: 3,
            captured_frame_count: 9,
            duration_ms: 1200,
            width: 640,
            height: 480,
            byte_count: 2048,
        };
        assert_eq!(
            physical::EncodedRecording::from(Encoded::from(encoded)),
            encoded
        );

        let returned = physical::RecordingDelivery::Returned {
            mime_type: "image/gif".into(),
            data: "R0lG".into(),
        };
        for delivery in [
            physical::RecordingDelivery::Attached { tab_id: 8 },
            physical::RecordingDelivery::Downloaded,
            returned,
        ] {
            let wire = Delivery::from(delivery.clone());
            assert_eq!(
                encode(&wire),
                serde_json::to_value(&delivery).expect("physical encodes")
            );
            assert_eq!(physical::RecordingDelivery::try_from(wire), Ok(delivery));
        }
        assert_eq!(
            physical::RecordingDelivery::try_from(Delivery::Unknown),
            Err(UnknownVariant)
        );
    }

    #[test]
    fn the_presentation_mirrors_match_the_physical_names() {
        let shape = physical::ClickShape {
            clicks: 2,
            button: "left".into(),
        };
        assert_eq!(
            physical::ClickShape::from(ClickShape::from(shape.clone())),
            shape
        );
        let kinds = [
            physical::PresentationKind::Start,
            physical::PresentationKind::Target,
            physical::PresentationKind::Progress,
            physical::PresentationKind::Completion,
            physical::PresentationKind::Denial,
        ];
        for kind in kinds {
            let wire = PresentationKind::from(kind);
            assert_eq!(physical::PresentationKind::from(wire), kind);
            assert_eq!(
                encode(&wire),
                serde_json::to_value(kind).expect("physical encodes")
            );
        }
        let activities = [
            physical::PresentationActivity::Quiet,
            physical::PresentationActivity::Navigate,
            physical::PresentationActivity::Click,
            physical::PresentationActivity::Hover,
            physical::PresentationActivity::Drag,
            physical::PresentationActivity::Type,
            physical::PresentationActivity::Key,
            physical::PresentationActivity::Scroll,
            physical::PresentationActivity::Read,
            physical::PresentationActivity::Find,
            physical::PresentationActivity::Screenshot,
            physical::PresentationActivity::Zoom,
            physical::PresentationActivity::Fill,
            physical::PresentationActivity::Upload,
            physical::PresentationActivity::Script,
            physical::PresentationActivity::Wait,
            physical::PresentationActivity::Dialog,
        ];
        for activity in activities {
            let wire = PresentationActivity::from(activity);
            assert_eq!(physical::PresentationActivity::from(wire), activity);
            assert_eq!(
                encode(&wire),
                serde_json::to_value(activity).expect("physical encodes")
            );
        }
        for source in [
            physical::DiagnosticSource::Both,
            physical::DiagnosticSource::Console,
            physical::DiagnosticSource::Network,
        ] {
            assert_eq!(
                physical::DiagnosticSource::from(DiagnosticSource::from(source)),
                source
            );
        }
        for detail in [
            physical::DiagnosticDetail::Problems,
            physical::DiagnosticDetail::All,
        ] {
            assert_eq!(
                physical::DiagnosticDetail::from(DiagnosticDetail::from(detail)),
                detail
            );
        }
        for readiness in [
            physical::BrowserReadiness::Loading,
            physical::BrowserReadiness::Interactive,
            physical::BrowserReadiness::Complete,
            physical::BrowserReadiness::Unknown,
        ] {
            assert_eq!(
                physical::BrowserReadiness::from(Readiness::from(readiness)),
                readiness
            );
        }
    }

    #[test]
    fn a_notice_carries_a_content_free_signal() {
        let notice = Outbound::Notice(Notice {
            tab_id: 6,
            frame: None,
            signal: NoticeSignal {
                invocation: "inv_1".into(),
                signal: physical::PresentationKind::Target.into(),
                activity: physical::PresentationActivity::Click.into(),
                phase: "Clicking".into(),
                detail: None,
                locator: Some("e12".into()),
                click: Some(
                    physical::ClickShape {
                        clicks: 1,
                        button: "left".into(),
                    }
                    .into(),
                ),
            },
            defer_until_visible: true,
        });
        let value = encode(&notice);
        assert_eq!(value["kind"], "notice");
        assert_eq!(value["tab_id"], 6);
        assert_eq!(value["signal"]["signal"], "target");
        assert_eq!(value["signal"]["activity"], "click");
        assert_eq!(decode::<Outbound>(value), notice);
    }
}
