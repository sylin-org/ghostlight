//! Typed physical-browser relay messages with no product-facing operations.

use serde::{Deserialize, Serialize};

/// Adapter protocol major negotiated end to end by the extension and orchestrator.
pub const ADAPTER_PROTOCOL_MAJOR: u16 = 1;

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
    /// Browser-local condition observation.
    pub const OBSERVATION: &str = "observation";
    /// JavaScript dialog observation and handling.
    pub const DIALOGS: &str = "dialogs";
    /// Duplicate suppression, cancellation, and operation disposition.
    pub const OPERATION_RECOVERY: &str = "operation_recovery";
    /// Content-free Ghostlight presentation.
    pub const PRESENTATION: &str = "presentation";
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
    /// Recheck credential classification immediately before a fill.
    DescribeTargets { tab_id: u64, locators: Vec<String> },
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
            Self::OpenTab { .. } => capability::ATOMIC_TAB_OPEN,
            Self::Navigate { .. } | Self::TraverseHistory { .. } | Self::Reload { .. } => {
                capability::NAVIGATION
            }
            Self::ReadText { .. }
            | Self::Inspect { .. }
            | Self::Find { .. }
            | Self::DescribeTargets { .. } => capability::SEMANTIC_DOCUMENT,
            Self::Screenshot { .. } => capability::CAPTURE,
            Self::Activate { .. }
            | Self::ActivatePoint { .. }
            | Self::Scroll { .. }
            | Self::Hover { .. }
            | Self::HoverPoint { .. }
            | Self::Drag { .. }
            | Self::DragPoints { .. } => capability::POINTER_INPUT,
            Self::Fill { .. } | Self::TypeText { .. } | Self::PressKey { .. } => {
                capability::KEYBOARD_INPUT
            }
            Self::UploadFiles { .. } => capability::FILES,
            Self::EvaluateScript { .. } => capability::SCRIPT,
            Self::Observe { .. } => capability::OBSERVATION,
            Self::InspectDialog { .. } | Self::HandleDialog { .. } => capability::DIALOGS,
            Self::Cancel { .. } => capability::OPERATION_RECOVERY,
            Self::Present { .. } => capability::PRESENTATION,
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
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// Scroll or reveal receipt.
    Scrolled { tab_id: u64, x: f64, y: f64 },
    /// Zoom receipt.
    Zoomed { tab_id: u64, zoom: f64 },
    /// Hover receipt.
    Hovered { tab_id: u64 },
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
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// Keyboard receipt.
    KeyPressed {
        tab: PhysicalTab,
        key: String,
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// Drag receipt.
    Dragged {
        tab: PhysicalTab,
        #[serde(default)]
        committed_urls: Vec<String>,
    },
    /// File upload receipt.
    FilesUploaded {
        tab_id: u64,
        uploaded_count: usize,
        uploaded_bytes: u64,
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
    /// Service sends a primitive request.
    Request { request: BrowserRequest },
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
        adapter_capability, AdapterCapability, BrowserCommand, BrowserFrame, BrowserRequest,
        PresentationActivity, PresentationKind, PresentationSignal, ADAPTER_PROTOCOL_MAJOR,
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
                    },
                },
            },
        };
        let encoded = serde_json::to_vec(&frame).expect("frame serializes");
        let decoded: BrowserFrame = serde_json::from_slice(&encoded).expect("frame deserializes");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn adapter_hello_capabilities_round_trip() {
        let frame = BrowserFrame::Hello {
            major: ADAPTER_PROTOCOL_MAJOR,
            adapter_version: "1.0.0".into(),
            browser_id: "browser_test".into(),
            adapter_epoch: "adapter_test".into(),
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
}
