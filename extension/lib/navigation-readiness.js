// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- bounded, policy-free navigation commit and readiness state.
//
// The service owns navigation meaning, landing authorization, and canonical result semantics.
// This module owns only browser-mechanism correlation: one active token per tab, a bounded journal
// of top-level document commits, one dispatch-to-readiness deadline, and fail-closed cleanup.
(function initNavigationReadiness(root) {
"use strict";

const DEFAULT_TIMEOUT_MS = 10000;
const MAX_TIMEOUT_MS = 30000;
const DEFAULT_MIN_MS = 0;
const DEFAULT_SETTLE = true;
const DEFAULT_MAX_COMMITS = 16;
const DEFAULT_RETENTION_MS = 30000;
const MAX_OPAQUE_HANDLE_LENGTH = 128;
const MAX_URL_LENGTH = 4096;

function boundedUrl(value) {
  return typeof value === "string" && value.length > 0 &&
    !/[\u0000-\u001f\u007f-\u009f]/.test(value) &&
    new TextEncoder().encode(value).byteLength <= MAX_URL_LENGTH;
}

function defaultNow() {
  if (typeof performance !== "undefined" && typeof performance.now === "function") {
    return performance.now();
  }
  return Date.now();
}

function randomOpaque(prefix) {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}_${crypto.randomUUID()}`;
  }
  return `${prefix}_${Date.now()}_${Math.random().toString(16).slice(2)}`;
}

function boundedInteger(value) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(Number.MAX_SAFE_INTEGER, Math.floor(value)));
}

function validateOpaque(value, label) {
  if (typeof value !== "string" || !value || value.length > MAX_OPAQUE_HANDLE_LENGTH ||
      !/^[\x21-\x7e]+$/.test(value)) {
    throw new Error(`${label} must be a bounded opaque ASCII string`);
  }
  return value;
}

function normalizeReadiness(input) {
  const readiness = input === undefined ? {} : input;
  if (!readiness || typeof readiness !== "object" || Array.isArray(readiness)) {
    throw new Error("readiness must be an object");
  }
  const settle = readiness.settle === undefined ? DEFAULT_SETTLE : readiness.settle;
  const timeoutMs = readiness.timeout_ms === undefined
    ? DEFAULT_TIMEOUT_MS
    : readiness.timeout_ms;
  const minMs = readiness.min_ms === undefined ? DEFAULT_MIN_MS : readiness.min_ms;
  if (typeof settle !== "boolean") throw new Error("readiness.settle must be a boolean");
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > MAX_TIMEOUT_MS) {
    throw new Error(`readiness.timeout_ms must be an integer from 1 to ${MAX_TIMEOUT_MS}`);
  }
  if (!Number.isSafeInteger(minMs) || minMs < 0 || minMs > timeoutMs) {
    throw new Error("readiness.min_ms must be an integer no greater than timeout_ms");
  }
  return Object.freeze({ settle, timeout_ms: timeoutMs, min_ms: minMs });
}

function normalizeProviderFrame(frame) {
  if (!frame || typeof frame !== "object" || Array.isArray(frame)) return null;
  if (typeof frame.id !== "string" || !frame.id) return null;
  if (!boundedUrl(frame.url)) return null;
  const loaderId = typeof frame.loaderId === "string" ? frame.loaderId : "";
  const securityOrigin = typeof frame.securityOrigin === "string" ? frame.securityOrigin : "";
  const mimeType = typeof frame.mimeType === "string" ? frame.mimeType : "";
  return {
    frame_id: frame.id,
    loader_id: loaderId,
    url: frame.url,
    security_origin: securityOrigin,
    mime_type: mimeType,
    snapshot_key: `${frame.id}\n${loaderId}\n${frame.url}`,
  };
}

function createNavigationReadiness(options = {}) {
  const now = typeof options.now === "function" ? options.now : defaultNow;
  const makeToken = typeof options.makeToken === "function"
    ? options.makeToken
    : () => randomOpaque("n");
  const makeDocument = typeof options.makeDocument === "function"
    ? options.makeDocument
    : () => randomOpaque("d");
  const setTimer = typeof options.setTimer === "function" ? options.setTimer : setTimeout;
  const clearTimer = typeof options.clearTimer === "function" ? options.clearTimer : clearTimeout;
  const maxCommits = options.maxCommits === undefined
    ? DEFAULT_MAX_COMMITS
    : options.maxCommits;
  const retentionMs = options.retentionMs === undefined
    ? DEFAULT_RETENTION_MS
    : options.retentionMs;
  if (!Number.isSafeInteger(maxCommits) || maxCommits < 1) {
    throw new Error("maxCommits must be a positive integer");
  }
  if (!Number.isSafeInteger(retentionMs) || retentionMs < 0) {
    throw new Error("retentionMs must be a non-negative integer");
  }

  const byTab = new Map();
  const byToken = new Map();

  function notify(state) {
    const waiters = Array.from(state.waiters);
    state.waiters.clear();
    for (const waiter of waiters) waiter();
  }

  function remove(state, reason) {
    if (state.removed) return;
    state.removed = true;
    state.fatal_reason = state.fatal_reason || reason || "retired";
    if (state.retention_timer !== null) clearTimer(state.retention_timer);
    state.retention_timer = null;
    if (byTab.get(state.tab_id) === state) byTab.delete(state.tab_id);
    if (byToken.get(state.navigation_token) === state) {
      byToken.delete(state.navigation_token);
    }
    notify(state);
  }

  function requireState(navigationToken, tabId) {
    const token = validateOpaque(navigationToken, "navigation_token");
    const state = byToken.get(token);
    if (!state || state.removed) throw new Error("navigation token is unavailable");
    if (tabId !== undefined && state.tab_id !== tabId) {
      throw new Error("navigation token does not belong to the requested tab");
    }
    return state;
  }

  function elapsed(state) {
    if (state.dispatched_at_ms === null) return 0;
    return boundedInteger(now() - state.dispatched_at_ms);
  }

  function evidence(state, resultState, commit, elapsedOverride) {
    const result = {
      state: resultState,
      navigation_token: state.navigation_token,
      deadline_at_ms: boundedInteger(state.deadline_at_ms),
      elapsed_ms: elapsedOverride === undefined
        ? elapsed(state)
        : boundedInteger(elapsedOverride),
    };
    const selected = commit || state.delivered;
    if (selected) {
      result.document_handle = selected.document_handle;
      result.url = selected.url;
    }
    return result;
  }

  function landingUnknownEvidence(state) {
    const result = evidence(state, "landing_unknown", null);
    delete result.document_handle;
    delete result.url;
    return result;
  }

  function remaining(state) {
    if (state.deadline_at_ms === null) return 0;
    return Math.max(0, state.deadline_at_ms - now());
  }

  function deadlineElapsed(state) {
    return Math.max(state.readiness.timeout_ms, elapsed(state));
  }

  function subscribe(state) {
    let active = true;
    let resolvePromise;
    const resolve = () => {
      if (!active) return;
      active = false;
      state.waiters.delete(resolve);
      resolvePromise("signal");
    };
    const promise = new Promise((resolveOuter) => { resolvePromise = resolveOuter; });
    state.waiters.add(resolve);
    return {
      promise,
      cancel() {
        if (!active) return;
        active = false;
        state.waiters.delete(resolve);
      },
    };
  }

  function deadlineSignal(ms) {
    let timer = null;
    const promise = new Promise((resolve) => {
      timer = setTimer(() => resolve("deadline"), Math.max(0, Math.ceil(ms)));
    });
    return {
      promise,
      cancel() {
        if (timer !== null) clearTimer(timer);
        timer = null;
      },
    };
  }

  function takeCommit(state) {
    if (state.pending.length === 0) return null;
    const commit = state.pending.shift();
    state.delivered = commit;
    state.cached_readiness = null;
    return evidence(state, "committed", commit);
  }

  function recordCommit(state, provider) {
    if (!provider || state.removed || state.dispatched_at_ms === null || state.fatal_reason) {
      return false;
    }
    if (!state.timely_commit_seen && now() > state.deadline_at_ms) {
      state.commit_deadline_expired = true;
      notify(state);
      return false;
    }
    if (now() > state.deadline_at_ms) state.commit_deadline_expired = true;
    state.commit_count += 1;
    if (state.commit_count > maxCommits) {
      state.fatal_reason = "commit_journal_overflow";
      notify(state);
      return false;
    }
    let documentHandle = validateOpaque(makeDocument(), "generated document_handle");
    if (state.document_handles.has(documentHandle)) {
      state.fatal_reason = "duplicate_document_handle";
      notify(state);
      return false;
    }
    state.document_handles.add(documentHandle);
    state.timely_commit_seen = true;
    state.top_frame_id = provider.frame_id;
    state.last_provider = provider;
    state.pending.push({
      document_handle: documentHandle,
      url: provider.url,
      provider,
    });
    notify(state);
    return true;
  }

  function arm(tabId, readinessInput, initialFrame) {
    if (!Number.isSafeInteger(tabId)) throw new Error("navigation tab must be an integer");
    const readiness = normalizeReadiness(readinessInput);
    const prior = byTab.get(tabId);
    if (prior) remove(prior, "superseded");
    const navigationToken = validateOpaque(makeToken(), "generated navigation_token");
    if (byToken.has(navigationToken)) throw new Error("generated navigation token collided");
    const provider = normalizeProviderFrame(initialFrame);
    const state = {
      tab_id: tabId,
      navigation_token: navigationToken,
      readiness,
      top_frame_id: provider && provider.frame_id,
      last_provider: provider,
      dispatched_at_ms: null,
      deadline_at_ms: null,
      commit_count: 0,
      timely_commit_seen: false,
      commit_deadline_expired: false,
      document_handles: new Set(),
      pending: [],
      delivered: null,
      cached_readiness: null,
      watcher_unavailable: false,
      fatal_reason: null,
      waiters: new Set(),
      retention_timer: null,
      removed: false,
    };
    byTab.set(tabId, state);
    byToken.set(navigationToken, state);
    return Object.freeze({ navigation_token: navigationToken });
  }

  function markDispatched(navigationToken, dispatchedAtOverride) {
    const state = requireState(navigationToken);
    if (state.dispatched_at_ms !== null) throw new Error("navigation token was already dispatched");
    const observedNow = now();
    const dispatchedAt = dispatchedAtOverride === undefined
      ? observedNow
      : dispatchedAtOverride;
    if (!Number.isFinite(dispatchedAt) || dispatchedAt < 0 || dispatchedAt > observedNow) {
      remove(state, "invalid_clock");
      throw new Error("navigation dispatch time must be a non-negative finite time not in the future");
    }
    state.dispatched_at_ms = dispatchedAt;
    state.deadline_at_ms = dispatchedAt + state.readiness.timeout_ms;
    state.retention_timer = setTimer(() => {
      remove(state, "retention_expired");
    }, Math.max(0, state.deadline_at_ms - observedNow) + retentionMs);
    return Object.freeze({
      navigation_token: state.navigation_token,
      deadline_at_ms: boundedInteger(state.deadline_at_ms),
    });
  }

  function frameNavigated(tabId, frame) {
    const state = byTab.get(tabId);
    if (!state || frame && frame.parentId) return false;
    const provider = normalizeProviderFrame(frame);
    if (!provider) {
      invalidateCommittedFrame(state);
      return false;
    }
    return recordCommit(state, provider);
  }

  function invalidateCommittedFrame(state) {
    state.fatal_reason = "invalid_committed_frame";
    notify(state);
  }

  function navigatedWithinDocument(tabId, event) {
    const state = byTab.get(tabId);
    if (!state || !event || event.frameId !== state.top_frame_id) {
      return false;
    }
    if (!boundedUrl(event.url)) {
      invalidateCommittedFrame(state);
      return false;
    }
    const prior = state.last_provider;
    const provider = normalizeProviderFrame({
      id: event.frameId,
      loaderId: prior && prior.loader_id,
      url: event.url,
      securityOrigin: prior && prior.security_origin,
      mimeType: prior && prior.mime_type,
    });
    return recordCommit(state, provider);
  }

  async function waitForCommit(navigationToken) {
    const state = requireState(navigationToken);
    if (state.dispatched_at_ms === null) throw new Error("navigation was not dispatched");
    for (;;) {
      if (state.fatal_reason) {
        const result = landingUnknownEvidence(state);
        remove(state, state.fatal_reason);
        return result;
      }
      const committed = takeCommit(state);
      if (committed) return committed;
      if (state.commit_deadline_expired) {
        const result = evidence(state, "timed_out", null, deadlineElapsed(state));
        remove(state, "commit_after_deadline");
        return result;
      }
      if (state.watcher_unavailable) {
        const result = evidence(state, "unavailable", null);
        remove(state, "watcher_unavailable_before_commit");
        return result;
      }
      const ms = remaining(state);
      if (ms <= 0) {
        const result = evidence(state, "timed_out", null, deadlineElapsed(state));
        remove(state, "commit_deadline");
        return result;
      }
      const subscription = subscribe(state);
      const deadline = deadlineSignal(ms);
      await Promise.race([subscription.promise, deadline.promise]);
      subscription.cancel();
      deadline.cancel();
    }
  }

  function requireDelivered(state, documentHandle) {
    const handle = validateOpaque(documentHandle, "document_handle");
    if (!state.delivered || state.delivered.document_handle !== handle) {
      throw new Error("document_handle is not the current delivered navigation document");
    }
    return state.delivered;
  }

  function cacheReadiness(state, result) {
    state.cached_readiness = Object.freeze({ ...result });
    return { ...state.cached_readiness };
  }

  async function awaitReadiness(input, observe) {
    if (!input || typeof input !== "object" || Array.isArray(input)) {
      throw new Error("navigation readiness input must be an object");
    }
    if (!Number.isSafeInteger(input.tab)) throw new Error("navigation tab must be an integer");
    const state = requireState(input.navigation_token, input.tab);
    requireDelivered(state, input.document_handle);
    if (state.fatal_reason) return cacheReadiness(state, landingUnknownEvidence(state));
    const next = takeCommit(state);
    if (next) return next;
    if (state.watcher_unavailable) {
      return cacheReadiness(state, evidence(state, "unavailable"));
    }
    if (state.cached_readiness) return { ...state.cached_readiness };
    if (!state.readiness.settle) {
      return cacheReadiness(state, evidence(state, "not_requested"));
    }
    const ms = remaining(state);
    if (ms <= 0) {
      return cacheReadiness(
        state,
        evidence(state, "timed_out", null, deadlineElapsed(state))
      );
    }
    if (typeof observe !== "function") throw new Error("readiness observer is required");
    const minMs = Math.max(0, state.readiness.min_ms - elapsed(state));
    const subscription = subscribe(state);
    const deadline = deadlineSignal(ms);
    const observation = Promise.resolve()
      .then(() => observe({
        settle: true,
        timeout_ms: Math.max(0, Math.floor(ms)),
        min_ms: Math.max(0, Math.floor(minMs)),
      }))
      .then(
        (value) => ({ kind: "observation", value }),
        () => ({ kind: "observation_unavailable" })
      );
    const winner = await Promise.race([
      observation,
      subscription.promise.then(() => ({ kind: "signal" })),
      deadline.promise.then(() => ({ kind: "deadline" })),
    ]);
    subscription.cancel();
    deadline.cancel();

    if (state.fatal_reason) return cacheReadiness(state, landingUnknownEvidence(state));
    const changed = takeCommit(state);
    if (changed) return changed;
    if (state.watcher_unavailable) {
      return cacheReadiness(state, evidence(state, "unavailable"));
    }
    if (winner.kind === "deadline" || remaining(state) <= 0) {
      return cacheReadiness(
        state,
        evidence(state, "timed_out", null, deadlineElapsed(state))
      );
    }
    if (winner.kind === "observation_unavailable") {
      return cacheReadiness(state, evidence(state, "unavailable"));
    }
    if (winner.kind === "observation" && winner.value && winner.value.timeout === true) {
      return cacheReadiness(
        state,
        evidence(state, "timed_out", null, deadlineElapsed(state))
      );
    }
    if (winner.kind === "observation" && winner.value && winner.value.settled === true) {
      return cacheReadiness(state, evidence(state, "ready"));
    }
    return cacheReadiness(state, evidence(state, "unavailable"));
  }

  function verify(input, current) {
    if (!input || typeof input !== "object" || Array.isArray(input)) {
      throw new Error("navigation verification input must be an object");
    }
    if (!Number.isSafeInteger(input.tab)) throw new Error("navigation tab must be an integer");
    const state = requireState(input.navigation_token, input.tab);
    requireDelivered(state, input.document_handle);
    if (state.fatal_reason) {
      const result = landingUnknownEvidence(state);
      remove(state, state.fatal_reason);
      return result;
    }
    const queued = takeCommit(state);
    if (queued) return queued;
    if (state.watcher_unavailable) {
      const result = evidence(state, "unavailable");
      remove(state, "watcher_unavailable");
      return result;
    }
    const provider = current && normalizeProviderFrame(current.frame);
    const targetUrl = current && current.target_url;
    if (!provider || typeof targetUrl !== "string" || !targetUrl || targetUrl !== provider.url) {
      const result = evidence(state, "unavailable");
      remove(state, "verification_unavailable");
      return result;
    }
    if (!state.delivered || provider.snapshot_key !== state.delivered.provider.snapshot_key) {
      if (!recordCommit(state, provider) || state.fatal_reason) {
        const result = landingUnknownEvidence(state);
        remove(state, state.fatal_reason || "verification_commit_failed");
        return result;
      }
      return takeCommit(state);
    }
    const result = evidence(state, "same");
    remove(state, "verified");
    return result;
  }

  function watcherUnavailable(tabId) {
    const state = byTab.get(tabId);
    if (!state || state.removed) return false;
    state.watcher_unavailable = true;
    notify(state);
    return true;
  }

  function destroyTab(tabId) {
    const state = byTab.get(tabId);
    if (!state) return false;
    state.fatal_reason = "surface_destroyed";
    remove(state, state.fatal_reason);
    return true;
  }

  function abandon(navigationToken, reason) {
    const state = requireState(navigationToken);
    remove(state, reason || "abandoned");
  }

  function clear() {
    for (const state of Array.from(byToken.values())) remove(state, "cleared");
  }

  return Object.freeze({
    abandon,
    activeCount: () => byToken.size,
    arm,
    awaitReadiness,
    clear,
    destroyTab,
    frameNavigated,
    markDispatched,
    navigatedWithinDocument,
    verify,
    waitForCommit,
    watcherUnavailable,
  });
}

function attachNavigation(result, navigation) {
  if (!result || typeof result !== "object" || Array.isArray(result)) {
    throw new Error("navigation result must be an object");
  }
  if (!navigation || typeof navigation !== "object" || Array.isArray(navigation)) {
    throw new Error("navigation evidence must be an object");
  }
  const structured = result.structuredContent === undefined
    ? {}
    : result.structuredContent;
  if (!structured || typeof structured !== "object" || Array.isArray(structured)) {
    throw new Error("navigation structuredContent must be an object");
  }
  result.structuredContent = { ...structured, navigation: { ...navigation } };
  return result;
}

const GhostlightNavigationReadiness = Object.freeze({
  attachNavigation,
  createNavigationReadiness,
  normalizeReadiness,
});
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightNavigationReadiness;
} else {
  root.GhostlightNavigationReadiness = GhostlightNavigationReadiness;
}
})(typeof self !== "undefined" ? self : globalThis);
