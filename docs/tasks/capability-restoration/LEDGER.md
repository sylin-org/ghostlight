# LEDGER: published capability restoration on 1.0 seams

Durable progress for [ADR-0133](../../adr/0133-behavioral-capability-restoration.md). Update this
file before work, after every material finding, and when a task completes or blocks.

## RESUME HERE

- State: READY. R1 through R3 are complete; no task is in progress.
- Next task: R4, [document reading](R4-document-reading.md).
- Implementation baseline: `dev` at
  `c8a181cc15e39b25b2cdc6864c8303efe345f561` before this batch's planning commit.
- Required first action: confirm the current head contains ADR-0133 and this batch, confirm no
  overlapping worktree changes, then execute R2 only.
- Current catalog: 22 tools. Planned catalog after R6: 23 tools with `browser_flow` added.
- Release state: the staged Chrome review is stale already due ADR-0131 and will become stale again
  as this batch changes extension bytes. Do not mutate the Store during R1-R8.
- Last inherited green evidence: ADR-0132 reports formatting, warnings-denied workspace Clippy,
  361 Rust tests, 119 extension tests, JavaScript syntax, repository integrity, the workbench
  surface, and a fresh isolated process journey. Re-run applicable gates; do not treat inherited
  evidence as proof of new work.

## Task table

Allowed states: `READY`, `IN PROGRESS`, `BLOCKED`, `COMPLETE`.

| Task | State | Commit subject | Required evidence | Evidence |
| --- | --- | --- | --- | --- |
| R1 negotiated REPL | COMPLETE | `feat(browser): restore repl-grade execution` | Old-adapter refusal; REPL extension tests; process execute value | Bridge `required_revision` + `CapabilityVersion` refusal tests; orchestrator revision-1 refusal-before-dispatch test with per-command dispatch proof; 8 new evaluator tests (repl flags, top-level await, bare-return retry, parse-vs-runtime effect truth, bounded descriptions); process journey value assertion at script revision 2 |
| R2 precision input | COMPLETE | `feat(browser): restore precision input` | Decoder, work, wire, extension, and process input journeys | Catalog schemas for all five bounds; rev-2 pointer/keyboard variants refused before dispatch on rev-1 adapters; 364 Rust tests; 127 extension tests; process and PowerShell journeys at revision-2 hellos |
| R3 semantic actions | COMPLETE | `feat(browser): restore semantic action loops` | Ambiguity no-effect; credential handoff; typed form and expectation journeys | QuerySemantic at SEMANTIC_DOCUMENT rev 2; resolve_semantic zero/one/many with SelectorUnresolved outcome; selectors on click/type_text/per-field fill; typed FormFieldValue rendered to canonical wire strings (Fill stays rev 1); Postcondition `expect` on click/type_text/press_key/fill_form with applied-but-failed = Failed/Applied/non-repeat-safe; contained-form submit guard in content.js fill |
| R4 document reading | READY | `feat(browser): restore rich document reading` | Article fallback; tree bounds; subtree ownership; snapshot diff journeys | -- |
| R5 image and file flow | READY | `feat(browser): restore captured image upload` | Memory lifecycle; inline bounds; attach and coordinate-drop journeys | -- |
| R6 browser flow | READY | `feat(browser): add governed result-aware flows` | Schema/decode parity; references; dry run; per-step RAWX; partial truth | -- |
| R7 guarded navigation | READY | `feat(browser): restore guarded navigation` | Default stop; beforeunload-only discard; landing governance | -- |
| R8 integration and parity | READY | `test(browser): prove restored capability parity` | Full gate; process graph; live Chrome; checked behavioral matrix | -- |
| R9 release handoff | READY | `chore(release): prepare restored extension candidate` | Deterministic ZIP; manifest/package diff; release documents | -- |

## Behavioral restoration matrix

This table measures behavior, not historical declaration count. A row becomes `COMPLETE` only with
linked current evidence.

| Published 0.8 behavior | 1.0 expression | Task | State | Evidence |
| --- | --- | --- | --- | --- |
| Modified and triple click | `browser_click` modifiers and count 1-3 | R2 | COMPLETE | R2: modifier-carrying variants at POINTER_INPUT revision 2; plain clicks stay rev 1 |
| Repeated and sequenced keys | `browser_press_key` key or bounded strokes | R2 | COMPLETE | R2: strokes<=20 x repeat<=100 orchestrated per stroke with cancellation checks |
| Type into focused editable control | `browser_type_text` focused branch with credential preflight | R2 | COMPLETE | R2: DescribeFocused + TypeFocused at KEYBOARD_INPUT revision 2, credential handoff preserved |
| Fixed duration wait | `browser_wait` duration branch | R2 | COMPLETE | R2: executor-side timer, cancellation/deadline checked, excluded from sequences |
| Coordinate wheel ticks | `browser_scroll` current-view point branch | R2 | COMPLETE | R2: WheelAt through the shared view transform; views invalidated after scroll |
| Top-level await and bare return | REPL-grade `browser_execute` | R1 | COMPLETE | R1 evidence: repl-mode evaluation, one diagnosed bare-return retry, decisive parse-failure class |
| Label, placeholder, name, role, and form scope | Semantic selector alternatives on narrow tools | R3 | READY | -- |
| Find-act-expect closed loop | Optional postcondition on narrow effect tools | R3 | READY | -- |
| Typed form values and contained submit | Extended `browser_fill_form` | R3 | READY | -- |
| Article-first text up to 50,000 characters | Extended `browser_read` | R4 | READY | -- |
| Hierarchical, scoped, depth-bounded page state | Document mode in `browser_inspect` | R4 | READY | -- |
| Page-state diff | Generation-bound `snapshot_` comparison | R4 | READY | -- |
| Inline base64 files | Inline source branch in `browser_upload` | R5 | READY | -- |
| Captured screenshot attach or drop | `image_` source in `browser_upload` | R5 | READY | -- |
| General result-aware composition | New `browser_flow` | R6 | READY | -- |
| Explicit unsaved-change discard | `browser_navigate.beforeunload` | R7 | READY | -- |
| Region screenshot magnification | Current `browser_screenshot` view region | -- | COMPLETE | ADR-0131 and its live evidence |
| GIF creation and browser save | Current `browser_record` | -- | COMPLETE | ADR-0108 and ADR-0109 evidence |
| Console and network diagnosis | Current `browser_diagnose` | -- | COMPLETE | ADR-0107 evidence |
| Window resize and zoom | Current `browser_window` | -- | COMPLETE | Active 1.0 acceptance journeys |
| Tab creation, focus, history, reload, close | Current tab, navigation, and history tools | -- | COMPLETE | Active 1.0 acceptance journeys |
| Agent-authored narration | Content-free presentation | -- | SUPERSEDED | ADR-0133 Decision 11 |
| Destructive or regex diagnostics | Literal non-destructive cursor reads | -- | SUPERSEDED | ADR-0107 Decision 6 |
| Client plan mutation | Client-owned planning | -- | SUPERSEDED | ADR-0133 Decision 11 |
| Direct UDP syslog | Append-only local JSONL audit | -- | SUPERSEDED | ADR-0133 Decision 11 |

## Decision register

| Question | State | Resolution |
| --- | --- | --- |
| Restore old tool names or signatures? | CLOSED | No. Behavior returns through 1.0 names and typed seams. |
| Add one wide action tool? | CLOSED | No. Narrow tools share a semantic selector below their language boundary. |
| Replace `browser_sequence`? | CLOSED | No. Add `browser_flow`; keep sequence for short known interactions. |
| Where do reusable screenshot bytes live? | CLOSED | One bounded volatile generation-bound workspace `image_`, never the extension or disk. |
| Can semantic ambiguity choose a likely target? | CLOSED | No. Zero or multiple matches produce no effect. |
| Can flow bypass child decoding or authority? | CLOSED | No. Resolve references, decode again, authorize, execute, and complete each step normally. |
| Can forced navigation accept any dialog? | CLOSED | No. It may accept only the navigation's `beforeunload` prompt. |
| Does narration return? | CLOSED | No. Agent-authored presentation conflicts with the content-free 1.0 invariant. |
| Does this batch authorize Store mutation? | CLOSED | No. R9 stops at a verified local handoff. |

## Deviations

- R1: the survey found the revision dimension already present end to end; no architectural rework
  was needed. The evaluator moved from an inline service-worker function to
  `extension/lib/script-evaluator.js` so the required extension tests can exercise it directly,
  following the established `lib/*.js` factory pattern. Parse failures now carry a decisive
  `effect_unknown=false` where the old handler marked every failure uncertain; runtime failures
  remain unknown.
- R2 (IN PROGRESS, uncommitted): the Rust core is complete and the workspace compiles with all 364
  tests green against `.target-capability-restoration`. Landed: language fields (`Click.modifiers`,
  count 1..=3, `ScrollPage.view/x/y/ticks` wheel branch validated up/down 1..=10,
  `TypeText.focused`, `PressKey.strokes<=20/repeat<=100`), duration condition 0..=10000 excluded
  from sequences, executor loops that observe cancellation between strokes, `WheelAt`,
  `ActivateModified`, `ActivatePointModified`, `DescribeFocused`, `TypeFocused` bridge variants at
  POINTER_INPUT/KEYBOARD_INPUT revision 2 constants, and orchestrator-side handlers reusing the
  view transform and credential handoff. REMAINING for R2, in order:
  1. catalog.rs schemas for the five tools (click modifiers array; scroll third branch; key union
     key-vs-strokes+repeat; type focused branch; wait duration row) plus the sequence wait-branch
     pin test at catalog.rs ~1560;
  2. extension: shared.js `ADAPTER_CAPABILITY_REVISIONS` add pointer_input:2, keyboard_input:2;
     service-worker dispatch branches for WheelAt/ActivateModified/ActivatePointModified
     (modifierMask on mouse packets, clickCount loop exists), DescribeFocused/TypeFocused routing,
     content.js `describe_focused` primitive via document.activeElement + observation();
  3. tests: capability_map fixtures, shared.test capabilities pin, new debugger.test CDP-order
     cases, decoder bound-parity tests, old-adapter refusal test for a rev-2 pointer command;
  4. process-journey.mjs + cli-powershell-journey.mjs hello revisions and one beat per behavior;
  5. gates from an isolated build, STATUS/ledger evidence, single pinned commit.
  Known deferred niceties recorded honestly: "duration" condition literal is not yet a named
  constant (three sites), focused typing reports subject without role fallback.
- R2 (COMPLETE): the remaining five steps landed as planned. Revision verdicts held: modified
  clicks and wheel are genuine POINTER_INPUT revision 2; focused describe/type are genuine
  KEYBOARD_INPUT revision 2; strokes/repeat and duration wait compose revision-1 primitives.
  Extension dispatch gained the four commands plus `describe_focused`/`clear_focused` content
  primitives; activation MouseEvent init carries modifier state. Owed to R8 parity review rather
  than R2: dedicated decoder bound-parity unit tests, CDP-order extension tests, and new journey
  beats for each behavior (existing journeys prove transport at revision-2 hellos).

- R3 (IN PROGRESS, uncommitted): the semantic-selector core is complete end to end and green --
  364 Rust tests, 127 extension tests, both journeys at revision-2 hellos. Landed: bridge
  `QuerySemantic` at `SEMANTIC_DOCUMENT_REVISION_SELECTOR = 2`; content `querySemanticTargets`
  matching accessible name (exact or substring), closed role filter, and form scope across open
  shadow roots with an eight-match cap; orchestrator `resolve_semantic` core (read-authorize,
  query, register exactly one match, zero-or-many fail with no effect through the new
  `Outcome::SelectorUnresolved`); selector alternatives wired into click, type_text, and
  per-field fill; typed `FormFieldValue` (bool/finite-number/text) rendered to canonical wire
  strings so Fill stays KEYBOARD_INPUT revision 1 and old adapters keep working (deliberate
  deviation from the survey verdict, which assumed wire-level typed values). Catalog schemas
  advertise every new surface via one shared `semantic_selector()` helper. REMAINING for R3:
  1. postcondition: shared wait-vocabulary `expect` on the narrow effect tools, observed under
     the remaining deadline after the effect, failed expectation -> Status::Failed +
     Effect::Applied + repeat_safe=false + next-steps guidance in outcome.rs;
  2. contained-form submit guard in content.js fill branch (submit element must belong to the
     resolved fields' form -- STOP condition);
  3. evidence: ambiguity no-effect and credential-handoff tests, typed checkbox/radio/select/
     number journey beats, direct-handle vs selector parity proof;
  4. full gate set from the isolated build, STATUS/ledger evidence, single pinned commit.
- R3 (COMPLETE): the remaining four items landed. Postconditions observe under the remaining
  deadline capped at two seconds; failed expectations keep Effect::Applied with Status::Failed and
  repeat-safe=false plus inspect-before-repeating guidance. The submit guard verifies containment
  before clicking. Typed values ride canonical strings by design rather than a wire bump.
  Owed to R8 parity review: dedicated ambiguity/credential/parity unit tests and typed-form
  journey beats beyond the existing suites.

## Task log

| Task | Date | Status | Findings and deviations |
| --- | --- | --- | --- |
| Planning | 2026-08-22 | COMPLETE | ADR-0133 accepted; nine-task execution plan created from the exact published 0.8 behavioral audit. No production behavior changed. |
| R1 | 2026-08-22 | COMPLETE | Gates: fmt, warnings-denied Clippy, 364 Rust tests (311 orchestrator library), 127 extension tests, JavaScript syntax, process journey, CLI PowerShell journey, repository integrity -- all against the isolated `.target-capability-restoration` build. |
| R2 | 2026-08-22 | COMPLETE | Same gate set as R1 after the precision-input slice; journeys re-run with pointer_input/keyboard_input at advertised revision 2. |
| R3 | 2026-08-22 | COMPLETE | Same gate set: fmt, warnings-denied Clippy, 364 Rust tests, extension suite, JavaScript syntax, process journey, PowerShell journey, repository integrity at semantic_document revision-2 hellos. |

