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
 *
 * The page asks the service first what it may do, and renders what it is told. Inside the native
 * shell it speaks to the shell; anywhere else it speaks to the service over loopback, with the
 * key the shell hands to its own page and a browser tab never has.
 */

const {
  CHANGE_EVENT, HEARTBEAT_MS, FEED_LIMIT, SEARCH_VIEWS, POLICY_CHANGED_ELSEWHERE, COPIED_SETUP, COPIED_COMMAND,
  WATCHING_TURNED_ON, WATCHING_TURNED_OFF, TEST_NOTIFICATION_SENT, ADDRESS_COPIED, SETTINGS, DEFAULT_VIEW,
  parseAddress, screenState, byteSize
} = globalThis.GhostlightWords;

/** How long an act waits for the change it caused to reach this page, before it asks again. */
const SEQ_WAIT_MS = 2000;

const { ACT, CONTEXT_CLOSED, OPENER_SERVICE, COPY_KIND } = globalThis.GhostlightTransport;

const native = window.__TAURI__?.core?.invoke;
const transport = native
  ? globalThis.GhostlightTransport.create({
    invoke: window.__TAURI__?.core?.invoke,
    listen: window.__TAURI__?.event?.listen,
    changeEvent: CHANGE_EVENT
  })
  : globalThis.GhostlightTransport.overHttp({
    fetch: window.fetch.bind(window),
    clipboard: navigator.clipboard,
    bearer: globalThis.GhostlightAccess?.bearer
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
  coverageFor: (invocation) => store.snapshot()?.document_coverage?.find((item) => item.invocation === invocation),
  onFailure: reportFailure
});

/*
 * Settings.
 *
 * It is drawn from the controls part of the snapshot and acts through the functions below, so it
 * holds no transport of its own. Its personal settings are kept in this browser when it lets us.
 */
function personalStorage() {
  try {
    return window.localStorage ?? null;
  } catch (error) {
    return null;
  }
}

const settings = globalThis.GhostlightSettings.create({
  document,
  words: { ...SETTINGS, screenState, byteSize },
  storage: personalStorage(),
  applyIntent,
  setWatching,
  setDiagnostics,
  revealDiagnostics,
  testNotification,
  continueInWindow,
  quit: quitGhostlight,
  mayQuit: () => allowed(ACT.quit),
  copyText: typeof transport.copyText === "function" ? copyAddress : undefined,
  onFailure: reportFailure
});

/* The controls part last painted, so a collection that did not change it repaints nothing. */
let paintedControls = null;

/** Draw the controls part in Settings, and the band's chip from the context it names. */
function paintControls(part) {
  if (!part) return;
  const signature = JSON.stringify(part);
  if (signature === paintedControls) return;
  const painted = attempt("painting Settings", () => {
    settings.render(part);
    if (part.you) view.chip(part.you);
  });
  if (painted) paintedControls = signature;
}

/* What the service said this page may do. Nothing is offered until it has said. */
let context = null;
const allowed = (act) => context?.acts?.[act]?.available ?? true;
const showsData = () => context !== null && context.context !== CONTEXT_CLOSED;

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
      paintControls(detail.snapshot.controls);
      break;
    default:
      reportFailure("drawing a change", new Error(`unknown change ${kind}`));
  }
}

/* ------------------------------ synchronizing --------------------------- */

/* Changes that arrived before the first snapshot, in the order they arrived. */
const early = [];

/* The sequence the first hello of the live stream named, until a snapshot has caught up with it. */
let helloSeq = null;
let helloSeen = false;

/* Acts waiting for the change they caused to reach this page. */
const waiters = new Set();

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
  caughtUp();
}

/** A snapshot is in: replay what arrived while it was on its way, and wake the acts that waited. */
function caughtUp() {
  for (const event of early.splice(0)) receiveChange(event);
  if (helloSeq !== null && helloSeq > store.seq()) {
    helloSeq = null;
    resync({ rebuildFeed: true });
  }
  reached();
}

function receiveChange(event) {
  if (!store.snapshot()) {
    if (early.length < FEED_LIMIT) early.push(event);
    return;
  }
  // A gap means this cache can no longer be trusted. Rebuild rather than guess.
  const change = store.applyChange(event);
  if (change === "gap") resync({ rebuildFeed: true });
  else if (change === "refresh") resync();
  else if (change === "hinted") hinted(event.change);
  reached();
}

/** Resolve every act whose change has reached the cache. */
function reached() {
  for (const waiter of [...waiters]) {
    if (store.seq() < waiter.seq) continue;
    clearTimeout(waiter.timer);
    waiters.delete(waiter);
    waiter.resolve(true);
  }
}

/** Wait, for a short while, until the cache has the change numbered `seq`. */
function reaches(seq) {
  if (store.seq() >= seq) return Promise.resolve(true);
  return new Promise((resolve) => {
    const waiter = { seq, resolve, timer: null };
    waiter.timer = setTimeout(() => {
      waiters.delete(waiter);
      resolve(false);
    }, SEQ_WAIT_MS);
    waiters.add(waiter);
  });
}

/**
 * Bring the cache up to date after an act.
 *
 * An act that names the change it caused is waited for, because the live stream delivers that
 * change and asking again would be a second copy of it. An act that names none, or whose change
 * does not arrive, is followed by a snapshot.
 */
async function settled(result) {
  const seq = globalThis.GhostlightTransport.seqOf(result);
  if (seq !== undefined && await reaches(seq)) return;
  await resync();
}

/*
 * A part that is read on demand has changed.
 *
 * The orchestrator says which part and gives its tag. A part this window shows is read again; one
 * it does not show is remembered as out of date and read when the person opens it. A tag already
 * acted on is not acted on twice. Rules the person has edited and not applied are theirs: the
 * policy is not read over them, and it stays out of date until they apply or discard the edits.
 */
const HINT_VIEWS = Object.freeze({ integrations: "integrations", policy: "policy", diagnostics: "status" });
const hintTags = new Map();
const outdated = new Set();

function reread(part) {
  // The snapshot carries integrations and diagnostics; the policy is its own read.
  if (part === "policy") rereadPolicy();
  else resync();
}

function rereadPolicy() {
  // The person's own write reads the policy when it is done, so a hint about it says nothing new.
  if (policyWriting) return;
  if (view.draftIsDirty()) {
    outdated.add("policy");
    view.editorStatus(POLICY_CHANGED_ELSEWHERE);
    return;
  }
  loadPolicy();
}

function discardPolicyDraft() {
  view.discardDraft();
  if (outdated.has("policy")) loadPolicy();
}

function hinted({ part, etag }) {
  if (!HINT_VIEWS[part]) return;
  if (etag !== undefined && hintTags.get(part) === etag) return;
  hintTags.set(part, etag);
  if (view.showing() === HINT_VIEWS[part]) reread(part);
  else outdated.add(part);
}

/**
 * Bring up to date whatever the person is about to see.
 *
 * Opening the clients looks for them again, once. Anything else that changed while it was out of
 * sight is read again.
 */
function openedView(name) {
  if (!showsData()) return;
  if (name === "integrations" && allowed(ACT.rescan)) {
    outdated.delete("integrations");
    rescan().catch((error) => view.toast(String(error), true));
    return;
  }
  for (const part of [...outdated]) {
    if (HINT_VIEWS[part] !== name) continue;
    outdated.delete(part);
    reread(part);
  }
}

/** Look for the supported clients again, and bring the cache up to date. */
async function rescan() {
  await settled(await transport.refreshHarnesses());
}

/* --------------------------------- policy ------------------------------- */

/* The tag of the rules the Policy destination was drawn from, which a write must quote. */
let policyEtag;
let policyWriting = false;

/*
 * The Policy destination is fetched rather than carried in every snapshot.
 *
 * It changes when somebody changes a policy, which is rare, and it carries the exact documents.
 * Pulling it on arrival and on demand keeps the safety pull small.
 */
async function loadPolicy({ keepDraft = false } = {}) {
  if (!transport.available) return;
  try {
    const compiled = await transport.policy();
    policyEtag = compiled.etag;
    view.policy(compiled, { keepDraft });
    outdated.delete("policy");
  } catch (error) {
    reportFailure("reading the policy", error);
  }
}

async function checkPolicy() {
  const draft = view.draftDocument();
  if (!draft) return;
  try {
    view.previewResult(await transport.previewPolicy(draft));
  } catch (error) {
    view.previewCleared();
    view.editorStatus(String(error));
  }
}

/** Write the person's rules, or take them to where the write is made. */
async function writePolicy(write, refused) {
  policyWriting = true;
  try {
    const result = await write();
    view.toast(result.message);
    view.previewCleared();
    await loadPolicy();
    await settled(result);
  } catch (error) {
    if (error?.code === globalThis.GhostlightTransport.STALE) {
      // Rules that moved on are read again, and the person's draft stays exactly as it is.
      view.toast(String(error), true);
      await loadPolicy({ keepDraft: true });
    } else {
      refused(error);
    }
  } finally {
    policyWriting = false;
  }
}

async function applyPolicy() {
  const draft = view.draftDocument();
  if (!draft) return;
  // A refused document changed nothing, so the draft stays exactly as the person left it.
  await writePolicy(() => transport.applyPolicy(draft, policyEtag), (error) => view.editorStatus(String(error)));
}

async function removePolicy() {
  await writePolicy(() => transport.removePolicy(policyEtag), (error) => view.toast(String(error), true));
}

/* -------------------------------- intents ------------------------------- */

async function applyIntent(intent) {
  if (!transport.available) return;
  try {
    const result = await transport.applyIntent(intent);
    view.toast(result.message);
    await settled(result);
  } catch (error) {
    view.toast(String(error), true);
  }
}

/** Bring the Ghostlight application forward, where the change this page cannot make is made. */
async function continueInWindow() {
  try {
    await transport.continueInWindow();
  } catch (error) {
    view.toast(String(error), true);
  }
}

/** Turn "Others can watch" on or off. Settings draws the part the service answers with. */
async function setWatching(watch) {
  if (!transport.available) return;
  try {
    const part = await transport.setWatching(watch);
    view.toast(watch ? WATCHING_TURNED_ON : WATCHING_TURNED_OFF);
    await settled(part);
  } catch (error) {
    view.toast(String(error), true);
    await resync();
  }
}

// The person's diagnostics controls: set the shared marker, or open its folder through the
// service. Both live behind closed commands, like every other act in this window.
async function setDiagnostics(turningOn) {
  if (!transport.available) return;
  try {
    const report = await transport.setDiagnostics(turningOn);
    view.toast(`Process diagnostics are ${report.layer === "off" ? "off" : `on (${report.layer})`}.`);
    await settled(report);
  } catch (error) {
    view.toast(String(error), true);
    await resync();
  }
}

async function revealDiagnostics() {
  if (!transport.available) return;
  try {
    await transport.revealDiagnostics();
  } catch (error) {
    view.toast(String(error), true);
  }
}

/** Prove the notification path end to end. */
async function testNotification() {
  if (!transport.available) return;
  try {
    await transport.testNotification();
    view.toast(TEST_NOTIFICATION_SENT);
  } catch (error) {
    view.toast(String(error), true);
  }
}

/**
 * Quit Ghostlight, as the tray does. The service answers before it ends, and the band then says
 * so in place of "Reconnecting", which is what its lost stream would otherwise read as.
 */
async function quitGhostlight() {
  if (!transport.available) return;
  const answer = await transport.quit();
  view.leaving(answer?.message ?? "");
}

/** Put the address of the watch view on the clipboard of this screen. */
async function copyAddress(address) {
  try {
    await transport.copyText(address);
    view.toast(ADDRESS_COPIED);
  } catch (error) {
    view.toast(String(error), true);
  }
}

/** The text a client's Copy button puts on the clipboard, from what this page already holds. */
function harnessText(id, kind) {
  const harness = store.snapshot()?.harnesses.find((item) => item.id === id);
  if (!harness) throw new Error(`no client ${id}`);
  return kind === COPY_KIND.command ? harness.connector_command : harness.manual_setup;
}

async function handleHarnessAction(button) {
  if (!transport.available || button.disabled) return;
  const { harness: id, harnessOperation: operation, harnessAction: action, harnessState: state,
    harnessName: name, harnessTarget: target, product: productId, copyKind: copyKind } = button.dataset;
  if (operation === "manage" && action === "uninstall" && !(await view.confirmRemoval(name))) return;
  if (operation === "manage" && action === "fix" && !(await view.confirmFix(name, target))) return;
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
      if (typeof transport.copyText === "function") {
        await transport.copyText(harnessText(id, copyKind));
        view.toast(copyKind === COPY_KIND.command ? COPIED_COMMAND : COPIED_SETUP);
      } else {
        view.toast(await transport.copyHarnessText(id, copyKind));
      }
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
      await settled(result);
    } else {
      const result = await transport.manageHarness(id, action, state);
      view.toast(result.message);
      await settled(result);
    }
  } catch (error) {
    // A client that changed since it was drawn is not a client that could not be set up.
    if (operation === "manage" && error?.code !== globalThis.GhostlightTransport.STALE) view.openHarnessManual(id);
    view.toast(String(error), true);
    await resync();
  } finally {
    store.endHarness(id);
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

/**
 * Every destination opens in the browser you already use, which is the one Ghostlight drives.
 *
 * The shell asks the service to open it. A browser tab opens the address the service listed, as
 * an ordinary link, and sends nothing: the page holds no address of its own.
 */
async function openDestination(destination) {
  if (context !== null && context.opener !== OPENER_SERVICE) {
    const address = context.links?.[destination];
    if (address) window.open(address, "_blank", "noopener,noreferrer");
    return;
  }
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
    if (done) view.toast(done);
  } catch (error) {
    view.toast(String(error), true);
  } finally {
    button.disabled = false;
  }
}

/* ------------------------------- navigating ----------------------------- */

/**
 * Show a destination, and for Settings one of its sections, and bring up to date what the person
 * is about to see. A tab, a rail entry and an address all arrive here.
 */
function go(name, section) {
  view.navigate(name, section);
  if (name === "policy" && showsData() && !view.draftIsDirty()) {
    outdated.delete("policy");
    loadPolicy();
  }
  openedView(name);
}

/**
 * Open the page an address names.
 *
 * Text that is not an address opens the default destination and is replaced by its address. A
 * page that loads with no address stays as it is; one whose address was emptied opens the default.
 */
function openAddress(hash, loading) {
  const wanted = parseAddress(hash);
  if (wanted) go(wanted.view, wanted.section ?? undefined);
  else if (hash || !loading) go(DEFAULT_VIEW);
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
    if (confirmation && view.answerConfirmation(confirmation.dataset.confirm === "accept")) return;
    const tab = event.target.closest("[data-view]");
    if (tab) go(tab.dataset.view);
    const entry = event.target.closest("[data-settings-section]");
    if (entry) go("settings", entry.dataset.settingsSection);
    const ruleAction = event.target.closest("[data-rule-action]");
    if (ruleAction && !ruleAction.disabled) {
      view.ruleAction(Number(ruleAction.dataset.rule), ruleAction.dataset.ruleAction);
      return;
    }
    // Opening a rule is the whole interaction for one nobody may change, and the way in for one
    // they may. An action inside an open rule is handled above and must not close it again.
    const toggle = event.target.closest("[data-rule-toggle]");
    if (toggle) view.toggleRule(toggle.dataset.ruleToggle);
    // The wheel is the band's own control; Settings wires the buttons it holds.
    const wheel = event.target.closest("#wheel");
    if (wheel && !wheel.disabled) applyIntent(wheel.dataset.intent);
    const harness = event.target.closest("[data-harness-operation]");
    if (harness) handleHarnessAction(harness);
    const carryOn = event.target.closest("[data-continue]");
    if (carryOn) continueInWindow();
    const destination = event.target.closest("[data-destination]");
    if (destination) openDestination(destination.dataset.destination);
    const hit = event.target.closest("[data-search-view]");
    if (hit) {
      view.navigate(SEARCH_VIEWS[hit.dataset.searchView] ?? "monitor");
      view.closePalette();
    }
    if (event.target === el.palette) view.closePalette();
  });

  window.addEventListener("hashchange", () => openAddress(window.location.hash, false));

  el["palette-query"].addEventListener("input", () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => search(view.paletteQuery()), 140);
  });

  document.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      if (view.paletteOpen()) view.closePalette();
      else if (showsData()) view.openPalette();
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
    const choice = event.target.closest("[data-setting-choice]");
    if (choice) {
      view.setChoice(choice.dataset.settingChoice, choice.value);
      return;
    }
    const box = event.target.closest("[data-restriction]");
    // The checkbox is the permission as the person sees it: checked means allowed. setPermission
    // is the seam that turns that back into the tightening-only value the schema can express.
    if (box) view.setPermission(box.dataset.restriction, box.checked);
  });

  el["sacred-hosts"].addEventListener("input", (event) => view.setSacred(event.target.value));
  el["add-rule"].addEventListener("click", () => view.addRule());
  el["check-policy"].addEventListener("click", () => checkPolicy());
  // A screen that cannot write the rules takes the draft to where they are written, and keeps it.
  el["apply-policy"].addEventListener("click", () => (allowed(ACT.applyPolicy) ? applyPolicy() : continueInWindow()));
  el["discard-policy"].addEventListener("click", () => discardPolicyDraft());
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
    withButton(event, rescan, "MCP clients re-checked."));

  el["setup-integrations"].addEventListener("click", async (event) => {
    const button = event.currentTarget;
    button.disabled = true;
    try {
      const result = await transport.setupDetectedHarnesses();
      await settled(result);
      view.toast(result.message, result.failures.length > 0);
    } catch (error) {
      view.toast(String(error), true);
    } finally {
      button.disabled = button.dataset.actionable !== "true";
    }
  });

  // Only the native shell has no live stream that tells it when it was away, so only the native
  // shell asks again when it comes back into view.
  if (native) {
    document.addEventListener("visibilitychange", () => {
      if (!document.hidden) resync();
    });
  }

  setInterval(() => {
    if (!document.hidden) view.tickElapsed(store.feed());
  }, 100);
  setInterval(() => {
    if (!document.hidden) view.tickAges(store.feed());
  }, 15000);
}

/* ------------------------------- the stream ----------------------------- */

/** Watching turned off under a page that was watching: it shows what a closed page shows. */
async function becomeClosed(sentence) {
  stopStream?.();
  stopStream = null;
  let told = null;
  try {
    told = await transport.context();
  } catch (error) {
    told = null;
  }
  const base = told?.context === CONTEXT_CLOSED ? told : { ...context, context: CONTEXT_CLOSED };
  context = { ...base, sentence: sentence ?? base.sentence };
  view.closed(context);
  attempt("closing Settings", () => settings.closed(context.sentence));
}

const streamHandlers = {
  change: (message) => receiveChange(message.payload),
  hello(data) {
    const reconnected = helloSeen;
    helloSeen = true;
    // After a stream was lost the cache cannot be trusted. On the first hello it can be, unless
    // the snapshot was taken before the stream opened and changes lie between the two.
    if (reconnected || (store.snapshot() && data.seq > store.seq())) {
      resync({ rebuildFeed: true });
      return;
    }
    helloSeq = data.seq;
  },
  reset: () => resync({ rebuildFeed: true }),
  closed: (data) => becomeClosed(data?.sentence),
  reconnecting: (on) => view.reconnecting(on)
};

let stopStream = null;

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
async function start() {
  try {
    context = await transport.context();
  } catch (error) {
    store.setConnected(false);
    reportFailure("reading what this page may do", error);
    return;
  }
  if (context.context === CONTEXT_CLOSED) {
    view.closed(context);
    attempt("closing Settings", () => settings.closed(context.sentence));
    attempt("opening the address", () => openAddress(window.location.hash, true));
    return;
  }
  attempt("telling the page what it may do", () => view.context(context));
  attempt("opening the address", () => openAddress(window.location.hash, true));
  attempt("subscribing to changes", () => {
    stopStream = transport.subscribe(streamHandlers);
  });
  attempt("first snapshot", () => resync({ rebuildFeed: true }));
}

function boot() {
  // The native shell's heartbeat is its own recovery, so it is installed before anything that can
  // fail. A bad subscription or a bad first snapshot then costs one cycle rather than the window.
  // A page on the live stream has no heartbeat: the stream tells it when it was away.
  if (native) {
    setInterval(() => {
      if (!document.hidden) resync();
    }, HEARTBEAT_MS);
  }
  attempt("wiring the surface", wire);
  attempt("wiring Settings", () => settings.wire());
  attempt("starting", () => {
    start();
  });
  attempt("about card", () => view.armCard());
}

// Anything that escapes a listener or a promise still has to reach the person using the window.
window.addEventListener("error", (event) => reportFailure("surface", event.error ?? event.message));
window.addEventListener("unhandledrejection", (event) => reportFailure("surface", event.reason));

boot();
