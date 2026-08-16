"use strict";

/*
 * Ghostlight workbench -- the composition root.
 *
 * The orchestrator is the only authority. This surface holds a cache it can always prove is
 * current: every change arrives with a monotonic sequence, and a gap means the cache is thrown
 * away and rebuilt from a fresh snapshot. Nothing here is ever the source of truth.
 *
 * This file composes four parts and owns nothing itself:
 *
 *   Words      the fixed vocabulary and its number formatting  (pure)
 *   Entries    the shape one row of the monitor takes          (pure)
 *   Transport  the only thing that talks to the orchestrator   (no document, no state)
 *   Store      the cache, and the only thing that may change it (no document)
 *   View       everything that touches the document            (cannot fetch, cannot mutate)
 *
 * Data flows one way: transport brings a snapshot, the store folds it in and announces what
 * changed from a closed list of kinds, and the view draws what it is handed. A view that cannot
 * fetch can never fail on a missing snapshot, and a store that never sees the document cannot be
 * corrupted by a rendering fault.
 */

const { CHANGE_EVENT, HEARTBEAT_MS, SEARCH_VIEWS } = globalThis.GhostlightWords;

const transport = globalThis.GhostlightTransport.create({
  invoke: window.__TAURI__?.core?.invoke,
  listen: window.__TAURI__?.event?.listen,
  changeEvent: CHANGE_EVENT
});

/*
 * Nothing in this window is allowed to fail quietly.
 *
 * A surface that throws where nobody is looking is indistinguishable from a surface that is
 * merely slow, and the person waiting has no way to tell which. Every failure gets a visible
 * notice; identical failures are stated once so a repeating fault does not bury the screen.
 */
let lastFailure = null;
function reportFailure(what, error) {
  const detail = `${what}: ${error?.message ?? error}`;
  if (lastFailure === detail) return;
  lastFailure = detail;
  // The console is the last channel that still works when the surface itself is broken, so a
  // failure goes there whether or not there is anything left to render a notice with.
  console.error(detail, error);
  if (view?.el.toast) view.toast(detail, true);
}

/** Run one fallible step. Report what went wrong, and tell the caller it did not happen. */
function attempt(what, step) {
  try {
    step();
    return true;
  } catch (error) {
    reportFailure(what, error);
    return false;
  }
}

const store = globalThis.GhostlightStore.create({ announce: (kind, detail) => draw(kind, detail) });
const view = globalThis.GhostlightView.create({
  sessionFor: (workspace) => store.sessionFor(workspace),
  onFailure: reportFailure
});

/**
 * The one place a change becomes a picture.
 *
 * Exhaustive on the store's vocabulary on purpose: a new kind of change must be given a drawing
 * here, and an unknown one is reported rather than ignored, because silently drawing nothing is
 * how a window ends up lying about what happened.
 */
function draw(kind, detail) {
  const { CHANGE } = store;
  switch (kind) {
    case CHANGE.Feed:
      view.rebuildFeed(store.feed());
      break;
    case CHANGE.Promoted:
      view.promote(detail.entry, detail.previous, store.feed());
      break;
    case CHANGE.Hero:
      view.hero(detail.entry, false);
      break;
    case CHANGE.Row:
      view.row(detail.entry);
      break;
    case CHANGE.Dropped:
      view.drop(detail.entry);
      break;
    case CHANGE.Band:
      view.attempt("painting the band", () => view.band(store.band()));
      break;
    case CHANGE.Collections:
      view.collections(detail.snapshot, store.pending());
      break;
    default:
      reportFailure("drawing a change", new Error(`unknown change ${kind}`));
  }
}

/* ------------------------------ synchronizing --------------------------- */

async function resync({ rebuildFeed = false, quiet = true } = {}) {
  if (!transport.available) {
    store.setConnected(false);
    return;
  }
  let snapshot;
  try {
    snapshot = await transport.snapshot();
  } catch (error) {
    // Losing the orchestrator is an ordinary condition with a state of its own to show.
    store.setConnected(false);
    if (!quiet) view.toast(String(error), true);
    return;
  }
  store.setConnected(true);
  // A surface that failed to draw is not a surface that lost its connection. One catch around
  // both said "Not connected" for either, which sends the reader looking at the wrong thing.
  attempt("rendering the snapshot", () =>
    store.applySnapshot(snapshot, rebuildFeed || snapshot.seq !== store.snapshot()?.seq || !store.snapshot()));
}

function receiveChange(event) {
  // A gap means this cache can no longer be trusted. Rebuild rather than guess.
  if (store.applyChange(event) === "gap") resync({ rebuildFeed: true });
}

/* --------------------------------- policy ------------------------------- */

/*
 * The Policy destination is fetched rather than carried in every snapshot.
 *
 * It changes when somebody changes a policy, which is rare, and it carries the exact documents.
 * Pulling it on arrival and on demand keeps the ten-second safety pull small.
 */
async function loadPolicy() {
  if (!transport.available) return;
  try {
    view.policy(await transport.policy());
  } catch (error) {
    reportFailure("reading the policy", error);
  }
}

async function checkPolicy() {
  const document = view.draftDocument();
  if (!document) return;
  try {
    view.previewResult(await transport.previewPolicy(document));
  } catch (error) {
    view.previewCleared();
    view.editorStatus(String(error));
  }
}

async function applyPolicy() {
  const document = view.draftDocument();
  if (!document) return;
  try {
    view.toast((await transport.applyPolicy(document)).message);
    view.previewCleared();
    await loadPolicy();
    await resync();
  } catch (error) {
    // A refused document changed nothing, so the draft stays exactly as the person left it.
    view.editorStatus(String(error));
  }
}

async function removePolicy() {
  try {
    view.toast((await transport.removePolicy()).message);
    view.previewCleared();
    await loadPolicy();
    await resync();
  } catch (error) {
    view.toast(String(error), true);
  }
}

/* -------------------------------- intents ------------------------------- */

async function applyIntent(intent) {
  if (!transport.available) return;
  try {
    view.toast((await transport.applyIntent(intent)).message);
    await resync();
  } catch (error) {
    view.toast(String(error), true);
  }
}

async function handleHarnessAction(button) {
  if (!transport.available || button.disabled) return;
  const { harness: id, harnessOperation: operation, harnessAction: action,
    harnessName: name, product: productId, copyKind: copyKind } = button.dataset;
  if (operation === "manage" && action === "uninstall" && !(await view.confirmRemoval(name))) return;
  if (operation === "download") {
    try {
      await transport.openHarnessDownload(productId);
      view.toast(`Opened the official ${name} download page.`);
    } catch (error) {
      view.toast(String(error), true);
    }
    return;
  }
  if (operation === "copy") {
    try {
      view.toast(await transport.copyHarnessText(id, copyKind));
    } catch (error) {
      view.toast(String(error), true);
    }
    return;
  }
  store.beginHarness(id);
  try {
    if (operation === "locate") {
      const result = await transport.locateHarness(id);
      if (result) view.toast(result.message);
    } else {
      view.toast((await transport.manageHarness(id, action)).message);
    }
  } catch (error) {
    if (operation === "manage") view.openHarnessManual(id);
    view.toast(String(error), true);
  } finally {
    store.endHarness(id);
    await resync();
  }
}

async function search(query) {
  const trimmed = query.trim();
  if (!trimmed) {
    view.searchResults([]);
    return;
  }
  try {
    view.searchResults(await transport.search(trimmed));
  } catch (error) {
    view.searchFailed(error);
  }
}

/** Every destination opens in the browser you already use, which is the one Ghostlight drives. */
async function openDestination(destination) {
  try {
    await transport.openDestination(destination);
  } catch (error) {
    view.toast(String(error), true);
  }
}

async function withButton(event, work, done) {
  // event.currentTarget is only live during synchronous dispatch: the browser nulls it once the
  // listener returns, and an async listener returns at its first await, long before work()
  // settles. Reading it again in finally threw on a null, uncaught, and left the button stuck
  // disabled. A captured element reference has no such expiry.
  const button = event.currentTarget;
  button.disabled = true;
  try {
    await work();
    view.toast(done);
  } catch (error) {
    view.toast(String(error), true);
  } finally {
    button.disabled = false;
  }
}

/* -------------------------------- wiring -------------------------------- */

/**
 * Attach every listener the surface needs.
 *
 * These ran as loose top-level statements, which put them ahead of boot: one listener failing to
 * attach took the first snapshot, the change subscription and the heartbeat down with it, because
 * nothing after the throw ever ran. They are one isolated step now.
 */
function wire() {
  const el = view.el;
  let searchTimer = null;

  document.addEventListener("click", (event) => {
    const confirmation = event.target.closest("[data-confirm]");
    if (confirmation && view.answerConfirmation(confirmation.dataset.confirm === "remove")) return;
    const tab = event.target.closest("[data-view]");
    if (tab) {
      view.navigate(tab.dataset.view);
      if (tab.dataset.view === "policy") loadPolicy();
    }
    const ruleAction = event.target.closest("[data-rule-action]");
    if (ruleAction && !ruleAction.disabled) {
      view.ruleAction(Number(ruleAction.dataset.rule), ruleAction.dataset.ruleAction);
      return;
    }
    // Opening a rule is the whole interaction for one nobody may change, and the way in for one
    // they may. An action inside an open rule is handled above and must not close it again.
    const toggle = event.target.closest("[data-rule-toggle]");
    if (toggle) view.toggleRule(toggle.dataset.ruleToggle);
    const intent = event.target.closest("[data-intent]");
    if (intent && !intent.disabled) applyIntent(intent.dataset.intent);
    const harness = event.target.closest("[data-harness-operation]");
    if (harness) handleHarnessAction(harness);
    const destination = event.target.closest("[data-destination]");
    if (destination) openDestination(destination.dataset.destination);
    const hit = event.target.closest("[data-search-view]");
    if (hit) {
      view.navigate(SEARCH_VIEWS[hit.dataset.searchView] ?? "monitor");
      view.closePalette();
    }
    if (event.target === el.palette) view.closePalette();
  });

  window.addEventListener("focus", async () => {
    if (!transport.available) return;
    try {
      await transport.refreshHarnesses();
      await resync();
    } catch (error) {
      view.toast(String(error), true);
    }
  });

  el["palette-query"].addEventListener("input", () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => search(view.paletteQuery()), 140);
  });

  document.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      if (view.paletteOpen()) view.closePalette();
      else view.openPalette();
      return;
    }
    if (event.key === "Escape" && view.paletteOpen()) view.closePalette();
  });

  // The editor is one delegated listener per kind of change, so a rule list that redraws after
  // every edit never leaves a listener behind on a node that no longer exists.
  el["rule-list"].addEventListener("input", (event) => {
    const field = event.target.closest("[data-field]");
    if (field) view.editRule(Number(field.dataset.rule), field.dataset.field, field.value);
  });

  el["rule-list"].addEventListener("change", (event) => {
    const box = event.target.closest("[data-capability]");
    if (box) view.toggleCapability(Number(box.dataset.rule), box.dataset.capability, box.checked);
  });

  el["setting-groups"].addEventListener("change", (event) => {
    const box = event.target.closest("[data-restriction]");
    // The checkbox is the permission as the person sees it: checked means allowed. setPermission
    // is the seam that turns that back into the tightening-only value the schema can express.
    if (box) view.setPermission(box.dataset.restriction, box.checked);
  });

  el["sacred-hosts"].addEventListener("input", (event) => view.setSacred(event.target.value));
  el["add-rule"].addEventListener("click", () => view.addRule());
  el["observe-mode"].addEventListener("change", (event) => view.setObserve(event.target.checked));
  el["check-policy"].addEventListener("click", () => checkPolicy());
  el["apply-policy"].addEventListener("click", () => applyPolicy());
  el["discard-policy"].addEventListener("click", () => view.discardDraft());
  el["remove-policy"].addEventListener("click", () => removePolicy());
  el["refresh-policy"].addEventListener("click", () => {
    loadPolicy().then(() => view.toast("Policy re-read."));
  });

  el["refresh-status"].addEventListener("click", () => {
    resync({ quiet: false }).then(() => view.toast("Status refreshed."));
  });

  el["clear-monitor"].addEventListener("click", () => {
    const cleared = store.clearCompleted();
    if (!cleared) return;
    view.toast(`Cleared ${cleared} ${cleared === 1 ? "entry" : "entries"} from this view. Audit history is unchanged.`);
  });

  el["refresh-integrations"].addEventListener("click", (event) =>
    withButton(event, async () => {
      await transport.refreshHarnesses();
      await resync();
    }, "MCP clients re-checked."));

  el["test-notification"].addEventListener("click", (event) =>
    withButton(event, () => transport.testNotification(), "Test notification sent."));

  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) resync();
  });

  setInterval(() => {
    if (!document.hidden) view.tickElapsed(store.feed());
  }, 100);
  setInterval(() => {
    if (!document.hidden) view.tickAges(store.feed());
  }, 15000);
}

/*
 * Start the window, in the order that matters.
 *
 * These were loose top-level statements, so a throw in any one of them silently abandoned every
 * statement after it -- which is how one missing element id left the band reading "Starting"
 * forever with no snapshot, no change subscription, and no heartbeat to recover with.
 *
 * The rule now: the live surface is brought up first, and anything decorative goes last, where
 * failing cannot cost the window its connection to the truth.
 */
function boot() {
  // The heartbeat is this surface's own recovery, so it is installed before anything that can
  // fail. A bad subscription or a bad first snapshot then costs one cycle rather than the window.
  setInterval(() => {
    if (!document.hidden) resync();
  }, HEARTBEAT_MS);
  attempt("wiring the surface", wire);
  attempt("subscribing to changes", () => transport.subscribe(receiveChange));
  attempt("first snapshot", () => resync({ rebuildFeed: true }));
  attempt("about card", () => view.armCard());
}

// Anything that escapes a listener or a promise still has to reach the person using the window.
window.addEventListener("error", (event) => reportFailure("surface", event.error ?? event.message));
window.addEventListener("unhandledrejection", (event) => reportFailure("surface", event.reason));

boot();
