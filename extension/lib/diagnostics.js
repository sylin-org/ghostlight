// Ghostlight -- bounded volatile console and network evidence for controlled tabs.
(function installGhostlightDiagnostics(root, factory) {
  const api = factory();
  root.GhostlightDiagnostics = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(globalThis, function createGhostlightDiagnosticsApi() {
  "use strict";

  const DEFAULT_MAX_ENTRIES = 1000;
  const DEFAULT_MAX_BYTES = 2 * 1024 * 1024;
  const DEFAULT_IDLE_MS = 5 * 60 * 1000;
  const DEFAULT_READ_LIMIT = 50;
  const MAX_READ_LIMIT = 200;
  const MAX_CONSOLE_CHARS = 2000;
  const MAX_MATCH_CHARS = 500;
  const MAX_CURSOR_CHARS = 160;
  const MAX_URL_CHARS = 2048;
  const MAX_METHOD_CHARS = 32;
  const MAX_RESOURCE_TYPE_CHARS = 80;
  const MAX_FAILURE_CHARS = 160;
  const MAX_CLEAR_TABS = 256;
  const DEFAULT_MAX_EXECUTION_CONTEXTS = 256;
  const MAX_STACK_TRACE_DEPTH = 4;
  const MAX_STACK_FRAMES = 32;
  const CURSOR_PATTERN = /^diag_([0-9a-z]+)_([0-9a-f]{8})$/;

  function bounded(value, maximum) {
    return String(value ?? "").slice(0, maximum);
  }

  function requirePositiveInteger(value, name) {
    if (!Number.isInteger(value) || value < 1) {
      throw new RangeError(`${name} must be a positive integer`);
    }
    return value;
  }

  function requireTabId(tabId) {
    if (!Number.isSafeInteger(tabId) || tabId < 0) {
      throw new RangeError("tabId must be a non-negative safe integer");
    }
    return tabId;
  }

  function randomSecret() {
    try {
      const bytes = new Uint32Array(4);
      globalThis.crypto.getRandomValues(bytes);
      return Array.from(bytes, (value) => value.toString(16).padStart(8, "0")).join("");
    } catch (_error) {
      return `${Date.now().toString(36)}_${Math.random().toString(36).slice(2)}`;
    }
  }

  function checksum(value) {
    let hash = 2166136261;
    for (let index = 0; index < value.length; index += 1) {
      hash ^= value.charCodeAt(index);
      hash = Math.imul(hash, 16777619);
    }
    return (hash >>> 0).toString(16).padStart(8, "0");
  }

  function renderRemoteObject(value) {
    if (!value || typeof value !== "object") return bounded(value, MAX_CONSOLE_CHARS);
    if (Object.prototype.hasOwnProperty.call(value, "value")) {
      if (typeof value.value === "string") return value.value;
      try {
        const rendered = JSON.stringify(value.value);
        if (rendered !== undefined) return rendered;
      } catch (_error) {
        // Fall through to a content-free type or bounded CDP description.
      }
      return String(value.value);
    }
    if (value.unserializableValue !== undefined) return String(value.unserializableValue);
    if (value.description !== undefined) return String(value.description);
    if (value.subtype === "null") return "null";
    return String(value.type ?? "unknown");
  }

  function consoleText(params) {
    const args = Array.isArray(params?.args) ? params.args : [];
    const rendered = args.map(renderRemoteObject).join(" ");
    return bounded(rendered || params?.type || "console", MAX_CONSOLE_CHARS);
  }

  function exceptionText(params) {
    const details = params?.exceptionDetails ?? {};
    const exception = details.exception;
    const rendered = exception ? renderRemoteObject(exception) : "";
    return bounded(rendered || details.text || "Uncaught exception", MAX_CONSOLE_CHARS);
  }

  function consoleLevel(value) {
    const level = bounded(value || "log", 32).toLowerCase();
    if (level === "warn") return "warning";
    if (level === "assert") return "error";
    return level;
  }

  function sanitizeUrl(value) {
    try {
      const parsed = new URL(String(value));
      if (parsed.protocol === "blob:") {
        return bounded(`blob:${sanitizeUrl(parsed.pathname)}`, MAX_URL_CHARS);
      }
      if (parsed.origin === "null") return bounded(parsed.protocol, MAX_URL_CHARS);
      return bounded(`${parsed.origin}${parsed.pathname || "/"}`, MAX_URL_CHARS);
    } catch (_error) {
      return "invalid:";
    }
  }

  function resourceType(value) {
    return bounded(value || "other", MAX_RESOURCE_TYPE_CHARS).toLowerCase();
  }

  function methodName(value) {
    return bounded(value || "unknown", MAX_METHOD_CHARS).toUpperCase();
  }

  function responseStatus(value) {
    const status = Number(value);
    if (!Number.isInteger(status) || status < 0 || status > 65535) return null;
    return status;
  }

  function failureCategory(params) {
    if (params?.blockedReason) return bounded(`blocked:${params.blockedReason}`, MAX_FAILURE_CHARS);
    if (params?.canceled) return "cancelled";
    const text = String(params?.errorText ?? "");
    const networkCode = /net::[A-Z0-9_]+/i.exec(text)?.[0];
    return bounded(networkCode || "failed", MAX_FAILURE_CHARS);
  }

  function approximateBytes(entry) {
    let characters = 0;
    for (const value of Object.values(entry)) {
      if (typeof value === "string") characters += value.length;
    }
    return 128 + (characters * 2);
  }

  function create({
    maximumEntries = DEFAULT_MAX_ENTRIES,
    maximumBytes = DEFAULT_MAX_BYTES,
    maximumExecutionContexts = DEFAULT_MAX_EXECUTION_CONTEXTS,
    idleMs = DEFAULT_IDLE_MS,
    now = Date.now,
    cursorSecret = randomSecret(),
    setTimer = setTimeout,
    clearTimer = clearTimeout,
    onExpired = () => {}
  } = {}) {
    requirePositiveInteger(maximumEntries, "maximumEntries");
    requirePositiveInteger(maximumBytes, "maximumBytes");
    requirePositiveInteger(maximumExecutionContexts, "maximumExecutionContexts");
    requirePositiveInteger(idleMs, "idleMs");
    if (typeof now !== "function") throw new TypeError("now must be a function");
    if (typeof setTimer !== "function" || typeof clearTimer !== "function") {
      throw new TypeError("setTimer and clearTimer must be functions");
    }
    if (typeof onExpired !== "function") throw new TypeError("onExpired must be a function");
    if (typeof cursorSecret !== "string" || cursorSecret.length === 0) {
      throw new TypeError("cursorSecret must be a non-empty string");
    }

    const tabs = new Map();
    let generation = 0;

    function newState(tabId) {
      generation += 1;
      return {
        consoleEntries: [],
        networkEntries: [],
        requests: new Map(),
        executionContexts: new Map(),
        sequence: 0,
        evictedThrough: 0,
        retainedBytes: 0,
        captureStartedPending: true,
        cursorKey: `${cursorSecret}:${tabId}:${generation}`,
        timer: null,
        expiresAt: 0
      };
    }

    function expire(tabId, expected) {
      if (tabs.get(tabId) !== expected) return;
      expected.timer = null;
      tabs.delete(tabId);
      try {
        Promise.resolve(onExpired(tabId)).catch(() => {});
      } catch (_error) {
        // Expiry is complete even when the worker's best-effort CDP cleanup fails.
      }
    }

    function touch(tabId, state) {
      if (state.timer !== null) clearTimer(state.timer);
      state.expiresAt = currentTime() + idleMs;
      state.timer = setTimer(() => expire(tabId, state), idleMs);
      state.timer?.unref?.();
    }

    function activeState(tabId) {
      const state = tabs.get(tabId);
      if (!state) return null;
      if (currentTime() < state.expiresAt) return state;
      expire(tabId, state);
      return null;
    }

    function enable(tabId) {
      requireTabId(tabId);
      const existing = activeState(tabId);
      if (existing) {
        touch(tabId, existing);
        return false;
      }
      const state = newState(tabId);
      tabs.set(tabId, state);
      touch(tabId, state);
      return true;
    }

    function currentTime() {
      const value = Number(now());
      return Number.isFinite(value) && value >= 0 ? Math.floor(value) : 0;
    }

    function cursorFor(state, sequence) {
      const encoded = sequence.toString(36);
      return `diag_${encoded}_${checksum(`${state.cursorKey}:${encoded}`)}`;
    }

    function sequenceFromCursor(state, cursor) {
      if (typeof cursor !== "string" || cursor.length === 0 || cursor.length > MAX_CURSOR_CHARS) {
        throw new TypeError(`after must be an opaque diag_ cursor no longer than ${MAX_CURSOR_CHARS} characters`);
      }
      const match = CURSOR_PATTERN.exec(cursor);
      if (!match) throw new RangeError("after is not a valid diagnostic cursor");
      const sequence = Number.parseInt(match[1], 36);
      if (!Number.isSafeInteger(sequence)
        || sequence < 1
        || sequence > state.sequence
        || sequence.toString(36) !== match[1]
        || checksum(`${state.cursorKey}:${match[1]}`) !== match[2]) {
        throw new RangeError("after is not a valid diagnostic cursor for this tab capture");
      }
      return sequence;
    }

    function oldestEntry(state) {
      const consoleEntry = state.consoleEntries[0];
      const networkEntry = state.networkEntries[0];
      if (!consoleEntry) return networkEntry;
      if (!networkEntry) return consoleEntry;
      return consoleEntry.sequence < networkEntry.sequence ? consoleEntry : networkEntry;
    }

    function evictOldest(state) {
      const oldest = oldestEntry(state);
      if (!oldest) return false;
      const ring = oldest.entry === "console" ? state.consoleEntries : state.networkEntries;
      ring.shift();
      state.retainedBytes -= oldest.retainedBytes;
      state.evictedThrough = Math.max(state.evictedThrough, oldest.sequence);
      if (oldest.requestId && state.requests.get(oldest.requestId) === oldest) {
        state.requests.delete(oldest.requestId);
      }
      return true;
    }

    function retainedCount(state) {
      return state.consoleEntries.length + state.networkEntries.length;
    }

    function enforceBounds(state) {
      while (retainedCount(state) > maximumEntries || state.retainedBytes > maximumBytes) {
        if (!evictOldest(state)) break;
      }
    }

    function append(state, entry) {
      entry.sequence = state.sequence + 1;
      state.sequence = entry.sequence;
      entry.timestamp_ms = currentTime();
      entry.retainedBytes = approximateBytes(entry);
      const ring = entry.entry === "console" ? state.consoleEntries : state.networkEntries;
      ring.push(entry);
      state.retainedBytes += entry.retainedBytes;
      enforceBounds(state);
      return entry;
    }

    function usableUrl(value) {
      const url = sanitizeUrl(value);
      return url === "invalid:" ? null : url;
    }

    function stackTraceUrl(stackTrace) {
      let trace = stackTrace;
      for (let depth = 0; depth < MAX_STACK_TRACE_DEPTH && trace; depth += 1) {
        const frames = Array.isArray(trace.callFrames)
          ? trace.callFrames.slice(0, MAX_STACK_FRAMES)
          : [];
        for (const frame of frames) {
          const url = usableUrl(frame?.url);
          if (url) return url;
        }
        trace = trace.parent;
      }
      return null;
    }

    function contextId(value) {
      return Number.isSafeInteger(value) && value > 0 ? value : null;
    }

    function sameOrigin(left, right) {
      if (left === "invalid:" || right === "invalid:") return false;
      try {
        const leftOrigin = new URL(left).origin;
        const rightOrigin = new URL(right).origin;
        return leftOrigin !== "null" && leftOrigin === rightOrigin;
      } catch (_error) {
        return false;
      }
    }

    function consoleUrl(state, params) {
      const details = params?.exceptionDetails ?? {};
      const id = contextId(params?.executionContextId ?? details.executionContextId);
      const contextUrl = id === null ? null : state.executionContexts.get(id);
      if (!contextUrl || contextUrl === "invalid:") return "invalid:";
      const candidate = stackTraceUrl(params?.stackTrace ?? details.stackTrace)
        ?? usableUrl(details.url);
      return candidate && sameOrigin(candidate, contextUrl) ? candidate : contextUrl;
    }

    function executionContextCreated(tabId, params = {}) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      const id = contextId(params?.context?.id);
      if (id === null) return false;
      touch(tabId, state);
      state.executionContexts.delete(id);
      state.executionContexts.set(id, sanitizeUrl(params.context.origin));
      while (state.executionContexts.size > maximumExecutionContexts) {
        state.executionContexts.delete(state.executionContexts.keys().next().value);
      }
      return true;
    }

    function executionContextDestroyed(tabId, params = {}) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      const id = contextId(params?.executionContextId);
      if (id === null) return false;
      touch(tabId, state);
      return state.executionContexts.delete(id);
    }

    function executionContextsCleared(tabId) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      touch(tabId, state);
      state.executionContexts.clear();
      return true;
    }

    function consoleAPICalled(tabId, params = {}) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      touch(tabId, state);
      append(state, {
        entry: "console",
        level: consoleLevel(params.type),
        text: consoleText(params),
        url: consoleUrl(state, params)
      });
      return true;
    }

    function exceptionThrown(tabId, params = {}) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      touch(tabId, state);
      append(state, {
        entry: "console",
        level: "exception",
        text: exceptionText(params),
        url: consoleUrl(state, params)
      });
      return true;
    }

    function requestId(params) {
      return bounded(params?.requestId, 200);
    }

    function rememberRequest(state, id, value) {
      if (!id) return;
      state.requests.delete(id);
      state.requests.set(id, value);
      while (state.requests.size > maximumEntries) {
        state.requests.delete(state.requests.keys().next().value);
      }
    }

    function networkEntry(state, request, patch = {}) {
      return append(state, {
        entry: "network",
        requestId: request?.requestId ?? "",
        method: request?.method ?? "UNKNOWN",
        url: request?.url ?? "",
        resource_type: request?.resource_type ?? "other",
        status: request?.status ?? null,
        failure: request?.failure ?? null,
        ...patch
      });
    }

    function replaceNetwork(state, entry, patch) {
      if (entry) {
        const index = state.networkEntries.indexOf(entry);
        if (index >= 0) {
          state.networkEntries.splice(index, 1);
          state.retainedBytes -= entry.retainedBytes;
        }
        if (entry.requestId && state.requests.get(entry.requestId) === entry) {
          state.requests.delete(entry.requestId);
        }
      }
      return networkEntry(state, entry, patch);
    }

    function requestWillBeSent(tabId, params = {}) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      touch(tabId, state);
      const id = requestId(params);
      const existing = id ? state.requests.get(id) : null;
      if (existing && params.redirectResponse) {
        replaceNetwork(state, existing, {
          status: responseStatus(params.redirectResponse.status),
          url: sanitizeUrl(params.redirectResponse.url || existing.url)
        });
      }
      if (id) state.requests.delete(id);
      const pending = networkEntry(state, null, {
        requestId: id,
        method: methodName(params.request?.method),
        url: sanitizeUrl(params.request?.url),
        resource_type: resourceType(params.type),
        status: null,
        failure: null
      });
      if (state.networkEntries.includes(pending)) rememberRequest(state, id, pending);
      return true;
    }

    function responseReceived(tabId, params = {}) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      touch(tabId, state);
      const id = requestId(params);
      const request = id ? state.requests.get(id) : null;
      const completed = replaceNetwork(state, request, {
        requestId: id,
        method: request?.method ?? methodName(params.method),
        url: sanitizeUrl(params.response?.url || request?.url),
        resource_type: resourceType(params.type || request?.resource_type),
        status: responseStatus(params.response?.status),
        failure: null
      });
      if (state.networkEntries.includes(completed)) rememberRequest(state, id, completed);
      return true;
    }

    function loadingFailed(tabId, params = {}) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      touch(tabId, state);
      const id = requestId(params);
      const request = id ? state.requests.get(id) : null;
      replaceNetwork(state, request, {
        requestId: id,
        method: request?.method ?? methodName(params.method),
        url: request?.url ?? (params.url ? sanitizeUrl(params.url) : ""),
        resource_type: resourceType(params.type || request?.resource_type),
        status: request?.status ?? null,
        failure: failureCategory(params)
      });
      if (id) state.requests.delete(id);
      return true;
    }

    function orderedEntries(state) {
      return state.consoleEntries.concat(state.networkEntries)
        .sort((left, right) => left.sequence - right.sequence);
    }

    function isProblem(entry) {
      if (entry.entry === "console") {
        return entry.level === "warning" || entry.level === "error" || entry.level === "exception";
      }
      return entry.failure !== null || (entry.status !== null && entry.status >= 400);
    }

    function matchesLiteral(entry, matchText) {
      if (!matchText) return true;
      const searchable = entry.entry === "console"
        ? `${entry.level} ${entry.text}`
        : `${entry.method} ${entry.url} ${entry.resource_type} ${entry.status ?? ""} ${entry.failure ?? ""}`;
      return searchable.toLowerCase().includes(matchText);
    }

    function outputEntry(state, entry) {
      if (entry.entry === "console") {
        return {
          entry: "console",
          cursor: cursorFor(state, entry.sequence),
          timestamp_ms: entry.timestamp_ms,
          level: entry.level,
          text: entry.text,
          url: entry.url
        };
      }
      return {
        entry: "network",
        cursor: cursorFor(state, entry.sequence),
        timestamp_ms: entry.timestamp_ms,
        method: entry.method,
        url: entry.url,
        resource_type: entry.resource_type,
        status: entry.status,
        failure: entry.failure
      };
    }

    function read(tabId, {
      source = "both",
      detail = "problems",
      match_text: matchText,
      after,
      limit = DEFAULT_READ_LIMIT,
      allowNetworkUrl = () => true
    } = {}) {
      requireTabId(tabId);
      if (!["both", "console", "network"].includes(source)) {
        throw new RangeError("source must be both, console, or network");
      }
      if (!["problems", "all"].includes(detail)) {
        throw new RangeError("detail must be problems or all");
      }
      if (!Number.isInteger(limit) || limit < 1 || limit > MAX_READ_LIMIT) {
        throw new RangeError(`limit must be an integer from 1 to ${MAX_READ_LIMIT}`);
      }
      if (matchText !== undefined
        && (typeof matchText !== "string" || matchText.length < 1 || matchText.length > MAX_MATCH_CHARS)) {
        throw new RangeError(`match_text must contain 1 to ${MAX_MATCH_CHARS} characters`);
      }
      if (typeof allowNetworkUrl !== "function") {
        throw new TypeError("allowNetworkUrl must be a function");
      }

      const captureStarted = enable(tabId);
      const state = activeState(tabId);
      const afterSequence = after === undefined ? 0 : sequenceFromCursor(state, after);
      const entries = [];
      const literal = matchText?.toLowerCase();
      let omittedCount = 0;
      let scannedSequence = afterSequence;
      let truncated = false;

      for (const entry of orderedEntries(state)) {
        if (entry.sequence <= afterSequence) continue;
        const selectedSource = source === "both" || source === entry.entry;
        const selectedDetail = detail === "all" || isProblem(entry);
        if (!selectedSource || !selectedDetail) {
          scannedSequence = entry.sequence;
          continue;
        }
        if (entry.entry === "network") {
          let allowed = false;
          try {
            allowed = allowNetworkUrl(entry.url) === true;
          } catch (_error) {
            allowed = false;
          }
          if (!allowed) {
            omittedCount += 1;
            scannedSequence = entry.sequence;
            continue;
          }
        }
        if (!matchesLiteral(entry, literal)) {
          scannedSequence = entry.sequence;
          continue;
        }
        if (entries.length === limit) {
          truncated = true;
          break;
        }
        entries.push(outputEntry(state, entry));
        scannedSequence = entry.sequence;
      }

      const result = {
        entries,
        cursor: scannedSequence > 0 ? cursorFor(state, scannedSequence) : null,
        truncated,
        evicted: state.evictedThrough > 0
          && (after === undefined || afterSequence <= state.evictedThrough),
        capture_started: captureStarted || state.captureStartedPending,
        omitted_count: omittedCount
      };
      state.captureStartedPending = false;
      return result;
    }

    function clear(tabId) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      touch(tabId, state);
      if (state.sequence > 0) state.evictedThrough = state.sequence;
      state.consoleEntries.length = 0;
      state.networkEntries.length = 0;
      state.requests.clear();
      state.executionContexts.clear();
      state.retainedBytes = 0;
      return true;
    }

    function forget(tabId) {
      requireTabId(tabId);
      const state = activeState(tabId);
      if (!state) return false;
      if (state.timer !== null) clearTimer(state.timer);
      return tabs.delete(tabId);
    }

    function forgetMany(tabIds) {
      if (!Array.isArray(tabIds) || tabIds.length < 1 || tabIds.length > MAX_CLEAR_TABS) {
        throw new RangeError(`tab_ids must contain 1 to ${MAX_CLEAR_TABS} tab ids`);
      }
      const unique = new Set();
      for (const tabId of tabIds) {
        if (!Number.isSafeInteger(tabId) || tabId < 1) {
          throw new RangeError("tab_ids must contain only positive safe integers");
        }
        if (unique.has(tabId)) throw new RangeError("tab_ids must not contain duplicates");
        unique.add(tabId);
      }
      let clearedCount = 0;
      for (const tabId of tabIds) {
        if (forget(tabId)) clearedCount += 1;
      }
      return clearedCount;
    }

    function clearAll() {
      const tabIds = Array.from(tabs.keys());
      for (const state of tabs.values()) {
        if (state.timer !== null) clearTimer(state.timer);
      }
      tabs.clear();
      return tabIds;
    }

    function isEnabled(tabId) {
      requireTabId(tabId);
      return activeState(tabId) !== null;
    }

    return Object.freeze({
      enable,
      executionContextCreated,
      executionContextDestroyed,
      executionContextsCleared,
      consoleAPICalled,
      exceptionThrown,
      requestWillBeSent,
      responseReceived,
      loadingFailed,
      read,
      clear,
      forget,
      forgetMany,
      clearAll,
      isEnabled
    });
  }

  return Object.freeze({
    DEFAULT_MAX_ENTRIES,
    DEFAULT_MAX_BYTES,
    DEFAULT_IDLE_MS,
    DEFAULT_READ_LIMIT,
    MAX_READ_LIMIT,
    MAX_CONSOLE_CHARS,
    MAX_MATCH_CHARS,
    MAX_CURSOR_CHARS,
    MAX_CLEAR_TABS,
    DEFAULT_MAX_EXECUTION_CONTEXTS,
    create
  });
});
