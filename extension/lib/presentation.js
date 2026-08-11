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
    style.textContent = `
      :host{all:initial}*{box-sizing:border-box}
      .surface{position:fixed;inset:0;pointer-events:none;color:${INK};font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
      .scope{position:fixed;inset:0;opacity:0;transition:opacity .3s ease-in-out;box-shadow:inset 0 0 14px rgba(${SKY_RGB},.7),inset 0 0 26px rgba(${SKY_RGB},.35)}
      .scope.on{animation:ghostlight-control-breathe 4s ease-in-out infinite}
      .cursor{position:fixed;left:0;top:0;width:22px;height:28px;opacity:0;filter:drop-shadow(0 0 3px rgba(${SKY_RGB},.9)) drop-shadow(0 0 8px rgba(${SKY_RGB},.5));transform:translate3d(-100px,-100px,0);transition:transform 150ms cubic-bezier(.2,0,0,1),opacity 120ms ease-out;will-change:transform}
      .cursor.on{opacity:1}
      .fx,.signatures,.denials{position:fixed;inset:0;pointer-events:none}
      .caption{position:fixed;left:50%;bottom:22px;z-index:4;opacity:0;transform:translate(-50%,8px);padding:6px 13px;border:1px solid rgba(${SKY_RGB},.4);border-radius:999px;color:${INK};background:rgba(10,16,26,.82);font:12px/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;transition:opacity .2s ease,transform .2s ${SPRING}}
      .caption.on{opacity:1;transform:translate(-50%,0)}
      .target-glow{position:fixed;border-radius:8px;box-shadow:0 0 0 2px rgba(${SKY_RGB},.9),0 0 20px rgba(${SKY_RGB},.55);animation:ghostlight-targetglow 720ms ease-out forwards}
      .ripple{position:fixed;width:34px;height:34px;border:2px solid rgba(${SKY_RGB},.9);border-radius:50%;box-shadow:0 0 12px rgba(${SKY_RGB},.55),inset 0 0 8px rgba(${SKY_RGB},.35);transform:translate(-50%,-50%) scale(.3);animation:ghostlight-ripple 620ms ease-out forwards}
      .trail-dot{position:fixed;width:14px;height:14px;border-radius:50%;transform:translate(-50%,-50%);background:radial-gradient(circle,rgba(${SKY_RGB},.9) 0%,rgba(${SKY_RGB},0) 70%);animation:ghostlight-trail 520ms ease-out forwards}
      .field-shimmer{position:fixed;border:1.5px solid rgba(${SKY_RGB},.85);border-radius:6px;box-shadow:0 0 10px rgba(${SKY_RGB},.5),inset 0 0 8px rgba(${SKY_RGB},.25);animation:ghostlight-shimmer 900ms ease-in-out forwards}
      .field-splash{position:fixed;border:2px solid rgba(${SKY_RGB},.9);border-radius:8px;background:radial-gradient(ellipse at center,rgba(${SKY_RGB},.26) 0%,rgba(${SKY_RGB},.08) 55%,rgba(${SKY_RGB},0) 78%);box-shadow:0 0 14px rgba(${SKY_RGB},.55),inset 0 0 10px rgba(${SKY_RGB},.3);transform-origin:center;animation:ghostlight-fieldsplash 700ms ${SPRING} forwards}
      .chevrons{position:fixed;left:50%;top:50%;display:flex;flex-direction:column;align-items:center;gap:1px;transform:translate(-50%,-50%)}
      .chevrons svg{opacity:0;animation:ghostlight-chev 900ms ease-out forwards}
      .chevrons svg:nth-child(2){animation-delay:100ms}.chevrons svg:nth-child(3){animation-delay:200ms}
      .read-scan{position:fixed;left:0;right:0;top:0;height:80px;background:linear-gradient(180deg,transparent,rgba(${SKY_RGB},.15) 62%,rgba(${SKY_RGB},.8));box-shadow:0 6px 20px rgba(${SKY_RGB},.35);animation:ghostlight-scan 1450ms cubic-bezier(.4,0,.5,1) forwards}
      .nav-pill{position:fixed;left:50%;top:16px;z-index:4;max-width:min(88vw,640px);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;padding:8px 15px;border:1px solid rgba(${SKY_RGB},.5);border-radius:999px;color:${INK};background:rgba(10,16,26,.9);box-shadow:0 12px 30px -12px rgba(${SKY_RGB},.8);font:12px/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;animation:ghostlight-nav 1600ms ease-out forwards}
      .nav-arrow{margin-right:7px;color:${SKY}}
      .key-lozenge{position:fixed;left:50%;bottom:64px;z-index:4;display:flex;align-items:center;gap:10px;padding:8px 14px;border:1px solid rgba(${SKY_RGB},.55);border-radius:10px;color:${INK};background:rgba(12,20,32,.9);box-shadow:0 10px 30px -12px rgba(${SKY_RGB},.8);font:600 14px/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;animation:ghostlight-lozenge 1250ms ${SPRING} forwards}
      .private-keycap{display:block;width:16px;height:14px;border:1px solid rgba(${SKY_RGB},.72);border-radius:4px;background:rgba(${SKY_RGB},.12);box-shadow:0 0 9px rgba(${SKY_RGB},.56),inset 0 -2px 0 rgba(${SKY_RGB},.18)}
      .capture-flash{position:fixed;inset:0;background:rgba(${SKY_RGB},.42);animation:ghostlight-flash 260ms ease-out forwards}
      .capture-frame{position:fixed;inset:8px;border:2px solid rgba(${SKY_RGB},.9);border-radius:8px;background:rgba(${SKY_RGB},.08);box-shadow:0 0 26px rgba(${SKY_RGB},.45);transform-origin:100% 100%;animation:ghostlight-capframe 1500ms cubic-bezier(.5,0,.2,1) forwards}
      .zoom-frame{position:fixed;border:2px solid rgba(${SKY_RGB},.9);border-radius:6px;box-shadow:0 0 22px rgba(${SKY_RGB},.5);animation:ghostlight-zoomframe 1150ms ${SPRING} forwards}
      .signature{position:absolute;right:18px;top:18px;width:58px;height:58px;display:grid;place-items:center;overflow:visible;border:1px solid rgba(${SKY_RGB},.58);border-radius:18px;color:${SKY};background:rgba(10,16,26,.92);box-shadow:0 12px 32px -14px rgba(${SKY_RGB},.9),inset 0 1px 0 rgba(255,255,255,.09),0 0 18px -8px rgba(${SKY_RGB},.75);opacity:1;transform:scale(1);transition:opacity 320ms ease-out,transform 420ms ${SPRING};will-change:opacity,transform}
      .signature.entering{opacity:0;transform:scale(.84)}.signature.leaving{opacity:0;transform:scale(.9)}
      .signature-icon{position:relative;width:100%;height:100%;display:grid;place-items:center}
      .workwheel{width:35px;height:35px;animation:ghostlight-signature-gear 2400ms linear infinite}
      .particle{position:absolute;width:5px;height:5px;border-radius:50%;background:${SKY};box-shadow:0 0 8px rgba(${SKY_RGB},.9);animation:ghostlight-signature-particle 1300ms ease-in-out infinite}
      .particle.p1{right:7px;top:6px}.particle.p2{right:4px;bottom:12px;animation-delay:180ms}.particle.p3{left:7px;bottom:6px;animation-delay:360ms}
      .keyboard{width:34px;height:24px;filter:drop-shadow(0 0 3px rgba(${SKY_RGB},.35));animation:ghostlight-signature-keyboard 1150ms ease-in-out infinite}
      .wait-lights{display:flex;align-items:center;gap:5px}.wait-lights span{width:7px;height:7px;border-radius:50%;background:${SKY};box-shadow:0 0 8px rgba(${SKY_RGB},.8);animation:ghostlight-signature-dot 1050ms ease-in-out infinite}.wait-lights span:nth-child(2){animation-delay:150ms}.wait-lights span:nth-child(3){animation-delay:300ms}
      .camera,.lens{width:31px;height:31px;filter:drop-shadow(0 0 6px rgba(${SKY_RGB},.75))}.lens{animation:ghostlight-find-lens 1250ms ease-in-out infinite}
      .glint{position:absolute;left:50%;top:50%;width:50px;height:50px;border-radius:50%;opacity:0;background:conic-gradient(from 0deg,transparent 0 78%,rgba(255,255,255,.95) 86%,transparent 94%)}
      .signature.completing .glint,.signature.confirming .glint{animation:ghostlight-signature-glint 520ms ${SPRING} 1}
      .signature.completing .workwheel,.signature.completing .keyboard,.signature.completing .lens{animation-play-state:paused}
      .denials{z-index:5;display:grid;place-items:center}.denial-ribbon{--gl-rgb:239,68,68;display:flex;align-items:center;justify-content:center;gap:12px;width:min(88vw,620px);padding:14px 20px;border-radius:16px;color:${INK};background:${GROUND};box-shadow:0 14px 44px -16px rgba(var(--gl-rgb),.75),inset 0 1px 0 rgba(255,255,255,.12);transform-origin:center;animation:ghostlight-notif-grow 320ms ${SPRING} forwards}.denial-badge{flex:0 0 auto;width:52px;height:52px;display:grid;place-items:center;border-radius:50%;color:rgb(var(--gl-rgb));background:rgb(var(--gl-rgb));box-shadow:0 4px 16px rgba(0,0,0,.35)}.denial-badge svg{width:72%;height:auto}.denial-title{font:600 16px/1.3 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}.denial-description{margin-top:2px;color:${INK};opacity:0;font:13px/1.35 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;animation:ghostlight-notif-desc 320ms ease-out 220ms forwards}
      .attention{position:fixed;inset:0;z-index:6;display:none;place-items:center;padding:24px;pointer-events:auto;background:rgba(5,10,18,.55);backdrop-filter:blur(5px) saturate(.7)}.attention.on{display:grid}.attention-card{width:min(92vw,560px);padding:24px;border:1px solid rgba(239,68,68,.7);border-radius:22px;color:${INK};background:rgba(10,16,26,.97);box-shadow:0 24px 80px -24px rgba(239,68,68,.75);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}.attention-icon{display:grid;place-items:center;width:58px;height:58px;margin:0 auto 14px;border-radius:50%;color:#fff;background:#ef4444;box-shadow:0 0 28px rgba(239,68,68,.55);font:800 30px/1 system-ui}.attention-card h2{margin:0;text-align:center;font-size:22px}.attention-card p{margin:8px 0 20px;color:#cbd5e1;text-align:center;font-size:14px;line-height:1.45}.attention-actions{display:grid;grid-template-columns:1fr 1fr;gap:9px}.attention-actions button{min-height:42px;padding:9px 12px;border:1px solid rgba(148,163,184,.38);border-radius:10px;color:${INK};background:#172033;font:600 12px/1.25 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;cursor:pointer}.attention-actions button:hover,.attention-actions button:focus-visible{border-color:${SKY};background:#1d2b42;outline:none}.attention-actions .danger{border-color:rgba(239,68,68,.6);color:#fecaca}
      @keyframes ghostlight-control-breathe{0%,100%{opacity:.58}50%{opacity:.82}}
      @keyframes ghostlight-ripple{0%{opacity:.85;transform:translate(-50%,-50%) scale(.3)}100%{opacity:0;transform:translate(-50%,-50%) scale(2.8)}}
      @keyframes ghostlight-trail{0%{opacity:.9;transform:translate(-50%,-50%) scale(1)}100%{opacity:0;transform:translate(-50%,-50%) scale(.55)}}
      @keyframes ghostlight-shimmer{0%{opacity:0;transform:scale(.985)}25%{opacity:1;transform:scale(1)}60%{opacity:.7}100%{opacity:0;transform:scale(1.015)}}
      @keyframes ghostlight-fieldsplash{0%{opacity:0;transform:scale(.97)}18%{opacity:1;transform:scale(1)}62%{opacity:.85}100%{opacity:0;transform:scale(1.05)}}
      @keyframes ghostlight-targetglow{0%{opacity:0;transform:scale(.94)}22%{opacity:1;transform:scale(1)}100%{opacity:0;transform:scale(1.06)}}
      @keyframes ghostlight-chev{0%{opacity:0;transform:translateY(-8px)}30%{opacity:1}100%{opacity:0;transform:translateY(10px)}}
      @keyframes ghostlight-scan{0%{opacity:0;transform:translateY(-80px)}12%{opacity:1}90%{opacity:1}100%{opacity:0;transform:translateY(100vh)}}
      @keyframes ghostlight-nav{0%{opacity:0;transform:translate(-50%,-14px)}14%{opacity:1;transform:translate(-50%,0)}82%{opacity:1;transform:translate(-50%,0)}100%{opacity:0;transform:translate(-50%,-8px)}}
      @keyframes ghostlight-lozenge{0%{opacity:0;transform:translate(-50%,12px)}16%{opacity:1;transform:translate(-50%,0)}78%{opacity:1;transform:translate(-50%,0)}100%{opacity:0;transform:translate(-50%,-6px)}}
      @keyframes ghostlight-flash{0%{opacity:.42}100%{opacity:0}}
      @keyframes ghostlight-capframe{0%{opacity:0;transform:scale(1.03)}9%{opacity:1;transform:scale(1)}34%{opacity:1;transform:scale(1)}60%{opacity:1;transform:scale(.17);border-radius:16px}88%{opacity:1;transform:scale(.17);border-radius:16px}100%{opacity:0;transform:scale(.17);border-radius:16px}}
      @keyframes ghostlight-zoomframe{0%{opacity:0;transform:scale(1.35)}22%{opacity:1}70%{opacity:1;transform:scale(1)}100%{opacity:0;transform:scale(1)}}
      @keyframes ghostlight-signature-gear{to{transform:rotate(360deg)}}
      @keyframes ghostlight-signature-particle{0%,100%{opacity:.28;transform:scale(.65)}50%{opacity:1;transform:scale(1.18)}}
      @keyframes ghostlight-signature-keyboard{0%,100%{transform:translateY(0);filter:drop-shadow(0 0 3px rgba(${SKY_RGB},.35))}50%{transform:translateY(-1px);filter:drop-shadow(0 0 9px rgba(${SKY_RGB},.95))}}
      @keyframes ghostlight-signature-dot{0%,20%,100%{opacity:.25;transform:translateY(1px) scale(.72)}50%{opacity:1;transform:translateY(-1px) scale(1)}}
      @keyframes ghostlight-signature-glint{0%{opacity:0;transform:translate(-50%,-50%) rotate(-80deg)}25%{opacity:1}100%{opacity:0;transform:translate(-50%,-50%) rotate(250deg)}}
      @keyframes ghostlight-find-lens{0%,100%{transform:translate(-1px,-1px) rotate(-7deg)}50%{transform:translate(2px,2px) rotate(5deg)}}
      @keyframes ghostlight-notif-grow{0%{opacity:0;transform:translateY(10px) scale(.96)}100%{opacity:1;transform:none}}
      @keyframes ghostlight-notif-desc{0%{opacity:0}100%{opacity:.85}}
      @media(max-width:520px){.attention-actions{grid-template-columns:1fr}}
      @media(prefers-reduced-motion:reduce){.scope.on{animation:none;opacity:.7}.cursor{transition:none}.ripple{animation-name:ghostlight-fade}.trail-dot,.field-shimmer,.field-splash,.target-glow,.chevrons svg,.read-scan,.nav-pill,.key-lozenge,.capture-flash,.capture-frame,.zoom-frame,.workwheel,.particle,.keyboard,.wait-lights span,.lens,.glint{animation-name:ghostlight-fade!important;animation-duration:180ms!important}.signature{transition:opacity 180ms ease-out}.denial-ribbon{animation:none!important}.denial-description{animation:none!important;opacity:.85}}
      @keyframes ghostlight-fade{0%{opacity:0}50%{opacity:.8}100%{opacity:0}}
    `;

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
    scope.classList.toggle("on", managed && runtimeReachable);
  }

  function addEffect(className, styles, lifetime) {
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
    setTimeout(remove, lifetime);
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
    const effect = addEffect("target-glow", {}, 780);
    placeRectangle(effect, paddedRectangle(rectangle, 4));
  }

  function clickRipple(rectangle) {
    const point = center(rectangle);
    addEffect("ripple", { left: `${point.x}px`, top: `${point.y}px` }, 700);
  }

  function dragTrail(rectangle) {
    const { previous, point } = moveCursor(rectangle);
    for (let step = 0; step < 12; step += 1) {
      const ratio = (step + 1) / 12;
      const x = previous.x + (point.x - previous.x) * ratio;
      const y = previous.y + (point.y - previous.y) * ratio;
      setTimeout(() => addEffect("trail-dot", { left: `${x}px`, top: `${y}px` }, 600), step * 22);
    }
  }

  function fieldEffect(rectangle, treatment) {
    if (!rectangle) return;
    const padding = treatment === "field-splash" ? 4 : 3;
    const effect = addEffect(treatment, {}, treatment === "field-splash" ? 780 : 1000);
    placeRectangle(effect, paddedRectangle(rectangle, padding));
  }

  function scrollCue(rectangle) {
    const point = center(rectangle);
    const effect = addEffect("chevrons", {}, 1150);
    effect.style.left = `${point.x}px`;
    effect.style.top = `${point.y}px`;
    effect.innerHTML = chevron + chevron + chevron;
    if (rectangle) targetGlow(rectangle);
  }

  function readScan() {
    addEffect("read-scan", {}, 1510);
  }

  function navigationPill() {
    const path = `${location.host}${location.pathname === "/" ? "" : location.pathname}` || "this page";
    const pill = addEffect("nav-pill", {}, 1640);
    const arrow = document.createElement("span");
    arrow.className = "nav-arrow";
    arrow.textContent = "->";
    const destination = document.createElement("span");
    destination.textContent = path.slice(0, 58);
    pill.append(arrow, destination);
  }

  function keyLozenge() {
    const lozenge = addEffect("key-lozenge", {}, 1290);
    const keycap = document.createElement("span");
    keycap.className = "private-keycap";
    lozenge.appendChild(keycap);
  }

  function screenshotEffect() {
    addEffect("capture-flash", {}, 320);
    addEffect("capture-frame", {}, 1560);
  }

  function zoomEffect(rectangle) {
    const region = rectangle || {
      left: innerWidth * 0.2,
      top: innerHeight * 0.2,
      width: innerWidth * 0.6,
      height: innerHeight * 0.6
    };
    const effect = addEffect("zoom-frame", {}, 1210);
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
        clickRipple(rectangle);
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
    setRuntimeState,
    visualIdentity
  });
});
