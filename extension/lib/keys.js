// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- keyboard/mouse tables: key name maps, virtual key codes, and modifier bits.
//
// IIFE-wrapped so its internal const/function bindings stay function-scoped, not global lexical
// bindings in the service worker (see lib/geometry.js for the full rationale: importScripts shares
// the worker global scope, so top-level consts here collide with service-worker.js's destructure
// and with a re-import). Only the export assignment is global.
(function () {
const KEY_MAP = {
  enter: "Enter", return: "Enter", tab: "Tab", escape: "Escape", esc: "Escape",
  backspace: "Backspace", delete: "Delete", space: " ",
  up: "ArrowUp", down: "ArrowDown", left: "ArrowLeft", right: "ArrowRight",
  arrowup: "ArrowUp", arrowdown: "ArrowDown", arrowleft: "ArrowLeft", arrowright: "ArrowRight",
  home: "Home", end: "End", pageup: "PageUp", pagedown: "PageDown",
};
// DOM MouseEvent.buttons bitmask per button name.
const BUTTON_BITS = { left: 1, right: 2, middle: 4 };
function modifierBits(str) {
  let bits = 0;
  for (const p of (str || "").toLowerCase().split("+").map((x) => x.trim())) {
    if (p === "ctrl" || p === "control") bits |= 2;
    else if (p === "alt") bits |= 1;
    else if (p === "shift") bits |= 8;
    else if (["meta", "cmd", "command", "win", "windows"].includes(p)) bits |= 4;
  }
  return bits;
}
// Best-effort DOM `code` for a resolved key, so pages that branch on event.code / keyCode work.
function keyCode(key) {
  if (key.length === 1) {
    if (/[a-zA-Z]/.test(key)) return "Key" + key.toUpperCase();
    if (/[0-9]/.test(key)) return "Digit" + key;
    if (CODE_PUNCT[key]) return CODE_PUNCT[key];
  }
  return key; // named keys (Enter, Tab, ArrowUp, ...) use the key name as their code
}
// Windows virtual key codes, so Chrome interprets shortcuts (ctrl+a select-all, etc.) as commands.
const VK_NAMED = {
  Enter: 13, Tab: 9, Escape: 27, Backspace: 8, Delete: 46, " ": 32,
  ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39,
  Home: 36, End: 35, PageUp: 33, PageDown: 34, Insert: 45,
};
// Windows virtual key codes for US-QWERTY punctuation keys (VK_OEM_*).
const VK_PUNCT = {
  ";": 186, "=": 187, ",": 188, "-": 189, ".": 190, "/": 191,
  "`": 192, "[": 219, "\\": 220, "]": 221, "'": 222,
};
// DOM `code` values for US-QWERTY punctuation keys (and Space).
const CODE_PUNCT = {
  ";": "Semicolon", "=": "Equal", ",": "Comma", "-": "Minus",
  ".": "Period", "/": "Slash", "`": "Backquote", "[": "BracketLeft",
  "\\": "Backslash", "]": "BracketRight", "'": "Quote", " ": "Space",
};
function vkCode(key) {
  if (key.length === 1) {
    const up = key.toUpperCase();
    if (up >= "A" && up <= "Z") return up.charCodeAt(0); // A-Z -> 65-90
    if (key >= "0" && key <= "9") return key.charCodeAt(0); // 0-9 -> 48-57
    if (VK_PUNCT[key]) return VK_PUNCT[key];
  }
  return VK_NAMED[key] || 0;
}
// US-QWERTY: shifted printable -> the unshifted character on the same key.
const SHIFT_BASE = {
  "!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6",
  "&": "7", "*": "8", "(": "9", ")": "0",
  "_": "-", "+": "=", "{": "[", "}": "]", "|": "\\", ":": ";",
  '"': "'", "<": ",", ">": ".", "?": "/", "~": "`",
};
// Resolve one typed character to Input.dispatchKeyEvent fields, or null when the character has no
// key mapping (control characters, non-ASCII) and must fall back to Input.insertText instead.
function charKeyInfo(ch) {
  if (ch === "\n" || ch === "\r") {
    return { key: "Enter", code: "Enter", vk: 13, shift: false, text: "\r", unmodifiedText: "\r" };
  }
  if (ch < " " || ch > "~") return null;
  let base = ch, shift = false;
  if (ch >= "A" && ch <= "Z") { base = ch.toLowerCase(); shift = true; }
  else if (SHIFT_BASE[ch]) { base = SHIFT_BASE[ch]; shift = true; }
  return { key: ch, code: keyCode(base), vk: vkCode(base), shift, text: ch, unmodifiedText: base };
}

const GhostlightKeys = {
  KEY_MAP, BUTTON_BITS, modifierBits, keyCode, VK_NAMED, VK_PUNCT, CODE_PUNCT, vkCode, SHIFT_BASE, charKeyInfo,
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightKeys;
} else {
  self.GhostlightKeys = GhostlightKeys;
}
})();
