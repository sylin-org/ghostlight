//! Coordinates, point routing and capture geometry.
//!
//! Every input the service sends is a point in the top document's viewport, but a page asset
//! only knows its own document's coordinates. This module does the arithmetic between the two:
//! where an embedded document's viewport starts inside the top one, which embedded document
//! owns a point, whether the view a screenshot was taken in is still the view now, and how big
//! a captured image may be.
//!
//! All of it is pure. The calls that gather the facts are made elsewhere.

use std::ops::{Add, Sub};

use thiserror::Error;

use super::wire::{CaptureBudget, CaptureClip, Captured, Point, ViewFacts};
use super::world::{Hit, OwnerBox, OwnerProbe};
use crate::browser::adapter::{
    CaptureScope, PhysicalActionSubject, PhysicalRectangle, ViewportGeometry,
};
use crate::browser::BrowserError;

/// The most documents deep a point is routed.
pub const MAX_ROUTE_DEPTH: usize = 8;

/// How far a view position or size may drift, in CSS pixels, and still be the same view.
const POSITION_TOLERANCE: f64 = 1.0;
/// How far the device scale may drift and still be the same view.
const DEVICE_SCALE_TOLERANCE: f64 = 0.02;
/// How far the zoom factor may drift and still be the same view.
const ZOOM_TOLERANCE: f64 = 0.001;

/// The longest side of a captured image, in pixels.
const MAX_SIDE: u32 = 2400;
/// The most pixels a captured image holds.
const MAX_PIXELS: u32 = 4_000_000;
/// The smallest scale a capture may shrink to.
const MIN_SCALE: f64 = 0.05;
/// The JPEG qualities a capture tries, best first. The second is the fallback for an image too
/// large for the best.
const JPEG_QUALITIES: [u8; 2] = [55, 30];
/// The longest base64 image a receipt may carry.
const MAX_BASE64_CHARS: usize = 6_000_000;

/// A measurement that cannot describe a capture.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GeometryError {
    /// A size that must be above zero and finite was not.
    #[error("{0} must be positive")]
    NotPositive(&'static str),
    /// A position that must be zero or above and finite was not.
    #[error("{0} must be non-negative")]
    Negative(&'static str),
}

impl From<GeometryError> for BrowserError {
    fn from(error: GeometryError) -> Self {
        Self::Primitive(error.to_string())
    }
}

impl Add for Point {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl Sub for Point {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

/// Where a document's viewport starts inside the top viewport: the sum of the content-box
/// origins of every owner element from the document up to the top, exclusive.
#[must_use]
pub fn offset(boxes: &[OwnerBox]) -> Point {
    boxes.iter().fold(Point { x: 0.0, y: 0.0 }, |sum, owner| {
        sum + Point {
            x: owner.left,
            y: owner.top,
        }
    })
}

/// What one level of point routing found.
#[derive(Clone, Debug, PartialEq)]
pub enum RouteStep<C> {
    /// The point lands in this document, on this subject.
    Subject {
        /// The role and name of the element under the point, when it has any.
        subject: Option<PhysicalActionSubject>,
    },
    /// The point lands in an embedded document, at this position inside its viewport.
    Descend {
        /// The embedded document that owns the point.
        child: C,
        /// The point in the child's own viewport.
        local: Point,
    },
    /// The point cannot be placed in exactly one document, so no input may use it.
    Unresolved,
}

/// Reduce one level of point routing.
///
/// `probes` pairs each embedded document of the current one with the answer to its owner probe,
/// or `None` when the probe got no answer. When an embedded document lies under the point,
/// exactly one probe must report the hit. Zero hits leave the point unresolved, as do several,
/// and so does an embed no probe covers, such as a frame the extension could not describe. A
/// guess would send input to a document nobody checked.
#[must_use]
pub fn route_step<C>(
    hit: &Hit,
    probes: impl IntoIterator<Item = (C, Option<OwnerProbe>)>,
) -> RouteStep<C> {
    if !hit.embed {
        return RouteStep::Subject {
            subject: hit.subject.clone(),
        };
    }
    let mut owners = probes
        .into_iter()
        .filter_map(|(child, probe)| probe.filter(|probe| probe.hit).map(|probe| (child, probe)));
    match (owners.next(), owners.next()) {
        (Some((child, probe)), None) => RouteStep::Descend {
            child,
            local: Point {
                x: hit.x - probe.left,
                y: hit.y - probe.top,
            },
        },
        _ => RouteStep::Unresolved,
    }
}

/// Whether two measurements agree within a tolerance. A measurement that is not a number never
/// agrees.
fn near(left: f64, right: f64, tolerance: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= tolerance
}

/// Check that the view a capture or input needs is still the view the model looked at.
///
/// # Errors
///
/// Returns [`BrowserError::StaleView`] when the visual viewport, device scale or zoom has moved
/// beyond the tolerance. Nothing was sent, so nothing happened.
pub fn check_view(current: &ViewFacts, expected: &ViewportGeometry) -> Result<(), BrowserError> {
    let agrees = near(
        current.visual_page_x,
        expected.visual_page_x,
        POSITION_TOLERANCE,
    ) && near(
        current.visual_page_y,
        expected.visual_page_y,
        POSITION_TOLERANCE,
    ) && near(
        current.visual_css_width,
        expected.visual_css_width,
        POSITION_TOLERANCE,
    ) && near(
        current.visual_css_height,
        expected.visual_css_height,
        POSITION_TOLERANCE,
    ) && near(
        current.device_scale,
        expected.device_scale,
        DEVICE_SCALE_TOLERANCE,
    ) && near(current.zoom, expected.zoom, ZOOM_TOLERANCE);
    if agrees {
        Ok(())
    } else {
        Err(BrowserError::StaleView)
    }
}

/// The size limits of an ordinary capture, which shrinks a large clip and never magnifies a
/// small one.
#[must_use]
pub fn ordinary_budget() -> CaptureBudget {
    budget(false)
}

/// The size limits of a region capture, which spends the whole budget on magnifying a small
/// clip.
#[must_use]
pub fn region_budget() -> CaptureBudget {
    budget(true)
}

fn budget(magnify: bool) -> CaptureBudget {
    CaptureBudget {
        max_side: MAX_SIDE,
        max_pixels: MAX_PIXELS,
        min_scale: MIN_SCALE,
        magnify,
        qualities: JPEG_QUALITIES.to_vec(),
        max_base64_chars: MAX_BASE64_CHARS,
    }
}

/// A size that is above zero and finite.
fn require_extent(value: f64, name: &'static str) -> Result<f64, GeometryError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(GeometryError::NotPositive(name))
    }
}

/// A position that is zero or above and finite.
fn require_coordinate(value: f64, name: &'static str) -> Result<f64, GeometryError> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(GeometryError::Negative(name))
    }
}

/// The scale a clip of the given CSS size is captured at: as large as the side and pixel limits
/// allow, never below the budget's floor, and never above one unless the budget magnifies.
///
/// # Errors
///
/// Returns [`GeometryError::NotPositive`] for a width or height that is not above zero.
pub fn output_scale(width: f64, height: f64, budget: &CaptureBudget) -> Result<f64, GeometryError> {
    let width = require_extent(width, "width")?;
    let height = require_extent(height, "height")?;
    let side = f64::from(budget.max_side);
    let fit = (side / width)
        .min(side / height)
        .min((f64::from(budget.max_pixels) / (width * height)).sqrt());
    let wanted = if budget.magnify { fit } else { fit.min(1.0) };
    Ok(wanted.max(budget.min_scale))
}

/// The capture clip for a region of the page.
///
/// # Errors
///
/// Returns a [`GeometryError`] for a region with a negative or missing origin or an empty size.
pub fn region_clip(region: &PhysicalRectangle) -> Result<CaptureClip, GeometryError> {
    Ok(CaptureClip::PageRect {
        x: require_coordinate(region.x, "x")?,
        y: require_coordinate(region.y, "y")?,
        width: require_extent(region.width, "width")?,
        height: require_extent(region.height, "height")?,
    })
}

/// The geometry a screenshot was taken in, as the governed view handle will carry it: the
/// region captured, the visual viewport and zoom at that moment, and the scale of the image.
#[must_use]
pub fn viewport(scope: CaptureScope, captured: &Captured) -> ViewportGeometry {
    ViewportGeometry {
        scope,
        page_x: captured.clip.x,
        page_y: captured.clip.y,
        css_width: captured.clip.width,
        css_height: captured.clip.height,
        visual_page_x: captured.view.visual_page_x,
        visual_page_y: captured.view.visual_page_y,
        visual_css_width: captured.view.visual_css_width,
        visual_css_height: captured.view.visual_css_height,
        device_scale: captured.view.device_scale,
        zoom: captured.view.zoom,
        output_scale: captured.output_scale,
    }
}

#[cfg(test)]
mod tests;
