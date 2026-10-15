// Ghostlight web apps -- the shared Controls sheet.
//
// Every Ghostlight screen carries the same Controls button on its band and opens the same sheet.
// This draws the service's controls part and nothing else: layout only. Every state word, label
// and sentence about the computer comes from the part, and the few labels that belong to the page
// (its headings, its personal settings, the verb on each button) are handed in as `words`. An act
// the part says this screen may not do is drawn as the state it would have changed, with one
// Continue line saying where the change is made.
//
// The sheet's markup lives in the page, so every element a guard reaches for exists there. This
// fills it in. It has no transport of its own: pressing a button calls a function the page handed
// it. The personal settings on the first group are kept in the browser, never sent, and the sheet
// draws correctly without them.
(function installGhostlightControls(root, factory) {
  const api = factory();
  root.GhostlightControls = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightControlsApi() {
  "use strict";

  /** Where this browser keeps the personal settings of this screen. */
  const STORAGE_KEY = "ghostlight.screen";

  /** The personal settings: the values each may take, the first being the default, and the body class each value wears. */
  const PERSONAL = Object.freeze({
    density: Object.freeze({ values: Object.freeze(["comfortable", "compact"]), classes: Object.freeze({ compact: "density-compact" }) }),
    motion: Object.freeze({ values: Object.freeze(["system", "reduced"]), classes: Object.freeze({ reduced: "motion-reduced" }) })
  });

  /** The runtime acts, by the id of the button that does each. The act key is also the intent it asks for. */
  const RUNTIME_BUTTONS = Object.freeze({
    hold: "controls-hold",
    resume: "controls-resume",
    start_session: "controls-start",
    end_session: "controls-end"
  });

  /** The runtime acts that make sense in each runtime state. */
  const RUNTIME_OFFERS = Object.freeze({
    active: Object.freeze(["hold", "end_session"]),
    held: Object.freeze(["resume", "end_session"]),
    ended: Object.freeze(["start_session"])
  });

  /** The token the service gives the Ghostlight window's own context. */
  const CONTEXT_CONTROL = "control";

  /** The data attributes the sheet's buttons are found by. */
  const VIEWING_TOGGLE = "toggle";
  const VIEWING_COPY = "copy";
  const DIAGNOSTICS_TOGGLE = "toggle";
  const DIAGNOSTICS_REVEAL = "reveal";

  /** The default of a personal setting: its first value. */
  const fallbackOf = (name) => PERSONAL[name].values[0];

  function create({
    document,
    words = {},
    storage = null,
    applyIntent = async () => {},
    setWatching = async () => {},
    setDiagnostics = async () => {},
    revealDiagnostics = async () => {},
    testNotification = async () => {},
    continueInWindow = async () => {},
    copyText,
    onFailure = () => {}
  } = {}) {
    let current = null;
    let wired = false;
    const personal = Object.fromEntries(Object.keys(PERSONAL).map((name) => [name, fallbackOf(name)]));

    /** The element with this id. A missing one is a fault in the page's markup, said out loud. */
    function node(id) {
      const found = document.getElementById(id);
      if (!found) throw new Error(`the Controls sheet has no #${id}`);
      return found;
    }

    /* --------------------------- personal settings -------------------------- */

    /** Read what this browser kept. Anything unreadable, or not a value the setting takes, is ignored. */
    function remembered() {
      try {
        const kept = JSON.parse(storage?.getItem(STORAGE_KEY) ?? "null");
        for (const [name, setting] of Object.entries(PERSONAL)) {
          if (setting.values.includes(kept?.[name])) personal[name] = kept[name];
        }
      } catch (error) {
        // A browser that keeps nothing still draws the sheet; the defaults stand.
      }
    }

    /** Keep the personal settings in this browser. A browser that refuses is left alone. */
    function remember() {
      try {
        storage?.setItem(STORAGE_KEY, JSON.stringify(personal));
      } catch (error) {
        // The choice still applies to this screen until it is closed.
      }
    }

    /** Dress the page for the personal settings, and mark the chosen button of each. */
    function dress() {
      const classes = document.body?.classList;
      for (const [name, setting] of Object.entries(PERSONAL)) {
        for (const [value, className] of Object.entries(setting.classes)) {
          classes?.toggle(className, personal[name] === value);
        }
      }
      for (const button of document.querySelectorAll("[data-personal]")) {
        const chosen = personal[button.dataset.personal] === button.dataset.value;
        button.setAttribute("aria-pressed", String(chosen));
        button.classList?.toggle("chosen", chosen);
      }
    }

    function choose(name, value) {
      if (!PERSONAL[name]?.values.includes(value) || personal[name] === value) return;
      personal[name] = value;
      remember();
      dress();
    }

    /* -------------------------------- drawing ------------------------------- */

    /** Say `text` in the element `id`. */
    function say(id, text) {
      node(id).textContent = text ?? "";
    }

    /** Show or hide the element `id`. */
    function show(id, shown) {
      node(id).hidden = !shown;
    }

    function drawRuntime(runtime) {
      say("controls-runtime-label", runtime.label);
      say("controls-runtime-word", runtime.word);
      say("controls-runtime-note", runtime.note);
      const offered = RUNTIME_OFFERS[runtime.state] ?? [];
      for (const [act, id] of Object.entries(RUNTIME_BUTTONS)) {
        show(id, offered.includes(act) && Boolean(runtime.acts[act]?.available));
      }
    }

    function drawViewing(viewing) {
      say("controls-viewing-label", viewing.label);
      say("controls-viewing-word", viewing.word);
      say("controls-viewing-url", viewing.url);
      show("controls-viewing-url", Boolean(viewing.url));
      // Copying the address is for the screen that changes who can watch; a watcher is already there.
      show(
        "controls-viewing-copy",
        Boolean(viewing.url) && typeof copyText === "function" && Boolean(viewing.acts.set_watching?.available)
      );
      say("controls-viewing-toggle", viewing.watch ? words.turnOff : words.turnOn);
      show("controls-viewing-toggle", Boolean(viewing.acts.set_watching?.available));
    }

    function drawDiagnostics(diagnostics) {
      say("controls-diagnostics-label", diagnostics.label);
      say("controls-diagnostics-word", diagnostics.word);
      const place = diagnostics.folder
        ? `${diagnostics.folder} (${words.byteSize ? words.byteSize(diagnostics.used_bytes) : diagnostics.used_bytes})`
        : "";
      say("controls-diagnostics-place", place);
      show("controls-diagnostics-place", Boolean(place));
      say("controls-diagnostics-toggle", diagnostics.on ? words.turnOff : words.turnOn);
      show("controls-diagnostics-toggle", Boolean(diagnostics.acts.set_diagnostics?.available));
      // The folder is named and openable even while diagnostics are off: retained logs live there.
      show("controls-diagnostics-reveal", Boolean(diagnostics.acts.reveal_diagnostics?.available));
    }

    function drawNotifications(notifications) {
      say("controls-notifications-label", notifications.label);
      say("controls-notifications-word", notifications.word);
      show("test-notification", Boolean(notifications.acts.test_notification?.available));
    }

    /**
     * One Continue line for the group: the reason the first act this screen may not do gives.
     * Only a screen that is not the window has it. In the window, an act the service withholds
     * (the watch view is not served) is a state the window already says, and continuing in the
     * window would send it nowhere.
     */
    function drawContinue(part) {
      if (part.you.context === CONTEXT_CONTROL) {
        show("controls-continue", false);
        return;
      }
      const acts = [
        ...Object.values(part.runtime.acts),
        ...Object.values(part.viewing.acts),
        ...Object.values(part.diagnostics.acts),
        ...Object.values(part.notifications.acts)
      ];
      const withheld = acts.find((act) => act && act.available === false);
      say("controls-continue-why", withheld?.why_not);
      show("controls-continue", Boolean(withheld));
    }

    /** Draw the controls part. `null` is a screen with no controls to show: nothing is drawn. */
    function render(part) {
      if (!part) {
        hide();
        return;
      }
      current = part;
      say("controls-service", part.service.word);
      drawRuntime(part.runtime);
      drawViewing(part.viewing);
      drawDiagnostics(part.diagnostics);
      drawNotifications(part.notifications);
      drawContinue(part);
      show("controls-open", true);
    }

    /** Take the button and the sheet away, for a screen that shows nothing. */
    function hide() {
      current = null;
      close();
      node("controls-open").hidden = true;
    }

    /* ----------------------------- opening, closing ------------------------- */

    const isOpen = () => !node("controls-sheet").hidden;

    function open() {
      if (!current) return;
      node("controls-sheet").hidden = false;
      node("controls-open").setAttribute("aria-expanded", "true");
      node("controls-sheet").focus?.();
    }

    function close() {
      node("controls-sheet").hidden = true;
      node("controls-open").setAttribute("aria-expanded", "false");
    }

    function toggle() {
      if (isOpen()) close();
      else open();
    }

    /** Run what a button asks for, keep the button from being pressed twice, and report a failure. */
    function press(button, work) {
      if (button.disabled) return;
      button.disabled = true;
      let outcome;
      try {
        outcome = work();
      } catch (error) {
        button.disabled = false;
        onFailure("using the controls", error);
        return;
      }
      Promise.resolve(outcome)
        .catch((error) => onFailure("using the controls", error))
        .finally(() => {
          button.disabled = false;
        });
    }

    function onClick(event) {
      const target = event.target;
      if (target.closest("#controls-open")) {
        toggle();
        return;
      }
      if (!target.closest("#controls-sheet")) {
        if (isOpen()) close();
        return;
      }
      const choice = target.closest("[data-personal]");
      if (choice) {
        choose(choice.dataset.personal, choice.dataset.value);
        return;
      }
      if (!current) return;
      const intent = target.closest("[data-intent]");
      if (intent) return press(intent, () => applyIntent(intent.dataset.intent));
      const viewing = target.closest("[data-viewing]");
      if (viewing?.dataset.viewing === VIEWING_TOGGLE) {
        return press(viewing, () => setWatching(!current.viewing.watch));
      }
      if (viewing?.dataset.viewing === VIEWING_COPY && typeof copyText === "function") {
        return press(viewing, () => copyText(current.viewing.url));
      }
      const diagnostics = target.closest("[data-diagnostics-operation]");
      if (diagnostics?.dataset.diagnosticsOperation === DIAGNOSTICS_TOGGLE) {
        return press(diagnostics, () => setDiagnostics(!current.diagnostics.on));
      }
      if (diagnostics?.dataset.diagnosticsOperation === DIAGNOSTICS_REVEAL) {
        return press(diagnostics, () => revealDiagnostics());
      }
      const notify = target.closest("#test-notification");
      if (notify) return press(notify, () => testNotification());
      const carryOn = target.closest("[data-controls-continue]");
      if (carryOn) return press(carryOn, () => continueInWindow());
    }

    function onKey(event) {
      if (event.key !== "Escape" || !isOpen()) return;
      close();
      node("controls-open").focus?.();
    }

    /** Fill in the page's own labels, dress the page for the personal settings and listen. */
    function wire() {
      if (wired) return;
      wired = true;
      for (const label of document.querySelectorAll("[data-word]")) {
        label.textContent = words[label.dataset.word] ?? "";
      }
      remembered();
      dress();
      document.addEventListener("click", onClick);
      document.addEventListener("keydown", onKey);
    }

    return Object.freeze({
      wire, render, hide, open, close, toggle, isOpen,
      /** The personal settings as they stand. */
      personal: () => ({ ...personal })
    });
  }

  return Object.freeze({ create, STORAGE_KEY, PERSONAL });
});
