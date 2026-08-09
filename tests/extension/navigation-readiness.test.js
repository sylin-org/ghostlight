// SPDX-License-Identifier: Apache-2.0 OR MIT

const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const {
  attachNavigation,
  createNavigationReadiness,
  normalizeReadiness,
} = require("../../extension/lib/navigation-readiness.js");

function topFrame(url, overrides = {}) {
  return {
    id: "top-frame",
    loaderId: `loader-${url}`,
    url,
    securityOrigin: new URL(url).origin,
    mimeType: "text/html",
    ...overrides,
  };
}

function harness(t, options = {}) {
  let clock = 100;
  let tokenSequence = 0;
  let documentSequence = 0;
  const coordinator = createNavigationReadiness({
    now: () => clock,
    makeToken: () => `n_${++tokenSequence}`,
    makeDocument: () => `d_${++documentSequence}`,
    retentionMs: 60000,
    ...options,
  });
  t.after(() => coordinator.clear());
  return {
    coordinator,
    now: () => clock,
    setNow: (value) => { clock = value; },
  };
}

function dispatch(h, tab, readiness = {}, initialUrl = "https://before.example/") {
  const armed = h.coordinator.arm(tab, readiness, topFrame(initialUrl));
  const dispatched = h.coordinator.markDispatched(armed.navigation_token);
  return { ...armed, ...dispatched };
}

async function commit(h, tab, navigation, url) {
  assert.equal(h.coordinator.frameNavigated(tab, topFrame(url)), true);
  return h.coordinator.waitForCommit(navigation.navigation_token);
}

function input(tab, navigation, committed) {
  return {
    tab,
    navigation_token: navigation.navigation_token,
    document_handle: committed.document_handle,
  };
}

test("readiness defaults and bounds are canonical and corrective", () => {
  assert.deepEqual(normalizeReadiness(), {
    settle: true,
    timeout_ms: 10000,
    min_ms: 0,
  });
  assert.deepEqual(normalizeReadiness({ settle: false, timeout_ms: 30000, min_ms: 30000 }), {
    settle: false,
    timeout_ms: 30000,
    min_ms: 30000,
  });
  for (const invalid of [
    null,
    [],
    { settle: "yes" },
    { timeout_ms: 0 },
    { timeout_ms: -1 },
    { timeout_ms: 30001 },
    { timeout_ms: 1.5 },
    { timeout_ms: 5, min_ms: 6 },
  ]) {
    assert.throws(() => normalizeReadiness(invalid), Error, JSON.stringify(invalid));
  }
});

test("a transaction can retain the physical dispatch time learned before tab identity", (t) => {
  const h = harness(t);
  h.setNow(250);
  const armed = h.coordinator.arm(9, { timeout_ms: 1000 }, null);
  const dispatched = h.coordinator.markDispatched(armed.navigation_token, 100);
  assert.equal(dispatched.deadline_at_ms, 1100);
});

test("default wire handles and committed URLs match the bounded core grammar", async (t) => {
  const coordinator = createNavigationReadiness({ retentionMs: 60000 });
  t.after(() => coordinator.clear());
  const armed = coordinator.arm(21, {}, topFrame("https://before.example/"));
  assert.match(armed.navigation_token, /^n_[\x21-\x7e]{1,126}$/);
  coordinator.markDispatched(armed.navigation_token);
  assert.equal(coordinator.frameNavigated(21, topFrame("https://after.example/")), true);
  const committed = await coordinator.waitForCommit(armed.navigation_token);
  assert.match(committed.document_handle, /^d_[\x21-\x7e]{1,126}$/);

  const other = harness(t);
  for (const [tab, url] of [
    [23, `https://example.com/${"x".repeat(4096)}`],
    [24, `https://example.com/${"e".repeat(4050)}${"\u00e9".repeat(20)}`],
  ]) {
    const bounded = dispatch(other, tab);
    assert.equal(other.coordinator.frameNavigated(tab, topFrame(url)), false);
    assert.equal(
      (await other.coordinator.waitForCommit(bounded.navigation_token)).state,
      "landing_unknown"
    );
  }
});

test("the watcher is armed before dispatch and records only top-level commits", async (t) => {
  const h = harness(t);
  const armed = h.coordinator.arm(7, {}, topFrame("https://before.example/"));

  // An event before physical dispatch is not evidence for this operation.
  assert.equal(h.coordinator.frameNavigated(7, topFrame("https://too-early.example/")), false);
  const dispatched = h.coordinator.markDispatched(armed.navigation_token);
  assert.equal(dispatched.deadline_at_ms, 10100);
  assert.equal(h.coordinator.frameNavigated(7, {
    ...topFrame("https://iframe.example/"),
    id: "child",
    parentId: "top-frame",
  }), false);
  assert.equal(h.coordinator.frameNavigated(7, topFrame("https://allowed.example/")), true);

  const result = await h.coordinator.waitForCommit(armed.navigation_token);
  assert.deepEqual(result, {
    state: "committed",
    navigation_token: "n_1",
    deadline_at_ms: 10100,
    elapsed_ms: 0,
    document_handle: "d_1",
    url: "https://allowed.example/",
  });
  assert.doesNotMatch(JSON.stringify(result), /loader-|top-frame|security_origin|mime_type/);
});

test("rapid redirects are journaled in order and overflow fails closed", async (t) => {
  const h = harness(t, { maxCommits: 3 });
  const navigation = dispatch(h, 4);
  for (const url of ["https://one.example/", "https://two.example/", "https://three.example/"]) {
    assert.equal(h.coordinator.frameNavigated(4, topFrame(url)), true);
  }
  const first = await h.coordinator.waitForCommit(navigation.navigation_token);
  const second = await h.coordinator.awaitReadiness(input(4, navigation, first), async () => {
    throw new Error("must not observe an older document while a commit is queued");
  });
  const third = await h.coordinator.awaitReadiness(input(4, navigation, second), async () => {
    throw new Error("must not observe an older document while a commit is queued");
  });
  assert.deepEqual([first.url, second.url, third.url], [
    "https://one.example/",
    "https://two.example/",
    "https://three.example/",
  ]);

  assert.equal(h.coordinator.frameNavigated(4, topFrame("https://overflow.example/")), false);
  const unknown = await h.coordinator.awaitReadiness(
    input(4, navigation, third),
    async () => ({ settled: true })
  );
  assert.equal(unknown.state, "landing_unknown");
  assert.equal(unknown.document_handle, undefined);
  assert.equal(unknown.url, undefined);
});

test("invalid top-frame commit evidence reports an unknown landing", async (t) => {
  const h = harness(t);
  const navigation = dispatch(h, 40);
  assert.equal(h.coordinator.frameNavigated(40, { id: "top", url: "" }), false);
  const unknown = await h.coordinator.waitForCommit(navigation.navigation_token);
  assert.equal(unknown.state, "landing_unknown");
  assert.equal(unknown.document_handle, undefined);
  assert.equal(unknown.url, undefined);
});

test("a first commit observed after the original deadline is never promoted to success", async (t) => {
  const h = harness(t);
  const navigation = dispatch(h, 16, { timeout_ms: 10 });
  h.setNow(111);
  assert.equal(
    h.coordinator.frameNavigated(16, topFrame("https://late.example/")),
    false
  );
  const result = await h.coordinator.waitForCommit(navigation.navigation_token);
  assert.equal(result.state, "timed_out");
  assert.equal(result.document_handle, undefined);
  assert.equal(result.deadline_at_ms, 110);
});

test("a redirect observed after the original deadline is never promoted", async (t) => {
  const h = harness(t);
  const navigation = dispatch(h, 20, { timeout_ms: 10 });
  const first = await commit(h, 20, navigation, "https://first.example/");
  h.setNow(111);
  assert.equal(
    h.coordinator.frameNavigated(20, topFrame("https://late-redirect.example/")),
    true
  );
  const changed = await h.coordinator.awaitReadiness(
    input(20, navigation, first),
    async () => ({ settled: true })
  );
  assert.equal(changed.state, "committed");
  assert.equal(changed.url, "https://late-redirect.example/");
  const result = await h.coordinator.awaitReadiness(
    input(20, navigation, changed),
    async () => ({ settled: true })
  );
  assert.equal(result.state, "timed_out");
  assert.equal(result.document_handle, changed.document_handle);
  assert.equal(result.url, "https://late-redirect.example/");
  assert.equal(result.deadline_at_ms, 110);
});

test("a queued timely commit is delivered before a later post-deadline redirect", async (t) => {
  const h = harness(t);
  const navigation = dispatch(h, 24, { timeout_ms: 10 });
  assert.equal(
    h.coordinator.frameNavigated(24, topFrame("https://timely.example/")),
    true
  );
  h.setNow(111);
  assert.equal(
    h.coordinator.frameNavigated(24, topFrame("https://late.example/")),
    true
  );

  const timely = await h.coordinator.waitForCommit(navigation.navigation_token);
  assert.equal(timely.state, "committed");
  assert.equal(timely.url, "https://timely.example/");
  const late = await h.coordinator.awaitReadiness(
    input(24, navigation, timely),
    async () => ({ settled: true })
  );
  assert.equal(late.state, "committed");
  assert.equal(late.url, "https://late.example/");
  const timedOut = await h.coordinator.awaitReadiness(
    input(24, navigation, late),
    async () => ({ settled: true })
  );
  assert.equal(timedOut.state, "timed_out");
  assert.equal(timedOut.url, "https://late.example/");
});

test("same-document navigation gets a fresh opaque document handle", async (t) => {
  const h = harness(t);
  const navigation = dispatch(h, 5);
  const first = await commit(h, 5, navigation, "https://example.com/start");
  assert.equal(h.coordinator.navigatedWithinDocument(5, {
    frameId: "top-frame",
    url: "https://example.com/next#section",
  }), true);
  assert.equal(h.coordinator.navigatedWithinDocument(5, {
    frameId: "child",
    url: "https://example.com/ignored",
  }), false);
  const next = await h.coordinator.awaitReadiness(
    input(5, navigation, first),
    async () => ({ settled: true })
  );
  assert.equal(next.state, "committed");
  assert.equal(next.url, "https://example.com/next#section");
  assert.notEqual(next.document_handle, first.document_handle);
});

test("invalid top-frame same-document evidence makes the landing unknown", async (t) => {
  const h = harness(t);
  const navigation = dispatch(h, 25);
  const first = await commit(h, 25, navigation, "https://allowed.example/start");
  assert.equal(h.coordinator.navigatedWithinDocument(25, {
    frameId: "top-frame",
    url: `https://denied.example/${"x".repeat(4097)}`,
  }), false);
  const unknown = await h.coordinator.awaitReadiness(
    input(25, navigation, first),
    async () => ({ settled: true })
  );
  assert.equal(unknown.state, "landing_unknown");
  assert.equal(unknown.document_handle, undefined);
  assert.equal(unknown.url, undefined);
});

test("ready, timed_out, unavailable, and not_requested remain separate", async (t) => {
  async function run(tab, readiness, observe) {
    const h = harness(t);
    const navigation = dispatch(h, tab, readiness);
    const committed = await commit(h, tab, navigation, `https://case-${tab}.example/`);
    return h.coordinator.awaitReadiness(input(tab, navigation, committed), observe);
  }

  assert.equal((await run(1, {}, async () => ({ settled: true }))).state, "ready");
  assert.equal((await run(2, {}, async () => ({ timeout: true }))).state, "timed_out");
  assert.equal((await run(3, {}, async () => {
    throw new Error("content script unavailable");
  })).state, "unavailable");

  let observed = false;
  const notRequested = await run(6, { settle: false }, async () => {
    observed = true;
    return { settled: true };
  });
  assert.equal(notRequested.state, "not_requested");
  assert.equal(observed, false);
});

test("PDF classification follows observability, never a URL suffix", async (t) => {
  const h1 = harness(t);
  const nav1 = dispatch(h1, 17);
  h1.coordinator.frameNavigated(17, topFrame("https://files.example/report.pdf"));
  const pdfNamed = await h1.coordinator.waitForCommit(nav1.navigation_token);
  const ready = await h1.coordinator.awaitReadiness(
    input(17, nav1, pdfNamed),
    async () => ({ settled: true })
  );
  assert.equal(ready.state, "ready");

  const h2 = harness(t);
  const nav2 = dispatch(h2, 18);
  h2.coordinator.frameNavigated(18, topFrame("https://files.example/download", {
    mimeType: "application/pdf",
  }));
  const actualPdf = await h2.coordinator.waitForCommit(nav2.navigation_token);
  const unavailable = await h2.coordinator.awaitReadiness(
    input(18, nav2, actualPdf),
    async () => { throw new Error("content script unavailable"); }
  );
  assert.equal(unavailable.state, "unavailable");
});

test("the original absolute deadline and minimum survive delayed follow-ups", async (t) => {
  const h = harness(t);
  const navigation = dispatch(h, 8, { timeout_ms: 1000, min_ms: 700 });
  const committed = await commit(h, 8, navigation, "https://deadline.example/");
  h.setNow(500);
  let observedSpec;
  const result = await h.coordinator.awaitReadiness(
    input(8, navigation, committed),
    async (spec) => {
      observedSpec = spec;
      return { settled: true };
    }
  );
  assert.deepEqual(observedSpec, { settle: true, timeout_ms: 600, min_ms: 300 });
  assert.equal(result.deadline_at_ms, 1100);
  assert.equal(result.elapsed_ms, 400);
  assert.equal(result.state, "ready");
});

test("settlement observed after the original deadline cannot become ready", async (t) => {
  const h = harness(t);
  const navigation = dispatch(h, 19, { timeout_ms: 100 });
  const committed = await commit(h, 19, navigation, "https://deadline.example/");
  const result = await h.coordinator.awaitReadiness(
    input(19, navigation, committed),
    async () => {
      h.setNow(201);
      return { settled: true };
    }
  );
  assert.equal(result.state, "timed_out");
  assert.equal(result.deadline_at_ms, 200);
});

test("a new commit interrupts old-document settlement", async (t) => {
  const h = harness(t);
  const navigation = dispatch(h, 9);
  const first = await commit(h, 9, navigation, "https://first.example/");
  let observeStarted;
  const started = new Promise((resolve) => { observeStarted = resolve; });
  const waiting = h.coordinator.awaitReadiness(input(9, navigation, first), async () => {
    observeStarted();
    return new Promise(() => {});
  });
  await started;
  h.setNow(250);
  h.coordinator.frameNavigated(9, topFrame("https://redirect.example/"));
  const changed = await waiting;
  assert.equal(changed.state, "committed");
  assert.equal(changed.url, "https://redirect.example/");
  assert.equal(changed.deadline_at_ms, 10100);
});

test("verification returns same, surfaces missed commits, and fails closed on mismatch", async (t) => {
  const h1 = harness(t);
  const nav1 = dispatch(h1, 10);
  const doc1 = await commit(h1, 10, nav1, "https://same.example/");
  const same = h1.coordinator.verify(input(10, nav1, doc1), {
    frame: topFrame("https://same.example/"),
    target_url: "https://same.example/",
  });
  assert.equal(same.state, "same");
  assert.equal(h1.coordinator.activeCount(), 0);

  const hMissing = harness(t);
  const navMissing = dispatch(hMissing, 20);
  const docMissing = await commit(
    hMissing,
    20,
    navMissing,
    "https://missing-target.example/"
  );
  const missingTarget = hMissing.coordinator.verify(
    input(20, navMissing, docMissing),
    { frame: topFrame("https://missing-target.example/") }
  );
  assert.equal(missingTarget.state, "unavailable");

  const h2 = harness(t);
  const nav2 = dispatch(h2, 11);
  const doc2 = await commit(h2, 11, nav2, "https://before-change.example/");
  const changed = h2.coordinator.verify(input(11, nav2, doc2), {
    frame: topFrame("https://missed-change.example/"),
    target_url: "https://missed-change.example/",
  });
  assert.equal(changed.state, "committed");
  assert.equal(changed.url, "https://missed-change.example/");

  const unavailable = h2.coordinator.verify(input(11, nav2, changed), {
    frame: topFrame("https://missed-change.example/"),
    target_url: "https://different.example/",
  });
  assert.equal(unavailable.state, "unavailable");
  assert.equal(h2.coordinator.activeCount(), 0);
});

test("detach, tab destruction, and supersession retire state without optimistic proof", async (t) => {
  const h = harness(t);
  const beforeCommit = dispatch(h, 12);
  assert.equal(h.coordinator.watcherUnavailable(12), true);
  assert.equal((await h.coordinator.waitForCommit(beforeCommit.navigation_token)).state, "unavailable");

  const afterCommit = dispatch(h, 13);
  const document = await commit(h, 13, afterCommit, "https://protected.example/");
  assert.equal(h.coordinator.watcherUnavailable(13), true);
  assert.equal((await h.coordinator.awaitReadiness(
    input(13, afterCommit, document),
    async () => ({ settled: true })
  )).state, "unavailable");

  const destroyed = dispatch(h, 14);
  assert.equal(h.coordinator.destroyTab(14), true);
  await assert.rejects(
    h.coordinator.waitForCommit(destroyed.navigation_token),
    /navigation token is unavailable/
  );

  const old = dispatch(h, 15);
  const replacement = h.coordinator.arm(15, {}, topFrame("https://replacement.example/"));
  assert.equal(replacement.navigation_token, "n_5");
  await assert.rejects(
    h.coordinator.waitForCommit(old.navigation_token),
    /navigation token is unavailable/
  );
});

test("feature evidence is additive to exact legacy navigate and reload result shapes", () => {
  const evidence = {
    state: "committed",
    navigation_token: "n_1",
    document_handle: "d_1",
    url: "https://example.com/",
    deadline_at_ms: 10100,
    elapsed_ms: 25,
  };
  for (const original of [
    {
      content: [{ type: "text", text: "Navigated to https://example.com/." }],
      structuredContent: {
        tabId: 7,
        url: "https://example.com/",
        title: "Example",
      },
    },
    {
      content: [{ type: "text", text: "Tab reload observed." }],
      structuredContent: {
        interactionReceipt: {
          action: "reload",
          observedAfter: { tabId: 7, reloaded: true },
        },
      },
    },
  ]) {
    const before = structuredClone(original);
    const result = attachNavigation(original, evidence);
    assert.deepEqual(result.content, before.content);
    const { navigation, ...nonNavigation } = result.structuredContent;
    assert.deepEqual(navigation, evidence);
    assert.deepEqual(nonNavigation, before.structuredContent);
  }
});

test("service-worker source keeps the readiness watcher ahead of physical navigation", () => {
  const worker = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  assert.match(worker, /"lib\/navigation-readiness\.js"/);
  assert.match(worker, /navigationReadiness\.markDispatched\(armed\.navigation_token\)/);
  const start = worker.indexOf("async navigate_readiness_start");
  const enabled = worker.indexOf('enableDomain(tabId, "Page")', start);
  const armed = worker.indexOf("navigationReadiness.arm(tabId", start);
  const marked = worker.indexOf(
    "navigationReadiness.markDispatched(armed.navigation_token)",
    start
  );
  const dispatched = worker.indexOf("await runNavigationPlan(tabId, plan)", start);
  assert.ok(start >= 0 && enabled > start && armed > enabled && marked > armed &&
    dispatched > marked);
  assert.match(worker, /return chrome\.tabs\.update\(tabId, \{ url: plan\.url \}\)/);
  const featureStart = worker.slice(start, worker.indexOf("async navigation_readiness_await", start));
  assert.doesNotMatch(featureStart, /waitForLoad\(/);
  assert.match(worker, /method === "Page\.frameNavigated"/);
  assert.match(worker, /method === "Page\.navigatedWithinDocument"/);
});

test("URL tab creation is one observed physical transaction with no blank-page hop", () => {
  const worker = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  const start = worker.indexOf("async tabs_open_mcp");
  const end = worker.indexOf("async navigate(", start);
  const handler = worker.slice(start, end);
  const observer = handler.indexOf("createOpenNavigationObserver()");
  const dispatchedAt = handler.indexOf("const dispatchedAt = performance.now()");
  const create = handler.indexOf("createTabInSessionGroup(key, workspaceRequest, plan.url)");
  const arm = handler.indexOf("navigationReadiness.arm(tab.id");
  const mark = handler.indexOf(
    "navigationReadiness.markDispatched(armed.navigation_token, dispatchedAt)"
  );
  assert.ok(start >= 0 && observer >= 0 && dispatchedAt > observer && create > dispatchedAt);
  assert.ok(arm > create && mark > arm, "tab identity binds to the pre-create dispatch clock");
  assert.doesNotMatch(handler, /tabs\.update|about:blank|waitForLoad/);
  assert.match(worker, /chrome\.webNavigation\.onCommitted\.addListener\(record\)/);

  const manifest = JSON.parse(fs.readFileSync(
    path.join(__dirname, "../../extension/manifest.json"),
    "utf8"
  ));
  assert.ok(manifest.permissions.includes("webNavigation"));
});

test("typed navigation rejects an invalid URL before arming or dispatching", () => {
  const worker = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  const start = worker.indexOf("async navigate_readiness_start");
  const end = worker.indexOf("async navigation_readiness_await", start);
  const handler = worker.slice(start, end);
  const plan = handler.indexOf("const plan = navigationDispatchPlan(a)");
  const rejection = handler.indexOf(
    'if (plan.error) throw hopError("navigation", plan.error)'
  );
  const arm = handler.indexOf("navigationReadiness.arm");
  const dispatch = handler.indexOf("runNavigationPlan");
  assert.ok(plan >= 0 && rejection > plan, "invalid plans are rejected explicitly");
  assert.ok(rejection < arm && rejection < dispatch, "rejection precedes arm and dispatch");
});

test("service worker preserves committed evidence when tab metadata lookup fails", () => {
  const worker = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  const fallback = worker.indexOf("async function committedNavigationResult");
  const metadata = worker.indexOf("return await legacyNavigateResult(tabId)", fallback);
  const bounded = worker.indexOf("result.structuredContent = { tabId }", fallback);
  const caller = worker.indexOf("await committedNavigationResult(tabId, evidence)");
  assert.ok(fallback >= 0 && metadata > fallback && bounded > metadata && caller > bounded);
  assert.match(worker.slice(fallback, caller), /typeof evidence\.url === "string"/);
});
