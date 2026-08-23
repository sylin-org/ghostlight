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
  const ADAPTER_CAPABILITY_REVISIONS = Object.freeze({
    script: 2,
    pointer_input: 2,
    keyboard_input: 2,
    semantic_document: 3,
    navigation: 2,
    files: 2
  });
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
    "adapter_liveness",
    "adapter_attention"
  ].map((name) => Object.freeze({ name, revision: ADAPTER_CAPABILITY_REVISIONS[name] ?? 1 })));
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

  function dragPackets(start, end, steps = 12) {
    if (!Number.isSafeInteger(steps) || steps < 1 || steps > 60) {
      throw new RangeError("drag steps must be from 1 through 60");
    }
    const packets = [
      { type: "mouseMoved", x: start.x, y: start.y },
      { type: "mousePressed", x: start.x, y: start.y, button: "left", clickCount: 1 }
    ];
    for (let step = 1; step <= steps; step += 1) {
      const ratio = step / steps;
      packets.push({
        type: "mouseMoved",
        x: start.x + (end.x - start.x) * ratio,
        y: start.y + (end.y - start.y) * ratio,
        button: "left",
        buttons: 1,
        force: 1
      });
    }
    packets.push({
      type: "mouseReleased",
      x: end.x,
      y: end.y,
      button: "left",
      clickCount: 1
    });
    return packets;
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

  // Chromium names its browser in the user-agent brand list. The service never routes on this
  // string; it exists so a person or a model can tell two connected browsers apart.
  const BROWSER_NAME_MAX = 40;
  const GENERIC_BRANDS = /not.*a.*brand|chromium/i;

  function browserName(brands) {
    if (!Array.isArray(brands)) return null;
    const named = brands
      .map((entry) => entry?.brand)
      .filter((brand) => typeof brand === "string" && brand.trim() && !GENERIC_BRANDS.test(brand));
    // The most specific brand is last: Chromium, then Chrome, then Edge on an Edge build.
    const chosen = named[named.length - 1];
    return chosen ? bounded(chosen, BROWSER_NAME_MAX) : null;
  }

  // Chrome reports a missing native-messaging host through the disconnect reason, which is the one
  // local signal that separates "Ghostlight was never installed on this computer" from "the service
  // is not running right now". A browser profile syncs the extension onto a new machine; the native
  // host does not travel with it, so this is the ordinary state on a second computer, not an edge
  // case. The match is a narrowing hint on Chrome's wording, never a contract: anything we do not
  // recognize stays the ordinary unreachable state, because a vague answer beats a wrong one.
  const NATIVE_HOST_ABSENT_MARKER = "native messaging host not found";

  const LINK_CONNECTED = "connected";
  const LINK_UNREACHABLE = "unreachable";
  const LINK_HOST_ABSENT = "host_absent";

  function linkState({ connected, compatible, lastError }) {
    if (connected && compatible) return LINK_CONNECTED;
    const reason = typeof lastError === "string" ? lastError.toLowerCase() : "";
    if (reason.includes(NATIVE_HOST_ABSENT_MARKER)) return LINK_HOST_ABSENT;
    return LINK_UNREACHABLE;
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
    dragPackets,
    presentationLabel,
    activityLabel,
    browserEventFrame,
    browserName,
    heartbeatAcknowledgement,
    NATIVE_HOST_ABSENT_MARKER,
    LINK_CONNECTED,
    LINK_UNREACHABLE,
    LINK_HOST_ABSENT,
    linkState
  });
});
