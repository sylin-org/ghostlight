# ADR-0205: Friendly usage, strict authority

Date: 2026-10-29. Status: Accepted by owner direction. On 2026-10-29 the owner reviewed a session
in which an agent tripped over Ghostlight's inputs and set the rule: "I want Ghostlight to be a
friendly helper to the agent that'll be calling it, accepting what it can. All the really
important restrictive stuff - say, the policies - can still be strict. Usage can be friendly."
The same day the owner named the model for values: "handle certain tool parameters like CSS
handles it. a number, or a magical word, or a logically placed value - all can be used. And if a
non-mandatory parameter is missing, that's fine too." The wording of each rule below is the
agent's, under the delegation recorded in ADR-0198.

Builds on ADR-0198 (what agents see, and its acceptance test: a mistake costs one call) and the
standing leniency rule in docs/MEMORY.md ("Resilience through leniency"). Amends ADR-0203
Decision 8 only in that a wait may now leave its condition out.

## Context

A Sonnet 5.5 agent ran an ordinary work routine through Ghostlight on 2026-10-29: a SharePoint page
behind single sign-on, then a ServiceNow workspace. Of 18 calls that finished, 6 failed and 1
reported a success that did not happen. About 7 calls were needed.

Three failures were refusals of calls whose meaning was clear:

- `browser_press_key {key:"PageDown", repeat:2}` was refused with "repeat applies to a stroke
  sequence". The schema allowed the call; only the decoder forbade it.
- `browser_scroll {amount:600}` was refused because `amount` took four words only.
- `browser_wait` without a condition was refused with "missing field `condition`", which did not
  list the conditions. The agent gave up on waiting.

All three are the shapes Claude in Chrome uses for the same actions: a repeat count beside one key,
a numeric scroll amount, a wait that takes only a duration. Ghostlight harvests that interface
(ADR-0050), so it looks familiar, and the small differences are where a confident model slips.

The other failures were Ghostlight's own false answers:

- Twice a screenshot was refused with "That tab handle names no open tab in this session", and
  the next call listed the same handle. The page had committed a new document while the capture
  ran (a sign-in redirect, then a slow application's redirects). The capture succeeded; storing it
  failed a generation check that reports a changed page as a missing tab.
- A navigation to ServiceNow was refused as "ran out of time before the browser confirmed what the
  sent command did". The browser had reported the commit. Only the load had not finished inside
  the 8 second ready wait, and the timeout discarded the commits the receipt carried.
- A scroll said "Scrolled down" while nothing moved. With no target it scrolled the window, and
  SharePoint, like most enterprise applications, scrolls an inner container. The result hard-coded
  `scrolled: true`.

A review of the decoder found the posture behind the refusals. Of 98 refusal sites, more than 40
are rules between fields that the published schema never states, so a call can be valid against the
schema and still be refused. About 18 of them refuse harmless extra information ("load_ready
accepts no value", "bypass_cache is only valid when action is reload", "modifiers must be unique").
An unknown field is refused on every tool, and the refusal leaves out the fields the tool does take.

The owner's own audit shows the cost is usually small: 0.9% of 2.0 calls and 1.2% of earlier calls
were refused as invalid. It bursts when a model or its instructions lean on another tool's
conventions, as this session did.

## Decision

### Usage is friendly

1. **Ghostlight accepts every call it can read.** A value, a field or a whole call that has one
   sensible reading runs with that reading. Ghostlight refuses only what it cannot read safely, and
   the list of what stays strict is closed (Decision 9).
2. **Values read like CSS.** A field takes the forms a person would write. A number, a number with
   its unit, a keyword, or nothing at all can each be right:

   | Kind | Fields | Accepted |
   |---|---|---|
   | Duration | `timeout_ms`, a duration wait | `2000`, `"2000"`, `"2000ms"`, `"2s"`, `"1.5s"` |
   | Count | `repeat`, `ticks`, `percent`, `max_chars`, `max_results`, `max_items`, `max_depth`, `limit` | `3` or `"3"`; `percent` also `"125%"` |
   | Length | `x`, `y`, `width`, `height`, drag points | `120`, `"120"`, `"120px"` |
   | Distance | scroll `amount` | `600`, `"600px"`, `"50%"` of the viewport, `small`, `medium`, `large`, `page`, `top`, `bottom`, `end`; a negative distance goes the other way |
   | Flag | booleans | `true` or `"true"`, `false` or `"false"` |
   | Keys | `key`, `strokes` | named keys in any case and their common names (`esc`, `return`, `pgdn`, `up`); a chord (`"ctrl+a"`) is the key with its modifiers; a sequence (`"PageDown PageDown"`) is strokes; `repeat` works with one key too |
   | Choice | `button`, `direction`, and every closed word | any case; `button` also `left`, `right`, `middle` |
   | Address | `url` | an address without a scheme gets `https://`, or `http://` for `localhost` and loopback addresses, as the address bar does |

   Field names are read in snake case, camel case or kebab case, so `timeoutMs` and `new-tab` are
   `timeout_ms` and `new_tab`. A limit on how much is read (`max_chars`, `max_results`,
   `max_items`, `max_depth`, `limit`) or how long a call may take (`timeout_ms`) is brought inside
   its bounds instead of refused.
3. **A value left out comes from the page.** Like CSS `auto`, an omitted value is resolved where
   the facts are:
   - A page scroll with no direction goes forward on the axis that can scroll: down on an ordinary
     page, right on a page that only scrolls sideways. At the end it moves nothing and says so. It
     never turns back on its own, so an agent reading down a page cannot bounce.
   - A wait with no condition reads its condition from what it was given: nothing is `load_ready`,
     a duration is `duration`, a `target_` handle is `target_present`, a selector is
     `selector_present`, an `http://` or `https://` address is `url_contains`, and other text is
     `text_present`. `condition` stays for what the value cannot say: absence, settling, and a
     plain word meant as part of an address.
   - A handle carries its kind in its prefix (`tab_`, `target_`, `view_`), so it is never
     misread.
4. **Harmless extra information is ignored, never refused.** A field that does not apply to the
   chosen action, a repeated modifier or path, a value a condition does not use: the call runs
   without it and the result names it. On a tool that only observes (`browser_read`,
   `browser_find`, `browser_inspect`, `browser_screenshot`, `browser_wait`, `browser_diagnose`,
   `policy_explain`, and `browser_tabs` listing) an unknown field is ignored the same way.
5. **The result says how the call was read.** When Ghostlight used a value other than the one sent,
   or left a field out, the result says so in two facts, present only when they have content:
   `ignored` lists the fields not used, and `adjusted` maps each field to the value used. Where the
   page resolves a value, the tool's own facts carry it: a scroll reports its `direction`, the
   pixels it `moved`, and `at_end`; a wait reports the `condition` it waited for. An agent learns
   what Ghostlight understood without another call, and a wrong guess costs nothing hidden.
6. **A refusal carries its fix.** It names the accepted forms or values of the field it refused.
   An unknown field's refusal names the closest field the tool takes, when one is close, and lists
   the tool's fields.
7. **The schema shows the canonical form; the decoder reads more.** A client that validates against
   the schema sends canonical values and works as before. The schema advertises the other forms
   only where a model needs to know them: the scroll distance, a key's `repeat`, and the wait's
   value and optional condition. The scroll distance takes a number or a word through a
   field-level `anyOf`, the form the admitted clients already accept for a fill value; nothing
   composes at the root. Chords and key sequences stay out of the key schema, whose `oneOf` a
   strict client checks, so only a lenient client sends them.

### Truthful answers

8. **The three false answers end.**
   - A page that commits a new document while a read runs has not lost its tab. The refusal says
     the page changed, and an observation (`browser_read`, `browser_find`, `browser_inspect`,
     `browser_screenshot`) runs once more on the new page, authorized again for its address,
     inside the call's own deadline. "No open tab" is said only when the handle names no tab.
   - A navigation whose commit the browser reported succeeds. When the page has not finished
     loading in the ready wait, the result says `readiness: "loading"`, at the committed address.
   - A page scroll moves what actually scrolls: the document, or else the scrolling container
     under the middle of the viewport. It reports what moved. Nothing moving is effect `none`, and
     the summary says the page is already at its end or does not scroll.

### What stays strict

9. **These are refused, and nothing above loosens them:**
   - **Authority.** Policy, governance, Pause and Stop, host restrictions and capability checks are
     unchanged. `user_authorized_credentials` takes only `true`; it is a statement of the user's
     permission, not a value to read generously.
   - **Content.** Typed text, form values (ADR-0198 Decision 8's exact, then unique normalized
     matching), scripts, file paths and an address beyond its missing scheme are used as given and
     never reinterpreted.
   - **Two readings with different effects.** A target and a selector for one click, `key` and
     `strokes` together, a view region with a target, `tab` with `new_tab`, a recording saved to a
     target and as a download. The refusal names both readings.
   - **Counts of actions beyond their bounds.** A key repeated 500 times or a quadruple click is
     refused with the bound, because doing less than asked would misreport the effect.
   - **An unknown field on a tool that acts.** Ignoring a misspelled `new_tab` would act in the
     wrong tab. The refusal names the closest field.
   - **A value with no sensible reading**, such as `amount: "huge"`.

## Consequences

- A model that writes another tool's familiar shapes completes its first attempt. The three
  refusals in the 2026-10-29 session each run as meant.
- Results gain `ignored` and `adjusted` only when they say something, so ordinary results stay the
  same size. The catalog grows by a few descriptions.
- The catalog self-check keeps its promise that every advertised example and ordinary call is
  accepted, and drops the cases that were harmless extras. Its invalid cases keep only what Decision
  9 still refuses.
- Ghostlight now does a little interpretation, so the interpretation must be visible. Decision 5 is
  that guarantee; a reading that cannot be reported is not made.

## Verification

- One test holds a set of familiar calls, in Claude in Chrome and Playwright style and the three
  from this session, and proves each decodes to the operation it means.
- Each false answer gets a test that reproduces it: a page committing during a capture, a ready
  timeout with a reported commit, and a scroll on a page whose document does not move.
- The change is deployed through the dev loop and tried live on the owner's installation, on a page
  that scrolls an inner container.
