// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- developer diagnostics (ADR-0059): the extension's half of the debug-event
// forwarder. Off by default (chrome.storage.local "ghostlight_debug"); when on, sends a
// fire-and-forget `{"type":"debug_event","event","detail"}` note the service appends into its
// own structured per-pid debug log (`ghostlight_core::hub::outbound::diagnostics`), so one file
// shows the extension's view of a connection interleaved with the service's, by arrival order.
//
// Pure and storage/port agnostic (matches lib/grouping.js's injected-chrome precedent): takes
// the storage area and a `post` callback as parameters rather than reaching for `chrome.runtime`
// or a module-level `nativePort` itself, so it is unit-testable with a fake storage and no
// mocked extension globals.
(function () {
const MAX_PENDING_DEBUG_EVENTS = 20;

// One forwarder instance per service-worker lifetime (holds the buffer of notes raised while no
// port was open -- most usefully, from inside onDisconnect itself, where the note about WHY the
// port died obviously cannot go out on the port that just died).
function createDebugForwarder(storage) {
  let pending = [];

  async function send(post, event, detail) {
    let on = false;
    try {
      const r = await storage.get("ghostlight_debug");
      on = !!(r && r.ghostlight_debug);
    } catch {
      return; // storage unavailable; never let diagnostics itself throw
    }
    if (!on) return;
    const msg = { type: "debug_event", event, detail: detail === undefined ? null : detail };
    if (post) {
      try {
        post(msg);
        return;
      } catch {
        /* fall through to buffering */
      }
    }
    pending.push(msg);
    if (pending.length > MAX_PENDING_DEBUG_EVENTS) pending.shift();
  }

  function flush(post) {
    if (!post || pending.length === 0) return;
    const queued = pending;
    pending = [];
    for (const msg of queued) {
      try {
        post(msg);
      } catch {
        break; // port gone again; drop the rest rather than lose the buffer's ordering
      }
    }
  }

  return { send, flush };
}

const GhostlightDebug = { createDebugForwarder };
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightDebug;
} else {
  self.GhostlightDebug = GhostlightDebug;
}
})();
