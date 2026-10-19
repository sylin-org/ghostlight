// Ghostlight -- the small helpers the worker and the other extension modules share: bounded text,
// readiness and activity vocabulary, the browser name, the link classification and the heartbeat reply.
(function installGhostlightShared(root, factory) {
  const api = factory();
  root.GhostlightShared = api;
  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  }
})(globalThis, function createGhostlightShared() {
  "use strict";

  function bounded(value, maximum) {
    return String(value ?? "").slice(0, maximum);
  }

  function readinessForStatus(status) {
    if (status === "loading") return "loading";
    if (status === "complete") return "complete";
    return "unknown";
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
  const LINK_SERVICE_OUTDATED = "service_outdated";
  const LINK_ADAPTER_OUTDATED = "adapter_outdated";

  // The code of the service's `error` frame that refuses a hello this extension sent because the
  // service does not speak its protocol: this extension is the side to update.
  const ADAPTER_INCOMPATIBLE = "adapter_incompatible";

  // The closed classification a surface renders. A service that refused this extension as outdated
  // said so outright, so that outranks everything; an outdated service is known from the hello that
  // never came, so it outranks a disconnect reason; a connected link is never reclassified.
  function linkState({ connected, compatible, outdated = false, adapterOutdated = false, lastError }) {
    if (adapterOutdated) return LINK_ADAPTER_OUTDATED;
    if (outdated) return LINK_SERVICE_OUTDATED;
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
    bounded,
    readinessForStatus,
    activityLabel,
    browserEventFrame,
    browserName,
    heartbeatAcknowledgement,
    NATIVE_HOST_ABSENT_MARKER,
    LINK_CONNECTED,
    LINK_UNREACHABLE,
    LINK_HOST_ABSENT,
    LINK_SERVICE_OUTDATED,
    LINK_ADAPTER_OUTDATED,
    ADAPTER_INCOMPATIBLE,
    linkState
  });
});
