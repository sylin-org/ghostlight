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

function bounded(value, maximum) {
  return String(value ?? "").slice(0, maximum);
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

function presentationLabel(signal) {
  const labels = {
    start: "Ghostlight starting",
    target: "Ghostlight target",
    progress: "Ghostlight working",
    completion: "Ghostlight complete",
    denial: "Ghostlight blocked"
  };
  return labels[signal] ?? "Ghostlight";
}

function activityLabel(activity) {
  const labels = {
    quiet: "Ghostlight",
    navigate: "Navigating",
    click: "Clicking",
    hover: "Hovering",
    drag: "Dragging",
    type: "Typing",
    key: "Keyboard",
    scroll: "Scrolling",
    read: "Reading page",
    find: "Finding on page",
    screenshot: "Screenshot",
    zoom: "Zooming",
    fill: "Filling form",
    upload: "Uploading file",
    script: "Running JavaScript",
    wait: "Waiting",
    dialog: "Browser dialog"
  };
  return labels[activity] ?? "Ghostlight";
}

const shared = Object.freeze({ bounded, isCredentialMetadata, normalizeNeedle, presentationLabel, activityLabel });
