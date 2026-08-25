(function ghostlightContent() {
  "use strict";

  const shared = globalThis.GhostlightShared;
  // Frame transparency (ADR-0138): this instance may live in an embedded frame. Perpetual
  // visuals belong to the top document only; target-anchored transients render wherever
  // their element lives, because that is what the person should see.
  const IS_TOP = window.self === window.top;
  const ACTIONABLE_SELECTOR = "a[href],button,input,textarea,select,summary,[role],[contenteditable='true']";
  const locators = new Map();
  const reverse = new WeakMap();
  let nextLocator = 1;
  let dragObservation = null;

  function finishDragObservation() {
    if (!dragObservation) return { started: false, cancelled: false };
    const result = {
      started: dragObservation.started,
      cancelled: dragObservation.cancelled
    };
    window.removeEventListener("dragstart", dragObservation.listener, true);
    dragObservation = null;
    return result;
  }

  function armDragObservation() {
    finishDragObservation();
    const observation = { started: false, cancelled: false, listener: null };
    observation.listener = (event) => {
      observation.started = true;
      queueMicrotask(() => {
        if (dragObservation === observation) observation.cancelled = event.defaultPrevented;
      });
    };
    dragObservation = observation;
    window.addEventListener("dragstart", observation.listener, true);
    return { armed: true };
  }

  function dragObservationStatus() {
    return dragObservation
      ? { started: dragObservation.started, cancelled: dragObservation.cancelled }
      : { started: false, cancelled: false };
  }

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

  function renderedText(element) {
    const rendered = typeof element?.innerText === "string" ? element.innerText : "";
    const fallback = typeof element?.textContent === "string" ? element.textContent : "";
    return (rendered || fallback).replace(/\s+/g, " ").trim();
  }

  function accessibleName(element) {
    const labelledBy = element.getAttribute("aria-labelledby");
    if (labelledBy) {
      const root = element.getRootNode();
      const text = labelledBy.split(/\s+/).map((id) => renderedText(root.getElementById?.(id))).join(" ").trim();
      if (text) return shared.bounded(text, 500);
    }
    const aria = element.getAttribute("aria-label");
    if (aria) return shared.bounded(aria, 500);
    if (element.labels?.length) return shared.bounded(Array.from(element.labels).map(renderedText).join(" ").trim(), 500);
    const tag = String(element.tagName ?? "").toLowerCase();
    const type = String(element.getAttribute("type") ?? "").toLowerCase();
    const buttonLikeInput = tag === "input" && ["button", "submit", "reset"].includes(type);
    const fixed = element.getAttribute("alt") || element.getAttribute("title") || element.getAttribute("placeholder") || (buttonLikeInput ? element.getAttribute("value") : "");
    if (fixed) return shared.bounded(fixed, 500).trim();
    const editable = tag === "input" || tag === "textarea" || tag === "select" || element.isContentEditable;
    if (editable) return "";
    return shared.bounded(renderedText(element), 500);
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

  // The refusal names the exact control so a model can ask its human for one precise thing.
  function credentialHandoffError(element) {
    const name = shared.bounded(accessibleName(element) || element.id || "", 80);
    return new Error(`credential-class target requires user handoff: the ${roleFor(element)}${name ? ` "${name}"` : ""}`);
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

  function matchesSemanticSelector(element, message) {
    const needle = String(message.name ?? "").trim().toLocaleLowerCase();
    if (!needle) return false;
    const name = accessibleName(element).toLocaleLowerCase();
    const hit = message.exact ? name === needle : name.includes(needle);
    if (!hit) return false;
    if (message.role && roleFor(element) !== message.role) return false;
    if (message.form_scope) {
      const labeledInForm = Array.from(element.labels ?? []).some((label) => label.closest("form"));
      if (!(element.closest && element.closest("form")) && !labeledInForm) return false;
    }
    return true;
  }

  function querySemanticTargets(message) {
    const matches = [];
    for (const element of candidates("controls").slice(0, 3000)) {
      if (!element.isConnected) continue;
      if (matchesSemanticSelector(element, message)) {
        matches.push(observation(element));
        if (matches.length >= 8) break;
      }
    }
    return matches;
  }

  function extractArticle() {
    const candidates = Array.from(document.querySelectorAll("article, main, [role='main'], [itemprop='articleBody']"));
    for (const element of candidates) {
      const text = String(element.innerText ?? "").trim();
      if (text.length >= 80) return text;
    }
    return String((document.body || document.documentElement)?.innerText ?? "").trim();
  }

  function inspectTree(rootElement, maxDepth) {
    const controlRoles = ["button", "link", "textbox", "checkbox", "radio", "combobox", "tab", "menuitem", "option", "slider"];
    let count = 0;
    function visible(element) {
      if (!(element instanceof HTMLElement)) return false;
      if (element.hidden || element.getAttribute?.("aria-hidden") === "true") return false;
      const style = window.getComputedStyle(element);
      return style.display !== "none" && style.visibility !== "hidden";
    }
    function build(element, depth) {
      count += 1;
      const role = roleFor(element);
      const kind = controlRoles.includes(role) ? "control" : /^h[1-6]$/.test(element.tagName.toLowerCase()) ? "heading" : "container";
      const node = { kind, label: shared.bounded(accessibleName(element), 120), children: [] };
      if (depth < maxDepth && count < 400) {
        for (const child of element.children) {
          if (count >= 400) break;
          if (!visible(child)) continue;
          node.children.push(build(child, depth + 1));
        }
        if (element.shadowRoot && count < 400) node.children.push(build(element.shadowRoot, depth + 1));
      }
      return node;
    }
    const tree = build(rootElement, 1);
    return { tree, truncated: count >= 400 };
  }

  function setNativeValue(element, value) {
    const prototype = element instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
    if (setter) setter.call(element, value); else element.value = value;
  }

  function fillElement(element, value) {
    requireActionable(element, "fill");
    if (credentialClass(element)) throw credentialHandoffError(element);
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

  function requireActionable(element, intent) {
    if (!element.isConnected) throw new Error("stale browser locator");
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    if (style.display === "none" || element.getAttribute("aria-hidden") === "true" || style.visibility === "hidden" || Number(style.opacity) === 0 || rect.width <= 0 || rect.height <= 0) {
      // Naming the exact predicate is the difference between a dead end and a decision: the
      // driver can scroll, reopen, or report instead of guessing what "not visible" meant.
      const reason =
        style.display === "none" ? "display:none"
        : element.getAttribute("aria-hidden") === "true" ? "aria-hidden"
        : style.visibility === "hidden" ? "visibility:hidden"
        : Number(style.opacity) === 0 ? "opacity:0"
        : "zero-size";
      throw new Error(`target is not visible for ${intent} (${reason})`);
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
    // Activation replies before it dispatches. A click whose handler opens a page-blocking
    // dialog (window.prompt, confirm, alert) freezes this page's main thread inside the
    // dispatch, so a reply that waited for the dispatch to finish could never arrive. Every
    // step that can fail or throw runs first; sendResponse crosses to the service worker while
    // the thread is still live; only the validated dispatch follows.
    if (message.kind === "activate") {
      try {
        const element = requireActionable(resolve(message.locator), "activate");
        const subject = actionSubject(element);
        element.scrollIntoView({ block: "center", inline: "center" });
        const plan = shared.activationPlan(message);
        sendResponse({ ok: true, result: { activated: true, subject } });
        if (plan.native) element.click();
        else for (const init of plan.clicks) element.dispatchEvent(new MouseEvent("click", init));
      } catch (error) {
        sendResponse({ ok: false, error: String(error?.message ?? error) });
      }
      return false;
    }
    // Form fill with a submit control follows the same rule as activation: the submit click is
    // the dispatch tail, and a submit handler that opens a page-blocking dialog would freeze
    // this thread mid-click. Everything that can fail or throw runs first; the reply crosses to
    // the service worker while the thread is still live; only the verified submit follows.
    if (message.kind === "fill") {
      try {
        const elements = message.fields.map((field) => resolve(field.locator));
        elements.forEach((element, index) => fillElement(element, message.fields[index].value));
        let submitElement = null;
        if (message.submit_locator) {
          submitElement = resolve(message.submit_locator);
          const owner = elements[0]?.closest?.("form") ?? null;
          if (!owner || !owner.contains(submitElement)) throw new Error("submit control is not contained in the resolved form");
        }
        sendResponse({ ok: true, result: { filled_count: message.fields.length, submitted: Boolean(submitElement) } });
        if (submitElement) submitElement.click();
      } catch (error) {
        sendResponse({ ok: false, error: String(error?.message ?? error) });
      }
      return false;
    }
    // Perpetual state is a top-document decision; embedded frames acknowledge and ignore.
    if (!IS_TOP && (message.kind === "managed_scope" || message.kind === "recording_state" || message.kind === "runtime_state")) {
      sendResponse({ ok: true, result: { gated: true } });
      return false;
    }
    Promise.resolve().then(async () => {
      if (message.kind === "read_text") {
        let whole;
        if (message.locator) whole = String(resolve(message.locator).innerText ?? "");
        else if (message.mode === "article") whole = extractArticle();
        else whole = String((document.body || document.documentElement)?.innerText ?? "");
        return { text: whole.slice(0, message.max_chars), truncated: whole.length > message.max_chars, title: shared.bounded(document.title, 500), url: location.href };
      }
      if (message.kind === "inspect_tree") {
        const root = message.locator ? resolve(message.locator) : document.body || document.documentElement;
        const built = inspectTree(root, message.max_depth ?? 6);
        return { tree: built.tree, truncated: built.truncated };
      }
      if (message.kind === "inspect") return { targets: inspect(message.inspect_kind, message.max_items) };
      if (message.kind === "find") return { targets: findTargets(message.text, message.find_kind, message.max_results) };
      if (message.kind === "describe") return { targets: message.locators.map((locator) => observation(resolve(locator))) };
      if (message.kind === "query_semantic") return { targets: querySemanticTargets(message) };
      if (message.kind === "describe_focused") { const element = document.activeElement; if (!element || element === document.body || element === document.documentElement) throw new Error("no editable control is focused"); return { targets: [observation(element)] }; }
      if (message.kind === "clear_focused") { const element = requireActionable(document.activeElement, "type"); if (credentialClass(element)) throw credentialHandoffError(element); const subject = actionSubject(element); if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) setNativeValue(element, ""); else if (element.isContentEditable) element.textContent = ""; else throw new Error("target is not text-editable"); element.dispatchEvent(new Event("input", { bubbles: true, composed: true })); return { cleared: true, subject }; }
      if (message.kind === "drop_files") {
        const dropTarget = document.elementFromPoint(message.x, message.y);
        if (!dropTarget) throw new Error("no element is at the drop point");
        const transfer = new DataTransfer();
        for (const file of message.files) transfer.items.add(decodeFile(file));
        dropTarget.dispatchEvent(new DragEvent("drop", { bubbles: true, cancelable: true, composed: true, dataTransfer: transfer }));
        return { uploaded_count: message.files.length, uploaded_bytes: message.files.reduce((sum, file) => sum + file.size, 0) };
      }
      if (message.kind === "box") { const element = requireActionable(resolve(message.locator), "box"); return { rectangle: viewportRectangle(element), subject: actionSubject(element) }; }
      if (message.kind === "scroll_offset") return { x: scrollX, y: scrollY };
      if (message.kind === "focus") { const element = requireActionable(resolve(message.locator), "focus"); const subject = actionSubject(element); element.scrollIntoView({ block: "center", inline: "center" }); element.focus({ preventScroll: true }); return { focused: true, subject }; }
      if (message.kind === "clear") { const element = requireActionable(resolve(message.locator), "type"); if (credentialClass(element)) throw credentialHandoffError(element); const subject = actionSubject(element); if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) setNativeValue(element, ""); else if (element.isContentEditable) element.textContent = ""; else throw new Error("target is not text-editable"); element.dispatchEvent(new Event("input", { bubbles: true, composed: true })); return { cleared: true, subject }; }
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
      if (message.kind === "drag_observation_arm") return armDragObservation();
      if (message.kind === "drag_observation_status") return dragObservationStatus();
      if (message.kind === "drag_observation_finish") return finishDragObservation();
      if (message.kind === "upload_files") { const element = resolve(message.locator); const subject = actionSubject(element); return { ...uploadFiles(element, message.files), subject }; }
      if (message.kind === "observe") return observe(message);
      if (message.kind === "present") {
        if (!IS_TOP && !message.signal?.locator) return { presented: false };
        return { presented: renderPresentation(message.signal, message.preferences) };
      }
      if (message.kind === "managed_scope") { globalThis.GhostlightPresentation.setManaged(message.active); return { managed: Boolean(message.active) }; }
      if (message.kind === "presentation_visibility") { globalThis.GhostlightPresentation.setHidden(message.hidden); return { hidden: Boolean(message.hidden) }; }
      if (message.kind === "recording_state") { globalThis.GhostlightPresentation.setRecording(message.active); return { recording: Boolean(message.active) }; }
      if (message.kind === "runtime_state") { globalThis.GhostlightPresentation.setRuntimeState(message.state); return { state: message.state }; }
      throw new Error("unknown content primitive");
    }).then((result) => sendResponse({ ok: true, result })).catch((error) => sendResponse({ ok: false, error: String(error?.message ?? error) }));
    return true;
  });
})();
