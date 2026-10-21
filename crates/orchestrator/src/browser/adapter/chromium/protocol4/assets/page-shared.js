// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- the few helpers the page sources share. Included by world.js.
const CREDENTIAL_AUTOCOMPLETE = new Set([
  "current-password",
  "new-password",
  "one-time-code",
  "cc-number",
  "cc-csc",
  "cc-exp",
  "cc-name"
]);

// Names that mark a credential-class field. Matched on the lowercased name and id after a
// camelCase name is split into words, each side bounded by a non-letter or the string end.
const CREDENTIAL_NAME = new RegExp(
  "(^|[^a-z])(password|passwd|passcode|pin|ssn|iban|otp|one.?time|secret|token|cvv|cvc|" +
  "card.?number|api.?key|cc-exp|cc-name)([^a-z]|$)"
);

// Page text is cut and counted in characters (code points), as the service counts the limits it
// sends, so a cut never ends on the first half of a pair: the extension cannot send half an emoji,
// and a 2.0.0 extension would drop the whole answer.
function bounded(value, maximum) {
  const text = String(value ?? "");
  if (text.length <= maximum) return text;
  let kept = 0;
  let end = 0;
  for (const character of text) {
    if (kept === maximum) break;
    kept += 1;
    end += character.length;
  }
  return text.slice(0, end);
}

// How many characters (code points) the text holds, the unit of every limit the service sends.
function characters(value) {
  let count = 0;
  for (const _character of String(value ?? "")) count += 1;
  return count;
}

function isCredentialMetadata(metadata) {
  const type = String(metadata.type ?? "").toLowerCase();
  if (type === "password") return true;
  const autocomplete = String(metadata.autocomplete ?? "").toLowerCase().split(/\s+/);
  if (autocomplete.some((token) => CREDENTIAL_AUTOCOMPLETE.has(token))) return true;
  const words = (value) => String(value ?? "").replace(/([a-z])([A-Z])/g, "$1 $2");
  const identity = `${words(metadata.name)} ${words(metadata.id)}`.toLowerCase();
  return CREDENTIAL_NAME.test(identity);
}

// The agent's text-condition value, shaped like page text: a whitespace run holding a newline
// is one newline, any other whitespace run is one space, and the ends are trimmed. Page block
// breaks stay as they are and matching stays case-sensitive.
function normalizeNeedle(value) {
  return String(value ?? "")
    .replace(/\s+/g, (run) => (run.includes("\n") ? "\n" : " "))
    .trim();
}

const shared = Object.freeze({ bounded, characters, isCredentialMetadata, normalizeNeedle });
