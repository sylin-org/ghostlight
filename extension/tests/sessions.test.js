"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const sessions = require("../lib/sessions.js");
const protocol = require("../lib/protocol.js");

const installer = async () => {};
const noEvents = { addListener() {} };

function fakeDebugger() {
  const calls = [];
  const focus = new Map();
  const listeners = [];
  return {
    calls,
    focus,
    onEvent: { addListener(listener) { listeners.push(listener); } },
    emit(source, method, params) { for (const listener of listeners) listener(source, method, params); },
    async attach(target, version) { calls.push(["attach", target.tabId, version]); },
    async detach(target) { calls.push(["detach", target.tabId]); focus.delete(target.tabId); },
    async sendCommand(target, method, params) {
      calls.push(["command", target.tabId, method]);
      if (method === "Emulation.setFocusEmulationEnabled") focus.set(target.tabId, params.enabled);
    }
  };
}

test("concurrent debugger users share one attachment", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });

  await Promise.all([lifecycle.acquire(7), lifecycle.acquire(7)]);
  assert.deepEqual(chromeDebugger.calls, [
    ["attach", 7, "1.3"],
    ["command", 7, "Page.enable"],
    ["command", 7, "Target.setAutoAttach"]
  ]);
  assert.equal(lifecycle.attachedCount(), 1);

  await lifecycle.release(7);
  assert.equal(lifecycle.attachedCount(), 1);
  await lifecycle.release(7);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(chromeDebugger.calls.at(-1), ["detach", 7]);
});

test("withLease gives the lease back however its work ends", async () => {
  const lifecycle = sessions.create(fakeDebugger(), { installer });
  assert.equal(await lifecycle.withLease(7, async () => lifecycle.attachedCount()), 1);
  assert.equal(lifecycle.attachedCount(), 0);
  await assert.rejects(lifecycle.withLease(7, async () => { throw new Error("boom"); }), /boom/);
  assert.equal(lifecycle.attachedCount(), 0, "a thrown work still releases");
});

test("a retained controlled tab stays attached between sequential operations", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });

  await lifecycle.retain(8);
  await lifecycle.acquire(8);
  await lifecycle.release(8);
  await lifecycle.acquire(8);
  await lifecycle.release(8);

  assert.equal(lifecycle.attachedCount(), 1);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "attach"), [["attach", 8, "1.3"]]);
  assert.equal(chromeDebugger.calls.filter(([kind]) => kind === "detach").length, 0);

  lifecycle.forget(8);
  assert.equal(lifecycle.attachedCount(), 0);
});

function iframeAttached(chromeDebugger, tabId, sessionId, extra = {}) {
  chromeDebugger.emit(
    { tabId },
    "Target.attachedToTarget",
    { sessionId, targetInfo: { type: "iframe", targetId: `target-${sessionId}` }, waitingForDebugger: true, ...extra }
  );
}

test("sessions require the Chrome debugger API and an installer", () => {
  assert.throws(() => sessions.create({}, { installer }), /Chrome debugger API/);
  assert.throws(() => sessions.create(fakeDebugger()), /installer/);
  assert.throws(() => sessions.create(fakeDebugger(), { installer: "script" }), /installer/);
});

test("a method outside the CDP allowlist is refused with browser_refused and never sent", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await assert.rejects(lifecycle.cdp({ tabId: 40 }, "Debugger.enable"), (error) => {
    assert.equal(error.code, "browser_refused");
    assert.match(error.message, /Debugger\.enable/);
    return true;
  });
  await lifecycle.retain(40);
  await assert.rejects(lifecycle.enableDomain(40, "DOM"), { code: "browser_refused" });
  assert.equal(chromeDebugger.calls.some(([, , method]) => method === "Debugger.enable" || method === "DOM.enable"), false);
  await lifecycle.detachAll();
});

test("an allowed method reaches the debugger with its target and parameters", async () => {
  const sent = [];
  const chromeDebugger = fakeDebugger();
  chromeDebugger.sendCommand = async (target, method, params) => { sent.push({ target, method, params }); return { ok: true }; };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  const reply = await lifecycle.cdp({ tabId: 41, sessionId: "s" }, "Page.getFrameTree", { a: 1 });
  assert.deepEqual(reply, { ok: true });
  assert.deepEqual(sent, [{ target: { tabId: 41, sessionId: "s" }, method: "Page.getFrameTree", params: { a: 1 } }]);
});

test("the allowlist is the closed protocol 4 list and refusal is a known receipt code", () => {
  assert.equal(Object.isFrozen(sessions.CDP_ALLOW), true);
  assert.equal(new Set(sessions.CDP_ALLOW).size, sessions.CDP_ALLOW.length);
  for (const method of ["Runtime.enable", "Page.createIsolatedWorld", "Input.cancelDragging", "Accessibility.queryAXTree"]) {
    assert.ok(sessions.CDP_ALLOW.includes(method), method);
  }
  // The trust boundary: no script debugging, no network interception, no navigation to a url the
  // service has not authorized, no compiling outside a page call, and no attaching to other targets.
  for (const method of ["Debugger.enable", "Fetch.enable", "Page.navigate", "Runtime.compileScript", "Target.attachToTarget"]) {
    assert.equal(sessions.CDP_ALLOW.includes(method), false, method);
  }
  assert.ok(sessions.CDP_ALLOW.every((method) => /^[A-Z][A-Za-z]+\.[a-z][A-Za-z]+$/.test(method)));
  assert.equal(protocol.isKnown("receipt_failures", protocol.failure("browser_refused", "x").code), true);
});

test("attaching never sends Runtime.enable and only enableDomain does", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.retain(42);
  iframeAttached(chromeDebugger, 42, "child-a");
  await lifecycle.acquire(42);
  await lifecycle.release(42);
  assert.equal(chromeDebugger.calls.some(([, , method]) => method === "Runtime.enable"), false);
  assert.equal(await lifecycle.enableDomain(42, "Runtime"), true);
  assert.equal(chromeDebugger.calls.filter(([, , method]) => method === "Runtime.enable").length, 1);
  await lifecycle.disableDomain(42, "Runtime");
  assert.equal(chromeDebugger.calls.filter(([, , method]) => method === "Runtime.disable").length, 1);
  await lifecycle.detachAll();
});

test("the installer registers the root session once per attachment", async () => {
  const chromeDebugger = fakeDebugger();
  const installed = [];
  const lifecycle = sessions.create(chromeDebugger, { installer: async (session) => { installed.push(session); } });
  await lifecycle.retain(43);
  await lifecycle.acquire(43);
  await lifecycle.enableDomain(43, "Network");
  assert.equal(installed.length, 1);
  assert.deepEqual(installed[0].target, { tabId: 43 });
  assert.equal(installed[0].targetId, null);
  assert.equal(installed[0], lifecycle.rootSession(43));
  await lifecycle.release(43);
  await lifecycle.unretain(43);
  assert.equal(lifecycle.attachedCount(), 0);
  await lifecycle.acquire(43);
  assert.equal(installed.length, 2);
  await lifecycle.detachAll();
});

test("the root is enabled, registered and then set to auto-attach iframes", async () => {
  const chromeDebugger = fakeDebugger();
  const order = [];
  const sendCommand = chromeDebugger.sendCommand;
  chromeDebugger.sendCommand = async (target, method, params) => {
    order.push(method);
    if (method === "Target.setAutoAttach") {
      assert.deepEqual(params, {
        autoAttach: true,
        waitForDebuggerOnStart: true,
        flatten: true,
        filter: [{ type: "iframe", exclude: false }]
      });
    }
    return sendCommand(target, method, params);
  };
  const lifecycle = sessions.create(chromeDebugger, { installer: async () => { order.push("installer"); } });
  await lifecycle.acquire(44);
  assert.deepEqual(order, ["Page.enable", "installer", "Target.setAutoAttach"]);
  await lifecycle.release(44);
});

test("a child session is recorded on attachedToTarget and dropped on detachedFromTarget", async () => {
  const chromeDebugger = fakeDebugger();
  const gone = [];
  const lifecycle = sessions.create(chromeDebugger, {
    installer,
    onSessionGone: (tabId, sessionId) => gone.push([tabId, sessionId])
  });
  await lifecycle.retain(45);
  assert.deepEqual(lifecycle.childSessions(45), []);

  iframeAttached(chromeDebugger, 45, "child-1");
  const [child] = lifecycle.childSessions(45);
  assert.deepEqual(child.target, { tabId: 45, sessionId: "child-1" });
  assert.equal(child.targetId, "target-child-1");
  assert.deepEqual(child.identifiers, []);

  chromeDebugger.emit({ tabId: 45 }, "Target.detachedFromTarget", { sessionId: "child-1" });
  assert.deepEqual(lifecycle.childSessions(45), []);
  assert.deepEqual(gone, [[45, "child-1"]]);
  await lifecycle.detachAll();
});

test("a child resumes only after Page.enable, the installer and the nested auto-attach", async () => {
  const chromeDebugger = fakeDebugger();
  const order = [];
  const sendCommand = chromeDebugger.sendCommand;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (target.sessionId) order.push(`${method}@${target.sessionId}`);
    return sendCommand(target, method, params);
  };
  const lifecycle = sessions.create(chromeDebugger, {
    installer: async (session) => { if (session.target.sessionId) order.push(`installer@${session.target.sessionId}`); }
  });
  await lifecycle.retain(46);
  iframeAttached(chromeDebugger, 46, "child-2");
  await lifecycle.acquire(46);
  assert.deepEqual(order, [
    "Page.enable@child-2",
    "installer@child-2",
    "Target.setAutoAttach@child-2",
    "Runtime.runIfWaitingForDebugger@child-2"
  ]);
  await lifecycle.detachAll();
});

test("a child that did not attach paused is not resumed", async () => {
  const chromeDebugger = fakeDebugger();
  const sent = [];
  const sendCommand = chromeDebugger.sendCommand;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (target.sessionId) sent.push(method);
    return sendCommand(target, method, params);
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.retain(47);
  iframeAttached(chromeDebugger, 47, "child-3", { waitingForDebugger: false });
  await lifecycle.acquire(47);
  assert.deepEqual(sent, ["Page.enable", "Target.setAutoAttach"]);
  await lifecycle.detachAll();
});

test("a child resumes last even when its installer fails, and the failure reaches the next attach", async () => {
  const chromeDebugger = fakeDebugger();
  const sent = [];
  const sendCommand = chromeDebugger.sendCommand;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (target.sessionId) sent.push(method);
    return sendCommand(target, method, params);
  };
  const lifecycle = sessions.create(chromeDebugger, {
    installer: async (session) => { if (session.target.sessionId) throw new Error("registration failed"); }
  });
  await lifecycle.retain(48);
  iframeAttached(chromeDebugger, 48, "child-4");
  await assert.rejects(lifecycle.acquire(48), /registration failed/);
  assert.deepEqual(sent, ["Page.enable", "Runtime.runIfWaitingForDebugger"]);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(lifecycle.childSessions(48), []);
  await lifecycle.detachAll();
});

test("a child that ended before its setup finished does not fail the tab", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (target.sessionId === "child-5" && method === "Page.enable") {
      chromeDebugger.emit({ tabId: 49 }, "Target.detachedFromTarget", { sessionId: "child-5" });
      throw new Error("session not found");
    }
    return sendCommand(target, method, params);
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.retain(49);
  iframeAttached(chromeDebugger, 49, "child-5");
  await lifecycle.acquire(49);
  assert.equal(lifecycle.attachedCount(), 1);
  assert.deepEqual(lifecycle.childSessions(49), []);
  await lifecycle.detachAll();
});

test("a nested iframe arrives through the same listener and other target types are ignored", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.retain(50);
  iframeAttached(chromeDebugger, 50, "child-6");
  chromeDebugger.emit({ tabId: 50, sessionId: "child-6" }, "Target.attachedToTarget", {
    sessionId: "grandchild", targetInfo: { type: "iframe", targetId: "target-grandchild" }, waitingForDebugger: true
  });
  chromeDebugger.emit({ tabId: 50 }, "Target.attachedToTarget", {
    sessionId: "worker-1", targetInfo: { type: "worker", targetId: "w" }, waitingForDebugger: false
  });
  await lifecycle.acquire(50);
  assert.deepEqual(lifecycle.childSessions(50).map((child) => child.target.sessionId), ["child-6", "grandchild"]);
  await lifecycle.detachAll();
});

test("attach events for an unattached or unknown tab are ignored", () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  iframeAttached(chromeDebugger, 51, "orphan");
  chromeDebugger.emit({}, "Target.attachedToTarget", { sessionId: "none", targetInfo: { type: "iframe" } });
  assert.deepEqual(lifecycle.childSessions(51), []);
  assert.equal(chromeDebugger.calls.length, 0);
});

test("onRelease runs before every voluntary detach and never before an external one", async () => {
  const chromeDebugger = fakeDebugger();
  const released = [];
  const lifecycle = sessions.create(chromeDebugger, {
    installer,
    onRelease: async (tabId) => { chromeDebugger.calls.push(["release", tabId]); released.push(tabId); }
  });
  const detaches = () => chromeDebugger.calls.filter(([kind]) => kind === "detach" || kind === "release").map(([kind, tab]) => `${kind}:${tab}`);

  await lifecycle.acquire(52);
  await lifecycle.release(52);
  await lifecycle.retain(53);
  await lifecycle.unretain(53);
  await Promise.all([lifecycle.retain(54), lifecycle.retain(55)]);
  await lifecycle.detachAll();
  assert.deepEqual(detaches().slice(0, 4), ["release:52", "detach:52", "release:53", "detach:53"]);
  assert.deepEqual(released.sort(), [52, 53, 54, 55]);
  for (const tab of [54, 55]) {
    const all = detaches();
    assert.ok(all.indexOf(`release:${tab}`) < all.indexOf(`detach:${tab}`));
  }

  await lifecycle.acquire(56);
  lifecycle.detached(56);
  assert.equal(released.includes(56), false);
  await lifecycle.release(56);
  await lifecycle.detachAll();
});

test("a failing onRelease does not prevent the detach", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, {
    installer,
    onRelease: async () => { throw new Error("world already gone"); }
  });
  await lifecycle.acquire(57);
  await lifecycle.release(57);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(chromeDebugger.calls.at(-1), ["detach", 57]);
});

test("onRelease still sees the sessions and their identifiers", async () => {
  const chromeDebugger = fakeDebugger();
  let seen;
  const lifecycle = sessions.create(chromeDebugger, {
    installer: async (session) => { session.identifiers.push(`id-${session.target.sessionId ?? "root"}`); },
    onRelease: async (tabId) => {
      seen = [lifecycle.rootSession(tabId), ...lifecycle.childSessions(tabId)].map((session) => session.identifiers.slice());
    }
  });
  await lifecycle.retain(58);
  iframeAttached(chromeDebugger, 58, "child-7");
  await lifecycle.acquire(58);
  await lifecycle.detachAll();
  assert.deepEqual(seen, [["id-root"], ["id-child-7"]]);
});

test("registration identifiers are recorded per session and die with the attachment", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, {
    installer: async (session) => { session.identifiers.push(`reg-${session.target.sessionId ?? "root"}`); }
  });
  await lifecycle.retain(59);
  iframeAttached(chromeDebugger, 59, "child-8");
  iframeAttached(chromeDebugger, 59, "child-9");
  await lifecycle.acquire(59);

  assert.deepEqual(lifecycle.rootSession(59).identifiers, ["reg-root"]);
  const [first, second] = lifecycle.childSessions(59);
  assert.deepEqual(first.identifiers, ["reg-child-8"]);
  assert.deepEqual(second.identifiers, ["reg-child-9"]);

  lifecycle.detached(59);
  assert.equal(lifecycle.rootSession(59), null);
  assert.deepEqual(lifecycle.childSessions(59), []);
  await lifecycle.acquire(59);
  assert.deepEqual(lifecycle.rootSession(59).identifiers, ["reg-root"]);
  await lifecycle.detachAll();
});

test("forEachSession visits the root and children of attached tabs and finishes after a failure", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.retain(60);
  await lifecycle.retain(61);
  iframeAttached(chromeDebugger, 60, "child-10");
  await lifecycle.acquire(60);
  await lifecycle.release(60);

  const visited = [];
  await lifecycle.forEachSession(async (session) => { visited.push([session.target.tabId, session.target.sessionId ?? null]); });
  assert.deepEqual(visited.sort(), [[60, null], [60, "child-10"], [61, null]]);

  const attempted = [];
  await assert.rejects(
    lifecycle.forEachSession(async (session) => {
      attempted.push(session.target.tabId);
      if (session.target.sessionId === "child-10") throw new Error("register failed");
    }),
    (error) => error instanceof AggregateError && error.errors.length === 1 && /register failed/.test(error.errors[0].message)
  );
  assert.equal(attempted.length, 3);

  lifecycle.detached(61);
  const remaining = [];
  await lifecycle.forEachSession(async (session) => { remaining.push(session.target.tabId); });
  assert.deepEqual(remaining, [60, 60]);
  await lifecycle.detachAll();
});

test("sessions of a tab that is not attached are not exposed", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  assert.equal(lifecycle.rootSession(62), null);
  assert.deepEqual(lifecycle.childSessions(62), []);
  await lifecycle.acquire(62);
  assert.deepEqual(lifecycle.rootSession(62).target, { tabId: 62 });
  await lifecycle.release(62);
  assert.equal(lifecycle.rootSession(62), null);
});

test("a dialog records its message and the session that raised it", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.acquire(63);
  lifecycle.openDialog(63, { type: "alert" });
  assert.deepEqual(lifecycle.currentDialog(63), { type: "alert", message: "", defaultPrompt: "", url: "", sessionId: null });
  lifecycle.openDialog(63, {});
  assert.equal(lifecycle.currentDialog(63).type, "unknown");
  lifecycle.openDialog(63, { type: "prompt", message: "Name?", defaultPrompt: "Ada" });
  assert.equal(lifecycle.currentDialog(63).defaultPrompt, "Ada");
  lifecycle.openDialog(63, { type: "beforeunload", message: "Leave?", url: "https://example.com/form", sessionId: "child-11" });
  const copy = lifecycle.currentDialog(63);
  copy.message = "changed";
  assert.deepEqual(lifecycle.currentDialog(63),
    { type: "beforeunload", message: "Leave?", defaultPrompt: "", url: "https://example.com/form", sessionId: "child-11" });
  await lifecycle.closeDialog(63);
  await lifecycle.release(63);
  assert.equal(lifecycle.currentDialog(63), null);
});

test("terminal shutdown detaches retained controlled tabs", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });

  await Promise.all([lifecycle.retain(17), lifecycle.retain(18)]);
  await lifecycle.detachAll();

  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "detach").sort(), [
    ["detach", 17],
    ["detach", 18]
  ]);
});

test("an open JavaScript dialog retains its debugger session until handled", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });

  await lifecycle.acquire(9);
  lifecycle.openDialog(9, { type: "prompt", message: "Name?", sessionId: null });
  await lifecycle.release(9);

  assert.deepEqual(lifecycle.currentDialog(9), { type: "prompt", message: "Name?", defaultPrompt: "", url: "", sessionId: null });
  assert.equal(lifecycle.attachedCount(), 1);
  assert.equal(chromeDebugger.calls.filter(([kind]) => kind === "detach").length, 0);

  await lifecycle.acquire(9);
  await lifecycle.closeDialog(9);
  assert.equal(lifecycle.attachedCount(), 1);
  await lifecycle.release(9);

  assert.equal(lifecycle.currentDialog(9), null);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "attach"), [["attach", 9, "1.3"]]);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "detach"), [["detach", 9]]);
});

test("an external detach preserves a known dialog for a later handling lease", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });

  await lifecycle.acquire(11);
  lifecycle.openDialog(11, { type: "confirm", message: "Sure?", sessionId: "child-1" });
  lifecycle.detached(11);
  await lifecycle.release(11);
  assert.deepEqual(lifecycle.currentDialog(11), { type: "confirm", message: "Sure?", defaultPrompt: "", url: "", sessionId: "child-1" });

  await lifecycle.acquire(11);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "attach"), [
    ["attach", 11, "1.3"],
    ["attach", 11, "1.3"]
  ]);
  await lifecycle.closeDialog(11);
  await lifecycle.release(11);
});

test("a new lease waits for an in-flight detach and then reattaches", async () => {
  const calls = [];
  let finishFirstDetach;
  let detachCount = 0;
  const lifecycle = sessions.create({
    onEvent: noEvents,
    async attach(target, version) { calls.push(["attach", target.tabId, version]); },
    async detach(target) {
      calls.push(["detach", target.tabId]);
      detachCount += 1;
      if (detachCount === 1) await new Promise((resolve) => { finishFirstDetach = resolve; });
    },
    async sendCommand(target, method) { calls.push(["command", target.tabId, method]); }
  }, { installer });

  await lifecycle.acquire(12);
  const releasing = lifecycle.release(12);
  await new Promise((resolve) => setImmediate(resolve));
  const acquiring = lifecycle.acquire(12);
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(calls.filter(([kind]) => kind === "attach").length, 1);

  finishFirstDetach();
  await Promise.all([releasing, acquiring]);
  assert.equal(calls.filter(([kind]) => kind === "attach").length, 2);
  assert.equal(lifecycle.attachedCount(), 1);
  await lifecycle.release(12);
});

test("failed Page enablement leaves no attached lifecycle state", async () => {
  const calls = [];
  const lifecycle = sessions.create({
    onEvent: noEvents,
    async attach(target, version) { calls.push(["attach", target.tabId, version]); },
    async detach(target) { calls.push(["detach", target.tabId]); },
    async sendCommand() { throw new Error("Page domain unavailable"); }
  }, { installer });

  await assert.rejects(lifecycle.acquire(13), /Page domain unavailable/);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(calls, [["attach", 13, "1.3"], ["detach", 13]]);
});

test("optional CDP domains share the existing attachment", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });

  await lifecycle.retain(14);
  assert.equal(await lifecycle.enableDomain(14, "Runtime"), true);
  assert.equal(await lifecycle.enableDomain(14, "Runtime"), false);
  assert.equal(await lifecycle.enableDomain(14, "Network"), true);
  await lifecycle.disableDomain(14, "Network");

  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "attach"), [["attach", 14, "1.3"]]);
  assert.deepEqual(chromeDebugger.calls.filter(([, , method]) => method === "Runtime.enable"), [
    ["command", 14, "Runtime.enable"]
  ]);
  assert.deepEqual(chromeDebugger.calls.filter(([, , method]) => method === "Network.enable"), [
    ["command", 14, "Network.enable"]
  ]);
  assert.deepEqual(chromeDebugger.calls.filter(([, , method]) => method === "Network.disable"), [
    ["command", 14, "Network.disable"]
  ]);
});

test("only retained tabs emulate focus, and only after active runtime negotiation", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.retain(21);
  await lifecycle.acquire(22);
  assert.equal(chromeDebugger.focus.size, 0);

  await lifecycle.setFocusEmulationEnabled(true);
  assert.equal(chromeDebugger.focus.get(21), true);
  assert.equal(chromeDebugger.focus.has(22), false);
  await lifecycle.acquire(21);
  await lifecycle.release(21);
  assert.equal(chromeDebugger.focus.get(21), true);
  await lifecycle.retain(23);
  assert.equal(chromeDebugger.focus.get(23), true);
  await lifecycle.detachAll();
});

test("pause restores focus on every retained tab and resume preserves their attachments", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  await Promise.all([lifecycle.retain(24), lifecycle.retain(25)]);
  await lifecycle.setFocusEmulationEnabled(false);
  assert.deepEqual([...chromeDebugger.focus.values()], [false, false]);
  assert.equal(lifecycle.attachedCount(), 2);
  await lifecycle.setFocusEmulationEnabled(true);
  assert.deepEqual([...chromeDebugger.focus.values()], [true, true]);
  assert.equal(chromeDebugger.calls.filter(([kind]) => kind === "attach").length, 2);
  await lifecycle.detachAll();
});

test("releasing ownership restores focus immediately while a command still holds its lease", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  await lifecycle.retain(26);
  await lifecycle.acquire(26);
  await lifecycle.unretain(26);
  assert.equal(chromeDebugger.focus.get(26), false);
  assert.equal(lifecycle.attachedCount(), 1);
  await lifecycle.release(26);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.has(26), false);
});

test("shutdown and local release clear focus before detaching every tab", async () => {
  const chromeDebugger = fakeDebugger();
  const transitions = [];
  const sendCommand = chromeDebugger.sendCommand;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (method === "Emulation.setFocusEmulationEnabled") transitions.push([target.tabId, params.enabled]);
    return sendCommand(target, method, params);
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  await Promise.all([lifecycle.retain(27), lifecycle.retain(28)]);
  await lifecycle.detachAll();
  assert.deepEqual(transitions.sort((a, b) => a[0] - b[0]), [[27, true], [27, false], [28, true], [28, false]]);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
});

test("a pause that arrives during focus enablement wins before setup finishes", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  let completeEnable;
  chromeDebugger.sendCommand = async (target, method, params) => {
    await sendCommand(target, method, params);
    if (params?.enabled) await new Promise(resolve => { completeEnable = resolve; });
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  const retaining = lifecycle.retain(29);
  await new Promise(resolve => setImmediate(resolve));
  const pausing = lifecycle.setFocusEmulationEnabled(false);
  completeEnable();
  await Promise.all([retaining, pausing]);
  assert.equal(chromeDebugger.focus.get(29), false);
  await lifecycle.detachAll();
});

test("a released tab cannot regain focus from setup already in flight", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  let completePageEnable;
  chromeDebugger.sendCommand = async (target, method, params) => {
    await sendCommand(target, method, params);
    if (method === "Page.enable") await new Promise(resolve => { completePageEnable = resolve; });
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  const retaining = lifecycle.retain(30);
  await new Promise(resolve => setImmediate(resolve));
  const releasing = lifecycle.unretain(30);
  completePageEnable();
  await Promise.all([retaining, releasing]);
  assert.equal(chromeDebugger.focus.size, 0);
  assert.equal(lifecycle.attachedCount(), 0);
});

test("an external detach stays detached until an explicit operation reacquires the tab", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  await lifecycle.retain(31);
  await chromeDebugger.detach({ tabId: 31 });
  lifecycle.detached(31);
  await lifecycle.setFocusEmulationEnabled(false);
  await lifecycle.setFocusEmulationEnabled(true);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
  await lifecycle.acquire(31);
  assert.equal(chromeDebugger.focus.get(31), true);
  await lifecycle.release(31);
  await lifecycle.detachAll();
});

test("failed focus enablement releases its possibly applied override and permits a later retry", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  let fail = true;
  chromeDebugger.sendCommand = async (target, method, params) => {
    await sendCommand(target, method, params);
    if (params?.enabled && fail) throw new Error("focus command failed after dispatch");
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  await assert.rejects(lifecycle.retain(32), /focus command failed/);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
  fail = false;
  await lifecycle.acquire(32);
  assert.equal(chromeDebugger.focus.get(32), true);
  await lifecycle.release(32);
  await lifecycle.detachAll();
});

test("a failed pause on one tab still restores every other tab", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (target.tabId === 33 && params?.enabled === false) throw new Error("focus disable failed");
    await sendCommand(target, method, params);
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  await Promise.all([lifecycle.retain(33), lifecycle.retain(34)]);
  await assert.rejects(lifecycle.setFocusEmulationEnabled(false), /Could not update controlled-tab focus/);
  assert.equal(chromeDebugger.focus.has(33), false);
  assert.equal(chromeDebugger.focus.get(34), false);
  await lifecycle.detachAll();
});

test("unconfirmed cleanup remains tracked and a later release retries it", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  const detach = chromeDebugger.detach;
  let fail = false;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (fail && params?.enabled === false) throw new Error("focus disable unavailable");
    return sendCommand(target, method, params);
  };
  chromeDebugger.detach = async target => {
    if (fail) throw new Error("debugger detach unavailable");
    return detach(target);
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  await lifecycle.retain(35);
  fail = true;
  await assert.rejects(lifecycle.detachAll(), /Could not release every debugger session/);
  assert.equal(lifecycle.attachedCount(), 1);
  assert.equal(chromeDebugger.focus.get(35), true);
  fail = false;
  await lifecycle.detachAll();
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
});

test("stop during attachment rejects unfinished setup and leaves no session", async () => {
  const chromeDebugger = fakeDebugger();
  const attach = chromeDebugger.attach;
  let completeAttach;
  chromeDebugger.attach = async (target, version) => {
    await attach(target, version);
    await new Promise(resolve => { completeAttach = resolve; });
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  const retaining = assert.rejects(lifecycle.retain(36), /released during setup/);
  await new Promise(resolve => setImmediate(resolve));
  const stopping = lifecycle.detachAll();
  completeAttach();
  await Promise.all([retaining, stopping]);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
  assert.equal(chromeDebugger.calls.filter(([, , method]) => method === "Emulation.setFocusEmulationEnabled").length, 0);
});

test("a late focus acknowledgement cannot resurrect an externally detached session", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  let completeEnable;
  chromeDebugger.sendCommand = async (target, method, params) => {
    await sendCommand(target, method, params);
    if (params?.enabled) await new Promise(resolve => { completeEnable = resolve; });
  };
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  const retaining = assert.rejects(lifecycle.retain(37), /released during setup/);
  await new Promise(resolve => setImmediate(resolve));
  await chromeDebugger.detach({ tabId: 37 });
  lifecycle.detached(37);
  completeEnable();
  await retaining;
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
  await lifecycle.detachAll();
});

// Waiting on a page that shows a dialog never ends, so a test that would wait fails at this limit.
const WAITS_ON_A_DIALOG = { timeout: 5_000 };

// A debugger whose page answers no command while it shows a JavaScript dialog, as Chromium's renderer
// does: a command to a tab after `show(tabId)` waits until `answer(tabId)`. Page.handleJavaScriptDialog
// is the browser's own and always answers. `sent` records every command as it is sent.
function pageWithDialogs() {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  const showing = new Map();
  chromeDebugger.sent = [];
  chromeDebugger.sendCommand = async (target, method, params) => {
    chromeDebugger.sent.push([target.tabId, method]);
    const held = showing.get(target.tabId);
    if (held && method !== "Page.handleJavaScriptDialog") await held.promise;
    return sendCommand(target, method, params);
  };
  chromeDebugger.show = (tabId) => {
    let open;
    showing.set(tabId, { promise: new Promise((resolve) => { open = resolve; }), open: () => open() });
  };
  chromeDebugger.answer = (tabId) => {
    showing.get(tabId)?.open();
    showing.delete(tabId);
  };
  return chromeDebugger;
}

test("a tab that shows a dialog holds up no focus change and catches up when the dialog closes", WAITS_ON_A_DIALOG, async () => {
  const chromeDebugger = pageWithDialogs();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  await Promise.all([lifecycle.retain(41), lifecycle.retain(42)]);
  chromeDebugger.show(41);
  lifecycle.openDialog(41, { type: "prompt", message: "Name?" });
  chromeDebugger.sent.length = 0;

  // A pause, a resume and a pause again finish at once, and none of them reaches the page that
  // shows the dialog.
  await lifecycle.setFocusEmulationEnabled(false);
  await lifecycle.setFocusEmulationEnabled(true);
  await lifecycle.setFocusEmulationEnabled(false);
  assert.deepEqual(chromeDebugger.sent.filter(([tabId]) => tabId === 41), []);
  assert.equal(chromeDebugger.focus.get(42), false);

  chromeDebugger.answer(41);
  await lifecycle.closeDialog(41);
  assert.equal(chromeDebugger.focus.get(41), false, "the tab takes what control asks for now");
  assert.equal(lifecycle.attachedCount(), 2);
  await lifecycle.detachAll();
});

test("a dialog that opens while a focus change is in flight frees the wait, and the tab catches up when it closes", WAITS_ON_A_DIALOG, async () => {
  const chromeDebugger = pageWithDialogs();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await lifecycle.setFocusEmulationEnabled(true);
  await lifecycle.retain(43);
  // The pause reaches the page just as it shows a dialog, so the page answers it only afterwards.
  chromeDebugger.show(43);
  const pausing = lifecycle.setFocusEmulationEnabled(false);
  await new Promise((resolve) => setImmediate(resolve));
  lifecycle.openDialog(43, { type: "alert", message: "Hi" });
  await pausing;
  // A resume while the dialog is open: the unanswered pause still counts as unconfirmed.
  await lifecycle.setFocusEmulationEnabled(true);

  chromeDebugger.answer(43);
  await lifecycle.closeDialog(43);
  assert.equal(chromeDebugger.focus.get(43), true, "the late pause does not outlast the resume");
  await lifecycle.detachAll();
});

test("a release, a domain's disable and a terminal shutdown never wait on a page that shows a dialog", WAITS_ON_A_DIALOG, async () => {
  const chromeDebugger = pageWithDialogs();
  const released = [];
  const lifecycle = sessions.create(chromeDebugger, {
    installer,
    // Releasing page code needs the page.
    onRelease: async (tabId) => {
      released.push(tabId);
      await lifecycle.cdp({ tabId }, "Runtime.releaseObjectGroup", { objectGroup: "ghostlight" });
    }
  });
  await lifecycle.setFocusEmulationEnabled(true);
  await lifecycle.retain(44);
  await lifecycle.enableDomain(44, "Network");
  chromeDebugger.show(44);
  lifecycle.openDialog(44, { type: "confirm", message: "Sure?" });

  await lifecycle.unretain(44);
  assert.equal(lifecycle.attachedCount(), 1, "the dialog keeps the session, so it can still be answered");
  await lifecycle.disableDomain(44, "Network");
  await lifecycle.detachAll();
  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(released, [44]);
  assert.equal(lifecycle.currentDialog(44), null);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "detach"), [["detach", 44]]);
});

test("forEachSession passes over a tab that shows a dialog, before or during its visit", WAITS_ON_A_DIALOG, async () => {
  const chromeDebugger = pageWithDialogs();
  const lifecycle = sessions.create(chromeDebugger, { installer });
  await Promise.all([lifecycle.acquire(45), lifecycle.acquire(46), lifecycle.acquire(47)]);
  chromeDebugger.show(45);
  lifecycle.openDialog(45, { type: "alert" });
  const visited = [];

  await lifecycle.forEachSession(async (session) => {
    const tabId = session.target.tabId;
    visited.push(tabId);
    if (tabId === 47) {
      // This page shows a dialog while it is being visited.
      chromeDebugger.show(47);
      lifecycle.openDialog(47, { type: "alert" });
    }
    await lifecycle.cdp(session.target, "Page.addScriptToEvaluateOnNewDocument", { source: "" });
  });
  assert.deepEqual(visited.sort(), [46, 47]);
  await lifecycle.detachAll();
});

test("no module names a CDP command but through the sessions table, and every key it uses exists", () => {
  const fs = require("node:fs");
  const path = require("node:path");
  const extension = path.join(__dirname, "..");
  const QUOTED = /"((?:Page|Runtime|Network|Target|DOM|Input|Emulation|Accessibility|Log)\.[A-Za-z]+)"/g;
  const KEY = /\bCDP\.([A-Z_]+)\b/g;
  const allowed = new Set(sessions.CDP_ALLOW);
  const keys = new Set(Object.keys(sessions.CDP));
  const problems = (name, text) => [
    ...[...text.matchAll(QUOTED)].map((m) => m[1])
      .filter((method) => name !== "worker.js" || allowed.has(method))
      .map((method) => `${name} names ${method} itself`),
    ...[...text.matchAll(KEY)].map((m) => m[1]).filter((key) => !keys.has(key)).map((key) => `${name} uses CDP.${key}`)
  ];
  assert.deepEqual(problems("lib/x.js", 'cdp(t, "Page.enable"); cdp(t, CDP.PAGE_ENABLE); cdp(t, CDP.PAGE_ENABEL);'),
    ["lib/x.js names Page.enable itself", "lib/x.js uses CDP.PAGE_ENABEL"], "the scan sees a literal and a mistyped key");
  assert.deepEqual(problems("worker.js", 'on("Page.screencastFrame"); cdp(t, "Page.screencastFrameAck");'),
    ["worker.js names Page.screencastFrameAck itself"], "the worker may name the events it routes, not a command");
  const files = ["worker.js", ...fs.readdirSync(path.join(extension, "lib")).filter((name) => name.endsWith(".js"))
    .filter((name) => name !== "sessions.js").map((name) => `lib/${name}`)];
  assert.ok(files.length > 10);
  assert.deepEqual(files.flatMap((name) => problems(name, fs.readFileSync(path.join(extension, name), "utf8"))), []);
  assert.deepEqual(new Set(Object.values(sessions.CDP)).size, sessions.CDP_ALLOW.length, "one key per command");
});

test("a dialog watch wakes with the type when a dialog opens, and at once when one is already open", async () => {
  const lifecycle = sessions.create(fakeDebugger(), { installer });
  await lifecycle.retain(7);
  const early = lifecycle.watchDialog(7);
  assert.doesNotThrow(() => lifecycle.requireNoDialog(7));
  lifecycle.openDialog(7, { type: "confirm" });
  assert.equal(await early.opened, "confirm");
  early.forget();
  assert.throws(() => lifecycle.requireNoDialog(7), { code: "dialog_open" });
  const late = lifecycle.watchDialog(7);
  assert.equal(await late.opened, "confirm", "a watch started after the dialog opened still sees it");
  late.forget();
  await lifecycle.closeDialog(7);
  assert.doesNotThrow(() => lifecycle.requireNoDialog(7));
  await lifecycle.detachAll();
});
