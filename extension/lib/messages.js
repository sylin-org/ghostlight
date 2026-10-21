// Ghostlight -- the words the extension's own parts send each other: its pages and the worker, and
// the worker and the offscreen document. They never leave the browser, so they are not protocol
// words; protocol.js holds those.
(function installGhostlightMessages(root, factory) {
  const api = factory();
  root.GhostlightMessages = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightMessagesApi() {
  "use strict";

  // What a page asks the worker, and the one broadcast the worker sends every page when its state
  // changes.
  const PAGE = Object.freeze({
    UI_SNAPSHOT: "ui_snapshot",
    UI_STATE_CHANGED: "ui_state_changed",
    GET_PREFERENCES: "get_preferences",
    SET_PREFERENCES: "set_preferences",
    RUNTIME_CONTROL: "runtime_control",
    SHOW_GHOSTLIGHT: "show_ghostlight",
    RELEASE_DEBUGGER_SESSIONS: "release_debugger_sessions",
    CONNECTION_DIAGNOSTICS: "connection_diagnostics"
  });

  // The offscreen document's address and what the worker asks of it.
  const OFFSCREEN = Object.freeze({
    TARGET: "ghostlight-offscreen",
    ENCODE: "encode_recording",
    RELEASE: "release_recording"
  });

  // The two forms an encoded GIF comes back in: an object URL a download reads without the bytes
  // passing through the worker, or the bytes as base64 for a destination that must receive them.
  const TRANSFER = Object.freeze({ OBJECT_URL: "object_url", BASE64: "base64" });

  // The keyboard commands manifest.json declares, which cannot import this file; a test ties them.
  const COMMAND = Object.freeze({ TOGGLE_HOLD: "toggle-hold" });

  return Object.freeze({ PAGE, OFFSCREEN, TRANSFER, COMMAND });
});
