//! The complete model-facing catalog, typo-closed decoding, and executable defaults.

pub mod capability_map;
pub mod environment;
pub mod outcome;
pub mod readiness;
#[path = "catalog.rs"]
mod tool_catalog;

pub use tool_catalog::{catalog, catalog_for};

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

const DEFAULT_TIMEOUT_MS: u64 = 8_000;
const MIN_TIMEOUT_MS: u64 = 100;
const MAX_TIMEOUT_MS: u64 = 30_000;
const COMMON_FIELDS: &[&str] = &["restrict_hosts", "restrict_capabilities"];

/// Model-facing instructions supplied to every protocol edge by the orchestrator.
pub const SERVER_INSTRUCTIONS: &str = "Ghostlight controls the user's visible Chromium browser. Use the advertised short calls and inspect current handles after navigation.";
const CAPABILITIES: &[&str] = &["read", "action", "write", "execute"];
/// Closed role vocabulary a semantic selector may filter on.
const SEMANTIC_ROLES: &[&str] = &[
    "button",
    "link",
    "checkbox",
    "radio",
    "textbox",
    "searchbox",
    "combobox",
    "listbox",
    "select",
    "slider",
    "spinbutton",
    "tab",
    "menuitem",
    "option",
    "heading",
    "image",
];

const NAMED_KEYS: &[&str] = &[
    "Enter",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Space",
];

/// Optional caller restrictions that can only tighten authority.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct RequestRestrictions {
    /// Host patterns allowed for this invocation.
    #[serde(default)]
    pub restrict_hosts: Option<Vec<String>>,
    /// Capabilities allowed for this invocation.
    #[serde(default)]
    pub restrict_capabilities: Option<Vec<String>>,
}

/// A typed user job decoded by the language context.
#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    /// List controlled tabs.
    ListTabs(ListTabs),
    /// Bring an exact controlled tab into view.
    ActivateTab(ActivateTab),
    /// Open a governed page.
    OpenPage(OpenPage),
    /// Navigate a controlled tab.
    NavigatePage(NavigatePage),
    /// Traverse browser history.
    NavigateHistory(NavigateHistory),
    /// Reload a controlled page.
    ReloadPage(ReloadPage),
    /// Close an exact controlled tab.
    CloseTab(CloseTab),
    /// Read bounded page text.
    ReadPage(ReadPage),
    /// Inspect semantic page facts.
    InspectPage(InspectPage),
    /// Find semantic targets.
    Find(Find),
    /// Capture a screenshot.
    TakeScreenshot(TakeScreenshot),
    /// Activate a semantic target.
    Click(Click),
    /// Scroll a page or reveal a target.
    ScrollPage(ScrollPage),
    /// Set visible tab zoom.
    SetZoom(SetZoom),
    /// Resize a browser window through one controlled tab.
    ResizeWindow(ResizeWindow),
    /// Hover a target or governed view point.
    Hover(Hover),
    /// Fill ordinary form fields.
    FillForm(FillForm),
    /// Type ordinary text through input events.
    TypeText(TypeText),
    /// Send a keyboard action.
    PressKey(PressKey),
    /// Drag targets or governed view points.
    Drag(Drag),
    /// Upload explicitly named local files.
    UploadFiles(UploadFiles),
    /// Evaluate an explicit bounded page script.
    RunScript(RunScript),
    /// Wait for an explicit condition.
    Wait(Wait),
    /// Run a short known sequence.
    RunSequence(RunSequence),
    /// Run one governed result-aware flow of decoded steps.
    RunFlow(RunFlow),
    /// Resolve a browser dialog.
    HandleDialog(HandleDialog),
    /// Control one memory-only browser recording.
    Record(Record),
    /// Read bounded opt-in browser diagnostics.
    Diagnose(Diagnose),

    /// Explain the authority in force (ADR-0136).
    ExplainPolicy(ExplainPolicy),
}

impl Operation {
    /// Return caller restrictions carried by this operation.
    #[must_use]
    pub fn restrictions(&self) -> &RequestRestrictions {
        match self {
            Self::ListTabs(value) => &value.restrictions,
            Self::ActivateTab(value) => &value.restrictions,
            Self::OpenPage(value) => &value.restrictions,
            Self::NavigatePage(value) => &value.restrictions,
            Self::NavigateHistory(value) => &value.restrictions,
            Self::ReloadPage(value) => &value.restrictions,
            Self::CloseTab(value) => &value.restrictions,
            Self::ReadPage(value) => &value.restrictions,
            Self::InspectPage(value) => &value.restrictions,
            Self::Find(value) => &value.restrictions,
            Self::TakeScreenshot(value) => &value.restrictions,
            Self::Click(value) => &value.restrictions,
            Self::ScrollPage(value) => &value.restrictions,
            Self::SetZoom(value) => &value.restrictions,
            Self::ResizeWindow(value) => &value.restrictions,
            Self::Hover(value) => &value.restrictions,
            Self::FillForm(value) => &value.restrictions,
            Self::TypeText(value) => &value.restrictions,
            Self::PressKey(value) => &value.restrictions,
            Self::Drag(value) => &value.restrictions,
            Self::UploadFiles(value) => &value.restrictions,
            Self::RunScript(value) => &value.restrictions,
            Self::Wait(value) => &value.restrictions,
            Self::RunSequence(value) => &value.restrictions,
            Self::RunFlow(value) => &value.restrictions,
            Self::HandleDialog(value) => &value.restrictions,
            Self::Record(value) => &value.restrictions,
            Self::Diagnose(value) => &value.restrictions,
            Self::ExplainPolicy(value) => &value.restrictions,
        }
    }

    /// Return the exact catalog name of this operation.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ListTabs(_) | Self::ActivateTab(_) | Self::CloseTab(_) => "browser_tabs",
            Self::OpenPage(_) | Self::NavigatePage(_) => "browser_navigate",
            Self::NavigateHistory(_) | Self::ReloadPage(_) => "browser_history",
            Self::ReadPage(_) => "browser_read",
            Self::InspectPage(_) => "browser_inspect",
            Self::Find(_) => "browser_find",
            Self::TakeScreenshot(_) => "browser_screenshot",
            Self::Click(_) => "browser_click",
            Self::ScrollPage(_) => "browser_scroll",
            Self::SetZoom(_) | Self::ResizeWindow(_) => "browser_window",
            Self::Hover(_) => "browser_hover",
            Self::FillForm(_) => "browser_fill_form",
            Self::TypeText(_) => "browser_type_text",
            Self::PressKey(_) => "browser_press_key",
            Self::Drag(_) => "browser_drag",
            Self::UploadFiles(_) => "browser_upload",
            Self::RunScript(_) => "browser_execute",
            Self::Wait(_) => "browser_wait",
            Self::RunSequence(_) => "browser_sequence",
            Self::RunFlow(_) => "browser_flow",
            Self::HandleDialog(_) => "browser_dialog",
            Self::Record(_) => "browser_record",
            Self::Diagnose(_) => "browser_diagnose",
            Self::ExplainPolicy(_) => "policy_explain",
        }
    }
}

/// Model input for the cohesive tab controller.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TabsRequest {
    pub action: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Model input for the cohesive history controller.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct HistoryRequest {
    pub action: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub bypass_cache: bool,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Model input for opening or reusing a controlled browser tab.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct NavigateRequest {
    pub url: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub browser: Option<String>,
    #[serde(default)]
    pub new_tab: bool,
    /// Whether a fresh open may adopt an unbound same-host tab (ADR-0137). Defaults to domain
    /// reuse; `new_tab` always creates fresh regardless of this field.
    #[serde(default)]
    pub reuse: Option<String>,
    /// Explicitly discard a blocking unsaved-change prompt from this navigation.
    #[serde(default)]
    pub beforeunload: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Model input for the cohesive browser-window controller.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct WindowRequest {
    pub action: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub percent: Option<u16>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for listing controlled tabs.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ListTabs {
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for bringing one exact tab into view.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ActivateTab {
    pub tab: String,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Whether opening a page may adopt an existing unbound same-host tab (ADR-0137).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReusePolicy {
    /// Adopt the nearest unbound same-host tab, exact URL preferred; create only when none.
    #[default]
    Domain,
    /// Always create a fresh tab.
    Never,
}

impl ReusePolicy {
    /// The wire token sent to the browser adapter.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Never => "never",
        }
    }
}

/// Input for opening a page.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct OpenPage {
    pub url: String,
    /// Which connected browser to open in.
    ///
    /// Opening the first tab is the one moment a workspace has no browser yet, so it is the one
    /// call that can name one. Omitting it is the ordinary case: Ghostlight uses the browser the
    /// person most recently attended.
    #[serde(default)]
    pub browser: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Whether the open may adopt an unbound same-host tab instead of creating one.
    #[serde(default)]
    pub reuse: ReusePolicy,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for navigating a page.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct NavigatePage {
    pub url: String,
    /// Discard this navigation's own beforeunload prompt.
    #[serde(default)]
    pub beforeunload_discard: bool,
    #[serde(default)]
    pub tab: Option<String>,
    /// Applied only when a workspace with no tabs falls back to the open path (ADR-0137).
    #[serde(default)]
    pub reuse: ReusePolicy,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for browser history traversal.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct NavigateHistory {
    pub direction: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for reloading a controlled tab.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ReloadPage {
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub bypass_cache: bool,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for closing an exact tab.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CloseTab {
    pub tab: String,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for bounded page reading.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ReadPage {
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    /// Full-page visible text (default) or article extraction; target reads ignore it.
    #[serde(default)]
    pub mode: Option<ReadMode>,
    #[serde(default = "default_max_chars")]
    pub max_chars: usize,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// The closed document-reading strategy.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadMode {
    /// Read the composed visible page across open shadow roots and embedded frames.
    Visible,
    /// Prefer the top document's article and fall back to the composed visible page.
    Article,
}

impl ReadMode {
    /// Return the stable browser-wire name for this mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Article => "article",
        }
    }
}

/// Input for semantic inspection.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct InspectPage {
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_inspect_kind")]
    pub scope: String,
    /// Optional subtree root for document scope.
    #[serde(default)]
    pub root: Option<String>,
    /// Optional bounded depth for document scope.
    #[serde(default)]
    pub max_depth: Option<usize>,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for semantic finding.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Find {
    pub text: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_find_kind")]
    pub scope: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for a screenshot.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TakeScreenshot {
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub full_page: bool,
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// One typed semantic selector resolved against the live document.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SemanticSelector {
    /// Required accessible-name text.
    pub name: String,
    /// Optional closed role filter.
    #[serde(default)]
    pub role: Option<String>,
    /// Require the whole accessible name to equal the text.
    #[serde(default)]
    pub exact: bool,
}

/// One optional typed expectation checked after an applied effect.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Postcondition {
    /// Closed observable condition from the shared wait vocabulary.
    pub condition: String,
    /// Required by the textual conditions.
    #[serde(default)]
    pub value: Option<String>,
}

/// Input for semantic activation.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Click {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub selector: Option<SemanticSelector>,
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_button")]
    pub button: String,
    #[serde(default = "default_click_count")]
    pub click_count: u8,
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Optional expectation checked after the applied effect.
    #[serde(default)]
    pub expect: Option<Postcondition>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for page scrolling or target reveal.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ScrollPage {
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub ticks: Option<u8>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for setting visible tab zoom.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SetZoom {
    pub percent: u16,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for resizing the browser window that contains a controlled tab.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ResizeWindow {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for semantic or screenshot-coordinate hover.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Hover {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// One ordinary form value.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormField {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub selector: Option<SemanticSelector>,
    pub value: FormFieldValue,
}

/// One typed ordinary form value.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum FormFieldValue {
    /// Checkbox or radio truth.
    Flag(bool),
    /// Finite number for numeric inputs.
    Number(f64),
    /// Literal text, select option value or label.
    Text(String),
}

/// Input for grouped form filling.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct FillForm {
    pub fields: Vec<FormField>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub submit_target: Option<String>,
    /// Optional expectation checked after the applied effect.
    #[serde(default)]
    pub expect: Option<Postcondition>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for typing ordinary text through browser input events.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TypeText {
    #[serde(default)]
    pub target: String,
    /// Type into the currently focused editable control instead of `target`.
    #[serde(default)]
    pub focused: bool,
    /// Resolve this semantic selector instead of using `target`.
    #[serde(default)]
    pub selector: Option<SemanticSelector>,
    pub text: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub clear_first: bool,
    /// Optional expectation checked after the applied effect.
    #[serde(default)]
    pub expect: Option<Postcondition>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for one keyboard action.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct PressKey {
    #[serde(default)]
    pub key: String,
    /// Ordered keystroke sequence replacing `key`; at most twenty entries.
    #[serde(default)]
    pub strokes: Vec<String>,
    /// Repetitions of the whole stroke sequence, one through one hundred.
    #[serde(default = "default_repeat")]
    pub repeat: u16,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Optional expectation checked after the applied effect.
    #[serde(default)]
    pub expect: Option<Postcondition>,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for semantic or screenshot-coordinate drag.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Drag {
    #[serde(default)]
    pub source_target: Option<String>,
    #[serde(default)]
    pub destination_target: Option<String>,
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub start_x: Option<f64>,
    #[serde(default)]
    pub start_y: Option<f64>,
    #[serde(default)]
    pub end_x: Option<f64>,
    #[serde(default)]
    pub end_y: Option<f64>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for bounded local file upload.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct UploadFiles {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub selector: Option<SemanticSelector>,
    /// Absolute local paths.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Bounded inline files supplied by the caller.
    #[serde(default)]
    pub files: Vec<InlineFile>,
    /// One captured image handle to attach or drop.
    #[serde(default)]
    pub source_image: Option<String>,
    /// Drop-point view; only valid with source_image.
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// One bounded inline file supplied directly by the caller.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct InlineFile {
    /// Bounded file name.
    pub name: String,
    /// Bounded media type.
    #[serde(default = "default_media_type")]
    pub media_type: String,
    /// Base64-encoded bytes.
    pub data_base64: String,
}

fn default_media_type() -> String {
    "application/octet-stream".into()
}

/// Input for explicit page script evaluation.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RunScript {
    pub script: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_max_chars")]
    pub max_result_chars: usize,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for one explicit wait.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Wait {
    pub condition: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    /// Typed semantic selector for `selector_present`: waiting on what the page calls a
    /// control, without pre-resolving any handle.
    #[serde(default)]
    pub selector: Option<SemanticSelector>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// One flat step in a short sequence.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum SequenceStep {
    /// Activate a current target.
    Click {
        target: String,
        #[serde(default = "default_button")]
        button: String,
        #[serde(default = "default_click_count")]
        click_count: u8,
    },
    /// Fill one ordinary field.
    Fill { target: String, value: String },
    /// Type ordinary text through browser input events.
    TypeText {
        target: String,
        text: String,
        #[serde(default)]
        clear_first: bool,
    },
    /// Send one keyboard action.
    PressKey {
        key: String,
        #[serde(default)]
        target: Option<String>,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    /// Scroll in a direction or reveal a target.
    Scroll {
        #[serde(default)]
        target: Option<String>,
        #[serde(default)]
        direction: Option<String>,
        #[serde(default)]
        amount: Option<String>,
    },
    /// Hover one current target.
    Hover { target: String },
    /// Observe one condition.
    Wait {
        condition: String,
        #[serde(default)]
        value: Option<String>,
        #[serde(default)]
        target: Option<String>,
    },
}

/// Input for a short sequence.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RunSequence {
    pub steps: Vec<SequenceStep>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// One explicit result reference embedded in a flow argument.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ResultReference {
    /// Earlier step id whose canonical result envelope is read.
    pub step: String,
    /// JSON Pointer into that envelope's structured content.
    pub pointer: String,
}

/// One named step of a governed flow.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct FlowStep {
    /// Unique step id within this flow.
    pub id: String,
    /// Current advertised non-composite Ghostlight tool.
    pub tool: String,
    /// Argument object; values may embed `{"flow_ref":{"step","pointer"}}`.
    #[serde(default)]
    pub arguments: Value,
}

/// Input for one governed result-aware flow.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RunFlow {
    pub steps: Vec<FlowStep>,
    #[serde(default = "default_on_error")]
    pub on_error: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

fn default_on_error() -> String {
    "stop".into()
}

/// Input for dialog handling.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct HandleDialog {
    pub action: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for the memory-only recording lifecycle.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Record {
    pub action: String,
    #[serde(default)]
    pub recording: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    /// Let the browser write the replay to a file instead of returning it.
    #[serde(default)]
    pub download: bool,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for bounded opt-in browser diagnostics.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Diagnose {
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_diagnostic_source")]
    pub source: String,
    #[serde(default = "default_diagnostic_detail")]
    pub detail: String,
    #[serde(default)]
    pub r#match: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default = "default_diagnostic_limit")]
    pub limit: usize,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for the always-available policy explain operation (ADR-0136).
///
/// There is nothing to configure: the projection is compiled from the authority in force, so the
/// input carries only the shared request restrictions every tool accepts.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ExplainPolicy {
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// A model-language validation failure before work starts.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LanguageError {
    /// The tool name is not in the catalog.
    #[error("unknown tool `{0}`")]
    UnknownTool(String),
    /// The input is not a valid object for this tool.
    #[error("invalid input: {0}")]
    Invalid(String),
}

impl LanguageError {
    /// The bare expectation or problem, phrased for use inside a next-step sentence.
    #[must_use]
    pub fn guidance(&self) -> String {
        match self {
            Self::UnknownTool(name) => {
                format!("`{name}` is not a Ghostlight tool; pick one from the advertised catalog.")
            }
            Self::Invalid(message) => message.clone(),
        }
    }
}

/// Decode and validate one catalog invocation.
pub fn decode(name: &str, input: Value) -> Result<Operation, LanguageError> {
    match name {
        "browser_tabs" => decode_tabs(input),
        "browser_navigate" => decode_navigate(input),
        "browser_history" => decode_history(input),
        "browser_window" => decode_window(input),
        "browser_read" => Operation::ReadPage(parse(
            input,
            &["tab", "target", "mode", "max_chars"],
            |value: &ReadPage| {
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_optional_handle(value.target.as_deref(), "target_")?;
                if value.mode.is_some() && value.target.is_some() {
                    return Err(LanguageError::Invalid(
                        "mode cannot be combined with target".into(),
                    ));
                }
                validate_range(value.max_chars, 500, 50_000, "max_chars")?;
                validate_restrictions(&value.restrictions)
            },
        )?)
        .into_ok(),
        "browser_inspect" => Operation::InspectPage(parse(
            input,
            &["tab", "scope", "root", "max_depth", "max_items"],
            |value: &InspectPage| {
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_choice(
                    &value.scope,
                    &["controls", "structure", "all", "document"],
                    "scope",
                )?;
                if value.scope == "document" {
                    validate_optional_handle(value.root.as_deref(), "target_")?;
                    if let Some(depth) = value.max_depth {
                        validate_range(depth, 1, 12, "max_depth")?;
                    }
                } else {
                    if value.root.is_some() || value.max_depth.is_some() {
                        return Err(LanguageError::Invalid(
                            "root and max_depth apply only to document scope".into(),
                        ));
                    }
                }
                validate_range(value.max_items, 1, 200, "max_items")?;
                validate_restrictions(&value.restrictions)
            },
        )?)
        .into_ok(),
        "browser_find" => Operation::Find(parse(
            input,
            &["text", "tab", "scope", "max_results"],
            |value: &Find| {
                validate_text(&value.text, 500, "text")?;
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_choice(&value.scope, &["any", "control", "text"], "scope")?;
                validate_range(value.max_results, 1, 50, "max_results")?;
                validate_restrictions(&value.restrictions)
            },
        )?)
        .into_ok(),
        "browser_screenshot" => Operation::TakeScreenshot(parse(
            input,
            &[
                "tab",
                "target",
                "full_page",
                "view",
                "x",
                "y",
                "width",
                "height",
                "timeout_ms",
            ],
            validate_screenshot,
        )?)
        .into_ok(),
        "browser_click" => Operation::Click(parse(
            input,
            &[
                "target",
                "selector",
                "view",
                "x",
                "y",
                "tab",
                "button",
                "click_count",
                "modifiers",
                "expect",
                "timeout_ms",
            ],
            validate_click,
        )?)
        .into_ok(),
        "browser_scroll" => Operation::ScrollPage(parse(
            input,
            &[
                "tab",
                "target",
                "direction",
                "amount",
                "view",
                "x",
                "y",
                "ticks",
                "timeout_ms",
            ],
            validate_scroll,
        )?)
        .into_ok(),
        "browser_hover" => Operation::Hover(parse(
            input,
            &["target", "view", "x", "y", "tab", "timeout_ms"],
            validate_hover,
        )?)
        .into_ok(),
        "browser_fill_form" => Operation::FillForm(parse(
            input,
            &["fields", "tab", "submit_target", "expect", "timeout_ms"],
            validate_fill,
        )?)
        .into_ok(),
        "browser_type_text" => Operation::TypeText(parse(
            input,
            &[
                "target",
                "focused",
                "selector",
                "text",
                "tab",
                "clear_first",
                "expect",
                "timeout_ms",
            ],
            validate_type_text,
        )?)
        .into_ok(),
        "browser_press_key" => Operation::PressKey(parse(
            input,
            &[
                "key",
                "strokes",
                "repeat",
                "tab",
                "target",
                "modifiers",
                "expect",
            ],
            validate_press_key,
        )?)
        .into_ok(),
        "browser_drag" => Operation::Drag(parse(
            input,
            &[
                "source_target",
                "destination_target",
                "view",
                "start_x",
                "start_y",
                "end_x",
                "end_y",
                "tab",
                "timeout_ms",
            ],
            validate_drag,
        )?)
        .into_ok(),
        "browser_upload" => Operation::UploadFiles(parse(
            input,
            &[
                "target",
                "selector",
                "paths",
                "files",
                "source_image",
                "view",
                "x",
                "y",
                "tab",
                "timeout_ms",
            ],
            validate_upload,
        )?)
        .into_ok(),
        "browser_execute" => Operation::RunScript(parse(
            input,
            &["script", "tab", "max_result_chars", "timeout_ms"],
            |value: &RunScript| {
                validate_text(&value.script, 20_000, "script")?;
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_range(value.max_result_chars, 100, 20_000, "max_result_chars")?;
                validate_timeout(value.timeout_ms)?;
                validate_restrictions(&value.restrictions)
            },
        )?)
        .into_ok(),
        "browser_wait" => Operation::Wait(parse(
            input,
            &[
                "condition",
                "tab",
                "value",
                "target",
                "selector",
                "timeout_ms",
            ],
            validate_wait,
        )?)
        .into_ok(),
        "browser_sequence" => Operation::RunSequence(parse(
            input,
            &["steps", "tab", "timeout_ms"],
            validate_sequence,
        )?)
        .into_ok(),
        "browser_flow" => Operation::RunFlow(parse(
            input,
            &["steps", "on_error", "dry_run", "tab", "timeout_ms"],
            validate_flow,
        )?)
        .into_ok(),
        "browser_dialog" => decode_dialog(input),
        "browser_record" => decode_record(input),
        "browser_diagnose" => decode_diagnose(input),
        "policy_explain" => decode_explain_policy(input),
        other => Err(LanguageError::UnknownTool(other.into())),
    }
}

trait OperationResult {
    fn into_ok(self) -> Result<Operation, LanguageError>;
}

impl OperationResult for Operation {
    fn into_ok(self) -> Result<Operation, LanguageError> {
        Ok(self)
    }
}

fn decode_tabs(input: Value) -> Result<Operation, LanguageError> {
    let value: TabsRequest = parse(input, &["action", "tab"], |value: &TabsRequest| {
        validate_choice(&value.action, &["list", "focus", "close"], "action")?;
        match value.action.as_str() {
            "list" if value.tab.is_some() => {
                return Err(LanguageError::Invalid(
                    "tab is not valid when action is list".into(),
                ))
            }
            "focus" | "close" => validate_handle(
                value.tab.as_deref().ok_or_else(|| {
                    LanguageError::Invalid(
                        "action focus and action close need a tab: the handle of the tab to act on"
                            .into(),
                    )
                })?,
                "tab_",
            )?,
            _ => {}
        }
        validate_restrictions(&value.restrictions)
    })?;
    let restrictions = value.restrictions;
    Ok(match value.action.as_str() {
        "list" => Operation::ListTabs(ListTabs { restrictions }),
        "focus" => Operation::ActivateTab(ActivateTab {
            tab: value.tab.expect("validated tab"),
            restrictions,
        }),
        "close" => Operation::CloseTab(CloseTab {
            tab: value.tab.expect("validated tab"),
            restrictions,
        }),
        _ => unreachable!("validated action"),
    })
}

fn decode_navigate(input: Value) -> Result<Operation, LanguageError> {
    let value: NavigateRequest = parse(
        input,
        &[
            "url",
            "tab",
            "browser",
            "new_tab",
            "reuse",
            "beforeunload",
            "timeout_ms",
        ],
        |value: &NavigateRequest| {
            validate_url(&value.url)?;
            validate_optional_handle(value.tab.as_deref(), "tab_")?;
            validate_optional_handle(value.browser.as_deref(), "browser_")?;
            if let Some(reuse) = value.reuse.as_deref() {
                validate_choice(reuse, &["domain", "never"], "reuse")?;
            }
            if value.new_tab && value.tab.is_some() {
                return Err(LanguageError::Invalid(
                    "tab and new_tab cannot be combined".into(),
                ));
            }
            if value.beforeunload.is_some() {
                if value.new_tab || value.browser.is_some() {
                    return Err(LanguageError::Invalid(
                        "beforeunload applies only to same-tab navigation".into(),
                    ));
                }
                validate_choice(
                    value.beforeunload.as_deref().expect("validated presence"),
                    &["discard"],
                    "beforeunload",
                )?;
            }
            if value.browser.is_some() && !value.new_tab {
                return Err(LanguageError::Invalid(
                    "browser is only valid when new_tab is true".into(),
                ));
            }
            validate_timeout(value.timeout_ms)?;
            validate_restrictions(&value.restrictions)
        },
    )?;
    Ok(if value.new_tab {
        let reuse = match value.reuse.as_deref() {
            // A fresh tab is the entire point of new_tab; reuse never applies there. An
            // explicit "domain" beside new_tab is contradictory, so it is refused rather than
            // silently ignored.
            Some("domain") => {
                return Err(LanguageError::Invalid(
                    "reuse cannot be combined with new_tab".into(),
                ))
            }
            _ => ReusePolicy::Never,
        };
        Operation::OpenPage(OpenPage {
            url: value.url,
            browser: value.browser,
            timeout_ms: value.timeout_ms,
            reuse,
            restrictions: value.restrictions,
        })
    } else {
        Operation::NavigatePage(NavigatePage {
            beforeunload_discard: value.beforeunload.as_deref() == Some("discard"),
            url: value.url,
            tab: value.tab,
            reuse: match value.reuse.as_deref() {
                Some("never") => ReusePolicy::Never,
                _ => ReusePolicy::Domain,
            },
            timeout_ms: value.timeout_ms,
            restrictions: value.restrictions,
        })
    })
}

fn decode_history(input: Value) -> Result<Operation, LanguageError> {
    let bypass_present = has_field(&input, "bypass_cache");
    let value: HistoryRequest = parse(
        input,
        &["action", "tab", "bypass_cache", "timeout_ms"],
        |value: &HistoryRequest| {
            validate_choice(&value.action, &["back", "forward", "reload"], "action")?;
            validate_optional_handle(value.tab.as_deref(), "tab_")?;
            if value.action != "reload" && bypass_present {
                return Err(LanguageError::Invalid(
                    "bypass_cache is only valid when action is reload".into(),
                ));
            }
            validate_timeout(value.timeout_ms)?;
            validate_restrictions(&value.restrictions)
        },
    )?;
    Ok(if value.action == "reload" {
        Operation::ReloadPage(ReloadPage {
            tab: value.tab,
            bypass_cache: value.bypass_cache,
            timeout_ms: value.timeout_ms,
            restrictions: value.restrictions,
        })
    } else {
        Operation::NavigateHistory(NavigateHistory {
            direction: value.action,
            tab: value.tab,
            timeout_ms: value.timeout_ms,
            restrictions: value.restrictions,
        })
    })
}

fn decode_window(input: Value) -> Result<Operation, LanguageError> {
    let value: WindowRequest = parse(
        input,
        &["action", "tab", "percent", "width", "height"],
        |value: &WindowRequest| {
            validate_choice(&value.action, &["zoom", "resize"], "action")?;
            validate_optional_handle(value.tab.as_deref(), "tab_")?;
            match value.action.as_str() {
                "zoom" => {
                    let percent = value
                        .percent
                        .ok_or_else(|| LanguageError::Invalid("action zoom needs percent: the zoom level as a whole number, 25 to 500".into()))?;
                    validate_range(usize::from(percent), 25, 500, "percent")?;
                    if value.width.is_some() || value.height.is_some() {
                        return Err(LanguageError::Invalid(
                            "width and height are only valid when action is resize".into(),
                        ));
                    }
                }
                "resize" => {
                    let width = value.width.ok_or_else(|| {
                        LanguageError::Invalid(
                            "action resize needs width and height: the outer window size in pixels"
                                .into(),
                        )
                    })?;
                    let height = value.height.ok_or_else(|| {
                        LanguageError::Invalid(
                            "action resize needs width and height: the outer window size in pixels"
                                .into(),
                        )
                    })?;
                    validate_range(width as usize, 320, 7_680, "width")?;
                    validate_range(height as usize, 240, 4_320, "height")?;
                    if value.percent.is_some() {
                        return Err(LanguageError::Invalid(
                            "percent is only valid when action is zoom".into(),
                        ));
                    }
                }
                _ => unreachable!("validated action"),
            }
            validate_restrictions(&value.restrictions)
        },
    )?;
    Ok(if value.action == "zoom" {
        Operation::SetZoom(SetZoom {
            percent: value.percent.expect("validated percent"),
            tab: value.tab,
            restrictions: value.restrictions,
        })
    } else {
        Operation::ResizeWindow(ResizeWindow {
            width: value.width.expect("validated width"),
            height: value.height.expect("validated height"),
            tab: value.tab,
            restrictions: value.restrictions,
        })
    })
}

fn decode_dialog(input: Value) -> Result<Operation, LanguageError> {
    let text_present = has_field(&input, "text");
    let value: HandleDialog = parse(input, &["action", "tab", "text"], |value: &HandleDialog| {
        validate_choice(
            &value.action,
            &["status", "accept", "dismiss", "respond"],
            "action",
        )?;
        validate_optional_handle(value.tab.as_deref(), "tab_")?;
        match value.action.as_str() {
            "respond" => validate_text_allow_empty(
                value.text.as_deref().ok_or_else(|| {
                    LanguageError::Invalid(
                        "action respond needs text: the prompt response, which may be empty".into(),
                    )
                })?,
                2_000,
                "text",
            )?,
            _ if text_present => {
                return Err(LanguageError::Invalid(
                    "text is only valid when action is respond".into(),
                ))
            }
            _ => {}
        }
        validate_restrictions(&value.restrictions)
    })?;
    Ok(Operation::HandleDialog(value))
}

fn decode_record(input: Value) -> Result<Operation, LanguageError> {
    let value: Record = parse(
        input,
        &["action", "recording", "tab", "target", "download"],
        |value: &Record| {
            validate_choice(
                &value.action,
                &["start", "status", "stop", "save", "discard"],
                "action",
            )?;
            validate_optional_handle(value.recording.as_deref(), "recording_")?;
            validate_optional_handle(value.tab.as_deref(), "tab_")?;
            validate_optional_handle(value.target.as_deref(), "target_")?;
            match value.action.as_str() {
                "start" if value.recording.is_some() || value.target.is_some() => {
                    return Err(LanguageError::Invalid(
                        "start accepts tab but not recording or target".into(),
                    ))
                }
                "start" => {}
                "save" if value.tab.is_some() => {
                    return Err(LanguageError::Invalid(
                        "tab is only valid when action is start".into(),
                    ))
                }
                // One replay goes to one place. Asking for two is a mistake worth naming rather
                // than a preference to resolve silently.
                "save" if value.target.is_some() && value.download => {
                    return Err(LanguageError::Invalid(
                        "save accepts target or download, not both".into(),
                    ))
                }
                "save" => {}
                _ if value.tab.is_some() || value.target.is_some() || value.download => {
                    return Err(LanguageError::Invalid(
                        "tab is only valid for start; target and download are only valid for save"
                            .into(),
                    ))
                }
                _ => {}
            }
            validate_restrictions(&value.restrictions)
        },
    )?;
    Ok(Operation::Record(value))
}

fn decode_diagnose(input: Value) -> Result<Operation, LanguageError> {
    let value: Diagnose = parse(
        input,
        &["tab", "source", "detail", "match", "after", "limit"],
        |value: &Diagnose| {
            validate_optional_handle(value.tab.as_deref(), "tab_")?;
            validate_choice(&value.source, &["both", "console", "network"], "source")?;
            validate_choice(&value.detail, &["problems", "all"], "detail")?;
            if let Some(pattern) = &value.r#match {
                validate_text(pattern, 500, "match")?;
            }
            validate_optional_handle(value.after.as_deref(), "diag_")?;
            validate_range(value.limit, 1, 200, "limit")?;
            validate_restrictions(&value.restrictions)
        },
    )?;
    Ok(Operation::Diagnose(value))
}

fn decode_explain_policy(input: Value) -> Result<Operation, LanguageError> {
    let value: ExplainPolicy = parse(input, &[], |_| Ok(()))?;
    Ok(Operation::ExplainPolicy(value))
}

fn has_field(input: &Value, field: &str) -> bool {
    input
        .as_object()
        .is_some_and(|object| object.contains_key(field))
}
fn parse<T: DeserializeOwned>(
    input: Value,
    fields: &[&str],
    validate: impl FnOnce(&T) -> Result<(), LanguageError>,
) -> Result<T, LanguageError> {
    ensure_fields(&input, fields)?;
    let value: T =
        serde_json::from_value(input).map_err(|error| LanguageError::Invalid(error.to_string()))?;
    validate(&value)?;
    Ok(value)
}

fn ensure_fields(input: &Value, fields: &[&str]) -> Result<(), LanguageError> {
    let object = input
        .as_object()
        .ok_or_else(|| LanguageError::Invalid("input must be an object".into()))?;
    for key in object.keys() {
        if !fields.contains(&key.as_str()) && !COMMON_FIELDS.contains(&key.as_str()) {
            return Err(LanguageError::Invalid(format!(
                "unknown field `{key}`: check this tool's advertised fields"
            )));
        }
    }
    Ok(())
}

fn validate_selector(selector: &SemanticSelector) -> Result<(), LanguageError> {
    validate_text(&selector.name, 500, "name")?;
    if let Some(role) = &selector.role {
        validate_choice(role, SEMANTIC_ROLES, "role")?;
    }
    Ok(())
}

/// Composite tools a flow step may never name.
const FLOW_FORBIDDEN_TOOLS: &[&str] = &["browser_flow", "browser_sequence"];
const FLOW_STEP_LIMIT: usize = 20;
const FLOW_REF_POINTER_LIMIT: usize = 512;
const FLOW_REF_DEPTH_LIMIT: usize = 32;

fn validate_flow(value: &RunFlow) -> Result<(), LanguageError> {
    validate_range(value.steps.len(), 1, FLOW_STEP_LIMIT, "steps")?;
    validate_choice(&value.on_error, &["stop", "continue"], "on_error")?;
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_timeout(value.timeout_ms)?;
    let mut seen: Vec<&str> = Vec::with_capacity(value.steps.len());
    for (index, step) in value.steps.iter().enumerate() {
        validate_text(&step.id, 64, "id")?;
        if seen.contains(&step.id.as_str()) {
            return Err(LanguageError::Invalid(format!(
                "step id `{}` is not unique",
                step.id
            )));
        }
        seen.push(&step.id);
        if FLOW_FORBIDDEN_TOOLS.contains(&step.tool.as_str()) {
            return Err(LanguageError::Invalid(
                "a flow step may not name a composite tool".into(),
            ));
        }
        if !crate::language::capability_map::DIRECTORY
            .iter()
            .any(|entry| entry.tool == step.tool)
        {
            return Err(LanguageError::Invalid(format!(
                "step tool `{}` is not an advertised Ghostlight tool",
                step.tool
            )));
        }
        if !step.arguments.is_object() {
            return Err(LanguageError::Invalid(
                "step arguments must be an object".into(),
            ));
        }
        if has_restriction_fields(&step.arguments) {
            return Err(LanguageError::Invalid(
                "flow steps do not accept their own restrictions; the flow's apply".into(),
            ));
        }
        validate_flow_references(&step.arguments, &seen[..seen.len() - 1], index)?;
    }
    validate_restrictions(&value.restrictions)
}

fn has_restriction_fields(input: &Value) -> bool {
    input
        .as_object()
        .is_some_and(|object| object.contains_key("restrict_hosts"))
        || input
            .as_object()
            .is_some_and(|object| object.contains_key("restrict_capabilities"))
}

fn validate_flow_references(
    input: &Value,
    earlier: &[&str],
    index: usize,
) -> Result<(), LanguageError> {
    match input {
        Value::Object(object) => {
            if let Some(reference) = object.get("flow_ref") {
                if object.len() != 1 {
                    return Err(LanguageError::Invalid(
                        "a flow reference object must contain only `flow_ref`".into(),
                    ));
                }
                let parsed: ResultReference =
                    serde_json::from_value(reference.clone()).map_err(|_| {
                        LanguageError::Invalid(
                            "flow_ref requires `step` and `pointer` strings".into(),
                        )
                    })?;
                if !earlier.contains(&parsed.step.as_str()) {
                    return Err(LanguageError::Invalid(format!(
                        "step {index} references `{}`, which is missing or later",
                        parsed.step
                    )));
                }
                let pointer = parsed.pointer.as_bytes();
                if pointer.is_empty()
                    || pointer[0] != b'/'
                    || parsed.pointer.len() > FLOW_REF_POINTER_LIMIT
                    || parsed.pointer.matches('/').count() > FLOW_REF_DEPTH_LIMIT
                {
                    return Err(LanguageError::Invalid(
                        "flow_ref pointer must be a bounded JSON Pointer starting with `/`".into(),
                    ));
                }
                return Ok(());
            }
            for nested in object.values() {
                validate_flow_references(nested, earlier, index)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for item in items {
                validate_flow_references(item, earlier, index)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_expect(expect: &Option<Postcondition>) -> Result<(), LanguageError> {
    let Some(expectation) = expect else {
        return Ok(());
    };
    match expectation.condition.as_str() {
        "load_ready" => {
            if expectation.value.is_some() {
                return Err(LanguageError::Invalid("load_ready accepts no value".into()));
            }
            Ok(())
        }
        "url_contains" | "text_present" | "text_absent" => {
            let value = expectation.value.as_deref().ok_or_else(|| {
                LanguageError::Invalid(format!("{} requires value", expectation.condition))
            })?;
            validate_text(value, 2_000, "value")
        }
        _ => Err(LanguageError::Invalid(
            "expect supports load_ready, url_contains, text_present, or text_absent".into(),
        )),
    }
}

fn validate_click(value: &Click) -> Result<(), LanguageError> {
    if let Some(selector) = &value.selector {
        if value.target.is_some() || value.view.is_some() || value.x.is_some() || value.y.is_some()
        {
            return Err(LanguageError::Invalid(
                "selector cannot be combined with target, view, or coordinates".into(),
            ));
        }
        validate_selector(selector)?;
    } else {
        validate_location(
            value.target.as_deref(),
            value.view.as_deref(),
            value.x,
            value.y,
        )?;
    }
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_choice(&value.button, &["primary", "middle", "secondary"], "button")?;
    validate_range(usize::from(value.click_count), 1, 3, "click_count")?;
    validate_modifiers(&value.modifiers)?;
    validate_expect(&value.expect)?;
    validate_timeout(value.timeout_ms)?;
    validate_restrictions(&value.restrictions)
}

fn validate_screenshot(value: &TakeScreenshot) -> Result<(), LanguageError> {
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    let region_requested = value.view.is_some()
        || value.x.is_some()
        || value.y.is_some()
        || value.width.is_some()
        || value.height.is_some();
    if region_requested {
        if value.target.is_some() || value.full_page {
            return Err(LanguageError::Invalid(
                "view region cannot be combined with target or full_page".into(),
            ));
        }
        validate_handle(
            value
                .view
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("region work needs a current view handle: take a screenshot, then pass its view with these coordinates".into()))?,
            "view_",
        )?;
        for (name, coordinate) in [("x", value.x), ("y", value.y)] {
            validate_coordinate(
                coordinate.ok_or_else(|| LanguageError::Invalid("region capture needs x and y: the top-left corner of the rectangle in view coordinates".into()))?,
                name,
            )?;
        }
        for (name, extent) in [("width", value.width), ("height", value.height)] {
            validate_extent(
                extent.ok_or_else(|| {
                    LanguageError::Invalid(
                        "region capture needs width and height: the rectangle size in CSS pixels"
                            .into(),
                    )
                })?,
                name,
            )?;
        }
    } else {
        validate_optional_handle(value.target.as_deref(), "target_")?;
        if value.target.is_some() && value.full_page {
            return Err(LanguageError::Invalid(
                "target and full_page cannot be combined".into(),
            ));
        }
    }
    validate_timeout(value.timeout_ms)?;
    validate_restrictions(&value.restrictions)
}

fn validate_hover(value: &Hover) -> Result<(), LanguageError> {
    validate_location(
        value.target.as_deref(),
        value.view.as_deref(),
        value.x,
        value.y,
    )?;
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_timeout(value.timeout_ms)?;
    validate_restrictions(&value.restrictions)
}

fn validate_location(
    target: Option<&str>,
    view: Option<&str>,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), LanguageError> {
    match (target, view, x, y) {
        (Some(target), None, None, None) => validate_handle(target, "target_"),
        (None, Some(view), Some(x), Some(y)) => {
            validate_handle(view, "view_")?;
            validate_coordinate(x, "x")?;
            validate_coordinate(y, "y")
        }
        _ => Err(LanguageError::Invalid(
            "provide exactly target, or view with x and y".into(),
        )),
    }
}

fn validate_scroll(value: &ScrollPage) -> Result<(), LanguageError> {
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    let wheel =
        value.view.is_some() || value.x.is_some() || value.y.is_some() || value.ticks.is_some();
    if let Some(target) = &value.target {
        if wheel {
            return Err(LanguageError::Invalid(
                "target cannot be combined with a view point".into(),
            ));
        }
        validate_handle(target, "target_")?;
        if value.direction.is_some() || value.amount.is_some() {
            return Err(LanguageError::Invalid(
                "target cannot be combined with direction or amount".into(),
            ));
        }
    } else if wheel {
        let direction = value
            .direction
            .as_deref()
            .ok_or_else(|| LanguageError::Invalid("wheel scrolling requires direction".into()))?;
        validate_choice(direction, &["up", "down"], "direction")?;
        validate_handle(
            value
                .view
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("region work needs a current view handle: take a screenshot, then pass its view with these coordinates".into()))?,
            "view_",
        )?;
        for (name, coordinate) in [("x", value.x), ("y", value.y)] {
            validate_coordinate(
                coordinate.ok_or_else(|| LanguageError::Invalid("coordinate wheel scrolling needs x and y: the point to scroll at, in view coordinates".into()))?,
                name,
            )?;
        }
        validate_range(
            usize::from(value.ticks.ok_or_else(|| {
                LanguageError::Invalid(
                    "coordinate wheel scrolling needs ticks: how far to scroll, 1 to 10".into(),
                )
            })?),
            1,
            10,
            "ticks",
        )?;
    } else {
        if let Some(direction) = &value.direction {
            validate_choice(direction, &["up", "down", "left", "right"], "direction")?;
        }
        if let Some(amount) = &value.amount {
            validate_choice(amount, &["small", "medium", "large", "page"], "amount")?;
        }
    }
    validate_timeout(value.timeout_ms)?;
    validate_restrictions(&value.restrictions)
}

fn validate_fill(value: &FillForm) -> Result<(), LanguageError> {
    validate_range(value.fields.len(), 1, 30, "fields")?;
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_optional_handle(value.submit_target.as_deref(), "target_")?;
    validate_expect(&value.expect)?;
    validate_timeout(value.timeout_ms)?;
    for field in &value.fields {
        let branches = usize::from(field.target.is_some()) + usize::from(field.selector.is_some());
        if branches != 1 {
            return Err(LanguageError::Invalid(
                "each field provides exactly one of target or selector".into(),
            ));
        }
        if let Some(target) = &field.target {
            validate_handle(target, "target_")?;
        }
        if let Some(selector) = &field.selector {
            validate_selector(selector)?;
        }
        if let FormFieldValue::Text(text) = &field.value {
            validate_text_allow_empty(text, 8_000, "field value")?;
        }
    }
    validate_restrictions(&value.restrictions)
}

fn validate_type_text(value: &TypeText) -> Result<(), LanguageError> {
    let branches = usize::from(!value.target.is_empty())
        + usize::from(value.focused)
        + usize::from(value.selector.is_some());
    if branches != 1 {
        return Err(LanguageError::Invalid(
            "provide exactly one of target, focused, or selector".into(),
        ));
    }
    if !value.target.is_empty() {
        validate_handle(&value.target, "target_")?;
    }
    if let Some(selector) = &value.selector {
        validate_selector(selector)?;
    }
    if value.text.is_empty() && !value.clear_first {
        return Err(LanguageError::Invalid(
            "text cannot be empty unless clear_first is true".into(),
        ));
    }
    validate_text_allow_empty(&value.text, 8_000, "text")?;
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_expect(&value.expect)?;
    validate_timeout(value.timeout_ms)?;
    validate_restrictions(&value.restrictions)
}

fn validate_drag(value: &Drag) -> Result<(), LanguageError> {
    let targets = value.source_target.is_some() || value.destination_target.is_some();
    let points = value.view.is_some()
        || value.start_x.is_some()
        || value.start_y.is_some()
        || value.end_x.is_some()
        || value.end_y.is_some();
    if targets == points {
        return Err(LanguageError::Invalid(
            "provide exactly target endpoints or view coordinates".into(),
        ));
    }
    if targets {
        validate_handle(
            value
                .source_target
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("drag by targets needs source_target and destination_target: current handles from browser_find or browser_inspect".into()))?,
            "target_",
        )?;
        validate_handle(
            value
                .destination_target
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("drag by targets needs destination_target as well as source_target: where to drop".into()))?,
            "target_",
        )?;
    } else {
        validate_handle(
            value
                .view
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("region work needs a current view handle: take a screenshot, then pass its view with these coordinates".into()))?,
            "view_",
        )?;
        for (name, coordinate) in [
            ("start_x", value.start_x),
            ("start_y", value.start_y),
            ("end_x", value.end_x),
            ("end_y", value.end_y),
        ] {
            validate_coordinate(
                coordinate.ok_or_else(|| LanguageError::Invalid("drag by coordinates needs all four of start_x, start_y, end_x, and end_y: the two view points to drag between".into()))?,
                name,
            )?;
        }
    }
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_timeout(value.timeout_ms)?;
    validate_restrictions(&value.restrictions)
}

const INLINE_FILE_LIMIT: usize = 5;
const UPLOAD_AGGREGATE_BYTES: usize = 5_000_000;

fn validate_upload(value: &UploadFiles) -> Result<(), LanguageError> {
    let sources = usize::from(!value.paths.is_empty())
        + usize::from(!value.files.is_empty())
        + usize::from(value.source_image.is_some());
    if sources != 1 {
        return Err(LanguageError::Invalid(
            "provide exactly one of paths, files, or source_image".into(),
        ));
    }
    let dropping = value.view.is_some() || value.x.is_some() || value.y.is_some();
    if !value.paths.is_empty() || !value.files.is_empty() {
        if dropping {
            return Err(LanguageError::Invalid(
                "only a source_image may be dropped at a view point".into(),
            ));
        }
        let branches = usize::from(value.target.is_some()) + usize::from(value.selector.is_some());
        if branches != 1 {
            return Err(LanguageError::Invalid(
                "provide exactly one of target or selector".into(),
            ));
        }
    }
    if let Some(selector) = &value.selector {
        validate_selector(selector)?;
    }
    if dropping {
        if value.source_image.is_none() {
            return Err(LanguageError::Invalid(
                "a drop requires source_image".into(),
            ));
        }
        if value.target.is_some() || value.selector.is_some() {
            return Err(LanguageError::Invalid(
                "a drop provides a view point, not target or selector".into(),
            ));
        }
        validate_handle(
            value
                .view
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("region work needs a current view handle: take a screenshot, then pass its view with these coordinates".into()))?,
            "view_",
        )?;
        for (name, coordinate) in [("x", value.x), ("y", value.y)] {
            validate_coordinate(
                coordinate.ok_or_else(|| {
                    LanguageError::Invalid(
                        "dropping an image needs x and y: the view point to drop at".into(),
                    )
                })?,
                name,
            )?;
        }
    } else if let Some(image) = &value.source_image {
        validate_handle(image, "image_")?;
        if value.target.is_none() && value.selector.is_none() {
            return Err(LanguageError::Invalid(
                "attaching an image requires target or selector".into(),
            ));
        }
    }
    if !value.paths.is_empty() {
        validate_range(value.paths.len(), 1, 5, "paths")?;
        if has_duplicates(&value.paths) {
            return Err(LanguageError::Invalid("paths must be unique".into()));
        }
        for path in &value.paths {
            validate_text(path, 4_096, "path")?;
            if !Path::new(path).is_absolute() {
                return Err(LanguageError::Invalid("paths must be absolute".into()));
            }
        }
    } else {
        validate_range(value.files.len(), 0, INLINE_FILE_LIMIT, "files")?;
        let mut aggregate = 0usize;
        for file in &value.files {
            validate_text(&file.name, 255, "name")?;
            validate_text(&file.media_type, 100, "media_type")?;
            let data = file.data_base64.as_bytes();
            if data.is_empty() || data.len() % 4 != 0 {
                return Err(LanguageError::Invalid(
                    "data_base64 must be non-empty valid base64".into(),
                ));
            }
            if !data.iter().all(|byte| {
                byte.is_ascii_alphanumeric() || *byte == b'+' || *byte == b'/' || *byte == b'='
            }) {
                return Err(LanguageError::Invalid(
                    "data_base64 must be standard base64".into(),
                ));
            }
            aggregate = aggregate.saturating_add(data.len() / 4 * 3);
        }
        if aggregate > UPLOAD_AGGREGATE_BYTES {
            return Err(LanguageError::Invalid(
                "inline files exceed the 5,000,000-byte upload ceiling".into(),
            ));
        }
    }
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_timeout(value.timeout_ms)?;
    validate_restrictions(&value.restrictions)
}

fn validate_coordinate(value: f64, field: &str) -> Result<(), LanguageError> {
    if value.is_finite() && (0.0..=1_000_000.0).contains(&value) {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} must be a finite non-negative coordinate"
        )))
    }
}

fn validate_extent(value: f64, field: &str) -> Result<(), LanguageError> {
    if value.is_finite() && (0.0..=1_000_000.0).contains(&value) && value > 0.0 {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} must be a finite positive extent"
        )))
    }
}

fn validate_press_key(value: &PressKey) -> Result<(), LanguageError> {
    if value.strokes.is_empty() {
        validate_key(&value.key)?;
        if value.repeat != 1 {
            return Err(LanguageError::Invalid(
                "repeat applies to a stroke sequence".into(),
            ));
        }
    } else {
        if !value.key.is_empty() {
            return Err(LanguageError::Invalid(
                "provide exactly key or strokes".into(),
            ));
        }
        validate_range(value.strokes.len(), 1, 20, "strokes")?;
        for stroke in &value.strokes {
            validate_key(stroke)?;
        }
        validate_range(usize::from(value.repeat), 1, 100, "repeat")?;
    }
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_optional_handle(value.target.as_deref(), "target_")?;
    validate_modifiers(&value.modifiers)?;
    validate_expect(&value.expect)?;
    validate_restrictions(&value.restrictions)
}

fn validate_wait(value: &Wait) -> Result<(), LanguageError> {
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_timeout(value.timeout_ms)?;
    if value.condition == "selector_present" {
        let selector = value
            .selector
            .as_ref()
            .ok_or_else(|| LanguageError::Invalid("selector_present requires selector".into()))?;
        if selector.name.trim().is_empty() {
            return Err(LanguageError::Invalid(
                "selector_present requires a non-empty name".into(),
            ));
        }
        if value.value.is_some() || value.target.is_some() {
            return Err(LanguageError::Invalid(
                "selector_present accepts neither value nor target".into(),
            ));
        }
        return validate_restrictions(&value.restrictions);
    }
    if value.selector.is_some() {
        return Err(LanguageError::Invalid(
            "only selector_present accepts selector".into(),
        ));
    }
    validate_condition(
        &value.condition,
        value.value.as_deref(),
        value.target.as_deref(),
    )?;
    validate_restrictions(&value.restrictions)
}

fn validate_sequence(value: &RunSequence) -> Result<(), LanguageError> {
    validate_range(value.steps.len(), 2, 8, "steps")?;
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_timeout(value.timeout_ms)?;
    for step in &value.steps {
        match step {
            SequenceStep::Click {
                target,
                button,
                click_count,
            } => {
                validate_handle(target, "target_")?;
                validate_choice(button, &["primary", "middle", "secondary"], "button")?;
                validate_range(usize::from(*click_count), 1, 2, "click_count")?;
            }
            SequenceStep::Fill { target, value } => {
                validate_handle(target, "target_")?;
                validate_text_allow_empty(value, 8_000, "value")?;
            }
            SequenceStep::TypeText {
                target,
                text,
                clear_first,
            } => {
                validate_handle(target, "target_")?;
                if text.is_empty() && !clear_first {
                    return Err(LanguageError::Invalid(
                        "text cannot be empty unless clear_first is true".into(),
                    ));
                }
                validate_text_allow_empty(text, 8_000, "text")?;
            }
            SequenceStep::PressKey {
                key,
                target,
                modifiers,
            } => {
                validate_key(key)?;
                validate_optional_handle(target.as_deref(), "target_")?;
                validate_modifiers(modifiers)?;
            }
            SequenceStep::Scroll {
                target,
                direction,
                amount,
            } => {
                let step = ScrollPage {
                    tab: None,
                    target: target.clone(),
                    direction: direction.clone(),
                    amount: amount.clone(),
                    view: None,
                    x: None,
                    y: None,
                    ticks: None,
                    timeout_ms: DEFAULT_TIMEOUT_MS,
                    restrictions: RequestRestrictions::default(),
                };
                validate_scroll(&step)?;
            }
            SequenceStep::Hover { target } => validate_handle(target, "target_")?,
            SequenceStep::Wait {
                condition,
                value,
                target,
            } => {
                if condition == "duration" {
                    return Err(LanguageError::Invalid(
                        "duration waits do not run inside sequences".into(),
                    ));
                }
                validate_condition(condition, value.as_deref(), target.as_deref())?
            }
        }
    }
    validate_restrictions(&value.restrictions)
}

fn validate_condition(
    condition: &str,
    value: Option<&str>,
    target: Option<&str>,
) -> Result<(), LanguageError> {
    validate_choice(
        condition,
        &[
            "load_ready",
            "url_contains",
            "text_present",
            "text_absent",
            "target_present",
            "target_absent",
            "duration",
        ],
        "condition",
    )?;
    if condition == "duration" {
        let raw = value.ok_or_else(|| LanguageError::Invalid("duration requires value".into()))?;
        if target.is_some() {
            return Err(LanguageError::Invalid(
                "duration does not accept target".into(),
            ));
        }
        let milliseconds = raw
            .parse::<u64>()
            .map_err(|_| LanguageError::Invalid("duration requires whole milliseconds".into()))?;
        return validate_range(
            usize::try_from(milliseconds).unwrap_or(usize::MAX),
            0,
            10_000,
            "duration",
        );
    }
    match condition {
        "load_ready" if value.is_some() || target.is_some() => Err(LanguageError::Invalid(
            "load_ready accepts neither value nor target".into(),
        )),
        "url_contains" | "text_present" | "text_absent" => {
            let value = value
                .ok_or_else(|| LanguageError::Invalid(format!("{condition} requires value")))?;
            validate_text(value, 2_000, "value")?;
            if target.is_some() {
                Err(LanguageError::Invalid(format!(
                    "{condition} does not accept target"
                )))
            } else {
                Ok(())
            }
        }
        "target_present" | "target_absent" => {
            let target = target
                .ok_or_else(|| LanguageError::Invalid(format!("{condition} requires target")))?;
            validate_handle(target, "target_")?;
            if value.is_some() {
                Err(LanguageError::Invalid(format!(
                    "{condition} does not accept value"
                )))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

fn validate_restrictions(value: &RequestRestrictions) -> Result<(), LanguageError> {
    if let Some(hosts) = &value.restrict_hosts {
        if hosts.is_empty()
            || hosts.iter().any(|host| {
                host.trim().is_empty()
                    || host.len() > 253
                    || host.contains('/')
                    || host.contains(':')
                    || (host.contains('*') && !host.starts_with("*."))
            })
        {
            return Err(LanguageError::Invalid(
                "restrict_hosts must contain non-empty bounded patterns".into(),
            ));
        }
        if has_duplicates(hosts) {
            return Err(LanguageError::Invalid(
                "restrict_hosts must be unique".into(),
            ));
        }
    }
    if let Some(capabilities) = &value.restrict_capabilities {
        if capabilities.is_empty() {
            return Err(LanguageError::Invalid(
                "restrict_capabilities cannot be empty".into(),
            ));
        }
        for capability in capabilities {
            validate_choice(capability, CAPABILITIES, "restrict_capabilities")?;
        }
        if has_duplicates(capabilities) {
            return Err(LanguageError::Invalid(
                "restrict_capabilities must be unique".into(),
            ));
        }
    }
    Ok(())
}

fn validate_url(value: &str) -> Result<(), LanguageError> {
    if value.len() > 4_096 {
        return Err(LanguageError::Invalid(format!(
            "url is limited to 4096 bytes; got {}",
            value.len()
        )));
    }
    let parsed = Url::parse(value).map_err(|_| {
        LanguageError::Invalid(
            "url must be absolute: include the scheme, as in https://example.com".into(),
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(LanguageError::Invalid(
            "url must use http or https and include a host".into(),
        ));
    }
    Ok(())
}

fn validate_timeout(value: u64) -> Result<(), LanguageError> {
    if (MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&value) {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "timeout_ms must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}; got {value}"
        )))
    }
}

fn validate_range(
    value: usize,
    minimum: usize,
    maximum: usize,
    field: &str,
) -> Result<(), LanguageError> {
    if (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} must be between {minimum} and {maximum}; got {value}"
        )))
    }
}

fn validate_choice(value: &str, choices: &[&str], field: &str) -> Result<(), LanguageError> {
    if choices.contains(&value) {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} must be one of {}: got `{value}`",
            choices.join(", ")
        )))
    }
}

fn validate_handle(value: &str, prefix: &str) -> Result<(), LanguageError> {
    if value.starts_with(prefix) && value.len() > prefix.len() && value.len() <= 160 {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "handles start with {prefix}; pass one exactly as a previous result issued it"
        )))
    }
}

fn validate_optional_handle(value: Option<&str>, prefix: &str) -> Result<(), LanguageError> {
    value.map_or(Ok(()), |handle| validate_handle(handle, prefix))
}

fn validate_text(value: &str, maximum: usize, field: &str) -> Result<(), LanguageError> {
    if value.trim().is_empty() {
        return Err(LanguageError::Invalid(format!("{field} cannot be empty")));
    }
    validate_text_allow_empty(value, maximum, field)
}

fn validate_text_allow_empty(
    value: &str,
    maximum: usize,
    field: &str,
) -> Result<(), LanguageError> {
    let length = value.chars().count();
    if length <= maximum {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} is limited to {maximum} characters; got {length}"
        )))
    }
}

fn validate_modifiers(values: &[String]) -> Result<(), LanguageError> {
    let mut seen = Vec::new();
    for value in values {
        validate_choice(value, &["Alt", "Control", "Meta", "Shift"], "modifiers")?;
        if seen.contains(value) {
            return Err(LanguageError::Invalid("modifiers must be unique".into()));
        }
        seen.push(value.clone());
    }
    Ok(())
}

fn validate_key(value: &str) -> Result<(), LanguageError> {
    if NAMED_KEYS.contains(&value) || value.chars().count() == 1 {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "key must be one character or a supported named key: {}",
            NAMED_KEYS.join(", ")
        )))
    }
}

fn has_duplicates(values: &[String]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_MS
}
fn default_max_chars() -> usize {
    8_000
}
fn default_inspect_kind() -> String {
    "controls".into()
}
fn default_find_kind() -> String {
    "any".into()
}
fn default_max_items() -> usize {
    80
}
fn default_max_results() -> usize {
    20
}
fn default_button() -> String {
    "primary".into()
}
fn default_click_count() -> u8 {
    1
}

fn default_repeat() -> u16 {
    1
}
fn default_diagnostic_source() -> String {
    "both".into()
}
fn default_diagnostic_detail() -> String {
    "problems".into()
}
fn default_diagnostic_limit() -> usize {
    50
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{catalog, decode, LanguageError, Operation, ReadMode, ReusePolicy};

    #[test]
    fn catalog_has_unique_exact_tools_and_typo_closed_schemas() {
        let catalog = catalog();
        assert_eq!(catalog.len(), 24);
        let mut names: Vec<_> = catalog.iter().map(|tool| tool.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 24);
        for tool in catalog {
            assert!(tool.input_schema.is_object());
            assert!(tool.output_schema.is_some());
            assert!(tool.annotations.is_some());
        }
    }

    #[test]
    fn shortest_calls_receive_executable_defaults() {
        let Operation::NavigatePage(navigate) =
            decode("browser_navigate", json!({"url":"https://example.com"})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(navigate.timeout_ms, 8_000);
        assert_eq!(navigate.reuse, ReusePolicy::Domain);
        let Operation::ReadPage(read) = decode("browser_read", json!({})).unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(read.max_chars, 8_000);
        assert_eq!(read.mode, None);
        let Operation::ReadPage(article) =
            decode("browser_read", json!({"mode":"article"})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(article.mode, Some(ReadMode::Article));
        let Operation::InspectPage(inspect) = decode("browser_inspect", json!({})).unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(inspect.scope, "controls");
        assert_eq!(inspect.max_items, 80);
    }

    #[test]
    fn navigate_reuse_follows_the_documented_ladder() {
        let reuse_of = |input: serde_json::Value| match decode("browser_navigate", input).unwrap() {
            Operation::NavigatePage(value) => value.reuse,
            Operation::OpenPage(value) => value.reuse,
            other => panic!("wrong operation {other:?}"),
        };
        assert_eq!(
            reuse_of(json!({"url":"https://example.com","reuse":"never"})),
            ReusePolicy::Never
        );
        assert_eq!(
            reuse_of(json!({"url":"https://example.com","new_tab":true})),
            ReusePolicy::Never
        );
        assert!(matches!(
            decode(
                "browser_navigate",
                json!({"url":"https://example.com","reuse":"sometimes"})
            ),
            Err(LanguageError::Invalid(_))
        ));
        assert!(matches!(
            decode(
                "browser_navigate",
                json!({"url":"https://example.com","new_tab":true,"reuse":"domain"})
            ),
            Err(LanguageError::Invalid(_))
        ));
    }

    #[test]
    fn execute_replaces_the_unreleased_evaluate_name_without_an_alias() {
        let Operation::RunScript(script) =
            decode("browser_execute", json!({"script":"document.title"})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(script.script, "document.title");
        assert!(matches!(
            decode("browser_evaluate", json!({"script":"document.title"})),
            Err(LanguageError::UnknownTool(name)) if name == "browser_evaluate"
        ));
    }

    #[test]
    fn unknown_fields_and_ambiguous_waits_fail() {
        let error = decode("browser_read", json!({"max_chars":8000,"max_char":1})).unwrap_err();
        assert!(matches!(error, LanguageError::Invalid(message) if message.contains("max_char")));
        assert!(decode("browser_wait", json!({"condition":"text_present"})).is_err());
        assert!(decode(
            "browser_wait",
            json!({"condition":"target_present","value":"x"})
        )
        .is_err());
    }

    #[test]
    fn validation_messages_teach_the_expected_shape() {
        let error = decode("browser_tabs", json!({"action":"nope"})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid input: action must be one of list, focus, close: got `nope`"
        );
        let error = decode("browser_window", json!({"action":"zoom","percent":900})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid input: percent must be between 25 and 500; got 900"
        );
        let error = decode("browser_tabs", json!({"action":"close","tab":"nope"})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid input: handles start with tab_; pass one exactly as a previous result issued it"
        );
        let error = decode(
            "browser_press_key",
            json!({"key":"a","tab":"tab_x","oops":1}),
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .starts_with("invalid input: unknown field `oops`:"));
        let error = decode("browser_press_key", json!({"key":"PageLeft"})).unwrap_err();
        assert!(error
            .to_string()
            .starts_with("invalid input: key must be one character or a supported named key: "));
    }

    #[test]
    fn screenshot_target_and_full_page_are_mutually_exclusive() {
        assert!(decode(
            "browser_screenshot",
            json!({"target":"target_x","full_page":true})
        )
        .is_err());
    }

    #[test]
    fn invalid_input_guidance_teaches_the_expectation_without_the_diagnostic_prefix() {
        let error = decode(
            "browser_screenshot",
            json!({"tab":"tab_x","x":0,"y":0,"width":300,"height":200}),
        )
        .unwrap_err();
        assert_eq!(
            error.guidance(),
            "region work needs a current view handle: take a screenshot, then pass its view with these coordinates"
        );
        assert!(error.to_string().starts_with("invalid input: "));
        assert_eq!(
            LanguageError::UnknownTool("browser_evaluate".into()).guidance(),
            "`browser_evaluate` is not a Ghostlight tool; pick one from the advertised catalog."
        );
    }

    #[test]
    fn screenshot_region_uses_one_complete_current_view_rectangle() {
        let Operation::TakeScreenshot(capture) = decode(
            "browser_screenshot",
            json!({"view":"view_x","x":10.5,"y":20.25,"width":300,"height":200}),
        )
        .unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(capture.view.as_deref(), Some("view_x"));
        assert_eq!(capture.width, Some(300.0));
        for invalid in [
            json!({"view":"view_x","x":0,"y":0,"width":300}),
            json!({"view":"view_x","x":0,"y":0,"width":0,"height":200}),
            json!({"view":"view_x","x":0,"y":0,"width":300,"height":200,"full_page":true}),
            json!({"view":"view_x","x":0,"y":0,"width":300,"height":200,"target":"target_x"}),
        ] {
            assert!(decode("browser_screenshot", invalid).is_err());
        }
    }

    #[test]
    fn pointer_locations_are_exactly_semantic_or_view_bound() {
        assert!(decode(
            "browser_click",
            json!({"target":"target_x","view":"view_x","x":1,"y":1})
        )
        .is_err());
        assert!(decode("browser_click", json!({"x":1,"y":1})).is_err());
        let Operation::Click(click) =
            decode("browser_click", json!({"view":"view_x","x":10.5,"y":20.25})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(click.view.as_deref(), Some("view_x"));
    }

    #[test]
    fn scroll_defaults_are_contextual_and_upload_paths_are_absolute() {
        let Operation::ScrollPage(scroll) = decode("browser_scroll", json!({})).unwrap() else {
            panic!("wrong operation")
        };
        assert!(scroll.direction.is_none());
        assert!(scroll.amount.is_none());
        assert!(decode(
            "browser_scroll",
            json!({"target":"target_x","direction":"down"})
        )
        .is_err());
        assert!(decode(
            "browser_upload",
            json!({"target":"target_x","paths":["relative.txt"]})
        )
        .is_err());
    }

    #[test]
    fn action_families_reject_impossible_shapes() {
        assert!(decode("browser_tabs", json!({"action":"list","tab":"tab_x"})).is_err());
        assert!(decode(
            "browser_history",
            json!({"action":"back","bypass_cache":true})
        )
        .is_err());
        assert!(decode(
            "browser_window",
            json!({"action":"zoom","width":800,"height":600})
        )
        .is_err());
        assert!(decode("browser_dialog", json!({"action":"accept","text":"yes"})).is_err());
        assert!(decode(
            "browser_record",
            json!({"action":"stop","target":"target_x"})
        )
        .is_err());
    }

    #[test]
    fn flows_validate_steps_and_references_before_dispatch() {
        let ok = decode(
            "browser_flow",
            json!({"steps":[
                {"id":"list","tool":"browser_tabs","arguments":{"action":"list"}},
                {"id":"read","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"list","pointer":"/facts/tab"}}}}
            ]}),
        );
        assert!(ok.is_ok(), "backward references are accepted: {ok:?}");
        for invalid in [
            json!({"steps":[
                {"id":"a","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"b","pointer":"/x"}}}},
                {"id":"b","tool":"browser_tabs","arguments":{"action":"list"}}
            ]}),
            json!({"steps":[
                {"id":"a","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"ghost","pointer":"/x"}}}}
            ]}),
            json!({"steps":[{"id":"a","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"a","pointer":"no-slash"}}}}]}),
            json!({"steps":[{"id":"a","tool":"browser_flow","arguments":{}}]}),
            json!({"steps":[
                {"id":"dup","tool":"browser_tabs","arguments":{"action":"list"}},
                {"id":"dup","tool":"browser_read","arguments":{}}
            ]}),
            json!({"steps":[
                {"id":"a","tool":"browser_read","arguments":{"restrict_hosts":["example.com"]}}
            ]}),
        ] {
            assert!(
                decode("browser_flow", invalid.clone()).is_err(),
                "expected rejection: {invalid}"
            );
        }
    }
}
