//! Observation: which documents a tab holds and where a request's subjects live, and waiting for
//! a condition to hold.
//!
//! Describing documents reads no page content. It reports the tab's frame tree as the authority
//! names documents, and binds each locator, point and the focus to the document it belongs to,
//! so the authority can decide before any content is read. A wait asks the document its
//! condition lives in: the top for readiness and URL, the locator's document for a target, and
//! every readable document for text, because an embedded form's confirmation is as real as the
//! top page's.

use std::time::Instant;

use super::super::link::{CallRequest, Link};
use super::super::locator::Locator;
use super::super::scope::{PlannedCall, Scope};
use super::super::tree::{document_id, FrameTree};
use super::super::wire::{CallMode, FrameRef, Point};
use super::super::world::{self, FocusProbe, Observed};
use super::{route, value};
use crate::browser::adapter::documents::DocumentInventory;
use crate::browser::adapter::{BrowserOutcome, BrowserReadiness, PhysicalPoint, PhysicalWait};
use crate::browser::BrowserError;
use crate::language::PageCondition;
use crate::workspace::StaleReason;

/// How much longer than the page's own wait a waiting call may take, so the page answers
/// before the call gives up.
const CALL_MARGIN_MS: u64 = 500;
/// What a request asks a document description to bind.
#[derive(Clone, Copy, Debug)]
pub struct Subjects<'a> {
    /// Locators whose documents are subjects.
    pub locators: &'a [String],
    /// Page points whose documents are subjects.
    pub points: &'a [PhysicalPoint],
    /// Whether the focused document is a subject.
    pub focused: bool,
}

/// The call that waits up to `settle_ms` for the top document to stop moving. It answers rather
/// than fails when the page does not settle in time.
pub(super) fn settle(settle_ms: u64) -> PlannedCall {
    PlannedCall::top(world::observe(
        PageCondition::VisualSettle.as_str(),
        None,
        None,
        settle_ms,
        false,
    ))
}

/// Give the top document up to `settle_ms`, the authority's preparation budget, to stop moving,
/// and go on whatever the answer. A zero budget sends nothing.
pub(super) fn settle_top(l: &mut Link<'_>, tab_id: u64, settle_ms: u64) {
    if settle_ms > 0 {
        let plans = [settle(settle_ms)];
        let request = CallRequest::sequence(tab_id, &plans);
        let _ = l.call(request.budget(settle_ms + CALL_MARGIN_MS), None);
    }
}

/// Describe the tab's documents and bind the request's subjects to them.
///
/// With a preparation budget the top document first gets up to `settle_ms` to settle, and a
/// failure there changes nothing. A locator is current only while its frame still holds
/// the loader that minted it: a frame that holds another loader, or that a complete inventory
/// no longer has, replaced or removed the locator's document. A locator whose frame an
/// incomplete inventory does not show binds nothing, and the incomplete mark says so. A point
/// is routed from the top page down. The focus is the first document in tree order that holds
/// it. A point or focus that cannot be bound marks the inventory unresolved; an inventory the
/// extension could not complete is marked incomplete.
///
/// # Errors
///
/// Returns the failure of a primitive after the settle,
/// [`BrowserError::DocumentUnavailable`] for an inventory that is not one tree, and
/// [`BrowserError::StaleLocator`] for a locator that is malformed (`unknown`) or whose document
/// was replaced or removed (`navigated`), before anything is read.
pub fn describe_documents(
    l: &mut Link<'_>,
    tab_id: u64,
    subjects: Subjects<'_>,
    settle_ms: u64,
) -> Result<BrowserOutcome, BrowserError> {
    settle_top(l, tab_id, settle_ms);
    let inventory = l.send(super::super::link::Frames(tab_id))?;
    let tree = FrameTree::from_inventory(&inventory)?;
    let mut bound: Vec<String> = Vec::new();
    let mut unresolved = false;
    let mut bind = |doc: Option<FrameRef>| match doc {
        Some(doc) => {
            let id = document_id(&doc);
            if !bound.contains(&id) {
                bound.push(id);
            }
        }
        None => unresolved = true,
    };
    for locator in subjects.locators {
        let doc = Locator::decode(locator)?.doc;
        if tree.contains(&doc) {
            bind(Some(doc));
        } else if inventory.complete || tree.holds_frame(&doc.frame_id) {
            return Err(BrowserError::stale(StaleReason::Navigated));
        }
    }
    for point in subjects.points {
        let at = Point {
            x: point.x,
            y: point.y,
        };
        bind(route(l, &tree, tab_id, at, true)?.map(|routed| routed.doc));
    }
    if subjects.focused {
        bind(focused_document(l, &tree, tab_id)?);
    }
    Ok(BrowserOutcome::Documents {
        tab_id,
        inventory: DocumentInventory {
            documents: tree.documents(),
            subjects: bound,
            unresolved,
            incomplete: !inventory.complete,
        },
    })
}

/// The first addressable document in tree order that holds the page's focus.
fn focused_document(
    l: &mut Link<'_>,
    tree: &FrameTree,
    tab_id: u64,
) -> Result<Option<FrameRef>, BrowserError> {
    let docs: Vec<FrameRef> = tree
        .real_documents()
        .filter(|document| document.supported)
        .map(|document| document.doc.clone())
        .collect();
    if docs.is_empty() {
        return Ok(None);
    }
    let plans: Vec<PlannedCall> = docs
        .iter()
        .map(|doc| PlannedCall::unscoped(doc, world::focus_probe()))
        .collect();
    let answers = l.call(CallRequest::parallel(tab_id, &plans), None)?;
    Ok(docs
        .into_iter()
        .zip(&answers)
        .find(|(_, answer)| value::<FocusProbe>(answer).is_ok_and(|probe| probe.focused))
        .map(|(doc, _)| doc))
}

/// Wait for one condition in the document it lives in.
///
/// Text presence races every readable document: the first that shows the text wins and the
/// extension cancels the rest. Text absence needs every readable document, and one that cannot
/// answer counts as absent; the authority judges what an unread document means. The page owns
/// the timeout; the call only backs it up.
///
/// # Errors
///
/// Returns the plain failure of a malformed locator, the failure of a
/// `call` primitive, and a single document's own failure.
pub fn observe(
    l: &mut Link<'_>,
    scope: &mut Scope,
    wait: &PhysicalWait,
) -> Result<BrowserOutcome, BrowserError> {
    let tab_id = wait.tab_id;
    let budget = wait.timeout_ms.saturating_add(CALL_MARGIN_MS);
    let (doc, local) = match wait.condition {
        PageCondition::VisualSettle => scope.target(wait.locator.as_deref())?,
        PageCondition::LoadReady | PageCondition::UrlContains => scope.target(None)?,
        PageCondition::TargetPresent | PageCondition::TargetAbsent => {
            scope.target(Some(wait.locator.as_deref().unwrap_or_default()))?
        }
        PageCondition::TextPresent | PageCondition::TextAbsent => {
            return observe_text(l, scope, tab_id, wait, budget)
        }
    };
    let plan = scope.call(
        &doc,
        world::observe(
            wait.condition.as_str(),
            wait.value.as_deref(),
            local.as_deref(),
            wait.timeout_ms,
            false,
        ),
    )?;
    let plans = [plan];
    let request = CallRequest::sequence(tab_id, &plans).guard(scope.guard());
    let answers = l.call(request.budget(budget), Some(&mut *scope))?;
    let observed: Observed = value(&answers[0])?;
    Ok(BrowserOutcome::Observed {
        tab_id,
        satisfied: observed.satisfied,
        elapsed_ms: observed.elapsed_ms,
        readiness: observed.readiness.into(),
    })
}

/// Wait for text to appear in some readable document, or to be gone from all of them.
fn observe_text(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    wait: &PhysicalWait,
    budget: u64,
) -> Result<BrowserOutcome, BrowserError> {
    let presence = wait.condition == PageCondition::TextPresent;
    let docs = scope.content_docs();
    if docs.is_empty() {
        return Err(BrowserError::DocumentUnavailable);
    }
    let plans = docs
        .iter()
        .map(|doc| {
            scope.call(
                doc,
                world::observe(
                    wait.condition.as_str(),
                    wait.value.as_deref(),
                    None,
                    wait.timeout_ms,
                    presence,
                ),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mode = if presence {
        CallMode::Race
    } else {
        CallMode::Parallel
    };
    let started = Instant::now();
    let request = CallRequest::new(tab_id, &plans, mode).guard(scope.guard());
    let answers = l.call(request.budget(budget), Some(&mut *scope))?;
    let observed: Vec<Option<Observed>> = answers.iter().map(|answer| value(answer).ok()).collect();
    let satisfied = if presence {
        observed.iter().flatten().any(|answer| answer.satisfied)
    } else {
        observed
            .iter()
            .all(|answer| answer.is_none_or(|answer| answer.satisfied))
    };
    let elapsed_ms = observed
        .iter()
        .flatten()
        .map(|answer| answer.elapsed_ms)
        .max()
        .unwrap_or_else(|| u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
    Ok(BrowserOutcome::Observed {
        tab_id,
        satisfied,
        elapsed_ms,
        readiness: top_readiness(l, scope, tab_id)?,
    })
}

/// How far the top document has loaded, or loading when the top cannot say.
///
/// # Errors
///
/// Returns the failure of the `call` primitive.
fn top_readiness(
    l: &mut Link<'_>,
    scope: &Scope,
    tab_id: u64,
) -> Result<BrowserReadiness, BrowserError> {
    let top = scope.top();
    let Ok(plan) = scope.call(
        &top,
        world::observe(PageCondition::LoadReady.as_str(), None, None, 0, false),
    ) else {
        return Ok(BrowserReadiness::Loading);
    };
    let answers = l.call(CallRequest::sequence(tab_id, &[plan]), None)?;
    Ok(
        value::<Observed>(&answers[0]).map_or(BrowserReadiness::Loading, |observed| {
            observed.readiness.into()
        }),
    )
}
