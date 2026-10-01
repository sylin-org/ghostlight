// Isolated-world, one-way delivery of bounded structural form observations.
// Chromium supplies sender identity; page rows cannot request work or state changes.
(function installFormDiagnosticsRelay(root) {
  "use strict";
  if (root.GhostlightFormDiagnosticsRelay || window.self !== window.top) return;
  const api = root.GhostlightFormDiagnostics;
  let pending = Promise.resolve();
  let count = 0;

  window.addEventListener(api.ROW_EVENT_KIND, event => {
    try {
      if (event.target !== window || typeof event.detail !== "string"
        || event.detail.length > api.ROW_MAX_CHARS || count >= api.LIMIT) return;
      const row = api.project(JSON.parse(event.detail));
      if (!row) return;
      count++;
      pending = pending.then(() => chrome.runtime.sendMessage({ kind: api.MESSAGE_KIND, row }))
        .catch(() => {})
        .finally(() => { count--; });
    } catch (_) { /* Optional diagnostics cannot fail page event dispatch. */ }
  });

  root.GhostlightFormDiagnosticsRelay = Object.freeze({
    async flush() { await pending; }
  });
})(globalThis);
