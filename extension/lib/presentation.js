// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- policy-free rendering of the established product visual language.
(function installGhostlightPresentation(root, factory) {
  root.GhostlightPresentation = factory();
})(globalThis, function createGhostlightPresentation() {
  "use strict";

  const SKY = "#38bdf8";
  const SKY_RGB = "56,189,248";
  const INK = "#eaf6ff";
  const GROUND = "#0c0f14";
  const SPRING = "cubic-bezier(.22,1,.36,1)";
  const DENIAL_MS = 5000;
  const CLICK_STAGGER_MS = 150;

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
    ripple_ms: 620,
    field_splash_ms: 700,
    read_scan_ms: 1450,
    navigation_ms: 1600,
    screenshot_ms: 1500,
    zoom_ms: 1150,
    denial_ms: DENIAL_MS
  });

  let host = null;
  let surface = null;
  let scope = null;
  let cursor = null;
  let fxLayer = null;
  let caption = null;
  let signatureLayer = null;
  let denialLayer = null;
  let attention = null;
  let signature = null;
  let signatureKind = null;
  let signatureTimer = null;
  let denialTimer = null;
  let captionTimer = null;
  let managed = false;
  let runtimeReachable = true;
  let hiddenForTool = false;
  let recordingActive = false;
  let lastPointer = null;

  const chevron =
    `<svg width="40" height="24" viewBox="0 0 40 24" fill="none" aria-hidden="true">` +
    `<path d="M6 6 L20 18 L34 6" stroke="${SKY}" stroke-width="3.4" ` +
    `stroke-linecap="round" stroke-linejoin="round"/></svg>`;

  function mount() {
    if (host?.isConnected) return;
    host = document.createElement("div");
    host.id = "ghostlight-presentation-root";
    Object.assign(host.style, {
      all: "initial",
      position: "fixed",
      inset: "0",
      zIndex: "2147483647",
      pointerEvents: "none"
    });
    const shadow = host.attachShadow({ mode: "closed" });
    const style = document.createElement("style");
    style.textContent = globalThis.GhostlightPresentationCss.build(TOKENS, REDUCED_FADE_SELECTOR);

    surface = document.createElement("div");
    surface.className = "surface";
    scope = document.createElement("div");
    scope.className = "scope";
    fxLayer = document.createElement("div");
    fxLayer.className = "fx";
    cursor = document.createElement("div");
    cursor.className = "cursor";
    cursor.innerHTML =
      `<svg width="22" height="28" viewBox="0 0 22 28" aria-hidden="true">` +
      `<path d="M0 0 L0 19 L5 14.5 L8.2 22 L11.4 20.6 L8.3 13.5 L14.5 13.5 Z" ` +
      `fill="${SKY}" stroke="white" stroke-width="1.5" stroke-linejoin="round"/></svg>`;
    caption = document.createElement("div");
    caption.className = "caption";
    signatureLayer = document.createElement("div");
    signatureLayer.className = "signatures";
    denialLayer = document.createElement("div");
    denialLayer.className = "denials";
    attention = buildAttention();
    surface.append(scope, fxLayer, cursor, caption, signatureLayer, denialLayer, attention);
    shadow.append(style, surface);
    (document.documentElement || document).appendChild(host);
    syncVisibility();
  }

  function buildAttention() {
    const overlay = document.createElement("div");
    overlay.className = "attention";
    const card = document.createElement("section");
    card.className = "attention-card";
    const icon = document.createElement("div");
    icon.className = "attention-icon";
    icon.textContent = "!";
    const title = document.createElement("h2");
    title.textContent = "Ghostlight needs your attention";
    const description = document.createElement("p");
    description.textContent = "Agent browsing is paused until you decide what happens next.";
    const actions = document.createElement("div");
    actions.className = "attention-actions";
    for (const [disposition, label, dangerous] of [
      ["keep_paused", "Keep paused", false],
      ["resume", "Resume", false],
      ["resume_quiet", "Resume + quiet", false],
      ["end_session", "End session", true]
    ]) {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = label;
      if (dangerous) button.className = "danger";
      button.addEventListener("click", () => {
        chrome.runtime.sendMessage({ kind: "attention_action", disposition }).catch(() => {});
      });
      actions.appendChild(button);
    }
    card.append(icon, title, description, actions);
    overlay.appendChild(card);
    return overlay;
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
    setTimeout(remove, lifetimeFor(className));
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
      setTimeout(() => {
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
      setTimeout(() => addEffect("trail-dot", { left: `${x}px`, top: `${y}px` }), step * 22);
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
    effect.innerHTML = chevron + chevron + chevron;
    if (rectangle) targetGlow(rectangle);
  }

  function readScan() {
    addEffect("read-scan", {});
  }

  function navigationPill() {
    const path = `${location.host}${location.pathname === "/" ? "" : location.pathname}` || "this page";
    const pill = addEffect("nav-pill", {});
    const arrow = document.createElement("span");
    arrow.className = "nav-arrow";
    arrow.textContent = "->";
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
    if (kind === "script") {
      const wheel = document.createElement("div");
      wheel.className = "workwheel";
      wheel.innerHTML = `<svg viewBox="0 0 48 48" aria-hidden="true" fill="none" stroke="currentColor" stroke-linecap="round"><circle cx="24" cy="24" r="12" stroke-width="3.5"/><circle cx="24" cy="24" r="3.2" stroke-width="3.5"/><path d="M24 4v7M24 37v7M4 24h7M37 24h7M9.9 9.9l5 5M33.1 33.1l5 5M38.1 9.9l-5 5M14.9 33.1l-5 5" stroke-width="4.2"/></svg>`;
      icon.appendChild(wheel);
      for (let index = 1; index <= 3; index += 1) {
        const particle = document.createElement("span");
        particle.className = `particle p${index}`;
        icon.appendChild(particle);
      }
    } else if (kind === "type") {
      const keyboard = document.createElement("div");
      keyboard.className = "keyboard";
      keyboard.innerHTML = `<svg viewBox="0 0 34 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="1" y="2" width="32" height="20" rx="4"/><path d="M6 7h2m3 0h2m3 0h2m3 0h2m3 0h2M6 12h2m3 0h2m3 0h2m3 0h2m3 0h2M8 17h18" stroke-linecap="round"/></svg>`;
      icon.appendChild(keyboard);
    } else if (kind === "wait") {
      const lights = document.createElement("div");
      lights.className = "wait-lights";
      lights.append(document.createElement("span"), document.createElement("span"), document.createElement("span"));
      icon.appendChild(lights);
    } else if (kind === "find") {
      const lens = document.createElement("div");
      lens.className = "lens";
      lens.innerHTML = `<svg viewBox="0 0 40 40" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="3.4" stroke-linecap="round"><circle cx="17" cy="17" r="10.5"/><path d="M25 25l9 9"/><path d="M11 17h12" opacity=".45"/></svg>`;
      icon.appendChild(lens);
    } else {
      const camera = document.createElement("div");
      camera.className = "camera";
      camera.innerHTML = `<svg viewBox="0 0 40 40" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="3" stroke-linejoin="round"><path d="M5 13h8l3-4h8l3 4h8v21H5z"/><circle cx="20" cy="23" r="7"/></svg>`;
      icon.appendChild(camera);
    }
    const glint = document.createElement("div");
    glint.className = "glint";
    icon.appendChild(glint);
    return icon;
  }

  function beginSignature(kind, confirming = false) {
    clearTimeout(signatureTimer);
    if (signature) signature.remove();
    signature = document.createElement("div");
    signature.className = `signature entering${confirming ? " confirming" : ""}`;
    signature.appendChild(signatureMarkup(kind));
    signatureLayer.appendChild(signature);
    signatureKind = kind;
    requestAnimationFrame(() => signature?.classList.remove("entering"));
    if (confirming) signatureTimer = setTimeout(() => finishSignature(kind), 900);
  }

  function finishSignature(kind) {
    if (!signature || signatureKind !== kind) return;
    const finishing = signature;
    finishing.classList.add("completing");
    clearTimeout(signatureTimer);
    signatureTimer = setTimeout(() => {
      finishing.classList.add("leaving");
      setTimeout(() => {
        if (signature === finishing) {
          signature = null;
          signatureKind = null;
        }
        finishing.remove();
      }, 460);
    }, 180);
  }

  function showCaption(activity, enabled) {
    if (!enabled) return;
    clearTimeout(captionTimer);
    caption.textContent = globalThis.GhostlightShared.activityLabel(activity);
    caption.classList.add("on");
    captionTimer = setTimeout(() => caption.classList.remove("on"), 1500);
  }

  function showDenial(signal) {
    clearTimeout(denialTimer);
    denialLayer.replaceChildren();
    const ribbon = document.createElement("section");
    ribbon.className = "denial-ribbon";
    ribbon.setAttribute("role", "status");
    ribbon.setAttribute("aria-live", "polite");
    ribbon.setAttribute("aria-atomic", "true");
    const badge = document.createElement("div");
    badge.className = "denial-badge";
    badge.innerHTML = `<svg viewBox="0 0 24 26" aria-hidden="true"><path d="M12 1 L21 4.5 V11 C21 17 17 21.5 12 24 C7 21.5 3 17 3 11 V4.5 Z" fill="#fff"/><rect x="10.9" y="7.4" width="2.2" height="8.4" rx="1.1" fill="currentColor"/><circle cx="12" cy="18.3" r="1.35" fill="currentColor"/></svg>`;
    const text = document.createElement("div");
    const title = document.createElement("div");
    title.className = "denial-title";
    title.textContent = globalThis.GhostlightShared.bounded(signal.phase || "Ghostlight blocked this action", 100);
    const description = document.createElement("div");
    description.className = "denial-description";
    description.textContent = globalThis.GhostlightShared.bounded(signal.detail || "A configured guardrail prevented it.", 240);
    text.append(title, description);
    ribbon.append(badge, text);
    denialLayer.appendChild(ribbon);
    denialTimer = setTimeout(() => denialLayer.replaceChildren(), DENIAL_MS);
  }

  function clearTransient() {
    if (!surface) return;
    fxLayer.replaceChildren();
    caption.classList.remove("on");
    if (signature) signature.remove();
    signature = null;
    signatureKind = null;
    clearTimeout(signatureTimer);
  }

  function render(signal, preferences, rectangle) {
    mount();
    const kind = signal.signal;
    const activity = signal.activity || "quiet";
    if (kind === "attention") {
      clearTransient();
      attention.classList.add("on");
      return true;
    }
    if (kind === "denial") {
      showDenial(signal);
      return true;
    }
    if (activity === "quiet") return false;
    showCaption(activity, Boolean(preferences.captions));
    if (!preferences.effects) return Boolean(preferences.captions);

    if (kind === "start" || kind === "progress") {
      if (activity === "read") readScan();
      if (["find", "script", "type", "wait"].includes(activity)) beginSignature(activity);
      if (activity === "key") keyLozenge();
      if (activity === "scroll") scrollCue(null);
    }

    if (kind === "target") {
      if (["click", "hover"].includes(activity)) moveCursor(rectangle);
      if (activity === "click") {
        targetGlow(rectangle);
        clickRipple(rectangle, signal.click);
      }
      if (activity === "drag") dragTrail(rectangle);
      if (["fill", "upload"].includes(activity)) fieldEffect(rectangle, "field-splash");
      if (activity === "type") fieldEffect(rectangle, "field-shimmer");
      if (activity === "scroll") scrollCue(rectangle);
    }

    if (kind === "completion") {
      if (activity === "navigate") navigationPill();
      if (activity === "screenshot") {
        screenshotEffect();
        beginSignature("screenshot", true);
      }
      if (activity === "zoom") zoomEffect(rectangle);
      if (["find", "script", "type", "wait"].includes(activity)) finishSignature(activity);
    }
    return true;
  }

  function setManaged(value) {
    managed = Boolean(value);
    mount();
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
    runtimeReachable = !["ended", "disconnected"].includes(value);
    attention.classList.toggle("on", value === "attention");
    syncVisibility();
  }

  return Object.freeze({
    render,
    clearTransient,
    setManaged,
    setHidden,
    setRecording,
    setRuntimeState,
    visualIdentity
  });
});
