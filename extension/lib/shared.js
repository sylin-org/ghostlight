(function installGhostlightShared(root, factory) {
  const api = factory();
  root.GhostlightShared = api;
  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  }
})(globalThis, function createGhostlightShared() {
  "use strict";

  const NATIVE_HOST_NAME = "org.sylin.ghostlight";
  const ADAPTER_PROTOCOL_MAJOR = 2;
  const ADAPTER_CAPABILITIES = Object.freeze([
    "tabs",
    "atomic_tab_open",
    "navigation",
    "semantic_document",
    "capture",
    "pointer_input",
    "keyboard_input",
    "files",
    "script",
    "observation",
    "dialogs",
    "operation_recovery",
    "presentation",
    "window_geometry",
    "diagnostics",
    "recording",
    "chunked_commands",
    "adapter_liveness"
  ].map((name) => Object.freeze({ name, revision: 1 })));
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

  function readinessForStatus(status) {
    if (status === "loading") return "loading";
    if (status === "complete") return "complete";
    return "unknown";
  }

  function isCredentialMetadata(metadata) {
    const type = String(metadata.type ?? "").toLowerCase();
    if (type === "password") return true;
    const autocomplete = String(metadata.autocomplete ?? "").toLowerCase().split(/\s+/);
    if (autocomplete.some((token) => CREDENTIAL_AUTOCOMPLETE.has(token))) return true;
    const identity = `${metadata.name ?? ""} ${metadata.id ?? ""}`.toLowerCase();
    return /(^|[^a-z])(password|passwd|passcode|otp|one.?time|secret|token|cvv|cvc)([^a-z]|$)/.test(identity);
  }

  function modifierMask(modifiers) {
    let mask = 0;
    for (const modifier of modifiers ?? []) {
      if (modifier === "Alt") mask |= 1;
      if (modifier === "Control") mask |= 2;
      if (modifier === "Meta") mask |= 4;
      if (modifier === "Shift") mask |= 8;
    }
    return mask;
  }

  function keyDescriptor(key) {
    const named = {
      Enter: ["Enter", 13], Tab: ["Tab", 9], Escape: ["Escape", 27], Backspace: ["Backspace", 8],
      Delete: ["Delete", 46], ArrowUp: ["ArrowUp", 38], ArrowDown: ["ArrowDown", 40],
      ArrowLeft: ["ArrowLeft", 37], ArrowRight: ["ArrowRight", 39], Home: ["Home", 36],
      End: ["End", 35], PageUp: ["PageUp", 33], PageDown: ["PageDown", 34], Space: ["Space", 32]
    };
    if (named[key]) {
      const [code, virtualKey] = named[key];
      return { key: key === "Space" ? " " : key, code, windowsVirtualKeyCode: virtualKey, nativeVirtualKeyCode: virtualKey };
    }
    return { key, text: key };
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

  function browserEventFrame(event) {
    return { kind: "event", event: { ...event } };
  }

  function heartbeatAcknowledgement(frame) {
    if (frame?.kind !== "heartbeat"
      || !Number.isSafeInteger(frame.sequence)
      || frame.sequence < 1
      || frame.sequence > 0xffffffff) return null;
    return { kind: "heartbeat_ack", sequence: frame.sequence };
  }

  return Object.freeze({
    NATIVE_HOST_NAME,
    ADAPTER_PROTOCOL_MAJOR,
    ADAPTER_CAPABILITIES,
    bounded,
    readinessForStatus,
    isCredentialMetadata,
    modifierMask,
    keyDescriptor,
    presentationLabel,
    activityLabel,
    browserEventFrame,
    heartbeatAcknowledgement
  });
});
