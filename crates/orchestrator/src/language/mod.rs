//! The complete model-facing catalog, typo-closed decoding, and executable defaults.

pub mod outcome;

use std::collections::BTreeMap;
use std::path::Path;

use ghostlight_bridge::service::ToolDefinition;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use thiserror::Error;
use url::Url;

const DEFAULT_TIMEOUT_MS: u64 = 8_000;
const MIN_TIMEOUT_MS: u64 = 100;
const MAX_TIMEOUT_MS: u64 = 30_000;
const COMMON_FIELDS: &[&str] = &["restrict_hosts", "restrict_capabilities"];

/// Model-facing instructions supplied to every protocol edge by the orchestrator.
pub const SERVER_INSTRUCTIONS: &str = "Ghostlight controls the user's visible Chromium browser. Use the advertised short calls and inspect current handles after navigation.";
const CAPABILITIES: &[&str] = &["read", "action", "write", "execute"];
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
    /// Resolve a browser dialog.
    HandleDialog(HandleDialog),
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
            Self::Hover(value) => &value.restrictions,
            Self::FillForm(value) => &value.restrictions,
            Self::TypeText(value) => &value.restrictions,
            Self::PressKey(value) => &value.restrictions,
            Self::Drag(value) => &value.restrictions,
            Self::UploadFiles(value) => &value.restrictions,
            Self::RunScript(value) => &value.restrictions,
            Self::Wait(value) => &value.restrictions,
            Self::RunSequence(value) => &value.restrictions,
            Self::HandleDialog(value) => &value.restrictions,
        }
    }

    /// Return the exact catalog name of this operation.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ListTabs(_) => "browser_list_tabs",
            Self::ActivateTab(_) => "browser_activate_tab",
            Self::OpenPage(_) => "browser_open_page",
            Self::NavigatePage(_) => "browser_navigate_page",
            Self::NavigateHistory(_) => "browser_navigate_history",
            Self::ReloadPage(_) => "browser_reload_page",
            Self::CloseTab(_) => "browser_close_tab",
            Self::ReadPage(_) => "browser_read_page",
            Self::InspectPage(_) => "browser_inspect_page",
            Self::Find(_) => "browser_find",
            Self::TakeScreenshot(_) => "browser_take_screenshot",
            Self::Click(_) => "browser_click",
            Self::ScrollPage(_) => "browser_scroll_page",
            Self::SetZoom(_) => "browser_set_zoom",
            Self::Hover(_) => "browser_hover",
            Self::FillForm(_) => "browser_fill_form",
            Self::TypeText(_) => "browser_type_text",
            Self::PressKey(_) => "browser_press_key",
            Self::Drag(_) => "browser_drag",
            Self::UploadFiles(_) => "browser_upload_files",
            Self::RunScript(_) => "browser_run_script",
            Self::Wait(_) => "browser_wait",
            Self::RunSequence(_) => "browser_run_sequence",
            Self::HandleDialog(_) => "browser_handle_dialog",
        }
    }
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

/// Input for opening a page.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct OpenPage {
    pub url: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for navigating a page.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct NavigatePage {
    pub url: String,
    #[serde(default)]
    pub tab: Option<String>,
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
    #[serde(default = "default_max_chars")]
    pub max_chars: usize,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for semantic inspection.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct InspectPage {
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_inspect_kind")]
    pub kind: String,
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
    pub kind: String,
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
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for semantic activation.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Click {
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
    #[serde(default = "default_button")]
    pub button: String,
    #[serde(default = "default_click_count")]
    pub click_count: u8,
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
    pub target: String,
    pub value: String,
}

/// Input for grouped form filling.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct FillForm {
    pub fields: Vec<FormField>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub submit_target: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for typing ordinary text through browser input events.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TypeText {
    pub target: String,
    pub text: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub clear_first: bool,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
}

/// Input for one keyboard action.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct PressKey {
    pub key: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub modifiers: Vec<String>,
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
    pub target: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(flatten)]
    pub restrictions: RequestRestrictions,
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

/// Input for dialog handling.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct HandleDialog {
    pub accept: bool,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
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

/// Return the complete, deterministic 1.0 model-facing catalog.
#[must_use]
pub fn catalog() -> Vec<ToolDefinition> {
    vec![
        tool(
            "browser_list_tabs",
            "List the tabs Ghostlight currently controls.",
            object_schema(vec![], vec![]),
        ),
        tool(
            "browser_activate_tab",
            "Bring one exact controlled tab and its window into view.",
            object_schema(vec![("tab", handle_schema("tab_"))], vec!["tab"]),
        ),
        tool(
            "browser_open_page",
            "Open a governed URL in a controlled tab and return the landed page.",
            object_schema(
                vec![("url", url_schema()), ("timeout_ms", timeout_schema())],
                vec!["url"],
            ),
        ),
        tool(
            "browser_navigate_page",
            "Navigate a controlled tab to a governed URL and return the landing.",
            object_schema(
                vec![
                    ("url", url_schema()),
                    ("tab", handle_schema("tab_")),
                    ("timeout_ms", timeout_schema()),
                ],
                vec!["url"],
            ),
        ),
        tool(
            "browser_navigate_history",
            "Move a controlled tab backward or forward through browser history.",
            object_schema(
                vec![
                    ("direction", enum_only_schema(&["back", "forward"])),
                    ("tab", handle_schema("tab_")),
                    ("timeout_ms", timeout_schema()),
                ],
                vec!["direction"],
            ),
        ),
        tool(
            "browser_reload_page",
            "Reload a controlled page and govern the resulting landing.",
            object_schema(
                vec![
                    ("tab", handle_schema("tab_")),
                    ("bypass_cache", json!({"type":"boolean","default":false})),
                    ("timeout_ms", timeout_schema()),
                ],
                vec![],
            ),
        ),
        tool(
            "browser_close_tab",
            "Close one exact controlled tab.",
            object_schema(vec![("tab", handle_schema("tab_"))], vec!["tab"]),
        ),
        tool(
            "browser_read_page",
            "Read useful bounded text from a controlled page or target.",
            object_schema(
                vec![
                    ("tab", handle_schema("tab_")),
                    ("target", handle_schema("target_")),
                    ("max_chars", integer_schema(500, 20_000, 8_000)),
                ],
                vec![],
            ),
        ),
        tool(
            "browser_inspect_page",
            "Inspect semantic controls and structure on a controlled page.",
            object_schema(
                vec![
                    ("tab", handle_schema("tab_")),
                    (
                        "kind",
                        enum_schema(&["controls", "structure", "all"], "controls"),
                    ),
                    ("max_items", integer_schema(1, 200, 80)),
                ],
                vec![],
            ),
        ),
        tool(
            "browser_find",
            "Find current semantic targets by visible or accessible text.",
            object_schema(
                vec![
                    ("text", nonempty_string_schema(500)),
                    ("tab", handle_schema("tab_")),
                    ("kind", enum_schema(&["any", "control", "text"], "any")),
                    ("max_results", integer_schema(1, 50, 20)),
                ],
                vec!["text"],
            ),
        ),
        tool(
            "browser_take_screenshot",
            "Capture a viewport, full page, or semantic target screenshot.",
            object_schema(
                vec![
                    ("tab", handle_schema("tab_")),
                    ("target", handle_schema("target_")),
                    ("full_page", json!({"type":"boolean","default":false})),
                    ("timeout_ms", timeout_schema()),
                ],
                vec![],
            ),
        ),
        tool(
            "browser_click",
            "Activate a current semantic target or a point in a current screenshot.",
            object_schema(
                vec![
                    ("target", handle_schema("target_")),
                    ("view", handle_schema("view_")),
                    ("x", coordinate_schema()),
                    ("y", coordinate_schema()),
                    ("tab", handle_schema("tab_")),
                    (
                        "button",
                        enum_schema(&["primary", "middle", "secondary"], "primary"),
                    ),
                    ("click_count", integer_schema(1, 2, 1)),
                    ("timeout_ms", timeout_schema()),
                ],
                vec![],
            ),
        ),
        tool(
            "browser_scroll_page",
            "Scroll a page in a direction or reveal one semantic target.",
            object_schema(
                vec![
                    ("tab", handle_schema("tab_")),
                    ("target", handle_schema("target_")),
                    (
                        "direction",
                        enum_only_schema(&["up", "down", "left", "right"]),
                    ),
                    (
                        "amount",
                        enum_only_schema(&["small", "medium", "large", "page"]),
                    ),
                    ("timeout_ms", timeout_schema()),
                ],
                vec![],
            ),
        ),
        tool(
            "browser_set_zoom",
            "Set the visible zoom of a controlled tab.",
            object_schema(
                vec![
                    ("percent", integer_schema_no_default(25, 500)),
                    ("tab", handle_schema("tab_")),
                ],
                vec!["percent"],
            ),
        ),
        tool(
            "browser_hover",
            "Hover a semantic target or a point in a current screenshot.",
            object_schema(
                vec![
                    ("target", handle_schema("target_")),
                    ("view", handle_schema("view_")),
                    ("x", coordinate_schema()),
                    ("y", coordinate_schema()),
                    ("tab", handle_schema("tab_")),
                    ("timeout_ms", timeout_schema()),
                ],
                vec![],
            ),
        ),
        tool(
            "browser_fill_form",
            "Fill ordinary form controls, with credential fields handed to the user.",
            object_schema(
                vec![
                    (
                        "fields",
                        json!({"type":"array","minItems":1,"maxItems":30,"items":{"type":"object","additionalProperties":false,"properties":{"target":handle_schema("target_"),"value":{"type":"string","maxLength":8000}},"required":["target","value"]}}),
                    ),
                    ("tab", handle_schema("tab_")),
                    ("submit_target", handle_schema("target_")),
                    ("timeout_ms", timeout_schema()),
                ],
                vec!["fields"],
            ),
        ),
        tool(
            "browser_type_text",
            "Type ordinary text through browser input events after credential preflight.",
            object_schema(
                vec![
                    ("target", handle_schema("target_")),
                    ("text", json!({"type":"string","maxLength":8000})),
                    ("tab", handle_schema("tab_")),
                    ("clear_first", json!({"type":"boolean","default":false})),
                    ("timeout_ms", timeout_schema()),
                ],
                vec!["target", "text"],
            ),
        ),
        tool(
            "browser_press_key",
            "Send one explicit keyboard action to a controlled page or target.",
            object_schema(
                vec![
                    ("key", key_schema()),
                    ("tab", handle_schema("tab_")),
                    ("target", handle_schema("target_")),
                    (
                        "modifiers",
                        json!({"type":"array","uniqueItems":true,"items":{"enum":["Alt","Control","Meta","Shift"]},"default":[]}),
                    ),
                ],
                vec!["key"],
            ),
        ),
        tool(
            "browser_drag",
            "Drag between semantic targets or two points in a current screenshot.",
            object_schema(
                vec![
                    ("source_target", handle_schema("target_")),
                    ("destination_target", handle_schema("target_")),
                    ("view", handle_schema("view_")),
                    ("start_x", coordinate_schema()),
                    ("start_y", coordinate_schema()),
                    ("end_x", coordinate_schema()),
                    ("end_y", coordinate_schema()),
                    ("tab", handle_schema("tab_")),
                    ("timeout_ms", timeout_schema()),
                ],
                vec![],
            ),
        ),
        tool(
            "browser_upload_files",
            "Upload explicitly named bounded local files to one ordinary file input.",
            object_schema(
                vec![
                    ("target", handle_schema("target_")),
                    (
                        "paths",
                        json!({"type":"array","minItems":1,"maxItems":5,"uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":4096}}),
                    ),
                    ("tab", handle_schema("tab_")),
                    ("timeout_ms", timeout_schema()),
                ],
                vec!["target", "paths"],
            ),
        ),
        tool(
            "browser_run_script",
            "Evaluate an explicit bounded script in a controlled page.",
            object_schema(
                vec![
                    ("script", nonempty_string_schema(20_000)),
                    ("tab", handle_schema("tab_")),
                    ("max_result_chars", integer_schema(100, 20_000, 8_000)),
                    ("timeout_ms", timeout_schema()),
                ],
                vec!["script"],
            ),
        ),
        tool(
            "browser_wait",
            "Wait for one explicit observable page condition.",
            object_schema(
                vec![
                    (
                        "condition",
                        enum_only_schema(&[
                            "load_ready",
                            "url_contains",
                            "text_present",
                            "text_absent",
                            "target_present",
                            "target_absent",
                        ]),
                    ),
                    ("tab", handle_schema("tab_")),
                    ("value", nonempty_string_schema(2_000)),
                    ("target", handle_schema("target_")),
                    ("timeout_ms", timeout_schema()),
                ],
                vec!["condition"],
            ),
        ),
        tool(
            "browser_run_sequence",
            "Run two to eight fully specified actions on one controlled tab.",
            object_schema(
                vec![
                    ("steps", sequence_schema()),
                    ("tab", handle_schema("tab_")),
                    ("timeout_ms", timeout_schema()),
                ],
                vec!["steps"],
            ),
        ),
        tool(
            "browser_handle_dialog",
            "Accept or dismiss the current JavaScript dialog.",
            object_schema(
                vec![
                    ("accept", json!({"type":"boolean"})),
                    ("tab", handle_schema("tab_")),
                    ("text", json!({"type":"string","maxLength":2000})),
                ],
                vec!["accept"],
            ),
        ),
    ]
}

/// Decode and validate one catalog invocation.
pub fn decode(name: &str, input: Value) -> Result<Operation, LanguageError> {
    let operation = match name {
        "browser_list_tabs" => Operation::ListTabs(parse(input, &[], |value: &ListTabs| {
            validate_restrictions(&value.restrictions)
        })?),
        "browser_activate_tab" => {
            Operation::ActivateTab(parse(input, &["tab"], |value: &ActivateTab| {
                validate_handle(&value.tab, "tab_")?;
                validate_restrictions(&value.restrictions)
            })?)
        }
        "browser_open_page" => {
            Operation::OpenPage(parse(input, &["url", "timeout_ms"], |value: &OpenPage| {
                validate_url(&value.url)?;
                validate_timeout(value.timeout_ms)?;
                validate_restrictions(&value.restrictions)
            })?)
        }
        "browser_navigate_page" => Operation::NavigatePage(parse(
            input,
            &["url", "tab", "timeout_ms"],
            |value: &NavigatePage| {
                validate_url(&value.url)?;
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_timeout(value.timeout_ms)?;
                validate_restrictions(&value.restrictions)
            },
        )?),
        "browser_navigate_history" => Operation::NavigateHistory(parse(
            input,
            &["direction", "tab", "timeout_ms"],
            |value: &NavigateHistory| {
                validate_choice(&value.direction, &["back", "forward"], "direction")?;
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_timeout(value.timeout_ms)?;
                validate_restrictions(&value.restrictions)
            },
        )?),
        "browser_reload_page" => Operation::ReloadPage(parse(
            input,
            &["tab", "bypass_cache", "timeout_ms"],
            |value: &ReloadPage| {
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_timeout(value.timeout_ms)?;
                validate_restrictions(&value.restrictions)
            },
        )?),
        "browser_close_tab" => Operation::CloseTab(parse(input, &["tab"], |value: &CloseTab| {
            validate_handle(&value.tab, "tab_")?;
            validate_restrictions(&value.restrictions)
        })?),
        "browser_read_page" => Operation::ReadPage(parse(
            input,
            &["tab", "target", "max_chars"],
            |value: &ReadPage| {
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_optional_handle(value.target.as_deref(), "target_")?;
                validate_range(value.max_chars, 500, 20_000, "max_chars")?;
                validate_restrictions(&value.restrictions)
            },
        )?),
        "browser_inspect_page" => Operation::InspectPage(parse(
            input,
            &["tab", "kind", "max_items"],
            |value: &InspectPage| {
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_choice(&value.kind, &["controls", "structure", "all"], "kind")?;
                validate_range(value.max_items, 1, 200, "max_items")?;
                validate_restrictions(&value.restrictions)
            },
        )?),
        "browser_find" => Operation::Find(parse(
            input,
            &["text", "tab", "kind", "max_results"],
            |value: &Find| {
                validate_text(&value.text, 500, "text")?;
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_choice(&value.kind, &["any", "control", "text"], "kind")?;
                validate_range(value.max_results, 1, 50, "max_results")?;
                validate_restrictions(&value.restrictions)
            },
        )?),
        "browser_take_screenshot" => Operation::TakeScreenshot(parse(
            input,
            &["tab", "target", "full_page", "timeout_ms"],
            |value: &TakeScreenshot| {
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_optional_handle(value.target.as_deref(), "target_")?;
                if value.target.is_some() && value.full_page {
                    return Err(LanguageError::Invalid(
                        "target and full_page cannot be combined".into(),
                    ));
                }
                validate_timeout(value.timeout_ms)?;
                validate_restrictions(&value.restrictions)
            },
        )?),
        "browser_click" => Operation::Click(parse(
            input,
            &[
                "target",
                "view",
                "x",
                "y",
                "tab",
                "button",
                "click_count",
                "timeout_ms",
            ],
            validate_click,
        )?),
        "browser_scroll_page" => Operation::ScrollPage(parse(
            input,
            &["tab", "target", "direction", "amount", "timeout_ms"],
            validate_scroll,
        )?),
        "browser_set_zoom" => {
            Operation::SetZoom(parse(input, &["percent", "tab"], |value: &SetZoom| {
                validate_range(usize::from(value.percent), 25, 500, "percent")?;
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_restrictions(&value.restrictions)
            })?)
        }
        "browser_hover" => Operation::Hover(parse(
            input,
            &["target", "view", "x", "y", "tab", "timeout_ms"],
            validate_hover,
        )?),
        "browser_fill_form" => Operation::FillForm(parse(
            input,
            &["fields", "tab", "submit_target", "timeout_ms"],
            validate_fill,
        )?),
        "browser_type_text" => Operation::TypeText(parse(
            input,
            &["target", "text", "tab", "clear_first", "timeout_ms"],
            validate_type_text,
        )?),
        "browser_press_key" => Operation::PressKey(parse(
            input,
            &["key", "tab", "target", "modifiers"],
            validate_press_key,
        )?),
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
        )?),
        "browser_upload_files" => Operation::UploadFiles(parse(
            input,
            &["target", "paths", "tab", "timeout_ms"],
            validate_upload,
        )?),
        "browser_run_script" => Operation::RunScript(parse(
            input,
            &["script", "tab", "max_result_chars", "timeout_ms"],
            |value: &RunScript| {
                validate_text(&value.script, 20_000, "script")?;
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                validate_range(value.max_result_chars, 100, 20_000, "max_result_chars")?;
                validate_timeout(value.timeout_ms)?;
                validate_restrictions(&value.restrictions)
            },
        )?),
        "browser_wait" => Operation::Wait(parse(
            input,
            &["condition", "tab", "value", "target", "timeout_ms"],
            validate_wait,
        )?),
        "browser_run_sequence" => Operation::RunSequence(parse(
            input,
            &["steps", "tab", "timeout_ms"],
            validate_sequence,
        )?),
        "browser_handle_dialog" => Operation::HandleDialog(parse(
            input,
            &["accept", "tab", "text"],
            |value: &HandleDialog| {
                validate_optional_handle(value.tab.as_deref(), "tab_")?;
                if let Some(text) = &value.text {
                    validate_text_allow_empty(text, 2_000, "text")?;
                    if !value.accept {
                        return Err(LanguageError::Invalid("text requires accept: true".into()));
                    }
                }
                validate_restrictions(&value.restrictions)
            },
        )?),
        other => return Err(LanguageError::UnknownTool(other.into())),
    };
    Ok(operation)
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
            return Err(LanguageError::Invalid(format!("unknown field `{key}`")));
        }
    }
    Ok(())
}

fn validate_click(value: &Click) -> Result<(), LanguageError> {
    validate_location(
        value.target.as_deref(),
        value.view.as_deref(),
        value.x,
        value.y,
    )?;
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_choice(&value.button, &["primary", "middle", "secondary"], "button")?;
    validate_range(usize::from(value.click_count), 1, 2, "click_count")?;
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
    if let Some(target) = &value.target {
        validate_handle(target, "target_")?;
        if value.direction.is_some() || value.amount.is_some() {
            return Err(LanguageError::Invalid(
                "target cannot be combined with direction or amount".into(),
            ));
        }
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
    validate_timeout(value.timeout_ms)?;
    for field in &value.fields {
        validate_handle(&field.target, "target_")?;
        validate_text_allow_empty(&field.value, 8_000, "field value")?;
    }
    validate_restrictions(&value.restrictions)
}

fn validate_type_text(value: &TypeText) -> Result<(), LanguageError> {
    validate_handle(&value.target, "target_")?;
    if value.text.is_empty() && !value.clear_first {
        return Err(LanguageError::Invalid(
            "text cannot be empty unless clear_first is true".into(),
        ));
    }
    validate_text_allow_empty(&value.text, 8_000, "text")?;
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
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
                .ok_or_else(|| LanguageError::Invalid("source_target is required".into()))?,
            "target_",
        )?;
        validate_handle(
            value
                .destination_target
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("destination_target is required".into()))?,
            "target_",
        )?;
    } else {
        validate_handle(
            value
                .view
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("view is required".into()))?,
            "view_",
        )?;
        for (name, coordinate) in [
            ("start_x", value.start_x),
            ("start_y", value.start_y),
            ("end_x", value.end_x),
            ("end_y", value.end_y),
        ] {
            validate_coordinate(
                coordinate.ok_or_else(|| LanguageError::Invalid(format!("{name} is required")))?,
                name,
            )?;
        }
    }
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_timeout(value.timeout_ms)?;
    validate_restrictions(&value.restrictions)
}

fn validate_upload(value: &UploadFiles) -> Result<(), LanguageError> {
    validate_handle(&value.target, "target_")?;
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

fn validate_press_key(value: &PressKey) -> Result<(), LanguageError> {
    validate_key(&value.key)?;
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_optional_handle(value.target.as_deref(), "target_")?;
    validate_modifiers(&value.modifiers)?;
    validate_restrictions(&value.restrictions)
}

fn validate_wait(value: &Wait) -> Result<(), LanguageError> {
    validate_optional_handle(value.tab.as_deref(), "tab_")?;
    validate_timeout(value.timeout_ms)?;
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
            } => validate_condition(condition, value.as_deref(), target.as_deref())?,
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
        ],
        "condition",
    )?;
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
        return Err(LanguageError::Invalid("url exceeds 4096 bytes".into()));
    }
    let parsed =
        Url::parse(value).map_err(|_| LanguageError::Invalid("url must be absolute".into()))?;
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
            "timeout_ms must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}"
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
            "{field} must be between {minimum} and {maximum}"
        )))
    }
}

fn validate_choice(value: &str, choices: &[&str], field: &str) -> Result<(), LanguageError> {
    if choices.contains(&value) {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} has unsupported value `{value}`"
        )))
    }
}

fn validate_handle(value: &str, prefix: &str) -> Result<(), LanguageError> {
    if value.starts_with(prefix) && value.len() > prefix.len() && value.len() <= 80 {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "handle must start with {prefix}"
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
    if value.chars().count() <= maximum {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} exceeds {maximum} characters"
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
        Err(LanguageError::Invalid(
            "key must be one character or a supported named key".into(),
        ))
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

fn tool(name: &str, description: &str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: description.into(),
        input_schema,
    }
}

fn object_schema(fields: Vec<(&str, Value)>, required: Vec<&str>) -> Value {
    let mut properties = BTreeMap::new();
    properties.insert(
        "restrict_capabilities",
        json!({"type":"array","minItems":1,"uniqueItems":true,"items":{"enum":CAPABILITIES}}),
    );
    properties.insert("restrict_hosts", json!({"type":"array","minItems":1,"uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":253,"pattern":"^(\\*\\.)?[^/:*]+$"}}));
    for (name, schema) in fields {
        properties.insert(name, schema);
    }
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
}

fn url_schema() -> Value {
    json!({"type":"string","format":"uri","pattern":"^https?://","maxLength":4096})
}
fn handle_schema(prefix: &str) -> Value {
    json!({"type":"string","pattern":format!("^{prefix}"),"maxLength":80})
}
fn nonempty_string_schema(maximum: usize) -> Value {
    json!({"type":"string","minLength":1,"maxLength":maximum,"pattern":"\\S"})
}
fn integer_schema(minimum: usize, maximum: usize, default: usize) -> Value {
    json!({"type":"integer","minimum":minimum,"maximum":maximum,"default":default})
}
fn integer_schema_no_default(minimum: usize, maximum: usize) -> Value {
    json!({"type":"integer","minimum":minimum,"maximum":maximum})
}
fn coordinate_schema() -> Value {
    json!({"type":"number","minimum":0,"maximum":1_000_000})
}
fn timeout_schema() -> Value {
    json!({"type":"integer","minimum":MIN_TIMEOUT_MS,"maximum":MAX_TIMEOUT_MS,"default":DEFAULT_TIMEOUT_MS})
}
fn enum_schema(values: &[&str], default: &str) -> Value {
    json!({"type":"string","enum":values,"default":default})
}
fn enum_only_schema(values: &[&str]) -> Value {
    json!({"type":"string","enum":values})
}
fn key_schema() -> Value {
    json!({"oneOf":[{"type":"string","minLength":1,"maxLength":1},{"type":"string","enum":NAMED_KEYS}]})
}

fn sequence_schema() -> Value {
    let click = json!({"type":"object","additionalProperties":false,"properties":{"action":{"const":"click"},"target":handle_schema("target_"),"button":enum_schema(&["primary","middle","secondary"],"primary"),"click_count":integer_schema(1,2,1)},"required":["action","target"]});
    let fill = json!({"type":"object","additionalProperties":false,"properties":{"action":{"const":"fill"},"target":handle_schema("target_"),"value":{"type":"string","maxLength":8000}},"required":["action","target","value"]});
    let type_text = json!({"type":"object","additionalProperties":false,"properties":{"action":{"const":"type_text"},"target":handle_schema("target_"),"text":{"type":"string","maxLength":8000},"clear_first":{"type":"boolean","default":false}},"required":["action","target","text"]});
    let key = json!({"type":"object","additionalProperties":false,"properties":{"action":{"const":"press_key"},"key":key_schema(),"target":handle_schema("target_"),"modifiers":{"type":"array","uniqueItems":true,"items":{"enum":["Alt","Control","Meta","Shift"]},"default":[]}},"required":["action","key"]});
    let scroll = json!({"type":"object","additionalProperties":false,"properties":{"action":{"const":"scroll"},"target":handle_schema("target_"),"direction":{"enum":["up","down","left","right"]},"amount":{"enum":["small","medium","large","page"]}},"required":["action"]});
    let hover = json!({"type":"object","additionalProperties":false,"properties":{"action":{"const":"hover"},"target":handle_schema("target_")},"required":["action","target"]});
    let wait = json!({"type":"object","additionalProperties":false,"properties":{"action":{"const":"wait"},"condition":{"enum":["load_ready","url_contains","text_present","text_absent","target_present","target_absent"]},"value":nonempty_string_schema(2000),"target":handle_schema("target_")},"required":["action","condition"]});
    json!({"type":"array","minItems":2,"maxItems":8,"items":{"oneOf":[click,fill,type_text,key,scroll,hover,wait]}})
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{catalog, decode, LanguageError, Operation};

    #[test]
    fn catalog_has_unique_exact_tools_and_typo_closed_schemas() {
        let catalog = catalog();
        assert_eq!(catalog.len(), 24);
        let mut names: Vec<_> = catalog.iter().map(|tool| tool.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 24);
        for tool in catalog {
            assert_eq!(tool.input_schema["additionalProperties"], false);
        }
    }

    #[test]
    fn shortest_calls_receive_executable_defaults() {
        let Operation::OpenPage(open) =
            decode("browser_open_page", json!({"url":"https://example.com"})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(open.timeout_ms, 8_000);
        let Operation::ReadPage(read) = decode("browser_read_page", json!({})).unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(read.max_chars, 8_000);
        let Operation::InspectPage(inspect) = decode("browser_inspect_page", json!({})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(inspect.kind, "controls");
        assert_eq!(inspect.max_items, 80);
    }

    #[test]
    fn unknown_fields_and_ambiguous_waits_fail() {
        let error =
            decode("browser_read_page", json!({"max_chars":8000,"max_char":1})).unwrap_err();
        assert!(matches!(error, LanguageError::Invalid(message) if message.contains("max_char")));
        assert!(decode("browser_wait", json!({"condition":"text_present"})).is_err());
        assert!(decode(
            "browser_wait",
            json!({"condition":"target_present","value":"x"})
        )
        .is_err());
    }

    #[test]
    fn screenshot_target_and_full_page_are_mutually_exclusive() {
        assert!(decode(
            "browser_take_screenshot",
            json!({"target":"target_x","full_page":true})
        )
        .is_err());
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
        let Operation::ScrollPage(scroll) = decode("browser_scroll_page", json!({})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert!(scroll.direction.is_none());
        assert!(scroll.amount.is_none());
        assert!(decode(
            "browser_scroll_page",
            json!({"target":"target_x","direction":"down"})
        )
        .is_err());
        assert!(decode(
            "browser_upload_files",
            json!({"target":"target_x","paths":["relative.txt"]})
        )
        .is_err());
    }
}
