//! The translation of each unit of browser work into protocol-4 primitives.
//!
//! Each submodule owns one family of work. A function there sends the primitives its work
//! needs through the [`Link`] and turns their outcomes into the one
//! [`BrowserOutcome`](crate::browser::adapter::BrowserOutcome) the rest of the service reads.
//! This module holds what the families share: the scope every content command needs, the
//! reading of one world call's answer, the sending of input and the reading of how it ended, and
//! the routing of a point to the document it lands in.

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
use super::tree::FrameTree;
use super::wire::{
    CallAnswer, CallFailure, DocumentGuard, FrameRef, Input, InputDone, InputStep, InputStop, Point,
};
use super::world::{self, AssetDetail, EffectClass, Hit, OwnerProbe};
use crate::browser::adapter::PhysicalActionSubject;
use crate::browser::BrowserError;

/// The protocol failure of a content command sent without a document scope.
const UNSCOPED_CONTENT: &str = "content command sent without a document scope";
/// The failure of a point that cannot be placed in exactly one document.
const UNRESOLVED_POINT: &str = "point has no current document subject";
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

/// The tab's frame tree, from one `frames` primitive.
///
/// # Errors
///
/// Returns the primitive's failure, and [`BrowserError::DocumentUnavailable`] for an inventory
/// that is not one tree.
pub fn frame_tree(l: &mut Link<'_>, tab_id: u64) -> Result<FrameTree, BrowserError> {
    let inventory = l.send(super::link::Frames(tab_id))?;
    FrameTree::from_inventory(&inventory)
}

/// Read one world call's answer as the value its kind returns.
///
/// # Errors
///
/// Returns the asset's own failure as a plain failure, as [`BrowserError::FieldsRejected`]
/// when it named the fields a fill could not take, as [`BrowserError::StaleLocator`] for a
/// locator that names no control, or as [`BrowserError::PageRefused`] for a control the page
/// refused to act on. Returns [`BrowserError::DocumentUnavailable`] when the
/// document was gone or replaced, [`BrowserError::DialogOpen`] when a JavaScript dialog blocks
/// the page, and [`BrowserError::Protocol`] for a value that does not have the kind's shape.
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
                    None => BrowserError::Primitive(if error.message.is_empty() {
                        wire_name(&error.code)
                    } else {
                        error.message
                    }),
                },
            }),
            None => Err(BrowserError::Protocol(CALL_UNKNOWN.into())),
        },
        CallAnswer::Failed {
            code: CallFailure::DocumentChanged | CallFailure::FrameMissing,
            ..
        } => Err(BrowserError::DocumentUnavailable),
        CallAnswer::Failed {
            code: CallFailure::DialogOpen,
            ..
        } => Err(BrowserError::DialogOpen),
        CallAnswer::Failed { code, message, .. } => {
            Err(BrowserError::Primitive(if message.is_empty() {
                wire_name(code)
            } else {
                message.clone()
            }))
        }
        CallAnswer::Skipped => Err(BrowserError::Primitive(CALL_SKIPPED.into())),
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

/// A closed code as the wire spells it.
fn wire_name(code: &impl serde::Serialize) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
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

/// Find the document a point lands in, and the subject under it there.
///
/// Routing starts at the top document and descends one embedded document per round trip, at
/// most [`MAX_ROUTE_DEPTH`] deep. `page` says the point is in the top page's coordinates rather
/// than its viewport's. A point that cannot be placed in exactly one document answers `None`:
/// nothing under it, an embed no probe owns, an embed the extension could not describe, or a
/// route deeper than the bound.
///
/// # Errors
///
/// Returns the failure of a `call` primitive.
pub fn route(
    l: &mut Link<'_>,
    tree: &FrameTree,
    tab_id: u64,
    point: Point,
    page: bool,
) -> Result<Option<Routed>, BrowserError> {
    let mut doc = tree.top().clone();
    let mut local = point;
    let mut page = page;
    for _ in 0..MAX_ROUTE_DEPTH {
        let children: Vec<FrameRef> = tree.children(&doc).into_iter().cloned().collect();
        let probes = |at: Point| -> Vec<PlannedCall> {
            children
                .iter()
                .map(|child| {
                    PlannedCall::owner(&doc, &child.frame_id, world::owner_probe(at.x, at.y))
                })
                .collect()
        };
        let mut plans = vec![PlannedCall::unscoped(
            &doc,
            world::hit_point(local.x, local.y, page),
        )];
        let together = !page && !children.is_empty();
        if together {
            plans.extend(probes(local));
        }
        let answers = l.call(CallRequest::parallel(tab_id, &plans), None)?;
        let Ok(hit) = value::<Hit>(&answers[0]) else {
            return Ok(None);
        };
        let owners: Vec<Option<OwnerProbe>> = if together {
            answers[1..]
                .iter()
                .map(|answer| value(answer).ok())
                .collect()
        } else if hit.embed && !children.is_empty() {
            let at = Point { x: hit.x, y: hit.y };
            l.call(CallRequest::parallel(tab_id, &probes(at)), None)?
                .iter()
                .map(|answer| value(answer).ok())
                .collect()
        } else {
            Vec::new()
        };
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
                page = false;
            }
            RouteStep::Unresolved => return Ok(None),
        }
    }
    Ok(None)
}

/// Place a point of the top viewport in its document, and admit that document as the subject of
/// the input that will reach the point.
///
/// This is the check a scope that leaves some document unreadable needs before input: a point
/// over an embedded document that scope does not allow must not be clicked, and neither must
/// one that cannot be placed at all.
///
/// # Errors
///
/// Returns the failure of a `call` primitive, a plain failure for a point that cannot be placed
/// in exactly one document, and [`BrowserError::DocumentUnavailable`] when the scope does not
/// allow the document the point lands in. Nothing has been sent as input in any of these cases.
pub fn route_and_admit(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    point: Point,
) -> Result<Routed, BrowserError> {
    let routed = route(l, scope.tree(), tab_id, point, false)?
        .ok_or_else(|| BrowserError::Primitive(UNRESOLVED_POINT.into()))?;
    scope.admit_input_subject(&routed.doc)?;
    Ok(routed)
}

#[cfg(test)]
mod tests;
