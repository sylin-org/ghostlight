# ADR-0133: Restore published browser capabilities through the 1.0 language

- Status: Accepted
- Date: 2026-08-22
- Amends: ADR-0107 Decisions 1, 2, 4, and 7 and the active 1.0 language contract
- Builds on: ADR-0035, ADR-0036, ADR-0037, ADR-0050, ADR-0078, ADR-0080, ADR-0101,
  ADR-0103, ADR-0111, ADR-0131, and ADR-0132

## Context

The clean-room 1.0 rebuild preserved Ghostlight's architecture and safety boundaries but did not
preserve every browser capability that the published 0.8 extension exposed. The checked 0.8
recovery inventory dispositioned tests and artifacts, but a disposition is not proof that a user
capability still exists. A direct comparison of the exact published 0.8 catalog at
`993135b048b60622157266b53b21f1719c9df4b3` with the current source found genuine behavioral
contractions.

The missing families are:

- modified and triple clicks, repeated or sequenced keys, focused typing, fixed waits, and
  coordinate wheel input;
- REPL-grade JavaScript, including top-level await and the earlier bare-return recovery;
- unambiguous label, accessible-name, placeholder, role, and form-scoped targeting in the same
  call as an action;
- article-first text, larger bounded reads, hierarchical inspection, subtree reads, and diffs;
- bounded inline-file upload and reuse of a captured screenshot as an uploaded or dropped image;
- guarded navigation that can explicitly discard an unsaved-change prompt; and
- a bounded composition surface with result flow, error policy, dry run, and one total budget.

Restoring the old names and schemas would contradict the 1.0 language. Treating the missing work
as an extension-only patch would also be wrong: model semantics belong to the orchestrator, while
only DOM and Chromium mechanics belong to the extension.

## Decision

### 1. Parity is measured by behavior, not historical names

The published 0.8 surface is an evidence source. It is not an implementation or naming authority.
Each durable behavior is re-expressed through the current typed language, one executor, workspace
aggregate, governance facade, browser port, and completion path. No 0.8 tool name becomes an alias.

The active restoration batch is [`../tasks/capability-restoration/`](../tasks/capability-restoration/).
Its ledger is the authority on implementation progress. This ADR records the destination, not a
claim that planned work already exists.

### 2. Keep narrow tools and add one explicit composition tool

The existing 22 tools keep their names and cohesive jobs. Their schemas gain branches only where
the branch expresses the same user intent. Click remains click, fill remains fill, and reading
remains separate from inspection.

One new tool, `browser_flow`, restores general bounded composition. It does not replace
`browser_sequence`. `browser_sequence` remains the shortest surface for two to eight already-known
interactions on one tab. `browser_flow` is for one to twenty decoded Ghostlight operations whose
later inputs may depend on earlier results.

This amends ADR-0107's exact 22-tool count to 23. The count is still a consequence of cohesive
intent boundaries, not a target metric.

### 3. Semantic selection is an alternative input to narrow actions

Actions that currently require a `target_` may instead accept a typo-closed semantic selector with
a required accessible `name` and optional closed `role`, exactness, and form scope. The adapter
observes accessible names from labels, placeholders, ordinary name sources, and open shadow roots.
The orchestrator owns the product query and ambiguity rule.

Zero matches fail without an effect. More than one match fails without an effect and returns
bounded candidates. Exactly one match is registered as a current generation-bound target and
passes the same authorization, credential preflight, indication, physical effect, landing
governance, and completion path as a supplied handle.

`browser_fill_form` accepts target handles or semantic selectors per field and typed string,
boolean, or finite-number values. It may explicitly submit the resolved containing form without
requiring a separately discovered submit handle. Ordinary target-handle and `submit_target`
branches remain valid.

Narrow effect tools may carry one optional postcondition using the same closed observation
vocabulary as `browser_wait`. The executor applies the effect and observes the condition under one
deadline. A failed postcondition reports the effect truthfully; it never rewrites an applied effect
as no effect.

### 4. Precision input returns through current view and target semantics

`browser_click` accepts unique modifiers and click counts from one through three for target,
semantic-selector, and current-view point branches.

`browser_press_key` accepts either the existing one-key input or an ordered sequence of at most
twenty typed keystrokes. A sequence may repeat from one through one hundred times within the
ordinary deadline. `browser_type_text` may target a current target, a semantic selector, or the
currently focused editable control. Focused input requires a physical focused-target description
and the same credential handoff used by explicit targets.

`browser_wait` gains a duration branch bounded from zero through 10,000 milliseconds. It remains
cancellable and deadline-bound.

`browser_scroll` gains a current-view point branch with a direction and one through ten wheel
ticks. Image coordinates resolve once through the workspace's existing ownership, generation,
viewport, zoom, and bounds checks. The extension receives page coordinates, not a model-facing
view handle.

### 5. Reading restores useful prose and structured document state

`browser_read` becomes article-first by default, with an explicit visible-text mode and a raised
50,000-character ceiling. Article extraction falls back to visible document text when no useful
article exists. Target reads remain current-target reads.

`browser_inspect` retains its current bounded target-list mode and gains a hierarchical document
mode. Document mode accepts an optional current target as a subtree root, a bounded depth, and a
50,000-character result ceiling. It returns a bounded typed tree and a generation-bound
`snapshot_` handle. Supplying a current snapshot returns a bounded diff against the new observation.
Snapshots contain semantic structure only, never screenshot bytes, editable values, credentials,
or arbitrary hidden DOM.

### 6. `browser_execute` regains REPL behavior without changing its signature

The page script continues to arrive through `browser_execute` and the execute capability. CDP
evaluation enables promise waiting, by-value return, user gesture, and REPL mode. A syntax failure
caused only by a bare top-level `return` receives one async-function fallback. Exceptions return
their useful bounded description. Source and results remain excluded from audit and presentation.

### 7. Upload accepts bounded local, inline, or captured bytes

`browser_upload` accepts exactly one source: absolute local paths, bounded inline files, or one
current `image_` handle. All sources converge on the existing `PhysicalFile` boundary after
governance and credential preflight. The five-file and 5,000,000-byte aggregate limits remain.

Every successful `browser_screenshot` returns its normal image content plus a generation-bound
`image_` handle backed by a single bounded volatile workspace asset. A newer image supersedes the
older asset. Navigation, ownership loss, workspace release, or service exit erases it. Image bytes
never enter audit, presentation, structured facts, disk, or extension storage.

Upload may attach bytes to a resolved ordinary file input or drop one captured image at a current
view point. Coordinate drop uses the same view checks as pointer work. A receipt states only that
the browser dispatched the attach or drop; it does not claim that a page or remote service
accepted the file.

### 8. Navigation may explicitly resolve only `beforeunload`

`browser_navigate` gains a closed `beforeunload` choice. The default stops and reports the blocking
dialog. `discard` accepts only a `beforeunload` prompt produced by that requested navigation and
continues. It does not accept alerts, confirms, or prompts unrelated to leaving the document.

This is the 1.0 replacement for 0.8 `force:true`: the name states what is discarded and avoids a
general force bypass.

### 9. `browser_flow` composes decoded operations, not browser commands

A flow contains one through twenty steps. Each step has a unique id, a current advertised tool
name other than `browser_flow` or `browser_sequence`, and an argument object. A later argument may
contain an explicit result reference object naming an earlier step and a JSON Pointer into its
canonical result envelope. There are no magic selector strings and no access outside earlier
bounded results.

Before execution, Ghostlight validates every step name, reference direction, static argument
shape, total count, and the bounded total budget. `dry_run:true` decodes and classifies the plan but
dispatches nothing. Runtime argument resolution is followed by the ordinary decoder again.

Every executed step uses the same immutable invocation authority ceiling, intersects the flow's
restrictions with its own, enters the ordinary operation executor, authorizes its complete RAWX
set, and produces its normal typed result. `on_error` is `stop` by default or explicit `continue`.
Partial or uncertain effects remain partial or uncertain in the flow result. Composition cannot
make an individual effect repeat-safe.

### 10. New physical mechanisms are revision-negotiated

The adapter protocol remains one closed typed fringe. Each browser command declares both its
required capability name and minimum revision. New semantic document, precision input, focus,
guarded navigation, and image-drop mechanisms require revision 2 of their existing capability
families or a named new family where no honest family exists.

An older installed extension fails before dispatch with a precise capability-version result. It
never receives a new command it might partially understand. The opaque browser connector remains
unchanged.

### 11. Superseded mechanisms do not return

Full restoration does not mean restoring behavior that 1.0 deliberately replaced:

- agent-authored `narrate` prose conflicts with content-free presentation;
- destructive diagnostic clearing and regular expressions remain superseded by bounded literal,
  cursor-based reads;
- `update_plan` is a client workflow concern, not browser capability;
- the old `explain` name does not replace the owed first-class policy explanation surface;
- direct UDP syslog remains superseded by local append-only structured audit; and
- old tool aliases, numeric browser ids, selectors, path-based screenshot coordinates, and nested
  raw browser commands do not return.

### 12. Evidence precedes replacement release artifacts

Each restoration slice adds decoder/schema parity tests, executor and governance tests, typed wire
round trips, extension mechanism tests, and JavaScript syntax checks. The closing integration stage
runs the full repository gates, real process boundary, and live installed-extension journeys for
every restored family.

Any extension change makes the currently pending Chrome Store draft stale. Only after live proof
may the deterministic 1.0 ZIP and release evidence be rebuilt. Uploading, resubmitting, publishing,
or mutating any public channel still requires a separate explicit owner confirmation.

## Consequences

- A capable model regains the published browser jobs without learning a second Ghostlight dialect.
- Common calls remain short; semantic selection and postconditions remove avoidable discovery
  round trips without creating a wide `browser_act` union.
- The workspace gains two bounded volatile semantic resources, `image_` and `snapshot_`, with the
  same ownership and generation posture as current handles.
- The bridge gains additive typed commands and per-command revision requirements. The browser
  connector remains opaque and stable.
- The active 1.0 contracts change only as each implementation slice becomes true.
- The existing Store review cannot be treated as the release review after this batch changes the
  extension bytes.

## Rejected alternatives

### Restore the 25 historical tool declarations

Rejected because names such as `computer`, `javascript_tool`, `form_input`, `script`, and
`upload_image` expose old mechanics and duplicate current 1.0 intents.

### Put every restored action in one `browser_act` union

Rejected for the same reason as ADR-0107: it trades an obvious tool choice for a large conditional
argument choice. Shared semantic selection belongs below the narrow verbs.

### Let `browser_flow` forward nested calls through the MCP connector

Rejected because it would bypass the one workspace lease, duplicate protocol concerns, and make
step authority and effect truth depend on a recursive client boundary.

### Store screenshots in the extension

Rejected because the extension should not gain a second pixel-retention lifecycle. One bounded
workspace asset is enough to bridge an explicit capture to an explicit upload.

