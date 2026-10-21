//! Adapter protocol 4: the frozen browser primitive vocabulary.
//!
//! Every frame, primitive, argument and outcome the service and the Chromium extension exchange
//! is a type here. The vocabulary carries mechanism only, with no product semantics: the service
//! classifies effects and owns every sentence, and the extension reports facts.
//!
//! Every reader is tolerant. No type uses `deny_unknown_fields`, and each closed enum the
//! extension sends has an `Unknown` or `Other` arm, so a newer extension never breaks an older
//! service. Within protocol 4 nothing is removed or renamed. The mirrors of the physical
//! vocabulary (`DiagnosticEntry`, `DiagnosticSource`, `DiagnosticDetail`, `RecordingSummary`,
//! `Encoded`, `Delivery`, `Readiness`, the presentation types and the runtime control state)
//! convert in the one direction each crosses: what the service sends from the types the rest of
//! the service uses, and what the extension sends into them, so the wire can stay tolerant while
//! the service stays strict.

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
/// Chromium's cap on one host-to-extension message.
const NATIVE_MESSAGE_BYTES: usize = 1024 * 1024;
const _: () = assert!(4 * CHUNK_PAYLOAD_BYTES.div_ceil(3) < NATIVE_MESSAGE_BYTES);
const _: () = assert!(TRANSFER_MAX_BYTES <= CHUNK_PAYLOAD_BYTES * TRANSFER_MAX_CHUNKS as usize);
// A recording GIF returned to a client travels base64-encoded inside one transfer.
const _: () = assert!(4 * physical::RECORDING_TRANSFER_MAX_BYTES.div_ceil(3) < TRANSFER_MAX_BYTES);
/// Most world calls one `call` primitive carries. A route sends one call per document, so the
/// limit is never below the document limit and a route never splits. The wire fixes the number;
/// the assertion below only keeps the document limit from passing it.
pub const MAX_WORLD_CALLS: usize = 256;
const _: () = assert!(MAX_WORLD_CALLS >= physical::documents::DOCUMENT_LIMIT);
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimitiveName {
    /// `list_tabs`.
    ListTabs,
    /// `open_tab`.
    OpenTab,
    /// `navigate`.
    Navigate,
    /// `focus_tab`.
    FocusTab,
    /// `close_tab`.
    CloseTab,
    /// `set_zoom`.
    SetZoom,
    /// `resize_window`.
    ResizeWindow,
    /// `frames`.
    Frames,
    /// `call`.
    Call,
    /// `evaluate_main`.
    EvaluateMain,
    /// `view`.
    View,
    /// `input`.
    Input,
    /// `capture`.
    Capture,
    /// `dialog`.
    Dialog,
    /// `diagnostics`.
    Diagnostics,
    /// `recording`.
    Recording,
    /// `accessibility_snapshot`.
    AccessibilitySnapshot,
}

impl PrimitiveName {
    /// Every primitive the service can send.
    pub const ALL: [Self; 17] = [
        Self::ListTabs,
        Self::OpenTab,
        Self::Navigate,
        Self::FocusTab,
        Self::CloseTab,
        Self::SetZoom,
        Self::ResizeWindow,
        Self::Frames,
        Self::Call,
        Self::EvaluateMain,
        Self::View,
        Self::Input,
        Self::Capture,
        Self::Dialog,
        Self::Diagnostics,
        Self::Recording,
        Self::AccessibilitySnapshot,
    ];

    /// The `name` tag a request carries for this primitive.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ListTabs => "list_tabs",
            Self::OpenTab => "open_tab",
            Self::Navigate => "navigate",
            Self::FocusTab => "focus_tab",
            Self::CloseTab => "close_tab",
            Self::SetZoom => "set_zoom",
            Self::ResizeWindow => "resize_window",
            Self::Frames => "frames",
            Self::Call => "call",
            Self::EvaluateMain => "evaluate_main",
            Self::View => "view",
            Self::Input => "input",
            Self::Capture => "capture",
            Self::Dialog => "dialog",
            Self::Diagnostics => "diagnostics",
            Self::Recording => "recording",
            Self::AccessibilitySnapshot => "accessibility_snapshot",
        }
    }
}

/// The word serde writes for a closed wire value, such as a failure code, or empty for a value
/// that is not a word.
#[must_use]
pub fn name_of(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
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
    /// The extension's life in this browser run, an opaque id the extension keeps in session
    /// storage. The browser keeps it across a worker and a native-host restart and clears it on
    /// a browser restart and on an extension reload, update or disable, so one life names one
    /// browser run, in which tab ids are never reused (ADR-0203 Decision 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub life: Option<String>,
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
    /// A person in an extension page asked to see Ghostlight.
    ShowRequested,
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
    /// Fixed Ghostlight-authored activity label, which the page draws as its caption. A frame
    /// without one shows no caption: the page has no words of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
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
}

impl From<Tab> for physical::PhysicalTab {
    /// The physical tab the rest of the service uses.
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
    /// For a prompt, the text its field holds, which accept answers with, bounded to 500
    /// characters and empty when the field is empty; status only. An extension older than 2.0.1
    /// does not send it.
    #[serde(default)]
    pub default_prompt: Option<String>,
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

/// The reason a protocol-4 extension gives when the page did not take a replay at the save's
/// target: the target is not a file input, or not one that takes it. The words are the
/// extension's (`buffers.js`), fixed for protocol 4.
pub const PAGE_REFUSED_REPLAY: &str = "the page did not accept the replay";

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
        /// Bounded reason. [`PAGE_REFUSED_REPLAY`] says the page did not take the replay.
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
    /// Back or forward found no entry that way in the tab's history. Always `dispatched:false`.
    /// A service older than 2.0.1 reads it as `Other`.
    NoHistoryEntry,
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

impl From<Readiness> for physical::BrowserReadiness {
    fn from(readiness: Readiness) -> Self {
        match readiness {
            Readiness::Loading => Self::Loading,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn decode<T: serde::de::DeserializeOwned>(value: Value) -> T {
        serde_json::from_value(value).expect("decodes")
    }

    /// A word a newer extension sends and this service does not know decodes to its type's
    /// catch-all, so an extension update never breaks the service it talks to. The golden
    /// tolerance fixtures cover frames, events, receipt failures, call answers, readiness and
    /// input stops; these are the types they do not.
    #[test]
    fn a_word_from_a_newer_extension_decodes_to_its_catch_all() {
        const NEWER: &str = "from_the_future";
        assert_eq!(decode::<CallFailure>(json!(NEWER)), CallFailure::Other);
        assert_eq!(
            decode::<RecordingFacts>(json!({"result": NEWER})),
            RecordingFacts::Unknown
        );
        assert_eq!(
            decode::<DiagnosticEntry>(json!({"entry": NEWER})),
            DiagnosticEntry::Unknown
        );
        assert_eq!(
            decode::<Delivery>(json!({"delivery": NEWER})),
            Delivery::Unknown
        );
    }
}
