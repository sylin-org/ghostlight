// Ghostlight workbench -- the Policy view: the one page that answers "what may agents do here, and
// who decided", and the editor for the person's own rules.
//
// Everything it draws arrives compiled from the orchestrator. It chooses shapes and order; it never
// decides what is allowed, never composes a policy sentence, and never computes a decision. The
// draft it holds is disposable view state until the person applies it. The view hands it the
// element table, what this screen may do (`can`) and the helpers every view shares, and calls what
// it gives back.
(function installGhostlightPolicyView(root, factory) {
  const api = factory();
  root.GhostlightPolicyView = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightPolicyViewApi() {
  "use strict";

  const {
    CAPABILITY_BADGE, CAPABILITY_ORDER, CAPABILITY_TONE, CHEVRON, NO_RULES, NO_RULES_LEFT,
    OPEN_RULE, OPEN_RULE_NOTE, POLICY_IN_WINDOW, SACRED_KEY, SETTING_GROUPS, addressOf, escapeHtml,
    hostReadback, neverTouchedCount, patternCovers, settingLimits, settingWords, words
  } = globalThis.GhostlightWords;
  const { ACT } = globalThis.GhostlightTransport;

  /**
   * The Policy view on the elements in `el`. `drawnOnce` is the view's set of destinations drawn at
   * least once. Registers the editor's listeners on creation.
   */
  function create({ el, can, continueMarkup, drawnOnce }) {
    /*
     * The one page that answers "what may agents do here, and who decided".
     *
     * Everything drawn below arrives compiled from the orchestrator. This function chooses shapes
     * and order; it never decides what is allowed, never composes a policy sentence, and never
     * computes a decision. The draft the editor holds is disposable view state and stays that way
     * until the person applies it.
     */
    let draft = null;
    let applied = null;

    /**
     * Draw the compiled policy.
     *
     * The draft is replaced by the applied rules unless the caller says to keep it, which a screen
     * does when it learns that the applied rules moved on while the person's edits were unsent.
     */
    function policy(view, { keepDraft = false } = {}) {
      applied = view;
      drawnOnce.add("policy");
      el["policy-headline"].textContent = view.headline;

      organizationCard(view.organization, view.passport);

      el["capability-board"].innerHTML = view.capabilities.map((line) => {
        // The badge states polarity, because "some sites" is true of both an open baseline with
        // holes cut in it and a closed one with holes opened, and they are opposite situations.
        const tone = CAPABILITY_TONE[line.state] ?? "no";
        const badge = CAPABILITY_BADGE[line.state] ?? "Not allowed";
        return `<article class="cap cap-${tone}">`
          + `<div class="cap-top"><h3>${escapeHtml(line.label)}</h3>`
          + `<span class="cap-state">${badge}</span></div>`
          + `<p class="cap-covers">${escapeHtml(line.covers)}</p>`
          + `<p class="cap-detail">${escapeHtml(line.detail)}</p></article>`;
      }).join("");

      el["policy-ceilings"].innerHTML = view.ceilings
        .map((line) => `<li>${escapeHtml(line)}</li>`).join("");

      settingsAndDocuments(view.layers);

      const user = view.user_layer;
      el["policy-editor"].hidden = !user.editable;
      el["add-rule"].hidden = !(user.editable && can(ACT.applyPolicy));
      el["policy-blocked"].hidden = user.editable || !user.blocked_reason;
      if (user.blocked_reason) el["policy-blocked-reason"].textContent = user.blocked_reason;
      el["policy-remove"].hidden = !(user.editable && user.source === "workbench" && hasUserLayer(view)
        && can(ACT.removePolicy));

      if (!user.editable) {
        draft = null;
        opened.clear();
      } else if (!(keepDraft && draft)) {
        draft = draftFrom(view);
        opened.clear();
      }
      renderRules();
    }

    function hasUserLayer(view) {
      return view.layers.some((layer) => layer.kind === "user");
    }

    /**
     * The organization whose policy is in force, in its own words, and where its signed policy
     * came from. The service names it; a passport only says how fresh that policy is.
     */
    function organizationCard(organization, passport) {
      const card = el["policy-organization"];
      const identity = organization?.identity;
      if (!identity && !passport?.configured) {
        card.hidden = true;
        card.innerHTML = "";
        return;
      }
      const contacts = (identity?.contacts ?? [])
        .map((contact) => `<li><span>${escapeHtml(contact.label || words(contact.kind))}</span>${escapeHtml(contact.value)}</li>`)
        .join("");
      const provenance = [];
      if (passport?.configured) {
        provenance.push(words(passport.freshness));
        if (passport.sequence != null) provenance.push(`verified sequence ${passport.sequence}`);
        if (passport.source_class && passport.source_class !== "none") {
          provenance.push(passport.source_class === "https" ? "from an HTTPS source" : `from a ${words(passport.source_class)} source`);
        }
        if (passport.last_success_ms) {
          provenance.push(`last checked ${new Date(passport.last_success_ms).toLocaleString()}`);
        }
      }
      card.hidden = false;
      card.innerHTML = `<h2>${escapeHtml(organization?.title ?? "")}</h2>`
        + (identity?.statement ? `<p class="org-statement">${escapeHtml(identity.statement)}</p>` : "")
        + (identity?.url ? `<p class="org-url">${escapeHtml(identity.url)}</p>` : "")
        + (contacts ? `<ul class="org-contacts">${contacts}</ul>` : "")
        + (provenance.length ? `<p class="org-provenance">${escapeHtml(provenance.join(", "))}.</p>` : "");
    }

    /**
     * The settings a layer authors, and the exact document behind every layer.
     *
     * Settings only appear when a layer actually sets one, the way a policy page that lists every
     * possible setting teaches nothing. The documents stay reachable so this page never becomes the
     * only way to read the policy.
     */
    function settingsAndDocuments(layers) {
      const settings = layers.flatMap((layer) =>
        layer.settings.map((setting) => ({ ...setting, owner: layer.title, kind: layer.kind })));
      el["policy-settings"].innerHTML = settings.length
        ? `<h2 class="subhead">Restrictions in force</h2><ul class="layer-settings">`
          + settings.map((setting) => {
            let value = setting.value;
            try {
              value = JSON.parse(setting.value);
            } catch (error) {
              // A value the orchestrator could not render as JSON is shown as it arrived.
            }
            // A screen that may not name the never-touch sites is told how many there are.
            const counted = setting.key === SACRED_KEY && typeof setting.count === "number";
            return `<li>${escapeHtml(counted ? neverTouchedCount(setting.count) : settingWords(setting.key, value))}`
              + ` <span class="owner">${escapeHtml(setting.owner)}</span></li>`;
          }).join("")
          + `</ul>`
          + limitNotes(settingLimits(settings.filter((setting) => setting.value === "false").map((setting) => setting.key)))
        : "";

      el["policy-documents"].innerHTML = layers.map((layer) => {
        if (!layer.document) return "";
        const where = layer.path ? `<span class="doc-path">${escapeHtml(layer.path)}</span>` : "";
        return `<details class="layer-doc"><summary class="disclosure">${escapeHtml(layer.title)}: `
          + `${escapeHtml(layer.policy_name)} ${escapeHtml(layer.version)}${CHEVRON}</summary>`
          + where + `<pre>${escapeHtml(layer.document)}</pre></details>`;
      }).join("");
    }

    /* ------------------------------ the editor ----------------------------- */

    /**
     * Turn the applied user layer into an editable draft, or start an empty one.
     *
     * Only a draft with no user layer behind it is `open`: while it has no rules, its document
     * carries OPEN_RULE, so a setting turned off applies alone and agents keep the ordinary
     * websites they have now. An applied layer is never open, even with no rules: a policy with no
     * grants refuses everything today, and its draft says so instead of widening it.
     */
    function draftFrom(view) {
      const layer = view.layers.find((entry) => entry.kind === "user");
      if (!layer) return { rules: [], settings: emptySettings(), dirty: false, open: true };
      return {
        dirty: false,
        open: false,
        rules: layer.rules.map((rule) => ({
          id: rule.id,
          hosts: rule.allow.join(", "),
          description: rule.description || "",
          allowed: new Set(rule.allowed)
        })),
        settings: settingsFrom(layer.settings)
      };
    }

    /** No opinion on anything, which is what an absent setting means. */
    function emptySettings() {
      return { restricted: new Set(), sacred: "", sacredCount: null, startup: null, choices: {} };
    }

    /**
     * Read authored settings back into the draft.
     *
     * A boolean restriction is present in the document or it is not; the permissive value never
     * appears. The browser-startup setting is different by design: it is a closed operational
     * choice, so its authored string is read back directly.
     */
    function settingsFrom(authored) {
      const settings = emptySettings();
      for (const setting of authored) {
        if (setting.key === SACRED_KEY) {
          settings.sacred = parseHostList(setting.value).join(", ");
          if (typeof setting.count === "number") settings.sacredCount = setting.count;
        } else if (setting.key === "browser.startup") {
          try {
            const value = JSON.parse(setting.value);
            if (value === "on_demand" || value === "manual") settings.startup = value;
          } catch (error) {
            // The orchestrator validates the document. A malformed projected value stays absent.
          }
        } else if (SETTING_GROUPS.some((group) => group.items.some((item) => item.key === setting.key && item.kind === "choice"))) {
          try { settings.choices[setting.key] = JSON.parse(setting.value); } catch (_) { /* invalid authored choice remains absent */ }
        } else if (setting.value === "false") {
          settings.restricted.add(setting.key);
        }
      }
      return settings;
    }

    /** The compiled view renders values as JSON text, which is what the orchestrator sent. */
    function parseHostList(value) {
      try {
        const parsed = JSON.parse(value);
        return Array.isArray(parsed) ? parsed.map(String) : [];
      } catch (error) {
        return [];
      }
    }

    /**
     * The permission grid, grouped by what a person actually thinks about.
     *
     * Every boolean switch here defaults on, because absence is what "allowed" means. A switch
     * reads as the permission, not the internal flag --
     * "MCP clients" with a checked box, not "turn off MCP clients" with an unchecked one. Unchecking
     * one is the only thing this editor can do to it; the schema underneath is still a restriction
     * that is only ever authored as `false` (ADR-0122 A3), but a person should never have to hold
     * that inversion in their head to use the page. The startup choice branches to a closed select.
     */
    function renderSettings() {
      if (!draft) return;
      el["setting-groups"].innerHTML = SETTING_GROUPS.map((group, at) => {
        // What a group cannot promise leads it, under its heading, and each switch it covers is
        // described by it too.
        const limits = settingLimits(group.items.map((item) => item.key));
        const limitIds = new Map(limits.map((limit, index) => [limit, `setting-group-${at}-limit-${index}`]));
        return `<div class="setting-group"><h3>${escapeHtml(group.title)}</h3>`
          + limitNotes(limits, limitIds)
          + group.items.map((item) => settingRow(item, limitIds)).join("")
          + `</div>`;
      }).join("");
      el["sacred-hosts"].value = draft.settings.sacred;
      refreshSacred();
    }

    /**
     * Say each limit once. With `ids`, a map from a limit to the id it wears, the switches it
     * covers can point at it.
     */
    function limitNotes(limits, ids = new Map()) {
      return limits.map((limit) => {
        const id = ids.get(limit);
        return `<p class="setting-limit"${id ? ` id="${escapeHtml(id)}"` : ""}>${escapeHtml(limit)}</p>`;
      }).join("");
    }

    /** The id a setting's row parts are named by, made from its registered key. */
    function settingId(key) {
      return `setting-${String(key).replace(/[^A-Za-z0-9]+/g, "-")}`;
    }

    /**
     * One switch: the permission as the person sees it, named by its row's name and described by
     * its detail, the browser setting beside it and the limit its group states.
     */
    function settingRow(item, limitIds) {
      if (item.kind === "choice") return choiceRow(item);
      const forcedBy = organizationForces(item.key);
      const checked = !forcedBy && !draft.settings.restricted.has(item.key);
      const detail = forcedBy
        ? `${escapeHtml(forcedBy)} already turned this off.`
        : escapeHtml(checked ? item.on : item.off);
      const id = settingId(item.key);
      // A setting the browser keeps is read here and changed there, so it is a line, not a control.
      const also = item.browser
        ? `<span class="setting-also" id="${id}-also">${escapeHtml(item.browser.word)}`
          + `<span class="chip context-chip">${escapeHtml(item.browser.where)}</span></span>`
        : "";
      const described = [`${id}-detail`, item.browser ? `${id}-also` : null, limitIds.get(item.limit) ?? null]
        .filter(Boolean).join(" ");
      return `<div class="setting-row${forcedBy ? " setting-forced" : ""}">`
        + `<label class="toggle">`
        + `<input type="checkbox" data-restriction="${escapeHtml(item.key)}"`
        + `${checked ? " checked" : ""}${forcedBy || !can(ACT.applyPolicy) ? " disabled" : ""}`
        + ` role="switch" aria-labelledby="${id}-name" aria-describedby="${described}">`
        + `<span class="toggle-track"><span class="toggle-thumb"></span></span></label>`
        + `<div class="setting-body"><span class="setting-name" id="${id}-name">${escapeHtml(item.name)}</span>`
        + `<span class="setting-detail" id="${id}-detail">${detail}</span>${also}</div></div>`;
    }

    /** Render the first closed string setting without turning it into a free-form field. */
    function choiceRow(item) {
      if (item.key !== "browser.startup") return authoredChoiceRow(item);
      const ceiling = applied?.browser_startup?.organization_ceiling;
      const forcedBy = ceiling === "manual" ? applied?.organization?.title : null;
      const selected = forcedBy
        ? "manual"
        : draft.settings.startup ?? applied?.browser_startup?.value ?? "manual";
      const choice = item.choices.find((entry) => entry.value === selected) ?? item.choices[0];
      const options = item.choices.map((entry) =>
        `<option value="${escapeHtml(entry.value)}"${entry.value === selected ? " selected" : ""}>`
        + `${escapeHtml(entry.label)}</option>`).join("");
      const detail = forcedBy
        ? `${escapeHtml(forcedBy)} requires browser auto-open to stay off.`
        : escapeHtml(choice.detail);
      return `<div class="setting-row${forcedBy ? " setting-forced" : ""}">`
        + `<div class="setting-body"><label class="setting-name" for="setting-browser-startup">`
        + `${escapeHtml(item.name)}</label>`
        + `<select class="setting-choice" id="setting-browser-startup" data-setting-choice="${escapeHtml(item.key)}"`
        + `${forcedBy || !can(ACT.applyPolicy) ? " disabled" : ""}>${options}</select>`
        + `<span class="setting-detail">${detail}</span></div></div>`;
    }

    // Closed choices share the effective policy's authored source and organization floor.
    function authoredChoiceRow(item) {
      const authority = applied?.[item.authority];
      const ceiling = authority?.[`organization_${item.field}`];
      const minimum = Math.max(0, item.choices.findIndex((choice) => choice.value === ceiling));
      const desired = draft.settings.choices[item.key] ?? authority?.[item.field] ?? item.default;
      const selected = item.choices[Math.max(minimum, item.choices.findIndex((choice) => choice.value === desired))];
      const options = item.choices.map((choice, index) => `<option value="${escapeHtml(choice.value)}"${choice === selected ? " selected" : ""}${index < minimum ? " disabled" : ""}>${escapeHtml(choice.label)}</option>`).join("");
      const source = authority?.[item.source ?? `${item.field}_source`];
      const author = source === "organization" ? applied?.organization?.title : source === "user" ? "Your rules" : "Ghostlight default";
      return `<div class="setting-row"><div class="setting-body"><label class="setting-name">${escapeHtml(item.name)}`
        + `<select class="setting-choice" data-setting-choice="${escapeHtml(item.key)}"`
        + `${can(ACT.applyPolicy) ? "" : " disabled"}>${options}</select></label>`
        + `<span class="setting-detail">${escapeHtml(selected.detail)} ${escapeHtml(author)}.</span></div></div>`;
    }

    function organizationForces(key) {
      const organization = applied?.layers?.find((layer) => layer.kind === "organization");
      const setting = organization?.settings.find((entry) => entry.key === key);
      if (setting?.value !== "false") return null;
      return applied?.organization?.title;
    }

    function refreshSacred() {
      const patterns = splitHosts(draft?.settings.sacred ?? "");
      const readback = patterns.map((pattern) => hostReadback(pattern)).join("; ");
      // A screen that may not name the sites shows how many there are, and leaves them as they are.
      const counted = draft?.settings.sacredCount;
      if (typeof counted === "number" && !can(ACT.applyPolicy)) {
        el["sacred-readback"].textContent = neverTouchedCount(counted);
        el["sacred-readback"].classList.toggle("readback-empty", false);
        return;
      }
      el["sacred-readback"].textContent = readback
        ? `Never touched: ${readback}.`
        : "No sites beyond the ones Ghostlight always refuses.";
      el["sacred-readback"].classList.toggle("readback-empty", !readback);
    }

    /**
     * Switch one permission on or off, as the person sees it.
     *
     * `allowed` is the checkbox state, which is the opposite of what the schema stores: allowing
     * something removes it from the restricted set (no opinion authored), and disallowing it adds
     * the one value a user layer may ever write for that key. This is the seam where the delightful
     * vocabulary in words.js turns back into the tightening-only schema in ADR-0122 A3.
     */
    function setPermission(key, allowed) {
      if (!draft) return;
      if (allowed) draft.settings.restricted.delete(key);
      else draft.settings.restricted.add(key);
      draft.dirty = true;
      renderSettings();
      editorReady();
      el["discard-policy"].hidden = false;
    }

    /** Select one value from a closed string setting. */
    function setChoice(key, value) {
      const item = SETTING_GROUPS.flatMap((group) => group.items).find((item) => item.key === key && item.kind === "choice");
      if (!draft || !item?.choices.some((choice) => choice.value === value)) return;
      if (key === "browser.startup") draft.settings.startup = value;
      else draft.settings.choices[key] = value;
      draft.dirty = true;
      renderSettings();
      editorReady();
      el["discard-policy"].hidden = false;
    }

    function setSacred(value) {
      if (!draft) return;
      draft.settings.sacred = value;
      draft.dirty = true;
      refreshSacred();
      editorReady();
      el["discard-policy"].hidden = false;
    }

    /**
     * Which capabilities an organization has refused everywhere.
     *
     * Only an organization ceiling may disable a control. A capability that merely happens to be
     * missing from this person's own rules is exactly what they are here to change, and greying it
     * out would tell them they cannot grant themselves something they plainly can.
     */
    function ceilingFor(capability) {
      const line = applied?.capabilities?.find((entry) => entry.capability === capability);
      if (!line || line.state !== "unavailable") return null;
      if (!line.decided_by?.includes("organization")) return null;
      return `${applied?.organization?.title} does not allow this`;
    }

    /**
     * Every rule in force, in the order authority actually considers them.
     *
     * One list, not two. The organization's rules come first because they are checked first and
     * cannot be edited here, and this person's own follow. Each is one line: what it covers, and
     * whose it is. Opening a line is what reveals detail, so the common case -- reading what
     * applies -- stays a single glance instead of a page of repeated headings.
     */
    function renderRules() {
      const organization = (applied?.layers ?? []).filter((layer) => layer.kind === "organization");
      const rows = organization.flatMap((layer) =>
        layer.rules.map((rule) => organizationRow(rule, layer)));
      const mine = draft ? draft.rules.map((rule, index) => userRow(rule, index)) : [];
      // How rules are checked matters once there is one; before that the headline says it all.
      el["rules-note"].hidden = !(rows.length || mine.length);
      el["rule-list"].innerHTML = rows.concat(mine).join("")
        || `<p class="rules-empty">${escapeHtml(NO_RULES)}</p>`;
      if (draft) {
        el["discard-policy"].hidden = !draft.dirty;
        renderSettings();
      }
      editorReady();
    }

    /** The sentence a rule reads as, which is the whole row when it is closed. */
    function sentenceFor(hosts, deny, capabilities) {
      const where = hosts.length ? hosts.map(escapeHtml).join(", ") : "no sites";
      const except = deny.length ? ` except ${deny.map(escapeHtml).join(", ")}` : "";
      const verbs = capabilities.length
        ? capabilities.map((capability) => escapeHtml(words(capability))).join(", ")
        : "nothing";
      return `On <b>${where}</b>${except}, agents may ${verbs}.`;
    }

    /** What an open rule shows when it cannot be edited here: its purpose, its sites and its name. */
    function ruleDetail(description, allow, deny, id) {
      return `<div class="rule-detail">`
        + (description ? `<p>${escapeHtml(description)}</p>` : "")
        + `<dl><dt>Sites</dt><dd>${allow.map(escapeHtml).join(", ") || "none"}</dd>`
        + (deny.length ? `<dt>Never</dt><dd>${deny.map(escapeHtml).join(", ")}</dd>` : "")
        + `<dt>Named</dt><dd>${escapeHtml(id)}</dd></dl></div>`;
    }

    /** A rule that is read here and not edited: the sentence it reads as, and whose it is. */
    function readOnlyRow(key, sentence, owner, detail) {
      const open = opened.has(key);
      return `<article class="rule rule-theirs${open ? " open" : ""}" data-rule-key="${escapeHtml(key)}">`
        + `<button class="rule-row" type="button" data-rule-toggle="${escapeHtml(key)}"`
        + ` aria-expanded="${open}">`
        + `<span class="rule-sentence">${sentence}</span>`
        + `<span class="rule-owner">${escapeHtml(owner)}</span></button>`
        + (open ? detail : "")
        + `</article>`;
    }

    function organizationRow(rule, layer) {
      return readOnlyRow(`org:${layer.title}:${rule.id}`, sentenceFor(rule.allow, rule.deny, rule.allowed),
        layer.title, ruleDetail(rule.description, rule.allow, rule.deny, rule.id));
    }

    function userRow(rule, index) {
      const key = `mine:${index}`;
      const open = opened.has(key);
      const capabilities = CAPABILITY_ORDER.filter((capability) => rule.allowed.has(capability));
      const sentence = sentenceFor(splitHosts(rule.hosts), [], capabilities);
      // A screen that cannot write the rules reads this person's own as it reads an organization's.
      if (!can(ACT.applyPolicy)) {
        return readOnlyRow(key, sentence, applied?.layers?.find((layer) => layer.kind === "user")?.title ?? "",
          ruleDetail(rule.description, splitHosts(rule.hosts), [], rule.id ?? ""));
      }
      const shadow = shadowedBy(index);
      return `<article class="rule rule-mine${open ? " open" : ""}${shadow ? " rule-inert" : ""}"`
        + ` data-rule-key="${escapeHtml(key)}">`
        + `<button class="rule-row" type="button" data-rule-toggle="${escapeHtml(key)}"`
        + ` aria-expanded="${open}">`
        + `<span class="rule-sentence">${sentence}</span>`
        + `<span class="rule-edit-hint">${open ? "Close" : "Edit"}</span></button>`
        + (open ? userEditor(rule, index, shadow) : "")
        + `</article>`;
    }

    function userEditor(rule, index, shadow) {
      const boxes = CAPABILITY_ORDER.map((capability) => {
        const blocked = ceilingFor(capability);
        const checked = rule.allowed.has(capability) && !blocked;
        return `<label class="cap-box${blocked ? " cap-box-blocked" : ""}">`
          + `<input type="checkbox" data-rule="${index}" data-capability="${capability}"`
          + `${checked ? " checked" : ""}${blocked ? " disabled" : ""}>`
          + `<span>${escapeHtml(words(capability))}</span>`
          + (blocked ? `<em>${escapeHtml(blocked)}</em>` : "")
          + `</label>`;
      }).join("");
      const readback = splitHosts(rule.hosts).map((pattern) => hostReadback(pattern)).join("; ");
      return `<div class="rule-detail">`
        + `<div class="rule-line"><span>On</span>`
        + `<input class="hosts" type="text" data-rule="${index}" data-field="hosts"`
        + ` value="${escapeHtml(rule.hosts)}" placeholder="example.com, *.example.com" spellcheck="false">`
        + `<span>agents may</span></div>`
        + `<div class="cap-boxes">${boxes}</div>`
        + (readback
          ? `<p class="readback">Matches ${escapeHtml(readback)}.</p>`
          : `<p class="readback readback-empty">Add a site for this rule to do anything.</p>`)
        + (shadow
          ? `<p class="rule-note">Rule ${shadow} above already covers this, so this one never applies. Move it up to use it.</p>`
          : "")
        + `<div class="rule-foot">`
        + `<input class="why" type="text" data-rule="${index}" data-field="description"`
        + ` value="${escapeHtml(rule.description)}" placeholder="What is this rule for? (optional)" maxlength="200">`
        + `<button class="link-button" type="button" data-rule="${index}" data-rule-action="up"${index === 0 ? " disabled" : ""}>Move up</button>`
        + `<button class="link-button" type="button" data-rule="${index}" data-rule-action="remove">Remove</button>`
        + `</div></div>`;
    }

    /** Which rows are open. Disposable view state, and the only thing this list remembers. */
    const opened = new Set();

    function toggleRule(key) {
      if (opened.has(key)) opened.delete(key);
      else opened.add(key);
      renderRules();
      if (opened.has(key)) {
        el["rule-list"].querySelector(`[data-rule-key="${key}"] .hosts`)?.focus();
      }
    }

    /**
     * What the editor will let you do with the draft as it stands.
     *
     * A draft whose rules were all removed is a policy that refuses everything, so neither applying
     * it nor asking what it would have done is a question worth answering. Saying that once,
     * plainly, beats letting someone press a button and read a number that sounds like an
     * accusation. An open draft with no rules carries OPEN_RULE, and says so once it has a change.
     */
    function editorReady() {
      if (!draft) return;
      const bare = !draft.rules.length;
      const empty = bare && !draft.open;
      el["apply-policy"].disabled = empty || !draft.dirty;
      el["check-policy"].disabled = empty;
      // Clearing matters as much as setting: a note about the rules must not outlive the rules it
      // was about and sit under good ones, contradicting them.
      const said = el["editor-status"].textContent;
      if (said === NO_RULES_LEFT || said === OPEN_RULE_NOTE) el["editor-status"].textContent = "";
      if (empty) {
        previewCleared();
        el["editor-status"].textContent = NO_RULES_LEFT;
      } else if (bare && draft.dirty) {
        el["editor-status"].textContent = OPEN_RULE_NOTE;
      }
    }

    /**
     * Which earlier rule, if any, makes this one unreachable.
     *
     * Grants are answered in written order, so a later rule fully covered by an earlier one can
     * never fire. Hiding that would turn a known ordering hazard into a silent one.
     */
    function shadowedBy(index) {
      const rule = draft.rules[index];
      const patterns = splitHosts(rule.hosts);
      if (!patterns.length) return null;
      for (let earlier = 0; earlier < index; earlier += 1) {
        const above = draft.rules[earlier];
        const covers = [...rule.allowed].every((capability) => above.allowed.has(capability));
        const hosts = splitHosts(above.hosts);
        if (covers && patterns.every((pattern) => hosts.some((broad) => patternCovers(broad, pattern)))) {
          return earlier + 1;
        }
      }
      return null;
    }

    function splitHosts(value) {
      return String(value || "").split(",").map((pattern) => pattern.trim()).filter(Boolean);
    }

    /** What a draft says: its rules and its settings, and nothing about when it was written. */
    function documentBody(source) {
      const open = { ...OPEN_RULE, allowed: new Set(CAPABILITY_ORDER) };
      const rules = source.rules.length || !source.open ? source.rules : [open];
      return {
        grants: rules.map((rule, index) => {
          const grant = {
            id: rule.id && /^[A-Za-z0-9._-]+$/.test(rule.id) ? rule.id : `rule-${index + 1}`,
            hosts: { allow: splitHosts(rule.hosts) },
            allowed: CAPABILITY_ORDER.filter((capability) => rule.allowed.has(capability))
          };
          if (rule.description.trim()) grant.description = rule.description.trim();
          return grant;
        }),
        config: settingEntries(source.settings)
      };
    }

    /**
     * Compose the document the orchestrator will validate. The surface authors no semantics.
     *
     * The version is today's date only when the rules or settings differ from the applied ones;
     * a draft that says what is already applied keeps the applied version.
     */
    function draftDocument() {
      if (!draft) return null;
      const body = documentBody(draft);
      const user = applied?.layers?.find((layer) => layer.kind === "user");
      const unchanged = Boolean(user) && JSON.stringify(body) === JSON.stringify(documentBody(draftFrom(applied)));
      return JSON.stringify({
        schema: 4,
        name: "Your rules",
        version: unchanged ? user.version : new Date().toISOString().slice(0, 10),
        grants: body.grants,
        config: body.config
      }, null, 2);
    }

    /**
     * The settings the draft authors, in their registered typed shapes.
     *
     * `level` is not a choice offered here. Both levels only tighten in 1.0, and nothing sits below
     * this layer for a recommendation to be relaxed by, so asking would be a word without a
     * consequence.
     */
    function settingEntries(settings) {
      const entries = [...settings.restricted]
        .map((key) => ({ key, value: false, level: "mandatory" }));
      if (settings.startup) {
        entries.push({ key: "browser.startup", value: settings.startup, level: "mandatory" });
      }
      for (const [key, value] of Object.entries(settings.choices)) entries.push({ key, value, level: "mandatory" });
      const sacred = splitHosts(settings.sacred);
      if (sacred.length) entries.push({ key: SACRED_KEY, value: sacred, level: "mandatory" });
      return entries;
    }

    function editRule(index, field, value) {
      if (!draft?.rules[index]) return;
      draft.rules[index][field] = value;
      draft.dirty = true;
      el["discard-policy"].hidden = false;
      editorReady();
      if (field === "hosts") refreshRuleHints(index);
    }

    /**
     * Update one rule's readback in place.
     *
     * Redrawing the whole list on every keystroke would take the caret with it, so the hints that
     * depend on what was just typed are the only thing that changes while a person is typing.
     */
    function refreshRuleHints(index) {
      const card = el["rule-list"].querySelector(`[data-rule-key="mine:${index}"]`);
      if (!card) return;
      const rule = draft.rules[index];
      const patterns = splitHosts(rule.hosts);
      const readback = card.querySelector(".readback");
      if (readback) {
        const text = patterns.map((pattern) => hostReadback(pattern)).join("; ");
        readback.textContent = text ? `Matches ${text}.` : "Add a site for this rule to do anything.";
        readback.classList.toggle("readback-empty", !text);
      }
      const shadow = shadowedBy(index);
      let note = card.querySelector(".rule-note");
      if (shadow && !note) {
        note = document.createElement("p");
        note.className = "rule-note";
        readback?.insertAdjacentElement("afterend", note);
      }
      if (note) {
        note.textContent = shadow
          ? `Rule ${shadow} above already covers this, so this one never applies. Move it up to use it.`
          : "";
        note.hidden = !shadow;
      }
    }

    function toggleCapability(index, capability, on) {
      if (!draft?.rules[index]) return;
      if (on) draft.rules[index].allowed.add(capability);
      else draft.rules[index].allowed.delete(capability);
      draft.dirty = true;
      renderRules();
    }

    function ruleAction(index, action) {
      if (!draft) return;
      if (action === "remove") {
        draft.rules.splice(index, 1);
        opened.delete(`mine:${index}`);
      }
      if (action === "up" && index > 0) {
        const [moved] = draft.rules.splice(index, 1);
        draft.rules.splice(index - 1, 0, moved);
        opened.delete(`mine:${index}`);
        opened.add(`mine:${index - 1}`);
      }
      draft.dirty = true;
      renderRules();
    }

    function addRule(seed) {
      if (!draft) return;
      draft.rules.push({
        id: `rule-${draft.rules.length + 1}`,
        hosts: seed || "",
        description: "",
        allowed: new Set(["read"])
      });
      draft.dirty = true;
      // A rule you just asked for opens on its own; anything else is a second click for nothing.
      opened.add(`mine:${draft.rules.length - 1}`);
      renderRules();
      const inputs = el["rule-list"].querySelectorAll(".hosts");
      inputs[inputs.length - 1]?.focus();
    }

    function discardDraft() {
      if (!applied) return;
      draft = draftFrom(applied);
      renderRules();
      previewCleared();
      el["editor-status"].textContent = "Back to the rules that are applied.";
    }

    function previewResult(preview) {
      const box = el["policy-preview"];
      box.hidden = false;
      const rows = preview.refused.map((entry) =>
        `<li><b>${escapeHtml(entry.tool)}</b>${entry.host ? ` on ${escapeHtml(entry.host)}` : ""}`
        + ` <span>${entry.count}x</span></li>`).join("");
      box.innerHTML = `<p class="preview-summary">${escapeHtml(preview.summary)}</p>`
        + (rows ? `<ul class="preview-list">${rows}</ul>` : "");
    }

    function previewCleared() {
      el["policy-preview"].hidden = true;
      el["policy-preview"].innerHTML = "";
    }

    function editorStatus(message) {
      el["editor-status"].textContent = message;
    }

    function draftIsDirty() {
      return Boolean(draft?.dirty);
    }

    /**
     * Draw Policy for what this screen may do with it. A screen that cannot write the rules shows
     * every draft control as the state it holds, read-only, under one line saying where changes
     * are made; it has no Apply, because it has nothing to apply.
     */
    function policyAccess() {
      const writes = can(ACT.applyPolicy);
      el["policy-continue"].hidden = writes;
      el["policy-continue"].innerHTML = writes ? "" : continueMarkup(POLICY_IN_WINDOW, addressOf("policy"));
      el["apply-policy"].hidden = !writes;
      el["sacred-hosts"].disabled = !writes;
      el["add-rule"].hidden = !(writes && applied?.user_layer?.editable);
      if (draft) renderRules();
    }

    return Object.freeze({
      policy, policyAccess, draftDocument, draftIsDirty, editRule, toggleCapability, ruleAction, addRule, toggleRule,
      setPermission, setChoice, setSacred, discardDraft, renderRules, previewResult, previewCleared, editorStatus
    });
  }

  return Object.freeze({ create });
});
