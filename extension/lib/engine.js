// Browser-local attempt persistence and duplicate suppression. The journal stores only an epoch,
// opaque operation ids and closed phases; receipts and errors remain in this worker's memory.
(function installGhostlightOperationEngine(root, factory) {
  const api = factory();
  root.GhostlightOperationEngine = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightOperationEngineApi() {
  "use strict";

  const PHASE = Object.freeze({
    ACCEPTED: "accepted",
    DISPATCHED: "dispatched",
    COMPLETED: "completed",
    FAILED: "failed",
    UNCERTAIN: "uncertain"
  });
  const RESTORABLE_PHASES = new Set(Object.values(PHASE));
  const JOURNAL_ERROR = Object.freeze({
    RESULT_UNAVAILABLE: "operation_result_unavailable",
    LEDGER_FULL: "operation_ledger_full"
  });

  function recoveryError() {
    const error = new Error("The browser operation may have completed, but its result is unavailable.");
    error.code = JOURNAL_ERROR.RESULT_UNAVAILABLE;
    error.effectUnknown = true;
    return error;
  }

  function capacityError() {
    const error = new Error("The browser operation recovery ledger is full; this operation did not run.");
    error.code = JOURNAL_ERROR.LEDGER_FULL;
    error.effectUnknown = false;
    return error;
  }

  function validOpaqueId(value) {
    return typeof value === "string"
      && value.length > 0
      && value.length <= 96
      && /^[A-Za-z0-9_-]+$/.test(value);
  }

  function create({ load, save, maximumRecords = 256 }) {
    if (typeof load !== "function" || typeof save !== "function") {
      throw new TypeError("operation persistence requires load and save functions");
    }
    if (!Number.isSafeInteger(maximumRecords) || maximumRecords < 1) {
      throw new TypeError("maximumRecords must be a positive integer");
    }

    let epoch = null;
    const records = new Map();
    let saving = Promise.resolve();
    const ready = restore();

    async function restore() {
      let stored;
      try { stored = await load(); } catch (_error) { return; }
      if (!stored || typeof stored !== "object" || !validOpaqueId(stored.epoch)) return;
      epoch = stored.epoch;
      if (!Array.isArray(stored.records)) return;
      for (const item of stored.records.slice(-maximumRecords)) {
        if (!item || !validOpaqueId(item.id) || !RESTORABLE_PHASES.has(item.phase)) continue;
        records.set(item.id, { phase: item.phase, result: undefined, promise: null });
      }
    }

    function snapshot() {
      return {
        epoch,
        records: Array.from(records, ([id, record]) => ({ id, phase: record.phase }))
      };
    }

    function persist() {
      // Capture this transition now; a later transition must neither remove it before its save
      // nor finish an older save after a newer snapshot has reached storage.
      const value = snapshot();
      const pending = saving.then(() => save(value));
      saving = pending.catch(() => {});
      return pending;
    }

    async function activate(nextEpoch) {
      await ready;
      if (!validOpaqueId(nextEpoch)) throw new Error("service supplied an invalid operation epoch");
      if (epoch === nextEpoch) return false;
      epoch = nextEpoch;
      records.clear();
      try { await persist(); } catch (_error) { /* dispatch still verifies persistence */ }
      return true;
    }

    function run(record, id, operation) {
      record.phase = PHASE.DISPATCHED;
      records.set(id, record);
      const promise = (async () => {
        try {
          try {
            await persist();
          } catch (error) {
            if (records.get(id) === record) records.delete(id);
            throw error;
          }

          let result;
          try {
            result = await operation();
          } catch (error) {
            record.phase = error?.effectUnknown ? PHASE.UNCERTAIN : PHASE.FAILED;
            record.error = error;
            try { await persist(); } catch (_persistenceError) { /* disposition remains conservative */ }
            throw error;
          }
          record.phase = PHASE.COMPLETED;
          record.result = result;
          try { await persist(); } catch (_error) { /* retain the decisive in-memory receipt */ }
          return result;
        } finally {
          record.promise = null;
        }
      })();
      record.promise = promise;
      return promise;
    }

    async function execute(id, operation) {
      await ready;
      if (!epoch) throw new Error("browser operation engine is not negotiated");
      if (!validOpaqueId(id)) throw new Error("browser operation has an invalid correlation id");
      if (typeof operation !== "function") throw new TypeError("browser operation must be a function");

      const existing = records.get(id);
      if (existing?.promise) return existing.promise;
      if (existing?.phase === PHASE.COMPLETED && existing.result !== undefined) return existing.result;
      if (existing?.phase === PHASE.FAILED && existing.error) throw existing.error;
      if (existing?.phase === PHASE.ACCEPTED || existing?.phase === PHASE.FAILED) {
        return run(existing, id, operation);
      }
      if (existing) {
        throw recoveryError();
      }
      if (records.size >= maximumRecords) {
        throw capacityError();
      }

      return run({ phase: PHASE.DISPATCHED, result: undefined, promise: null }, id, operation);
    }

    async function acknowledge(id) {
      await ready;
      if (!validOpaqueId(id)) return;
      const record = records.get(id);
      if (!record || record.promise) return;
      records.delete(id);
      try { await persist(); } catch (_error) { /* an acknowledged effect cannot be replayed */ }
    }

    return Object.freeze({ activate, execute, acknowledge, snapshot });
  }

  return Object.freeze({ create });
});
