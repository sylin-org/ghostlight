// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- options page. Visual-feedback preferences only (the extension is policy-free;
// governance lives in the binary). Both settings are chrome.storage.local booleans the
// content-script indicator reads live: ghostlight_effects (master, default on) and
// ghostlight_captions (the subtitle track, default off).

const effects = document.getElementById("effects");
const captions = document.getElementById("captions");
const debugToggle = document.getElementById("debug");

function syncCaptionsAvailability() {
  // Captions are part of the visual feedback; if effects are off there is nothing to caption.
  captions.disabled = !effects.checked;
}

chrome.storage.local.get(
  ["ghostlight_effects", "ghostlight_captions", "ghostlight_debug"],
  (r) => {
    effects.checked = r.ghostlight_effects !== false; // default on
    captions.checked = !!r.ghostlight_captions; // default off
    debugToggle.checked = !!r.ghostlight_debug; // default off (ADR-0059)
    syncCaptionsAvailability();
  }
);

effects.addEventListener("change", () => {
  chrome.storage.local.set({ ghostlight_effects: effects.checked });
  syncCaptionsAvailability();
});

captions.addEventListener("change", () => {
  chrome.storage.local.set({ ghostlight_captions: captions.checked });
});

debugToggle.addEventListener("change", () => {
  chrome.storage.local.set({ ghostlight_debug: debugToggle.checked });
});

// --- Connection status: a live link indicator (polls the worker; the extension holds no policy).
// It flips green on its own when the native port connects, so a user who opens this page before
// starting the service sees it turn green without reopening.
const linkPill = document.getElementById("link-pill");
const linkText = document.getElementById("link-text");
const linkSub = document.getElementById("link-sub");

function renderLink(state) {
  if (state.killed) {
    linkPill.className = "pill";
    linkText.textContent = "Session ended";
    linkSub.textContent =
      "Browser access is severed. Start a new session from the toolbar popup to reconnect.";
  } else if (state.connected) {
    linkPill.className = "pill on";
    linkText.textContent = "Connected";
    linkSub.textContent = "The agent can reach this browser.";
  } else {
    linkPill.className = "pill wait";
    linkText.textContent = "Waiting";
    linkSub.textContent =
      "Waiting for the Ghostlight service. Start it, and this turns green on its own.";
  }
}

function refreshLink() {
  chrome.runtime.sendMessage({ type: "GET_SESSION_STATE" }, (state) => {
    renderLink(state || { killed: false, connected: false, attachedTabs: 0 });
  });
}

refreshLink();
setInterval(refreshLink, 1500);
