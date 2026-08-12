// Ghostlight -- plural, bounded, volatile browser recordings owned by the extension.
(function installGhostlightRecording(root, factory) {
  const api = factory();
  root.GhostlightRecording = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightRecordingApi() {
  "use strict";

  const HARD_DURATION_MS = 120_000;
  const RETENTION_MS = 5 * 60_000;
  const MIN_FRAME_INTERVAL_MS = 100;
  const JPEG_QUALITY = 80;
  const MAX_WIDTH = 1280;
  const MAX_HEIGHT = 720;
  const MAX_FRAME_BYTES = 2 * 1024 * 1024;
  const MAX_RECORDING_BYTES = 5 * 1024 * 1024;
  const MAX_GLOBAL_BYTES = 16 * 1024 * 1024;
  const MAX_FRAMES = 100;
  const MAX_RECORDINGS = 16;
  const MAX_SOURCE_URLS = 32;
  const MAX_FRAME_BASE64_CHARS = 4 * Math.ceil(MAX_FRAME_BYTES / 3);

  function requireWorkspace(value) {
    if (typeof value !== "string" || value.length < 1 || value.length > 160) {
      throw new TypeError("recording requires a bounded opaque workspace");
    }
    return value;
  }

  function requireTabId(value) {
    if (!Number.isSafeInteger(value) || value <= 0) throw new TypeError("recording requires a tab id");
    return value;
  }

  function decodedBytes(value) {
    if (typeof value !== "string"
      || value.length === 0
      || value.length > MAX_FRAME_BASE64_CHARS
      || value.length % 4 !== 0
      || !/^[A-Za-z0-9+/]*={0,2}$/.test(value)) return 0;
    const padding = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
    return Math.floor(value.length * 3 / 4) - padding;
  }

  function sanitizeSourceUrl(value) {
    try {
      const parsed = new URL(String(value));
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return null;
      parsed.username = "";
      parsed.password = "";
      parsed.search = "";
      parsed.hash = "";
      return parsed.toString().slice(0, 2048);
    } catch (_error) {
      return null;
    }
  }

  function create({
    now = Date.now,
    setTimer = setTimeout,
    clearTimer = clearTimeout,
    newId = () => `recording_${crypto.randomUUID().replaceAll("-", "")}`,
    onStop = () => {}
  } = {}) {
    const recordings = new Map();
    const activeByTab = new Map();
    let globalBytes = 0;

    function currentTime() {
      const value = Number(now());
      return Number.isFinite(value) && value >= 0 ? Math.floor(value) : 0;
    }

    function summary(state, current = currentTime()) {
      return {
        recording_id: state.id,
        tab_id: state.tabId,
        state: state.state,
        frame_count: state.frames.length,
        bytes_held: state.bytesHeld,
        duration_ms: Math.max(0, (state.stoppedAt ?? current) - state.startedAt),
        hard_expires_unix_ms: state.state === "recording" ? state.hardExpiresAt : undefined,
        retention_expires_unix_ms: state.state === "recording" ? undefined : state.retentionExpiresAt,
        stop_reason: state.stopReason ?? undefined,
        source_urls: Array.from(state.sourceUrls)
      };
    }

    function disarm(state) {
      if (state.timer !== null) clearTimer(state.timer);
      state.timer = null;
    }

    function erase(state) {
      if (recordings.get(state.id) !== state) return false;
      disarm(state);
      recordings.delete(state.id);
      if (activeByTab.get(state.tabId) === state.id) activeByTab.delete(state.tabId);
      globalBytes = Math.max(0, globalBytes - state.bytesHeld);
      state.frames.length = 0;
      state.bytesHeld = 0;
      state.sourceUrls.clear();
      return true;
    }

    function freeze(state, reason, current = currentTime()) {
      if (state.state !== "recording") return summary(state, current);
      activeByTab.delete(state.tabId);
      state.state = reason === "explicit" ? "frozen" : "interrupted";
      state.stopReason = reason;
      state.stoppedAt = current;
      state.retentionExpiresAt = current + RETENTION_MS;
      arm(state);
      return summary(state, current);
    }

    function expireIfDue(state, current = currentTime()) {
      if (recordings.get(state.id) !== state) return true;
      if (state.state === "recording" && current >= state.hardExpiresAt) {
        try { onStop(state.tabId, state.id, "hard_timeout"); } catch (_error) {}
        freeze(state, "hard_timeout", current);
        return false;
      }
      if (state.state !== "recording" && current >= state.retentionExpiresAt) {
        erase(state);
        return true;
      }
      return false;
    }

    function arm(state) {
      disarm(state);
      const deadline = state.state === "recording" ? state.hardExpiresAt : state.retentionExpiresAt;
      state.timer = setTimer(() => {
        state.timer = null;
        if (!expireIfDue(state)) arm(state);
      }, Math.max(0, deadline - currentTime()));
      state.timer?.unref?.();
    }

    function owned(workspace) {
      requireWorkspace(workspace);
      const current = currentTime();
      const values = [];
      for (const state of Array.from(recordings.values())) {
        if (!expireIfDue(state, current) && state.workspace === workspace) values.push(state);
      }
      values.sort((left, right) => left.id.localeCompare(right.id));
      return values;
    }

    function select(workspace, requested) {
      const eligible = owned(workspace);
      if (requested !== undefined && requested !== null) {
        const state = recordings.get(requested);
        return state && state.workspace === workspace && !expireIfDue(state) ? { state } : { notFound: true };
      }
      if (eligible.length === 0) return { notFound: true };
      if (eligible.length > 1) return { ambiguous: eligible.map((state) => state.id) };
      return { state: eligible[0] };
    }

    function start(workspace, tabId, sourceUrl) {
      workspace = requireWorkspace(workspace);
      tabId = requireTabId(tabId);
      const current = currentTime();
      const activeId = activeByTab.get(tabId);
      const active = activeId ? recordings.get(activeId) : null;
      if (active && !expireIfDue(active, current)) {
        if (active.workspace === workspace) return { existing: summary(active, current) };
        throw Object.assign(new Error("tab already has an active recording"), { code: "recording_active" });
      }
      for (const state of Array.from(recordings.values())) expireIfDue(state, current);
      if (recordings.size >= MAX_RECORDINGS) throw Object.assign(new Error("recording count bound reached"), { code: "recording_memory_limit" });
      const id = newId();
      if (typeof id !== "string" || !/^recording_[a-zA-Z0-9_]+$/.test(id) || id.length > 160 || recordings.has(id)) {
        throw new Error("recording id generator returned an invalid identity");
      }
      const state = {
        id, workspace, tabId, state: "recording", frames: [], bytesHeld: 0,
        startedAt: current, stoppedAt: null, hardExpiresAt: current + HARD_DURATION_MS,
        retentionExpiresAt: null, stopReason: null, lastFrameAt: 0, finalizing: false,
        sourceUrls: new Set(), timer: null
      };
      const url = sanitizeSourceUrl(sourceUrl);
      if (url) state.sourceUrls.add(url);
      recordings.set(id, state);
      activeByTab.set(tabId, id);
      arm(state);
      return { started: summary(state, current) };
    }

    function activeForTab(tabId) {
      const id = activeByTab.get(tabId);
      const state = id ? recordings.get(id) : null;
      return state && !expireIfDue(state) ? state : null;
    }

    function append(tabId, data, frameKind, timestampMs = currentTime()) {
      if (!["seed", "screencast", "final"].includes(frameKind)) {
        throw new TypeError("recording frame kind is invalid");
      }
      timestampMs = Number.isFinite(timestampMs) && timestampMs >= 0
        ? Math.floor(timestampMs)
        : currentTime();
      const state = activeForTab(tabId);
      if (!state || (state.finalizing && frameKind === "screencast")) return false;
      if (frameKind === "screencast" && timestampMs - state.lastFrameAt < MIN_FRAME_INTERVAL_MS) return false;
      const size = decodedBytes(data);
      if (size < 1 || size > MAX_FRAME_BYTES) {
        try { onStop(tabId, state.id, "frame_too_large"); } catch (_error) {}
        freeze(state, "frame_too_large", timestampMs);
        return false;
      }
      const fits = state.frames.length < MAX_FRAMES
        && state.bytesHeld + size <= MAX_RECORDING_BYTES
        && globalBytes + size <= MAX_GLOBAL_BYTES;
      if (!fits) {
        try { onStop(tabId, state.id, "memory_limit"); } catch (_error) {}
        freeze(state, "memory_limit", timestampMs);
        return false;
      }
      state.frames.push({ frame_kind: frameKind, timestamp_ms: timestampMs, mime_type: "image/jpeg", data });
      state.bytesHeld += size;
      globalBytes += size;
      state.lastFrameAt = timestampMs;
      return true;
    }

    function noteUrl(tabId, value) {
      const state = activeForTab(tabId);
      const url = sanitizeSourceUrl(value);
      if (!state || !url || state.sourceUrls.has(url)) return;
      if (state.sourceUrls.size === MAX_SOURCE_URLS) state.sourceUrls.delete(state.sourceUrls.values().next().value);
      state.sourceUrls.add(url);
    }

    function status(workspace, requested) {
      const selected = select(workspace, requested);
      if (selected.state) return { summary: summary(selected.state) };
      return selected;
    }

    function beginStop(workspace, requested) {
      const selected = select(workspace, requested);
      if (!selected.state) return selected;
      selected.state.finalizing = true;
      return { state: selected.state, summary: summary(selected.state) };
    }

    function finishStop(state, reason = "explicit") {
      return freeze(state, reason);
    }

    function interruptTab(tabId, reason) {
      const state = activeForTab(tabId);
      return state ? freeze(state, reason) : null;
    }

    function interruptAll(reason) {
      const summaries = [];
      for (const tabId of Array.from(activeByTab.keys())) {
        const stopped = interruptTab(tabId, reason);
        if (stopped) summaries.push(stopped);
      }
      return summaries;
    }

    function read(workspace, requested) {
      const selected = select(workspace, requested);
      if (!selected.state) return selected;
      return { summary: summary(selected.state), frames: selected.state.frames.map((frame) => ({ ...frame })) };
    }

    function discard(workspace, requested) {
      const selected = select(workspace, requested);
      if (!selected.state) return selected;
      const recordingId = selected.state.id;
      const releasedBytes = selected.state.bytesHeld;
      const active = selected.state.state === "recording";
      const tabId = selected.state.tabId;
      erase(selected.state);
      return { recordingId, releasedBytes, active, tabId };
    }

    function count() {
      for (const state of Array.from(recordings.values())) expireIfDue(state);
      return activeByTab.size;
    }

    return Object.freeze({
      start, activeForTab, append, noteUrl, status, beginStop, finishStop, interruptTab,
      interruptAll, read, discard, count
    });
  }

  return Object.freeze({
    HARD_DURATION_MS, RETENTION_MS, MAX_FRAME_BYTES, MAX_RECORDING_BYTES,
    MAX_GLOBAL_BYTES, MAX_FRAMES, MAX_RECORDINGS, MAX_FRAME_BASE64_CHARS,
    JPEG_QUALITY, MAX_WIDTH, MAX_HEIGHT, create
  });
});
