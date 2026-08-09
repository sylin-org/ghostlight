// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Protocol-neutral Ghostlight browser operations and results.
//!
//! The MCP edge translates model-facing Ghostlight calls into these typed operations before work
//! crosses the owner bridge. Browser mechanisms remain a separate, policy-free vocabulary below
//! the service operation pipeline.

use crate::workspace_id::WorkspaceId;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashSet;

macro_rules! stable_string_enum {
    (
        $(#[$enum_meta:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $wire:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )+
        }

        impl $name {
            /// Every value in stable wire order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Return the stable wire spelling.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            /// Parse one exact stable wire spelling.
            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $($wire => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).ok_or_else(|| {
                    serde::de::Error::custom(format_args!(
                        "unknown {} wire value: {value}",
                        stringify!($name)
                    ))
                })
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

stable_string_enum! {
    /// The complete model-facing and owner-bridge operation vocabulary.
    pub enum OperationKind {
        BrowserGetStatus => "browser_get_status",
        BrowserOpenTab => "browser_open_tab",
        BrowserListTabs => "browser_list_tabs",
        BrowserFocusTab => "browser_focus_tab",
        BrowserCloseTab => "browser_close_tab",
        BrowserNavigate => "browser_navigate",
        BrowserGoBack => "browser_go_back",
        BrowserGoForward => "browser_go_forward",
        BrowserReloadPage => "browser_reload_page",
        BrowserInspectPage => "browser_inspect_page",
        BrowserReadPage => "browser_read_page",
        BrowserTakeScreenshot => "browser_take_screenshot",
        BrowserClick => "browser_click",
        BrowserHover => "browser_hover",
        BrowserScrollToTarget => "browser_scroll_to_target",
        BrowserScrollPage => "browser_scroll_page",
        BrowserPressKey => "browser_press_key",
        BrowserPressEscape => "browser_press_escape",
        BrowserDrag => "browser_drag",
        BrowserFillForm => "browser_fill_form",
        BrowserWaitFor => "browser_wait_for",
        BrowserRunSequence => "browser_run_sequence",
        BrowserGetDialog => "browser_get_dialog",
        BrowserHandleDialog => "browser_handle_dialog"
    }
}

/// Maximum UTF-8 byte length of one canonical page target.
pub const MAX_OPERATION_TARGET_BYTES: usize = 1000;
/// Maximum suffix length of one opaque observation cursor.
pub const MAX_OPERATION_CURSOR_SUFFIX_BYTES: usize = 256;
/// Maximum UTF-8 byte length of one canonical URL.
pub const MAX_OPERATION_URL_BYTES: usize = 4096;
/// Maximum number of operations in one canonical sequence.
pub const MAX_OPERATION_SEQUENCE_STEPS: usize = 10;
/// Maximum image-bearing screenshot children accepted in one canonical sequence.
pub const MAX_OPERATION_SEQUENCE_MEDIA_PARTS: usize = 4;
/// Minimum number of operations in one canonical sequence.
pub const MIN_OPERATION_SEQUENCE_STEPS: usize = 2;

/// A model-authored target reference or unique accessible description.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct OperationTarget(String);

impl OperationTarget {
    /// Validate and construct one canonical target.
    pub fn parse(value: &str) -> Option<Self> {
        if value.is_empty()
            || value.len() > MAX_OPERATION_TARGET_BYTES
            || value.chars().any(char::is_control)
        {
            return None;
        }
        Some(Self(value.to_owned()))
    }

    /// Return the exact model-authored target value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OperationTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| {
            serde::de::Error::custom(
                "target must be non-empty, control-free, and at most 1000 UTF-8 bytes",
            )
        })
    }
}

/// Opaque continuation cursor bound to one prior canonical observation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct CanonicalCursor(String);

impl CanonicalCursor {
    /// Validate and construct one canonical continuation cursor.
    pub fn parse(value: &str) -> Option<Self> {
        let suffix = value.strip_prefix("c_")?;
        if !(8..=MAX_OPERATION_CURSOR_SUFFIX_BYTES).contains(&suffix.len())
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return None;
        }
        Some(Self(value.to_owned()))
    }

    /// Return the exact opaque cursor value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CanonicalCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| serde::de::Error::custom("invalid observation cursor"))
    }
}

stable_string_enum! {
    /// Optional keyboard modifiers in canonical order.
    pub enum KeyModifier {
        Alt => "alt",
        Control => "control",
        Meta => "meta",
        Shift => "shift"
    }
}

stable_string_enum! {
    /// Button used by a canonical click.
    pub enum ClickButton {
        Left => "left",
        Right => "right",
        Middle => "middle"
    }
}

#[allow(clippy::derivable_impls)]
impl Default for ClickButton {
    fn default() -> Self {
        Self::Left
    }
}

stable_string_enum! {
    /// Page-inspection detail level.
    pub enum InspectionDetail {
        Interactive => "interactive",
        All => "all"
    }
}

#[allow(clippy::derivable_impls)]
impl Default for InspectionDetail {
    fn default() -> Self {
        Self::Interactive
    }
}

stable_string_enum! {
    /// Direction of semantic page scrolling.
    pub enum ScrollDirection {
        Up => "up",
        Down => "down"
    }
}

stable_string_enum! {
    /// Amount of semantic page scrolling.
    pub enum ScrollAmount {
        Small => "small",
        Page => "page"
    }
}

#[allow(clippy::derivable_impls)]
impl Default for ScrollAmount {
    fn default() -> Self {
        Self::Page
    }
}

stable_string_enum! {
    /// Named non-printable key accepted by the canonical key operation.
    pub enum NamedKey {
        Enter => "Enter",
        Tab => "Tab",
        ArrowUp => "ArrowUp",
        ArrowDown => "ArrowDown",
        ArrowLeft => "ArrowLeft",
        ArrowRight => "ArrowRight",
        Home => "Home",
        End => "End",
        PageUp => "PageUp",
        PageDown => "PageDown",
        Backspace => "Backspace",
        Delete => "Delete",
        Space => "Space"
    }
}

stable_string_enum! {
    /// Condition state accepted by the canonical wait operation.
    pub enum WaitState {
        Visible => "visible",
        Present => "present",
        Gone => "gone"
    }
}

#[allow(clippy::derivable_impls)]
impl Default for WaitState {
    fn default() -> Self {
        Self::Visible
    }
}

stable_string_enum! {
    /// Explicit browser-dialog resolution.
    pub enum DialogResolution {
        Accept => "accept",
        Dismiss => "dismiss",
        Respond => "respond"
    }
}

/// Arguments for a call with no model-authored fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyArguments {}

/// Arguments for opening a separate controlled tab.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenTabArguments {
    /// Optional URL loaded in the new tab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Arguments that require one explicit controlled tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredTabArguments {
    /// Exact opaque controlled-tab handle.
    pub tab: TabHandle,
}

/// Arguments that may use the workspace current tab.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionalTabArguments {
    /// Exact opaque controlled-tab handle, when the current tab should not be used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
}

/// Arguments for URL navigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigateArguments {
    /// URL to load.
    pub url: String,
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
}

/// Arguments for inspecting page structure and controls.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectPageArguments {
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
    /// Optional bounded search query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Optional exact subtree target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<OperationTarget>,
    /// Detail returned when no query is supplied.
    #[serde(default)]
    pub include: InspectionDetail,
    /// Opaque continuation from a prior matching inspection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CanonicalCursor>,
}

/// Arguments for reading useful page text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadPageArguments {
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
    /// Optional exact subtree target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<OperationTarget>,
    /// Maximum returned characters.
    #[serde(default = "default_read_max_chars")]
    pub max_chars: u32,
    /// Opaque continuation from a prior matching read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CanonicalCursor>,
}

const fn default_read_max_chars() -> u32 {
    20_000
}

impl Default for ReadPageArguments {
    fn default() -> Self {
        Self {
            tab: None,
            target: None,
            max_chars: default_read_max_chars(),
            cursor: None,
        }
    }
}

/// Arguments for a viewport or target screenshot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotArguments {
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
    /// Optional exact capture target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<OperationTarget>,
}

/// Arguments for clicking one semantic target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickArguments {
    /// Exact target ref or unique accessible description.
    pub target: OperationTarget,
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
    /// Mouse button.
    #[serde(default)]
    pub button: ClickButton,
    /// Number of clicks.
    #[serde(default = "default_click_count")]
    pub clicks: u8,
    /// Keyboard modifiers applied to the click.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<KeyModifier>,
}

const fn default_click_count() -> u8 {
    1
}

/// Arguments for one target-only interaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetArguments {
    /// Exact target ref or unique accessible description.
    pub target: OperationTarget,
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
}

/// Arguments for semantic page scrolling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScrollPageArguments {
    /// Scroll direction.
    pub direction: ScrollDirection,
    /// Bounded scroll amount.
    #[serde(default)]
    pub amount: ScrollAmount,
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
}

/// Arguments for pressing one named key against one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PressKeyArguments {
    /// Named non-printable key.
    pub key: NamedKey,
    /// Exact target ref or unique accessible description.
    pub target: OperationTarget,
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
    /// Keyboard modifiers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<KeyModifier>,
}

/// Arguments for dragging one target to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DragArguments {
    /// Exact source target.
    pub from: OperationTarget,
    /// Exact destination target.
    pub to: OperationTarget,
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
}

/// One ordered canonical form field write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormField {
    /// Exact field target.
    pub field: OperationTarget,
    /// Scalar value written to the field.
    pub value: Value,
}

/// Arguments for atomic form preflight followed by ordered writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FillFormArguments {
    /// Ordered field writes.
    pub fields: Vec<FormField>,
    /// Optional exact submit control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit_target: Option<OperationTarget>,
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
}

/// Arguments for one bounded page condition wait.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitForArguments {
    /// Target description or text condition.
    pub condition: String,
    /// Requested condition state.
    #[serde(default)]
    pub state: WaitState,
    /// Absolute observation budget in milliseconds.
    #[serde(default = "default_wait_timeout_ms")]
    pub timeout_ms: u32,
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
}

const fn default_wait_timeout_ms() -> u32 {
    10_000
}

/// Arguments for resolving a browser dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandleDialogArguments {
    /// Explicit resolution.
    pub action: DialogResolution,
    /// Prompt response text, valid only with `respond`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Exact controlled tab, or the current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
}

/// Arguments for one fixed-input canonical sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSequenceArguments {
    /// Exact root tab, or the workspace current tab when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabHandle>,
    /// Fully decoded canonical child operations in execution order.
    pub steps: Vec<Operation>,
}

/// One closed, typed, protocol-neutral browser operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "arguments", rename_all = "snake_case")]
pub enum Operation {
    BrowserGetStatus(EmptyArguments),
    BrowserOpenTab(OpenTabArguments),
    BrowserListTabs(EmptyArguments),
    BrowserFocusTab(RequiredTabArguments),
    BrowserCloseTab(RequiredTabArguments),
    BrowserNavigate(NavigateArguments),
    BrowserGoBack(OptionalTabArguments),
    BrowserGoForward(OptionalTabArguments),
    BrowserReloadPage(OptionalTabArguments),
    BrowserInspectPage(InspectPageArguments),
    BrowserReadPage(ReadPageArguments),
    BrowserTakeScreenshot(ScreenshotArguments),
    BrowserClick(ClickArguments),
    BrowserHover(TargetArguments),
    BrowserScrollToTarget(TargetArguments),
    BrowserScrollPage(ScrollPageArguments),
    BrowserPressKey(PressKeyArguments),
    BrowserPressEscape(OptionalTabArguments),
    BrowserDrag(DragArguments),
    BrowserFillForm(FillFormArguments),
    BrowserWaitFor(WaitForArguments),
    BrowserRunSequence(RunSequenceArguments),
    BrowserGetDialog(OptionalTabArguments),
    BrowserHandleDialog(HandleDialogArguments),
}

/// Semantic validation failure for one typed canonical operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OperationError {
    #[error("URL must be non-empty, control-free, and at most 4096 UTF-8 bytes")]
    InvalidUrl,
    #[error("query must be non-empty and at most 1000 UTF-8 bytes")]
    InvalidQuery,
    #[error("inspect accepts query or target, not both")]
    AmbiguousInspection,
    #[error("max_chars must be between 1 and 50000")]
    InvalidMaxChars,
    #[error("clicks must be between 1 and 3; right and middle clicks must use one click")]
    InvalidClick,
    #[error("modifiers must be unique")]
    DuplicateModifier,
    #[error("form fields must contain 1 to 20 unique targets with scalar values")]
    InvalidFormFields,
    #[error("wait condition must be non-empty and at most 2000 UTF-8 bytes")]
    InvalidWaitCondition,
    #[error("wait timeout_ms must be between 1 and 30000")]
    InvalidWaitTimeout,
    #[error("dialog text is required only for respond and may contain at most 2000 UTF-8 bytes")]
    InvalidDialogResponse,
    #[error("sequence must contain 2 to 10 eligible non-nested operations on one tab")]
    InvalidSequence,
}

impl Operation {
    /// Return this operation's exact canonical identity.
    pub const fn kind(&self) -> OperationKind {
        match self {
            Self::BrowserGetStatus(_) => OperationKind::BrowserGetStatus,
            Self::BrowserOpenTab(_) => OperationKind::BrowserOpenTab,
            Self::BrowserListTabs(_) => OperationKind::BrowserListTabs,
            Self::BrowserFocusTab(_) => OperationKind::BrowserFocusTab,
            Self::BrowserCloseTab(_) => OperationKind::BrowserCloseTab,
            Self::BrowserNavigate(_) => OperationKind::BrowserNavigate,
            Self::BrowserGoBack(_) => OperationKind::BrowserGoBack,
            Self::BrowserGoForward(_) => OperationKind::BrowserGoForward,
            Self::BrowserReloadPage(_) => OperationKind::BrowserReloadPage,
            Self::BrowserInspectPage(_) => OperationKind::BrowserInspectPage,
            Self::BrowserReadPage(_) => OperationKind::BrowserReadPage,
            Self::BrowserTakeScreenshot(_) => OperationKind::BrowserTakeScreenshot,
            Self::BrowserClick(_) => OperationKind::BrowserClick,
            Self::BrowserHover(_) => OperationKind::BrowserHover,
            Self::BrowserScrollToTarget(_) => OperationKind::BrowserScrollToTarget,
            Self::BrowserScrollPage(_) => OperationKind::BrowserScrollPage,
            Self::BrowserPressKey(_) => OperationKind::BrowserPressKey,
            Self::BrowserPressEscape(_) => OperationKind::BrowserPressEscape,
            Self::BrowserDrag(_) => OperationKind::BrowserDrag,
            Self::BrowserFillForm(_) => OperationKind::BrowserFillForm,
            Self::BrowserWaitFor(_) => OperationKind::BrowserWaitFor,
            Self::BrowserRunSequence(_) => OperationKind::BrowserRunSequence,
            Self::BrowserGetDialog(_) => OperationKind::BrowserGetDialog,
            Self::BrowserHandleDialog(_) => OperationKind::BrowserHandleDialog,
        }
    }

    /// Bind one exact tab to a tab-scoped operation and its sequence children.
    ///
    /// Returns false for topology, diagnostic, and creator operations that do not accept a tab.
    pub fn bind_tab(&mut self, tab: Option<TabHandle>) -> bool {
        let target = match self {
            Self::BrowserNavigate(arguments) => &mut arguments.tab,
            Self::BrowserGoBack(arguments)
            | Self::BrowserGoForward(arguments)
            | Self::BrowserReloadPage(arguments)
            | Self::BrowserPressEscape(arguments)
            | Self::BrowserGetDialog(arguments) => &mut arguments.tab,
            Self::BrowserInspectPage(arguments) => &mut arguments.tab,
            Self::BrowserReadPage(arguments) => &mut arguments.tab,
            Self::BrowserTakeScreenshot(arguments) => &mut arguments.tab,
            Self::BrowserClick(arguments) => &mut arguments.tab,
            Self::BrowserHover(arguments) | Self::BrowserScrollToTarget(arguments) => {
                &mut arguments.tab
            }
            Self::BrowserScrollPage(arguments) => &mut arguments.tab,
            Self::BrowserPressKey(arguments) => &mut arguments.tab,
            Self::BrowserDrag(arguments) => &mut arguments.tab,
            Self::BrowserFillForm(arguments) => &mut arguments.tab,
            Self::BrowserWaitFor(arguments) => &mut arguments.tab,
            Self::BrowserHandleDialog(arguments) => &mut arguments.tab,
            Self::BrowserRunSequence(arguments) => {
                arguments.tab = tab.clone();
                for step in &mut arguments.steps {
                    if !step.bind_tab(tab.clone()) {
                        return false;
                    }
                }
                return true;
            }
            _ => return false,
        };
        *target = tab;
        true
    }

    /// Validate all semantic constraints that do not require live browser state.
    pub fn validate(&self) -> Result<(), OperationError> {
        match self {
            Self::BrowserOpenTab(arguments) => {
                if let Some(url) = arguments.url.as_deref() {
                    validate_url(url)?;
                }
            }
            Self::BrowserNavigate(arguments) => validate_url(&arguments.url)?,
            Self::BrowserInspectPage(arguments) => {
                if arguments.query.is_some() && arguments.target.is_some() {
                    return Err(OperationError::AmbiguousInspection);
                }
                if arguments.query.as_deref().is_some_and(|query| {
                    query.is_empty() || query.len() > 1000 || query.chars().any(char::is_control)
                }) {
                    return Err(OperationError::InvalidQuery);
                }
            }
            Self::BrowserReadPage(arguments) if !(1..=50_000).contains(&arguments.max_chars) => {
                return Err(OperationError::InvalidMaxChars);
            }
            Self::BrowserClick(arguments) => {
                if !(1..=3).contains(&arguments.clicks)
                    || (arguments.button != ClickButton::Left && arguments.clicks != 1)
                {
                    return Err(OperationError::InvalidClick);
                }
                validate_modifiers(&arguments.modifiers)?;
            }
            Self::BrowserPressKey(arguments) => validate_modifiers(&arguments.modifiers)?,
            Self::BrowserFillForm(arguments) => {
                if arguments.fields.is_empty() || arguments.fields.len() > 20 {
                    return Err(OperationError::InvalidFormFields);
                }
                let mut targets = HashSet::with_capacity(arguments.fields.len());
                if arguments.fields.iter().any(|field| {
                    !targets.insert(field.field.clone())
                        || !matches!(
                            field.value,
                            Value::Bool(_) | Value::Number(_) | Value::String(_)
                        )
                        || field
                            .value
                            .as_str()
                            .is_some_and(|value| value.len() > 20_000)
                }) {
                    return Err(OperationError::InvalidFormFields);
                }
            }
            Self::BrowserWaitFor(arguments) => {
                if arguments.condition.is_empty()
                    || arguments.condition.len() > 1000
                    || arguments.condition.chars().any(char::is_control)
                {
                    return Err(OperationError::InvalidWaitCondition);
                }
                if !(1..=30_000).contains(&arguments.timeout_ms) {
                    return Err(OperationError::InvalidWaitTimeout);
                }
            }
            Self::BrowserHandleDialog(arguments) => {
                let valid = match (arguments.action, arguments.text.as_deref()) {
                    (DialogResolution::Respond, Some(text)) => {
                        text.len() <= 2000 && !text.chars().any(char::is_control)
                    }
                    (DialogResolution::Respond, None) => false,
                    (_, None) => true,
                    (_, Some(_)) => false,
                };
                if !valid {
                    return Err(OperationError::InvalidDialogResponse);
                }
            }
            Self::BrowserRunSequence(arguments) => validate_sequence(arguments)?,
            _ => {}
        }
        Ok(())
    }
}

fn validate_url(url: &str) -> Result<(), OperationError> {
    if url.is_empty()
        || url.len() > MAX_OPERATION_URL_BYTES
        || url.chars().any(char::is_control)
        || url::Url::parse(url).ok().is_none_or(|parsed| {
            !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none()
        })
    {
        return Err(OperationError::InvalidUrl);
    }
    Ok(())
}

fn validate_modifiers(modifiers: &[KeyModifier]) -> Result<(), OperationError> {
    let unique = modifiers.iter().copied().collect::<HashSet<_>>();
    if unique.len() != modifiers.len() {
        return Err(OperationError::DuplicateModifier);
    }
    Ok(())
}

fn validate_sequence(arguments: &RunSequenceArguments) -> Result<(), OperationError> {
    if !(MIN_OPERATION_SEQUENCE_STEPS..=MAX_OPERATION_SEQUENCE_STEPS)
        .contains(&arguments.steps.len())
    {
        return Err(OperationError::InvalidSequence);
    }
    let mut media_parts = 0usize;
    for step in &arguments.steps {
        if matches!(step, Operation::BrowserTakeScreenshot(_)) {
            media_parts += 1;
        }
        if matches!(
            step,
            Operation::BrowserGetStatus(_)
                | Operation::BrowserOpenTab(_)
                | Operation::BrowserListTabs(_)
                | Operation::BrowserFocusTab(_)
                | Operation::BrowserCloseTab(_)
                | Operation::BrowserRunSequence(_)
        ) || operation_tab(step) != arguments.tab.as_ref()
            || step.validate().is_err()
        {
            return Err(OperationError::InvalidSequence);
        }
    }
    if media_parts > MAX_OPERATION_SEQUENCE_MEDIA_PARTS {
        return Err(OperationError::InvalidSequence);
    }
    Ok(())
}

fn operation_tab(operation: &Operation) -> Option<&TabHandle> {
    match operation {
        Operation::BrowserNavigate(arguments) => arguments.tab.as_ref(),
        Operation::BrowserGoBack(arguments)
        | Operation::BrowserGoForward(arguments)
        | Operation::BrowserReloadPage(arguments)
        | Operation::BrowserPressEscape(arguments)
        | Operation::BrowserGetDialog(arguments) => arguments.tab.as_ref(),
        Operation::BrowserInspectPage(arguments) => arguments.tab.as_ref(),
        Operation::BrowserReadPage(arguments) => arguments.tab.as_ref(),
        Operation::BrowserTakeScreenshot(arguments) => arguments.tab.as_ref(),
        Operation::BrowserClick(arguments) => arguments.tab.as_ref(),
        Operation::BrowserHover(arguments) | Operation::BrowserScrollToTarget(arguments) => {
            arguments.tab.as_ref()
        }
        Operation::BrowserScrollPage(arguments) => arguments.tab.as_ref(),
        Operation::BrowserPressKey(arguments) => arguments.tab.as_ref(),
        Operation::BrowserDrag(arguments) => arguments.tab.as_ref(),
        Operation::BrowserFillForm(arguments) => arguments.tab.as_ref(),
        Operation::BrowserWaitFor(arguments) => arguments.tab.as_ref(),
        Operation::BrowserHandleDialog(arguments) => arguments.tab.as_ref(),
        _ => None,
    }
}

/// Maximum UTF-8 byte length accepted for an opaque tab handle.
pub const MAX_TAB_HANDLE_BYTES: usize = 130;

/// Opaque, service-issued proof that a tab belongs to a workspace.
///
/// A tab handle is verification-only. It is never authority without the corresponding
/// [`WorkspaceId`]. Debug and display output are redacted so native identity cannot leak into
/// logs by accident.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TabHandle(String);

impl TabHandle {
    /// Parse a non-empty, bounded, control-free opaque tab handle.
    pub fn parse(value: &str) -> Option<Self> {
        let suffix = value.strip_prefix("t_")?;
        if !(4..=128).contains(&suffix.len())
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return None;
        }
        Some(Self(value.to_owned()))
    }

    /// Return the raw handle for serialization or exact service-side verification.
    ///
    /// Never use this value as workspace authority or write it to a log, error, or metric label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for TabHandle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TabHandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| serde::de::Error::custom("invalid opaque tab handle"))
    }
}

impl std::fmt::Display for TabHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("<redacted-tab-handle>")
    }
}

impl std::fmt::Debug for TabHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TabHandle(<redacted>)")
    }
}

stable_string_enum! {
    /// Version marker for the canonical browser-result envelope.
    pub enum BrowserResultSchema {
        /// Initial canonical browser-result vocabulary.
        V1 => "ghostlight.browser.result/1"
    }
}

stable_string_enum! {
    /// Canonical terminal status of one browser operation.
    pub enum BrowserResultStatus {
        /// The browser mechanism completed as defined.
        Ok => "ok",
        /// An acknowledged effect completed but a requested follow-up fact did not.
        Partial => "partial",
        /// A requested observation was not met.
        NotMet => "not_met",
        /// Policy, target validity, or another precondition blocked completion.
        Blocked => "blocked",
        /// Human-control hold prevented dispatch.
        Held => "held",
        /// The workspace denial circuit requires user attention.
        AttentionRequired => "attention_required",
        /// Cooperative cancellation retired the work.
        Cancelled => "cancelled",
        /// Admission failed before browser dispatch.
        NotDispatched => "not_dispatched",
        /// Dispatch may have occurred without conclusive terminal proof.
        OutcomeUnknown => "outcome_unknown",
        /// A declared operation was temporarily unavailable.
        Unavailable => "unavailable"
    }
}

stable_string_enum! {
    /// Proven physical-effect disposition for one operation.
    pub enum OperationEffect {
        /// The requested effect was proven not to have been dispatched.
        None => "none",
        /// Dispatch was accepted but no terminal acknowledgement was received.
        Dispatched => "dispatched",
        /// The defined browser acknowledgement was received.
        Committed => "committed",
        /// Available evidence cannot determine whether the effect ran.
        Unknown => "unknown"
    }
}

stable_string_enum! {
    /// Corrective retry guidance derived from the proven effect disposition.
    pub enum RetryDisposition {
        /// Repeating the operation cannot duplicate a dispatched effect.
        Safe => "safe",
        /// Repeating the operation may duplicate an effect.
        Unsafe => "unsafe",
        /// Refresh state before deciding whether to retry.
        AfterStateChange => "after_state_change"
    }
}

stable_string_enum! {
    /// Aggregate state of a requested readiness observation.
    pub enum ReadinessStatus {
        /// Every requested readiness axis completed.
        Ready => "ready",
        /// The readiness deadline expired.
        TimedOut => "timed_out",
        /// The current document could not provide readiness evidence.
        Unavailable => "unavailable",
        /// No readiness observation was requested.
        NotRequested => "not_requested"
    }
}

stable_string_enum! {
    /// State of the settlement readiness axis.
    pub enum SettlementStatus {
        /// The document met the settlement predicate.
        Settled => "settled",
        /// The document did not settle before the deadline.
        NotSettled => "not_settled",
        /// The document could not provide settlement evidence.
        Unavailable => "unavailable"
    }
}

/// Condition-axis readiness evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessCondition {
    /// Whether the operation requested a condition observation.
    pub requested: bool,
    /// Whether the requested condition was met.
    pub met: bool,
}

/// Settlement-axis readiness evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessSettlement {
    /// Whether the operation requested settlement observation.
    pub requested: bool,
    /// Proven settlement disposition.
    pub status: SettlementStatus,
}

/// Canonical readiness evidence, separate from operation success.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Readiness {
    /// Aggregate readiness disposition.
    pub status: ReadinessStatus,
    /// Condition-axis evidence, omitted when that axis was not requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<ReadinessCondition>,
    /// Settlement-axis evidence, omitted when that axis was not requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement: Option<ReadinessSettlement>,
    /// Time spent on requested readiness observation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

stable_string_enum! {
    /// Stable service-authored explanation for a non-normal canonical outcome.
    pub enum ResultProblemCode {
        InvalidArguments => "invalid_arguments",
        WorkspaceUnavailable => "workspace_unavailable",
        TabUnavailable => "tab_unavailable",
        TargetNotFound => "target_not_found",
        TargetAmbiguous => "target_ambiguous",
        TargetStale => "target_stale",
        TargetIneligible => "target_ineligible",
        CredentialInputRequired => "credential_input_required",
        ConditionNotMet => "condition_not_met",
        OperationBlocked => "operation_blocked",
        PolicyBlocked => "policy_blocked",
        ProtectedHost => "protected_host",
        RequestRestriction => "request_restriction",
        HeldByUser => "held_by_user",
        AttentionRequired => "attention_required",
        SessionEnded => "session_ended",
        BrowserDisconnected => "browser_disconnected",
        CapabilityUnavailable => "capability_unavailable",
        DecisionTraceOverflow => "decision_trace_overflow",
        LandingIdentityLost => "landing_identity_lost",
        PartialCompletion => "partial_completion",
        Cancelled => "cancelled",
        NotDispatched => "not_dispatched",
        SequenceStopped => "sequence_stopped",
        OutcomeUnknown => "outcome_unknown"
    }
}

/// One bounded service-authored problem attached to a non-normal result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultProblem {
    /// Stable machine-readable problem identity.
    pub code: ResultProblemCode,
    /// Concise service-authored explanation.
    pub message: String,
}

/// One canonical recovery or continuation that an edge may render for a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SuggestedNextStep {
    /// Offer one complete canonical call without running it automatically.
    Call {
        /// Why the call is relevant now.
        reason: String,
        /// Closed typed operation that the edge renders in its current surface.
        operation: Operation,
    },
    /// Give the model one exact question to ask the user.
    AskUser {
        /// Why user input is required.
        reason: String,
        /// Bounded service-authored question.
        question: String,
    },
    /// Tell the model to wait for the user to return browser control.
    WaitForUser {
        /// Why waiting is the safe next move.
        reason: String,
    },
    /// Tell the model that the browser connection must be restored.
    ReconnectBrowser {
        /// Why reconnection is required.
        reason: String,
    },
    /// Tell the model that its MCP connection must be restored.
    ReconnectClient {
        /// Why reconnection is required.
        reason: String,
    },
    /// Tell the model to stop rather than guess or replay.
    Stop {
        /// Why stopping is the safe next move.
        reason: String,
    },
}

/// Maximum number of model-facing next steps on one result.
pub const MAX_SUGGESTED_NEXT_STEPS: usize = 2;
/// Maximum UTF-8 byte length of canonical summaries, problems, reasons, and questions.
pub const MAX_RESULT_GUIDANCE_BYTES: usize = 240;

/// One concise, protocol-neutral model-facing result part.
///
/// These are product result parts, not MCP content blocks or vendor result wrappers. GIF output
/// uses the image variant with its actual media type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultPart {
    /// Concise canonical text.
    Text {
        /// Text returned to the selected edge renderer.
        text: String,
    },
    /// Base64-encoded image or image-media output.
    Image {
        /// Base64-encoded bytes without a data-URL prefix.
        data: String,
        /// Validated media type, such as `image/jpeg` or `image/gif`.
        mime_type: String,
    },
}

/// Maximum byte length accepted for one canonical image media type.
pub const MAX_RESULT_IMAGE_MIME_TYPE_BYTES: usize = 128;

/// Validation failure for one canonical result part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ResultPartError {
    /// Image data was not non-empty canonical RFC 4648 standard base64.
    #[error("result image data must be non-empty canonical standard base64")]
    InvalidImageData,
    /// The media type was not one bounded, concrete, canonical image type.
    #[error("result image media type must be a bounded concrete lowercase image type")]
    InvalidImageMimeType,
}

impl ResultPart {
    /// Construct one validated canonical image part.
    pub fn image(
        data: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Result<Self, ResultPartError> {
        let data = data.into();
        if !is_canonical_standard_base64(&data) {
            return Err(ResultPartError::InvalidImageData);
        }

        let mime_type = normalize_image_mime_type(&mime_type.into())
            .ok_or(ResultPartError::InvalidImageMimeType)?;
        Ok(Self::Image { data, mime_type })
    }

    /// Validate that this part satisfies the canonical transport invariant.
    pub fn validate(&self) -> Result<(), ResultPartError> {
        let Self::Image { data, mime_type } = self else {
            return Ok(());
        };
        if !is_canonical_standard_base64(data) {
            return Err(ResultPartError::InvalidImageData);
        }
        if normalize_image_mime_type(mime_type).as_deref() != Some(mime_type.as_str()) {
            return Err(ResultPartError::InvalidImageMimeType);
        }
        Ok(())
    }
}

impl Serialize for ResultPart {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;
        match self {
            Self::Text { text } => {
                let mut state = serializer.serialize_struct("ResultPart", 2)?;
                state.serialize_field("type", "text")?;
                state.serialize_field("text", text)?;
                state.end()
            }
            Self::Image { data, mime_type } => {
                let mut state = serializer.serialize_struct("ResultPart", 3)?;
                state.serialize_field("type", "image")?;
                state.serialize_field("data", data)?;
                state.serialize_field("mime_type", mime_type)?;
                state.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ResultPart {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum WireResultPart {
            Text { text: String },
            Image { data: String, mime_type: String },
        }

        match WireResultPart::deserialize(deserializer)? {
            WireResultPart::Text { text } => Ok(Self::Text { text }),
            WireResultPart::Image { data, mime_type } => {
                Self::image(data, mime_type).map_err(serde::de::Error::custom)
            }
        }
    }
}

fn is_canonical_standard_base64(data: &str) -> bool {
    if data.is_empty() {
        return false;
    }

    let bytes = data.as_bytes();
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2 {
        return false;
    }
    let symbol_count = bytes.len() - padding;
    if symbol_count == 0
        || bytes[..symbol_count]
            .iter()
            .any(|byte| base64_sextet(*byte).is_none())
    {
        return false;
    }

    let remainder = symbol_count % 4;
    let valid_padding = match padding {
        0 => remainder != 1,
        1 => bytes.len().is_multiple_of(4) && remainder == 3,
        2 => bytes.len().is_multiple_of(4) && remainder == 2,
        _ => false,
    };
    if !valid_padding {
        return false;
    }

    let tail = base64_sextet(bytes[symbol_count - 1]).expect("symbols were validated above");
    match remainder {
        2 => tail & 0x0f == 0,
        3 => tail & 0x03 == 0,
        _ => true,
    }
}

fn base64_sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn normalize_image_mime_type(mime_type: &str) -> Option<String> {
    if mime_type.is_empty()
        || mime_type.len() > MAX_RESULT_IMAGE_MIME_TYPE_BYTES
        || !mime_type.is_ascii()
    {
        return None;
    }
    let (top_level, subtype) = mime_type.split_once('/')?;
    if !top_level.eq_ignore_ascii_case("image") || subtype.is_empty() {
        return None;
    }

    let mut bytes = subtype.bytes();
    if !bytes.next()?.is_ascii_alphanumeric()
        || bytes.any(|byte| {
            !byte.is_ascii_alphanumeric()
                && !matches!(
                    byte,
                    b'!' | b'#' | b'$' | b'&' | b'-' | b'^' | b'_' | b'.' | b'+'
                )
        })
    {
        return None;
    }
    Some(mime_type.to_ascii_lowercase())
}

/// Tab facts returned by the canonical service result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultTab {
    /// Opaque verification-only tab handle.
    pub id: TabHandle,
    /// Best available page-derived URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Best available page-derived title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Whether this is the workspace's current controlled tab.
    #[serde(default, skip_serializing_if = "is_false")]
    pub current: bool,
    /// Why page facts were withheld from this inventory entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted: Option<TabFactRedaction>,
}

stable_string_enum! {
    /// Browser connectivity reported by `browser_get_status`.
    pub enum BrowserConnectionStatus {
        Connected => "connected",
        Disconnected => "disconnected"
    }
}

stable_string_enum! {
    /// Effective policy source reported by `browser_get_status`.
    pub enum PolicySourceStatus {
        None => "none",
        User => "user",
        Machine => "machine",
        Managed => "managed"
    }
}

stable_string_enum! {
    /// Effective governance posture reported by `browser_get_status`.
    pub enum GovernanceModeStatus {
        Open => "open",
        Observe => "observe",
        Enforce => "enforce"
    }
}

stable_string_enum! {
    /// Mechanical action that one inspected target supports.
    pub enum TargetAction {
        Click => "click",
        Hover => "hover",
        ScrollTo => "scroll_to",
        Fill => "fill",
        Drag => "drag",
        PressKey => "press_key"
    }
}

stable_string_enum! {
    /// Scope represented by one screenshot result.
    pub enum CaptureScope {
        Viewport => "viewport",
        Target => "target"
    }
}

stable_string_enum! {
    /// JavaScript dialog kind observed by the browser.
    pub enum DialogKind {
        Alert => "alert",
        Confirm => "confirm",
        Prompt => "prompt",
        BeforeUnload => "beforeunload",
        Unknown => "unknown"
    }
}

/// Bounded, operation-ready target facts returned by inspection and action operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetFact {
    /// Fresh opaque target reference.
    pub r#ref: String,
    /// Accessible role when observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Accessible name when observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Current visibility when observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Current eligibility when observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Mechanical actions supported by the target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<TargetAction>,
}

/// Effective status authority facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusAuthority {
    /// Selected policy source.
    pub policy_source: PolicySourceStatus,
    /// Effective policy mode.
    pub mode: GovernanceModeStatus,
}

/// Fixed service limits reported to a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusLimits {
    /// Maximum children in one sequence.
    pub max_sequence_steps: u32,
    /// Maximum owned tabs returned in one inventory.
    pub max_tabs: u32,
    /// Maximum characters returned by one page read.
    pub max_read_chars: u32,
}

/// One successfully filled form field, without its supplied value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilledFieldResult {
    /// Model-authored field description.
    pub field: String,
}

/// One form field that was not mutated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFieldResult {
    /// Model-authored field description.
    pub field: String,
    /// Stable service-authored reason token.
    pub code: String,
}

/// Closed typed result vocabulary for all Ghostlight operations.
///
/// This enum is the owner-bridge result authority. Its serde tag is an internal transport detail;
/// model-facing rendering emits only the selected variant payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum OperationResult {
    /// `browser_get_status` result.
    BrowserGetStatus {
        /// Browser connectivity.
        browser: BrowserConnectionStatus,
        /// Effective authority.
        authority: StatusAuthority,
        /// Available operations.
        operations: Vec<OperationKind>,
        /// Enabled optional packs.
        packs: Vec<String>,
        /// Fixed limits.
        limits: StatusLimits,
    },
    /// `browser_open_tab` result.
    BrowserOpenTab {
        /// Whether tab creation committed.
        created: bool,
        /// Whether requested initial navigation conclusively committed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        navigated: Option<bool>,
    },
    /// `browser_list_tabs` result.
    BrowserListTabs { count: u32 },
    /// `browser_focus_tab` result.
    BrowserFocusTab { focused: bool },
    /// `browser_close_tab` result.
    BrowserCloseTab { closed: bool },
    /// `browser_navigate` result.
    BrowserNavigate { landed: bool },
    /// `browser_go_back` result.
    BrowserGoBack { moved: bool },
    /// `browser_go_forward` result.
    BrowserGoForward { moved: bool },
    /// `browser_reload_page` result.
    BrowserReloadPage { reloaded: bool },
    /// `browser_inspect_page` result.
    BrowserInspectPage {
        targets: Vec<TargetFact>,
        more: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<CanonicalCursor>,
    },
    /// `browser_read_page` result.
    BrowserReadPage {
        text: String,
        more: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<CanonicalCursor>,
    },
    /// `browser_take_screenshot` result.
    BrowserTakeScreenshot {
        frame: String,
        width: u32,
        height: u32,
        scope: CaptureScope,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<TargetFact>,
    },
    /// `browser_click` result.
    BrowserClick {
        target: TargetFact,
        clicked: bool,
        page_changed: bool,
    },
    /// `browser_hover` result.
    BrowserHover {
        target: TargetFact,
        hovered: bool,
        page_changed: bool,
    },
    /// `browser_scroll_to_target` result.
    BrowserScrollToTarget {
        target: TargetFact,
        visible: bool,
        moved: bool,
        page_changed: bool,
    },
    /// `browser_scroll_page` result.
    BrowserScrollPage {
        direction: ScrollDirection,
        amount: ScrollAmount,
        moved: bool,
        page_changed: bool,
    },
    /// `browser_press_key` result.
    BrowserPressKey {
        key: NamedKey,
        target: TargetFact,
        pressed: bool,
        page_changed: bool,
    },
    /// `browser_press_escape` result.
    BrowserPressEscape { pressed: bool, page_changed: bool },
    /// `browser_drag` result.
    BrowserDrag {
        from: TargetFact,
        to: TargetFact,
        dragged: bool,
        page_changed: bool,
    },
    /// `browser_fill_form` result.
    BrowserFillForm {
        filled: Vec<FilledFieldResult>,
        skipped: Vec<SkippedFieldResult>,
        submitted: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        submit_target: Option<TargetFact>,
    },
    /// `browser_wait_for` result.
    BrowserWaitFor {
        condition: String,
        state: WaitState,
        met: bool,
        elapsed_ms: u32,
    },
    /// `browser_run_sequence` result.
    BrowserRunSequence(FlowResultData),
    /// `browser_get_dialog` result.
    BrowserGetDialog {
        open: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<DialogKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        actions: Vec<DialogResolution>,
    },
    /// `browser_handle_dialog` result.
    BrowserHandleDialog {
        action: DialogResolution,
        resolved: bool,
    },
}

impl OperationResult {
    /// Return the exact operation kind that owns this result.
    pub fn kind(&self) -> OperationKind {
        match self {
            Self::BrowserGetStatus { .. } => OperationKind::BrowserGetStatus,
            Self::BrowserOpenTab { .. } => OperationKind::BrowserOpenTab,
            Self::BrowserListTabs { .. } => OperationKind::BrowserListTabs,
            Self::BrowserFocusTab { .. } => OperationKind::BrowserFocusTab,
            Self::BrowserCloseTab { .. } => OperationKind::BrowserCloseTab,
            Self::BrowserNavigate { .. } => OperationKind::BrowserNavigate,
            Self::BrowserGoBack { .. } => OperationKind::BrowserGoBack,
            Self::BrowserGoForward { .. } => OperationKind::BrowserGoForward,
            Self::BrowserReloadPage { .. } => OperationKind::BrowserReloadPage,
            Self::BrowserInspectPage { .. } => OperationKind::BrowserInspectPage,
            Self::BrowserReadPage { .. } => OperationKind::BrowserReadPage,
            Self::BrowserTakeScreenshot { .. } => OperationKind::BrowserTakeScreenshot,
            Self::BrowserClick { .. } => OperationKind::BrowserClick,
            Self::BrowserHover { .. } => OperationKind::BrowserHover,
            Self::BrowserScrollToTarget { .. } => OperationKind::BrowserScrollToTarget,
            Self::BrowserScrollPage { .. } => OperationKind::BrowserScrollPage,
            Self::BrowserPressKey { .. } => OperationKind::BrowserPressKey,
            Self::BrowserPressEscape { .. } => OperationKind::BrowserPressEscape,
            Self::BrowserDrag { .. } => OperationKind::BrowserDrag,
            Self::BrowserFillForm { .. } => OperationKind::BrowserFillForm,
            Self::BrowserWaitFor { .. } => OperationKind::BrowserWaitFor,
            Self::BrowserRunSequence(_) => OperationKind::BrowserRunSequence,
            Self::BrowserGetDialog { .. } => OperationKind::BrowserGetDialog,
            Self::BrowserHandleDialog { .. } => OperationKind::BrowserHandleDialog,
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

stable_string_enum! {
    /// Stable reason that an owned tab inventory entry omits its page URL and title.
    pub enum TabFactRedaction {
        ProtectedHost => "protected_host",
        Policy => "policy",
        RequestRestriction => "request_restriction",
        ResourceIndeterminate => "resource_indeterminate"
    }
}

/// Maximum number of owned tab facts carried by one canonical result inventory.
pub const MAX_RESULT_TABS: usize = 64;
/// Maximum UTF-8 byte length of a canonical tab URL.
pub const MAX_RESULT_TAB_URL_BYTES: usize = 4096;
/// Maximum UTF-8 byte length of a canonical tab title.
pub const MAX_RESULT_TAB_TITLE_BYTES: usize = 1024;

/// Maximum UTF-8 byte length accepted for a page origin carried as provenance.
pub const MAX_PAGE_ORIGIN_BYTES: usize = 240;

/// Validation failure for page-derived result provenance.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PageProvenanceError {
    /// A provenance marker is only useful when it names at least one page-derived field.
    #[error("page provenance must name at least one untrusted field")]
    Empty,
    /// The JSON pointer names a service-authored or unknown result field.
    #[error("untrusted field is outside the page-derived result scope: {pointer}")]
    OutOfScope {
        /// Rejected JSON pointer.
        pointer: String,
    },
    /// A frame origin was empty, unbounded, or contained a control character.
    #[error("frame origin must be non-empty, control-free, and at most 240 UTF-8 bytes")]
    InvalidFrameOrigin,
}

/// Scoped provenance for page-derived fields in one canonical result.
///
/// Only page payload under `result`, text/image bytes under `parts`, and page-derived tab URL/title
/// facts may be named. Service-authored schema, operation, status, effect, retry, recovery,
/// workspace, and handle facts remain trusted by construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageProvenance {
    untrusted_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    top_origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session_nonce: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frame_origin: Option<String>,
}

impl PageProvenance {
    /// Validate and construct scoped page-derived provenance.
    pub fn new(
        untrusted_fields: Vec<String>,
        top_origin: Option<String>,
        session_nonce: Option<String>,
        frame_origin: Option<String>,
    ) -> std::result::Result<Self, PageProvenanceError> {
        if untrusted_fields.is_empty() {
            return Err(PageProvenanceError::Empty);
        }
        if let Some(pointer) = untrusted_fields
            .iter()
            .find(|pointer| !is_page_derived_pointer(pointer))
        {
            return Err(PageProvenanceError::OutOfScope {
                pointer: pointer.clone(),
            });
        }
        if frame_origin
            .as_deref()
            .is_some_and(|origin| !is_valid_page_origin(origin))
        {
            return Err(PageProvenanceError::InvalidFrameOrigin);
        }
        Ok(Self {
            untrusted_fields,
            top_origin,
            session_nonce,
            frame_origin,
        })
    }

    /// Return the scoped JSON pointers identifying page-derived result fields.
    pub fn untrusted_fields(&self) -> &[String] {
        &self.untrusted_fields
    }

    /// Return the best available top-level page origin.
    pub fn top_origin(&self) -> Option<&str> {
        self.top_origin.as_deref()
    }

    /// Return the service-authored nonce that delimits one page-content session.
    pub fn session_nonce(&self) -> Option<&str> {
        self.session_nonce.as_deref()
    }

    /// Return the best available origin for the frame that supplied the page payload.
    pub fn frame_origin(&self) -> Option<&str> {
        self.frame_origin.as_deref()
    }
}

impl<'de> Deserialize<'de> for PageProvenance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireProvenance {
            untrusted_fields: Vec<String>,
            #[serde(default)]
            top_origin: Option<String>,
            #[serde(default)]
            session_nonce: Option<String>,
            #[serde(default)]
            frame_origin: Option<String>,
        }

        let value = WireProvenance::deserialize(deserializer)?;
        Self::new(
            value.untrusted_fields,
            value.top_origin,
            value.session_nonce,
            value.frame_origin,
        )
        .map_err(serde::de::Error::custom)
    }
}

fn is_valid_page_origin(origin: &str) -> bool {
    !origin.is_empty()
        && origin.len() <= MAX_PAGE_ORIGIN_BYTES
        && !origin.chars().any(char::is_control)
}

fn is_page_derived_pointer(pointer: &str) -> bool {
    if pointer == "/result" || pointer.starts_with("/result/") {
        return true;
    }
    if matches!(pointer, "/tab/url" | "/tab/title") {
        return true;
    }
    if let Some(inventory_pointer) = pointer.strip_prefix("/tabs/") {
        let Some((index, field)) = inventory_pointer.split_once('/') else {
            return false;
        };
        return !index.is_empty()
            && index.bytes().all(|byte| byte.is_ascii_digit())
            && matches!(field, "url" | "title");
    }

    let Some(part_pointer) = pointer.strip_prefix("/parts/") else {
        return false;
    };
    let Some((index, field)) = part_pointer.split_once('/') else {
        return false;
    };
    !index.is_empty()
        && index.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(field, "text" | "data")
}

/// Versioned, protocol-neutral terminal result produced by the operation pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserResult {
    /// Canonical result schema marker.
    pub schema: BrowserResultSchema,
    /// Exact canonical operation that produced this result.
    pub operation: OperationKind,
    /// Canonical terminal status.
    pub status: BrowserResultStatus,
    /// Concise service-authored account of the outcome.
    pub summary: String,
    /// Proven physical-effect disposition.
    pub effect: OperationEffect,
    /// Corrective replay guidance derived from the proven terminal effect.
    pub repeat: RetryDisposition,
    /// Stable service-authored explanation for a non-normal outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<ResultProblem>,
    /// Zero to two safe, immediately actionable continuations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_next_steps: Vec<SuggestedNextStep>,
    /// Readiness evidence, distinct from operation success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<Readiness>,
    /// Workspace used or created by the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceId>,
    /// Bounded tab facts relevant to the result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<ResultTab>,
    /// Stable, deduplicated inventory of owned tabs relevant to the result.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<ResultTab>,
    /// Concise protocol-neutral text and image output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ResultPart>,
    /// Exact typed result owned by this operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<OperationResult>,
    /// Scoped page-derived provenance, omitted when the result has no page payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<PageProvenance>,
}

/// A canonical browser result carries an internally inconsistent terminal disposition.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BrowserResultValidationError {
    /// The service-authored result summary was empty, unbounded, or contained a control character.
    #[error(
        "a canonical result summary must be non-empty, control-free, and at most 240 UTF-8 bytes"
    )]
    InvalidSummary,
    /// Normal results cannot claim a problem, and every non-normal result must explain one.
    #[error("a canonical problem is required exactly when status is not ok")]
    InvalidProblemPresence,
    /// A problem message was empty, unbounded, or contained a control character.
    #[error(
        "a canonical problem message must be non-empty, control-free, and at most 240 UTF-8 bytes"
    )]
    InvalidProblemMessage,
    /// A result exceeded the fixed next-step bound.
    #[error("a canonical result may suggest at most two next steps")]
    TooManySuggestedNextSteps,
    /// A suggested reason or question was empty, unbounded, or contained a control character.
    #[error(
        "canonical next-step text must be non-empty, control-free, and at most 240 UTF-8 bytes"
    )]
    InvalidSuggestedNextStep,
    /// An uncertain result suggested replaying the same operation or another effectful action.
    #[error(
        "an uncertain result may suggest only an observation, user handoff, reconnect, or stop"
    )]
    UnsafeSuggestedNextStep,
    /// Normal and partially completed operations require their exact typed result payload.
    #[error("ok, partial, and not_met browser results require a typed operation result")]
    MissingOperationResult,
    /// A typed result variant belonged to a different operation.
    #[error("a canonical operation result must match its enclosing operation")]
    MismatchedOperationResult,
    /// Terminal results cannot retain the in-flight dispatched effect.
    #[error("a terminal browser result cannot have effect dispatched")]
    TerminalDispatched,
    /// Outcome-unknown status has one exact effect and retry contract.
    #[error("outcome_unknown requires effect unknown and retry unsafe")]
    InvalidOutcomeUnknown,
    /// Unknown effect cannot accompany another terminal status.
    #[error("effect unknown requires outcome_unknown status")]
    UnknownEffectWithTerminalStatus,
    /// Cancellation retry guidance must match its proven effect.
    #[error("cancelled requires no/safe retry with no effect and unsafe retry otherwise")]
    InvalidCancellation,
    /// A proven pre-dispatch terminal cannot claim a physical effect.
    #[error("held, attention_required, and not_dispatched require effect none")]
    PreDispatchStatusWithEffect,
    /// Readiness aggregate and axis evidence must agree exactly.
    #[error("readiness aggregate status does not match its requested axes")]
    InvalidReadiness,
    /// Canonical tab inventories have one fixed upper bound.
    #[error("a canonical result tab inventory may contain at most {max} entries")]
    TooManyTabs {
        /// Maximum accepted inventory length.
        max: usize,
    },
    /// One opaque tab may appear at most once in the canonical inventory.
    #[error("a canonical result tab inventory contains a duplicate handle")]
    DuplicateTab,
    /// Tab URLs are bounded, non-empty, and control-free.
    #[error("a canonical result contains an invalid tab URL")]
    InvalidTabUrl,
    /// Tab titles are bounded, non-empty, and control-free.
    #[error("a canonical result contains an invalid tab title")]
    InvalidTabTitle,
    /// A redacted inventory entry cannot retain the page facts it withholds.
    #[error("a redacted canonical tab cannot contain a URL or title")]
    RedactedTabHasPageFacts,
}

impl BrowserResult {
    /// Construct an empty version-one canonical result envelope.
    pub fn new(
        operation: OperationKind,
        status: BrowserResultStatus,
        effect: OperationEffect,
    ) -> Self {
        Self {
            schema: BrowserResultSchema::V1,
            operation,
            status,
            summary: default_result_summary(operation, status, effect),
            effect,
            repeat: repeat_for_terminal(status, effect),
            problem: default_result_problem(status),
            suggested_next_steps: Vec::new(),
            readiness: None,
            workspace: None,
            tab: None,
            tabs: Vec::new(),
            parts: Vec::new(),
            result: None,
            provenance: None,
        }
    }

    /// Validate the closed status/effect/retry relationship before edge rendering.
    pub fn validate_semantics(&self) -> Result<(), BrowserResultValidationError> {
        if !is_valid_guidance_text(&self.summary) {
            return Err(BrowserResultValidationError::InvalidSummary);
        }
        if (self.status == BrowserResultStatus::Ok) != self.problem.is_none() {
            return Err(BrowserResultValidationError::InvalidProblemPresence);
        }
        if self
            .problem
            .as_ref()
            .is_some_and(|problem| !is_valid_guidance_text(&problem.message))
        {
            return Err(BrowserResultValidationError::InvalidProblemMessage);
        }
        if self.suggested_next_steps.len() > MAX_SUGGESTED_NEXT_STEPS {
            return Err(BrowserResultValidationError::TooManySuggestedNextSteps);
        }
        for suggestion in &self.suggested_next_steps {
            if !suggestion_has_valid_text(suggestion) {
                return Err(BrowserResultValidationError::InvalidSuggestedNextStep);
            }
            if self.effect == OperationEffect::Unknown
                && matches!(suggestion, SuggestedNextStep::Call { operation, .. } if !is_observation_operation(operation.kind()))
            {
                return Err(BrowserResultValidationError::UnsafeSuggestedNextStep);
            }
        }
        if self.tabs.len() > MAX_RESULT_TABS {
            return Err(BrowserResultValidationError::TooManyTabs {
                max: MAX_RESULT_TABS,
            });
        }
        let mut tab_handles = HashSet::with_capacity(self.tabs.len());
        if self
            .tabs
            .iter()
            .any(|tab| !tab_handles.insert(tab.id.clone()))
        {
            return Err(BrowserResultValidationError::DuplicateTab);
        }
        for tab in self.tab.iter().chain(self.tabs.iter()) {
            if tab.redacted.is_some() && (tab.url.is_some() || tab.title.is_some()) {
                return Err(BrowserResultValidationError::RedactedTabHasPageFacts);
            }
            if tab.url.as_ref().is_some_and(|url| {
                url.is_empty()
                    || url.len() > MAX_RESULT_TAB_URL_BYTES
                    || url.chars().any(char::is_control)
            }) {
                return Err(BrowserResultValidationError::InvalidTabUrl);
            }
            if tab.title.as_ref().is_some_and(|title| {
                title.is_empty()
                    || title.len() > MAX_RESULT_TAB_TITLE_BYTES
                    || title.chars().any(char::is_control)
            }) {
                return Err(BrowserResultValidationError::InvalidTabTitle);
            }
        }
        if let Some(readiness) = &self.readiness {
            if matches!(
                self.status,
                BrowserResultStatus::Blocked
                    | BrowserResultStatus::Held
                    | BrowserResultStatus::AttentionRequired
                    | BrowserResultStatus::NotDispatched
                    | BrowserResultStatus::OutcomeUnknown
                    | BrowserResultStatus::Unavailable
            ) || !readiness_is_consistent(readiness)
            {
                return Err(BrowserResultValidationError::InvalidReadiness);
            }
        }
        if self.effect == OperationEffect::Dispatched {
            return Err(BrowserResultValidationError::TerminalDispatched);
        }
        if self.status == BrowserResultStatus::OutcomeUnknown {
            if self.effect != OperationEffect::Unknown || self.repeat != RetryDisposition::Unsafe {
                return Err(BrowserResultValidationError::InvalidOutcomeUnknown);
            }
            return Ok(());
        }
        if self.status == BrowserResultStatus::Cancelled {
            let valid = match self.effect {
                OperationEffect::None => self.repeat == RetryDisposition::Safe,
                OperationEffect::Committed | OperationEffect::Unknown => {
                    self.repeat == RetryDisposition::Unsafe
                }
                OperationEffect::Dispatched => false,
            };
            return valid
                .then_some(())
                .ok_or(BrowserResultValidationError::InvalidCancellation);
        }
        if self.effect == OperationEffect::Unknown {
            return Err(BrowserResultValidationError::UnknownEffectWithTerminalStatus);
        }
        if matches!(
            self.status,
            BrowserResultStatus::Held
                | BrowserResultStatus::AttentionRequired
                | BrowserResultStatus::NotDispatched
        ) && self.effect != OperationEffect::None
        {
            return Err(BrowserResultValidationError::PreDispatchStatusWithEffect);
        }
        if matches!(
            self.status,
            BrowserResultStatus::Ok | BrowserResultStatus::Partial | BrowserResultStatus::NotMet
        ) && self.result.is_none()
        {
            return Err(BrowserResultValidationError::MissingOperationResult);
        }
        if self
            .result
            .as_ref()
            .is_some_and(|result| result.kind() != self.operation)
        {
            return Err(BrowserResultValidationError::MismatchedOperationResult);
        }
        Ok(())
    }
}

fn default_result_summary(
    operation: OperationKind,
    status: BrowserResultStatus,
    effect: OperationEffect,
) -> String {
    let job = operation
        .as_str()
        .strip_prefix("browser_")
        .unwrap_or(operation.as_str())
        .replace('_', " ");
    match status {
        BrowserResultStatus::Ok if effect == OperationEffect::Committed => {
            format!("Completed {job}.")
        }
        BrowserResultStatus::Ok => format!("Finished {job}."),
        BrowserResultStatus::Partial => {
            format!("Part of {job} completed; keep the committed work.")
        }
        BrowserResultStatus::NotMet => format!("The requested {job} condition was not met."),
        BrowserResultStatus::Blocked => format!("Ghostlight stopped {job} before acting."),
        BrowserResultStatus::Held => {
            "The user is controlling the browser, so this call is paused.".into()
        }
        BrowserResultStatus::AttentionRequired => {
            "Ghostlight needs the user's attention before browser work can continue.".into()
        }
        BrowserResultStatus::Cancelled => {
            format!("Cancelled {job}; completed effects were not undone.")
        }
        BrowserResultStatus::NotDispatched => format!("Did not send {job} to the browser."),
        BrowserResultStatus::OutcomeUnknown => {
            format!("Ghostlight cannot prove whether {job} completed.")
        }
        BrowserResultStatus::Unavailable => format!("The browser could not complete {job}."),
    }
}

fn default_result_problem(status: BrowserResultStatus) -> Option<ResultProblem> {
    let (code, message) = match status {
        BrowserResultStatus::Ok => return None,
        BrowserResultStatus::Partial => (
            ResultProblemCode::PartialCompletion,
            "Part of the requested work completed.",
        ),
        BrowserResultStatus::NotMet => (
            ResultProblemCode::ConditionNotMet,
            "The requested condition or state was not met.",
        ),
        BrowserResultStatus::Blocked => (
            ResultProblemCode::OperationBlocked,
            "Ghostlight blocked the operation before it could complete.",
        ),
        BrowserResultStatus::Held => (
            ResultProblemCode::HeldByUser,
            "The user currently controls the browser.",
        ),
        BrowserResultStatus::AttentionRequired => (
            ResultProblemCode::AttentionRequired,
            "Ghostlight needs the user's attention before work can continue.",
        ),
        BrowserResultStatus::Cancelled => {
            (ResultProblemCode::Cancelled, "The operation was cancelled.")
        }
        BrowserResultStatus::NotDispatched => (
            ResultProblemCode::NotDispatched,
            "The operation was not sent to the browser.",
        ),
        BrowserResultStatus::OutcomeUnknown => (
            ResultProblemCode::OutcomeUnknown,
            "Ghostlight cannot prove whether the operation completed.",
        ),
        BrowserResultStatus::Unavailable => (
            ResultProblemCode::CapabilityUnavailable,
            "The required browser capability is unavailable.",
        ),
    };
    Some(ResultProblem {
        code,
        message: message.into(),
    })
}

fn is_valid_guidance_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_RESULT_GUIDANCE_BYTES
        && !value.chars().any(char::is_control)
}

fn suggestion_has_valid_text(suggestion: &SuggestedNextStep) -> bool {
    match suggestion {
        SuggestedNextStep::Call { reason, operation } => {
            is_valid_guidance_text(reason) && operation.validate().is_ok()
        }
        SuggestedNextStep::AskUser { reason, question } => {
            is_valid_guidance_text(reason) && is_valid_guidance_text(question)
        }
        SuggestedNextStep::WaitForUser { reason }
        | SuggestedNextStep::ReconnectBrowser { reason }
        | SuggestedNextStep::ReconnectClient { reason }
        | SuggestedNextStep::Stop { reason } => is_valid_guidance_text(reason),
    }
}

const fn is_observation_operation(operation: OperationKind) -> bool {
    matches!(
        operation,
        OperationKind::BrowserGetStatus
            | OperationKind::BrowserListTabs
            | OperationKind::BrowserInspectPage
            | OperationKind::BrowserReadPage
            | OperationKind::BrowserTakeScreenshot
            | OperationKind::BrowserWaitFor
            | OperationKind::BrowserGetDialog
    )
}

/// Derive the safe default replay guidance for one proven terminal disposition.
pub const fn repeat_for_terminal(
    status: BrowserResultStatus,
    effect: OperationEffect,
) -> RetryDisposition {
    match (status, effect) {
        (
            BrowserResultStatus::Blocked
            | BrowserResultStatus::Held
            | BrowserResultStatus::AttentionRequired,
            _,
        ) => RetryDisposition::AfterStateChange,
        (
            _,
            OperationEffect::Committed | OperationEffect::Dispatched | OperationEffect::Unknown,
        ) => RetryDisposition::Unsafe,
        _ => RetryDisposition::Safe,
    }
}

fn readiness_is_consistent(readiness: &Readiness) -> bool {
    let condition = readiness.condition;
    let settlement = readiness.settlement;
    if condition.is_some_and(|axis| !axis.requested)
        || settlement.is_some_and(|axis| !axis.requested)
    {
        return false;
    }
    match readiness.status {
        ReadinessStatus::NotRequested => condition.is_none() && settlement.is_none(),
        ReadinessStatus::Ready => {
            (condition.is_some() || settlement.is_some())
                && condition.is_none_or(|axis| axis.met)
                && settlement.is_none_or(|axis| axis.status == SettlementStatus::Settled)
        }
        ReadinessStatus::TimedOut => {
            (condition.is_some() || settlement.is_some())
                && (condition.is_some_and(|axis| !axis.met)
                    || settlement.is_some_and(|axis| axis.status == SettlementStatus::NotSettled))
                && settlement.is_none_or(|axis| axis.status != SettlementStatus::Unavailable)
        }
        ReadinessStatus::Unavailable => {
            settlement.is_some_and(|axis| axis.status == SettlementStatus::Unavailable)
        }
    }
}

stable_string_enum! {
    /// Canonical execution state of one step inside a browser flow.
    pub enum FlowStepStatus {
        /// The step completed as defined.
        Ok => "ok",
        /// The step committed an effect but did not complete every requested observation.
        Partial => "partial",
        /// The step's requested observation was not met.
        NotMet => "not_met",
        /// A semantic precondition blocked the step.
        Blocked => "blocked",
        /// A policy decision denied the step before dispatch.
        Denied => "denied",
        /// Human control held the step before dispatch.
        Held => "held",
        /// The workspace denial circuit requires user attention.
        AttentionRequired => "attention_required",
        /// Cooperative cancellation retired the step.
        Cancelled => "cancelled",
        /// Admission failed before browser dispatch.
        NotDispatched => "not_dispatched",
        /// Dispatch may have occurred without conclusive terminal proof.
        OutcomeUnknown => "outcome_unknown",
        /// The operation was temporarily unavailable or returned an invalid result.
        Unavailable => "unavailable",
        /// Flow control prevented this declared step from running.
        NotRun => "not_run",
        /// Preflight proved that the step would be admitted without dispatching it.
        WouldAllow => "would_allow",
        /// Preflight proved that policy would deny the step without dispatching it.
        WouldDeny => "would_deny"
    }
}

/// One ordered canonical step result inside a browser flow.
///
/// The nested [`BrowserResult`] carries canonical operation identity, effect and retry facts,
/// result parts, structured data, and typed provenance. `status` adds flow-control distinctions
/// such as `denied`, `not_run`, and preflight verdicts without introducing a surface tool name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStepResult {
    /// One-based position in the declared flow.
    pub step: u32,
    /// Flow-control disposition for this step.
    pub status: FlowStepStatus,
    /// Canonical result facts for the declared operation.
    pub result: BrowserResult,
}

stable_string_enum! {
    /// Why canonical flow iteration stopped.
    pub enum FlowTerminationReason {
        /// Every declared step was considered, including continue-on-error execution.
        Completed => "completed",
        /// A non-policy step failure stopped iteration.
        Failed => "failed",
        /// A policy denial stopped iteration.
        Denied => "denied",
        /// Human control held iteration.
        Held => "held",
        /// The workspace denial circuit stopped iteration.
        AttentionRequired => "attention_required",
        /// Cooperative cancellation stopped iteration.
        Cancelled => "cancelled",
        /// The bounded flow wall-clock budget stopped iteration.
        BudgetExhausted => "budget_exhausted"
    }
}

/// Typed terminal reason for one canonical flow execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowTermination {
    /// Stable reason independent of human summary text.
    pub reason: FlowTerminationReason,
    /// One-based step at or after which iteration stopped, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
}

/// Canonical structured result of one browser flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowResultData {
    /// Ordered result entry for every declared step, including steps that did not run.
    pub steps: Vec<FlowStepResult>,
    /// Concise aggregate summary of completed and stopped work.
    pub summary: String,
    /// Total wall-clock execution duration.
    pub duration_ms: u64,
    /// Typed aggregate stop reason used for truthful status derivation.
    pub termination: FlowTermination,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_handle_is_bounded_opaque_and_redacted() {
        let handle = TabHandle::parse("t_generation_7").expect("valid handle");
        assert_eq!(handle.as_str(), "t_generation_7");
        assert_eq!(handle.to_string(), "<redacted-tab-handle>");
        assert_eq!(format!("{handle:?}"), "TabHandle(<redacted>)");
        let json = serde_json::to_string(&handle).expect("serialize handle");
        assert_eq!(
            serde_json::from_str::<TabHandle>(&json).expect("deserialize handle"),
            handle
        );
        assert!(TabHandle::parse("").is_none());
        assert!(TabHandle::parse("bad\nhandle").is_none());
        assert!(TabHandle::parse(&"x".repeat(MAX_TAB_HANDLE_BYTES + 1)).is_none());
    }

    #[test]
    fn image_parts_enforce_canonical_base64_and_media_types() {
        for data in [
            "AA==", "AA", "AAA=", "AAA", "AAAA", "Zg==", "Zg", "Zm8=", "Zm8", "R0lGODlh",
        ] {
            assert!(ResultPart::image(data, "image/png").is_ok(), "{data}");
        }
        for data in [
            "",
            "A",
            "=AAA",
            "AA=A",
            "AA=",
            "AA===",
            "AAA==",
            "AAAA=",
            "AA A=",
            "AA\nA",
            "AA-_",
            "data:image/png;base64,AA==",
            "Zh==",
            "Zh",
            "Zm9=",
            "Zm9",
        ] {
            assert_eq!(
                ResultPart::image(data, "image/png"),
                Err(ResultPartError::InvalidImageData),
                "{data}"
            );
        }

        let uppercase = ResultPart::image("AA==", "IMAGE/PNG").expect("case normalizes");
        assert_eq!(
            uppercase,
            ResultPart::Image {
                data: "AA==".into(),
                mime_type: "image/png".into(),
            }
        );
        for mime_type in [
            "image/jpeg",
            "image/gif",
            "image/svg+xml",
            "image/vnd.microsoft.icon",
        ] {
            assert!(ResultPart::image("AA==", mime_type).is_ok(), "{mime_type}");
        }
        for mime_type in [
            "",
            "image/",
            "/png",
            "text/plain",
            "image/*",
            "image/png; charset=binary",
            "image//png",
            " image/png",
            "image/png ",
            "image/p\nng",
            "image/pong\u{00e9}",
            "image/+png",
        ] {
            assert_eq!(
                ResultPart::image("AA==", mime_type),
                Err(ResultPartError::InvalidImageMimeType),
                "{mime_type:?}"
            );
        }
        let exact_bound = format!(
            "image/{}",
            "a".repeat(MAX_RESULT_IMAGE_MIME_TYPE_BYTES - "image/".len())
        );
        assert!(ResultPart::image("AA==", exact_bound).is_ok());
        let over_bound = format!(
            "image/{}",
            "a".repeat(MAX_RESULT_IMAGE_MIME_TYPE_BYTES - "image/".len() + 1)
        );
        assert_eq!(
            ResultPart::image("AA==", over_bound),
            Err(ResultPartError::InvalidImageMimeType)
        );
    }

    #[test]
    fn image_part_wire_shape_validates_in_both_directions() {
        let part = ResultPart::image("AA==", "image/png").expect("valid image");
        assert_eq!(
            serde_json::to_string(&part).expect("serialize image"),
            r#"{"type":"image","data":"AA==","mime_type":"image/png"}"#
        );
        assert_eq!(
            serde_json::from_value::<ResultPart>(serde_json::json!({
                "type": "image",
                "data": "AA==",
                "mime_type": "IMAGE/PNG"
            }))
            .expect("deserialize and normalize image"),
            part
        );

        let invalid_in_process = ResultPart::Image {
            data: "AA=".into(),
            mime_type: "image/png".into(),
        };
        assert!(serde_json::to_value(&invalid_in_process).is_err());
        assert!(serde_json::from_value::<ResultPart>(serde_json::json!({
            "type": "image",
            "data": "AA=",
            "mime_type": "image/png"
        }))
        .is_err());

        let mut result = BrowserResult::new(
            OperationKind::BrowserTakeScreenshot,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        result.parts = vec![part];
        let mut value = serde_json::to_value(result).expect("serialize browser result");
        value["parts"][0]["data"] = serde_json::json!("AAAA=");
        assert!(serde_json::from_value::<BrowserResult>(value).is_err());
    }

    #[test]
    fn browser_result_is_canonical_and_protocol_neutral() {
        let workspace = WorkspaceId::mint();
        let tab = TabHandle::parse("t_generation_7").expect("valid handle");
        let inventory_tab = TabHandle::parse("t_generation_8").expect("valid handle");
        let provenance = PageProvenance::new(
            vec![
                "/tab/url".into(),
                "/tab/title".into(),
                "/tabs/0/url".into(),
                "/tabs/0/title".into(),
                "/parts/0/text".into(),
                "/parts/1/data".into(),
                "/result/target".into(),
            ],
            Some("https://example.com".into()),
            Some("session-7".into()),
            Some("https://frame.example".into()),
        )
        .expect("scoped provenance");

        let mut result = BrowserResult::new(
            OperationKind::BrowserClick,
            BrowserResultStatus::Ok,
            OperationEffect::Committed,
        );
        result.readiness = Some(Readiness {
            status: ReadinessStatus::Ready,
            condition: Some(ReadinessCondition {
                requested: true,
                met: true,
            }),
            settlement: Some(ReadinessSettlement {
                requested: true,
                status: SettlementStatus::Settled,
            }),
            elapsed_ms: Some(1850),
        });
        result.workspace = Some(workspace.clone());
        result.tab = Some(ResultTab {
            id: tab,
            url: Some("https://example.com".into()),
            title: Some("Example".into()),
            current: false,
            redacted: None,
        });
        result.tabs = vec![ResultTab {
            id: inventory_tab,
            url: Some("https://inventory.example".into()),
            title: Some("Inventory".into()),
            current: false,
            redacted: None,
        }];
        result.parts = vec![
            ResultPart::Text {
                text: "clicked".into(),
            },
            ResultPart::Image {
                data: "aW1hZ2U=".into(),
                mime_type: "image/jpeg".into(),
            },
        ];
        result.result = Some(OperationResult::BrowserClick {
            target: TargetFact {
                r#ref: "r_save".into(),
                role: Some("button".into()),
                name: Some("Save".into()),
                visible: Some(true),
                enabled: Some(true),
                actions: vec![TargetAction::Click],
            },
            clicked: true,
            page_changed: true,
        });
        result.provenance = Some(provenance);

        let value = serde_json::to_value(&result).expect("serialize canonical result");
        assert_eq!(value["schema"], "ghostlight.browser.result/1");
        assert_eq!(value["operation"], "browser_click");
        assert!(value.get("intent").is_none());
        assert_eq!(value["workspace"], workspace.as_str());
        assert_eq!(value["tabs"][0]["id"], "t_generation_8");
        assert_eq!(value["parts"][0]["type"], "text");
        assert_eq!(value["parts"][1]["type"], "image");
        assert_eq!(value["parts"][1]["mime_type"], "image/jpeg");
        assert_eq!(value["provenance"]["frame_origin"], "https://frame.example");
        assert_eq!(
            serde_json::from_value::<BrowserResult>(value.clone())
                .expect("deserialize canonical result"),
            result
        );

        let rendered = value.to_string();
        assert!(!rendered.contains("\"content\""));
        assert!(!rendered.contains("jsonrpc"));
        assert!(!rendered.contains("structuredContent"));
    }

    #[test]
    fn browser_result_tab_inventory_is_optional_bounded_and_unique() {
        let mut result = BrowserResult::new(
            OperationKind::BrowserListTabs,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        result.result = Some(OperationResult::BrowserListTabs { count: 0 });
        let empty = serde_json::to_value(&result).expect("serialize empty inventory");
        assert!(empty.get("tabs").is_none());

        let first = ResultTab {
            id: TabHandle::parse("t_first").unwrap(),
            url: None,
            title: None,
            current: false,
            redacted: None,
        };
        result.tabs = vec![first.clone(), first];
        assert_eq!(
            result.validate_semantics(),
            Err(BrowserResultValidationError::DuplicateTab)
        );

        result.tabs = (0..=MAX_RESULT_TABS)
            .map(|index| ResultTab {
                id: TabHandle::parse(&format!("t_tab{index}")).unwrap(),
                url: None,
                title: None,
                current: false,
                redacted: None,
            })
            .collect();
        assert_eq!(
            result.validate_semantics(),
            Err(BrowserResultValidationError::TooManyTabs {
                max: MAX_RESULT_TABS
            })
        );
    }

    #[test]
    fn browser_result_tab_fact_bounds_apply_to_singular_and_plural_tabs() {
        fn result_with_tab(tab: ResultTab, plural: bool) -> BrowserResult {
            let mut result = BrowserResult::new(
                OperationKind::BrowserListTabs,
                BrowserResultStatus::Ok,
                OperationEffect::None,
            );
            result.result = Some(OperationResult::BrowserListTabs { count: 1 });
            if plural {
                result.tabs = vec![tab];
            } else {
                result.tab = Some(tab);
            }
            result
        }

        fn assert_invalid_fact(
            url: Option<String>,
            title: Option<String>,
            expected: BrowserResultValidationError,
        ) {
            for plural in [false, true] {
                let tab = ResultTab {
                    id: TabHandle::parse("t_fact_bounds").unwrap(),
                    url: url.clone(),
                    title: title.clone(),
                    current: false,
                    redacted: None,
                };
                assert_eq!(
                    result_with_tab(tab, plural).validate_semantics(),
                    Err(expected.clone())
                );
            }
        }

        let url_prefix = "https://example.test/";
        let boundary_url = format!(
            "{url_prefix}{}",
            "u".repeat(MAX_RESULT_TAB_URL_BYTES - url_prefix.len())
        );
        let boundary_title = "t".repeat(MAX_RESULT_TAB_TITLE_BYTES);
        assert_eq!(boundary_url.len(), MAX_RESULT_TAB_URL_BYTES);
        assert_eq!(boundary_title.len(), MAX_RESULT_TAB_TITLE_BYTES);
        for plural in [false, true] {
            let tab = ResultTab {
                id: TabHandle::parse("t_fact_boundaries").unwrap(),
                url: Some(boundary_url.clone()),
                title: Some(boundary_title.clone()),
                current: false,
                redacted: None,
            };
            assert_eq!(result_with_tab(tab, plural).validate_semantics(), Ok(()));
        }

        assert_invalid_fact(
            Some("u".repeat(MAX_RESULT_TAB_URL_BYTES + 1)),
            Some("valid title".into()),
            BrowserResultValidationError::InvalidTabUrl,
        );
        assert_invalid_fact(
            Some("https://example.test/bad\nurl".into()),
            Some("valid title".into()),
            BrowserResultValidationError::InvalidTabUrl,
        );
        assert_invalid_fact(
            Some("https://example.test/".into()),
            Some("t".repeat(MAX_RESULT_TAB_TITLE_BYTES + 1)),
            BrowserResultValidationError::InvalidTabTitle,
        );
        assert_invalid_fact(
            Some("https://example.test/".into()),
            Some("bad\ntitle".into()),
            BrowserResultValidationError::InvalidTabTitle,
        );
    }

    #[test]
    fn flow_result_round_trips_without_nested_surface_identity() {
        let mut completed = BrowserResult::new(
            OperationKind::BrowserTakeScreenshot,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        completed.parts = vec![
            ResultPart::Text {
                text: "captured".into(),
            },
            ResultPart::Image {
                data: "AAAA".into(),
                mime_type: "image/jpeg".into(),
            },
        ];
        completed.result = Some(OperationResult::BrowserTakeScreenshot {
            frame: "f_image1".into(),
            width: 1,
            height: 1,
            scope: CaptureScope::Viewport,
            target: None,
        });
        completed.provenance = Some(
            PageProvenance::new(
                vec!["/parts/0/text".into(), "/parts/1/data".into()],
                Some("https://example.com".into()),
                Some("session-1".into()),
                None,
            )
            .expect("valid nested provenance"),
        );

        let not_run = BrowserResult::new(
            OperationKind::BrowserClick,
            BrowserResultStatus::NotDispatched,
            OperationEffect::None,
        );
        let flow = FlowResultData {
            steps: vec![
                FlowStepResult {
                    step: 1,
                    status: FlowStepStatus::Ok,
                    result: completed,
                },
                FlowStepResult {
                    step: 2,
                    status: FlowStepStatus::NotRun,
                    result: not_run,
                },
            ],
            summary: "1/2 steps completed".into(),
            duration_ms: 7,
            termination: FlowTermination {
                reason: FlowTerminationReason::Failed,
                step: Some(1),
            },
        };

        let value = serde_json::to_value(&flow).expect("serialize flow result");
        assert_eq!(
            value["steps"][0]["result"]["operation"],
            "browser_take_screenshot"
        );
        assert_eq!(value["steps"][1]["status"], "not_run");
        let rendered = value.to_string();
        assert!(!rendered.contains("\"tool\""));
        assert!(!rendered.contains("\"name\""));
        assert_eq!(
            serde_json::from_value::<FlowResultData>(value).expect("deserialize flow result"),
            flow
        );
    }

    #[test]
    fn browser_result_terminal_dispositions_validate_as_one_closed_contract() {
        let operation = OperationKind::BrowserClick;
        for (status, effect, repeat, expected) in [
            (
                BrowserResultStatus::Ok,
                OperationEffect::Unknown,
                RetryDisposition::Unsafe,
                BrowserResultValidationError::UnknownEffectWithTerminalStatus,
            ),
            (
                BrowserResultStatus::Partial,
                OperationEffect::Dispatched,
                RetryDisposition::Unsafe,
                BrowserResultValidationError::TerminalDispatched,
            ),
            (
                BrowserResultStatus::OutcomeUnknown,
                OperationEffect::Unknown,
                RetryDisposition::Safe,
                BrowserResultValidationError::InvalidOutcomeUnknown,
            ),
            (
                BrowserResultStatus::Held,
                OperationEffect::Committed,
                RetryDisposition::Unsafe,
                BrowserResultValidationError::PreDispatchStatusWithEffect,
            ),
            (
                BrowserResultStatus::Cancelled,
                OperationEffect::Committed,
                RetryDisposition::Safe,
                BrowserResultValidationError::InvalidCancellation,
            ),
        ] {
            let mut result = BrowserResult::new(operation, status, effect);
            result.repeat = repeat;
            assert_eq!(result.validate_semantics(), Err(expected));
        }

        for (status, effect, repeat) in [
            (
                BrowserResultStatus::OutcomeUnknown,
                OperationEffect::Unknown,
                RetryDisposition::Unsafe,
            ),
            (
                BrowserResultStatus::Blocked,
                OperationEffect::Committed,
                RetryDisposition::Unsafe,
            ),
            (
                BrowserResultStatus::Cancelled,
                OperationEffect::Committed,
                RetryDisposition::Unsafe,
            ),
            (
                BrowserResultStatus::Cancelled,
                OperationEffect::Unknown,
                RetryDisposition::Unsafe,
            ),
        ] {
            let mut result = BrowserResult::new(operation, status, effect);
            result.repeat = repeat;
            assert_eq!(result.validate_semantics(), Ok(()));
        }
    }

    #[test]
    fn browser_result_readiness_axes_validate_as_one_closed_contract() {
        let mut result = BrowserResult::new(
            OperationKind::BrowserNavigate,
            BrowserResultStatus::Ok,
            OperationEffect::Committed,
        );
        result.result = Some(OperationResult::BrowserNavigate { landed: true });
        result.readiness = Some(Readiness {
            status: ReadinessStatus::Ready,
            condition: None,
            settlement: Some(ReadinessSettlement {
                requested: true,
                status: SettlementStatus::Settled,
            }),
            elapsed_ms: Some(125),
        });
        assert_eq!(result.validate_semantics(), Ok(()));

        for readiness in [
            Readiness {
                status: ReadinessStatus::Ready,
                condition: None,
                settlement: None,
                elapsed_ms: Some(1),
            },
            Readiness {
                status: ReadinessStatus::TimedOut,
                condition: None,
                settlement: Some(ReadinessSettlement {
                    requested: true,
                    status: SettlementStatus::Settled,
                }),
                elapsed_ms: Some(1),
            },
            Readiness {
                status: ReadinessStatus::NotRequested,
                condition: None,
                settlement: Some(ReadinessSettlement {
                    requested: true,
                    status: SettlementStatus::Unavailable,
                }),
                elapsed_ms: Some(1),
            },
        ] {
            result.readiness = Some(readiness);
            assert_eq!(
                result.validate_semantics(),
                Err(BrowserResultValidationError::InvalidReadiness)
            );
        }

        result.status = BrowserResultStatus::Blocked;
        result.problem = default_result_problem(BrowserResultStatus::Blocked);
        result.readiness = Some(Readiness {
            status: ReadinessStatus::Unavailable,
            condition: None,
            settlement: Some(ReadinessSettlement {
                requested: true,
                status: SettlementStatus::Unavailable,
            }),
            elapsed_ms: Some(1),
        });
        assert_eq!(
            result.validate_semantics(),
            Err(BrowserResultValidationError::InvalidReadiness)
        );
    }

    #[test]
    fn blocked_result_can_carry_typed_state_refresh_guidance() {
        let mut result = BrowserResult::new(
            OperationKind::BrowserClick,
            BrowserResultStatus::Blocked,
            OperationEffect::None,
        );
        result.repeat = RetryDisposition::AfterStateChange;
        result.problem = Some(ResultProblem {
            code: ResultProblemCode::TargetStale,
            message: "The target belongs to an older page revision.".into(),
        });
        result.suggested_next_steps = vec![SuggestedNextStep::Call {
            reason: "Refresh page targets before choosing another action.".into(),
            operation: Operation::BrowserInspectPage(InspectPageArguments::default()),
        }];

        let value = serde_json::to_value(result).expect("serialize blocked result");
        assert_eq!(value["status"], "blocked");
        assert_eq!(value["effect"], "none");
        assert_eq!(value["repeat"], "after_state_change");
        assert_eq!(
            value["suggested_next_steps"][0]["operation"],
            serde_json::json!({
                "operation": "browser_inspect_page",
                "arguments": {"include":"interactive"}
            })
        );
    }

    #[test]
    fn canonical_problem_and_recovery_guidance_fail_closed() {
        let mut missing_problem = BrowserResult::new(
            OperationKind::BrowserClick,
            BrowserResultStatus::Blocked,
            OperationEffect::None,
        );
        missing_problem.problem = None;
        assert_eq!(
            missing_problem.validate_semantics(),
            Err(BrowserResultValidationError::InvalidProblemPresence)
        );

        let mut uncertain = BrowserResult::new(
            OperationKind::BrowserClick,
            BrowserResultStatus::OutcomeUnknown,
            OperationEffect::Unknown,
        );
        uncertain.suggested_next_steps = vec![SuggestedNextStep::Call {
            reason: "Do the click again.".into(),
            operation: Operation::BrowserClick(ClickArguments {
                tab: None,
                target: OperationTarget::parse("r_example").unwrap(),
                button: ClickButton::Left,
                clicks: 1,
                modifiers: Vec::new(),
            }),
        }];
        assert_eq!(
            uncertain.validate_semantics(),
            Err(BrowserResultValidationError::UnsafeSuggestedNextStep)
        );

        uncertain.suggested_next_steps = vec![SuggestedNextStep::Call {
            reason: "Inspect current state without replaying the click.".into(),
            operation: Operation::BrowserInspectPage(InspectPageArguments::default()),
        }];
        assert_eq!(uncertain.validate_semantics(), Ok(()));
    }

    #[test]
    fn provenance_cannot_mark_service_facts_or_handles_untrusted() {
        for pointer in [
            "/schema",
            "/operation",
            "/intent",
            "/status",
            "/effect",
            "/repeat",
            "/problem",
            "/suggested_next_steps",
            "/workspace",
            "/tab/id",
            "/tabs/0/id",
            "/parts/0/type",
            "/parts/0/mime_type",
        ] {
            assert!(PageProvenance::new(vec![pointer.into()], None, None, None).is_err());
        }

        assert!(serde_json::from_value::<PageProvenance>(serde_json::json!({
            "untrusted_fields": ["/status"]
        }))
        .is_err());
    }

    #[test]
    fn provenance_frame_origin_is_bounded_and_validated_on_decode() {
        let provenance = PageProvenance::new(
            vec!["/result".into()],
            Some("https://example.com".into()),
            Some("00112233445566778899aabbccddeeff".into()),
            Some("https://frame.example".into()),
        )
        .expect("bounded frame origin");
        let wire = serde_json::to_value(&provenance).expect("serialize provenance");
        assert_eq!(wire["frame_origin"], "https://frame.example");
        assert_eq!(
            serde_json::from_value::<PageProvenance>(wire).expect("deserialize provenance"),
            provenance
        );

        for frame_origin in [
            String::new(),
            "https://frame.example\nforged".into(),
            "x".repeat(MAX_PAGE_ORIGIN_BYTES + 1),
        ] {
            assert_eq!(
                PageProvenance::new(vec!["/result".into()], None, None, Some(frame_origin)),
                Err(PageProvenanceError::InvalidFrameOrigin)
            );
        }

        assert!(serde_json::from_value::<PageProvenance>(serde_json::json!({
            "untrusted_fields": ["/result"],
            "frame_origin": "bad\norigin"
        }))
        .is_err());
    }
}
