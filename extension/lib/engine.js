(function installGhostlightOperationEngine(root, factory) {
  const api = factory();
  root.GhostlightOperationEngine = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightOperationEngineApi() {
  "use strict";

  const RESTORABLE_PHASES = new Set(["accepted", "dispatched", "completed", "failed", "uncertain"]);

  function recoveryError(message) {
    const error = new Error(message);
    error.code = "operation_result_unavailable";
    error.effectUnknown = true;
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

    async function persist() {
      await save(snapshot());
    }

    async function activate(nextEpoch) {
      await ready;
      if (!validOpaqueId(nextEpoch)) throw new Error("service supplied an invalid operation epoch");
      if (epoch === nextEpoch) return;
      epoch = nextEpoch;
      records.clear();
      try { await persist(); } catch (_error) { /* dispatch still verifies persistence */ }
    }

    function run(record, id, operation) {
      const promise = (async () => {
        try {
          try {
            await persist();
            record.phase = "dispatched";
            await persist();
          } catch (error) {
            record.phase = "accepted";
            throw error;
          }

          let result;
          try {
            result = await operation();
          } catch (error) {
            record.phase = error?.effectUnknown ? "uncertain" : "failed";
            record.error = error;
            try { await persist(); } catch (_persistenceError) { /* disposition remains conservative */ }
            throw error;
          }
          record.phase = "completed";
          record.result = result;
          try { await persist(); } catch (_error) { /* retain the decisive in-memory receipt */ }
          return result;
        } finally {
          record.promise = null;
        }
      })();
      record.promise = promise;
      records.set(id, record);
      return promise;
    }

    async function execute(id, operation) {
      await ready;
      if (!epoch) throw new Error("browser operation engine is not negotiated");
      if (!validOpaqueId(id)) throw new Error("browser operation has an invalid correlation id");
      if (typeof operation !== "function") throw new TypeError("browser operation must be a function");

      const existing = records.get(id);
      if (existing?.promise) return existing.promise;
      if (existing?.phase === "completed" && existing.result !== undefined) return existing.result;
      if (existing?.phase === "failed" && existing.error) throw existing.error;
      if (existing?.phase === "accepted" || existing?.phase === "failed") {
        return run(existing, id, operation);
      }
      if (existing) {
        throw recoveryError("The browser operation may have completed, but its result is unavailable.");
      }
      if (records.size >= maximumRecords) {
        throw recoveryError("The browser operation recovery ledger is full.");
      }

      return run({ phase: "accepted", result: undefined, promise: null }, id, operation);
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
