// Ghostlight -- the setup page. It places the words GhostlightSetupView chose, follows the
// connection the worker reports, plays the meeting once when this computer and the browser
// connect, and nothing else. It holds no policy and fetches nothing.
(function ghostlightSetupPage() {
  "use strict";

  const view = globalThis.GhostlightSetupView;
  const shared = globalThis.GhostlightShared;
  const { STATE, STEP, LIGHT, WORDS, LINKS } = view;

  const MESSAGE = Object.freeze({
    SNAPSHOT: "ui_snapshot",
    STATE_CHANGED: "ui_state_changed",
    SHOW: view.SHOW_GHOSTLIGHT
  });
  // Set once the meeting has played, so a later visit shows the connected page at rest.
  const SETUP_MET_KEY = "ghostlight.setup_met";
  const POLL_MS = 1500;
  const COPIED_MS = 1600;
  const MEETING_SWAP_MS = 320;
  const ANNOUNCE_DELAY_MS = 60;

  const ICON = Object.freeze({
    check: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>',
    copy: '<svg class="gl-copy__idle" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" '
      + 'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="11" height="11" rx="2"/>'
      + '<path d="M5 15V6a2 2 0 0 1 2-2h9"/></svg>',
    copied: '<svg class="gl-copy__done" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" '
      + 'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>',
    spinner: '<svg viewBox="0 0 24 24" class="gl-spin" aria-hidden="true"><circle cx="12" cy="12" r="8" stroke-dasharray="26 50"/></svg>',
    arrow: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" '
      + 'stroke-linejoin="round" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6"/></svg>'
  });

  const companion = document.getElementById("companion");
  const discs = [...document.querySelectorAll(".gl-stepper__disc")];
  const segments = [...document.querySelectorAll(".gl-stepper__seg")];
  const names = [...document.querySelectorAll(".gl-stepper__names > li")];
  const main = document.getElementById("main");
  const live = document.getElementById("live");
  const after = document.getElementById("after");
  const announcer = document.getElementById("announce");

  const openedAt = Date.now();
  const reducedMotion = globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true;
  let platform = null;
  let current = null;
  let signature = null;
  let metPlayed = false;
  let copiedAt = null;
  let fallbackShown = false;
  let meetingTimer = null;
  let announceTimer = null;
  let latestSequence = 0;
  let appliedSequence = 0;

  function element(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  }

  // The icon strings above are constants in this file; nothing from the page or the worker is
  // ever parsed as markup.
  function svg(markup) {
    const holder = document.createElement("template");
    holder.innerHTML = markup;
    return holder.content.firstElementChild;
  }

  function icon(markup, className) {
    const holder = element("span", className);
    holder.append(svg(markup));
    return holder;
  }

  function announce(text) {
    clearTimeout(announceTimer);
    announcer.textContent = "";
    announceTimer = setTimeout(() => { announcer.textContent = text; }, ANNOUNCE_DELAY_MS);
  }

  function headlineText(model) {
    const { lead, accent, trail } = model.headline;
    return `${lead}${accent}${trail}`;
  }

  // ---- the companion: the ghost's light, the three discs and their names ------------------

  function renderCompanion(model, popped = []) {
    companion.dataset.light = model.light;
    model.steps.forEach((entry, index) => {
      const disc = discs[index];
      disc.className = `gl-stepper__disc gl-stepper__disc--${index}`
        + (entry.state === STEP.TODO ? "" : ` is-${entry.state}`)
        + (popped.includes(index) ? " is-pop" : "");
      if (entry.state === STEP.DONE) {
        disc.replaceChildren(svg(ICON.check));
      } else {
        disc.textContent = `0${index + 1}`;
      }
      const item = names[index];
      item.className = entry.state === STEP.TODO ? "" : `is-${entry.state}`;
      if (entry.state === STEP.CURRENT) item.setAttribute("aria-current", "step");
      else item.removeAttribute("aria-current");
      item.querySelector(".gl-sr").textContent = WORDS.stepSuffix[entry.state];
      item.querySelector(".gl-stepper__w").textContent = entry.word;
    });
    const done = model.steps.map((entry) => entry.state === STEP.DONE);
    segments.forEach((segment, index) => {
      segment.classList.toggle("is-lit", done[index] && done[index + 1]);
    });
  }

  function celebrate() {
    companion.classList.remove("is-celebrating");
    void companion.offsetWidth;
    companion.classList.add("is-celebrating");
  }

  // ---- the right column --------------------------------------------------------------------

  function selectText(node) {
    const range = document.createRange();
    range.selectNodeContents(node);
    const selection = globalThis.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  }

  // A panel with a command and its copy button. onCopied runs after a successful copy only.
  function codePanel(command, onCopied) {
    const panel = element("div", "gl-code");
    panel.append(element("span", "gl-code__label", command.label));
    const text = element("code", "gl-code__text");
    if (command.prompt) {
      const glyph = element("span", "gl-code__p", "$ ");
      glyph.setAttribute("aria-hidden", "true");
      text.append(glyph);
    }
    const body = element("span", "", command.text);
    text.append(body);
    const button = element("button", "gl-copy");
    button.type = "button";
    button.setAttribute("aria-label", WORDS.copyName);
    button.append(svg(ICON.copy), svg(ICON.copied));
    const label = element("span", "gl-copy__lbl", WORDS.copy);
    button.append(label);
    let resetTimer = null;
    button.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(command.text);
      } catch {
        selectText(body);
        return;
      }
      clearTimeout(resetTimer);
      button.classList.add("is-copied");
      label.textContent = WORDS.copied;
      announce(WORDS.copied);
      onCopied?.();
      resetTimer = setTimeout(() => {
        button.classList.remove("is-copied");
        label.textContent = WORDS.copy;
      }, COPIED_MS);
    });
    panel.append(text, button);
    return panel;
  }

  function renderMain(model, swap) {
    const head = element("header", "gl-head");
    head.append(element("span", "gl-eyebrow", model.eyebrow));
    const heading = element("h1", "gl-h1 gl-setup__h1");
    heading.append(model.headline.lead);
    if (model.headline.accent) heading.append(element("em", "", model.headline.accent));
    heading.append(model.headline.trail);
    head.append(heading);
    if (model.lead) head.append(element("p", "gl-lead", model.lead));
    const parts = [head];
    if (model.command) parts.push(codePanel(model.command, () => { copiedAt = Date.now(); }));
    if (model.action) {
      const row = element("div", "gl-btn-row");
      const button = element("button", "gl-btn gl-btn--primary", model.action.label);
      button.type = "button";
      button.append(svg(ICON.arrow));
      button.addEventListener("click", () => {
        chrome.runtime.sendMessage({ kind: model.action.kind }).catch(() => {});
      });
      row.append(button);
      parts.push(row);
    }
    main.replaceChildren(...parts);
    if (swap) {
      main.classList.remove("gl-swap");
      void main.offsetWidth;
      main.classList.add("gl-swap");
    }
  }

  function renderLive(model) {
    if (!model.live) {
      live.replaceChildren();
      return;
    }
    live.replaceChildren(icon(ICON.spinner, "gl-live__icon"), element("span", "", model.live));
  }

  function renderFallback(model) {
    if (!model.fallback) {
      after.replaceChildren();
      fallbackShown = false;
      return;
    }
    const due = view.fallbackDue({
      openedAt,
      copiedAt,
      now: Date.now(),
      visible: document.visibilityState === "visible",
      connected: model.state === STATE.MET
    });
    // Once shown it stays until Ghostlight answers; a callout that vanishes while read is worse.
    if (fallbackShown || !due) return;
    fallbackShown = true;
    const callout = element("div", "gl-callout gl-callout--warn gl-fallback gl-arrived");
    callout.append(
      element("p", "gl-h3", model.fallback.title),
      element("p", "gl-text", model.fallback.text),
      codePanel(model.fallback.command)
    );
    after.replaceChildren(callout);
  }

  // ---- following the connection ------------------------------------------------------------

  function render(model) {
    renderCompanion(model);
    renderMain(model, false);
    renderLive(model);
    renderFallback(model);
  }

  // The first time this computer and the browser meet while the page is open: disc 01 fills, the
  // lantern flares, and the right column changes to the connected words a beat later.
  function playMeeting(model) {
    clearTimeout(meetingTimer);
    renderCompanion(model, [0]);
    celebrate();
    renderLive(model);
    renderFallback(model);
    meetingTimer = setTimeout(() => {
      renderMain(model, true);
      announce(WORDS.met.announcement);
    }, MEETING_SWAP_MS);
    markMeetingPlayed();
  }

  function markMeetingPlayed() {
    metPlayed = true;
    chrome.storage.local.set({ [SETUP_MET_KEY]: true }).catch(() => {});
  }

  function apply(model) {
    const next = JSON.stringify(model);
    if (current === null) {
      render(model);
    } else if (model.state === STATE.MET && current.state !== STATE.MET && !metPlayed && !reducedMotion) {
      playMeeting(model);
    } else if (next !== signature) {
      clearTimeout(meetingTimer);
      render(model);
      if (model.state === STATE.MET && current.state !== STATE.MET) {
        if (!metPlayed) markMeetingPlayed();
        announce(WORDS.met.announcement);
      } else if (model.state !== current.state) {
        announce(headlineText(model) + (model.lead ? ` ${model.lead}` : ""));
      }
    } else {
      renderFallback(model);
    }
    current = model;
    signature = next;
  }

  async function request(message) {
    const response = await chrome.runtime.sendMessage(message);
    if (!response?.ok) throw new Error(response?.error || "Ghostlight did not respond.");
    return response.value;
  }

  async function refresh() {
    const sequence = ++latestSequence;
    let snapshot;
    try {
      snapshot = await request({ kind: MESSAGE.SNAPSHOT });
    } catch {
      snapshot = { connected: false, compatible: true, link_state: shared.LINK_UNREACHABLE };
    }
    if (sequence < appliedSequence) return;
    appliedSequence = sequence;
    apply(view.view({
      link_state: snapshot.link_state,
      connected: snapshot.connected,
      compatible: snapshot.compatible,
      platform,
      browser: shared.browserName(navigator.userAgentData?.brands)
    }));
  }

  async function start() {
    const link = document.getElementById("doc-link");
    link.textContent = LINKS.documentation.label;
    link.href = LINKS.documentation.url;
    link.hidden = false;
    document.querySelectorAll(".gl-stepper__t").forEach((node, index) => {
      node.textContent = WORDS.steps[index];
    });
    try {
      platform = (await chrome.runtime.getPlatformInfo()).os;
    } catch {
      platform = null;
    }
    try {
      metPlayed = (await chrome.storage.local.get(SETUP_MET_KEY))[SETUP_MET_KEY] === true;
    } catch {
      metPlayed = false;
    }
    chrome.runtime.onMessage.addListener((message) => {
      if (message?.kind === MESSAGE.STATE_CHANGED) refresh();
    });
    document.addEventListener("visibilitychange", () => { if (current) renderFallback(current); });
    setInterval(refresh, POLL_MS);
    await refresh();
  }

  start();
})();
