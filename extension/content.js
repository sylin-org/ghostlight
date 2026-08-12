(function ghostlightContent() {
  "use strict";

  const shared = globalThis.GhostlightShared;
  const ACTIONABLE_SELECTOR = "a[href],button,input,textarea,select,summary,[role],[contenteditable='true']";
  const locators = new Map();
  const reverse = new WeakMap();
  let nextLocator = 1;

  function locatorFor(element) {
    let locator = reverse.get(element);
    if (!locator) {
      locator = `locator_${nextLocator++}`;
      reverse.set(element, locator);
      locators.set(locator, element);
    }
    return locator;
  }

  function resolve(locator) {
    const element = locators.get(locator);
    if (!element || !element.isConnected) throw new Error("stale browser locator");
    return element;
  }

  function accessibleName(element) {
    const labelledBy = element.getAttribute("aria-labelledby");
    if (labelledBy) {
      const root = element.getRootNode();
      const text = labelledBy.split(/\s+/).map((id) => root.getElementById?.(id)?.textContent ?? "").join(" ").trim();
      if (text) return shared.bounded(text, 500);
    }
    const aria = element.getAttribute("aria-label");
    if (aria) return shared.bounded(aria, 500);
    if (element.labels?.length) return shared.bounded(Array.from(element.labels).map((label) => label.textContent ?? "").join(" ").trim(), 500);
    const tag = String(element.tagName ?? "").toLowerCase();
    const type = String(element.getAttribute("type") ?? "").toLowerCase();
    const buttonLikeInput = tag === "input" && ["button", "submit", "reset"].includes(type);
    const fixed = element.getAttribute("alt") || element.getAttribute("title") || element.getAttribute("placeholder") || (buttonLikeInput ? element.getAttribute("value") : "");
    if (fixed) return shared.bounded(fixed, 500).trim();
    const editable = tag === "input" || tag === "textarea" || tag === "select" || element.isContentEditable;
    if (editable) return "";
    return shared.bounded(element.innerText || element.textContent || "", 500).trim();
  }

  function roleFor(element) {
    const explicit = element.getAttribute("role");
    if (explicit) return shared.bounded(explicit, 100);
    const tag = element.tagName.toLowerCase();
    const type = String(element.getAttribute("type") ?? "").toLowerCase();
    if (tag === "a") return "link";
    if (tag === "button") return "button";
    if (tag === "select") return "combobox";
    if (tag === "textarea") return "textbox";
    if (tag === "input" && type === "checkbox") return "checkbox";
    if (tag === "input" && type === "radio") return "radio";
    if (tag === "input" && ["button", "submit", "reset"].includes(type)) return "button";
    if (tag === "input") return "textbox";
    if (/^h[1-6]$/.test(tag)) return "heading";
    return tag;
  }

  function stateFor(element) {
    const state = [];
    if (element.disabled || element.getAttribute("aria-disabled") === "true") state.push("disabled");
    if (element.checked || element.getAttribute("aria-checked") === "true") state.push("checked");
    if (element.getAttribute("aria-expanded") === "true") state.push("expanded");
    if (element.getAttribute("aria-expanded") === "false") state.push("collapsed");
    if (element.selected) state.push("selected");
    if (element.hidden || element.getAttribute("aria-hidden") === "true") state.push("hidden");
    return state.slice(0, 8);
  }

  function credentialClass(element) {
    return shared.isCredentialMetadata({
      type: element.getAttribute("type"),
      autocomplete: element.getAttribute("autocomplete"),
      name: element.getAttribute("name"),
      id: element.id
    });
  }

  function observation(element) {
    return {
      locator: locatorFor(element),
      role: roleFor(element),
      name: accessibleName(element),
      state: stateFor(element),
      credential_class: credentialClass(element)
    };
  }

  function actionSubject(element) {
    return {
      role: roleFor(element),
      name: accessibleName(element)
    };
  }

  function subjectAtViewportPoint(x, y) {
    const hit = document.elementFromPoint?.(x, y);
    if (!hit) return null;
    return actionSubject(hit.closest?.(ACTIONABLE_SELECTOR) || hit);
  }

  function roots() {
    const found = [document];
    for (let index = 0; index < found.length; index += 1) {
      for (const element of found[index].querySelectorAll("*")) {
        if (element.shadowRoot) found.push(element.shadowRoot);
      }
    }
    return found;
  }

  function queryAll(selector) {
    const unique = new Set();
    for (const root of roots()) {
      for (const element of root.querySelectorAll(selector)) unique.add(element);
    }
    return Array.from(unique);
  }

  function candidates(kind) {
    const controls = ACTIONABLE_SELECTOR;
    const structure = "main,nav,header,footer,form,table,ul,ol,h1,h2,h3,h4,h5,h6,section,article";
    const selector = kind === "controls" ? controls : kind === "structure" ? structure : `${controls},${structure}`;
    return queryAll(selector);
  }

  function inspect(kind, maximum) {
    return candidates(kind).filter((element) => element.isConnected).slice(0, maximum).map(observation);
  }

  function findTargets(text, kind, maximum) {
    const needle = text.toLocaleLowerCase();
    const pool = kind === "control" ? candidates("controls") : queryAll("a,button,input,textarea,select,[role],p,span,li,h1,h2,h3,h4,h5,h6,label");
    const matches = [];
    for (const element of pool.slice(0, 3000)) {
      const haystack = `${accessibleName(element)} ${element.innerText ?? element.textContent ?? ""}`.toLocaleLowerCase();
      const isControl = element.matches("a[href],button,input,textarea,select,summary,[role],[contenteditable='true']");
      if (haystack.includes(needle) && (kind !== "control" || isControl) && (kind !== "text" || !isControl)) {
        matches.push(observation(element));
        if (matches.length >= maximum) break;
      }
    }
    return matches;
  }

  function setNativeValue(element, value) {
    const prototype = element instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
    if (setter) setter.call(element, value); else element.value = value;
  }

  function fillElement(element, value) {
    requireActionable(element, "fill");
    if (credentialClass(element)) throw new Error("credential-class target requires user handoff");
    element.scrollIntoView({ block: "center", inline: "center" });
    element.focus({ preventScroll: true });
    if (element instanceof HTMLSelectElement) {
      const option = Array.from(element.options).find((candidate) => candidate.value === value || candidate.text === value);
      if (!option) throw new Error("select option not found");
      element.value = option.value;
    } else if (element instanceof HTMLInputElement && ["checkbox", "radio"].includes(element.type)) {
      element.checked = ["true", "1", "yes", "on"].includes(String(value).toLowerCase());
    } else if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
      setNativeValue(element, value);
    } else if (element.isContentEditable) {
      element.textContent = value;
    } else {
      throw new Error("target is not fillable");
    }
    element.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
    element.dispatchEvent(new Event("change", { bubbles: true, composed: true }));
  }

  function geometry(element) {
    const rect = element.getBoundingClientRect();
    return {
      x: rect.left + window.scrollX,
      y: rect.top + window.scrollY,
      width: rect.width,
      height: rect.height
    };
  }

  function requireActionable(element, intent) {
    if (!element.isConnected) throw new Error("stale browser locator");
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    if (style.display === "none" || style.visibility === "hidden" || Number(style.opacity) === 0 || rect.width <= 0 || rect.height <= 0) {
      throw new Error(`target is not visible for ${intent}`);
    }
    if (element.disabled || element.getAttribute("aria-disabled") === "true" || element.closest("[inert]")) {
      throw new Error(`target is disabled for ${intent}`);
    }
    return element;
  }

  function viewportRectangle(element) {
    const rect = element.getBoundingClientRect();
    return { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
  }

  function renderPresentation(signal, preferences) {
    let rectangle = null;
    if (signal.locator) {
      try { rectangle = viewportRectangle(resolve(signal.locator)); } catch (_error) { /* target is optional presentation */ }
    }
    return globalThis.GhostlightPresentation.render(signal, preferences, rectangle);
  }

  function decodeFile(file) {
    const binary = atob(file.data);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
    if (bytes.byteLength !== file.size) throw new Error("file payload size mismatch");
    return new File([bytes], file.name, { type: file.media_type });
  }

  function requireUploadTarget(element) {
    if (!element.isConnected) throw new Error("stale browser locator");
    if (!(element instanceof HTMLInputElement) || element.type !== "file") {
      throw new Error("target is not a file input");
    }
    if (element.disabled || element.getAttribute("aria-disabled") === "true" || element.closest("[inert]")) {
      throw new Error("target is disabled for upload");
    }
    return element;
  }

  function uploadFiles(element, files) {
    requireUploadTarget(element);
    const transfer = new DataTransfer();
    for (const file of files) transfer.items.add(decodeFile(file));
    element.files = transfer.files;
    element.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
    element.dispatchEvent(new Event("change", { bubbles: true, composed: true }));
    return { uploaded_count: transfer.files.length, uploaded_bytes: files.reduce((sum, file) => sum + file.size, 0) };
  }

  async function observe(message) {
    const started = performance.now();
    const deadline = started + message.timeout_ms;
    while (true) {
      let satisfied = false;
      if (message.condition === "load_ready") satisfied = document.readyState === "interactive" || document.readyState === "complete";
      if (message.condition === "url_contains") satisfied = location.href.includes(message.value);
      if (message.condition === "text_present") satisfied = (document.body?.innerText ?? "").includes(message.value);
      if (message.condition === "text_absent") satisfied = !(document.body?.innerText ?? "").includes(message.value);
      if (message.condition === "target_present") satisfied = Boolean(locators.get(message.locator)?.isConnected);
      if (message.condition === "target_absent") satisfied = !locators.get(message.locator)?.isConnected;
      if (satisfied) return { satisfied: true, elapsed_ms: Math.round(performance.now() - started), readiness: document.readyState === "complete" ? "complete" : "interactive" };
      const remaining = deadline - performance.now();
      if (remaining <= 0) break;
      await new Promise((resolvePromise) => setTimeout(resolvePromise, Math.min(100, remaining)));
    }
    return { satisfied: false, elapsed_ms: Math.round(performance.now() - started), readiness: document.readyState === "complete" ? "complete" : document.readyState === "interactive" ? "interactive" : "loading" };
  }

  chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
    Promise.resolve().then(async () => {
      if (message.kind === "read_text") {
        const source = message.locator ? resolve(message.locator) : document.body || document.documentElement;
        const whole = String(source.innerText ?? source.textContent ?? "");
        return { text: whole.slice(0, message.max_chars), truncated: whole.length > message.max_chars, title: shared.bounded(document.title, 500), url: location.href };
      }
      if (message.kind === "inspect") return { targets: inspect(message.inspect_kind, message.max_items) };
      if (message.kind === "find") return { targets: findTargets(message.text, message.find_kind, message.max_results) };
      if (message.kind === "describe") return { targets: message.locators.map((locator) => observation(resolve(locator))) };
      if (message.kind === "geometry") return geometry(resolve(message.locator));
      if (message.kind === "focus") { const element = requireActionable(resolve(message.locator), "focus"); const subject = actionSubject(element); element.scrollIntoView({ block: "center", inline: "center" }); element.focus({ preventScroll: true }); return { focused: true, subject }; }
      if (message.kind === "clear") { const element = requireActionable(resolve(message.locator), "type"); if (credentialClass(element)) throw new Error("credential-class target requires user handoff"); const subject = actionSubject(element); if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) setNativeValue(element, ""); else if (element.isContentEditable) element.textContent = ""; else throw new Error("target is not text-editable"); element.dispatchEvent(new Event("input", { bubbles: true, composed: true })); return { cleared: true, subject }; }
      if (message.kind === "activate") {
        const element = requireActionable(resolve(message.locator), "activate");
        const subject = actionSubject(element);
        element.scrollIntoView({ block: "center", inline: "center" });
        if (message.button === "primary" && message.click_count === 1) element.click();
        else for (let count = 0; count < message.click_count; count += 1) element.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, composed: true, button: message.button === "middle" ? 1 : 2, detail: message.click_count }));
        return { activated: true, subject };
      }
      if (message.kind === "fill") {
        for (const field of message.fields) fillElement(resolve(field.locator), field.value);
        if (message.submit_locator) resolve(message.submit_locator).click();
        return { filled_count: message.fields.length, submitted: Boolean(message.submit_locator) };
      }
      if (message.kind === "scroll") {
        let subject = null;
        if (message.locator) { const element = requireActionable(resolve(message.locator), "scroll"); subject = actionSubject(element); element.scrollIntoView({ block: "center", inline: "center", behavior: "instant" }); }
        else {
          const viewport = message.amount === "small" ? 0.25 : message.amount === "large" ? 0.75 : message.amount === "page" ? 0.95 : 0.5;
          const horizontal = message.direction === "left" ? -innerWidth * viewport : message.direction === "right" ? innerWidth * viewport : 0;
          const vertical = message.direction === "up" ? -innerHeight * viewport : message.direction === "down" ? innerHeight * viewport : 0;
          scrollBy({ left: horizontal, top: vertical, behavior: "instant" });
        }
        return { x: scrollX, y: scrollY, subject };
      }
      if (message.kind === "scroll_point") {
        const margin = 24;
        if (message.x < scrollX + margin || message.x > scrollX + innerWidth - margin || message.y < scrollY + margin || message.y > scrollY + innerHeight - margin) {
          scrollTo({ left: Math.max(0, message.x - innerWidth / 2), top: Math.max(0, message.y - innerHeight / 2), behavior: "instant" });
        }
        const x = message.x - scrollX;
        const y = message.y - scrollY;
        return { x, y, subject: subjectAtViewportPoint(x, y) };
      }
      if (message.kind === "viewport_point") {
        const x = message.x - scrollX;
        const y = message.y - scrollY;
        if (x < 0 || y < 0 || x >= innerWidth || y >= innerHeight) throw new Error("drag point is outside the current viewport");
        return { x, y, subject: subjectAtViewportPoint(x, y) };
      }
      if (message.kind === "hover") { const element = requireActionable(resolve(message.locator), "hover"); element.scrollIntoView({ block: "center", inline: "center", behavior: "instant" }); return { rectangle: viewportRectangle(element), subject: actionSubject(element) }; }
      if (message.kind === "drag_geometry") { const source = requireActionable(resolve(message.source_locator), "drag"); const destination = requireActionable(resolve(message.destination_locator), "drop"); source.scrollIntoView({ block: "center", inline: "center", behavior: "instant" }); return { source: viewportRectangle(source), destination: viewportRectangle(destination), source_subject: actionSubject(source), destination_subject: actionSubject(destination) }; }
      if (message.kind === "upload_files") { const element = resolve(message.locator); const subject = actionSubject(element); return { ...uploadFiles(element, message.files), subject }; }
      if (message.kind === "observe") return observe(message);
      if (message.kind === "present") return { presented: renderPresentation(message.signal, message.preferences) };
      if (message.kind === "managed_scope") { globalThis.GhostlightPresentation.setManaged(message.active); return { managed: Boolean(message.active) }; }
      if (message.kind === "presentation_visibility") { globalThis.GhostlightPresentation.setHidden(message.hidden); return { hidden: Boolean(message.hidden) }; }
      if (message.kind === "recording_state") { globalThis.GhostlightPresentation.setRecording(message.active); return { recording: Boolean(message.active) }; }
      if (message.kind === "runtime_state") { globalThis.GhostlightPresentation.setRuntimeState(message.state); return { state: message.state }; }
      throw new Error("unknown content primitive");
    }).then((result) => sendResponse({ ok: true, result })).catch((error) => sendResponse({ ok: false, error: String(error?.message ?? error) }));
    return true;
  });
})();
