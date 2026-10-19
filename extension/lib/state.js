// Ghostlight -- the extension's browser-local state: its storage keys, the person's preferences
// and the toolbar badge.
(function installGhostlightState(root, factory) {
  const api = factory();
  root.GhostlightState = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightState() {
  "use strict";

  const BROWSER_ID_KEY = "ghostlight.browser_id";
  // The life of this extension in this browser run, kept in session storage. Chrome keeps it across a
  // worker and a native-host restart and clears it on a browser restart and on an extension reload,
  // update or disable, so one life names one browser run, in which tab ids are never reused.
  const LIFE_KEY = "ghostlight.life";
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
  // The toolbar badge: the accent for a pause, red for a recording. The text is dark on every badge colour.
  const BADGE_HELD = "#5eead4";
  const BADGE_RECORDING = "#ef4444";
  const BADGE_TEXT_COLOR = "#08110f";

  function newBrowserId(randomUuid) {
    return `browser_${randomUuid().replaceAll("-", "")}`;
  }

  function newLife(randomUuid) {
    return `life_${randomUuid().replaceAll("-", "")}`;
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

  function badge(snapshot) {
    if (snapshot.control_state === "held") return { text: "II", color: BADGE_HELD };
    if (snapshot.recording_tabs > 0) return { text: "REC", color: BADGE_RECORDING };
    return { text: "", color: BADGE_HELD };
  }

  return Object.freeze({
    BROWSER_ID_KEY,
    LIFE_KEY,
    PRESENTATIONS_KEY,
    EFFECTS_KEY,
    CAPTIONS_KEY,
    DEBUG_KEY,
    PRESERVE_TABS_KEY,
    DEFAULT_PREFERENCES,
    BADGE_TEXT_COLOR,
    newBrowserId,
    newLife,
    preferences,
    preferencesFromStorage,
    preferencesForStorage,
    badge
  });
});
