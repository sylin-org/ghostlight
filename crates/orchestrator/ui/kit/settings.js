// Ghostlight web apps -- the Settings page.
//
// Every Ghostlight screen has a Settings tab, and this draws it: a rail of sections, each showing
// its current state, and the selected section beside it. It draws the service's controls part and
// nothing else: layout only. Every state word, label and sentence about the computer comes from
// the part, and the few labels that belong to the page (the rail's names, the personal settings,
// the verb on each button, the question Quit asks) are handed in as `words`. An act the part says
// this screen may not do is drawn as the state it would have changed, with one Continue line
// saying where the change is made. A screen that shows nothing says why in each section.
//
// The page's markup holds the rail and the sections, so every element a guard reaches for exists
// there; this fills them in. Which section shows is chrome and belongs to the view, which also
// keeps its address. This has no transport of its own: pressing a button calls a function the
// page handed it. The personal settings of This screen are kept in the browser, never sent, and
// the page draws correctly without them.
(function installGhostlightSettings(root, factory) {
  const api = factory();
  root.GhostlightSettings = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightSettingsApi() {
  "use strict";

  /** Where this browser keeps the personal settings of this screen. */
  const STORAGE_KEY = "ghostlight.screen";

  /** The personal settings: the values each may take, the first being the default, and the body class each value wears. */
  const PERSONAL = Object.freeze({
    density: Object.freeze({ values: Object.freeze(["comfortable", "compact"]), classes: Object.freeze({ compact: "density-compact" }) }),
    text: Object.freeze({ values: Object.freeze(["standard", "large"]), classes: Object.freeze({ large: "text-large" }) }),
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

  /** The page that holds Settings. A press outside it is not this module's. */
  const SETTINGS_VIEW_ID = "view-settings";

  /** The sections that show the service's state, which a screen with nothing to show closes. */
  const SERVICE_SECTIONS = Object.freeze(["sharing", "browser-work", "notifications", "diagnostics"]);

  /** The browser's own settings This screen names, by the key of each in the part's `browser`. */
  const BROWSER_SETTINGS = Object.freeze(["effects", "captions"]);

  /** The data attributes the buttons are found by. */
  const VIEWING_TOGGLE = "toggle";
  const VIEWING_COPY = "copy";
  const DIAGNOSTICS_TOGGLE = "toggle";
  const DIAGNOSTICS_REVEAL = "reveal";
  const QUIT_ASK = "ask";
  const QUIT_YES = "yes";
  const QUIT_NO = "no";

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
    quit = async () => {},
    mayQuit = () => true,
    copyText,
    onFailure = () => {}
  } = {}) {
    let current = null;
    let wired = false;
    let asking = false;
    const personal = Object.fromEntries(Object.keys(PERSONAL).map((name) => [name, fallbackOf(name)]));

    /** The element with this id. A missing one is a fault in the page's markup, said out loud. */
    function node(id) {
      const found = document.getElementById(id);
      if (!found) throw new Error(`Settings has no #${id}`);
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
        // A browser that keeps nothing still draws the page; the defaults stand.
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
      drawRailState("screen", words.screenState?.(personal.density, personal.text, personal.motion) ?? "");
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

    /** Write the state word of one rail entry, and the same text as its tooltip for when it is cut short. */
    function drawRailState(section, text) {
      const entry = node(`rail-${section}-state`);
      entry.textContent = text ?? "";
      entry.title = text ?? "";
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
     * The browser's own settings, on This screen: what each is and how it starts, and where it is
     * set. They are lines, never controls, because the browser keeps them.
     */
    function drawBrowser(browser) {
      for (const key of BROWSER_SETTINGS) {
        say(`controls-${key}-label`, browser?.[key]?.label);
        say(`controls-${key}-word`, browser?.[key]?.word);
        say(`controls-${key}-where`, browser?.set_in);
        show(`controls-${key}`, Boolean(browser?.[key]));
      }
    }

    /**
     * One Continue line for each section: the reason the first act this screen may not do in it
     * gives. Only a screen that is not the window has it. In the window, an act the service
     * withholds (the watch view is not served) is a state the window already says, and continuing
     * in the window would send it nowhere.
     */
    function drawContinue(part) {
      const sections = {
        sharing: part.viewing,
        "browser-work": part.runtime,
        notifications: part.notifications,
        diagnostics: part.diagnostics
      };
      const control = part.you.context === CONTEXT_CONTROL;
      for (const [section, group] of Object.entries(sections)) {
        const withheld = control ? undefined : Object.values(group.acts).find((act) => act && act.available === false);
        say(`settings-${section}-why`, withheld?.why_not);
        show(`settings-${section}-continue`, Boolean(withheld));
      }
    }

    /** The Quit row: in the window alone, a button that asks first, inline, and then a question with two answers. */
    function drawQuit() {
      const offered = Boolean(current) && current.you.context === CONTEXT_CONTROL && mayQuit();
      if (!offered) asking = false;
      show("settings-quit", offered);
      show("settings-quit-ask", !asking);
      show("settings-quit-question", asking);
      show("settings-quit-yes", asking);
      show("settings-quit-no", asking);
    }

    /** Draw the controls part: every section, the rail's state words and the Quit row. */
    function render(part) {
      if (!part) return;
      current = part;
      for (const section of SERVICE_SECTIONS) {
        show(`settings-${section}-rows`, true);
        show(`settings-${section}-closed`, false);
      }
      say("settings-sharing-who", part.you.sentence);
      drawRuntime(part.runtime);
      drawViewing(part.viewing);
      drawDiagnostics(part.diagnostics);
      drawNotifications(part.notifications);
      drawBrowser(part.browser);
      drawContinue(part);
      drawQuit();
      drawRailState("sharing", part.viewing.word);
      drawRailState("browser-work", part.runtime.word);
      drawRailState("notifications", part.notifications.word);
      drawRailState("diagnostics", part.diagnostics.word);
    }

    /**
     * Draw a screen with nothing to show: This screen still works, and every other section says,
     * in one line, why it shows nothing.
     */
    function closed(sentence) {
      current = null;
      drawBrowser(null);
      for (const section of SERVICE_SECTIONS) {
        show(`settings-${section}-rows`, false);
        show(`settings-${section}-continue`, false);
        say(`settings-${section}-closed`, sentence);
        show(`settings-${section}-closed`, true);
        drawRailState(section, "");
      }
      drawQuit();
    }

    /* --------------------------------- presses ------------------------------ */

    /** Run what a button asks for, keep the button from being pressed twice, and report a failure. */
    function press(button, work, failed) {
      if (button.disabled) return;
      button.disabled = true;
      let outcome;
      try {
        outcome = work();
      } catch (error) {
        button.disabled = false;
        failed?.();
        onFailure("using Settings", error);
        return;
      }
      Promise.resolve(outcome)
        .catch((error) => {
          failed?.();
          onFailure("using Settings", error);
        })
        .finally(() => {
          button.disabled = false;
        });
    }

    /** Ask before quitting, answer the question, or quit. */
    function onQuit(button) {
      const step = button.dataset.quit;
      if (step === QUIT_ASK) {
        asking = true;
        drawQuit();
        node("settings-quit-no").focus?.();
      } else if (step === QUIT_NO) {
        asking = false;
        drawQuit();
        node("settings-quit-ask").focus?.();
      } else if (step === QUIT_YES) {
        press(button, () => quit(), () => {
          asking = false;
          drawQuit();
        });
      }
    }

    function onClick(event) {
      const target = event.target;
      if (!target.closest(`#${SETTINGS_VIEW_ID}`)) return;
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
      const carryOn = target.closest("[data-settings-continue]");
      if (carryOn) {
        const section = carryOn.closest(".settings-section")?.dataset.section;
        return press(carryOn, () => continueInWindow(words.addressOf("settings", section)));
      }
      const leaving = target.closest("[data-quit]");
      if (leaving) onQuit(leaving);
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
    }

    return Object.freeze({
      wire, render, closed,
      /** The personal settings as they stand. */
      personal: () => ({ ...personal })
    });
  }

  return Object.freeze({ create, STORAGE_KEY, PERSONAL, SERVICE_SECTIONS });
});
