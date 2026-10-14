// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- the few helpers the page sources share. Included by world.js.
const CREDENTIAL_AUTOCOMPLETE = new Set([
  "current-password",
  "new-password",
  "one-time-code",
  "cc-number",
  "cc-csc"
]);

function bounded(value, maximum) {
  return String(value ?? "").slice(0, maximum);
}

function isCredentialMetadata(metadata) {
  const type = String(metadata.type ?? "").toLowerCase();
  if (type === "password") return true;
  const autocomplete = String(metadata.autocomplete ?? "").toLowerCase().split(/\s+/);
  if (autocomplete.some((token) => CREDENTIAL_AUTOCOMPLETE.has(token))) return true;
  const identity = `${metadata.name ?? ""} ${metadata.id ?? ""}`.toLowerCase();
  return /(^|[^a-z])(password|passwd|passcode|otp|one.?time|secret|token|cvv|cvc)([^a-z]|$)/.test(identity);
}

function presentationLabel(signal) {
  const labels = {
    start: "Ghostlight starting",
    target: "Ghostlight target",
    progress: "Ghostlight working",
    completion: "Ghostlight complete",
    denial: "Ghostlight blocked",
    attention: "Ghostlight needs you"
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

const shared = Object.freeze({ bounded, isCredentialMetadata, presentationLabel, activityLabel });
