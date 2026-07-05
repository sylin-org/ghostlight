// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- agent visual indicator (content script).
//
// User-facing "watching" affordance (mechanism, not policy):
//   - a phantom cursor showing where the agent's pointer is about to act,
//   - a subtle "agent active" glow while a tool runs,
//   - a sky-blue click ripple (one ring per click; a right-click ring is dashed),
//   - a comet-trail along a click-drag path,
//   - a soft shimmer on the focused field when the agent types.
// All are hidden during screenshots so the model's image stays clean, and are excluded from
// read_page/find (their ids are prefixed "ghostlight-" and skipped in content.js). Driven by the
// service worker via chrome.tabs.sendMessage. A lean reimplementation of the concept; no upstream
// extension code is copied.

(function () {
  if (window.__browserMcpIndicator) return;
  window.__browserMcpIndicator = true;

  const CURSOR_ID = "ghostlight-cursor";
  const GLOW_ID = "ghostlight-active";
  const FX_LAYER_ID = "ghostlight-ripples"; // holds all transient effects (rings, trail, shimmer)
  const STYLE_ID = "ghostlight-indicator-styles";
  // Ghostlight brand accent: a luminous sky blue. SKY_RGB is the same color for rgba() shadows.
  const SKY = "#38bdf8";
  const SKY_RGB = "56,189,248";
  const FADE_MS = 4000;
  const RIPPLE_MS = 620; // one click ring's expand-and-fade duration
  const RIPPLE_STAGGER_MS = 140; // gap between rings of a multi-click, so 2/3 read as a rhythm
  // Extended-vocabulary timings (the visual feedback dictionary).
  const LOZENGE_MS = 1250; // keystroke lozenge (type / key)
  const SCAN_MS = 1450; // read-page scan-line sweep
  const CAPFRAME_MS = 1500; // screenshot frame "files" itself to the corner
  const ZOOMFRAME_MS = 1150; // zoom magnifier frame
  const CHEV_MS = 1150; // scroll chevron cascade
  const NAVPILL_MS = 1600; // navigate destination pill

  let cursorEl = null;
  let glowEl = null;
  let fxLayer = null;
  let fadeTimer = null;
  let fxSeq = 0;
  let glowActive = false; // whether the glow should be visible (independent of capture-hiding)
  let hiddenForTool = false; // suppressed during a screenshot capture

  function reduceMotion() {
    return !!(window.matchMedia && window.matchMedia("(prefers-reduced-motion:reduce)").matches);
  }

  function ensureStyles() {
    if (document.getElementById(STYLE_ID)) return;
    const s = document.createElement("style");
    s.id = STYLE_ID;
    s.textContent =
      "@keyframes ghostlight-pulse{0%,100%{opacity:.5}50%{opacity:.9}}" +
      "#" + GLOW_ID + "{animation:ghostlight-pulse 2s ease-in-out infinite}" +
      "@keyframes ghostlight-ripple{0%{opacity:.85;transform:translate(-50%,-50%) scale(.3)}" +
      "100%{opacity:0;transform:translate(-50%,-50%) scale(2.8)}}" +
      "@keyframes ghostlight-ripple-rm{0%{opacity:.7;transform:translate(-50%,-50%) scale(1)}" +
      "100%{opacity:0;transform:translate(-50%,-50%) scale(1)}}" +
      "@keyframes ghostlight-trail{0%{opacity:.9}100%{opacity:0}}" +
      "@keyframes ghostlight-shimmer{0%{opacity:0}25%{opacity:1}60%{opacity:.7}100%{opacity:0}}" +
      "@keyframes ghostlight-shimmer-rm{0%{opacity:0}50%{opacity:.7}100%{opacity:0}}" +
      "@keyframes ghostlight-targetglow{0%{opacity:0}22%{opacity:1}100%{opacity:0}}" +
      "@keyframes ghostlight-flash{0%{opacity:.42}100%{opacity:0}}" +
      "@keyframes ghostlight-capframe{0%{opacity:0;transform:scale(1.03)}9%{opacity:1;transform:scale(1)}34%{opacity:1;transform:scale(1)}60%{opacity:1;transform:scale(.17);border-radius:16px}88%{opacity:1;transform:scale(.17);border-radius:16px}100%{opacity:0;transform:scale(.17);border-radius:16px}}" +
      "@keyframes ghostlight-zoomframe{0%{opacity:0;transform:scale(1.35)}22%{opacity:1}70%{opacity:1;transform:scale(1)}100%{opacity:0;transform:scale(1)}}" +
      "@keyframes ghostlight-scan{0%{opacity:0;transform:translateY(-80px)}12%{opacity:1}90%{opacity:1}100%{opacity:.85;transform:translateY(100vh)}}" +
      "@keyframes ghostlight-chev{0%{opacity:0;transform:translateY(-8px)}30%{opacity:1}100%{opacity:0;transform:translateY(10px)}}" +
      "@keyframes ghostlight-nav{0%{opacity:0;transform:translate(-50%,-14px)}14%{opacity:1;transform:translate(-50%,0)}82%{opacity:1;transform:translate(-50%,0)}100%{opacity:0;transform:translate(-50%,-8px)}}" +
      "@keyframes ghostlight-breath{0%,100%{opacity:.35;transform:translate(-50%,-50%) scale(.7)}50%{opacity:1;transform:translate(-50%,-50%) scale(1.2)}}" +
      "@keyframes ghostlight-lozenge{0%{opacity:0;transform:translate(-50%,12px)}16%{opacity:1;transform:translate(-50%,0)}78%{opacity:1;transform:translate(-50%,0)}100%{opacity:0;transform:translate(-50%,-6px)}}" +
      ".ghostlight-cap{color:" + SKY + "}.ghostlight-arrow{color:" + SKY + ";margin-right:7px}" +
      "@media (prefers-reduced-motion:reduce){#" + GLOW_ID + "{animation:none}#" + CURSOR_ID + "{transition:none}}";
    (document.head || document.documentElement).appendChild(s);
  }

  function makeCursor() {
    const el = document.createElement("div");
    el.id = CURSOR_ID;
    el.setAttribute("aria-hidden", "true");
    el.style.cssText =
      "position:fixed;top:0;left:0;pointer-events:none;z-index:2147483647;" +
      "transform:translate3d(-100px,-100px,0);transition:transform 150ms cubic-bezier(.2,0,0,1);" +
      "will-change:transform;filter:drop-shadow(0 0 3px rgba(" + SKY_RGB + ",.9)) drop-shadow(0 0 8px rgba(" + SKY_RGB + ",.5))";
    // Own arrow glyph; the tip sits at (0,0) so translate(x,y) places the tip exactly on the target.
    el.innerHTML =
      "<svg width='22' height='28' viewBox='0 0 22 28' style='position:absolute;top:0;left:0;overflow:visible'>" +
      "<path d='M0 0 L0 19 L5 14.5 L8.2 22 L11.4 20.6 L8.3 13.5 L14.5 13.5 Z' " +
      "fill='" + SKY + "' stroke='white' stroke-width='1.5' stroke-linejoin='round'/></svg>";
    return el;
  }

  function makeGlow() {
    const el = document.createElement("div");
    el.id = GLOW_ID;
    el.setAttribute("aria-hidden", "true");
    el.style.cssText =
      "position:fixed;inset:0;pointer-events:none;z-index:2147483646;opacity:0;" +
      "transition:opacity .3s ease-in-out;" +
      "box-shadow:inset 0 0 14px rgba(" + SKY_RGB + ",.7),inset 0 0 26px rgba(" + SKY_RGB + ",.35)";
    return el;
  }

  // A full-viewport, pointer-transparent layer that holds every transient effect. Its own id and
  // each effect's id are "ghostlight-" prefixed, so content.js skips them in read_page/find.
  function ensureFxLayer() {
    if (!fxLayer || !fxLayer.isConnected) {
      fxLayer = document.createElement("div");
      fxLayer.id = FX_LAYER_ID;
      fxLayer.setAttribute("aria-hidden", "true");
      fxLayer.style.cssText = "position:fixed;inset:0;pointer-events:none;z-index:2147483646";
      (document.body || document.documentElement).appendChild(fxLayer);
    }
    return fxLayer;
  }

  // Append a transient effect element to the fx layer and remove it when its animation ends.
  function addEphemeral(el, maxMs) {
    ensureFxLayer().appendChild(el);
    let done = false;
    const remove = () => { if (done) return; done = true; el.remove(); };
    el.addEventListener("animationend", remove, { once: true });
    setTimeout(remove, maxMs); // fallback if animationend never fires
  }

  function addRipple(x, y, dashed) {
    if (hiddenForTool || document.hidden) return;
    const el = document.createElement("div");
    el.id = FX_LAYER_ID + "-r" + fxSeq++; // "ghostlight-" prefix -> excluded from reads
    el.setAttribute("aria-hidden", "true");
    const anim = reduceMotion() ? "ghostlight-ripple-rm" : "ghostlight-ripple";
    el.style.cssText =
      "position:fixed;left:" + Math.round(x) + "px;top:" + Math.round(y) + "px;" +
      "width:34px;height:34px;border-radius:50%;box-sizing:border-box;pointer-events:none;" +
      "border:2px " + (dashed ? "dashed" : "solid") + " rgba(" + SKY_RGB + ",.9);" +
      "box-shadow:0 0 12px rgba(" + SKY_RGB + ",.55),inset 0 0 8px rgba(" + SKY_RGB + ",.35);" +
      "transform:translate(-50%,-50%) scale(.3);" +
      "animation:" + anim + " " + RIPPLE_MS + "ms ease-out forwards";
    addEphemeral(el, RIPPLE_MS + 80);
  }

  // One ring per click: count is the click count (1 single, 2 double, 3 triple), staggered so a
  // multi-click reads as a rhythm. A right-click ring is dashed to read as a secondary action.
  function spawnRipples(x, y, count, button) {
    if (hiddenForTool || document.hidden) return;
    ensureStyles();
    const dashed = button === "right";
    const n = Math.max(1, Math.min((count | 0) || 1, 5));
    for (let i = 0; i < n; i++) {
      if (i === 0) addRipple(x, y, dashed);
      else setTimeout(() => addRipple(x, y, dashed), i * RIPPLE_STAGGER_MS);
    }
  }

  // A soft dot dropped along a drag path; the sequence of fading dots reads as a comet trail.
  function addTrailDot(x, y) {
    if (hiddenForTool || document.hidden) return;
    ensureStyles();
    const el = document.createElement("div");
    el.id = FX_LAYER_ID + "-t" + fxSeq++;
    el.setAttribute("aria-hidden", "true");
    el.style.cssText =
      "position:fixed;left:" + Math.round(x) + "px;top:" + Math.round(y) + "px;" +
      "width:14px;height:14px;border-radius:50%;pointer-events:none;transform:translate(-50%,-50%);" +
      "background:radial-gradient(circle,rgba(" + SKY_RGB + ",.9) 0%,rgba(" + SKY_RGB + ",0) 70%);" +
      "animation:ghostlight-trail 520ms ease-out forwards";
    addEphemeral(el, 600);
  }

  // A gentle sky-blue outline over the currently focused field while the agent types into it.
  function shimmerFocused() {
    if (hiddenForTool || document.hidden) return;
    const target = document.activeElement;
    if (!target || target === document.body || target === document.documentElement) return;
    let rect;
    try { rect = target.getBoundingClientRect(); } catch (e) { return; }
    if (!rect || (rect.width === 0 && rect.height === 0)) return;
    ensureStyles();
    const pad = 3;
    const anim = reduceMotion() ? "ghostlight-shimmer-rm" : "ghostlight-shimmer";
    const el = document.createElement("div");
    el.id = FX_LAYER_ID + "-s" + fxSeq++;
    el.setAttribute("aria-hidden", "true");
    el.style.cssText =
      "position:fixed;box-sizing:border-box;pointer-events:none;border-radius:6px;" +
      "left:" + (rect.left - pad) + "px;top:" + (rect.top - pad) + "px;" +
      "width:" + (rect.width + pad * 2) + "px;height:" + (rect.height + pad * 2) + "px;" +
      "border:1.5px solid rgba(" + SKY_RGB + ",.85);" +
      "box-shadow:0 0 10px rgba(" + SKY_RGB + ",.5),inset 0 0 8px rgba(" + SKY_RGB + ",.25);" +
      "animation:" + anim + " 900ms ease-in-out forwards";
    addEphemeral(el, 1000);
  }

  function showGlow() {
    glowActive = true;
    if (fadeTimer) clearTimeout(fadeTimer);
    fadeTimer = setTimeout(hideGlow, FADE_MS);
    if (hiddenForTool || document.hidden) return;
    ensureStyles();
    if (!glowEl) { glowEl = makeGlow(); (document.body || document.documentElement).appendChild(glowEl); }
    glowEl.style.display = "";
    requestAnimationFrame(() => { if (glowEl && glowActive && !hiddenForTool) glowEl.style.opacity = "1"; });
  }

  function hideGlow() {
    glowActive = false;
    if (fadeTimer) { clearTimeout(fadeTimer); fadeTimer = null; }
    if (glowEl) glowEl.style.opacity = "0";
  }

  function moveCursor(x, y) {
    return new Promise((resolve) => {
      showGlow();
      if (hiddenForTool || document.hidden) return resolve();
      ensureStyles();
      if (!cursorEl) { cursorEl = makeCursor(); (document.body || document.documentElement).appendChild(cursorEl); }
      cursorEl.style.display = "";
      cursorEl.style.transform = "translate3d(" + Math.round(x) + "px," + Math.round(y) + "px,0)";
      let done = false;
      const finish = () => {
        if (done) return;
        done = true;
        if (cursorEl) cursorEl.removeEventListener("transitionend", finish);
        resolve();
      };
      cursorEl.addEventListener("transitionend", finish, { once: true });
      setTimeout(finish, 200); // fallback if no transition fires (e.g. first placement)
    });
  }

  function setHiddenForTool(v) {
    hiddenForTool = v;
    if (cursorEl) cursorEl.style.display = v ? "none" : "";
    if (captionEl) captionEl.style.display = v ? "none" : ""; // keep the subtitle out of the agent's own capture

    if (glowEl) {
      if (v) glowEl.style.display = "none";
      else if (glowActive) { glowEl.style.display = ""; glowEl.style.opacity = "1"; }
    }
    if (fxLayer) {
      if (v) { fxLayer.style.display = "none"; fxLayer.replaceChildren(); } // clear in-flight effects for a clean capture
      else fxLayer.style.display = "";
    }
  }

  // ----- Extended vocabulary: one visible treatment per agent action (the visual feedback
  // dictionary). Drawn from Screen Studio (glide + rings), KeyCastr/Keyviz (keystroke lozenges,
  // scroll cues), and Playwright's .highlight() (confirm the target). Every effect below respects
  // hiddenForTool / document.hidden, so none pollutes the agent's own screenshot; the screenshot
  // flash is the one that fires only AFTER a capture. -----
  const CHEV = "<svg width='40' height='24' viewBox='0 0 40 24' fill='none' aria-hidden='true'>" +
    "<path d='M6 6 L20 18 L34 6' stroke='" + SKY + "' stroke-width='3.4' stroke-linecap='round' stroke-linejoin='round'/></svg>";
  let effectsEnabled = true; // master switch (options page); default on
  let captionsEnabled = false;
  let captionEl = null;

  function escapeHtml(s) {
    return String(s).replace(/[&<>"]/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c];
    });
  }
  function clip(s, n) { s = String(s); return s.length > n ? s.slice(0, n - 1) + "…" : s; }
  function hostPath(u) {
    try { const url = new URL(u); return url.host + (url.pathname === "/" ? "" : url.pathname); }
    catch (e) { return String(u); }
  }

  // Optional subtitle track (off by default; SET_CAPTIONS toggles it): names the current action,
  // bottom-center. Gorgeous for a recorded demo, too chatty for everyday driving.
  function caption(label) {
    if (!captionsEnabled || hiddenForTool || document.hidden) return;
    if (!captionEl || !captionEl.isConnected) {
      captionEl = document.createElement("div");
      captionEl.id = "ghostlight-caption"; // ghostlight- prefix -> excluded from reads
      captionEl.setAttribute("aria-hidden", "true");
      captionEl.style.cssText =
        "position:fixed;left:50%;bottom:22px;transform:translate(-50%,8px);z-index:2147483645;" +
        "pointer-events:none;opacity:0;font:12px/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;" +
        "color:#eaf6ff;background:rgba(10,16,26,.82);border:1px solid rgba(" + SKY_RGB + ",.4);" +
        "padding:6px 13px;border-radius:999px;transition:opacity .2s ease,transform .2s cubic-bezier(.22,1,.36,1)";
      (document.body || document.documentElement).appendChild(captionEl);
    }
    captionEl.textContent = label;
    captionEl.style.opacity = "1";
    captionEl.style.transform = "translate(-50%,0)";
    clearTimeout(caption._t);
    caption._t = setTimeout(function () {
      if (captionEl) { captionEl.style.opacity = "0"; captionEl.style.transform = "translate(-50%,8px)"; }
    }, 1500);
  }

  // click: the element under the point glows briefly -- confirms WHAT was acted on (Playwright).
  function targetGlow(x, y) {
    if (hiddenForTool || document.hidden) return;
    ensureStyles();
    let rect = null, el = null;
    try { el = document.elementFromPoint(x, y); } catch (e) { el = null; }
    if (el && el.getBoundingClientRect) rect = el.getBoundingClientRect();
    if (!rect || (rect.width < 2 && rect.height < 2) || rect.width > window.innerWidth * 0.98) {
      rect = { left: x - 22, top: y - 15, width: 44, height: 30 }; // no sensible element: glow the point
    }
    const pad = 4;
    const g = document.createElement("div");
    g.id = FX_LAYER_ID + "-g" + fxSeq++;
    g.setAttribute("aria-hidden", "true");
    g.style.cssText =
      "position:fixed;box-sizing:border-box;pointer-events:none;border-radius:8px;" +
      "left:" + (rect.left - pad) + "px;top:" + (rect.top - pad) + "px;" +
      "width:" + (rect.width + pad * 2) + "px;height:" + (rect.height + pad * 2) + "px;" +
      "box-shadow:0 0 0 2px rgba(" + SKY_RGB + ",.9),0 0 20px rgba(" + SKY_RGB + ",.55);" +
      "animation:ghostlight-targetglow 720ms ease-out forwards";
    addEphemeral(g, 780);
  }

  // type / key: a keystroke lozenge, bottom-center (KeyCastr). type shows the text; key the combo.
  function keystrokeLozenge(textStr, kind) {
    if (hiddenForTool || document.hidden) return;
    ensureStyles();
    const html = kind === "key"
      ? String(textStr).split(/[+ ]/).filter(Boolean)
          .map(function (k) { return "<span class='ghostlight-cap'>" + escapeHtml(k) + "</span>"; }).join(" + ")
      : escapeHtml(clip(textStr, 44));
    const el = document.createElement("div");
    el.id = FX_LAYER_ID + "-k" + fxSeq++;
    el.setAttribute("aria-hidden", "true");
    el.innerHTML = html;
    el.style.cssText =
      "position:fixed;left:50%;bottom:64px;z-index:2147483645;pointer-events:none;white-space:nowrap;" +
      "font:600 14px/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:#eaf6ff;" +
      "padding:8px 14px;border-radius:10px;background:rgba(12,20,32,.9);border:1px solid rgba(" + SKY_RGB + ",.55);" +
      "box-shadow:0 10px 30px -12px rgba(" + SKY_RGB + ",.8);" +
      "animation:ghostlight-lozenge " + LOZENGE_MS + "ms cubic-bezier(.22,1,.36,1) forwards";
    addEphemeral(el, LOZENGE_MS + 40);
  }

  // scroll: directional chevrons cascading the way the page moves (Keyviz).
  function scrollCue(direction) {
    if (hiddenForTool || document.hidden) return;
    ensureStyles();
    const rot = direction === "up" ? 180 : direction === "left" ? 90 : direction === "right" ? -90 : 0;
    const wrap = document.createElement("div");
    wrap.id = FX_LAYER_ID + "-sc" + fxSeq++;
    wrap.setAttribute("aria-hidden", "true");
    wrap.innerHTML = CHEV + CHEV + CHEV;
    wrap.style.cssText =
      "position:fixed;left:50%;top:50%;pointer-events:none;display:flex;flex-direction:column;align-items:center;gap:1px;" +
      "transform:translate(-50%,-50%) rotate(" + rot + "deg)";
    for (let i = 0; i < wrap.children.length; i++) {
      wrap.children[i].style.opacity = "0";
      wrap.children[i].style.animation = "ghostlight-chev 900ms ease-out " + (i * 100) + "ms forwards";
    }
    addEphemeral(wrap, CHEV_MS);
    caption("Scroll " + direction);
  }

  // read_page / find / get_page_text: a scan-line sweeps down -- "the agent is reading" (ours alone).
  function readScan() {
    if (hiddenForTool || document.hidden) return;
    ensureStyles();
    const el = document.createElement("div");
    el.id = FX_LAYER_ID + "-scan" + fxSeq++;
    el.setAttribute("aria-hidden", "true");
    el.style.cssText =
      "position:fixed;left:0;right:0;top:0;height:80px;pointer-events:none;" +
      "background:linear-gradient(180deg,transparent,rgba(" + SKY_RGB + ",.15) 62%,rgba(" + SKY_RGB + ",.8));" +
      "box-shadow:0 6px 20px rgba(" + SKY_RGB + ",.35);animation:ghostlight-scan " + SCAN_MS + "ms cubic-bezier(.4,0,.5,1) forwards";
    addEphemeral(el, SCAN_MS + 60);
    caption("Reading page");
  }

  // navigate: a destination pill (host + path), top-center, after the new page loads.
  function navigatePill(url) {
    if (hiddenForTool || document.hidden) return;
    ensureStyles();
    const el = document.createElement("div");
    el.id = FX_LAYER_ID + "-nav" + fxSeq++;
    el.setAttribute("aria-hidden", "true");
    el.innerHTML = "<span class='ghostlight-arrow'>&#8594;</span>" + escapeHtml(clip(hostPath(url), 58));
    el.style.cssText =
      "position:fixed;left:50%;top:16px;z-index:2147483645;pointer-events:none;white-space:nowrap;" +
      "font:12px/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:#eaf6ff;" +
      "padding:8px 15px;border-radius:999px;background:rgba(10,16,26,.9);border:1px solid rgba(" + SKY_RGB + ",.5);" +
      "box-shadow:0 12px 30px -12px rgba(" + SKY_RGB + ",.8);animation:ghostlight-nav " + NAVPILL_MS + "ms ease-out forwards";
    addEphemeral(el, NAVPILL_MS + 40);
    caption("Navigate");
  }

  // screenshot: fired AFTER the capture (never in the image). A sky shutter flash, then the frame
  // "files itself" into the bottom-right corner -- the gesture everyone reads as "captured".
  function screenshotFx() {
    if (document.hidden) return;
    ensureStyles();
    const flash = document.createElement("div");
    flash.id = FX_LAYER_ID + "-flash" + fxSeq++;
    flash.setAttribute("aria-hidden", "true");
    flash.style.cssText = "position:fixed;inset:0;pointer-events:none;background:rgba(" + SKY_RGB + ",.42);animation:ghostlight-flash 260ms ease-out forwards";
    addEphemeral(flash, 320);
    const frame = document.createElement("div");
    frame.id = FX_LAYER_ID + "-cap" + fxSeq++;
    frame.setAttribute("aria-hidden", "true");
    frame.style.cssText =
      "position:fixed;inset:8px;pointer-events:none;border-radius:8px;border:2px solid rgba(" + SKY_RGB + ",.9);" +
      "background:rgba(" + SKY_RGB + ",.08);box-shadow:0 0 26px rgba(" + SKY_RGB + ",.45);transform-origin:100% 100%;" +
      "animation:ghostlight-capframe " + CAPFRAME_MS + "ms cubic-bezier(.5,0,.2,1) forwards";
    addEphemeral(frame, CAPFRAME_MS + 60);
    caption("Screenshot");
  }

  // zoom: a magnifier frame closes on the captured region (coordinates in CSS viewport px).
  function zoomFrame(x0, y0, x1, y1) {
    if (document.hidden) return;
    ensureStyles();
    const w = Math.max(2, x1 - x0), h = Math.max(2, y1 - y0);
    const el = document.createElement("div");
    el.id = FX_LAYER_ID + "-zf" + fxSeq++;
    el.setAttribute("aria-hidden", "true");
    el.style.cssText =
      "position:fixed;pointer-events:none;left:" + x0 + "px;top:" + y0 + "px;width:" + w + "px;height:" + h + "px;" +
      "border:2px solid rgba(" + SKY_RGB + ",.9);border-radius:6px;box-shadow:0 0 22px rgba(" + SKY_RGB + ",.5);" +
      "animation:ghostlight-zoomframe " + ZOOMFRAME_MS + "ms cubic-bezier(.22,1,.36,1) forwards";
    addEphemeral(el, ZOOMFRAME_MS + 60);
    caption("Zoom");
  }

  // wait: a soft breathing dot while the agent pauses.
  function waitPulse() {
    if (hiddenForTool || document.hidden) return;
    ensureStyles();
    const el = document.createElement("div");
    el.id = FX_LAYER_ID + "-w" + fxSeq++;
    el.setAttribute("aria-hidden", "true");
    el.style.cssText =
      "position:fixed;left:50%;top:50%;width:16px;height:16px;border-radius:50%;transform:translate(-50%,-50%);" +
      "pointer-events:none;background:" + SKY + ";box-shadow:0 0 18px rgba(" + SKY_RGB + ",.8);" +
      "animation:ghostlight-breath 1500ms ease-in-out 2";
    addEphemeral(el, 3200);
    caption("Waiting");
  }

  chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
    // Master switch: with effects off, swallow every render message (capture-management and the
    // caption preference still work; non-ours messages fall through to content.js below).
    if (!effectsEnabled) {
      switch (msg && msg.type) {
        case "UPDATE_PHANTOM_CURSOR":
        case "AGENT_CLICK_RIPPLE":
        case "AGENT_DRAG_TRAIL":
        case "AGENT_TYPE_SHIMMER":
        case "AGENT_TARGET_GLOW":
        case "AGENT_KEYSTROKE":
        case "AGENT_SCROLL_CUE":
        case "AGENT_READ_SCAN":
        case "AGENT_NAVIGATE_PILL":
        case "AGENT_SCREENSHOT_FX":
        case "AGENT_ZOOM_FRAME":
        case "AGENT_WAIT_PULSE":
        case "SHOW_AGENT_INDICATORS":
          sendResponse({ success: true });
          return true;
      }
    }
    switch (msg && msg.type) {
      case "UPDATE_PHANTOM_CURSOR":
        moveCursor(msg.x, msg.y).then(() => sendResponse({ success: true }));
        return true; // respond asynchronously (after the cursor settles)
      case "AGENT_CLICK_RIPPLE":
        spawnRipples(msg.x, msg.y, msg.count, msg.button); sendResponse({ success: true }); return true;
      case "AGENT_DRAG_TRAIL":
        addTrailDot(msg.x, msg.y); sendResponse({ success: true }); return true;
      case "AGENT_TYPE_SHIMMER":
        shimmerFocused(); sendResponse({ success: true }); return true;
      case "AGENT_TARGET_GLOW":
        targetGlow(msg.x, msg.y); sendResponse({ success: true }); return true;
      case "AGENT_KEYSTROKE":
        keystrokeLozenge(msg.text, msg.kind); sendResponse({ success: true }); return true;
      case "AGENT_SCROLL_CUE":
        scrollCue(msg.direction); sendResponse({ success: true }); return true;
      case "AGENT_READ_SCAN":
        readScan(); sendResponse({ success: true }); return true;
      case "AGENT_NAVIGATE_PILL":
        navigatePill(msg.url); sendResponse({ success: true }); return true;
      case "AGENT_SCREENSHOT_FX":
        screenshotFx(); sendResponse({ success: true }); return true;
      case "AGENT_ZOOM_FRAME":
        zoomFrame(msg.x0, msg.y0, msg.x1, msg.y1); sendResponse({ success: true }); return true;
      case "AGENT_WAIT_PULSE":
        waitPulse(); sendResponse({ success: true }); return true;
      case "SET_CAPTIONS":
        captionsEnabled = !!msg.enabled; sendResponse({ success: true }); return true;
      case "SHOW_AGENT_INDICATORS":
        showGlow(); sendResponse({ success: true }); return true;
      case "HIDE_AGENT_INDICATORS":
        hideGlow(); sendResponse({ success: true }); return true;
      case "HIDE_FOR_TOOL_USE":
        setHiddenForTool(true); sendResponse({ success: true }); return true;
      case "SHOW_AFTER_TOOL_USE":
        setHiddenForTool(false); sendResponse({ success: true }); return true;
      default:
        return false; // not ours -- let content.js handle it
    }
  });

  // Visual-feedback preferences (extension options / popup): the effects master switch (default on)
  // and the captions subtitle (default off), read on load and reacted to live, so they survive
  // navigation without a per-page message.
  try {
    chrome.storage.local.get(["ghostlight_effects", "ghostlight_captions"], function (r) {
      if (r) {
        effectsEnabled = r.ghostlight_effects !== false;
        captionsEnabled = !!r.ghostlight_captions;
      }
    });
    chrome.storage.onChanged.addListener(function (changes, area) {
      if (area !== "local") return;
      if (changes.ghostlight_effects) effectsEnabled = changes.ghostlight_effects.newValue !== false;
      if (changes.ghostlight_captions) captionsEnabled = !!changes.ghostlight_captions.newValue;
    });
  } catch (e) { /* storage unavailable: effects on, captions off */ }

  window.addEventListener("beforeunload", () => { hideGlow(); });
})();
