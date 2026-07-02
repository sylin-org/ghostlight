# 0009. Screenshot coordinate model: deviceScaleFactor:1 normalization

- Status: Superseded by ADR-0010
- Date: 2026-07

## Context
The `computer` tool feeds the model a screenshot, and the model replies with pixel
coordinates it wants clicked, typed at, or dragged. Those coordinates only land on the
right element if the pixels in the screenshot map 1:1 to the pixels the browser accepts
for Input dispatch. On a hi-DPI display the two disagree: the browser renders at a
device pixel ratio above 1, so a captured pixel and a CSS pixel are not the same unit.

The reference implementation solves this by forcing the coordinate spaces to coincide.
This ADR records that original choice.

## Decision
Before capture, call `Emulation.setDeviceMetricsOverride` to pin the tab to the window
size at `deviceScaleFactor: 1`. With DPR forced to 1, the captured screenshot pixel
grid equals the CSS/Input coordinate grid, so model-supplied coordinates are dispatched
verbatim with no rescaling. `resize_window` refreshes the device-metrics override to
track the new size. This mirrors the reference implementation and is pinned as
deliberate in CLAUDE.md (commit ce93e65).

## Consequences
Positive: the coordinate math is trivial. There is none. Screenshot pixels are input
pixels, so there is no per-tab state to track and no rescale step that can drift.

Negative: the capture has no pixel cap. On a large or hi-DPI window the forced-native
capture produces a very large image, inflating token cost, and in practice the
coordinates still fail to map back cleanly on 4K/hi-DPI windows (observed against the
official Claude-in-Chrome v1.0.78; see docs/research/12 section B). Overriding device
metrics also mutates the user's real tab, which conflicts with keeping the browser
untouched.

These defects motivate ADR-0010, which removes the device-metrics override entirely in
favor of the official probe + downscale + rescale model.
