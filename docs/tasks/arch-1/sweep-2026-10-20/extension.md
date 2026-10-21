# Second sweep, 2026-10-20: extension and the small crates

This sweep read `extension/` (the worker, `lib/`, and the popup, options, setup and offscreen
pages) and the small crates `crates/edge`, `crates/relay`, `crates/bridge` and `crates/win-peer`,
read-only, on `dev` at 8f6a43ec. Two constraints shape the ranking. The 2.0.1 extension is in
Chrome Web Store review, so its packaged files are frozen
(`tests/golden/extension-2.0.0.files.sha256`, checked by
`tests/guards/extension-freeze.test.mjs`): changing one means resubmitting, which is the owner's
call. `crates/edge` and `crates/relay` are locked to relay revision 4
(`tests/guards/connectors.test.mjs`): a source change there needs a relay revision bump. The
findings are grouped as actionable now, locked (needs a relay revision bump) and frozen (needs a
store resubmission, so a later candidate), and ranked by payoff over risk within each group.
Ratings are payoff and risk (high, medium, low) and effort (S, M, L).

## EXT-1: The crate named bridge is a diagnostics sink

- Group: actionable now. Not frozen, not locked.
- Kind: naming and structure.
- Where: `crates/bridge/src/lib.rs:1-5` (the crate root: "The service's diagnostics.") and
  `crates/bridge/src/diagnostics.rs:69-72` (`Component`, whose only variant is `Orchestrator`).
  Its only dependent is `crates/orchestrator` (`crates/orchestrator/Cargo.toml:20`), which has its
  own `crates/orchestrator/src/diagnostics.rs`, the process-diagnostics hub that imports it.
  AGENTS.md:131 and 133 call the edge's two relays "the MCP bridge" and "the browser bridge".
  ADR-0199 Decision 3 (`docs/adr/0199-one-service-thin-shells.md:102-103`) already says "`bridge`
  keeps only the service's diagnostics until 2.x dissolves it". AGENTS.md:113 and 167 and
  `docs/1.0/ARCHITECTURE.md:30` describe the crate.
- Problem: "bridge" names a diagnostics sink with one consumer and a one-variant enum, while
  AGENTS.md uses the same word for the MCP relay and the browser relay, so it means three things.
  The service also carries two modules named `diagnostics`: its own and
  `ghostlight_bridge::diagnostics`.
- Change: move `diagnostics.rs` into `crates/orchestrator/src/diagnostics/` (for example as a
  `sink` module beside the hub), with its `notify` dependency, and remove the crate from the
  workspace. Replace the one-variant enum with a constant. Write "MCP relay" and "browser relay" in
  AGENTS.md:131,133, and update AGENTS.md:113,167 and `docs/1.0/ARCHITECTURE.md:30`. Keep the name
  `orchestrator`: it matches the role the docs give it, and a rename would churn paths across the
  ADRs and docs. The manifest change needs the workspace-wide checks and
  `node --test tests/guards/connectors.test.mjs`.
- Ratings: payoff medium, risk low, effort S.
- Verdict: partial. The code facts hold: one dependent (`crates/orchestrator/Cargo.toml:20`), a
  one-variant `Component` (`crates/bridge/src/diagnostics.rs:69-72`), and the AGENTS.md and
  ARCHITECTURE lines cited. Its group, "actionable now", does not hold. ADR-0199 Decision 3
  (`docs/adr/0199-one-service-thin-shells.md:102-103`) only defers the dissolution to 2.x and
  names no slice. ADR-0203 declined the fold (`docs/adr/0203-one-owner-for-each-fact.md:614-615`),
  left it for 2.x (`:723`) and kept the one-variant enum in I12 (`:1613`). The fold needs an
  ADR-0145 amendment, since Decision 4 puts the event names "in the bridge crate"
  (`docs/adr/0145-shared-process-diagnostics-log.md:197`). The imports sit in five files of the
  core and install lanes, and the service has a third `diagnostics` module, `cli/diagnostics.rs`.
- Class: refactor.
- arch-1: reopens Considered and not taken, "Folding the bridge crate (V-11)".

## EXT-2: win-peer repeats the relay's current-user SID FFI

- Group: actionable now. Not frozen, not locked (the relay source does not change).
- Kind: duplication.
- Where: `crates/win-peer/src/token.rs:104-131` (`ProcessToken::current()` and `user_sid()`),
  used at `crates/win-peer/src/private_file.rs:105` and `:161`. The relay already exports
  `ghostlight_relay::os::current_user_sid()` (`crates/relay/src/os/pipe.rs:168`, re-exported at
  `crates/relay/src/os/mod.rs:61`).
- Problem: the two audited unsafe surfaces each declare and call the same OpenProcessToken,
  GetTokenInformation and ConvertSidToStringSidW path for the current user's SID.
- Change: win-peer depends on `ghostlight-relay` and calls `current_user_sid()`. Keep only the
  elevation query in `token.rs`, and `sid_text`, which `private_file.rs` still needs for owner and
  access-entry SIDs. The edge's dependency tree is unaffected.
- Ratings: payoff low to medium, risk low, effort S.
- Verdict: confirmed. Both crates open the token, read its user and convert the SID
  (`crates/relay/src/os/token.rs:149-181,236-254`, `crates/win-peer/src/token.rs:104-132`), and
  `current_user_sid()` is public on Windows (`crates/relay/src/os/mod.rs:60-62`). Relay source and
  relay's dependencies stay as they are, so neither reason ADR-0203 gave against a merge applies.
  The saving is small: `user_sid`, `TokenUser` and `TOKEN_USER_CLASS`.
- Class: refactor.
- arch-1: touches Considered and not taken, "Merging the relay's token code with win-peer's
  (G2-03)", from the other side; I13 kept the two apart on that entry
  (`docs/tasks/arch-1/LEDGER-install.md:318-319`).

## EXT-3: The edge's two relays duplicate their plumbing

- Group: locked. Needs a relay revision bump.
- Kind: duplication.
- Where:
  - Identical `Service` traits: `crates/edge/src/forward.rs:87-93` and
    `crates/edge/src/port.rs:75-81`, implemented twice, identically, in
    `crates/edge/src/service.rs:24-42`.
  - The `Log` type: `forward.rs:112` and `port.rs:84`.
  - The start helpers: `forward.rs:645-650` (`start`) and `port.rs:427-433` (`start_service`).
  - The edge identity: `forward.rs:637` (an inline `Edge::new`) and `port.rs:517-519` (`edge()`).
  - The refusal detail: `forward.rs:369-371` (inline) and `port.rs:522-526` (`describe`).
  - The parent watchdog and session listener wiring: `crates/edge/src/main.rs:79-92` (`run_mcp`)
    and `main.rs:108-119` (`run_browser`).
- Problem: every fix to the relays' plumbing must be made twice. Only the start reason differs
  (`forward.rs:84` and `port.rs:72`), and `Service::start` already takes it as a parameter.
- Change: put one `Service` trait, `Log`, `edge()`, `describe()` and a `start(service, reason, log)`
  helper in `service.rs`, with `InstalledService` implementing the one trait once, and one helper
  in `main.rs` that starts the watchdog and the listener for a relay's event channel. The test
  files `forward/tests.rs` and `port/tests.rs` are outside the lock.
- Ratings: payoff medium, risk low, effort S.
- Verdict: partial. Every cited pair is in the tree as described, and only `START_REASON` differs
  (`forward.rs:84`, `port.rs:72`). "Risk low" misses what the bump costs. `RELAY` rides in every
  opening (`crates/relay/src/lib.rs:33`), and the service refuses `opening.relay < RELAY`
  (`crates/orchestrator/src/intake/listener.rs:114`). A bump alone would refuse every running
  2.0.x edge, so the service also needs its own floor of 4, as
  `docs/design/relay-protocol-4.md:213` promises, and each install replaces the edge on the
  revision change (`crates/orchestrator/src/install/deployment.rs:1116`). While the edge stays
  frozen, no plumbing fix is made twice anyway.
- Class: refactor, locked.
- arch-1: none. Decision 13 keeps relay and edge source out of every slice.

## EXT-4: The relay's docs still say connector

- Group: locked. Needs a relay revision bump.
- Kind: stale docs.
- Where: `crates/relay/src/lib.rs:38,41` (the docs of `HOLD_LIMIT_BYTES` and `HOLD_LIMIT`: "an MCP
  connector"), `crates/relay/src/frame.rs:5`, `crates/relay/src/route.rs:13,29,31` and
  `crates/relay/src/watchdog.rs:1,3-4,37`.
- Problem: ADR-0199 Decision 3 deleted both connector crates in 2.0, and the edge's own docs call
  these roles the MCP relay and the browser relay. (`crates/edge/src/port.rs:60` uses "a 1.x
  connector" correctly, for the 1.x program.)
- Change: say "MCP relay" or "browser relay", in the same bump as EXT-3.
- Ratings: payoff low, risk none, effort S.
- Verdict: confirmed. These are the ten "connector" lines in the relay and the edge, and the
  edge's own docs say MCP relay and browser relay (`crates/edge/src/main.rs:10-13`). A
  comment-only edit still trips the lock (`tests/guards/connectors.test.mjs:40,210-223`), so it
  can only ride a bump made for another reason.
- Class: refactor, locked.
- arch-1: none (Decision 13).

## EXT-5: protocol.js lists the protocol's words but names none

- Group: frozen. Needs a store resubmission.
- Kind: magic literals, and a vocabulary that crosses a process boundary spelled outside its
  domain module.
- Where: `extension/lib/protocol.js:19-67` exports lists only, so other files re-declare the words:
  - Receipt and call failure codes, in 11 files: `extension/worker.js:44`, `lib/arguments.js:20`,
    `lib/buffers.js:75`, `lib/capture.js:24-28`, `lib/evaluate.js:32`, `lib/input.js:35`,
    `lib/inventory.js:23-24`, `lib/primitives.js:42-52`, `lib/sessions.js:15,37`,
    `lib/tabs.js:39-48` and `lib/world.js:53-64`.
  - Frame and event kinds: `worker.js:45-68`, `lib/shared.js:46,95-99` and
    `lib/primitives.js:444`.
  - The dialog type "unknown", in four files: `worker.js:36`, `lib/sessions.js:36`,
    `lib/primitives.js:56` and `lib/world.js:69`.
- Cross-process words that no golden covers:
  - The recording stop reasons, split over `worker.js:43`, `lib/buffers.js:60-65` and
    `lib/recording.js:150,311`, and decoded by the service as `RecordingStopReason` in
    `crates/orchestrator/src/browser/adapter/chromium/protocol4/wire.rs`.
  - The `disconnected` runtime fact (`worker.js:42`, `lib/world.js:70`), which the page asset
    `crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/presentation.js:616` reads.
  - The command-chunk limits in `lib/chunks.js:8-12` (512 KiB per chunk, 8 MiB per transfer, 64
    chunks), which copy `wire.rs:29-33` (`CHUNK_PAYLOAD_BYTES`, `TRANSFER_MAX_BYTES`,
    `TRANSFER_MAX_CHUNKS`) by hand.
- Problem: protocol.js is the closed vocabulary, but it checks membership and names nothing, so
  every module mints its own constants, and the words outside the golden can drift between the
  service and the frozen extension unseen.
- Change: protocol.js derives frozen name maps from its lists (for example
  `protocol.RECEIPT_FAILURE.TAB_MISSING`) and owns the `failure(code, message)` builder. Nine
  modules (arguments, buffers, capture, evaluate, input, inventory, primitives, tabs and world)
  import `sessions.js` today only for its failure builders. Add the stop reasons, the
  `disconnected` fact and the three chunk limits to `tests/golden/protocol4/vocabulary.json`.
  `extension/tests/golden-protocol4.test.js:61` compares `protocol.vocabulary` with that file
  exactly, so the golden and protocol.js change together, and the service's `golden.rs:588`
  (`the_constants_and_limits_equal_the_vocabulary`) asserts the limits on its side.
- Ratings: payoff high, risk low, effort M.
- Verdict: partial. The copies are as counted: failure codes in 11 files, the frame and event
  kinds, "unknown" in four files, and nine modules that take only `failure`, `dialogOpen` and the
  `DIALOG_OPEN` pair from sessions.js. Corrections: protocol.js does name its scalars and limits
  (`extension/lib/protocol.js:79-91`); stop reasons are also literals at
  `extension/lib/recording.js:162,250,268`; and two chunk limits are already pinned to their
  values (`protocol4/wire.rs:2076-2077`, not in `golden.rs:588`), so only the chunk size is loose
  (`wire.rs:2078`). The chunk limits cannot go into `limits`:
  `extension/tests/golden-protocol4.test.js:110` pins that object whole against
  `vocabulary-2.0.0.json`, so they need a new key. The change edits `vocabulary.json`,
  additively, and leaves `vocabulary-2.0.0.json` alone. The pins need no package change: C18
  pins the stop reasons by reading the JavaScript, and the same read can pin `disconnected` and
  the chunk size. Only the name maps and the moved builder are frozen.
- Class: refactor, frozen.
- arch-1: X-09. Its crossing-name half is C18 (todo, `docs/tasks/arch-1/LEDGER-core.md:41`); its
  private-copy half is Considered and not taken, "The extension's private copies of
  `protocol.js` members", which the name maps reopen.

## EXT-6: The dispatcher is mixed with the handlers

- Group: frozen. Needs a store resubmission.
- Kind: structure.
- Where: `extension/lib/primitives.js:130-182` validates `call`, `input` and `frames`, whose
  runners live in `lib/world.js` (`callPrimitive`), `lib/input.js` (`run`) and `lib/inventory.js`
  (`framesPrimitive`), wrapped at `primitives.js:425-429`. `primitives.js:309-413` holds the dialog
  and accessibility handlers; its own doc says they are "small enough to live here"
  (`primitives.js:39-40`). `handler()` exists twice (`primitives.js:126-128`,
  `lib/tabs.js:312-314`), while `lib/capture.js:184-205`, `lib/evaluate.js:66-68` and
  `lib/buffers.js:191-193,502-505` build their specs inline. The input step vocabulary STEP is in
  both `primitives.js:68` and `lib/input.js:28`.
- Problem: a primitive's handler spec is built three ways, and a primitive's validation can live
  in a different module from its runner.
- Change: each module exports its own `{ validate, run, addresses, tabOf }` spec through one
  `handler()` in `lib/arguments.js`, the dialog and accessibility handlers move to the modules that
  own their CDP calls, and `primitives.js` keeps dispatch, cancel and the effect facts.
- Ratings: payoff high, risk low, effort M.
- Verdict: confirmed. Validators and runners sit apart as cited, `handler()` is in two files, five
  specs are built inline, and STEP is copied (`primitives.js:68`, `input.js:28`). "Payoff high" is
  generous for a change no receipt can see. Neither handler has an owning module to move to: the
  accessibility calls exist only here, and the dialog's is also sent at `tabs.js:545`.
- Class: refactor, frozen.
- arch-1: none. The STEP copy is one of X-09's private copies (Considered and not taken).

## EXT-7: worker.js reaches into the diagnostics and recording stores

- Group: frozen. Needs a store resubmission.
- Kind: duplicated logic between worker.js and a lib module.
- Where: `extension/worker.js:96` (`DIAGNOSTIC_DOMAINS`) and `worker.js:687-690`
  (`disableDiagnosticCapture`) repeat `lib/buffers.js:45` (`DOMAIN`) and `buffers.js:134-137`
  (`disableCapture`). `applyRuntimeState` (`worker.js:698-710`) and `releaseDebuggerSessions`
  (`worker.js:727-731`) re-implement `buffers.settleBoundaryState` (`buffers.js:605-609`) with other
  stop reasons, and the `onExpired` hook (`worker.js:236`) repeats its capture teardown.
- Problem: the worker's copies cannot clear the buffers module's read bindings
  (`buffers.js:130`), so the paths differ in what they settle, and the worker must know CDP domain
  names.
- Change: one `buffers.settle(reason)` used by all three paths and by `onExpired`. The worker then
  names no CDP domain.
- Ratings: payoff medium to high, risk low, effort S.
- Verdict: partial. The copies are real (`worker.js:96,236,687-690,698-710,727-731` against
  `buffers.js:45,134-137,605-609`). Corrections: the binding difference is close to inert, since
  after `clearAll` a stale binding at most makes a later read under another binding reset the
  ring once more (`buffers.js:143-149`); and the worker would still name CDP domains, because it
  routes Runtime, Network, Page and Input events by name (`worker.js:79-95`). `onExpired` is per
  tab, so it needs a per-tab teardown, not `settle(reason)`.
- Class: refactor, frozen.
- arch-1: none.

## EXT-8: tabs.js and capture.js re-implement the shared argument checks

- Group: frozen. Needs a store resubmission.
- Kind: duplicated helpers.
- Where: `extension/lib/tabs.js:59-77,87-89` (`invalid`, `isObject`, `requireTabId`,
  `requireText`, `requireArgs`) and `lib/capture.js:35-45` (`invalid`, `isObject`,
  `requireTabId`) repeat `lib/arguments.js`, with different messages: `tabs.js:68` says "must be a
  positive integer" where `arguments.js:44-56` says "must be a whole number of at least 1".
  `capture.js:53-55` (`requirePositiveInteger`) and `tabs.js:83-85` (`requireDuration`) are
  `requireInteger` with other messages. `requireTab` in `tabs.js:285-291` is the same as
  `lib/buffers.js:311-317`.
- Problem: one argument check has several spellings of its refusal, and a fix must be made in
  each copy.
- Change: use GhostlightArguments, which the worker already loads before both files, and keep one
  `requireTab`.
- Ratings: payoff medium, risk low, effort S.
- Verdict: confirmed. Each copy and each message difference is at the cited line, and the worker
  loads `lib/arguments.js` before both files (`worker.js:9`). The two `requireTab` differ only in
  the message bound (500 at `tabs.js:26`, 200 at `buffers.js:85`). One wording per check changes
  the advisory text of some `invalid_request` receipts; no golden or test pins it.
- Class: refactor, frozen.
- arch-1: none.

## EXT-9: CDP method names and Chrome error text are spelled per module

- Group: frozen. Needs a store resubmission.
- Kind: magic strings.
- Where: the allow list `extension/lib/sessions.js:19-30` sits beside eight per-module method
  tables (`lib/buffers.js:68-74`, `lib/capture.js:22`, `lib/evaluate.js:31`, `lib/input.js:20-27`,
  `lib/inventory.js:20`, `lib/primitives.js:62-66`, `lib/tabs.js:33-37` and `lib/world.js:41-51`)
  and the worker's event tables (`worker.js:79-95`). Some methods appear in two tables
  (`Page.getLayoutMetrics` and `Page.captureScreenshot` in `buffers.js:69-70` and `capture.js:22`;
  `Page.handleJavaScriptDialog` in `primitives.js:63` and `tabs.js:34`), and others are bare
  literals (`sessions.js:193-197,227,255,276,282`, and `Runtime.evaluate` in
  `lib/script-evaluator.js:98`, also listed in `world.js:45`). The Chrome error-text tables
  `primitives.js:73-81` and `world.js:74-82` have drifted: `world.js:79` lacks "Session with given
  id not found" and "debugger session was released", which `primitives.js:76` has.
- Problem: a method added to a module's table but not to the allow list is refused only when it
  runs, and the same Chrome error text maps to `session_lost` in a receipt but to `other`, with the
  cached world handle kept (`world.js:103-109`), in a call answer.
- Change: keep one CDP method table in `sessions.js`, the only path to CDP, whose values form the
  allow list, and one Chrome error classifier beside it, with the per-use flags world.js needs
  (`started`, `dropHandle`).
- Ratings: payoff medium, risk low, effort S.
- Verdict: partial. The tables, the shared methods and the bare literals are as cited, and
  `world.js:79` lacks both texts, but the gap misclassifies nothing a person or agent sees today.
  "debugger session was released" is the extension's own text from `ensureAttached`
  (`sessions.js:271,283,288`); world.js reaches that only through `acquire` at `world.js:485`,
  outside `mapCallError`, so it is already a `session_lost` receipt. Chrome's "Session with given
  id not found." does reach world.js, for an iframe session gone before its detach event, and
  answers `other`. The service reads `other` and `session_lost` alike
  (`protocol4/translate/mod.rs:114-120`, then `browser_primitive_failed` at `work/mod.rs:3244`);
  `started` is the same under both rows (true on the call path, though Chrome never sent the
  command; false at `world.js:393-396`); and the kept handle goes at the detach event
  (`sessions.js:212-214`, `world.js:593-597`). A real misread sits beside it, in a narrow race:
  `mapCallError` reads only `callCode`, so the coded `dialog_open` that `inventory.js:182` throws
  inside `worldHandle` and `ownerObject` (`world.js:228,248`) answers `other`, and the agent reads
  `browser_primitive_failed`, not `dialog_open` (`protocol4/link.rs:553-556`). One classifier has
  to honor a coded failure.
- Class: refactor, frozen (with coded failures honored, also defect and model-visible).
- arch-1: none. C12b (todo) types the receipt's `session_lost`; this drift shows only if call
  answers are typed the same way.

## EXT-10: The extension's own message kinds are spelled per file

- Group: frozen. Needs a store resubmission.
- Kind: magic strings (a vocabulary that crosses a JavaScript context boundary).
- Where: the page-to-worker kinds in `extension/worker.js:69-78`, `extension/popup.js:9-14`,
  `extension/setup.js:11-15`, `extension/lib/setup-view.js:25` and
  `extension/options.js:33,76-77,90,108`; the offscreen target, kinds and transfer forms in
  `worker.js:32`, `lib/buffers.js:76-82` and `extension/offscreen.js:43,58-63`; the command name
  "toggle-hold" in `worker.js:756`, `popup.js:16` and `extension/manifest.json:48`; and the intent
  "toggle_hold" as a literal in `worker.js:758`.
- Problem: each page re-spells the kinds it sends or receives, and options.js uses bare literals.
- Change: one shared vocabulary module (`lib/state.js` or a new `lib/messages.js`) loaded by the
  worker, the pages and the offscreen document.
- Ratings: payoff medium, risk low, effort S.
- Verdict: confirmed. Each page re-spells its kinds at the cited lines, and options.js uses bare
  literals. `manifest.json:48` cannot import a module, so "toggle-hold" keeps one copy there that
  only a test can tie to the code.
- Class: refactor, frozen.
- arch-1: none.

## EXT-11: Two registries wait for the same JavaScript dialog

- Group: frozen. Needs a store resubmission.
- Kind: convoluted flow.
- Where: `extension/lib/sessions.js:112,146-153,374-378` (the per-tab `dialogWaiters`,
  `answeredBeforeDialog` and `openDialog`) and `lib/world.js:163,287-309` (`dialogWaiters`,
  `watchForDialog` and `announceDialog`). The worker must call both, in order
  (`worker.js:820-821`). The guard "read `currentDialog`, then throw `dialogOpen()`" is repeated
  five times: `lib/capture.js:148`, `lib/evaluate.js:91`, `lib/input.js:208` and
  `lib/inventory.js:102,182`.
- Problem: one fact, an open dialog, has two subscriber lists, and the worker has to keep them in
  step.
- Change: `sessions.js`, which owns the dialog state, offers `watchDialog(tabId)` and
  `requireNoDialog(tabId)`; drop `world.announceDialog`, and world.js and input.js wait on the
  sessions watch.
- Ratings: payoff medium, risk medium (dialog races are subtle, and the world's watch resolves
  with the dialog type, which the sessions waiters do not carry today), effort M.
- Verdict: confirmed. Two waiter sets, the two calls the worker keeps in step
  (`worker.js:820-821`) and five guards, as cited. The sessions waiters are new (2f9c11f9,
  2026-10-20) and ship in the 2.0.1 package now in review, so risk medium is right.
- Class: refactor, frozen.
- arch-1: none.

## EXT-12: worker.js holds the native link's state machine

- Group: frozen. Needs a store resubmission.
- Kind: structure.
- Where: `extension/worker.js`, 895 lines. Sixteen mutable top-level variables
  (`worker.js:114-137`, `376-377`) run the hello, retry and welcome logic across
  `worker.js:393-683`. Error bounding, `shared.bounded(error?.message ?? error, ERROR_CHARS)`, is
  repeated five times (lines 162, 469, 470, 532 and 891). "Ghostlight service is disconnected." is
  written twice (lines 713 and 721). The badge red at line 213 duplicates `lib/state.js:28`
  (`BADGE_RECORDING`). Brand parsing (`worker.js:28-31`) sits apart from `shared.browserName`
  (`lib/shared.js:51-52`).
- Problem: the size is not the problem; the mixed duties are. The worker describes itself as the
  bootstrap and router (`worker.js:3`), but it also carries the connection state machine,
  vocabulary tables and store teardown.
- Change: after EXT-5, EXT-7 and EXT-10, move the native link (`worker.js:393-683`) into
  `lib/link.js` with one state object, and keep the worker as wiring and Chrome listeners. The
  worker tests drive it only through the port, the listeners and the message channel
  (`extension/tests/worker.test.js:3-5`), so they cover the move.
- Ratings: payoff medium, risk medium, effort M.
- Verdict: confirmed. 895 lines, 16 `let` bindings (14 at `worker.js:114-137`, two at
  `376-377`), five identical error bounds, two copies of the sentence, the badge red and the brand
  parsing are as cited. Four of the 16 are not link state (`browserId`, `life`, `preferences`,
  `persistChain`). The worker test loads lib files through `importScripts`
  (`worker.test.js:178-180`), so a `lib/link.js` stays under it.
- Class: refactor, frozen.
- arch-1: none.

## EXT-13: The evaluate primitive is split across two modules with a private vocabulary

- Group: frozen. Needs a store resubmission.
- Kind: dead code and a needless split.
- Where: `extension/lib/script-evaluator.js:76,89` invent the codes `invalid_script` and
  `primitive_failed`, which `lib/evaluate.js:33,102-106` translates back into protocol results.
  `effectUnknown` (`script-evaluator.js:77,90,100`) is read only by
  `extension/tests/evaluate.test.js`.
- Problem: one primitive has two modules and a private code vocabulary between them, and a flag
  that no production code reads.
- Change: fold `script-evaluator.js` into `evaluate.js`, throw protocol failures directly, and drop
  `effectUnknown`.
- Ratings: payoff low to medium, risk low, effort S.
- Verdict: confirmed. The private codes, their translation and the test-only `effectUnknown` are
  as cited, and h3 already says the protocol-4 receipt carries the uncertainty
  (`docs/tasks/security-hardening/h3-script-effect-truth.md:75-76`). Risk is a notch above low:
  the protocol-4 design kept this module unchanged on purpose
  (`docs/design/adapter-protocol-4.md:708`), and the h3 regressions in
  `extension/tests/evaluate.test.js:89-214` assert `effectUnknown`, so they must be rewritten to
  read `dispatched`.
- Class: refactor, frozen.
- arch-1: none.

## EXT-14: Lease and hidden-presentation brackets are written out by hand

- Group: frozen. Needs a store resubmission.
- Kind: repeated code.
- Where: 15 acquire/try/finally/release lease brackets in 8 modules (`lib/buffers.js` 4,
  `lib/primitives.js` 3, `lib/capture.js` 2, `lib/tabs.js` 2, and `lib/evaluate.js`,
  `lib/input.js`, `lib/inventory.js` and `lib/world.js` 1 each). The bracket that hides
  Ghostlight's presentation during a capture is the same in `lib/capture.js:224-253` and
  `lib/buffers.js:286-307`.
- Problem: each copy must pair acquire with release, and hide with show, by hand.
- Change: add `lifecycle.withLease(tabId, work)` and `world.withHidden(tabId, work)`.
- Ratings: payoff low to medium, risk low, effort S.
- Verdict: confirmed. 15 brackets in the eight modules as counted, and the two hidden brackets
  match. `withLease` does not fit two as they stand: evaluate.js leases only once the script
  parses and releases only if it leased (`evaluate.js:88-112`), and the dialog status leases only
  when no dialog is known (`primitives.js:311-321`).
- Class: refactor, frozen.
- arch-1: none.

## EXT-15: Module docs

- Group: frozen (needs a store resubmission), except the README.
- Kind: missing and stale docs.
- Where: `extension/lib/chunks.js` has no module doc. `extension/lib/protocol.js:8` and
  `extension/lib/sessions.js:8` put theirs below the wrapper instead of opening the file.
  `extension/README.md` lists the files but omits `lib/popup-view.js`.
- Change: open each module with its doc comment. The README is not packaged, so fix it now.
- Ratings: payoff low, risk none, effort S.
- Verdict: partial. `chunks.js` has no comment at all, and protocol.js and sessions.js open with
  the wrapper. The README's file list (`extension/README.md:12-38`) also omits
  `lib/arguments.js`, `vendor/gifenc.js`, `kit.css` and `options.css`, not only
  `lib/popup-view.js`.
- Class: refactor, frozen (the README is neither).
- arch-1: none.

## Overall

The Rust crates are disciplined: relay names its constants and documents every module. Their debt
is the edge's twin relays (EXT-3) and one misnamed crate (EXT-1). The six copies of the test
`Scratch` helper are the price of the relay lock and the no-extra-crate rule, so leave them. The
extension's modules are tidy one by one, but they share vocabulary by copying it, because
protocol.js checks membership and names nothing. The change that helps most is EXT-5. One named
vocabulary and failure builder in protocol.js removes most of the magic strings and takes
sessions.js out of nine imports. It also prepares EXT-6, EXT-7 and EXT-12 to go out in the same
resubmission.

## Skeptic summary

- Verdicts: 9 confirmed (EXT-2, EXT-4, EXT-6, EXT-8 and EXT-10 to EXT-14), 6 partial (EXT-1,
  EXT-3, EXT-5, EXT-7, EXT-9 and EXT-15), none refuted.
- Seams: no change adds, removes or renames an adapter protocol 4 word or field. EXT-9 changes
  which closed code some call answers carry, and EXT-8 the advisory text of some
  `invalid_request` receipts. Only EXT-5 edits `vocabulary.json`, additively, and none edits
  `vocabulary-2.0.0.json`. EXT-3 and EXT-4 move the relay revision every opening carries.
- First three:
  1. EXT-5's pins, now, through C18 and with no package change. A service that raised its chunk
     size past the frozen extension's 512 KiB would have every chunked command refused, and no
     test stops that today.
  2. EXT-9, first in the next package, widened so one classifier honors coded failures. It fixes
     the one real misread found here, a `dialog_open` read as `browser_primitive_failed`. A test
     that is not packaged can check every CDP method literal against `CDP_ALLOW` now.
  3. EXT-2, now. It removes a second copy of the user-SID FFI from an audited unsafe surface at no
     seam cost; the install ledger should record it against G2-03.
- Later: EXT-15's README lines can ride any docs commit. EXT-1 needs an ADR-0145 amendment and
  V-11 reopened first. EXT-3 and EXT-4 wait for a `RELAY` bump made for another reason. The other
  frozen findings wait for the next extension revision.
