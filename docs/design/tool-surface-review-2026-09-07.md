# Tool surface review, 2026-09-07

The owner rejected caller-authored host restrictions after an all-open Reddit session repeatedly
sent a Reddit-only host list, then asked for other misaligned capabilities to remove from tools.
This is a review of the current surface, not an implementation decision for the candidates below.
No tool, configured policy, or browser behavior was changed by this review.

Subsequent decision: the owner authorized removal of both request restrictions and flow dry-run,
and consolidation of sequence into flow. [ADR-0162](../adr/0162-configured-authority-and-one-flow-tool.md)
records the implemented decision. Submission bundling remains supported. This review retains its
original 24-tool baseline and reproduction findings below.

## Scope and evidence

Baseline: `c7f46e8a`. Reviewed all 24 tools in the current source catalog and a fresh installed
MCP `tools/list` response. The fresh response includes the recently added `expect` schemas;
the current chat's previously loaded tool descriptions are older and are not catalog authority.
Checked input decoding, authority snapshots, relevant executor paths, and ADRs 0101, 0107,
0113, 0133, 0136, 0157, and 0158. The current language and intent contracts govern the review.

The criterion is ownership and user value. A useful input states browser intent, selects a
resource, requests an observation, or bounds time/output. A model-authored authority restriction
belongs to a different concern. An advanced operation is not misaligned merely because it is
powerful or optional.

The installed catalog is retained at `.tmp/tool-surface-catalog.json`. Two effect-free installed
dry-run probes are at `.tmp/tool-surface-dry-run-evidence.json`. No page navigation, editing,
submission, or script execution was needed for these probes.

## Recommended removal order

### 1. Remove request-authored authority from the model-facing contract

Remove `restrict_hosts` and `restrict_capabilities` from all ordinary tools, including
`policy_explain`. They let the model reduce its own call's authority, cannot constrain a hostile
caller that omits them, and have caused real failures in the owner's open workflow. Configured
user/organization authority, human runtime controls, and document admission remain necessary.

The incident's four blocked script requests explicitly supplied
`["reddit.com", "www.reddit.com", "old.reddit.com"]`. Audit denial `D-219e5238` records zero
configured policy layers, request restrictions evaluated, and no browser effects. The human
coverage details identified a Google embed outside that list. Reads returned permitted content;
the accepted unrestricted-script boundary refused execution when a document was excluded.

Source: `language/catalog.rs:1470`, `language/mod.rs:75`, `governance/mod.rs:1821`, and
`work/documents.rs:253`. This was an explicit existing contract in ADR-0107, not a hidden policy.

### 2. Remove flow dry-run from ordinary browser tools

`browser_flow.dry_run` is decoder/classifier inspection. It does not establish that live targets
exist, document access is permitted, or browser effects will succeed. That can be useful in
developer validation, but it is not a browser job and should not be a routine execution mode.
Normal execution must still validate inputs before effects.

The current implementation also produces misleading success evidence. Installed reproductions:

| Input | Actual result |
| --- | --- |
| Dry-run one `browser_click` with empty arguments | `status:succeeded`, `effect:none`, summary says one step decoded, but the step contains `decode_error` because no target or point was supplied. |
| Dry-run inspect, then click using a reference to inspect's result | `status:succeeded`, summary says two steps decoded, but the click row says the earlier inspect step is missing. The dry-run reference map is always empty. |

These prove the current result problem, not an execution or policy bypass. The probes have
invocations `invocation_a85e5a5b816e4563b74ba3a1151506d8` and
`invocation_b8e38ad82df845368a5791e516712287`.
Source: `work/flow.rs:42`, `language/catalog.rs:1156`, `language/outcome.rs:393`.

### 3. Consider removing explicit submission from form fill

`browser_fill_form.submit_target` combines setting values with activating a submit control.
The field changes the required capability set and adds a distinct effect to a tool named Fill.
A simpler contract would fill values, with submission expressed as an explicit click. An ordered
composition can retain the one-call convenience when both actions are actually wanted.

This is a proposed API clarification, not another demonstrated wrong submission. The current
default does not explicitly submit, and the browser tests verify unsent drafts. Removing the
field would not create a drafts-only security boundary: websites may autosave or attach effects
to editing, and a separately authorized click or script can still submit.
Source: `language/catalog.rs:804`, `work/forms.rs:93`, ADR-0133 Decision 3.

### 4. Consolidate the two batching languages before removing one

`browser_sequence` accepts a second vocabulary of action-specific step objects. Its executor
turns those objects back into ordinary operations. `browser_flow` already composes ordinary tool
calls and adds result references. Maintaining both duplicates input rules and teaches two ways
to express ordered browser work; for example, sequence click lacks the ordinary click's selector,
modifiers, and postcondition options.

Recommend one composition surface, with an equally short path for fully known steps, then retire
the redundant surface. Do not simply delete sequence and force every short batch to acquire named
steps and reference machinery. ADR-0133 deliberately retained sequence for brevity; changing that
choice needs a new decision preserving that benefit. Keep bounded composition, truthful child
receipts, default stop behavior, and explicitly requested continuation.
Source: `work/sequence.rs:25`, `work/flow.rs:26`, `language/catalog.rs:1191`, ADR-0133 Decision 2.

## Full tool inventory

Request restriction removal applies to every row. Other recommendations are specific below.

| Tool | Disposition and reason |
| --- | --- |
| `browser_tabs` | Keep list/focus/close: concrete browser resource operations. |
| `browser_navigate` | Keep URL/tab/new-tab/browser selection and bounded reuse. `beforeunload:discard` is a narrowly named unsaved-change action, not a general policy override. |
| `browser_history` | Keep back/forward/reload and explicit cache bypass. This is browser navigation, not deletion of audit history. |
| `browser_window` | Keep zoom/resize: meaningful browser UI operations with explicit dimensions. |
| `browser_read` | Keep target/mode/output bounds: useful observation intent and context control. |
| `browser_inspect` | Keep scope/root/depth/item bounds: structural observation and target discovery. |
| `browser_find` | Keep text/scope/ranking bounds: semantic target discovery. |
| `browser_screenshot` | Keep page/target/region capture. Current view handles bind coordinates to an actual observation. |
| `browser_click` | Keep target/selector/point and explicit input modifiers. `expect` states a desired observed outcome. |
| `browser_scroll` | Keep page/target/wheel forms: they express distinct visible scrolling jobs. |
| `browser_hover` | Keep target/point input: required for hover-driven page UI. |
| `browser_fill_form` | Keep typed field filling; discuss removing `submit_target` as above. |
| `browser_type_text` | Keep native typing, focused input, and clear-first. Real editor compatibility depends on native input. |
| `browser_press_key` | Keep keys/modifiers/strokes/repetition: explicit browser input, with fixed bounds. |
| `browser_drag` | Keep target/point drag: needed for canvas and ordinary drag UI. |
| `browser_wait` | Keep observable conditions. A bounded explicit duration is a timing request, not an authority knob; no demonstrated removal case. |
| `browser_dialog` | Keep status/accept/dismiss/respond: concrete dialog handling. |
| `browser_upload` | Keep explicit file/image sources and destinations: core attachment jobs. |
| `browser_execute` | Keep explicit page JavaScript under configured Execute authority. It is a core browser capability; removing it would lose supported user work. |
| `browser_sequence` | Consolidation candidate, preserving a short batching path. |
| `browser_flow` | Keep composition and result references; remove model-facing dry-run, consider consolidation. |
| `browser_record` | Keep recording lifecycle and delivery. Discard releases a recording, not governance history. |
| `browser_diagnose` | Keep page console/network observations. It does not configure Ghostlight audit or grant authority. |
| `policy_explain` | Keep read-only explanation, especially during holds; remove its request restrictions with the common fields. |

## Findings that should not be mistaken for removable capabilities

- No model-facing policy editor, Pause/Stop override, attention reset, audit-disabling flag,
  frame-policy override, signature override, or generic force switch exists in these 24 tools.
- Timeout and output-size inputs bound the requested job/result; they do not let a client exceed
  service ceilings. Removing them indiscriminately would lose useful long-wait and small-output
  requests. Named defaults should keep them unnecessary for ordinary calls.
- Postconditions and semantic selectors can legitimately require additional Read authority.
  That is a requested observation, unlike self-imposed restrictions. Their semantics remain useful
  after caller restriction removal.
- The UI's `Refused by your own rules` caption misattributes request restrictions because it
  treats every non-managed tier as a personal policy (`ui/lib/view.js:268`). Correct the retained
  history rendering even if new requests can no longer carry those fields.
- Excluded document hosts are deliberately absent from durable permission rows and present only
  in volatile human coverage details. Improve the unlabeled embedded-document row and incomplete
  coverage explanation without weakening that privacy decision.

## Implementation boundary

The current ADRs and tests explicitly preserve several candidates. The next implementation cycle
needs a new decision for the selected removals, current catalog/decoder agreement, deliberate
handling of older clients still sending removed fields, and preservation of configured authority.
Do not silently strip a restriction from queued or already accepted work. Do not silently erase
an optional submit action from an accepted request. Refresh client catalogs when the advertised
contract changes. The review does not authorize a wider loss of browser functionality.
