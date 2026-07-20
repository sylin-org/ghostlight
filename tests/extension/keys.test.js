// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/keys.js (key/input tables and modifier bits).

const { test } = require("node:test");
const assert = require("node:assert");
const {
  modifierBits,
  vkCode,
  keyCode,
  KEY_CUE_PRIVATE_TOKEN,
  KEY_CUE_TARGETS,
  keyCuePresentation,
  isKeyCuePresentation,
  charKeyInfo,
  textDispatchPlan,
  keyDispatchPlan,
} = require("../../extension/lib/keys.js");

test("modifier bits match CDP values", () => {
  assert.strictEqual(modifierBits("ctrl"), 2);
  assert.strictEqual(modifierBits("alt"), 1);
  assert.strictEqual(modifierBits("shift"), 8);
  assert.strictEqual(modifierBits("meta"), 4);
  assert.strictEqual(modifierBits("ctrl+shift"), 10);
});

test("named virtual key codes", () => {
  assert.strictEqual(vkCode("Enter"), 13);
  assert.strictEqual(vkCode("Tab"), 9);
});

test("punctuation maps", () => {
  assert.strictEqual(vkCode(";"), 186);
  assert.strictEqual(keyCode(";"), "Semicolon");
});

test("charKeyInfo maps newline to Enter", () => {
  assert.strictEqual(charKeyInfo("\n").key, "Enter");
  assert.strictEqual(charKeyInfo("\r").key, "Enter");
});

test("charKeyInfo rejects control and non-ASCII", () => {
  assert.strictEqual(charKeyInfo("\u0001"), null);
  assert.strictEqual(charKeyInfo("\u00e9"), null);
});

test("text dispatch plan preserves keyboard events and Unicode insertion", () => {
  const plan = textDispatchPlan("A\u00e9\ud83d\ude42\r\n");
  assert.equal(plan.characterCount, 4);
  assert.deepStrictEqual(plan.operations.map((operation) => operation.method), [
    "Input.dispatchKeyEvent",
    "Input.dispatchKeyEvent",
    "Input.insertText",
    "Input.insertText",
    "Input.dispatchKeyEvent",
    "Input.dispatchKeyEvent",
  ]);
  assert.equal(plan.operations[0].params.text, "A");
  assert.equal(plan.operations[0].params.modifiers, 8);
  assert.deepStrictEqual(plan.operations[2].params, { text: "\u00e9" });
  assert.deepStrictEqual(plan.operations[3].params, { text: "\ud83d\ude42" });
  assert.equal(plan.operations[4].params.key, "Enter");
});

test("key dispatch plan inserts ordinary printable keys", () => {
  const lower = keyDispatchPlan("a");
  assert.deepStrictEqual(lower.keyDown, {
    type: "keyDown",
    key: "a",
    code: "KeyA",
    modifiers: 0,
    windowsVirtualKeyCode: 65,
    nativeVirtualKeyCode: 65,
    text: "a",
    unmodifiedText: "a",
  });
  assert.equal(Object.hasOwn(lower.keyUp, "text"), false);

  const upper = keyDispatchPlan("A");
  assert.equal(upper.keyDown.key, "A");
  assert.equal(upper.keyDown.code, "KeyA");
  assert.equal(upper.keyDown.modifiers, 8);
  assert.equal(upper.keyDown.text, "A");
  assert.equal(upper.keyDown.unmodifiedText, "a");
});

test("key dispatch plan resolves shifted printables", () => {
  const letter = keyDispatchPlan("Shift+b");
  assert.equal(letter.keyDown.key, "B");
  assert.equal(letter.keyDown.code, "KeyB");
  assert.equal(letter.keyDown.modifiers, 8);
  assert.equal(letter.keyDown.text, "B");
  assert.equal(letter.keyDown.unmodifiedText, "b");

  const punctuation = keyDispatchPlan("Shift+/");
  assert.equal(punctuation.keyDown.key, "?");
  assert.equal(punctuation.keyDown.code, "Slash");
  assert.equal(punctuation.keyDown.windowsVirtualKeyCode, 191);
  assert.equal(punctuation.keyDown.text, "?");
  assert.equal(punctuation.keyDown.unmodifiedText, "/");
});

test("key dispatch plan omits text for command shortcuts", () => {
  const ctrlA = keyDispatchPlan("Ctrl+A");
  assert.equal(ctrlA.keyDown.key, "a");
  assert.equal(ctrlA.keyDown.modifiers, 2);
  assert.equal(Object.hasOwn(ctrlA.keyDown, "text"), false);

  const command = keyDispatchPlan("Cmd+Shift+P");
  assert.equal(command.keyDown.key, "P");
  assert.equal(command.keyDown.modifiers, 12);
  assert.equal(Object.hasOwn(command.keyDown, "text"), false);
});

test("key dispatch plan maps function keys and standalone modifiers", () => {
  const f2 = keyDispatchPlan("f2");
  assert.equal(f2.keyDown.key, "F2");
  assert.equal(f2.keyDown.code, "F2");
  assert.equal(f2.keyDown.windowsVirtualKeyCode, 113);

  const shift = keyDispatchPlan("Shift");
  assert.equal(shift.keyDown.key, "Shift");
  assert.equal(shift.keyDown.code, "ShiftLeft");
  assert.equal(shift.keyDown.windowsVirtualKeyCode, 16);
  assert.equal(shift.keyDown.modifiers, 8);
  assert.equal(shift.keyUp.modifiers, 0);
});

test("key dispatch plan preserves direct reload behavior", () => {
  assert.deepStrictEqual(keyDispatchPlan("Ctrl+R").reload, { bypassCache: false });
  assert.deepStrictEqual(keyDispatchPlan("Shift+F5").reload, { bypassCache: true });
  assert.equal(keyDispatchPlan("Enter").reload, null);
});

test("key cue keeps named keys and command shortcuts", () => {
  assert.deepStrictEqual(keyCuePresentation(
    "CTRL+A Enter Cmd+Shift+P Alt+/",
    Array(4).fill(KEY_CUE_TARGETS.UNKNOWN)
  ), {
    chords: [
      { target: "unknown", tokens: ["Ctrl", "A"] },
      { target: "unknown", tokens: ["Enter"] },
      { target: "unknown", tokens: ["Cmd", "Shift", "P"] },
      { target: "unknown", tokens: ["Alt", "/"] },
    ],
    more: false,
  });
  assert.deepStrictEqual(keyCuePresentation("ArrowDown F5 space", Array(3).fill("ordinary")), {
    chords: [
      { target: "ordinary", tokens: ["ArrowDown"] },
      { target: "ordinary", tokens: ["F5"] },
      { target: "ordinary", tokens: ["Space"] },
    ],
    more: false,
  });
});

test("key cue reveals ordinary printable keys and masks protected targets", () => {
  assert.deepStrictEqual(keyCuePresentation("a Shift+B password", ["ordinary", "protected", "protected"]), {
    chords: [
      { target: "ordinary", tokens: ["A"] },
      { target: "protected", tokens: ["Shift", KEY_CUE_PRIVATE_TOKEN] },
      { target: "protected", tokens: [KEY_CUE_PRIVATE_TOKEN] },
    ],
    more: false,
  });
  const privateCue = keyCuePresentation("s e c r e t", Array(6).fill("protected"));
  assert.equal(JSON.stringify(privateCue).includes("secret"), false);
  assert.equal(privateCue.chords.every((chord) => chord.tokens[0] === KEY_CUE_PRIVATE_TOKEN), true);
});

test("key cue masks printable keys when target observation is incomplete", () => {
  assert.deepStrictEqual(keyCuePresentation("a Shift+B", ["ordinary"]), {
    chords: [
      { target: "unknown", tokens: [KEY_CUE_PRIVATE_TOKEN] },
      { target: "unknown", tokens: ["Shift", KEY_CUE_PRIVATE_TOKEN] },
    ],
    more: false,
  });
  assert.equal(
    keyCuePresentation("space", ["protected"]).chords[0].tokens[0],
    KEY_CUE_PRIVATE_TOKEN
  );
});

test("key cue combines repeated target observations conservatively", () => {
  assert.deepStrictEqual(keyCuePresentation("a Enter", ["ordinary", "ordinary", "protected", "ordinary"], 2), {
    chords: [
      { target: "protected", tokens: [KEY_CUE_PRIVATE_TOKEN] },
      { target: "ordinary", tokens: ["Enter"] },
    ],
    more: false,
  });
});

test("key cue is bounded and reports omitted chord groups", () => {
  const cue = keyCuePresentation("Enter Tab Escape Home End PageDown ArrowUp", Array(7).fill("ordinary"));
  assert.equal(cue.chords.length, 6);
  assert.equal(cue.more, true);
  assert.deepStrictEqual(keyCuePresentation("  "), { chords: [], more: false });
});

test("renderer-side key cue validation rejects arbitrary text", () => {
  assert.equal(isKeyCuePresentation(keyCuePresentation("Ctrl+A Enter")), true);
  assert.equal(isKeyCuePresentation({ chords: [{ target: "ordinary", tokens: ["secret"] }], more: false }), false);
  assert.equal(isKeyCuePresentation({ chords: [{ target: "unknown", tokens: ["Ctrl", "secret"] }], more: false }), false);
  assert.equal(isKeyCuePresentation({ chords: [{ target: "unknown", tokens: ["A"] }], more: false }), false);
  assert.equal(isKeyCuePresentation({ chords: [{ target: "ordinary", tokens: ["A"] }], more: false }), true);
  assert.equal(isKeyCuePresentation({ chords: [{ target: "protected", tokens: [KEY_CUE_PRIVATE_TOKEN] }], more: false }), true);
  assert.equal(isKeyCuePresentation({ chords: [{ target: "unknown", tokens: ["Ctrl", "A"] }], more: false }), true);
});
