// SPDX-License-Identifier: Apache-2.0 OR MIT
//! GIF overlay geometry, routing, and RGBA compositing (ADR-0053 Decisions 4 and 5).
//!
//! Ported from the extension's lib/gifoverlay.js + service-worker canvas draws, which the
//! thin-extension rule relocated into the binary. The overlay VOCABULARY and GEOMETRY are harvested
//! from the official Claude-in-Chrome v1.0.80 offscreen.js (drawClickIndicator/drawActionLabel/
//! drawProgressBar/applyActionIndicators): same radii, label box, edge clamping, and
//! scaleFactor = canvasWidth / viewportWidth. Deliberate divergences: overlays are Ghostlight
//! sky-blue (#38BDF8) instead of Claude coral, the watermark is a "Ghostlight" pill (not Claude's
//! logo), text renders from the embedded bitmap font (font.rs), and the reference's soft label
//! shadow is omitted (pure-buffer drawing, no canvas shadow machinery).

use serde_json::Value;

use super::font;

/// Ghostlight brand color (sky-blue #38BDF8), replacing the reference's Claude-coral overlays.
pub(crate) const BRAND_RGB: (u8, u8, u8) = (56, 189, 248);

const MIN_FRAME_DELAY_MS: i64 = 100;
const MAX_FRAME_DELAY_MS: i64 = 4000;
const LAST_FRAME_DELAY_MS: i64 = 800 + 2000;

/// Per-frame GIF delays from real capture timestamps (ADR-0052 D3 semantics). Frame i plays for
/// the time that actually elapsed until frame i+1, clamped to [100, 4000] ms; the last frame holds
/// 800 + 2000 ms (the official extension's end-of-animation viewing pause).
pub(crate) fn compute_frame_delays(timestamps_ms: &[i64]) -> Vec<u32> {
    let mut out = Vec::with_capacity(timestamps_ms.len());
    for i in 0..timestamps_ms.len() {
        if i + 1 < timestamps_ms.len() {
            let d = timestamps_ms[i + 1] - timestamps_ms[i];
            out.push(d.clamp(MIN_FRAME_DELAY_MS, MAX_FRAME_DELAY_MS) as u32);
        } else {
            out.push(LAST_FRAME_DELAY_MS as u32);
        }
    }
    out
}

/// Overlay metadata for one recorded action (ADR-0052 D4). Coordinates are CSS viewport px.
#[derive(Debug, Clone)]
pub struct ActionMeta {
    /// The action kind: a `computer` action name, or "navigate".
    pub kind: String,
    pub coordinate: Option<(f64, f64)>,
    pub start_coordinate: Option<(f64, f64)>,
    /// Human caption drawn as the on-frame label.
    pub description: String,
    /// When the action was dispatched (Unix ms). Set by the recorder, not by `describe_action`.
    pub ts_ms: i64,
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        let head: String = s.chars().take(n).collect();
        format!("{head}...")
    } else {
        s.to_string()
    }
}

fn short_url(u: &str) -> String {
    match url::Url::parse(u) {
        Ok(parsed) => match parsed.host_str() {
            Some(h) if !h.is_empty() => h.to_string(),
            _ => truncate(u, 30),
        },
        Err(_) => truncate(u, 30),
    }
}

fn coord_from(v: Option<&Value>) -> Option<(f64, f64)> {
    let arr = v?.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    Some((arr[0].as_f64()?, arr[1].as_f64()?))
}

/// Build per-frame action metadata from a dispatched tool call. Returns None for tools we do not
/// annotate. Coordinates are copied through in the caller's space; the recorder rescales them to
/// CSS viewport px before storing.
pub fn describe_action(tool: &str, args: &Value) -> Option<ActionMeta> {
    if tool == "navigate" {
        let description = match args.get("url").and_then(Value::as_str) {
            Some(u) => format!("navigate: {}", short_url(u)),
            None => "navigate".to_string(),
        };
        return Some(ActionMeta {
            kind: "navigate".to_string(),
            coordinate: None,
            start_coordinate: None,
            description,
            ts_ms: 0,
        });
    }
    if tool == "computer" {
        let action = args
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("action");
        let text = args.get("text").and_then(Value::as_str);
        let description = match (action, text) {
            ("type", Some(t)) => format!("type: {}", truncate(t, 30)),
            ("key", Some(t)) => format!("key: {t}"),
            _ => action.to_string(),
        };
        return Some(ActionMeta {
            kind: action.to_string(),
            coordinate: coord_from(args.get("coordinate")),
            start_coordinate: coord_from(args.get("start_coordinate")),
            description,
            ts_ms: 0,
        });
    }
    None
}

/// Pop the action a newly KEPT frame should carry (ADR-0052 D4): the oldest pending action whose
/// timestamp is at or before the frame's -- the frame where that action's effect first painted.
pub fn take_action_for_frame(
    pending: &mut Vec<ActionMeta>,
    frame_ts_ms: i64,
) -> Option<ActionMeta> {
    if pending.first().map(|a| a.ts_ms <= frame_ts_ms) == Some(true) {
        Some(pending.remove(0))
    } else {
        None
    }
}

/// The overlay switches resolved from the tool's open `options` object: every switch defaults to
/// true (the reference's `?? true`); only an explicit boolean `false` disables one.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OverlayOptions {
    pub click_indicators: bool,
    pub drag_paths: bool,
    pub action_labels: bool,
    pub progress_bar: bool,
    pub watermark: bool,
}

pub(crate) fn resolve_overlay_options(options: &Value) -> OverlayOptions {
    let on = |key: &str| options.get(key).and_then(Value::as_bool) != Some(false);
    OverlayOptions {
        click_indicators: on("showClickIndicators"),
        drag_paths: on("showDragPaths"),
        action_labels: on("showActionLabels"),
        progress_bar: on("showProgressBar"),
        watermark: on("showWatermark"),
    }
}

/// scaleFactor maps CSS viewport px -> canvas (frame) px. Falls back to 1 with no viewport info.
pub(crate) fn scale_factor_for(canvas_width: usize, viewport_width: Option<f64>) -> f64 {
    match viewport_width {
        Some(v) if v > 0.0 && canvas_width > 0 => canvas_width as f64 / v,
        _ => 1.0,
    }
}

/// Action-label box geometry (reference drawActionLabel): 14px font, 8px padding, 20px text
/// height, 6px corner radius, offset up-right of the anchor, edge-clamped against the right/top
/// canvas edges. `text_width` is measured by the caller.
#[derive(Debug, PartialEq)]
pub(crate) struct LabelBox {
    pub bg_x: f64,
    pub bg_y: f64,
    pub bg_w: f64,
    pub bg_h: f64,
    pub radius: f64,
    pub text_x: f64,
    pub text_y: f64,
}

pub(crate) fn label_box(x: f64, y: f64, text_width: f64, canvas_width: f64, sf: f64) -> LabelBox {
    let text_height = 20.0 * sf;
    let padding = 8.0 * sf;
    let radius = 6.0 * sf;

    let mut label_x = x + 20.0 * sf;
    let mut label_y = y - 10.0 * sf;
    if label_x + text_width + padding * 2.0 > canvas_width {
        label_x = x - text_width - padding * 2.0 - 20.0 * sf;
    }
    if label_y < 0.0 {
        label_y = y + 20.0 * sf;
    }

    LabelBox {
        bg_x: label_x,
        bg_y: label_y,
        bg_w: text_width + padding * 2.0,
        bg_h: text_height + padding,
        radius,
        text_x: label_x + padding,
        text_y: label_y + padding,
    }
}

/// Progress-bar rect (reference drawProgressBar): full width, 4px tall, bottom-anchored.
#[derive(Debug, PartialEq)]
pub(crate) struct ProgressBar {
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill_width: f64,
}

pub(crate) fn progress_bar_rect(
    canvas_width: f64,
    canvas_height: f64,
    progress: f64,
    sf: f64,
) -> ProgressBar {
    let height = 4.0 * sf;
    ProgressBar {
        y: canvas_height - height,
        width: canvas_width,
        height,
        fill_width: canvas_width * progress,
    }
}

/// Which overlays a frame gets, mirroring the reference applyActionIndicators routing:
/// click/scroll with a coordinate -> ring (+ label near it); left_click_drag with both coords ->
/// drag path (+ label near the end); type/key/wait with no coordinate -> a top-left label.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct OverlayPlan {
    pub click_ring: Option<(f64, f64)>,
    pub drag_path: Option<(f64, f64, f64, f64)>,
    pub label: Option<(String, f64, f64)>,
}

pub(crate) fn overlay_plan(meta: Option<&ActionMeta>, opts: &OverlayOptions) -> OverlayPlan {
    let mut plan = OverlayPlan::default();
    let Some(meta) = meta else { return plan };
    let is_click = meta.kind.contains("click") || meta.kind == "scroll";

    if opts.click_indicators && is_click {
        if let Some((x, y)) = meta.coordinate {
            plan.click_ring = Some((x, y));
            if opts.action_labels && !meta.description.is_empty() {
                plan.label = Some((meta.description.clone(), x, y));
            }
        }
    }

    if opts.drag_paths && meta.kind == "left_click_drag" {
        if let (Some((sx, sy)), Some((ex, ey))) = (meta.start_coordinate, meta.coordinate) {
            plan.drag_path = Some((sx, sy, ex, ey));
            if opts.action_labels && !meta.description.is_empty() {
                plan.label = Some((meta.description.clone(), ex, ey));
            }
        }
    }

    if opts.action_labels
        && !meta.description.is_empty()
        && meta.coordinate.is_none()
        && matches!(meta.kind.as_str(), "type" | "key" | "wait" | "navigate")
    {
        plan.label = Some((meta.description.clone(), 20.0, 20.0));
    }

    plan
}

// ---- RGBA drawing -------------------------------------------------------------------------

/// A mutable RGBA frame buffer plus its dimensions; all overlay drawing happens through it.
pub(crate) struct Canvas<'a> {
    pub buf: &'a mut [u8],
    pub w: usize,
    pub h: usize,
}

impl Canvas<'_> {
    fn blend_px(&mut self, x: i64, y: i64, rgb: (u8, u8, u8), a: f64) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        let inv = 1.0 - a;
        self.buf[i] = (rgb.0 as f64 * a + self.buf[i] as f64 * inv).round() as u8;
        self.buf[i + 1] = (rgb.1 as f64 * a + self.buf[i + 1] as f64 * inv).round() as u8;
        self.buf[i + 2] = (rgb.2 as f64 * a + self.buf[i + 2] as f64 * inv).round() as u8;
        self.buf[i + 3] = 255;
    }

    fn fill_circle(&mut self, cx: f64, cy: f64, r: f64, rgb: (u8, u8, u8), a: f64) {
        let (x0, x1) = ((cx - r).floor() as i64, (cx + r).ceil() as i64);
        let (y0, y1) = ((cy - r).floor() as i64, (cy + r).ceil() as i64);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                if dx * dx + dy * dy <= r * r {
                    self.blend_px(x, y, rgb, a);
                }
            }
        }
    }

    fn stroke_circle(&mut self, cx: f64, cy: f64, r: f64, lw: f64, rgb: (u8, u8, u8), a: f64) {
        let outer = r + lw / 2.0;
        let (x0, x1) = ((cx - outer).floor() as i64, (cx + outer).ceil() as i64);
        let (y0, y1) = ((cy - outer).floor() as i64, (cy + outer).ceil() as i64);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                let dist = (dx * dx + dy * dy).sqrt();
                if (dist - r).abs() <= lw / 2.0 {
                    self.blend_px(x, y, rgb, a);
                }
            }
        }
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, rgb: (u8, u8, u8), a: f64) {
        for py in y.floor() as i64..(y + h).ceil() as i64 {
            for px in x.floor() as i64..(x + w).ceil() as i64 {
                self.blend_px(px, py, rgb, a);
            }
        }
    }

    fn fill_rounded_rect(&mut self, rect: (f64, f64, f64, f64), r: f64, rgb: (u8, u8, u8), a: f64) {
        let (x, y, w, h) = rect;
        let r = r.min(w / 2.0).min(h / 2.0);
        for py in y.floor() as i64..(y + h).ceil() as i64 {
            for px in x.floor() as i64..(x + w).ceil() as i64 {
                let (fx, fy) = (px as f64 + 0.5, py as f64 + 0.5);
                if fx < x || fx > x + w || fy < y || fy > y + h {
                    continue;
                }
                // Inside unless in a corner square and outside that corner's circle.
                let cx = if fx < x + r {
                    Some(x + r)
                } else if fx > x + w - r {
                    Some(x + w - r)
                } else {
                    None
                };
                let cy = if fy < y + r {
                    Some(y + r)
                } else if fy > y + h - r {
                    Some(y + h - r)
                } else {
                    None
                };
                if let (Some(cx), Some(cy)) = (cx, cy) {
                    let (dx, dy) = (fx - cx, fy - cy);
                    if dx * dx + dy * dy > r * r {
                        continue;
                    }
                }
                self.blend_px(px, py, rgb, a);
            }
        }
    }

    fn thick_line(&mut self, from: (f64, f64), to: (f64, f64), lw: f64, rgb: (u8, u8, u8), a: f64) {
        let ((x0, y0), (x1, y1)) = (from, to);
        let half = lw / 2.0;
        let (bx0, bx1) = (
            (x0.min(x1) - half).floor() as i64,
            (x0.max(x1) + half).ceil() as i64,
        );
        let (by0, by1) = (
            (y0.min(y1) - half).floor() as i64,
            (y0.max(y1) + half).ceil() as i64,
        );
        let (vx, vy) = (x1 - x0, y1 - y0);
        let len2 = vx * vx + vy * vy;
        for y in by0..=by1 {
            for x in bx0..=bx1 {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let t = if len2 > 0.0 {
                    (((fx - x0) * vx + (fy - y0) * vy) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (dx, dy) = (fx - (x0 + t * vx), fy - (y0 + t * vy));
                if (dx * dx + dy * dy).sqrt() <= half {
                    self.blend_px(x, y, rgb, a);
                }
            }
        }
    }

    fn fill_triangle(&mut self, p: [(f64, f64); 3], rgb: (u8, u8, u8), a: f64) {
        let sign = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
            (a.0 - c.0) * (b.1 - c.1) - (b.0 - c.0) * (a.1 - c.1)
        };
        let (x0, x1) = (
            p.iter().map(|q| q.0).fold(f64::INFINITY, f64::min).floor() as i64,
            p.iter()
                .map(|q| q.0)
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil() as i64,
        );
        let (y0, y1) = (
            p.iter().map(|q| q.1).fold(f64::INFINITY, f64::min).floor() as i64,
            p.iter()
                .map(|q| q.1)
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil() as i64,
        );
        for y in y0..=y1 {
            for x in x0..=x1 {
                let pt = (x as f64 + 0.5, y as f64 + 0.5);
                let (d1, d2, d3) = (
                    sign(pt, p[0], p[1]),
                    sign(pt, p[1], p[2]),
                    sign(pt, p[2], p[0]),
                );
                let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
                let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
                if !(has_neg && has_pos) {
                    self.blend_px(x, y, rgb, a);
                }
            }
        }
    }

    fn draw_text(&mut self, x: f64, y: f64, text: &str, scale: usize, rgb: (u8, u8, u8), a: f64) {
        let mut pen_x = x.round() as i64;
        let pen_y = y.round() as i64;
        for &ch in text.as_bytes() {
            let g = font::glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..font::GLYPH_SIZE {
                    if bits >> col & 1 == 1 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                self.blend_px(
                                    pen_x + (col * scale + sx) as i64,
                                    pen_y + (row * scale + sy) as i64,
                                    rgb,
                                    a,
                                );
                            }
                        }
                    }
                }
            }
            pen_x += (font::GLYPH_SIZE * scale) as i64;
        }
    }
}

/// Integer glyph scale approximating the reference's `px` font size at overlay scale `sf`.
fn glyph_scale(px: f64, sf: f64) -> usize {
    ((px * sf / font::GLYPH_SIZE as f64).round() as usize).max(1)
}

fn draw_click_indicator(c: &mut Canvas, x: f64, y: f64, sf: f64) {
    // Reference geometry: outer glow r=15 (0.3), inner fill r=11 (0.5), border r=11 stroke lw=2.
    c.fill_circle(x, y, 15.0 * sf, BRAND_RGB, 0.3);
    c.fill_circle(x, y, 11.0 * sf, BRAND_RGB, 0.5);
    c.stroke_circle(x, y, 11.0 * sf, 2.0 * sf, BRAND_RGB, 1.0);
}

fn draw_drag_path(c: &mut Canvas, sx: f64, sy: f64, ex: f64, ey: f64, sf: f64) {
    c.thick_line((sx, sy), (ex, ey), 3.0 * sf, BRAND_RGB, 1.0);
    let angle = (ey - sy).atan2(ex - sx);
    let arrow = 15.0 * sf;
    c.fill_triangle(
        [
            (ex, ey),
            (
                ex - arrow * (angle - std::f64::consts::FRAC_PI_6).cos(),
                ey - arrow * (angle - std::f64::consts::FRAC_PI_6).sin(),
            ),
            (
                ex - arrow * (angle + std::f64::consts::FRAC_PI_6).cos(),
                ey - arrow * (angle + std::f64::consts::FRAC_PI_6).sin(),
            ),
        ],
        BRAND_RGB,
        1.0,
    );
    for (mx, my) in [(sx, sy), (ex, ey)] {
        c.fill_circle(mx, my, 6.0 * sf, (255, 255, 255), 1.0);
        c.stroke_circle(mx, my, 6.0 * sf, 2.0 * sf, BRAND_RGB, 1.0);
    }
}

fn draw_action_label(c: &mut Canvas, text: &str, x: f64, y: f64, sf: f64) {
    let scale = glyph_scale(14.0, sf);
    let text_width = font::text_width(text, scale) as f64;
    let b = label_box(x, y, text_width, c.w as f64, sf);
    c.fill_rounded_rect((b.bg_x, b.bg_y, b.bg_w, b.bg_h), b.radius, (0, 0, 0), 0.85);
    c.draw_text(b.text_x, b.text_y, text, scale, (255, 255, 255), 1.0);
}

fn draw_progress_bar(c: &mut Canvas, progress: f64, sf: f64) {
    let bar = progress_bar_rect(c.w as f64, c.h as f64, progress, sf);
    c.fill_rect(0.0, bar.y, bar.width, bar.height, (0, 0, 0), 0.3);
    c.fill_rect(0.0, bar.y, bar.fill_width, bar.height, BRAND_RGB, 1.0);
}

fn draw_watermark(c: &mut Canvas, sf: f64) {
    // Ghostlight watermark: a compact sky-blue rounded pill with white "Ghostlight" text,
    // bottom-right. Replaces the reference's Claude-logo Path2D (we do not draw Claude's mark).
    let label = "Ghostlight";
    let scale = glyph_scale(12.0, sf);
    let font_h = (font::GLYPH_SIZE * scale) as f64;
    let (pad_x, pad_y, pad) = (8.0 * sf, 5.0 * sf, 8.0 * sf);
    let tw = font::text_width(label, scale) as f64;
    let (w, h) = (tw + pad_x * 2.0, font_h + pad_y * 2.0);
    let x = c.w as f64 - pad - w;
    let y = c.h as f64 - pad - h - 4.0 * sf;
    c.fill_rounded_rect((x, y, w, h), h / 2.0, BRAND_RGB, 0.92);
    c.draw_text(x + pad_x, y + pad_y, label, scale, (255, 255, 255), 1.0);
}

/// Composite the overlays for one frame: the action's indicators (ring / drag path / label), the
/// progress bar, and the watermark, per the resolved options. `vp_w` is the frame's CSS viewport
/// width (scaleFactor = canvas.w / vp_w).
pub(crate) fn composite_overlays(
    canvas: &mut Canvas,
    meta: Option<&ActionMeta>,
    opts: &OverlayOptions,
    progress: f64,
    vp_w: Option<f64>,
) {
    let sf = scale_factor_for(canvas.w, vp_w);
    let plan = overlay_plan(meta, opts);
    if let Some((x, y)) = plan.click_ring {
        draw_click_indicator(canvas, x * sf, y * sf, sf);
    }
    if let Some((sx, sy, ex, ey)) = plan.drag_path {
        draw_drag_path(canvas, sx * sf, sy * sf, ex * sf, ey * sf, sf);
    }
    if let Some((text, x, y)) = &plan.label {
        draw_action_label(canvas, text, x * sf, y * sf, sf);
    }
    if opts.progress_bar {
        draw_progress_bar(canvas, progress, sf);
    }
    if opts.watermark {
        draw_watermark(canvas, sf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meta(
        kind: &str,
        coord: Option<(f64, f64)>,
        start: Option<(f64, f64)>,
        desc: &str,
    ) -> ActionMeta {
        ActionMeta {
            kind: kind.to_string(),
            coordinate: coord,
            start_coordinate: start,
            description: desc.to_string(),
            ts_ms: 0,
        }
    }

    #[test]
    fn compute_frame_delays_clamps_and_holds() {
        // The ported JS oracle: 250 kept; 50 clamps up to 100; 7700 clamps down to 4000; the last
        // frame always plays 2800 ms.
        assert_eq!(
            compute_frame_delays(&[1000, 1250, 1300, 9000]),
            vec![250, 100, 4000, 2800]
        );
        assert_eq!(compute_frame_delays(&[5000]), vec![2800]);
        assert_eq!(compute_frame_delays(&[]), Vec::<u32>::new());
        // A non-monotonic clock clamps up to the minimum instead of going backwards.
        assert_eq!(compute_frame_delays(&[2000, 1500]), vec![100, 2800]);
    }

    #[test]
    fn take_action_for_frame_pops_the_oldest_due_action_once() {
        let mut q = vec![
            ActionMeta {
                ts_ms: 100,
                ..meta("left_click", None, None, "a")
            },
            ActionMeta {
                ts_ms: 300,
                ..meta("type", None, None, "b")
            },
        ];
        assert!(
            take_action_for_frame(&mut q, 50).is_none(),
            "nothing due before the first action"
        );
        assert_eq!(
            take_action_for_frame(&mut q, 150).unwrap().kind,
            "left_click"
        );
        assert!(
            take_action_for_frame(&mut q, 150).is_none(),
            "the next action is not due yet"
        );
        assert_eq!(take_action_for_frame(&mut q, 400).unwrap().kind, "type");
        assert!(
            take_action_for_frame(&mut q, 500).is_none(),
            "queue drained"
        );
    }

    #[test]
    fn describe_action_builds_metadata() {
        let click = describe_action(
            "computer",
            &json!({"action":"left_click","coordinate":[100,200]}),
        )
        .unwrap();
        assert_eq!(click.kind, "left_click");
        assert_eq!(click.coordinate, Some((100.0, 200.0)));
        assert_eq!(click.description, "left_click");

        let drag = describe_action(
            "computer",
            &json!({"action":"left_click_drag","start_coordinate":[10,20],"coordinate":[30,40]}),
        )
        .unwrap();
        assert_eq!(drag.start_coordinate, Some((10.0, 20.0)));
        assert_eq!(drag.coordinate, Some((30.0, 40.0)));

        let typed =
            describe_action("computer", &json!({"action":"type","text":"hello world"})).unwrap();
        assert_eq!(typed.description, "type: hello world");
        assert!(typed.coordinate.is_none());

        let long =
            describe_action("computer", &json!({"action":"type","text":"x".repeat(50)})).unwrap();
        assert!(long.description.ends_with("..."), "long text is truncated");

        let key = describe_action("computer", &json!({"action":"key","text":"Enter"})).unwrap();
        assert_eq!(key.description, "key: Enter");

        let nav =
            describe_action("navigate", &json!({"url":"https://example.com/path?q=1"})).unwrap();
        assert_eq!(nav.kind, "navigate");
        assert_eq!(nav.description, "navigate: example.com");

        assert!(
            describe_action("read_page", &json!({})).is_none(),
            "un-annotated tools return None"
        );
    }

    #[test]
    fn options_default_true_and_disable_only_on_false() {
        let all = resolve_overlay_options(&Value::Null);
        assert!(
            all.click_indicators
                && all.drag_paths
                && all.action_labels
                && all.progress_bar
                && all.watermark
        );
        let some =
            resolve_overlay_options(&json!({"showWatermark": false, "showProgressBar": false}));
        assert!(!some.watermark && !some.progress_bar && some.click_indicators);
        // A non-boolean value is not a literal false, so the switch stays enabled.
        let odd = resolve_overlay_options(&json!({"showClickIndicators": 0}));
        assert!(odd.click_indicators);
    }

    #[test]
    fn label_box_offsets_and_edge_clamps() {
        // Ported oracles: comfortably inside -> right of the anchor (x + 20*sf), width + 2*padding.
        let inside = label_box(100.0, 100.0, 50.0, 1000.0, 1.0);
        assert_eq!(inside.bg_x, 120.0);
        assert_eq!(inside.bg_w, 50.0 + 16.0);
        assert_eq!(inside.text_x, 128.0);
        // Near the right edge: flips left of the anchor.
        let edge = label_box(980.0, 100.0, 50.0, 1000.0, 1.0);
        assert!(edge.bg_x < 980.0);
        // Near the top edge: drops below the anchor instead of going negative.
        let top = label_box(100.0, 5.0, 50.0, 1000.0, 1.0);
        assert!(top.bg_y >= 0.0);
    }

    #[test]
    fn progress_bar_rect_oracle() {
        let bar = progress_bar_rect(400.0, 300.0, 0.25, 1.0);
        assert_eq!(
            bar,
            ProgressBar {
                y: 296.0,
                width: 400.0,
                height: 4.0,
                fill_width: 100.0
            }
        );
    }

    #[test]
    fn overlay_plan_routes_by_action_type() {
        let opts = resolve_overlay_options(&Value::Null);
        // Click -> ring + label near it.
        let click = overlay_plan(
            Some(&meta("left_click", Some((50.0, 60.0)), None, "left_click")),
            &opts,
        );
        assert_eq!(click.click_ring, Some((50.0, 60.0)));
        assert!(click.drag_path.is_none());
        assert_eq!(click.label, Some(("left_click".to_string(), 50.0, 60.0)));
        // Drag -> path (+ ring, since the reference's click branch also fires on "click" kinds).
        let drag = overlay_plan(
            Some(&meta(
                "left_click_drag",
                Some((3.0, 4.0)),
                Some((1.0, 2.0)),
                "drag",
            )),
            &opts,
        );
        assert_eq!(drag.drag_path, Some((1.0, 2.0, 3.0, 4.0)));
        assert_eq!(drag.click_ring, Some((3.0, 4.0)));
        // Typing -> top-left label only.
        let typed = overlay_plan(Some(&meta("type", None, None, "type: hi")), &opts);
        assert!(typed.click_ring.is_none());
        assert_eq!(typed.label, Some(("type: hi".to_string(), 20.0, 20.0)));
        // Gating: indicators off -> no ring and no ring-riding label.
        let gated = overlay_plan(
            Some(&meta("left_click", Some((5.0, 5.0)), None, "x")),
            &resolve_overlay_options(&json!({"showClickIndicators": false})),
        );
        assert!(gated.click_ring.is_none() && gated.label.is_none());
        // Labels off -> ring stays, label drops.
        let no_label = overlay_plan(
            Some(&meta("left_click", Some((5.0, 5.0)), None, "x")),
            &resolve_overlay_options(&json!({"showActionLabels": false})),
        );
        assert_eq!(no_label.click_ring, Some((5.0, 5.0)));
        assert!(no_label.label.is_none());
        // No metadata -> empty plan.
        assert_eq!(overlay_plan(None, &opts), OverlayPlan::default());
    }

    #[test]
    fn drawing_touches_only_expected_pixels() {
        let (w, h) = (60usize, 40usize);
        let mut buf = vec![0u8; w * h * 4];
        let mut c = Canvas {
            buf: &mut buf,
            w,
            h,
        };
        c.fill_circle(30.0, 20.0, 5.0, (255, 0, 0), 1.0);
        let px = |b: &[u8], x: usize, y: usize| b[(y * w + x) * 4];
        assert_eq!(px(c.buf, 30, 20), 255, "center painted");
        assert_eq!(px(c.buf, 30, 5), 0, "far pixel untouched");

        // The watermark paints the bottom-right region, never the top-left.
        let mut buf2 = vec![0u8; w * h * 4];
        let mut c2 = Canvas {
            buf: &mut buf2,
            w,
            h,
        };
        draw_watermark(&mut c2, 0.25); // small sf keeps the pill inside the tiny canvas
        assert_eq!(px(c2.buf, 0, 0), 0, "top-left untouched");
        assert!(c2.buf.iter().any(|&b| b != 0), "something was painted");
    }
}
