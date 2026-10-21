//! The tools Ghostlight advertises: one closed table of each tool's name, title, settlement and
//! default call time, in catalog order. The catalog, the decoder, settlement and the operation
//! names read it, so "is this a tool?" has one answer.

use super::bounds::{DEFAULT_FILL_TIMEOUT_MS, DEFAULT_RECORD_TIMEOUT_MS, DEFAULT_TIMEOUT_MS};

/// One advertised tool.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tool {
    Tabs,
    Navigate,
    History,
    Window,
    Read,
    Inspect,
    Find,
    Screenshot,
    Click,
    Scroll,
    Hover,
    FillForm,
    TypeText,
    PressKey,
    Drag,
    Wait,
    Dialog,
    Upload,
    Execute,
    Flow,
    Record,
    Diagnose,
    PolicyExplain,
}

impl Tool {
    /// Every tool, in the order the catalog lists them.
    pub const ALL: [Self; 23] = [
        Self::Tabs,
        Self::Navigate,
        Self::History,
        Self::Window,
        Self::Read,
        Self::Inspect,
        Self::Find,
        Self::Screenshot,
        Self::Click,
        Self::Scroll,
        Self::Hover,
        Self::FillForm,
        Self::TypeText,
        Self::PressKey,
        Self::Drag,
        Self::Wait,
        Self::Dialog,
        Self::Upload,
        Self::Execute,
        Self::Flow,
        Self::Record,
        Self::Diagnose,
        Self::PolicyExplain,
    ];

    /// The tool's catalog name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tabs => "browser_tabs",
            Self::Navigate => "browser_navigate",
            Self::History => "browser_history",
            Self::Window => "browser_window",
            Self::Read => "browser_read",
            Self::Inspect => "browser_inspect",
            Self::Find => "browser_find",
            Self::Screenshot => "browser_screenshot",
            Self::Click => "browser_click",
            Self::Scroll => "browser_scroll",
            Self::Hover => "browser_hover",
            Self::FillForm => "browser_fill_form",
            Self::TypeText => "browser_type_text",
            Self::PressKey => "browser_press_key",
            Self::Drag => "browser_drag",
            Self::Wait => "browser_wait",
            Self::Dialog => "browser_dialog",
            Self::Upload => "browser_upload",
            Self::Execute => "browser_execute",
            Self::Flow => "browser_flow",
            Self::Record => "browser_record",
            Self::Diagnose => "browser_diagnose",
            Self::PolicyExplain => "policy_explain",
        }
    }

    /// The tool's human title, the catalog's `annotations.title`.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Tabs => "Browser tabs",
            Self::Navigate => "Navigate",
            Self::History => "Browser history",
            Self::Window => "Browser window",
            Self::Read => "Read page",
            Self::Inspect => "Inspect page",
            Self::Find => "Find on page",
            Self::Screenshot => "Take screenshot",
            Self::Click => "Click",
            Self::Scroll => "Scroll",
            Self::Hover => "Hover",
            Self::FillForm => "Fill form",
            Self::TypeText => "Type text",
            Self::PressKey => "Press key",
            Self::Drag => "Drag",
            Self::Wait => "Wait",
            Self::Dialog => "Browser dialog",
            Self::Upload => "Upload files",
            Self::Execute => "Execute JavaScript",
            Self::Flow => "Run flow",
            Self::Record => "Record browser",
            Self::Diagnose => "Diagnose page",
            Self::PolicyExplain => "Explain policy",
        }
    }

    /// Whether the tool is an ordinary page tool that settles the page first, and so takes
    /// `visual_settle` (ADR-0195). Explicit waits own their conditions.
    #[must_use]
    pub const fn settles(self) -> bool {
        matches!(
            self,
            Self::Read
                | Self::Inspect
                | Self::Find
                | Self::Screenshot
                | Self::Click
                | Self::Scroll
                | Self::Hover
                | Self::FillForm
                | Self::TypeText
                | Self::PressKey
                | Self::Drag
                | Self::Upload
                | Self::Execute
        )
    }

    /// The call time the tool gets when the caller names no `timeout_ms`.
    #[must_use]
    pub const fn default_timeout_ms(self) -> u64 {
        match self {
            Self::FillForm => DEFAULT_FILL_TIMEOUT_MS,
            Self::Record => DEFAULT_RECORD_TIMEOUT_MS,
            _ => DEFAULT_TIMEOUT_MS,
        }
    }

    /// The tool a catalog name names, if any.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::Tool;

    #[test]
    fn every_tool_round_trips_through_its_name() {
        for tool in Tool::ALL {
            assert_eq!(Tool::named(tool.name()), Some(tool));
        }
        assert_eq!(Tool::named("browser_sequence"), None);
    }
}
