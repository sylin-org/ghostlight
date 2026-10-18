// Ghostlight workbench -- the only thing here that talks to the orchestrator.
//
// Every call out of this page goes through one object, so what the surface can ask for is a list
// you can read in one sitting. It holds no state and touches no page: give it a way to fetch and a
// clipboard, and it will tell you what came back or throw trying.
//
// The page has one road to the service: JSON over loopback, in the Ghostlight window and in a
// browser tab alike. A command's name is looked up in one table that gives its method, path and
// body.
(function installGhostlightTransport(root, factory) {
  const api = factory();
  root.GhostlightTransport = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightTransportApi() {
  "use strict";

  /** The header an act answers with: the last change the act caused. */
  const SEQ_HEADER = "Ghostlight-Seq";
  /** The header a policy write names the rules it was drawn from by. */
  const IF_MATCH_HEADER = "If-Match";
  /** The path of the live stream. */
  const EVENTS_PATH = "/api/events";
  /** The code of a refusal that says the rules or the client changed since the page drew them. */
  const STALE = "stale";
  /**
   * The act keys of the context document, by the name this page uses for each.
   *
   * The service lists what the page may do under these keys. Both the page's boot and its view
   * ask by name from here, so a key the service renames or drops fails a test instead of drawing
   * a control the service then refuses.
   */
  const ACT = Object.freeze({
    search: "search",
    checkPolicy: "check_policy",
    rescan: "rescan",
    continueInWindow: "continue_in_window",
    hold: "hold",
    resume: "resume",
    startSession: "start_session",
    endSession: "end_session",
    applyPolicy: "apply_policy",
    removePolicy: "remove_policy",
    setDiagnostics: "set_diagnostics",
    setWatching: "set_watching",
    setupAll: "setup_all",
    harnessAct: "harness_act",
    testNotification: "test_notification",
    open: "open",
    revealDiagnostics: "reveal_diagnostics",
    locate: "locate",
    setupAct: "setup_act",
    quit: "quit"
  });
  /** The context in which the service shows nothing of this computer. */
  const CONTEXT_CLOSED = "closed";
  /** The opener of the window: the service opens a link for it. A tab opens its own links. */
  const OPENER_SERVICE = "service";
  /** What a client's setup can be copied as. */
  const COPY_KIND = Object.freeze({ command: "command", setup: "setup" });
  /** The status that says watching is off. */
  const STATUS_WATCHING_OFF = 403;
  /** The parts a snapshot is asked for, in the order the service lists them. */
  const SNAPSHOT_PARTS = Object.freeze(["status", "connections", "activity", "integrations", "diagnostics", "controls"]);
  /** How long a stream waits before it opens again, after each failure in a row. */
  const RECONNECT_STEPS_MS = Object.freeze([1000, 2000, 5000, 10000]);
  /** How long a stream may stay silent before it counts as dead. */
  const DEAD_AFTER_MS = 40000;

  /** How many bytes of an image are turned to text at a time, which keeps each call's arguments small. */
  const BASE64_CHUNK = 0x8000;

  const GET = "GET";
  const POST = "POST";
  const PUT = "PUT";
  const DELETE = "DELETE";

  /** The window view a page can ask to have brought forward. */
  const WORKBENCH_VIEW = "workbench";

  /**
   * Where each command goes over loopback.
   *
   * `body` turns a command's arguments, and the extra it quotes, into the JSON the route reads.
   * The extra is the state of the client a command was drawn with. A `tagged` command quotes the
   * tag of the rules it was drawn from in an `If-Match` header.
   */
  const ROUTES = Object.freeze({
    workbench_context: { method: GET, path: "/api/context" },
    workbench_snapshot: { method: GET, path: `/api/snapshot?parts=${SNAPSHOT_PARTS.join(",")}`, read: flatten },
    workbench_setup: { method: GET, path: "/api/snapshot?parts=setup", read: setupOf },
    workbench_search: { method: POST, path: "/api/search", body: ({ query }) => ({ query }) },
    workbench_policy: { method: GET, path: "/api/policy" },
    preview_user_policy: { method: POST, path: "/api/policy/preview", body: ({ document }) => ({ document }) },
    apply_user_policy: { method: PUT, path: "/api/policy/user", body: ({ document }) => ({ document }), tagged: true },
    remove_user_policy: { method: DELETE, path: "/api/policy/user", tagged: true },
    apply_runtime_intent: { method: POST, path: "/api/runtime", body: ({ intent }) => ({ intent }) },
    set_diagnostics: { method: PUT, path: "/api/controls/diagnostics", body: ({ on }) => ({ on }) },
    set_watching: { method: PUT, path: "/api/controls/viewing", body: ({ watch }) => ({ watch }) },
    setup_act: { method: POST, path: "/api/setup/act", body: (action) => action },
    refresh_harnesses: { method: POST, path: "/api/integrations/rescan", body: () => ({}) },
    setup_detected_harnesses: { method: POST, path: "/api/integrations/setup-all", body: () => ({}) },
    manage_harness: {
      method: POST,
      path: "/api/integrations/act",
      body: ({ id, action }, { expectedState }) => ({ id, action, expected_state: expectedState })
    },
    locate_harness: { method: POST, path: "/api/integrations/locate", body: ({ id }) => ({ id }) },
    test_notification: { method: POST, path: "/api/notifications/test", body: () => ({}) },
    reveal_diagnostics: { method: POST, path: "/api/diagnostics/reveal", body: () => ({}) },
    open_destination: { method: POST, path: "/api/open", body: ({ destination }) => ({ destination }) },
    open_harness_download: { method: POST, path: "/api/open", body: ({ id }) => ({ harness: id }) },
    continue_in_window: {
      method: POST,
      path: "/api/window",
      body: ({ address }) => (address ? { view: WORKBENCH_VIEW, address } : { view: WORKBENCH_VIEW })
    },
    quit_ghostlight: { method: POST, path: "/api/quit", body: () => ({}) }
  });

  /**
   * Put the parts of a snapshot answer back into the one flat shape the store and the view read.
   *
   * Pure: the answer in, the snapshot out. The counts the overview carries are derived from the
   * parts, so they cannot disagree with them.
   */
  function flatten(answer) {
    const parts = answer.parts ?? {};
    const status = parts.status ?? {};
    const connections = parts.connections ?? {};
    const activity = parts.activity ?? {};
    const integrations = parts.integrations ?? {};
    const diagnostics = parts.diagnostics ?? {};
    const controls = parts.controls;
    const sessions = connections.sessions ?? [];
    const browsers = connections.browsers ?? [];
    const operations = activity.operations ?? [];
    const history = activity.history ?? [];
    const flat = {
      seq: answer.seq,
      instance: answer.instance,
      service: status.service,
      readiness: status.readiness,
      audit_health: status.audit_health,
      audit_notice: status.audit_notice,
      configuration: status.configuration,
      overview: {
        active_sessions: sessions.length,
        active_operations: operations.length,
        connected_browsers: browsers.length,
        blocked_in_history: history.filter((item) => item.allowed === false).length
      },
      sessions,
      browsers,
      harnesses_in_use: connections.harnesses_in_use ?? [],
      operations,
      history,
      document_coverage: activity.document_coverage ?? [],
      harnesses: integrations.harnesses ?? [],
      diagnostics: diagnostics.diagnostics ?? []
    };
    if (status.runtime_note !== undefined) flat.runtime_note = status.runtime_note;
    if (connections.no_browser !== undefined) flat.no_browser = connections.no_browser;
    if (controls !== undefined) flat.controls = controls;
    return flat;
  }

  /**
   * The setup part of a snapshot answer, or null when the service leaves it out: a screen that
   * is shown the ordinary workbench while setup is under way is given no setup part.
   */
  function setupOf(answer) {
    return answer.parts?.setup ?? null;
  }

  /**
   * The change an act caused, when the service said so.
   *
   * The sequence rides on the result of an act, out of sight of anything that copies the result.
   */
  function seqOf(result) {
    return typeof result?.seq === "number" ? result.seq : undefined;
  }

  /** Every refusal this module made, so a caller can tell the service's sentence from an exception. */
  const refusals = new WeakSet();

  /**
   * A refusal, as the page reads it.
   *
   * It reads as the service's sentence wherever it is turned into text, and it also carries the
   * status and the code for a caller that cares.
   */
  function refusal(message, status, code) {
    const refused = Object.freeze({ message, status, code, toString: () => message });
    refusals.add(refused);
    return refused;
  }

  /**
   * Whether `value` is a refusal the service answered: its message is the service's own sentence,
   * which a person may read. Anything else that was thrown is an exception, whose text is not.
   */
  const isRefusal = (value) => typeof value === "object" && value !== null && refusals.has(value);

  /** The refusal an unsuccessful response holds. */
  async function refusalOf(response) {
    let envelope = null;
    try {
      envelope = JSON.parse(await response.text());
    } catch (error) {
      envelope = null;
    }
    const detail = envelope?.error;
    return refusal(detail?.message ?? `HTTP ${response.status}`, response.status, detail?.code);
  }

  /** The commands, sent as JSON to the service that served the page. */
  function overHttp({ fetch: send, clipboard, bearer, schedule, cancel } = {}) {
    const available = typeof send === "function";
    const later = schedule ?? ((work, ms) => setTimeout(work, ms));
    const earlier = cancel ?? ((handle) => clearTimeout(handle));

    function headers(withBody) {
      const sent = {};
      if (withBody) sent["Content-Type"] = "application/json";
      if (bearer) sent.Authorization = `Bearer ${bearer}`;
      return sent;
    }

    async function call(command, args = {}, extra = {}) {
      if (!available) throw new Error("no orchestrator is attached");
      const route = ROUTES[command];
      const body = route.body ? route.body(args, extra) : undefined;
      const sent = headers(body !== undefined);
      if (route.tagged && extra.etag !== undefined) sent[IF_MATCH_HEADER] = `"${extra.etag}"`;
      const response = await send(route.path, {
        method: route.method,
        headers: sent,
        body: body === undefined ? undefined : JSON.stringify(body),
        credentials: "omit",
        cache: "no-store"
      });
      if (!response.ok) throw await refusalOf(response);
      const text = await response.text();
      let result = text ? JSON.parse(text) : null;
      if (route.read) result = route.read(result);
      const seq = response.headers?.get?.(SEQ_HEADER);
      if (seq !== null && seq !== undefined && result !== null && typeof result === "object") {
        Object.defineProperty(result, "seq", { value: Number(seq), enumerable: false, configurable: true });
      }
      return result;
    }

    /**
     * An image the service serves, as a source the page can draw.
     *
     * The service answers an image only to a request that carries the page's key, or to a watcher
     * while watching is on, so the page asks with `fetch` like any other call and draws the bytes
     * it was given.
     */
    async function image(address) {
      if (!available) throw new Error("no orchestrator is attached");
      const response = await send(`/${address}`, {
        method: GET,
        headers: headers(false),
        credentials: "omit"
      });
      if (!response.ok) throw await refusalOf(response);
      const kind = response.headers?.get?.("Content-Type") ?? "";
      const bytes = new Uint8Array(await response.arrayBuffer());
      let text = "";
      for (let at = 0; at < bytes.length; at += BASE64_CHUNK) {
        text += String.fromCharCode(...bytes.subarray(at, at + BASE64_CHUNK));
      }
      return `data:${kind};base64,${btoa(text)}`;
    }

    /**
     * Read the live stream until it ends, and open it again after each failure.
     *
     * An event the service names is passed to the handler of that name. Any other event is a
     * change, handed over as `{ payload }`. A stream that ends, fails or says nothing for too long
     * counts as lost until its next hello.
     */
    function subscribe(handlers) {
      let stopped = false;
      let lost = false;
      let step = 0;
      let controller = null;
      let deadTimer = null;
      let wake = null;

      /** A handler that throws is the page's fault to report, never a reason to drop the stream. */
      function safely(work) {
        try {
          work();
        } catch (error) {
          Promise.reject(error);
        }
      }

      function lose() {
        if (lost) return;
        lost = true;
        safely(() => handlers.reconnecting?.(true));
      }

      function arm() {
        earlier(deadTimer);
        deadTimer = later(() => controller?.abort(), DEAD_AFTER_MS);
      }

      function dispatch(block) {
        let name = "message";
        const lines = [];
        for (const line of block.split("\n")) {
          if (line.startsWith(":")) continue;
          if (line.startsWith("event:")) name = line.slice("event:".length).trim();
          else if (line.startsWith("data:")) lines.push(line.slice("data:".length).replace(/^ /, ""));
        }
        if (!lines.length) return;
        let data;
        try {
          data = JSON.parse(lines.join("\n"));
        } catch (error) {
          return;
        }
        if (name === "hello") {
          if (lost) {
            lost = false;
            safely(() => handlers.reconnecting?.(false));
          }
          safely(() => handlers.hello?.(data));
        } else if (name === "closed") {
          safely(() => handlers.closed?.(data));
        } else if (name === "reset") {
          safely(() => handlers.reset?.(data));
        } else {
          safely(() => handlers.change?.({ payload: data }));
        }
      }

      async function read(response) {
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffered = "";
        for (;;) {
          const { done, value } = await reader.read();
          if (done) return;
          arm();
          step = 0;
          buffered = (buffered + decoder.decode(value, { stream: true })).replace(/\r\n/g, "\n");
          let end = buffered.indexOf("\n\n");
          while (end >= 0) {
            const block = buffered.slice(0, end);
            buffered = buffered.slice(end + 2);
            dispatch(block);
            end = buffered.indexOf("\n\n");
          }
        }
      }

      function pause(ms) {
        return new Promise((resolve) => {
          const handle = later(resolve, ms);
          wake = () => {
            earlier(handle);
            resolve();
          };
        });
      }

      async function run() {
        while (!stopped) {
          controller = new AbortController();
          try {
            arm();
            const response = await send(EVENTS_PATH, {
              method: GET,
              headers: headers(false),
              credentials: "omit",
              cache: "no-store",
              signal: controller.signal
            });
            if (response.status === STATUS_WATCHING_OFF) {
              const refused = await refusalOf(response);
              stop();
              safely(() => handlers.closed?.({ sentence: refused.message }));
              return;
            }
            if (!response.ok) throw await refusalOf(response);
            await read(response);
          } catch (error) {
            if (stopped) return;
          }
          earlier(deadTimer);
          if (stopped) return;
          lose();
          await pause(RECONNECT_STEPS_MS[Math.min(step, RECONNECT_STEPS_MS.length - 1)]);
          step += 1;
        }
      }

      function stop() {
        stopped = true;
        earlier(deadTimer);
        controller?.abort();
        wake?.();
      }

      run();
      return stop;
    }

    return Object.freeze({
      available,
      ...commands(call),

      image,

      /** Put text on the clipboard of the screen that asked. */
      copyText(text) {
        if (typeof clipboard?.writeText !== "function") return Promise.reject(new Error("no clipboard is attached"));
        return clipboard.writeText(text);
      },

      subscribe
    });
  }

  /**
   * The commands, written against a way to call.
   *
   * `call(command, args, extra)` carries `extra`: the tag of the rules a policy write was drawn
   * from, and the state of the client an act on it was drawn with.
   */
  function commands(call) {
    return {
      /** The three-way answer to what this screen may do, and the words for it. */
      context: () => call("workbench_context"),

      /** The whole current truth, which the store then decides what to do with. */
      snapshot: () => call("workbench_snapshot"),

      /**
       * The setup part alone. It is read on its own, not with the snapshot, because an unfinished
       * setup reads this computer and a resync must not pay for that.
       */
      setupPart: () => call("workbench_setup"),

      /** Start a browser on a step of the setup view; answers the setup part as it is then. */
      setupAct: (action) => call("setup_act", action),

      /** Ranked matches for the palette. */
      search: (query) => call("workbench_search", { query }),

      /** Hold, resume, end or start the runtime session. */
      applyIntent: (intent) => call("apply_runtime_intent", { intent }),

      /** Turn process diagnostics on or off; returns the resulting state. */
      setDiagnostics: (on) => call("set_diagnostics", { on }),

      /** Turn "Others can watch" on or off; returns the controls part the window now shows. */
      setWatching: (watch) => call("set_watching", { watch }),

      /** Open the diagnostics folder through the service. */
      revealDiagnostics: () => call("reveal_diagnostics"),

      /** The compiled policy: what applies here, and who decided each line. */
      policy: () => call("workbench_policy"),

      /**
       * What a candidate policy would have done to work this machine already did.
       *
       * Read-only. The orchestrator decides recorded history against the candidate and writes
       * nothing, so this can be asked as often as the person edits.
       *
       * The policy text is named `policy` here rather than `document`: this module must never
       * reach the page, and a local binding by that name only invites someone to try.
       */
      previewPolicy: (policy) => call("preview_user_policy", { document: policy }),

      /** Replace this machine's own rules with one complete policy text, drawn from the rules `etag` names. */
      applyPolicy: (policy, etag) => call("apply_user_policy", { document: policy }, { etag }),

      /** Remove this machine's own rules, drawn from the rules `etag` names. */
      removePolicy: (etag) => call("remove_user_policy", undefined, { etag }),

      /** Re-detect the MCP clients installed for this user. */
      refreshHarnesses: () => call("refresh_harnesses"),

      /** Set up or update every detected MCP target the orchestrator can change safely. */
      setupDetectedHarnesses: () => call("setup_detected_harnesses"),

      /** Set up, update, fix, or remove Ghostlight from one concrete MCP target, drawn in `expectedState`. */
      manageHarness: (id, action, expectedState) => call("manage_harness", { id, action }, { expectedState }),

      /** Ask the service to have the person locate one supported harness executable or settings file. */
      locateHarness: (id) => call("locate_harness", { id }),

      /** Open the official download page of one host, named by its harness id. */
      openHarnessDownload: (id) => call("open_harness_download", { id }),

      /** Prove the notification path end to end. */
      testNotification: () => call("test_notification"),

      /**
       * Open one of the places the product is willing to point at.
       *
       * A name, never an address: the orchestrator owns the URL, so this page cannot be talked
       * into opening something the product did not choose.
       */
      openDestination: (destination) => call("open_destination", { destination }),

      /**
       * Bring the Ghostlight application forward, where changes are made, at `address` (one of the
       * page's addresses) when one is given. The service checks the address.
       */
      continueInWindow: (address) => call("continue_in_window", { address }),

      /** Quit Ghostlight, as the tray does. Answers the sentence the page then shows. */
      quit: () => call("quit_ghostlight")
    };
  }

  return Object.freeze({
    overHttp, flatten, seqOf, isRefusal, STALE, SNAPSHOT_PARTS, ACT, CONTEXT_CLOSED, OPENER_SERVICE, COPY_KIND
  });
});
