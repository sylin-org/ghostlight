//! Pointer work on a located target: clicks, hovers, scrolls and drags.
//!
//! A click or a hover is two steps. The first asks the target's own document to bring it into
//! view and report where it is, along with the position of every embedded document between it
//! and the top, all in one sequence. The second sends the pointer to that spot, in top-viewport
//! coordinates, as input guarded by the scope. A target that cannot be prepared fails with
//! nothing done; the input is the only step that can change the page. A drag measures both of
//! its ends the same way before it presses anywhere.

use std::ops::Range;

use serde_json::Value;

use super::super::geometry;
use super::super::keys;
use super::super::link::Link;
use super::super::locator::Locator;
use super::super::scope::Scope;
use super::super::wire::{CallMode, FrameRef, InputStep, Point};
use super::super::world::{self, Message, OwnerBox, Prepared, ScrollOffset, Scrolled};
use super::{input_for, route_and_admit, send_input, settled, value, OnDialog};
use crate::browser::adapter::{BrowserOutcome, PhysicalActionSubject};
use crate::browser::BrowserError;

/// How long the page may take to settle after a click, in milliseconds.
pub(super) const CLICK_SETTLE_MS: u64 = 250;
/// How many moves a drag takes from where it presses to where it releases.
const DRAG_STEPS: usize = 12;
/// The calls a drag makes to bring its two ends into view: the destination, then the source.
const DRAG_PREPARATIONS: usize = 2;
/// Where the destination's preparing hover sits among a drag's calls.
const DRAG_DESTINATION_HOVER: usize = 0;
/// Where the source's preparing hover sits among a drag's calls.
const DRAG_SOURCE_HOVER: usize = 1;
/// Where the source's measurement sits among a drag's calls.
const DRAG_SOURCE_BOX: usize = 2;
/// Where the destination's measurement sits among a drag's calls.
const DRAG_DESTINATION_BOX: usize = 3;
/// How many calls a drag makes before the owner-box chains that place its ends.
const DRAG_FIXED_CALLS: usize = 4;

/// One click on a located target.
#[derive(Clone, Copy, Debug)]
pub struct Click<'a> {
    /// The tab.
    pub tab_id: u64,
    /// The target's encoded locator.
    pub locator: &'a str,
    /// The button's name.
    pub button: &'a str,
    /// How many clicks, one after another.
    pub click_count: u8,
    /// The modifier names held during the clicks.
    pub modifiers: &'a [String],
}

/// Where a prepared target sits, and what it is.
struct Aim {
    /// The document the target lives in.
    doc: FrameRef,
    /// The target's role and name.
    subject: PhysicalActionSubject,
    /// The middle of the target in top-viewport coordinates.
    point: Point,
}

/// Bring a target into view and find where the pointer must go to reach it.
///
/// The preparation message goes to the target's document, followed in the same sequence by the
/// owner-box call for every embedded document above it. Nothing here changes the page beyond
/// scrolling, so any failure is a plain one.
fn aim(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    locator: &str,
    prepare: impl FnOnce(&str) -> Message,
) -> Result<Aim, BrowserError> {
    let target = Locator::decode(locator)?;
    let mut plans = vec![scope.call(&target.doc, prepare(&target.local))?];
    plans.extend(scope.tree().chain(&target.doc)?);
    let guard = Some(scope.guard());
    let budget = l.budget_ms();
    let answers = l.call(
        tab_id,
        &plans,
        CallMode::Sequence,
        guard,
        0,
        budget,
        Some(&mut *scope),
    )?;
    let prepared: Prepared = value(&answers[0])?;
    let boxes = answers[1..]
        .iter()
        .map(value::<OwnerBox>)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Aim {
        doc: target.doc,
        subject: prepared.subject,
        point: prepared.rectangle.center() + geometry::offset(&boxes),
    })
}

/// Admit the document the pointer will reach as a subject of the scope's input. A scope that
/// leaves some document unreadable first places the target's point in its document, because
/// something else may lie over the target.
///
/// # Errors
///
/// Returns what [`route_and_admit`] and the scope's admission return. Nothing has been sent as
/// input.
fn admit_aim(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    aim: &Aim,
) -> Result<(), BrowserError> {
    if scope.needs_point_check() {
        route_and_admit(l, scope, tab_id, aim.point)?;
    }
    scope.admit_input_subject(&aim.doc)
}

/// Click a located target.
///
/// A scope that leaves some document unreadable has the click's point placed in its document
/// first, and a document it does not allow is refused before any input. A dialog the click
/// opens is the page's answer, so it counts as success.
///
/// # Errors
///
/// Returns the plain failure of a target that cannot be prepared, the refusals above, and the
/// failure of the input, which is an unknown effect once the click began.
pub fn activate(
    l: &mut Link<'_>,
    scope: &mut Scope,
    click: Click<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let aim = aim(l, scope, click.tab_id, click.locator, world::activate)?;
    admit_aim(l, scope, click.tab_id, &aim)?;
    let steps = keys::click(
        aim.point,
        keys::mouse_button(click.button),
        click.click_count,
        keys::modifier_mask(click.modifiers),
    );
    let input = input_for(
        l,
        click.tab_id,
        None,
        steps,
        Some(scope.guard()),
        CLICK_SETTLE_MS,
    );
    send_input(l, input, OnDialog::Succeeded)?;
    Ok(BrowserOutcome::Activated {
        tab: l.tab(click.tab_id)?.into(),
        subject: Some(aim.subject),
        committed_urls: l.committed(),
    })
}

/// Move the pointer over a located target.
///
/// # Errors
///
/// Returns what [`activate`] returns for the same target.
pub fn hover(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    locator: &str,
) -> Result<BrowserOutcome, BrowserError> {
    let aim = aim(l, scope, tab_id, locator, world::hover)?;
    admit_aim(l, scope, tab_id, &aim)?;
    let input = input_for(
        l,
        tab_id,
        None,
        keys::hover(aim.point),
        Some(scope.guard()),
        0,
    );
    send_input(l, input, OnDialog::Succeeded)?;
    Ok(BrowserOutcome::Hovered {
        tab_id,
        subject: Some(aim.subject),
    })
}

/// What a scroll asks for.
#[derive(Clone, Copy, Debug)]
pub struct Scroll<'a> {
    /// The tab.
    pub tab_id: u64,
    /// The target to reveal, when the scroll names one.
    pub locator: Option<&'a str>,
    /// The direction to scroll the page in, when it names no target.
    pub direction: Option<&'a str>,
    /// How far to scroll the page, when it names no target.
    pub amount: Option<&'a str>,
}

/// Scroll the page or reveal a located target.
///
/// A target inside an embedded document is revealed there, and the position reported is the top
/// document's, read in the same sequence.
///
/// # Errors
///
/// Returns the failure of the `call` primitive and the document's own failure.
pub fn scroll(
    l: &mut Link<'_>,
    scope: &mut Scope,
    request: Scroll<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let top = scope.top();
    let (doc, local) = scope.target(request.locator)?;
    let mut plans = vec![scope.call(
        &doc,
        world::scroll(local.as_deref(), request.direction, request.amount),
    )?];
    let nested = doc != top;
    if nested {
        plans.push(scope.call(&top, world::scroll_offset())?);
    }
    let guard = Some(scope.guard());
    let budget = l.budget_ms();
    let answers = l.call(
        request.tab_id,
        &plans,
        CallMode::Sequence,
        guard,
        0,
        budget,
        Some(&mut *scope),
    )?;
    let scrolled: Scrolled = settled(l, &answers[0])?;
    let (x, y) = if nested {
        let offset: ScrollOffset = settled(l, &answers[1])?;
        (offset.x, offset.y)
    } else {
        (scrolled.x, scrolled.y)
    };
    Ok(BrowserOutcome::Scrolled {
        tab_id: request.tab_id,
        x,
        y,
        subject: scrolled.subject,
    })
}

/// What a drag between two located targets asks for.
#[derive(Clone, Copy, Debug)]
pub struct Drag<'a> {
    /// The tab.
    pub tab_id: u64,
    /// The encoded locator of the target the drag starts on.
    pub source: &'a str,
    /// The encoded locator of the target the drag ends on.
    pub destination: &'a str,
}

/// Drag from one located target to another.
///
/// The destination is brought into view first and then the source, so the source is the one
/// left in view, and both are measured in the same sequence as the owner-box chains that place
/// them in the top viewport. The pointer then presses on the source, moves in equal strides and
/// releases on the destination. A drag is not bounded to the documents at its ends, so the
/// scope's admission of the two targets is the check, and no point is routed.
///
/// # Errors
///
/// Returns the plain failure of a target that cannot be prepared, and the failure of the
/// input, which is an unknown effect once the pointer pressed.
pub fn drag(
    l: &mut Link<'_>,
    scope: &mut Scope,
    request: Drag<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let source = Locator::decode(request.source)?;
    let destination = Locator::decode(request.destination)?;
    let source_chain = scope.tree().chain(&source.doc)?;
    let destination_chain = scope.tree().chain(&destination.doc)?;
    let source_end = DRAG_FIXED_CALLS + source_chain.len();
    let destination_end = source_end + destination_chain.len();
    let mut plans = vec![
        scope.call(&destination.doc, world::hover(&destination.local))?,
        scope.call(&source.doc, world::hover(&source.local))?,
        scope.call(&source.doc, world::box_(&source.local))?,
        scope.call(&destination.doc, world::box_(&destination.local))?,
    ];
    plans.extend(source_chain);
    plans.extend(destination_chain);
    let guard = Some(scope.guard());
    let budget = l.budget_ms();
    let answers = l.call(
        request.tab_id,
        &plans,
        CallMode::Sequence,
        guard,
        0,
        budget,
        Some(&mut *scope),
    )?;
    for answer in &answers[..DRAG_PREPARATIONS] {
        value::<Value>(answer)?;
    }
    let mut from: Prepared = value(&answers[DRAG_SOURCE_BOX])?;
    let mut onto: Prepared = value(&answers[DRAG_DESTINATION_BOX])?;
    // The hover that brings an end into view is the call that follows a re-rendered handle; the
    // measurement after it finds the control live and cannot say so.
    from.subject.rebound |= world::answer_rebound(&answers[DRAG_SOURCE_HOVER]);
    onto.subject.rebound |= world::answer_rebound(&answers[DRAG_DESTINATION_HOVER]);
    let boxes = |range: Range<usize>| {
        answers[range]
            .iter()
            .map(value::<OwnerBox>)
            .collect::<Result<Vec<_>, _>>()
    };
    let start = from.rectangle.center() + geometry::offset(&boxes(DRAG_FIXED_CALLS..source_end)?);
    let end = onto.rectangle.center() + geometry::offset(&boxes(source_end..destination_end)?);
    scope.admit_input_subject(&source.doc)?;
    scope.admit_input_subject(&destination.doc)?;
    dragged(
        l,
        scope,
        request.tab_id,
        (start, end),
        (Some(from.subject), Some(onto.subject)),
    )
}

/// The subjects at the two ends of a drag, source first.
pub(super) type DragSubjects = (Option<PhysicalActionSubject>, Option<PhysicalActionSubject>);

/// Press at the start of a drag, move to its end and release, then report the tab and the
/// subjects at both ends.
///
/// A dialog that opens while the pointer is down leaves the drag half done, so it is an unknown
/// effect.
///
/// # Errors
///
/// Returns the failure of the input, and a plain failure when the browser no longer has the tab.
pub(super) fn dragged(
    l: &mut Link<'_>,
    scope: &Scope,
    tab_id: u64,
    (start, end): (Point, Point),
    (source_subject, destination_subject): DragSubjects,
) -> Result<BrowserOutcome, BrowserError> {
    let steps = vec![InputStep::Drag {
        path: keys::drag_path(start, end, DRAG_STEPS),
        modifiers: 0,
    }];
    let input = input_for(l, tab_id, None, steps, Some(scope.guard()), 0);
    send_input(l, input, OnDialog::EffectUnknown)?;
    Ok(BrowserOutcome::Dragged {
        tab: l.tab(tab_id)?.into(),
        source_subject,
        destination_subject,
        committed_urls: l.committed(),
    })
}
