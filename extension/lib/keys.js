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
const KEY_MODIFIERS = Object.freeze({
  alt: { bit: 1, key: "Alt", code: "AltLeft", vk: 18, label: "Alt", command: true },
  ctrl: { bit: 2, key: "Control", code: "ControlLeft", vk: 17, label: "Ctrl", command: true },
  control: { bit: 2, key: "Control", code: "ControlLeft", vk: 17, label: "Ctrl", command: true },
  meta: { bit: 4, key: "Meta", code: "MetaLeft", vk: 91, label: "Meta", command: true },
  cmd: { bit: 4, key: "Meta", code: "MetaLeft", vk: 91, label: "Cmd", command: true },
  command: { bit: 4, key: "Meta", code: "MetaLeft", vk: 91, label: "Cmd", command: true },
  win: { bit: 4, key: "Meta", code: "MetaLeft", vk: 91, label: "Win", command: true },
  windows: { bit: 4, key: "Meta", code: "MetaLeft", vk: 91, label: "Win", command: true },
  shift: { bit: 8, key: "Shift", code: "ShiftLeft", vk: 16, label: "Shift", command: false },
});
function modifierBits(str) {
  let bits = 0;
  for (const p of (str || "").toLowerCase().split("+").map((x) => x.trim())) {
    const modifier = KEY_MODIFIERS[p];
    if (modifier) bits |= modifier.bit;
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
  Shift: 16, Control: 17, Alt: 18, Meta: 91,
};
const CODE_NAMED = {
  Shift: "ShiftLeft", Control: "ControlLeft", Alt: "AltLeft", Meta: "MetaLeft",
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
  if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(key)) return 111 + Number(key.slice(1));
  return VK_NAMED[key] || 0;
}
// US-QWERTY: shifted printable -> the unshifted character on the same key.
const SHIFT_BASE = {
  "!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6",
  "&": "7", "*": "8", "(": "9", ")": "0",
  "_": "-", "+": "=", "{": "[", "}": "]", "|": "\\", ":": ";",
  '"': "'", "<": ",", ">": ".", "?": "/", "~": "`",
};
const BASE_SHIFT = Object.freeze(Object.entries(SHIFT_BASE).reduce((result, entry) => {
  result[entry[1]] = entry[0];
  return result;
}, {}));
// Page-visible key feedback is deliberately smaller than the execution vocabulary. Named keys
// and real command shortcuts are useful to a watcher; printable input is visible only when the
// trusted key event landed on an ordinary target. The service worker sends only this derived
// structure to the renderer (ADR-0087).
const KEY_CUE_MAX_CHORDS = 6;
const KEY_CUE_PRIVATE_TOKEN = "private-key";
const KEY_CUE_TARGETS = Object.freeze({
  ORDINARY: "ordinary",
  PROTECTED: "protected",
  UNKNOWN: "unknown",
});
const KEY_CUE_OBSERVATION_MESSAGES = Object.freeze({
  BEGIN: "beginKeyCueObservation",
  FINISH: "finishKeyCueObservation",
});
const KEY_CUE_FIXED_LABELS = new Set([
  "Ctrl", "Alt", "Shift", "Meta", "Cmd", "Win", "Space",
  "Enter", "Tab", "Escape", "Backspace", "Delete",
  "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
  "Home", "End", "PageUp", "PageDown",
]);
const KEY_CUE_COMMAND_LABELS = new Set(["Ctrl", "Alt", "Meta", "Cmd", "Win"]);

function namedKeyCue(raw) {
  const lower = String(raw || "").toLowerCase();
  if (Object.prototype.hasOwnProperty.call(KEY_MAP, lower)) {
    const mapped = KEY_MAP[lower];
    return mapped === " " ? "Space" : mapped;
  }
  if (/^f(?:[1-9]|1[0-9]|2[0-4])$/i.test(raw)) return raw.toUpperCase();
  return null;
}

function normalizedTarget(target) {
  return Object.values(KEY_CUE_TARGETS).includes(target) ? target : KEY_CUE_TARGETS.UNKNOWN;
}

function chordTargets(rawChordCount, observedTargets, repeat) {
  const repetitions = Number.isInteger(repeat) && repeat > 0 ? repeat : 1;
  const expected = rawChordCount * repetitions;
  if (!Array.isArray(observedTargets) || observedTargets.length !== expected) {
    return Array(rawChordCount).fill(KEY_CUE_TARGETS.UNKNOWN);
  }
  const targets = [];
  for (let chordIndex = 0; chordIndex < rawChordCount; chordIndex++) {
    const observed = [];
    for (let repetition = 0; repetition < repetitions; repetition++) {
      observed.push(normalizedTarget(observedTargets[(repetition * rawChordCount) + chordIndex]));
    }
    if (observed.includes(KEY_CUE_TARGETS.PROTECTED)) {
      targets.push(KEY_CUE_TARGETS.PROTECTED);
    } else if (observed.every((target) => target === KEY_CUE_TARGETS.ORDINARY)) {
      targets.push(KEY_CUE_TARGETS.ORDINARY);
    } else {
      targets.push(KEY_CUE_TARGETS.UNKNOWN);
    }
  }
  return targets;
}

function keyCuePresentation(text, observedTargets, repeat) {
  const source = String(text || "").trim();
  if (!source) return { chords: [], more: false };
  const rawChords = source.split(/\s+/).filter(Boolean);
  const targets = chordTargets(rawChords.length, observedTargets, repeat);
  const chords = rawChords.slice(0, KEY_CUE_MAX_CHORDS).map((rawChord, chordIndex) => {
    const labels = [];
    const keys = [];
    let command = false;
    for (const rawPart of rawChord.split("+").map((part) => part.trim()).filter(Boolean)) {
      const modifier = KEY_MODIFIERS[rawPart.toLowerCase()];
      if (modifier) {
        if (!labels.includes(modifier.label)) labels.push(modifier.label);
        command = command || modifier.command;
      } else {
        keys.push(rawPart);
      }
    }
    const rawKey = keys.length ? keys[keys.length - 1] : null;
    if (rawKey !== null) {
      const named = namedKeyCue(rawKey);
      const revealPrintable = command || targets[chordIndex] === KEY_CUE_TARGETS.ORDINARY;
      const printable = revealPrintable && /^[\x20-\x7e]$/.test(rawKey);
      const visibleNamed = named && (named !== "Space" || revealPrintable) ? named : null;
      const keyLabel = visibleNamed || (printable
        ? (/[a-z]/i.test(rawKey) ? rawKey.toUpperCase() : rawKey)
        : KEY_CUE_PRIVATE_TOKEN);
      labels.push(keyLabel);
    }
    return {
      target: targets[chordIndex],
      tokens: labels.length ? labels : [KEY_CUE_PRIVATE_TOKEN],
    };
  });
  return { chords, more: rawChords.length > KEY_CUE_MAX_CHORDS };
}

function isKeyCuePresentation(value) {
  if (!value || !Array.isArray(value.chords) || typeof value.more !== "boolean") return false;
  if (value.chords.length > KEY_CUE_MAX_CHORDS) return false;
  return value.chords.every((chord) => {
    if (!chord || !Object.values(KEY_CUE_TARGETS).includes(chord.target) ||
        !Array.isArray(chord.tokens) || chord.tokens.length < 1 || chord.tokens.length > 5) {
      return false;
    }
    const hasCommand = chord.tokens.some((token) => KEY_CUE_COMMAND_LABELS.has(token));
    return chord.tokens.every((token) => typeof token === "string" && (
      token === KEY_CUE_PRIVATE_TOKEN ||
      KEY_CUE_FIXED_LABELS.has(token) ||
      /^F(?:[1-9]|1[0-9]|2[0-4])$/.test(token) ||
      ((hasCommand || chord.target === KEY_CUE_TARGETS.ORDINARY) && /^[\x20-\x7e]$/.test(token))
    ));
  });
}
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

function textDispatchPlan(text) {
  const source = Array.from(String(text || ""));
  const operations = [];
  let characterCount = 0;
  for (let index = 0; index < source.length; index++) {
    const character = source[index];
    // A CRLF pair is one user-visible newline and one Enter dispatch. A standalone CR remains a
    // separate Enter, matching the prior behavior.
    if (character === "\r" && source[index + 1] === "\n") continue;
    characterCount += 1;
    const info = charKeyInfo(character);
    if (!info) {
      operations.push({ method: "Input.insertText", params: { text: character } });
      continue;
    }
    const modifiers = info.shift ? 8 : 0;
    const shared = {
      key: info.key,
      code: info.code,
      modifiers,
      windowsVirtualKeyCode: info.vk,
      nativeVirtualKeyCode: info.vk,
    };
    operations.push({
      method: "Input.dispatchKeyEvent",
      params: {
        type: "keyDown",
        ...shared,
        text: info.text,
        unmodifiedText: info.unmodifiedText,
      },
    });
    operations.push({
      method: "Input.dispatchKeyEvent",
      params: { type: "keyUp", ...shared },
    });
  }
  return {
    characterCount,
    operations,
  };
}

function shiftedPrintableKey(key) {
  if (/^[a-z]$/.test(key)) return key.toUpperCase();
  return BASE_SHIFT[key] || key;
}

function keyDispatchPlan(combo) {
  const parts = String(combo || "").split("+").map((part) => part.trim()).filter(Boolean);
  let modifiers = 0;
  let key = "";
  let standaloneModifier = null;
  for (const rawPart of parts) {
    const lower = rawPart.toLowerCase();
    const modifier = KEY_MODIFIERS[lower];
    if (modifier) {
      modifiers |= modifier.bit;
      if (parts.length === 1) standaloneModifier = modifier;
    } else {
      key = KEY_MAP[lower] || rawPart;
    }
  }
  if (!key && standaloneModifier) key = standaloneModifier.key;
  if (!key) key = String(combo || "");
  if (/^f(?:[1-9]|1[0-9]|2[0-4])$/i.test(key)) key = key.toUpperCase();

  const command = (modifiers & 7) !== 0;
  const explicitShift = (modifiers & 8) !== 0;
  let info = null;
  if (key.length === 1) {
    let character = key;
    if (command && !explicitShift && /^[A-Z]$/.test(character)) {
      character = character.toLowerCase();
    } else if (explicitShift) {
      character = shiftedPrintableKey(character);
    }
    info = charKeyInfo(character);
    if (info && info.shift && (!command || explicitShift || !/^[A-Z]$/.test(key))) {
      modifiers |= 8;
    }
  }

  const eventKey = info ? info.key : key;
  const code = standaloneModifier ? standaloneModifier.code : (info ? info.code : (CODE_NAMED[eventKey] || keyCode(eventKey)));
  const vk = standaloneModifier ? standaloneModifier.vk : (info ? info.vk : vkCode(eventKey));
  const shared = {
    key: eventKey,
    code,
    modifiers,
    windowsVirtualKeyCode: vk,
    nativeVirtualKeyCode: vk,
  };
  const keyDown = { type: "keyDown", ...shared };
  if (info && !command) {
    keyDown.text = info.text;
    keyDown.unmodifiedText = info.unmodifiedText;
  }
  const keyUp = {
    type: "keyUp",
    ...shared,
    modifiers: standaloneModifier ? (modifiers & ~standaloneModifier.bit) : modifiers,
  };
  const bare = eventKey.toLowerCase();
  const ctrlOrCmd = (modifiers & 2) !== 0 || (modifiers & 4) !== 0;
  const reload = (ctrlOrCmd && bare === "r") || bare === "f5"
    ? { bypassCache: (modifiers & 8) !== 0 }
    : null;
  return { keyDown, keyUp, reload };
}

const GhostlightKeys = {
  KEY_MAP, modifierBits, keyCode, VK_NAMED, VK_PUNCT, CODE_PUNCT, vkCode, SHIFT_BASE,
  KEY_CUE_MAX_CHORDS, KEY_CUE_PRIVATE_TOKEN, KEY_CUE_TARGETS, KEY_CUE_OBSERVATION_MESSAGES,
  keyCuePresentation, isKeyCuePresentation, charKeyInfo, textDispatchPlan, keyDispatchPlan,
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightKeys;
} else {
  self.GhostlightKeys = GhostlightKeys;
}
})();
