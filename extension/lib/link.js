// Ghostlight -- the extension's native link of adapter protocol 4: the port to the browser relay,
// the hello and the wait for its answer, the retries, and the welcome page an install opens once.
// It holds the link's state in one object and decides nothing about frames: the worker routes them.
(function installGhostlightLink(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { protocol: require("./protocol.js"), shared: require("./shared.js") }
    : { protocol: root.GhostlightProtocol, shared: root.GhostlightShared });
  root.GhostlightLink = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightLinkApi(deps) {
  "use strict";

  const { protocol, shared } = deps;

  const WELCOME_PAGE = "setup.html";
  const NATIVE_ENDED = "Native connection ended.";
  const RECONNECT_ALARM = "ghostlight-reconnect";
  const RECONNECT_DELAY_MS = 3_000;
  const RECONNECT_ALARM_MINUTES = 0.05;
  const ERROR_CHARS = 500;
  const ENGINE_VERSION = /^[0-9.]{1,32}$/;
  const ENGINE_USER_AGENT = /Chrome\/([0-9.]{1,32})/;
  const PLACEHOLDER_BRAND = /not.*a.*brand/i;
  const ENGINE_BRAND = "Chromium";

  // How the first connection of this worker ended, as far as the welcome page cares.
  const FIRST_CONNECTION = Object.freeze({ ACCEPTED: "accepted", HOST_ABSENT: "host_absent", OTHER: "other" });

  // An error's words as the link keeps them: bounded, whatever was thrown.
  function errorText(error) {
    return shared.bounded(error?.message ?? error, ERROR_CHARS);
  }

  /**
   * The link for one worker. `chromeApi` supplies `runtime`, `alarms`, `tabs` and `windows`;
   * `navigatorApi` the user agent; `log` and `events` the connection log. `ready()` makes the
   * local state the hello names and answers `{ browserId, life }`. `onFrame(frame, port)` routes a
   * frame; `onClosed(reason)` settles what must not outlive the service. `setConnection(patch)`
   * reports the connection to the pages, and `adapterOutdated()` says whether the service refused
   * this extension as outdated.
   */
  function create({ chromeApi, navigatorApi, manifest, log, events, ready, onFrame, onClosed, setConnection, adapterOutdated }) {
    const state = {
      port: null,
      attempt: null,
      attempts: 0,
      helloTimer: null,
      helloUnanswered: false,
      retryTimer: null,
      backendLost: false,
      firstConnection: null,
      welcomePending: false
    };

    // The setup page opens once, for an install whose first connection finds the service absent.
    // Only a definite outcome settles it: a port that is open and unanswered is not one, because
    // Chrome reports a missing host through the disconnect a moment after connectNative returns. A
    // worker restart forgets the pending welcome and opens nothing.
    function settleWelcome(outcome) {
      state.firstConnection ??= outcome;
      if (!state.welcomePending) return;
      state.welcomePending = false;
      if (outcome === FIRST_CONNECTION.HOST_ABSENT) chromeApi.tabs.create({ url: chromeApi.runtime.getURL(WELCOME_PAGE) }).catch(() => {});
    }

    function expectWelcome() {
      state.welcomePending = true;
      if (state.firstConnection) settleWelcome(state.firstConnection);
    }

    function failedConnection(reason) {
      const absent = shared.linkState({ connected: false, compatible: false, lastError: reason }) === shared.LINK_HOST_ABSENT;
      settleWelcome(absent ? FIRST_CONNECTION.HOST_ABSENT : FIRST_CONNECTION.OTHER);
    }

    function send(frame) {
      if (!state.port) return false;
      const fitted = protocol.fitted(shared.wellFormed(frame));
      if (!fitted) return false;
      try {
        state.port.postMessage(fitted);
        return true;
      } catch (_error) {
        return false;
      }
    }

    function connect(trigger = "worker") {
      log.record(events.CONNECT_REQUESTED, {
        trigger: typeof trigger === "string" ? trigger : "event",
        has_port: Boolean(state.port),
        pending: Boolean(state.attempt)
      });
      if (state.port) return Promise.resolve();
      if (state.attempt) return state.attempt;
      state.attempt = establish().finally(() => { state.attempt = null; });
      return state.attempt;
    }

    // Whether this browser currently holds a focused window. Reported in the hello so a browser
    // that is already in front is routable at once. Connecting is not attention on its own.
    async function holdsFocusedWindow() {
      try {
        const window = await chromeApi.windows.getLastFocused();
        return Boolean(window?.focused);
      } catch (_error) {
        return false;
      }
    }

    // The browser engine's full version: informational for the service, never a routing key.
    async function engineVersion() {
      try {
        const high = await navigatorApi.userAgentData?.getHighEntropyValues?.(["fullVersionList"]);
        const entries = Array.isArray(high?.fullVersionList) ? high.fullVersionList : [];
        const named = entries.find((entry) => entry?.brand === ENGINE_BRAND)
          ?? entries.find((entry) => typeof entry?.brand === "string" && !PLACEHOLDER_BRAND.test(entry.brand));
        if (ENGINE_VERSION.test(named?.version)) return named.version;
      } catch (_error) { /* the user-agent string answers instead */ }
      return ENGINE_USER_AGENT.exec(navigatorApi.userAgent ?? "")?.[1];
    }

    // Everything asynchronous happens before the port opens, so the hello is the first frame on it.
    async function establish() {
      const attempt = ++state.attempts;
      let stage = "initialize";
      try {
        const identity = await ready();
        if (state.port) return;
        stage = "attention";
        const attended = await holdsFocusedWindow();
        const engine = await engineVersion();
        if (state.port) return;
        stage = "connect_native";
        const port = chromeApi.runtime.connectNative(protocol.NATIVE_HOST_NAME);
        log.record(events.NATIVE_PORT_OPENED, { attempt });
        state.port = port;
        state.backendLost = false;
        port.onMessage.addListener((frame) => {
          Promise.resolve(onFrame(frame, port)).catch((error) => setConnection({ last_error: errorText(error) }));
        });
        port.onDisconnect.addListener(() => closed(port, attempt));
        stage = "hello";
        send({
          kind: protocol.FRAME.HELLO,
          protocol: protocol.PROTOCOL,
          browser_id: identity.browserId,
          life: identity.life ?? undefined,
          browser_name: shared.browserName(navigatorApi.userAgentData?.brands) ?? undefined,
          platform: protocol.PLATFORM,
          engine_version: engine,
          adapter_version: manifest.version,
          attended
        });
        log.record(events.NATIVE_HELLO_SENT, { attempt, browser_id: identity.browserId });
        clearRetry();
        chromeApi.alarms.clear(RECONNECT_ALARM).catch(() => {});
        awaitHelloAnswer();
      } catch (error) {
        state.port = null;
        setConnection({ connected: false, last_error: errorText(error) });
        log.record(events.CONNECT_FAILED, { attempt, stage, error: errorText(error) });
        failedConnection(error?.message);
        scheduleRetry(attempt);
      }
    }

    // The wait for the service to answer a hello starts with the first hello and ends when a
    // hello_accepted arrives, or when the browser relay says no service is there at all (a
    // backend_unavailable whose service is absent), which reads as not connected. Nothing else ends
    // it: the service closing the socket, a bare backend_unavailable and the port being replaced do
    // not, because a 1.3.x service answers a protocol 4 hello that way, again and again, and never
    // accepts it.
    function awaitHelloAnswer() {
      if (state.helloUnanswered) return;
      state.helloUnanswered = true;
      state.helloTimer = setTimeout(helloTimedOut, protocol.HELLO_TIMEOUT_MS);
    }

    function endHelloWait() {
      state.helloUnanswered = false;
      if (state.helloTimer) clearTimeout(state.helloTimer);
      state.helloTimer = null;
    }

    // A service that has not accepted the hello in time is one this extension cannot work with.
    function helloTimedOut() {
      state.helloTimer = null;
      settleWelcome(FIRST_CONNECTION.OTHER);
      setConnection({ connected: false, compatible: false, service_outdated: true });
    }

    // The service accepted the hello.
    function accepted() {
      endHelloWait();
      settleWelcome(FIRST_CONNECTION.ACCEPTED);
    }

    // The service refused this extension's hello because it does not speak the service's protocol.
    // The link says to update Ghostlight in Browser until a later hello is accepted, and the
    // reconnect after the browser relay exits waits for the delayed retry instead of restarting the
    // native host at once.
    function refused(message) {
      log.record(events.ADAPTER_REFUSED);
      endHelloWait();
      settleWelcome(FIRST_CONNECTION.OTHER);
      const refusal = shared.bounded(message, ERROR_CHARS);
      setConnection({
        connected: false,
        compatible: false,
        service_outdated: false,
        adapter_outdated: true,
        adapter_refusal: refusal === "" ? null : refusal
      });
    }

    // The browser relay said the service went away, and whether none is there at all.
    function serviceLost(absent) {
      log.record(events.BACKEND_UNAVAILABLE);
      state.backendLost = true;
      if (absent) endHelloWait();
    }

    // A service took the link back after it was lost.
    function serviceBack() {
      state.backendLost = false;
    }

    function clearRetry() {
      if (state.retryTimer) clearTimeout(state.retryTimer);
      state.retryTimer = null;
    }

    function scheduleRetry(attempt) {
      if (!state.retryTimer) {
        state.retryTimer = setTimeout(() => {
          state.retryTimer = null;
          if (!state.port) connect("retry_timer");
        }, RECONNECT_DELAY_MS);
      }
      Promise.resolve().then(() => chromeApi.alarms.create(RECONNECT_ALARM, { delayInMinutes: RECONNECT_ALARM_MINUTES }))
        .then(() => chromeApi.alarms.get(RECONNECT_ALARM))
        .then((alarm) => log.record(events.RETRY_SCHEDULED, { attempt, scheduled_time: alarm?.scheduledTime ?? 0 }))
        .catch((error) => log.record(events.RETRY_FAILED, { attempt, error: errorText(error) }));
    }

    // The reconnect alarm fired.
    function alarmed(alarm) {
      if (alarm.name !== RECONNECT_ALARM) return;
      clearRetry();
      log.record(events.ALARM_FIRED, { scheduled_time: alarm.scheduledTime });
      connect("alarm");
    }

    // The port is gone. What must not outlive the service is settled, and the connection comes back
    // at once when the connector left on purpose after it said the service was unavailable, else
    // after a short delay. A service that refused this extension as outdated would refuse it again,
    // so that case always takes the delay.
    function closed(port, attempt, reason = chromeApi.runtime.lastError?.message || NATIVE_ENDED) {
      if (state.port !== port) return;
      state.port = null;
      const orderly = state.backendLost;
      state.backendLost = false;
      log.record(events.NATIVE_DISCONNECTED, { attempt, error: reason });
      failedConnection(reason);
      setConnection({ connected: false, last_error: reason });
      onClosed(reason);
      if (orderly && !adapterOutdated()) connect("service_returned");
      else scheduleRetry(attempt);
    }

    // The port is closed from this side, which Chrome does not report back as a disconnect.
    function close(port, reason) {
      try {
        port.disconnect();
      } catch (_error) { /* a port that is already gone needs no closing */ }
      closed(port, state.attempts, reason);
    }

    return Object.freeze({
      connect,
      send,
      close,
      accepted,
      refused,
      serviceLost,
      serviceBack,
      alarmed,
      expectWelcome,
      /** Whether `port` is the link's current port. */
      isCurrent: (port) => port === state.port,
      /** Whether the link has a port at all. */
      open: () => state.port !== null
    });
  }

  return Object.freeze({ create, errorText });
});
