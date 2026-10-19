// Ghostlight workbench -- one row of the monitor as a value, in the shape this window keeps it.
(function installGhostlightEntries(root, factory) {
  const api = factory();
  root.GhostlightEntries = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightEntries() {
  "use strict";

  const { PHASE, OUTCOME } = globalThis.GhostlightWords;

  /* -------------------------------- entries ------------------------------- */

  function entryFromOperation(operation) {
    return {
      invocation: operation.invocation,
      workspace: operation.workspace,
      tool: operation.tool,
      activity: operation.activity,
      capability: operation.capability,
      provenance: operation.provenance ?? null,
      agent: operation.agent ?? null,
      channel: operation.provenance?.channel ?? null,
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
      phase: record.complete === false && existing && !existing.settled ? existing.phase
        : record.outcome === OUTCOME.refused ? "blocked" : "completed",
      reason: record.reason,
      // Kept so a refusal can name the rule that caused it rather than only that it happened.
      policyTier: record.policy_tier ?? null,
      grantId: record.grant_id ?? null,
      denialId: record.denial_id ?? null,
      // What the service decided the call came to; the row is drawn from it alone.
      outcome: record.outcome ?? null,
      effect: record.effect,
      summary: record.summary,
      storage: record.storage,
      storage_detail: record.storage_detail,
      steps: record.steps ?? [],
      composition: record.composition ?? null,
      permissions: record.permissions ?? { checks: [], truncated: false },
      permissionExplanations: record.permission_explanations ?? [],
      durationMs: record.duration_ms,
      observed: record.observed ?? null,
      provenance: record.provenance ?? null,
      agent: record.agent ?? null,
      channel: record.channel ?? null,
      browser: record.browser ?? existing?.browser ?? null,
      settled: record.complete !== false || !existing || existing.settled
    };
  }

  const entryTime = entry => entry.endedAt ?? entry.startedAt ?? 0;

  /**
   * How long the work took. The orchestrator measures this, so a record restored from the audit
   * file reports a real span instead of the blank left by never having watched it start.
   */
  const settledMs = entry =>
    entry.durationMs || (entry.endedAt && entry.startedAt ? entry.endedAt - entry.startedAt : NaN);

  /** Whether the work is running now: the only work that looks live. */
  const isRunning = entry => !entry.settled && entry.phase === PHASE.running;

  /** Whether the work is accepted but not running: waiting its turn, or held by a pause. */
  const isPending = entry => !entry.settled && (entry.phase === PHASE.waiting || entry.phase === PHASE.held);

  /** Whether the work has not settled yet, running or pending. */
  const isUnsettled = entry => isRunning(entry) || isPending(entry);

  /** Whether policy or a person refused the action: the one outcome drawn in the full red treatment. */
  const isBlocked = entry => entry.phase === "blocked" || entry.outcome === OUTCOME.refused;

  /**
   * Whether a settled action did not do what it was asked: it failed, or what it did cannot be
   * confirmed. A refusal is not a failure; it has its own treatment.
   */
  const isFailed = entry => Boolean(entry.settled)
    && (entry.outcome === OUTCOME.failed || entry.outcome === OUTCOME.unknown);

  return Object.freeze({
    entryFromOperation, entryFromRecord, entryTime, settledMs, isRunning, isPending, isUnsettled,
    isBlocked, isFailed
  });
});
