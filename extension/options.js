// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- options page. Visual-feedback preferences only (the extension is policy-free;
// governance lives in the binary). Both settings are chrome.storage.local booleans the
// content-script indicator reads live: ghostlight_effects (master, default on) and
// ghostlight_captions (the subtitle track, default off).

const effects = document.getElementById("effects");
const captions = document.getElementById("captions");

function syncCaptionsAvailability() {
  // Captions are part of the visual feedback; if effects are off there is nothing to caption.
  captions.disabled = !effects.checked;
}

chrome.storage.local.get(["ghostlight_effects", "ghostlight_captions"], (r) => {
  effects.checked = r.ghostlight_effects !== false; // default on
  captions.checked = !!r.ghostlight_captions; // default off
  syncCaptionsAvailability();
});

effects.addEventListener("change", () => {
  chrome.storage.local.set({ ghostlight_effects: effects.checked });
  syncCaptionsAvailability();
});

captions.addEventListener("change", () => {
  chrome.storage.local.set({ ghostlight_captions: captions.checked });
});
