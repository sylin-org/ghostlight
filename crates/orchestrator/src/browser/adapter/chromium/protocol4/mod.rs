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

pub mod assets;
pub mod connection;
#[cfg(test)]
pub(crate) mod fake;
pub mod geometry;
#[cfg(test)]
mod golden;
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
use self::locator::bind_lookup;
use self::scope::Scope;
use self::translate::observe::{self, Subjects};
use self::translate::records::{self, PageExport};
use self::translate::{
    capture, forms, keyboard, pointer, points, present, reach, read, script, tabs,
};
use crate::browser::adapter::{BrowserCommand, BrowserOutcome, RecordingDestination};
use crate::browser::BrowserError;

/// The failure of document-scoped work inside document-scoped work, or of a document
/// description inside a scope.
const NESTED_DOCUMENTS: &str = "nested document execution is invalid";

/// Carry one unit of browser work over a protocol-4 connection.
///
/// Work inside a document scope runs with that scope admitted, and answers with what it read
/// alongside what the scope reached. A command that names one locator tells the [`Link`], so a
/// document change the browser reports is that locator gone stale, and a stale locator's lookup
/// is bound to that locator's document.
///
/// # Errors
///
/// Returns the failure of the first primitive that failed, as the [`Link`] maps it.
pub fn run(command: BrowserCommand, l: &mut Link<'_>) -> Result<BrowserOutcome, BrowserError> {
    let subject = command.sole_locator().map(str::to_owned);
    if subject.is_some() {
        l.names_one_locator();
    }
    carry(command, l).map_err(|error| bind_lookup(error, subject.as_deref()))
}

/// Carry one unit of browser work, scoped or not.
fn carry(command: BrowserCommand, l: &mut Link<'_>) -> Result<BrowserOutcome, BrowserError> {
    match command {
        BrowserCommand::InDocuments { scope, primitive } => {
            if matches!(
                *primitive,
                BrowserCommand::InDocuments { .. } | BrowserCommand::DescribeDocuments { .. }
            ) {
                return Err(nested());
            }
            let mut scope = Scope::admit(&scope)?;
            let result = translate(*primitive, l, Some(&mut scope))?;
            Ok(BrowserOutcome::InDocuments {
                observation: scope.observation(&result),
                result: Box::new(result),
            })
        }
        other => translate(other, l, None),
    }
}

/// The failure of document-scoped work that nests.
fn nested() -> BrowserError {
    BrowserError::Protocol(NESTED_DOCUMENTS.into())
}

/// Carry one unit of work. A content command runs only inside the scope the authority admitted.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per unit of work keeps the whole vocabulary in one exhaustive match"
)]
fn translate(
    command: BrowserCommand,
    l: &mut Link<'_>,
    scope: Option<&mut Scope>,
) -> Result<BrowserOutcome, BrowserError> {
    match command {
        BrowserCommand::InDocuments { .. } => Err(nested()),
        BrowserCommand::DescribeDocuments { .. } if scope.is_some() => Err(nested()),
        BrowserCommand::DescribeDocuments {
            tab_id,
            locators,
            points,
            focused,
            settle_ms,
        } => observe::describe_documents(
            l,
            tab_id,
            Subjects {
                locators: &locators,
                points: &points,
                focused,
            },
            settle_ms,
        ),
        BrowserCommand::ListTabs => tabs::list_tabs(l),
        BrowserCommand::FocusTab { tab_id } => tabs::focus_tab(l, tab_id),
        BrowserCommand::OpenTab {
            url,
            group_title,
            focus_window,
        } => tabs::open_tab(l, url, group_title, focus_window),
        BrowserCommand::AdoptTab {
            tab_id,
            url,
            group_title,
            focus_window,
        } => tabs::adopt_tab(l, tab_id, url, group_title, focus_window),
        BrowserCommand::Navigate { tab_id, url } => {
            tabs::navigate(l, tab_id, wire::NavigationTarget::Url { url }, false)
        }
        BrowserCommand::TraverseHistory { tab_id, direction } => {
            tabs::traverse_history(l, tab_id, direction)
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
        BrowserCommand::ReadText {
            tab_id,
            locator,
            max_chars,
        } => read::read_text(l, reach(scope)?, tab_id, locator.as_deref(), max_chars),
        BrowserCommand::Inspect {
            tab_id,
            kind,
            max_items,
        } => read::inspect(l, reach(scope)?, tab_id, kind, max_items),
        BrowserCommand::ReadDocument {
            tab_id,
            mode,
            max_chars,
        } => read::read_document(l, reach(scope)?, tab_id, mode, max_chars),
        BrowserCommand::InspectTree {
            tab_id,
            locator,
            max_depth,
        } => read::inspect_tree(l, reach(scope)?, tab_id, locator.as_deref(), max_depth),
        BrowserCommand::Find {
            tab_id,
            text,
            kind,
            max_results,
        } => read::find(l, reach(scope)?, tab_id, &text, kind, max_results),
        BrowserCommand::DescribeTargets { tab_id, locators } => {
            read::describe_targets(l, reach(scope)?, tab_id, &locators)
        }
        BrowserCommand::QuerySemantic(selector) => {
            read::query_semantic(l, reach(scope)?, &selector)
        }
        BrowserCommand::DescribeFocused { tab_id } => {
            read::describe_focused(l, reach(scope)?, tab_id)
        }
        BrowserCommand::Observe(wait) => observe::observe(l, reach(scope)?, &wait),
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
        BrowserCommand::ReadDiagnostics(read) => records::read_diagnostics(l, reach(scope)?, read),
        BrowserCommand::ClearDiagnostics { tab_ids } => records::clear_diagnostics(l, tab_ids),
        BrowserCommand::StartRecording { tab_id } => records::start(l, reach(scope)?, tab_id),
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
        BrowserCommand::ExportRecording {
            recording_id,
            destination:
                RecordingDestination::Target {
                    tab_id,
                    locator,
                    file_name,
                },
            max_output_bytes,
        } => records::export_to_page(
            l,
            reach(scope)?,
            recording_id,
            PageExport {
                tab_id,
                locator: &locator,
                file_name: &file_name,
            },
            max_output_bytes,
        ),
        BrowserCommand::DiscardRecording { recording_id } => records::discard(l, recording_id),
        BrowserCommand::Present { signal } => present::present(l, signal),
        BrowserCommand::Screenshot {
            tab_id,
            locator,
            full_page,
            settle_ms,
        } => capture::screenshot(l, tab_id, locator.as_deref(), full_page, settle_ms),
        BrowserCommand::ScreenshotRegion {
            tab_id,
            region,
            expected_viewport,
            settle_ms,
        } => capture::screenshot_region(l, tab_id, &region, &expected_viewport, settle_ms),
        BrowserCommand::Activate(click) => pointer::activate(l, reach(scope)?, &click),
        BrowserCommand::Hover { tab_id, locator } => {
            pointer::hover(l, reach(scope)?, tab_id, &locator)
        }
        BrowserCommand::Scroll(request) => pointer::scroll(l, reach(scope)?, &request),
        BrowserCommand::TypeText(request) => keyboard::type_text(l, reach(scope)?, &request),
        BrowserCommand::TypeFocused(request) => keyboard::type_focused(l, reach(scope)?, &request),
        BrowserCommand::PressKey(request) => keyboard::press_key(l, reach(scope)?, &request),
        BrowserCommand::UploadFiles {
            tab_id,
            locator,
            files,
        } => keyboard::upload_files(l, reach(scope)?, tab_id, &locator, &files),
        BrowserCommand::Fill(fill) => forms::fill(l, reach(scope)?, &fill),
        BrowserCommand::EvaluateScript {
            tab_id,
            script,
            max_result_chars,
        } => script::evaluate(l, reach(scope)?, tab_id, &script, max_result_chars),
        BrowserCommand::ActivatePoint(click) => points::activate(l, reach(scope)?, &click),
        BrowserCommand::WheelAt(wheel) => points::wheel(l, reach(scope)?, &wheel),
        BrowserCommand::HoverPoint(view) => points::hover(l, reach(scope)?, &view),
        BrowserCommand::Drag(drag) => pointer::drag(l, reach(scope)?, &drag),
        BrowserCommand::DragPoints(drag) => points::drag(l, reach(scope)?, &drag),
        BrowserCommand::DropImageAt(drop) => points::drop_file(l, reach(scope)?, &drop),
    }
}
