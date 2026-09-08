// Local, opt-in connection evidence that survives worker and browser restarts.
(function installConnectionLog(root, factory) {
  const api = factory();
  root.GhostlightConnectionLog = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(globalThis, function connectionLogApi() {
  "use strict";
  const KEY = "ghostlight.connection_log";
  const LIMIT = 400;
  const EVENTS = Object.freeze({
    WORKER_STARTED: "worker_started", INITIALIZE_STARTED: "initialize_started",
    INITIALIZE_FINISHED: "initialize_finished", CONNECT_REQUESTED: "connect_requested",
    NATIVE_PORT_OPENED: "native_port_opened", NATIVE_HELLO_SENT: "native_hello_sent",
    NATIVE_HELLO_SKIPPED: "native_hello_skipped",
    NATIVE_DISCONNECTED: "native_disconnected", CONNECT_FAILED: "connect_failed",
    RETRY_SCHEDULED: "retry_scheduled", RETRY_FAILED: "retry_failed", ALARM_FIRED: "alarm_fired",
    BROWSER_STARTED: "browser_started", EXTENSION_INSTALLED: "extension_installed",
    BACKEND_UNAVAILABLE: "backend_unavailable", HELLO_ACCEPTED: "hello_accepted",
    PREFERENCES_CHANGED: "preferences_changed"
  });
  const names = new Set(Object.values(EVENTS));
  const fields = new Set(["attempt", "trigger", "stage", "error", "browser_id", "has_port",
    "pending", "scheduled_time", "duration_ms", "service_version", "compatible"]);
  const bounded = value => String(value ?? "").slice(0, 500);
  function create({ storage, debugKey, context, now = Date.now }) {
    let enabled = false, entries = [], boots = [], writeFailures = 0;
    const identity = Object.fromEntries(["epoch", "adapter_version", "extension_id", "browser_version"]
      .map(key => [key, bounded(context[key])]));
    let queue = storage.get([debugKey, KEY]).then(saved => {
      enabled = saved[debugKey] === true;
      // Re-project saved fields as well: arbitrary stored data is never exported as diagnostics.
      entries = (Array.isArray(saved[KEY]?.entries) ? saved[KEY].entries : []).slice(-LIMIT)
        .filter(row => names.has(row.event)).map(row => project(row.event, row, row));
      boots = (Array.isArray(saved[KEY]?.boots) ? saved[KEY].boots : []).slice(-16)
        .map(row => project(EVENTS.WORKER_STARTED, row, row));
    }).catch(() => { writeFailures++; });
    function project(event, details, origin = {}) {
      const row = { time_ms: Number.isFinite(origin.time_ms) ? origin.time_ms : now(),
        epoch: bounded(origin.epoch ?? identity.epoch), event };
      for (const [key, value] of Object.entries(details)) {
        if (fields.has(key) && ["string", "boolean", "number"].includes(typeof value)) {
          row[key] = typeof value === "string" ? bounded(value) : value;
        }
      }
      return row;
    }
    function record(event, details = {}) {
      if (!names.has(event)) return Promise.resolve();
      const row = project(event, details);
      // Serialize persistence only. Connection work never waits for log writes.
      queue = queue.then(async () => {
        if (!enabled) return;
        entries.push(row); entries = entries.slice(-LIMIT);
        if (event === EVENTS.WORKER_STARTED) { boots.push(row); boots = boots.slice(-16); }
        await storage.set({ [KEY]: { entries, boots } });
      }).catch(() => { writeFailures++; });
      return queue;
    }
    function setEnabled(value) {
      queue = queue.then(() => { enabled = value === true; }).catch(() => { writeFailures++; });
      return queue;
    }
    async function snapshot() {
      await queue;
      return { schema: "ghostlight-connection-log-1", context: identity, enabled,
        write_failures: writeFailures, boots: structuredClone(boots), entries: structuredClone(entries) };
    }
    return Object.freeze({ record, setEnabled, snapshot });
  }
  return Object.freeze({ KEY, LIMIT, EVENTS, create });
});
