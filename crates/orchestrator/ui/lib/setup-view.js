// Ghostlight workbench -- the setup view.
//
// While setup is under way the window shows this in place of its tabs: the mascot and the three
// steps on the left, and on the right what to do next. It draws a SetupView exactly as the service
// composed it -- every sentence, step, command and row -- and adds only the moments between two
// views: the meeting of the computer and the browser, the completion, and the answer to a click.
//
// It holds no authority. It fetches nothing and decides nothing about what the window shows:
// the page calls open, update and close, and hears about the person's clicks through callbacks.
// Time comes from the page (schedule, cancel, now), so a test can run the choreography by hand.
(function installGhostlightSetupView(root, factory) {
  const api = factory();
  root.GhostlightSetupView = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightSetupViewApi() {
  "use strict";

  /** Where the person is in the journey, as the service names it. */
  const STAGE = Object.freeze({
    computer: "computer",
    browser: "browser",
    agent: "agent",
    done: "done",
    retired: "retired"
  });

  /** The stages in which the person still has something to do. */
  const WAITING = Object.freeze([STAGE.computer, STAGE.browser, STAGE.agent]);

  /** What a transition between two views plays. */
  const PLAY = Object.freeze({ meet: "meet", finish: "finish" });

  /** The kinds of step the service asks the page to carry out. */
  const ACTION = Object.freeze({
    addInBrowser: "add_in_browser",
    openBrowser: "open_browser",
    getChrome: "get_chrome",
    connectAgent: "connect_agent",
    reviewAgent: "review_agent",
    copySetup: "copy_setup"
  });

  /** How long the browser step must have been on screen before a connection is worth a meeting. */
  const MEET_AFTER_MS = 2000;
  /** How long the browser stage rests after the meeting before the agent stage replaces it. */
  const MEET_SWAP_MS = 1900;
  /** When the tagline arrives after the completion, for a person who has motion turned off. */
  const TAGLINE_MS = 1050;
  /** How long the completion stays on screen before the page moves on. */
  const FINISHED_HOLD_MS = 6000;
  /** How long a copy button says it copied. */
  const COPIED_MS = 1600;
  /** A beat before the screen-reader region is filled, so a replaced sentence is read again. */
  const ANNOUNCE_DELAY_MS = 60;

  /** The phases of the choreography. */
  const PHASE = Object.freeze({ rest: "rest", meeting: "meeting", finishing: "finishing" });

  const CONNECTED_WORD_SEPARATOR = " ";

  /** What stands between two steps in the line that says them all. */
  const STEP_SEPARATOR = ", ";

  /** A step's number as its disc and the step line show it: "01" for the first. */
  const stepNumber = (index) => `0${index + 1}`;

  /*
   * Icons. These strings are constants of this file; nothing from the service or the page is ever
   * parsed as markup.
   */
  const ICON = Object.freeze({
    check: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>',
    drawnCheck: '<svg viewBox="0 0 24 24" aria-hidden="true"><path class="gl-draw" d="M5 12.5l4.5 4.5L19 7.5"/></svg>',
    spinner: '<svg viewBox="0 0 24 24" class="gl-spin" aria-hidden="true"><circle cx="12" cy="12" r="8" stroke-dasharray="26 50"/></svg>',
    attention: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 7v6M12 17h.01"/></svg>',
    minus: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 12h10"/></svg>',
    external: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" '
      + 'stroke-linejoin="round" aria-hidden="true"><path d="M14 5h5v5M19 5l-8 8M18 14v4a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h4"/></svg>',
    copy: '<svg class="gl-copy__idle" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" '
      + 'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="11" height="11" rx="2"/>'
      + '<path d="M5 15V6a2 2 0 0 1 2-2h9"/></svg>',
    copied: '<svg class="gl-copy__done" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" '
      + 'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>'
  });

  /**
   * Whether the window shows the setup view for a setup part.
   *
   * `skipped` is whether this window was sent away from setup, and `shown` whether it is on screen
   * now. Setup shows while there is something to do, unless this window skipped it. The finished
   * view shows only to a window that was showing setup when it finished; one that opens afterwards,
   * or that skipped, gets the workbench. A part the service leaves out (a watcher) shows nothing.
   */
  function visible(part, { skipped, shown }) {
    if (!part) return false;
    if (part.stage === STAGE.done) return shown;
    return WAITING.includes(part.stage) && !skipped;
  }

  /**
   * What changing from one view to the next plays, or null for a plain redraw.
   *
   * Completion wins: a view that finishes setup plays the completion alone, even in the update
   * that also connected a browser. The meeting plays when a browser connects after the browser step
   * has been on screen for MEET_AFTER_MS, so a window opened on a browser that was already
   * connected, or one that skipped through, never sees it.
   */
  function transition(previous, next, shownBrowserMs) {
    if (!previous || !next) return null;
    if (next.stage === STAGE.done && WAITING.includes(previous.stage)) return PLAY.finish;
    if (previous.stage === STAGE.browser && next.stage === STAGE.agent && shownBrowserMs >= MEET_AFTER_MS) {
      return PLAY.meet;
    }
    return null;
  }

  /** The headline as one sentence. */
  function headlineText(view) {
    const { lead, accent, trail } = view.headline;
    return `${lead}${accent}${trail}`;
  }

  /**
   * The row of the browser step that stands for the browser that connected, or null.
   *
   * The service names the connected browser in its connected line ("Chrome is connected."), and a
   * row is named in full ("Google Chrome"), so the row is the one whose last word starts the line.
   * Failing that, the row the person clicked is the one.
   */
  function connectedRow(previous, next, clicked) {
    const line = next.connected_line ?? "";
    const named = previous.rows.find((row) => line.startsWith(row.name.split(CONNECTED_WORD_SEPARATOR).at(-1)));
    return named ?? previous.rows.find((row) => clicked.has(row.id)) ?? null;
  }

  /**
   * Draw setup views into the markup of the window.
   *
   * `document` holds the markup; `words` the page's own three words (copy, copied, skip);
   * `schedule(work, ms)` and `cancel(handle)` run and stop timers; `now()` is the clock;
   * `hasFocus()` and `reducedMotion()` say whether the window has the person's attention and
   * whether motion is off. The callbacks hear what the person did: `onAction(action, row)` for a
   * click on a row's button, `onSkip()`, `copyText(text)` (a promise), `bringForward()` when the
   * meeting happens in a window the person is not looking at, `onFinished()` when the completion
   * has been on screen long enough, and `onFailure(what, error)`.
   */
  function create({
    document, words, schedule, cancel, now, hasFocus, reducedMotion,
    onAction, onSkip, copyText, bringForward, onFinished, onFailure
  }) {
    const el = (id) => document.getElementById(id);
    const rootNode = el("setup-root");
    const companion = el("setup-companion");
    const discs = [...el("setup-discs").querySelectorAll(".gl-stepper__disc")];
    const segments = [...el("setup-discs").querySelectorAll(".gl-stepper__seg")];
    const names = [...el("setup-steps").querySelectorAll("li")];
    const rowsList = el("setup-rows");
    const skip = el("setup-skip");

    /** The view on screen (the latest one, while the meeting plays), or null when closed. */
    let current = null;
    let phase = PHASE.rest;
    /** When the browser stage was first on screen, or null when it is not. */
    let browserSince = null;
    /** The signature of what each region last drew, so a region that did not change is left alone. */
    let drawn = new Map();
    /** The rows whose buttons were clicked and whose next change has not come: id to what to show. */
    const pending = new Map();
    let meetingTimer = null;
    let taglineTimer = null;
    let finishedTimer = null;
    let announceTimer = null;
    const copyTimers = new Set();

    const motion = () => !reducedMotion();

    function element(tag, className, text) {
      const node = document.createElement(tag);
      if (className) node.className = className;
      if (text !== undefined) node.textContent = text;
      return node;
    }

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

    /** Draw a region unless it is exactly what it last drew. */
    function paint(region, value, draw, force = false) {
      const signature = JSON.stringify(value);
      if (!force && drawn.get(region) === signature) return;
      draw(value);
      drawn.set(region, signature);
    }

    /** Play a class once: remove it, make the browser notice, add it. */
    function replay(node, className) {
      node.classList.remove(className);
      void node.offsetWidth;
      node.classList.add(className);
    }

    function announce(text) {
      cancel(announceTimer);
      el("setup-say").textContent = "";
      announceTimer = schedule(() => { el("setup-say").textContent = text; }, ANNOUNCE_DELAY_MS);
    }

    /* ---- the companion: the ghost's light, the three discs and their names ---- */

    function drawCompanion({ steps, light, popped }) {
      companion.setAttribute("data-light", light);
      steps.forEach((step, index) => {
        const disc = discs[index];
        disc.className = `gl-stepper__disc gl-stepper__disc--${index}`
          + (step.state === "todo" ? "" : ` is-${step.state}`)
          + (popped.includes(index) && motion() ? " is-pop" : "");
        if (step.state === "done") disc.replaceChildren(svg(ICON.check));
        else disc.textContent = stepNumber(index);
        const item = names[index];
        item.className = step.state === "todo" ? "" : `is-${step.state}`;
        if (step.state === "current") item.setAttribute("aria-current", "step");
        else item.removeAttribute("aria-current");
        item.querySelector(".gl-stepper__t").textContent = step.name;
        item.querySelector(".gl-sr").textContent = step.sr;
        item.querySelector(".gl-stepper__w").textContent = step.word;
      });
      const done = steps.map((step) => step.state === "done");
      segments.forEach((segment, index) => {
        segment.classList.toggle("is-lit", done[index] && done[index + 1]);
      });
      drawStepLine(steps);
    }

    /**
     * The steps in one line under the eyebrow, for the one-column layout that puts the companion
     * below the instruction: "01 This computer, 02 Your browser, 03 Your agent", the current one
     * marked. A screen reader hears the steps from the companion, so the line is hidden from it.
     */
    function drawStepLine(steps) {
      const line = el("setup-stepline");
      const parts = [];
      steps.forEach((step, index) => {
        if (index) parts.push(STEP_SEPARATOR);
        parts.push(element("span", step.state === "current" ? "is-current" : "", `${stepNumber(index)} ${step.name}`));
      });
      line.replaceChildren(...parts);
    }

    function companionOf(view, popped = []) {
      return { steps: view.steps, light: view.light, popped };
    }

    function celebrate() {
      if (!motion()) return;
      replay(companion, "is-celebrating");
    }

    /* ---- the right column ---- */

    function drawHead({ view, swap }) {
      el("setup-eyebrow").textContent = view.eyebrow;
      el("setup-h1-lead").textContent = view.headline.lead;
      el("setup-h1-accent").textContent = view.headline.accent;
      el("setup-h1-accent").hidden = !view.headline.accent;
      el("setup-h1-trail").textContent = view.headline.trail;
      el("setup-lead").textContent = view.lead ?? "";
      el("setup-lead").hidden = !view.lead;
      if (swap && motion()) replay(el("setup-main"), "gl-swap");
    }

    function selectText(node) {
      try {
        const range = document.createRange();
        range.selectNodeContents(node);
        const selection = globalThis.getSelection();
        selection.removeAllRanges();
        selection.addRange(range);
      } catch (error) {
        // Selecting the text is a courtesy to a person whose clipboard refused.
      }
    }

    /** A panel with a command or a prompt and its copy button. */
    function codePanel(panel) {
      const wrap = element("div", "gl-code");
      wrap.append(element("span", "gl-code__label", panel.label));
      const text = element("code", "gl-code__text");
      const body = element("span", "", panel.text);
      text.append(body);
      const button = element("button", "gl-copy");
      button.type = "button";
      button.setAttribute("aria-label", panel.copy_name);
      const label = element("span", "gl-copy__lbl", words.copy);
      button.append(svg(ICON.copy), svg(ICON.copied), label);
      let resetTimer = null;
      button.addEventListener("click", async () => {
        try {
          await copyText(panel.text);
        } catch (error) {
          selectText(body);
          onFailure("copying", error);
          return;
        }
        cancel(resetTimer);
        button.classList.add("is-copied");
        label.textContent = words.copied;
        announce(words.copied);
        resetTimer = schedule(() => {
          button.classList.remove("is-copied");
          label.textContent = words.copy;
        }, COPIED_MS);
        copyTimers.add(resetTimer);
      });
      wrap.append(text, button);
      return wrap;
    }

    function drawCode(container, panel) {
      container.replaceChildren(...(panel ? [codePanel(panel)] : []));
    }

    function liveIcon(done) {
      return icon(done ? ICON.check : ICON.spinner, "gl-live__icon");
    }

    function drawLive(live) {
      const node = el("setup-live");
      node.className = live?.done ? "gl-live is-ok" : "gl-live";
      node.replaceChildren(...(live?.text ? [liveIcon(live.done), element("span", "", live.text)] : []));
    }

    /** What the live line says: the pending words of a clicked row win over the view's own line. */
    function liveOf(view) {
      const waiting = [...pending.values()].filter((entry) => entry.live);
      if (waiting.length) return { text: waiting.at(-1).live, done: false };
      return view.live ?? null;
    }

    function rowIcon(row, flip) {
      if (row.tone === "done") return icon(flip && motion() ? ICON.drawnCheck : ICON.check, "gl-row__icon");
      if (row.tone === "working") return icon(ICON.spinner, "gl-row__icon");
      if (row.tone === "attention") return icon(ICON.attention, "gl-row__icon");
      if (row.tone === "off") return icon(ICON.minus, "gl-row__icon");
      return element("span", "gl-row__icon");
    }

    function rowNode(row, flip) {
      const item = element("li", `gl-row is-${row.tone}${flip && motion() ? " is-flip" : ""}`);
      item.setAttribute("data-row", row.id);
      const body = element("span", "gl-row__body");
      body.append(element("span", "gl-row__name", row.name), element("span", "gl-row__detail", row.detail));
      item.append(rowIcon(row, flip), body);
      if (row.action) {
        const action = element("span", "gl-row__action");
        const button = element("button", `gl-btn ${row.action.primary ? "gl-btn--primary" : "gl-btn--ghost"} gl-btn--sm`, row.action.label);
        button.type = "button";
        button.setAttribute("aria-label", row.action.name);
        button.setAttribute("data-row", row.id);
        button.setAttribute("data-kind", row.action.action.kind);
        if (row.action.external) button.append(svg(ICON.external));
        action.append(button);
        item.append(action);
      }
      return item;
    }

    function drawRows({ label, rows, flip }) {
      const block = el("setup-agents");
      block.hidden = rows.length === 0;
      el("setup-rows-label").textContent = label ?? "";
      el("setup-rows-label").hidden = !label;
      rowsList.replaceChildren(...rows.map((row) => rowNode(row, row.id === flip)));
    }

    /** The rows as shown: a clicked row says what is happening in place of its button. */
    function rowsOf(view) {
      return view.rows.map((row) => {
        const waiting = pending.get(row.id);
        return waiting ? { ...row, tone: "working", detail: waiting.detail, action: null } : row;
      });
    }

    function drawTagline(text, fresh) {
      const node = el("setup-tagline");
      node.textContent = text ?? "";
      node.hidden = !text;
      node.classList.toggle("is-new", Boolean(text) && fresh && motion());
    }

    /* ---- drawing a whole view ---- */

    /** Draw `view` at rest. `swap` plays the entrance of a replaced headline. */
    function render(view, { swap = false, popped = [], force = false } = {}) {
      paint("companion", companionOf(view, popped), drawCompanion, force);
      paint("head", { view: { eyebrow: view.eyebrow, headline: view.headline, lead: view.lead }, swap },
        ({ swap: playing }) => drawHead({ view, swap: playing }), force);
      paint("code", view.code, (panel) => drawCode(el("setup-code"), panel), force);
      paint("rows", { label: view.rows_label, rows: rowsOf(view), flip: null }, drawRows, force);
      paint("extra", view.extra_code, (panel) => drawCode(el("setup-extra-code"), panel), force);
      paint("live", liveOf(view), drawLive, force);
      paint("tagline", view.tagline ?? null, (text) => drawTagline(text, false), force);
      skip.hidden = !WAITING.includes(view.stage);
      if (swap && motion()) replay(el("setup-after"), "gl-swap");
    }

    /* ---- the choreography ---- */

    function stopTimers() {
      for (const timer of [meetingTimer, taglineTimer, finishedTimer, announceTimer, ...copyTimers]) cancel(timer);
      copyTimers.clear();
      meetingTimer = null;
      taglineTimer = null;
      finishedTimer = null;
      announceTimer = null;
    }

    /**
     * The browser connects while the browser step is on screen: its row turns done, the line says
     * so, disc 02 fills and the lantern flares. The agent step takes over after MEET_SWAP_MS.
     */
    function meet(previous, next) {
      phase = PHASE.meeting;
      const clicked = new Set(pending.keys());
      pending.clear();
      const row = connectedRow(previous, next, clicked);
      const rows = previous.rows.map((each) => (each === row
        ? { ...each, tone: "done", detail: next.steps[1].word, action: null }
        : { ...each, action: null }));
      paint("companion", companionOf(next, [1]), drawCompanion, true);
      celebrate();
      paint("rows", { label: previous.rows_label, rows, flip: row?.id ?? null }, drawRows, true);
      paint("live", next.connected_line ? { text: next.connected_line, done: true } : null, drawLive, true);
      skip.hidden = true;
      if (!hasFocus()) bringForward();
      meetingTimer = schedule(() => {
        meetingTimer = null;
        phase = PHASE.rest;
        render(current, { swap: true, force: true });
        announce(headlineText(current));
      }, MEET_SWAP_MS);
    }

    /**
     * Setup is complete while the window is open: disc 03 fills, the heading says so and the
     * tagline arrives. There is no second celebration. After FINISHED_HOLD_MS the page moves on.
     */
    function finish(next) {
      cancel(meetingTimer);
      meetingTimer = null;
      phase = PHASE.finishing;
      pending.clear();
      paint("companion", companionOf(next, [2]), drawCompanion, true);
      paint("head", { view: { eyebrow: next.eyebrow, headline: next.headline, lead: next.lead }, swap: true },
        ({ swap }) => drawHead({ view: next, swap }), true);
      paint("code", null, (panel) => drawCode(el("setup-code"), panel), true);
      paint("rows", { label: null, rows: [], flip: null }, drawRows, true);
      paint("extra", null, (panel) => drawCode(el("setup-extra-code"), panel), true);
      paint("live", null, drawLive, true);
      skip.hidden = true;
      if (motion()) {
        paint("tagline", next.tagline, (text) => drawTagline(text, true), true);
      } else {
        paint("tagline", null, (text) => drawTagline(text, false), true);
        taglineTimer = schedule(() => {
          taglineTimer = null;
          paint("tagline", next.tagline, (text) => drawTagline(text, false), true);
        }, TAGLINE_MS);
      }
      announce(headlineText(next));
      finishedTimer = schedule(() => {
        finishedTimer = null;
        onFinished();
      }, FINISHED_HOLD_MS);
    }

    /** Remember when the browser step came on screen, and forget it when it leaves. */
    function trackBrowser(view) {
      if (view.stage !== STAGE.browser) browserSince = null;
      else if (browserSince === null) browserSince = now();
    }

    /** Clear the pending words of every row the new view has changed. */
    function settle(view) {
      for (const [id, waiting] of [...pending]) {
        const row = view.rows.find((each) => each.id === id);
        if (!row || JSON.stringify(row) !== waiting.signature) pending.delete(id);
      }
    }

    /* ---- the person's clicks ---- */

    rowsList.addEventListener("click", (event) => {
      const button = event.target.closest("button[data-row]");
      if (!button || !current || phase !== PHASE.rest) return;
      const row = current.rows.find((each) => each.id === button.getAttribute("data-row"));
      if (!row?.action || pending.has(row.id)) return;
      const { pending: waiting } = row.action;
      if (waiting) {
        pending.set(row.id, { detail: waiting.detail, live: waiting.live, signature: JSON.stringify(row) });
        render(current);
      }
      onAction(row.action.action, row);
    });

    skip.addEventListener("click", () => onSkip());
    skip.textContent = words.skip;

    return {
      /** Show the setup view, at rest, for `view`. */
      open(view) {
        stopTimers();
        companion.classList.remove("is-celebrating");
        phase = PHASE.rest;
        pending.clear();
        drawn = new Map();
        current = view;
        browserSince = null;
        trackBrowser(view);
        render(view, { force: true });
        rootNode.hidden = false;
      },

      /**
       * Take the next setup part while the view is on screen: play the meeting or the completion
       * when the part calls for one, and otherwise redraw what changed.
       */
      update(next) {
        const previous = current;
        current = next;
        if (phase === PHASE.finishing) return;
        const shownBrowserMs = browserSince === null ? 0 : now() - browserSince;
        const play = phase === PHASE.rest ? transition(previous, next, shownBrowserMs) : null;
        trackBrowser(next);
        if (phase === PHASE.meeting) {
          if (next.stage === STAGE.done) finish(next);
          return;
        }
        settle(next);
        if (play === PLAY.finish) finish(next);
        else if (play === PLAY.meet) meet(previous, next);
        else render(next);
      },

      /** Take the setup view off the screen and stop everything it had scheduled. */
      close() {
        stopTimers();
        phase = PHASE.rest;
        pending.clear();
        current = null;
        browserSince = null;
        drawn = new Map();
        rootNode.hidden = true;
      },

      /** Put a row back as it was, after a click that did not do what it asked. */
      clearPending(id) {
        if (!pending.delete(id) || !current) return;
        render(current);
      },

      /** Whether the view is on screen. */
      isOpen: () => current !== null,

      /** The view last handed to open or update, or null. */
      view: () => current
    };
  }

  return Object.freeze({
    STAGE, WAITING, PLAY, ACTION, PHASE, MEET_AFTER_MS, MEET_SWAP_MS, TAGLINE_MS, FINISHED_HOLD_MS,
    COPIED_MS, ANNOUNCE_DELAY_MS, visible, transition, headlineText, connectedRow, create
  });
});
