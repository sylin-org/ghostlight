// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- the page-side primitives of adapter protocol 4. Included by world.js.
//
// Everything here is a closure binding: the page cannot see, call or replace it. The module hands
// the runtime three bindings: dispatch(message, ctx) runs one world message, applyAmbient(local)
// applies the extension-owned facts, and renderPresentation(signal, preferences) draws one
// signal. Frame transparency (ADR-0138): this instance may live in an embedded frame. Perpetual
// visuals belong to the top document only; target-anchored transients render wherever their
// element lives, because that is what the person should see.
//
// Every failure is an AssetError whose code the service reads; the message is the sentence the
// model may see. An effect kind throws a pre-effect code only before its first mutation, so a
// coded refusal always means the page is unchanged. Loops check ctx.signal.cancelled and end with
// the `cancelled` code.
const { dispatch, applyAmbient, renderPresentation } = (() => {
  const ACTIONABLE_SELECTOR = "a[href],button,input,textarea,select,summary,[role],[contenteditable='true']";
  const TEXT_FIND_SELECTOR = "a,button,input,textarea,select,[role],p,span,li,h1,h2,h3,h4,h5,h6,label,div,[draggable],[aria-label],[tabindex]";
  const TEXT_INPUT_TYPES = new Set(["text", "search", "tel", "url", "email", "number", "password"]);
  const TEXT_BLOCK_TAGS = new Set([
    "address", "article", "aside", "blockquote", "div", "dl", "fieldset", "figcaption",
    "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header",
    "hr", "li", "main", "nav", "ol", "p", "pre", "section", "table", "tr", "ul"
  ]);
  const TEXT_OMIT_TAGS = new Set(["input", "noscript", "option", "script", "select", "style", "template", "textarea"]);
  const DOCUMENT_TREE_NODE_LIMIT = 400;
  // What a masked credential value reads as. One short marker keeps masking cheap to read.
  const MASKED_VALUE = "[hidden]";
  // The accessibility role an element has by its tag alone, beyond the control roles roleFor
  // names. A tag that is not in this table has no role of its own and reads as generic.
  const IMPLICIT_ROLES = new Map([
    ["nav", "navigation"], ["main", "main"], ["header", "banner"], ["footer", "contentinfo"],
    ["aside", "complementary"], ["form", "form"], ["section", "region"], ["article", "article"],
    ["table", "table"], ["ul", "list"], ["ol", "list"], ["li", "listitem"],
    ["fieldset", "group"], ["img", "image"], ["summary", "button"]
  ]);
  const FILL_STABLE_MS = 750;
  const FILL_SETTLE_LIMIT_MS = 8_000;
  const FILL_RETAINED_STABLE_MS = 250;
  const FILL_RETAINED_POLL_MS = 25;
  const OBSERVE_POLL_MS = 100;
  // How long a sampled read waits without a page change before it answers.
  const QUIET_READ_MS = 300;
  const FILL_SETTLE_POLL_MS = 50;
  const LOCATOR_PREFIX = `locator_${world.EPOCH}_`;
  const EMBED_TAGS = new Set(["iframe", "frame"]);
  const MAX_LOCATORS = 500;
  const locators = new Map();
  const reverse = new WeakMap();
  let nextLocator = 1;

  function pruneDisconnectedLocators() {
    for (const [loc, el] of locators) {
      if (!el.isConnected) locators.delete(loc);
    }
  }

  function locatorFor(element) {
    let locator = reverse.get(element);
    if (!locator) {
      if (locators.size >= MAX_LOCATORS) pruneDisconnectedLocators();
      locator = `${LOCATOR_PREFIX}${nextLocator++}`;
      reverse.set(element, locator);
      locators.set(locator, element);
    }
    return locator;
  }

  function staleLocator() {
    return new AssetError("stale_locator", "stale browser locator");
  }

  // A locator minted by another world (a rebuilt page runtime) never aliases a new element.
  function resolve(locator) {
    if (typeof locator !== "string" || !locator.startsWith(LOCATOR_PREFIX)) throw staleLocator();
    const element = locators.get(locator);
    if (!element || !element.isConnected) {
      if (element) locators.delete(locator);
      throw staleLocator();
    }
    return element;
  }

  function cancelledError() {
    return new AssetError("cancelled", "operation cancelled");
  }

  function throwIfCancelled(ctx) {
    if (ctx.signal.cancelled) throw cancelledError();
  }

  function sleep(ms) {
    return new Promise((resolvePromise) => setTimeout(resolvePromise, ms));
  }

  function renderedText(element) {
    const rendered = typeof element?.innerText === "string" ? element.innerText : "";
    const fallback = typeof element?.textContent === "string" ? element.textContent : "";
    return (rendered || fallback).replace(/\s+/g, " ").trim();
  }

  function isComposedVisible(element) {
    if (element.hidden || element.getAttribute?.("aria-hidden") === "true") return false;
    const style = window.getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden" || style.contentVisibility === "hidden" || Number(style.opacity) === 0) return false;
    const rectangles = element.getClientRects?.();
    return !rectangles || rectangles.length > 0 || style.display === "contents";
  }

  function composedChildren(node) {
    if (String(node.tagName ?? "").toLowerCase() === "slot") {
      const assigned = node.assignedNodes?.({ flatten: true }) ?? [];
      if (assigned.length) return assigned;
    }
    if (node.shadowRoot) return Array.from(node.shadowRoot.childNodes ?? []);
    return Array.from(node.childNodes ?? []);
  }

  // The name the page declares for an element: aria-labelledby, aria-label, its labels, or its
  // alt, title or placeholder text. A field with no label of its own falls back to its
  // placeholder, as browsers do. Null when the page declared none; an empty string is a declared
  // empty label.
  function declaredName(element) {
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
    return fixed ? shared.bounded(fixed, 500).trim() : null;
  }

  function accessibleName(element) {
    const declared = declaredName(element);
    if (declared !== null) return declared;
    const tag = String(element.tagName ?? "").toLowerCase();
    const editable = tag === "input" || tag === "textarea" || tag === "select" || element.isContentEditable;
    if (editable) return "";
    return shared.bounded(composedVisibleText(element, 500).text || renderedText(element), 500);
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
    return IMPLICIT_ROLES.get(tag) ?? "generic";
  }

  function stateFor(element) {
    const state = [];
    if (element.disabled || element.getAttribute("aria-disabled") === "true") state.push("disabled");
    if (element.checked || element.getAttribute("aria-checked") === "true") state.push("checked");
    if (element.getAttribute("aria-expanded") === "true") state.push("expanded");
    if (element.getAttribute("aria-expanded") === "false") state.push("collapsed");
    if (element.selected) state.push("selected");
    if (!isComposedVisible(element)) state.push("hidden");
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
  function credentialAuthorizationError(element) {
    const name = shared.bounded(accessibleName(element) || element.id || "", 80);
    return new AssetError("credential_authorization_required", `credential-class target requires user authorization: the ${roleFor(element)}${name ? ` "${name}"` : ""}`);
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

  function deepestElementFromPoint(root, x, y) {
    let hit = root.elementFromPoint?.(x, y) ?? null;
    while (hit?.shadowRoot?.elementFromPoint) {
      const inner = hit.shadowRoot.elementFromPoint(x, y);
      if (!inner || inner === hit) break;
      hit = inner;
    }
    return hit;
  }

  function subjectAtViewportPoint(x, y) {
    const hit = deepestElementFromPoint(document, x, y);
    if (!hit) return null;
    return actionSubject(closestAcrossBoundaries(hit, ACTIONABLE_SELECTOR) || hit);
  }

  // closest() stops at the shadow boundary (ADR-0139): a component's inner node has no
  // light-DOM ancestors, so the nearest actionable element may live in an enclosing root.
  function closestAcrossBoundaries(start, selector) {
    for (let node = start; node; ) {
      const hit = node.closest?.(selector);
      if (hit) return hit;
      const root = node.getRootNode?.();
      node = root && root.host ? root.host : null;
    }
    return null;
  }

  // document.activeElement inside a component is the shadow host; the real focused
  // element hides one shadowRoot.activeElement hop per nesting level.
  function deepestActiveElement() {
    let element = document.activeElement;
    while (element?.shadowRoot?.activeElement) element = element.shadowRoot.activeElement;
    return element;
  }

  function focusedControl() {
    const element = deepestActiveElement();
    if (!element || element === document.body || element === document.documentElement) {
      throw new AssetError("no_focused_control", "no editable control is focused");
    }
    return element;
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

  // innerText stops at both iframe and shadow boundaries in Chromium. Frames are folded by
  // the service worker; this half walks the rendered composed tree inside one frame. An open
  // shadow root replaces its host's light children, while slots reinsert only assigned nodes.
  // Closed roots remain closed, and layout-less unslotted light children stay absent.
  function composedVisibleText(root, maxChars) {
    const limit = maxChars + 1;
    const visited = new Set();
    let output = "";
    let truncated = false;

    function append(value, separator = " ") {
      const normalized = String(value ?? "").replace(/\s+/g, " ").trim();
      if (!normalized) return;
      const joiner = output && !output.endsWith("\n") && !/^[,.;:!?)}\]]/.test(normalized)
        ? separator
        : "";
      const next = `${joiner}${normalized}`;
      const remaining = limit - output.length;
      if (remaining <= 0) {
        truncated = true;
        return;
      }
      output += next.slice(0, remaining);
      if (next.length > remaining) truncated = true;
    }

    function lineBreak() {
      if (!output || output.endsWith("\n")) return;
      if (output.length >= limit) truncated = true;
      else output += "\n";
    }

    function visit(node) {
      if (!node || visited.has(node) || output.length >= limit) {
        if (output.length >= limit) truncated = true;
        return;
      }
      visited.add(node);
      if (node.nodeType === 3) {
        const range = document.createRange?.();
        if (range) {
          range.selectNodeContents(node);
          const rendered = range.getClientRects().length > 0;
          if (!rendered) return;
        }
        append(node.nodeValue);
        return;
      }
      if (node.nodeType !== 1 && node.nodeType !== 9 && node.nodeType !== 11) return;
      const tag = String(node.tagName ?? "").toLowerCase();
      if (node.nodeType === 1) {
        if (!isComposedVisible(node)) return;
        if (tag === "input" || tag === "textarea") {
          const type = String(node.type ?? "").toLowerCase();
          if (type === "checkbox" || type === "radio") {
            append(node.checked ? "checked" : "unchecked");
          } else if (type !== "hidden") {
            // A placeholder is a hint, never a value. Every credential-class field is masked.
            if (node.value) append(credentialClass(node) ? MASKED_VALUE : node.value);
          }
        } else if (tag === "select") {
          const selectedText = Array.from(node.selectedOptions ?? []).map((opt) => opt.text).join(", ");
          if (selectedText) append(credentialClass(node) ? MASKED_VALUE : selectedText);
        }
        if (TEXT_OMIT_TAGS.has(tag)) return;
        if (tag === "br") {
          lineBreak();
          return;
        }
      }
      const block = TEXT_BLOCK_TAGS.has(tag);
      if (block) lineBreak();
      for (const child of composedChildren(node)) visit(child);
      if (block) lineBreak();
    }

    visit(root);
    const text = output.trim();
    return { text: text.slice(0, maxChars), truncated: truncated || text.length > maxChars };
  }

  function candidates(kind) {
    const controls = ACTIONABLE_SELECTOR;
    const structure = "main,nav,header,footer,form,table,ul,ol,h1,h2,h3,h4,h5,h6,section,article";
    const selector = kind === "controls" ? controls : kind === "structure" ? structure : `${controls},${structure}`;
    return queryAll(selector);
  }

  function inspect(kind, maximum) {
    const pool = candidates(kind).filter((element) => element.isConnected);
    return { targets: pool.slice(0, maximum).map(observation), truncated: pool.length > maximum };
  }

  // The parent in the composed tree: a slotted node's slot, else its parent, and a shadow
  // root's host, so an ancestor walk crosses open shadow boundaries instead of stopping there.
  function composedParent(node) {
    return node.assignedSlot ?? node.parentNode ?? node.host ?? null;
  }

  function findTargets(text, kind, maximum) {
    const needle = text.toLocaleLowerCase();
    const pool = kind === "control" ? candidates("controls") : queryAll(TEXT_FIND_SELECTOR);
    const hits = [];
    for (const element of pool.slice(0, 3000)) {
      const haystack = `${accessibleName(element)} ${composedVisibleText(element, 2000).text}`.toLocaleLowerCase();
      const isControl = element.matches("a[href],button,input,textarea,select,summary,[role],[contenteditable='true']");
      if (haystack.includes(needle) && (kind !== "control" || isControl) && (kind !== "text" || !isControl)) {
        hits.push({ element, isControl });
      }
    }
    // A container's text includes its descendants', so a plain container that holds another
    // match is dropped in favor of the deepest one. Controls are always kept. Containers are
    // found by walking each hit's composed ancestors once, which is O(hits * depth).
    const hitElements = new Set(hits.map((hit) => hit.element));
    const containers = new Set();
    for (const hit of hits) {
      for (let node = composedParent(hit.element); node; node = composedParent(node)) {
        if (hitElements.has(node)) containers.add(node);
      }
    }
    const kept = hits.filter((hit) => hit.isControl || !containers.has(hit.element));
    // Only the returned targets are observed, so only they get locators.
    return {
      targets: kept.slice(0, maximum).map((hit) => observation(hit.element)),
      truncated: kept.length > maximum || pool.length > 3000,
    };
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
    const pool = candidates("controls");
    for (const element of pool.slice(0, 3000)) {
      if (!element.isConnected) continue;
      if (matchesSemanticSelector(element, message)) {
        matches.push(observation(element));
        if (matches.length > 8) break;
      }
    }
    return { targets: matches.slice(0, 8), truncated: matches.length > 8 || pool.length > 3000 };
  }

  function extractArticle(maxChars) {
    const candidates = queryAll("article, main, [role='main'], [itemprop='articleBody']");
    for (const element of candidates) {
      const result = composedVisibleText(element, maxChars);
      if (result.text.length >= 80) return { ...result, article_found: true };
    }
    return { text: "", truncated: false, article_found: false };
  }

  function inspectTree(rootElement, maxDepth, maximum = DOCUMENT_TREE_NODE_LIMIT) {
    const controlRoles = ["button", "link", "textbox", "checkbox", "radio", "combobox", "tab", "menuitem", "option", "slider"];
    let count = 0;
    function build(element, depth) {
      count += 1;
      const role = roleFor(element);
      const kind = controlRoles.includes(role) ? "control" : /^h[1-6]$/.test(element.tagName.toLowerCase()) ? "heading" : "container";
      // A container's text is its descendants' text, which their own nodes already carry, so a
      // container is labelled only by a name the page declared for it.
      const label = kind === "container" ? declaredName(element) ?? "" : accessibleName(element);
      const node = { kind, label: shared.bounded(label, 120), children: [] };
      if (depth < maxDepth && count < maximum) {
        for (const child of composedChildren(element)) {
          if (count >= maximum) break;
          if (child.nodeType !== 1 || !isComposedVisible(child)) continue;
          node.children.push(build(child, depth + 1));
        }
      }
      return node;
    }
    const tree = build(rootElement, 1);
    return { tree, nodes: count, truncated: count >= maximum };
  }

  function setNativeValue(element, value) {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
    if (setter) setter.call(element, value); else element.value = value;
  }

  function setNativeChecked(element, value) {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "checked")?.set;
    if (setter) setter.call(element, value); else element.checked = value;
  }

  function clearText(element, allowCredentials = false) {
    const result = typeText(element, "", true, allowCredentials);
    return { cleared: true, subject: result.subject };
  }

  function validateTextEditElement(element, allowCredentials = false) {
    requireActionable(element, "type");
    if (credentialClass(element) && allowCredentials !== true) throw credentialAuthorizationError(element);
    if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element.isContentEditable)) {
      throw new AssetError("not_editable", "target is not text-editable");
    }
    if (element instanceof HTMLInputElement && !TEXT_INPUT_TYPES.has(element.type)) {
      throw new AssetError("not_editable", "target is not text-editable");
    }
  }

  function prepareBrowserText(element, allowCredentials = false) {
    validateTextEditElement(element, allowCredentials);
    const subject = actionSubject(element);
    element.scrollIntoView({ block: "center", inline: "center" });
    element.focus({ preventScroll: true });
    verifyBrowserTextFocus(element, allowCredentials);
    return { subject };
  }

  function verifyBrowserTextFocus(element, allowCredentials = false) {
    validateTextEditElement(element, allowCredentials);
    if (deepestActiveElement() !== element) throw new AssetError("focus_lost", "target did not retain browser input focus");
    return { focused: true };
  }

  function prepareTextEdit(element, clearFirst, allowCredentials = false) {
    validateTextEditElement(element, allowCredentials);
    const subject = actionSubject(element);
    element.scrollIntoView({ block: "center", inline: "center" });
    element.focus({ preventScroll: true });
    validateTextEditElement(element, allowCredentials);
    if (deepestActiveElement() !== element) throw new AssetError("focus_lost", "target did not retain input focus");
    if (clearFirst) {
      if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) element.select();
      else {
        const selection = element.getRootNode().getSelection?.() ?? document.getSelection();
        if (!selection) throw new AssetError("page_exception", "editable target has no selection");
        const range = document.createRange();
        range.selectNodeContents(element);
        selection.removeAllRanges();
        selection.addRange(range);
      }
    }
    validateTextEditElement(element, allowCredentials);
    if (deepestActiveElement() !== element) throw new AssetError("focus_lost", "target did not retain input focus");
    return { subject };
  }

  function typeText(element, text, clearFirst, allowCredentials = false) {
    const prepared = prepareTextEdit(element, clearFirst, allowCredentials);
    if (!text && !clearFirst) return { typed: true, subject: prepared.subject };
    if (!text && clearFirst && !(element.value ?? element.textContent)) return { typed: true, subject: prepared.subject };
    if (!document.execCommand(text ? "insertText" : "delete", false, text)) {
      throw new AssetError("page_exception", "browser could not type into editable target");
    }
    return { typed: true, subject: prepared.subject };
  }

  function selectOption(element, value) {
    return Array.from(element.options).find((candidate) => candidate.value === value || candidate.text === value);
  }

  function validateFillElement(element, value, allowCredentials = false) {
    requireActionable(element, "fill");
    if (credentialClass(element) && allowCredentials !== true) throw credentialAuthorizationError(element);
    if (element instanceof HTMLSelectElement) {
      const option = selectOption(element, value);
      if (!option) throw new AssetError("option_missing", "select option not found");
      return option;
    }
    if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element.isContentEditable)) {
      throw new AssetError("not_fillable", "target is not fillable");
    }
    return null;
  }

  // This is a read-only plan. Every known invalid field and submit target must be found
  // before an earlier field can be edited, including when the worker is preparing several frames.
  function prepareFill(message) {
    const elements = message.fields.map((field) => resolve(field.locator));
    elements.forEach((element, index) => validateFillElement(element, message.fields[index].value, message.allow_credentials));
    let submitElement = null;
    if (message.submit_locator) {
      submitElement = requireActionable(resolve(message.submit_locator), "activate");
      const owner = elements[0]?.closest?.("form") ?? null;
      if (!owner || !owner.contains(submitElement)) throw new AssetError("submit_outside_form", "submit control is not contained in the resolved form");
    }
    return { elements, submitElement, fieldKinds: elements.map((element) => isTextual(element) ? "browser_text" : "local") };
  }

  function fillStateSignature(elements) {
    return JSON.stringify(elements.map((element) => fillControlValue(element)));
  }

  function fillControlValue(element) {
    if (element instanceof HTMLInputElement && ["checkbox", "radio"].includes(element.type)) return element.checked;
    if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement) return element.value;
    if (element.isContentEditable) return element.textContent ? (element.innerText ?? element.textContent) : "";
    return element.textContent ?? "";
  }

  function expectedFillValue(element, value) {
    if (element instanceof HTMLInputElement && ["checkbox", "radio"].includes(element.type)) {
      return ["true", "1", "yes", "on"].includes(String(value).toLowerCase());
    }
    // A select reports the resolved option's value, so a label the caller used still matches.
    if (element instanceof HTMLSelectElement) return selectOption(element, value)?.value ?? String(value);
    // The textarea API and contenteditable innerText normalize both CRLF and lone CR to LF.
    return (element instanceof HTMLTextAreaElement || element.isContentEditable)
      ? String(value).replace(/\r\n?/g, "\n")
      : String(value);
  }


  function fillValuesRetained(fields, allowCredentials = false) {
    return fields.every((field) => {
      const element = resolve(field.locator);
      validateFillElement(element, field.value, allowCredentials);
      return fillControlValue(element) === expectedFillValue(element, field.value);
    });
  }

  // A browser document can be ready while a client-rendered form is still hydrating from an
  // API response. Editing during that window produces real trusted events and a temporary dirty
  // state, then the hydration result replaces the draft. Require a stable target set and
  // ready state briefly; the final current target set remains usable when the budget expires.
  async function prepareStableFill(message, ctx) {
    if (message.visual_settle === false) return prepareFill(message);
    const started = performance.now();
    let stableSince = started;
    let signature = null;
    while (true) {
      throwIfCancelled(ctx);
      const prepared = prepareFill(message);
      const nextSignature = fillStateSignature(prepared.elements);
      const now = performance.now();
      if (nextSignature !== signature) {
        signature = nextSignature;
        stableSince = now;
      }
      const ready = document.readyState === "interactive" || document.readyState === "complete";
      if (ready && now - stableSince >= FILL_STABLE_MS) {
        return prepared;
      }
      if (now - started >= Math.min(FILL_SETTLE_LIMIT_MS, message.settle_ms ?? FILL_SETTLE_LIMIT_MS)) {
        return prepareFill(message);
      }
      await sleep(FILL_SETTLE_POLL_MS);
    }
  }

  // The complete fill must be visible at the boundary the person sees: a later field can trigger
  // page code that rolls back or disables an earlier edit. Ok once every field has held its value
  // for stable_ms; not_retained when limit_ms passes first.
  async function verifyFillRetained(message, ctx) {
    const stableMs = message.stable_ms ?? FILL_RETAINED_STABLE_MS;
    const pollMs = message.poll_ms ?? FILL_RETAINED_POLL_MS;
    if (![stableMs, pollMs, message.limit_ms].every((value) => Number.isFinite(value) && value >= 0)) {
      throw new AssetError("invalid_request", "fill verification requires a physical time budget");
    }
    const limit = performance.now() + message.limit_ms;
    let stableSince = null;
    while (true) {
      throwIfCancelled(ctx);
      const now = performance.now();
      if (fillValuesRetained(message.fields, message.allow_credentials)) {
        if (stableSince === null) stableSince = now;
        if (now - stableSince >= stableMs) return { retained: true };
      } else {
        stableSince = null;
      }
      if (now >= limit) throw new AssetError("not_retained", "target did not retain filled value");
      await sleep(Math.max(1, Math.min(pollMs, limit - now)));
    }
  }

  function isTextual(element) {
    return element instanceof HTMLTextAreaElement
      || (element instanceof HTMLInputElement && TEXT_INPUT_TYPES.has(element.type))
      || element.isContentEditable;
  }

  // Controls that take no keystrokes: selects, checkboxes, radios and other non-text inputs. Text
  // controls are filled by the extension through the browser's trusted input after
  // focus_text_field, so this refuses them before it looks at anything else.
  function fillLocalElement(element, value, allowCredentials = false) {
    if (isTextual(element)) throw new AssetError("invalid_request", "text target requires browser input");
    const option = validateFillElement(element, value, allowCredentials);
    element.scrollIntoView({ block: "center", inline: "center" });
    element.focus({ preventScroll: true });
    if (element instanceof HTMLSelectElement) {
      element.value = option.value;
    } else if (element.type === "checkbox" || element.type === "radio") {
      setNativeChecked(element, ["true", "1", "yes", "on"].includes(String(value).toLowerCase()));
    } else {
      setNativeValue(element, value);
    }
    element.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
    element.dispatchEvent(new Event("change", { bubbles: true, composed: true }));
  }

  function requireActionable(element, intent) {
    if (!element.isConnected) throw staleLocator();
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
      throw new AssetError("not_visible", `target is not visible for ${intent} (${reason})`);
    }
    if (element.disabled || element.getAttribute("aria-disabled") === "true" || closestAcrossBoundaries(element, "[inert]")) {
      throw new AssetError("disabled", `target is disabled for ${intent}`);
    }
    if (["fill", "type"].includes(intent) && (element.readOnly || element.getAttribute("aria-readonly") === "true")) {
      throw new AssetError("read_only", `target is read-only for ${intent}`);
    }
    return element;
  }

  function isEmbed(element) {
    return EMBED_TAGS.has(String(element.tagName ?? "").toLowerCase());
  }

  // The content box of an embed: its viewport origin plus border and padding, and the size left
  // inside them. This is where the child document's own viewport starts.
  function contentBox(element) {
    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    const paddingLeft = parseFloat(style.paddingLeft) || 0;
    const paddingRight = parseFloat(style.paddingRight) || 0;
    const paddingTop = parseFloat(style.paddingTop) || 0;
    const paddingBottom = parseFloat(style.paddingBottom) || 0;
    return {
      left: rect.left + element.clientLeft + paddingLeft,
      top: rect.top + element.clientTop + paddingTop,
      width: element.clientWidth - paddingLeft - paddingRight,
      height: element.clientHeight - paddingTop - paddingBottom
    };
  }

  function requireOwner(ctx) {
    if (!ctx.owner) throw new AssetError("invalid_request", "owner call requires an owner element");
    return ctx.owner;
  }

  // True when the page itself, not an embedded frame, holds the browser's focus.
  function documentHasPageFocus() {
    return document.hasFocus() && !isEmbed(deepestActiveElement() ?? {});
  }

  function viewportRectangle(element) {
    const rect = element.getBoundingClientRect();
    return { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
  }

  // A frame other than the top renders only target-anchored signals.
  function renderPresentation(signal, preferences) {
    if (!IS_TOP && !signal?.locator) return false;
    let rectangle = null;
    if (signal.locator) {
      try {
        const box = viewportRectangle(resolve(signal.locator));
        // An anchored effect without a live box (stale, hidden, zero-size) renders
        // nowhere instead of collapsing onto the frame origin.
        rectangle = box.width > 0 && box.height > 0 ? box : null;
      } catch (_error) { /* target is optional presentation */ }
      if (!rectangle) return false;
    }
    return presentation.render(signal, preferences, rectangle);
  }

  function decodeFile(file) {
    let binary;
    try {
      binary = atob(file.data);
    } catch (_error) {
      throw new AssetError("payload_mismatch", "file payload is not valid base64");
    }
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
    if (bytes.byteLength !== file.size) throw new AssetError("payload_mismatch", "file payload size mismatch");
    return new File([bytes], file.name, { type: file.media_type });
  }

  function requireUploadTarget(element) {
    if (!element.isConnected) throw staleLocator();
    if (!(element instanceof HTMLInputElement) || element.type !== "file") {
      throw new AssetError("not_file_input", "target is not a file input");
    }
    if (element.disabled || element.getAttribute("aria-disabled") === "true" || closestAcrossBoundaries(element, "[inert]")) {
      throw new AssetError("disabled", "target is disabled for upload");
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

  // The condition poll. With until_satisfied an unmet condition at the timeout is the
  // `unsatisfied` failure; otherwise it is an ordinary answer with satisfied false.
  async function observe(message, ctx) {
    const started = performance.now();
    const deadline = started + message.timeout_ms;
    const readiness = () => document.readyState === "complete" ? "complete" : "interactive";
    const elapsed = () => Math.round(performance.now() - started);
    const unmet = (readyState) => {
      if (message.until_satisfied === true) throw new AssetError("unsatisfied", "condition was not satisfied before the timeout");
      return { satisfied: false, elapsed_ms: elapsed(), readiness: readyState };
    };
    const settleOptions = (timeoutMs) => ({
      document,
      timeout_ms: timeoutMs,
      now: () => performance.now(),
      cancelled: () => ctx.signal.cancelled
    });

    if (message.condition === "visual_settle") {
      const targetElement = message.locator ? locators.get(message.locator) : document;
      const visual = await sensor.settleVisual(targetElement, settleOptions(message.timeout_ms));
      if (!visual.settled) return unmet(readiness());
      return { satisfied: true, elapsed_ms: elapsed(), readiness: readiness() };
    }

    while (true) {
      throwIfCancelled(ctx);
      let satisfied = false;
      if (message.condition === "load_ready") satisfied = document.readyState === "interactive" || document.readyState === "complete";
      if (message.condition === "url_contains") satisfied = location.href.includes(message.value);
      if (message.condition === "text_present") satisfied = composedVisibleText(document.body || document.documentElement, Number.MAX_SAFE_INTEGER).text.includes(message.value);
      if (message.condition === "text_absent") satisfied = !composedVisibleText(document.body || document.documentElement, Number.MAX_SAFE_INTEGER).text.includes(message.value);
      if (message.condition === "target_present") satisfied = Boolean(locators.get(message.locator)?.isConnected);
      if (message.condition === "target_absent") satisfied = !locators.get(message.locator)?.isConnected;
      if (satisfied) {
        if (message.visual_settle === true) {
          const targetElement = message.locator ? locators.get(message.locator) : document;
          const visual = await sensor.settleVisual(targetElement, settleOptions(Math.max(0, deadline - performance.now())));
          if (!visual.settled) return unmet(readiness());
        }
        return { satisfied: true, elapsed_ms: elapsed(), readiness: readiness() };
      }
      const remaining = deadline - performance.now();
      if (remaining <= 0) break;
      await sleep(Math.min(OBSERVE_POLL_MS, remaining));
    }
    return unmet(document.readyState === "complete" ? "complete" : document.readyState === "interactive" ? "interactive" : "loading");
  }

  // Extension-owned facts reach the page with every call and through the ambient shim. Hiding is
  // per document; the perpetual visuals belong to the top document alone.
  function applyAmbient(local) {
    presentation.setHidden(local.hidden);
    if (!IS_TOP) return;
    presentation.setManaged(local.managed);
    presentation.setRecording(local.recording);
    presentation.setRuntimeState(local.runtime);
  }

  // One sampled read: the sensor answers once the page has been quiet for QUIET_READ_MS and the
  // sample is non-empty or the document is complete, unless the caller asked for no waiting.
  function sampled(message, ctx, extract, isSatisfied) {
    if (message.visual_settle === false) return extract();
    return sensor.settle(extract, isSatisfied, {
      maxWaitMs: message.settle_ms,
      quietMs: QUIET_READ_MS,
      requireQuiet: true,
      cancelled: () => ctx.signal.cancelled
    });
  }

  async function dispatch(message, ctx) {
    // Resolve and scroll inside the owning document, then return the exact live geometry. The
    // extension performs the click through Chromium's trusted pointer path. Page-generated clicks
    // can take a different framework path and have erased otherwise valid controlled-form edits.
    if (message.kind === "activate") {
      const element = requireActionable(resolve(message.locator), "activate");
      const subject = actionSubject(element);
      element.scrollIntoView({ block: "center", inline: "center" });
      requireActionable(element, "activate");
      return { activated: true, subject, rectangle: viewportRectangle(element) };
    }
    // Form submission follows the same rule as activation: the submit click is the dispatch
    // tail, and a handler that opens a page-blocking dialog would freeze this thread mid-click.
    if (message.kind === "submit_fill") {
      const { submitElement } = prepareFill(message);
      if (!submitElement) throw new AssetError("invalid_request", "submit control is required");
      setTimeout(() => submitElement.click(), 0);
      return { submitted: true };
    }
    if (message.kind === "prepare_fill") {
      const prepared = await prepareStableFill(message, ctx);
      return { prepared: true, field_kinds: prepared.fieldKinds };
    }
    if (message.kind === "verify_fill_retained") return verifyFillRetained(message, ctx);
    // Focus the exact text control for the browser's trusted input and prove it kept focus. The
    // value is typed by the extension afterwards, never by this call.
    if (message.kind === "focus_text_field") {
      const element = resolve(message.field.locator);
      validateFillElement(element, message.field.value, message.allow_credentials);
      if (!isTextual(element)) throw new AssetError("not_editable", "target does not use browser text input");
      const { subject } = prepareBrowserText(element, message.allow_credentials);
      return { focused: true, subject };
    }
    if (message.kind === "fill_local") {
      const element = resolve(message.field.locator);
      fillLocalElement(element, message.field.value, message.allow_credentials);
      return { filled: true };
    }
    if (message.kind === "read_text") {
      const extract = () => message.locator
        ? composedVisibleText(resolve(message.locator), message.max_chars)
        : message.mode === "article"
          ? extractArticle(message.max_chars)
          : composedVisibleText(document.body || document.documentElement, message.max_chars);
      const result = await sampled(message, ctx, extract, (res) => Boolean(res?.text && res.text.trim().length > 0));
      return { ...result, title: shared.bounded(document.title, 500), url: location.href };
    }
    if (message.kind === "inspect_tree") {
      const root = message.locator ? resolve(message.locator) : document.body || document.documentElement;
      const extract = () => inspectTree(root, message.max_depth ?? 6, message.max_nodes ?? DOCUMENT_TREE_NODE_LIMIT);
      return sampled(message, ctx, extract, (res) => Boolean(res?.tree && Array.isArray(res.tree.children) && res.tree.children.length > 0));
    }
    if (message.kind === "inspect") {
      const extract = () => inspect(message.inspect_kind, message.max_items);
      return sampled(message, ctx, extract, (res) => res.targets.length > 0);
    }
    if (message.kind === "find") {
      const extract = () => findTargets(message.text, message.find_kind, message.max_results);
      return sampled(message, ctx, extract, (res) => res.targets.length > 0);
    }
    if (message.kind === "describe") return { targets: message.locators.map((locator) => observation(resolve(locator))) };
    if (message.kind === "query_semantic") {
      const extract = () => querySemanticTargets(message);
      return sampled(message, ctx, extract, (res) => res.targets.length > 0);
    }
    if (message.kind === "describe_focused") return { targets: [observation(focusedControl())] };
    if (message.kind === "verify_focused_text") {
      if (!document.hasFocus()) throw new AssetError("no_focused_control", "no editable control is focused");
      return verifyBrowserTextFocus(deepestActiveElement(), message.allow_credentials);
    }
    if (message.kind === "clear_focused") return clearText(focusedControl(), message.allow_credentials);
    if (message.kind === "type_text") return typeText(resolve(message.locator), message.text, message.clear_first, message.allow_credentials);
    if (message.kind === "drop_files") {
      const dropTarget = deepestElementFromPoint(document, message.x, message.y);
      if (!dropTarget) throw new AssetError("no_subject", "no element is at the drop point");
      const transfer = new DataTransfer();
      for (const file of message.files) transfer.items.add(decodeFile(file));
      dropTarget.dispatchEvent(new DragEvent("drop", { bubbles: true, cancelable: true, composed: true, dataTransfer: transfer }));
      return { uploaded_count: message.files.length, uploaded_bytes: message.files.reduce((sum, file) => sum + file.size, 0) };
    }
    if (message.kind === "box") {
      const element = requireActionable(resolve(message.locator), "box");
      return { rectangle: viewportRectangle(element), subject: actionSubject(element) };
    }
    if (message.kind === "scroll_offset") return { x: scrollX, y: scrollY };
    if (message.kind === "focus") {
      const element = requireActionable(resolve(message.locator), "focus");
      const subject = actionSubject(element);
      element.scrollIntoView({ block: "center", inline: "center" });
      element.focus({ preventScroll: true });
      return { focused: true, subject };
    }
    if (message.kind === "scroll") {
      let subject = null;
      if (message.locator) {
        const element = requireActionable(resolve(message.locator), "scroll");
        subject = actionSubject(element);
        element.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
      } else {
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
      if (x < 0 || y < 0 || x >= innerWidth || y >= innerHeight) throw new AssetError("outside_viewport", "drag point is outside the current viewport");
      return { x, y, subject: subjectAtViewportPoint(x, y) };
    }
    // The routing half of pointer translation: what lies under a point in this document. An
    // embed is reported without a subject, because the caller descends into it.
    if (message.kind === "hit_point") {
      const x = message.x - (message.page === true ? scrollX : 0);
      const y = message.y - (message.page === true ? scrollY : 0);
      const hit = deepestElementFromPoint(document, x, y);
      if (!hit) throw new AssetError("no_subject", "point has no current document subject");
      const embed = isEmbed(hit);
      return { x, y, embed, subject: embed ? null : actionSubject(closestAcrossBoundaries(hit, ACTIONABLE_SELECTOR) || hit) };
    }
    // The parent-side half of pointer translation (ADR-0138): where the owner iframe's content
    // box sits in this document's viewport.
    if (message.kind === "owner_box") return contentBox(requireOwner(ctx));
    if (message.kind === "owner_probe") {
      const owner = requireOwner(ctx);
      const { left, top } = contentBox(owner);
      return { hit: deepestElementFromPoint(document, message.x, message.y) === owner, left, top };
    }
    if (message.kind === "focus_probe") return { focused: documentHasPageFocus() };
    if (message.kind === "require_focused") {
      if (!documentHasPageFocus()) throw new AssetError("focus_lost", "browser focus is not in this document");
      return { focused: true };
    }
    if (message.kind === "hover") {
      const element = requireActionable(resolve(message.locator), "hover");
      element.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
      return { rectangle: viewportRectangle(element), subject: actionSubject(element) };
    }
    if (message.kind === "upload_files") {
      const element = resolve(message.locator);
      const subject = actionSubject(element);
      return { ...uploadFiles(element, message.files), subject };
    }
    // A recording export: the bytes arrive as the call's attachment, never in the message.
    if (message.kind === "attach_file") {
      const attachment = ctx.attachment;
      if (!attachment) throw new AssetError("invalid_request", "attach_file requires an attachment");
      const element = resolve(message.locator);
      const subject = actionSubject(element);
      const file = { name: message.file_name, media_type: attachment.media_type, data: attachment.data, size: attachment.size };
      return { ...uploadFiles(element, [file]), subject };
    }
    if (message.kind === "observe") return observe(message, ctx);
    throw new AssetError("unknown_kind", "unknown content primitive");
  }

  world.onDispose(() => locators.clear());

  return { dispatch, applyAmbient, renderPresentation };
})();
