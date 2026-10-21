# arch-1: package ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

B2 and B1 have landed, and so has the top-frame diagnostics ring reset for unbound tabs (see
Notes). B1 rides in the same candidate as C21, or neither does.

The second sweep (2026-10-20) opens this lane again: see its section below. It started while
adapter waits for C17 (the coordinator's direction). P-D1 and EXT-7 have landed; EXT-8 is next.

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
with its skeptic's corrections. P-D1 and EXT-7 have landed; EXT-8 is next.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| P-D1 | fix(extension): a dialog that opens during world setup reads `dialog_open` | - | EXT-9 as corrected | landed (this commit) |
| EXT-5 | refactor(extension): `protocol.js` names its words and owns `failure`, and the vocabulary gains the missing words under new keys | C18 | EXT-5 as corrected | todo |
| EXT-6 | refactor(extension): each module exports its own handler spec through one `handler()` | EXT-5 | EXT-6 | todo |
| EXT-7 | refactor(extension): one `buffers.settle(reason)` for every boundary | - | EXT-7 | landed (this commit) |
| EXT-8 | refactor(extension): the argument checks come from `arguments.js` | - | EXT-8 | todo |
| EXT-9 | refactor(extension): one CDP method table and one session-lost classifier | P-D1 | EXT-9 | todo |
| EXT-10 | refactor(extension): one module of the extension's own message words | - | EXT-10 | todo |
| EXT-11 | refactor(extension): sessions owns the dialog watch | EXT-9 | EXT-11 | todo |
| EXT-13 | refactor(extension): `script-evaluator.js` folds into `evaluate.js` | - | EXT-13 | todo |
| EXT-14 | refactor(extension): `withLease` and `withHidden` | - | EXT-14 | todo |
| EXT-12 | refactor(extension): the native link moves into `lib/link.js` | EXT-5, EXT-7, EXT-10 | EXT-12 | todo |
| EXT-15 | docs(extension): module docs, and the README lists every file | - | EXT-15; the README is not packaged | todo |
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
