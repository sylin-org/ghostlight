# Form retention: focus emulation and framework lifecycle

Date: 2026-09-13. Status: live reset cause confirmed; the third-party site is unchanged.

This corrects the diagnosis in the September 12 investigation and the older sections of
`continuation.md`. It preserves those observations without treating their explanation as proven.
The investigation initially proposed no product change. The owner subsequently approved scoped
focus emulation in [ADR-0168](../adr/0168-controlled-tab-focus-emulation.md). No WebMCP integration
or site-specific application patch is part of that decision.

The subsequent [installed-source acceptance](../testing/controlled-tab-focus-installed-2026-09-13.json)
passed after correcting restoration of the adapter's ownership cache after reload. Ghostlight
alone retained all five DOM/model values through an 81.465-second tab switch, return, field click,
and another 29.159 seconds. This proves the scoped automation behavior; the normal-lifecycle reset
diagnosis below remains valid.

## Confirmed follow-up: both tools lose edits under normal tab lifecycle

The owner authorized the controlled comparison. The same installed Chrome tab and adapter were
used throughout. Neither an extension deployment nor a parallel product instance was created.
Debugger function-call logpoints recorded call stacks without pausing or changing application
return values. Form-model reads compared only the five harmless expected test values.

| Fill mechanism | Focus emulation during tab cycle | Time away | Result after return |
|---|---|---|---|
| Ghostlight native form fill | Enabled | At least 62 seconds | All five DOM/model values retained, including after a field click. |
| Same Ghostlight draft | Disabled | 72.8 seconds hidden | Values retained while hidden, then all five reset after return. |
| Codex browser `setValue` | Enabled | At least 71 seconds | All five DOM/model values retained, including after a field click. |
| Same Codex draft | Disabled | 78.9 seconds hidden | All five reset after return, without any field click. |

This was a sequential comparison in one live document, not a randomized benchmark. The source
and call trace establish the mechanism beyond the comparison alone. An initial Codex setup edit
was also reset before the measured emulation-on phase began; it is excluded from the table.

The decisive normal-lifecycle Codex run recorded this order, relative to the visible event:

1. At 0 ms, the document became visible.
2. At 5.4 ms, the site's visibility handler called `refreshUser`.
3. At 34.4 ms, its window-focus handler also called `refreshUser`.
4. Two successive GET requests to `/api/v1/users/me/` returned HTTP 200: user refresh, then profile
   reload. Only request paths, methods, status and timing were inspected; no response bodies or
   authentication headers are retained in the evidence.
5. At 502.5 ms, the profile-loading callback called React Hook Form `reset()`. All five test values
   disappeared from both DOM and model. The form object was unchanged. No field was clicked.

Source inspection of the exact scripts already loaded in that page confirms the full chain:

- `/_next/static/chunks/0bmxgjjbyvnvw.js` defines the user context. Window focus and document-visible
  events force a user refresh. Its normalization creates a new user-data object and stores it in
  React state, even when the account is the same.
- `/_next/static/chunks/0t~veiw0ik-.2.js` defines the profile. Its loading effect depends on that
  whole user-data object, alongside initialized state, router, and loading callback.
- When user data changes identity, the effect fetches the saved profile again, then calls the
  form's `reset` with the fetched values. It does not preserve dirty fields. The live reset caller
  was at line 1, column 40482. The visibility-triggered refresh caller was in the user-context
  script at line 1, column 12579.

The remaining demonstrated retention failure is an application reset after valid input. It is
not evidence of missing keystrokes, a React trust requirement, or a unique Codex fill advantage.
Focus emulation hides the triggering lifecycle events; restoring normal behavior exposes the
same failure in both tools. It does not guarantee retention against every other refresh source.

The site-level fix should initialize the form for a stable account identity rather than reset
on every refreshed user object. If background updates must populate the form, preserve edited
fields deliberately, including custom controls outside the form library. `keepDirtyValues` is a
candidate for the registered fields, not a complete review of all of the site's custom state.
These are fixes for the site owner; no production site code was changed in this experiment.

For Ghostlight, indefinite focus emulation would mask the symptom while changing human browsing.
ADR-0168 subsequently defines scoped emulation and its restoration boundaries. It is not a
universal repair for a site that resets valid drafts.

The [content-free comparison record](../testing/form-retention-focus-comparison-2026-09-13.json)
contains the measured phases and source locations. Diagnostic logpoints, page probe, remote
object handles, and explicit focus override were removed at the end. The form was never submitted.
No product code changed in this follow-up, and the earlier uncommitted adapter work remains.

## Earlier findings from the installed browser

The affected profile uses React Hook Form registered inputs. The inspected input props have
`name`, `onChange`, `onBlur`, and `ref`, but no controlled `value` prop. Describing this only as
React reconciling controlled inputs obscures the form library's separate model and registration.

After the document-local focus correction and reload of adapter source 1.1.5:

- Complete native keyboard replacement updated the actual form library's `getValues()` for all
  five test fields. The data reached application state, not just the DOM or dirty indicator.
- Whole-value native insertion also updated the actual form model. Both mechanisms later failed
  the complete tab-away, return, and click journey in live tests.
- A native single-field edit survived more than 70 seconds of visible waiting and a later click.
  Waiting alone is not a sufficient reproduction or retention proof.
- A passive wrapper around the five controls' existing value setters caught a later write to
  every field from the page's registration/ref path during React commit. The five writes occurred
  together at document times 187411-187413 ms. Subsequent `getValues()` returned the saved profile
  values, and dirty state was cleared. No test form was submitted.
- During Codex browser control, an inactive profile tab still reported `visibilityState` as
  `visible` and `hasFocus()` as true. Disabling `Emulation.setFocusEmulationEnabled` restored
  hidden/unfocused readings. A subsequent real focus cycle reproduced the model loss.

The captured setter stack ends in the page's minified `ref` callback. Its structure matches
React Hook Form's `register().ref -> updateValidAndValue -> setFieldValue` path. The stack identifies
the writer, not the earlier caller that replaced or reinitialized the model. A background refresh,
an application reset effect, or a form-instance lifecycle change remains to be distinguished.
The site's precise form-library version has not been established.

A separate fixture using React 19.1.0 and React Hook Form 7.62.0 retained values through rerenders
for controlled React input, registered input, Controller input, and Controller textarea. Complete
native keyboard input, whole-value native insertion, and the prototype-setter/event experiment
all retained values there. A hand-built fixture that rejects untrusted events cannot establish
that React or React Hook Form itself imposes that rule.

The fixture also reproduced a misleading diagnostic: a wrapper installed into one React props
object logged only the first edit, while the current handler and actual model processed both
edits and retained `AB`. React had replaced the props object. The older live probe's exact code
is unavailable, so this demonstrates a possible measurement error rather than proving that
specific probe was stale.

## Playwright: a concrete focus difference

[Playwright's Chromium initialization](https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/chromium/crPage.ts)
enables `Emulation.setFocusEmulationEnabled` for ordinary main-frame sessions, subject to its
default-override conditions. This changes the page's lifecycle environment independently of how
text is entered.

In [Playwright issue 11645](https://github.com/microsoft/playwright/issues/11645), a Windows user
reported missing blur events and `document.hasFocus()` remaining true. A
[maintainer response](https://github.com/microsoft/playwright/issues/11645#issuecomment-1022574372)
explains that pages are kept focused to avoid background scheduling restrictions, and opening
DevTools can reset that emulation. This is a close match to our measurement discrepancy.

[Issue 24130](https://github.com/microsoft/playwright/issues/24130) reports that opening another
tab does not fire `visibilitychange`. Its
[maintainer response](https://github.com/microsoft/playwright/issues/24130#issuecomment-1631163647)
identifies this as intentional behavior. The older
[background-tab feature request](https://github.com/microsoft/playwright/issues/3570) records
the broader difference between Playwright-controlled tabs and ordinary inactive tabs.

[Chrome's focus-emulation documentation](https://developer.chrome.com/docs/devtools/rendering/apply-effects#emulate_a_focused_page)
explicitly documents visible state and suppressed visibility-change events under focus emulation.
Thus an automation comparison can accidentally omit the lifecycle event it intends to test.

For ordinary text, Playwright's [fill operation](https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/dom.ts)
uses [injected focus/selection preparation](https://github.com/microsoft/playwright/blob/main/packages/injected/src/injectedScript.ts)
and [native text insertion](https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/chromium/crInput.ts).
This is prior art for a small browser-editing mechanism. It does not show that per-character
typing, a priming character, or a forced final Tab is required for React.

The official [OpenAI browser-extension documentation](https://learn.chatgpt.com/docs/chrome-extension)
describes its browser integration but does not specify its host-side fill algorithm.
[Research 28](28-chatgpt-browser-extension-2026-09.md) establishes that the extension package
delegates ordinary fill to the native runtime. The observed focus behavior is evidence about
our session; it is not source-level proof of Codex's complete implementation.

## React Hook Form: a matching reset report

[Discussion 8233](https://github.com/orgs/react-hook-form/discussions/8233) provides a directly
relevant reproduction: edit a user form, cause a background refresh by changing window/tab focus,
and watch remote data overwrite the draft through `reset(user)`. It discusses preserving edited
fields when applying refreshed data. This is a matching failure class, not a confirmed diagnosis
of the GenAI.Works page.

The [7.62.0 implementation](https://github.com/react-hook-form/react-hook-form/blob/v7.62.0/src/logic/createFormControl.ts)
explains a crucial diagnostic limit. `reset(values)` can replace internal values and clear the
field registry without calling native `form.reset()`. A later ref registration can then write
those values back into the same DOM nodes. Therefore, the absence of a DOM `reset` event or node
replacement does not rule out an application-initiated React Hook Form reset.

Searches for the affected site's hostname and profile-reset symptom found no directly matching
public report. They do not establish that no such report exists.

## WebMCP: relevant reports, different integration contract

[WebMCP issue 104](https://github.com/webmachinelearning/webmcp/issues/104), opened February 21,
2026, reports declarative tool execution changing fields without the usual input hooks noticing,
leaving UI and application state out of sync. The discussion points to tool lifecycle events.
It is evidence that a browser-managed form interface still needs an explicit event contract,
not proof that Ghostlight has the same defect.

[Issue 138](https://github.com/webmachinelearning/webmcp/issues/138), opened March 12, 2026, is an
Angular maintainer's analysis of JavaScript-driven forms. The rendered inputs can expose only
part of a richer model. It recommends evaluating imperative tools tied to that model for these
applications, with discussion of dynamic fields and collaboration between users and agents.

[Issue 199](https://github.com/webmachinelearning/webmcp/issues/199) separately discusses stale
closures and framework lifecycle. Registering an API once does not guarantee that its callback
reads current application state. This is conceptually related to our stale-probe demonstration,
but is not an identical bug.

[Chrome's declarative WebMCP guide](https://developer.chrome.com/docs/ai/webmcp/declarative-api)
describes annotated forms, browser population, tool lifecycle signals, and distinct manual versus
automatic submission behavior. This requires site participation. It cannot be assumed to repair
an arbitrary existing React form, and changing fields does not authorize submission.

## Earlier experiment plan and fix candidates

The focus comparison and reset-caller tracing in items 1-2 have now been completed above.

1. Run the same five-field edit and real tab-away/return/click sequence with focus emulation
   explicitly controlled. Compare Ghostlight and Codex with both normal lifecycle and emulated
   focus. Confirm inactive state through browser tab metadata as well as page visibility. Check
   retention after restoring normal focus behavior, not just while emulation remains enabled.
2. Capture the call that replaces React Hook Form's values or form instance. The existing setter
   stack is downstream of that event. Distinguish `reset(values)`, instance recreation, and data
   refresh effects. Read current state; do not rely on a wrapper installed in stale React props.
3. Keep browser-native editing as the general adapter mechanism. Prefer a small focus, selection,
   native-edit, and postcondition path. Complete keyboard descriptors remain useful for explicit
   typing, but adding more key events or longer fixed waits is not supported as a retention fix.
4. If lifecycle handling is responsible for the difference, evaluate any emulation as a scoped
   browser mechanism with explicit restoration. Leaving focus emulated indefinitely would only
   hide this failure and would change normal user browsing. A site-side reset effect that discards
   valid edits has no universal repair through a different keystroke sequence.
5. Treat site-provided WebMCP tools as a separate future integration option. Do not patch private
   React internals, suppress application resets, or repeatedly rewrite lost drafts as a generic
   Ghostlight fill implementation.

The document-local focus correction is in the installed 1.1.5 source. The extension suite passed
246 tests after that change; changed JavaScript syntax and whitespace checks passed. These gates
do not establish live retention. The research follow-up changed documentation only. No commit,
publication, parallel Ghostlight deployment, or form submission occurred. The temporary passive
profile probe was removed and the local fixture server was stopped after research.
