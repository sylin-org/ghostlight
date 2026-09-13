# Unsaved form reset investigation

Date: 2026-09-12. Status: cause established; source correction implemented.

## Observed reproduction

The owner requested a test draft at `hackathon.genai.works/profile#profile`, with no submission.
Ghostlight reused the existing tab but returned `document_unavailable` for inspection,
screenshots, and diagnostics. CUA could inspect the same tab and filled empty profile fields
with sample values. A screenshot and accessibility state showed the unsaved draft. After the
owner switched to another browser tab, returned, and clicked a field, the draft disappeared.
A subsequent inspection showed the original saved profile and empty previously edited fields.

This established the reported reset after a CUA fill. At that point it did not establish a
particular framework event mismatch or page refetch/reinitialization cause. No submission,
application-state replacement, or profile save was performed.

## Instrumentation

The existing extension Developer diagnostics preference controls a separate local structural
form trace in already-controlled top documents. It is independent of the native connection,
the process diagnostics marker, and `browser_diagnose` page console/network capture.

- Up to 100 ordinary controls; ten minutes per activation; 250 ms checks for changes.
- At most 400 persisted rows, re-projected on restoration and export.
- Focus, visibility, pagehide/pageshow, reset, empty-state and node-set changes.
- Coalesced input/beforeinput/change flags, including trusted versus synthetic input evidence.
- Explicit Ghostlight fill/type/clear start and completion markers.
- Chromium tab/document identifiers, closed states, counts, and times only.
- No values, lengths/hashes, labels, selectors, page URLs, event data/keys, or page logs.

Credential controls are excluded. Observers never write to the page, intercept page setters,
dispatch events, or move focus. New rows stop when disabled or ownership is released; old
rows remain available for the local export. ADR-0145 records the narrow metadata scope.

The trace detects nonempty-to-empty changes and control replacement. It does not compare
nonempty text or identify a framework state owner. Input flags indicate observations between
checkpoints, not definitive attribution. Embedded-frame forms are outside this trace.

## Reproduce with the owner

1. Reload the unpacked source extension. Refresh the profile only after confirming there is
   no draft to preserve; extension reload alone does not reinject existing documents.
2. Open/adopt the profile through Ghostlight so it is a controlled tab. No save or submit.
3. In Ghostlight extension Options, turn on Developer diagnostics. This is separate from the
   popup's Process diagnostics log. The owner is enabling this flag.
4. Fill the test draft, switch browser tabs, return, and click a field. Also compare a small
   draft typed natively, with the same focus transition, without submitting either draft.
5. Choose Save developer diagnostics in Options. The JSON contains `form_diagnostics` beside
   the connection report. Turn the flag off afterwards. Off/on starts a fresh observation
   period if ten minutes have elapsed.

Use document identity to distinguish a new document from a same-document reset. Inspect the
empty-state changes and node replacements near window/visibility/focus events and operation
markers. A missing trace is missing evidence, especially on an old document without a receiver.

## Verification

Formatting, workspace Clippy with warnings denied, all workspace Rust tests, all 242 extension
tests, changed JavaScript syntax, and diff whitespace checks pass. The extension tests exercise
the actual worker routing and content initialization race, privacy projection/restoration,
credential exclusion, bounded retention, expiry, coalescing, cleanup, and failure containment.

A disposable HTTP fixture in the user's normal Chrome ran the observer source. Native typing
produced trusted input evidence; silent clearing recorded two newly empty controls without
input flags; replacing two input nodes recorded two additions and two removals. After disabling,
further native typing left the row count unchanged. Sentinel field contents/attributes were
absent from the trace and the submission count stayed zero. Browser control did not produce
actual tab-visibility transitions in this fixture, so that physical lane remains with the owner.
The temporary fixture tabs and local server were closed.

The owner then requested direct JavaScript instrumentation of the affected live page. The same
observer was attached temporarily as `window.__ghostlightFormResetProbe`, without changing the
extension flag or patching page setters. It initially observed 17 ordinary controls, three
nonempty. Four test fields were refilled without submission; the trace recorded synthetic
input/change events and seven nonempty controls.

The owner switched away, returned, clicked a field, and again found the draft empty. The trace
showed all four test controls becoming empty together 40.25 seconds after the synthetic fill,
while the document was still visible and focused. The tab became hidden 22 seconds later. The
same document and the same 17 control nodes remained; no input, change, reset, navigation, or
network-resource event accompanied the clear.

A second temporary wrapper around the existing per-control value setters captured the call site
without retaining values. React's reconciler wrote the four input/textarea values to empty from
the loaded Next.js chunks. `HTMLFormElement.reset()` was not called. Repeating the synthetic fill
produced the same four React assignments after 40.26 seconds. The user-visible tab switch exposed
the loss but did not trigger it.

An A/B check then entered one field through real keyboard input and one through the synthetic
setter. Both survived the next 45-second cycle. This indicates that a native editing transaction
updates or initializes the form's authoritative model; a later synthetic sibling can then be
retained with that state. The exact private framework state remains opaque, but the physical
failure is established: setter plus synthetic generic events can produce an immediate DOM and
dirty indicator that React later reconciles away.

The owner separately confirmed that the embedded Codex browser control fills this form without
the loss. That agrees with the live native-keyboard control and distinguishes a site-wide form or
tab-lifecycle defect from the failing synthetic setter path. The correction belongs at the
browser input mechanism: ordinary input and textarea filling must create a native editing
transaction and prove retention across a later React render.

`read()` returned only bounded metadata rows. The temporary setter trace added source call sites
only to memory and was not part of the persisted product diagnostic schema. Both probes were
removed after capture. The final two A/B values were cleared through real key input, which also
removed the page's unsaved-change indicator. No form was submitted and no live trace was
exported.

## Correction

Source `browser_fill_form` and ordinary clear operations now route textual inputs and textareas
through the page-local native editing transaction already used for targeted typing. Selects,
checkboxes, radios, and file inputs retain their distinct semantic setters. If native editing is
unavailable, Ghostlight refuses with the prior draft intact and does not fabricate input/change
events.

The controlled-form real-browser fixture includes a negative control that uses the former
prototype setter plus generic synthetic input/change events, then forces a framework-style model
render and observes the original value return. The Ghostlight fill must update the fixture's model
through trusted native input and survive the same later render, for both an input and textarea,
with zero submissions. Unit coverage also pins the single native edit and refusal behavior.

At the owner's request, the exact official ChatGPT Chrome extension v1.26.901.11451 was downloaded
from Google's update service and inspected outside the repository. It exposes `set_value`,
`type_text`, and Playwright locator-fill commands, but delegates locator execution to the native
OpenAI runtime and provides a generic CDP relay. The package therefore confirms the layered
control boundary but does not disclose the host-side fill algorithm. The package identity, hash,
clean-room limits, and transferable lifecycle findings are in
[Research 28](../research/28-chatgpt-browser-extension-2026-09.md).

The extension source has not been reloaded into the installed adapter. The published adapter
version and reviewed release artifacts are unchanged. Formatting, warnings-denied Clippy, the full
Rust workspace, changed JavaScript syntax, and all 242 extension tests pass. The isolated
Chrome/MV3 frame journey passes 69 checks with Chrome 152.0.7977.82, including the synthetic
negative control, trusted input and textarea edits, a forced later model render, retained values,
and zero submissions.
