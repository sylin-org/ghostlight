# SPEC: one module owns what Ghostlight says happened

**Status: executed, 2026-08-11, in `9609fcfc`. This document is now history.** The durable decision
record is [ADR-0103](../../adr/0103-language-owned-outcome-voice.md); current state is in
[STATUS.md](../../STATUS.md); the execution log and its findings are in [LEDGER.md](LEDGER.md).
Nothing here is pending work. The sentence and projection tables below stayed the oracles and are
preserved as authored, so a later reader can see what was pinned before the code existed.

It was authored as a batch spec for unattended execution but was executed in one pass, so this
directory has no BOOTSTRAP. Where this SPEC and the live tree disagree now, the tree wins.

## 1. Problem

Every sentence Ghostlight speaks about a completed action is an inline string literal inside
`crates/orchestrator/src/work/mod.rs`, a 3,700-line executor. Consequences, all live today:

- The sentences cannot be reviewed as a set, so they do not share a voice. `Found 7 matches.` sits
  beside `Page opened and its landing was governed.`
- "What does Ghostlight say when a form is filled?" is answered by grepping string literals inside
  the executor.
- `language/` owns what Ghostlight *accepts*. What Ghostlight *says* lives in `work/`. The
  model-facing boundary is split across two modules for no reason but history.
- The same number is computed twice. `word_count(&text)` is called once for the sentence and once
  for the observation. They agree today because someone was careful. Careful rots.

## 2. Decision

One module, `crates/orchestrator/src/language/outcome.rs`, owns the product's account of an action
in both registers: the sentence a person reads and the measurement a machine queries.

- `Outcome` -- what a completed action did. Renders `summary()`, `next_steps()`, `observed()`.
- `Refusal` -- why work did not complete. Renders `summary()`, `next_steps()`.
- `Observed` -- the measurements. Moves here from `governance`, unchanged in shape.

### 2.1 The seam and the voice divide cleanly

This batch does not weaken the observation seam decided in `docs/design/action-observations.md`. It
sharpens the split:

- **The seam** (`Executor::dispatch`) records facts about crossing the browser: **host and
  readiness**. It stays exhaustive over `BrowserOutcome`, so a tool written tomorrow is observed
  without anyone remembering to observe it.
- **The voice** (`Outcome`) records facts about the work: **counts and sizes**, plus the host when
  its sentence names one. It cannot be forgotten either, because `succeeded` will not compile
  without an `Outcome`.

They touch disjoint fields. `finish` merges the outcome's observation over the seam's. Nothing is
computed twice, and neither guarantee depends on memory.

### 2.2 Whatever the sentence names, `observed()` carries

If a sentence says `example.com`, `observed().host` is `Some("example.com")`. If it says
`3 fields`, `observed().count` is `Some(3)`. This is the invariant T1 tests.

### 2.3 A sentence names the host only when it measured nothing

`Filled 3 fields.` does not name a host; the seam supplies it to the record anyway.
`Opened example.com.` names one, because it has nothing else to say.

This is why the surface can stop guessing. Today `app.js` decides whether the sentence already
states a measurement (`measured()`) in order to choose between the host and the sentence. After this
batch the orchestrator has already made that decision, and the row simply renders the sentence.

## 3. The vocabulary (oracles -- transcribe, never derive)

`place(host, fallback)` renders `host` when present, else `fallback`.
`counted(n, singular, plural)` renders `1 field` / `3 fields`, with the number grouped by 3.
`grouped(n)` renders `1240` as `1,240`.

### 3.1 `Outcome::summary()`

| Variant | Sentence |
| --- | --- |
| `TabsListed { count }` | `Listed {counted(count, "controlled tab", "controlled tabs")}.` |
| `TabActivated { host }` | `Brought {place(host, "the controlled tab")} into view.` |
| `PageOpened { host }` | `Opened {place(host, "the requested page")}.` |
| `PageNavigated { host }` | `Navigated to {place(host, "the requested page")}.` |
| `HistoryTraversed { direction, host }` | `Went {direction} to {place(host, "the previous page")}.` |
| `PageReloaded { host }` | `Reloaded {place(host, "the page")}.` |
| `TabClosed` | `Closed the controlled tab.` |
| `TextRead { words }` | `Read {counted(words, "word", "words")}.` |
| `TargetsListed { noun: Match, count }` | `Found {counted(count, "match", "matches")}.` |
| `TargetsListed { noun: Item, count }` | `Inspected the page and found {counted(count, "item", "items")}.` |
| `Captured { full_page, width, height }` | `Captured the {"full page" or "viewport"} at {width}x{height}.` |
| `TargetClicked { host }` | `Clicked a target on {place(host, "the page")}.` |
| `PointClicked { host }` | `Clicked a point on {place(host, "the page")}.` |
| `PageScrolled { host }` | `Scrolled {place(host, "the page")}.` |
| `TargetRevealed { host }` | `Revealed a target on {place(host, "the page")}.` |
| `ZoomSet { percent, host }` | `Set zoom to {percent}% on {place(host, "the page")}.` |
| `Hovered { host }` | `Hovered a target on {place(host, "the page")}.` |
| `FormFilled { fields, submitted: false }` | `Filled {counted(fields, "field", "fields")}.` |
| `FormFilled { fields, submitted: true }` | `Filled {counted(fields, "field", "fields")} and submitted the form.` |
| `TextTyped { host }` | `Typed text on {place(host, "the page")} through browser input events.` |
| `KeyboardSent { host }` | `Sent a keyboard action to {place(host, "the page")}.` |
| `Dragged { host }` | `Completed a drag on {place(host, "the page")}.` |
| `FilesUploaded { count }` | `Uploaded {counted(count, "file", "files")}.` |
| `ScriptEvaluated { host }` | `Evaluated a script on {place(host, "the page")}.` |
| `Waited { condition, elapsed_ms, satisfied: true }` | `Wait condition {condition} was satisfied after {elapsed_ms} ms.` |
| `Waited { condition, elapsed_ms, satisfied: false }` | `Wait condition {condition} was not satisfied within {elapsed_ms} ms.` |
| `SequenceRan { completed, total }` when `completed == total` | `Ran {counted(total, "step", "steps")}.` |
| `SequenceRan { completed, total }` otherwise | `Stopped at step {completed + 1} of {total}.` |
| `DialogHandled { accepted: true }` | `Accepted the browser dialog.` |
| `DialogHandled { accepted: false }` | `Dismissed the browser dialog.` |

Worked examples, exact: `Listed 4 controlled tabs.`, `Listed 1 controlled tab.`, `Read 1,240 words.`,
`Read 5 words.`, `Read 1 word.`, `Opened example.com.`, `Opened the requested page.`,
`Went back to example.com.`, `Captured the viewport at 1280x720.`, `Filled 3 fields and submitted
the form.`, `Stopped at step 3 of 5.`, `Ran 5 steps.`,
`Wait condition load_ready was satisfied after 1830 ms.`

### 3.2 `Outcome::next_steps()`

`Waited { satisfied: false, .. }` returns
`vec!["Inspect the current page before choosing another action.".into()]`.
Every other variant returns `vec![]`.

### 3.3 `Outcome::observed()`

| Variant | Projection |
| --- | --- |
| `TabsListed { count }` | `count` |
| `TextRead { words }` | `count` |
| `TargetsListed { count, .. }` | `count` |
| `FormFilled { fields, .. }` | `count` = fields |
| `FilesUploaded { count }` | `count` |
| `Waited { elapsed_ms, .. }` | `count` = elapsed_ms |
| `SequenceRan { completed, .. }` | `count` = completed |
| `Captured { width, height, .. }` | `width`, `height` |
| every variant carrying `host` | `host` |
| `TabClosed`, `DialogHandled` | `Observed::default()` |

Counts saturate into `u32` (`u32::try_from(value).unwrap_or(u32::MAX)`), matching the existing
`measured` helper in `work/mod.rs`.

### 3.4 `Refusal::summary()` and `next_steps()`

Sentences are the current strings verbatim. Do not reword them in this batch.

| Variant | Sentence | Next steps |
| --- | --- | --- |
| `InvalidRequest` | `The call does not match the Ghostlight catalog.` | `Correct the call using the advertised tool schema.` |
| `CancelledBeforeStart` | `The browser job was cancelled before it started.` | none |
| `DeadlineBeforeStart` | `The browser job deadline expired while waiting for the workspace.` | none |
| `AuthorityBlocked` | `Authority blocked the browser job.` | none |
| `AttentionRequired` | `The browser job requires user attention.` | none |
| `LocalInterlock` | `A local browser safety setting blocked this action.` | `The user can change the relevant Ghostlight extension setting or perform the action directly.` |
| `CredentialHandoff` | `A credential-class field requires user handoff in the visible browser.` | `Complete the credential field in the visible browser, then inspect the page again.` |
| `IncompatibleReceipt` | `The browser adapter returned an incompatible primitive receipt.` | none |
| `BrowserStopped { reconnect: true }` | `The browser job stopped before a physical effect.` | `Reconnect the Ghostlight browser adapter.` |
| `BrowserStopped { reconnect: false }` | same sentence | none |
| `EffectUnknown` | `A browser effect was dispatched, but its final state cannot be determined.` | none |
| `LandingDeniedUnknown` | `The landing was denied, but the new tab's final state cannot be determined.` | none |
| `WorkspaceUnusable { reason }` | `The requested workspace target is not currently usable.` | `reason.next_steps()` |
| `FilesUnreadable` | `The selected local files could not be prepared safely.` | none |
| `CaptureTooLarge` | `Screenshot exceeded the product result bound.` | none |
| `NoDialogVisible` | `No JavaScript dialog is currently visible.` | none |

### 3.5 `WorkspaceReason`

| Variant | `as_fact()` | `next_steps()` |
| --- | --- | --- |
| `TabUnavailable` | `tab_unavailable` | `Call browser_list_tabs to obtain current controlled tab handles.` |
| `StaleTarget` | `stale_target` | `Call browser_inspect_page or browser_find to obtain current target handles.` |
| `StaleView` | `stale_view` | `Call browser_take_screenshot to obtain a current view handle.` |
| `TabHeld` | `tab_held` | none |
| `WorkspaceBusy` | `workspace_busy` | `Wait for the active Ghostlight invocation to finish.` |
| `OwnershipMismatch` | `ownership_mismatch` | none |
| `WorkspaceClosed` | `workspace_closed` | none |

`WorkspaceError` maps to `WorkspaceReason` exactly as the current `workspace_failure` match does:
`StaleTab | NoTab | AmbiguousTab` -> `TabUnavailable`; `StaleTarget` -> `StaleTarget`;
`StaleView | ViewPointOutOfBounds` -> `StaleView`; `Held` -> `TabHeld`; `Busy` -> `WorkspaceBusy`;
`NotOwnedTab | NotOwnedTarget | NotOwnedView | TargetTabMismatch | ViewTabMismatch |
PhysicalTabOwned` -> `OwnershipMismatch`; `UnknownWorkspace` -> `WorkspaceClosed`.

## 4. What does not change

- No `facts` key, no `status`, no `effect`, no `readiness`, no `repeat_safe`, no capability
  classification, and no governance decision changes anywhere in this batch. Only Ghostlight-authored
  sentences, next steps, and where the observation's counts come from.
- The audit record stays payload-free. The host remains the only page-derived value, path/query/
  fragment never appear, and `InvocationResult::facts` is untouched.
- `Observed` keeps its five fields and its JSON shape. Moving the type changes no serialized byte.

## 5. Provenance (decided; do not re-litigate)

- **Why not per-tool fact reporting?** Rejected in `docs/design/action-observations.md`: correctness
  kept by memory rots. The seam keeps host and readiness for exactly that reason. The voice is safe
  from the same failure only because `succeeded` cannot be called without an `Outcome`.
- **Why does `Outcome` live in `language/` and not `work/`?** `language/` already owns the
  model-facing boundary in the input direction. This completes it in the output direction. It also
  removes a dependency question: `governance` already depends on `language`, so moving `Observed`
  into `language/outcome.rs` adds no module edge, while defining `Outcome` in `governance` or
  `work` would.
- **Why did `Observed` move at all?** So the sentence and its measurement are produced from one
  value in one place. That is the whole point of the batch.
- **Why keep refusal wording unchanged?** Refusal sentences were already reviewed and their reason
  codes carry the specifics. Rewording them would enlarge a mechanical batch with taste questions.
- **Why is the host absent from counted sentences?** Section 2.3. It keeps rows short and lets the
  surface stop deciding what the orchestrator already knows.
- **Naming a click target, a typed value, or a pressed key is not available.** Roles and labels are
  page content, and a keystroke is model input. Neither may enter a summary, because summaries enter
  audit. This is why those sentences name only the host.
