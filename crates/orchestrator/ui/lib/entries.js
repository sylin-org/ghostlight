// Ghostlight workbench -- one row of the monitor as a value, in the shape this window keeps it.
(function installGhostlightEntries(root, factory) {
  const api = factory();
  root.GhostlightEntries = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightEntries() {
  "use strict";

  /* -------------------------------- entries ------------------------------- */

  function entryFromOperation(operation) {
    return {
      invocation: operation.invocation,
      workspace: operation.workspace,
      tool: operation.tool,
      activity: operation.activity,
      capability: operation.capability,
      startedAt: operation.started_at_ms ?? Date.now(),
      phase: operation.phase,
      settled: false
    };
  }

  function entryFromRecord(record, existing) {
    return {
      ...(existing ?? {}),
      invocation: record.invocation,
      workspace: record.workspace,
      tool: record.tool,
      // No activity when the record was restored rather than watched: the medallion then comes
      // from the tool. Defaulting to the quiet label is what made every row identical.
      activity: existing?.activity,
      capability: record.capability,
      startedAt: existing?.startedAt,
      endedAt: record.timestamp_ms,
      phase: record.allowed ? "completed" : "blocked",
      allowed: record.allowed,
      reason: record.reason,
      status: record.status,
      effect: record.effect,
      summary: record.summary,
      durationMs: record.duration_ms,
      observed: record.observed ?? null,
      channel: record.channel ?? null,
      settled: true
    };
  }

  const entryTime = entry => entry.endedAt ?? entry.startedAt ?? 0;

  /**
   * How long the work took. The orchestrator measures this, so a record restored from the audit
   * file reports a real span instead of the blank left by never having watched it start.
   */
  const settledMs = entry =>
    entry.durationMs || (entry.endedAt && entry.startedAt ? entry.endedAt - entry.startedAt : NaN);
  const isRunning = entry => !entry.settled && (entry.phase === "running" || entry.phase === "held" || entry.phase === "attention");
  const isBlocked = entry => entry.phase === "blocked" || entry.allowed === false;

  return Object.freeze({ entryFromOperation, entryFromRecord, entryTime, settledMs, isRunning, isBlocked });
});
