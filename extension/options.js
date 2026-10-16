(function ghostlightOptions() {
  "use strict";

  const view = globalThis.GhostlightPopupView;
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
  let platform = null;
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
      const report = await request({ kind: "connection_diagnostics" });
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

  async function request(message) {
    const response = await chrome.runtime.sendMessage(message);
    if (!response?.ok) throw new Error(response?.error || "Ghostlight did not respond.");
    return response.value;
  }

  function syncCaptionsAvailability() {
    captions.disabled = !effects.checked;
  }

  function renderLink(snapshot) {
    const connection = view.connection(snapshot, { platform });
    const connected = connection.title === view.WORDS.connected.title;
    linkPill.className = connected ? "gl-pill gl-pill--ok" : "gl-pill gl-pill--warn";
    linkText.textContent = connection.title;
    linkSub.textContent = connection.line;
    setupRoute.hidden = !connection.needsSetup;
    setupRoute.textContent = connection.setupLabel ?? "";
  }

  async function load() {
    if (platform === null) platform = (await chrome.runtime.getPlatformInfo().catch(() => null))?.os ?? null;
    const [preferences, snapshot] = await Promise.all([
      request({ kind: "get_preferences" }),
      request({ kind: "ui_snapshot" })
    ]);
    effects.checked = preferences.effects;
    captions.checked = preferences.captions;
    diagnostics.checked = preferences.diagnostics;
    preserveTabs.checked = preferences.preserveTabs;
    syncCaptionsAvailability();
    renderLink(snapshot);
  }

  async function save() {
    status.textContent = "Saving";
    try {
      await request({
        kind: "set_preferences",
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

  effects.addEventListener("change", () => { syncCaptionsAvailability(); save(); });
  captions.addEventListener("change", save);
  diagnostics.addEventListener("change", save);
  preserveTabs.addEventListener("change", save);
  chrome.runtime.onMessage.addListener((message) => { if (message?.kind === "ui_state_changed") load(); });
  setInterval(() => load().catch(() => {}), 1500);
  load().catch((error) => { status.textContent = String(error?.message ?? error); });
})();
