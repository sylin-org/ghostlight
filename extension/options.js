// Ghostlight -- the options page. It places the connection sentence GhostlightPopupView chose,
// saves the person's browser preferences through the worker, and offers the same quiet link to the
// Ghostlight window as the popup. It holds no policy.
(function ghostlightOptions() {
  "use strict";

  const view = globalThis.GhostlightPopupView;
  const MESSAGE = globalThis.GhostlightMessages.PAGE;
  const SETUP_PAGE = "setup.html";
  const effects = document.getElementById("effects");
  const captions = document.getElementById("captions");
  const diagnostics = document.getElementById("debug");
  const preserveTabs = document.getElementById("preserve-tabs");
  const status = document.getElementById("save-status");
  const linkPill = document.getElementById("link-pill");
  const linkText = document.getElementById("link-text");
  const linkSub = document.getElementById("link-sub");
  const setupRoute = document.getElementById("setup-route");
  const showWindow = document.getElementById("show-window");
  const showStatus = document.getElementById("show-status");
  let platform = null;
  // The window link the connection offers, or null.
  let showAction = null;
  if (!globalThis.chrome?.runtime?.id) {
    linkPill.className = "gl-pill gl-pill--warn";
    linkText.textContent = "Open extension options";
    linkSub.textContent = "This is a local file, not the installed extension. Open Ghostlight's options from Chrome's Extensions menu to see its connection and settings.";
    document.querySelectorAll("input, button").forEach(control => { control.disabled = true; });
    return;
  }
  document.getElementById("export-connection-log").addEventListener("click", async () => {
    const reportStatus = document.getElementById("connection-log-status");
    try {
      const report = await request({ kind: MESSAGE.CONNECTION_DIAGNOSTICS });
      await chrome.downloads.download({
        url: `data:application/json;charset=utf-8,${encodeURIComponent(JSON.stringify(report, null, 2))}`,
        filename: `ghostlight-diagnostics-${Date.now()}.json`, saveAs: false
      });
      reportStatus.textContent = "Developer diagnostics saved to Downloads.";
    } catch (error) { reportStatus.textContent = String(error?.message ?? error); }
  });

  setupRoute.addEventListener("click", () => {
    chrome.tabs.create({ url: chrome.runtime.getURL(SETUP_PAGE) }).catch(() => {});
  });

  showWindow.addEventListener("click", async () => {
    if (!showAction) return;
    try {
      await request({ kind: showAction.kind });
      showStatus.textContent = "";
    } catch (error) { showStatus.textContent = String(error?.message ?? error); }
  });

  async function request(message) {
    const response = await chrome.runtime.sendMessage(message);
    if (!response?.ok) throw new Error(response?.error || "Ghostlight did not respond.");
    return response.value;
  }

  function renderLink(snapshot) {
    const connection = view.connection(snapshot, { platform });
    const connected = connection.title === view.WORDS.connected.title;
    linkPill.className = connected ? "gl-pill gl-pill--ok" : "gl-pill gl-pill--warn";
    linkText.textContent = connection.title;
    linkSub.textContent = connection.line;
    setupRoute.hidden = !connection.needsSetup;
    setupRoute.textContent = connection.setupLabel ?? "";
    showAction = connection.show;
    showWindow.hidden = showAction === null;
    showWindow.textContent = showAction?.label ?? "";
  }

  async function load() {
    if (platform === null) platform = (await chrome.runtime.getPlatformInfo().catch(() => null))?.os ?? null;
    const [preferences, snapshot] = await Promise.all([
      request({ kind: MESSAGE.GET_PREFERENCES }),
      request({ kind: MESSAGE.UI_SNAPSHOT })
    ]);
    effects.checked = preferences.effects;
    captions.checked = preferences.captions;
    diagnostics.checked = preferences.diagnostics;
    preserveTabs.checked = preferences.preserveTabs;
    renderLink(snapshot);
  }

  async function save() {
    status.textContent = "Saving";
    try {
      await request({
        kind: MESSAGE.SET_PREFERENCES,
        preferences: {
          effects: effects.checked,
          captions: captions.checked,
          diagnostics: diagnostics.checked,
          preserveTabs: preserveTabs.checked
        }
      });
      status.textContent = "Saved locally";
    } catch (error) {
      status.textContent = String(error?.message ?? error);
    }
  }

  effects.addEventListener("change", save);
  captions.addEventListener("change", save);
  diagnostics.addEventListener("change", save);
  preserveTabs.addEventListener("change", save);
  chrome.runtime.onMessage.addListener((message) => { if (message?.kind === MESSAGE.UI_STATE_CHANGED) load(); });
  setInterval(() => load().catch(() => {}), 1500);
  load().catch((error) => { status.textContent = String(error?.message ?? error); });
})();
