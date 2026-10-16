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
  // Elements the outline walk may visit, kept or pruned, before it stops and reports truncation.
  const DOCUMENT_TREE_VISIT_LIMIT = 4000;
  // What a masked credential value reads as. One short marker keeps masking cheap to read.
  const MASKED_VALUE = "[hidden]";
  // Caps on what an observed control carries. The service bounds the same keys again.
  const VALUE_MAX = 80;
  const VALUE_LIST_MAX = 25;
  const OPTIONS_MAX = 25;
  const OPTION_TEXT_MAX = 60;
  const HINT_MAX = 120;
  const GROUP_MAX = 80;
  // How far up the composed tree the row of a control is looked for, and how many nodes a row
  // is searched for its heading or first link.
  const ITEM_DEPTH_MAX = 25;
  const ITEM_VISIT_MAX = 200;
  // The identity record and the stale lookup (ADR-0198 decision 28). One read indexes at most
  // LINK_INDEX_MAX links and past that finds no row link. A row tries its first OWN_LINK_TRIES
  // links as its own link, and one read counts at most ITEM_COUNT_MAX rows to tell whether a row
  // label is unique. A stale lookup scans at most STALE_SCAN_MAX controls, counts at most
  // STALE_MATCH_MAX matches, names at most STALE_CANDIDATES_MAX of them, and stops after
  // STALE_LOOKUP_MS milliseconds.
  const LINK_INDEX_MAX = 2000;
  const OWN_LINK_TRIES = 8;
  const ITEM_COUNT_MAX = 1000;
  const STALE_SCAN_MAX = 3000;
  const STALE_MATCH_MAX = 50;
  const STALE_CANDIDATES_MAX = 3;
  const STALE_LOOKUP_MS = 50;
  // Every element that can be a row, before isItem decides which are.
  const ITEM_SELECTOR = "li,tr,article,fieldset,[role]";
  // Stands for "outside every row" in the index of where a link occurs.
  const OUTSIDE_ROW = Symbol("outside every row");
  const ERROR_MAX = 120;
  const LIMIT_MAX = 120;
  // The page areas an item can belong to (ADR-0198 decision 19). The service decodes the same closed
  // list. Content outside every landmark has no area.
  const AREA_WORDS = Object.freeze(["header", "nav", "aside", "footer", "dialog", "search"]);
  // The area an explicit ARIA landmark role names.
  const LANDMARK_ROLE_AREAS = new Map([
    ["banner", "header"],
    ["contentinfo", "footer"],
    ["navigation", "nav"],
    ["complementary", "aside"],
    ["dialog", "dialog"],
    ["alertdialog", "dialog"],
    ["search", "search"]
  ]);
  // The area a native element names when it has no role of its own.
  const LANDMARK_TAG_AREAS = new Map([
    ["header", "header"],
    ["footer", "footer"],
    ["nav", "nav"],
    ["aside", "aside"],
    ["dialog", "dialog"],
    ["search", "search"]
  ]);
  // A header or footer inside one of these belongs to that section or dialog, not to the page
  // (HTML-AAM, with a dialog scoping its own header and footer).
  const SECTIONING_TAGS = new Set(["article", "aside", "dialog", "main", "nav", "section"]);
  const SECTIONING_ROLES = new Set(["alertdialog", "article", "complementary", "dialog", "main", "navigation", "region"]);
  // The tiers a capped answer is cut by, and the one reason an item is dropped without a cut
  // (ADR-0198 decision 20). The service decodes the same closed list of keys.
  const LEFT_OUT = Object.freeze({
    hidden: "hidden",
    chrome: "chrome",
    aside: "aside",
    content: "content",
    behindDialog: "behind_dialog"
  });
  // The tiers from the one the cap keeps first to the one it cuts first.
  const TIERS_KEPT_FIRST = Object.freeze([LEFT_OUT.content, LEFT_OUT.aside, LEFT_OUT.chrome, LEFT_OUT.hidden]);
  // The areas that make an item page chrome rather than page content.
  const CHROME_AREAS = new Set(["header", "nav", "footer"]);
  // The words a refused fill names a field problem with. The service decodes the same closed list.
  const FIELD_PROBLEMS = Object.freeze({
    optionMissing: "option_missing",
    optionAmbiguous: "option_ambiguous",
    optionDisabled: "option_disabled",
    multipleValues: "multiple_values",
    notBoolean: "not_boolean",
    format: "format",
    wrongTool: "wrong_tool",
    reverted: "reverted"
  });
  // The tools a control that a fill cannot use is left to, as a wrong_tool problem names them. The
  // service decodes the same closed list.
  const WRONG_TOOL_ROUTES = Object.freeze({
    file: "file",
    button: "button",
    widget: "widget"
  });
  // The problems that make a fill refuse with the option code; any other refuses as not fillable.
  const OPTION_PROBLEMS = new Set([FIELD_PROBLEMS.optionMissing, FIELD_PROBLEMS.optionAmbiguous, FIELD_PROBLEMS.optionDisabled]);
  // What a refusal says when the service cannot read its detail. The prefix follows the code, so
  // a service that predates the detail reads the failure the same way. A lone problem adds its
  // own words after the prefix.
  const OPTION_MESSAGE = "select option not found";
  const NOT_FILLABLE_MESSAGE = "target is not fillable";
  const PROBLEM_WORDS = Object.freeze({
    [FIELD_PROBLEMS.optionAmbiguous]: "option is ambiguous",
    [FIELD_PROBLEMS.optionDisabled]: "option is disabled",
    [FIELD_PROBLEMS.multipleValues]: "takes one value, not a list",
    [FIELD_PROBLEMS.notBoolean]: "takes true or false"
  });
  // The shape each typed input takes, as a refusal tells the agent. The browser sanitizes any other
  // shape to an empty value, or to its default, so a fill checks the value against a detached
  // input of the same type before it edits anything.
  const TYPED_FORMATS = Object.freeze({
    date: "YYYY-MM-DD",
    time: "HH:MM",
    "datetime-local": "YYYY-MM-DDTHH:MM",
    month: "YYYY-MM",
    week: "YYYY-Www",
    number: "a number",
    range: "a number",
    color: "#rrggbb"
  });
  // The attributes a typed input's check and its refusal carry, because they bound the value.
  const TYPED_LIMIT_ATTRIBUTES = ["min", "max", "step"];
  // The bounds and step a range input applies when the page declares none. A range always clamps
  // to its bounds and snaps to its step, so its refusal always reports the ones in force.
  const RANGE_DEFAULT_LIMITS = Object.freeze({ min: 0, max: 100, step: 1 });
  // The step keyword that lets a range take any number inside its bounds.
  const RANGE_FREE_STEP = "any";
  // The input types that are buttons, which a click presses and a fill never sets.
  const BUTTON_INPUT_TYPES = new Set(["submit", "button", "reset", "image"]);
  // The roles of a page widget that is not a form control. A fill cannot choose in one.
  const WIDGET_ROLES = new Set(["combobox", "listbox"]);
  // How a fill value matched a choice when it was not the exact text, value or label.
  const MATCH_NORMALIZED = "normalized";
  // The words a checkbox or radio takes besides the booleans, compared without case.
  const BOOLEAN_WORDS = new Map([
    ["true", true], ["yes", true], ["on", true], ["1", true], ["checked", true],
    ["false", false], ["no", false], ["off", false], ["0", false], ["unchecked", false]
  ]);
  // How many options that contain the refused value a refusal lists when the select is long.
  const CONTAINING_MAX = 8;
  // Input types whose retained value a fill reads back whether or not it matches what was given.
  const ECHOED_INPUT_TYPES = new Set(["date", "time", "datetime-local", "month", "week", "number", "range", "color"]);
  // What an input mask adds or moves without changing what was typed: white space, brackets, hyphens
  // and slashes. A point or a comma stays significant, so 10.5 never matches 105.
  const FORMATTING_CHARACTERS = /[\s()\-/]/g;
  // The largest serialized detail a refusal carries. Options go first when it is over.
  const PROBLEM_DETAIL_MAX = 4096;
  // How much of a rich editor's text is read to size it.
  const RICH_VALUE_READ_MAX = 10000;
  // Input types whose kind an agent cannot guess from the role, so the item says it.
  const TYPED_INPUT_TYPES = new Set([
    "date", "time", "datetime-local", "month", "week", "number", "range", "color", "file",
    "email", "url", "tel"
  ]);
  // Input types that hold no typed value of their own: buttons name themselves, checkables
  // report their state, a file input would only echo a local path, and hidden is not a control.
  const VALUELESS_INPUT_TYPES = new Set([
    "button", "submit", "reset", "image", "checkbox", "radio", "file", "hidden"
  ]);
  // The constraint attributes a control reports when the page declares them.
  const LIMIT_ATTRIBUTES = ["min", "max", "step", "minlength", "maxlength", "pattern", "accept"];
  // Roles that name a place where the person enters or picks something.
  const FIELD_ROLES = new Set([
    "textbox", "combobox", "searchbox", "checkbox", "radio", "switch", "spinbutton", "slider", "listbox"
  ]);
  // The role an input type has by HTML-AAM. Any type not listed reads as a textbox.
  const INPUT_ROLES = new Map([
    ["checkbox", "checkbox"], ["radio", "radio"], ["button", "button"], ["submit", "button"],
    ["reset", "button"], ["range", "slider"], ["number", "spinbutton"], ["search", "searchbox"]
  ]);
  const GROUP_ROLES = new Set(["radiogroup", "group"]);
  // What makes an ancestor the row a control sits in. A group is a row only when the
  // page names it.
  const ITEM_ROLES = new Set(["row", "listitem", "article", "treeitem"]);
  const ITEM_TAGS = new Set(["li", "tr", "article"]);
  // The accessibility role an element has by its tag alone, beyond the control roles roleFor
  // names. A tag that is not in this table has no role of its own and reads as generic.
  const IMPLICIT_ROLES = new Map([
    ["nav", "navigation"], ["main", "main"], ["header", "banner"], ["footer", "contentinfo"],
    ["aside", "complementary"], ["form", "form"], ["section", "region"], ["article", "article"],
    ["table", "table"], ["ul", "list"], ["ol", "list"], ["li", "listitem"],
    ["fieldset", "group"], ["img", "image"], ["summary", "button"]
  ]);
  const FILL_STABLE_MS = 750;
  const FILL_RECHECK_MS = 50;
  const FILL_RETAINED_STABLE_MS = 250;
  const FILL_RETAINED_POLL_MS = 25;
  const OBSERVE_POLL_MS = 100;
  const FILL_SETTLE_POLL_MS = 50;
  const LOCATOR_PREFIX = `locator_${world.EPOCH}_`;
  const EMBED_TAGS = new Set(["iframe", "frame"]);
  // How many live locators, and how many remembered gone ones, the registry keeps. Past the live
  // limit the oldest locator is dropped and later reads as unknown. Past the gone limit the oldest
  // removed locator is dropped the same way.
  const MAX_LOCATORS = 500;
  // locator -> { element, gone, record }. A gone entry holds no element, so a removed node is never
  // pinned, and keeps the identity record of the control it named. The record never leaves the
  // page. The Map keeps mint order, so the first entry of a kind is the oldest.
  const locators = new Map();
  const reverse = new WeakMap();
  let nextLocator = 1;
  let liveCount = 0;
  let goneCount = 0;

  // Drop the oldest entries of one kind until at most `limit` of them remain.
  function trimLocators(gone, limit) {
    let count = gone ? goneCount : liveCount;
    for (const [locator, entry] of locators) {
      if (count <= limit) break;
      if (entry.gone !== gone) continue;
      if (!gone) reverse.delete(entry.element);
      locators.delete(locator);
      count -= 1;
    }
    if (gone) goneCount = count; else liveCount = count;
  }

  // A live entry whose element left the document keeps its locator and loses the element.
  function retireLocator(entry) {
    reverse.delete(entry.element);
    entry.element = null;
    entry.gone = true;
    liveCount -= 1;
    goneCount += 1;
    trimLocators(true, MAX_LOCATORS);
  }

  // Run at the start of every read that mints or describes locators, so a detached node is held
  // no longer than the next read.
  function releaseGoneLocators() {
    for (const entry of locators.values()) {
      if (!entry.gone && !entry.element.isConnected) retireLocator(entry);
    }
  }

  // The locator of an element, minted when it has none. Every observation refreshes the identity
  // record, so the latest view the agent saw is the one the record describes.
  function locatorFor(element, view, role, name) {
    let locator = reverse.get(element);
    if (!locator) {
      locator = `${LOCATOR_PREFIX}${nextLocator++}`;
      reverse.set(element, locator);
      locators.set(locator, { element, gone: false, record: null });
      liveCount += 1;
      trimLocators(false, MAX_LOCATORS);
    }
    const entry = locators.get(locator);
    if (entry) entry.record = identityOf(element, view, role, name);
    return locator;
  }

  // The two words a page can give: the element left the document, or the locator is not one this
  // page holds (another world, never minted, or dropped from the registry). A removed locator
  // also carries what the page found when it looked for the control again.
  function staleLocator(word, lookup) {
    return new AssetError("stale_locator", "stale browser locator", lookup ? { stale: word, ...lookup } : { stale: word });
  }

  // The refusal for a locator whose control left the page, with the lookup its record allows.
  function removedError(record) {
    return staleLocator("removed", record ? staleLookup(record) : undefined);
  }

  // The refusal for a held element that left the document while an action used it.
  function removedElementError(element) {
    const entry = locators.get(reverse.get(element));
    if (entry && !entry.gone) retireLocator(entry);
    return removedError(entry?.record);
  }

  // A locator minted by another world (a rebuilt page runtime) never aliases a new element. A
  // connected element whose row no longer holds the recorded link or declared name was recycled
  // for another row, so its locator is removed too.
  function resolve(locator) {
    if (typeof locator !== "string" || !locator.startsWith(LOCATOR_PREFIX)) throw staleLocator("unknown");
    const entry = locators.get(locator);
    if (!entry) throw staleLocator("unknown");
    if (entry.gone) throw removedError(entry.record);
    if (!entry.element.isConnected || recycled(entry)) {
      retireLocator(entry);
      throw removedError(entry.record);
    }
    return entry.element;
  }

  // The element a locator still holds, connected or not. A gone or unknown locator holds none.
  function heldElement(locator) {
    return locators.get(locator)?.element ?? undefined;
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
    return shared.bounded(plainText(element, 500).text, 500);
  }

  // A select that shows its options as a list: one that takes several choices or is sized to
  // show more than one row. Every other native select is a dropdown.
  function isListSelect(select) {
    const rows = Number.parseInt(select.getAttribute("size"), 10);
    return select.multiple === true || select.getAttribute("multiple") !== null || rows > 1;
  }

  function roleFor(element) {
    const explicit = element.getAttribute("role");
    if (explicit) return shared.bounded(explicit, 100);
    const tag = element.tagName.toLowerCase();
    const type = String(element.getAttribute("type") ?? "").toLowerCase();
    if (tag === "a") return "link";
    if (tag === "button") return "button";
    if (tag === "select") return isListSelect(element) ? "listbox" : "combobox";
    if (tag === "textarea") return "textbox";
    if (tag === "input") return INPUT_ROLES.get(type) ?? "textbox";
    if (/^h[1-6]$/.test(tag)) return "heading";
    return IMPLICIT_ROLES.get(tag) ?? "generic";
  }

  function stateFor(element) {
    const state = [];
    if (isDisabled(element)) state.push("disabled");
    if (element.checked || element.getAttribute("aria-checked") === "true") state.push("checked");
    if (element.getAttribute("aria-expanded") === "true") state.push("expanded");
    if (element.getAttribute("aria-expanded") === "false") state.push("collapsed");
    if (element.selected) state.push("selected");
    if (!isComposedVisible(element)) state.push("hidden");
    if (element.required === true || element.getAttribute("aria-required") === "true") state.push("required");
    if (element.readOnly === true || element.getAttribute("aria-readonly") === "true") state.push("readonly");
    if (isInvalid(element)) state.push("invalid");
    return state.slice(0, 8);
  }

  // Whether the control cannot be used: the browser's own :disabled, which also covers a control
  // inside a disabled fieldset, or the page's aria-disabled. An engine that does not know the
  // pseudo-class falls back to the disabled property.
  function isDisabled(element) {
    let native;
    try {
      native = element.matches(":disabled");
    } catch {
      native = element.disabled === true;
    }
    return native || element.getAttribute("aria-disabled") === "true";
  }

  // Whether the page or the browser calls the control invalid: after the person touched it, or
  // by the page's own aria-invalid. A browser without :user-invalid reports only the page's word.
  function isInvalid(element) {
    try {
      if (element.matches(":user-invalid")) return true;
    } catch {
      // An engine that does not know the pseudo-class falls back to aria-invalid.
    }
    return element.getAttribute("aria-invalid") === "true";
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

  // What one read remembers about the page so each element is placed once: the area of every
  // element already walked, the ordinal of each form, the label of each row, the controls of each
  // row, and the document's links and row-label counts. The forms, links and counts are built on
  // first use.
  function observationView() {
    return { areas: new Map(), forms: null, labels: new Map(), links: undefined, itemCounts: undefined, controls: new Map() };
  }

  function observation(element, view = observationView()) {
    const described = describeControl(element, view);
    return { locator: locatorFor(element, view, described.role, described.name), ...described };
  }

  // What the page tells about one control, without minting its locator.
  function describeControl(element, view) {
    const observed = {
      role: roleFor(element),
      name: accessibleName(element),
      state: stateFor(element),
      credential_class: credentialClass(element)
    };
    const area = pageArea(element, view);
    if (area) observed.area = area;
    const form = formOrdinal(element, view);
    if (form) observed.form = form;
    return Object.assign(observed, controlFacts(element, observed.role, observed.name));
  }

  // The first word of an element's role attribute, lower case, or an empty string.
  function explicitRole(element) {
    return String(element.getAttribute("role") ?? "").trim().split(/\s+/)[0].toLowerCase();
  }

  // The area word an element names for itself, or null. An explicit role decides alone. A native
  // header or footer names an area only when no section contains it.
  function landmarkRole(element) {
    const explicit = explicitRole(element);
    if (explicit) return LANDMARK_ROLE_AREAS.get(explicit) ?? null;
    const area = LANDMARK_TAG_AREAS.get(tagOf(element)) ?? null;
    if ((area === "header" || area === "footer") && insideSection(element)) return null;
    return area;
  }

  // Whether a section or dialog contains the element, so a header or footer is not the page's.
  function insideSection(element) {
    for (let node = composedParent(element); node; node = composedParent(node)) {
      if (node.nodeType !== 1) continue;
      const explicit = explicitRole(node);
      if (explicit ? SECTIONING_ROLES.has(explicit) : SECTIONING_TAGS.has(tagOf(node))) return true;
    }
    return false;
  }

  // The area that holds an element: its own landmark, else the nearest enclosing one, across
  // shadow boundaries. Null for page content.
  function pageArea(element, view) {
    const walked = [];
    let area = null;
    for (let node = element; node; node = composedParent(node)) {
      if (view.areas.has(node)) {
        area = view.areas.get(node);
        break;
      }
      walked.push(node);
      const own = node.nodeType === 1 ? landmarkRole(node) : null;
      if (own) {
        area = own;
        break;
      }
    }
    for (const node of walked) view.areas.set(node, area);
    return AREA_WORDS.includes(area) ? area : null;
  }

  // The form that owns an element: the browser's own answer when it has one, which honors the
  // form attribute, else the nearest form around it in the composed tree, which reaches a
  // control inside a shadow root. The form element owns itself.
  function formOwner(element) {
    if (tagOf(element) === "form") return element;
    if (element.form) return element.form;
    for (let node = composedParent(element); node; node = composedParent(node)) {
      if (node.nodeType === 1 && tagOf(node) === "form") return node;
    }
    return null;
  }

  // The document's forms in composed order, each with its 1-based ordinal, counted on first use.
  function documentForms(view) {
    if (!view.forms) {
      view.forms = new Map(queryAll("form").map((form, index) => [form, index + 1]));
    }
    return view.forms;
  }

  // The 1-based place of the element's form among the document's forms in composed order, or 0.
  function formOrdinal(element, view) {
    const owner = formOwner(element);
    return owner ? documentForms(view).get(owner) ?? 0 : 0;
  }

  // How many forms the document holds, whatever the answer kept, so the service can number the
  // forms of the documents after this one page-wide.
  function formCount(view) {
    return documentForms(view).size;
  }

  function oneLine(value) {
    return String(value ?? "").replace(/\s+/g, " ").trim();
  }

  function tagOf(element) {
    return String(element.tagName ?? "").toLowerCase();
  }

  function inputType(element) {
    return tagOf(element) === "input" ? String(element.getAttribute("type") ?? "").toLowerCase() : "";
  }

  // What kind of value a control holds: typed text, a chosen option, or rich text. Null when the
  // element holds none, such as a button, a link or a checkbox.
  function valueKind(element, readable) {
    const tag = tagOf(element);
    if (tag === "input") return !readable && VALUELESS_INPUT_TYPES.has(inputType(element)) ? null : "text";
    if (tag === "textarea") return "text";
    if (tag === "select") return "select";
    const declared = element.getAttribute("contenteditable");
    if (element.isContentEditable && declared !== null && declared !== "false") return "rich";
    return null;
  }

  // The one reader of a control's value, selected text and options for output. A credential-class
  // control shows the masked marker when set and nothing when empty, never its length, options or
  // placeholder. `listing` asks for the options too: "item" bounded as item keys, "read" as the
  // full line a targeted read ends with. A read (`readable`) also shows the value of a button or
  // a file input, which an item does not carry. Null when the element holds no value.
  function outwardValue(element, { limit = VALUE_MAX, listing = null, readable = false } = {}) {
    const kind = valueKind(element, readable);
    if (!kind) return null;
    const secret = credentialClass(element);
    if (kind === "select") return selectValue(element, secret, limit, secret ? null : listing);
    let full;
    if (kind === "rich") {
      full = secret ? oneLine(element.textContent) : composedVisibleText(element, RICH_VALUE_READ_MAX).text;
    } else {
      full = String(element.value ?? "");
    }
    if (secret) return { value: full ? MASKED_VALUE : "" };
    if (full.length <= limit) return { value: full };
    // A rich editor read up to its cap has no known full length, so it reports none.
    const sized = kind !== "rich" || full.length < RICH_VALUE_READ_MAX;
    return { value: full.slice(0, limit), chars: sized ? full.length : 0 };
  }

  // The placeholder of a native select: its first option when that option's value is empty. Only
  // that option reads as an empty choice; any other option, even one with an empty value, is a
  // real choice and reads as its own text.
  function selectPlaceholder(select) {
    const first = select.options?.[0];
    return first && first.value === "" ? first : null;
  }

  function selectValue(element, secret, limit, listing) {
    const placeholder = selectPlaceholder(element);
    const texts = Array.from(element.selectedOptions ?? [])
      .filter((option) => option !== placeholder)
      .map((option) => oneLine(option.text));
    const answer = {};
    if (secret) {
      answer.value = texts.length ? MASKED_VALUE : "";
    } else if (element.multiple === true) {
      answer.value = texts.slice(0, VALUE_LIST_MAX).map((text) => shared.bounded(text, limit));
    } else {
      const text = texts[0] ?? "";
      answer.value = shared.bounded(text, limit);
      if (text.length > limit) answer.chars = text.length;
    }
    if (listing) Object.assign(answer, selectChoices(element, listing));
    return answer;
  }

  // The options of a native select in document order, the placeholder set apart. A disabled
  // optgroup disables its options.
  function selectChoiceList(select) {
    const placeholder = selectPlaceholder(select);
    return Array.from(select.options ?? []).filter((option) => option !== placeholder).map((option) => {
      const parent = option.parentNode;
      const group = parent && tagOf(parent) === "optgroup" ? parent : null;
      return {
        option,
        text: oneLine(option.text),
        value: String(option.value ?? ""),
        disabled: optionDisabled(option),
        group
      };
    });
  }

  // The choices as an observed item lists them, or as the line a targeted read ends with.
  function selectChoices(select, listing) {
    const placeholder = selectPlaceholder(select);
    const choices = selectChoiceList(select);
    const found = { placeholder: placeholder ? oneLine(placeholder.text) : "" };
    if (listing === "read") {
      const names = choices.map((choice) => (choice.disabled ? `${choice.text} (disabled)` : choice.text));
      found.line = names.length ? `Options: ${names.join("; ")}` : "";
      return found;
    }
    const counts = new Map();
    for (const choice of choices) counts.set(choice.text, (counts.get(choice.text) ?? 0) + 1);
    const options = [];
    let open = null;
    for (const choice of choices.slice(0, OPTIONS_MAX)) {
      const entry = choiceEntry(choice, counts.get(choice.text) > 1);
      if (!choice.group) {
        open = null;
        options.push(entry);
        continue;
      }
      if (!open || open.node !== choice.group) {
        const label = choice.group.label ?? choice.group.getAttribute?.("label") ?? "";
        open = { node: choice.group, listing: { group: shared.bounded(oneLine(label), OPTION_TEXT_MAX), options: [] } };
        options.push(open.listing);
      }
      open.listing.options.push(entry);
    }
    found.options = options;
    if (choices.length > OPTIONS_MAX) found.optionsTotal = choices.length;
    return found;
  }

  // An option is its bare text unless that text alone would not pick it out or would be refused.
  function choiceEntry(choice, duplicated) {
    const text = shared.bounded(choice.text, OPTION_TEXT_MAX);
    if (!duplicated && !choice.disabled) return text;
    const entry = { text };
    if (duplicated) entry.value = shared.bounded(choice.value, OPTION_TEXT_MAX);
    if (choice.disabled) entry.disabled = true;
    return entry;
  }

  // The keys an observed control carries beyond its identity. A key appears only when it applies.
  // A credential-class control carries its masked value and nothing else.
  function controlFacts(element, role, name) {
    const secret = credentialClass(element);
    const out = outwardValue(element, { listing: "item" });
    const facts = {};
    if (out) {
      facts.value = out.value;
      if (out.chars) facts.value_chars = out.chars;
    }
    if (secret) return facts;
    if (out?.options) {
      facts.options = out.options;
      if (out.optionsTotal) facts.options_total = out.optionsTotal;
    }
    const type = inputType(element);
    if (TYPED_INPUT_TYPES.has(type)) facts.type = type;
    const limits = controlLimits(element);
    if (Object.keys(limits).length) facts.limits = limits;
    if (FIELD_ROLES.has(role) || valueKind(element) === "rich") {
      const hint = controlHint(element, name, out?.placeholder);
      if (hint) facts.hint = hint;
    }
    if (role === "radio" || role === "checkbox") {
      const group = controlGroup(element);
      if (group) facts.group = group;
    }
    if (isInvalid(element)) facts.error = controlError(element);
    return facts;
  }

  function controlLimits(element) {
    const tag = tagOf(element);
    const limits = {};
    if (tag !== "input" && tag !== "textarea" && tag !== "select") return limits;
    for (const attribute of LIMIT_ATTRIBUTES) {
      const declared = element.getAttribute(attribute);
      if (declared !== null && declared !== "") limits[attribute] = shared.bounded(oneLine(declared), LIMIT_MAX);
    }
    if (element.multiple === true || element.getAttribute("multiple") !== null) limits.multiple = true;
    return limits;
  }

  // Text the page gives for a field beyond its name: a placeholder that says something else, the
  // description it points to, or the empty first option of a select.
  function controlHint(element, name, placeholderOption) {
    const placeholder = oneLine(element.getAttribute("placeholder"));
    if (placeholder && placeholder !== oneLine(name)) return shared.bounded(placeholder, HINT_MAX);
    const described = describedText(element);
    if (described) return described;
    return shared.bounded(placeholderOption ?? "", HINT_MAX);
  }

  function describedText(element) {
    const ids = String(element.getAttribute("aria-describedby") ?? "").split(/\s+/).filter(Boolean);
    if (!ids.length) return "";
    const root = element.getRootNode();
    const text = ids
      .map((id) => root.getElementById?.(id))
      .filter(Boolean)
      .map((described) => oneLine(plainText(described, HINT_MAX).text))
      .join(" ")
      .trim();
    return shared.bounded(text, HINT_MAX);
  }

  // The name of the fieldset or labelled group a radio or checkbox belongs to.
  function controlGroup(element) {
    for (let node = composedParent(element); node; node = composedParent(node)) {
      if (node.nodeType !== 1) continue;
      let name = "";
      if (tagOf(node) === "fieldset") {
        const legend = Array.from(node.children ?? []).find((child) => tagOf(child) === "legend");
        name = legend ? oneLine(plainText(legend, GROUP_MAX).text) : "";
      } else if (GROUP_ROLES.has(node.getAttribute("role"))) {
        name = oneLine(declaredName(node));
      }
      if (name) return shared.bounded(name, GROUP_MAX);
    }
    return "";
  }

  // The name the page declares for an element through aria-labelledby or aria-label, read as page
  // text only: a referenced field or editable content contributes nothing, so a row named by a
  // field never carries what the field holds. Null when the page declared none.
  function declaredLabel(element) {
    const labelledBy = element.getAttribute("aria-labelledby");
    if (labelledBy) {
      const root = element.getRootNode();
      const text = oneLine(labelledBy.split(/\s+/).map((id) => {
        const referenced = root.getElementById?.(id);
        return referenced ? plainText(referenced, GROUP_MAX).text : "";
      }).join(" "));
      if (text) return shared.bounded(text, GROUP_MAX);
    }
    const aria = oneLine(element.getAttribute("aria-label"));
    return aria ? shared.bounded(aria, GROUP_MAX) : null;
  }

  // Whether an ancestor ends the walk for a row: a landmark, a main or region, a form the page
  // names, or a dialog. A row is never looked for outside the region that holds the control.
  function bordersRegion(node) {
    if (landmarkRole(node)) return true;
    const role = explicitRole(node);
    const tag = tagOf(node);
    if (role === "main" || role === "region" || tag === "main") return true;
    return tag === "form" && declaredLabel(node) !== null;
  }

  // Whether an element is a row, list item, article, tree item, a group the page names, or a
  // fieldset with a legend.
  function isItem(node) {
    const role = explicitRole(node);
    if (GROUP_ROLES.has(role) && declaredLabel(node) !== null) return true;
    if (ITEM_ROLES.has(role)) return true;
    const tag = tagOf(node);
    if (tag === "fieldset") return legendText(node) !== "";
    return ITEM_TAGS.has(tag);
  }

  // The nearest composed ancestor of an element that is a row, or null. The walk ends, with null, at
  // a landmark, a dialog or the body, so a row is always nearer than the region that holds it.
  function itemOf(element) {
    let node = composedParent(element);
    for (let depth = 0; node && depth < ITEM_DEPTH_MAX; depth += 1, node = composedParent(node)) {
      if (node.nodeType !== 1) continue;
      if (tagOf(node) === "body" || bordersRegion(node)) return null;
      if (isItem(node)) return node;
    }
    return null;
  }

  // The legend a fieldset shows, or an empty string.
  function legendText(fieldset) {
    const legend = Array.from(fieldset.children ?? []).find((child) => tagOf(child) === "legend");
    return legend ? shared.bounded(oneLine(plainText(legend, GROUP_MAX).text), GROUP_MAX) : "";
  }

  // The first descendant of a row, in document order, that `matches`, or null. The search stops
  // after ITEM_VISIT_MAX nodes.
  function firstWithin(item, matches) {
    const stack = composedChildren(item).slice().reverse();
    for (let visits = 0; stack.length && visits < ITEM_VISIT_MAX; visits += 1) {
      const node = stack.pop();
      if (node.nodeType !== 1 || !isComposedVisible(node)) continue;
      if (matches(node)) return node;
      stack.push(...composedChildren(node).slice().reverse());
    }
    return null;
  }

  function isHeading(node) {
    return /^h[1-6]$/.test(tagOf(node)) || explicitRole(node) === "heading";
  }

  function isLink(node) {
    return tagOf(node) === "a" && node.getAttribute("href") !== null;
  }

  // What a row calls itself: the name the page declared, a fieldset's legend, its first heading,
  // else the name of its first link. `declared` is false only for the link, which a page may reuse
  // across rows. The text is one bounded line of page text, never of a field's value.
  function itemLabel(item) {
    const line = (text) => shared.bounded(oneLine(text), GROUP_MAX);
    const named = declaredLabel(item) ?? (tagOf(item) === "fieldset" ? legendText(item) : "");
    if (named) return { text: line(named), declared: true };
    const heading = firstWithin(item, isHeading);
    const headed = heading ? line(plainText(heading, GROUP_MAX).text) : "";
    if (headed) return { text: headed, declared: true };
    const link = firstWithin(item, isLink);
    return { text: link ? line(declaredLabel(link) ?? plainText(link, GROUP_MAX).text) : "", declared: false };
  }

  // The name of the nearest named landmark, region or tab panel around an element, or an empty
  // string. A dialog ends the search, named or not.
  function regionName(element) {
    let node = composedParent(element);
    for (let depth = 0; node && depth < ITEM_DEPTH_MAX; depth += 1, node = composedParent(node)) {
      if (node.nodeType !== 1) continue;
      const dialog = landmarkRole(node) === "dialog";
      if (dialog || bordersRegion(node) || explicitRole(node) === "tabpanel") {
        const name = declaredLabel(node);
        if (name || dialog) return name ?? "";
      }
    }
    return "";
  }

  // What a row calls itself, read once per row: `labels` keeps the label of each row already read.
  function labelOf(item, labels) {
    if (!labels.has(item)) labels.set(item, itemLabel(item));
    return labels.get(item);
  }

  // Where a control sits, in the page's own words: its row's label, else its group, else the
  // region around it, else an empty string. A credential-class control has no context.
  function contextOf(element, labels = new Map()) {
    if (credentialClass(element)) return "";
    return contextWithin(element, itemOf(element), labels);
  }

  // The context of a control whose row, or none, is already known.
  function contextWithin(element, item, labels) {
    if (item) {
      const text = labelOf(item, labels).text;
      if (text) return text;
    }
    return controlGroup(element) || regionName(element);
  }

  // Tells apart controls whose role and name repeat among the observations of one answer
  // (ADR-0198 rule 1). Such a control carries `context`, where it sits, when that adds to its
  // group; those that still share a place carry `nth`, their position among them in document
  // order. A unique control and a credential-class control carry neither. `observed[i]` is the
  // observation of `elements[i]`.
  function annotateRepeats(observed, elements) {
    const repeats = new Map();
    observed.forEach((entry, index) => {
      if (entry.credential_class) return;
      const key = `${entry.role}\n${entry.name}`;
      if (repeats.has(key)) repeats.get(key).push(index);
      else repeats.set(key, [index]);
    });
    const labels = new Map();
    for (const members of repeats.values()) {
      if (members.length < 2) continue;
      const places = new Map();
      for (const index of members) {
        const entry = observed[index];
        const context = contextOf(elements[index], labels);
        if (context && context !== entry.group) entry.context = context;
        const place = entry.context ?? entry.group ?? "";
        if (places.has(place)) places.get(place).push(entry);
        else places.set(place, [entry]);
      }
      for (const same of places.values()) {
        if (same.length >= 2) same.forEach((entry, position) => { entry.nth = position + 1; });
      }
    }
    return observed;
  }

  // ---- identity records and the stale lookup (ADR-0198 decision 28) ----
  //
  // Every routine here is value-free: names come from declaredLabel or plainText, never from a
  // field's value, and walks cross open shadow roots through composedParent. What a record holds
  // never leaves the page.

  // The document's own address without its fragment, as links are compared, and its origin.
  function documentBase() {
    const own = new URL(location.href);
    own.hash = "";
    return { href: location.href, self: own.href, origin: own.origin };
  }

  // The address a link points to without its fragment, or null when it cannot name a row: not
  // http or https, another origin, the site root, or the document itself.
  function linkAddress(link, base) {
    let url;
    try {
      url = new URL(link.getAttribute("href") ?? "", base.href);
    } catch {
      return null;
    }
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    if (url.origin !== base.origin || url.pathname === "/") return null;
    url.hash = "";
    return url.href === base.self ? null : url.href;
  }

  // The links in a row's composed subtree, in document order, at most `maximum` of them. The
  // search stops after ITEM_VISIT_MAX nodes.
  function linksWithin(item, maximum) {
    const links = [];
    const stack = composedChildren(item).slice().reverse();
    for (let visits = 0; stack.length && links.length < maximum && visits < ITEM_VISIT_MAX; visits += 1) {
      const node = stack.pop();
      if (node.nodeType !== 1) continue;
      if (isLink(node)) links.push(node);
      stack.push(...composedChildren(node).slice().reverse());
    }
    return links;
  }

  // Where each usable link address occurs in the document: the set of rows that hold it, with
  // OUTSIDE_ROW standing for an occurrence in no row. Built once per read. Null when the document
  // holds more than LINK_INDEX_MAX links, so no row has a link of its own in that read.
  function linkIndex(view) {
    if (view.links !== undefined) return view.links;
    const links = queryAll("a[href]");
    if (links.length > LINK_INDEX_MAX) {
      view.links = null;
      return null;
    }
    const base = documentBase();
    const index = new Map();
    for (const link of links) {
      const address = linkAddress(link, base);
      if (!address) continue;
      if (!index.has(address)) index.set(address, new Set());
      index.get(address).add(itemOf(link) ?? OUTSIDE_ROW);
    }
    view.links = index;
    return index;
  }

  // The address of the first of a row's first links that occurs in this row and nowhere else on
  // the page: the row's own link, which a now-playing bar or a sidebar copy can never be. Null
  // when the row has none.
  function ownLink(item, view) {
    const index = linkIndex(view);
    if (!index) return null;
    const base = documentBase();
    for (const link of linksWithin(item, OWN_LINK_TRIES)) {
      const address = linkAddress(link, base);
      const holders = address ? index.get(address) : undefined;
      if (holders && holders.size === 1 && holders.has(item)) return address;
    }
    return null;
  }

  // How many rows of the document carry each declared label, counted once per read. Null when the
  // document holds more than ITEM_COUNT_MAX rows, so no label counts as unique in that read.
  function itemLabelCounts(view) {
    if (view.itemCounts !== undefined) return view.itemCounts;
    const counts = new Map();
    let rows = 0;
    for (const node of queryAll(ITEM_SELECTOR)) {
      if (!isItem(node)) continue;
      rows += 1;
      if (rows > ITEM_COUNT_MAX) {
        view.itemCounts = null;
        return null;
      }
      const label = labelOf(node, view.labels);
      if (label.declared && label.text) counts.set(label.text, (counts.get(label.text) ?? 0) + 1);
    }
    view.itemCounts = counts;
    return counts;
  }

  // Whether exactly one row of the document carries this declared label.
  function labelIsUnique(text, view) {
    return itemLabelCounts(view)?.get(text) === 1;
  }

  // The controls inside a row, in document order, each with its role and name, read once per row
  // per read. The search stops after ITEM_VISIT_MAX nodes.
  function controlsWithin(item, view) {
    if (view.controls.has(item)) return view.controls.get(item);
    const found = [];
    const stack = composedChildren(item).slice().reverse();
    for (let visits = 0; stack.length && visits < ITEM_VISIT_MAX; visits += 1) {
      const node = stack.pop();
      if (node.nodeType !== 1) continue;
      if (node.matches(ACTIONABLE_SELECTOR) && !isHiddenInput(node)) found.push({ element: node, role: roleFor(node), name: accessibleName(node) });
      stack.push(...composedChildren(node).slice().reverse());
    }
    view.controls.set(item, found);
    return found;
  }

  // Where an element sits on the page: the key of the nearest landmark, region, tab panel or
  // dialog around it, and whether any dialog holds it.
  function placeOf(element) {
    let regionKey = null;
    let inDialog = false;
    for (let node = composedParent(element); node; node = composedParent(node)) {
      if (node.nodeType !== 1) continue;
      if (landmarkRole(node) === "dialog") inDialog = true;
      if (regionKey === null && (bordersRegion(node) || explicitRole(node) === "tabpanel")) {
        regionKey = `${roleFor(node)}\n${declaredLabel(node) ?? ""}`;
      }
    }
    return { regionKey: regionKey ?? "", inDialog };
  }

  // The view an agent saw a control in: the document's address without its fragment, its title
  // and the region around the control. Compared only inside the page.
  function viewKey(regionKey) {
    return `${location.href.split("#")[0]}\n${document.title}\n${regionKey}`;
  }

  // What the page remembers about a control so it can later say whether a node is the same
  // control: its role, exact name and credential class, where it sat (the label of its row, else
  // its group, else its region), the row's declared name, whether that name was unique among
  // rows when no link was recorded, the row's own link, the view, whether a dialog held it, and
  // its place among the row's controls of the same role and name. A credential-class control
  // records no place, because it is never followed or named as a replacement.
  function identityOf(element, view, role, name) {
    const credential = credentialClass(element);
    const place = placeOf(element);
    const record = {
      role,
      name,
      credential,
      context: "",
      hasItem: false,
      itemDeclared: null,
      itemUnique: false,
      anchor: null,
      view: viewKey(place.regionKey),
      inDialog: place.inDialog,
      nth: null,
      count: null
    };
    if (credential) return record;
    const item = itemOf(element);
    record.context = contextWithin(element, item, view.labels);
    if (!item) return record;
    record.hasItem = true;
    const label = labelOf(item, view.labels);
    if (label.declared && label.text) record.itemDeclared = label.text;
    record.anchor = ownLink(item, view);
    if (record.anchor === null && record.itemDeclared !== null) record.itemUnique = labelIsUnique(record.itemDeclared, view);
    const same = controlsWithin(item, view).filter((control) => control.role === role && control.name === name);
    const position = same.findIndex((control) => control.element === element);
    if (position >= 0) {
      record.nth = position + 1;
      record.count = same.length;
    }
    return record;
  }

  // Whether a connected element no longer sits in the row its record describes: the row lost the
  // recorded link, or its declared name changed. A list that recycles its nodes for other rows
  // does this, and acting on such a node would act on another row. A record with neither a link
  // nor a declared row name says nothing about the row.
  function recycled(entry) {
    const record = entry.record;
    if (!record || (record.anchor === null && record.itemDeclared === null)) return false;
    const item = itemOf(entry.element);
    if (!item) return true;
    if (record.itemDeclared !== null) {
      const label = itemLabel(item);
      if (!label.declared || label.text !== record.itemDeclared) return true;
    }
    if (record.anchor !== null) {
      const base = documentBase();
      return !linksWithin(item, OWN_LINK_TRIES).some((link) => linkAddress(link, base) === record.anchor);
    }
    return false;
  }

  // Whether a node is the control a record describes, by one conjunction: its context equals the
  // recorded one, the view and the dialog are unchanged, and the row proves itself. A row proves
  // itself by its own link when the record has one, else by its declared name when that was
  // unique when recorded and is unique now. A control that sat in no row needs a node in no row.
  // Position is never part of the test. The caller has already matched role, name and class.
  function matchesRecord(record, element, view) {
    if (record.credential) return false;
    const place = placeOf(element);
    if (place.inDialog !== record.inDialog || viewKey(place.regionKey) !== record.view) return false;
    const item = itemOf(element);
    if (contextWithin(element, item, view.labels) !== record.context) return false;
    if (record.anchor !== null) return item !== null && ownLink(item, view) === record.anchor;
    if (!record.hasItem) return item === null;
    if (!item || record.itemDeclared === null || !record.itemUnique) return false;
    const label = labelOf(item, view.labels);
    return label.declared && label.text === record.itemDeclared && labelIsUnique(label.text, view);
  }

  // Look in this document for the control a record describes, in the failing call itself: no
  // painting, no effect, at most STALE_LOOKUP_MS of work. Counts the connected controls with the
  // same role, exact name and credential class (`matched`, up to STALE_MATCH_MAX), how many of
  // them are the same control by the conjunction (`same_count`), and names up to
  // STALE_CANDIDATES_MAX of them, the same ones first and then in document order, each with a
  // fresh locator, its context, and its nth when its context repeats. `truncated` says a cap or
  // the time limit cut the lookup short.
  function staleLookup(record) {
    const started = performance.now();
    const late = () => performance.now() - started > STALE_LOOKUP_MS;
    const view = observationView();
    const pool = candidates("controls");
    let truncated = pool.length > STALE_SCAN_MAX;
    const matches = [];
    for (const element of pool.slice(0, STALE_SCAN_MAX)) {
      if (late()) {
        truncated = true;
        break;
      }
      if (!element.isConnected || roleFor(element) !== record.role) continue;
      if (credentialClass(element) !== record.credential || accessibleName(element) !== record.name) continue;
      if (matches.length === STALE_MATCH_MAX) {
        truncated = true;
        break;
      }
      matches.push(element);
    }
    const described = [];
    const same = [];
    for (const element of matches) {
      if (late()) {
        truncated = true;
        break;
      }
      described.push(describeControl(element, view));
      same.push(matchesRecord(record, element, view));
    }
    const counted = matches.slice(0, described.length);
    annotateRepeats(described, counted);
    // A candidate says where it sits even when it is the only one left, because the agent picks
    // among candidates by their rows.
    described.forEach((entry, index) => {
      if (entry.credential_class || entry.context) return;
      const context = contextOf(counted[index], view.labels);
      if (context && context !== entry.group) entry.context = context;
    });
    const order = counted.map((_, index) => index).sort((a, b) => Number(same[b]) - Number(same[a]) || a - b);
    return {
      matched: matches.length,
      same_count: same.filter(Boolean).length,
      truncated,
      candidates: order.slice(0, STALE_CANDIDATES_MAX).map((index) => ({
        target: { locator: locatorFor(counted[index], view, described[index].role, described[index].name), ...described[index] },
        same: same[index]
      }))
    };
  }

  // Why the control is invalid, in words Ghostlight chooses. The browser's own message is read
  // only for a custom error, because it can quote the value or its length.
  function controlError(element) {
    const messageId = element.getAttribute("aria-errormessage");
    if (messageId) {
      const target = element.getRootNode().getElementById?.(messageId);
      const text = target ? oneLine(plainText(target, ERROR_MAX).text) : "";
      if (text) return shared.bounded(text, ERROR_MAX);
    }
    return validityPhrase(element) || "is invalid";
  }

  function validityPhrase(element) {
    const validity = element.validity;
    if (!validity || validity.valid) return "";
    const type = inputType(element);
    const declared = (attribute) => shared.bounded(oneLine(element.getAttribute(attribute)), 40);
    if (validity.valueMissing) return "is required";
    if (validity.typeMismatch) {
      if (type === "email") return "is not a valid email address";
      return type === "url" ? "is not a valid URL" : "is not valid";
    }
    if (validity.tooShort) {
      const least = declared("minlength");
      return /^\d+$/.test(least) ? `is too short (at least ${least} characters)` : "is too short";
    }
    if (validity.tooLong) return "is too long";
    if (validity.rangeUnderflow) {
      const least = declared("min");
      return least ? `is below the minimum (min ${least})` : "is below the minimum";
    }
    if (validity.rangeOverflow) {
      const most = declared("max");
      return most ? `is above the maximum (max ${most})` : "is above the maximum";
    }
    if (validity.stepMismatch) return "does not match the allowed step";
    if (validity.patternMismatch) return "does not match the required format";
    if (validity.badInput) return `is not a valid ${type || "value"}`;
    if (validity.customError) return shared.bounded(oneLine(element.validationMessage), ERROR_MAX);
    return "";
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

  // Matches in composed tree order. A shadow root's matches belong where its host sits, not
  // after the whole document's.
  function queryAll(selector) {
    const unique = new Set();
    const found = roots();
    for (const root of found) {
      for (const element of root.querySelectorAll(selector)) unique.add(element);
    }
    return found.length > 1 ? composedOrder(Array.from(unique)) : Array.from(unique);
  }

  // The elements sorted into the order the page renders them. An open shadow root replaces its
  // host's light children, and a slot holds the nodes assigned to it. An element the composed
  // tree never reaches, such as an unslotted light child, goes last in its given order.
  function composedOrder(elements) {
    const wanted = new Set(elements);
    const place = new Map();
    const seen = new Set();
    const stack = [document.body, document.documentElement].filter(Boolean);
    while (stack.length) {
      const node = stack.pop();
      if (seen.has(node)) continue;
      seen.add(node);
      if (wanted.has(node)) place.set(node, place.size);
      const children = composedChildren(node);
      for (let index = children.length - 1; index >= 0; index -= 1) stack.push(children[index]);
    }
    const unplaced = place.size;
    return elements
      .map((element, index) => ({ element, key: place.get(element) ?? unplaced + index }))
      .sort((a, b) => a.key - b.key)
      .map(({ element }) => element);
  }

  // innerText stops at both iframe and shadow boundaries in Chromium. Frames are folded by
  // the service worker; this half walks the rendered composed tree inside one frame. An open
  // shadow root replaces its host's light children, while slots reinsert only assigned nodes.
  // Closed roots remain closed, and layout-less unslotted light children stay absent.
  // With values on (a read), form controls contribute their state: typed values, checked words,
  // and the masked marker for credential fields. With values off (names, find, text conditions),
  // controls and editable content contribute nothing, so what the agent typed never reads back.
  function composedVisibleText(root, maxChars, { values = true } = {}) {
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
        if (!values && node.isContentEditable) return;
        if (values && (tag === "input" || tag === "textarea")) {
          const type = String(node.type ?? "").toLowerCase();
          if (type === "checkbox" || type === "radio") {
            append(node.checked ? "checked" : "unchecked");
          } else if (type !== "hidden") {
            // A placeholder is a hint, never a value. Every credential-class field is masked.
            const held = outwardValue(node, { limit: Number.MAX_SAFE_INTEGER, readable: true });
            if (held?.value) append(held.value);
          }
        } else if (values && tag === "select") {
          const held = outwardValue(node, { limit: Number.MAX_SAFE_INTEGER });
          if (held?.value) append(Array.isArray(held.value) ? held.value.join(", ") : held.value);
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

  // A targeted read: the control's text, and for a native select a last line naming every option.
  function readTarget(element, maxChars) {
    const read = composedVisibleText(element, maxChars);
    const line = tagOf(element) === "select" ? outwardValue(element, { listing: "read" })?.line : "";
    if (!line) return read;
    const text = read.text ? `${read.text}
${line}` : line;
    return { text: text.slice(0, maxChars), truncated: read.truncated || text.length > maxChars };
  }

  // The page's own text: composedVisibleText without any field value or editable content.
  function plainText(root, maxChars) {
    return composedVisibleText(root, maxChars, { values: false });
  }

  function candidates(kind) {
    const controls = ACTIONABLE_SELECTOR;
    const structure = "main,nav,header,footer,form,table,ul,ol,h1,h2,h3,h4,h5,h6,section,article";
    const selector = kind === "controls" ? controls : kind === "structure" ? structure : `${controls},${structure}`;
    return queryAll(selector).filter((element) => !isHiddenInput(element));
  }

  // An input of type hidden holds page data, not a control anyone can use.
  function isHiddenInput(element) {
    return inputType(element) === "hidden";
  }

  // The tier an item is cut in: hidden first, then page chrome, then asides, and page content last.
  function tierOf(element, view) {
    if (!isComposedVisible(element)) return LEFT_OUT.hidden;
    const area = pageArea(element, view);
    if (CHROME_AREAS.has(area)) return LEFT_OUT.chrome;
    return area === "aside" ? LEFT_OUT.aside : LEFT_OUT.content;
  }

  // The pool cut to `maximum` by tier. Within a tier the earliest items stay, and the kept items
  // keep their document order. `counts` holds only the tiers that lost items.
  function cutByTier(pool, maximum, view) {
    const counts = {};
    if (pool.length <= maximum) return { kept: pool, counts };
    const tiers = pool.map((element) => tierOf(element, view));
    const keep = new Set();
    let room = maximum;
    for (const tier of TIERS_KEPT_FIRST) {
      tiers.forEach((own, index) => {
        if (own !== tier) return;
        if (room > 0) {
          keep.add(index);
          room -= 1;
        } else {
          counts[tier] = (counts[tier] ?? 0) + 1;
        }
      });
    }
    return { kept: pool.filter((_, index) => keep.has(index)), counts };
  }

  // Whether the browser treats the dialog as modal and open, which makes the rest of the page inert.
  function isOpenModal(dialog) {
    if (dialog.open !== true) return false;
    try {
      return dialog.matches(":modal");
    } catch {
      return false;
    }
  }

  // What makes controls unusable without disabling them: open modal dialogs and inert subtrees.
  function inertGate() {
    return { modals: queryAll("dialog").filter(isOpenModal), inert: queryAll("[inert]").length > 0 };
  }

  // Whether `ancestor` holds `node` in the composed tree.
  function composedContains(ancestor, node) {
    for (let current = node; current; current = composedParent(current)) {
      if (current === ancestor) return true;
    }
    return false;
  }

  // Whether a control cannot be acted on because an open modal dialog or an inert ancestor covers
  // it. The action path refuses the same controls (the inert check in the target gate).
  function isBehindDialog(element, gate) {
    if (!element.matches(ACTIONABLE_SELECTOR)) return false;
    if (gate.inert && closestAcrossBoundaries(element, "[inert]")) return true;
    return gate.modals.length > 0 && !gate.modals.some((modal) => composedContains(modal, element));
  }

  // The pool is cut to `maximum` by tier, and what each tier lost is counted in `left_out`.
  // Controls behind an open modal or an inert ancestor are dropped before the cut and counted
  // apart. `truncated` says only that the cap cut something.
  function inspect(kind, maximum) {
    const view = observationView();
    const gate = inertGate();
    const pool = [];
    let behind = 0;
    for (const element of candidates(kind)) {
      if (!element.isConnected) continue;
      if (isBehindDialog(element, gate)) behind += 1;
      else pool.push(element);
    }
    const { kept, counts } = cutByTier(pool, maximum, view);
    const truncated = Object.keys(counts).length > 0;
    if (behind) counts[LEFT_OUT.behindDialog] = behind;
    return {
      targets: annotateRepeats(kept.map((element) => observation(element, view)), kept),
      truncated,
      forms: formCount(view),
      left_out: counts,
      modal_open: gate.modals.length > 0
    };
  }

  // The parent in the composed tree: a slotted node's slot, else its parent, and a shadow
  // root's host, so an ancestor walk crosses open shadow boundaries instead of stopping there.
  function composedParent(node) {
    return node.assignedSlot ?? node.parentNode ?? node.host ?? null;
  }

  // The option texts of a native select, so a dropdown is found by a choice it offers. A
  // credential-class select offers none: what it lists is never searchable.
  function optionTexts(element) {
    if (tagOf(element) !== "select" || credentialClass(element)) return "";
    return Array.from(element.options ?? []).map((option) => oneLine(option.text)).join(" ");
  }

  function findTargets(text, kind, maximum) {
    const needle = text.toLocaleLowerCase();
    const pool = kind === "control" ? candidates("controls") : queryAll(TEXT_FIND_SELECTOR).filter((element) => !isHiddenInput(element));
    const hits = [];
    for (const element of pool.slice(0, 3000)) {
      const haystack = `${accessibleName(element)} ${plainText(element, 2000).text} ${optionTexts(element)}`.toLocaleLowerCase();
      const isControl = element.matches(ACTIONABLE_SELECTOR);
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
    const view = observationView();
    const cut = cutByTier(kept.map((hit) => hit.element), maximum, view);
    return {
      targets: annotateRepeats(cut.kept.map((element) => observation(element, view)), cut.kept),
      truncated: Object.keys(cut.counts).length > 0 || pool.length > 3000,
      forms: formCount(view),
      left_out: cut.counts
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
    const view = observationView();
    for (const element of pool.slice(0, 3000)) {
      if (!element.isConnected) continue;
      if (matchesSemanticSelector(element, message)) {
        matches.push(element);
        if (matches.length > 8) break;
      }
    }
    const kept = matches.slice(0, 8);
    return {
      targets: annotateRepeats(kept.map((element) => observation(element, view)), kept),
      truncated: matches.length > 8 || pool.length > 3000,
      forms: formCount(view)
    };
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
    const controlRoles = ["button", "link", "textbox", "searchbox", "checkbox", "radio", "combobox", "listbox", "tab", "menuitem", "option", "slider", "spinbutton"];
    // `count` holds the nodes kept so far; a node is counted on entry and given back if it is pruned.
    let count = 0;
    let visits = 0;
    let visitLimitHit = false;
    // Returns the node, or null when it is an unlabeled container whose children were all pruned.
    function build(element, depth, isRoot) {
      count += 1;
      visits += 1;
      const role = roleFor(element);
      const kind = controlRoles.includes(role) ? "control" : /^h[1-6]$/.test(element.tagName.toLowerCase()) ? "heading" : "container";
      // A container's text is its descendants' text, which their own nodes already carry, so a
      // container is labelled only by a name the page declared for it.
      const label = kind === "container" ? declaredName(element) ?? "" : accessibleName(element);
      const node = { kind, label: shared.bounded(label, 120), children: [] };
      // Whether every visible child was visited. A cut node may hide content, so it is kept.
      let explored = true;
      for (const child of composedChildren(element)) {
        if (child.nodeType !== 1 || !isComposedVisible(child)) continue;
        if (visits >= DOCUMENT_TREE_VISIT_LIMIT) {
          visitLimitHit = true;
          explored = false;
          break;
        }
        if (depth >= maxDepth || count >= maximum) {
          explored = false;
          break;
        }
        const built = build(child, depth + 1, false);
        if (built) node.children.push(built);
      }
      if (!isRoot && explored && kind === "container" && node.label === "" && node.children.length === 0) {
        count -= 1;
        return null;
      }
      return node;
    }
    const tree = build(rootElement, 1, true);
    return { tree, nodes: count, truncated: count >= maximum || visitLimitHit };
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

  function optionDisabled(option) {
    const parent = option.parentNode;
    return option.disabled === true || (Boolean(parent) && tagOf(parent) === "optgroup" && parent.disabled === true);
  }

  function normalizedChoice(value) {
    return oneLine(String(value).normalize("NFC")).toLowerCase();
  }

  // The entries a fill value names, each entry holding the strings it answers to. Tier one is
  // exact on any of them. Only when tier one finds none does tier two compare both sides after
  // NFC, trimming, white space collapse and case folding. Never a prefix, a substring or the
  // closest. An empty value names nothing here: callers that give it a meaning handle it first.
  function matchEntries(entries, value) {
    if (value === "") return { hits: [], tier: "exact" };
    const exact = entries.filter((entry) => entry.keys.includes(value));
    if (exact.length) return { hits: exact, tier: "exact" };
    const needle = normalizedChoice(value);
    if (!needle) return { hits: [], tier: MATCH_NORMALIZED };
    return { hits: entries.filter((entry) => entry.keys.some((key) => normalizedChoice(key) === needle)), tier: MATCH_NORMALIZED };
  }

  // Whether a fill value is a boolean or one of the closed words for one. Undefined otherwise.
  function booleanOf(value) {
    return typeof value === "boolean" ? value : BOOLEAN_WORDS.get(String(value).toLowerCase());
  }

  function isMultipleSelect(element) {
    return element instanceof HTMLSelectElement && (element.multiple === true || element.getAttribute("multiple") !== null);
  }

  // What an option answers to: its value, its text and its label.
  function selectKeys(option) {
    const label = typeof option.label === "string" ? option.label : "";
    return [String(option.value ?? ""), String(option.text ?? ""), label].filter((key) => key !== "");
  }

  // The enabled options whose text contains the refused value, for a select too long to list.
  function optionsContaining(select, value) {
    const needle = normalizedChoice(value);
    if (!needle) return [];
    return selectChoiceList(select)
      .filter((choice) => !choice.disabled && normalizedChoice(choice.text).includes(needle))
      .slice(0, CONTAINING_MAX)
      .map((choice) => shared.bounded(choice.text, OPTION_TEXT_MAX));
  }

  // Why a select cannot take a value, in a form that lists what would work. The refused value is
  // never echoed back, and a credential-class select lists nothing.
  function selectProblem(select, kind, value, hits = []) {
    const problem = { problem: kind };
    if (credentialClass(select) || kind === FIELD_PROBLEMS.optionDisabled) return problem;
    if (kind === FIELD_PROBLEMS.optionAmbiguous) {
      const choices = selectChoiceList(select);
      problem.options = hits.map((hit) => choiceEntry(choices.find((choice) => choice.option === hit.option), true));
      return problem;
    }
    const listed = selectChoices(select, "item");
    problem.options = listed.options;
    if (listed.optionsTotal) {
      problem.options_total = listed.optionsTotal;
      const containing = optionsContaining(select, value);
      if (containing.length) problem.containing = containing;
    }
    return problem;
  }

  // The option one string names in a select, or the problem that says why none. An empty string
  // names the empty placeholder and no other text does.
  function resolveOption(select, value) {
    const placeholder = selectPlaceholder(select);
    if (value === "") {
      if (!placeholder) return { problem: selectProblem(select, FIELD_PROBLEMS.optionMissing, value) };
      return optionDisabled(placeholder) ? { problem: selectProblem(select, FIELD_PROBLEMS.optionDisabled, value) } : { option: placeholder };
    }
    const entries = Array.from(select.options ?? [])
      .filter((option) => option !== placeholder)
      .map((option) => ({ option, keys: selectKeys(option) }));
    const { hits, tier } = matchEntries(entries, value);
    if (hits.length > 1) return { problem: selectProblem(select, FIELD_PROBLEMS.optionAmbiguous, value, hits) };
    if (!hits.length) return { problem: selectProblem(select, FIELD_PROBLEMS.optionMissing, value) };
    if (optionDisabled(hits[0].option)) return { problem: selectProblem(select, FIELD_PROBLEMS.optionDisabled, value) };
    return { option: hits[0].option, normalized: tier === MATCH_NORMALIZED };
  }

  // What a fill value does to a select: the set of options it names, exactly. A single select
  // takes one string, a multiple select an array of them or one string for just that option.
  function resolveSelect(select, value) {
    const multiple = isMultipleSelect(select);
    const listed = Array.isArray(value);
    const chosen = [];
    let normalized = false;
    for (const [index, item] of (listed ? value : [value]).entries()) {
      const found = resolveOption(select, String(item));
      if (found.problem) return { problem: listed ? { ...found.problem, value_index: index } : found.problem };
      chosen.push(found.option);
      normalized = normalized || found.normalized === true;
    }
    const wanted = new Set(chosen);
    const placeholder = selectPlaceholder(select);
    const cleared = chosen.length === 1 && chosen[0] === placeholder;
    return {
      normalized,
      element: select,
      apply() {
        if (multiple) for (const option of Array.from(select.options)) option.selected = wanted.has(option);
        else chosen[0].selected = true;
      },
      holds() {
        const held = Array.from(select.selectedOptions ?? []);
        return held.length === wanted.size && held.every((option) => wanted.has(option));
      },
      reading() {
        if (cleared && this.holds()) return "";
        const texts = Array.from(select.selectedOptions ?? []).map((option) => oneLine(option.text));
        return shared.bounded(texts.join(", "), VALUE_MAX);
      }
    };
  }

  // What a fill value does to a checkbox: it sets the box, never toggles it.
  function resolveCheckbox(box, value) {
    const wanted = booleanOf(value);
    if (wanted === undefined) return { problem: { problem: FIELD_PROBLEMS.notBoolean } };
    return {
      normalized: false,
      element: box,
      apply() { setNativeChecked(box, wanted); },
      holds() { return box.checked === wanted; },
      reading() { return box.checked ? "checked" : "unchecked"; }
    };
  }

  // The radios that share this one's group: the same name, form owner and root.
  function radioGroup(radio) {
    const name = radio.getAttribute("name");
    if (!name) return [radio];
    const owner = radio.form ?? null;
    const members = Array.from(radio.getRootNode().querySelectorAll?.("input") ?? [])
      .filter((member) => inputType(member) === "radio" && member.getAttribute("name") === name && (member.form ?? null) === owner);
    return members.includes(radio) ? members : [radio, ...members];
  }

  // Why no radio of a group answers to a value: the group and the labels that would.
  function radioProblem(radio, members, kind, hits = []) {
    const problem = { problem: kind };
    if (credentialClass(radio) || kind === FIELD_PROBLEMS.optionDisabled) return problem;
    const shown = (kind === FIELD_PROBLEMS.optionAmbiguous ? hits.map((hit) => hit.element) : members).slice(0, OPTIONS_MAX);
    problem.options = shown.map((member) => choiceEntry({ text: oneLine(accessibleName(member)), value: "", disabled: isDisabled(member) }, false));
    if (kind === FIELD_PROBLEMS.optionMissing && members.length > OPTIONS_MAX) problem.options_total = members.length;
    const group = controlGroup(radio);
    if (group) problem.group = group;
    return problem;
  }

  // What a fill value does to a radio. A JSON boolean applies to this radio. A string names the
  // radio of the group whose label it matches, and that radio is the one set, focused and told of
  // the change; a string that names none but is a boolean word applies to this radio.
  function resolveRadio(radio, value) {
    let target = radio;
    let wanted = typeof value === "boolean" ? value : true;
    let normalized = false;
    if (typeof value !== "boolean") {
      const members = radioGroup(radio);
      const labelled = members.map((member) => ({ element: member, keys: [oneLine(accessibleName(member))].filter(Boolean) }));
      const { hits, tier } = matchEntries(labelled, value);
      if (hits.length > 1) return { problem: radioProblem(radio, members, FIELD_PROBLEMS.optionAmbiguous, hits) };
      if (hits.length === 1) {
        target = hits[0].element;
        normalized = tier === MATCH_NORMALIZED;
        if (isDisabled(target)) return { problem: radioProblem(radio, members, FIELD_PROBLEMS.optionDisabled) };
      } else {
        wanted = booleanOf(value);
        if (wanted === undefined) return { problem: radioProblem(radio, members, FIELD_PROBLEMS.optionMissing) };
      }
    }
    return {
      normalized,
      element: target,
      apply() { setNativeChecked(target, wanted); },
      holds() { return target.checked === wanted; },
      // What the addressed radio holds. A radio picked through a sibling's label is not the one
      // that changed, so its own reading never claims the sibling's label.
      reading() { return radio.checked ? "checked" : "unchecked"; },
      // The sibling picked in the addressed radio's place and the group, once the page holds it.
      picked() {
        if (target === radio || !target.checked) return undefined;
        return { label: shared.bounded(oneLine(accessibleName(target)), OPTION_TEXT_MAX), group: controlGroup(radio) };
      }
    };
  }

  // What a fill value does to a native choice control, or null for a control that is not one.
  // One function backs the check before a fill, the fill itself and the check that the page kept
  // it, so they cannot disagree. A result with a `problem` changes nothing. Its `element` is the
  // control the fill changes: a sibling when a radio is picked by another radio's label.
  function resolveChoice(element, value) {
    const kind = element instanceof HTMLSelectElement ? "select"
      : element instanceof HTMLInputElement && element.type === "checkbox" ? "checkbox"
      : element instanceof HTMLInputElement && element.type === "radio" ? "radio"
      : null;
    if (!kind) return null;
    if (Array.isArray(value) && !isMultipleSelect(element)) return { problem: { problem: FIELD_PROBLEMS.multipleValues } };
    if (kind === "select") return resolveSelect(element, value);
    return kind === "checkbox" ? resolveCheckbox(element, value) : resolveRadio(element, value);
  }

  // The control a fill is the wrong tool for, with the tool it is left to: a file input is
  // attached with an upload, a button or a submit, reset or image input is pressed with a click,
  // and a page widget with a combobox or listbox role that is not a form control is opened with a
  // click. The route comes from the element, never from the role the control reads as.
  function wrongToolProblem(element) {
    const tag = tagOf(element);
    const type = inputType(element);
    const refused = (route) => ({ problem: FIELD_PROBLEMS.wrongTool, route });
    if (tag === "input" && type === "file") return refused(WRONG_TOOL_ROUTES.file);
    if (tag === "button" || (tag === "input" && BUTTON_INPUT_TYPES.has(type))) return refused(WRONG_TOOL_ROUTES.button);
    const formControl = tag === "input" || tag === "textarea" || tag === "select" || element.isContentEditable;
    if (!formControl && WIDGET_ROLES.has(element.getAttribute("role"))) return refused(WRONG_TOOL_ROUTES.widget);
    return null;
  }

  // A number attribute of a range input, or the fallback when it is missing or not a number.
  function rangeAttribute(element, name, fallback) {
    const declared = element.getAttribute(name);
    const number = declared === null || declared.trim() === "" ? Number.NaN : Number(declared);
    return Number.isFinite(number) ? number : fallback;
  }

  // The bounds and step a range input applies, as the refusal reports them. A bound that is
  // missing or not a number is the default, a maximum below the minimum is the minimum, and a step
  // that is missing, not a number or not above zero is the default step. The keyword "any" is no
  // step at all: the browser clamps such a range to its bounds and never snaps it, so the limits
  // leave the step out.
  function rangeLimits(element) {
    const min = rangeAttribute(element, "min", RANGE_DEFAULT_LIMITS.min);
    const max = Math.max(min, rangeAttribute(element, "max", RANGE_DEFAULT_LIMITS.max));
    const limits = { min: String(min), max: String(max) };
    if (element.getAttribute("step")?.trim().toLowerCase() === RANGE_FREE_STEP) return limits;
    const declaredStep = rangeAttribute(element, "step", RANGE_DEFAULT_LIMITS.step);
    limits.step = String(declaredStep > 0 ? declaredStep : RANGE_DEFAULT_LIMITS.step);
    return limits;
  }

  // Whether the value a detached input kept is the value given. A color keeps its value in lower
  // case, and a range writes a number back in its own form, so those compare as the browser
  // reads them. Every other type must come back as given.
  function keptAsGiven(type, held, given) {
    if (type === "color") return held.toLowerCase() === given.toLowerCase();
    if (type === "range") return given.trim() !== "" && Number(held) === Number(given);
    return held === given;
  }

  // Why a typed input cannot take a value, or null when it can. The browser sanitizes a value it
  // does not understand to an empty one (or to its default for a color or a range), so the value is
  // assigned to a detached input of the same type with the same bounds and read back. It must come
  // back as given and inside the declared range. A range also clamps to its bounds and snaps to
  // its step, so a value it moved is refused with the bounds and step in force. The refusal says
  // the shape and the limits, never the value.
  function typedProblem(element, value) {
    const type = inputType(element);
    const expected = TYPED_FORMATS[type];
    if (!expected) return null;
    const given = String(value);
    const limits = {};
    const probe = document.createElement("input");
    probe.setAttribute("type", type);
    for (const attribute of TYPED_LIMIT_ATTRIBUTES) {
      const declared = element.getAttribute(attribute);
      if (declared === null || declared === "") continue;
      probe.setAttribute(attribute, declared);
      limits[attribute] = shared.bounded(oneLine(declared), LIMIT_MAX);
    }
    setNativeValue(probe, given);
    const outside = probe.validity?.rangeUnderflow === true || probe.validity?.rangeOverflow === true;
    if (keptAsGiven(type, String(probe.value), given) && !outside) return null;
    const problem = { problem: FIELD_PROBLEMS.format, type, expected };
    const reported = type === "range" ? rangeLimits(element) : limits;
    if (Object.keys(reported).length) problem.limits = reported;
    return problem;
  }

  // How a fill value lands on a control: the choice it resolves to, or the problem that says why
  // it cannot. A control that is not a native choice has neither, unless a list was given to it.
  // Before an edit (`beforeEdit`) the control must also be one a fill is the right tool for, and a
  // typed input must be able to hold the value. After the edit neither is asked again, because the
  // page may have moved a bound since.
  function checkFillElement(element, value, allowCredentials = false, beforeEdit = false) {
    const wrongTool = beforeEdit ? wrongToolProblem(element) : null;
    if (wrongTool) return { choice: null, problem: wrongTool };
    requireActionable(element, "fill");
    if (credentialClass(element) && allowCredentials !== true) throw credentialAuthorizationError(element);
    if (!(element instanceof HTMLSelectElement || element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element.isContentEditable)) {
      throw new AssetError("not_fillable", NOT_FILLABLE_MESSAGE);
    }
    const resolved = resolveChoice(element, value);
    if (resolved && !resolved.problem && resolved.element !== element) requireActionable(resolved.element, "fill");
    if (resolved) return { choice: resolved.problem ? null : resolved, problem: resolved.problem ?? null };
    if (Array.isArray(value)) return { choice: null, problem: { problem: FIELD_PROBLEMS.multipleValues } };
    return { choice: null, problem: beforeEdit ? typedProblem(element, value) : null };
  }

  // The refusal for one or more fields that cannot take their values. The detail is what the
  // service turns into a sentence: it never holds the value the caller gave.
  function fieldProblemsError(problems, withDetail) {
    const options = problems.every((problem) => OPTION_PROBLEMS.has(problem.problem));
    let message = options ? OPTION_MESSAGE : NOT_FILLABLE_MESSAGE;
    const words = problems.length > 1 ? `${problems.length} fields refused` : PROBLEM_WORDS[problems[0].problem];
    if (words) message += ` (${words})`;
    return new AssetError(options ? "option_missing" : "not_fillable", message, withDetail ? boundedDetail(problems) : undefined);
  }

  // How many options a listing holds, option groups flattened.
  function listedOptionCount(options) {
    return options.reduce((total, option) => total + (option.options ? listedOptionCount(option.options) : 1), 0);
  }

  // The detail within its size cap: options go first, then the lists of near matches. A problem
  // that loses its options still says how many there were, so the service never reads a dropped
  // list as an empty select.
  function boundedDetail(problems) {
    const detail = { problems: problems.map((problem) => ({ ...problem })) };
    const fits = () => JSON.stringify(detail).length <= PROBLEM_DETAIL_MAX;
    for (const key of ["options", "containing"]) {
      if (fits()) break;
      for (const problem of detail.problems) {
        if (key === "options" && problem.options?.length && problem.options_total === undefined) {
          problem.options_total = listedOptionCount(problem.options);
        }
        delete problem[key];
      }
    }
    return detail;
  }

  function validateFillElement(element, value, allowCredentials = false) {
    const { choice, problem } = checkFillElement(element, value, allowCredentials, true);
    if (problem) throw fieldProblemsError([problem], false);
    return choice;
  }

  // This is a read-only plan. Every known invalid field and submit target must be found
  // before an earlier field can be edited, including when the worker is preparing several frames.
  // Every field whose value cannot be taken is reported together, with what would be accepted.
  function prepareFill(message) {
    const elements = message.fields.map((field) => resolve(field.locator));
    const problems = [];
    elements.forEach((element, index) => {
      const { problem } = checkFillElement(element, message.fields[index].value, message.allow_credentials, true);
      if (problem) problems.push({ field: index, ...problem });
    });
    if (problems.length) throw fieldProblemsError(problems, true);
    let submitElement = null;
    if (message.submit_locator) {
      submitElement = requireActionable(resolve(message.submit_locator), "activate");
      const owner = elements[0] ? formOwner(elements[0]) : null;
      if (!owner || formOwner(submitElement) !== owner) throw new AssetError("submit_outside_form", "submit control is not contained in the resolved form");
    }
    return { elements, submitElement, fieldKinds: elements.map((element) => isTextual(element) ? "browser_text" : "local") };
  }

  function fillStateSignature(elements) {
    return JSON.stringify(elements.map((element) => fillControlValue(element)));
  }

  function fillControlValue(element) {
    if (element instanceof HTMLInputElement && ["checkbox", "radio"].includes(element.type)) return element.checked;
    if (isMultipleSelect(element)) return JSON.stringify(Array.from(element.selectedOptions ?? []).map((option) => option.value));
    if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement) return element.value;
    if (element.isContentEditable) return element.textContent ? (element.innerText ?? element.textContent) : "";
    return element.textContent ?? "";
  }

  // What a text or typed control holds once the value is in. A native choice is checked by its
  // resolution instead.
  function expectedFillValue(element, value) {
    // A color input holds its value in lower case whatever case it was given in.
    if (inputType(element) === "color") return String(value).toLowerCase();
    // A range input writes a number back in its own form: 5.0 is held as 5.
    if (inputType(element) === "range" && String(value).trim() !== "" && Number.isFinite(Number(value))) return String(Number(value));
    // The textarea API and contenteditable innerText normalize both CRLF and lone CR to LF.
    return (element instanceof HTMLTextAreaElement || element.isContentEditable)
      ? String(value).replace(/\r\n?/g, "\n")
      : String(value);
  }


  // Whether the fill reads this control's value back: a choice or a typed control always, free
  // text only when the page changed its formatting.
  function echoedControl(element) {
    if (element instanceof HTMLSelectElement) return true;
    return element instanceof HTMLInputElement
      && (element.type === "checkbox" || element.type === "radio" || ECHOED_INPUT_TYPES.has(element.type));
  }

  function foldedFormat(value) {
    return String(value).replace(FORMATTING_CHARACTERS, "").toLowerCase();
  }

  // What the page holds in a filled control, in words that can be said. Undefined for a
  // credential-class control, which never shows what it holds.
  function heldNow(element, choice = null) {
    if (credentialClass(element)) return undefined;
    if (choice) return choice.reading();
    return outwardValue(element)?.value;
  }

  // How each filled field stands against what was given: held as given, held in another format
  // (text only, never a credential-class control), or changed by the page.
  function fillStanding(fields, allowCredentials = false) {
    return fields.map((field, index) => {
      const element = resolve(field.locator);
      const { choice, problem } = checkFillElement(element, field.value, allowCredentials);
      if (problem) return { index, element, state: "changed", reacted: true };
      if (choice) return { index, element, choice, state: choice.holds() ? "held" : "changed" };
      const held = fillControlValue(element);
      const expected = expectedFillValue(element, field.value);
      if (held === expected) return { index, element, state: "held" };
      const reformatted = !credentialClass(element) && !echoedControl(element) && typeof held === "string" && foldedFormat(held) === foldedFormat(expected);
      return { index, element, state: reformatted ? "reformatted" : "changed" };
    });
  }

  // What a fill reports back once every value held: each choice and typed control with what it
  // holds, and each text field the page reformatted. Free text that equals what was given is not
  // echoed, and a credential-class control never is.
  function fillEchoes(standing) {
    const echoes = [];
    for (const { index, element, choice, state } of standing) {
      if (credentialClass(element) || (state === "held" && !echoedControl(element))) continue;
      const now = heldNow(element, choice);
      if (now === undefined) continue;
      const echo = { field: index, now };
      if (state === "reformatted") echo.tolerated = true;
      if (choice?.normalized) echo.matched = MATCH_NORMALIZED;
      const picked = choice?.picked?.();
      if (picked) {
        echo.picked = picked.label;
        if (picked.group) echo.group = picked.group;
      }
      echoes.push(echo);
    }
    return echoes;
  }

  // The failure of a fill whose values did not stay: every changed field and what it holds now.
  // A field the page reacted to by refusing the value it set holds nothing worth quoting.
  function revertedError(standing) {
    const problems = standing
      .filter(({ state }) => state === "changed")
      .map(({ index, element, choice, reacted }) => {
        const now = reacted ? undefined : heldNow(element, choice);
        return now === undefined ? { field: index, problem: FIELD_PROBLEMS.reverted } : { field: index, problem: FIELD_PROBLEMS.reverted, now };
      });
    return new AssetError("not_retained", "target did not retain filled value", problems.length ? { problems } : undefined);
  }

  // A browser document can be ready while a client-rendered form is still hydrating from an
  // API response. Editing during that window produces real trusted events and a temporary dirty
  // state, then the hydration result replaces the draft. Wait for a stable target set and ready
  // state, but only within the budget the call has left: visual_settle false skips the wait,
  // and a spent budget (settle_ms 0) with the agent's intent intact still rechecks the values
  // once. The final current target set remains usable when the budget expires.
  async function prepareStableFill(message, ctx) {
    if (message.visual_settle === false) return prepareFill(message);
    const budget = message.settle_ms ?? 0;
    if (budget <= 0) {
      prepareFill(message);
      await sleep(FILL_RECHECK_MS);
      throwIfCancelled(ctx);
      return prepareFill(message);
    }
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
      if (now - started >= budget) {
        return prepareFill(message);
      }
      await sleep(Math.min(FILL_SETTLE_POLL_MS, budget - (now - started)));
    }
  }

  // The complete fill must be visible at the boundary the person sees: a later field can trigger
  // page code that rolls back or disables an earlier edit. Ok once every field has held its value
  // for stable_ms, with what the page now holds in the fields worth reading back; not_retained
  // when limit_ms passes first, naming each field that was changed.
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
      const standing = fillStanding(message.fields, message.allow_credentials);
      if (standing.every(({ state }) => state !== "changed")) {
        if (stableSince === null) stableSince = now;
        if (now - stableSince >= stableMs) return { retained: true, fields: fillEchoes(standing) };
      } else {
        stableSince = null;
      }
      if (now >= limit) throw revertedError(standing);
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
    const choice = validateFillElement(element, value, allowCredentials);
    const changed = choice?.element ?? element;
    changed.scrollIntoView({ block: "center", inline: "center" });
    changed.focus({ preventScroll: true });
    if (choice) choice.apply(); else setNativeValue(element, String(value));
    changed.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
    changed.dispatchEvent(new Event("change", { bubbles: true, composed: true }));
  }

  function requireActionable(element, intent) {
    if (!element.isConnected) throw removedElementError(element);
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
    if (isDisabled(element) || closestAcrossBoundaries(element, "[inert]")) {
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
    if (!element.isConnected) throw removedElementError(element);
    if (!(element instanceof HTMLInputElement) || element.type !== "file") {
      throw new AssetError("not_file_input", "target is not a file input");
    }
    if (isDisabled(element) || closestAcrossBoundaries(element, "[inert]")) {
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
    const needle = typeof message.value === "string" ? shared.normalizeNeedle(message.value) : "";
    const pageHasText = (value) => plainText(document.body || document.documentElement, Number.MAX_SAFE_INTEGER).text.includes(value);
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
      const targetElement = message.locator ? heldElement(message.locator) : document;
      const visual = await sensor.settleVisual(targetElement, settleOptions(message.timeout_ms));
      if (!visual.settled) return unmet(readiness());
      return { satisfied: true, elapsed_ms: elapsed(), readiness: readiness() };
    }

    while (true) {
      throwIfCancelled(ctx);
      let satisfied = false;
      if (message.condition === "load_ready") satisfied = document.readyState === "interactive" || document.readyState === "complete";
      if (message.condition === "url_contains") satisfied = location.href.includes(message.value);
      if (message.condition === "text_present") satisfied = pageHasText(needle);
      if (message.condition === "text_absent") satisfied = !pageHasText(needle);
      if (message.condition === "target_present") satisfied = Boolean(heldElement(message.locator)?.isConnected);
      if (message.condition === "target_absent") satisfied = !heldElement(message.locator)?.isConnected;
      if (satisfied) {
        if (message.visual_settle === true) {
          const targetElement = message.locator ? heldElement(message.locator) : document;
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

  // One sampled read: the sensor answers once the page has been quiet and the sample is
  // non-empty or the document is complete, unless the caller asked for no waiting.
  function sampled(message, ctx, extract, isSatisfied) {
    if (message.visual_settle === false) return extract();
    return sensor.settle(extract, isSatisfied, {
      maxWaitMs: message.settle_ms,
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
        ? readTarget(resolve(message.locator), message.max_chars)
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
      releaseGoneLocators();
      const extract = () => inspect(message.inspect_kind, message.max_items);
      return sampled(message, ctx, extract, (res) => res.targets.length > 0);
    }
    if (message.kind === "find") {
      releaseGoneLocators();
      const extract = () => findTargets(message.text, message.find_kind, message.max_results);
      return sampled(message, ctx, extract, (res) => res.targets.length > 0);
    }
    if (message.kind === "describe") {
      releaseGoneLocators();
      const view = observationView();
      return { targets: message.locators.map((locator) => observation(resolve(locator), view)) };
    }
    if (message.kind === "query_semantic") {
      releaseGoneLocators();
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

  world.onDispose(() => {
    locators.clear();
    liveCount = 0;
    goneCount = 0;
  });

  return { dispatch, applyAmbient, renderPresentation };
})();
