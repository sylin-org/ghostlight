// Agnostic load-sensitive settle sensor for browser DOM operations (ADR-0171).
(function installSensor(root, factory) {
  const api = factory();
  root.GhostlightSensor = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(globalThis, function sensorApi() {
  "use strict";

  const DEFAULT_MAX_WAIT_MS = 1200;
  const DEFAULT_QUIET_MS = 50;
  const DEFAULT_POLL_INTERVAL_MS = 50;

  /**
   * Bounded settlement sensor for load-sensitive DOM operations.
   *
   * @param {() => any} sample Synchronous DOM extraction function.
   * @param {(candidate: any) => boolean} isSatisfied Truth test for a non-empty/settled result.
   * @param {object} [options] Optional overrides and dependencies.
   * @returns {Promise<any>} The settled or final candidate.
   */
  async function settle(sample, isSatisfied, options = {}) {
    const initial = sample();
    if (isSatisfied(initial)) return initial;

    const maxWaitMs = options.maxWaitMs ?? DEFAULT_MAX_WAIT_MS;
    const quietMs = options.quietMs ?? DEFAULT_QUIET_MS;
    const doc = options.document ?? (typeof document !== "undefined" ? document : null);
    const setTimeoutFn = options.setTimeout ?? setTimeout;
    const clearTimeoutFn = options.clearTimeout ?? clearTimeout;
    const Observer = options.MutationObserver ?? (typeof MutationObserver !== "undefined" ? MutationObserver : null);

    if (!doc) return initial;

    return new Promise((resolve, reject) => {
      let settledTimer = null;
      let ceilingTimer = null;
      let pollTimer = null;
      let observer = null;
      let latest = initial;
      let finished = false;

      function cleanup() {
        if (finished) return;
        finished = true;
        if (settledTimer) clearTimeoutFn(settledTimer);
        if (ceilingTimer) clearTimeoutFn(ceilingTimer);
        if (pollTimer) clearTimeoutFn(pollTimer);
        if (observer) {
          try {
            observer.disconnect();
          } catch (_error) {
            // Ignore disconnect errors during teardown.
          }
          observer = null;
        }
      }

      function evaluateAndSchedule() {
        if (finished) return;
        try {
          latest = sample();
        } catch (error) {
          cleanup();
          reject(error);
          return;
        }

        if (isSatisfied(latest)) {
          if (settledTimer) clearTimeoutFn(settledTimer);
          settledTimer = setTimeoutFn(() => {
            cleanup();
            resolve(latest);
          }, quietMs);
        }
      }

      // Hard timeout ceiling: always resolve with the latest sample even if unsatisfied.
      ceilingTimer = setTimeoutFn(() => {
        try {
          latest = sample();
        } catch (_error) {
          // Keep prior latest on sample error during ceiling timeout.
        }
        cleanup();
        resolve(latest);
      }, maxWaitMs);

      const targetRoot = doc.documentElement || doc.body || doc;
      if (Observer && targetRoot && typeof targetRoot.nodeType === "number") {
        try {
          observer = new Observer(() => {
            evaluateAndSchedule();
          });
          observer.observe(targetRoot, {
            childList: true,
            subtree: true,
            characterData: true
          });
        } catch (_error) {
          // If observer registration fails, fall back to interval polling.
          observer = null;
        }
      }

      // If MutationObserver is not available or if mutations are missed,
      // an interval poll guarantees progress until maxWaitMs.
      function poll() {
        if (finished) return;
        evaluateAndSchedule();
        if (!finished) {
          pollTimer = setTimeoutFn(poll, DEFAULT_POLL_INTERVAL_MS);
        }
      }

      if (!observer) {
        pollTimer = setTimeoutFn(poll, DEFAULT_POLL_INTERVAL_MS);
      }
    });
  }

  const DEFAULT_VISUAL_MAX_WAIT_MS = 1500;
  const DEFAULT_VISUAL_QUIET_MS = 60;

  /**
   * Bounded settlement sensor for layout stability and visual quiescence (ADR-0173).
   *
   * @param {Element|Document} [target] Element or document to observe.
   * @param {object} [options] Optional overrides and dependencies.
   * @returns {Promise<{ settled: boolean, elapsed_ms: number }>}
   */
  async function settleVisual(target, options = {}) {
    const maxWaitMs = options.maxWaitMs ?? options.timeout_ms ?? DEFAULT_VISUAL_MAX_WAIT_MS;
    const quietMs = options.quietMs ?? DEFAULT_VISUAL_QUIET_MS;
    const doc = options.document ?? (typeof document !== "undefined" ? document : null);
    const nowFn = options.now ?? (typeof performance !== "undefined" && performance.now ? () => performance.now() : Date.now);
    const setTimeoutFn = options.setTimeout ?? setTimeout;
    const clearTimeoutFn = options.clearTimeout ?? clearTimeout;
    const rafFn = options.requestAnimationFrame ?? (typeof requestAnimationFrame === "function" ? requestAnimationFrame : (cb) => setTimeoutFn(cb, 16));
    const cafFn = options.cancelAnimationFrame ?? (typeof cancelAnimationFrame === "function" ? cancelAnimationFrame : clearTimeoutFn);

    const started = nowFn();

    function sampleMetrics() {
      if (target && typeof target.getBoundingClientRect === "function") {
        try {
          const rect = target.getBoundingClientRect();
          return `${rect.x.toFixed(1)},${rect.y.toFixed(1)},${rect.width.toFixed(1)},${rect.height.toFixed(1)}`;
        } catch (_e) {
          // Fall through on error
        }
      }
      if (doc) {
        const docEl = doc.documentElement;
        const body = doc.body;
        const scrollH = docEl?.scrollHeight ?? body?.scrollHeight ?? 0;
        const scrollW = docEl?.scrollWidth ?? body?.scrollWidth ?? 0;
        const clientH = docEl?.clientHeight ?? 0;
        const clientW = docEl?.clientWidth ?? 0;
        return `${scrollW}x${scrollH}_${clientW}x${clientH}`;
      }
      return "stable";
    }

    function hasRunningFiniteAnimations() {
      if (!doc || typeof doc.getAnimations !== "function") return false;
      try {
        const anims = typeof target?.getAnimations === "function" ? target.getAnimations() : doc.getAnimations();
        return anims.some((anim) => {
          if (anim.playState !== "running" && anim.playState !== "pending") return false;
          const effect = anim.effect;
          const timing = effect && typeof effect.getTiming === "function" ? effect.getTiming() : null;
          // Ignore infinite animations like spinning progress indicators
          if (timing && timing.iterations === Infinity) return false;
          return true;
        });
      } catch (_e) {
        return false;
      }
    }

    return new Promise((resolve) => {
      let pendingTimer = null;
      let pendingRaf = null;
      let ceilingTimer = null;
      let finished = false;
      let lastMetrics = sampleMetrics();
      let stableSince = hasRunningFiniteAnimations() ? null : started;

      function cleanup() {
        if (finished) return;
        finished = true;
        if (pendingRaf) cafFn(pendingRaf);
        if (pendingTimer) clearTimeoutFn(pendingTimer);
        if (ceilingTimer) clearTimeoutFn(ceilingTimer);
      }

      function scheduleNext() {
        if (finished) return;
        let fired = false;
        const tick = () => {
          if (fired || finished) return;
          fired = true;
          if (pendingTimer) clearTimeoutFn(pendingTimer);
          if (pendingRaf) cafFn(pendingRaf);
          pendingTimer = null;
          pendingRaf = null;
          step();
        };

        pendingTimer = setTimeoutFn(tick, 25);
        if (!doc?.hidden) {
          pendingRaf = rafFn(tick);
        }
      }

      ceilingTimer = setTimeoutFn(() => {
        cleanup();
        const elapsed = Math.round(nowFn() - started);
        resolve({ settled: false, elapsed_ms: elapsed });
      }, maxWaitMs);

      function step() {
        if (finished) return;
        const now = nowFn();
        const currentMetrics = sampleMetrics();
        const isAnimRunning = hasRunningFiniteAnimations();

        if (currentMetrics === lastMetrics && !isAnimRunning) {
          if (stableSince === null) {
            stableSince = now;
          } else if (now - stableSince >= quietMs) {
            cleanup();
            const elapsed = Math.round(now - started);
            resolve({ settled: true, elapsed_ms: elapsed });
            return;
          }
        } else {
          lastMetrics = currentMetrics;
          stableSince = null;
        }

        if (now - started >= maxWaitMs) {
          cleanup();
          const elapsed = Math.round(now - started);
          resolve({ settled: false, elapsed_ms: elapsed });
          return;
        }

        scheduleNext();
      }

      if (stableSince !== null && quietMs <= 0) {
        cleanup();
        resolve({ settled: true, elapsed_ms: 0 });
        return;
      }

      scheduleNext();
    });
  }

  return {
    settle,
    settleVisual,
    DEFAULT_MAX_WAIT_MS,
    DEFAULT_QUIET_MS,
    DEFAULT_VISUAL_MAX_WAIT_MS,
    DEFAULT_VISUAL_QUIET_MS
  };
});
