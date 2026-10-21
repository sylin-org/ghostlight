// Ghostlight -- the toolbar popup. It places the words GhostlightPopupView chose, follows the
// connection the worker reports, and sends the person's choice back as a request. It holds no
// policy and no words of its own.
(function ghostlightPopup() {
  "use strict";

  const view = globalThis.GhostlightPopupView;
  const { PAGE: MESSAGE, COMMAND } = globalThis.GhostlightMessages;
  const SETUP_PAGE = "setup.html";
  const POLL_MS = 1500;

  // These strings are constants in this file; nothing from the page or the worker is ever parsed
  // as markup.
  const ICON = Object.freeze({
    pause: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" '
      + 'stroke-linejoin="round" aria-hidden="true"><path d="M9 6v12M15 6v12"/></svg>',
    play: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" '
      + 'stroke-linejoin="round" aria-hidden="true"><path d="M8 5.5v13l10-6.5z"/></svg>'
  });
  const ICON_FOR_INTENT = Object.freeze({
    [view.INTENT.HOLD]: ICON.pause,
    [view.INTENT.RESUME]: ICON.play
  });

  const pop = document.getElementById("pop");
  const eyebrow = document.getElementById("eyebrow");
  const title = document.getElementById("title");
  const line = document.getElementById("line");
  const line2 = document.getElementById("line2");
  const note = document.getElementById("note");
  const actions = document.getElementById("actions");
  const problem = document.getElementById("problem");
  const settings = document.getElementById("settings");
  const show = document.getElementById("show");

  // What the connection is when the worker does not answer: the same words as an unreachable link.
  const UNREACHABLE = Object.freeze({
    connected: false, compatible: true, control_state: "active", attached_tabs: 0, recording_tabs: 0, activity: []
  });

  let platform = null;
  let shortcut = null;
  let signature = null;
  // The window link the drawn view offers, or null.
  let showAction = null;

  async function request(message) {
    const response = await chrome.runtime.sendMessage(message);
    if (!response?.ok) throw new Error(response?.error || "Ghostlight did not respond.");
    return response.value;
  }

  function element(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  }

  function icon(markup) {
    const holder = document.createElement("template");
    holder.innerHTML = markup;
    return holder.content.firstElementChild;
  }

  function showProblem(message) {
    problem.textContent = message;
    problem.hidden = message === "";
  }

  function open(target) {
    if (target === view.OPEN.SETUP) {
      chrome.tabs.create({ url: chrome.runtime.getURL(SETUP_PAGE) }).catch(() => {});
    } else if (target === view.OPEN.OPTIONS) {
      chrome.runtime.openOptionsPage();
    }
  }

  // A request that changes the browser's state. The view is redrawn from the answer either way, so
  // a pressed button is never left disabled.
  async function send(message) {
    try {
      await request(message);
      showProblem("");
    } catch (error) {
      showProblem(String(error?.message ?? error));
    }
    await refresh(true);
  }

  function pressFor(action) {
    if (action.open) return () => open(action.open);
    if (action.release) return () => send({ kind: MESSAGE.RELEASE_DEBUGGER_SESSIONS });
    return () => send({ kind: MESSAGE.RUNTIME_CONTROL, intent: action.intent });
  }

  function actionButton(action, className) {
    const button = element("button", className);
    button.type = "button";
    // The intent names the request the button sends, so a test finds the button by what it does.
    if (action.intent) button.dataset.intent = action.intent;
    if (action.key) {
      const markup = ICON_FOR_INTENT[action.intent];
      if (markup) button.append(icon(markup));
      button.append(element("span", "gl-pop__pause-label", action.label));
      // The cap is a visual hint; the button names the shortcut for assistive technology.
      const cap = element("span", "gl-kbd", action.key);
      cap.setAttribute("aria-hidden", "true");
      button.append(cap);
      button.setAttribute("aria-keyshortcuts", action.key);
      button.classList.add("gl-pop__pause");
    } else {
      button.textContent = action.label;
    }
    const press = pressFor(action);
    button.addEventListener("click", () => {
      // A page that opens closes the popup; a request disables its button until the answer redraws.
      if (!action.open) button.disabled = true;
      press();
    });
    return button;
  }

  function renderActions(model) {
    actions.replaceChildren();
    if (model.primary) actions.append(actionButton(model.primary, "gl-btn gl-btn--primary"));
    if (model.secondary) actions.append(actionButton(model.secondary, "gl-btn gl-btn--ghost"));
    if (model.quiet) actions.append(actionButton(model.quiet, "gl-pop__quiet"));
    if (model.release) actions.append(actionButton(model.release, "gl-pop__quiet"));
  }

  // The view is drawn only when it changes, so the poll never drops the person's keyboard focus or
  // a button they are pressing.
  function render(snapshot, force) {
    const model = view.popupView(snapshot, { shortcut, platform });
    const next = JSON.stringify(model);
    if (!force && next === signature) return;
    signature = next;
    pop.dataset.light = model.light;
    eyebrow.textContent = model.eyebrow;
    title.textContent = model.title;
    line.textContent = model.line;
    line2.textContent = model.line2 ?? "";
    line2.hidden = model.line2 === null;
    note.textContent = model.note ?? "";
    note.hidden = model.note === null;
    settings.textContent = model.settings.label;
    showAction = model.show;
    show.textContent = showAction?.label ?? "";
    show.hidden = showAction === null;
    renderActions(model);
  }

  async function refresh(force = false) {
    try {
      render(await request({ kind: MESSAGE.UI_SNAPSHOT }), force);
    } catch {
      render(UNREACHABLE, force);
    }
  }

  async function start() {
    const [info, commands] = await Promise.all([
      chrome.runtime.getPlatformInfo().catch(() => null),
      chrome.commands.getAll().catch(() => [])
    ]);
    platform = info?.os ?? null;
    shortcut = commands.find((command) => command.name === COMMAND.TOGGLE_HOLD)?.shortcut || null;
    settings.addEventListener("click", (event) => {
      event.preventDefault();
      open(view.OPEN.OPTIONS);
    });
    // The window link sends the message the drawn view names.
    show.addEventListener("click", () => {
      if (showAction) send({ kind: showAction.kind });
    });
    chrome.runtime.onMessage.addListener((message) => { if (message?.kind === MESSAGE.UI_STATE_CHANGED) refresh(); });
    setInterval(refresh, POLL_MS);
    refresh();
  }

  start();
})();
