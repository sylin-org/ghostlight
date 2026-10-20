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

  return Object.freeze({
    vocabulary,
    isKnown,
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
    HELLO_TIMEOUT_MS: vocabulary.limits.hello_timeout_ms
  });
});
