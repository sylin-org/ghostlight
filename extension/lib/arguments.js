// Ghostlight -- the argument checks the primitives of adapter protocol 4 share.
(function installGhostlightArguments(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { protocol: require("./protocol.js"), shared: require("./shared.js") }
    : { protocol: root.GhostlightProtocol, shared: root.GhostlightShared });
  root.GhostlightArguments = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightArgumentsApi(deps) {
  "use strict";

  const { protocol, shared } = deps;

  // Every primitive checks its arguments before it touches the browser, and a violation is an
  // `invalid_request` that names what is wrong. These are the checks that more than one primitive
  // needs: ids, bounded text, whole numbers, a document guard, a frame reference, a world call, and
  // a tab id that names an open tab.
  // They read only the fields the protocol names. What a world message means is the service's
  // business, so a message is checked for its shape and never for its kind.

  const { INVALID_REQUEST, TAB_MISSING } = protocol.RECEIPT_FAILURE;
  const ID_CHARS = 128;
  // The longest message a receipt carries.
  const MESSAGE_CHARS = 500;

  /** Builds the `invalid_request` failure every check throws. */
  function invalid(message) {
    return protocol.failure(INVALID_REQUEST, message);
  }

  /** True for a plain object: not null, not an array. */
  function isObject(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }

  /** True when an optional argument was left out (absent or null). */
  function absent(value) {
    return value === undefined || value === null;
  }

  /** Requires the request arguments to be an object. */
  function requireArgs(args) {
    if (!isObject(args)) throw invalid("The request arguments must be an object.");
  }

  /** Requires `value` to be a whole number within the bounds. `name` is how the error calls it. */
  function requireInteger(value, name, { minimum = 0, maximum = Number.MAX_SAFE_INTEGER } = {}) {
    if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
      throw invalid(maximum === Number.MAX_SAFE_INTEGER
        ? `${name} must be a whole number of at least ${minimum}.`
        : `${name} must be a whole number from ${minimum} to ${maximum}.`);
    }
    return value;
  }

  /** Requires a positive tab id. */
  function requireTabId(value, name = "tab_id") {
    return requireInteger(value, name, { minimum: 1 });
  }

  /** Requires a number that is not infinite and not NaN. */
  function requireFinite(value, name) {
    if (typeof value !== "number" || !Number.isFinite(value)) throw invalid(`${name} must be a number.`);
    return value;
  }

  /** Requires true or false. */
  function requireBoolean(value, name) {
    if (typeof value !== "boolean") throw invalid(`${name} must be true or false.`);
    return value;
  }

  // The characters in `text` as the service counts them: Unicode code points, so an emoji is one
  // character and not the two UTF-16 code units of its `length`. Every text limit counts these.
  function characters(text) {
    return [...text].length;
  }

  /** Requires text of at most `maximum` characters; an empty string passes only with `empty`. */
  function requireText(value, name, maximum, { empty = false } = {}) {
    if (typeof value !== "string" || (!empty && value.length === 0) || (value.length > maximum && characters(value) > maximum)) {
      throw invalid(`${name} must be text of at most ${maximum} characters.`);
    }
    return value;
  }

  /** Requires a frame id or a loader id: non-empty bounded text. */
  function requireId(value, name) {
    return requireText(value, name, ID_CHARS);
  }

  /** Requires a frame reference `{ frame_id, loader_id }`. */
  function requireFrameRef(ref, name = "frame") {
    if (!isObject(ref)) throw invalid(`${name} must name a frame and its loader.`);
    requireId(ref.frame_id, `${name}.frame_id`);
    requireId(ref.loader_id, `${name}.loader_id`);
    return ref;
  }

  /** Requires one admitted document `{ frame_id, loader_id, url }`. */
  function requireDocument(document, name) {
    requireFrameRef(document, name);
    requireText(document.url, `${name}.url`, protocol.URL_CHARS, { empty: true });
    return document;
  }

  /**
   * Requires a document guard `{ documents, exact, unseen? }`. An absent guard passes: a guard is
   * optional wherever the protocol lists one.
   */
  function optionalGuard(guard, name = "guard") {
    if (absent(guard)) return;
    if (!isObject(guard)) throw invalid(`${name} must be an object.`);
    if (!Array.isArray(guard.documents) || guard.documents.length > protocol.MAX_WORLD_CALLS) {
      throw invalid(`${name}.documents must list at most ${protocol.MAX_WORLD_CALLS} documents.`);
    }
    guard.documents.forEach((document, index) => requireDocument(document, `${name}.documents[${index}]`));
    requireBoolean(guard.exact, `${name}.exact`);
    if (!absent(guard.unseen)) requireInteger(guard.unseen, `${name}.unseen`);
  }

  /** Requires a world call `{ frame?, message, owner_of? }`. The message needs a kind and nothing more. */
  function requireWorldCall(call, name = "call") {
    if (!isObject(call)) throw invalid(`${name} must be an object.`);
    if (!absent(call.frame)) requireFrameRef(call.frame, `${name}.frame`);
    if (!isObject(call.message) || typeof call.message.kind !== "string" || call.message.kind.length === 0) {
      throw invalid(`${name}.message must be an object with a kind.`);
    }
    if (!absent(call.owner_of)) requireId(call.owner_of, `${name}.owner_of`);
    return call;
  }

  /**
   * Requires that `tabId` names an open tab, and answers it. `tabsApi` is `chrome.tabs`. A tab the
   * browser does not have is `tab_missing`, with the browser's own words.
   */
  async function requireTab(tabsApi, tabId) {
    try {
      return await tabsApi.get(tabId);
    } catch (error) {
      throw protocol.failure(TAB_MISSING, shared.bounded(error?.message ?? `No tab with id: ${tabId}.`, MESSAGE_CHARS));
    }
  }

  /**
   * A primitive's handler spec, the one shape the dispatcher reads: `validate` and `run` as given,
   * `addresses(args)` from a flag or a function of the arguments (whether the primitive acts on a
   * tab the dispatcher must take custody of), and `tabOf(args)`, the tab's id, `args.tab_id`
   * unless given.
   */
  function handler({ validate, run, addresses, tabOf = (args) => args.tab_id }) {
    return Object.freeze({ validate, run, addresses: typeof addresses === "function" ? addresses : () => addresses, tabOf });
  }

  return Object.freeze({
    handler,
    invalid,
    isObject,
    absent,
    requireArgs,
    requireInteger,
    requireTabId,
    requireFinite,
    requireBoolean,
    requireText,
    requireId,
    requireFrameRef,
    requireDocument,
    optionalGuard,
    requireWorldCall,
    requireTab
  });
});
