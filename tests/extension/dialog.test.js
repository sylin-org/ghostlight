// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/dialog.js (ADR-0078 D7).

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const {
  MESSAGE_LIMIT,
  createDialogStore,
  resolutionCommand,
} = require("../../extension/lib/dialog.js");

test("dialog state is tab scoped bounded and replaced by the latest event", () => {
  const store = createDialogStore();
  store.opened(7, { type: "alert", message: "a".repeat(MESSAGE_LIMIT + 50) });
  store.opened(8, { type: "confirm", message: "Keep this" });
  assert.strictEqual(store.current(7).message.length, MESSAGE_LIMIT);
  assert.strictEqual(store.current(8).message, "Keep this");
  store.opened(7, { type: "prompt", message: "Latest" });
  assert.deepStrictEqual(store.current(7), { type: "prompt", message: "Latest" });
});

test("dialog state clears on close tab navigation session and panic mechanisms", () => {
  const store = createDialogStore();
  store.opened(1, { type: "alert", message: "one" });
  assert.strictEqual(store.remove(1), true);
  assert.strictEqual(store.current(1), null);
  store.opened(2, { type: "alert", message: "two" });
  store.opened(3, { type: "alert", message: "three" });
  store.clear();
  assert.strictEqual(store.current(2), null);
  assert.strictEqual(store.current(3), null);
});

test("each explicit resolution action maps to one CDP command", () => {
  assert.deepStrictEqual(resolutionCommand("accept"), { accept: true });
  assert.deepStrictEqual(resolutionCommand("dismiss"), { accept: false });
  assert.deepStrictEqual(resolutionCommand("respond", "Ada"), {
    accept: true,
    promptText: "Ada",
  });
  assert.throws(() => resolutionCommand("respond"), /requires text/);
  assert.throws(() => resolutionCommand("status"), /unsupported/);
});

test("worker observes and resolves dialogs without automatic acceptance", () => {
  const source = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  assert.match(source, /Page\.javascriptDialogOpening/);
  assert.match(source, /Page\.javascriptDialogClosed/);
  assert.match(source, /Page\.handleJavaScriptDialog/);
  assert.match(source, /dialogStore\.remove\(tabId\)/);
  const openingBranch = source.match(
    /if \(method === "Page\.javascriptDialogOpening"\) \{[\s\S]*?\n  \}/
  );
  assert.ok(openingBranch);
  assert.doesNotMatch(openingBranch[0], /handleJavaScriptDialog/);
  const guard = source.indexOf("if (dialogStore.current(tabId))", source.indexOf("async function withObservation"));
  const mutation = source.indexOf("const result = await run()", guard);
  assert.ok(guard > 0 && mutation > guard, "an unresolved dialog blocks before mutation dispatch");
  assert.match(
    source,
    /case "hover": \{\s*return withObservation\([^]*?const c = await resolveCoords\(tabId, a\)/,
    "click and hover ref resolution must run behind the dialog guard"
  );
  assert.match(source, /msg\.type === "narration_clear"[^]*dialogStore\.remove\(msg\.tabId\)/);
  assert.match(source, /async function killSession\(\)[^]*dialogStore\.clear\(\)/);
  assert.match(
    source,
    /a\.require_resolution === true\) out\.structuredContent\.resolution_not_met = true/,
    "strict native resolution reports a normal not-met receipt when no dialog is open",
  );
});

test("an open dialog blocks ref scroll before page-dependent preparation", async () => {
  const source = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  const observationStart = source.indexOf("async function appendDialogBlocker");
  const observationEnd = source.indexOf("// --- Screenshot pipeline", observationStart);
  const computerStart = source.indexOf("async function computer(a)");
  const computerEnd = source.indexOf("// --- Tool handlers ---", computerStart);
  assert.ok(observationStart >= 0 && observationEnd > observationStart);
  assert.ok(computerStart >= 0 && computerEnd > computerStart);

  const calls = { content: 0, resolveCoords: 0, scrollProbe: 0 };
  const context = {
    dialogStore: { current: () => ({ type: "alert", message: "blocked" }) },
    effectiveTabId: async () => 17,
    modifierBits: () => 0,
    ensureAttached: async () => {},
    enableDomain: async () => {},
    sleep: async () => {},
    pageMeta: async () => ({ tabId: 17, origin: "https://example.test" }),
    text: (value) => ({ content: [{ type: "text", text: value }] }),
    content: async () => { calls.content += 1; },
    resolveCoords: async () => { calls.resolveCoords += 1; return [10, 20]; },
    probeScrollState: async () => { calls.scrollProbe += 1; return null; },
  };
  const computer = vm.runInNewContext(
    `${source.slice(observationStart, observationEnd)}\n` +
      `${source.slice(computerStart, computerEnd)}\ncomputer;`,
    context
  );

  const result = await computer({
    action: "scroll",
    tabId: 17,
    ref: "r42",
    scroll_direction: "down",
  });

  assert.match(result.content[0].text, /Action not dispatched.*dialog/i);
  assert.deepStrictEqual(calls, { content: 0, resolveCoords: 0, scrollProbe: 0 });
});
