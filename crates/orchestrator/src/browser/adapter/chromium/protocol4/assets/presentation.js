// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- policy-free rendering of the established product visual language. Included by
// world.js.
//
// The page cannot reach this module. Its markup is built with DOM calls, never an HTML string, so
// pages that enforce Trusted Types still render it. Every timer and animation frame it starts is
// tracked, and dispose removes the host and stops them all.
const presentation = (() => {
  const ROOT_ID = "ghostlight-presentation-root";
  // A replacement runtime cannot reach a predecessor's closed shadow root. Retire its
  // owned host immediately so old controls cannot survive until the next page signal.
  for (const previous of document.querySelectorAll(`#${ROOT_ID}`)) previous.remove();

  const SKY = "#38bdf8";
  const SKY_RGB = "56,189,248";
  const INK = "#eaf6ff";
  const GROUND = "#0c0f14";
  const SPRING = "cubic-bezier(.22,1,.36,1)";
  const DENIAL_MS = 5000;
  const CLICK_STAGGER_MS = 150;
  // The service admits at most 30 seconds of work. This browser-local fallback leaves
  // room for terminal delivery, then retires decoration whose owner disappeared.
  const STALE_SIGNATURE_MS = 35_000;
  // A confirming signature completes on its own after this long.
  const CONFIRMING_SIGNATURE_MS = 900;
  // A completing signature plays its glint for this long, then leaves.
  const SIGNATURE_COMPLETE_MS = 180;
  // A leaving signature is removed once its 420ms exit transition has run, with a margin.
  const SIGNATURE_LEAVE_MS = 460;
  // How long a caption stays on screen.
  const CAPTION_MS = 1500;

  // The signal kinds and activities the service sends. golden.rs reads both tables and holds
  // them equal to the service's names, so a kind or activity this page does not know fails a
  // test instead of drawing nothing.
  const SIGNAL = Object.freeze({
    start: "start",
    target: "target",
    progress: "progress",
    completion: "completion",
    denial: "denial"
  });
  const ACTIVITY = Object.freeze({
    quiet: "quiet",
    navigate: "navigate",
    click: "click",
    hover: "hover",
    drag: "drag",
    type: "type",
    key: "key",
    scroll: "scroll",
    read: "read",
    find: "find",
    screenshot: "screenshot",
    zoom: "zoom",
    fill: "fill",
    upload: "upload",
    script: "script",
    wait: "wait",
    dialog: "dialog"
  });

  // The transient effect vocabulary. One row per treatment, and the row is the whole truth
  // about it: `selector` enrolls it for reduced motion, and `beat` is how long it is visibly
  // on screen, which is what its teardown is derived from.
  //
  // Adding a row is what gives a treatment reduced-motion coverage and a lifetime. Nothing about
  // an effect is hand-maintained anywhere else, so a new one cannot keep animating for someone
  // who asked it not to, and cannot be torn down at a number somebody guessed.
  //
  // `beat` covers the treatment's full visible span, including any internal stagger: the scroll
  // chevrons animate for 900ms behind a 200ms cascade, so they occupy 1100ms.
  // A row without a `beat` loops inside a signature medallion and is cleared by that signature's
  // lifecycle rather than by a timer of its own.
  const CLEANUP_GRACE_MS = 100;
  const TRANSIENT_EFFECTS = Object.freeze([
    { selector: "trail-dot", beat: 520 },
    { selector: "field-shimmer", beat: 900 },
    { selector: "field-splash", beat: 700 },
    { selector: "target-glow", beat: 720 },
    { selector: "ripple", beat: 620 },
    { selector: "chevrons svg", effect: "chevrons", beat: 1100 },
    { selector: "read-scan", beat: 1450 },
    { selector: "nav-pill", beat: 1600 },
    { selector: "key-lozenge", beat: 1250 },
    { selector: "capture-flash", beat: 260 },
    { selector: "capture-frame", beat: 1500 },
    { selector: "zoom-frame", beat: 1150 },
    { selector: "workwheel" },
    { selector: "particle" },
    { selector: "keyboard" },
    { selector: "wait-lights span" },
    { selector: "lens" },
    { selector: "glint" }
  ]);

  // The ripple keeps its own reduced-motion rule, so it is enrolled for a beat but not for the
  // generated fade selector.
  const REDUCED_FADE_SELECTOR = TRANSIENT_EFFECTS.filter((row) => row.selector !== "ripple")
    .map((row) => `.${row.selector}`)
    .join(",");

  const EFFECT_BEATS = new Map(
    TRANSIENT_EFFECTS.filter((row) => typeof row.beat === "number").map((row) => [
      row.effect || row.selector,
      row.beat
    ])
  );

  // A treatment is torn down one grace period after it stops being visible. Deriving this from
  // the row keeps the animation and its cleanup from drifting apart.
  function lifetimeFor(className) {
    const beat = EFFECT_BEATS.get(className);
    return (beat || 0) + CLEANUP_GRACE_MS;
  }

  // Identity values reach the stylesheet once, as custom properties. Everything below is then
  // static CSS with no interpolation, so the vocabulary reads as a dictionary rather than a
  // template, and a colour or curve changes in exactly one place.
  const TOKENS = `--gl-sky:${SKY};--gl-argb:${SKY_RGB};--gl-ink:${INK};` +
    `--gl-ground:${GROUND};--gl-spring:${SPRING}`;
  const visualIdentity = Object.freeze({
    sky: SKY,
    ink: INK,
    ground: GROUND,
    spring: SPRING,
    cursor_ms: 150,
    border_breathe_ms: 4000,
    ripple_ms: EFFECT_BEATS.get("ripple"),
    field_splash_ms: EFFECT_BEATS.get("field-splash"),
    read_scan_ms: EFFECT_BEATS.get("read-scan"),
    navigation_ms: EFFECT_BEATS.get("nav-pill"),
    screenshot_ms: EFFECT_BEATS.get("capture-frame"),
    zoom_ms: EFFECT_BEATS.get("zoom-frame"),
    denial_ms: DENIAL_MS
  });

  // Every timer and animation frame this module starts, so dispose can stop them all.
  const timers = new Set();
  const frames = new Set();

  function later(callback, delay) {
    const id = setTimeout(() => {
      timers.delete(id);
      callback();
    }, delay);
    timers.add(id);
    return id;
  }

  function unlater(id) {
    clearTimeout(id);
    timers.delete(id);
  }

  function nextFrame(callback) {
    const id = requestAnimationFrame(() => {
      frames.delete(id);
      callback();
    });
    frames.add(id);
    return id;
  }

  // SVG is built node by node: an HTML string assigned to innerHTML is refused on pages that
  // require Trusted Types.
  const SVG_NAMESPACE = "http://www.w3.org/2000/svg";

  function svgElement(tag, attributes = {}, children = []) {
    const element = document.createElementNS(SVG_NAMESPACE, tag);
    for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, String(value));
    element.append(...children);
    return element;
  }

  let host = null;
  let surface = null;
  let scope = null;
  let cursor = null;
  let fxLayer = null;
  let caption = null;
  let signatureLayer = null;
  let denialLayer = null;
  let signature = null;
  let signatureKind = null;
  let signatureInvocation = null;
  let signatureTimer = null;
  let signatureExpiryTimer = null;
  let denialTimer = null;
  let captionTimer = null;
  let managed = false;
  let runtimeReachable = true;
  let hiddenForTool = false;
  let recordingActive = false;
  let lastPointer = null;

  function chevron() {
    return svgElement("svg", { width: 40, height: 24, viewBox: "0 0 40 24", fill: "none", "aria-hidden": "true" }, [
      svgElement("path", {
        d: "M6 6 L20 18 L34 6",
        stroke: SKY,
        "stroke-width": 3.4,
        "stroke-linecap": "round",
        "stroke-linejoin": "round"
      })
    ]);
  }

  function mount() {
    if (host?.isConnected) return;
    host = document.createElement("div");
    host.id = ROOT_ID;
    Object.assign(host.style, {
      all: "initial",
      position: "fixed",
      inset: "0",
      zIndex: "2147483647",
      pointerEvents: "none"
    });
    const shadow = host.attachShadow({ mode: "closed" });
    const style = document.createElement("style");
    style.textContent = presentationCss.build(TOKENS, REDUCED_FADE_SELECTOR);

    surface = document.createElement("div");
    surface.className = "surface";
    // Page feedback never owns human input, even when the stylesheet is unavailable.
    surface.style.pointerEvents = "none";
    scope = document.createElement("div");
    scope.className = "scope";
    fxLayer = document.createElement("div");
    fxLayer.className = "fx";
    cursor = document.createElement("div");
    cursor.className = "cursor";
    cursor.append(svgElement("svg", { width: 22, height: 28, viewBox: "0 0 22 28", "aria-hidden": "true" }, [
      svgElement("path", {
        d: "M0 0 L0 19 L5 14.5 L8.2 22 L11.4 20.6 L8.3 13.5 L14.5 13.5 Z",
        fill: SKY,
        stroke: "white",
        "stroke-width": 1.5,
        "stroke-linejoin": "round"
      })
    ]));
    caption = document.createElement("div");
    caption.className = "caption";
    signatureLayer = document.createElement("div");
    signatureLayer.className = "signatures";
    denialLayer = document.createElement("div");
    denialLayer.className = "denials";
    surface.append(scope, fxLayer, cursor, caption, signatureLayer, denialLayer);
    shadow.append(style, surface);
    (document.documentElement || document).appendChild(host);
    syncVisibility();
  }

  function syncVisibility() {
    if (!host) return;
    host.style.display = hiddenForTool ? "none" : "block";
    scope.classList.toggle("on", managed && runtimeReachable && !recordingActive);
  }

  function addEffect(className, styles) {
    const element = document.createElement("div");
    element.className = className;
    Object.assign(element.style, styles);
    fxLayer.appendChild(element);
    let removed = false;
    const remove = () => {
      if (removed) return;
      removed = true;
      element.remove();
    };
    element.addEventListener("animationend", remove, { once: true });
    later(remove, lifetimeFor(className));
    return element;
  }

  function center(rectangle) {
    if (!rectangle) return { x: innerWidth / 2, y: innerHeight / 2 };
    return {
      x: rectangle.left + rectangle.width / 2,
      y: rectangle.top + rectangle.height / 2
    };
  }

  function paddedRectangle(rectangle, padding) {
    return {
      left: rectangle.left - padding,
      top: rectangle.top - padding,
      width: Math.max(1, rectangle.width + padding * 2),
      height: Math.max(1, rectangle.height + padding * 2)
    };
  }

  function placeRectangle(element, rectangle) {
    element.style.left = `${rectangle.left}px`;
    element.style.top = `${rectangle.top}px`;
    element.style.width = `${rectangle.width}px`;
    element.style.height = `${rectangle.height}px`;
  }

  function moveCursor(rectangle) {
    const point = center(rectangle);
    lastPointer = lastPointer || { x: point.x - 48, y: point.y - 24 };
    cursor.style.transform = `translate3d(${Math.round(point.x)}px,${Math.round(point.y)}px,0)`;
    cursor.classList.add("on");
    const previous = lastPointer;
    lastPointer = point;
    return { previous, point };
  }

  function targetGlow(rectangle) {
    if (!rectangle) return;
    const effect = addEffect("target-glow", {});
    placeRectangle(effect, paddedRectangle(rectangle, 4));
  }

  // One ring per click on the language's stagger unit, dashed for a secondary button.
  // Without a shape this stays exactly one primary ring, so an orchestrator that does not
  // describe the click renders what it always did.
  function clickRipple(rectangle, shape) {
    const point = center(rectangle);
    const clicks = Math.min(3, Math.max(1, Number(shape && shape.clicks) || 1));
    const secondary = Boolean(shape) && shape.button === "secondary";
    for (let index = 0; index < clicks; index += 1) {
      later(() => {
        const effect = addEffect("ripple", { left: `${point.x}px`, top: `${point.y}px` });
        if (secondary) effect.classList.add("secondary");
      }, index * CLICK_STAGGER_MS);
    }
  }

  function dragTrail(rectangle) {
    const { previous, point } = moveCursor(rectangle);
    for (let step = 0; step < 12; step += 1) {
      const ratio = (step + 1) / 12;
      const x = previous.x + (point.x - previous.x) * ratio;
      const y = previous.y + (point.y - previous.y) * ratio;
      later(() => addEffect("trail-dot", { left: `${x}px`, top: `${y}px` }), step * 22);
    }
  }

  function fieldEffect(rectangle, treatment) {
    if (!rectangle) return;
    const padding = treatment === "field-splash" ? 4 : 3;
    const effect = addEffect(treatment, {});
    placeRectangle(effect, paddedRectangle(rectangle, padding));
  }

  function scrollCue(rectangle) {
    const point = center(rectangle);
    const effect = addEffect("chevrons", {});
    effect.style.left = `${point.x}px`;
    effect.style.top = `${point.y}px`;
    effect.append(chevron(), chevron(), chevron());
    if (rectangle) targetGlow(rectangle);
  }

  function readScan() {
    addEffect("read-scan", {});
  }

  /** The pill's arrow: the kit's right arrow, drawn in the pill's sky. */
  function navigationArrow() {
    return svgElement("svg", {
      class: "nav-arrow", width: 12, height: 12, viewBox: "0 0 24 24", fill: "none", "aria-hidden": "true"
    }, [
      svgElement("path", {
        d: "M5 12h14M13 6l6 6-6 6",
        stroke: "currentColor",
        "stroke-width": 2.4,
        "stroke-linecap": "round",
        "stroke-linejoin": "round"
      })
    ]);
  }

  function navigationPill() {
    const path = `${location.host}${location.pathname === "/" ? "" : location.pathname}` || "this page";
    const pill = addEffect("nav-pill", {});
    const arrow = navigationArrow();
    const destination = document.createElement("span");
    destination.textContent = path.slice(0, 58);
    pill.append(arrow, destination);
  }

  function keyLozenge() {
    const lozenge = addEffect("key-lozenge", {});
    const keycap = document.createElement("span");
    keycap.className = "private-keycap";
    lozenge.appendChild(keycap);
  }

  function screenshotEffect() {
    addEffect("capture-flash", {});
    addEffect("capture-frame", {});
  }

  function zoomEffect(rectangle) {
    const region = rectangle || {
      left: innerWidth * 0.2,
      top: innerHeight * 0.2,
      width: innerWidth * 0.6,
      height: innerHeight * 0.6
    };
    const effect = addEffect("zoom-frame", {});
    placeRectangle(effect, region);
  }

  function signatureMarkup(kind) {
    const icon = document.createElement("div");
    icon.className = "signature-icon";
    if (kind === ACTIVITY.script) {
      const wheel = document.createElement("div");
      wheel.className = "workwheel";
      wheel.append(svgElement("svg", {
        viewBox: "0 0 48 48",
        "aria-hidden": "true",
        fill: "none",
        stroke: "currentColor",
        "stroke-linecap": "round"
      }, [
        svgElement("circle", { cx: 24, cy: 24, r: 12, "stroke-width": 3.5 }),
        svgElement("circle", { cx: 24, cy: 24, r: 3.2, "stroke-width": 3.5 }),
        svgElement("path", {
          d: "M24 4v7M24 37v7M4 24h7M37 24h7M9.9 9.9l5 5M33.1 33.1l5 5M38.1 9.9l-5 5M14.9 33.1l-5 5",
          "stroke-width": 4.2
        })
      ]));
      icon.appendChild(wheel);
      for (let index = 1; index <= 3; index += 1) {
        const particle = document.createElement("span");
        particle.className = `particle p${index}`;
        icon.appendChild(particle);
      }
    } else if (kind === ACTIVITY.type) {
      const keyboard = document.createElement("div");
      keyboard.className = "keyboard";
      keyboard.append(svgElement("svg", {
        viewBox: "0 0 34 24",
        fill: "none",
        stroke: "currentColor",
        "stroke-width": 2,
        "aria-hidden": "true"
      }, [
        svgElement("rect", { x: 1, y: 2, width: 32, height: 20, rx: 4 }),
        svgElement("path", {
          d: "M6 7h2m3 0h2m3 0h2m3 0h2m3 0h2M6 12h2m3 0h2m3 0h2m3 0h2m3 0h2M8 17h18",
          "stroke-linecap": "round"
        })
      ]));
      icon.appendChild(keyboard);
    } else if (kind === ACTIVITY.wait) {
      const lights = document.createElement("div");
      lights.className = "wait-lights";
      lights.append(document.createElement("span"), document.createElement("span"), document.createElement("span"));
      icon.appendChild(lights);
    } else if (kind === ACTIVITY.find) {
      const lens = document.createElement("div");
      lens.className = "lens";
      lens.append(svgElement("svg", {
        viewBox: "0 0 40 40",
        "aria-hidden": "true",
        fill: "none",
        stroke: "currentColor",
        "stroke-width": 3.4,
        "stroke-linecap": "round"
      }, [
        svgElement("circle", { cx: 17, cy: 17, r: 10.5 }),
        svgElement("path", { d: "M25 25l9 9" }),
        svgElement("path", { d: "M11 17h12", opacity: ".45" })
      ]));
      icon.appendChild(lens);
    } else {
      const camera = document.createElement("div");
      camera.className = "camera";
      camera.append(svgElement("svg", {
        viewBox: "0 0 40 40",
        "aria-hidden": "true",
        fill: "none",
        stroke: "currentColor",
        "stroke-width": 3,
        "stroke-linejoin": "round"
      }, [
        svgElement("path", { d: "M5 13h8l3-4h8l3 4h8v21H5z" }),
        svgElement("circle", { cx: 20, cy: 23, r: 7 })
      ]));
      icon.appendChild(camera);
    }
    const glint = document.createElement("div");
    glint.className = "glint";
    icon.appendChild(glint);
    return icon;
  }

  function removeSignature() {
    unlater(signatureTimer);
    unlater(signatureExpiryTimer);
    if (signature) signature.remove();
    signature = null;
    signatureKind = null;
    signatureInvocation = null;
  }

  function beginSignature(kind, invocation, confirming = false) {
    if (signature && signatureKind === kind && signatureInvocation === invocation
      && !signature.classList.contains("completing")) return;
    removeSignature();
    signature = document.createElement("div");
    signature.className = `signature entering${confirming ? " confirming" : ""}`;
    signature.appendChild(signatureMarkup(kind));
    signatureLayer.appendChild(signature);
    signatureKind = kind;
    signatureInvocation = invocation;
    const started = signature;
    nextFrame(() => started.classList.remove("entering"));
    signatureExpiryTimer = later(() => {
      if (signature === started) removeSignature();
    }, STALE_SIGNATURE_MS);
    if (confirming) signatureTimer = later(() => finishSignature(invocation), CONFIRMING_SIGNATURE_MS);
  }

  function finishSignature(invocation) {
    if (!signature || signatureInvocation !== invocation || signature.classList.contains("completing")) return;
    const finishing = signature;
    finishing.classList.add("completing");
    unlater(signatureTimer);
    unlater(signatureExpiryTimer);
    signatureTimer = later(() => {
      finishing.classList.add("leaving");
      later(() => {
        if (signature === finishing) {
          removeSignature();
        }
        finishing.remove();
      }, SIGNATURE_LEAVE_MS);
    }, SIGNATURE_COMPLETE_MS);
  }

  function showCaption(label, enabled) {
    if (!enabled || !label) return;
    unlater(captionTimer);
    caption.textContent = label;
    caption.classList.add("on");
    captionTimer = later(() => caption.classList.remove("on"), CAPTION_MS);
  }

  function showDenial(signal) {
    unlater(denialTimer);
    denialLayer.replaceChildren();
    const ribbon = document.createElement("section");
    ribbon.className = "denial-ribbon";
    ribbon.setAttribute("role", "status");
    ribbon.setAttribute("aria-live", "polite");
    ribbon.setAttribute("aria-atomic", "true");
    const badge = document.createElement("div");
    badge.className = "denial-badge";
    badge.append(svgElement("svg", { viewBox: "0 0 24 26", "aria-hidden": "true" }, [
      svgElement("path", { d: "M12 1 L21 4.5 V11 C21 17 17 21.5 12 24 C7 21.5 3 17 3 11 V4.5 Z", fill: "#fff" }),
      svgElement("rect", { x: 10.9, y: 7.4, width: 2.2, height: 8.4, rx: 1.1, fill: "currentColor" }),
      svgElement("circle", { cx: 12, cy: 18.3, r: 1.35, fill: "currentColor" })
    ]));
    const text = document.createElement("div");
    const title = document.createElement("div");
    title.className = "denial-title";
    title.textContent = shared.bounded(signal.phase, 100);
    const description = document.createElement("div");
    description.className = "denial-description";
    description.textContent = shared.bounded(signal.detail, 240);
    text.append(title, description);
    ribbon.append(badge, text);
    denialLayer.appendChild(ribbon);
    denialTimer = later(() => denialLayer.replaceChildren(), DENIAL_MS);
  }

  function clearTransient() {
    if (!surface) return;
    fxLayer.replaceChildren();
    caption.classList.remove("on");
    removeSignature();
  }

  function render(signal, preferences, rectangle) {
    mount();
    const kind = signal.signal;
    const activity = signal.activity || ACTIVITY.quiet;
    if (kind === SIGNAL.denial) {
      if (signatureInvocation === signal.invocation) removeSignature();
      showDenial(signal);
      return true;
    }
    // Teardown belongs to the invocation, regardless of its final activity, outcome,
    // or a preference change while it was running. Another invocation cannot finish it.
    if (kind === SIGNAL.completion) finishSignature(signal.invocation);
    if ((kind === SIGNAL.start || kind === SIGNAL.progress) && signatureInvocation === signal.invocation
      && signatureKind !== activity) removeSignature();
    if (!preferences.effects) removeSignature();
    if (activity === ACTIVITY.quiet) return false;
    showCaption(signal.label, Boolean(preferences.captions));
    if (!preferences.effects) return Boolean(preferences.captions);

    if (kind === SIGNAL.start || kind === SIGNAL.progress) {
      if (activity === ACTIVITY.read) readScan();
      if ([ACTIVITY.find, ACTIVITY.script, ACTIVITY.type, ACTIVITY.wait].includes(activity)) beginSignature(activity, signal.invocation);
      if (activity === ACTIVITY.key) keyLozenge();
      if (activity === ACTIVITY.scroll) scrollCue(null);
    }

    if (kind === SIGNAL.target) {
      if ([ACTIVITY.click, ACTIVITY.hover].includes(activity)) moveCursor(rectangle);
      if (activity === ACTIVITY.click) {
        targetGlow(rectangle);
        clickRipple(rectangle, signal.click);
      }
      if (activity === ACTIVITY.drag) dragTrail(rectangle);
      if ([ACTIVITY.fill, ACTIVITY.upload].includes(activity)) fieldEffect(rectangle, "field-splash");
      if (activity === ACTIVITY.type) fieldEffect(rectangle, "field-shimmer");
      if (activity === ACTIVITY.scroll) scrollCue(rectangle);
    }

    if (kind === SIGNAL.completion) {
      if (activity === ACTIVITY.navigate) navigationPill();
      if (activity === ACTIVITY.screenshot) {
        screenshotEffect();
        beginSignature(ACTIVITY.screenshot, signal.invocation, true);
      }
      if (activity === ACTIVITY.zoom) zoomEffect(rectangle);
    }
    return true;
  }

  function setManaged(value) {
    managed = Boolean(value);
    mount();
    if (!managed) clearTransient();
    syncVisibility();
  }

  function setHidden(value) {
    hiddenForTool = Boolean(value);
    mount();
    syncVisibility();
    if (hiddenForTool) clearTransient();
  }

  function setRecording(value) {
    recordingActive = Boolean(value);
    mount();
    syncVisibility();
  }

  function setRuntimeState(value) {
    mount();
    if (value !== "active") clearTransient();
    runtimeReachable = !["ended", "disconnected"].includes(value);
    syncVisibility();
  }

  world.onDispose(() => {
    for (const id of timers) clearTimeout(id);
    timers.clear();
    for (const id of frames) cancelAnimationFrame(id);
    frames.clear();
    host?.remove();
    host = null;
    surface = null;
  });

  return Object.freeze({
    render,
    clearTransient,
    setManaged,
    setHidden,
    setRecording,
    setRuntimeState,
    visualIdentity
  });
})();
