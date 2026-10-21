//! Point input: clicks, hovers, wheel turns, drags and file drops at points of a view.
//!
//! The model chose each point on a view it looked at, in page coordinates. Before anything is
//! sent, the tab's current view must still be that view. The top document then brings the point
//! into its viewport, and input goes to the viewport position it reports. A scope that leaves
//! some document unreadable has the point placed in the document it lands in, and a document
//! the scope does not allow is refused before any input; a scope that allows every document
//! skips the placing. A file drop is the exception: it is delivered inside the document, so it
//! always needs the document and the position in it.

use super::super::geometry;
use super::super::keys::{self, WheelDirection};
use super::super::link::{CallRequest, Link, View};
use super::super::scope::Scope;
use super::super::wire::Point;
use super::super::world::{self, Uploaded, ViewportPoint};
use super::pointer::{dragged, CLICK_SETTLE_MS};
use super::read::one;
use super::{input_for, route_and_admit, send_input, settled, value, OnDialog};
use crate::browser::adapter::{
    BrowserOutcome, PhysicalActionSubject, PhysicalPoint, PhysicalPointClick, PhysicalPointDrag,
    PhysicalPointDrop, PhysicalPointWheel, PhysicalViewPoint, ViewportGeometry,
};
use crate::browser::BrowserError;

/// How long the page may take to settle after a wheel turn, in milliseconds.
const WHEEL_SETTLE_MS: u64 = 120;

/// Where input must go to reach a point, and what is there.
struct Aim {
    /// The point in top-viewport coordinates.
    at: Point,
    /// The role and name of the element under the point.
    subject: Option<PhysicalActionSubject>,
}

/// Check that the tab's view is still the one the model chose its points on. Nothing has been
/// sent that could change the page.
///
/// # Errors
///
/// Returns the failure of the `view` primitive, and the plain failure of a view that moved.
fn confirm_view(
    l: &mut Link<'_>,
    tab_id: u64,
    expected: &ViewportGeometry,
) -> Result<(), BrowserError> {
    let current = l.send(View(tab_id))?;
    geometry::check_view(&current, expected)
}

/// Bring a page point into the top viewport and say where it is there. The top document
/// scrolls the point into view when it is not, which changes no page state.
///
/// # Errors
///
/// Returns the plain failure of a document the scope does not admit and of a page that cannot
/// place the point.
fn scroll_to(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    point: PhysicalPoint,
) -> Result<ViewportPoint, BrowserError> {
    let top = scope.top();
    let plan = scope.call(&top, world::scroll_point(point.x, point.y))?;
    let answers = one(l, scope, tab_id, plan)?;
    value(&answers[0])
}

/// The subject at a point of the top viewport, and its document admitted for input when the
/// scope needs the point checked. Without the check the top document's own subject stands.
///
/// # Errors
///
/// Returns what [`route_and_admit`] returns.
fn subject_at(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    placed: ViewportPoint,
) -> Result<Aim, BrowserError> {
    let at = placed.at();
    let subject = if scope.needs_point_check() {
        route_and_admit(l, scope, tab_id, at)?.subject
    } else {
        placed.subject
    };
    Ok(Aim { at, subject })
}

/// Validate the view, bring a page point into the top viewport and find where input must go.
///
/// # Errors
///
/// Returns the plain failure of a view that moved, of a point the scope refuses and of one the
/// page cannot place. Nothing has been sent as input.
fn aim(l: &mut Link<'_>, scope: &mut Scope, view: &PhysicalViewPoint) -> Result<Aim, BrowserError> {
    confirm_view(l, view.tab_id, &view.expected_viewport)?;
    let placed = scroll_to(l, scope, view.tab_id, view.point)?;
    subject_at(l, scope, view.tab_id, placed)
}

/// Click at a page point.
///
/// A dialog the click opens is the page's answer, so it counts as success.
///
/// # Errors
///
/// Returns the plain failures [`aim`] returns, and the failure of the input, which is an
/// unknown effect once the click began.
pub fn activate(
    l: &mut Link<'_>,
    scope: &mut Scope,
    click: &PhysicalPointClick,
) -> Result<BrowserOutcome, BrowserError> {
    let aim = aim(l, scope, &click.at)?;
    let steps = keys::click(
        aim.at,
        keys::mouse_button(&click.button),
        click.click_count,
        keys::modifier_mask(&click.modifiers),
    );
    let input = input_for(
        l,
        click.at.tab_id,
        None,
        steps,
        Some(scope.guard()),
        CLICK_SETTLE_MS,
    );
    send_input(l, input, OnDialog::Succeeded)?;
    Ok(BrowserOutcome::Activated {
        tab: l.tab(click.at.tab_id)?.into(),
        subject: aim.subject,
        committed_urls: l.committed(),
    })
}

/// Move the pointer to a page point.
///
/// # Errors
///
/// Returns what [`activate`] returns for the same point.
pub fn hover(
    l: &mut Link<'_>,
    scope: &mut Scope,
    view: &PhysicalViewPoint,
) -> Result<BrowserOutcome, BrowserError> {
    let aim = aim(l, scope, view)?;
    let input = input_for(
        l,
        view.tab_id,
        None,
        keys::hover(aim.at),
        Some(scope.guard()),
        0,
    );
    send_input(l, input, OnDialog::Succeeded)?;
    Ok(BrowserOutcome::Hovered {
        tab_id: view.tab_id,
        subject: aim.subject,
    })
}

/// Turn the wheel at a page point, 120 pixels a tick.
///
/// The receipt reports where the wheel turned, in top-viewport coordinates.
///
/// # Errors
///
/// Returns what [`activate`] returns for the same point.
pub fn wheel(
    l: &mut Link<'_>,
    scope: &mut Scope,
    wheel: &PhysicalPointWheel,
) -> Result<BrowserOutcome, BrowserError> {
    let aim = aim(l, scope, &wheel.at)?;
    let steps = keys::wheel(
        aim.at,
        WheelDirection::from_name(&wheel.direction),
        wheel.ticks,
    );
    let input = input_for(
        l,
        wheel.at.tab_id,
        None,
        steps,
        Some(scope.guard()),
        WHEEL_SETTLE_MS,
    );
    send_input(l, input, OnDialog::Succeeded)?;
    Ok(BrowserOutcome::Scrolled {
        tab_id: wheel.at.tab_id,
        x: aim.at.x,
        y: aim.at.y,
        subject: aim.subject,
    })
}

/// Drag between two page points.
///
/// Both ends are placed in the top viewport in one sequence, and a point outside the viewport is
/// a plain failure. The drag is not bounded to the documents at its ends, so the points are
/// placed in their documents only to name the subjects at the ends, and only when the scope
/// needs its points checked.
///
/// # Errors
///
/// Returns the plain failure of a view that moved, of a point outside the viewport or one the
/// scope refuses, and the failure of the input, which is an unknown effect once the pointer
/// pressed.
pub fn drag(
    l: &mut Link<'_>,
    scope: &mut Scope,
    request: &PhysicalPointDrag,
) -> Result<BrowserOutcome, BrowserError> {
    confirm_view(l, request.tab_id, &request.expected_viewport)?;
    let top = scope.top();
    let plans = [request.start, request.end]
        .map(|point| scope.call(&top, world::viewport_point(point.x, point.y)))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let call = CallRequest::sequence(request.tab_id, &plans).guard(scope.guard());
    let answers = l.call(call, Some(&mut *scope))?;
    let start: ViewportPoint = value(&answers[0])?;
    let end: ViewportPoint = value(&answers[1])?;
    let from = subject_at(l, scope, request.tab_id, start)?;
    let onto = subject_at(l, scope, request.tab_id, end)?;
    dragged(
        l,
        scope,
        request.tab_id,
        (from.at, onto.at),
        (from.subject, onto.subject),
    )
}

/// Drop a file at a page point.
///
/// The point is always placed in its document, because the drop is delivered there, at the
/// position in that document's own viewport. The document must be one the scope allows.
///
/// # Errors
///
/// Returns the plain failures [`aim`] returns, and the failure of the drop, which is an unknown
/// effect once it was delivered.
pub fn drop_file(
    l: &mut Link<'_>,
    scope: &mut Scope,
    request: &PhysicalPointDrop,
) -> Result<BrowserOutcome, BrowserError> {
    let view = &request.at;
    confirm_view(l, view.tab_id, &view.expected_viewport)?;
    let placed = scroll_to(l, scope, view.tab_id, view.point)?;
    let routed = route_and_admit(l, scope, view.tab_id, placed.at())?;
    let plan = scope.call(
        &routed.doc,
        world::drop_files(
            routed.local.x,
            routed.local.y,
            std::slice::from_ref(&request.file),
        ),
    )?;
    let answers = one(l, scope, view.tab_id, plan)?;
    let uploaded: Uploaded = settled(l, &answers[0])?;
    Ok(BrowserOutcome::FilesUploaded {
        tab_id: view.tab_id,
        uploaded_count: uploaded.uploaded_count,
        uploaded_bytes: uploaded.uploaded_bytes,
        subject: routed.subject.or(uploaded.subject),
    })
}
