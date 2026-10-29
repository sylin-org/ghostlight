//! The decoded language: one typed `Operation` per catalog call and the request types it
//! carries, with the defaults an omitted field takes.

use super::*;

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
    /// Return the exact catalog name of this operation.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.tool().name()
    }

    /// The advertised tool this operation came from.
    #[must_use]
    pub const fn tool(&self) -> Tool {
        match self {
            Self::ListTabs(_) | Self::ActivateTab(_) | Self::CloseTab(_) => Tool::Tabs,
            Self::OpenPage(_) | Self::NavigatePage(_) => Tool::Navigate,
            Self::NavigateHistory(_) | Self::ReloadPage(_) => Tool::History,
            Self::ReadPage(_) => Tool::Read,
            Self::InspectPage(_) => Tool::Inspect,
            Self::Find(_) => Tool::Find,
            Self::TakeScreenshot(_) => Tool::Screenshot,
            Self::Click(_) => Tool::Click,
            Self::ScrollPage(_) => Tool::Scroll,
            Self::SetZoom(_) | Self::ResizeWindow(_) => Tool::Window,
            Self::Hover(_) => Tool::Hover,
            Self::FillForm(_) => Tool::FillForm,
            Self::TypeText(_) => Tool::TypeText,
            Self::PressKey(_) => Tool::PressKey,
            Self::Drag(_) => Tool::Drag,
            Self::UploadFiles(_) => Tool::Upload,
            Self::RunScript(_) => Tool::Execute,
            Self::Wait(_) => Tool::Wait,
            Self::RunFlow(_) => Tool::Flow,
            Self::HandleDialog(_) => Tool::Dialog,
            Self::Record(_) => Tool::Record,
            Self::Diagnose(_) => Tool::Diagnose,
            Self::ExplainPolicy(_) => Tool::PolicyExplain,
        }
    }

    /// Return the caller's `timeout_ms`: the call time every tool accepts.
    ///
    /// The executor derives the invocation deadline from it, and [`Operation::timeout_can_grow`]
    /// decides from it whether a deadline refusal may suggest a longer `timeout_ms`.
    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        match self {
            Self::ListTabs(value) => value.timeout_ms,
            Self::ActivateTab(value) => value.timeout_ms,
            Self::OpenPage(value) => value.timeout_ms,
            Self::NavigatePage(value) => value.timeout_ms,
            Self::NavigateHistory(value) => value.timeout_ms,
            Self::ReloadPage(value) => value.timeout_ms,
            Self::CloseTab(value) => value.timeout_ms,
            Self::ReadPage(value) => value.timeout_ms,
            Self::InspectPage(value) => value.timeout_ms,
            Self::Find(value) => value.timeout_ms,
            Self::TakeScreenshot(value) => value.timeout_ms,
            Self::Click(value) => value.timeout_ms,
            Self::ScrollPage(value) => value.timeout_ms,
            Self::SetZoom(value) => value.timeout_ms,
            Self::ResizeWindow(value) => value.timeout_ms,
            Self::Hover(value) => value.timeout_ms,
            Self::FillForm(value) => value.timeout_ms,
            Self::TypeText(value) => value.timeout_ms,
            Self::PressKey(value) => value.timeout_ms,
            Self::Drag(value) => value.timeout_ms,
            Self::UploadFiles(value) => value.timeout_ms,
            Self::RunScript(value) => value.timeout_ms,
            Self::Wait(value) => value.timeout_ms,
            Self::RunFlow(value) => value.timeout_ms,
            Self::HandleDialog(value) => value.timeout_ms,
            Self::Record(value) => value.timeout_ms,
            Self::Diagnose(value) => value.timeout_ms,
            Self::ExplainPolicy(value) => value.timeout_ms,
        }
    }

    /// Whether repeating this call with a longer `timeout_ms` is a call the decoder accepts.
    ///
    /// True while the value in force is below the maximum. A call already at the maximum, such as
    /// `browser_fill_form` with its default, cannot be given more time, so a deadline refusal
    /// must not suggest it.
    #[must_use]
    pub fn timeout_can_grow(&self) -> bool {
        self.timeout_ms() < MAX_TIMEOUT_MS
    }
}

/// Model input for the cohesive tab controller.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TabsRequest {
    pub action: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
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
    /// Whether a fresh open may reuse a same-host tab Ghostlight opened and a released session of
    /// the same client left open (ADR-0196 Decision 9). Defaults to domain reuse; `new_tab` always
    /// creates fresh, so `domain` beside it is refused and `never` beside it changes nothing.
    #[serde(default)]
    pub reuse: Option<String>,
    /// Explicitly discard a blocking unsaved-change prompt from this navigation.
    #[serde(default)]
    pub beforeunload: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
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
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for listing controlled tabs.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ListTabs {
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for bringing one exact tab into view.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ActivateTab {
    pub tab: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Whether opening a page may reuse a tab instead of creating one (ADR-0137, ADR-0196).
///
/// The service makes the choice. Only a same-host tab Ghostlight opened and a released session of
/// the same client left open is ever a candidate; a tab the person opened never is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReusePolicy {
    /// Reuse a released Ghostlight tab of the same client on the same host, exact URL preferred;
    /// create only when none is left.
    #[default]
    Domain,
    /// Always create a fresh tab.
    Never,
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
    /// Whether the open may reuse a released Ghostlight tab of the same client on the same host
    /// instead of creating one.
    #[serde(default)]
    pub reuse: ReusePolicy,
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
}

/// Input for browser history traversal.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct NavigateHistory {
    pub direction: HistoryDirection,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
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
}

/// Input for closing an exact tab.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CloseTab {
    pub tab: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for bounded page reading.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ReadPage {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    /// Full-page visible text (default) or article extraction; target reads ignore it.
    #[serde(default)]
    pub mode: Option<ReadMode>,
    #[serde(default = "default_max_chars")]
    pub max_chars: usize,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
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
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_inspect_kind")]
    pub scope: InspectScope,
    /// Optional subtree root for document scope.
    #[serde(default)]
    pub root: Option<String>,
    /// Optional bounded depth for document scope.
    #[serde(default)]
    pub max_depth: Option<usize>,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for semantic finding.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Find {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
    pub text: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_find_kind")]
    pub scope: FindScope,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
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
    /// Optional layout-stability and visual quiescence check before capturing (defaults to true).
    #[serde(default)]
    pub visual_settle: Option<bool>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// One typed semantic selector resolved against the live document.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct Postcondition {
    /// Closed observable condition from the shared wait vocabulary.
    #[serde(deserialize_with = "wait::decode_expected")]
    pub condition: PageCondition,
    /// Required by the textual conditions.
    #[serde(default)]
    pub value: Option<String>,
}

/// Input for semantic activation.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Click {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
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
    pub button: PointerButton,
    #[serde(default = "default_click_count")]
    pub click_count: u8,
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Optional expectation checked after the applied effect.
    #[serde(default)]
    pub expect: Option<Postcondition>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for page scrolling or target reveal.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ScrollPage {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    /// The direction to scroll the page in; none lets the page choose (ADR-0205 Decision 3).
    #[serde(default)]
    pub direction: Option<ScrollDirection>,
    #[serde(default)]
    pub amount: Option<ScrollDistance>,
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
}

/// Input for setting visible tab zoom.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SetZoom {
    pub percent: u16,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for resizing the browser window that contains a controlled tab.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ResizeWindow {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for semantic or screenshot-coordinate hover.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Hover {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
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
    /// The options a multiple select ends up holding.
    Choices(Vec<String>),
}

/// Input for grouped form filling.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct FillForm {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
    pub fields: Vec<FormField>,
    /// Acknowledge the user's explicit instruction to enter credentials in this request.
    #[serde(default)]
    pub user_authorized_credentials: bool,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub submit_target: Option<String>,
    /// Optional expectation checked after the applied effect.
    #[serde(default)]
    pub expect: Option<Postcondition>,
    #[serde(default = "default_fill_timeout")]
    pub timeout_ms: u64,
}

/// Input for typing ordinary text through browser input events.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TypeText {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
    /// Acknowledge the user's explicit instruction to enter credentials in this request.
    #[serde(default)]
    pub user_authorized_credentials: bool,
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
}

/// Input for one keyboard action.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct PressKey {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
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
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for semantic or screenshot-coordinate drag.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Drag {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
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
}

/// Input for bounded local file upload.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct UploadFiles {
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
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
}

/// One bounded inline file supplied directly by the caller.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// Best-effort bounded settlement before page work; false skips waiting.
    #[serde(default)]
    pub visual_settle: Option<bool>,
    pub script: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_script_result_chars")]
    pub max_result_chars: usize,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for one explicit wait.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Wait {
    #[serde(deserialize_with = "wait::decode_condition")]
    pub condition: WaitCondition,
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
    /// Optional layout-stability and visual quiescence check before completing.
    #[serde(default)]
    pub visual_settle: Option<bool>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
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
#[serde(deny_unknown_fields)]
pub struct FlowStep {
    /// Unique step id; decoding assigns step_1, step_2, and so on when omitted.
    pub id: String,
    /// Current advertised non-composite Ghostlight tool.
    pub tool: String,
    /// Argument object; values may embed `{"flow_ref":{"step","pointer"}}`.
    #[serde(default = "empty_arguments")]
    pub arguments: Value,
}

/// Input for one governed result-aware flow.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RunFlow {
    pub steps: Vec<FlowStep>,
    #[serde(default = "default_on_error")]
    pub on_error: OnError,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn empty_arguments() -> Value {
    serde_json::json!({})
}

fn default_on_error() -> OnError {
    DEFAULT_ON_ERROR
}

/// Input for dialog handling.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct HandleDialog {
    pub action: DialogAction,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for the memory-only recording lifecycle.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Record {
    pub action: RecordAction,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    /// Let the browser write the replay to a file instead of returning it.
    #[serde(default)]
    pub download: bool,
    #[serde(default = "default_record_timeout")]
    pub timeout_ms: u64,
}

/// Input for bounded opt-in browser diagnostics.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Diagnose {
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_diagnostic_source")]
    pub source: DiagnosticSource,
    #[serde(default = "default_diagnostic_detail")]
    pub detail: DiagnosticDetail,
    #[serde(default)]
    pub r#match: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default = "default_diagnostic_limit")]
    pub limit: usize,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Input for the always-available policy explain operation (ADR-0136).
///
/// There is nothing to configure beyond the call time: the projection is compiled from the
/// authority in force.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ExplainPolicy {
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_MS
}
fn default_fill_timeout() -> u64 {
    DEFAULT_FILL_TIMEOUT_MS
}
fn default_record_timeout() -> u64 {
    DEFAULT_RECORD_TIMEOUT_MS
}
fn default_max_chars() -> usize {
    DEFAULT_READ_CHARS
}
fn default_script_result_chars() -> usize {
    DEFAULT_SCRIPT_RESULT_CHARS
}
fn default_inspect_kind() -> InspectScope {
    DEFAULT_INSPECT_SCOPE
}
fn default_find_kind() -> FindScope {
    DEFAULT_FIND_SCOPE
}
fn default_max_items() -> usize {
    DEFAULT_INSPECT_ITEMS
}
fn default_max_results() -> usize {
    DEFAULT_FIND_RESULTS
}
fn default_button() -> PointerButton {
    DEFAULT_BUTTON
}
fn default_click_count() -> u8 {
    DEFAULT_CLICK_COUNT
}

fn default_repeat() -> u16 {
    DEFAULT_KEY_REPEAT
}
fn default_diagnostic_source() -> DiagnosticSource {
    DEFAULT_DIAGNOSTIC_SOURCE
}
fn default_diagnostic_detail() -> DiagnosticDetail {
    DEFAULT_DIAGNOSTIC_DETAIL
}
fn default_diagnostic_limit() -> usize {
    DEFAULT_DIAGNOSTIC_LIMIT
}
