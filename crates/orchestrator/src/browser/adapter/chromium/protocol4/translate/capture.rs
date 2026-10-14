//! Screenshots.
//!
//! A capture is one primitive. The extension hides Ghostlight's own presentation for its
//! duration and restores it on every exit, so the service sends no hiding calls. The service
//! decides the clip: the visible viewport, the whole page, one located target placed in the top
//! viewport, or a region of the page the model chose on an earlier screenshot. Waiting for the
//! page to settle first is best effort and never fails a capture.

use super::super::geometry;
use super::super::link::Link;
use super::super::locator::Locator;
use super::super::scope::PlannedCall;
use super::super::wire::{CallMode, Capture, CaptureClip};
use super::super::world::{self, OwnerBox, Prepared};
use super::observe::condition;
use super::{frame_tree, value};
use crate::browser::adapter::{BrowserOutcome, CaptureScope, PhysicalRectangle, ViewportGeometry};
use crate::browser::BrowserError;

/// The longest the page may take to settle before a capture, in milliseconds.
const SETTLE_MS: u64 = 1_000;
/// How long a settle sent on its own may take to answer, in milliseconds.
const SETTLE_CALL_MS: u64 = 1_500;

/// The call that waits for the top document to stop moving. It answers rather than fails when the
/// page does not settle in time.
fn settle() -> PlannedCall {
    PlannedCall::top(world::observe(
        condition::VISUAL_SETTLE,
        None,
        None,
        SETTLE_MS,
        false,
    ))
}

/// Wait for the page to settle on its own, and go on whatever the answer.
fn settle_first(l: &mut Link<'_>, tab_id: u64) {
    let budget = l.budget_ms().min(SETTLE_CALL_MS);
    let _ = l.call(
        tab_id,
        &[settle()],
        CallMode::Sequence,
        None,
        0,
        budget,
        None,
    );
}

/// Capture the visible viewport, the whole page, or one located target.
///
/// A target is measured in its own document and placed in the top viewport by the position of
/// every embedded document above it. A settle that is off, or that fails, changes nothing.
///
/// # Errors
///
/// Returns the plain failure of a target that cannot be measured, and the failure of the
/// capture.
pub fn screenshot(
    l: &mut Link<'_>,
    tab_id: u64,
    locator: Option<&str>,
    full_page: bool,
    visual_settle: Option<bool>,
) -> Result<BrowserOutcome, BrowserError> {
    let settling = visual_settle != Some(false);
    let (clip, scope) = match locator {
        Some(locator) => {
            let target = Locator::decode(locator)?;
            let tree = frame_tree(l, tab_id)?;
            let mut plans: Vec<PlannedCall> = settling.then(settle).into_iter().collect();
            let measured = plans.len();
            plans.push(PlannedCall::unscoped(
                &target.doc,
                world::box_(&target.local),
            ));
            plans.extend(tree.chain(&target.doc)?);
            let budget = l.budget_ms();
            let answers = l.call(tab_id, &plans, CallMode::Sequence, None, 0, budget, None)?;
            let measure: Prepared = value(&answers[measured])?;
            let boxes = answers[measured + 1..]
                .iter()
                .map(value::<OwnerBox>)
                .collect::<Result<Vec<_>, _>>()?;
            let origin = geometry::offset(&boxes);
            (
                CaptureClip::ViewportRect {
                    left: measure.rectangle.left + origin.x,
                    top: measure.rectangle.top + origin.y,
                    width: measure.rectangle.width.max(1.0),
                    height: measure.rectangle.height.max(1.0),
                },
                CaptureScope::Target,
            )
        }
        None => {
            if settling {
                settle_first(l, tab_id);
            }
            if full_page {
                (CaptureClip::FullPage, CaptureScope::FullPage)
            } else {
                (CaptureClip::Viewport, CaptureScope::Viewport)
            }
        }
    };
    let captured = l.send(Capture {
        tab_id,
        clip,
        budget: geometry::ordinary_budget(),
    })?;
    Ok(BrowserOutcome::Screenshot {
        tab_id,
        viewport: geometry::viewport(scope, &captured),
        mime_type: captured.mime_type,
        data: captured.data,
        width: captured.width,
        height: captured.height,
    })
}

/// Capture one region of the page, magnified, for a view the model looked at earlier.
///
/// The view the capture was taken in must be the view the model chose its region on. A view that
/// moved beyond the tolerance drops the image: it would show something other than what the model
/// pointed at.
///
/// # Errors
///
/// Returns the plain failure of a region that cannot be captured or of a view that moved, and
/// the failure of the capture.
pub fn screenshot_region(
    l: &mut Link<'_>,
    tab_id: u64,
    region: &PhysicalRectangle,
    expected: &ViewportGeometry,
    visual_settle: Option<bool>,
) -> Result<BrowserOutcome, BrowserError> {
    let clip = geometry::region_clip(region)?;
    if visual_settle != Some(false) {
        settle_first(l, tab_id);
    }
    let captured = l.send(Capture {
        tab_id,
        clip,
        budget: geometry::region_budget(),
    })?;
    geometry::check_view(&captured.view, expected)?;
    Ok(BrowserOutcome::Screenshot {
        tab_id,
        viewport: geometry::viewport(CaptureScope::Region, &captured),
        mime_type: captured.mime_type,
        data: captured.data,
        width: captured.width,
        height: captured.height,
    })
}
