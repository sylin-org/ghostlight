// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Browser-input preparation for Ghostlight operations.
//!
//! This module performs one bounded job: resolve service-owned tab handles and translate typed
//! operation fields into the policy-free input objects consumed by browser mechanisms. It does
//! not create a second operation identity and owns no validation, governance, scheduling, or
//! result semantics.

use crate::hub::workspace::WorkspaceRegistry;
use ghostlight_transport::operation::{
    ClickButton, DialogResolution, InspectionDetail, KeyModifier, Operation, OperationTarget,
    ScrollAmount, ScrollDirection, TabHandle,
};
use ghostlight_transport::workspace_id::WorkspaceId;
use serde_json::{json, Value};

/// An operation could not be bound to its controlled browser resource.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PreparationError {
    #[error("this operation requires a live workspace")]
    MissingWorkspace,
    #[error("unknown tab")]
    UnknownTab,
    #[error("no controlled tab is available for this call")]
    NoCurrentTab,
}

/// Prepare one Ghostlight operation for its policy-free browser handler.
pub(crate) fn prepare(
    workspaces: &WorkspaceRegistry,
    workspace: Option<&WorkspaceId>,
    operation: &Operation,
) -> Result<Value, PreparationError> {
    use Operation as O;

    let input = match operation {
        O::BrowserGetStatus(_) => json!({}),
        O::BrowserOpenTab(arguments) => arguments.url.as_ref().map_or_else(
            || json!({}),
            |url| json!({"url":url,"readiness":navigation_defaults()}),
        ),
        O::BrowserListTabs(_) => json!({"create_if_empty":false}),
        O::BrowserFocusTab(arguments) | O::BrowserCloseTab(arguments) => {
            json!({"tab":resolve_exact_tab(workspaces, workspace, &arguments.tab)?})
        }
        O::BrowserNavigate(arguments) => {
            let tab = match arguments.tab.as_ref() {
                Some(tab) => Some(resolve_exact_tab(workspaces, workspace, tab)?),
                None => workspace.and_then(|workspace| workspaces.current_tab(workspace)),
            };
            match tab {
                Some(tab) => {
                    json!({"tab":tab,"url":arguments.url,"readiness":navigation_defaults()})
                }
                None => json!({"url":arguments.url,"readiness":navigation_defaults()}),
            }
        }
        O::BrowserGoBack(arguments)
        | O::BrowserGoForward(arguments)
        | O::BrowserReloadPage(arguments) => json!({
            "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
            "readiness":navigation_defaults()
        }),
        O::BrowserInspectPage(arguments) => {
            let mut input = json!({
                "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
                "filter":match arguments.include { InspectionDetail::Interactive => "interactive", InspectionDetail::All => "all" },
                "canonical_targets":true
            });
            if let Some(query) = &arguments.query {
                input["query"] = json!(query);
            }
            if let Some(target) = &arguments.target {
                input["target"] = mechanism_target(target);
            }
            input
        }
        O::BrowserReadPage(arguments) => {
            let mut input = json!({
                "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
                "max_chars":arguments.max_chars
            });
            if let Some(target) = &arguments.target {
                input["target"] = mechanism_target(target);
            }
            input
        }
        O::BrowserTakeScreenshot(arguments) => {
            let mut input = json!({"tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?});
            if let Some(target) = &arguments.target {
                input["target"] = mechanism_target(target);
            }
            input
        }
        O::BrowserClick(arguments) => {
            let mut input = json!({
                "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
                "target":mechanism_target(&arguments.target),
                "button":match arguments.button { ClickButton::Left => "left", ClickButton::Right => "right", ClickButton::Middle => "middle" },
                "clicks":arguments.clicks
            });
            insert_modifiers(&mut input, &arguments.modifiers);
            input
        }
        O::BrowserHover(arguments) | O::BrowserScrollToTarget(arguments) => json!({
            "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
            "target":mechanism_target(&arguments.target)
        }),
        O::BrowserScrollPage(arguments) => json!({
            "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
            "direction":match arguments.direction { ScrollDirection::Up => "up", ScrollDirection::Down => "down" },
            "amount":match arguments.amount { ScrollAmount::Small => 320, ScrollAmount::Page => 720 }
        }),
        O::BrowserPressKey(arguments) => {
            let mut input = json!({
                "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
                "target":mechanism_target(&arguments.target),
                "key":arguments.key.as_str()
            });
            insert_modifiers(&mut input, &arguments.modifiers);
            input
        }
        O::BrowserPressEscape(arguments) => json!({
            "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
            "key":"Escape","repeat":1
        }),
        O::BrowserDrag(arguments) => json!({
            "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
            "from":mechanism_target(&arguments.from),"to":mechanism_target(&arguments.to)
        }),
        O::BrowserFillForm(arguments) => {
            let fields = arguments
                .fields
                .iter()
                .map(|field| json!({"target":mechanism_target(&field.field),"value":field.value}))
                .collect::<Vec<_>>();
            let mut input = json!({
                "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
                "fields":fields,"partial":false,"reject_sensitive":true
            });
            if let Some(target) = &arguments.submit_target {
                input["submit_target"] = mechanism_target(target);
            }
            input
        }
        O::BrowserWaitFor(arguments) => json!({
            "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
            "text":arguments.condition,"state":arguments.state.as_str(),"timeout_ms":arguments.timeout_ms,
            "min_ms":0,"settle":true
        }),
        O::BrowserRunSequence(_) => json!({}),
        O::BrowserGetDialog(arguments) => {
            json!({"tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?})
        }
        O::BrowserHandleDialog(arguments) => {
            let mut input = json!({
                "tab":resolve_current_or_exact_tab(workspaces, workspace, arguments.tab.as_ref())?,
                "action":match arguments.action { DialogResolution::Accept => "accept", DialogResolution::Dismiss => "dismiss", DialogResolution::Respond => "respond" },
                "require_resolution":true
            });
            if let Some(text) = &arguments.text {
                input["text"] = json!(text);
            }
            input
        }
    };
    Ok(input)
}

fn navigation_defaults() -> Value {
    json!({"settle":true,"timeout_ms":10000,"min_ms":0})
}

fn mechanism_target(target: &OperationTarget) -> Value {
    if let Some(reference) = target.as_str().strip_prefix("r_") {
        json!({"ref":format!("ref_{reference}")})
    } else if target.as_str().starts_with("ref_") {
        json!({"ref":target.as_str()})
    } else {
        json!({"query":target.as_str()})
    }
}

fn insert_modifiers(value: &mut Value, modifiers: &[KeyModifier]) {
    if !modifiers.is_empty() {
        value["modifiers"] = json!(modifiers
            .iter()
            .map(|modifier| modifier.as_str())
            .collect::<Vec<_>>()
            .join("+"));
    }
}

fn resolve_exact_tab(
    workspaces: &WorkspaceRegistry,
    workspace: Option<&WorkspaceId>,
    handle: &TabHandle,
) -> Result<i64, PreparationError> {
    let workspace = workspace.ok_or(PreparationError::MissingWorkspace)?;
    workspaces
        .resolve_tab(workspace, handle)
        .ok_or(PreparationError::UnknownTab)
}

fn resolve_current_or_exact_tab(
    workspaces: &WorkspaceRegistry,
    workspace: Option<&WorkspaceId>,
    handle: Option<&TabHandle>,
) -> Result<i64, PreparationError> {
    if let Some(handle) = handle {
        return resolve_exact_tab(workspaces, workspace, handle);
    }
    let workspace = workspace.ok_or(PreparationError::MissingWorkspace)?;
    workspaces
        .current_tab(workspace)
        .ok_or(PreparationError::NoCurrentTab)
}
