// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Bounded compatibility for replaying historical audit identities.
//!
//! Current audit records carry canonical operation-family and intent strings. Records written
//! before ADR-0101 instead carry the then-current model-facing tool and action names. Policy
//! simulation resolves the canonical vocabulary first and consults this closed alias table only
//! for those historical rows. The aliases are never valid operation input and never participate
//! in live routing, scheduling, governance, or browser dispatch.

use crate::governance::ports::Capability;
use ghostlight_transport::operation::{IntentId, OperationId, OperationKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HistoricalAlias {
    tool: &'static str,
    action: Option<&'static str>,
    key: OperationKey,
}

macro_rules! alias {
    ($tool:literal, $action:expr, $id:ident, $intent:ident) => {
        HistoricalAlias {
            tool: $tool,
            action: $action,
            key: OperationKey::new(OperationId::$id, IntentId::$intent),
        }
    };
}

/// The complete pre-ADR-0101 audit identity dictionary.
///
/// This is intentionally not exported. New surface profiles must never grow this historical
/// table; their calls are recorded canonically before reaching the audit sink.
const HISTORICAL_ALIASES: &[HistoricalAlias] = &[
    alias!("tabs_context_mcp", None, BrowserTabs, TabsList),
    alias!("tabs_create_mcp", None, BrowserTabs, TabsNew),
    alias!("navigate", None, BrowserNavigate, NavigateUrl),
    alias!(
        "computer",
        Some("left_click"),
        BrowserInput,
        InputPointerClick
    ),
    alias!(
        "computer",
        Some("right_click"),
        BrowserInput,
        InputPointerRightClick
    ),
    alias!("computer", Some("type"), BrowserInput, InputTypeText),
    alias!(
        "computer",
        Some("screenshot"),
        BrowserScreenshot,
        ScreenshotViewport
    ),
    alias!("computer", Some("wait"), BrowserWait, WaitDelay),
    alias!("computer", Some("scroll"), BrowserInput, InputWheel),
    alias!("computer", Some("key"), BrowserInput, InputPressKey),
    alias!(
        "computer",
        Some("left_click_drag"),
        BrowserInput,
        InputPointerDrag
    ),
    alias!(
        "computer",
        Some("double_click"),
        BrowserInput,
        InputPointerDoubleClick
    ),
    alias!(
        "computer",
        Some("triple_click"),
        BrowserInput,
        InputPointerTripleClick
    ),
    alias!(
        "computer",
        Some("zoom"),
        BrowserScreenshot,
        ScreenshotRegion
    ),
    alias!(
        "computer",
        Some("scroll_to"),
        BrowserInput,
        InputScrollToOffset
    ),
    alias!("computer", Some("hover"), BrowserInput, InputPointerHover),
    alias!("find", None, BrowserFind, FindQuery),
    alias!("form_input", None, BrowserFill, FillField),
    alias!("get_page_text", None, BrowserRead, ReadText),
    alias!("javascript_tool", None, BrowserEvaluate, EvaluateJavascript),
    alias!("read_console_messages", None, BrowserConsole, ConsoleRead),
    alias!("read_network_requests", None, BrowserNetwork, NetworkRead),
    alias!("read_page", None, BrowserSnapshot, SnapshotCapture),
    alias!("resize_window", None, BrowserViewport, ViewportResizeWindow),
    alias!("update_plan", None, WorkflowPlan, PlanUpdate),
    alias!("narrate", None, BrowserPresent, PresentNarrate),
    alias!("wait_for", None, BrowserWait, WaitUntil),
    alias!("script", None, BrowserFlow, FlowExecute),
    alias!("form_fill", None, BrowserFill, FillFields),
    alias!(
        "form_fill",
        Some("submit"),
        BrowserFill,
        FillFieldsAndSubmit
    ),
    alias!("act_on", Some("left_click"), BrowserAct, ActClick),
    alias!("act_on", Some("right_click"), BrowserAct, ActRightClick),
    alias!("act_on", Some("double_click"), BrowserAct, ActDoubleClick),
    alias!("act_on", Some("hover"), BrowserAct, ActHover),
    alias!("act_on", Some("scroll_to"), BrowserAct, ActScrollIntoView),
    alias!("act_on", Some("set_value"), BrowserAct, ActSetValue),
    alias!("dialog", Some("status"), BrowserDialog, DialogStatus),
    alias!("dialog", Some("accept"), BrowserDialog, DialogAccept),
    alias!("dialog", Some("dismiss"), BrowserDialog, DialogDismiss),
    alias!("dialog", Some("respond"), BrowserDialog, DialogRespond),
    alias!("tab_control", Some("focus"), BrowserTabs, TabsFocus),
    alias!(
        "tab_control",
        Some("reload"),
        BrowserNavigate,
        NavigateReload
    ),
    alias!("tab_control", Some("close"), BrowserTabs, TabsClose),
    alias!("file_upload", None, BrowserUpload, UploadClientFiles),
    alias!("browser_batch", None, BrowserFlow, FlowExecute),
    alias!("upload_image", None, BrowserUpload, UploadCapturedArtifact),
    alias!(
        "gif_creator",
        Some("start_recording"),
        BrowserRecord,
        RecordStart
    ),
    alias!(
        "gif_creator",
        Some("stop_recording"),
        BrowserRecord,
        RecordStop
    ),
    alias!("gif_creator", Some("status"), BrowserRecord, RecordStatus),
    alias!("gif_creator", Some("clear"), BrowserRecord, RecordClear),
    alias!("gif_creator", Some("export"), BrowserRecord, RecordExport),
    alias!("explain", None, BrowserContext, ContextDescribe),
];

/// Resolve an audit identity into the canonical operation registry.
///
/// Exact canonical family and intent strings always take precedence. The historical table is
/// consulted only if the row is not a canonical pair.
pub fn operation_key(tool: &str, action: Option<&str>) -> Option<OperationKey> {
    if let (Some(id), Some(intent)) = (OperationId::parse(tool), action.and_then(IntentId::parse)) {
        let key = OperationKey::new(id, intent);
        return crate::operation::registry::descriptor(key).map(|_| key);
    }

    HISTORICAL_ALIASES
        .iter()
        .find(|alias| alias.tool == tool && alias.action == action)
        .map(|alias| alias.key)
}

/// Resolve the canonical capability requirement for a current or historical audit row.
///
/// Historical records never contained normalized arguments, so argument-dependent refinements
/// cannot be reconstructed. This intentionally returns the canonical descriptor's baseline,
/// matching the old directory replay contract.
pub fn requires(tool: &str, action: Option<&str>) -> Option<&'static [Capability]> {
    let key = operation_key(tool, action)?;
    crate::operation::registry::descriptor(key).map(|descriptor| descriptor.requires)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_identity_is_resolved_directly() {
        assert_eq!(
            operation_key("browser.act", Some("act.click")),
            Some(OperationKey::new(
                OperationId::BrowserAct,
                IntentId::ActClick
            ))
        );
        assert_eq!(operation_key("browser.act", Some("tabs.list")), None);
        assert_eq!(operation_key("browser.act", None), None);
    }

    #[test]
    fn all_historical_aliases_point_to_live_canonical_descriptors() {
        assert_eq!(HISTORICAL_ALIASES.len(), 52);
        for alias in HISTORICAL_ALIASES {
            assert!(
                crate::operation::registry::descriptor(alias.key).is_some(),
                "historical alias {:?} / {:?} has no canonical descriptor",
                alias.tool,
                alias.action
            );
            assert_eq!(operation_key(alias.tool, alias.action), Some(alias.key));
        }
    }

    #[test]
    fn unknown_or_incomplete_historical_identity_fails_closed() {
        assert_eq!(operation_key("computer", None), None);
        assert_eq!(operation_key("computer", Some("bogus")), None);
        assert_eq!(operation_key("future_surface_tool", None), None);
    }

    #[test]
    fn requirements_come_only_from_the_canonical_registry() {
        assert_eq!(
            requires("browser.snapshot", Some("snapshot.capture")),
            Some(&[Capability::Read][..])
        );
        assert_eq!(
            requires("read_page", None),
            requires("browser.snapshot", Some("snapshot.capture"))
        );
    }
}
