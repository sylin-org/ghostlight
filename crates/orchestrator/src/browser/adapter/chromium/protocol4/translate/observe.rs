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

use super::super::link::Link;
use super::super::locator::Locator;
use super::super::scope::{PlannedCall, Scope};
use super::super::tree::{document_id, FrameTree};
use super::super::wire::{CallMode, FrameRef, Point};
use super::super::world::{self, FocusProbe, Observed};
use super::{route, value};
use crate::browser::adapter::documents::DocumentInventory;
use crate::browser::adapter::{BrowserOutcome, BrowserReadiness, PhysicalPoint};
use crate::browser::BrowserError;
use crate::workspace::StaleReason;

/// The most preparation time a description may spend letting the page settle, in milliseconds.
const SETTLE_CAP_MS: u64 = 1_000;
/// How much longer than the page's own wait a waiting call may take, so the page answers
/// before the call gives up.
const CALL_MARGIN_MS: u64 = 500;
/// The failure for a wait condition this service does not know.
const UNKNOWN_CONDITION: &str = "unknown wait condition";

/// The wait conditions work may name.
pub mod condition {
    /// The page stops moving.
    pub const VISUAL_SETTLE: &str = "visual_settle";
    /// The top document is interactive or complete.
    pub const LOAD_READY: &str = "load_ready";
    /// The top document's URL contains the value.
    pub const URL_CONTAINS: &str = "url_contains";
    /// The locator's target is in the page.
    pub const TARGET_PRESENT: &str = "target_present";
    /// The locator's target is gone.
    pub const TARGET_ABSENT: &str = "target_absent";
    /// Some readable document shows the text.
    pub const TEXT_PRESENT: &str = "text_present";
    /// No readable document shows the text.
    pub const TEXT_ABSENT: &str = "text_absent";
}

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

/// Describe the tab's documents and bind the request's subjects to them.
///
/// With a preparation budget the top document first gets up to [`SETTLE_CAP_MS`] to settle,
/// and a failure there changes nothing. A locator is current only while its frame still holds
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
    if settle_ms > 0 {
        let settle = settle_ms.min(SETTLE_CAP_MS);
        let budget = settle + CALL_MARGIN_MS;
        let plan = PlannedCall::top(world::observe(
            condition::VISUAL_SETTLE,
            None,
            None,
            settle,
            false,
        ));
        let _ = l.call(tab_id, &[plan], CallMode::Sequence, None, 0, budget, None);
    }
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
    let budget = l.budget_ms();
    let answers = l.call(tab_id, &plans, CallMode::Parallel, None, 0, budget, None)?;
    Ok(docs
        .into_iter()
        .zip(&answers)
        .find(|(_, answer)| value::<FocusProbe>(answer).is_ok_and(|probe| probe.focused))
        .map(|(doc, _)| doc))
}

/// One wait for a condition.
#[derive(Clone, Copy, Debug)]
pub struct Wait<'a> {
    /// The condition.
    pub condition: &'a str,
    /// The text or URL fragment the condition compares, when it takes one.
    pub value: Option<&'a str>,
    /// The target the condition watches, when it watches one.
    pub locator: Option<&'a str>,
    /// How long the page may wait, in milliseconds.
    pub timeout_ms: u64,
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
/// Returns the plain failure of an unknown condition or a malformed locator, the failure of a
/// `call` primitive, and a single document's own failure.
pub fn observe(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    wait: Wait<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let budget = wait.timeout_ms.saturating_add(CALL_MARGIN_MS);
    let (doc, local) = match wait.condition {
        condition::VISUAL_SETTLE => scope.target(wait.locator)?,
        condition::LOAD_READY | condition::URL_CONTAINS => scope.target(None)?,
        condition::TARGET_PRESENT | condition::TARGET_ABSENT => {
            scope.target(Some(wait.locator.unwrap_or_default()))?
        }
        condition::TEXT_PRESENT | condition::TEXT_ABSENT => {
            return observe_text(l, scope, tab_id, wait, budget)
        }
        other => {
            return Err(BrowserError::Primitive(format!(
                "{UNKNOWN_CONDITION} {other}"
            )))
        }
    };
    let plan = scope.call(
        &doc,
        world::observe(
            wait.condition,
            wait.value,
            local.as_deref(),
            wait.timeout_ms,
            false,
        ),
    )?;
    let guard = Some(scope.guard());
    let answers = l.call(
        tab_id,
        &[plan],
        CallMode::Sequence,
        guard,
        0,
        budget,
        Some(&mut *scope),
    )?;
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
    wait: Wait<'_>,
    budget: u64,
) -> Result<BrowserOutcome, BrowserError> {
    let presence = wait.condition == condition::TEXT_PRESENT;
    let docs = scope.content_docs();
    if docs.is_empty() {
        return Err(BrowserError::DocumentUnavailable);
    }
    let plans = docs
        .iter()
        .map(|doc| {
            scope.call(
                doc,
                world::observe(wait.condition, wait.value, None, wait.timeout_ms, presence),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mode = if presence {
        CallMode::Race
    } else {
        CallMode::Parallel
    };
    let started = Instant::now();
    let guard = Some(scope.guard());
    let answers = l.call(tab_id, &plans, mode, guard, 0, budget, Some(&mut *scope))?;
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
        world::observe(condition::LOAD_READY, None, None, 0, false),
    ) else {
        return Ok(BrowserReadiness::Loading);
    };
    let budget = l.budget_ms();
    let answers = l.call(tab_id, &[plan], CallMode::Sequence, None, 0, budget, None)?;
    Ok(
        value::<Observed>(&answers[0]).map_or(BrowserReadiness::Loading, |observed| {
            observed.readiness.into()
        }),
    )
}
