# LEDGER: published capability restoration on 1.0 seams

Durable progress for [ADR-0133](../../adr/0133-behavioral-capability-restoration.md). Update this
file before work, after every material finding, and when a task completes or blocks.

## RESUME HERE

- State: READY. Planning is complete; no production restoration task has started.
- Next task: R1, [capability revisions and REPL-grade execute](R1-negotiated-repl.md).
- Implementation baseline: `dev` at
  `c8a181cc15e39b25b2cdc6864c8303efe345f561` before this batch's planning commit.
- Required first action: confirm the current head contains ADR-0133 and this batch, confirm no
  overlapping worktree changes, then execute R1 only.
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
| R1 negotiated REPL | READY | `feat(browser): restore repl-grade execution` | Old-adapter refusal; REPL extension tests; process execute value | -- |
| R2 precision input | READY | `feat(browser): restore precision input` | Decoder, work, wire, extension, and process input journeys | -- |
| R3 semantic actions | READY | `feat(browser): restore semantic action loops` | Ambiguity no-effect; credential handoff; typed form and expectation journeys | -- |
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
| Modified and triple click | `browser_click` modifiers and count 1-3 | R2 | READY | -- |
| Repeated and sequenced keys | `browser_press_key` key or bounded strokes | R2 | READY | -- |
| Type into focused editable control | `browser_type_text` focused branch with credential preflight | R2 | READY | -- |
| Fixed duration wait | `browser_wait` duration branch | R2 | READY | -- |
| Coordinate wheel ticks | `browser_scroll` current-view point branch | R2 | READY | -- |
| Top-level await and bare return | REPL-grade `browser_execute` | R1 | READY | -- |
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

None.

## Task log

| Task | Date | Status | Findings and deviations |
| --- | --- | --- | --- |
| Planning | 2026-08-22 | COMPLETE | ADR-0133 accepted; nine-task execution plan created from the exact published 0.8 behavioral audit. No production behavior changed. |

