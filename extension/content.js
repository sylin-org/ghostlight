// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- content script.
//
// Policy-free DOM mechanism: accessibility-tree generation, element-ref mapping (WeakRef), text
// extraction, element finding, and form input with shadow-DOM traversal. Runs in the page; the
// service worker calls in via chrome.tabs.sendMessage. No governance here.
//
// The engine is TRUTHFUL: it returns the raw page, including secret field values. It only MARKS a
// secret field's value with the `secret_value="..."` attribute (a neutral fact: the page marks this
// field secret). The governance overlay in the binary rewrites that marker -- redacting per the
// `content.security.secrets.redact` policy key -- before the result leaves the machine. The decision
// is the binary's; the engine never lies.

(function () {
  if (window.__browserMcpLoaded) return;
  window.__browserMcpLoaded = true;

  // --- Element refs (persist across calls; WeakRef so the page can still GC) ---
  // Each ref also remembers the render serial at which it was minted (ADR-0037 D4): a deref miss on
  // a ref whose serial is older than the current one is a stale ref (the page re-rendered), which
  // gets a corrective error naming the re-render; a ref that was never minted keeps today's message.
  let refSeq = 0;
  let renderSerial = 0;
  const refToEl = {}; // ref -> WeakRef<Element>
  const refToSerial = {}; // ref -> render serial at mint time
  const elToRef = new WeakMap();
  function refFor(el) {
    const existing = elToRef.get(el);
    if (existing && refToEl[existing] && refToEl[existing].deref() === el) return existing;
    const ref = "ref_" + ++refSeq;
    refToEl[ref] = new WeakRef(el);
    refToSerial[ref] = renderSerial;
    elToRef.set(el, ref);
    return ref;
  }
  function deref(ref) {
    const wr = refToEl[ref];
    if (!wr) return null;
    const el = wr.deref();
    if (!el) { delete refToEl[ref]; return null; } // serial kept for stale-ref diagnosis below
    return el;
  }
  // ADR-0037 D4 / PINS.md SS11: when a ref no longer resolves because the page re-rendered since
  // the read that minted it (the ref's mint serial is older than the current render serial), the
  // failure is corrective -- it names the re-render and the fix. A ref that was never minted (or
  // minted in this same not-yet-re-rendered serial) keeps today's "not found" wording: there is no
  // re-render to blame. Returns null when the plain message should stand.
  function staleRefMessage(ref) {
    const s = refToSerial[ref];
    if (s === undefined || s >= renderSerial) return null;
    return `${ref} no longer resolves: the page re-rendered since your last read (render serial ${s} -> ${renderSerial}). Call read_page (or read_page with diff: true) and use a fresh ref.`;
  }

  // --- Subtree mutation counter (shared by wait_for's settle detector and the consequence-digest
  // sampler): one permanent MutationObserver, lazily started, that only increments a counter -- no
  // node retention, so a hostile page cannot bloat memory. wait_for reads this in 500ms windows;
  // the observe pair reads the delta across a 300ms settle sample (ADR-0037 D2/D5). ---
  let mutationCounter = 0;
  let rootObserver = null;
  function ensureRootObserver() {
    if (rootObserver) return;
    rootObserver = new MutationObserver(() => { mutationCounter += 1; });
    rootObserver.observe(document, { childList: true, subtree: true, attributes: true, characterData: true });
    // The render serial (ADR-0037 D3/D4) bumps once per 500ms window with >= 3 mutations, so a
    // re-render large enough to invalidate prior refs is detectable. Lazy-started alongside the
    // observer so the windowing begins the first time anything reads mutations.
    let windowStart = Date.now();
    let windowCount = 0;
    let lastRead = 0;
    setInterval(() => {
      const cur = mutationCounter;
      windowCount += cur - lastRead;
      lastRead = cur;
      if (windowCount >= 3) renderSerial += 1;
      windowCount = 0;
    }, 500);
  }
  function readMutations() {
    ensureRootObserver();
    return mutationCounter;
  }

  // --- read_page diff baseline (ADR-0037 D3): the last full-tree render's lines, kept per
  // content-script instance (one per tab document). A read_page with diff:true diffs against this;
  // the first read, or one after reinjection, has no baseline and falls back to a full tree. ---
  let lastTreeLines = null;

  // --- Role / name / interactivity / visibility ---
  const TAG_ROLE = {
    a: "link", button: "button", input: "textbox", textarea: "textbox", select: "combobox",
    img: "img", h1: "heading", h2: "heading", h3: "heading", h4: "heading", h5: "heading",
    h6: "heading", nav: "navigation", main: "main", form: "form", ul: "list", ol: "list",
    li: "listitem", table: "table", tr: "row", th: "columnheader", td: "cell", dialog: "dialog",
    section: "region", article: "article", summary: "button",
  };
  function role(el) {
    if (el.getAttribute("role")) return el.getAttribute("role");
    const tag = el.tagName.toLowerCase();
    if (tag === "input") {
      const t = (el.type || "text").toLowerCase();
      return ({ checkbox: "checkbox", radio: "radio", range: "slider", button: "button",
        submit: "button", reset: "button", search: "searchbox", number: "spinbutton" })[t] || "textbox";
    }
    return TAG_ROLE[tag] || null;
  }
  function accessibleName(el) {
    // A <select> names itself by its selected option so the model sees the current choice.
    if (el.tagName.toLowerCase() === "select") {
      const sel = el.querySelector("option[selected]") || (el.options && el.options[el.selectedIndex]);
      if (sel && sel.textContent && sel.textContent.trim()) return sel.textContent.trim();
    }
    const ariaLabel = el.getAttribute("aria-label");
    if (ariaLabel) return ariaLabel.trim();
    const labelledBy = el.getAttribute("aria-labelledby");
    if (labelledBy) {
      const names = labelledBy.split(/\s+/).map((id) => {
        const t = document.getElementById(id);
        return t && t.textContent ? t.textContent.trim() : "";
      }).filter(Boolean);
      if (names.length) return names.join(" ");
    }
    // typeof-guarded: on a <form>, a child control named/id'd "title", "placeholder", or "alt"
    // shadows the same-named IDL property with that control element (HTMLFormElement's named-item
    // behavior), so el.title etc. can be an Element instead of a string -- .trim() would throw.
    if (typeof el.placeholder === "string" && el.placeholder) return el.placeholder.trim();
    if (typeof el.title === "string" && el.title) return el.title.trim();
    if (typeof el.alt === "string" && el.alt) return el.alt.trim();
    if (el.id) {
      const label = document.querySelector(`label[for="${CSS.escape(el.id)}"]`);
      if (label) return label.textContent.trim();
    }
    const wrapping = el.closest && el.closest("label");
    if (wrapping) { const t = wrapping.textContent.trim(); if (t) return t; }
    const tag = el.tagName.toLowerCase();
    if (["a", "button", "h1", "h2", "h3", "h4", "h5", "h6", "li", "summary", "label", "th", "td", "span"].includes(tag)) {
      const t = el.textContent && el.textContent.trim();
      if (t && t.length < 200) return t;
    }
    return "";
  }
  function interactive(el) {
    const tag = el.tagName.toLowerCase();
    if (["a", "button", "input", "textarea", "select", "summary", "details"].includes(tag)) return true;
    const r = el.getAttribute("role");
    if (r && ["button", "link", "textbox", "checkbox", "radio", "tab", "menuitem", "switch", "combobox", "slider", "spinbutton", "searchbox", "option"].includes(r)) return true;
    if (el.tabIndex >= 0) return true;
    if (el.onclick || el.getAttribute("onclick")) return true;
    if (el.isContentEditable) return true;
    return false;
  }
  function visible(el) {
    if (el.offsetParent === null && el.tagName.toLowerCase() !== "body" && getComputedStyle(el).position !== "fixed") return false;
    const s = getComputedStyle(el);
    return s.display !== "none" && s.visibility !== "hidden";
  }
  // getBoundingClientRect is viewport-relative for every element, so this is correct at any
  // scroll position and for position:fixed elements without special cases.
  function intersectsViewport(el) {
    const rect = el.getBoundingClientRect();
    return rect.bottom > 0 && rect.right > 0 && rect.top < window.innerHeight && rect.left < window.innerWidth;
  }

  // --- Sensitive fields: mark (do not hide) their values so the binary's policy overlay can act ---
  // Gate on the input type and on the sensitive `autocomplete` tokens the platform defines for
  // credentials, one-time codes, and payment data (the platform's own signal that a field is a
  // secret -- a structural fact, not content inspection).
  const SENSITIVE_AUTOCOMPLETE = [
    "current-password", "new-password", "one-time-code",
    "cc-number", "cc-csc", "cc-exp", "cc-exp-month", "cc-exp-year",
  ];
  function sensitive(el) {
    const t = (el.getAttribute("type") || "").toLowerCase();
    if (t === "password" || t === "hidden") return true;
    const ac = (el.getAttribute("autocomplete") || "").toLowerCase();
    return SENSITIVE_AUTOCOMPLETE.some((s) => ac.indexOf(s) !== -1);
  }

  // ADR-0087: observe the actual trusted keydown target, not document.activeElement before the
  // action. Focus may move while a chord is dispatched. Retain only a bounded structural target
  // class; never retain the key, field value, element, or any page text.
  const KEY_CUE_OBSERVATION_MAX_EVENTS = 600;
  const KEY_CUE_OBSERVATION_MESSAGES = self.GhostlightKeys.KEY_CUE_OBSERVATION_MESSAGES;
  let keyCueObservationSequence = 0;
  let activeKeyCueObservation = null;

  function keyCueTarget(event) {
    const targets = self.GhostlightKeys && self.GhostlightKeys.KEY_CUE_TARGETS;
    if (!targets) return "unknown";
    let target = null;
    if (typeof event.composedPath === "function") {
      target = event.composedPath().find((node) => node instanceof Element) || null;
    }
    if (!target && event.target instanceof Element) target = event.target;
    if (!target) return targets.UNKNOWN;
    const tag = target.tagName.toLowerCase();
    if ((tag === "input" || tag === "textarea") && sensitive(target)) {
      return targets.PROTECTED;
    }
    return targets.ORDINARY;
  }

  function stopKeyCueObservation() {
    if (!activeKeyCueObservation) return null;
    document.removeEventListener("keydown", activeKeyCueObservation.listener, true);
    const stopped = activeKeyCueObservation;
    activeKeyCueObservation = null;
    return stopped;
  }

  function beginKeyCueObservation(expectedCount) {
    stopKeyCueObservation();
    const token = ++keyCueObservationSequence;
    const limit = Math.min(
      Math.max(0, Number.isInteger(expectedCount) ? expectedCount : 0),
      KEY_CUE_OBSERVATION_MAX_EVENTS
    );
    const observation = { token, limit, targetStates: [], overflow: false, listener: null };
    observation.listener = (event) => {
      if (!event.isTrusted) return;
      if (observation.targetStates.length >= observation.limit) {
        observation.overflow = true;
        return;
      }
      observation.targetStates.push(keyCueTarget(event));
    };
    activeKeyCueObservation = observation;
    document.addEventListener("keydown", observation.listener, true);
    return { token };
  }

  function finishKeyCueObservation(token) {
    if (!activeKeyCueObservation || activeKeyCueObservation.token !== token) {
      stopKeyCueObservation();
      return { targetStates: [], overflow: true };
    }
    const observation = stopKeyCueObservation();
    return {
      targetStates: observation.targetStates.slice(),
      overflow: observation.overflow,
    };
  }

  // ADR-0088: distinguish a native HTML drag from pointer-only gestures without inspecting the
  // dragged element or its payload. The listener retains only whether a trusted dragstart occurred
  // and whether the page cancelled it. The post-dispatch microtask observes defaultPrevented after
  // bubble listeners have run.
  const DRAG_OBSERVATION_MESSAGES = self.GhostlightDragSession.DRAG_OBSERVATION_MESSAGES;
  let dragObservationSequence = 0;
  let activeDragObservation = null;

  function stopDragObservation() {
    if (!activeDragObservation) return null;
    document.removeEventListener("dragstart", activeDragObservation.listener, true);
    const stopped = activeDragObservation;
    activeDragObservation = null;
    return stopped;
  }

  function beginDragObservation() {
    stopDragObservation();
    const observation = {
      token: ++dragObservationSequence,
      started: false,
      cancelled: false,
      pending: Promise.resolve(),
      listener: null,
    };
    observation.listener = (event) => {
      if (!event.isTrusted || observation.started) return;
      observation.started = true;
      observation.pending = Promise.resolve().then(() => {
        observation.cancelled = event.defaultPrevented;
      });
    };
    activeDragObservation = observation;
    document.addEventListener("dragstart", observation.listener, true);
    return { token: observation.token };
  }

  async function finishDragObservation(token) {
    if (!activeDragObservation || activeDragObservation.token !== token) {
      stopDragObservation();
      return { started: false, cancelled: false };
    }
    const observation = stopDragObservation();
    await observation.pending;
    return { started: observation.started, cancelled: observation.cancelled };
  }

  // ADR-0078 D2: one compact element vocabulary shared by find, targeted read_page, and later
  // semantic action resolution. These are mechanism facts only. In particular,
  // `mechanicalActions` never means a governance grant allows the action.
  function mechanicalActions(el) {
    const actions = [];
    if (visible(el)) actions.push("hover", "scroll_to");
    const enabled = !el.disabled && el.getAttribute("aria-disabled") !== "true";
    if (enabled && interactive(el)) {
      actions.push("left_click", "right_click", "double_click");
      const tag = el.tagName.toLowerCase();
      if (["input", "textarea", "select"].includes(tag) || el.isContentEditable) {
        actions.push("set_value");
      }
    }
    return actions;
  }

  function elementSummary(el) {
    if (!el) return null;
    const tag = el.tagName.toLowerCase();
    const rect = el.getBoundingClientRect();
    const facts = {
      ref: refFor(el),
      role: role(el) || tag,
      name: accessibleName(el) || el.textContent || "",
      visible: visible(el),
      enabled: !el.disabled && el.getAttribute("aria-disabled") !== "true",
      box: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
      renderSerial,
      mechanicalActions: mechanicalActions(el),
    };
    if (typeof el.checked === "boolean" && ["checkbox", "radio"].includes((el.type || "").toLowerCase())) {
      facts.checked = el.checked;
    }
    if (typeof el.selected === "boolean" && tag === "option") facts.selected = el.selected;
    if (["input", "textarea", "select"].includes(tag) && el.value !== undefined && el.value !== "") {
      facts.value = String(el.value);
      facts.secret = sensitive(el);
    }
    if (tag === "a" && el.href) facts.href = el.href;
    return (self.GhostlightActionable || GhostlightActionable).makeSummary(facts);
  }

  // --- Accessibility tree (custom walk incl. shadow DOM) ---
  // Two-pass design: pass 1 (measure) walks the DOM once and builds a render tree with
  // per-subtree measurements; pass 2 (emit) walks that render tree top-down and decides, node
  // by node, whether the whole subtree fits the character budget, whether it must collapse
  // behind a marker, or whether the budget is exhausted and the walk must stop. Output that
  // fits the budget is byte-identical to a plain top-down serialization: markers and summary
  // lines only appear once the budget or the element cap is actually exceeded.
  function accessibilityTree(options) {
    options = options || {};
    const filter = options.filter || "all";
    const maxDepth = options.depth || 15;
    const maxChars = options.max_chars || 50000;
    const MAX_ELEMENTS = 10000;
    // A real page's reachable node count within a given depth is unbounded (wide, deeply nested
    // markup -- citation lists, infobox tables, and similar structures push this into the tens of
    // thousands well before any depth/filter/char-budget limit would otherwise apply). MAX_ELEMENTS
    // above only bounds pass 2's OUTPUT; without a bound on pass 1 itself, a single call could force
    // an unbounded synchronous DOM walk (getComputedStyle/getBoundingClientRect per node) on a large
    // page. MAX_MEASURED bounds pass 1's own work independent of maxDepth: once hit, deeper/further
    // nodes are treated as absent from the render tree (same as exceeding maxDepth), so the call
    // still returns promptly with whatever was measured rather than not returning at all.
    const MAX_MEASURED = 20000;
    let measured = 0;
    let culled = false; // true once the viewport test removes an element that would otherwise show

    // Pass 1: measure. Same entry guards, same show computation, same recursion order as a
    // single-pass walk would use; the only difference is that each visited node returns a
    // record (unit text plus subtree measurements) instead of appending to an output string.
    function measure(el, depth, indent) {
      if (depth > maxDepth || !el || el.nodeType !== 1) return null;
      if (++measured > MAX_MEASURED) return null;
      if (el.id && el.id.indexOf("ghostlight-") === 0) return null; // our own visual-indicator overlay
      const tag = el.tagName.toLowerCase();
      if (["script", "style", "noscript", "template"].includes(tag)) return null;
      const r = role(el);
      const n = accessibleName(el);
      const isInteractive = interactive(el);
      const isVisible = visible(el);
      const isContainer = el.children.length > 0;
      if (filter === "interactive" && !isInteractive && !isContainer) return null;
      const wouldShow = ((filter === "all" && (r || n)) || (filter === "interactive" && isInteractive)) && isVisible;
      const show = wouldShow && (filter === "all" || intersectsViewport(el));
      if (wouldShow && !show) culled = true;
      let unit = "";
      let ref = null;
      if (show) {
        ref = refFor(el);
        let line = indent + (r || tag);
        if (n) line += ` "${n.slice(0, 100)}"`;
        line += ` [${ref}]`;
        if (tag === "a" && el.href) line += ` href="${el.href}"`;
        if (["input", "textarea"].includes(tag) && el.value) {
          // Truthful: always emit the raw value. Secret fields use the `secret_value` marker so the
          // binary's policy overlay can redact it; the engine itself makes no such decision.
          const attr = sensitive(el) ? "secret_value" : "value";
          line += ` ${attr}="${String(el.value).slice(0, 80)}"`;
        }
        if (tag === "input") line += ` type="${el.type || "text"}"`;
        const placeholder = el.getAttribute && el.getAttribute("placeholder");
        if (placeholder) line += ` placeholder="${placeholder}"`;
        if (el.disabled) line += " disabled";
        unit = line + "\n";
        // Emit <select> options as child lines so the model can see the available choices.
        if (tag === "select") {
          for (const opt of el.options) {
            const otext = (opt.textContent || "").replace(/\s+/g, " ").trim().slice(0, 100);
            let ol = indent + "  option";
            if (otext) ol += ` "${otext}"`;
            if (opt.selected) ol += " (selected)";
            if (opt.value && opt.value !== otext) ol += ` value="${opt.value}"`;
            unit += ol + "\n";
          }
        }
      }
      const children = [];
      // A <select> is a leaf in the tree: its <option>s are emitted above (or deliberately
      // suppressed when sensitive), so we never descend into them.
      if (tag !== "select") {
        const nextIndent = show ? indent + "  " : indent;
        if (el.shadowRoot) {
          for (const c of el.shadowRoot.children) {
            const child = measure(c, depth + 1, nextIndent);
            if (child) children.push(child);
          }
        }
        for (const c of el.children) {
          const child = measure(c, depth + 1, nextIndent);
          if (child) children.push(child);
        }
      }
      let subtreeChars = unit.length;
      let elements = show ? 1 : 0;
      for (const child of children) {
        subtreeChars += child.subtreeChars;
        elements += child.elements;
      }
      return { unit, ref, indent, children, unitChars: unit.length, subtreeChars, elements, show };
    }

    let root = document.body;
    if (options.ref_id) {
      const el = deref(options.ref_id);
      if (!el) {
        const stale = staleRefMessage(options.ref_id);
        return stale || `Error: ref_id "${options.ref_id}" not found or was garbage-collected.`;
      }
      root = el;
    }
    const rootRecord = measure(root, 0, "");
    const total = rootRecord ? rootRecord.elements : 0;

    // Pass 2: emit. Walk the render tree top-down and decide, per record, whether it fits whole,
    // must collapse behind a marker, or the whole emit pass must halt because even a collapsed
    // form does not fit. Once halted, no later record (at any level) is emitted: output is
    // always a prefix of document order plus markers, never a sequence with silent gaps.
    let out = "";
    let remaining = maxChars;
    let shown = 0;
    let collapsed = false; // a collapse marker was emitted
    let stopped = false; // the walk halted because even a collapsed form did not fit
    let capped = false; // the element cap was reached
    function emit(record, isRoot) {
      if (stopped || capped) return;
      if (!record.show) {
        // Pass-through node: it owns no line, so it cannot collapse; only its children can.
        for (const child of record.children) {
          emit(child);
          if (stopped || capped) return;
        }
        return;
      }
      if (record.subtreeChars <= remaining) {
        out += record.unit;
        remaining -= record.unitChars;
        shown++;
        if (shown >= MAX_ELEMENTS) { capped = true; return; }
        for (const child of record.children) {
          emit(child);
          if (stopped || capped) return;
        }
        return;
      }
      if (isRoot) {
        // The record the caller re-rooted at (via ref_id) must never collapse behind a marker
        // naming its own ref -- that would be an unexpandable loop (the caller is already looking
        // at this ref_id). Show its own line if it fits, then let each child decide individually
        // whether it fits whole, collapses behind its own marker, or halts the walk.
        if (record.unitChars > remaining) { stopped = true; return; }
        out += record.unit;
        remaining -= record.unitChars;
        shown++;
        if (shown >= MAX_ELEMENTS) { capped = true; return; }
        for (const child of record.children) {
          emit(child);
          if (stopped || capped) return;
        }
        return;
      }
      const markerLine = `${record.indent}  [subtree collapsed: ${record.elements - 1} elements; call read_page with ref_id=${record.ref} to expand]\n`;
      if (record.unitChars + markerLine.length <= remaining) {
        out += record.unit + markerLine;
        remaining -= record.unitChars + markerLine.length;
        shown++;
        if (shown >= MAX_ELEMENTS) capped = true;
        collapsed = true;
        return;
      }
      stopped = true;
    }
    if (rootRecord) emit(rootRecord, true);

    // The diffable lines are the element/structure lines emitted above -- everything in `out`
    // before the trailing summary and viewport footer. Split on "\n" and drop the empty tail.
    const treeLines = out.split("\n").filter((l) => l.length > 0);

    // read_page diff mode (ADR-0037 D3, PINS.md SS11): when diff:true and a baseline from a prior
    // full read on this tab exists, answer with only the changed/removed/added lines (render order
    // ~ / - / +) instead of the whole tree. The baseline is the prior full read's treeLines; this
    // read's treeLines become the next baseline regardless (a diff read still refreshes it). A
    // ref_id-rooted read does not establish a baseline (it is a subtree expansion, not the page).
    // No baseline yet (first read, or the content script was reinjected): fall back to a full tree
    // prefixed with the marker line so the model knows this is not a diff.
    if (options.diff && !options.ref_id) {
      if (lastTreeLines) {
        const d = (self.GhostlightTreeDiff || GhostlightTreeDiff).diffLines(lastTreeLines, treeLines);
        const diffOut = [];
        for (const l of d.changed) diffOut.push("~ " + l);
        for (const l of d.removed) diffOut.push("- " + l);
        for (const l of d.added) diffOut.push("+ " + l);
        if (!diffOut.length) diffOut.push("(no changes since your last read)");
        lastTreeLines = treeLines;
        return diffOut.join("\n") + `\n\nViewport: ${window.innerWidth}x${window.innerHeight}`;
      }
      lastTreeLines = treeLines;
      return "(no baseline; full tree)\n" + out + `\nViewport: ${window.innerWidth}x${window.innerHeight}`;
    }
    if (!options.ref_id) lastTreeLines = treeLines;

    const omitted = total - shown;
    if (capped && omitted > 0) {
      out += `[element cap reached: output stopped after ${MAX_ELEMENTS} elements; use filter="interactive", a ref_id subtree, or a smaller depth]\n`;
    }
    if (omitted > 0) {
      out += `[showing ${shown} of ${total} elements; expand a collapsed subtree with ref_id, or narrow with filter="interactive" or a smaller depth]\n`;
    }
    let result = out + `\nViewport: ${window.innerWidth}x${window.innerHeight}`;
    if (culled) {
      result += "\nNote: interactive results are limited to the current viewport; scroll or use filter=all for the full document.";
    }
    return result;
  }

  // --- Page text ---
  // Main-content candidates. An element can match several selectors; the FIRST selector in
  // this list that finds it is the one reported in the "Source element:" header, and ties
  // on innerText length go to the earlier selector.
  const PAGE_TEXT_SELECTORS = [
    "article",
    "main",
    '[role="main"]',
    '[itemprop="articleBody"]',
    ".entry-content",
    ".content-body",
    ".article-body",
    ".articleBody",
    ".post-content",
    ".story-body",
    "#content",
    ".content",
  ];
  // Conservative cleanup only: innerText already excludes hidden text and preserves layout
  // line breaks, so just tidy line endings and keep paragraph breaks intact.
  function normalizePageText(t) {
    return t
      .replace(/\r\n?/g, "\n")
      .replace(/[ \t]+\n/g, "\n")
      .replace(/\n{3,}/g, "\n\n")
      .trim();
  }
  function pageText(maxCharsArg) {
    const maxChars = typeof maxCharsArg === "number" && Number.isFinite(maxCharsArg) && maxCharsArg >= 1
      ? Math.floor(maxCharsArg)
      : 50000;
    let bestEl = null, bestText = "", bestSel = "body";
    const seen = new Set();
    for (const sel of PAGE_TEXT_SELECTORS) {
      for (const el of document.querySelectorAll(sel)) {
        if (seen.has(el)) continue;
        seen.add(el);
        const t = el.innerText || "";
        if (t.length > bestText.length) { bestEl = el; bestText = t; bestSel = sel; }
      }
    }
    if (!bestEl || bestText.length === 0) {
      bestSel = "body";
      bestText = (document.body && document.body.innerText) || "";
    }
    const body = normalizePageText(bestText);
    if (body.length < 10) {
      return `No readable text content found (source element: ${bestSel}). The page may be mostly visual or may render text dynamically. Use read_page to inspect the page structure instead.`;
    }
    const header = `Source element: ${bestSel}\n\n`;
    if (body.length > maxChars) {
      return header + body.slice(0, maxChars) + `\n\n[Truncated at ${maxChars} characters. Retry with a larger max_chars, or use read_page to get a structured view with element refs.]`;
    }
    return header + body;
  }

  // --- Find (traverses shadow roots) ---
  function collectAll(rootNode) {
    const out = [];
    for (const el of rootNode.querySelectorAll("*")) {
      out.push(el);
      if (el.shadowRoot) out.push(...collectAll(el.shadowRoot));
    }
    return out;
  }

  function actionableCandidates() {
    const candidates = [];
    for (const el of collectAll(document)) {
      if (!visible(el)) continue;
      if (el.id && el.id.indexOf("ghostlight-") === 0) continue; // our own visual-indicator overlay
      const tag = el.tagName.toLowerCase();
      if (["script", "style", "noscript", "template"].includes(tag)) continue;
      const summary = elementSummary(el);
      candidates.push(Object.assign({}, summary, {
        searchText: `${summary.role} ${summary.name} ${(el.textContent || "").slice(0, 200)} ${el.placeholder || ""} ${el.getAttribute("aria-label") || ""} ${typeof el.title === "string" ? el.title : ""} ${el.type || ""} ${tag}`,
      }));
    }
    return candidates;
  }

  function publicCandidate(candidate) {
    const result = Object.assign({}, candidate);
    delete result.searchText;
    const box = result.box || { x: 0, y: 0, width: 0, height: 0 };
    result.x = Math.round(box.x + box.width / 2);
    result.y = Math.round(box.y + box.height / 2);
    return result;
  }

  const FIND_VISUAL_RANGES_PER_RESULT = 8;
  const FIND_VISUAL_TOTAL_RANGES = 80;

  function findVisualTextNodes(root) {
    const nodes = [];
    const visit = (scope) => {
      const walker = document.createTreeWalker(scope, NodeFilter.SHOW_TEXT);
      let node;
      while ((node = walker.nextNode())) {
        const parent = node.parentElement;
        if (!parent || !node.nodeValue) continue;
        if (parent.closest && parent.closest('[id^="ghostlight-"]')) continue;
        if (["script", "style", "noscript", "template"].includes(parent.tagName.toLowerCase())) continue;
        nodes.push(node);
      }
      const elements = scope.querySelectorAll ? scope.querySelectorAll("*") : [];
      for (const element of elements) {
        if (element.shadowRoot) visit(element.shadowRoot);
      }
    };
    visit(root);
    return nodes;
  }

  function findVisualRanges(element, needles, seen, remaining) {
    const ranges = [];
    for (const rawNeedle of needles) {
      const needle = String(rawNeedle || "").trim().toLocaleLowerCase();
      if (!needle) continue;
      for (const node of findVisualTextNodes(element)) {
        const source = node.nodeValue || "";
        const haystack = source.toLocaleLowerCase();
        let start = 0;
        while (ranges.length < FIND_VISUAL_RANGES_PER_RESULT && ranges.length < remaining) {
          const index = haystack.indexOf(needle, start);
          if (index < 0) break;
          start = index + Math.max(needle.length, 1);
          let offsets = seen.get(node);
          if (!offsets) { offsets = new Set(); seen.set(node, offsets); }
          const key = `${index}:${index + needle.length}`;
          if (offsets.has(key)) continue;
          try {
            const range = document.createRange();
            range.setStart(node, index);
            range.setEnd(node, index + needle.length);
            if (Array.from(range.getClientRects()).some((rect) => rect.width > 0 && rect.height > 0)) {
              offsets.add(key);
              ranges.push(range);
            }
          } catch (_error) { /* a live page changed between ranking and presentation */ }
        }
        if (ranges.length >= FIND_VISUAL_RANGES_PER_RESULT || ranges.length >= remaining) break;
      }
      if (ranges.length) break;
    }
    return ranges;
  }

  function presentFindResults(selected, query, more) {
    if (!self.GhostlightFx || typeof self.GhostlightFx.findResults !== "function") return;
    try {
      const seen = new WeakMap();
      let rangeCount = 0;
      const entries = selected.map((candidate, index) => {
        const element = deref(candidate.ref);
        if (!element) return null;
        const ranges = findVisualRanges(
          element,
          [query, candidate.name],
          seen,
          FIND_VISUAL_TOTAL_RANGES - rangeCount
        );
        rangeCount += ranges.length;
        return { element, ranges, strongest: index === 0 };
      }).filter(Boolean);
      self.GhostlightFx.findResults(entries, { more: !!more });
    } catch (_error) {
      // Presentation is best-effort and must never alter the find result.
    }
  }

  function find(query, present) {
    const ranked = (self.GhostlightActionable || GhostlightActionable).rankCandidates(query, actionableCandidates());
    const more = ranked.length > 20;
    const selected = ranked.slice(0, 20);
    const results = selected.map(publicCandidate);
    if (present) presentFindResults(selected, query, more);
    return { results, more };
  }

  function resolveActionable(target) {
    const page = {
      url: location.href,
      origin: location.origin,
      title: document.title || "",
      renderSerial,
    };
    if (target && target.ref) {
      const el = deref(target.ref);
      if (!el) {
        return { error: staleRefMessage(target.ref) || `Element ${target.ref} not found or was garbage-collected.`, page };
      }
      const summary = publicCandidate(elementSummary(el));
      const top = document.elementFromPoint(summary.x, summary.y);
      const covered = !!(top && top !== el && !el.contains(top) && !top.contains(el));
      return { target: summary, candidates: [], ambiguous: false, covered, page };
    }
    const query = target && (target.query || target.name);
    const ranked = (self.GhostlightActionable || GhostlightActionable)
      .rankCandidates(query, actionableCandidates(), target && target.role);
    if (!ranked.length) {
      const frameOrigins = Array.from(document.querySelectorAll("iframe, frame"))
        .slice(0, 5)
        .map((frame) => {
          try { return new URL(frame.src || "about:blank", location.href).origin; }
          catch (_error) { return "unknown"; }
        });
      return {
        target: null,
        candidates: [],
        ambiguous: false,
        frameUnsupported: frameOrigins.length > 0,
        frameOrigins,
        page,
      };
    }
    const bestRank = ranked[0].matchRank;
    const best = ranked.filter((candidate) => candidate.matchRank === bestRank);
    if (best.length !== 1) {
      return { target: null, candidates: best.slice(0, 5).map(publicCandidate), ambiguous: true, more: best.length > 5, page };
    }
    const summary = publicCandidate(best[0]);
    const el = deref(summary.ref);
    const top = document.elementFromPoint(summary.x, summary.y);
    const covered = !!(el && top && top !== el && !el.contains(top) && !top.contains(el));
    return { target: summary, candidates: [], ambiguous: false, covered, page };
  }

  function currentPageMeta() {
    return {
      url: location.href,
      origin: location.origin,
      title: document.title || "",
      renderSerial,
    };
  }

  // --- Form input (shadow-DOM traversal + native setter so framework inputs register) ---
  function innerInput(el) {
    const tag = el.tagName.toLowerCase();
    if (["input", "textarea", "select"].includes(tag)) return el;
    const root = el.shadowRoot || el;
    const inner = root.querySelector("input, textarea, select");
    if (inner) return inner;
    for (const child of root.querySelectorAll("*")) {
      if (child.shadowRoot) {
        const deep = child.shadowRoot.querySelector("input, textarea, select");
        if (deep) return deep;
      }
    }
    return null;
  }
  // Field-touch splash (docs/design/visual-language.md): show the watcher WHICH field a form
  // write just touched. A direct call into agent-visual-indicator.js's same-isolated-world
  // GhostlightFx seam (both scripts share the extension's isolated world); best-effort and never
  // load-bearing -- the indicator gates on its own effects switch and capture-hiding state.
  function fieldFx(target) {
    try {
      const fx = self.GhostlightFx;
      if (fx && typeof fx.fieldSplash === "function") fx.fieldSplash(target);
    } catch (e) { /* effects are decorative; a form write never fails on them */ }
  }
  function scrollTargetFx(target) {
    try {
      const fx = self.GhostlightFx;
      if (fx && typeof fx.scrollTarget === "function") fx.scrollTarget(target);
    } catch (e) { /* effects are decorative; scrolling never fails on them */ }
  }
  function imageDropFx(target, x, y) {
    try {
      const fx = self.GhostlightFx;
      if (fx && typeof fx.imageDrop === "function") fx.imageDrop(target, x, y);
    } catch (e) { /* effects are decorative; image dispatch never fails on them */ }
  }
  function setFormValue(ref, value) {
    const el = deref(ref);
    if (!el) {
      const stale = staleRefMessage(ref);
      return { error: stale || `Element ${ref} not found or was garbage-collected.` };
    }
    el.scrollIntoView({ block: "center", behavior: "instant" });
    const target = innerInput(el) || el;
    const tag = target.tagName.toLowerCase();
    const type = (target.type || "").toLowerCase();
    if (tag === "select") {
      const opt = Array.from(target.options).find((o) => o.value === String(value) || o.textContent.trim() === String(value));
      target.value = opt ? opt.value : String(value);
    } else if (type === "checkbox" || type === "radio") {
      const want = typeof value === "boolean" ? value
        : typeof value === "number" ? value !== 0
        : value === "true" || value === "1";
      if (type === "radio" && !want) {
        return { error: "cannot uncheck a radio button; set another radio in the same group instead" };
      }
      if (target.checked !== want) target.click();
      fieldFx(target);
      return { success: true, checked: target.checked };
    } else if (target.isContentEditable) {
      target.textContent = String(value);
    } else if (["input", "textarea"].includes(tag)) {
      const proto = tag === "textarea" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      const setter = Object.getOwnPropertyDescriptor(proto, "value") && Object.getOwnPropertyDescriptor(proto, "value").set;
      if (setter) setter.call(target, String(value));
      else target.value = String(value);
    } else {
      try { target.value = String(value); } catch { return { error: `Cannot set value on <${tag}>.` }; }
    }
    target.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
    target.dispatchEvent(new Event("change", { bubbles: true, composed: true }));
    fieldFx(target);
    return { success: true, value: target.value };
  }

  // file_upload (ADR-0050 Decision 2): decode the base64 `files` into File objects and assign them
  // to a file <input> located by ref, via an in-page DataTransfer -- the same event tail
  // setFormValue uses. Never touches the host filesystem; the caller supplies the bytes.
  function setFiles(ref, files) {
    const el = deref(ref);
    if (!el) {
      const stale = staleRefMessage(ref);
      return { error: stale || `Element ${ref} not found or was garbage-collected.` };
    }
    const target = innerInput(el) || el;
    if (target.tagName !== "INPUT" || target.type !== "file") {
      return { error: "Element is not a file input. Found: <" + target.tagName.toLowerCase() + ">." };
    }
    const r = (self.GhostlightFileset || GhostlightFileset).decodeFiles(files);
    if (!r.ok) return { error: r.error };
    const dt = new DataTransfer();
    for (const item of r.decoded) {
      dt.items.add(new File([item.bytes], item.name, { type: item.type, lastModified: Date.now() }));
    }
    target.files = dt.files;
    target.focus();
    target.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
    target.dispatchEvent(new Event("change", { bubbles: true, composed: true }));
    fieldFx(target);
    return {
      success: true,
      output: "Uploaded " + r.decoded.length + " file(s) to file input: "
        + r.decoded.map((f) => f.name).join(", ")
        + " (" + Math.round(r.totalBytes / 1024) + " KB total)",
    };
  }

  // upload_image (ADR-0050 Decision 4): place a previously captured screenshot (base64 bytes passed
  // in by the binary from its per-session cache) into a file <input> located by ref -- the SAME
  // DataTransfer/event tail setFiles uses -- OR drag-drop it at a viewport coordinate. Never touches
  // the host filesystem; the caller supplies the bytes.
  function setImage(ref, coordinate, dataB64, filename, mimeType) {
    const name = (typeof filename === "string" && filename.length > 0) ? filename : "image.png";
    const type = (typeof mimeType === "string" && mimeType.length > 0) ? mimeType : "image/png";
    const decoded = (self.GhostlightFileset || GhostlightFileset).decodeFiles([{ data: dataB64, name, mimeType: type }]);
    if (!decoded.ok) return { error: decoded.error };
    const dt = new DataTransfer();
    dt.items.add(new File([decoded.decoded[0].bytes], name, { type, lastModified: Date.now() }));

    if (ref) {
      const el = deref(ref);
      if (!el) {
        const stale = staleRefMessage(ref);
        return { error: stale || `Element ${ref} not found or was garbage-collected.` };
      }
      const target = innerInput(el) || el;
      if (target.tagName !== "INPUT" || target.type !== "file") {
        return { error: "Element is not a file input. Found: <" + target.tagName.toLowerCase() + ">." };
      }
      target.files = dt.files;
      target.focus();
      target.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
      target.dispatchEvent(new Event("change", { bubbles: true, composed: true }));
      fieldFx(target);
      return { success: true, output: "Uploaded screenshot (" + name + ") to file input." };
    }

    if (Array.isArray(coordinate) && coordinate.length === 2) {
      const x = coordinate[0], y = coordinate[1];
      let el = document.elementFromPoint(x, y);
      // IFRAME descent (official v1.0.80 technique): if the point lands on a same-origin iframe,
      // resolve the element inside it at the frame-relative coordinate. Cross-origin frames are
      // unreachable and fall back to the frame element itself.
      while (el && el.tagName === "IFRAME") {
        try {
          const doc = el.contentDocument;
          if (!doc) break;
          const rect = el.getBoundingClientRect();
          const inner = doc.elementFromPoint(x - rect.left, y - rect.top);
          if (!inner || inner === el) break;
          el = inner;
        } catch (e) { break; }
      }
      if (!el) return { error: "No element at coordinate [" + x + ", " + y + "]." };
      const opts = { bubbles: true, cancelable: true, composed: true, clientX: x, clientY: y, dataTransfer: dt };
      el.dispatchEvent(new DragEvent("dragenter", opts));
      const dragOverAccepted = !el.dispatchEvent(new DragEvent("dragover", opts));
      const dropHandled = !el.dispatchEvent(new DragEvent("drop", opts));
      const handled = dragOverAccepted || dropHandled;
      imageDropFx(el, x, y);
      return {
        success: true,
        accepted: handled,
        output: handled
          ? "Page signaled handling for screenshot drag/drop (" + name + ") at [" + x + ", " + y + "]."
          : "Dispatched screenshot drag/drop (" + name + ") at [" + x + ", " + y
            + "]; the page did not signal handling.",
      };
    }

    return { error: "Either ref or coordinate is required." };
  }

  function refCoordinates(ref) {
    const el = deref(ref);
    if (!el) {
      const stale = staleRefMessage(ref);
      return stale ? { error: stale } : null;
    }
    const rect = el.getBoundingClientRect();
    return { x: Math.round(rect.x + rect.width / 2), y: Math.round(rect.y + rect.height / 2) };
  }

  // --- wait_for (ADR-0037): poll a condition at 250ms while a MutationObserver counter, binned
  // into 500ms windows, feeds the adaptive settle detector (lib/settle.js). Resolves when the
  // condition (if any) holds AND the settle gate (unless settle:false) has passed AND the minimum
  // elapsed time has run; times out after timeout_ms. This is the content script's first
  // long-running handler, so it owns sendResponse itself and returns true to hold the channel open. ---
  function evaluateCondition(spec) {
    // Returns the matched element's ref when the condition is satisfied, else null. A bare settle
    // wait (no selector/text) reports a found:null condition as satisfied -- the settle gate alone
    // decides -- and never mints a ref.
    if (spec.state === "settled") return { ok: true, ref: null };
    const wantPresent = spec.state === "visible" || spec.state === "present";
    const wantGone = spec.state === "gone";
    let matched = null;
    if (spec.selector) {
      // CSS selector path: query the whole document (shadow roots are opaque to querySelector, so a
      // selector miss is a real miss -- the text path below walks shadow roots and is the fallback).
      try {
        const el = document.querySelector(spec.selector);
        matched = el || null;
      } catch {
        matched = null; // invalid selector string -- treated as no match this poll.
      }
    } else if (spec.text) {
      const q = spec.text.toLowerCase();
      for (const el of collectAll(document)) {
        if (!visible(el)) continue;
        const tag = el.tagName.toLowerCase();
        if (["script", "style", "noscript", "template"].includes(tag)) continue;
        const hay = `${accessibleName(el) || ""} ${(el.textContent || "").slice(0, 200)}`.toLowerCase();
        if (hay.includes(q)) { matched = el; break; }
      }
    }
    if (matched && wantPresent) {
      const present = spec.state === "present" || visible(matched);
      return present ? { ok: true, ref: refFor(matched) } : { ok: false, ref: null };
    }
    if (!matched && wantGone) return { ok: true, ref: null };
    return { ok: false, ref: null };
  }

  function runWaitFor(spec) {
    return new Promise((resolve) => {
      const startedAt = Date.now();
      const deadline = startedAt + spec.timeout_ms;
      let lastRead = readMutations(); // baseline mutation count at wait start
      let windowStart = startedAt;
      let windowCount = 0;
      let lastSettled = false; // the return of the detector's most recent push().
      const detector = (self.GhostlightSettle || GhostlightSettle).createSettleDetector();
      const poll = () => {
        const now = Date.now();
        const elapsed = now - startedAt;
        // Fold this poll's mutations into the open 500ms window; close the window when it elapses.
        const cur = readMutations();
        windowCount += cur - lastRead;
        lastRead = cur;
        if (now - windowStart >= 500) {
          lastSettled = detector.push(windowCount);
          windowStart = now;
          windowCount = 0;
        }
        const cond = evaluateCondition(spec);
        const settled = lastSettled;
        const minMet = elapsed >= spec.min_ms;
        if (cond.ok && (!spec.settle || settled) && minMet) {
          resolve({
            found: cond.ok,
            settled: spec.settle ? settled : undefined,
            elapsedMs: elapsed,
            ref: cond.ref,
            peakMutations: spec.settle ? detector.peak : undefined,
            finalRate: spec.settle ? detector.lastRate : undefined,
          });
          return;
        }
        if (now >= deadline) {
          resolve({
            timeout: true,
            rate: detector.lastRate,
            title: document.title || "",
            excerpt: cond.ref || "",
          });
          return;
        }
        setTimeout(poll, 250);
      };
      setTimeout(poll, 250);
    });
  }

  // --- Consequence digest sampler (ADR-0037 D2, PINS.md SS10): the SW calls observeSnap before a
  // mutating action and observeSample after. observeSample waits the 300ms settle window, then
  // diffs url/title/focus and counts mutations since the snap; alert/status elements whose text
  // was NOT present at snap time are "newly appeared" (first 200 chars of textContent); a
  // role=dialog present at sample but not at snap is reported. lib/receipt.js bounds and renders
  // the facts as correlation-only evidence; it never claims the action caused the observation. ---
  function roleTexts(roles) {
    const out = {};
    for (const el of collectAll(document)) {
      const r = el.getAttribute && el.getAttribute("role");
      if (r && roles.includes(r) && visible(el)) {
        const t = (el.textContent || "").trim().slice(0, 200);
        if (t) {
          (out[r] = out[r] || []).push(t);
        }
      }
    }
    return out;
  }

  function focusedName() {
    const ae = document.activeElement;
    if (!ae || ae.tagName && ae.tagName.toLowerCase() === "body") return "";
    return (accessibleName(ae) || "").trim();
  }

  function observeSnap() {
    const rt = roleTexts(["alert", "status", "dialog"]);
    return {
      url: location.href,
      title: document.title || "",
      focus: focusedName(),
      mutations: readMutations(),
      renderSerial,
      alerts: rt.alert || [],
      statuses: rt.status || [],
      dialogPresent: !!(rt.dialog && rt.dialog.length),
    };
  }

  function observeSample(before, meta) {
    return new Promise((resolve) => {
      setTimeout(() => {
        const url = location.href;
        const title = document.title || "";
        const focus = focusedName();
        const mutations = readMutations() - (before.mutations || 0);
        const rt = roleTexts(["alert", "status", "dialog"]);
        const newAlert = (rt.alert || []).find((t) => !(before.alerts || []).includes(t)) || "";
        const newStatus = (rt.status || []).find((t) => !(before.statuses || []).includes(t)) || "";
        const dialogNow = !!(rt.dialog && rt.dialog.length);
        const dialogAppeared = dialogNow && !before.dialogPresent;
        const changedElements = [];
        if (focus && focus !== before.focus && document.activeElement) {
          const summary = elementSummary(document.activeElement);
          if (summary) changedElements.push(summary);
        }
        const receiptLib = self.GhostlightReceipt || GhostlightReceipt;
        const receipt = receiptLib.makeReceipt({
          tabId: meta && meta.tabId,
          action: meta && meta.action,
          targetAssurance: meta && meta.targetAssurance,
          target: meta && meta.target,
          before,
          after: {
            url,
            title,
            mutations,
            renderSerial,
            alert: newAlert,
            status: newStatus,
            dialogOpened: dialogAppeared,
            changedElements,
          },
        });
        resolve({ digest: receiptLib.renderReceipt(receipt), receipt });
      }, 300);
    });
  }

  // --- formStructure (ADR-0036 D5, PINS.md SS12): the value-free form identity read form_fill
  // matches against. Returns the controls grouped by their containing <form> (document order,
  // formIndex from 0) plus the formless controls, each control carrying only identity fields
  // (label, placeholder, name, id, aria-label, type, disabled, readonly) -- NO field values read.
  // Submit candidates are buttons whose accessible name exactly matches the pinned list, plus native
  // submit inputs/buttons. Visibility-filtered; refs via the existing refFor. ---
  const SUBMIT_LABELS = ["submit", "sign in", "log in", "save"];

  // The label for matching: label[for] text, else a wrapping <label>'s text, else null. Deliberately
  // NOT accessibleName (which collapses aria-label/placeholder/title into one string); form_fill
  // matches against the label association the page itself declares, separate from the other fields.
  function controlLabel(el) {
    if (el.id) {
      const label = document.querySelector(`label[for="${CSS.escape(el.id)}"]`);
      if (label && label.textContent.trim()) return label.textContent.trim();
    }
    const wrapping = el.closest && el.closest("label");
    if (wrapping && wrapping.textContent.trim()) return wrapping.textContent.trim();
    return null;
  }

  function controlType(el) {
    const tag = el.tagName.toLowerCase();
    if (tag === "textarea") return "textarea";
    if (tag === "select") return "select";
    if (tag === "input") return (el.type && el.type !== "hidden") ? el.type : "text";
    return "text";
  }

  function isFormControl(el) {
    const tag = el.tagName.toLowerCase();
    return (tag === "input" && (el.type || "text") !== "hidden") || tag === "select" || tag === "textarea";
  }

  function readControl(el) {
    return {
      ref: refFor(el),
      type: controlType(el),
      label: controlLabel(el),
      placeholder: typeof el.placeholder === "string" && el.placeholder ? el.placeholder : null,
      name: el.name || null,
      id: el.id || null,
      ariaLabel: el.getAttribute("aria-label") || null,
      disabled: !!el.disabled,
      readonly: !!el.readOnly,
    };
  }

  function submitKind(el) {
    const tag = el.tagName.toLowerCase();
    const type = (el.type || "").toLowerCase();
    if (tag === "input" && type === "submit") return "input-submit";
    if (tag === "button" && type === "submit") return "button-submit";
    if (tag === "button") {
      const name = (accessibleName(el) || "").toLowerCase().trim();
      if (SUBMIT_LABELS.includes(name)) return "labeled-button";
    }
    return null;
  }

  function formStructure() {
    const forms = [];
    const formElements = collectAll(document).filter((el) => el.tagName.toLowerCase() === "form");
    const seen = new WeakSet();
    // Build each form's controls and submit candidates in document order.
    for (let fi = 0; fi < formElements.length; fi++) {
      const form = formElements[fi];
      const controls = [];
      const submits = [];
      for (const el of collectAll(form)) {
        if (seen.has(el) || !visible(el)) continue;
        if (isFormControl(el)) {
          seen.add(el);
          controls.push(readControl(el));
        }
        const kind = submitKind(el);
        if (kind) {
          seen.add(el);
          submits.push({ ref: refFor(el), label: accessibleName(el) || null, kind });
        }
      }
      forms.push({ formIndex: fi, controls, submits });
    }
    // Formless controls (not inside any <form>).
    const formless = [];
    for (const el of collectAll(document)) {
      if (seen.has(el) || !visible(el)) continue;
      if (isFormControl(el)) {
        seen.add(el);
        formless.push(readControl(el));
      }
    }
    return { forms, formless };
  }

  // --- Message handler ---
  chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
    switch (msg.type) {
      case "accessibilityTree": sendResponse({ result: accessibilityTree(msg.options) }); return true;
      case "elementSummary": {
        const el = deref(msg.ref);
        if (!el) {
          sendResponse({ result: { error: staleRefMessage(msg.ref) || `Element ${msg.ref} not found or was garbage-collected.` } });
        } else {
          sendResponse({ result: elementSummary(el) });
        }
        return true;
      }
      case "pageText": sendResponse({ result: pageText(msg.max_chars) }); return true;
      case "find": sendResponse({ result: find(msg.query, msg.present === true) }); return true;
      case "resolveActionable": sendResponse({ result: resolveActionable(msg.target || {}) }); return true;
      case "pageMeta": sendResponse({ result: currentPageMeta() }); return true;
      case "setFormValue": sendResponse({ result: setFormValue(msg.ref, msg.value) }); return true;
      case "setFiles": sendResponse({ result: setFiles(msg.ref, msg.files) }); return true;
      case "setImage": sendResponse({ result: setImage(msg.ref, msg.coordinate, msg.data, msg.filename, msg.mimeType) }); return true;
      case "refCoordinates": sendResponse({ result: refCoordinates(msg.ref) }); return true;
      case "scrollToRef": {
        const el = deref(msg.ref);
        if (!el) {
          const stale = staleRefMessage(msg.ref);
          sendResponse({ result: stale ? { error: stale } : false });
          return true;
        }
        el.scrollIntoView({ block: "center", behavior: "instant" });
        scrollTargetFx(el);
        sendResponse({ result: true });
        return true;
      }
      case "waitFor": {
        runWaitFor(msg.spec).then((result) => sendResponse({ result }));
        return true;
      }
      case "observeSnap": sendResponse({ result: observeSnap() }); return true;
      case "observeSample": {
        observeSample(msg.before || {}, msg.meta || {}).then((result) => sendResponse({ result }));
        return true;
      }
      case KEY_CUE_OBSERVATION_MESSAGES.BEGIN:
        sendResponse({ result: beginKeyCueObservation(msg.expectedCount) }); return true;
      case KEY_CUE_OBSERVATION_MESSAGES.FINISH:
        sendResponse({ result: finishKeyCueObservation(msg.token) }); return true;
      case DRAG_OBSERVATION_MESSAGES.BEGIN:
        sendResponse({ result: beginDragObservation() }); return true;
      case DRAG_OBSERVATION_MESSAGES.FINISH:
        finishDragObservation(msg.token).then((result) => sendResponse({ result })); return true;
      case "formStructure": sendResponse({ result: formStructure() }); return true;
      default:
        return false; // not ours -- let the visual-indicator content script handle it
    }
  });
})();
