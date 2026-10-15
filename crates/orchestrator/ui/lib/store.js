// Ghostlight workbench -- the cache, and the only thing allowed to change it.
//
// The orchestrator is the authority; this is a copy the window can always prove is current.
// Every mutation happens here and nowhere else, and each one announces what changed from a
// closed list of kinds. Nothing in this file touches the document, which is what stops a
// rendering fault from ever being able to corrupt what the window believes.
(function installGhostlightStore(root, factory) {
  const api = factory();
  root.GhostlightStore = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightStoreApi() {
  "use strict";

  const { FEED_LIMIT, WORKING_LATCH_MS } = globalThis.GhostlightWords;
  const { entryFromOperation, entryFromRecord, entryTime, isRunning } = globalThis.GhostlightEntries;

  /**
   * Everything the store can announce.
   *
   * A closed list rather than a general event bus: a reader can see every way this window is
   * allowed to move by reading eight lines, and a new one cannot be added by accident.
   */
  const CHANGE = Object.freeze({
    Feed: "feed",             // the whole feed is new; rebuild it
    Promoted: "promoted",     // a new action took the hero and the old one slid down
    Hero: "hero",             // the hero's own facts changed
    Row: "row",               // one queued row changed
    Dropped: "dropped",       // one entry left the feed
    Band: "band",             // the lamp, the word, or the counts beside it
    Collections: "collections" // connections, about, integrations, status
  });

  function create({
    announce = () => {},
    now = Date.now,
    setTimer = setTimeout,
    clearTimer = clearTimeout
  } = {}) {
    const state = {
      seq: 0,
      connected: false,
      runtime: "active",
      snapshot: null,
      feed: [],
      hidden: new Set(),
      pending: new Set(),
      interactionAt: 0,
      latchTimer: null
    };

    const emit = (kind, detail = {}) => announce(kind, detail);

    function trim() {
      while (state.feed.length > FEED_LIMIT) {
        const dropped = state.feed.pop();
        emit(CHANGE.Dropped, { entry: dropped });
      }
    }

    /**
     * Whether anything is working through Ghostlight right now.
     *
     * Per-operation truth flickers, because most calls settle in well under a second, so the
     * answer latches and every action pushes the deadline back.
     */
    function working() {
      if (state.feed.some(isRunning)) return true;
      return now() - state.interactionAt < WORKING_LATCH_MS;
    }

    /**
     * Mark that something interacted, and arrange to notice when that stops.
     *
     * Nothing else wakes the band once the last operation settles, so the latch schedules its own
     * expiry or the word stays lit until an unrelated repaint.
     */
    function touch() {
      state.interactionAt = now();
      clearTimer(state.latchTimer);
      state.latchTimer = setTimer(() => emit(CHANGE.Band), WORKING_LATCH_MS + 60);
    }

    function seed(snapshot) {
      const live = snapshot.operations.map(entryFromOperation);
      const byInvocation = new Map();
      for (const entry of live) byInvocation.set(entry.invocation, entry);
      for (const record of snapshot.history) {
        const existing = byInvocation.get(record.invocation);
        byInvocation.set(record.invocation, entryFromRecord(record, existing));
      }
      state.feed = [...byInvocation.values()]
        .filter((entry) => !state.hidden.has(entry.invocation))
        .sort((left, right) => {
          const rank = entry => isRunning(entry) ? (entry.phase === "waiting" ? 1 : 0) : 2;
          return rank(left) - rank(right) || entryTime(right) - entryTime(left);
        });
      trim();
    }

    /** The conveyor: whatever held the hero slides down, the new action rises. */
    function promote(entry) {
      const previous = state.feed[0];
      state.feed.unshift(entry);
      trim();
      emit(CHANGE.Promoted, { entry, previous: previous?.invocation === entry.invocation ? null : previous });
    }

    function started(operation) {
      if (state.hidden.has(operation.invocation)) return;
      const entry = entryFromOperation(operation);
      const index = state.feed.findIndex((item) => item.invocation === entry.invocation);
      if (entry.phase === "waiting" && index < 0 && state.feed.some(item => isRunning(item) && item.phase !== "waiting")) {
        const lastLive = state.feed.findLastIndex(isRunning);
        state.feed.splice(lastLive + 1, 0, entry);
        trim();
        emit(CHANGE.Feed);
        return;
      }
      if (index === 0) {
        state.feed[0] = { ...state.feed[0], ...entry };
        emit(CHANGE.Hero, { entry: state.feed[0] });
        return;
      }
      if (index > 0) {
        const [removed] = state.feed.splice(index, 1);
        emit(CHANGE.Dropped, { entry: removed });
      }
      promote(entry);
    }

    function changed(operation) {
      if (state.hidden.has(operation.invocation)) return;
      const index = state.feed.findIndex((item) => item.invocation === operation.invocation);
      if (index < 0) return started(operation);
      state.feed[index] = { ...state.feed[index], ...entryFromOperation(operation) };
      if (index === 0) emit(CHANGE.Hero, { entry: state.feed[0] });
      else emit(CHANGE.Row, { entry: state.feed[index] });
    }

    /**
     * Take the status part the orchestrator published as this window's own.
     *
     * The part replaces what the snapshot held under the same names, so the word, the lamp and
     * the chip change without a pull.
     */
    function applyStatus(status) {
      state.runtime = status.service.runtime_state;
      const snapshot = state.snapshot;
      if (!snapshot) return;
      snapshot.service = status.service;
      snapshot.readiness = status.readiness;
      snapshot.audit_health = status.audit_health;
      snapshot.audit_notice = status.audit_notice;
      if (status.runtime_note) snapshot.runtime_note = status.runtime_note;
      else delete snapshot.runtime_note;
      snapshot.configuration = {
        ...snapshot.configuration,
        ...status.configuration,
        runtime_state: status.service.runtime_state
      };
      emit(CHANGE.Collections, { snapshot });
    }

    /** Take the controls part the orchestrator published as this window's own. */
    function applyControls(controls) {
      const snapshot = state.snapshot;
      if (!snapshot) return;
      snapshot.controls = controls;
      emit(CHANGE.Collections, { snapshot });
    }

    /** Take the connections part the orchestrator published as this window's own. */
    function applyConnections(connections) {
      const snapshot = state.snapshot;
      if (!snapshot) return;
      snapshot.sessions = connections.sessions;
      snapshot.browsers = connections.browsers;
      snapshot.overview = {
        ...snapshot.overview,
        active_sessions: connections.sessions.length,
        connected_browsers: connections.browsers.length
      };
      emit(CHANGE.Collections, { snapshot });
    }

    function settled(record) {
      if (state.snapshot) {
        const previous = state.snapshot.history.findIndex((item) => item.invocation === record.invocation);
        if (previous >= 0) state.snapshot.history[previous] = record;
        else state.snapshot.history.unshift(record);
        state.snapshot.history.length = Math.min(state.snapshot.history.length, FEED_LIMIT);
      }
      if (state.hidden.has(record.invocation)) return;
      const index = state.feed.findIndex((item) => item.invocation === record.invocation);
      if (index < 0) {
        promote(entryFromRecord(record, null));
        return;
      }
      state.feed[index] = entryFromRecord(record, state.feed[index]);
      if (index === 0) emit(CHANGE.Hero, { entry: state.feed[0] });
      else emit(CHANGE.Row, { entry: state.feed[index] });
    }

    return Object.freeze({
      CHANGE,

      // ---- what the window may ask -------------------------------------------------
      feed: () => state.feed,
      hero: () => state.feed[0],
      snapshot: () => state.snapshot,
      seq: () => state.seq,
      runtime: () => state.runtime,
      connected: () => state.connected,
      pending: () => state.pending,
      working,

      /** The content-free facts the band shows, assembled in one place rather than at the paint. */
      band() {
        return {
          connected: state.connected,
          runtime: state.runtime,
          working: working(),
          snapshot: state.snapshot,
          running: state.feed.filter(isRunning).length
        };
      },

      /** Which client asked, and over which intake, for a workspace still connected. */
      sessionFor(workspace) {
        return state.snapshot?.sessions.find((item) => item.id === workspace) ?? null;
      },

      // ---- the only ways it may move -----------------------------------------------
      setConnected(value) {
        if (state.connected === value) return;
        state.connected = value;
        emit(CHANGE.Band);
      },

      applySnapshot(snapshot, rebuild) {
        state.snapshot = snapshot;
        state.seq = snapshot.seq;
        state.runtime = snapshot.service.runtime_state;
        if (rebuild) {
          seed(snapshot);
          emit(CHANGE.Feed);
        }
        emit(CHANGE.Collections, { snapshot });
        emit(CHANGE.Band);
      },

      /**
       * Fold one sequenced change in.
       *
       * A gap means this cache can no longer be trusted, so it says so rather than guessing, and
       * the caller rebuilds from a fresh snapshot. A change numbered at or below the snapshot's is
       * already in it.
       */
      applyChange(event) {
        if (!event || typeof event.seq !== "number") return "ignored";
        if (event.seq <= state.seq) return "ignored";
        if (event.seq !== state.seq + 1) return "gap";
        state.seq = event.seq;
        const change = event.change;
        switch (change.kind) {
          case "audit_health_changed": return "refresh";
          case "document_coverage_changed": {
            if (state.snapshot) {
              state.snapshot.document_coverage = [change.details, ...(state.snapshot.document_coverage ?? []).filter((item) => item.invocation !== change.details.invocation)].slice(0, FEED_LIMIT);
            }
            const entry = state.feed.find((item) => item.invocation === change.details.invocation);
            if (entry) emit(state.feed[0] === entry ? CHANGE.Hero : CHANGE.Row, { entry });
            break;
          }
          case "operation_started": touch(); started(change.operation); break;
          case "operation_changed": touch(); changed(change.operation); break;
          case "operation_settled": touch(); settled(change.record); break;
          case "composition_changed": touch(); settled(change.record); break;
          case "runtime_changed": state.runtime = change.runtime_state; break;
          case "status_changed": applyStatus(change.status); break;
          case "connections_changed": applyConnections(change.connections); break;
          case "controls_changed": applyControls(change.controls); break;
          // The part is read on demand; the caller decides whether this window shows it.
          case "part_changed": return "hinted";
          default: return "ignored";
        }
        emit(CHANGE.Band);
        return "applied";
      },

      /**
       * Hide everything settled from this view.
       *
       * The audit is untouched: these invocations are remembered as hidden so a later snapshot
       * does not quietly bring them back.
       */
      clearCompleted() {
        const completed = state.feed.filter((entry) => !isRunning(entry));
        if (!completed.length) return 0;
        for (const entry of completed) state.hidden.add(entry.invocation);
        state.feed = state.feed.filter(isRunning);
        emit(CHANGE.Feed);
        emit(CHANGE.Band);
        return completed.length;
      },

      beginHarness(id) {
        state.pending.add(id);
        if (state.snapshot) emit(CHANGE.Collections, { snapshot: state.snapshot });
      },

      endHarness(id) {
        state.pending.delete(id);
        if (state.snapshot) emit(CHANGE.Collections, { snapshot: state.snapshot });
      }
    });
  }

  return Object.freeze({ CHANGE, create });
});
