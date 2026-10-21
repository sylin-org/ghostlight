//! Tabs, windows, navigation and dialogs: work the browser does natively, one primitive each.

use super::super::link::{FocusTab, Link};
use super::super::wire;
use crate::browser::adapter::BrowserOutcome;
use crate::browser::BrowserError;
use crate::install::icons::sha256_hex;

/// The longest wait for an opened or navigated tab to become ready, in milliseconds.
pub const READY_TIMEOUT_MS: u64 = 8_000;
/// The prefix of every tab group key.
const GROUP_KEY_PREFIX: &str = "g_";
/// How many hex characters of the group title's digest a group key keeps.
const GROUP_KEY_HEX_CHARS: usize = 16;
/// The failure for a history direction that is neither back nor forward.
const UNKNOWN_DIRECTION: &str = "browser history direction must be back or forward";

/// The history directions work may name.
pub mod direction {
    /// One entry back.
    pub const BACK: &str = "back";
    /// One entry forward.
    pub const FORWARD: &str = "forward";
}

/// List every tab.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn list_tabs(l: &mut Link<'_>) -> Result<BrowserOutcome, BrowserError> {
    let listed = l.send(wire::ListTabs { tab_ids: None })?;
    Ok(BrowserOutcome::Tabs {
        tabs: listed.tabs.into_iter().map(Into::into).collect(),
    })
}

/// Bring a tab and its window into view.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn focus_tab(l: &mut Link<'_>, tab_id: u64) -> Result<BrowserOutcome, BrowserError> {
    let focused = l.send(FocusTab(tab_id))?;
    Ok(BrowserOutcome::TabFocused {
        tab_id: focused.tab_id,
        active: focused.active,
        window_focused: focused.window_focused,
    })
}

/// Open one URL in the workspace's tab group.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn open_tab(
    l: &mut Link<'_>,
    url: String,
    group_title: String,
    focus_window: bool,
) -> Result<BrowserOutcome, BrowserError> {
    let opened = l.send(wire::OpenTab {
        url: Some(url),
        group_key: group_key(&group_title),
        group_title,
        adopt_tab_id: None,
        focus_window,
        ready_timeout_ms: ready_timeout_ms(l),
    })?;
    Ok(BrowserOutcome::TabOpened {
        tab: opened.tab.into(),
        committed_urls: l.committed(),
    })
}

/// Put an existing tab in the workspace's tab group and load one URL in it.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn adopt_tab(
    l: &mut Link<'_>,
    tab_id: u64,
    url: String,
    group_title: String,
    focus_window: bool,
) -> Result<BrowserOutcome, BrowserError> {
    let adopted = l.send(wire::OpenTab {
        url: Some(url),
        group_key: group_key(&group_title),
        group_title,
        adopt_tab_id: Some(tab_id),
        focus_window,
        ready_timeout_ms: ready_timeout_ms(l),
    })?;
    Ok(BrowserOutcome::TabOpened {
        tab: adopted.tab.into(),
        committed_urls: l.committed(),
    })
}

/// Navigate a tab: to a URL, through history, or by reloading.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn navigate(
    l: &mut Link<'_>,
    tab_id: u64,
    to: wire::NavigationTarget,
    discard_beforeunload: bool,
) -> Result<BrowserOutcome, BrowserError> {
    let navigated = l.send(wire::Navigate {
        tab_id,
        to,
        discard_beforeunload,
        ready_timeout_ms: ready_timeout_ms(l),
    })?;
    Ok(BrowserOutcome::Navigated {
        tab: navigated.tab.into(),
        committed_urls: l.committed(),
    })
}

/// Move one entry back or forward in a tab's history.
///
/// # Errors
///
/// Returns a plain failure, with nothing sent, for any other direction, and otherwise the
/// primitive's failure.
pub fn traverse_history(
    l: &mut Link<'_>,
    tab_id: u64,
    direction: &str,
) -> Result<BrowserOutcome, BrowserError> {
    let to = match direction {
        direction::BACK => wire::NavigationTarget::Back,
        direction::FORWARD => wire::NavigationTarget::Forward,
        _ => return Err(BrowserError::primitive(UNKNOWN_DIRECTION)),
    };
    navigate(l, tab_id, to, false)
}

/// Close a tab; `released` marks a close that releases a tab from an ended workspace.
///
/// # Errors
///
/// Returns the primitive's failure; the preserve-tabs veto is a local interlock.
pub fn close_tab(
    l: &mut Link<'_>,
    tab_id: u64,
    released: bool,
) -> Result<BrowserOutcome, BrowserError> {
    let closed = l.send(wire::CloseTab { tab_id, released })?;
    Ok(BrowserOutcome::TabClosed {
        tab_id: closed.tab_id,
    })
}

/// Set a tab's zoom.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn set_zoom(l: &mut Link<'_>, tab_id: u64, zoom: f64) -> Result<BrowserOutcome, BrowserError> {
    let zoomed = l.send(wire::SetZoom { tab_id, zoom })?;
    Ok(BrowserOutcome::Zoomed {
        tab_id: zoomed.tab_id,
        zoom: zoomed.zoom,
    })
}

/// Resize the window that holds a tab.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn resize_window(
    l: &mut Link<'_>,
    tab_id: u64,
    width: u32,
    height: u32,
) -> Result<BrowserOutcome, BrowserError> {
    let resized = l.send(wire::ResizeWindow {
        tab_id,
        width,
        height,
    })?;
    Ok(BrowserOutcome::WindowResized {
        tab_id: resized.tab_id,
        width: resized.width,
        height: resized.height,
        affected_tab_ids: resized.affected_tab_ids,
    })
}

/// Report whether a JavaScript dialog is showing, with its bounded message and, for a prompt, the
/// text its field holds.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn inspect_dialog(l: &mut Link<'_>, tab_id: u64) -> Result<BrowserOutcome, BrowserError> {
    let facts = l.send(wire::Dialog {
        tab_id,
        op: wire::DialogOp::Status,
    })?;
    Ok(BrowserOutcome::Dialog {
        tab_id,
        present: facts.present,
        dialog_type: facts.dialog_type,
        message: facts.message,
        default_prompt: facts.default_prompt,
        url: facts.url,
    })
}

/// Accept or dismiss the showing JavaScript dialog.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn handle_dialog(
    l: &mut Link<'_>,
    tab_id: u64,
    accept: bool,
    text: Option<String>,
) -> Result<BrowserOutcome, BrowserError> {
    let facts = l.send(wire::Dialog {
        tab_id,
        op: wire::DialogOp::Handle { accept, text },
    })?;
    if !facts.handled {
        return Ok(BrowserOutcome::DialogAbsent { tab_id });
    }
    Ok(BrowserOutcome::DialogHandled {
        tab_id,
        dialog_type: facts.dialog_type,
        accepted: facts.accepted,
        url: facts.url,
    })
}

/// The stable key of the tab group a title names: `g_` and the first 16 hex characters of the
/// title's SHA-256.
#[must_use]
pub fn group_key(group_title: &str) -> String {
    let mut key = format!("{GROUP_KEY_PREFIX}{}", sha256_hex(group_title.as_bytes()));
    key.truncate(GROUP_KEY_PREFIX.len() + GROUP_KEY_HEX_CHARS);
    key
}

/// The ready wait a tab primitive may use, within what the deadline leaves.
fn ready_timeout_ms(l: &Link<'_>) -> u64 {
    READY_TIMEOUT_MS.min(l.budget_ms())
}
