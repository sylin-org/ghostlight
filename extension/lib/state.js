(function installGhostlightState(root, factory) {
  const api = factory();
  root.GhostlightState = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightState() {
  "use strict";

  const BROWSER_ID_KEY = "ghostlight.browser_id";
  const TOPOLOGY_KEY = "ghostlight.topology";
  const OPERATIONS_KEY = "ghostlight.operations";
  const PRESENTATIONS_KEY = "ghostlight.presentations";
  const EFFECTS_KEY = "ghostlight_effects";
  const CAPTIONS_KEY = "ghostlight_captions";
  const DEBUG_KEY = "ghostlight_debug";
  const PRESERVE_TABS_KEY = "ghostlight_preserve_tabs";
  const DEFAULT_PREFERENCES = Object.freeze({
    effects: true,
    captions: false,
    diagnostics: false,
    preserveTabs: true
  });
  const CONTROL_STATES = Object.freeze(["active", "held", "attention", "ended"]);

  function newBrowserId(randomUuid) {
    return `browser_${randomUuid().replaceAll("-", "")}`;
  }

  function preferences(value) {
    const source = value && typeof value === "object" ? value : {};
    return {
      effects: source.effects !== false,
      captions: source.captions === true,
      diagnostics: source.diagnostics === true,
      preserveTabs: source.preserveTabs !== false
    };
  }

  function preferencesFromStorage(value) {
    const source = value && typeof value === "object" ? value : {};
    return preferences({
      effects: source[EFFECTS_KEY],
      captions: source[CAPTIONS_KEY],
      diagnostics: source[DEBUG_KEY],
      preserveTabs: source[PRESERVE_TABS_KEY]
    });
  }

  function preferencesForStorage(value) {
    const normalized = preferences(value);
    return {
      [EFFECTS_KEY]: normalized.effects,
      [CAPTIONS_KEY]: normalized.captions,
      [DEBUG_KEY]: normalized.diagnostics,
      [PRESERVE_TABS_KEY]: normalized.preserveTabs
    };
  }

  function controlState(value) {
    return CONTROL_STATES.includes(value) ? value : "active";
  }

  function connectionLabel(snapshot) {
    if (!snapshot.connected) return "Service disconnected";
    if (!snapshot.compatible) return "Version mismatch";
    if (snapshot.control_state === "held") return "Paused";
    if (snapshot.control_state === "attention") return "Needs attention";
    if (snapshot.control_state === "ended") return "Session ended";
    return "Connected";
  }

  function badge(snapshot) {
    if (snapshot.control_state === "attention") return { text: "!", color: "#dc2626" };
    if (snapshot.control_state === "held") return { text: "II", color: "#38bdf8" };
    if (snapshot.recording_tabs > 0) return { text: "REC", color: "#ef4444" };
    return { text: "", color: "#38bdf8" };
  }

  return Object.freeze({
    BROWSER_ID_KEY,
    TOPOLOGY_KEY,
    OPERATIONS_KEY,
    PRESENTATIONS_KEY,
    EFFECTS_KEY,
    CAPTIONS_KEY,
    DEBUG_KEY,
    PRESERVE_TABS_KEY,
    DEFAULT_PREFERENCES,
    newBrowserId,
    preferences,
    preferencesFromStorage,
    preferencesForStorage,
    controlState,
    connectionLabel,
    badge
  });
});
