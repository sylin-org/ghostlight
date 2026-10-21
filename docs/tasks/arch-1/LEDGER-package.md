# arch-1: package ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

B2 and B1 have landed, and so has the top-frame diagnostics ring reset for unbound tabs (see
Notes). B1 rides in the same candidate as C21, or neither does.

The second sweep (2026-10-20) opens this lane again: see its section below. It started while
adapter waits for C17 (the coordinator's direction). P-D1, EXT-7, EXT-8, EXT-13, EXT-14, EXT-15,
EXT-9, EXT-10, EXT-11, C18, P-D2 and EXT-5 have landed, and so have P-D3, the owner's group title,
P-D4 and P-D5 (the coordinator, 2026-10-21). Next: EXT-6 (each module exports its own
handler spec through one `handler()`), then EXT-12, then B-final. B-final regenerates the freeze
record from the release workflow's candidate zip, not a test-02 package, and stops there: the
store cancel and resubmission are the owner's call. Parked 2026-10-21 for the night with nothing
open: no patch, no stash, the tree clean.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| B2 | fix(extension): custody follows the service that granted it, and an outdated extension says so | candidate | ux-2 closes | package items 2, 3, 4 and 7; see Notes | landed (c79aef1b) |
| B1 | feat(extension): the browser names its life | candidate | ux-2 closes | package item 1; lands with C21; see Notes | landed (this commit) |

## Notes

- B2, cross-lane: the extension no longer reports `opener_tab_id`, so the slice also removes the
  field from `protocol4/wire.rs`'s `Tab` and from `tests/golden/protocol4/receipts/list_tabs.ok.json`
  (adapter lane). Nothing in the service read it. `docs/design/adapter-protocol-4.md` still names
  the field; D2 takes it out. A `list_tabs` that names tabs still lists the tabs they opened, as the
  wire's doc says; only the reported opener goes.
- B2, `omitted_count`: `allowNetworkUrl` goes, and the receipt keeps the field at 0, because the
  service adds what its own policy withholds to it.
- B2, the diagnostics ring: the reset at a navigation now needs the top frame. The reset at a
  changed binding stays, because the binding is the authority's say (A2 sends one only when it
  watches documents).
- B2, the snapshot fields nothing read: `adapter_version`, `browser_id`, `service_version`,
  `unseen_denials`, `last_error` and `service_outdated` leave the popup snapshot, and with
  `last_error` the worker's own "Update Ghostlight on this computer." words. The snapshot gains
  `adapter_refusal`, the sentence the service sent with `adapter_incompatible`. The popup and the
  options page show it under "Update Ghostlight in Browser.", and the setup page shows it as the
  lead, so the update sentence keeps one owner, `language::adapter::ADAPTER_INCOMPATIBLE`.
  With `last_error` gone nothing shows a raw connection error, so the options page now says only
  that Developer diagnostics keeps connection details for this browser (review fixup).
- B2, the retry: Decision 3 says the extension stops restarting the native host until a hello is
  accepted; the Rebuild plan says it takes the delayed retry. The slice follows the plan: after a
  refusal the relay's exit no longer reconnects at once, and the 3 s retry keeps trying, so a
  service that is put back to a compatible build heals the link with no person involved.
- B2, extras: the connection log gains an `adapter_refused` event, so an exported diagnostics file
  shows the refusal. `state.js` gains the module doc comment it lacked. The controlled-tab hook loses
  `managedIds`, which only the opener computation called.
- B2, the error golden: `tests/golden/protocol4/frames/error.json` still carries an older sentence
  than `language::adapter::ADAPTER_INCOMPATIBLE`. It is the adapter lane's file and no test compares
  the two, so the slice leaves it.
- B2, journeys for the coordinator: none was written or run here. On a test machine, a service
  crash or restart while an agent holds tabs should leave no debugging bar on them once the new
  service answers, and a worker restart against the same service should keep them bound.
  `tests/golden/agents/native/linux.txt` is unchanged.
- Landed (this commit), from A2's review: a top-frame navigation must start a fresh diagnostics
  ring for an unbound tab too. A2 sends a binding only when the scope watches documents, so in the default
  all-open mode no tab is bound, and `beforeNavigate` in `lib/buffers.js`, which resets only when
  it deletes a binding, keeps the prior page's console and network entries. Decision 6.3 keeps the
  top-frame reset for every tab. The fix: a top-frame `beforeNavigate` forgets and re-enables the
  ring of any tab with diagnostics capture, bound or not, and deletes the binding if there is one.
  Prove it with an unbound read in `worker.test.js` or `buffers.test.js`: a subframe navigation
  keeps the entries and a top-frame navigation clears them. It changes a packaged file, so the
  same commit regenerates `tests/golden/extension-2.0.0.files.sha256` (ADR-0200 Decision 16). A2's
  candidate gate waited for it; see LEDGER-adapter.md. The proof is in both places: the worker's
  navigation test runs bound and unbound, and `buffers.test.js` pins the unbound reset.
- B1, the life: `state.js` holds `LIFE_KEY` (`ghostlight.life`) and `newLife`, beside the browser
  id's key and minter, and the life reads `life_` plus 32 hex digits. `initializeLocalState` reads it
  with the rest of session storage and mints it only when none is there. A failed write of a fresh
  life still sends it: the next worker then mints another, which proves nothing, so the URL decides.
- B1, the hello golden: `tests/golden/protocol4/frames/hello.json` is the adapter lane's fixture and
  C21 adds `life` to it. The worker test therefore compares the hello to the fixture with the life
  taken from the hello, and passes before and after C21. Until C21 lands the service ignores the
  field under its tolerant reader (`wire.rs`), so B1 alone changes no behavior.
- B1, docs: `extension/README.md` says what the hello carries and what the life proves. The
  adapter-protocol-4 design gains `life` in D2.
- B1, journeys for the coordinator: none was written or run here. With C21 on a test machine, a
  worker restart (`chrome://serviceworker-internals` stop) and a service replace should keep a
  single-page app's tab bound, and a browser restart should leave its handle "no longer open".
  `tests/golden/agents/native/linux.txt` is unchanged.

## The second sweep (2026-10-20)

Rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Machine: test-02 (Bluefin), after adapter. A
slice's specification is its finding in [sweep-2026-10-20/extension.md](sweep-2026-10-20/extension.md), read
with its skeptic's corrections. P-D1, EXT-7, EXT-8, EXT-13, EXT-14, EXT-15, EXT-9, EXT-10, EXT-11, C18, P-D2, EXT-5, P-D3, P-D4 and P-D5 have landed; EXT-6 is next.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| P-D1 | fix(extension): a dialog that opens during world setup reads `dialog_open` | - | EXT-9 as corrected | landed (this commit) |
| C18 | refactor(presentation): one table of activity and denial words, with the crossing names pinned | - | ADR-0203 C18, moved from core-service; SURF-4's label half; EXT-5's pins | landed (this commit) |
| P-D2 | fix(extension): a command the service cancelled while it waited never runs late | - | the coordinator, from leo-desktop-02's 2.0.0 stall | landed (this commit) |
| EXT-5 | refactor(extension): `protocol.js` names its words and owns `failure`, and the vocabulary gains the missing words under new keys | C18 | EXT-5 as corrected | landed (this commit) |
| EXT-6 | refactor(extension): each module exports its own handler spec through one `handler()` | EXT-5 | EXT-6 | todo |
| EXT-7 | refactor(extension): one `buffers.settle(reason)` for every boundary | - | EXT-7 | landed (this commit) |
| EXT-8 | refactor(extension): the argument checks come from `arguments.js` | - | EXT-8 | landed (this commit) |
| EXT-9 | refactor(extension): one CDP method table and one session-lost classifier | P-D1 | EXT-9 | landed (this commit) |
| EXT-10 | refactor(extension): one module of the extension's own message words | - | EXT-10 | landed (this commit) |
| EXT-11 | refactor(extension): sessions owns the dialog watch | EXT-9 | EXT-11 | landed (this commit) |
| EXT-13 | refactor(extension): `script-evaluator.js` folds into `evaluate.js` | - | EXT-13 | landed (this commit) |
| EXT-14 | refactor(extension): `withLease` and `withHidden` | - | EXT-14 | landed (this commit) |
| EXT-12 | refactor(extension): the native link moves into `lib/link.js` | EXT-5, EXT-7, EXT-10 | EXT-12 | todo |
| EXT-15 | docs(extension): module docs, and the README lists every file | - | EXT-15; the README is not packaged | landed (this commit) |
| P-D3 | feat(extension): a group's title is the ghost and the agent's name, and a 2.0.0 group is retitled | - | the owner's request of 2026-10-21 (Notes); done by the coordinator while the lanes were parked, cross-lane into `language/provenance.rs` | landed (this commit) |
| P-D4 | fix(extension): a text limit counts characters, as the service does | - | found while doing P-D3 (Notes); the coordinator | landed (this commit) |
| P-D5 | fix(browser): half an emoji in page text no longer drops the browser connection | - | the coordinator's live check of P-D4's note (Notes) | landed (this commit) |
| P-D6 | fix(extension): no frame over the relay bound leaves the extension, and an oversized answer says so | P-D5; core-service S-D8 | test-02's answer to the coordinator's question, 2026-10-21 (Notes); cross-lane into the adapter's receipt words and `language/` | todo |
| B-final | chore(extension): the freeze golden for one new 2.0.1 submission | all above | cancelling the staged review and submitting are the owner's go | todo |

### Notes (second sweep)

- The freeze record: every package slice changes a packaged file, so each one regenerates
  `tests/golden/extension-2.0.0.files.sha256` in its own commit, as the freeze guard requires,
  and the tree stays green. Its zip line is then the hash of a zip packaged on test-02
  (`scripts/package-extension.ps1` under PowerShell 7.6.7), not of a store artifact. The same
  tree packages to other zip bytes here than in the release workflow: the 2.0.1 tree gives
  cfa6226d on test-02, while the record and the candidate bundle of run 39517667200 hold
  bddfb37a. B-final records the zip of the release workflow's candidate, the artifact a
  submission uploads.
- P-D1, the defect: a dialog that opens while a world is set up makes `inventory.currentLoader`
  throw the coded `dialog_open` of `sessions.failure`, inside `worldHandle` or `ownerObject`.
  `mapCallError` read only `callCode`, so the answer said `other`, and the service read
  `browser_primitive_failed` where it would read `dialog_open` (`protocol4/link.rs`, the
  dialog refusal). `mapCallError` now keeps a coded failure's code when that code is also a call
  failure (`dialog_open`, `session_lost`, and the rest of `CALL`), counted as started like any
  failure that does not say; the set-up path already answers not started. Any other code still
  reads as text. Proofs, in `extension/tests/world.test.js`: "a dialog that opens while a world is
  set up answers dialog_open, not started" (the message never reaches the page) and
  "mapCallError keeps the call code of a coded failure, and reads any other code as text". It
  failed with `other` before the fix.
- P-D1, live check: not run. test-02 holds the 2.0.1 candidate as the coordinator asked, and
  loading this extension into the owner's Chromium changes its browser data; the coordinator
  decides when the package lane's live check runs.
- EXT-7: `buffers.settle(reason)` replaces `settleBoundaryState`, and the worker's three
  boundaries call it: the service's going (`service_disconnected`), a held or ended runtime
  (`runtime_held`) and a person's release of the debugger sessions (`browser_detached`). The
  worker's `DIAGNOSTIC_DOMAINS` and `disableDiagnosticCapture` go, and it no longer interrupts
  recordings or clears diagnostics rings itself. The held and released paths now also forget the
  read bindings, as the service boundary did; the skeptic found the difference close to inert
  (at most one more ring reset on a later read). The worker still names CDP domains where it
  routes events, as the skeptic corrected. `onExpired` is per tab, so it calls
  `buffers.captureExpired(tabId)`, which turns that tab's capture off as before and keeps its
  binding. The stop reason `service_disconnected` moves from the buffers' table to the worker's,
  its one user; EXT-5 gives the stop reasons one home. Proofs, in `extension/tests/buffers.test.js`:
  "a held runtime settles with its own stop reason, and the read binding goes with the ring" and
  "an expired ring turns off its tab's capture".
- EXT-8: `tabs.js` and `capture.js` take `invalid`, `isObject`, `requireTabId`, `requireText`,
  `requireArgs` and `requireInteger` from `arguments.js`; their copies go, and so do `tabs.js`'s
  `requireDuration` and `capture.js`'s `requirePositiveInteger`, which were `requireInteger` with
  other words. Two local checks stay because they are not copies: `requireOptionalBoolean`
  (`null` is refused, where `absent` would let it pass) and `capture.js`'s `requireNumber` (an
  exclusive minimum). The one `requireTab` is `arguments.requireTab(chrome.tabs, tabId)`; the two
  copies differed only in bounding Chrome's message to 500 or 200 characters, and every receipt
  message is bounded to 500 anyway (`primitives.js`, `execute`), so it bounds to 500. The refusal
  words of a bad tab id, a bad wait or a bad capture budget change to the shared ones, an advisory
  text no golden pins (the `open_tab.invalid_request` receipt golden still matches). Proofs:
  "a tab primitive refuses a bad tab id and wait in the shared words" in `tabs.test.js` and "a
  capture refuses a bad tab id and budget in the shared words" in `capture.test.js`, which read the
  old words before this change.
- EXT-13: `script-evaluator.js` folds into `evaluate.js`, which takes the parser (`vendor/acorn.js`)
  itself. A script that does not parse throws `invalid_request` directly, and a script the page
  threw on answers `Evaluated` with its `exception`, so the private codes `invalid_script` and
  `primitive_failed` go, and so does `effectUnknown`, which no production code read: once the
  evaluation is sent, `fx.dispatched` is what says its effect may be uncertain, and the receipt
  carries it. The form selection, the REPL flags, the hashbang and coordinate handling and the
  deadline are unchanged; parsing still comes before the lease, and the lease is now one plain
  bracket. The worker imports one file fewer. The h3 regressions in
  `extension/tests/evaluate.test.js` now run through the `evaluate_main` primitive and read the
  receipt: a parse failure is `invalid_request`, not dispatched, with nothing sent; a thrown script
  or a lost reply is dispatched and sent once. The import-order test runs the worker's real import
  list and evaluates through `GhostlightEvaluate`.
- EXT-13, owed to docs: `docs/design/adapter-protocol-4.md` says the protocol-4 design kept the
  script evaluator unchanged as its own module; it is now part of `evaluate.js`. The README's file
  list is EXT-15's; its line naming `script-evaluator.js` is corrected here. Three historical
  documents linked to the removed file (`docs/design/security-assessment-2026-09-06.md` twice,
  `docs/tasks/security-hardening/h3-script-effect-truth.md` once); each link now points at
  `evaluate.js`, keeps the old name, and says where the code went.
- EXT-14: `lifecycle.withLease(tabId, work)` in `sessions.js` and `world.withHidden(tabId, work)`
  replace the hand-written brackets: 14 of the 15 lease brackets in eight modules (`evaluate.js`
  fits now that EXT-13 took its lease out of the evaluator's send, and the dialog status leases
  inside its `if`, as before), and both hidden brackets (`capture.js` and `buffers.js`), whose
  hold, page notice and show-again were identical. The one bracket left by hand is
  `tabs.js`'s navigate: it leases only when it discards a beforeunload prompt and counts the
  discard in the same bracket. Every module's `acquire` and `release` create options go: nothing
  injected them, and with them the brackets could have leased one way and released another.
  `holdHidden` and `releaseHidden` stay inside `world.js`; their test now runs through
  `withHidden`. Proofs: "withLease gives the lease back however its work ends" in
  `sessions.test.js` and "withHidden shows the presentation again when its work throws" in
  `world.test.js`.
- EXT-15: `lib/chunks.js`, `lib/protocol.js` and `lib/sessions.js` open with the one-line
  `// Ghostlight -- ...` role every other module has; the two longer docs stay inside their
  factories, as `arguments.js` keeps its own. The README's file list adds `lib/arguments.js`,
  `lib/popup-view.js`, `kit.css`, `options.css`, both vendor files with their licenses,
  `icons/` (which of them are source art the package leaves out) and the test files, which the
  package leaves out too. The three module headers are packaged, so the freeze record is
  regenerated; the README is not packaged.
- EXT-9, the table: `sessions.CDP` names every command Ghostlight sends, and its values are the
  allow list (`CDP_ALLOW`), so a command a module names but the list lacks cannot exist. The eight
  per-module tables go (`buffers`, `capture`, `evaluate`, `input`, `inventory`, `primitives`,
  `tabs` and `world`), and so do `sessions.js`'s own bare literals; the keys are the method names
  (`Page.captureScreenshot` is `CDP.PAGE_CAPTURE_SCREENSHOT`). The worker keeps naming the events it
  routes, which are not commands. Guard, not packaged: "no module names a CDP command but through
  the sessions table, and every key it uses exists" in `sessions.test.js`, with its negative
  controls (a literal, a mistyped key, a command in the worker).
- EXT-9, the classifier: `sessions.SESSION_LOST_TEXT` is the one text of a lost debugger session,
  read by the receipt classifier in `primitives.js` and by the world call classifier in
  `world.js`, whose copy had drifted. A world call that meets Chrome's "Session with given id not
  found." (an iframe session gone before its detach event) or this module's "debugger session was
  released" now answers `session_lost` and drops its cached handle, where it answered `other`.
  The service reads both alike today, as the skeptic found. The coded failure it also needed
  (`dialog_open` thrown inside the world setup) landed in P-D1. Proof: the `mapCallError` table
  test in `world.test.js`, which reads `other` for both texts before this change.
- EXT-10: `lib/messages.js` holds the words the extension's parts send each other and nothing
  else: the page-to-worker kinds and the one broadcast (`PAGE`), the offscreen document's target
  and kinds (`OFFSCREEN`), the two forms an encoded GIF comes back in (`TRANSFER`) and the
  keyboard command (`COMMAND`). The worker, `popup.js`, `setup.js`, `options.js` (whose bare
  literals go), `offscreen.js`, `lib/buffers.js` and `lib/setup-view.js` read it, and the four
  pages and the worker load it. `manifest.json` cannot import it and keeps its one copy of
  `toggle-hold`; "the manifest declares the commands messages.js names" ties the two. "every page
  whose script reads GhostlightMessages loads messages.js before it" fails when a page drops the
  tag (checked against `popup.html`). The `toggle_hold` intent the worker sends the service is a
  protocol word, not one of these, so it waits for EXT-5's names in `protocol.js`.
- EXT-11: `sessions.js`, which holds a tab's open dialog, offers the one watch,
  `lifecycle.watchDialog(tabId)`, whose `opened` resolves with the dialog's type, and
  `lifecycle.requireNoDialog(tabId)`. The world's own registry (`dialogWaiters`, `watchForDialog`,
  `announceDialog`) goes, so the worker opens a dialog with one call where it made two in step;
  the world's call race and `input.js` wait on the sessions watch, and the five copies of "read
  `currentDialog`, then throw `dialogOpen()`" (`capture.js`, `evaluate.js`, `input.js` and twice
  `inventory.js`) are `requireNoDialog`. The sessions waiters now carry the type, as the skeptic
  said they would need to. One behavior is new and safer: a watch started while a dialog is
  already open resolves at once, where the world's watch heard only announcements made after it
  began and an announcement that came between a caller's check and its watch went unheard until
  the deadline. Three tests that announced a dialog to the world alone now open it through
  `lifecycle.openDialog`, as the worker does, and the golden receipts pass unchanged. Proof: "a
  dialog watch wakes with the type when a dialog opens, and at once when one is already open" in
  `sessions.test.js`.
- C18 (cross-lane, moved from core-service by the coordinator): `language/presentation.rs` holds
  the one table of the words the page shows, `activity_label` for the 17 activities and
  `denial_lines` for the phase and detail of a blocked job, beside the held-tab lines it already
  had. `presentation/mod.rs` reads it, and its `activity_label` and `denial_copy` go. The page
  draws only the service's words: the caption shows `signal.label`, or nothing when a frame has
  none, and the denial shows `signal.phase` and `signal.detail` without its two fallback
  sentences. `page-shared.js` loses `activityLabel` and the unused `presentationLabel`.
  `presentation.js` names the kinds and activities it draws in two frozen tables, `SIGNAL` and
  `ACTIVITY`, which `render` and the signature code read in place of 33 literals, so `golden.rs`
  can pin them. Five pins in `protocol4/golden.rs` read the JavaScript and edit none of it:
  `every_presentation_name_the_service_sends_is_one_the_page_draws` (both tables equal the wire's
  `PresentationKind` and `PresentationActivity`),
  `every_area_and_tier_the_page_names_is_one_the_service_decodes` (`content.js`'s `AREA_WORDS`
  and `LEFT_OUT` equal `PageArea::ALL` and `LeftOutKey::ALL`),
  `every_stop_reason_the_extension_sends_is_one_the_service_decodes` (the `STOP_REASON` tables
  of `worker.js` and `lib/buffers.js` and `recording.js`'s literals decode to a known
  `RecordingStopReason`), and EXT-5's two pins,
  `the_runtime_states_the_extension_reports_are_the_services_and_disconnected` and
  `the_extensions_chunk_limits_equal_the_services` (`lib/chunks.js`'s 512 KiB, 8 MiB and 64
  equal `wire.rs`'s). No packaged file changes. The extension's own `activityLabel` in
  `lib/shared.js` stays: it is packaged and frozen. S5, in the surface lane, points
  `workbench/mod.rs` at `activity_label` and deletes its copy. Proofs for the page: "the page
  caption is the word the service sent" and "a denial draws the phase and detail the service sent
  and invents neither" (`assets/tests/world.test.js`). The service half is
  `every_notice_carries_the_label_of_its_activity` (`presentation/mod.rs`), now against the
  language table. The asset id is regenerated on purpose (`tests/golden/protocol4/assets.sha256`).
  Also corrected: the asset-set export list in `docs/design/adapter-protocol-4.md` and the wire
  comment on `NoticeSignal.label`.
- P-D2 (a defect, found on leo-desktop-02's store 2.0.0: four `browser_navigate new_tab` calls
  stalled behind a held bell prompt, answered unknown at their deadline, and then all ran when
  the prompt closed, opening tabs and a window nobody wanted any more): the service does send
  `cancel` at the deadline (`link.rs`), but the extension looked at it once, in `execute` before
  custody, and in the ready wait. A command that passed that check and then waited ran its effect
  when its turn came. Two waits had no check after them: `controlled.manage`, which queues behind
  a stalled attach, and the `serialized` chain of `tabs.js` that every open and adoption takes. Now
  `execute` looks again after custody, and `serialized` looks when a request's turn comes,
  before it touches the browser. An adoption's `dispatched` is set in its turn, as a new tab's
  already was. The cancelled sentence keeps its three copies (`primitives.js`, `tabs.js`,
  `world.js`); EXT-5's failure builder in `protocol.js` is where it becomes one. Left: a cancel is
  best effort (`Link::cancel` leaves a writer busy with another transfer alone), and a request
  carries no deadline the extension could check without one, which would be a protocol change.
  Proofs in `primitives.test.js`: "a tab cancelled while it waits behind another open is never
  created", "a tab adoption cancelled while it waits behind an open never moves or loads the tab"
  and "a navigate cancelled while it waits for custody of its tab never loads the page", each
  failing before the fix; in `tabs.test.js`, "an open cancelled before its turn creates nothing and
  dispatches nothing". "a cancel during the ready wait ends it with cancelled and dispatched stays
  true" now cancels once the tab exists, the moment it names. The freeze record is regenerated
  (a test-02 package line, like the others before B-final).
- EXT-5: `protocol.js` derives a frozen name map from each closed list it needs (`FRAME`,
  `EVENT`, `RECEIPT_FAILURE`, `CALL_FAILURE`, `RUNTIME_STATE`, `RECORDING_STOP_REASON`) and owns
  `failure`, `dialogOpen` and `DIALOG_OPEN_MESSAGE`, which leave `sessions.js` with no shim.
  `sessions.js` and `chunks.js` take `protocol` as a dependency, as do `capture.js` and
  `evaluate.js`; `arguments.js` no longer needs `sessions.js`. The copies the finding counted
  become references: the failure tables of `arguments`, `buffers`, `capture`, `evaluate`, `input`,
  `inventory`, `primitives`, `tabs` and `world`, the worker's frame, event, runtime-state,
  stop-reason and failure tables, `buffers.js`'s stop reasons, `primitives.js`'s receipt kind,
  `world.js`'s `disconnected`, the three `"unknown"` dialog types and `chunks.js`'s three
  defaults. `vocabulary.json` gains four keys and changes none: `runtime_states`,
  `recording_stop_reasons` (the service's closed set, `frame_too_large` included),
  `unknown_dialog_type` and `chunk_limits`. `vocabulary-2.0.0.json` is untouched. In `golden.rs`
  C18's three readings of the worker, `world.js` and `chunks.js` now read the vocabulary, and a
  fifth pin, `the_unknown_dialog_type_is_the_one_the_receipts_carry`, joins them. Not taken:
  `shared.js` and `recording.js` keep their few protocol words (shared's frame kinds and
  readiness words, recording's three stop reasons), because the options, setup and popup pages and
  the offscreen document load them without `protocol.js`. `recording.js`'s literals stay pinned by
  reading in `every_stop_reason_the_extension_sends_is_one_the_service_decodes`. The other local
  tables (steps, clips, ops, destinations) are outside the finding. The cancelled sentence keeps
  its three copies; a receipt-text table is a later slice. Later, additive (P-D2's note): a request
  deadline field. The freeze record is regenerated (a test-02 package line).
- P-D3, the owner's request of 2026-10-21: the ghost emoji (U+1F47B) replaces `Ghostlight - ` in a
  group's title (ADR-0098 amendment of 2026-10-21). `vocabulary.json` gains two keys and changes
  none: `group_title_prefix`, which the service's `GROUP_TITLE_PREFIX` pins, and
  `legacy_group_title_prefixes`, which only the extension reads. `protocol.legacyGroupTitles` gives
  the titles 2.0.0 gave the same agent's group. `tabs.js` retitles the lowest-id one when no group
  carries the current title and merges any other into the current group, and never retitles a
  group with another title. `notice.js` names the popup's agent through `protocol.groupAgentName`
  after either prefix. The `open_tab` fixtures carry the ghost title, so the extension's group
  tests run on it and only the legacy tests use the old one. The freeze record is regenerated from a
  local package; B-final takes it from the release workflow's candidate. The owner checked the tab
  strip on Windows on 2026-10-21 ("it's working beautifully"), and test-01 (CachyOS) and test-03
  (Alpine) saw the color ghost through Noto Color Emoji. Bluefin and leo-desktop-02 are owed. Both
  Linux browsers run the store 2.0.0 extension, so neither saw an old group renamed; the rename
  rests on its tests until a machine runs the local build.
- P-D4, found while doing P-D3: `arguments.requireText` counted a text's `length`, in UTF-16 code
  units, while the service's checks count `chars()`, code points. A character outside the Basic
  Multilingual Plane, such as an emoji, counts twice in the extension and once in the service, so
  text the service accepted could be refused as `invalid_request`. The group title showed it: the
  service cuts the agent's name at 80 characters, and an emoji name of 59 or more made a title over
  the extension's 120, so that agent could open no tab. `browser_find`'s name and
  `browser_diagnose`'s match (500 on both sides) and a recording's file name (255) had the same
  gap. `requireText` now counts code points, and only when the text's `length` is over the limit.
  The 120 moves into the vocabulary as the top-level `group_title_chars` (a key inside `limits`
  would change 2.0.0's `limits`, which the compatibility test forbids, as with EXT-5's
  `chunk_limits`), and `golden.rs` pins that the longest title the service composes fits it.
  `shared.bounded` still cuts by code units and can leave half an emoji at the end of a cut tab
  title, which drops the browser connection; P-D5 takes that.
- P-D4, found live on 2026-10-21 after it landed: `diagnostics.read` checks `match_text` a second
  time, by its `length`, so a 260-emoji `browser_diagnose` filter still came back as
  `invalid_request` after `requireText` passed it. That check counts code points now too, and
  `diagnostics.test.js` holds the 260-emoji case. No other check of agent text counts code units:
  the remaining `length` checks are on arrays and on ids the extension makes.
- P-D5, found live on the owner's machine on 2026-10-21 while checking P-D4's note: a script set
  a tab's title to 499 letters, an emoji and a letter, and the next call failed with "Connection
  lost before the browser confirmed completion." The session lost the tab. The cut to 500 code
  units left half the emoji. V8 writes a lone surrogate as a JSON escape, serde_json refuses it,
  and the service's read loop ended the whole connection on that one frame, so every session's
  browser went with it. Any cut the extension makes in text can leave such a half: `evaluate.js`
  cuts a script's result at `max_result_chars` and its exception text, and `diagnostics.js` cuts
  console messages, so a script that returns text with emoji could drop the connection too. The
  fix has two halves. In the extension, `shared.bounded` cuts before the first half of a pair, and the
  worker's `send` passes every frame through `shared.wellFormed`, which replaces a lone surrogate
  with U+FFFD and returns the frame itself when nothing needs it, which covers every cut, whichever
  made the half (a cut other than `shared.bounded` may end in U+FFFD). In the service,
  `connection::Reading::run` drops a frame whose JSON it cannot decode and reads on, since the frame
  was read whole and the stream is still aligned. That half covers the 2.0.0 extension the store
  still serves: its frame is lost, so that call waits out its deadline instead of every session
  losing its browser. The packaged `lib/shared.js` and `worker.js` change, and the freeze record is
  regenerated. Owed: the tab the check opened is left open with the long title in the owner's
  Chrome, outside any session.
- P-D6, found on 2026-10-21 (core-service S-D8 has the paths): the worker's `send` measures each
  frame's UTF-8 length against the relay bound before it posts it. An oversized receipt becomes a
  failure receipt with its correlation, `dispatched`, committed URLs and tab, so its effect
  semantics survive, under one new additive receipt word, `answer_too_large`; an older service
  decodes it as `other` through the tolerance rule. The service gives it its own refusal and next
  step: ask for fewer characters or items, or name one document. Any other oversized frame (an
  event, a hello) is dropped, not sent. Later, not in 2.0.1: the service plans a read, an inspect
  or a find across documents by its worst-case size and splits it over several `call` primitives,
  so a normal page never reaches the refusal.
