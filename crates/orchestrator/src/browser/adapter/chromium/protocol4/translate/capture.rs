//! Screenshots.
//!
//! A capture is one primitive. The extension hides Ghostlight's own presentation for its
//! duration and restores it on every exit, so the service sends no hiding calls. The service
//! decides the clip: the visible viewport, the whole page, one located target placed in the top
//! viewport, or a region of the page the model chose on an earlier screenshot. Waiting for the
//! page to settle first is best effort and never fails a capture.
//!
//! Chromium draws only the tab each window shows, so a capture of any other tab would wait for a
//! frame that never comes until the call's deadline. Every capture first asks whether its tab is
//! showing and refuses at once when it is not.

use super::super::geometry;
use super::super::link::Link;
use super::super::locator::Locator;
use super::super::scope::PlannedCall;
use super::super::wire::{CallMode, Capture, CaptureClip, ListTabs};
use super::super::world::{self, OwnerBox, Prepared};
use super::observe::{settle, settle_top};
use super::{frame_tree, value};
use crate::browser::adapter::{BrowserOutcome, CaptureScope, PhysicalRectangle, ViewportGeometry};
use crate::browser::BrowserError;

/// Refuse a capture of `tab_id` when its window shows another tab. A tab the listing does not
/// name is left to the capture's own failure.
fn require_showing(l: &mut Link<'_>, tab_id: u64) -> Result<(), BrowserError> {
    let listed = l.send(ListTabs {
        tab_ids: Some(vec![tab_id]),
    })?;
    match listed.tabs.iter().find(|tab| tab.tab_id == tab_id) {
        Some(tab) if !tab.active => Err(BrowserError::TabNotShowing),
        _ => Ok(()),
    }
}

/// Capture the visible viewport, the whole page, or one located target.
///
/// The page first gets up to `settle_ms`, the authority's preparation budget, to stop moving, so
/// what the deadline leaves past that budget is the capture's. A target is measured in its own
/// document and placed in the top viewport by the position of every embedded document above it.
/// A settle that is off, or that fails, changes nothing.
///
/// # Errors
///
/// Returns [`BrowserError::TabNotShowing`] for a tab its window does not show, the plain failure
/// of a target that cannot be measured, and the failure of the capture.
pub fn screenshot(
    l: &mut Link<'_>,
    tab_id: u64,
    locator: Option<&str>,
    full_page: bool,
    settle_ms: u64,
) -> Result<BrowserOutcome, BrowserError> {
    require_showing(l, tab_id)?;
    let (clip, scope) = match locator {
        Some(locator) => {
            let target = Locator::decode(locator)?;
            let tree = frame_tree(l, tab_id)?;
            let mut plans: Vec<PlannedCall> = (settle_ms > 0)
                .then(|| settle(settle_ms))
                .into_iter()
                .collect();
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
            settle_top(l, tab_id, settle_ms);
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

/// Capture one region of the page, magnified, for a view the model looked at earlier, after
/// up to `settle_ms` for the page to stop moving.
///
/// The view the capture was taken in must be the view the model chose its region on. A view that
/// moved beyond the tolerance drops the image: it would show something other than what the model
/// pointed at.
///
/// # Errors
///
/// Returns [`BrowserError::TabNotShowing`] for a tab its window does not show, the plain failure
/// of a region that cannot be captured or of a view that moved, and the failure of the capture.
pub fn screenshot_region(
    l: &mut Link<'_>,
    tab_id: u64,
    region: &PhysicalRectangle,
    expected: &ViewportGeometry,
    settle_ms: u64,
) -> Result<BrowserOutcome, BrowserError> {
    let clip = geometry::region_clip(region)?;
    require_showing(l, tab_id)?;
    settle_top(l, tab_id, settle_ms);
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
