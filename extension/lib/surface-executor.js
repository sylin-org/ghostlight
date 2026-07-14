// SPDX-License-Identifier: Apache-2.0 OR MIT
// Bounded, resource-scoped command execution for one extension worker generation (ADR-0080).
// Mechanism only: the service chooses each execution class and resource key.
(function initSurfaceExecutor(root) {
  "use strict";

  function createSurfaceExecutor(options) {
    const execute = options.execute;
    const onAccepted = options.onAccepted || (() => {});
    const onRejected = options.onRejected || (() => {});
    const onTerminal = options.onTerminal || (() => {});
    const setTimer = options.setTimer || setTimeout;
    const clearTimer = options.clearTimer || clearTimeout;
    const maxHeld = options.maxHeld || 128;
    const maxHeldBytes = options.maxHeldBytes || 32 * 1024 * 1024;
    const maxPerKey = options.maxPerKey || 32;
    const queueTtlMs = options.queueTtlMs || 120000;
    const dedupTtlMs = options.dedupTtlMs || 120000;
    const queues = new Map();
    const seen = new Map();
    let held = 0;
    let heldBytes = 0;

    function queueFor(key) {
      let queue = queues.get(key);
      if (!queue) {
        queue = { active: null, waiting: [] };
        queues.set(key, queue);
      }
      return queue;
    }

    function erasePayload(record) {
      if (record.item && record.item.request) record.item.request = null;
      record.item = null;
    }

    function release(record) {
      if (record.released) return;
      record.released = true;
      if (record.timer) clearTimer(record.timer);
      held = Math.max(0, held - 1);
      heldBytes = Math.max(0, heldBytes - record.bytes);
      erasePayload(record);
    }

    function rememberTerminal(commandId) {
      const current = seen.get(commandId);
      if (!current) return;
      current.state = "terminal";
      current.timer = setTimer(() => seen.delete(commandId), dedupTtlMs);
    }

    function rejectRecord(record, reason) {
      const item = record.item;
      release(record);
      rememberTerminal(record.commandId);
      onRejected(item, reason);
    }

    function finish(record) {
      const item = record.item;
      release(record);
      rememberTerminal(record.commandId);
      onTerminal(item);
    }

    function runRecord(record, queue) {
      record.started = true;
      if (record.timer) {
        clearTimer(record.timer);
        record.timer = null;
      }
      Promise.resolve()
        .then(() => execute(record.item))
        .catch(() => {})
        .then(() => {
          finish(record);
          if (queue) {
            queue.active = null;
            pump(record.key);
          }
        });
    }

    function pump(key) {
      const queue = queues.get(key);
      if (!queue || queue.active) return;
      const record = queue.waiting.shift();
      if (!record) {
        queues.delete(key);
        return;
      }
      queue.active = record;
      runRecord(record, queue);
    }

    function expire(record) {
      if (record.started || record.released) return;
      const queue = queues.get(record.key);
      if (!queue) return;
      const index = queue.waiting.indexOf(record);
      if (index < 0) return;
      queue.waiting.splice(index, 1);
      rejectRecord(record, "queue_expired");
      if (!queue.active && queue.waiting.length === 0) queues.delete(record.key);
    }

    function submit(item) {
      const valid = item && typeof item.commandId === "string" && item.commandId.length > 0 &&
        typeof item.key === "string" && item.key.length > 0 &&
        Number.isSafeInteger(item.bytes) && item.bytes >= 0;
      if (!valid) {
        onRejected(item, "invalid_execution_metadata");
        return false;
      }

      const prior = seen.get(item.commandId);
      if (prior) {
        onAccepted(item, true);
        if (prior.state === "terminal") onTerminal(item);
        return true;
      }

      const bypass = item.bypass === true;
      const queue = bypass ? null : queueFor(item.key);
      const perKeyHeld = queue ? queue.waiting.length + (queue.active ? 1 : 0) : 0;
      if (held >= maxHeld || heldBytes + item.bytes > maxHeldBytes || perKeyHeld >= maxPerKey) {
        if (queue && !queue.active && queue.waiting.length === 0) queues.delete(item.key);
        onRejected(item, "queue_overloaded");
        return false;
      }

      const record = {
        item,
        commandId: item.commandId,
        key: item.key,
        bytes: item.bytes,
        timer: null,
        started: false,
        released: false,
      };
      held += 1;
      heldBytes += item.bytes;
      seen.set(item.commandId, { state: "active", timer: null });
      onAccepted(item, false);

      if (bypass) {
        runRecord(record, null);
      } else {
        queue.waiting.push(record);
        record.timer = setTimer(() => expire(record), queueTtlMs);
        pump(item.key);
      }
      return true;
    }

    function rejectWaiting(key, reason) {
      const queue = queues.get(key);
      if (!queue) return;
      const waiting = queue.waiting.splice(0);
      for (const record of waiting) rejectRecord(record, reason);
      if (!queue.active) queues.delete(key);
    }

    function destroyKey(key) {
      rejectWaiting(key, "resource_destroyed");
    }

    function clear() {
      for (const key of Array.from(queues.keys())) rejectWaiting(key, "executor_disconnected");
    }

    return {
      submit,
      destroyKey,
      clear,
      stats: () => ({ held, bytes: heldBytes, resources: queues.size }),
    };
  }

  const GhostlightSurfaceExecutor = { createSurfaceExecutor };
  if (typeof module !== "undefined" && module.exports) {
    module.exports = GhostlightSurfaceExecutor;
  } else {
    root.GhostlightSurfaceExecutor = GhostlightSurfaceExecutor;
  }
})(typeof self !== "undefined" ? self : globalThis);
