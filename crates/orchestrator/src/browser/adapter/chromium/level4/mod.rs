//! Chromium adapter protocol 4: the service's translation of browser work into frozen
//! primitives.
//!
//! The extension runs a small closed set of primitives and reports facts. The service decides
//! what they mean. This module is the service's side of that contract: the wire vocabulary, the
//! connection that carries it, the world messages its page assets understand, the frame
//! trees, locators and document scope that keep every call inside the documents the authority
//! admitted, and the pure helpers that turn names into input steps, fold per-document results
//! into one answer and do the coordinate arithmetic between documents. [`run`] is where one
//! unit of browser work becomes the primitives that carry it.

pub mod connection;
#[cfg(test)]
mod fake;
pub mod geometry;
pub mod keys;
pub mod link;
pub mod locator;
pub mod merge;
pub mod scope;
pub mod translate;
pub mod tree;
pub mod wire;
pub mod world;

use self::link::Link;
use self::translate::{present, records, tabs, untranslated};
use crate::browser::adapter::{BrowserCommand, BrowserOutcome, RecordingDestination};
use crate::browser::BrowserError;

/// The failure of a page-runtime install sent to a protocol-4 connection, which receives its
/// page assets in `hello_accepted`.
const INSTALL_IS_LEVEL_THREE: &str = "page runtime installs are level 3 only";

/// Carry one unit of browser work over a protocol-4 connection.
///
/// Work this service does not translate yet answers that the extension lacks the capability,
/// so no unit of work is ever sent half-translated.
///
/// # Errors
///
/// Returns the failure of the first primitive that failed, as the [`Link`] maps it.
pub fn run(command: BrowserCommand, l: &mut Link<'_>) -> Result<BrowserOutcome, BrowserError> {
    match command {
        BrowserCommand::InstallPageRuntime { .. } => {
            Err(BrowserError::Protocol(INSTALL_IS_LEVEL_THREE.into()))
        }
        BrowserCommand::ListTabs => tabs::list_tabs(l),
        BrowserCommand::FocusTab { tab_id } => tabs::focus_tab(l, tab_id),
        BrowserCommand::OpenTab {
            url,
            group_title,
            focus_window,
            ..
        } => tabs::open_tab(l, url, group_title, focus_window),
        BrowserCommand::Navigate { tab_id, url } => {
            tabs::navigate(l, tab_id, wire::NavigationTarget::Url { url }, false)
        }
        BrowserCommand::TraverseHistory { tab_id, direction } => {
            tabs::traverse_history(l, tab_id, &direction)
        }
        BrowserCommand::Reload {
            tab_id,
            bypass_cache,
        } => tabs::navigate(
            l,
            tab_id,
            wire::NavigationTarget::Reload { bypass_cache },
            false,
        ),
        BrowserCommand::CloseTab { tab_id, released } => tabs::close_tab(l, tab_id, released),
        BrowserCommand::NavigateDiscardingBeforeUnload { tab_id, url } => {
            tabs::navigate(l, tab_id, wire::NavigationTarget::Url { url }, true)
        }
        BrowserCommand::SetZoom { tab_id, zoom } => tabs::set_zoom(l, tab_id, zoom),
        BrowserCommand::ResizeWindow {
            tab_id,
            width,
            height,
        } => tabs::resize_window(l, tab_id, width, height),
        BrowserCommand::InspectDialog { tab_id } => tabs::inspect_dialog(l, tab_id),
        BrowserCommand::HandleDialog {
            tab_id,
            accept,
            text,
        } => tabs::handle_dialog(l, tab_id, accept, text),
        BrowserCommand::ClearDiagnostics { tab_ids } => records::clear_diagnostics(l, tab_ids),
        BrowserCommand::StatusRecording { recording_id } => records::status(l, recording_id),
        BrowserCommand::StopRecording { recording_id } => records::stop(l, recording_id),
        BrowserCommand::ExportRecording {
            recording_id,
            destination: RecordingDestination::Download { file_name },
            max_output_bytes,
        } => records::export(
            l,
            recording_id,
            wire::RecordingTarget::Download { file_name },
            max_output_bytes,
        ),
        BrowserCommand::ExportRecording {
            recording_id,
            destination: RecordingDestination::Client,
            max_output_bytes,
        } => records::export(
            l,
            recording_id,
            wire::RecordingTarget::Client,
            max_output_bytes,
        ),
        BrowserCommand::DiscardRecording { recording_id } => records::discard(l, recording_id),
        BrowserCommand::Cancel { correlation } => Ok(present::cancel(l, &correlation)),
        BrowserCommand::Present { signal } => present::present(l, signal),
        BrowserCommand::DescribeDocuments { .. }
        | BrowserCommand::InDocuments { .. }
        | BrowserCommand::ReadText { .. }
        | BrowserCommand::Inspect { .. }
        | BrowserCommand::ReadDocument { .. }
        | BrowserCommand::InspectTree { .. }
        | BrowserCommand::Find { .. }
        | BrowserCommand::Screenshot { .. }
        | BrowserCommand::ScreenshotRegion { .. }
        | BrowserCommand::DescribeTargets { .. }
        | BrowserCommand::QuerySemantic { .. }
        | BrowserCommand::Activate { .. }
        | BrowserCommand::ActivatePoint { .. }
        | BrowserCommand::ActivateModified { .. }
        | BrowserCommand::ActivatePointModified { .. }
        | BrowserCommand::WheelAt { .. }
        | BrowserCommand::Scroll { .. }
        | BrowserCommand::Hover { .. }
        | BrowserCommand::HoverPoint { .. }
        | BrowserCommand::Fill { .. }
        | BrowserCommand::TypeText { .. }
        | BrowserCommand::DescribeFocused { .. }
        | BrowserCommand::TypeFocused { .. }
        | BrowserCommand::PressKey { .. }
        | BrowserCommand::Drag { .. }
        | BrowserCommand::DragPoints { .. }
        | BrowserCommand::UploadFiles { .. }
        | BrowserCommand::DropImageAt { .. }
        | BrowserCommand::EvaluateScript { .. }
        | BrowserCommand::Observe { .. }
        | BrowserCommand::ReadDiagnostics { .. }
        | BrowserCommand::StartRecording { .. }
        | BrowserCommand::ExportRecording {
            destination: RecordingDestination::Target { .. },
            ..
        } => Err(untranslated(&command)),
    }
}
