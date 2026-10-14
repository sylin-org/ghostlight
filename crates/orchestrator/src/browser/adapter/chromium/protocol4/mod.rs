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
use self::scope::Scope;
use self::translate::forms::{self, Fill};
use self::translate::keyboard::{PressKey, TypeFocused, TypeText};
use self::translate::observe::{self, Subjects, Wait};
use self::translate::pointer::{Click, Drag, Scroll};
use self::translate::points::{PointClick, PointDrag, PointDrop, PointWheel, ViewPoint};
use self::translate::records::{self, DiagnosticsRead, PageExport};
use self::translate::{capture, keyboard, pointer, points, present, read, script, tabs};
use crate::browser::adapter::{BrowserCommand, BrowserOutcome, RecordingDestination};
use crate::browser::BrowserError;

/// The failure of document-scoped work inside document-scoped work, or of a document
/// description inside a scope.
const NESTED_DOCUMENTS: &str = "nested document execution is invalid";

/// Carry one unit of browser work over a protocol-4 connection.
///
/// Work inside a document scope runs with that scope admitted, and answers with what it read
/// alongside what the scope reached.
///
/// # Errors
///
/// Returns the failure of the first primitive that failed, as the [`Link`] maps it.
pub fn run(command: BrowserCommand, l: &mut Link<'_>) -> Result<BrowserOutcome, BrowserError> {
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

/// Carry one unit of work, inside the admitted scope when there is one.
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
        BrowserCommand::ReadText {
            tab_id,
            locator,
            max_chars,
        } => read::read_text(l, scope, tab_id, locator.as_deref(), max_chars),
        BrowserCommand::Inspect {
            tab_id,
            kind,
            max_items,
        } => read::inspect(l, scope, tab_id, &kind, max_items),
        BrowserCommand::ReadDocument {
            tab_id,
            mode,
            max_chars,
        } => read::read_document(l, scope, tab_id, &mode, max_chars),
        BrowserCommand::InspectTree {
            tab_id,
            locator,
            max_depth,
        } => read::inspect_tree(l, scope, tab_id, locator.as_deref(), max_depth),
        BrowserCommand::Find {
            tab_id,
            text,
            kind,
            max_results,
        } => read::find(l, scope, tab_id, &text, &kind, max_results),
        BrowserCommand::DescribeTargets { tab_id, locators } => {
            read::describe_targets(l, scope, tab_id, &locators)
        }
        BrowserCommand::QuerySemantic {
            tab_id,
            name,
            role,
            exact,
            form_scope,
        } => read::query_semantic(
            l,
            scope,
            tab_id,
            &read::Semantic {
                name: &name,
                role: role.as_deref(),
                exact,
                form_scope,
            },
        ),
        BrowserCommand::DescribeFocused { tab_id } => read::describe_focused(l, scope, tab_id),
        BrowserCommand::Observe {
            tab_id,
            condition,
            value,
            locator,
            timeout_ms,
        } => observe::observe(
            l,
            scope,
            tab_id,
            Wait {
                condition: &condition,
                value: value.as_deref(),
                locator: locator.as_deref(),
                timeout_ms,
            },
        ),
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
        BrowserCommand::ReadDiagnostics {
            tab_id,
            source,
            detail,
            match_text,
            after,
            limit,
        } => records::read_diagnostics(
            l,
            scope.as_deref(),
            DiagnosticsRead {
                tab_id,
                source,
                detail,
                match_text,
                after,
                limit,
            },
        ),
        BrowserCommand::ClearDiagnostics { tab_ids } => records::clear_diagnostics(l, tab_ids),
        BrowserCommand::StartRecording { tab_id } => records::start(l, scope.as_deref(), tab_id),
        BrowserCommand::StatusRecording => records::status(l),
        BrowserCommand::StopRecording => records::stop(l),
        BrowserCommand::ExportRecording {
            destination: RecordingDestination::Download { file_name },
            max_output_bytes,
        } => records::export(
            l,
            wire::RecordingTarget::Download { file_name },
            max_output_bytes,
        ),
        BrowserCommand::ExportRecording {
            destination: RecordingDestination::Client,
            max_output_bytes,
        } => records::export(l, wire::RecordingTarget::Client, max_output_bytes),
        BrowserCommand::ExportRecording {
            destination:
                RecordingDestination::Target {
                    tab_id,
                    locator,
                    file_name,
                },
            max_output_bytes,
        } => records::export_to_page(
            l,
            scope.as_deref(),
            PageExport {
                tab_id,
                locator: &locator,
                file_name: &file_name,
            },
            max_output_bytes,
        ),
        BrowserCommand::DiscardRecording => records::discard(l),
        BrowserCommand::Present { signal } => present::present(l, signal),
        BrowserCommand::Screenshot {
            tab_id,
            locator,
            full_page,
            visual_settle,
        } => capture::screenshot(l, tab_id, locator.as_deref(), full_page, visual_settle),
        BrowserCommand::ScreenshotRegion {
            tab_id,
            region,
            expected_viewport,
            visual_settle,
        } => capture::screenshot_region(l, tab_id, &region, &expected_viewport, visual_settle),
        BrowserCommand::Activate {
            tab_id,
            locator,
            button,
            click_count,
        } => pointer::activate(
            l,
            scope,
            Click {
                tab_id,
                locator: &locator,
                button: &button,
                click_count,
                modifiers: &[],
            },
        ),
        BrowserCommand::ActivateModified {
            tab_id,
            locator,
            button,
            click_count,
            modifiers,
        } => pointer::activate(
            l,
            scope,
            Click {
                tab_id,
                locator: &locator,
                button: &button,
                click_count,
                modifiers: &modifiers,
            },
        ),
        BrowserCommand::Hover { tab_id, locator } => pointer::hover(l, scope, tab_id, &locator),
        BrowserCommand::Scroll {
            tab_id,
            locator,
            direction,
            amount,
        } => pointer::scroll(
            l,
            scope,
            Scroll {
                tab_id,
                locator: locator.as_deref(),
                direction: direction.as_deref(),
                amount: amount.as_deref(),
            },
        ),
        BrowserCommand::TypeText {
            tab_id,
            locator,
            text,
            clear_first,
            allow_credentials,
        } => keyboard::type_text(
            l,
            scope,
            TypeText {
                tab_id,
                locator: &locator,
                text: &text,
                clear_first,
                allow_credentials,
            },
        ),
        BrowserCommand::TypeFocused {
            tab_id,
            text,
            clear_first,
            allow_credentials,
        } => keyboard::type_focused(
            l,
            scope,
            TypeFocused {
                tab_id,
                text: &text,
                clear_first,
                allow_credentials,
            },
        ),
        BrowserCommand::PressKey {
            tab_id,
            locator,
            key,
            modifiers,
        } => keyboard::press_key(
            l,
            scope,
            PressKey {
                tab_id,
                locator: locator.as_deref(),
                key: &key,
                modifiers: &modifiers,
            },
        ),
        BrowserCommand::UploadFiles {
            tab_id,
            locator,
            files,
        } => keyboard::upload_files(l, scope, tab_id, &locator, &files),
        BrowserCommand::Fill {
            tab_id,
            fields,
            submit_locator,
            timeout_ms,
            allow_credentials,
        } => forms::fill(
            l,
            scope,
            Fill {
                tab_id,
                fields: &fields,
                submit_locator: submit_locator.as_deref(),
                timeout_ms,
                allow_credentials,
            },
        ),
        BrowserCommand::EvaluateScript {
            tab_id,
            script,
            max_result_chars,
        } => script::evaluate(l, scope, tab_id, &script, max_result_chars),
        BrowserCommand::ActivatePoint {
            tab_id,
            point,
            expected_viewport,
            button,
            click_count,
        } => points::activate(
            l,
            scope,
            PointClick {
                at: ViewPoint {
                    tab_id,
                    point,
                    expected: &expected_viewport,
                },
                button: &button,
                click_count,
                modifiers: &[],
            },
        ),
        BrowserCommand::ActivatePointModified {
            tab_id,
            point,
            expected_viewport,
            button,
            click_count,
            modifiers,
        } => points::activate(
            l,
            scope,
            PointClick {
                at: ViewPoint {
                    tab_id,
                    point,
                    expected: &expected_viewport,
                },
                button: &button,
                click_count,
                modifiers: &modifiers,
            },
        ),
        BrowserCommand::WheelAt {
            tab_id,
            point,
            expected_viewport,
            direction,
            ticks,
        } => points::wheel(
            l,
            scope,
            PointWheel {
                at: ViewPoint {
                    tab_id,
                    point,
                    expected: &expected_viewport,
                },
                direction: &direction,
                ticks,
            },
        ),
        BrowserCommand::HoverPoint {
            tab_id,
            point,
            expected_viewport,
        } => points::hover(
            l,
            scope,
            ViewPoint {
                tab_id,
                point,
                expected: &expected_viewport,
            },
        ),
        BrowserCommand::Drag {
            tab_id,
            source_locator,
            destination_locator,
        } => pointer::drag(
            l,
            scope,
            Drag {
                tab_id,
                source: &source_locator,
                destination: &destination_locator,
            },
        ),
        BrowserCommand::DragPoints {
            tab_id,
            start,
            end,
            expected_viewport,
        } => points::drag(
            l,
            scope,
            PointDrag {
                tab_id,
                start,
                end,
                expected: &expected_viewport,
            },
        ),
        BrowserCommand::DropImageAt {
            tab_id,
            point,
            expected_viewport,
            file,
        } => points::drop_file(
            l,
            scope,
            PointDrop {
                at: ViewPoint {
                    tab_id,
                    point,
                    expected: &expected_viewport,
                },
                file: &file,
            },
        ),
    }
}
