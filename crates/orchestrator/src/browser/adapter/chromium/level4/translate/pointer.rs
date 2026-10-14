//! Pointer work on a located target: clicks, hovers and scrolls.
//!
//! A click or a hover is two steps. The first asks the target's own document to bring it into
//! view and report where it is, along with the position of every embedded document between it
//! and the top, all in one sequence. The second sends the pointer to that spot, in top-viewport
//! coordinates, as input guarded by the scope. A target that cannot be prepared fails with
//! nothing done; the input is the only step that can change the page.

use super::super::geometry;
use super::super::keys;
use super::super::link::Link;
use super::super::locator::Locator;
use super::super::scope::Scope;
use super::super::wire::{CallMode, FrameRef, Point};
use super::super::world::{self, Message, OwnerBox, Prepared, ScrollOffset, Scrolled};
use super::{input_for, reach, send_input, settled, value, OnDialog, Reach};
use crate::browser::adapter::{BrowserOutcome, PhysicalActionSubject};
use crate::browser::BrowserError;

/// How long the page may take to settle after a click, in milliseconds.
const CLICK_SETTLE_MS: u64 = 250;

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
    reach: &mut Reach<'_>,
    tab_id: u64,
    locator: &str,
    prepare: impl FnOnce(&str) -> Message,
) -> Result<Aim, BrowserError> {
    let target = Locator::decode(locator)?;
    let mut plans = vec![reach.get().call(&target.doc, prepare(&target.local))?];
    plans.extend(reach.get().tree().chain(&target.doc)?);
    let guard = reach.guard();
    let budget = l.budget_ms();
    let answers = l.call(
        tab_id,
        &plans,
        CallMode::Sequence,
        guard,
        0,
        budget,
        Some(reach.scope()),
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

/// Click a located target.
///
/// A point that must be placed in its document before input is refused for now: no input
/// reaches a document the scope leaves unreadable, and the placing is not done here. A dialog
/// the click opens is the page's answer, so it counts as success.
///
/// # Errors
///
/// Returns the plain failure of a target that cannot be prepared, the refusal above, and the
/// failure of the input, which is an unknown effect once the click began.
pub fn activate(
    l: &mut Link<'_>,
    scope: Option<&mut Scope>,
    click: Click<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let mut reach = reach(l, scope, click.tab_id)?;
    let aim = aim(l, &mut reach, click.tab_id, click.locator, world::activate)?;
    if reach.point_check_needed() {
        return Err(BrowserError::DocumentUnavailable);
    }
    reach.admit_input(&aim.doc)?;
    let steps = keys::click(
        aim.point,
        keys::mouse_button(click.button),
        click.click_count,
        keys::modifier_mask(click.modifiers),
    );
    let input = input_for(l, click.tab_id, None, steps, reach.guard(), CLICK_SETTLE_MS);
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
    scope: Option<&mut Scope>,
    tab_id: u64,
    locator: &str,
) -> Result<BrowserOutcome, BrowserError> {
    let mut reach = reach(l, scope, tab_id)?;
    let aim = aim(l, &mut reach, tab_id, locator, world::hover)?;
    if reach.point_check_needed() {
        return Err(BrowserError::DocumentUnavailable);
    }
    reach.admit_input(&aim.doc)?;
    let input = input_for(l, tab_id, None, keys::hover(aim.point), reach.guard(), 0);
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
    scope: Option<&mut Scope>,
    request: Scroll<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let mut reach = reach(l, scope, request.tab_id)?;
    let top = reach.get().top();
    let (doc, local) = reach.target(request.locator)?;
    let mut plans = vec![reach.get().call(
        &doc,
        world::scroll(local.as_deref(), request.direction, request.amount),
    )?];
    let nested = doc != top;
    if nested {
        plans.push(reach.get().call(&top, world::scroll_offset())?);
    }
    let guard = reach.guard();
    let budget = l.budget_ms();
    let answers = l.call(
        request.tab_id,
        &plans,
        CallMode::Sequence,
        guard,
        0,
        budget,
        Some(reach.scope()),
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
