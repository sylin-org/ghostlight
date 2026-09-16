// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- the in-page presentation stylesheet, its own module.
//
// Extracted verbatim from lib/presentation.js, which now calls build() with the two values
// the stylesheet is allowed to receive: the identity custom-property block and the
// reduced-motion selector generated from the effect registry. Everything else is static CSS
// with no interpolation, so the vocabulary reads as a dictionary and a colour or curve changes
// in exactly one place. tests/shared.test.js pins that shape.
(function installGhostlightPresentationCss(root) {
  "use strict";

  root.GhostlightPresentationCss = Object.freeze({
    build(tokens, reducedFadeSelector) {
      return `
      :host{all:initial;${tokens}}*{box-sizing:border-box}
      .surface{position:fixed;inset:0;pointer-events:none;color:var(--gl-ink);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
      .scope{position:fixed;inset:0;opacity:0;transition:opacity .3s ease-in-out;box-shadow:inset 0 0 14px rgba(var(--gl-argb),.7),inset 0 0 26px rgba(var(--gl-argb),.35)}
      .scope.on{animation:ghostlight-control-breathe 4s ease-in-out infinite}
      .cursor{position:fixed;left:0;top:0;width:22px;height:28px;opacity:0;filter:drop-shadow(0 0 3px rgba(var(--gl-argb),.9)) drop-shadow(0 0 8px rgba(var(--gl-argb),.5));transform:translate3d(-100px,-100px,0);transition:transform 150ms cubic-bezier(.2,0,0,1),opacity 120ms ease-out;will-change:transform}
      .cursor.on{opacity:1}
      .fx,.signatures,.denials{position:fixed;inset:0;pointer-events:none}
      .caption{position:fixed;left:50%;bottom:22px;z-index:4;opacity:0;transform:translate(-50%,8px);padding:6px 13px;border:1px solid rgba(var(--gl-argb),.4);border-radius:999px;color:var(--gl-ink);background:rgba(10,16,26,.82);font:12px/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;transition:opacity .2s ease,transform .2s var(--gl-spring)}
      .caption.on{opacity:1;transform:translate(-50%,0)}
      .target-glow{position:fixed;border-radius:8px;box-shadow:0 0 0 2px rgba(var(--gl-argb),.9),0 0 20px rgba(var(--gl-argb),.55);animation:ghostlight-targetglow 720ms ease-out forwards}
      .ripple.secondary{border-style:dashed}
      .ripple{position:fixed;width:34px;height:34px;border:2px solid rgba(var(--gl-argb),.9);border-radius:50%;box-shadow:0 0 12px rgba(var(--gl-argb),.55),inset 0 0 8px rgba(var(--gl-argb),.35);transform:translate(-50%,-50%) scale(.3);animation:ghostlight-ripple 620ms ease-out forwards}
      .trail-dot{position:fixed;width:14px;height:14px;border-radius:50%;transform:translate(-50%,-50%);background:radial-gradient(circle,rgba(var(--gl-argb),.9) 0%,rgba(var(--gl-argb),0) 70%);animation:ghostlight-trail 520ms ease-out forwards}
      .field-shimmer{position:fixed;border:1.5px solid rgba(var(--gl-argb),.85);border-radius:6px;box-shadow:0 0 10px rgba(var(--gl-argb),.5),inset 0 0 8px rgba(var(--gl-argb),.25);animation:ghostlight-shimmer 900ms ease-in-out forwards}
      .field-splash{position:fixed;border:2px solid rgba(var(--gl-argb),.9);border-radius:8px;background:radial-gradient(ellipse at center,rgba(var(--gl-argb),.26) 0%,rgba(var(--gl-argb),.08) 55%,rgba(var(--gl-argb),0) 78%);box-shadow:0 0 14px rgba(var(--gl-argb),.55),inset 0 0 10px rgba(var(--gl-argb),.3);transform-origin:center;animation:ghostlight-fieldsplash 700ms var(--gl-spring) forwards}
      .chevrons{position:fixed;left:50%;top:50%;display:flex;flex-direction:column;align-items:center;gap:1px;transform:translate(-50%,-50%)}
      .chevrons svg{opacity:0;animation:ghostlight-chev 900ms ease-out forwards}
      .chevrons svg:nth-child(2){animation-delay:100ms}.chevrons svg:nth-child(3){animation-delay:200ms}
      .read-scan{position:fixed;left:0;right:0;top:0;height:80px;background:linear-gradient(180deg,transparent,rgba(var(--gl-argb),.15) 62%,rgba(var(--gl-argb),.8));box-shadow:0 6px 20px rgba(var(--gl-argb),.35);animation:ghostlight-scan 1450ms cubic-bezier(.4,0,.5,1) forwards}
      .nav-pill{position:fixed;left:50%;top:16px;z-index:4;max-width:min(88vw,640px);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;padding:8px 15px;border:1px solid rgba(var(--gl-argb),.5);border-radius:999px;color:var(--gl-ink);background:rgba(10,16,26,.9);box-shadow:0 12px 30px -12px rgba(var(--gl-argb),.8);font:12px/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;animation:ghostlight-nav 1600ms ease-out forwards}
      .nav-arrow{margin-right:7px;color:var(--gl-sky)}
      .key-lozenge{position:fixed;left:50%;bottom:64px;z-index:4;display:flex;align-items:center;gap:10px;padding:8px 14px;border:1px solid rgba(var(--gl-argb),.55);border-radius:10px;color:var(--gl-ink);background:rgba(12,20,32,.9);box-shadow:0 10px 30px -12px rgba(var(--gl-argb),.8);font:600 14px/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;animation:ghostlight-lozenge 1250ms var(--gl-spring) forwards}
      .private-keycap{display:block;width:16px;height:14px;border:1px solid rgba(var(--gl-argb),.72);border-radius:4px;background:rgba(var(--gl-argb),.12);box-shadow:0 0 9px rgba(var(--gl-argb),.56),inset 0 -2px 0 rgba(var(--gl-argb),.18)}
      .capture-flash{position:fixed;inset:0;background:rgba(var(--gl-argb),.42);animation:ghostlight-flash 260ms ease-out forwards}
      .capture-frame{position:fixed;inset:8px;border:2px solid rgba(var(--gl-argb),.9);border-radius:8px;background:rgba(var(--gl-argb),.08);box-shadow:0 0 26px rgba(var(--gl-argb),.45);transform-origin:100% 100%;animation:ghostlight-capframe 1500ms cubic-bezier(.5,0,.2,1) forwards}
      .zoom-frame{position:fixed;border:2px solid rgba(var(--gl-argb),.9);border-radius:6px;box-shadow:0 0 22px rgba(var(--gl-argb),.5);animation:ghostlight-zoomframe 1150ms var(--gl-spring) forwards}
      .signature{position:absolute;right:18px;top:18px;width:58px;height:58px;display:grid;place-items:center;overflow:visible;border:1px solid rgba(var(--gl-argb),.58);border-radius:18px;color:var(--gl-sky);background:rgba(10,16,26,.92);box-shadow:0 12px 32px -14px rgba(var(--gl-argb),.9),inset 0 1px 0 rgba(255,255,255,.09),0 0 18px -8px rgba(var(--gl-argb),.75);opacity:1;transform:scale(1);transition:opacity 320ms ease-out,transform 420ms var(--gl-spring);will-change:opacity,transform}
      .signature.entering{opacity:0;transform:scale(.84)}.signature.leaving{opacity:0;transform:scale(.9)}
      .signature-icon{position:relative;width:100%;height:100%;display:grid;place-items:center}
      .workwheel{width:35px;height:35px;animation:ghostlight-signature-gear 2400ms linear infinite}
      .particle{position:absolute;width:5px;height:5px;border-radius:50%;background:var(--gl-sky);box-shadow:0 0 8px rgba(var(--gl-argb),.9);animation:ghostlight-signature-particle 1300ms ease-in-out infinite}
      .particle.p1{right:7px;top:6px}.particle.p2{right:4px;bottom:12px;animation-delay:180ms}.particle.p3{left:7px;bottom:6px;animation-delay:360ms}
      .keyboard{width:34px;height:24px;filter:drop-shadow(0 0 3px rgba(var(--gl-argb),.35));animation:ghostlight-signature-keyboard 1150ms ease-in-out infinite}
      .wait-lights{display:flex;align-items:center;gap:5px}.wait-lights span{width:7px;height:7px;border-radius:50%;background:var(--gl-sky);box-shadow:0 0 8px rgba(var(--gl-argb),.8);animation:ghostlight-signature-dot 1050ms ease-in-out infinite}.wait-lights span:nth-child(2){animation-delay:150ms}.wait-lights span:nth-child(3){animation-delay:300ms}
      .camera,.lens{width:31px;height:31px;filter:drop-shadow(0 0 6px rgba(var(--gl-argb),.75))}.lens{animation:ghostlight-find-lens 1250ms ease-in-out infinite}
      .glint{position:absolute;left:50%;top:50%;width:50px;height:50px;border-radius:50%;opacity:0;background:conic-gradient(from 0deg,transparent 0 78%,rgba(255,255,255,.95) 86%,transparent 94%)}
      .signature.completing .glint,.signature.confirming .glint{animation:ghostlight-signature-glint 520ms var(--gl-spring) 1}
      .signature.completing .workwheel,.signature.completing .keyboard,.signature.completing .lens{animation-play-state:paused}
      .denials{z-index:5;display:grid;place-items:center}.denial-ribbon{--gl-rgb:239,68,68;display:flex;align-items:center;justify-content:center;gap:12px;width:min(88vw,620px);padding:14px 20px;border-radius:16px;color:var(--gl-ink);background:var(--gl-ground);box-shadow:0 14px 44px -16px rgba(var(--gl-rgb),.75),inset 0 1px 0 rgba(255,255,255,.12);transform-origin:center;animation:ghostlight-notif-grow 320ms var(--gl-spring) forwards}.denial-badge{flex:0 0 auto;width:52px;height:52px;display:grid;place-items:center;border-radius:50%;color:rgb(var(--gl-rgb));background:rgb(var(--gl-rgb));box-shadow:0 4px 16px rgba(0,0,0,.35)}.denial-badge svg{width:72%;height:auto}.denial-title{font:600 16px/1.3 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}.denial-description{margin-top:2px;color:var(--gl-ink);opacity:0;font:13px/1.35 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;animation:ghostlight-notif-desc 320ms ease-out 220ms forwards}
      .attention{position:fixed;inset:0;z-index:6;display:none;place-items:center;padding:24px;pointer-events:auto;background:rgba(5,10,18,.55);backdrop-filter:blur(5px) saturate(.7)}.attention.on{display:grid}.attention-card{width:min(92vw,560px);padding:24px;border:1px solid rgba(239,68,68,.7);border-radius:22px;color:var(--gl-ink);background:rgba(10,16,26,.97);box-shadow:0 24px 80px -24px rgba(239,68,68,.75);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}.attention-icon{display:grid;place-items:center;width:58px;height:58px;margin:0 auto 14px;border-radius:50%;color:#fff;background:#ef4444;box-shadow:0 0 28px rgba(239,68,68,.55);font:800 30px/1 system-ui}.attention-card h2{margin:0;text-align:center;font-size:22px}.attention-card p{margin:8px 0 20px;color:#cbd5e1;text-align:center;font-size:14px;line-height:1.45}.attention-actions{display:grid;grid-template-columns:1fr 1fr;gap:9px}.attention-actions button{min-height:42px;padding:9px 12px;border:1px solid rgba(148,163,184,.38);border-radius:10px;color:var(--gl-ink);background:#172033;font:600 12px/1.25 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;cursor:pointer}.attention-actions button:hover,.attention-actions button:focus-visible{border-color:var(--gl-sky);background:#1d2b42;outline:none}.attention-actions .danger{border-color:rgba(239,68,68,.6);color:#fecaca}
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
      @keyframes ghostlight-signature-keyboard{0%,100%{transform:translateY(0);filter:drop-shadow(0 0 3px rgba(var(--gl-argb),.35))}50%{transform:translateY(-1px);filter:drop-shadow(0 0 9px rgba(var(--gl-argb),.95))}}
      @keyframes ghostlight-signature-dot{0%,20%,100%{opacity:.25;transform:translateY(1px) scale(.72)}50%{opacity:1;transform:translateY(-1px) scale(1)}}
      @keyframes ghostlight-signature-glint{0%{opacity:0;transform:translate(-50%,-50%) rotate(-80deg)}25%{opacity:1}100%{opacity:0;transform:translate(-50%,-50%) rotate(250deg)}}
      @keyframes ghostlight-find-lens{0%,100%{transform:translate(-1px,-1px) rotate(-7deg)}50%{transform:translate(2px,2px) rotate(5deg)}}
      @keyframes ghostlight-notif-grow{0%{opacity:0;transform:translateY(10px) scale(.96)}100%{opacity:1;transform:none}}
      @keyframes ghostlight-notif-desc{0%{opacity:0}100%{opacity:.85}}
      @media(max-width:520px){.attention-actions{grid-template-columns:1fr}}
      @media(prefers-reduced-motion:reduce){.scope.on{animation:none;opacity:.7}.cursor{transition:none}.ripple{animation-name:ghostlight-fade}${reducedFadeSelector}{animation-name:ghostlight-fade!important;animation-duration:180ms!important}.signature{transition:opacity 180ms ease-out}.denial-ribbon{animation:none!important}.denial-description{animation:none!important;opacity:.85}}
      @keyframes ghostlight-fade{0%{opacity:0}50%{opacity:.8}100%{opacity:0}}
    `;
    }
  });
})(globalThis);
