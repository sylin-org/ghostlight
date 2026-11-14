// Ghostlight -- view facts and screenshots of adapter protocol 4.
(function installGhostlightCapture(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { protocol: require("./protocol.js"), sessions: require("./sessions.js"), args: require("./arguments.js") }
    : { protocol: root.GhostlightProtocol, sessions: root.GhostlightSessions, args: root.GhostlightArguments });
  root.GhostlightCapture = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightCaptureApi(deps) {
  "use strict";

  const { protocol, sessions, args: check } = deps;
  const { CDP } = sessions;
  const { invalid, isObject, requireTabId, requireInteger } = check;

  // The `view` and `capture` primitives. A view is the numbers that relate CSS pixels to the page
  // and the screen: where the visual viewport sits, how big the content is, the device pixel ratio
  // and the zoom. A capture is one JPEG of a clip of the page, sized by the budget the service sends
  // and encoded at each quality until it fits.
  //
  // A capture is taken one of two ways. A clip the visual viewport shows is photographed: one plain
  // capture of the tab's surface, which leaves the page exactly as it is, then cut to the clip,
  // scaled and encoded here, off the page. The whole page, and a clip that reaches beyond the
  // viewport, is re-rendered: Chrome is given the clip and lays the page out again to draw it,
  // which can catch a busy embedded frame squeezed or blank. A photograph is used only where its
  // geometry is certain, and anything else is re-rendered.
  //
  // While Chrome reads pixels Ghostlight's own presentation is hidden in every world, with a per-tab
  // hold count so overlapping captures stay hidden until the last one ends. The service chooses the
  // clip and the budget and reads the view facts back; this module chooses the mechanism, never
  // what to capture.

  const CLIP = Object.freeze({ VIEWPORT: "viewport", FULL_PAGE: "full_page", VIEWPORT_RECT: "viewport_rect", PAGE_RECT: "page_rect" });
  const FAILURE = protocol.RECEIPT_FAILURE;
  const MIME_JPEG = "image/jpeg";
  const MIME_PNG = "image/png";
  const FORMAT_JPEG = "jpeg";
  const FORMAT_PNG = "png";
  const MAX_QUALITY = 100;
  const TOO_LARGE = "No quality in the budget fits the size limit.";
  // The least extent of a clip, in CSS pixels.
  const MIN_EXTENT = 1;
  // How far the image's two scale ratios may differ before the measured size is not trusted.
  const SCALE_TOLERANCE = 0.02;
  // CSS pixels a clip may reach past the visual viewport and still be photographed, and that the
  // viewport may move while it is. The service allows a view the same distance before it is stale.
  const VIEW_TOLERANCE = 1;
  // How far the visual viewport's scale may be from one before the page counts as pinch zoomed.
  const PINCH_TOLERANCE = 0.001;
  // Device pixels a photograph may differ from the window's inner size and still be that window,
  // where one CSS pixel is no more than this.
  const SURFACE_TOLERANCE = 2;
  // A photograph is taken again once when the viewport moved under it, never more: after that the
  // clip is re-rendered.
  const MAX_PHOTOGRAPHS = 2;
  // What a photograph asks Chrome for: the tab's surface as it is, with no clip and nothing else
  // that makes Chrome lay the page out again. PNG is lossless, so the JPEG encoded from it here is
  // the only lossy step.
  const PHOTOGRAPH = Object.freeze({ format: FORMAT_PNG, optimizeForSpeed: true, fromSurface: true });
  // The smoothing a cut is scaled with, whether it shrinks or grows.
  const RESIZE_QUALITY = "high";
  // Bytes turned into text per call when an image becomes base64: under the engine's argument limit.
  const BINARY_CHUNK_BYTES = 0x8000;
  // What comes between the type and the text of an encoded image in a data address.
  const DATA_BASE64 = ";base64,";

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
    return { x: rect.x, y: rect.y, width: Math.max(MIN_EXTENT, rect.width), height: Math.max(MIN_EXTENT, rect.height) };
  }

  function near(left, right, tolerance) {
    return Number.isFinite(left) && Number.isFinite(right) && Math.abs(left - right) <= tolerance;
  }

  /**
   * The clip a photograph can show for a request: the clip of `clipFor`, held to the visual
   * viewport so that it is exactly what the picture will show. Null when the request has to be
   * re-rendered: it is the whole page, the page is pinch zoomed, the clip reaches more than
   * VIEW_TOLERANCE beyond the viewport or keeps less than a pixel inside it, or the held clip
   * starts left of or above the page's origin, where the clip of a photograph never does.
   */
  function photographClip(request, visual, content) {
    // Chrome reports the pinch scale; a page it does not report one for is not pinch zoomed.
    const pinched = typeof visual.scale === "number" && !near(visual.scale, 1, PINCH_TOLERANCE);
    if (request.clip === CLIP.FULL_PAGE || pinched) return null;
    const clip = clipFor(request, visual, content);
    const left = visual.pageX ?? 0;
    const top = visual.pageY ?? 0;
    const right = left + visual.clientWidth;
    const bottom = top + visual.clientHeight;
    const inside = clip.x >= left - VIEW_TOLERANCE && clip.y >= top - VIEW_TOLERANCE
      && clip.x + clip.width <= right + VIEW_TOLERANCE && clip.y + clip.height <= bottom + VIEW_TOLERANCE;
    if (!inside) return null;
    const x = Math.max(clip.x, left);
    const y = Math.max(clip.y, top);
    const width = clip.width - (x - clip.x) - Math.max(0, clip.x + clip.width - right);
    const height = clip.height - (y - clip.y) - Math.max(0, clip.y + clip.height - bottom);
    return x >= 0 && y >= 0 && width >= MIN_EXTENT && height >= MIN_EXTENT ? { x, y, width, height } : null;
  }

  /**
   * True when the page facts a photograph is checked and cut by were all read: the device pixel
   * ratio and the window's inner size. Chrome draws the main scrollbar on the right and the bottom
   * of the window whatever direction the page reads in, so the page starts at the picture's corner
   * and nothing else about the window has to be known.
   */
  function readable(geometry) {
    return isObject(geometry)
      && [geometry.devicePixelRatio, geometry.innerWidth, geometry.innerHeight].every((fact) => Number.isFinite(fact) && fact > 0);
  }

  /**
   * True when a picture is the window the page facts describe: its inner size in device pixels.
   * The inner size is a whole number of CSS pixels, so where one CSS pixel is more than
   * SURFACE_TOLERANCE device pixels the window is known to one CSS pixel and no better.
   */
  function showsWindow(picture, geometry) {
    const ratio = geometry.devicePixelRatio;
    const tolerance = Math.max(SURFACE_TOLERANCE, ratio);
    return near(picture.width, geometry.innerWidth * ratio, tolerance) && near(picture.height, geometry.innerHeight * ratio, tolerance);
  }

  /**
   * Where a page-space clip lies in a photograph, as `{ x, y, width, height }` in the picture's own
   * device pixels. A photograph starts at the visual viewport's page offset and holds `ratio`
   * device pixels for each CSS pixel. Each edge is rounded to a whole pixel on its own, so none is
   * more than half a device pixel from where the clip puts it, and the rectangle is kept inside the
   * picture. The clip ends where the viewport does, which is how a scrollbar strip on the right or
   * the bottom is left out.
   */
  function cropFor(clip, visual, ratio, picture) {
    // One axis as its first pixel and its length: `start` and `extent` are CSS pixels from the picture's corner.
    const span = (start, extent, limit) => {
      const first = Math.min(Math.max(0, Math.round(start * ratio)), limit - 1);
      const end = Math.min(Math.max(first + 1, Math.round((start + extent) * ratio)), limit);
      return [first, end - first];
    };
    const [x, width] = span(clip.x - (visual.pageX ?? 0), clip.width, picture.width);
    const [y, height] = span(clip.y - (visual.pageY ?? 0), clip.height, picture.height);
    return { x, y, width, height };
  }

  /** True when the visual viewport's origin or size differs between two reads by more than VIEW_TOLERANCE. */
  function viewportMoved(before, after) {
    return !(near(before.pageX ?? 0, after.pageX ?? 0, VIEW_TOLERANCE) && near(before.pageY ?? 0, after.pageY ?? 0, VIEW_TOLERANCE)
      && near(before.clientWidth, after.clientWidth, VIEW_TOLERANCE) && near(before.clientHeight, after.clientHeight, VIEW_TOLERANCE));
  }

  /**
   * An encoded image, given as standard base64 text, as a blob of its type. Where the engine cannot
   * turn the text into bytes itself the browser reads it as a data address, so megabytes of text
   * are never walked a character at a time here.
   */
  async function blobOf(base64, mimeType) {
    if (typeof Uint8Array.fromBase64 === "function") return new Blob([Uint8Array.fromBase64(base64)], { type: mimeType });
    return (await fetch(`data:${mimeType}${DATA_BASE64}${base64}`)).blob();
  }

  /** Bytes as standard padded base64 text. */
  function base64Of(bytes) {
    if (typeof bytes.toBase64 === "function") return bytes.toBase64();
    let binary = "";
    for (let offset = 0; offset < bytes.length; offset += BINARY_CHUNK_BYTES) {
      binary += String.fromCharCode.apply(null, bytes.subarray(offset, offset + BINARY_CHUNK_BYTES));
    }
    return btoa(binary);
  }

  /**
   * The image tool of the service worker, and the only place pixels are decoded, cut, scaled or
   * encoded. Resolves a picture of an encoded image: `width` and `height` are its size in pixels,
   * `jpeg({ crop, width, height, quality })` resolves the padded base64 of a JPEG that shows the
   * `crop` rectangle (`{ x, y, width, height }` in the picture's own pixels) at `width` by `height`
   * pixels, and `close()` gives the pixels back. A cut is scaled once and can then be encoded at
   * several qualities. Every picture that was opened must be closed. Rejects when the browser
   * cannot decode the image; `jpeg` rejects when it cannot scale or encode here.
   */
  async function openImage(base64, mimeType) {
    const bitmap = await createImageBitmap(await blobOf(base64, mimeType));
    let cut = null;
    return {
      width: bitmap.width,
      height: bitmap.height,
      async jpeg({ crop, width, height, quality }) {
        const key = [crop.x, crop.y, crop.width, crop.height, width, height].join(" ");
        if (cut?.key !== key) {
          // A cut that is already the size asked for is copied, never resampled.
          const resize = width === crop.width && height === crop.height
            ? {}
            : { resizeWidth: width, resizeHeight: height, resizeQuality: RESIZE_QUALITY };
          const scaled = await createImageBitmap(bitmap, crop.x, crop.y, crop.width, crop.height, resize);
          try {
            const canvas = new OffscreenCanvas(width, height);
            canvas.getContext("2d", { alpha: false }).drawImage(scaled, 0, 0);
            cut = { key, canvas };
          } finally {
            scaled.close();
          }
        }
        const blob = await cut.canvas.convertToBlob({ type: MIME_JPEG, quality: quality / MAX_QUALITY });
        // A canvas that cannot encode the type asked for answers with a PNG instead of failing.
        if (blob.type !== MIME_JPEG) throw new Error("The browser did not encode a JPEG.");
        return base64Of(new Uint8Array(await blob.arrayBuffer()));
      },
      close() {
        bitmap.close();
        cut = null;
      }
    };
  }

  /**
   * Creates the view and capture primitives. `lifecycle` is a GhostlightSessions instance and
   * `worldCalls` a GhostlightWorld instance. `chromeApi` supplies `tabs.getZoom`.
   * `imageTool(base64, mimeType)` opens an encoded image as `openImage` does, which is the default
   * and needs the browser's decoder and canvas.
   */
  function create({
    lifecycle,
    worldCalls,
    chromeApi = globalThis.chrome,
    imageTool = openImage
  } = {}) {
    if (!lifecycle?.cdp || !worldCalls?.withHidden) {
      throw new TypeError("capture requires debugger sessions and the world");
    }
    if (!chromeApi?.tabs?.getZoom) throw new TypeError("capture requires chrome.tabs.getZoom");
    const cdp = lifecycle.cdp;
    const rootTarget = (tabId) => ({ tabId });
    const tooLarge = () => protocol.failure(FAILURE.CAPTURE_TOO_LARGE, TOO_LARGE);

    // Reads the layout metrics once and derives the view facts. The caller holds a lease.
    async function readView(tabId) {
      const metrics = await cdp(rootTarget(tabId), CDP.PAGE_GET_LAYOUT_METRICS);
      const visual = metrics?.cssVisualViewport || metrics?.visualViewport;
      const content = metrics?.cssContentSize || metrics?.contentSize;
      if (!visual || !content) throw protocol.failure(FAILURE.OTHER, "The browser did not report the page layout.");
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
        const picture = await imageTool(base64, MIME_JPEG);
        const { width, height } = picture;
        picture.close();
        const scaleX = width / clip.width;
        const scaleY = height / clip.height;
        if (!near(scaleX, scaleY, SCALE_TOLERANCE)) throw new RangeError("screenshot output scale is inconsistent");
        return { width, height, outputScale: (scaleX + scaleY) / 2 };
      } catch (_error) {
        return { width: Math.round(clip.width * scale), height: Math.round(clip.height * scale), outputScale: scale };
      }
    }

    // What a photograph of one read would show, as `{ clip, visual, geometry, view }`, or null when
    // the request has to be re-rendered, as it is when the page facts cannot be read.
    async function photographPlan(tabId, request, read) {
      const clip = photographClip(request, read.visual, read.content);
      if (!clip) return null;
      let geometry = null;
      try {
        geometry = await worldCalls.pageGeometry(tabId);
      } catch (_error) { /* unreadable page facts leave the capture to be re-rendered */ }
      return readable(geometry) ? { clip, visual: read.visual, geometry, view: read.view } : null;
    }

    // Asks Chrome for the clip itself, once for each quality until one fits. Chrome lays the page
    // out again for every one of them. Resolves `{ shot, clip, scale, view }`.
    async function rerender(tabId, request, budget, read) {
      const clip = clipFor(request, read.visual, read.content);
      const scale = outputScale(clip.width, clip.height, budget);
      for (const quality of budget.qualities) {
        const shot = await cdp(rootTarget(tabId), CDP.PAGE_CAPTURE_SCREENSHOT,
          { format: FORMAT_JPEG, quality, clip: { ...clip, scale }, captureBeyondViewport: true, fromSurface: true });
        if (shot.data.length <= budget.max_base64_chars) return { shot, clip, scale, view: read.view };
      }
      throw tooLarge();
    }

    // Reads the pixels of a capture; the caller hides the presentation around it. Resolves
    // `{ photograph }`, a plan with its `shot`, or `{ rendered }`, what `rerender` resolves.
    //
    // A photograph shows whatever the viewport showed when Chrome copied it, so the layout is read
    // again afterwards and the photograph is kept only when the two reads agree. A viewport that
    // moved in between gets one more photograph with the fresh read. One that moved again cannot be
    // photographed truthfully, so the clip of the last read is re-rendered: a re-rendered image
    // shows the clip it was asked for wherever the viewport is.
    async function takePixels(tabId, request, budget, mayPhotograph) {
      let read = await readView(tabId);
      for (let taken = 0; mayPhotograph && taken < MAX_PHOTOGRAPHS; taken += 1) {
        const plan = await photographPlan(tabId, request, read);
        if (!plan) break;
        const shot = await cdp(rootTarget(tabId), CDP.PAGE_CAPTURE_SCREENSHOT, PHOTOGRAPH);
        const after = await readView(tabId);
        if (!viewportMoved(read.visual, after.visual)) return { photograph: { ...plan, shot } };
        read = after;
      }
      return { rendered: await rerender(tabId, request, budget, read) };
    }

    // Makes the receipt of a photograph, off the page: cuts the clip out of the picture, scales it
    // to the size the budget gives and encodes it at each quality until one fits. Resolves null
    // when the picture is not the window that was measured or the image tool fails, and the caller
    // re-renders instead.
    async function fromPhotograph({ shot, clip, visual, geometry, view }, budget) {
      const scale = outputScale(clip.width, clip.height, budget);
      const size = { width: Math.max(1, Math.round(clip.width * scale)), height: Math.max(1, Math.round(clip.height * scale)) };
      let picture = null;
      try {
        picture = await imageTool(shot.data, MIME_PNG);
        if (!showsWindow(picture, geometry)) return null;
        const crop = cropFor(clip, visual, geometry.devicePixelRatio, picture);
        for (const quality of budget.qualities) {
          const data = await picture.jpeg({ crop, ...size, quality });
          if (data.length <= budget.max_base64_chars) {
            return { mime_type: MIME_JPEG, data, ...size, output_scale: scale, clip, view };
          }
        }
      } catch (_error) {
        return null;
      } finally {
        picture?.close();
      }
      throw tooLarge();
    }

    // Makes the receipt of a re-rendered capture: Chrome's JPEG as it came, with its measured size.
    async function fromRender({ shot, clip, scale, view }) {
      const size = await measure(shot.data, clip, scale);
      return {
        mime_type: MIME_JPEG,
        data: shot.data,
        width: size.width,
        height: size.height,
        output_scale: size.outputScale,
        clip,
        view
      };
    }

    const view = check.handler({
      addresses: true,
      validate(args) {
        if (!isObject(args)) throw invalid("The request arguments must be an object.");
        requireTabId(args.tab_id);
      },
      /** Resolves `ViewFacts` for the tab: visual viewport, content size, device scale and zoom. */
      async run(args) {
        return lifecycle.withLease(args.tab_id, async () => {
          lifecycle.requireNoDialog(args.tab_id);
          return (await readView(args.tab_id)).view;
        });
      }
    });

    const capture = check.handler({
      addresses: true,
      validate(args) {
        if (!isObject(args)) throw invalid("The request arguments must be an object.");
        requireTabId(args.tab_id);
        validateClip(args.clip);
        validateBudget(args.budget);
      },
      /**
       * Resolves `Captured`: a JPEG of the clip at the budget's scale, its size, the clip in page
       * space and the view facts of the read the picture agrees with. The first quality that fits
       * `max_base64_chars` is taken, else the capture fails `capture_too_large`. Ghostlight's own
       * presentation is hidden only while Chrome reads pixels and is shown again on every path,
       * before any image is decoded or encoded.
       */
      async run(args) {
        const tabId = args.tab_id;
        const budget = args.budget;
        return lifecycle.withLease(tabId, async () => {
          const hiddenPixels = (mayPhotograph) => {
            lifecycle.requireNoDialog(tabId);
            return worldCalls.withHidden(tabId, () => takePixels(tabId, args.clip, budget, mayPhotograph));
          };
          let taken = await hiddenPixels(true);
          if (taken.photograph) {
            const receipt = await fromPhotograph(taken.photograph, budget);
            if (receipt) return receipt;
            // The photograph could not be used, so the clip is drawn the way it always was.
            taken = await hiddenPixels(false);
          }
          return fromRender(taken.rendered);
        });
      }
    });

    return Object.freeze({ view, capture });
  }

  return Object.freeze({ create, outputScale, clipFor, openImage, PHOTOGRAPH, PHOTOGRAPH_MIME: MIME_PNG });
});
