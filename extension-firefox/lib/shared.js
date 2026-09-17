(function installGhostlightShared(root, factory) {
  if (typeof dump === "function") {
    dump("[Ghostlight Firefox] lib/shared.js loading...\n");
  }
  const api = factory();
  root.GhostlightShared = api;
  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  }
})(globalThis, function createGhostlightShared() {
  "use strict";

  const NATIVE_HOST_NAME = "org.sylin.ghostlight";
  const ADAPTER_PROTOCOL_MAJOR = 2;
  const BROWSER_PLATFORM = "ghostlight/gecko";
  const BROWSER_NAME = "Firefox";

  const ADAPTER_CAPABILITIES = Object.freeze([
    { name: "document_scope", revision: 1 },
    { name: "tabs", revision: 1 },
    { name: "atomic_tab_open", revision: 1 },
    { name: "navigation", revision: 2 },
    { name: "capture", revision: 2 },
    { name: "script", revision: 2 },
    { name: "window_geometry", revision: 1 },
    { name: "presentation", revision: 1 },
    { name: "adapter_liveness", revision: 1 },
    { name: "adapter_attention", revision: 1 }
  ]);

  function bounded(value, maximum) {
    return String(value ?? "").slice(0, maximum);
  }

  function readinessForStatus(status) {
    if (status === "loading") return "loading";
    if (status === "complete") return "complete";
    return "unknown";
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

  const NATIVE_HOST_ABSENT_MARKER = "no such native application";

  const LINK_CONNECTED = "connected";
  const LINK_UNREACHABLE = "unreachable";
  const LINK_HOST_ABSENT = "host_absent";

  function linkState({ connected, compatible, lastError }) {
    if (connected && compatible) return LINK_CONNECTED;
    const reason = typeof lastError === "string" ? lastError.toLowerCase() : "";
    if (reason.includes(NATIVE_HOST_ABSENT_MARKER) || reason.includes("not found")) return LINK_HOST_ABSENT;
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
    BROWSER_PLATFORM,
    BROWSER_NAME,
    ADAPTER_CAPABILITIES,
    bounded,
    readinessForStatus,
    presentationLabel,
    activityLabel,
    browserEventFrame,
    heartbeatAcknowledgement,
    NATIVE_HOST_ABSENT_MARKER,
    LINK_CONNECTED,
    LINK_UNREACHABLE,
    LINK_HOST_ABSENT,
    linkState
  });
});
