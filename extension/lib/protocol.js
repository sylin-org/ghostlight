// Ghostlight -- the closed vocabulary and bounds of adapter protocol 4.
(function installGhostlightProtocol(root, factory) {
  const api = factory();
  root.GhostlightProtocol = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightProtocolApi() {
  "use strict";

  // The closed adapter protocol 4 vocabulary. tests/golden/protocol4/vocabulary.json is the shared
  // drift source: a test compares this object with that file, and nothing reads the file at runtime.
  // The extension never adds a name here that the service has not agreed to in that file.
  function deepFreeze(value) {
    if (value !== null && typeof value === "object") {
      for (const member of Object.values(value)) deepFreeze(member);
      Object.freeze(value);
    }
    return value;
  }

  const vocabulary = deepFreeze({
    protocol: 4,
    world_name: "ghostlight",
    native_host_name: "org.sylin.ghostlight",
    platform: "ghostlight/chromium",
    frame_kinds: {
      extension_to_service: ["hello", "receipt", "heartbeat_ack", "event"],
      service_to_extension: ["hello_accepted", "request", "command_chunk", "cancel", "notice", "control_state", "heartbeat", "error"],
      relay_to_extension: ["backend_unavailable"]
    },
    primitives: ["list_tabs", "open_tab", "navigate", "focus_tab", "close_tab", "set_zoom", "resize_window", "frames", "call",
      "evaluate_main", "view", "input", "capture", "dialog", "diagnostics", "recording", "accessibility_snapshot"],
    receipt_failures: ["unsupported", "invalid_request", "tab_missing", "debugger_unavailable", "document_changed", "dialog_open",
      "timeout", "cancelled", "local_interlock", "capture_too_large", "browser_refused", "session_lost", "no_history_entry", "other"],
    call_failures: ["document_changed", "frame_missing", "owner_missing", "dialog_open", "timeout", "cancelled", "session_lost",
      "page_exception", "unserializable", "other"],
    call_answers: ["returned", "failed", "skipped"],
    events: ["document_committed", "readiness_changed", "dialog_changed", "tab_closed", "attended", "runtime_control_requested", "show_requested"],
    runtime_control_intents: ["toggle_hold", "hold", "resume", "end_session", "start_session"],
    input_steps: ["mouse", "key", "insert_text", "drag", "pause"],
    mouse_actions: ["moved", "pressed", "released", "wheel"],
    mouse_buttons: ["none", "left", "middle", "right"],
    key_actions: ["down", "up", "raw_down", "char"],
    clips: ["viewport", "full_page", "viewport_rect", "page_rect"],
    call_modes: ["sequence", "parallel", "race"],
    navigation_targets: ["url", "back", "forward", "reload"],
    dialog_ops: ["status", "handle"],
    diagnostics_ops: ["read", "clear"],
    diagnostic_sources: ["both", "console", "network"],
    diagnostic_details: ["problems", "all"],
    recording_ops: ["start", "status", "stop", "export", "discard"],
    recording_destinations: ["download", "client", "page"],
    readiness: ["loading", "complete", "unknown"],
    control_states: ["active", "held", "ended"],
    runtime_states: ["active", "held", "ended", "disconnected"],
    recording_stop_reasons: ["document_boundary", "explicit", "hard_timeout", "memory_limit", "browser_detached", "runtime_held",
      "service_disconnected", "frame_too_large"],
    unknown_dialog_type: "unknown",
    group_title_prefix: "\u{1F47B} ",
    legacy_group_title_prefixes: ["Ghostlight - "],
    group_title_chars: 120,
    chunk_limits: { chunk_payload_bytes: 524288, transfer_max_bytes: 8388608, transfer_max_chunks: 64 },
    clear_diagnostics_tabs: 256,
    limits: {
      direct_frame_bytes: 786432,
      max_world_calls: 256,
      max_input_steps: 4096,
      max_unseen: 64,
      max_ax_nodes: 5000,
      max_pause_ms: 1000,
      resize_width: { min: 320, max: 7680 },
      resize_height: { min: 240, max: 4320 },
      diagnostics_limit: { min: 1, max: 200 },
      url_chars: 8192,
      title_chars: 500,
      hello_timeout_ms: 10000
    }
  });

  // True when `value` is a member of the closed list named `list` (a vocabulary key such as "primitives").
  function isKnown(list, value) {
    const members = vocabulary[list];
    if (!Array.isArray(members)) throw new TypeError(`unknown vocabulary list: ${list}`);
    return members.includes(value);
  }

  // A closed list as a frozen map from each word's upper-snake name to the word, so a module writes
  // protocol.RECEIPT_FAILURE.TAB_MISSING instead of spelling the word again.
  function named(list) {
    return Object.freeze(Object.fromEntries(list.map((word) => [word.toUpperCase(), word])));
  }

  const RECEIPT_FAILURE = named(vocabulary.receipt_failures);
  const DIALOG_OPEN_MESSAGE = "A JavaScript dialog blocks the page.";

  /**
   * Builds a coded failure: an Error whose `code` is a protocol 4 receipt or call failure code.
   * Callers throw or reject with it so the worker can report the code unchanged.
   */
  function failure(code, message) {
    return Object.assign(new Error(message), { code });
  }

  // A tab group the service names is titled with `group_title_prefix` and its agent's name. Earlier
  // releases used the prefixes in `legacy_group_title_prefixes` (2.0.0 wrote
  // "Ghostlight - <agent>"), so a group found under one is the same agent's group.

  /** The agent a group title names: the title without the current or a retired prefix, or the title as it is. */
  function groupAgentName(title) {
    for (const prefix of [vocabulary.group_title_prefix, ...vocabulary.legacy_group_title_prefixes]) {
      if (title.startsWith(prefix)) return title.slice(prefix.length);
    }
    return title;
  }

  /** The titles earlier releases gave the group the current title `title` names; none for any other title. */
  function legacyGroupTitles(title) {
    if (!title.startsWith(vocabulary.group_title_prefix)) return [];
    const agent = title.slice(vocabulary.group_title_prefix.length);
    return vocabulary.legacy_group_title_prefixes.map((prefix) => `${prefix}${agent}`);
  }

  /**
   * The failure for work that needs a renderer a JavaScript dialog is blocking: the receipt code
   * `dialog_open` with its one message. Anything that would wait on the page throws it at once.
   */
  function dialogOpen() {
    return failure(RECEIPT_FAILURE.DIALOG_OPEN, DIALOG_OPEN_MESSAGE);
  }

  return Object.freeze({
    vocabulary,
    isKnown,
    failure,
    dialogOpen,
    groupAgentName,
    legacyGroupTitles,
    DIALOG_OPEN_MESSAGE,
    FRAME: named(Object.values(vocabulary.frame_kinds).flat()),
    EVENT: named(vocabulary.events),
    RECEIPT_FAILURE,
    CALL_FAILURE: named(vocabulary.call_failures),
    RUNTIME_STATE: named(vocabulary.runtime_states),
    RECORDING_STOP_REASON: named(vocabulary.recording_stop_reasons),
    UNKNOWN_DIALOG_TYPE: vocabulary.unknown_dialog_type,
    CHUNK_PAYLOAD_BYTES: vocabulary.chunk_limits.chunk_payload_bytes,
    TRANSFER_MAX_BYTES: vocabulary.chunk_limits.transfer_max_bytes,
    TRANSFER_MAX_CHUNKS: vocabulary.chunk_limits.transfer_max_chunks,
    PROTOCOL: vocabulary.protocol,
    WORLD_NAME: vocabulary.world_name,
    NATIVE_HOST_NAME: vocabulary.native_host_name,
    PLATFORM: vocabulary.platform,
    DIRECT_FRAME_BYTES: vocabulary.limits.direct_frame_bytes,
    MAX_WORLD_CALLS: vocabulary.limits.max_world_calls,
    MAX_INPUT_STEPS: vocabulary.limits.max_input_steps,
    MAX_UNSEEN: vocabulary.limits.max_unseen,
    MAX_AX_NODES: vocabulary.limits.max_ax_nodes,
    MAX_PAUSE_MS: vocabulary.limits.max_pause_ms,
    URL_CHARS: vocabulary.limits.url_chars,
    TITLE_CHARS: vocabulary.limits.title_chars,
    GROUP_TITLE_CHARS: vocabulary.group_title_chars,
    HELLO_TIMEOUT_MS: vocabulary.limits.hello_timeout_ms
  });
});
