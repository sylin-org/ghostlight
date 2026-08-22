# ADR-0131: View-bound region screenshots

- Status: Accepted
- Date: 2026-08-22
- Amends: ADR-0107 Decision 1 and its `browser_screenshot` contract
- Builds on: ADR-0010, ADR-0101, and ADR-0107

## Context

Ghostlight 0.8 could crop a rectangle from a screenshot, magnify it within a bounded image budget,
and repeat the operation against the result. The clean-room 1.0 language retained screenshot view
handles for point actions but omitted this reading capability. Browser page zoom is not a
substitute: it changes the page and its layout, while region magnification returns a closer image
without mutating the document.

The old `computer` action and its `region` tuple are not part of the 1.0 language. Restoring that
signature would expose an obsolete tool family and bypass the current ownership and stale-view
model. Adding optional rectangle fields to the existing physical `screenshot` primitive would
also be unsafe. An older adapter could ignore unknown fields and return a viewport while the
orchestrator reports a region.

## Decision

### 1. Region capture is a fourth `browser_screenshot` branch

The model supplies `view`, `x`, `y`, `width`, and `height`, with optional `tab` and the ordinary
timeout and restriction fields. Coordinates are pixels in the referenced image. The rectangle
must be finite, positive in area, and wholly inside that current view. It cannot be combined with
`target` or `full_page`.

### 2. The workspace owns the transform and lifecycle

The workspace resolves the image rectangle through the source view's page origin and output scale.
It performs the same ownership, tab, document-generation, and bounds checks used by point actions.
The adapter rechecks the source view's visual viewport, device scale, and browser zoom immediately
before capture.

A successful region capture returns a normal image block and mints a new `view_` handle with the
exact region transform. As with every newer screenshot on that tab, it supersedes the prior view.
The new view can be used for another region capture, so magnification is chainable without a second
coordinate system.

### 3. Region capture is a distinct physical primitive

The browser bridge carries `screenshot_region` with a page-CSS rectangle and the expected source
viewport. It still requires the existing `capture` capability. An adapter that does not implement
the primitive fails explicitly instead of silently returning the wrong screenshot.

### 4. Magnification spends the existing bounded image budget

Viewport, full-page, and target screenshots never scale above 1. A region screenshot may scale
above 1 until one of the existing limits is reached: 2400 pixels on either side or 4,000,000 output
pixels. JPEG quality and transfer bounds remain unchanged. The extension owns this browser-specific
fidelity calculation and stores no screenshot bytes.

## Consequences

- Models use one current screenshot tool rather than an old compatibility signature.
- Region images remain governed reads and preserve the same content and audit boundaries as other
  screenshots.
- A pending or installed adapter built before this decision must be replaced by the matching 1.0
  package before region capture is available.
- Unit, wire, process, and real-browser journeys must cover the initial region and one chained
  region.

