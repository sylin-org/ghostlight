# LEDGER: published capability restoration on 1.0 seams

Durable progress for [ADR-0133](../../adr/0133-behavioral-capability-restoration.md). Update this
file before work, after every material finding, and when a task completes or blocks.

- State: BATCH COMPLETE. R1 through R9 are done. The exact handoff artifact is
  `dist/r8-candidate-run1.zip` (byte-identical to `run2`), built from source revision
  `3c820a98`. The pending Chrome Store review is stale against these bytes. NEXT OWNER ACTION,
  requiring explicit approval and no agent involvement: replace the Store draft with this ZIP via
  the Developer Dashboard (or the API sequence in `scripts/publish-extension.ps1`), which
  supersedes the staged review; then clear the extension-errors card noted in STATUS.

## RESUME HERE marker retained for provenance; no task remains.

- State: READY. R1 through R8 are complete; only R9 remains.
- Next task: R9, [release handoff](R9-release-handoff.md).
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
| R4 document reading | COMPLETE | `feat(browser): restore rich document reading` | Article fallback; tree bounds; subtree ownership; snapshot diff journeys | ReadDocument/InspectTree at SEMANTIC_DOCUMENT rev 3; article-first with visible fallback in content.js; depth/node-bounded structure-only tree (no values, hidden excluded, shadow-aware); SnapshotHandle superseded per tab with generation-stale resolution and closed-tab removal; bounded 50-path diff in facts; read ceiling 50,000 |
| R5 image and file flow | COMPLETE | `feat(browser): restore captured image upload` | Memory lifecycle; inline bounds; attach and coordinate-drop journeys | UploadFiles accepts exactly one of paths / inline base64 files / image_ handle; target-or-selector attach; view-point drop via DropImageAt at FILES rev 2; volatile image_ asset superseded per tab, refused above the 5 MB ceiling, generation-stale on take, removed on tab closure; inline validation in language plus strict decode after authorize and credential preflight |
| R6 browser flow | COMPLETE | `feat(browser): add governed result-aware flows` | Schema/decode parity; references; dry run; per-step RAWX; partial truth | browser_flow as tool 23 (catalog/annotations/capability-map/medallion pins swept); FlowStep ids unique 1..=20, composite tools forbidden, per-step restriction fields rejected; backward-only bounded JSON-Pointer refs validated at decode and resolved before the ordinary child decode re-runs via run(); children authorize under the immutable snapshot; on_error stop|continue; dry_run decodes with zero dispatch; 100 KB envelope budget omits rather than lies; aggregate Applied/Partial/Unknown truthfulness; decoder test covering every invalid-reference family |
| R7 guarded navigation | COMPLETE | `feat(browser): restore guarded navigation` | Default stop; beforeunload-only discard; landing governance | NavigateDiscardingBeforeUnload at NAVIGATION rev 2; extension accepts only Page.javascriptDialogOpening type=beforeunload while this navigation's acceptor is armed; discard flows through ordinary commit + landing governance; default navigation unchanged at rev 1 |
| R8 integration and parity | COMPLETE | `test(browser): prove restored capability parity` | Full gate; process graph; live Chrome; checked behavioral matrix | Checked `tests/capability-matrix.mjs` in repository integrity (21 COMPLETE + 4 SUPERSEDED rows, all evidenced; it caught ten stale rows on first run); process journey through every family incl. referenced flow, ambiguity refusal, wheel, inline/image upload, drop, guarded navigation; live Chrome lanes on the swapped dev authority: REPL, semantic click, article, tree+diff (execute mutation -> added:1 at /+1), wheel, image drop (23,550 bytes, subject named), guarded discard, referenced flow; two live-found defects fixed: inspect_tree wire encoding (object vs string) and flow on_error:stop continuing after a failed step | -- |
| R9 release handoff | COMPLETE | `chore(release): prepare restored extension candidate` | Deterministic ZIP; manifest/package diff; release documents | Two independent runs byte-identical: SHA-256 97bd48165ab9b0796a3ce93fcab063aa64f3e27977063b1b73409729bcae49a6, 89,441 bytes, 32 entries, v1.0.0 MV3, development key stripped, icons 16/32/48/128, source revision 3c820a98; permissions diff vs published 0.8 explained in the task log; artifact at `dist/r8-candidate-run1.zip` pending separate owner Store approval |

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
| Label, placeholder, name, role, and form scope | Semantic selector alternatives on narrow tools | R3 | COMPLETE | QuerySemantic at SEMANTIC_DOCUMENT rev 2; resolve_semantic zero/one/many with SelectorUnresolved no-effect outcome |
| Find-act-expect closed loop | Optional postcondition on narrow effect tools | R3 | COMPLETE | `expect` on click/type_text/press_key/fill_form; failed expectation keeps Effect::Applied, Failed, non-repeat-safe |
| Typed form values and contained submit | Extended `browser_fill_form` | R3 | COMPLETE | FormFieldValue bool/number/text to canonical wire strings; contained-form submit guard in content.js |
| Article-first text up to 50,000 characters | Extended `browser_read` | R4 | COMPLETE | ReadDocument at SEMANTIC_DOCUMENT rev 3; extractArticle with visible-text fallback; ceiling 50,000 |
| Hierarchical, scoped, depth-bounded page state | Document mode in `browser_inspect` | R4 | COMPLETE | InspectTree rev 3; structure-only tree, 12-depth/400-node caps, hidden excluded, shadow-aware |
| Page-state diff | Generation-bound `snapshot_` comparison | R4 | COMPLETE | register_snapshot superseded per tab, stale on commit, removed on close; bounded 50-path diff in facts |
| Inline base64 files | Inline source branch in `browser_upload` | R5 | COMPLETE | InlineFile validation in language + strict decode after authorize and credential preflight |
| Captured screenshot attach or drop | `image_` source in `browser_upload` | R5 | COMPLETE | One volatile image_ per tab beside the view; DropImageAt at FILES rev 2 through the view transform |
| General result-aware composition | New `browser_flow` | R6 | COMPLETE | Tool 23; backward-only refs re-decoded per child; dry run; stop/continue; 100 KB envelope budget |
| Explicit unsaved-change discard | `browser_navigate.beforeunload` | R7 | COMPLETE | NavigateDiscardingBeforeUnload at NAVIGATION rev 2; beforeunload-only acceptor armed per navigation |
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
- R4 (COMPLETE): article-first reading with visible-text mode and 50,000-character ceiling;
  document-scope inspect returning a bounded structure-only tree (400-node/12-depth caps, hidden
  content excluded, open shadow roots traversed, editable values never read) with a
  generation-bound snapshot_ handle superseded per tab, stale on commit, removed on tab closure;
  a bounded diff (added/removed/changed counts plus at most 50 structural paths) returned in
  facts when a current prior snapshot exists. Snapshots are volatile workspace state only.
  Owed to R8 parity review: dedicated snapshot lifecycle unit tests, Unicode/truncation tests,
  and the mutate-fixture diff journey beat.
- R5 (COMPLETE): three upload sources with exactly-one validation, inline base64 validated in
  language and strictly decoded only after authorize and credential preflight (STOP condition
  honored: deferred decode), one volatile image_ asset per workspace registered beside the view
  from the existing screenshot bytes and erased by supersession/tab closure/release with no new
  persistence, and a distinct DropImageAt physical command at FILES revision 2 retaining the full
  view_ stale-geometry protection. Names/paths/media types/bytes stay out of audit facts.
  Owed to R8 parity review: inline/duplicate/malformed bound unit tests, image ownership tests,
  and live attach/drop journey beats.
- R6 (COMPLETE): children dispatch through the existing run() seam under one invocation, lease,
  event stream, and completion gate -- no recursive execute(), no second lease. Per-step
  restrictions are rejected at decode (the flow's ceiling applies; tightening deferred by design
  and recorded here). Process-journey tool-count pins moved to 23 and both journeys pass.
  Owed to R8 parity review: per-step RAWX/audit/stale-handle unit journeys, stop-vs-continue
  aggregate tests beyond the decoder family, and the find-reference-read journey beat.
- R7 (COMPLETE): correlation is structural -- the acceptor exists only while this guarded
  navigation is in flight for this exact tab, and only `type === "beforeunload"` is ever accepted;
  alert, confirm, and prompt remain browser_dialog's domain. Discard reuses the ordinary commit,
  landing-authorize, hold-on-denial path unchanged. Owed to R8: dirty-form fixture journey and
  dialog-distinction extension tests.
- R8 (IN PROGRESS, uncommitted): the checked behavioral matrix now exists
  (`tests/capability-matrix.mjs`, wired into repository integrity) and immediately caught ten
  stale READY rows, which are now COMPLETE with evidence. The process journey exercises every
  R1-R7 family through the real executable graph: article/tree/diff reads, semantic click with an
  ambiguity refusal (selector_matched=2, none chosen), modified click, strokes x repeat, duration
  wait, inline upload with deferred decode, captured-image attach and view-point drop, coordinate
  wheel, guarded navigation, flow dry-run plus a referenced three-step flow over ordinary MCP
  text and structured content. Workspace snapshot/image lifecycle unit tests added; 23-tool pins
  swept across language, desktop, journey, LANGUAGE/ACCEPTANCE contracts. REMAINING for R8:
  1. live Chrome lanes -- BLOCKED on reloading the exact current unpacked extension bytes in the
     owner's Chromium (STOP condition forbids claiming live proof otherwise). Once reloaded, run:
     REPL execute, precision input, semantic form loop, article/tree read, inline+image upload,
     referenced flow, guarded navigation against the dirty-form fixture;
  2. remaining owed unit tests: CDP-order extension tests, inline bound family, per-step RAWX
     audit assertions;
  3. reconcile docs/0.8 recovery claims + final full-gate evidence capture into this ledger.
- R8 (COMPLETE): live lanes ran on the owner's daily Chrome through the exact-path development
  swap (orchestrator only; connector sources unchanged since the batch base, so both connectors
  rode through on reconnect per DEV-LOOP step 2). Live-proven: REPL execute (top-level await +
  bare return), semantic selector click with the none-chosen refusal, article read (120 words,
  iana.org), document tree with a real execute-mutation diff (added:1 at /+1), coordinate wheel,
  captured-image drop at a view point (subject receipt names the receiving element), guarded
  navigation discarding an armed beforeunload, and a referenced three-step flow. The stale-view
  refusal after wheel invalidation proved R5's protection live. Live lanes found and fixed two
  defects: inspect_tree encoded its tree as an object where the bridge requires a string, and
  flow on_error:stop set its flag but kept executing later steps. Image attach to a real file
  input remains process-journey-proven; example.com offers no file input for a live attach.
  Owed to R9 handoff notes: CDP-order extension unit tests and the dirty-form fixture journey
  stay listed as future hardening, honestly incomplete rather than silently dropped.
- R9 (COMPLETE): the deterministic packager (fixed timestamps, sorted entries, explicit
  allowlist, dev key stripped, forbidden-pattern guard) produced byte-identical ZIPs across two
  runs. Permission diff vs published 0.8 (tabs, debugger, scripting, nativeMessaging): added
  alarms (liveness/reconnect fallback), downloads (recording save destination), offscreen (GIF
  encode per ADR-0109), storage (adapter identity/preferences/journal), tabGroups (workspace
  grouping), webNavigation (landing observation), windows (dedicated placement); removed
  scripting (CDP via chrome.debugger replaced injection). Store justifications cover every
  permission one-for-one (repository integrity gate). No upload, review submission, or external
  mutation performed.

## Task log

| Task | Date | Status | Findings and deviations |
| --- | --- | --- | --- |
| Planning | 2026-08-22 | COMPLETE | ADR-0133 accepted; nine-task execution plan created from the exact published 0.8 behavioral audit. No production behavior changed. |
| R1 | 2026-08-22 | COMPLETE | Gates: fmt, warnings-denied Clippy, 364 Rust tests (311 orchestrator library), 127 extension tests, JavaScript syntax, process journey, CLI PowerShell journey, repository integrity -- all against the isolated `.target-capability-restoration` build. |
| R2 | 2026-08-22 | COMPLETE | Same gate set as R1 after the precision-input slice; journeys re-run with pointer_input/keyboard_input at advertised revision 2. |
| R3 | 2026-08-22 | COMPLETE | Same gate set: fmt, warnings-denied Clippy, 364 Rust tests, extension suite, JavaScript syntax, process journey, PowerShell journey, repository integrity at semantic_document revision-2 hellos. |
| R4 | 2026-08-22 | COMPLETE | Same gate set at semantic_document revision 3; journeys re-run green. |
| R5 | 2026-08-22 | COMPLETE | Same gate set with files at revision 2; journeys and repository integrity green. |
| R6 | 2026-08-22 | COMPLETE | Same gate set at the 23-tool catalog: fmt, warnings-denied Clippy, workspace tests including the new flow decoder family, extension suite, both journeys, repository integrity. A stale-binary journey failure was diagnosed to an unisolated rebuild and re-proven against `.target-capability-restoration`. |
| R7 | 2026-08-22 | COMPLETE | Same gate set at navigation revision 2; journeys and repository integrity green. |
| R8 | 2026-08-22 | COMPLETE | Full gate from `.target-capability-restoration`: fmt, warnings-denied Clippy, 7/7 workspace suites green, 127 extension tests, JavaScript syntax, process journey (extended through every family), PowerShell journey, capability matrix, repository integrity. Live: orchestrator swapped onto the daily-Chrome graph; every family exercised through real MCP calls; two live-found defects fixed and re-proven. |
| R9 | 2026-08-22 | COMPLETE | Two-run byte-identical package at source revision 3c820a98; manifest/allowlist inspection clean (no dev key, no tests/maps/sources); permission diff documented; repository integrity green including the capability-matrix gate; no Store mutation. |

