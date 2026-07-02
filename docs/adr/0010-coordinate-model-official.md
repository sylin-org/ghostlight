# 0010. Screenshot coordinate model: official DPR-probe + downscale + rescale

- Status: Supersedes ADR-0009
- Date: 2026-07

## Context
The `computer` tool sends the model a screenshot and receives back pixel coordinates to
act on. Two forces must be reconciled: the image must stay within a sane token budget
(large or hi-DPI windows otherwise produce huge captures), and coordinates the model
reads off that image must map back to the pixels the browser accepts for Input dispatch.
The user's real tab must not be mutated to achieve this. The official Claude-in-Chrome
v1.0.78 solves all three without overriding device metrics; we adopt its model.

## Decision
No `Emulation.setDeviceMetricsOverride`. On each screenshot:
- `probeViewport(tabId)` runs `Runtime.evaluate` for `innerWidth`, `innerHeight`, and
  `devicePixelRatio` (no scripting permission; the debugger is already attached).
- Capture at native resolution, then downscale in an `OffscreenCanvas` to a token
  budget: `ceil(w/28)*ceil(h/28) <= 1568` tokens and longest side `<= 1568` px. Encode
  JPEG at quality 0.55, falling back to 0.30 if the base64 exceeds ~1.1 MB. If
  OffscreenCanvas is unavailable, keep the raw native capture.
- Record a per-tab ScreenshotContext `{vpW, vpH, shotW, shotH}`.

`rescaleCoord()` maps each model-provided coordinate back to CSS viewport px via
`round(v * viewportDim / screenshotDim)`, applied to `coordinate`, `start_coordinate`,
and both drag endpoints before Input dispatch. Coordinates derived from the page itself
(`getBoundingClientRect`) are already CSS px and are NOT rescaled. `resize_window` drops
the device-metrics refresh and instead invalidates stale ScreenshotContext for the
window's tabs; context is also cleared on tab removal. (Commit 9682eae;
docs/research/12 section B; CLAUDE.md Screenshot Behavior.)

## Consequences
Positive: images stay within the token budget, fixing the uncapped-image blowup of
ADR-0009 on large/hi-DPI windows; coordinates map back correctly; the user's tab is
never mutated. Typical laptop viewports fall under the budget and map 1:1 with no
downscale. Rescale math verified numerically (corner->corner, center->center) across
1280x800 to 3840x2160.

Negative / follow-ups: rescale correctness now depends on per-tab state, so a stale or
missing ScreenshotContext falls back to passthrough rounding. Residual: `zoom` still
returns a full downscaled screenshot rather than cropping its region; the region would
rescale through the same context when that lands.
