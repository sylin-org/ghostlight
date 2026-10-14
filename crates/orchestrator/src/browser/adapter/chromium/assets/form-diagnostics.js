// Opt-in, content-free form observations and bounded local diagnostic persistence.
(function installFormDiagnostics(root, factory) {
  const api = factory();
  root.GhostlightFormDiagnostics = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(globalThis, function formDiagnosticsApi() {
  "use strict";

  const KEY = "ghostlight.form_diagnostics";
  const MESSAGE_KIND = "form_diagnostics";
  const STATE_MESSAGE_KIND = "form_diagnostics_state";
  const LIMIT = 400;
  const CONTROL_LIMIT = 100;
  const INTERVAL_MS = 250;
  const DURATION_MS = 10 * 60 * 1000;
  const EVENTS = Object.freeze(["trace_started", "trace_stopped", "trace_expired", "checkpoint", "operation_started", "operation_finished"]);
  const TRIGGERS = Object.freeze(["enabled", "disabled", "expired", "poll", "window_focus", "window_blur", "focusin", "focusout", "visibilitychange", "pagehide", "pageshow", "reset", "operation"]);
  const OPERATIONS = new Set(["fill", "type", "clear"]);
  const COUNTS = ["control_count", "nonempty_count", "became_empty", "became_nonempty", "added_count", "removed_count"];
  const FLAGS = ["limited", "input_seen", "beforeinput_seen", "change_seen", "trusted_input_seen", "synthetic_input_seen", "focused", "succeeded", "persisted"];
  const INPUT_TYPES = new Set(["text", "search", "tel", "url", "email", "number", "date", "datetime-local", "month", "week", "time", "color", "range"]);
  const DOCUMENT_ID = /^(?:[a-f0-9]{32}|[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})$/i;
  const flagState = () => ({ input_seen: false, beforeinput_seen: false, change_seen: false, trusted_input_seen: false, synthetic_input_seen: false });
  const validCount = (value, maximum) => Number.isSafeInteger(value) && value >= 0 && value <= maximum;

  function validIdentity(identity) {
    return Number.isSafeInteger(identity?.tab_id) && identity.tab_id > 0
      && typeof identity.document_id === "string" && DOCUMENT_ID.test(identity.document_id);
  }

  // Reconstruct every row, including restored rows, rather than copying caller fields.
  function project(row) {
    if (!row || !EVENTS.includes(row.event) || !TRIGGERS.includes(row.trigger)) return null;
    const result = { event: row.event, trigger: row.trigger };
    for (const key of COUNTS) if (validCount(row[key], CONTROL_LIMIT)) result[key] = row[key];
    for (const key of FLAGS) if (typeof row[key] === "boolean") result[key] = row[key];
    if (["visible", "hidden", "unknown"].includes(row.visibility)) result.visibility = row.visibility;
    if (OPERATIONS.has(row.operation)) result.operation = row.operation;
    if (validCount(row.elapsed_ms, DURATION_MS)) result.elapsed_ms = row.elapsed_ms;
    return result;
  }

  // The caller supplies already-composed controls and owns the diagnostic scope.
  function createObserver({ document, window, queryControls, credentialClass, emit, now = Date.now,
    setInterval = globalThis.setInterval, clearInterval = globalThis.clearInterval,
    setTimeout = globalThis.setTimeout, clearTimeout = globalThis.clearTimeout }) {
    let enabled = false, requested = false, started = 0, interval = null, expiry = null;
    let tracked = new Map(), pending = flagState(), listeners = [];
    let last = { control_count: 0, nonempty_count: 0, became_empty: 0, became_nonempty: 0,
      added_count: 0, removed_count: 0, limited: false };

    function ordinary(element) {
      if (!element || element.isConnected === false || credentialClass(element)) return false;
      const tag = String(element.tagName ?? "").toLowerCase();
      if (tag === "input" && !INPUT_TYPES.has(element.type || "text")) return false;
      if (!["input", "textarea", "select"].includes(tag) && !element.isContentEditable) return false;
      if (element.hidden || element.getAttribute?.("aria-hidden") === "true") return false;
      const style = window.getComputedStyle?.(element);
      if (style && (style.display === "none" || style.visibility === "hidden" || style.opacity === "0")) return false;
      const box = element.getBoundingClientRect?.();
      return !box || (box.width > 0 && box.height > 0);
    }

    function sample() {
      try {
        const next = new Map();
        let limited = false, becameEmpty = 0, becameNonempty = 0, added = 0, nonempty = 0;
        for (const element of queryControls()) {
          if (!ordinary(element) || next.has(element)) continue;
          if (next.size === CONTROL_LIMIT) { limited = true; break; }
          // Only this boolean survives the read. Never retain a value, hash, or length.
          const present = Boolean(element.isContentEditable ? element.textContent : element.value);
          next.set(element, present);
          if (present) nonempty++;
          if (!tracked.has(element)) added++;
          else if (tracked.get(element) !== present) {
            if (present) becameNonempty++; else becameEmpty++;
          }
        }
        let removed = 0;
        for (const element of tracked.keys()) if (!next.has(element)) removed++;
        tracked = next;
        last = { control_count: next.size, nonempty_count: nonempty, became_empty: becameEmpty,
          became_nonempty: becameNonempty, added_count: added, removed_count: removed, limited };
        return last;
      } catch (_) { return null; }
    }

    function deliver(event, trigger, extra = {}, current = last) {
      try {
        const row = project({ ...current, ...pending, event, trigger, ...extra,
          visibility: ["visible", "hidden"].includes(document.visibilityState) ? document.visibilityState : "unknown",
          focused: document.hasFocus?.() === true,
          elapsed_ms: Math.max(0, Math.min(DURATION_MS, Math.floor(now() - started))) });
        pending = flagState();
        const result = emit(row);
        if (result && typeof result.catch === "function") result.catch(() => {});
      } catch (_) { /* Diagnostic delivery cannot change the observed page. */ }
    }

    function detach() {
      enabled = false;
      for (const [surface, name, listener] of listeners) {
        try { surface.removeEventListener(name, listener, true); } catch (_) { /* already gone */ }
      }
      listeners = [];
      try { if (interval !== null) clearInterval(interval); } catch (_) { /* timer unavailable */ }
      try { if (expiry !== null) clearTimeout(expiry); } catch (_) { /* timer unavailable */ }
      interval = expiry = null;
      tracked.clear();
      pending = flagState();
    }

    function stop(expired) {
      if (!enabled) return;
      // Turning the flag off does not perform one final page read.
      deliver(expired ? "trace_expired" : "trace_stopped", expired ? "expired" : "disabled", {},
        { ...last, became_empty: 0, became_nonempty: 0, added_count: 0, removed_count: 0 });
      detach();
    }

    function checkpoint(trigger, extra = {}, event = "checkpoint") {
      if (!enabled) return;
      if (now() - started >= DURATION_MS) { stop(true); return; }
      const previouslyLimited = last.limited;
      const current = sample();
      if (!current) {
        if (trigger !== "poll") deliver(event, trigger, extra,
          { ...last, became_empty: 0, became_nonempty: 0, added_count: 0, removed_count: 0, limited: true });
        return;
      }
      if (trigger === "poll" && !current.became_empty && !current.became_nonempty && !current.added_count && !current.removed_count && current.limited === previouslyLimited) return;
      deliver(event, trigger, extra, current);
    }

    function listen(surface, name, callback) {
      const listener = event => {
        if (!enabled) return;
        try { callback(event); } catch (_) { /* Page event dispatch must never be affected. */ }
      };
      surface.addEventListener(name, listener, true);
      listeners.push([surface, name, listener]);
    }

    function setEnabled(value) {
      if (value !== true) { requested = false; stop(false); return; }
      // Repeated scope synchronization must not restart an expired trace.
      if (requested) return;
      requested = true;
      try {
        enabled = true; started = now(); tracked = new Map(); pending = flagState();
        last = { control_count: 0, nonempty_count: 0, became_empty: 0, became_nonempty: 0,
          added_count: 0, removed_count: 0, limited: false };
        for (const name of ["beforeinput", "input", "change"]) listen(document, name, event => {
          const target = event.composedPath?.()[0] ?? event.target;
          if (!tracked.has(target) || !ordinary(target)) return;
          pending[`${name}_seen`] = true;
          if (name === "input") pending[event.isTrusted === true ? "trusted_input_seen" : "synthetic_input_seen"] = true;
        });
        for (const name of ["focusin", "focusout", "visibilitychange", "reset"]) listen(document, name, () => checkpoint(name));
        for (const name of ["focus", "blur"]) listen(window, name, event => {
          if (event.target === window) checkpoint(`window_${name}`);
        });
        for (const name of ["pagehide", "pageshow"]) listen(window, name, event => checkpoint(name, { persisted: event.persisted === true }));
        checkpoint("enabled", {}, "trace_started");
        interval = setInterval(() => { try { checkpoint("poll"); } catch (_) { /* observer only */ } }, INTERVAL_MS);
        expiry = setTimeout(() => stop(true), DURATION_MS);
      } catch (_) { detach(); }
    }

    function run(operation, callback) {
      if (!enabled || !OPERATIONS.has(operation)) return callback();
      try { checkpoint("operation", { operation }, "operation_started"); } catch (_) { /* observer only */ }
      let succeeded = false;
      try {
        const result = callback();
        succeeded = true;
        return result;
      } finally {
        try { checkpoint("operation", { operation, succeeded }, "operation_finished"); } catch (_) { /* preserve callback result */ }
      }
    }

    return Object.freeze({ setEnabled, run });
  }

  // Persistence accepts only projected structural rows and browser-supplied identity.
  function createLog({ storage, debugKey, now = Date.now }) {
    let enabled = false, entries = [], writeFailures = 0;
    function storedRow(row, identity, time) {
      const result = project(row);
      if (!result || !validIdentity(identity) || !validCount(time, Number.MAX_SAFE_INTEGER)) return null;
      return { ...result, time_ms: time, tab_id: identity.tab_id, document_id: identity.document_id };
    }
    let queue = Promise.resolve().then(() => storage.get([debugKey, KEY])).then(saved => {
      enabled = saved?.[debugKey] === true;
      entries = (Array.isArray(saved?.[KEY]?.entries) ? saved[KEY].entries : []).slice(-LIMIT)
        .map(row => storedRow(row, row, row?.time_ms)).filter(Boolean);
    }).catch(() => { writeFailures++; });
    function record(row, identity) {
      let projected;
      try { projected = storedRow(row, identity, Math.floor(now())); } catch (_) { return Promise.resolve(); }
      if (!projected) return Promise.resolve();
      queue = queue.then(async () => {
        if (!enabled) return;
        entries.push(projected); entries = entries.slice(-LIMIT);
        await storage.set({ [KEY]: { entries } });
      }).catch(() => { writeFailures++; });
      return queue;
    }
    function setEnabled(value) {
      queue = queue.then(() => { enabled = value === true; }).catch(() => { writeFailures++; });
      return queue;
    }
    async function snapshot() {
      await queue;
      return { schema: "ghostlight-form-diagnostics-1", enabled, write_failures: writeFailures,
        entries: entries.map(row => ({ ...row })) };
    }
    return Object.freeze({ record, setEnabled, snapshot });
  }

  return Object.freeze({ KEY, MESSAGE_KIND, STATE_MESSAGE_KIND, LIMIT, CONTROL_LIMIT, INTERVAL_MS, DURATION_MS, EVENTS, TRIGGERS, validIdentity, createObserver, createLog });
});
