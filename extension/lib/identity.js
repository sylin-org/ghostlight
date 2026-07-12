// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- extension-owned browser identity (ADR-0061). The extension is the one component
// that persists across every relay reconnect AND service-worker relaunch, so it owns the browser's
// stable identity rather than letting the ephemeral relay guess it from an OS parent pid (the
// ADR-0058 approach, which degraded to a colliding pid=0 when proc::parent() could not resolve).
//
// A UUID is minted ONCE and persisted in chrome.storage.local (local, NOT session -- it must
// survive service-worker death) under "ghostlight_browser_id", then read back on every startup and
// announced to the service as the opening frame of each native-messaging connection. Always
// present, never blank, unique per browser profile, stable across all the churn.
//
// Pure and storage-agnostic (matches lib/debug.js / lib/grouping.js's injected-dependency
// precedent): the storage area and the UUID generator are parameters, so it is unit-testable with a
// fake storage and a deterministic generator, no mocked extension globals.
(function () {
// The chrome.storage.local key the minted UUID lives under. `local`, not `session`: the identity
// must survive a service-worker restart, which clears session storage.
const STORAGE_KEY = "ghostlight_browser_id";

function createBrowserIdentity(storage, generate) {
  const gen = generate || (() => crypto.randomUUID());
  // In-memory cache for this service-worker lifetime, so repeated connects within one worker never
  // re-hit storage. Storage remains the source of truth across worker restarts.
  let cached = null;

  // Resolve this browser's persistent id: the cached value, else the persisted one, else a freshly
  // minted UUID persisted for next time. Always resolves to a non-empty string -- a storage read
  // OR write failure still returns a usable in-memory id (a fresh mint per worker in the degenerate
  // case, which the service still accepts; it just would not survive a worker restart).
  async function get() {
    if (cached) return cached;
    try {
      const r = await storage.get(STORAGE_KEY);
      const existing = r && r[STORAGE_KEY];
      if (typeof existing === "string" && existing) {
        cached = existing;
        return cached;
      }
    } catch {
      /* storage unavailable; fall through to mint (best effort) */
    }
    const minted = gen();
    cached = minted;
    try {
      await storage.set({ [STORAGE_KEY]: minted });
    } catch {
      /* could not persist; still return the in-memory id for this session */
    }
    return minted;
  }

  return { get };
}

const GhostlightIdentity = { createBrowserIdentity, STORAGE_KEY };
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightIdentity;
} else {
  self.GhostlightIdentity = GhostlightIdentity;
}
})();
