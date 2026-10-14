# ADR-0198: What agents see

Date: 2026-10-14. Status: Accepted by owner delegation (docs/MEMORY.md, "The agent decides
agent-facing experience", 2026-10-14). The owner delegated every decision about what agents see
to the agent, as the audience. Governance, truthfulness, privacy and the trust claims are
unchanged and stay the owner's. Each decision below is reversible on its own.

Amends ADR-0152 Decision 3 (controls carry their value and answer space in a separate key; option
text joins the find haystack), ADR-0110 Decision 4 (which results the name switch governs),
ADR-0158 Decision 5 (the model view shows coverage only when something limited it), ADR-0173
Decisions 1-3 (one change recorder, a geometry fast path, the presentation host filter),
ADR-0195 Decisions 1-2 (intent is carried apart from budget; form preparation shares the budget)
and ADR-0197 Decisions 17 and 21 (older MCP revisions get the result in the text block; actions
report what they changed). Builds on ADR-0164, which binds every decision here: references go
stale silently when the page changes, and Ghostlight governs the next actual request. Also amends
the field-guidance wording of ACCEPTANCE gate 2.

## Context

The test the owner set: an agent never needs an extra call to learn what a control accepts or
what its action changed, and a mistake costs one call. An agent used Ghostlight for a full session
on 2026-10-14 and recorded where that test failed.

- After a click or a fill, the agent usually spent one more call reading the page to learn what
  happened. The click result already said `dialog_opened`, and that was the one consequence it
  could trust without a read.
- `browser_inspect` on a sign-up page returned 22 items. Nine were the form's controls; the rest
  were navigation and footer links, in document order, with nothing to tell them apart.
- A select item carried no options, so a fill with a wrong option failed with "select option not
  found" and the next step "adjust the call or the page, then repeat". A stale handle said it had
  expired but not what replaced it.
- With a prompt open, three calls burned their deadlines before reaching the browser. The dialog
  lane has since made renderer-bound work refuse at once with `dialog_open`.
- Every result carried an eight-key coverage block that was almost always zeros, and every item
  carried `state:[]` and `credential_class:false`. In 74 calls, defaults that save no call were
  23% of the 67,984 characters the agent read. Document-tree labels dumped long container text.
  The catalog cost about 42,000 characters before any work.
- `text_present` on a page with an iframe took 4,250 ms for text that was already there, and the
  summary said it "appeared in 4 seconds".

A source review found the causes. `observation()` returns five keys (content.js:189-197). The
inspect, find and semantic predicates test `Array.isArray` against an object, so every sampled read
waits for its ceiling (content.js:902-911). `accessibleName` falls back to text that includes input
values (content.js:137-144, 318-331), which breaks ADR-0110 Decision 2 for some names. Click and
submit sleep a fixed 250 ms and report nothing they saw. Handles are minted fresh on every read and
never reused. Five designs and their reviews followed; this ADR records the result.

## Decision

### Rules that hold everywhere

1. **The test above is the acceptance rule.** Each change below is judged by the calls it saves
   or the mistake it makes cheap. New keys appear only where they carry information.
2. **Three kinds of page-derived text, three rules.**
   - *Names* (accessible names, dialog titles, form labels) are value-free (ADR-0110 Decision 2).
     They come from one page routine, `plainText`, that never emits a field value, a checked word
     or editable content. Results of read-class tools carry names under read authority, as today.
     Summaries of every tool, and the facts of every other tool (actions, refusals, recovery
     candidates), carry names only when `preserve_target_names` is on. This is the scope of
     ADR-0110 Decision 4.
   - *Page content* (option text, live-region text, error text, hints) follows read authorization
     on the document it came from, like `browser_read`.
   - *Values* (what a field holds) appear only in an item's `value` key and in the fill echo,
     bounded, through one page function, `outwardValue`. A credential-class control shows
     `"[hidden]"` when set and `""` when empty, and nothing else: no options, hint, length, error
     text or match.
3. **Audit and history stay content-minimized.** They never hold options, given or echoed values,
   page text, handles or contexts. Every new outcome gets an explicit structural audit projection.
4. **Page strings in sentences are bounded and defanged.** One line, a fixed character cap, quote
   replacement, and handle-shaped tokens replaced by `[handle]`.
5. **Wording is timed, never causal.** "Then the page showed ..." never claims the action caused
   it (ADR-0164 Decision 2). Ghostlight never acts on a candidate on its own.
6. **No extension change.** New information travels as new fields inside existing messages and as
   an optional `detail` object in the page error envelope. No new wire error code or world message
   kind is added.

### Controls say what they accept

7. **Controls carry their value and answer space** (owner decision 1). Inspect and find items keep
   their keys and gain, only when they apply: `value` (80 chars, `value_chars` when longer),
   `options` (25 entries of 60 chars, `options_total` when cut), `type`, `limits`, `hint`, `group`
   and `error`. State words gain `required`, `readonly` and `invalid`. Roles follow HTML-AAM.
   Hidden inputs leave the controls pool. Error text is a Ghostlight phrase built from the native
   validity flags; the page's `validationMessage` is used only for a custom error, because the
   browser's own messages can quote the value or its length.
8. **Matching is exact, then unique normalized, always echoed** (owner decision 2). Normalized
   means NFC, trimmed, whitespace collapsed and case-folded. Two candidates are refused with the
   list. There is no prefix, substring or fuzzy match. Disabled options are refused. A checkbox
   takes booleans or a closed word list and is set, never toggled. A radio may be chosen by a
   sibling's label.
9. **Multi-selects take arrays** in the fill schema now (owner decision 3).
10. **A refusal lists what would succeed, before any edit.** The page collects every field problem
    first and returns a closed problem vocabulary in the envelope `detail`. The summary says
    nothing changed and names each bad field; the next step names the exact handle and the route.
11. **A fill says what the page now holds** for choice and typed controls, and names a field the
    page changed back.
12. **Custom widgets fill by their own options** (phase 2, owner decision 4). Choice fill clicks
    only `role=option` inside the control's own popup, under write, and verifies the result.
    Anything that could navigate or submit stays with `browser_click` under action.

### Actions report what they changed

13. **Every click, key, fill, submit, hover, scroll and drag reports observed, net, visible
    changes,** with no parameter. The window starts at the action's first trusted event, so
    Ghostlight's own scrolling and pointer travel do not count. Reported: new live-region text,
    fields whose invalid state changed, dialogs that became visible, focus, enablement, controls
    that appeared (net by role and name), controls whose value changed, and the target's own state.
    The measured window is stated. Raw record counts are never reported.
14. **Text in a change report needs read on that document,** checked before dispatch and again
    with evidence at the report's URL. Without read, the report has counts and no handles.
15. **"Nothing changed" is narrow.** It is said only for click, key, submit and drag, only when
    every document reported, coverage is complete, nothing changed, and no navigation start was
    seen. Its next step warns before a repeat. When the report could not be taken, the result
    is silent about changes and keeps its status.
16. **No change report** for `browser_execute`, navigation, history or reload. A tab that moved
    says "The tab is now on ..." and quotes the title only when the page is ready and readable.
17. **Handles are stable.** The same tab, generation and locator always map to the same handle.
    Each tab keeps its current generation and the two before it, at most 2,000 records per
    workspace. A closed tab's handles and a removed element's handle become tombstones that
    refuse at once.

### Forms read as forms

18. **`browser_inspect` gains the scope `forms`.** It returns each form's fields in order with what
    each accepts, and its submit controls. Membership follows the form owner, including the
    `form` attribute. Fields outside any form join an implicit group marked `implicit`, and their
    likely buttons are marked `guess`. A guessed button is never a `submit_target`; the refusal
    names the `browser_click` call instead. Radio groups with a value-free label collapse into one
    entry whose options keep their own handles.
19. **Items carry `area` and `form` when they carry information.** Areas are a closed list:
    header, nav, aside, footer, dialog, search. Absence means page content.
20. **Tag, do not reorder.** When the item cap cuts, it cuts by tier: hidden first, then header,
    nav and footer, then aside, then page content. Output stays in document order. What was cut
    is counted in `left_out`. Items behind an open modal are left out and counted.

### Lean results and a lean catalog

21. **One lean model view at the MCP edge.** Fields at their default are omitted:
    `history_storage:"saved"`, `readiness:"not_applicable"`, `next_steps:[]`, an unlimited
    coverage block, item `state:[]` and `credential_class:false`, tree `label:""` and
    `children:[]`, and flow rows saved to history. `status`, `effect`, `repeat_safe`, `summary`,
    `invocation` and the tab always stay. Explicit facts such as `value:""`, `"[hidden]"` and
    `history_storage:"unconfirmed"` stay. Every MCP client sees this view, the command line
    included, because it is an MCP client of the same stream. The canonical envelope stays inside
    the service for flow references, audit and the workbench, so a reference written against the
    lean view always resolves.
22. **Coverage appears only when something limited it** (ADR-0158 Decision 5 for the model view;
    the durable audit is unchanged). A composition carries a limited child's coverage even when
    the child's result was cut from the parent, and the parent summary says so.
23. **Connector failure templates are truthful.** They say `history_storage:"unconfirmed"`.
24. **Clients on MCP revisions before 2025-06-18** get the lean JSON in the text block, because
    they cannot read `structuredContent` (ADR-0197 Decision 17).
25. **The document tree prunes in the page.** Unlabeled containers whose children were all pruned
    are dropped and do not count against the node limit. The diff sentence says "outline", which
    is what it compares.
26. **The catalog says only what name, type, enum and bounds do not.** Every bound and default
    stays. Governance phrases stay and are pinned: "for this request", "admitted before the
    effect", "may cause an external effect", "needs write even for unsent drafts". The coordinate
    fact moves into each `view` description. A size budget ratchets down and only rises by the
    measured size of a clause that a new behavior needs. Omitting spec-default tool annotations
    waits for a live check of client behavior.

### Stale handles and blocked states

27. **A stale handle says why, in one of four words:** `removed`, `navigated`, `tab_closed` or
    `unknown`. It never says "replaced".
28. **A refusal names the replacement only when it can be trusted.** The step "Repeat <tool> with
    <field> <handle>" appears only when the element was removed in the same document, the lookup
    saw every document, exactly one control matches role, exact name and credential class, and its
    context (a row label, a group or the nearest named landmark) equals the context recorded when
    the old handle was minted. After a navigation there is no retry step; candidates go in facts
    and the step is `browser_inspect`. The lookup is its own read-authorized observation, bounded
    to 500 ms, under its own invocation key, so nothing is painted and the action's coverage is
    untouched.
29. **Every block is named once, with the call that clears it.** An open dialog names its kind,
    read from the browser, and the `browser_dialog` actions valid for it. A held tab is described
    truthfully by the attempted host. A loading page retries a read once and refuses an action
    with the wait to run. A page that changed while the call was prepared says so and is safe to
    repeat when nothing was sent. Human pause and stop carry the fact only (ADR-0126). Caches only
    shape wording; they never refuse before dispatch.
30. **Accepting a leave-page dialog is a governed landing.** The page the tab lands on is
    authorized like any navigation, and a denial holds the tab.

### Waits return the moment they hold

31. **A sampled read answers once the document is quiet:** after 300 ms with no change, when the
    sample is non-empty or the document is complete. A satisfied first sample also needs quiet, so
    a form mid-hydration is not cut short.
32. **A condition that already held says "already".** Settling time is reported apart from the
    condition's time, and only when settling ran and did not finish.
33. **Text conditions read page text, not field values.** Only the agent's value is normalized.
    Typing "Ada" can never confirm a submit that showed nothing.
34. **One per-document change recorder** records only timestamps, ignores Ghostlight's own
    presentation host, and wakes waits and settling instead of polling. "Settled" still needs an
    unchanged geometry tick and no finite animation that is not Ghostlight's own.
35. **Intent is carried apart from budget.** Only an explicit `visual_settle:false` skips
    settling. Form preparation shares the invocation's budget, and a spent budget still runs one
    short value recheck.
36. **An expect check says when it cannot show the effect.** A pre-action sample records whether
    the condition already held. When it did, the result says the check does not show the action's
    effect and names a condition only the result satisfies. The pre-sample is governed like any
    read and never refuses or changes the action.

### Server instructions

37. When every lane above has landed, the instructions read, in six sentences:

    > Ghostlight controls the user's visible Chromium browser. Open or reuse a tab with
    > browser_navigate, or list this session's tabs with browser_tabs; omit tab when one tab is
    > unambiguous. Read a page with browser_read, and get handles for its controls from
    > browser_find or browser_inspect, which also show what each form control accepts. Act with
    > those handles, for example with browser_click or browser_fill_form; timeout_ms bounds the
    > whole call. Action results report changes Ghostlight saw on the page right after the action,
    > and a refusal for an expired handle names a usable replacement when Ghostlight can tell;
    > after a new page, fetch fresh handles. Every result states its effect and whether repeating
    > it is safe, and omits fields at their default; a successful read without coverage saw every
    > document it was asked to read.

    Each clause lands with the behavior it describes, never before it.

## Consequences

- An agent learns what a control accepts and what an action changed from the call that produced
  it. The common extra `browser_read` after an action goes away.
- A wrong option, date or checkbox word costs one refused call with nothing changed, and that
  refusal holds the answer.
- Results shrink by about a quarter on ordinary work, and the catalog by about a quarter. Large
  forms and long option lists grow items, bounded by the caps above.
- Actions take about the same time: a report window of at least 300 ms replaces the fixed
  250 ms sleep. Sampled reads on an idle page drop from about 1,000 ms to about 300 ms, and to
  near zero once the recorder lands.
- Scripts that read `--json` see the lean view. An absent key means its default.
- Handles stay stable across reads, which makes the memory per workspace bounded instead of
  growing with every read.
- Masking widens: camelCase credential names such as `tokenType` become credential-class and
  refuse a fill until the user authorizes it. That is the safe direction.
- A late consequence (a toast after 900 ms) is missed. The stated window keeps that honest.
- Deferred: a "new tab opened" clause until ADR-0197 Decision 20 lands; the extension's post-click
  window, which is a plain sleep and would need an extension release; spec-default annotation
  omission until a live client check.

## Verification

- Page assets: unit tests per decision in `assets/tests/*.test.js`, the asset hash refreshed per
  change.
- Service: sentence, facts and audit tests per new outcome with `preserve_target_names` on and
  off, and a test that no new key appears on a credential-class control.
- A controls fixture under `tests/fixtures/agent-ux/` and one live journey cover the
  options-bearing refusal, the one-call fill, the change report, the forms scope, the stale-handle
  retry and an already-held wait. The settle removal in decision 13 and the leave-page landing in
  decision 30 are confirmed live before release.
