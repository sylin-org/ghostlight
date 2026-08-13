// Ghostlight workbench -- the only thing here that talks to the orchestrator.
//
// Every call out of this window goes through one object, so what the surface can ask for is a
// list you can read in one sitting. It holds no state and touches no document: give it a way to
// invoke and a way to listen, and it will tell you what came back or throw trying.
(function installGhostlightTransport(root, factory) {
  const api = factory();
  root.GhostlightTransport = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightTransportApi() {
  "use strict";

  function create({ invoke, listen, changeEvent } = {}) {
    /** Whether there is an orchestrator to talk to at all. */
    const available = typeof invoke === "function";

    function call(command, args) {
      if (!available) return Promise.reject(new Error("no orchestrator is attached"));
      return invoke(command, args);
    }

    return Object.freeze({
      available,

      /** The whole current truth, which the store then decides what to do with. */
      snapshot: () => call("workbench_snapshot"),

      /** Ranked matches for the palette. */
      search: (query) => call("workbench_search", { query }),

      /** Hold, resume, end or start the runtime session. */
      applyIntent: (intent) => call("apply_runtime_intent", { intent }),

      /** Re-detect the MCP clients installed for this user. */
      refreshHarnesses: () => call("refresh_harnesses"),

      /** Connect or disconnect Ghostlight from one MCP client. */
      manageHarness: (id, action) => call("manage_harness", { id, action }),

      /** Prove the notification path end to end. */
      testNotification: () => call("test_notification"),

      /**
       * Open one of the places the product is willing to point at.
       *
       * A name, never an address: the orchestrator owns the URL, so this window cannot be talked
       * into opening something the product did not choose.
       */
      openDestination: (destination) => call("open_destination", { destination }),

      /** Sequenced changes, pushed. Returns false when there is nothing to subscribe to. */
      subscribe(handler) {
        if (typeof listen !== "function") return false;
        listen(changeEvent, (message) => handler(message.payload));
        return true;
      }
    });
  }

  return Object.freeze({ create });
});
