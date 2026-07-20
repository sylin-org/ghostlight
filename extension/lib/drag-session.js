// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- bounded per-tab coordinator for CDP native drag interception.
//
// IIFE-wrapped because importScripts and content-script injection share global lexical scopes. This
// module owns correlation, phases, and expiry. Drag data stays opaque and is returned only to CDP.
(function () {
const DRAG_SESSION_PHASES = Object.freeze({
  ARMED: "armed",
  NATIVE: "native",
  POINTER: "pointer",
  CANCELLED: "cancelled",
});

const DRAG_OBSERVATION_MESSAGES = Object.freeze({
  BEGIN: "dragObservationBegin",
  FINISH: "dragObservationFinish",
});

function createDragCoordinator(defaultTimeoutMs) {
  const defaultWaitMs = Number.isFinite(defaultTimeoutMs) && defaultTimeoutMs >= 0
    ? defaultTimeoutMs
    : 250;
  const sessions = new Map();

  function settle(session, result) {
    if (session.settled) return;
    session.settled = true;
    session.resolve(result);
  }

  function cancel(tabId) {
    const session = sessions.get(tabId);
    if (!session) return false;
    sessions.delete(tabId);
    session.phase = DRAG_SESSION_PHASES.CANCELLED;
    settle(session, { mode: DRAG_SESSION_PHASES.CANCELLED });
    return true;
  }

  function begin(tabId) {
    cancel(tabId);
    let resolve;
    const promise = new Promise((done) => { resolve = done; });
    const session = {
      tabId,
      phase: DRAG_SESSION_PHASES.ARMED,
      promise,
      resolve,
      settled: false,
    };
    sessions.set(tabId, session);
    return session;
  }

  function intercepted(tabId, data) {
    const session = sessions.get(tabId);
    if (!session) return false;
    session.phase = DRAG_SESSION_PHASES.NATIVE;
    settle(session, { mode: DRAG_SESSION_PHASES.NATIVE, data });
    return true;
  }

  async function finish(session, timeoutMs) {
    if (!session || sessions.get(session.tabId) !== session) {
      return { mode: DRAG_SESSION_PHASES.CANCELLED };
    }
    const waitMs = Number.isFinite(timeoutMs) && timeoutMs >= 0 ? timeoutMs : defaultWaitMs;
    let timer = null;
    const timeout = new Promise((resolve) => {
      timer = setTimeout(() => resolve({ mode: DRAG_SESSION_PHASES.POINTER }), waitMs);
    });
    const result = await Promise.race([session.promise, timeout]);
    if (timer !== null) clearTimeout(timer);
    if (session.phase === DRAG_SESSION_PHASES.CANCELLED) {
      return { mode: DRAG_SESSION_PHASES.CANCELLED };
    }
    if (sessions.get(session.tabId) === session) {
      sessions.delete(session.tabId);
      if (result.mode === DRAG_SESSION_PHASES.POINTER) {
        session.phase = DRAG_SESSION_PHASES.POINTER;
        settle(session, result);
      }
    }
    return result;
  }

  function clear() {
    for (const tabId of Array.from(sessions.keys())) cancel(tabId);
  }

  return { begin, intercepted, finish, cancel, clear };
}

const GhostlightDragSession = {
  DRAG_SESSION_PHASES,
  DRAG_OBSERVATION_MESSAGES,
  createDragCoordinator,
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightDragSession;
} else {
  self.GhostlightDragSession = GhostlightDragSession;
}
})();
