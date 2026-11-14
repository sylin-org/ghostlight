//! Observation: which documents a tab holds and which of them a request's locators belong to,
//! and waiting for a condition to hold.
//!
//! Describing documents asks the page nothing. It reports the tab's frame tree as the authority
//! names documents, and names the document of each locator, so the authority can decide before
//! any content is read. A wait asks the document its
//! condition lives in: the top for readiness and URL, the locator's document for a target, and
//! every readable document for text, because an embedded form's confirmation is as real as the
//! top page's.

use std::time::Instant;

use super::super::link::{CallRequest, Link};
use super::super::locator::Locator;
use super::super::scope::{PlannedCall, Scope};
use super::super::tree::{document_id, Described};
use super::super::wire::{CallAnswer, CallMode};
use super::super::world::{self, AssetErrorCode, Observed};
use super::read::some_answered;
use super::{frame_tree, value};
use crate::browser::adapter::documents::DocumentInventory;
use crate::browser::adapter::{BrowserOutcome, BrowserReadiness, PhysicalWait};
use crate::browser::BrowserError;
use crate::language::document_refusal::DocumentCause;
use crate::language::PageCondition;
use crate::workspace::StaleReason;

/// How much longer than the page's own wait a waiting call may take, so the page answers
/// before the call gives up.
const CALL_MARGIN_MS: u64 = 500;
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

/// Describe the tab's documents and say which of them the request's locators belong to.
///
/// This is one `frames` primitive and asks the page nothing. With a preparation budget the top
/// document first gets up to `settle_ms` to settle, and a failure there changes nothing. A
/// locator is current only while the tree still holds the document that minted it. One whose
/// document the tree does not hold is stale as navigated, however complete the inventory: a
/// handle is never left to a later refusal that would name another cause. A point and the focus
/// are not bound here. The page shows where they are only once the view is checked and the point
/// is in the viewport, so the work that sends the input binds them, through its scope.
///
/// # Errors
///
/// Returns the failure of a primitive after the settle,
/// [`BrowserError::DocumentUnavailable`] for an inventory with no top document, and
/// [`BrowserError::StaleLocator`] for a locator that is malformed (`unknown`) or whose document
/// was replaced or removed (`navigated`), before anything is read.
pub fn describe_documents(
    l: &mut Link<'_>,
    tab_id: u64,
    locators: &[String],
    settle_ms: u64,
) -> Result<BrowserOutcome, BrowserError> {
    settle_top(l, tab_id, settle_ms);
    let Described { tree, whole } = frame_tree(l, tab_id)?;
    let mut subjects: Vec<String> = Vec::new();
    for locator in locators {
        let doc = Locator::decode(locator)?.doc;
        if !tree.contains(&doc) {
            return Err(BrowserError::stale(StaleReason::Navigated));
        }
        let id = document_id(&doc);
        if !subjects.contains(&id) {
            subjects.push(id);
        }
    }
    Ok(BrowserOutcome::Documents {
        tab_id,
        inventory: DocumentInventory {
            documents: tree.documents(),
            subjects,
            incomplete: !whole,
        },
    })
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
/// `call` primitive, a single document's own failure, and the first document's own failure when
/// no readable document answered a text wait.
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
    // The fact this site knows: the scope holds no readable document to wait in.
    if docs.is_empty() {
        return Err(BrowserError::DocumentUnavailable(
            DocumentCause::NoWebDocument,
        ));
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
    // A document that waited out an unmet condition answered. When none answered at all, the
    // wait says nothing about the page, and the first document's own failure says why.
    if !answers.iter().any(waited_out) {
        some_answered::<Observed>(&answers)?;
    }
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

/// Whether a document answered a wait by waiting until its timeout with the condition unmet.
fn waited_out(answer: &CallAnswer) -> bool {
    matches!(
        world::decode_answer(answer),
        Some(Err(error)) if error.code == AssetErrorCode::Unsatisfied
    )
}

/// How far the top document has loaded, or loading when the top cannot say or the wait left no
/// time to ask it: a wait that used its whole deadline still reports what it saw.
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
    if l.budget_ms() == 0 {
        return Ok(BrowserReadiness::Loading);
    }
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
