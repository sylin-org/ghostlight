// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- the isolated-world page runtime, adapter protocol 4.
//
// This file is the body of a function. The extension wraps it as
// `function(__ghostlightAssetId) { <this file> }` and runs it once per document in the
// `ghostlight` isolated world. The include marker below is replaced by the other protocol-4 page
// sources, so everything they define is a closure binding here and nothing is a page global. The
// only property left on the world's global is the frozen runtime object at the end.
"use strict";
const ID = __ghostlightAssetId;
const previous = globalThis.__ghostlightRuntime;
if (previous?.id === ID) return;
try { previous?.dispose(); } catch (_) {}
const IS_TOP = window.self === window.top;
const EPOCH = Array.from(crypto.getRandomValues(new Uint8Array(4)), (b) => b.toString(16).padStart(2, "0")).join("");
const disposers = [];
const tokens = new Map();
const world = Object.freeze({ IS_TOP, EPOCH, onDispose: (fn) => disposers.push(fn) });
class AssetError extends Error {
  constructor(code, message, detail) {
    super(message);
    this.code = code;
    if (detail !== undefined) this.detail = detail;
  }
}
const CODES = new Set([
  "stale_locator",
  "not_visible",
  "disabled",
  "read_only",
  "not_editable",
  "not_fillable",
  "option_missing",
  "credential_authorization_required",
  "submit_outside_form",
  "not_file_input",
  "no_focused_control",
  "focus_lost",
  "outside_viewport",
  "no_subject",
  "unknown_kind",
  "invalid_request",
  "runtime_missing",
  "cancelled",
  "unsatisfied",
  "not_retained",
  "payload_mismatch",
  "page_exception"
]);
/*@include page-shared.js sensor.js presentation-css.js presentation.js content.js*/
const failure = (e) => {
  const error = { code: CODES.has(e?.code) ? e.code : "page_exception", message: String(e?.message ?? e).slice(0, 500) };
  if (e instanceof AssetError && e.detail !== undefined) error.detail = e.detail;
  return { ok: false, error };
};
globalThis.__ghostlightRuntime = Object.freeze({
  id: ID,
  async call(message, { local, token, attachment, owner } = {}) {
    if (typeof message?.kind !== "string") return failure(new AssetError("invalid_request", "world call requires a kind"));
    const signal = { cancelled: false };
    if (token) tokens.set(token, signal);
    try {
      if (local) applyAmbient(local);
      return { ok: true, value: (await dispatch(message, { signal, attachment, owner })) ?? null };
    } catch (e) {
      return failure(e);
    } finally {
      if (token) tokens.delete(token);
    }
  },
  cancel(token) {
    const s = tokens.get(token);
    if (s) s.cancelled = true;
    return Boolean(s);
  },
  ambient(local) {
    applyAmbient(local);
    return true;
  },
  present(signal, local) {
    return renderPresentation(signal, local?.preferences ?? {});
  },
  dispose() {
    for (const s of tokens.values()) s.cancelled = true;
    tokens.clear();
    for (const fn of disposers.splice(0).reverse()) {
      try { fn(); } catch (_) {}
    }
    if (globalThis.__ghostlightRuntime?.id === ID) delete globalThis.__ghostlightRuntime;
  }
});
