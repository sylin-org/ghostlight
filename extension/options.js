(function ghostlightOptions() {
  "use strict";

  const effects = document.getElementById("effects");
  const captions = document.getElementById("captions");
  const diagnostics = document.getElementById("debug");
  const preserveTabs = document.getElementById("preserve-tabs");
  const status = document.getElementById("save-status");
  const linkPill = document.getElementById("link-pill");
  const linkText = document.getElementById("link-text");
  const linkSub = document.getElementById("link-sub");
  const setupRoute = document.getElementById("setup-route");

  setupRoute.addEventListener("click", () => {
    const destination = navigator.onLine
      ? "https://sylin.org/ghostlight/chromium-extension/post-install/"
      : chrome.runtime.getURL("setup.html");
    chrome.tabs.create({ url: destination }).catch(() => {});
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
    if (snapshot.control_state === "ended") {
      linkPill.className = "pill";
      linkText.textContent = "Session ended";
      linkSub.textContent = "Browser access is severed. Start a new session from the toolbar popup to reconnect.";
    } else if (snapshot.connected && snapshot.compatible) {
      linkPill.className = "pill on";
      linkText.textContent = "Connected";
      linkSub.textContent = "The agent can reach this browser.";
    } else if (snapshot.link_state === "host_absent") {
      linkPill.className = "pill wait";
      linkText.textContent = "Not installed here";
      linkSub.textContent = "Ghostlight is not installed on this computer yet. The extension came with your Chrome profile. Install Ghostlight here to connect it.";
    } else {
      linkPill.className = "pill wait";
      linkText.textContent = snapshot.compatible ? "Waiting" : "Version mismatch";
      linkSub.textContent = snapshot.compatible
        ? "Waiting for the Ghostlight service. Start it, and this turns green on its own."
        : "The extension and Ghostlight service cannot communicate until their adapter contracts match.";
    }
    setupRoute.hidden = snapshot.link_state !== "host_absent";
  }

  async function load() {
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
