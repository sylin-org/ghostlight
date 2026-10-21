// Ghostlight -- view facts and screenshots of adapter protocol 4.
(function installGhostlightCapture(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { sessions: require("./sessions.js"), args: require("./arguments.js") }
    : { sessions: root.GhostlightSessions, args: root.GhostlightArguments });
  root.GhostlightCapture = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightCaptureApi(deps) {
  "use strict";

  const { sessions, args: check } = deps;
  const { CDP } = sessions;
  const { invalid, isObject, requireTabId, requireInteger } = check;

  // The `view` and `capture` primitives. A view is the numbers that relate CSS pixels to the page
  // and the screen: where the visual viewport sits, how big the content is, the device pixel ratio
  // and the zoom. A capture is one JPEG of a clip of the page, sized by the budget the service sends
  // and tried at each quality until it fits. While a capture runs Ghostlight's own presentation is
  // hidden in every world, with a per-tab hold count so overlapping captures stay hidden until the
  // last one ends. The service chooses the clip and the budget and reads the view facts back; this
  // module decides nothing about what to capture.

  const CLIP = Object.freeze({ VIEWPORT: "viewport", FULL_PAGE: "full_page", VIEWPORT_RECT: "viewport_rect", PAGE_RECT: "page_rect" });
  const FAILURE = Object.freeze({
    CAPTURE_TOO_LARGE: "capture_too_large",
    OTHER: "other"
  });
  const MIME_JPEG = "image/jpeg";
  const FORMAT_JPEG = "jpeg";
  const MAX_QUALITY = 100;
  // How far the image's two scale ratios may differ before the measured size is not trusted.
  const SCALE_TOLERANCE = 0.02;

  function requireNumber(value, name, { minimum = -Infinity, exclusive = false } = {}) {
    if (typeof value !== "number" || !Number.isFinite(value) || (exclusive ? value <= minimum : value < minimum)) {
      throw invalid(`${name} must be a number ${exclusive ? "above" : "of at least"} ${minimum}.`);
    }
  }

  function validateClip(clip) {
    if (!isObject(clip)) throw invalid("clip must be an object.");
    if (clip.clip === CLIP.VIEWPORT || clip.clip === CLIP.FULL_PAGE) return;
    if (clip.clip === CLIP.VIEWPORT_RECT) {
      requireNumber(clip.left, "clip.left");
      requireNumber(clip.top, "clip.top");
    } else if (clip.clip === CLIP.PAGE_RECT) {
      requireNumber(clip.x, "clip.x", { minimum: 0 });
      requireNumber(clip.y, "clip.y", { minimum: 0 });
    } else {
      throw invalid("clip must name a known clip.");
    }
    requireNumber(clip.width, "clip.width", { exclusive: true, minimum: 0 });
    requireNumber(clip.height, "clip.height", { exclusive: true, minimum: 0 });
  }

  function validateBudget(budget) {
    if (!isObject(budget)) throw invalid("budget must be an object.");
    requireInteger(budget.max_side, "budget.max_side", { minimum: 1 });
    requireInteger(budget.max_pixels, "budget.max_pixels", { minimum: 1 });
    requireNumber(budget.min_scale, "budget.min_scale", { exclusive: true, minimum: 0 });
    if (typeof budget.magnify !== "boolean") throw invalid("budget.magnify must be true or false.");
    requireInteger(budget.max_base64_chars, "budget.max_base64_chars", { minimum: 1 });
    if (!Array.isArray(budget.qualities) || budget.qualities.length === 0) throw invalid("budget.qualities must list at least one quality.");
    for (const quality of budget.qualities) {
      if (!Number.isSafeInteger(quality) || quality < 1 || quality > MAX_QUALITY) throw invalid("budget.qualities must be whole numbers from 1 to 100.");
    }
  }

  /**
   * The scale a clip of the given CSS size is captured at: as large as the side and pixel limits of
   * the budget allow, never below its floor, and never above one unless the budget magnifies.
   */
  function outputScale(width, height, budget) {
    const fit = Math.min(budget.max_side / width, budget.max_side / height, Math.sqrt(budget.max_pixels / (width * height)));
    return Math.max(budget.min_scale, budget.magnify ? fit : Math.min(1, fit));
  }

  /**
   * The page-space clip a request names, from the layout metrics of the same read. The result is
   * `{ x, y, width, height }` with extents of at least one pixel.
   */
  function clipFor(request, visual, content) {
    const pageX = visual.pageX ?? 0;
    const pageY = visual.pageY ?? 0;
    let rect;
    if (request.clip === CLIP.FULL_PAGE) rect = { x: 0, y: 0, width: content.width, height: content.height };
    else if (request.clip === CLIP.VIEWPORT_RECT) {
      rect = { x: Math.max(0, pageX + request.left), y: Math.max(0, pageY + request.top), width: request.width, height: request.height };
    } else if (request.clip === CLIP.PAGE_RECT) rect = { x: request.x, y: request.y, width: request.width, height: request.height };
    else rect = { x: pageX, y: pageY, width: visual.clientWidth, height: visual.clientHeight };
    return { x: rect.x, y: rect.y, width: Math.max(1, rect.width), height: Math.max(1, rect.height) };
  }

  function near(left, right, tolerance) {
    return Number.isFinite(left) && Number.isFinite(right) && Math.abs(left - right) <= tolerance;
  }

  /** Measures an encoded JPEG: resolves `{ width, height }` in pixels. Rejects when the browser cannot decode it. */
  async function decodeImageSize(base64) {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
    const bitmap = await createImageBitmap(new Blob([bytes], { type: MIME_JPEG }));
    const size = { width: bitmap.width, height: bitmap.height };
    bitmap.close();
    return size;
  }

  /**
   * Creates the view and capture primitives. `lifecycle` is a GhostlightSessions instance and
   * `worldCalls` a GhostlightWorld instance. `chromeApi` supplies `tabs.getZoom`. `imageSize(base64)`
   * measures an encoded image (default: the browser's decoder).
   */
  function create({
    lifecycle,
    worldCalls,
    chromeApi = globalThis.chrome,
    imageSize = decodeImageSize
  } = {}) {
    if (!lifecycle?.cdp || !worldCalls?.withHidden) {
      throw new TypeError("capture requires debugger sessions and the world");
    }
    if (!chromeApi?.tabs?.getZoom) throw new TypeError("capture requires chrome.tabs.getZoom");
    const cdp = lifecycle.cdp;
    const rootTarget = (tabId) => ({ tabId });

    function requireNoDialog(tabId) {
      if (lifecycle.currentDialog(tabId)) throw sessions.dialogOpen();
    }

    // Reads the layout metrics once and derives the view facts. The caller holds a lease.
    async function readView(tabId) {
      const metrics = await cdp(rootTarget(tabId), CDP.PAGE_GET_LAYOUT_METRICS);
      const visual = metrics?.cssVisualViewport || metrics?.visualViewport;
      const content = metrics?.cssContentSize || metrics?.contentSize;
      if (!visual || !content) throw sessions.failure(FAILURE.OTHER, "The browser did not report the page layout.");
      const view = {
        visual_page_x: visual.pageX ?? 0,
        visual_page_y: visual.pageY ?? 0,
        visual_css_width: visual.clientWidth,
        visual_css_height: visual.clientHeight,
        content_css_width: content.width,
        content_css_height: content.height,
        device_scale: Number(await worldCalls.devicePixelRatio(tabId)) || 1,
        zoom: await chromeApi.tabs.getZoom(tabId)
      };
      return { visual, content, view };
    }

    // The size of the encoded image and the scale it really has. A measured scale that disagrees
    // between its axes is not trusted; the requested scale stands in.
    async function measure(base64, clip, scale) {
      try {
        const { width, height } = await imageSize(base64);
        const scaleX = width / clip.width;
        const scaleY = height / clip.height;
        if (!near(scaleX, scaleY, SCALE_TOLERANCE)) throw new RangeError("screenshot output scale is inconsistent");
        return { width, height, outputScale: (scaleX + scaleY) / 2 };
      } catch (_error) {
        return { width: Math.round(clip.width * scale), height: Math.round(clip.height * scale), outputScale: scale };
      }
    }

    const view = Object.freeze({
      addresses: () => true,
      tabOf: (args) => args.tab_id,
      validate(args) {
        if (!isObject(args)) throw invalid("The request arguments must be an object.");
        requireTabId(args.tab_id);
      },
      /** Resolves `ViewFacts` for the tab: visual viewport, content size, device scale and zoom. */
      async run(args) {
        return lifecycle.withLease(args.tab_id, async () => {
          requireNoDialog(args.tab_id);
          return (await readView(args.tab_id)).view;
        });
      }
    });

    const capture = Object.freeze({
      addresses: () => true,
      tabOf: (args) => args.tab_id,
      validate(args) {
        if (!isObject(args)) throw invalid("The request arguments must be an object.");
        requireTabId(args.tab_id);
        validateClip(args.clip);
        validateBudget(args.budget);
      },
      /**
       * Resolves `Captured`: a JPEG of the clip at the budget's scale, its measured size, the clip in
       * page space and the view facts of the same read. Tries each quality in order and takes the
       * first that fits `max_base64_chars`, else fails `capture_too_large`. Ghostlight's own
       * presentation is hidden for the duration and shown again in `finally`, on every path.
       */
      async run(args) {
        const tabId = args.tab_id;
        const budget = args.budget;
        return lifecycle.withLease(tabId, async () => {
          requireNoDialog(tabId);
          return worldCalls.withHidden(tabId, async () => {
            const { visual, content, view: facts } = await readView(tabId);
            const clip = clipFor(args.clip, visual, content);
            const scale = outputScale(clip.width, clip.height, budget);
            let shot = null;
            for (const quality of budget.qualities) {
              const attempt = await cdp(rootTarget(tabId), CDP.PAGE_CAPTURE_SCREENSHOT,
                { format: FORMAT_JPEG, quality, clip: { ...clip, scale }, captureBeyondViewport: true, fromSurface: true });
              if (attempt.data.length <= budget.max_base64_chars) {
                shot = attempt;
                break;
              }
            }
            if (!shot) throw sessions.failure(FAILURE.CAPTURE_TOO_LARGE, "No quality in the budget fits the size limit.");
            const size = await measure(shot.data, clip, scale);
            return {
              mime_type: MIME_JPEG,
              data: shot.data,
              width: size.width,
              height: size.height,
              output_scale: size.outputScale,
              clip,
              view: facts
            };
          });
        });
      }
    });

    return Object.freeze({ view, capture });
  }

  return Object.freeze({ create, outputScale, clipFor, decodeImageSize });
});
