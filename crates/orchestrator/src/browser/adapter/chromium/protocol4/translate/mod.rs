//! The translation of each unit of browser work into protocol-4 primitives.
//!
//! Each submodule owns one family of work. A function there sends the primitives its work
//! needs through the [`Link`] and turns their outcomes into the one
//! [`BrowserOutcome`](crate::browser::adapter::BrowserOutcome) the rest of the service reads.
//! This module holds what the families share: the scope every content command needs, the
//! reading of one world call's answer, the sending of input and the reading of how it ended, and
//! the routing of a point to the document it lands in, which happens when input is sent and
//! nowhere else.

pub mod capture;
pub mod forms;
pub mod keyboard;
pub mod observe;
pub mod pointer;
pub mod points;
pub mod present;
pub mod read;
pub mod records;
pub mod script;
pub mod tabs;

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::geometry::{self, RouteStep, MAX_ROUTE_DEPTH};
use super::link::{CallRequest, Link};
use super::scope::{PlannedCall, Scope};
use super::tree::{Described, FrameTree};
use super::wire::{
    CallAnswer, CallFailure, DocumentGuard, FrameRef, Input, InputDone, InputStep, InputStop, Point,
};
use super::world::{self, AssetDetail, AssetErrorCode, EffectClass, Hit};
use crate::browser::adapter::PhysicalActionSubject;
use crate::browser::BrowserError;
use crate::language::document_refusal::DocumentCause;

/// The protocol failure of a content command sent without a document scope.
const UNSCOPED_CONTENT: &str = "content command sent without a document scope";
/// The failure of a world call the extension skipped because an earlier one failed.
const CALL_SKIPPED: &str = "a page call was skipped";
/// The protocol failure of a world call status this service does not know.
const CALL_UNKNOWN: &str = "a page call answered with a status this service does not know";
/// The unknown effect of input that a dialog interrupted.
const DIALOG_DURING_INPUT: &str = "a dialog opened while input was being sent";
/// The unknown effect of input that stopped short of its last step.
const INPUT_INCOMPLETE: &str = "input stopped before its last step";
/// The protocol failure of input whose precondition went unanswered.
const PRECONDITION_UNANSWERED: &str = "input did not answer its precondition";

/// The scope a content command runs in. Every content command reaches the page only through the
/// documents the authority admitted, so one sent without a scope is a malformed request.
///
/// # Errors
///
/// Returns [`BrowserError::Protocol`] when there is no scope, before anything is sent.
pub fn reach(scope: Option<&mut Scope>) -> Result<&mut Scope, BrowserError> {
    scope.ok_or_else(|| BrowserError::Protocol(UNSCOPED_CONTENT.into()))
}

/// The tab's frame tree, from one `frames` primitive, and whether it is the whole tab.
///
/// # Errors
///
/// Returns the primitive's failure, and [`BrowserError::DocumentUnavailable`] for an inventory
/// with no top document.
pub fn frame_tree(l: &mut Link<'_>, tab_id: u64) -> Result<Described, BrowserError> {
    let inventory = l.send(super::link::Frames(tab_id))?;
    FrameTree::describe(&inventory)
}

/// Read one world call's answer as the value its kind returns.
///
/// # Errors
///
/// Returns the asset's own failure as a plain failure, as [`BrowserError::FieldsRejected`]
/// when it named the fields a fill could not take, as [`BrowserError::StaleLocator`] for a
/// locator that names no control, or as [`BrowserError::PageRefused`] for a control the page
/// refused to act on. Returns [`BrowserError::PageChanged`] when the document was gone or
/// replaced or its session ended, [`BrowserError::DialogOpen`] when a JavaScript dialog blocks
/// the page, and [`BrowserError::Protocol`] for a value that does not have the kind's shape. A
/// command that names one handle restates the change as that handle gone stale.
pub fn value<T: DeserializeOwned>(answer: &CallAnswer) -> Result<T, BrowserError> {
    match answer {
        CallAnswer::Returned { .. } => match world::decode_answer(answer) {
            Some(Ok(value)) => serde_json::from_value(value)
                .map_err(|error| BrowserError::Protocol(error.to_string())),
            Some(Err(error)) => Err(match error.detail {
                Some(AssetDetail::Stale { reason, lookup }) => {
                    BrowserError::StaleLocator { reason, lookup }
                }
                Some(AssetDetail::Fields(detail)) => BrowserError::FieldsRejected {
                    problems: detail.problems,
                },
                None => match error.code.page_refusal() {
                    Some(refusal) => BrowserError::PageRefused(refusal),
                    None => BrowserError::primitive(if error.message.is_empty() {
                        super::wire::name_of(&error.code)
                    } else {
                        error.message
                    }),
                },
            }),
            None => Err(BrowserError::Protocol(CALL_UNKNOWN.into())),
        },
        // A session lost under one call is that document's session ending, not the tab's: the
        // document is gone, as much as one that was replaced or whose frame went away.
        CallAnswer::Failed {
            code:
                CallFailure::DocumentChanged | CallFailure::FrameMissing | CallFailure::SessionLost,
            ..
        } => Err(BrowserError::PageChanged),
        CallAnswer::Failed {
            code: CallFailure::DialogOpen,
            ..
        } => Err(BrowserError::DialogOpen),
        CallAnswer::Failed { code, message, .. } => {
            Err(BrowserError::primitive(if message.is_empty() {
                super::wire::name_of(code)
            } else {
                message.clone()
            }))
        }
        CallAnswer::Skipped => Err(BrowserError::primitive(CALL_SKIPPED)),
        CallAnswer::Unknown => Err(BrowserError::Protocol(CALL_UNKNOWN.into())),
    }
}

/// Read one world call's answer as the value its kind returns, restating a failure as an unknown
/// effect once the command has acted.
///
/// # Errors
///
/// Returns what [`value`] returns, settled by the link.
pub fn settled<T: DeserializeOwned>(l: &Link<'_>, answer: &CallAnswer) -> Result<T, BrowserError> {
    value(answer).map_err(|error| l.settle(error))
}

/// What input that a dialog interrupted means to the work that sent it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnDialog {
    /// The input landed: a dialog is the page's answer to a click.
    Succeeded,
    /// The input may have landed only in part: a dialog is not an answer to typing.
    EffectUnknown,
}

/// Input ready to send, with what its precondition can have changed.
pub struct PlannedInput {
    /// The primitive's arguments.
    input: Input,
    /// What the precondition can have changed in the page, which says whether a precondition
    /// the page answered counts as an effect.
    precondition_effect: EffectClass,
}

impl PlannedInput {
    /// The same input, bounded also by `budget_ms`.
    #[must_use]
    pub fn within(mut self, budget_ms: u64) -> Self {
        self.input.timeout_ms = self.input.timeout_ms.min(budget_ms);
        self
    }
}

/// The input for `steps` on a tab, bounded by what the deadline leaves.
#[must_use]
pub fn input_for(
    l: &Link<'_>,
    tab_id: u64,
    precondition: Option<PlannedCall>,
    steps: Vec<InputStep>,
    guard: Option<DocumentGuard>,
    settle_ms: u64,
) -> PlannedInput {
    let precondition_effect = precondition
        .as_ref()
        .map_or(EffectClass::Read, |plan| plan.effect);
    PlannedInput {
        input: Input {
            tab_id,
            precondition: precondition.map(|plan| plan.call),
            steps,
            guard,
            settle_ms,
            timeout_ms: l.budget_ms(),
        },
        precondition_effect,
    }
}

/// Send input and read how it ended.
///
/// A precondition the page refused sends nothing, and fails as the page's own failure. An input
/// that stopped for a dialog is [`OnDialog`]'s to judge, one that was cancelled is a cancellation
/// whether or not it had begun, and one that stopped short for any other reason has an unknown
/// effect. The precondition's answer is returned for the caller to read facts from.
///
/// # Errors
///
/// Returns the failure of the `input` primitive, and the failures above.
pub fn send_input(
    l: &mut Link<'_>,
    planned: PlannedInput,
    on_dialog: OnDialog,
) -> Result<Option<CallAnswer>, BrowserError> {
    let PlannedInput {
        input,
        precondition_effect,
    } = planned;
    let steps = input.steps.len();
    let preconditioned = input.precondition.is_some();
    let done = l.input(input, precondition_effect)?;
    let InputDone {
        precondition,
        completed_steps,
        stopped,
        ..
    } = done;
    if preconditioned {
        let answer = precondition
            .as_ref()
            .ok_or_else(|| l.settle(BrowserError::Protocol(PRECONDITION_UNANSWERED.into())))?;
        settled::<Value>(l, answer)?;
    }
    match stopped {
        None if usize::try_from(completed_steps).is_ok_and(|completed| completed >= steps) => {
            Ok(precondition)
        }
        None | Some(InputStop::Other) => Err(BrowserError::EffectUnknown(INPUT_INCOMPLETE.into())),
        Some(InputStop::Dialog { .. }) => match on_dialog {
            OnDialog::Succeeded => Ok(precondition),
            OnDialog::EffectUnknown => Err(BrowserError::EffectUnknown(DIALOG_DURING_INPUT.into())),
        },
        Some(InputStop::Cancelled) => Err(if l.effect_started() {
            BrowserError::CancelledAfterDispatch
        } else {
            BrowserError::CancelledBeforeDispatch
        }),
    }
}

/// A point placed in the document it lands in.
#[derive(Clone, Debug, PartialEq)]
pub struct Routed {
    /// The deepest document under the point.
    pub doc: FrameRef,
    /// The point in that document's own viewport.
    pub local: Point,
    /// The role and name of the element under the point.
    pub subject: Option<PhysicalActionSubject>,
}

/// Find the document a point of the top viewport lands in, and the subject under it there.
///
/// Routing starts at the top document and descends one embedded document per round trip, at
/// most [`MAX_ROUTE_DEPTH`] deep. Every call goes through the scope, and a hit or an owner probe
/// reads no content, so it may ask any document of the tree. A point that cannot be placed in
/// exactly one document answers `None`: nothing under it, an embed no probe owns, an embed the
/// extension could not describe, or a route deeper than the bound.
///
/// The point must already be in the viewport. A page point is brought there first, by the work
/// that owns the view check, so a point below the fold is placed where the page shows it.
///
/// # Errors
///
/// Returns the failure of a `call` primitive, and a document's own failure to say what is under
/// the point when that failure is not the point having no place: a document that changed under
/// the call or a dialog over it is that fact, not a point nobody claims.
fn route(
    l: &mut Link<'_>,
    scope: &Scope,
    tab_id: u64,
    point: Point,
) -> Result<Option<Routed>, BrowserError> {
    let tree = scope.tree();
    let mut doc = tree.top().clone();
    let mut local = point;
    for _ in 0..MAX_ROUTE_DEPTH {
        let children: Vec<FrameRef> = tree.children(&doc).into_iter().cloned().collect();
        let mut plans = vec![scope.call(&doc, world::hit_point(local.x, local.y, false))?];
        plans.extend(children.iter().map(|child| {
            PlannedCall::owner(&doc, &child.frame_id, world::owner_probe(local.x, local.y))
        }));
        let answers = l.call(CallRequest::parallel(tab_id, &plans), None)?;
        let hit = match value::<Hit>(&answers[0]) {
            Ok(hit) => hit,
            Err(error) => {
                return if unplaced(tree, &doc, &answers[0]) {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
        };
        let owners = answers[1..].iter().map(|answer| value(answer).ok());
        match geometry::route_step(&hit, children.iter().cloned().zip(owners)) {
            RouteStep::Subject { subject } => {
                return Ok(Some(Routed {
                    doc,
                    local: Point { x: hit.x, y: hit.y },
                    subject,
                }))
            }
            RouteStep::Descend { child, local: next } => {
                doc = child;
                local = next;
            }
            RouteStep::Unresolved => return Ok(None),
        }
    }
    Ok(None)
}

/// Whether a failed hit means the point has no place in `doc`: the page found nothing under it,
/// or `doc` is a document the adapter does not address, which answers no call reliably.
fn unplaced(tree: &FrameTree, doc: &FrameRef, answer: &CallAnswer) -> bool {
    !tree.addresses(doc)
        || matches!(
            world::decode_answer(answer),
            Some(Err(error)) if error.code == AssetErrorCode::NoSubject
        )
}

/// Place a point of the top viewport in its document, and admit that document as the subject of
/// the input that will reach the point.
///
/// This is the one place a point is bound to a document, and it runs when the input is about to
/// be sent: after the view check and after the point was brought into the viewport. It is the
/// check a scope that leaves some document unreadable needs before input: a point over an
/// embedded document that scope does not allow must not be clicked, and neither must one that
/// cannot be placed at all.
///
/// # Errors
///
/// Returns the failure of a `call` primitive, [`BrowserError::DocumentUnavailable`] for a point
/// that cannot be placed in exactly one document, and [`BrowserError::OutsideScope`] when the
/// scope does not allow the document the point lands in. Nothing has been sent as input in any
/// of these cases.
pub fn route_and_admit(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    point: Point,
) -> Result<Routed, BrowserError> {
    // The fact this site knows: the point is in the viewport and no single document of the tree
    // claims it (nothing under it, an embed no owner or several claim, an embed the extension
    // could not describe, or a route past the depth bound), so its document cannot be judged.
    let routed = route(l, scope, tab_id, point)?.ok_or(BrowserError::DocumentUnavailable(
        DocumentCause::SubjectUnreachable,
    ))?;
    scope.admit_input_subject(&routed.doc)?;
    Ok(routed)
}

#[cfg(test)]
mod tests;
